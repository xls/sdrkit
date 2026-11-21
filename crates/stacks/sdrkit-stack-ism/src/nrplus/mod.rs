//! DECT NR+ (`dect_nrplus`): DECT-2020 NR packets on a 1.728 MHz channel. Each packet is found by its STF; its first two OFDM
//! symbols give the physical layer control field (network id, transmitter and receiver ids, length, MCS, HARQ) after the turbo
//! code and the CRC. The data field is not decoded. Only µ = 1, β = 1 and one transmit stream are read.
//! Standard: ETSI TS 103 636-3 V1.5.1 and TS 103 636-4 V1.5.1.

pub mod gen;
pub mod phy;

use crate::family::REPORT_S;
use crate::typed::{
    NrPlusDeviceRow, NrPlusFields, NrPlusPacketEvent, NrPlusPacketRow, NRPLUS_DEVICES,
    NRPLUS_PACKET, NRPLUS_PACKETS,
};
use phy::{
    bin, carrier, drs_indices, pcc_cells, scrambling, stf_base, ControlField, COVER, CP,
    DRS, NFFT, OCCUPIED, PCC_BITS, PCC_CINIT, RATE_HZ, STF_LEN, SYMBOL,
};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::interface::{Fields, ReadFields, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, EventDef, GenConfig, Generator, LockProfile, LockState,
    Receiver, Record, Schema, StackConfig, StackDescriptor, StateBlob, StateError, Tuning, Values,
};
use sdrkit_graph::{
    no_values, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kept,
    Kind, Outbox, Port, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "dect_nrplus";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 1.728e6;
/// The default frequency of the test signal (channel 1657), Hz.
pub const TEST_DIAL_HZ: f64 = 1881.792e6;
/// The most packets kept.
pub const PACKET_CAP: usize = 300;
/// The most devices kept.
pub const DEVICE_CAP: usize = 200;
/// A packet within this many seconds of signal shows the lock.
pub const RECENT_S: f64 = 5.0;
/// The STF metric a candidate needs.
pub const MIN_METRIC: f32 = 0.7;
/// The share of the window power that the STF correlation needs.
pub const MIN_RHO: f32 = 0.45;
/// What no public document confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: the channel formula of TS 103 636-2, which was not read, and so the channel numbers in the data file. Only µ = 1, β = 1 and one stream are read.";

/// A packet read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NrPacket {
    /// Signal time of the STF, s.
    pub t: f64,
    /// Wall time, when live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall: Option<f64>,
    /// The id: "network 5A, RD 1234".
    pub id: String,
    /// "control field type 1" or 2.
    pub kind: String,
    /// The text of the field.
    pub text: String,
    /// The transmitter identity.
    pub transmitter: u16,
    /// The carrier offset, Hz.
    pub cfo_hz: f64,
    /// DRS power over noise, dB.
    pub snr_db: f32,
}

impl Shift for NrPacket {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// A device, keyed by its transmitter identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NrDevice {
    /// The id of its last packet.
    pub id: String,
    /// Packets heard.
    pub packets: u64,
    /// The text of its last field.
    pub last: String,
    /// The last SNR, dB.
    pub snr_db: f32,
}

impl Shift for NrDevice {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// What the stack keeps and saves.
#[derive(Serialize, Deserialize)]
pub struct NrHeld {
    /// The packets.
    pub packets: LogRing<NrPacket>,
    /// The devices.
    pub devices: ObjectTable<u16, NrDevice>,
    /// STF detections.
    pub stf: u64,
    /// Packets read.
    pub good: u64,
}

impl Default for NrHeld {
    fn default() -> Self {
        NrHeld {
            packets: LogRing::new(PACKET_CAP),
            devices: ObjectTable::new(DEVICE_CAP),
            stf: 0,
            good: 0,
        }
    }
}

impl Shift for NrHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.packets.shift_times(by_s);
        self.devices.shift_times(by_s);
    }
}

impl Tables for NrHeld {
    const FORMAT: StateFormat = StateFormat::new("ism.dect_nrplus", 1);
    fn clear(&mut self) {
        self.packets.clear();
        self.devices.clear();
        self.stf = 0;
        self.good = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.packets.adopt(saved.packets);
        self.devices.adopt(saved.devices);
        self.stf = saved.stf;
        self.good = saved.good;
    }
}

impl Shaped for NrPacket {
    type Row = NrPlusPacketRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&NrPlusPacketRow {
            utc: self.wall.map(Time),
            time_s: self.t,
            id: &self.id,
            r#type: &self.kind,
            text: &self.text,
            cfo_hz: self.cfo_hz,
            snr_db: f64::from(self.snr_db),
        });
    }
}

impl Shaped for NrDevice {
    type Row = NrPlusDeviceRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&NrPlusDeviceRow {
            transmitter: self.id.rsplit(' ').next().unwrap_or(""),
            id: &self.id,
            packets: self.packets as i64,
            text: &self.last,
            snr_db: f64::from(self.snr_db),
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

impl Item for NrPacket {
    const KIND: &'static str = "dect_nrplus.packet";
    fn event_def() -> Option<&'static EventDef> {
        Some(NRPLUS_PACKET.def())
    }
    fn write_fields(&self, f: &mut Record) {
        NrPlusPacketEvent {
            id: &self.id,
            r#type: &self.kind,
            text: &self.text,
            family: Family::Sensor,
        }
        .write(f);
    }
}

/// The STF detector state: the best candidate of the current run.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    w: i64,
    metric: f32,
    d: Complex32,
}

/// The `ism.dect_nrplus` decoder.
pub struct NrDecoder {
    rate: f64,
    /// Signal time of the first sample after the last (re)configure, s: the sample index counts from there.
    origin_s: f64,
    fresh: bool,
    resampler: Option<Resampler<Complex32>>,
    /// Moves the tuned channel to 0 Hz when the radio sits off it.
    mixer: Option<Nco>,
    mixed: Vec<Complex32>,
    work: Vec<Complex32>,
    /// Samples at 1.728 Msps; `x[0]` has stream index `start`.
    x: Vec<Complex32>,
    start: i64,
    /// The STF correlation y(n) for n from `start`.
    y: Vec<Complex32>,
    s: [Complex32; 16],
    s_energy: f32,
    /// The next window start to test.
    next: i64,
    hold_until: i64,
    best: Option<Candidate>,
    last_hit: i64,
    fft: Fft,
    scr: [u8; PCC_BITS],
    /// The turbo decoders and rate matchers of the PCC, made with the component.
    pcc: phy::PccDecoder,
    held: NrHeld,
    out: Vec<Timed<NrPacket>>,
    snr_db: Option<f64>,
    cfo_hz: Option<f64>,
    level_acc: f64,
    level_n: u64,
    level_dbfs: Option<f64>,
    now: f64,
    live: bool,
    clock: Option<sdrkit_core::Clock>,
    dirty: bool,
    cleared: bool,
    started: bool,
}

impl Default for NrDecoder {
    fn default() -> Self {
        let s = stf_base();
        NrDecoder {
            rate: 0.0,
            origin_s: 0.0,
            fresh: true,
            resampler: None,
            mixer: None,
            mixed: Vec::new(),
            work: Vec::new(),
            x: Vec::new(),
            start: 0,
            y: Vec::new(),
            s_energy: s.iter().map(|z| z.norm_sqr()).sum(),
            s,
            next: 0,
            hold_until: 0,
            best: None,
            last_hit: i64::MIN / 2,
            fft: Fft::forward(NFFT),
            scr: scrambling(PCC_CINIT),
            pcc: phy::PccDecoder::default(),
            held: NrHeld::default(),
            out: Vec::new(),
            snr_db: None,
            cfo_hz: None,
            level_acc: 0.0,
            level_n: 0,
            level_dbfs: None,
            now: 0.0,
            live: false,
            clock: None,
            dirty: true,
            cleared: false,
            started: false,
        }
    }
}

/// Samples a packet needs from the STF start: the STF and two OFDM symbols, and the prefixes of symbols 3 and 4.
const NEED: i64 = (STF_LEN + 4 * SYMBOL) as i64;

impl NrDecoder {
    /// The devices, the packets and the counters.
    pub fn held(&self) -> &NrHeld {
        &self.held
    }

    fn at(&self, i: i64) -> Complex32 {
        self.x
            .get((i - self.start) as usize)
            .copied()
            .unwrap_or_default()
    }

    fn end(&self) -> i64 {
        self.start + self.x.len() as i64
    }

    /// The STF metric of a window starting at `w`: (M, ρ, D).
    fn metric(&self, w: i64) -> (f32, f32, Complex32) {
        let y = |n: i64| self.y[(n - self.start) as usize];
        let r: [Complex32; 7] = std::array::from_fn(|k| y(w + 16 * k as i64) * COVER[k]);
        let d: Complex32 = (1..7).map(|k| r[k] * r[k - 1].conj()).sum();
        let e: f32 = r.iter().map(|z| z.norm_sqr()).sum();
        let m = d.norm() / (e * 6.0 / 7.0).max(1e-20);
        if m <= MIN_METRIC {
            // Not a candidate: the window power is not needed.
            return (m, 0.0, d);
        }
        let p: f32 = (0..STF_LEN as i64).map(|i| self.at(w + i).norm_sqr()).sum();
        let rho = e / (p * self.s_energy).max(1e-20);
        (m, rho, d)
    }

    /// The PCC soft values (positive means 0, before descrambling), the carrier offset (Hz) and the DRS SNR (dB) of the
    /// packet whose STF starts at `c.w`.
    fn pcc_soft(&mut self, c: &Candidate) -> ([f32; PCC_BITS], f64, f32) {
        let w = c.w;
        let coarse = f64::from(c.d.arg()) / 16.0;
        // Fine: the cyclic prefixes of symbols 1 to 4 after the coarse correction.
        let rot = |i: i64| Complex32::from_polar(1.0, (-coarse * (i - w) as f64) as f32);
        let mut acc = Complex32::default();
        for l in 0..4 {
            let s0 = w + STF_LEN as i64 + (l * SYMBOL) as i64;
            for i in 0..CP as i64 {
                let a = self.at(s0 + i) * rot(s0 + i);
                let b = self.at(s0 + i + NFFT as i64) * rot(s0 + i + NFFT as i64);
                acc += a * b.conj();
            }
        }
        let w_per = coarse - f64::from(acc.arg()) / NFFT as f64;
        let cfo = w_per * RATE_HZ / std::f64::consts::TAU;
        // The two symbols, from 2 samples before the end of each prefix.
        let mut sym = [[Complex32::default(); NFFT]; 2];
        for (l, out) in sym.iter_mut().enumerate() {
            let s0 = w + STF_LEN as i64 + (l * SYMBOL + CP - 2) as i64;
            for (n, o) in out.iter_mut().enumerate() {
                let i = s0 + n as i64;
                *o = self.at(i) * Complex32::from_polar(1.0, (-w_per * (i - w) as f64) as f32);
            }
            self.fft.process(out);
        }
        // The window starts 2 samples early: each carrier turns by 2π c 2 / 64, which is taken out so the channel is smooth.
        let cell = |l: usize, k: usize| {
            let c = carrier(k);
            let turn = std::f32::consts::TAU * (2 * c) as f32 / NFFT as f32;
            sym[l - 1][bin(c)] * Complex32::from_polar(1.0, turn)
        };
        // The channel on the DRS of symbol 1, linear between them, flat beyond the last.
        let mut h = [Complex32::default(); OCCUPIED];
        let raw: Vec<(usize, Complex32)> = drs_indices()
            .map(|k| (k, cell(1, k) * f32::from(DRS[k])))
            .collect();
        // Each DRS estimate is smoothed with its neighbours (1, 2, 3, 2, 1): the channel changes little over 8 carriers.
        let drs: Vec<(usize, Complex32)> = (0..raw.len())
            .map(|i| {
                let mut acc = Complex32::default();
                let mut wsum = 0.0;
                for (d, w) in [(-2isize, 1.0f32), (-1, 2.0), (0, 3.0), (1, 2.0), (2, 1.0)] {
                    let j = i as isize + d;
                    if (0..raw.len() as isize).contains(&j) {
                        acc += raw[j as usize].1 * w;
                        wsum += w;
                    }
                }
                (raw[i].0, acc / wsum)
            })
            .collect();
        for k in 0..OCCUPIED {
            let i = (k / 4).min(drs.len() - 1);
            h[k] = if i + 1 < drs.len() {
                let (k0, h0) = drs[i];
                let (k1, h1) = drs[i + 1];
                let f = (k - k0) as f32 / (k1 - k0) as f32;
                h0 * (1.0 - f) + h1 * f
            } else {
                drs[i].1
            };
        }
        let null = [-32, -31, -30, -29, 0, 29, 30, 31];
        let noise = null
            .iter()
            .flat_map(|&c| sym.iter().map(move |s| s[bin(c)].norm_sqr()))
            .sum::<f32>()
            / 16.0;
        let noise = noise.max(1e-12);
        let signal = raw.iter().map(|d| d.1.norm_sqr()).sum::<f32>() / raw.len() as f32;
        let gain = 2.0 * std::f32::consts::SQRT_2 / noise;
        let mut soft = [0f32; PCC_BITS];
        for (q, &(l, k)) in pcc_cells().iter().enumerate() {
            let z = cell(l, k) * h[k].conj();
            soft[2 * q] = z.re * gain;
            soft[2 * q + 1] = z.im * gain;
        }
        (soft, cfo, 10.0 * (signal / noise).max(1e-6).log10())
    }

    /// Reads the packet whose STF starts at `w`.
    fn packet(&mut self, c: Candidate) {
        self.held.stf += 1;
        self.dirty = true;
        let w = c.w;
        let (soft, cfo, snr) = self.pcc_soft(&c);
        // The LLRs as i8: their mean magnitude at 32.
        let mean = soft.iter().map(|v| v.abs()).sum::<f32>() / PCC_BITS as f32;
        let scale = 32.0 / mean.max(1e-12);
        let scr = &self.scr;
        let llr: [i8; PCC_BITS] = std::array::from_fn(|i| {
            let v = soft[i] * scale * if scr[i] == 1 { -1.0 } else { 1.0 };
            v.round().clamp(-127.0, 127.0) as i8
        });
        let Some((field, mask)) = self.pcc.read(&llr) else {
            return;
        };
        let Some(f) = ControlField::read(field) else {
            return;
        };
        let t = self.origin_s + w as f64 / RATE_HZ;
        let stamp = match self.clock.as_ref().and_then(|c| self.live.then(|| c.now())) {
            Some(wl) => Stamp::at(t).with_wall(wl),
            None => Stamp::at(t),
        };
        let p = NrPacket {
            t,
            wall: stamp.wall,
            id: f.id(),
            kind: format!("control field type {}", f.kind),
            text: f.text(mask),
            transmitter: f.transmitter,
            cfo_hz: cfo,
            snr_db: snr,
        };
        self.held.good += 1;
        self.snr_db = Some(f64::from(snr));
        self.cfo_hz = Some(cfo);
        if let Ok(up) = self
            .held
            .devices
            .upsert(&f.transmitter, stamp, || NrDevice {
                id: p.id.clone(),
                packets: 0,
                last: String::new(),
                snr_db: snr,
            })
        {
            let dv = up.row;
            dv.id.clone_from(&p.id);
            dv.packets += 1;
            dv.last.clone_from(&p.text);
            dv.snr_db = snr;
        }
        self.held.packets.push(p.clone());
        self.out.push(Timed { time: t, value: p });
        self.hold_until = w + (STF_LEN + 2 * SYMBOL) as i64;
    }

    /// Runs the STF search over the new samples.
    fn search(&mut self) {
        // y(n) needs x[n .. n + 16].
        let have = self.start + self.y.len() as i64;
        for n in have..self.end() - 15 {
            let base = (n - self.start) as usize;
            let v: Complex32 = (0..16).map(|i| self.x[base + i] * self.s[i].conj()).sum();
            self.y.push(v);
        }
        let y_end = self.start + self.y.len() as i64;
        while self.next + 96 < y_end && self.next + STF_LEN as i64 <= self.end() {
            let w = self.next;
            self.next += 1;
            if w < self.hold_until {
                continue;
            }
            if let Some(b) = self.best {
                if w - self.last_hit > 12 {
                    if self.end() < b.w + NEED {
                        // Wait for the rest of the packet.
                        self.next = w;
                        return;
                    }
                    self.best = None;
                    self.packet(b);
                    if w < self.hold_until {
                        continue;
                    }
                }
            }
            let (m, rho, d) = self.metric(w);
            if m > MIN_METRIC && rho > MIN_RHO {
                self.last_hit = w;
                if self.best.is_none_or(|b| m > b.metric) {
                    self.best = Some(Candidate { w, metric: m, d });
                }
            }
        }
    }

    fn trim(&mut self) {
        let keep_from = self.next - 4 * NEED;
        if keep_from > self.start + (1 << 16) {
            let cut = (keep_from - self.start) as usize;
            self.x.drain(..cut);
            self.y.drain(..cut.min(self.y.len()));
            self.start += cut as i64;
        }
    }
}

impl Decoder for NrDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "ism.dect_nrplus"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<NrPacket>("packets")]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Published {
        crate::typed::results(ID)
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE_HZ - 1.0 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.fresh = true;
        let off = input.signal_offset_hz();
        self.mixer = (off.abs() > 0.5).then(|| Nco::new(input.rate, -off));
        self.resampler = if (input.rate - RATE_HZ).abs() < 1.0 {
            None
        } else {
            Some(Resampler::new(input.rate, RATE_HZ).map_err(ConfigError::from)?)
        };
        self.x.clear();
        self.y.clear();
        self.start = 0;
        self.next = 0;
        self.best = None;
        self.hold_until = 0;
        Ok(vec![Format {
            rate: 0.0,
            frame_len: 0,
            media: None,
            ..input.with_kind(Kind::typed::<NrPacket>())
        }])
    }
    fn retune(&mut self, input: &Format) {
        let _ = self.configure(input);
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.clock = Some(ctx.clock.clone());
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.now = (first + x.len() as u64) as f64 / self.rate.max(1.0);
        if std::mem::take(&mut self.fresh) {
            self.origin_s = first as f64 / self.rate.max(1.0);
        }
        for z in x {
            self.level_acc += f64::from(z.norm_sqr());
        }
        self.level_n += x.len() as u64;
        let input: &[Complex32] = match self.mixer.as_mut() {
            Some(m) => {
                self.mixed.clear();
                self.mixed.resize(x.len(), Complex32::default());
                m.mix(x, &mut self.mixed);
                &self.mixed
            }
            None => x,
        };
        match self.resampler.as_mut() {
            Some(r) => {
                self.work.clear();
                r.process(input, &mut self.work);
                self.x.extend_from_slice(&self.work);
            }
            None => self.x.extend_from_slice(input),
        }
        self.search();
        self.trim();
        out.items::<NrPacket>(0).append(&mut self.out);
    }
    fn report(&mut self, p: &mut Published) {
        if self.level_n > 0 {
            self.level_dbfs =
                Some(10.0 * (self.level_acc / self.level_n as f64).max(1e-20).log10());
            self.level_acc = 0.0;
            self.level_n = 0;
        }
        p.put(&NrPlusFields {
            frames_ok: self.held.good as i64,
            stf_detections: self.held.stf as i64,
            devices: self.held.devices.len() as i64,
            snr_db: self.snr_db,
            cfo_hz: self.cfo_hz,
            level_dbfs: self.level_dbfs,
            note: NOT_CONFIRMED,
        });
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&NRPLUS_PACKETS);
            p.clear_rows(&NRPLUS_DEVICES);
        }
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.held.packets, &NRPLUS_PACKETS, self.now, p);
            shaped::rows(&self.held.devices, &NRPLUS_DEVICES, self.now, p);
        }
        let newest = self.held.packets.newest_first().next();
        let recent = newest.is_some_and(|f| self.now - f.t <= RECENT_S);
        let lock = if !self.started {
            LockState::Off
        } else if recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let summary = match newest {
            Some(f) if recent => format!(
                "DECT NR+: {} packets, {} devices; last {}: {}",
                self.held.good,
                self.held.devices.len(),
                f.id,
                f.text
            ),
            _ if self.started => format!("DECT NR+: {} STF detections, listening", self.held.stf),
            _ => "DECT NR+: waiting for samples".to_string(),
        };
        crate::family::set_status(p, lock, &summary);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.held.clear();
        self.cleared = true;
        self.dirty = true;
    }
    fn forget(&mut self, kept: &Kept) {
        self.level_dbfs = None;
        self.level_acc = 0.0;
        self.level_n = 0;
        self.snr_db = None;
        self.cfo_hz = None;
        if !kept.table(NRPLUS_PACKETS.key) {
            self.held.packets.clear();
            self.cleared = true;
        }
        if !kept.table(NRPLUS_DEVICES.key) {
            self.held.devices.clear();
            self.cleared = true;
        }
        if !kept.field("frames_ok") {
            self.held.good = 0;
        }
        if !kept.field("stf_detections") {
            self.held.stf = 0;
        }
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.now)?;
        self.cleared = true;
        self.dirty = true;
        Ok(())
    }
}

/// The settings of the stack: none.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA.get_or_init(|| Arc::new(Schema::of(&[]))).clone()
}

/// The `dect_nrplus` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("rx", DecoderComponent::new(NrDecoder::default()))
        .input("rx")
        .output("rx")
        .clock(REPORT_S)
        .status_from("rx")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE_HZ,
            &crate::family::min_rate_message("DECT NR+", MIN_RATE_HZ),
        )
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::NrPlusGenerator::from_config(cfg)?))
}

/// DECT NR+. The default frequency is that of the test signal (channel 1657); the session gives the real one.
pub static DECT_NRPLUS: StackDescriptor = StackDescriptor {
    id: ID,
    name: "DECT NR+",
    group: "IoT, 1.9 GHz",
    blurb: "DECT-2020 NR packets on a 1.728 MHz channel: the physical layer control field (network id, transmitter and receiver ids, length, MCS, HARQ); the data field is not decoded.",
    tuning: Tuning::anywhere(TEST_DIAL_HZ, 4e6, MIN_RATE_HZ, 1.728e6),
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

fn verify_cases(o: &sdrkit_core::verify::VerifyOptions) -> Vec<sdrkit_core::verify::VerifyCase> {
    let mut c = sdrkit_core::verify::VerifyCase::new("test signal, three devices", o, 0.0);
    c.seconds = o.seconds.max(1.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = NrPlusFields::read(r.fields());
        vec![
            crate::family::at_least_value("frames_ok", f.as_ref().map_or(0, |f| f.frames_ok), 30),
            crate::family::at_least_value("devices", f.as_ref().map_or(0, |f| f.devices), 3),
        ]
    });
    vec![c]
}

/// The verify case of `dect_nrplus`.
pub static VERIFY: sdrkit_core::verify::VerifyHooks = sdrkit_core::verify::VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The benchmark of the receiver: 0.5 s of the test signal at 4 Msps, fed in one piece.
pub fn benchmarks() -> Vec<sdrkit_core::bench::Benchmark> {
    vec![sdrkit_core::bench::Benchmark {
        name: "dect_nrplus.receiver_4m",
        run: bench_receiver,
    }]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::bench::BenchResult {
    let rate = 4e6;
    let mut x = vec![Complex32::default(); (rate * 0.5) as usize];
    if let Ok(mut s) = gen::scene(rate, 1) {
        s.generate(&mut x);
    }
    let Ok(mut rx) = receiver(&StackConfig::new(Values::defaults(&schema()))) else {
        return sdrkit_core::bench::BenchResult::default();
    };
    let _ =
        rx.configure(&sdrkit_core::InputInfo::new(rate, TEST_DIAL_HZ, TEST_DIAL_HZ).recording());
    let mut out = sdrkit_core::Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), 0.5, || {
        out.clear();
        rx.feed(&x, &mut out);
    })
}
