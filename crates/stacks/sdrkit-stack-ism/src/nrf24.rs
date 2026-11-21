//! nRF24L01 Enhanced ShockBurst (`nrf24`): bursts on one 2.4 GHz channel, read at 250 kbit/s, 1 and 2 Mbit/s, and frames found by
//! trying start positions and address lengths against the CRC. Content a device encrypts is shown only as bytes.
//!
//! Standard: Nordic nRF24L01+ Product Specification 1.0, clause 7.3 (Enhanced ShockBurst packet format).

use crate::family::{hex, min_rate_message, set_status, CleanScene, REPORT_S};
use crate::typed::{
    NrfAddressEvent, NrfAddressRow, NrfBadBurstRow, NrfFields, NrfFrameRow, NRF_ADDRESS,
    NRF_ADDRESSES, NRF_ADDRESS_ROW, NRF_BAD_BURSTS, NRF_FRAMES, NRF_REMOVE,
};
use sdrkit_algo_dsp::fir::{centred_convolve, design_by_taps};
use sdrkit_algo_dsp::floor::MinFollower;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_algo_dsp::stats::WindowSum;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::gmsk::{discriminator_taps, Mlse};
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, ReadFields, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::AutoGain;
use sdrkit_core::SignalType;
use sdrkit_core::{
    Complex32, ConfigError, Event, GenConfig, Generator, HostRequest, LockProfile, LockState,
    Receiver, Record, Results, ResultsDecl, Setting, SettingError, SettingId, SettingSet,
    SignalInfo, StackConfig, StackDescriptor, StateBlob, StateError, TableId, Tuning, Value,
    Values,
};
use sdrkit_graph::Catalog;
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Decoder, DecoderComponent, Format,
    GraphBuilder, GraphError, Io, Item, Kept, Kind, Outbox, Port, RateFrom, SlicePool, StackGraph,
    Timed,
};
use sdrkit_util_bits::crc::catalog::{CRC16_IBM_3740, CRC8_ESB};
use sdrkit_util_bits::pack::{pack, push_u64, to_u64, unpack};
use sdrkit_util_bits::{BitOrder, Pad};
use sdrkit_util_objects::commands;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    fingerprint, Dedup, Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp, Tables,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::sync::Arc;

/// The rates: (bit rate, deviation, filter cut-off, smoothing in bits), in the order they are tried.
pub const RATES: [(f64, f64, f64, f64); 3] = [
    (1e6, 160e3, 900e3, 1.0),
    (2e6, 320e3, 1.5e6, 1.0),
    (250e3, 160e3, 450e3, 0.5),
];
/// The least sample rate.
pub const MIN_RATE: f64 = 4e6;
/// The frames kept in the log.
pub const FRAME_CAP: usize = 300;
/// The addresses kept.
pub const ADDRESS_CAP: usize = 100;
/// The bursts without a frame kept.
pub const BAD_CAP: usize = 60;
/// The time constant of the burst detector's power, s (6 / 450 kHz).
const TAU_S: f64 = 6.0 / 450e3;

/// The bits of a frame (one bit a byte): preamble, address, the 9-bit control field (length, PID, NO_ACK), payload and CRC
/// (8 or 16 bits, CRC-8/ESB or CRC-16/IBM-3740 over the bits from the address on).
pub fn frame_bits(addr: &[u8], pid: u8, no_ack: bool, payload: &[u8], crc16: bool) -> Vec<u8> {
    let mut b = Vec::with_capacity(8 * (addr.len() + payload.len() + 4) + 9);
    unpack(addr, BitOrder::Msb, &mut b);
    push_u64(&mut b, payload.len() as u64 & 0x3F, 6);
    push_u64(&mut b, pid as u64 & 3, 2);
    b.push(no_ack as u8);
    unpack(payload, BitOrder::Msb, &mut b);
    if crc16 {
        let c = CRC16_IBM_3740.bits(&b);
        push_u64(&mut b, c as u64, 16);
    } else {
        let c = CRC8_ESB.bits(&b);
        push_u64(&mut b, c as u64, 8);
    }
    let pre = if b[0] == 1 { 0xAAu8 } else { 0x55 };
    let mut out = Vec::with_capacity(b.len() + 8);
    unpack(&[pre], BitOrder::Msb, &mut out);
    out.extend(b);
    out
}

/// A frame found by the sniffer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EsbFrame {
    /// The address, first byte first.
    pub addr: Vec<u8>,
    /// The PID.
    pub pid: u8,
    /// The NO_ACK bit.
    pub no_ack: bool,
    /// The payload.
    pub payload: Vec<u8>,
    /// The CRC bits (8 or 16).
    pub crc_bits: u8,
    /// The bit rate, bit/s.
    pub rate: f64,
    /// The first address bit.
    pub start: usize,
    /// The bits from the address to the CRC.
    pub nbits: usize,
    /// The SNR, dB.
    pub snr_db: f32,
    /// The carrier offset from the channel, Hz.
    pub offset_hz: f32,
}

fn bits_value(bits: &[u8], from: usize, n: usize) -> u64 {
    to_u64(&bits[from..from + n])
}

fn packed(bits: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(bits.len() / 8);
    pack(bits, BitOrder::Msb, Pad::Zero, &mut v);
    v
}

fn overlaps(a: &EsbFrame, b: &EsbFrame) -> bool {
    a.start < b.start + b.nbits && b.start < a.start + a.nbits
}

/// Finds the frames in `bits`: start positions 1 to 16 after an alternating preamble, address lengths 5, 4 and 3, then the
/// CRC-16 and the CRC-8 (the public sniffer method). CRC-16 frames come first.
pub fn sniff(bits: &[u8]) -> Vec<EsbFrame> {
    let mut c16: Vec<EsbFrame> = Vec::new();
    let mut c8: Vec<EsbFrame> = Vec::new();
    for s in 6..=16usize {
        if s + 8 * 3 + 9 + 8 > bits.len() {
            continue;
        }
        if !(s - 6..s - 1).all(|k| bits[k] != bits[k + 1]) || bits[s] == bits[s - 1] {
            continue;
        }
        for al in [5usize, 4, 3] {
            let p = s + 8 * al;
            if p + 9 > bits.len() {
                continue;
            }
            let len = bits_value(bits, p, 6) as usize;
            if len > 32 {
                continue;
            }
            let n = 8 * al + 9 + 8 * len;
            let mk = |crc: u8| EsbFrame {
                addr: packed(&bits[s..s + 8 * al]),
                pid: bits_value(bits, p + 6, 2) as u8,
                no_ack: bits[p + 8] != 0,
                payload: packed(&bits[p + 9..p + 9 + 8 * len]),
                crc_bits: crc,
                rate: 0.0,
                start: s,
                nbits: n + crc as usize,
                snr_db: 0.0,
                offset_hz: 0.0,
            };
            if s + n + 16 <= bits.len()
                && CRC16_IBM_3740.bits(&bits[s..s + n]) as u64 == bits_value(bits, s + n, 16)
            {
                let f = mk(16);
                if !c16.iter().any(|o| overlaps(o, &f)) {
                    c16.push(f);
                }
            } else if s + n + 8 <= bits.len()
                && CRC8_ESB.bits(&bits[s..s + n]) as u64 == bits_value(bits, s + n, 8)
            {
                c8.push(mk(8));
            }
        }
    }
    c8.retain(|f| !c16.iter().any(|o| overlaps(o, f)));
    c16.extend(c8);
    c16
}

/// One burst (`nrf.burst`): the samples mixed to the channel, from the pre-roll to the end.
#[derive(Clone, Debug)]
pub struct NrfBurst {
    /// The samples.
    pub samples: Arc<[Complex32]>,
    /// The sample rate.
    pub rate: f64,
    /// The index of the start of the burst (after the pre-roll).
    pub start: usize,
    /// The index of the end of the signal.
    pub end: usize,
    /// The SNR, dB.
    pub snr_db: f32,
}

impl Item for NrfBurst {
    const KIND: &'static str = "nrf.burst";
}

/// The triangular low-pass of the spec: two running means of `l` samples in a row.
struct Triangle {
    a: WindowSum<Complex32>,
    b: WindowSum<Complex32>,
    l: f32,
}

impl Triangle {
    fn new(l: usize) -> Triangle {
        let l = l.max(1);
        Triangle {
            a: WindowSum::new(l).expect("a positive length"),
            b: WindowSum::new(l).expect("a positive length"),
            l: l as f32,
        }
    }
    fn push(&mut self, x: Complex32) -> Complex32 {
        let a = self.a.push(x);
        let y1 = Complex32::new(a.re as f32, a.im as f32) / self.l;
        let b = self.b.push(y1);
        Complex32::new(b.re as f32, b.im as f32) / self.l
    }
}

/// The burst detector (`nrf.burst`): the channel mixed to 0 Hz, the triangular low-pass, the smoothed power over a floor, and a
/// pre-roll ring.
#[derive(Default)]
pub struct BurstDet {
    rate: f64,
    nco: Option<Nco>,
    tri: Option<Triangle>,
    ring: Option<SampleRing<Complex32>>,
    pool: SlicePool<Complex32>,
    floor: Option<MinFollower>,
    mixed: Vec<Complex32>,
    p: f32,
    n: u64,
    init: u64,
    init_sum: f64,
    open: bool,
    buf: Vec<Complex32>,
    act: Vec<f32>,
    peak: f32,
    quiet: u32,
    t0: f64,
    pre_len: usize,
    snr: f32,
}

impl Component for BurstDet {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "nrf.burst",
            vec![Port::iq("in")],
            vec![Port::typed::<NrfBurst>("bursts").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let rate = f.rate;
        self.rate = rate;
        self.nco = Some(Nco::new(rate, -f.signal_offset_hz()));
        self.tri = Some(Triangle::new((0.32 * rate / 450e3).round() as usize));
        let pre = ((70e-6 * rate) as usize).max(1);
        self.ring = SampleRing::new(pre).ok();
        self.floor =
            MinFollower::with_times(rate, 20e-6, 10.0 * std::f64::consts::LOG10_E / 50e-3).ok();
        self.init = (300e-6 * rate) as u64;
        self.n = 0;
        self.init_sum = 0.0;
        self.p = 0.0;
        self.open = false;
        let maxn = (5e-3 * rate) as usize + pre;
        self.buf = Vec::with_capacity(maxn);
        self.act = Vec::with_capacity(maxn);
        let mut o = f.with_kind(Kind::typed::<NrfBurst>());
        o.rate = 0.0;
        Ok(vec![o])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let first = io.input(0).first();
        let rate = self.rate;
        let k_p = 1.0 - (-1.0 / (TAU_S * rate)).exp() as f32;
        let pre = (70e-6 * rate) as usize;
        let quiet_n = (8e-6 * rate) as u32;
        let maxn = (5e-3 * rate) as usize;
        let (Some(nco), Some(tri), Some(ring), Some(floor)) = (
            self.nco.as_mut(),
            self.tri.as_mut(),
            self.ring.as_mut(),
            self.floor.as_mut(),
        ) else {
            return;
        };
        self.mixed.resize(x.len(), Complex32::default());
        nco.mix(x, &mut self.mixed);
        for (k, &s) in self.mixed.iter().enumerate() {
            let y = tri.push(s);
            self.p += k_p * (y.norm_sqr() - self.p);
            self.n += 1;
            if !self.open {
                ring.push(s);
            }
            if self.n <= self.init {
                self.init_sum += self.p as f64;
                if self.n == self.init {
                    floor.push((self.init_sum / self.init as f64) as f32);
                }
                continue;
            }
            let fl = floor.floor().unwrap_or(self.p).max(1e-20);
            let act = self.p / fl;
            if self.open {
                self.buf.push(s);
                self.act.push(act);
                self.peak = self.peak.max(act);
                self.quiet = if act < 3.0 { self.quiet + 1 } else { 0 };
                if self.quiet >= quiet_n || self.buf.len() >= maxn + pre {
                    self.open = false;
                    let half = self.peak / 2.0;
                    let end = self
                        .act
                        .iter()
                        .rposition(|&a| a > half)
                        .map_or(self.buf.len(), |i| i + 1);
                    if end.saturating_sub(self.pre_len) as f64 / rate >= 20e-6 {
                        self.snr =
                            0.8 * self.snr + 0.2 * (10.0 * (self.peak - 1.0).max(1e-3).log10());
                        let buf = &self.buf;
                        let samples = self.pool.exact(buf.len(), |d| d.copy_from_slice(buf));
                        io.items_out::<NrfBurst>(0).push(Timed {
                            time: self.t0,
                            value: NrfBurst {
                                samples,
                                rate,
                                start: self.pre_len,
                                end,
                                snr_db: self.snr,
                            },
                        });
                    }
                    self.buf.clear();
                    self.act.clear();
                    ring.clear();
                }
                continue;
            }
            floor.push(self.p);
            if act > 6.0 {
                self.open = true;
                self.peak = act;
                self.quiet = 0;
                self.buf.clear();
                let have = ((ring.end() - ring.start()) as usize).min(pre);
                let from = ring.end() - have as u64;
                self.buf.resize(have, Complex32::default());
                ring.copy(from, &mut self.buf);
                self.pre_len = have;
                self.act.clear();
                self.act.resize(have, 0.0);
                self.t0 = (first + k as u64 + 1) as f64 / rate - have as f64 / rate;
            }
        }
    }
}

/// Taps of the channel low-pass per (sample rate / cut-off): about four periods of the cut-off.
const CHANNEL_TAPS_PER_CUT: f64 = 4.0;
/// The bit phases tried for the bit instants of a burst.
const TIMING_PHASES: usize = 8;
/// The most bits read from one burst: the longest burst (5 ms and the pre-roll) at 2 Mbit/s, and some margin.
const MLSE_STEPS: usize = 10_400;

/// The demodulator of one burst at all rates, with its working buffers.
pub struct Demod {
    full: Vec<Complex32>,
    y: Vec<Complex32>,
    f: Vec<f32>,
    s: Vec<f32>,
    pw: Vec<f32>,
    scratch: Vec<f32>,
    bits: Vec<u8>,
    inverted: Vec<u8>,
    z: Vec<Complex32>,
    taps: Vec<Complex32>,
    mlse: Option<Mlse>,
}

impl Default for Demod {
    fn default() -> Self {
        // BT 0.5 at every rate: the intersymbol interference a one-bit integrate-and-dump leaves on the discriminator.
        let taps = discriminator_taps(0.5)
            .unwrap_or_else(|_| vec![1.0])
            .into_iter()
            .map(|t| Complex32::new(t, 0.0))
            .collect();
        Demod {
            full: Vec::new(),
            y: Vec::new(),
            f: Vec::new(),
            s: Vec::new(),
            pw: Vec::new(),
            scratch: Vec::new(),
            bits: Vec::new(),
            inverted: Vec::new(),
            z: Vec::new(),
            taps,
            mlse: None,
        }
    }
}

impl Demod {
    /// Reads burst `b` at rate `RATES[ri]` into `self.bits` by MLSE over the discriminator levels (the caller tries both
    /// polarities). Returns the mid level (the carrier offset, Hz), or `None` when the burst has too few bits or too small a swing.
    fn bits_at(&mut self, b: &NrfBurst, ri: usize) -> Option<f32> {
        let (br, dev, cut, sm) = RATES[ri];
        let d = ((b.rate / (12.0 * br)).floor() as usize).max(1);
        let r2 = b.rate / d as f64;
        // The rate's channel low-pass (centred, so the burst indices stay), then one sample of every d.
        let taps = ((CHANNEL_TAPS_PER_CUT * b.rate / cut).round() as usize).max(7) | 1;
        let h = design_by_taps(taps, (cut / b.rate).min(0.5), Window::Hamming).ok()?;
        self.full.resize(b.samples.len(), Complex32::default());
        centred_convolve(&b.samples, &h, &mut self.full).ok()?;
        self.y.clear();
        self.y.extend(self.full.iter().step_by(d).copied());
        let mut disc = Discriminator::try_new(r2, 1.0).ok()?;
        self.f.clear();
        for &v in &self.y {
            self.f.push(disc.process_sample(v));
        }
        if let Some(f0) = self.f.first_mut() {
            *f0 = 0.0;
        }
        let w = ((sm * r2 / br).round() as usize).max(1);
        let mut boxcar = WindowSum::<f32>::new(w).ok()?;
        self.s.clear();
        for &v in &self.f {
            self.s.push((boxcar.push(v) / w as f64) as f32);
        }
        let a = b.start / d;
        let e = (b.end / d).min(self.s.len());
        if e <= a + 4 {
            return None;
        }
        self.pw.clear();
        self.pw.extend(self.y.iter().map(|c| c.norm_sqr()));
        let mean = self.pw[a..e].iter().sum::<f32>() / (e - a) as f32;
        // The percentiles use only the values whose whole smoothing window has signal. Noise alone produces wild frequencies.
        self.f.clear();
        self.f.extend(
            (a.max(w - 1)..e)
                .filter(|&i| self.pw[i] > mean / 2.0 && self.pw[i + 1 - w] > mean / 2.0)
                .map(|i| self.s[i]),
        );
        if self.f.len() < 4 {
            return None;
        }
        let lo = percentile(&self.f, 0.04, &mut self.scratch);
        let hi = percentile(&self.f, 0.96, &mut self.scratch);
        if hi - lo < 0.6 * dev as f32 {
            return None;
        }
        let mid = (lo + hi) / 2.0;
        // Start where the power first exceeds half the burst's mean power.
        let win = ((0.5 * r2 / br).max(3e-6 * r2)).round().max(1.0) as usize;
        let st = (a.saturating_sub(win)..e).find(|&i| {
            self.pw[i..(i + win).min(self.pw.len())].iter().sum::<f32>() / win as f32 > mean / 2.0
        })?;
        let st = st.saturating_sub(win / 2);
        let spb = r2 / br;
        // The last bit ends at the signal end: read on for two more bits.
        let (body, e) = (
            a.max(st) as f64..e as f64,
            (e + (2.0 * spb) as usize).min(self.s.len()),
        );
        let level = |s: &[f32], t: f64| -> Option<f32> {
            let i = t as usize;
            let fr = (t - i as f64) as f32;
            (i + 1 < e).then(|| s[i] * (1.0 - fr) + s[i + 1] * fr)
        };
        // The bit instants: the phase in the bit, of TIMING_PHASES tried, where the levels inside the burst lie furthest from the
        // mid level (each level counts up to the half spacing, so the noise around the burst cannot win).
        let half = (hi - lo) / 2.0;
        let open = |t0: f64| (0..).map(move |k| t0 + k as f64 * spb);
        let phase = (0..TIMING_PHASES)
            .map(|p| st as f64 + p as f64 * spb / TIMING_PHASES as f64)
            .max_by(|&p, &q| {
                let eye = |t0: f64| -> f32 {
                    open(t0)
                        .skip_while(|t| *t < body.start)
                        .take_while(|t| body.contains(t))
                        .filter_map(|t| level(&self.s, t))
                        .map(|v| (v - mid).abs().min(half))
                        .sum()
                };
                eye(p).total_cmp(&eye(q))
            })?;
        // The levels at the bit instants, scaled so a long run of one bit reads +-(the sum of the taps), and +1 for bit 0.
        let gain = self.taps.iter().map(|t| t.re).sum::<f32>();
        // The bits start at the first change of level after the start: the preamble.
        let up = self.s[st] > mid;
        let edge = (st + 1..e).find(|&k| (self.s[k] > mid) != up)? as f64;
        self.z.clear();
        self.z.extend(
            open(phase)
                .skip_while(|t| *t < edge)
                .map_while(|t| level(&self.s, t))
                .take(MLSE_STEPS)
                .map(|v| Complex32::new((mid - v) / half * gain, 0.0)),
        );
        let n = self.z.len();
        if n < 40 {
            return None;
        }
        // MLSE output k is the bit of level k + K (K the taps on each side); the first K bits come from the sign of their level.
        let k = self.taps.len() / 2;
        if self.mlse.is_none() {
            self.mlse = Some(Mlse::new(&self.taps, MLSE_STEPS).ok()?);
        }
        let mlse = self.mlse.as_mut()?;
        self.bits.clear();
        self.bits
            .extend(self.z[..k].iter().map(|v| (v.re < 0.0) as u8));
        self.bits.resize(n, 0);
        mlse.decode(&self.z[..n - k], 1.0, None, &mut self.bits[k..])
            .ok()?;
        Some(mid)
    }

    /// The candidate frames of burst `b`: the CRC-16 frames of the first rate that has one, else the CRC-8 candidates, each
    /// with the length check.
    pub fn burst_frames(&mut self, b: &NrfBurst) -> Vec<EsbFrame> {
        let dur = b.end.saturating_sub(b.start) as f64 / b.rate;
        let mut c8 = Vec::new();
        for (ri, rate) in RATES.iter().enumerate() {
            let Some(mid) = self.bits_at(b, ri) else {
                continue;
            };
            let br = rate.0;
            let mut found16 = Vec::new();
            self.inverted.clear();
            self.inverted.extend(self.bits.iter().map(|&x| 1 - x));
            for bits in [&self.bits, &self.inverted] {
                for mut f in sniff(bits) {
                    let expected = (f.nbits + 8) as f64 / br;
                    if (dur - expected).abs() > 0.2 * expected + 3.0 * TAU_S + 6e-6 {
                        continue;
                    }
                    f.rate = br;
                    f.snr_db = b.snr_db;
                    f.offset_hz = mid;
                    if f.crc_bits == 16 {
                        found16.push(f);
                    } else {
                        c8.push(f);
                    }
                }
            }
            if !found16.is_empty() {
                found16.dedup_by(|a, b| a.addr == b.addr && a.payload == b.payload);
                return found16;
            }
        }
        c8.dedup_by(|a, b| a.addr == b.addr && a.payload == b.payload);
        c8
    }
}

/// The result of a burst (`nrf.frame`).
#[derive(Clone, Debug, PartialEq)]
pub struct BurstResult {
    /// The candidate frames.
    pub frames: Vec<EsbFrame>,
    /// The duration of the burst, µs.
    pub dur_us: f64,
    /// The SNR, dB.
    pub snr_db: f32,
}

impl Item for BurstResult {
    const KIND: &'static str = "nrf.frame";
}

impl Decoder for Demod {
    type Input = Timed<NrfBurst>;

    fn name(&self) -> &'static str {
        "nrf.demod"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<BurstResult>("out").rate_from(RateFrom::Fixed(0.0))]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        let mut o = input.with_kind(Kind::typed::<BurstResult>());
        o.rate = 0.0;
        Ok(vec![o])
    }
    fn process(&mut self, x: &[Timed<NrfBurst>], _first: u64, out: &mut Outbox) {
        for b in x {
            let v = &b.value;
            let frames = self.burst_frames(v);
            out.items::<BurstResult>(0).push(Timed {
                time: b.time,
                value: BurstResult {
                    frames,
                    dur_us: v.end.saturating_sub(v.start) as f64 / v.rate * 1e6,
                    snr_db: v.snr_db,
                },
            });
        }
    }
}

/// A row of the frame log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRow {
    /// The signal time, s.
    pub time_s: f64,
    /// The bit rate, bit/s.
    pub rate: f64,
    /// The address.
    pub addr: Vec<u8>,
    /// The PID.
    pub pid: u8,
    /// The NO_ACK bit.
    pub no_ack: bool,
    /// The CRC bits.
    pub crc_bits: u8,
    /// The payload.
    pub payload: Vec<u8>,
    /// The SNR, dB.
    pub snr_db: f32,
    /// The same frame again in a row within 2 s (retransmissions).
    pub repeats: u32,
}

impl Shift for FrameRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time_s += by_s;
    }
}

/// An address heard.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AddrRow {
    /// The address.
    pub addr: Vec<u8>,
    /// The bit rate, bit/s.
    pub rate: f64,
    /// The CRC bits.
    pub crc_bits: u8,
    /// The last payload length.
    pub last_len: usize,
    /// The carrier frequency, Hz.
    pub freq_hz: f64,
}

impl Shift for AddrRow {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// A burst that produced no frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BadBurst {
    /// The signal time, s.
    pub time_s: f64,
    /// The duration, µs.
    pub dur_us: f64,
    /// The SNR, dB.
    pub snr_db: f32,
}

impl Shift for BadBurst {
    fn shift_times(&mut self, by_s: f64) {
        self.time_s += by_s;
    }
}

/// The printable text of a payload ("." for other bytes).
pub fn printable(p: &[u8]) -> String {
    p.iter()
        .map(|&c| {
            if (0x20..0x7F).contains(&c) {
                c as char
            } else {
                '.'
            }
        })
        .collect()
}

impl Shaped for FrameRow {
    type Row = NrfFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&NrfFrameRow {
            time_s: self.time_s,
            bit_rate: Hertz(self.rate),
            address: &hex(&self.addr),
            address_len: self.addr.len() as i64,
            length: self.payload.len() as i64,
            pid: i64::from(self.pid),
            no_ack: self.no_ack,
            crc_bits: i64::from(self.crc_bits),
            payload: &hex(&self.payload),
            text: &printable(&self.payload),
            snr_db: f64::from(self.snr_db),
            repeats: i64::from(self.repeats),
        });
    }
}

impl Shaped for AddrRow {
    type Row = NrfAddressRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&NrfAddressRow {
            address: &hex(&self.addr),
            address_len: self.addr.len() as i64,
            bit_rate: Hertz(self.rate),
            crc_bits: i64::from(self.crc_bits),
            length: self.last_len as i64,
            frequency: Hertz(self.freq_hz),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

impl Shaped for BadBurst {
    type Row = NrfBadBurstRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&NrfBadBurstRow {
            time_s: self.time_s,
            duration_us: self.dur_us,
            snr_db: f64::from(self.snr_db),
        });
    }
}

/// What `nrf24` keeps: the frame log, the addresses and the bursts without a frame.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The frame log.
    pub frames: LogRing<FrameRow>,
    /// The addresses, keyed by their hex text.
    pub addresses: ObjectTable<String, AddrRow>,
    /// The bursts without a frame.
    pub bad: LogRing<BadBurst>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            frames: LogRing::new(FRAME_CAP),
            addresses: ObjectTable::new(ADDRESS_CAP),
            bad: LogRing::new(BAD_CAP),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.frames.shift_times(by_s);
        self.addresses.shift_times(by_s);
        self.bad.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("ism.nrf24", 1);
    fn clear(&mut self) {
        self.frames.clear();
        self.addresses.clear();
        self.bad.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.frames.adopt(saved.frames);
        self.addresses.adopt(saved.addresses);
        self.bad.adopt(saved.bad);
    }
}

const CHANNEL: Setting<field::I64> = Setting::new("channel", "Channel", "", 2)
    .range(0.0, 125.0, 1.0)
    .help("Asks the host to tune 2400 + N MHz.");

sdrkit_core::settings! {
    /// The settings of [`NrfTables`].
    pub enum NrfTablesSetting {
        /// The channel number.
        Channel = CHANNEL,
    }
}

sdrkit_core::commands! {
    /// The commands of [`NrfTables`].
    pub enum NrfTablesCommand {
        /// Forgets everything heard.
        Clear = commands::CLEAR,
        /// Forgets one address.
        Remove = NRF_REMOVE.command,
    }
}

/// The acceptance rules and the tables (`nrf.tables`).
pub struct NrfTables {
    store: Store,
    published: Published,
    addresses_table: TableId,
    sightings: ObjectTable<u64, u32>,
    recent: Dedup<u64>,
    events: Vec<Event>,
    requests: Vec<HostRequest>,
    summary: String,
    channel_hz: f64,
    channel: u32,
    configured: bool,
    frames: u64,
    bad: u64,
    bursts: u64,
    crc16: u64,
    crc8: u64,
    now: f64,
    last_frame: f64,
    last_burst: f64,
}

impl Default for NrfTables {
    fn default() -> Self {
        let published = Published::from_decl(Arc::new(
            ResultsDecl::new()
                .with_fields(NrfFields::SHAPE.fields)
                .with_table(&NRF_FRAMES)
                .with_table(&NRF_ADDRESSES)
                .with_table(&NRF_BAD_BURSTS),
        ));
        let addresses_table = published
            .results()
            .decl()
            .table_id(NRF_ADDRESSES.key)
            .expect("the tables of nrf.tables are declared");
        NrfTables {
            store: Store::default(),
            published,
            addresses_table,
            sightings: ObjectTable::new(1024),
            recent: Dedup::new(2e-3, 64),
            events: Vec::new(),
            requests: Vec::new(),
            summary: String::new(),
            channel_hz: 0.0,
            channel: 2,
            configured: false,
            frames: 0,
            bad: 0,
            bursts: 0,
            crc16: 0,
            crc8: 0,
            now: 0.0,
            last_frame: f64::NEG_INFINITY,
            last_burst: f64::NEG_INFINITY,
        }
    }
}

/// The key of a frame's content: address, PID, NO_ACK and payload.
fn content_key(f: &EsbFrame) -> u64 {
    let mut b = Vec::with_capacity(f.addr.len() + f.payload.len() + 3);
    b.push(f.addr.len() as u8);
    b.extend_from_slice(&f.addr);
    b.push(f.pid);
    b.push(f.no_ack as u8);
    b.extend_from_slice(&f.payload);
    fingerprint(&b)
}

impl NrfTables {
    /// The tables held.
    pub fn store(&self) -> &Store {
        &self.store
    }

    fn take(&mut self, t: f64, f: &EsbFrame) {
        if self.recent.seen(content_key(f), t) {
            return;
        }
        self.frames += 1;
        self.last_frame = t;
        if f.crc_bits == 16 {
            self.crc16 += 1;
        } else {
            self.crc8 += 1;
        }
        let freq = self.channel_hz + f.offset_hz as f64;
        match self.store.frames.newest_mut() {
            Some(r)
                if r.addr == f.addr
                    && r.payload == f.payload
                    && r.pid == f.pid
                    && r.no_ack == f.no_ack
                    && t - r.time_s < 2.0 =>
            {
                r.repeats += 1;
                r.time_s = t;
            }
            _ => {
                self.store.frames.push(FrameRow {
                    time_s: t,
                    rate: f.rate,
                    addr: f.addr.clone(),
                    pid: f.pid,
                    no_ack: f.no_ack,
                    crc_bits: f.crc_bits,
                    payload: f.payload.clone(),
                    snr_db: f.snr_db,
                    repeats: 1,
                });
            }
        }
        let ah = hex(&f.addr);
        let Ok(up) = self
            .store
            .addresses
            .upsert(ah.as_str(), Stamp::at(t), AddrRow::default)
        else {
            return;
        };
        if up.new {
            let kbps = (f.rate / 1e3).round();
            self.events.push(NRF_ADDRESS.event(
                t,
                freq,
                &NrfAddressEvent {
                    source: &ah,
                    text: &format!(
                        "nRF24: new address {ah} ({kbps} kbit/s, CRC-{})",
                        f.crc_bits
                    ),
                    family: Family::Sensor,
                    bit_rate: Hertz(f.rate),
                    crc_bits: i64::from(f.crc_bits),
                },
            ));
            up.row.addr = f.addr.clone();
            up.row.rate = f.rate;
            up.row.crc_bits = f.crc_bits;
        }
        up.row.last_len = f.payload.len();
        up.row.freq_hz = freq;
    }

    fn crc_of(&self, addr: &[u8]) -> Option<u8> {
        self.store
            .addresses
            .get(hex(addr).as_str())
            .map(|a| a.crc_bits)
    }

    fn burst(&mut self, t: f64, b: &BurstResult) {
        self.bursts += 1;
        self.last_burst = t;
        self.now = self.now.max(t);
        let mut any = false;
        let has16 = b.frames.iter().any(|f| f.crc_bits == 16);
        for f in &b.frames {
            if f.crc_bits == 16 {
                self.take(t, f);
                any = true;
                continue;
            }
            let known = self.crc_of(&f.addr);
            if has16 || known == Some(16) {
                continue;
            }
            let key = content_key(f);
            let n = match self.sightings.upsert(&key, Stamp::at(t), || 0) {
                Ok(up) => {
                    *up.row += 1;
                    *up.row
                }
                Err(_) => 0,
            };
            if known == Some(8) || n >= 3 {
                self.take(t, f);
                any = true;
            }
        }
        if !any && b.frames.is_empty() && b.dur_us >= 20.0 {
            self.bad += 1;
            self.store.bad.push(BadBurst {
                time_s: t,
                dur_us: b.dur_us,
                snr_db: b.snr_db,
            });
        }
    }

    fn publish(&mut self) {
        let state = if self.now - self.last_frame < 30.0 && self.frames > 0 {
            2
        } else if self.now - self.last_burst < 30.0 && self.bursts > 0 {
            1
        } else {
            0
        };
        let n = self.store.addresses.len();
        self.summary.clear();
        if self.frames > 0 {
            let _ = write!(
                self.summary,
                "nRF24: {} frames from {n} address(es)",
                self.frames
            );
        } else if self.bursts > 0 {
            let _ = write!(
                self.summary,
                "2.4 GHz channel: {} bursts, no ESB frame decoded",
                self.bursts
            );
        } else {
            self.summary.push_str("nRF24: no signal");
        }
        let p = &mut self.published;
        set_status(p, LockState::from_level(state), &self.summary);
        p.put(&NrfFields {
            frames: self.frames as i64,
            bad_bursts: self.bad as i64,
            bursts: self.bursts as i64,
            crc16_frames: self.crc16 as i64,
            crc8_frames: self.crc8 as i64,
            addresses: n as i64,
            channel: i64::from(self.channel),
        });
        shaped::log_rows(&self.store.frames, &NRF_FRAMES, self.now, p);
        shaped::rows(&self.store.addresses, &NRF_ADDRESSES, self.now, p);
        shaped::log_rows(&self.store.bad, &NRF_BAD_BURSTS, self.now, p);
    }
}

impl NrfTables {
    /// Forgets everything heard and every count.
    fn clear(&mut self) {
        self.store.clear();
        self.sightings.clear();
        self.recent.clear();
        (self.frames, self.bad, self.bursts, self.crc16, self.crc8) = (0, 0, 0, 0, 0);
        self.published.clear_rows(&NRF_FRAMES);
        self.published.clear_rows(&NRF_ADDRESSES);
        self.published.clear_rows(&NRF_BAD_BURSTS);
    }
}

impl Component for NrfTables {
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "nrf.tables",
            vec![Port::typed::<BurstResult>("in")],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(NrfTablesSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.channel_hz = inputs.first().map_or(0.0, |f| f.tuned_hz);
        self.configured = true;
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut new = false;
        for b in io.items::<BurstResult>(0) {
            self.burst(b.time, &b.value);
            new = true;
        }
        let tick = io.tick();
        if tick {
            self.now += REPORT_S;
        }
        if new || tick {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
        let now = self.now;
        for r in self.requests.drain(..) {
            io.items_out::<HostRequest>(1).push(Timed {
                time: now,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match NrfTablesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            NrfTablesSetting::Channel => {
                let n = CHANNEL.read(id, v)? as u32;
                // A change by the user (after the start) requests the host to tune there.
                if self.configured && n != self.channel {
                    self.requests.push(HostRequest::retune(
                        2400e6 + f64::from(n) * 1e6,
                        "nRF24 channel changed",
                    ));
                }
                self.channel = n;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            NrfTablesSetting::Channel,
            Value::I64(i64::from(self.channel)),
        )])
    }
    fn commands(&self) -> &Commands {
        NrfTablesCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match NrfTablesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            NrfTablesCommand::Clear => self.clear(),
            NrfTablesCommand::Remove => commands::remove(
                &mut self.store.addresses,
                self.addresses_table,
                &mut self.published,
                args,
                &NRF_ADDRESS_ROW,
            )?,
        }
        self.publish();
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(NRF_FRAMES.key) {
            self.store.frames.clear();
            self.published.clear_rows(&NRF_FRAMES);
        }
        if !kept.table(NRF_ADDRESSES.key) {
            self.store.addresses.clear();
            self.published.clear_rows(&NRF_ADDRESSES);
        }
        if !kept.table(NRF_BAD_BURSTS.key) {
            self.store.bad.clear();
            self.published.clear_rows(&NRF_BAD_BURSTS);
        }
        for (key, n) in [
            ("frames", &mut self.frames),
            ("bad_bursts", &mut self.bad),
            ("bursts", &mut self.bursts),
            ("crc16_frames", &mut self.crc16),
            ("crc8_frames", &mut self.crc8),
        ] {
            if !kept.field(key) {
                *n = 0;
            }
        }
        self.sightings.clear();
        self.recent.clear();
        self.last_frame = f64::NEG_INFINITY;
        self.last_burst = f64::NEG_INFINITY;
        self.publish();
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now)?;
        self.publish();
        Ok(())
    }
}

/// The `nrf24` receiver: the burst detector, the demodulator at three rates and the tables.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among
/// them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add("burst", BurstDet::default())
        .add("demod", DecoderComponent::new(Demod::default()))
        .add("tables", NrfTables::default())
        .input("burst")
        .connect("burst", "demod")
        .connect("demod.out", "tables.in")
        .output("tables.events")
        .output("tables.requests")
        .clock(REPORT_S)
        .status_from("tables")
        .build()?;
    StackGraph::of(crate::typed::shape("nrf24"), graph)
        .min_rate(MIN_RATE, &min_rate_message("nRF24L01", MIN_RATE))
        .route(CHANNEL.key(), &["tables.channel"])?
        .command("clear", sdrkit_graph::CommandTarget::of("tables", "clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("tables", "remove"),
        )
        .with_settings(&cfg.settings)
        .build()
}

sdrkit_core::choice_enum! {
    /// The bit rates of the test signal.
    pub enum GenRate {
        /// 1 Mbit/s.
        Mbit1 = ("1000", "1 Mbit/s"),
        /// 2 Mbit/s.
        Mbit2 = ("2000", "2 Mbit/s"),
        /// 250 kbit/s.
        Kbit250 = ("250", "250 kbit/s"),
        /// The three rates in turn.
        All = ("all", "All three in turn"),
    }
}

sdrkit_core::choice_enum! {
    /// The frames of the test signal.
    pub enum GenCrc {
        /// The frames with a CRC-16.
        Crc16 = ("16", "CRC-16"),
        /// The frames with a CRC-8.
        Crc8 = ("8", "CRC-8"),
        /// All frames.
        Both = ("both", "Both"),
    }
}

/// The bit rates of the test signal.
pub const GEN_RATE: Setting<field::Choice<GenRate>> =
    Setting::choice("gen_rate", "Test signal rate", GenRate::All)
        .section("Test signal")
        .advanced();
/// The frames of the test signal.
pub const GEN_CRC: Setting<field::Choice<GenCrc>> =
    Setting::choice("gen_crc", "Test frames", GenCrc::Both)
        .section("Test signal")
        .advanced();
/// Bursts of random bits in place of frames.
pub const GEN_RANDOM: Setting<field::Bool> =
    Setting::new("gen_random", "Random-bit bursts (no frame)", "", false)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `nrf24` stack.
    pub enum NrfSetting {
        /// The channel number.
        Channel = CHANNEL,
        /// The bit rates of the test signal.
        GenRate = GEN_RATE,
        /// The frames of the test signal.
        GenCrc = GEN_CRC,
        /// Random-bit bursts.
        GenRandom = GEN_RANDOM,
    }
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// A test frame: (address, PID, NO_ACK, payload, CRC-16).
pub type TestFrame = (Vec<u8>, u8, bool, Vec<u8>, bool);

/// The test frames.
pub fn test_frames() -> Vec<TestFrame> {
    vec![
        (vec![0xE7; 5], 0, false, b"Hello nRF24".to_vec(), true),
        (
            vec![0xC2; 5],
            1,
            false,
            b"temp=21.5C hum=40%".to_vec(),
            true,
        ),
        (
            vec![0x12, 0x34, 0x56, 0x78],
            2,
            true,
            b"mouse dx=3 dy=-2".to_vec(),
            true,
        ),
        (vec![0xA5, 0x5A, 0x3C], 3, false, b"ping".to_vec(), true),
        (
            vec![0x01, 0x23, 0x45, 0x67, 0x89],
            0,
            false,
            b"light on".to_vec(),
            false,
        ),
        (
            vec![0x01, 0x23, 0x45, 0x67, 0x89],
            1,
            false,
            b"light off".to_vec(),
            false,
        ),
        (
            vec![0xB3, 0xB4, 0xB5, 0xB6],
            2,
            false,
            vec![0x00, 0x01, 0x02, 0xFF, 0x10],
            false,
        ),
    ]
}

/// The settings of the generator.
#[derive(Clone, Debug)]
pub struct GenParams {
    /// The rates to send in turn, bit/s.
    pub rates: Vec<f64>,
    /// Which frames: 16, 8 or both (0).
    pub crc: u8,
    /// Bursts of random bits instead of frames.
    pub random: bool,
    /// The seed of the content.
    pub seed: u64,
}

/// The GFSK modulator of a rate (BT 0.5, the rate's deviation) at `rate` samples a second.
pub fn modulator(bit_rate: f64, rate: f64) -> Option<Cpm> {
    let dev = RATES
        .iter()
        .find(|r| r.0 == bit_rate)
        .map_or(160e3, |r| r.1);
    Cpm::gfsk(0.5, 2.0 * dev / bit_rate, rate / bit_rate).ok()
}

/// The `nrf24` generator at `rate` samples a second: the test frames one after another with 2 ms gaps, GFSK BT 0.5. The signal is
/// clean: noise, offset and clock error come from the `gen_*` settings.
pub fn generator(rate: f64, p: GenParams) -> CleanScene {
    let rate = if rate > 0.0 && rate.is_finite() {
        rate
    } else {
        MIN_RATE
    };
    let fastest = p.rates.iter().copied().fold(0.0, f64::max).max(250e3);
    let info = SignalInfo::new(fastest)
        .with_symbol_rate(fastest)
        .with_bit_rate(fastest);
    let mut scene = CleanScene::new(Scene::new(rate).expect("a positive rate"), rate, info);
    let frames: Vec<TestFrame> = test_frames()
        .into_iter()
        .filter(|f| p.crc == 0 || (p.crc == 16) == f.4)
        .collect();
    if frames.is_empty() || p.rates.is_empty() {
        return scene;
    }
    let mods: Vec<Option<Cpm>> = p.rates.iter().map(|&r| modulator(r, rate)).collect();
    let mut mods = mods;
    let mut rng = Rng::new(p.seed ^ 0x24);
    let mut t = 0.001f64;
    let mut k = 0usize;
    let mut levels = Vec::new();
    let random = p.random;
    let source = move || loop {
        let ri = (k / frames.len()) % mods.len();
        let f = &frames[k % frames.len()];
        k += 1;
        let bits = if random {
            let mut b = vec![0u8; 200];
            rng.fill_bits(&mut b);
            b
        } else {
            frame_bits(&f.0, f.1, f.2, &f.3, f.4)
        };
        let Some(m) = mods[ri].as_mut() else {
            continue;
        };
        levels.clear();
        levels.extend(bits.iter().map(|&b| if b != 0 { 1.0f32 } else { -1.0 }));
        let mut samples = Vec::new();
        m.push(&levels, &mut samples);
        m.flush(&mut samples);
        let start = t;
        t += samples.len() as f64 / rate + 2e-3;
        return Some(Burst {
            start_s: start,
            offset_hz: 0.0,
            level_db: 0.0,
            samples,
        });
    };
    scene.scene_mut().add_source(Box::new(source));
    scene
}

/// The settings of the generator from a configuration.
pub fn gen_params(cfg: &GenConfig) -> GenParams {
    let s = &cfg.settings;
    GenParams {
        rates: match s.get(&GEN_RATE) {
            GenRate::Mbit1 => vec![1e6],
            GenRate::Mbit2 => vec![2e6],
            GenRate::Kbit250 => vec![250e3],
            GenRate::All => vec![1e6, 2e6, 250e3],
        },
        crc: match s.get(&GEN_CRC) {
            GenCrc::Crc16 => 16,
            GenCrc::Crc8 => 8,
            GenCrc::Both => 0,
        },
        random: s.get(&GEN_RANDOM),
        seed: cfg.seed,
    }
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(generator(cfg.rate_hz, gen_params(cfg))))
}

/// The verify case: the test frames at all three rates.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, three rates", o, 0.0);
    c.seconds = o.seconds.max(1.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = NrfFields::read(r.fields()).map_or(0, |f| f.crc16_frames);
        vec![CheckLine::new(
            "crc16_frames",
            n >= 4,
            format!("{n}, at least 4 expected"),
        )]
    });
    vec![c]
}

/// nRF24L01.
pub static NRF24: StackDescriptor = StackDescriptor {
    id: "nrf24",
    name: "nRF24L01",
    group: "2.4 GHz, channel 0-125",
    blurb: "nRF24L01 Enhanced ShockBurst frames on one 2.4 GHz channel at 250 kbit/s, 1 and 2 Mbit/s (needs a user test on a real signal).",
    tuning: Tuning { min_hz: 2400e6, max_hz: 2526e6, tune_offset_hz: 1.5e6, ..Tuning::anywhere(2402e6, 8e6, 4e6, 5e6) },
    submodes: &[],
    settings: NrfSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Nrf24],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::{InputInfo, Output};

    /// The CRC-16 frames the receiver finds in 0.25 s of the generator at `bit_rate`, 15 dB Eb/N0 and `offset_hz`, and the
    /// bursts sent.
    fn frames_at_15_db(bit_rate: f64, offset_hz: f64) -> (i64, i64) {
        use sdrkit_components::sim::{with_generator_settings, NoiseKind};
        use sdrkit_core::{Choice, Owner};
        let schema = with_generator_settings(&NrfSetting::schema()).unwrap();
        let mut v = Values::defaults(&schema);
        let mut set = |key: &str, value: Value| v.set(schema.id(key).unwrap(), &value).unwrap();
        set("gen_offset_hz", Value::F64(offset_hz));
        set("gen_noise", Value::Choice(NoiseKind::EbN0.index()));
        set("gen_ebn0_db", Value::F64(15.0));
        let p = GenParams {
            rates: vec![bit_rate],
            crc: 16,
            random: false,
            seed: 1,
        };
        let g: Box<dyn Generator> = Box::new(generator(8e6, p));
        let mut g = sdrkit_components::sim::impaired(g, &v).unwrap();
        let mut x = vec![Complex32::default(); 2_000_000];
        g.generate(&mut x);
        let mut r = receiver(&StackConfig::new(Values::defaults(&NrfSetting::schema()))).unwrap();
        r.configure(&InputInfo::new(8e6, 2402e6, 2402e6).recording())
            .unwrap();
        let mut out = Output::default();
        for c in x.chunks(50_000) {
            r.feed(c, &mut out);
        }
        r.feed(&vec![Complex32::default(); 100_000], &mut out);
        r.refresh_results();
        let t = r.graph().component_as::<NrfTables>("tables").unwrap();
        let s = t.store();
        let f = NrfFields::read(r.results().fields()).expect("the nRF24 fields");
        let addrs: std::collections::BTreeSet<String> =
            s.frames.iter().map(|f| hex(&f.addr)).collect();
        assert_eq!(addrs.len(), 4, "{bit_rate}: {addrs:?}");
        (f.crc16_frames, f.bursts)
    }

    #[test]
    fn two_mbit_frames_decode_at_15_db() {
        // Modulation index 0.32: the MLSE over the discriminator levels decides almost every frame.
        for off in [0.0, -100e3, 100e3] {
            let (n, bursts) = frames_at_15_db(2e6, off);
            assert!(
                n * 100 >= bursts * 92 && bursts >= 110,
                "{off}: {n} of {bursts}"
            );
        }
        let (n, bursts) = frames_at_15_db(1e6, 100e3);
        assert!(
            n * 100 >= bursts * 85 && bursts >= 100,
            "1 Mbit/s: {n} of {bursts}"
        );
    }
}
