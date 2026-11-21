//! mioty (`mioty`): TS-UNB uplinks. The 24 radio bursts of a frame are found by their pilot anywhere in the capture, the frame
//! by the TSMA pattern that explains them; each burst is demodulated coherently from its pilot, the code is decoded, both CRCs
//! are checked and extension bursts are read. The MAC fields of the fixed MAC mode are shown; the payload stays encrypted.
//! Standard: ETSI TS 103 357 V1.1.1 clause 6 (uplink, standard TSMA mode). The downlink, the sync burst, the UL-ER mode and the
//! narrow and wide TSMA modes are not read.

pub mod code;
pub mod gen;

use crate::family::{hex, REPORT_S};
use crate::typed::{
    MiotyDeviceRow, MiotyFields, MiotyFrameEvent, MiotyFrameRow, MIOTY_DEVICES, MIOTY_FRAME,
    MIOTY_FRAMES,
};
use code::{
    extension_bursts, extension_count, placement, read_frame, read_header, Mac, BURST_SYMBOLS,
    CORE_PILOT, EXTENSION_PILOT, PATTERNS, PILOT_START, SYMBOL_RATE,
};
use gen::rotation;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_fec::conv::codes::MIOTY_K7_R3;
use sdrkit_algo_fec::conv::Viterbi;
use sdrkit_algo_fec::llr::Llr8;
use sdrkit_algo_modem::tsma::{fits, Detection, Fit, FitConfig, Pattern, PilotConfig, PilotSearch};
use sdrkit_core::interface::{Fields, Hertz, ReadFields, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, EventDef, GenConfig, Generator, LockProfile, LockState,
    Receiver, Record, Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor,
    StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kept,
    Kind, Outbox, Port, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "mioty";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 250e3;
/// Samples per symbol at the working rate.
pub const SPS: usize = 48;
/// The working rate, Hz.
pub const WORK_RATE_HZ: f64 = SPS as f64 * SYMBOL_RATE;
/// Seconds of samples kept.
pub const HISTORY_S: f64 = 20.0;
/// Seconds of signal between two pattern searches.
pub const FIT_EVERY_S: f64 = 0.5;
/// The most frames kept.
pub const FRAME_CAP: usize = 200;
/// The most hypotheses decoded in one pattern search.
pub const MAX_TRIES: usize = 40;
/// Detections this recent anchor a pattern search, s.
pub const ANCHOR_S: f64 = 4.5;
/// The most devices kept.
pub const DEVICE_CAP: usize = 200;
/// A frame within this many seconds of signal shows the lock.
pub const RECENT_S: f64 = 30.0;
/// The default frequency of the test signal (EU0 / EU1 channel A), Hz.
pub const TEST_DIAL_HZ: f64 = 868.18e6;
/// What no public document confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: the bit numbering of the MAC header (bit 0 taken as the least significant), the shift direction of the extension LFSR (right shift), the tap order of the convolutional code (as the 802.11 code), and the extension carrier taken modulo 25 against the 24 core carriers.";

/// Detected bursts needed to try a pattern.
pub const MIN_BURSTS: Setting<field::I64> = Setting::new(
    "min_bursts",
    "Bursts needed to try a pattern",
    "count",
    5,
)
.range(3.0, 24.0, 1.0)
.help("A TSMA pattern is tried when this many of its 24 bursts were detected where it puts them.");

sdrkit_core::settings! {
    /// The settings of `ism.mioty` and of the `mioty` stack.
    pub enum MiotySetting {
        /// The bursts needed.
        MinBursts = MIN_BURSTS,
    }
}

/// One half-sine pulse sample: cos(π u / 2T) for |u| < T, `u` in samples at the working rate.
fn pulse(u: f64) -> f32 {
    if u.abs() < SPS as f64 {
        (std::f64::consts::PI * u / (2.0 * SPS as f64)).cos() as f32
    } else {
        0.0
    }
}

/// The centre of symbol `n` from the pilot centre, samples at the working rate.
fn symbol_at(n: usize) -> isize {
    (SPS * n) as isize - (SPS * 35 / 2) as isize
}

/// Half the length of the pilot window (6.5 symbols), samples.
const PILOT_HALF: isize = 312;
/// Half the part of the pilot window that holds pilot symbols only (5.5 symbols), samples.
const PURE_HALF: isize = 264;

/// The pilot waveform from −6.5 to +6.5 symbols around the pilot centre at the working rate.
pub fn pilot_template(pilot: &[u8; 12]) -> Vec<Complex32> {
    (-PILOT_HALF..PILOT_HALF)
        .map(|u| {
            let mut z = Complex32::default();
            for (i, &p) in pilot.iter().enumerate() {
                let n = PILOT_START + i;
                let g = pulse((u - symbol_at(n)) as f64);
                if g != 0.0 {
                    z += rotation(n) * ((1.0 - 2.0 * f32::from(p)) * g);
                }
            }
            z
        })
        .collect()
}

/// A frame read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MiotyFrame {
    /// Signal time of the pilot centre of burst 0, s.
    pub t: f64,
    /// Wall time, when live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall: Option<f64>,
    /// The device id ("short 1234").
    pub id: String,
    /// The pattern ("UPG1/3").
    pub pattern: String,
    /// MPDU bytes.
    pub psi: u8,
    /// The MAC mode bits.
    pub mmode: u8,
    /// The MAC fields as text.
    pub mac: String,
    /// The MPDU.
    pub mpdu: Vec<u8>,
    /// The middle of the 24 carriers, Hz (absolute when the radio centre is known).
    pub freq_hz: f64,
    /// Pilot power over residual, dB.
    pub snr_db: f32,
    /// Core bursts that the detector found.
    pub matched: u8,
}

impl Shift for MiotyFrame {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// A device, keyed by its id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MiotyDevice {
    /// The id.
    pub id: String,
    /// Frames heard.
    pub frames: u64,
    /// The last packet counter.
    pub counter: u32,
    /// The last SNR, dB.
    pub snr_db: f32,
    /// The last frequency, Hz.
    pub freq_hz: f64,
}

impl Shift for MiotyDevice {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// The counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Counters {
    /// Bursts detected.
    pub bursts: u64,
    /// Frames read.
    pub frames: u64,
    /// Patterns that did not decode.
    pub failed: u64,
}

/// What the stack keeps and saves.
#[derive(Serialize, Deserialize)]
pub struct MiotyHeld {
    /// The frames.
    pub frames: LogRing<MiotyFrame>,
    /// The devices.
    pub devices: ObjectTable<String, MiotyDevice>,
    /// The counters.
    pub counters: Counters,
}

impl Default for MiotyHeld {
    fn default() -> Self {
        MiotyHeld {
            frames: LogRing::new(FRAME_CAP),
            devices: ObjectTable::new(DEVICE_CAP),
            counters: Counters::default(),
        }
    }
}

impl Shift for MiotyHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.frames.shift_times(by_s);
        self.devices.shift_times(by_s);
    }
}

impl Tables for MiotyHeld {
    const FORMAT: StateFormat = StateFormat::new("ism.mioty", 1);
    fn clear(&mut self) {
        self.frames.clear();
        self.devices.clear();
        self.counters = Counters::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.frames.adopt(saved.frames);
        self.devices.adopt(saved.devices);
        self.counters = saved.counters;
    }
}

impl Shaped for MiotyFrame {
    type Row = MiotyFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&MiotyFrameRow {
            utc: self.wall.map(Time),
            time_s: self.t,
            id: &self.id,
            pattern: &self.pattern,
            psi: i64::from(self.psi),
            mmode: i64::from(self.mmode),
            mac: &self.mac,
            mpdu: &hex(&self.mpdu[..self.mpdu.len().min(32)]),
            freq_hz: Hertz(self.freq_hz),
            snr_db: f64::from(self.snr_db),
            bursts_matched: i64::from(self.matched),
        });
    }
}

impl Shaped for MiotyDevice {
    type Row = MiotyDeviceRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&MiotyDeviceRow {
            id: &self.id,
            frames: self.frames as i64,
            counter: i64::from(self.counter),
            snr_db: f64::from(self.snr_db),
            freq_hz: Hertz(self.freq_hz),
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

impl Item for MiotyFrame {
    const KIND: &'static str = "mioty.frame";
    fn event_def() -> Option<&'static EventDef> {
        Some(MIOTY_FRAME.def())
    }
    fn write_fields(&self, f: &mut Record) {
        MiotyFrameEvent {
            id: &self.id,
            pattern: &self.pattern,
            psi: i64::from(self.psi),
            mac: &self.mac,
            family: Family::Sensor,
        }
        .write(f);
    }
}

/// A burst after refinement.
#[derive(Clone, Copy, Debug)]
struct Refined {
    /// Its pilot centre, an index of the history.
    at: i64,
    hz: f64,
    h: Complex32,
    noise: f32,
    signal: f32,
}

/// The working-rate history: samples with the stream index of the first.
struct History {
    x: Vec<Complex32>,
    start: i64,
}

impl History {
    fn end(&self) -> i64 {
        self.start + self.x.len() as i64
    }
    fn get(&self, i: i64) -> Complex32 {
        let k = i - self.start;
        if k < 0 || k >= self.x.len() as i64 {
            Complex32::default()
        } else {
            self.x[k as usize]
        }
    }
}

/// The `ism.mioty` decoder.
pub struct MiotyDecoder {
    min_bursts: usize,
    rate: f64,
    centre_hz: f64,
    tuned_hz: f64,
    /// Signal time of the first sample after the last (re)configure, s: the history counts from there.
    origin_s: f64,
    fresh: bool,
    resampler: Option<Resampler<Complex32>>,
    /// Moves the tuned channel to 0 Hz when the radio sits off it.
    mixer: Option<Nco>,
    mixed: Vec<Complex32>,
    work: Vec<Complex32>,
    hist: History,
    search: Option<PilotSearch>,
    core: Vec<Complex32>,
    ext: Vec<Complex32>,
    patterns: Vec<Pattern>,
    dets: Vec<Detection>,
    found: Vec<Detection>,
    core_buf: Vec<Refined>,
    soft_buf: Vec<[f32; BURST_SYMBOLS]>,
    core_place: Vec<(u16, u8)>,
    llr_buf: Vec<Llr8>,
    used: Vec<bool>,
    failed: HashSet<(usize, i64, i64)>,
    next_fit: i64,
    viterbi: Viterbi,
    held: MiotyHeld,
    out: Vec<Timed<MiotyFrame>>,
    snr_db: Option<f64>,
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

impl Default for MiotyDecoder {
    fn default() -> Self {
        MiotyDecoder {
            min_bursts: 5,
            rate: 0.0,
            centre_hz: 0.0,
            tuned_hz: 0.0,
            origin_s: 0.0,
            fresh: true,
            resampler: None,
            mixer: None,
            mixed: Vec::new(),
            work: Vec::new(),
            hist: History {
                x: Vec::new(),
                start: 0,
            },
            search: None,
            core: pilot_template(&CORE_PILOT),
            ext: pilot_template(&EXTENSION_PILOT),
            patterns: PATTERNS
                .iter()
                .map(|p| Pattern {
                    carriers: p.carriers.iter().map(|&c| u16::from(c)).collect(),
                    times: p
                        .times()
                        .iter()
                        .map(|&t| f64::from(t) * SPS as f64)
                        .collect(),
                })
                .collect(),
            dets: Vec::new(),
            found: Vec::new(),
            core_buf: Vec::with_capacity(24),
            soft_buf: Vec::with_capacity(24),
            core_place: placement(code::CORE_CODED),
            llr_buf: Vec::with_capacity(code::coded_len(code::MAX_PSI)),
            used: Vec::new(),
            failed: HashSet::new(),
            next_fit: 0,
            // A frame of 245 bytes has 1990 steps.
            viterbi: Viterbi::new(MIOTY_K7_R3, 2048).expect("the code of the standard"),
            held: MiotyHeld::default(),
            out: Vec::new(),
            snr_db: None,
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

/// The phasor e^(−j 2π f u / rate) for `u` from `from`, and the step between two samples.
fn rotator(hz: f64, from: isize) -> (Complex32, Complex32) {
    let w = -std::f64::consts::TAU * hz / WORK_RATE_HZ;
    let p = w * from as f64;
    (
        Complex32::new(p.cos() as f32, p.sin() as f32),
        Complex32::new(w.cos() as f32, w.sin() as f32),
    )
}

impl MiotyDecoder {
    /// The devices, the frames and the counters.
    pub fn held(&self) -> &MiotyHeld {
        &self.held
    }

    /// The correlation of the history at pilot centre `at` and frequency `hz` with `template`.
    fn correlate(&self, template: &[Complex32], at: i64, hz: f64) -> Complex32 {
        let (mut r, step) = rotator(hz, -PILOT_HALF);
        let mut acc = Complex32::default();
        for (i, p) in template.iter().enumerate() {
            let y = self.hist.get(at - PILOT_HALF as i64 + i as i64);
            acc += y * r * p.conj();
            r *= step;
        }
        acc
    }

    /// Searches the pilot of a burst near `at` and `hz`: on a grid of 6 samples and 30 Hz, then to 1 sample and to 5 Hz.
    fn refine(&self, template: &[Complex32], at: f64, hz: f64) -> Refined {
        let mut best = (at.round() as i64, hz, 0.0f32);
        let try_at = |s: &Self, t: i64, f: f64, best: &mut (i64, f64, f32)| {
            let c = s.correlate(template, t, f).norm_sqr();
            if c > best.2 {
                *best = (t, f, c);
            }
        };
        // A grid of 6 samples and 30 Hz, then 1 sample at that frequency, then 5 Hz at that time.
        let (t0, f0) = (best.0, best.1);
        for dt in (-24..=24).step_by(6) {
            for k in -5..=5 {
                try_at(self, t0 + dt, f0 + f64::from(k) * 30.0, &mut best);
            }
        }
        let (t1, f1) = (best.0, best.1);
        for dt in -5..=5 {
            try_at(self, t1 + dt, f1, &mut best);
        }
        let (t2, f2) = (best.0, best.1);
        for k in -5..=5 {
            try_at(self, t2, f2 + f64::from(k) * 5.0, &mut best);
        }
        let (at, hz, _) = best;
        let energy: f32 = template.iter().map(|p| p.norm_sqr()).sum();
        let h = self.correlate(template, at, hz) / energy;
        let (mut r, step) = rotator(hz, -PURE_HALF);
        let (mut noise, mut signal) = (0f32, 0f32);
        for u in -PURE_HALF..PURE_HALF {
            let p = template[(u + PILOT_HALF) as usize];
            let y = self.hist.get(at + u as i64) * r;
            noise += (y - h * p).norm_sqr();
            signal += (h * p).norm_sqr();
            r *= step;
        }
        let n = (2 * PURE_HALF) as f32;
        Refined {
            at,
            hz,
            h,
            noise: (noise / n).max(1e-12),
            signal,
        }
    }

    /// The LLRs of the data symbols of a refined burst, by symbol (positive means 0).
    fn soft(&self, b: &Refined) -> [f32; BURST_SYMBOLS] {
        let mut v = [0f32; BURST_SYMBOLS];
        for (n, o) in v.iter_mut().enumerate() {
            if (PILOT_START..PILOT_START + 12).contains(&n) {
                continue;
            }
            let c = symbol_at(n);
            let (mut r, step) = rotator(b.hz, c - SPS as isize + 1);
            let mut y = Complex32::default();
            for u in c - SPS as isize + 1..c + SPS as isize {
                y += self.hist.get(b.at + u as i64) * r * pulse((u - c) as f64);
                r *= step;
            }
            let re = (y * b.h.conj() * rotation(n).conj()).re;
            *o = 4.0 * re / b.noise;
        }
        v
    }

    /// Tries to read the frame of `fit`. `None` when it fails, `Some(None)` when its end has not arrived.
    fn decode(&mut self, fit: &Fit) -> Option<Option<(MiotyFrame, Vec<(f64, f64)>)>> {
        let p = &PATTERNS[fit.pattern];
        let times = p.times();
        let end = self.hist.end();
        let last = fit.t0 + f64::from(times[23]) * SPS as f64 + 20.0 * SPS as f64;
        if last > end as f64 {
            return Some(None);
        }
        let mut carry = (0.0f64, 0.0f64);
        let spacing = SYMBOL_RATE;
        // The core bursts and their header go through buffers that the decoder keeps: most hypotheses end at the header.
        self.core_buf.clear();
        self.soft_buf.clear();
        for k in 0..24 {
            let at = fit.t0 + f64::from(times[k]) * SPS as f64 + carry.0;
            let hz = fit.f0 + f64::from(p.carriers[k]) * spacing + carry.1;
            let r = self.refine(&self.core, at, hz);
            if r.signal > 4.0 * r.noise * (2 * PURE_HALF) as f32 {
                carry.0 += 0.5 * (r.at as f64 - at);
                carry.1 += 0.5 * (r.hz - hz);
            }
            let sv = self.soft(&r);
            self.core_buf.push(r);
            self.soft_buf.push(sv);
            // A hypothesis whose first bursts hold almost no pilot is dropped before the rest are searched.
            if k == 7 {
                // A pilot at the weakest usable level stands well above the noise of one sample after the fit.
                let strong = self
                    .core_buf
                    .iter()
                    .filter(|b| b.signal > 16.0 * b.noise)
                    .count();
                if strong < 2 {
                    return None;
                }
            }
        }
        let llr = |soft: &[[f32; BURST_SYMBOLS]], place: &[(u16, u8)], out: &mut Vec<Llr8>| {
            out.clear();
            out.extend(place.iter().map(|&(b, m)| {
                let v = soft.get(usize::from(b)).map_or(0.0, |s| s[usize::from(m)]);
                (2.0 * v).round().clamp(-127.0, 127.0) as i8
            }));
        };
        llr(&self.soft_buf, &self.core_place, &mut self.llr_buf);
        let head = read_header(&mut self.viterbi, &self.llr_buf)?;
        let se = extension_count(head.psi);
        let mut bursts = self.core_buf.clone();
        let mut soft = self.soft_buf.clone();
        if se > 0 {
            let ext = extension_bursts(head.hcrc, head.pcrc, se, p.spacing);
            let mut t = fit.t0 + f64::from(times[23]) * SPS as f64;
            let total: u32 = ext.iter().map(|e| e.0).sum();
            if t + f64::from(total + 20) * SPS as f64 + carry.0 > end as f64 {
                return Some(None);
            }
            for (dt, c) in ext {
                t += f64::from(dt) * SPS as f64;
                let r = self.refine(
                    &self.ext,
                    t + carry.0,
                    fit.f0 + f64::from(c) * spacing + carry.1,
                );
                soft.push(self.soft(&r));
                bursts.push(r);
            }
        }
        llr(
            &soft,
            &placement(code::coded_len(head.psi)),
            &mut self.llr_buf,
        );
        let (mpdu, mmode) = read_frame(&mut self.viterbi, &self.llr_buf, head.psi)?;
        let (s, n): (f32, f32) = bursts.iter().fold((0.0, 0.0), |a, b| {
            (a.0 + b.signal, a.1 + b.noise * (2 * PURE_HALF) as f32)
        });
        let mac = code::read_mac(&mpdu);
        let t = self.origin_s + bursts[0].at as f64 / WORK_RATE_HZ;
        let f0 = bursts[..24]
            .iter()
            .zip(p.carriers)
            .map(|(b, c)| b.hz - f64::from(c) * spacing)
            .sum::<f64>()
            / 24.0;
        // The samples were moved so the tuned frequency is at 0 Hz.
        let mid = f0
            + 12.0 * spacing
            + if self.centre_hz > 0.0 {
                self.tuned_hz
            } else {
                0.0
            };
        let places = bursts.iter().map(|b| (b.at as f64, b.hz)).collect();
        Some(Some((
            MiotyFrame {
                t,
                wall: None,
                id: mac
                    .as_ref()
                    .map_or_else(|| "unknown".into(), |m| m.id.clone()),
                pattern: p.name.to_string(),
                psi: head.psi as u8,
                mmode,
                mac: mac.as_ref().map_or_else(
                    || format!("{} bytes, MAC header not read", mpdu.len()),
                    |m: &Mac| m.text.clone(),
                ),
                mpdu,
                freq_hz: mid,
                snr_db: 10.0 * (s / n.max(1e-12)).max(1e-6).log10(),
                matched: fit.score as u8,
            },
            places,
        )))
    }

    /// Searches the patterns among the detections and reads every frame that they give.
    fn fit(&mut self) {
        let cfg = FitConfig {
            spacing_hz: SYMBOL_RATE,
            time_tol: 60.0,
            hz_tol: 200.0,
            min_score: self.min_bursts,
        };
        // Bursts of a frame that can now be read are recent: older detections are matched, not tried as anchors again.
        let recent = self.hist.end() as f64 - ANCHOR_S * WORK_RATE_HZ;
        let from = self.dets.partition_point(|d| d.t < recent);
        let all = fits(&self.dets, &self.used, &self.patterns, &cfg, from);
        let mut tries = 0;
        for fit in all {
            let k = key(&fit);
            if tries >= MAX_TRIES
                || self.failed.contains(&k)
                || fit.matched.iter().flatten().any(|&i| self.used[i])
            {
                continue;
            }
            tries += 1;
            match self.decode(&fit) {
                Some(Some((mut frame, bursts))) => {
                    for &i in fit.matched.iter().flatten() {
                        self.used[i] = true;
                    }
                    // Other detections on a burst of the frame are its spread, not bursts of another frame.
                    for (d, u) in self.dets.iter().zip(self.used.iter_mut()) {
                        if bursts.iter().any(|&(t, f)| {
                            (d.t - t).abs() <= 18.0 * SPS as f64
                                && (d.hz - f).abs() <= 3.0 * SYMBOL_RATE
                        }) {
                            *u = true;
                        }
                    }
                    self.frame(&mut frame);
                }
                Some(None) => {}
                None => {
                    self.failed.insert(k);
                    self.held.counters.failed += 1;
                    self.dirty = true;
                }
            }
        }
    }

    fn frame(&mut self, f: &mut MiotyFrame) {
        let stamp = match self.clock.as_ref().and_then(|c| self.live.then(|| c.now())) {
            Some(w) => Stamp::at(f.t).with_wall(w),
            None => Stamp::at(f.t),
        };
        f.wall = stamp.wall;
        self.held.counters.frames += 1;
        self.snr_db = Some(f64::from(f.snr_db));
        let counter = code::read_mac(&f.mpdu).map_or(0, |m| m.counter);
        if let Ok(up) = self.held.devices.upsert(&f.id, stamp, || MiotyDevice {
            id: f.id.clone(),
            frames: 0,
            counter,
            snr_db: f.snr_db,
            freq_hz: f.freq_hz,
        }) {
            let d = up.row;
            d.frames += 1;
            d.counter = counter;
            d.snr_db = f.snr_db;
            d.freq_hz = f.freq_hz;
        }
        self.held.frames.push(f.clone());
        self.out.push(Timed {
            time: f.t,
            value: f.clone(),
        });
        self.dirty = true;
    }

    /// Drops the samples and detections older than the history.
    fn trim(&mut self) {
        let keep = (HISTORY_S * WORK_RATE_HZ) as usize;
        if self.hist.x.len() > keep + keep / 4 {
            let cut = self.hist.x.len() - keep;
            self.hist.x.drain(..cut);
            self.hist.start += cut as i64;
            let from = self.hist.start as f64;
            let n = self.dets.partition_point(|d| d.t < from);
            self.dets.drain(..n);
            self.used.drain(..n);
            let start = self.hist.start;
            self.failed.retain(|k| k.1 * SPS as i64 >= start);
        }
    }
}

/// The key of a hypothesis: the pattern, the start to a symbol and the frequency to 100 Hz.
fn key(f: &Fit) -> (usize, i64, i64) {
    (
        f.pattern,
        (f.t0 / SPS as f64).round() as i64,
        (f.f0 / 100.0).round() as i64,
    )
}

impl Decoder for MiotyDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "ism.mioty"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<MiotyFrame>("frames")]
    }
    fn settings(&self) -> Arc<sdrkit_core::Schema> {
        MiotySetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(MiotySetting::MinBursts, Value::I64(self.min_bursts as i64))])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match MiotySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MiotySetting::MinBursts => {
                self.min_bursts = MIN_BURSTS.read(id, v)?.clamp(3, 24) as usize;
            }
        }
        Ok(())
    }
    fn results(&self) -> Published {
        crate::typed::results(ID)
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE_HZ {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.tuned_hz = input.tuned_hz;
        self.fresh = true;
        let off = input.signal_offset_hz();
        self.mixer = (off.abs() > 0.5).then(|| Nco::new(input.rate, -off));
        self.centre_hz = input.centre_hz;
        self.resampler = Some(Resampler::new(input.rate, WORK_RATE_HZ).map_err(ConfigError::from)?);
        self.search = Some(
            PilotSearch::new(PilotConfig {
                template: self.core.clone(),
                fft: 1024,
                step: SPS / 2,
                over_mean: 18.0,
                over_peak: 0.1,
                peak_bins: 12,
                merge_samples: 14.0 * SPS as f64,
                merge_hz: 300.0,
                shadow: 0.25,
                final_samples: 16.0 * SPS as f64,
                rate_hz: WORK_RATE_HZ,
            })
            .map_err(ConfigError::from)?,
        );
        self.hist.x.clear();
        // The history stays below this, so it never grows while samples flow.
        self.hist
            .x
            .reserve(((HISTORY_S * 1.3 + 1.0) * WORK_RATE_HZ) as usize);
        self.found.reserve(64);
        self.dets.reserve(4096);
        self.used.reserve(4096);
        self.hist.start = 0;
        self.dets.clear();
        self.used.clear();
        self.failed.clear();
        self.next_fit = (FIT_EVERY_S * WORK_RATE_HZ) as i64;
        Ok(vec![Format {
            rate: 0.0,
            frame_len: 0,
            media: None,
            ..input.with_kind(Kind::typed::<MiotyFrame>())
        }])
    }
    fn retune(&mut self, input: &Format) {
        self.centre_hz = input.centre_hz;
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
        let (Some(rs), Some(search)) = (self.resampler.as_mut(), self.search.as_mut()) else {
            return;
        };
        self.work.clear();
        rs.process(input, &mut self.work);
        self.hist.x.extend_from_slice(&self.work);
        let mut found = std::mem::take(&mut self.found);
        search.process(&self.work, |d| found.push(d));
        for d in found.drain(..) {
            let i = self.dets.partition_point(|e| e.t <= d.t);
            self.dets.insert(i, d);
            self.used.insert(i, false);
            self.held.counters.bursts += 1;
        }
        self.found = found;
        if self.hist.end() >= self.next_fit {
            self.next_fit = self.hist.end() + (FIT_EVERY_S * WORK_RATE_HZ) as i64;
            self.fit();
            self.trim();
        }
        out.items::<MiotyFrame>(0).append(&mut self.out);
    }
    fn finish(&mut self, out: &mut Outbox) {
        if let Some(s) = self.search.as_mut() {
            let mut found = Vec::new();
            s.finish(|d| found.push(d));
            for d in found {
                let i = self.dets.partition_point(|e| e.t <= d.t);
                self.dets.insert(i, d);
                self.used.insert(i, false);
                self.held.counters.bursts += 1;
            }
        }
        self.fit();
        out.items::<MiotyFrame>(0).append(&mut self.out);
    }
    fn report(&mut self, p: &mut Published) {
        if self.level_n > 0 {
            self.level_dbfs =
                Some(10.0 * (self.level_acc / self.level_n as f64).max(1e-20).log10());
            self.level_acc = 0.0;
            self.level_n = 0;
        }
        let c = self.held.counters;
        p.put(&MiotyFields {
            frames_ok: c.frames as i64,
            bursts_detected: c.bursts as i64,
            failed_hypotheses: c.failed as i64,
            devices: self.held.devices.len() as i64,
            snr_db: self.snr_db,
            level_dbfs: self.level_dbfs,
            note: NOT_CONFIRMED,
        });
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&MIOTY_FRAMES);
            p.clear_rows(&MIOTY_DEVICES);
        }
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.held.frames, &MIOTY_FRAMES, self.now, p);
            shaped::rows(&self.held.devices, &MIOTY_DEVICES, self.now, p);
        }
        let newest = self.held.frames.newest_first().next();
        let recent = newest.is_some_and(|f| self.now - f.t <= RECENT_S);
        let lock = if !self.started {
            LockState::Off
        } else if recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let summary = match newest {
            Some(f) if recent => {
                format!("mioty: {} frames; last {}, {}", c.frames, f.pattern, f.mac)
            }
            _ if c.bursts > 0 => {
                format!("mioty: {} bursts detected, {} frames", c.bursts, c.frames)
            }
            _ if self.started => "mioty: searching for radio bursts".to_string(),
            _ => "mioty: waiting for samples".to_string(),
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
        if !kept.table(MIOTY_FRAMES.key) {
            self.held.frames.clear();
            self.cleared = true;
        }
        if !kept.table(MIOTY_DEVICES.key) {
            self.held.devices.clear();
            self.cleared = true;
        }
        let c = &mut self.held.counters;
        if !kept.field("frames_ok") {
            c.frames = 0;
        }
        if !kept.field("bursts_detected") {
            c.bursts = 0;
        }
        if !kept.field("failed_hypotheses") {
            c.failed = 0;
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

/// The `mioty` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a value of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("rx", DecoderComponent::new(MiotyDecoder::default()))
        .input("rx")
        .output("rx")
        .clock(REPORT_S)
        .status_from("rx")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE_HZ,
            &crate::family::min_rate_message("mioty", MIN_RATE_HZ),
        )
        .route(MIN_BURSTS.key(), &["rx.min_bursts"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::MiotyGenerator::from_config(cfg)?))
}

/// mioty. The default frequency is that of the test signal; the session gives the real one.
pub static MIOTY: StackDescriptor = StackDescriptor {
    id: ID,
    name: "mioty",
    group: "IoT, 868 / 902 MHz",
    blurb: "mioty (TS-UNB) uplinks: the radio bursts of each frame found by their TSMA pattern, the frame decoded and checked, and the MAC fields shown; payloads stay encrypted.",
    tuning: Tuning::anywhere(TEST_DIAL_HZ, 1e6, MIN_RATE_HZ, 150e3),
    submodes: &[],
    settings: MiotySetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

fn verify_cases(o: &sdrkit_core::verify::VerifyOptions) -> Vec<sdrkit_core::verify::VerifyCase> {
    let mut c = sdrkit_core::verify::VerifyCase::new("test signal, uplink frames", o, 0.0);
    c.seconds = o.seconds.max(6.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = MiotyFields::read(r.fields()).map_or(0, |f| f.frames_ok);
        vec![crate::family::at_least_value("frames_ok", n, 2)]
    });
    vec![c]
}

/// The verify case of `mioty`.
pub static VERIFY: sdrkit_core::verify::VerifyHooks = sdrkit_core::verify::VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The benchmark of the receiver: 6 s of the test signal at 1 Msps, fed in one piece.
pub fn benchmarks() -> Vec<sdrkit_core::bench::Benchmark> {
    vec![sdrkit_core::bench::Benchmark {
        name: "mioty.receiver_1m",
        run: bench_receiver,
    }]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::bench::BenchResult {
    let rate = 1e6;
    let mut x = vec![Complex32::default(); (rate * 6.0) as usize];
    if let Ok(mut s) = gen::scene(rate, 1) {
        s.generate(&mut x);
    }
    let Ok(mut rx) = receiver(&StackConfig::new(Values::defaults(&MiotySetting::schema()))) else {
        return sdrkit_core::bench::BenchResult::default();
    };
    let _ =
        rx.configure(&sdrkit_core::InputInfo::new(rate, TEST_DIAL_HZ, TEST_DIAL_HZ).recording());
    let mut out = sdrkit_core::Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), 6.0, || {
        out.clear();
        rx.feed(&x, &mut out);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_soft_values_of_a_clean_burst_give_its_bits() {
        let mpdu: Vec<u8> = (0..15).map(|k| (k * 29 + 3) as u8).collect();
        let (coded, _, _) = code::encode(&mpdu, 0);
        let levels = gen::burst_levels(&coded, 15);
        let x = gen::burst_samples(&levels[0], WORK_RATE_HZ);
        let mut d = MiotyDecoder::default();
        let centre = 1000 + 888;
        d.hist.x = vec![Complex32::default(); 4000];
        for (i, v) in x.iter().enumerate() {
            d.hist.x[1000 + i] = *v;
        }
        let r = d.refine(&d.core, centre as f64 + 10.0, 40.0);
        let soft = d.soft(&r);
        let wrong: Vec<usize> = (0..36)
            .filter(|&n| !(12..24).contains(&n))
            .filter(|&n| (soft[n] < 0.0) != (levels[0][n] < 0.0))
            .collect();
        assert_eq!((r.at, wrong), (centre as i64, vec![]), "{soft:?}");
    }

    #[test]
    fn a_clean_frame_at_the_working_rate_decodes() {
        let mpdu = gen::test_mpdu(1, 15, &mut sdrkit_algo_sim::rng::Rng::new(3));
        let mut d = MiotyDecoder::default();
        let len = 300_000;
        d.hist.x = vec![Complex32::default(); len];
        let t0 = 5000usize;
        for (dt, c, x) in gen::frame(&mpdu, 16, WORK_RATE_HZ) {
            let at = t0 + (dt * WORK_RATE_HZ).round() as usize - 888;
            let f = f64::from(c) * SYMBOL_RATE;
            for (i, v) in x.iter().enumerate() {
                let ph = (std::f64::consts::TAU * f * (at + i) as f64 / WORK_RATE_HZ) as f32;
                d.hist.x[at + i] += v * Complex32::from_polar(1.0, ph);
            }
        }
        let fit = Fit {
            pattern: 16,
            t0: t0 as f64,
            f0: 0.0,
            matched: vec![None; 24],
            score: 24,
        };
        let f = d.decode(&fit).unwrap().unwrap().0;
        assert_eq!(f.mpdu, mpdu);
    }

    #[test]
    fn a_clean_frame_through_the_resampler_decodes() {
        let mpdu: Vec<u8> = (0..15).map(|k| (k * 29 + 3) as u8).collect();
        let rate = 250e3;
        let mut x = vec![Complex32::default(); (rate * 1.2) as usize];
        let t0_s = 0.1;
        for (dt, c, b) in gen::frame(&mpdu, 16, rate) {
            let at = ((t0_s + dt - 18.5 / SYMBOL_RATE) * rate).round() as usize;
            let f = -12.0 * SYMBOL_RATE + f64::from(c) * SYMBOL_RATE;
            for (i, v) in b.iter().enumerate() {
                let ph = (std::f64::consts::TAU * f * (at + i) as f64 / rate) as f32;
                x[at + i] += v * Complex32::from_polar(1.0, ph);
            }
        }
        let mut r = Resampler::<Complex32>::new(rate, WORK_RATE_HZ).unwrap();
        let mut d = MiotyDecoder::default();
        r.process(&x, &mut d.hist.x);
        eprintln!("delay {}", r.delay());
        let fit = Fit {
            pattern: 16,
            t0: t0_s * WORK_RATE_HZ + r.delay() * WORK_RATE_HZ / rate,
            f0: -12.0 * SYMBOL_RATE,
            matched: vec![None; 24],
            score: 24,
        };
        let f = d.decode(&fit).unwrap().unwrap().0;
        assert_eq!(f.mpdu, mpdu);
    }
}
