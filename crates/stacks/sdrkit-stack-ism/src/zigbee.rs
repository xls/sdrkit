//! Zigbee and IEEE 802.15.4 on 2.4 GHz (`zigbee`): O-QPSK chips, frames with their FCS, the MAC, Zigbee NWK and APS headers,
//! beacons and device announcements. A secured MAC frame opens with a key of the setting `keys` ([`security`]); without
//! one its payload is shown as encrypted.
//! Standard: IEEE 802.15.4-2020 clause 12 (2450 MHz O-QPSK) and clause 7 (MAC frames), Zigbee specification R22 (NWK, APS).

pub mod security;
pub mod send;

use crate::family::{at_least_value, min_rate_message, set_status, CleanScene, REPORT_S};
use crate::mac154::{self, eui, Addr};
use crate::typed::{
    PanDeviceRow, ZigbeeFields, ZigbeeFrameRow, ZigbeeNetworkRow, ZigbeePanEvent, PAN_DEVICES,
    PAN_DEVICE_ROW, PAN_REMOVE, ZIGBEE_FRAMES, ZIGBEE_NETWORKS, ZIGBEE_PAN, ZIGBEE_RATE,
};
use sdrkit_algo_dsp::fir::{design_by_taps, Fir};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::stats::WindowSum;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, ReadFields, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::AutoGain;
use sdrkit_core::SignalType;
use sdrkit_core::{
    Complex32, ConfigError, Event, GenConfig, Generator, HostRequest, LockProfile, LockState,
    Receiver, Record, Results, ResultsDecl, SeriesId, Setting, SettingError, SettingId, SettingSet,
    SignalInfo, StackConfig, StackDescriptor, StateBlob, StateError, TableId, Tuning, Value,
    Values,
};
use sdrkit_graph::Catalog;
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Kept, Kind, Port, RateFrom, StackGraph, Timed,
};
use sdrkit_util_bits::crc::catalog::CRC16_KERMIT;
use sdrkit_util_bits::crc::Trailer;
use sdrkit_util_ids::oui::vendor_of_address;
use sdrkit_util_objects::commands;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::Tables as _;
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f32::consts::FRAC_PI_2;
use std::fmt::Write as _;
use std::sync::Arc;

/// The 16 chip sequences (c0 is the most significant bit). IEEE 802.15.4-2020 table 12-1.
pub const CHIPS: [u32; 16] = [
    0xD9C3522E, 0xED9C3522, 0x2ED9C352, 0x22ED9C35, 0x522ED9C3, 0x3522ED9C, 0xC3522ED9, 0x9C3522ED,
    0x8C96077B, 0xB8C96077, 0x7B8C9607, 0x77B8C960, 0x077B8C96, 0x6077B8C9, 0x96077B8C, 0xC96077B8,
];

/// Chip `n` of symbol `s` as +1 or -1.
fn chip(s: usize, n: usize) -> f32 {
    if (CHIPS[s] >> (31 - n)) & 1 != 0 {
        1.0
    } else {
        -1.0
    }
}

/// The frequency-sign pattern `d[s][n]`: the sign of the phase step during chip `n` of symbol `s`.
pub fn sign_patterns() -> [[f32; 32]; 16] {
    let mut d = [[0f32; 32]; 16];
    for (s, row) in d.iter_mut().enumerate() {
        for n in 0..32 {
            let prev = if n == 0 { chip(s, 31) } else { chip(s, n - 1) };
            let sg = if (n + 1) % 2 == 0 { 1.0 } else { -1.0 };
            row[n] = chip(s, n) * prev * sg;
        }
    }
    d
}

/// One 802.15.4 frame (`mac154.frame`), from the O-QPSK or the SUN FSK receiver.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame154 {
    /// The PSDU, FCS included.
    pub psdu: Vec<u8>,
    /// The FCS passed.
    pub fcs_ok: bool,
    /// The PHY: "O-QPSK" or "SUN FSK".
    pub phy: String,
    /// The O-QPSK channel number 11 to 26 (0 when not known).
    pub channel: u32,
    /// The carrier frequency (0 when not known).
    pub freq_hz: f64,
    /// The level, dBFS.
    pub level_db: f32,
    /// The SNR, dB.
    pub snr_db: f32,
    /// The carrier offset from the channel centre, Hz.
    pub offset_hz: f64,
    /// The signal time of the preamble start, seconds.
    pub time: f64,
}

impl Item for Frame154 {
    const KIND: &'static str = "mac154.frame";
}

/// The channel 11 to 26 whose centre is within 2.5 MHz of `hz` (0 when none).
pub fn channel_of(hz: f64) -> u32 {
    (11..=26)
        .find(|k| (hz - channel_hz(*k)).abs() < 2.5e6)
        .unwrap_or(0)
}

/// The centre of channel `k` (11 to 26): 2405 + 5 (k - 11) MHz. IEEE 802.15.4-2020 clause 10.1.3.
pub fn channel_hz(k: u32) -> f64 {
    2405e6 + 5e6 * (k as f64 - 11.0)
}

/// The rate inside the receiver: 4 samples per chip.
pub const CHIP_RATE_X4: f64 = 8e6;

/// The cut-off of the channel filter, Hz: a two-sided width of one chip rate, the best width in front of a detector of the
/// phase step over one chip of MSK. With 1.3 MHz the noise in front of the step product costs about 0.4 dB.
const FRONT_CUT_HZ: f64 = 1.0e6;

/// `zb.front`: mixes the channel to 0 Hz, low-passes it (cut-off 1.0 MHz, Hamming window) and resamples it to 8 Msps.
#[derive(Default)]
pub struct Front {
    nco: Option<Nco>,
    fir: Option<Fir<Complex32>>,
    rs: Option<FractionalResampler<Complex32>>,
    work: Vec<Complex32>,
}

impl Component for Front {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "zb.front",
            vec![Port::iq("in")],
            vec![Port::iq("out").rate_from(RateFrom::Fixed(CHIP_RATE_X4))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let fs = f.rate;
        let taps = ((3.3 * fs / 1.6e6).max(15.0) as usize) | 1;
        let h = design_by_taps(taps, FRONT_CUT_HZ / fs, Window::Hamming)?;
        self.nco = Some(Nco::new(fs, -f.signal_offset_hz()));
        self.fir = Some(Fir::new(&h));
        self.rs = Some(FractionalResampler::with_rates(fs, CHIP_RATE_X4)?);
        let mut o = f.with_rate(CHIP_RATE_X4);
        o.centre_hz = f.tuned_hz;
        Ok(vec![o])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let (Some(nco), Some(fir), Some(rs)) = (&mut self.nco, &mut self.fir, &mut self.rs) else {
            return;
        };
        self.work.resize(x.len(), Complex32::default());
        nco.mix(x, &mut self.work);
        fir.process_in_place(&mut self.work);
        rs.process(&self.work, io.iq_out(0));
    }
    /// Runs the samples the low-pass filter still holds out through the resampler: it feeds the filter as many zeros as it has
    /// taps.
    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        let (Some(fir), Some(rs)) = (&mut self.fir, &mut self.rs) else {
            return;
        };
        self.work.clear();
        self.work.resize(fir.len(), Complex32::default());
        fir.process_in_place(&mut self.work);
        rs.process(&self.work, io.iq_out(0));
    }
}

/// Samples of the running mean of the step magnitude that weights the chip values.
const CHIP_MEAN_LEN: f32 = 256.0;
/// The largest chip value (radians; the nominal step is pi/2).
const CHIP_VALUE_LIMIT: f32 = 2.5;

/// `zb.chips`: the chip value over one chip (4 samples) at 8 Msps, the power and the cosine of the step, as (value, power,
/// cosine) triples in a real stream. With `z` the step product of one chip and `A` the running mean of `|z|` over 256 samples,
/// the value is `clamp(pi/2 Im(z) / A, -2.5, 2.5)`: the sine of the step weighted by the chip's strength, pi/2 at the nominal
/// step of a strong chip and near 0 at a click, where the angle of the step would count up to pi. The cosine is
/// `Re(z) / |z|`: 0 at the nominal step, and `-s sin d` for a step of sign `s` with a carrier error `d`, which is how the frame
/// decoder measures `d`.
#[derive(Default)]
pub struct Chips {
    hist: [Complex32; 4],
    pos: usize,
    /// The running mean of `|z|`.
    mean: f32,
}

impl Component for Chips {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("zb.chips", vec![Port::iq("in")], vec![Port::real("out")])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0]
            .with_kind(Kind::Real)
            .with_rate(inputs[0].rate * 3.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let out = io.real_out(0);
        for &s in x {
            let old = self.hist[self.pos];
            self.hist[self.pos] = s;
            self.pos = (self.pos + 1) & 3;
            let p = s * old.conj();
            let m = p.norm();
            self.mean = if self.mean > 0.0 {
                self.mean + (m - self.mean) / CHIP_MEAN_LEN
            } else {
                m
            };
            let v = std::f32::consts::FRAC_PI_2 * p.im / self.mean.max(1e-9);
            out.push(v.clamp(-CHIP_VALUE_LIMIT, CHIP_VALUE_LIMIT));
            out.push(s.norm_sqr());
            out.push(if m > 0.0 { p.re / m } else { 0.0 });
        }
    }
}

/// The 16 symbols.
const ALL_SYMBOLS: [usize; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

/// Samples of chip phase steps kept: 16 ms at 8 Msps, more than the longest frame.
const RING: usize = 1 << 17;

/// `zb.frame`: preamble triggers per sample phase, symbol decisions, the frame states and the FCS.
pub struct FrameDec {
    v: SampleRing<f32>,
    p: SampleRing<f32>,
    /// The cosines of the steps.
    u: SampleRing<f32>,
    mean: WindowSum<f32>,
    pos: u64,
    reg: [u32; 4],
    mem: [u128; 4],
    quiet: [u32; 4],
    pending: VecDeque<u64>,
    busy_until: u64,
    d: [[f32; 32]; 16],
    d0_bits: u32,
    channel: u32,
    tuned_hz: f64,
    /// Preamble candidates.
    pub candidates: u64,
    /// Bad frames (SFD found, FCS failed).
    pub bad: u64,
    out: Vec<Frame154>,
}

impl Default for FrameDec {
    fn default() -> Self {
        let d = sign_patterns();
        let mut bits = 0u32;
        for n in 1..32 {
            if d[0][n] > 0.0 {
                bits |= 1 << (31 - n);
            }
        }
        FrameDec {
            v: SampleRing::new(RING).expect("a ring of a fixed size"),
            p: SampleRing::new(RING).expect("a ring of a fixed size"),
            u: SampleRing::new(RING).expect("a ring of a fixed size"),
            mean: WindowSum::new(128).expect("a window of a fixed size"),
            pos: 0,
            reg: [0; 4],
            mem: [0; 4],
            quiet: [0; 4],
            pending: VecDeque::with_capacity(64),
            busy_until: 0,
            d,
            d0_bits: bits,
            channel: 0,
            tuned_hz: 0.0,
            candidates: 0,
            bad: 0,
            out: Vec::new(),
        }
    }
}

impl FrameDec {
    fn at(&self, t: i64) -> f32 {
        if t < 0 {
            return 0.0;
        }
        self.v.get(t as u64).unwrap_or(0.0)
    }
    fn corr(&self, end: i64, s: usize) -> f32 {
        let mut c = 0f32;
        for n in 1..32 {
            c += self.at(end - 4 * (31 - n as i64)) * self.d[s][n];
        }
        c
    }
    /// Adds the symbol `s` that ends at `end` to the sums of the carrier error `turn` (sine, cosine). The step of a chip of
    /// sign `g` has the cosine `-g sin d` and the sine `g cos d`, `d` the turn of the step over one chip (0.5 µs) by the
    /// carrier error; the sine has the sign of the chip value. Only chips with two chips of their own sign before them and
    /// one after (inside the symbol) count: their step is a quarter turn wherever in the chip it is read and through the
    /// channel filter; near a chip of the other sign it is less.
    fn add_turn(&self, end: i64, s: usize, turn: &mut (f64, f64)) {
        for n in 2..31usize {
            let g = self.d[s][n];
            if [n - 2, n - 1, n + 1].iter().any(|&k| self.d[s][k] != g) {
                continue;
            }
            let t = end - 4 * (31 - n as i64);
            let cosine = self.u.get(t.max(0) as u64).unwrap_or(0.0);
            let sine = (1.0 - cosine * cosine).max(0.0).sqrt().copysign(self.at(t));
            turn.0 -= f64::from(g * cosine);
            turn.1 += f64::from(g * sine);
        }
    }
    /// The correlation of symbol `s` after symbol `prev`: [`Self::corr`] over chips 1 to 31, and chip 0, whose step goes from
    /// the last chip of `prev` to the first of `s` (a 32nd of the symbol's energy, 0.14 dB).
    fn corr_after(&self, end: i64, s: usize, prev: usize) -> f32 {
        self.corr(end, s) - self.at(end - 4 * 31) * chip(s, 0) * chip(prev, 31)
    }
    /// One symbol after `end` and after the symbol `prev`: the best of the patterns `among`, with early and late tries. It
    /// updates the residual sums, the sums of the carrier error and `prev`. The chip values hold no carrier term: a carrier
    /// error `d` turns the steps to `+-pi/2 + d`, whose sines are `+-cos d`.
    fn symbol(
        &self,
        end: &mut i64,
        resid: &mut f64,
        nres: &mut usize,
        prev: &mut usize,
        turn: &mut (f64, f64),
        among: &[usize],
    ) -> usize {
        *end += 128;
        let mut bs = (0usize, f32::MIN, 0i64);
        for &s in among {
            let c = self.corr_after(*end, s, *prev);
            if c > bs.1 {
                bs = (s, c, 0);
            }
        }
        for dt in [-1i64, 1] {
            for &s in among {
                let c = self.corr_after(*end + dt, s, *prev);
                if c > bs.1 * 1.04 && c > bs.1 {
                    bs = (s, c, dt);
                }
            }
        }
        *end += bs.2;
        let s = bs.0;
        for n in 1..32 {
            let e = self.at(*end - 4 * (31 - n as i64)).abs() - FRAC_PI_2;
            *resid += (e * e) as f64;
            *nres += 1;
        }
        self.add_turn(*end, s, turn);
        *prev = s;
        s
    }
    fn decode(&mut self, trig: u64) {
        let t0 = trig as i64;
        // 1. alignment over the three preamble symbols
        let mut best: Option<(i64, f32)> = None;
        for sh in -1..=1i64 {
            let mut score = 0f32;
            for j in 0..3i64 {
                for n in 1..32 {
                    let t = t0 + sh - 128 * j - 4 * (31 - n as i64);
                    score += self.d[0][n] * self.at(t);
                }
            }
            if best.is_none_or(|b| score > b.1) {
                best = Some((sh, score));
            }
        }
        let Some((sh, score)) = best else {
            return;
        };
        if score < 0.35 * FRAC_PI_2 * 93.0 {
            return;
        }
        // The carrier error: summed over the three preamble symbols here, and over every symbol read after them.
        let mut turn = (0f64, 0f64);
        for j in 0..3i64 {
            self.add_turn(t0 + sh - 128 * j, 0, &mut turn);
        }
        let pre_start = t0 + sh - 3 * 128 + 1;
        let level = {
            let (mut a, mut n) = (0f64, 0);
            for t in pre_start.max(0)..=t0 + sh {
                if let Some(p) = self.p.get(t as u64) {
                    a += p as f64;
                    n += 1;
                }
            }
            a / n.max(1) as f64
        };
        let mut end = t0 + sh;
        let (mut resid, mut nres) = (0f64, 0usize);
        // The symbol before the first one read is the preamble's symbol 0.
        let mut prev = 0usize;
        // 3. states
        let mut extra = 0;
        let s = loop {
            // Until the start-of-frame delimiter only its first symbol or one more of the preamble can come.
            let s = self.symbol(
                &mut end,
                &mut resid,
                &mut nres,
                &mut prev,
                &mut turn,
                &[0, 7],
            );
            if s != 0 {
                break s;
            }
            extra += 1;
            if extra > 12 {
                return;
            }
        };
        if s != 7
            || self.symbol(
                &mut end,
                &mut resid,
                &mut nres,
                &mut prev,
                &mut turn,
                &ALL_SYMBOLS,
            ) != 0xA
        {
            return;
        }
        let lo = self.symbol(
            &mut end,
            &mut resid,
            &mut nres,
            &mut prev,
            &mut turn,
            &ALL_SYMBOLS,
        );
        let hi = self.symbol(
            &mut end,
            &mut resid,
            &mut nres,
            &mut prev,
            &mut turn,
            &ALL_SYMBOLS,
        );
        let len = lo | ((hi & 7) << 4);
        if len < 3 {
            return;
        }
        let mut psdu = Vec::with_capacity(len);
        for _ in 0..len {
            let a = self.symbol(
                &mut end,
                &mut resid,
                &mut nres,
                &mut prev,
                &mut turn,
                &ALL_SYMBOLS,
            );
            let b = self.symbol(
                &mut end,
                &mut resid,
                &mut nres,
                &mut prev,
                &mut turn,
                &ALL_SYMBOLS,
            );
            psdu.push((a | (b << 4)) as u8);
        }
        self.busy_until = end.max(0) as u64;
        if !CRC16_KERMIT.verify(&psdu, Trailer::Little) {
            self.bad += 1;
            return;
        }
        let offset_hz = turn.0.atan2(turn.1) * 2e6 / std::f64::consts::TAU;
        let mse = (resid / nres.max(1) as f64).max(1e-6);
        let snr = 10.0 * ((FRAC_PI_2 as f64).powi(2) / mse).log10();
        self.out.push(Frame154 {
            psdu,
            fcs_ok: true,
            phy: "O-QPSK".into(),
            channel: self.channel,
            freq_hz: if self.tuned_hz > 0.0 {
                self.tuned_hz + offset_hz
            } else {
                0.0
            },
            level_db: (10.0 * level.max(1e-20).log10()) as f32,
            snr_db: snr as f32,
            offset_hz,
            time: pre_start.max(0) as f64 / CHIP_RATE_X4,
        });
    }
    fn mark(&mut self, ph: usize, m: bool) {
        self.mem[ph] = (self.mem[ph] << 1) | m as u128;
    }
    fn matched_before(&self, ph: usize) -> bool {
        let m = self.mem[ph];
        (m >> 31) & 1 != 0 && (m >> 63) & 1 != 0
    }
}

impl Component for FrameDec {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "zb.frame",
            vec![Port::real("in")],
            vec![
                Port::typed::<Frame154>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<DecoderCounts>("counts").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.tuned_hz = inputs[0].tuned_hz;
        self.channel = channel_of(inputs[0].tuned_hz);
        let mut o = inputs[0].with_kind(Kind::typed::<Frame154>());
        o.rate = 0.0;
        let c = o.with_kind(Kind::typed::<DecoderCounts>());
        Ok(vec![o, c])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        for &[v, p, u] in x.as_chunks::<3>().0 {
            self.v.push(v);
            self.p.push(p);
            self.u.push(u);
            // running mean over 128 samples (32 chips)
            let mean = (self.mean.push(v) / 128.0) as f32;
            let ph = (self.pos & 3) as usize;
            self.reg[ph] = (self.reg[ph] << 1) | (v > mean) as u32;
            self.pos += 1;
            if self.quiet[ph] > 0 {
                self.quiet[ph] -= 1;
            }
            // chips 1 to 31 (the low 31 bits of the register) against symbol 0's pattern
            let hit = ((self.reg[ph] ^ self.d0_bits) & 0x7FFF_FFFF).count_ones() <= 7;
            // a match now, 32 chips ago and 64 chips ago on the same phase
            if hit && self.quiet[ph] == 0 && self.pos > self.busy_until && self.matched_before(ph) {
                self.candidates += 1;
                self.pending.push_back(self.pos - 1);
                self.quiet[ph] = 2 * 32;
            }
            self.mark(ph, hit);
            // decode a candidate once its longest frame is in
            while let Some(&t) = self.pending.front() {
                if self.pos <= t + 270 * 128 + 64 {
                    break;
                }
                self.pending.pop_front();
                if t > self.busy_until {
                    self.decode(t);
                }
            }
        }
        self.send(io, false);
    }
    /// Decodes the candidates that still wait for the rest of their longest frame: the input ended, so nothing more comes.
    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        while let Some(t) = self.pending.pop_front() {
            if t > self.busy_until {
                self.decode(t);
            }
        }
        self.send(io, true);
    }
}

impl FrameDec {
    /// Writes the decoded frames, and the counters on a report tick or when `now` is set.
    fn send(&mut self, io: &mut Io, now: bool) {
        for f in self.out.drain(..) {
            io.items_out::<Frame154>(0).push(Timed {
                time: f.time,
                value: f,
            });
        }
        if io.tick() || now {
            let t = self.pos as f64 / CHIP_RATE_X4;
            io.items_out::<DecoderCounts>(1).push(Timed {
                time: t,
                value: DecoderCounts {
                    bad: self.bad,
                    candidates: self.candidates,
                    time_s: t,
                },
            });
        }
    }
}

/// Counters the frame decoder publishes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DecoderCounts {
    /// Bad frames.
    pub bad: u64,
    /// Preamble candidates.
    pub candidates: u64,
    /// The signal time of the count, seconds.
    pub time_s: f64,
}

impl Item for DecoderCounts {
    const KIND: &'static str = "zb.counts";
}

/// What a frame says, for the log and the tables.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Described {
    /// The MAC header.
    pub mac: mac154::Mac,
    /// The details text.
    pub text: String,
    /// Some payload is encrypted.
    pub encrypted: bool,
    /// The PAN of the frame.
    pub pan: Option<u16>,
    /// A Zigbee beacon or NWK header was seen.
    pub zigbee: bool,
    /// Beacon: PAN coordinator, association permit, stack profile, extended PAN.
    pub beacon: Option<(bool, bool, Option<u8>, Option<u64>)>,
    /// NWK source and destination (short), NWK source IEEE.
    pub nwk: Option<(u16, u16, Option<u64>, bool)>,
    /// An (extended, short) pair learnt from the frame.
    pub bind: Option<(u64, u16)>,
}

fn profile_name(p: u16) -> &'static str {
    match p {
        0x0000 => "ZDO",
        0x0101 => "Industrial plant monitoring",
        0x0104 => "Home Automation",
        0x0105 => "Commercial Building Automation",
        0x0107 => "Telecom Applications",
        0x0108 => "Personal Home & Hospital Care",
        0x0109 => "Smart Energy",
        0xA1E0 => "Green Power",
        0xC05E => "ZigBee Light Link",
        _ => "profile",
    }
}

fn cluster_name(profile: u16, c: u16) -> String {
    if profile == 0 {
        let base = match c & 0x7FFF {
            0x0000 => "NWK_addr",
            0x0001 => "IEEE_addr",
            0x0002 => "Node_Desc",
            0x0003 => "Power_Desc",
            0x0004 => "Simple_Desc",
            0x0005 => "Active_EP",
            0x0006 => "Match_Desc",
            0x0013 => return "Device_annce".into(),
            0x0020 => "End_Device_Bind",
            0x0021 => "Bind",
            0x0022 => "Unbind",
            0x0030 => "Mgmt_NWK_Disc",
            0x0031 => "Mgmt_Lqi",
            0x0032 => "Mgmt_Rtg",
            0x0034 => "Mgmt_Leave",
            0x0036 => "Mgmt_Permit_Joining",
            0x0038 => "Mgmt_NWK_Update",
            _ => return "ZDO".into(),
        };
        return format!("{base}{}", if c & 0x8000 != 0 { "_rsp" } else { "_req" });
    }
    match c {
        0x0000 => "Basic",
        0x0001 => "Power Configuration",
        0x0003 => "Identify",
        0x0004 => "Groups",
        0x0005 => "Scenes",
        0x0006 => "On/Off",
        0x0008 => "Level Control",
        0x000A => "Time",
        0x0019 => "OTA Upgrade",
        0x0020 => "Poll Control",
        0x0102 => "Window Covering",
        0x0201 => "Thermostat",
        0x0300 => "Color Control",
        0x0400 => "Illuminance",
        0x0402 => "Temperature",
        0x0403 => "Pressure",
        0x0405 => "Humidity",
        0x0406 => "Occupancy",
        0x0500 => "IAS Zone",
        0x0702 => "Metering",
        0x0B04 => "Electrical Measurement",
        0x1000 => "Touchlink",
        _ => "cluster",
    }
    .into()
}

/// Describes a PSDU without its FCS.
pub fn describe(b: &[u8]) -> Option<Described> {
    let mac = mac154::parse(b)?;
    let end = b.len() - mac.aux.as_ref().map_or(0, |a| a.mic_len).min(b.len());
    let pay = b.get(mac.hdr_len..end).unwrap_or(&[]);
    describe_mac(mac, pay, false)
}

/// Describes a PSDU without its FCS, opening a secured frame of version 2006 with the first of `keys` that passes its MIC
/// (level 4 has none: the first key). `ext` gives the extended address of a short source address in a PAN, for the nonce.
pub fn describe_keyed(
    b: &[u8],
    keys: &[sdrkit_algo_crypto::AesKey],
    ext: impl Fn(u16, Option<u16>) -> Option<u64>,
) -> Option<Described> {
    let mac = mac154::parse(b)?;
    // The frame version 2006 security only: other versions keep their plain description.
    let Some(aux) = mac.aux.clone().filter(|_| !keys.is_empty() && mac.version == 1) else {
        return describe(b);
    };
    let src = match mac.src {
        Addr::Ext(e) => Some(e),
        Addr::Short(s) => ext(s, mac154::pan_of(&mac)),
        Addr::None => None,
    };
    let Some(src) = src else {
        let mut d = describe(b)?;
        d.text.push_str(", source EUI-64 not known");
        return Some(d);
    };
    match security::open(b, &mac, src, keys) {
        Some((plain, k)) => {
            let encrypted = mac.encrypted();
            let mut d = describe_mac(mac, &plain, true)?;
            d.encrypted |= encrypted;
            let how = match aux.level {
                4 => "decrypted, no MIC to check",
                l if l > 4 => "decrypted",
                _ => "MIC good",
            };
            let _ = write!(d.text, ", {how} (key {})", k + 1);
            Some(d)
        }
        None => {
            let mut d = describe(b)?;
            d.text.push_str(", no key fits");
            Some(d)
        }
    }
}

/// Describes the MAC payload `pay` (without its MIC) of a frame with header `mac`; `plain` when an encrypted payload was
/// decrypted.
fn describe_mac(mac: mac154::Mac, pay: &[u8], plain: bool) -> Option<Described> {
    let mut d = Described {
        pan: None,
        ..Default::default()
    };
    d.pan = mac154::pan_of(&mac);
    let seq = mac.seq.unwrap_or(0);
    if mac.encrypted() && !plain {
        d.text = format!("{}, MAC payload encrypted ({} bytes)", mac.type_name(), pay.len());
        d.encrypted = true;
        d.mac = mac;
        return Some(d);
    }
    d.text = match mac.ftype {
        2 => format!("Ack #{seq}"),
        0 => {
            let bcn = mac154::beacon(pay);
            let (coord, permit) = (bcn.coordinator, bcn.permit);
            let bp = pay.get(bcn.payload..).unwrap_or(&[]);
            let pan = mac.src_pan.unwrap_or(0);
            if bp.len() >= 15 && bp[0] == 0 {
                let sp = bp[1] & 0xF;
                let ver = bp[1] >> 4;
                let router = bp[2] & 0x04 != 0;
                let depth = (bp[2] >> 3) & 0xF;
                let ed = bp[2] & 0x80 != 0;
                let ext = u64::from_le_bytes(bp[3..11].try_into().ok()?);
                d.zigbee = true;
                d.beacon = Some((coord, permit, Some(sp), Some(ext)));
                format!(
                    "Zigbee beacon: PAN 0x{pan:04X}, ext PAN {}, stack profile {sp}, NWK version {ver}{}{}, depth {depth}, update id {}, join {}",
                    eui(ext),
                    if router { ", router slots" } else { "" },
                    if ed { ", end-device slots" } else { "" },
                    bp[14],
                    if permit { "open" } else { "closed" }
                )
            } else {
                d.beacon = Some((coord, permit, None, None));
                mac154::beacon_text(pan, &bcn)
            }
        }
        3 => {
            let (text, bind) = mac154::command_text(&mac, pay);
            d.bind = bind;
            text
        }
        1 => describe_nwk(pay, &mut d)
            .unwrap_or_else(|| format!("Data, {} bytes (not Zigbee NWK)", pay.len())),
        _ => format!("{}, {} bytes", mac.type_name(), pay.len()),
    };
    d.mac = mac;
    Some(d)
}

fn describe_nwk(p: &[u8], d: &mut Described) -> Option<String> {
    if p.len() < 8 {
        return None;
    }
    let fc = u16::from_le_bytes([p[0], p[1]]);
    let ft = fc & 3;
    let ver = (fc >> 2) & 0xF;
    if !(ver == 1 || ver == 2) || ft > 1 {
        return None;
    }
    let dst = u16::from_le_bytes([p[2], p[3]]);
    let src = u16::from_le_bytes([p[4], p[5]]);
    let radius = p[6];
    let seq = p[7];
    let mut i = 8;
    if fc & 0x0800 != 0 {
        i += 8;
    }
    let mut src_ieee = None;
    if fc & 0x1000 != 0 {
        src_ieee = Some(u64::from_le_bytes(p.get(i..i + 8)?.try_into().ok()?));
        i += 8;
    }
    if fc & 0x0100 != 0 {
        i += 1;
    }
    if fc & 0x0400 != 0 {
        let n = *p.get(i)? as usize;
        i += 2 + 2 * n;
    }
    d.zigbee = true;
    d.nwk = Some((src, dst, src_ieee, ft == 1));
    let kind = if ft == 0 { "data" } else { "command" };
    if fc & 0x0200 != 0 {
        d.encrypted = true;
        return Some(format!("Zigbee NWK {kind} 0x{src:04X} > 0x{dst:04X}, radius {radius}, seq {seq}, protocol {ver}, payload encrypted (network key)"));
    }
    let rest = p.get(i..)?;
    if ft == 1 {
        let name = match rest.first().copied().unwrap_or(0) {
            1 => "Route request",
            2 => "Route reply",
            3 => "Network status",
            4 => "Leave",
            5 => "Route record",
            6 => "Rejoin request",
            7 => "Rejoin response",
            8 => "Link status",
            9 => "Network report",
            10 => "Network update",
            11 => "End device timeout request",
            12 => "End device timeout response",
            _ => "NWK command",
        };
        return Some(format!(
            "Zigbee NWK {name} 0x{src:04X} > 0x{dst:04X}, radius {radius}"
        ));
    }
    // APS
    let afc = *rest.first()?;
    let at = afc & 3;
    let dm = (afc >> 2) & 3;
    let mut j = 1;
    let mut dep = 0u8;
    if dm == 0 || dm == 2 {
        dep = *rest.get(j)?;
        j += 1;
    } else if dm == 3 {
        j += 2;
    }
    if at == 0 {
        let cl = u16::from_le_bytes([*rest.get(j)?, *rest.get(j + 1)?]);
        let pr = u16::from_le_bytes([*rest.get(j + 2)?, *rest.get(j + 3)?]);
        let sep = *rest.get(j + 4)?;
        let aps_enc = afc & 0x20 != 0;
        if aps_enc {
            d.encrypted = true;
        }
        if pr == 0 && cl == 0x0013 && !aps_enc && p.len() >= 12 {
            let a = &p[p.len() - 12..];
            let short = u16::from_le_bytes([a[1], a[2]]);
            let ieee = u64::from_le_bytes(a[3..11].try_into().ok()?);
            d.bind = Some((ieee, short));
        }
        return Some(format!(
            "Zigbee 0x{src:04X} > 0x{dst:04X}, APS cluster 0x{cl:04X} {}, profile 0x{pr:04X} ({}), ep {sep} > {dep}{}",
            cluster_name(pr, cl),
            profile_name(pr),
            if aps_enc { ", APS encrypted" } else { "" }
        ));
    }
    Some(format!(
        "Zigbee 0x{src:04X} > 0x{dst:04X}, APS {}",
        if at == 1 { "command" } else { "ack" }
    ))
}

/// The generator schedule: (PSDU with FCS, carrier offset Hz, gap after in s).
pub fn schedule() -> Vec<(Vec<u8>, f64, f64)> {
    use mac154::build_header as h;
    let coord_ext = 0x00124B0001020304u64;
    let bulb = 0x001788010A0B0C0Du64;
    let pan = 0x1A62;
    let mut v = Vec::new();
    let mut seq = 0x40u8;
    let mut nx = || {
        seq = seq.wrapping_add(1);
        seq - 1
    };
    let beacon = |seq: u8, pan: u16, permit: bool, ext: [u8; 8]| {
        let mut b = h(
            0,
            false,
            false,
            0,
            seq,
            None,
            Addr::None,
            Some(pan),
            Addr::Short(0),
            false,
        );
        let sf: u16 = 0x0FFF | 0x4000 | if permit { 0x8000 } else { 0 };
        b.extend(sf.to_le_bytes());
        b.extend([0, 0, 0x00, 0x22, if permit { 0x84 } else { 0x00 }]);
        let mut e = ext;
        e.reverse();
        b.extend(e);
        b.extend([0xFF, 0xFF, 0xFF, 0x03]);
        b
    };
    let mut cmd = h(
        3,
        false,
        false,
        0,
        nx(),
        Some(0xFFFF),
        Addr::Short(0xFFFF),
        None,
        Addr::None,
        false,
    );
    cmd.push(7);
    v.push((with_fcs(cmd), 8e3, 0.002));
    v.push((
        with_fcs(beacon(
            nx(),
            pan,
            true,
            [0xDD, 0xDD, 0xDD, 0xDD, 0xDD, 0xDD, 0x1A, 0x62],
        )),
        -12e3,
        0.003,
    ));
    let mut a = h(
        3,
        true,
        false,
        0,
        nx(),
        Some(pan),
        Addr::Short(0),
        Some(0xFFFF),
        Addr::Ext(bulb),
        false,
    );
    a.extend([1, 0x8E]);
    v.push((with_fcs(a), 25e3, 0.002));
    let mut a = h(
        3,
        true,
        true,
        0,
        nx(),
        Some(pan),
        Addr::Ext(bulb),
        None,
        Addr::Ext(coord_ext),
        false,
    );
    a.extend([2, 0x34, 0x12, 0]);
    v.push((with_fcs(a), -12e3, 0.002));
    let dr_seq = nx();
    let mut a = h(
        3,
        true,
        true,
        0,
        dr_seq,
        Some(pan),
        Addr::Short(0),
        None,
        Addr::Short(0x5A5A),
        false,
    );
    a.push(4);
    v.push((with_fcs(a), 25e3, 0.002));
    v.push((with_fcs(vec![0x02, 0x00, dr_seq]), -12e3, 0.002));
    let mut d = h(
        1,
        false,
        true,
        0,
        nx(),
        Some(pan),
        Addr::Short(0xFFFF),
        None,
        Addr::Short(0x1234),
        false,
    );
    d.extend([0x08, 0x00, 0xFD, 0xFF, 0x34, 0x12, 30, 7]);
    d.extend([0x08, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 9]);
    d.extend([7, 0x34, 0x12]);
    d.extend(bulb.to_le_bytes());
    d.push(0x8E);
    v.push((with_fcs(d), 25e3, 0.002));
    let mut d = h(
        1,
        false,
        true,
        0,
        nx(),
        Some(pan),
        Addr::Short(0xFFFC),
        None,
        Addr::Short(0),
        false,
    );
    d.extend([0x09, 0x00, 0xFC, 0xFF, 0x00, 0x00, 1, 8, 0x08, 0x60]);
    v.push((with_fcs(d), -12e3, 0.002));
    for (ctr, (src, dst, ieee, off)) in (1u32..).zip([
        (0x7A12u16, 0x0000u16, Some(0x00158DFFFE123456u64), 40e3),
        (0x0000, 0x1234, None, -30e3),
        (0x1234, 0x0000, None, -30e3),
    ]) {
        let mut d = h(
            1,
            true,
            true,
            0,
            nx(),
            Some(pan),
            Addr::Short(dst),
            None,
            Addr::Short(src),
            false,
        );
        let fc: u16 = 0x0008 | 0x0040 | 0x0200 | if ieee.is_some() { 0x1000 } else { 0 };
        d.extend(fc.to_le_bytes());
        d.extend(dst.to_le_bytes());
        d.extend(src.to_le_bytes());
        d.extend([30, 7]);
        if let Some(e) = ieee {
            d.extend(e.to_le_bytes());
        }
        d.push(0x28);
        d.extend(ctr.to_le_bytes());
        d.extend(ieee.unwrap_or(0x00124B0001020304).to_le_bytes());
        d.push(0);
        d.extend((0..12u8).map(|i| i.wrapping_mul(37).wrapping_add(11)));
        v.push((with_fcs(d), off, 0.002));
    }
    v.push((
        with_fcs(beacon(
            nx(),
            0x3C2D,
            false,
            [0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0x07, 0x18],
        )),
        15e3,
        0.003,
    ));
    let mut d = h(
        1,
        false,
        true,
        0,
        nx(),
        Some(0x3C2D),
        Addr::Ext(coord_ext),
        None,
        Addr::Ext(0x14B457FFFE654321),
        false,
    );
    d.extend([0xDE, 0xAD, 0xBE, 0xEF]);
    v.push((with_fcs(d), 15e3, 0.003));
    v
}

/// Networks kept.
pub const NETWORK_CAP: usize = 100;
/// Devices kept.
pub const DEVICE_CAP: usize = 600;
/// Frames kept in the log.
pub const FRAME_CAP: usize = 600;
/// Seconds of the frames-per-second history.
pub const RATE_HISTORY: usize = 120;
/// New PANs that each give an event.
pub const PAN_EVENTS: u32 = 20;

/// A network, keyed by PAN and channel.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Network {
    /// The PAN id.
    pub pan: u16,
    /// The channel (0 when not known).
    pub channel: u32,
    /// The extended PAN id.
    pub ext_pan: Option<String>,
    /// The coordinator.
    pub coordinator: Option<String>,
    /// A Zigbee beacon or NWK header was seen.
    pub zigbee: bool,
    /// The last beacon allowed joining.
    pub join_open: bool,
    /// The Zigbee stack profile.
    pub stack_profile: Option<u8>,
    /// The devices in the PAN.
    pub devices: usize,
    /// The last level, dBFS.
    pub level_db: f32,
}

impl Shift for Network {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// The PAN id as text: "0x1A62".
pub fn pan_text(p: u16) -> String {
    format!("0x{p:04X}")
}

impl Shaped for Network {
    type Row = ZigbeeNetworkRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&ZigbeeNetworkRow {
            pan: &pan_text(self.pan),
            ext_pan: self.ext_pan.as_deref(),
            ch: i64::from(self.channel),
            coordinator: self.coordinator.as_deref(),
            devices: self.devices as i64,
            level_dbfs: f64::from(self.level_db),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// A device, found by its short address within its PAN or by its extended address.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// The short address.
    pub short: Option<u16>,
    /// The extended address.
    pub ext: Option<u64>,
    /// The vendor of the extended address.
    pub vendor: Option<String>,
    /// The PAN.
    pub pan: Option<u16>,
    /// The channel.
    pub channel: u32,
    /// "coordinator", "router" or "device".
    pub role: String,
    /// The last level, dBFS.
    pub level_db: f32,
    /// The last frame description.
    pub last_frame: String,
}

impl Shift for Device {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Device {
    type Row = PanDeviceRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let short = self.short.map(|s| format!("0x{s:04X}"));
        let extended = self.ext.map(eui);
        let pan = self.pan.map(pan_text);
        out.put(&PanDeviceRow {
            short: short.as_deref(),
            extended: extended.as_deref(),
            vendor: self.vendor.as_deref(),
            pan: pan.as_deref(),
            ch: i64::from(self.channel),
            role: &self.role,
            level_dbfs: f64::from(self.level_db),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
            last_frame: &self.last_frame,
        });
    }
}

/// A line of the frame log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameLine {
    /// The signal time, seconds.
    pub at: f64,
    /// The channel.
    pub channel: u32,
    /// The MAC frame type.
    pub kind: String,
    /// "source > destination".
    pub route: String,
    /// The PAN.
    pub pan: Option<u16>,
    /// The PSDU length with the FCS.
    pub len: usize,
    /// Some payload is encrypted.
    pub encrypted: bool,
    /// The level, dBFS.
    pub level_db: f32,
    /// The description.
    pub details: String,
}

impl Shift for FrameLine {
    fn shift_times(&mut self, by_s: f64) {
        self.at += by_s;
    }
}

impl Shaped for FrameLine {
    type Row = ZigbeeFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let pan = self.pan.map(pan_text);
        out.put(&ZigbeeFrameRow {
            time_s: self.at,
            ch: i64::from(self.channel),
            r#type: &self.kind,
            route: &self.route,
            pan: pan.as_deref(),
            len: self.len as i64,
            encrypted: self.encrypted,
            level_dbfs: f64::from(self.level_db),
            details: &self.details,
        });
    }
}

/// What `zb.tables` keeps across sessions.
#[derive(Serialize, Deserialize)]
pub struct Store {
    networks: ObjectTable<u64, Network>,
    devices: Devices,
    frames: LogRing<FrameLine>,
    good: u64,
    bad: u64,
    candidates: u64,
    encrypted: u64,
    per_type: [u64; 8],
    per_channel: [u64; 16],
    per_second: VecDeque<u32>,
    new_pans: u32,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            networks: ObjectTable::new(NETWORK_CAP),
            devices: Devices::new(DEVICE_CAP),
            frames: LogRing::new(FRAME_CAP),
            good: 0,
            bad: 0,
            candidates: 0,
            encrypted: 0,
            per_type: [0; 8],
            per_channel: [0; 16],
            per_second: VecDeque::with_capacity(RATE_HISTORY + 1),
            new_pans: 0,
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.networks.shift_times(by_s);
        self.devices.table.shift_times(by_s);
        self.frames.shift_times(by_s);
    }
}

impl sdrkit_util_objects::Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("ism.zigbee", 1);
    fn clear(&mut self) {
        let keep = (
            self.networks.cap(),
            self.devices.table.cap(),
            self.frames.cap(),
        );
        *self = Store::default();
        self.networks.set_cap(keep.0);
        self.devices.table.set_cap(keep.1);
        self.frames.set_cap(keep.2);
    }
    fn adopt(&mut self, saved: Self) {
        self.networks.adopt(saved.networks);
        self.devices.table.adopt(saved.devices.table);
        self.devices.next = saved.devices.next;
        self.frames.adopt(saved.frames);
        self.good = saved.good;
        self.bad = saved.bad;
        self.candidates = saved.candidates;
        self.encrypted = saved.encrypted;
        self.per_type = saved.per_type;
        self.per_channel = saved.per_channel;
        self.per_second = saved.per_second;
        self.new_pans = saved.new_pans;
    }
}

/// The rank of a role: a role only rises.
fn role_rank(r: &str) -> u8 {
    match r {
        "coordinator" => 2,
        "router" => 1,
        _ => 0,
    }
}

/// The devices of a receiver of 802.15.4 frames, each under a running number: a device is found by its extended address, or by
/// its short address within its PAN.
#[derive(Serialize, Deserialize)]
pub struct Devices {
    /// The devices.
    pub table: ObjectTable<u32, Device>,
    next: u32,
}

impl Devices {
    /// An empty table of at most `cap` devices.
    pub fn new(cap: usize) -> Devices {
        Devices {
            table: ObjectTable::new(cap),
            next: 0,
        }
    }

    /// The id of the device with this extended address, or this short address in this PAN.
    pub fn find(&self, short: Option<u16>, ext: Option<u64>, pan: Option<u16>) -> Option<u32> {
        self.table
            .iter()
            .find(|r| {
                let x = r.record;
                (ext.is_some() && x.ext == ext)
                    || (short.is_some()
                        && x.short == short
                        && (x.pan == pan || pan.is_none() || x.pan.is_none()))
            })
            .map(|r| *r.key)
    }

    /// The device of these addresses (made when new), heard at `at`.
    pub fn device(
        &mut self,
        short: Option<u16>,
        ext: Option<u64>,
        pan: Option<u16>,
        ch: u32,
        at: Stamp,
    ) -> u32 {
        let id = match self.find(short, ext, pan) {
            Some(id) => id,
            None => {
                self.next = self.next.wrapping_add(1);
                self.next
            }
        };
        if let Ok(up) = self.table.upsert(&id, at, || Device {
            channel: ch,
            role: "device".into(),
            ..Default::default()
        }) {
            let dv = up.row;
            dv.short = dv.short.or(short);
            dv.ext = dv.ext.or(ext);
            dv.pan = dv.pan.or(pan);
            if let Some(e) = dv.ext {
                dv.vendor = vendor_of_address(&e.to_be_bytes()).map(String::from);
            }
        }
        id
    }

    /// Joins the extended address `ext` and the short address `short` of one device.
    pub fn merge(&mut self, ext: u64, short: u16, pan: Option<u16>, ch: u32, at: Stamp) {
        let a = self
            .table
            .iter()
            .find(|r| r.record.ext == Some(ext))
            .map(|r| *r.key);
        let b = self
            .table
            .iter()
            .find(|r| {
                let x = r.record;
                x.short == Some(short) && x.ext.is_none() && (x.pan == pan || x.pan.is_none())
            })
            .map(|r| *r.key);
        match (a, b) {
            (Some(i), Some(j)) if i != j => {
                self.table.remove(&j);
                if let Some(dv) = self.table.get_mut(&i) {
                    dv.short = Some(short);
                }
            }
            (Some(i), _) => {
                if let Some(dv) = self.table.get_mut(&i) {
                    dv.short = Some(short);
                }
            }
            (None, Some(j)) => {
                if let Some(dv) = self.table.get_mut(&j) {
                    dv.ext = Some(ext);
                    dv.vendor = vendor_of_address(&ext.to_be_bytes()).map(String::from);
                }
            }
            (None, None) => {
                self.device(Some(short), Some(ext), pan, ch, at);
            }
        }
    }
}

impl Store {
    /// Takes a good frame, opening a secured one with `keys`. Returns the event of a new PAN.
    fn frame(&mut self, f: &Frame154, at: Stamp, keys: &[sdrkit_algo_crypto::AesKey]) -> Option<Event> {
        let n = f.psdu.len();
        let devices = &self.devices;
        let ext = |short: u16, pan: Option<u16>| {
            let id = devices.find(Some(short), None, pan)?;
            devices.table.get(&id)?.ext
        };
        let ds = describe_keyed(&f.psdu[..n.saturating_sub(2)], keys, ext)?;
        self.good += 1;
        let m = &ds.mac;
        self.per_type[(m.ftype & 7) as usize] += 1;
        if (11..=26).contains(&f.channel) {
            self.per_channel[(f.channel - 11) as usize] += 1;
        }
        if ds.encrypted {
            self.encrypted += 1;
        }
        self.frames.push(FrameLine {
            at: f.time,
            channel: f.channel,
            kind: m.type_name().into(),
            route: format!("{} > {}", m.src.text(), m.dst.text()),
            pan: ds.pan,
            len: n,
            encrypted: ds.encrypted,
            level_db: f.level_db,
            details: ds.text.clone(),
        });
        let mut event = None;
        if let Some(pan) = ds.pan {
            let key = (pan as u64) << 8 | f.channel as u64;
            if let Ok(up) = self.networks.upsert(&key, at, || Network {
                pan,
                channel: f.channel,
                ..Default::default()
            }) {
                if up.new && self.new_pans < PAN_EVENTS {
                    self.new_pans += 1;
                    let pan_id = pan_text(pan);
                    event = Some(ZIGBEE_PAN.event(
                        f.time,
                        f.freq_hz,
                        &ZigbeePanEvent {
                            source: &pan_id,
                            text: &format!("Zigbee: new PAN {pan_id} on channel {}", f.channel),
                            family: Family::Sensor,
                            pan: &pan_id,
                            ch: i64::from(f.channel),
                        },
                    ));
                }
                let nw = up.row;
                nw.level_db = f.level_db;
                nw.zigbee |= ds.zigbee;
                if let Some((coord, permit, sp, ext)) = ds.beacon {
                    nw.join_open = permit;
                    if sp.is_some() {
                        nw.stack_profile = sp;
                    }
                    if let Some(e) = ext {
                        nw.ext_pan = Some(eui(e));
                    }
                    if coord || nw.coordinator.is_none() {
                        nw.coordinator = Some(m.src.text());
                    }
                }
                if let Some((0, _, _, _)) = ds.nwk {
                    nw.coordinator.get_or_insert("0x0000".into());
                }
            }
        }
        let (short, ext) = match m.src {
            Addr::Short(s) => (Some(s), None),
            Addr::Ext(e) => (None, Some(e)),
            Addr::None => (None, None),
        };
        if short.is_some() || ext.is_some() {
            let pan = m.src_pan.filter(|&p| p != 0xFFFF).or(ds.pan);
            let id = self.devices.device(short, ext, pan, f.channel, at);
            let mut role = "device";
            if let Some((coord, _, _, _)) = ds.beacon {
                role = if coord { "coordinator" } else { "router" };
            }
            if short == Some(0) {
                role = "coordinator";
            }
            if matches!(ds.nwk, Some((_, _, _, true))) && role == "device" {
                role = "router";
            }
            if let Some(dv) = self.devices.table.get_mut(&id) {
                dv.level_db = f.level_db;
                dv.last_frame.clone_from(&ds.text);
                if role_rank(role) > role_rank(&dv.role) {
                    dv.role = role.into();
                }
            }
        }
        if let Some((src, _, ieee, _)) = ds.nwk {
            if let Some(e) = ieee {
                self.devices.merge(e, src, ds.pan, f.channel, at);
            }
            if Addr::Short(src) != m.src {
                self.devices.device(Some(src), None, ds.pan, f.channel, at);
            }
        }
        if let Some((e, s)) = ds.bind {
            self.devices.merge(e, s, ds.pan, f.channel, at);
        }
        let counts: Vec<(u64, usize)> = self
            .networks
            .iter()
            .map(|r| {
                let (pan, ch) = (r.record.pan, r.record.channel);
                let n = self
                    .devices
                    .table
                    .iter()
                    .filter(|d| {
                        let x = d.record;
                        x.pan == Some(pan) && (x.channel == ch || x.channel == 0 || ch == 0)
                    })
                    .count();
                (*r.key, n)
            })
            .collect();
        for (k, n) in counts {
            if let Some(nw) = self.networks.get_mut(&k) {
                nw.devices = n;
            }
        }
        event
    }
}

/// The channel.
pub const CHANNEL_SETTING: Setting<field::I64> = Setting::new("channel", "Channel", "", 11)
    .range(11.0, 26.0, 1.0)
    .help("Asks the host to tune 2405 + 5 (k - 11) MHz.");
const SCAN: Setting<field::Bool> = Setting::new("scan", "Scan channels 11 to 26", "", false)
    .help("Asks the host to visit each channel in turn.");
/// The time on each channel of a scan.
pub const DWELL: Setting<field::Duration> =
    Setting::new("dwell", "Scan dwell", "s", 5.0).range(1.0, 60.0, 1.0);
/// The keys that open secured MAC frames.
pub const KEYS: Setting<field::Text> = Setting::new("keys", "Keys (kept in memory only)", "", "").help(
    "AES-128 keys of 32 hex digits, separated by ';', tried in turn on each secured MAC frame (IEEE 802.15.4 CCM*). \
     Empty: secured payloads are shown as encrypted.",
);

sdrkit_core::settings! {
    /// The tuning settings of [`Tables`].
    pub enum TablesSetting {
        /// The channel.
        Channel = CHANNEL_SETTING,
        /// Scan the channels.
        Scan = SCAN,
        /// The scan dwell.
        Dwell = DWELL,
        /// The keys of secured frames.
        Keys = KEYS,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Tables`].
    pub enum TablesCommand {
        /// Forgets everything heard.
        Clear = commands::CLEAR,
        /// Forgets one device.
        Remove = PAN_REMOVE.command,
    }
}

/// `zb.tables`: networks, devices, the frame log, the counters, the status and the host requests of the tuning settings.
pub struct Tables {
    published: Published,
    devices_table: TableId,
    rate_series: Option<SeriesId>,
    summary: String,
    store: Store,
    last: Option<(f32, f32, f64)>,
    events: Vec<Event>,
    requests: Vec<HostRequest>,
    now: f64,
    last_good: f64,
    sec_count: u32,
    sec_acc: f64,
    channel: u32,
    want_channel: u32,
    scan: bool,
    dwell_s: f64,
    keys_text: String,
    keys: Vec<sdrkit_algo_crypto::AesKey>,
    configured: bool,
    live: bool,
}

impl Default for Tables {
    fn default() -> Self {
        let published = Published::from_decl(Arc::new(
            ResultsDecl::new()
                .with_fields(ZigbeeFields::SHAPE.fields)
                .with_table(&ZIGBEE_NETWORKS)
                .with_table(&PAN_DEVICES)
                .with_table(&ZIGBEE_FRAMES)
                .with_series(&ZIGBEE_RATE),
        ));
        let decl = published.results().decl();
        let devices_table = decl
            .table_id(PAN_DEVICES.key)
            .expect("the tables of zb.tables are declared");
        let rate_series = decl.series_id(ZIGBEE_RATE.key);
        Tables {
            published,
            devices_table,
            rate_series,
            summary: String::new(),
            store: Store::default(),
            last: None,
            events: Vec::new(),
            requests: Vec::new(),
            now: 0.0,
            last_good: f64::NEG_INFINITY,
            sec_count: 0,
            sec_acc: 0.0,
            channel: 0,
            want_channel: 11,
            scan: false,
            dwell_s: 5.0,
            keys_text: String::new(),
            keys: Vec::new(),
            configured: false,
            live: false,
        }
    }
}

impl Tables {
    fn stamp(&self, t: f64) -> Stamp {
        match crate::family::wall(self.live) {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        }
    }
    fn publish(&mut self) {
        let s = &self.store;
        let state = (self.now - self.last_good < 5.0 && s.good > 0) as u8 * 2;
        self.summary.clear();
        if s.good > 0 {
            let _ = write!(
                self.summary,
                "{} networks, {} devices, {} frames",
                s.networks.len(),
                s.devices.table.len(),
                s.good
            );
        } else if self.channel > 0 {
            let _ = write!(self.summary, "listening on channel {}", self.channel);
        } else {
            self.summary.push_str("listening for 802.15.4 frames");
        }
        let newest = self.last.map(|l| l.0);
        let p = &mut self.published;
        set_status(p, LockState::from_level(state), &self.summary);
        {
            let r = p.results_mut();
            r.update_status(|st| {
                let changed = st.level_dbfs != newest;
                st.level_dbfs = newest;
                changed
            });
            if let Some(id) = self.rate_series {
                let n = s.per_second.len() as f32;
                let pts = s
                    .per_second
                    .iter()
                    .enumerate()
                    .map(|(i, &c)| [i as f32 - n, c as f32]);
                r.update_series(id, |series| {
                    if series.line && series.points.iter().copied().eq(pts.clone()) {
                        return false;
                    }
                    series.line = true;
                    series.points.clear();
                    series.points.extend(pts);
                    true
                });
            }
        }
        p.put(&ZigbeeFields {
            frames_ok: s.good as i64,
            frames_bad: s.bad as i64,
            candidates: s.candidates as i64,
            encrypted_frames: s.encrypted as i64,
            networks: s.networks.len() as i64,
            devices: s.devices.table.len() as i64,
            channel: i64::from(self.channel),
            frames_per_s: i64::from(s.per_second.back().copied().unwrap_or(0)),
            snr_db: self.last.map(|l| f64::from(l.1)),
            offset_hz: self.last.map(|l| Hertz(l.2)),
        });
        shaped::rows(&s.networks, &ZIGBEE_NETWORKS, self.now, p);
        shaped::rows(&s.devices.table, &PAN_DEVICES, self.now, p);
        shaped::log_rows(&s.frames, &ZIGBEE_FRAMES, self.now, p);
    }
    /// The request that a tuning setting requires, once the receiver runs.
    fn tune_request(&mut self) {
        if !self.configured {
            return;
        }
        self.requests.push(if self.scan {
            HostRequest::Hop {
                channels: (11..=26).map(channel_hz).collect(),
                dwell_s: self.dwell_s,
                stop_on_lock: false,
            }
        } else {
            HostRequest::retune(channel_hz(self.want_channel), "Zigbee channel")
        });
    }
}

impl Component for Tables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "zb.tables",
            vec![
                Port::typed::<Frame154>("frames"),
                Port::typed::<DecoderCounts>("counts"),
            ],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(TablesSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.channel = channel_of(inputs[0].tuned_hz);
        self.configured = true;
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn attach(&mut self, ctx: &sdrkit_graph::RunContext) {
        self.live = ctx.live;
    }
    fn process(&mut self, io: &mut Io) {
        let mut new = false;
        for f in io.items::<Frame154>(0) {
            let at = self.stamp(f.value.time);
            self.now = self.now.max(f.value.time);
            if let Some(e) = self.store.frame(&f.value, at, &self.keys) {
                self.events.push(e);
            }
            self.last = Some((f.value.level_db, f.value.snr_db, f.value.offset_hz));
            self.last_good = f.value.time;
            self.sec_count += 1;
            new = true;
        }
        for c in io.items::<DecoderCounts>(1) {
            self.store.bad = c.value.bad;
            self.store.candidates = c.value.candidates;
            self.now = self.now.max(c.value.time_s);
        }
        if io.tick() {
            self.sec_acc += REPORT_S;
            if self.sec_acc >= 1.0 {
                self.sec_acc -= 1.0;
                let q = &mut self.store.per_second;
                if q.len() == RATE_HISTORY {
                    q.pop_front();
                }
                q.push_back(self.sec_count);
                self.sec_count = 0;
            }
        }
        if new || io.tick() {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
        for r in self.requests.drain(..) {
            io.items_out::<HostRequest>(1).push(Timed {
                time: self.now,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TablesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TablesSetting::Channel => {
                self.want_channel = CHANNEL_SETTING.read(id, v)? as u32;
                self.scan = false;
            }
            TablesSetting::Scan => self.scan = SCAN.read(id, v)?,
            TablesSetting::Dwell => self.dwell_s = DWELL.read(id, v)?,
            TablesSetting::Keys => {
                let t = KEYS.read(id, v)?;
                self.keys = security::keys_of(t).ok_or(SettingError::Malformed { id, at: None })?;
                t.clone_into(&mut self.keys_text);
                return Ok(Applied::Live);
            }
        }
        self.tune_request();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                TablesSetting::Channel,
                Value::I64(i64::from(self.want_channel)),
            ),
            (TablesSetting::Scan, Value::Bool(self.scan)),
            (TablesSetting::Dwell, Value::Duration(self.dwell_s)),
            (TablesSetting::Keys, Value::Text(self.keys_text.clone())),
        ])
    }
    fn commands(&self) -> &Commands {
        TablesCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TablesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TablesCommand::Clear => {
                self.store.clear();
                self.published.clear_rows(&ZIGBEE_NETWORKS);
                self.published.clear_rows(&PAN_DEVICES);
                self.published.clear_rows(&ZIGBEE_FRAMES);
            }
            TablesCommand::Remove => commands::remove(
                &mut self.store.devices.table,
                self.devices_table,
                &mut self.published,
                args,
                &PAN_DEVICE_ROW,
            )?,
        }
        self.publish();
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        let s = &mut self.store;
        if !kept.table(ZIGBEE_NETWORKS.key) {
            s.networks.clear();
            self.published.clear_rows(&ZIGBEE_NETWORKS);
        }
        if !kept.table(PAN_DEVICES.key) {
            s.devices.table.clear();
            self.published.clear_rows(&PAN_DEVICES);
        }
        if !kept.table(ZIGBEE_FRAMES.key) {
            s.frames.clear();
            self.published.clear_rows(&ZIGBEE_FRAMES);
        }
        for (key, n) in [
            ("frames_ok", &mut s.good),
            ("frames_bad", &mut s.bad),
            ("candidates", &mut s.candidates),
            ("encrypted_frames", &mut s.encrypted),
        ] {
            if !kept.field(key) {
                *n = 0;
            }
        }
        s.per_second.clear();
        self.sec_count = 0;
        self.sec_acc = 0.0;
        self.last = None;
        self.last_good = f64::NEG_INFINITY;
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

/// Every frame with a bad FCS in the test signal.
pub const GEN_BAD_FCS: Setting<field::Bool> =
    Setting::new("gen_bad_fcs", "Every frame with a bad FCS", "", false)
        .section("Test signal")
        .advanced();
/// The level of the adjacent transmitter of the test signal (0 for none).
pub const GEN_INTERFERER_DB: Setting<field::F64> = Setting::new(
    "gen_interferer_db",
    "Adjacent transmitter level (0 off)",
    "dB",
    0.0,
)
.range(0.0, 50.0, 1.0)
.section("Test signal")
.advanced();
/// The offset of the adjacent transmitter of the test signal.
pub const GEN_INTERFERER_HZ: Setting<field::Hz> = Setting::new(
    "gen_interferer_hz",
    "Adjacent transmitter offset",
    "Hz",
    5e6,
)
.range(-10e6, 10e6, 1e5)
.section("Test signal")
.advanced();

sdrkit_core::settings! {
    /// The settings of the `zigbee` stack.
    pub enum ZigbeeSetting {
        /// The channel.
        Channel = CHANNEL_SETTING,
        /// Scan the channels.
        Scan = SCAN,
        /// The scan dwell.
        Dwell = DWELL,
        /// The keys of secured frames.
        Keys = KEYS,
        /// Every frame with a bad FCS in the test signal.
        GenBadFcs = GEN_BAD_FCS,
        /// The level of the adjacent transmitter of the test signal.
        GenInterfererDb = GEN_INTERFERER_DB,
        /// The offset of the adjacent transmitter of the test signal.
        GenInterfererHz = GEN_INTERFERER_HZ,
        /// The level sent.
        TxLevel = crate::tx::TX_LEVEL,
    }
}

/// The least sample rate: two channel widths.
pub const MIN_RATE_HZ: f64 = 4e6;

/// The `zigbee` receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among
/// them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add("front", Front::default())
        .add("chips", Chips::default())
        .add("frame", FrameDec::default())
        .add("tables", Tables::default())
        .add("sender", crate::tx::FrameSender::<send::ZigbeeFrames>::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("front")
        .input("sender.in")
        .connect("front", "chips")
        .connect("chips", "frame")
        .connect("frame.frames", "tables.frames")
        .connect("frame.counts", "tables.counts")
        .connect("sender.out", "tx.in")
        .output("tables.events")
        .output("tables.requests")
        .output("sender.events")
        .clock(REPORT_S)
        .status_from("tables")
        .build()?;
    StackGraph::of(crate::typed::shape("zigbee"), graph)
        .min_rate(
            MIN_RATE_HZ,
            &min_rate_message("Zigbee / 802.15.4", MIN_RATE_HZ),
        )
        .route(CHANNEL_SETTING.key(), &["tables.channel"])?
        .route(SCAN.key(), &["tables.scan"])?
        .route(DWELL.key(), &["tables.dwell"])?
        .route(KEYS.key(), &["tables.keys"])?
        .route(crate::tx::TX_LEVEL.key(), &["sender.tx_level"])?
        .command("clear", sdrkit_graph::CommandTarget::of("tables", "clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("tables", "remove"),
        )
        .command("send", sdrkit_graph::CommandTarget::of("sender", "send"))
        .command("halt", sdrkit_graph::CommandTarget::of("sender", "halt"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The PSDU with its FCS (CRC-16/KERMIT, low octet first).
pub fn with_fcs(mut b: Vec<u8>) -> Vec<u8> {
    let r = CRC16_KERMIT.append(&mut b, Trailer::Little);
    debug_assert!(r.is_ok(), "a 16-bit FCS");
    b
}

/// The symbols of a PPDU: preamble (8 zero symbols, or 4 with `short_preamble`), SFD, PHR and the PSDU low nibble first.
/// `bad_fcs` sends the last symbol XOR 3.
pub fn ppdu_symbols(psdu: &[u8], short_preamble: bool, bad_fcs: bool) -> Vec<usize> {
    let mut syms: Vec<usize> = vec![0; if short_preamble { 4 } else { 8 }];
    syms.extend([7, 0xA]);
    syms.push(psdu.len() & 0xF);
    syms.push((psdu.len() >> 4) & 7);
    for &b in psdu {
        syms.push((b & 0xF) as usize);
        syms.push((b >> 4) as usize);
    }
    if bad_fcs {
        if let Some(l) = syms.last_mut() {
            *l ^= 3;
        }
    }
    syms
}

/// The chip pairs of a PPDU as unit-power O-QPSK symbols: even chips on I, odd chips on Q.
pub fn ppdu_chip_pairs(psdu: &[u8], short_preamble: bool, bad_fcs: bool) -> Vec<Complex32> {
    let k = std::f32::consts::FRAC_1_SQRT_2;
    let mut v = Vec::new();
    for s in ppdu_symbols(psdu, short_preamble, bad_fcs) {
        for n in (0..32).step_by(2) {
            v.push(Complex32::new(k * chip(s, n), k * chip(s, n + 1)));
        }
    }
    v
}

/// O-QPSK with half-sine pulses at 2 Mchip/s (`sdrkit_algo_sim::modulate::Linear`, offset Q rail) of chip pairs, at `rate`.
///
/// # Errors
///
/// The errors of the modulator for a rate it refuses.
pub fn oqpsk(pairs: &[Complex32], rate: f64) -> Result<Vec<Complex32>, ConfigError> {
    let mut m =
        Linear::new(sdrkit_algo_dsp::pulse::Shape::HalfSine, 1.0, rate / 1e6)?.with_offset_q();
    let mut out = Vec::with_capacity((pairs.len() as f64 * rate / 1e6) as usize + 16);
    m.push(pairs, &mut out);
    m.flush(&mut out);
    Ok(out)
}

/// What the `zigbee` generator sends besides its schedule.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GenOptions {
    /// Every frame with a bad FCS.
    pub bad_fcs: bool,
    /// The level of a continuous O-QPSK transmitter of random chips over the frames, dB (0 for none).
    pub interferer_db: f64,
    /// Its offset from the channel.
    pub interferer_hz: f64,
}

/// The generator options from settings that follow the schema of the stack.
pub fn gen_options(s: &Values) -> GenOptions {
    GenOptions {
        bad_fcs: s.get(&GEN_BAD_FCS),
        interferer_db: s.get(&GEN_INTERFERER_DB),
        interferer_hz: s.get(&GEN_INTERFERER_HZ),
    }
}

/// The `zigbee` generator: the test schedule at the centre of the samples, each frame at its own carrier offset,
/// in a loop, with the options. The clean signal only: the frames at unit power, or, with an adjacent transmitter, that
/// transmitter at unit power and the frames `interferer_db` below it (so no sample is beyond full scale).
///
/// # Errors
///
/// The errors of the scene and of the modulator for a rate they refuse.
pub fn generator(rate: f64, seed: u64, o: GenOptions) -> Result<CleanScene, ConfigError> {
    let sched = schedule();
    let frame_db = -o.interferer_db.max(0.0);
    let mut scene = Scene::new(rate)?;
    let mut t = 0.0005f64;
    let mut k = 0usize;
    scene.add_source(Box::new(move || {
        let (psdu, off, gap) = &sched[k % sched.len()];
        k += 1;
        let samples = oqpsk(&ppdu_chip_pairs(psdu, false, o.bad_fcs), rate).ok()?;
        let b = Burst {
            start_s: t,
            offset_hz: *off,
            level_db: frame_db,
            samples,
        };
        t += b.samples.len() as f64 / rate + gap;
        Some(b)
    }));
    if o.interferer_db > 0.0 {
        let mut rng = Rng::stream(seed, 0x1F);
        let k = std::f32::consts::FRAC_1_SQRT_2;
        let pairs: Vec<Complex32> = (0..50_000)
            .map(|_| {
                let a = if rng.bit() == 1 { k } else { -k };
                let b = if rng.bit() == 1 { k } else { -k };
                Complex32::new(a, b)
            })
            .collect();
        let s = oqpsk(&pairs, rate)?;
        let period = s.len() as f64 / rate;
        scene.add_repeating(0.0, period, o.interferer_hz, 0.0, s)?;
    }
    let info = SignalInfo::new(2e6)
        .with_symbol_rate(2e6)
        .with_bit_rate(250e3)
        .with_power(10f64.powf(frame_db / 10.0));
    Ok(CleanScene::new(scene, rate, info))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let o = gen_options(&cfg.settings);
    Ok(Box::new(generator(cfg.rate_hz, cfg.seed, o)?))
}

/// Zigbee / IEEE 802.15.4.
pub static ZIGBEE: StackDescriptor = StackDescriptor {
    id: "zigbee",
    name: "Zigbee / 802.15.4",
    group: "IoT, 2.4 GHz",
    blurb: "IEEE 802.15.4 frames on 2.4 GHz channels 11 to 26: MAC headers, beacons, Zigbee NWK and APS headers and device announcements; secured MAC frames opened with the keys of the setting `keys`, else shown as encrypted.",
    tuning: Tuning { min_hz: 2405e6, max_hz: 2480e6, tune_offset_hz: 1e6, ..Tuning::anywhere(2405e6, 4e6, 4e6, 2e6) },
    submodes: &[],
    settings: ZigbeeSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Ieee802154],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, two PANs", o, 0.0);
    c.seconds = o.seconds.max(1.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = ZigbeeFields::read(r.fields());
        let n = |v: fn(&ZigbeeFields) -> i64| f.as_ref().map_or(0, v);
        vec![
            at_least_value("frames_ok", n(|f| f.frames_ok), 20),
            at_least_value("networks", n(|f| f.networks), 2),
            at_least_value("encrypted_frames", n(|f| f.encrypted_frames), 3),
        ]
    });
    vec![c]
}

/// The verify case of `zigbee`.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The benchmark of the Zigbee receiver: 0.1 s of the test signal at 4 Msps, fed in one piece.
pub fn benchmarks() -> Vec<sdrkit_core::bench::Benchmark> {
    vec![sdrkit_core::bench::Benchmark {
        name: "ism.zigbee_receiver_4m",
        run: bench_receiver,
    }]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::bench::BenchResult {
    let rate = 4e6;
    let mut x = vec![Complex32::default(); (rate * 0.1) as usize];
    if let Ok(mut g) = generator(rate, 1, GenOptions::default()) {
        g.generate(&mut x);
    }
    let Ok(mut rx) = receiver(&StackConfig::new(
        Values::defaults(&ZigbeeSetting::schema()),
    )) else {
        return sdrkit_core::bench::BenchResult::default();
    };
    let _ = rx.configure(&sdrkit_core::InputInfo::new(rate, 2405e6, 2405e6).recording());
    let mut out = sdrkit_core::Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), 0.1, || {
        out.clear();
        rx.feed(&x, &mut out);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_pattern_matches_the_modulator() {
        // the phase step of each chip of a stream of symbol 0 against d[0][n]
        let pairs: Vec<Complex32> = (0..4)
            .flat_map(|_| {
                (0..32)
                    .step_by(2)
                    .map(|n| Complex32::new(chip(0, n), chip(0, n + 1)))
            })
            .collect();
        let s = oqpsk(&pairs, 8e6).unwrap();
        let d = sign_patterns();
        // symbol 2 starts at chip 64, and the pulse of chip 0 starts at output 0, so chip n of it ends at sample (64 + n + 1) * 4
        let mut agree = 0;
        for n in 1..32 {
            let end = (64 + n + 1) * 4;
            let p = s[end] * s[end - 4].conj();
            if (p.im > 0.0) == (d[0][n] > 0.0) {
                agree += 1;
            }
        }
        assert!(agree >= 30, "agree {agree}");
    }

    #[test]
    fn chip_table_follows_the_standard() {
        // symbol 0 as IEEE 802.15.4 lists it, chip c0 first
        assert_eq!(CHIPS[0], 0b1101_1001_1100_0011_0101_0010_0010_1110);
        for s in 1..8 {
            assert_eq!(CHIPS[s], CHIPS[0].rotate_right(4 * s as u32), "symbol {s}");
        }
        // symbols 8 to 15: the odd-indexed chips of symbols 0 to 7 inverted
        for s in 8..16 {
            assert_eq!(CHIPS[s], CHIPS[s - 8] ^ 0x5555_5555, "symbol {s}");
        }
    }
}
