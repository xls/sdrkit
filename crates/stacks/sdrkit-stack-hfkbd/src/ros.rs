//! ROS, identifier only: the framing, coding and FEC of ROS are not public, so nothing is read. Every 2 s the last 16 s of the
//! sideband are measured and a fit names the sub-mode by its symbol rate. The generator makes the publicly reported physical
//! layer only (random tones of the 144-tone grid) and the signals that must not be named ROS.

use crate::decoder::{self, flush_logs, put_text, Emit, HfGenerator, Modulator, GEN_SECTION};
use crate::front::{Sideband, SIDEBAND};
use crate::typed::{RosDetectionEvent, RosDetectionRow, RosFields, ROS_DETECTION, ROS_DETECTIONS};
use crate::util::{median, Tone};
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Complex32, ConfigError, GenConfig, Generator, LockState, SeriesId, Setting, SignalInfo, Status,
    Values,
};
use sdrkit_graph::{no_values, Format, Item, Kept, Outbox, Port, RateFrom, Timed};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, Seen};
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The sub-modes (id, name, baud, width, identified).
pub const SUBMODES: [(&str, &str, f64, f64, bool); 7] = [
    ("ros16", "ROS 16 baud", 16.0, 2250.0, true),
    ("ros8", "ROS 8 baud", 8.0, 2250.0, true),
    ("ros4", "ROS 4 baud", 4.0, 2250.0, true),
    ("ros1", "ROS 1 baud (older versions)", 1.0, 2200.0, false),
    ("rosmf", "ROS MF (MF1 / MF2 / MF7)", 0.0, 100.0, false),
    ("roseme", "ROS EME (VHF)", 0.0, 50.0, false),
    ("ros500", "ROS HF 500 Hz", 0.0, 500.0, false),
];

/// The grid spacing, Hz.
pub const GRID: f64 = 15.625;
/// The centre of the tone band, Hz.
pub const CENTRE: f64 = 1550.0;

/// One identifier report (`ros.detection`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RosDetection {
    /// Signal time.
    pub time: f64,
    /// The sub-mode id, when the rate is a reported one.
    pub sub_mode: Option<String>,
    /// Its name ("ROS-like" when not).
    pub name: String,
    /// What fitted and what did not.
    pub why: String,
    /// Symbol rate, baud.
    pub baud: f64,
    /// Grid spacing, Hz.
    pub spacing: f64,
    /// How well the tones sit on the grid, 0 to 1.
    pub fit: f64,
    /// Lowest and highest tone, Hz.
    pub tones_hz: [f64; 2],
    /// Highest minus lowest.
    pub span_hz: f64,
    /// Median share of a frame's power in its strongest tone.
    pub purity: f64,
    /// Share of frames with one clear tone.
    pub clear: f64,
    /// Distinct grid positions used.
    pub distinct: usize,
    /// 0 to 1.
    pub score: f64,
    /// Fitted on few symbols ("perhaps").
    pub weak: bool,
    /// ROS identified.
    pub detected: bool,
    /// A signal of about the right width.
    pub wide_enough: bool,
}

impl Item for RosDetection {
    const KIND: &'static str = "ros.detection";
}

impl Shaped for RosDetection {
    type Row = RosDetectionRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&RosDetectionRow {
            time_s: self.time,
            sub_mode: self.sub_mode.as_deref(),
            name: &self.name,
            symbol_rate_hz: Hertz(self.baud),
            why: &self.why,
        });
    }
}

/// The state of the identifier.
#[derive(Clone, Debug, Default)]
struct RosState {
    state: u8,
    detection: Option<RosDetection>,
    offset_hz: f64,
    events: Option<LogRing<RosDetection>>,
    good: u64,
    bad: u64,
}

sdrkit_core::choice_enum! {
    /// What the ROS test signal sends.
    pub enum GenSignal {
        /// Random tones of the 144-tone grid.
        Grid = ("grid", "Tone grid"),
        /// Noise only.
        Noise = ("noise", "Noise"),
        /// Two-tone FSK.
        Fsk = ("fsk", "FSK"),
        /// A carrier.
        Carrier = ("carrier", "Carrier"),
        /// A sweep.
        Sweep = ("sweep", "Sweep"),
    }
}

/// What the test signal sends.
pub const GEN_SIGNAL: Setting<field::Choice<GenSignal>> =
    Setting::choice("gen_signal", "Test signal", GenSignal::Grid)
        .section(GEN_SECTION)
        .advanced();
/// The symbol rate of the tone grid.
pub const GEN_BAUD: Setting<field::F64> = Setting::new("gen_baud", "Symbol rate", "Bd", 16.0)
    .range(0.5, 64.0, 0.0)
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of the ROS stack.
    pub enum RosGenSetting {
        /// What it sends.
        Signal = GEN_SIGNAL,
        /// The symbol rate.
        Baud = GEN_BAUD,
    }
}

const N: usize = 256;
const HOP: usize = 64;
const FRAMES: usize = 16 * 8000 / HOP;

#[derive(Clone, Copy, Debug, Default)]
struct Frame {
    freq: f64,
    share: f32,
}

/// The identifier.
pub struct RosDecoder {
    spec: WindowedFft,
    hist: SampleRing<Complex32>,
    hop: usize,
    frames: VecDeque<Frame>,
    since: usize,
    t: u64,
    det: RosState,
    dial: f64,
    new_events: Vec<RosDetection>,
    logs: Vec<String>,
    status: Status,
    waterfall: Option<SeriesId>,
    wf_acc: Vec<f32>,
    wf_n: usize,
    wf_row: [f32; 70],
    prev: (usize, Complex32),
}

impl Default for RosDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl RosDecoder {
    /// An identifier.
    pub fn new() -> Self {
        RosDecoder {
            spec: WindowedFft::new(N, N, Window::Hann).expect("a window that fits the transform"),
            hist: SampleRing::new(N).expect("a ring of at least one sample"),
            hop: 0,
            frames: VecDeque::with_capacity(FRAMES + 1),
            since: 0,
            t: 0,
            det: RosState {
                events: Some(LogRing::new(100)),
                ..Default::default()
            },
            dial: 0.0,
            new_events: Vec::new(),
            logs: Vec::new(),
            status: Status::default(),
            waterfall: None,
            wf_acc: vec![0.0; 70],
            wf_n: 0,
            wf_row: [-60.0; 70],
            prev: (0, Complex32::default()),
        }
    }

    fn frame(&mut self) {
        let p = self.spec.run_power(self.hist.last(N));
        let bin = 8000.0 / N as f64;
        let lo = (200.0 / bin) as usize;
        let hi = (2900.0 / bin) as usize;
        let (mut best, mut bi, mut tot) = (0.0f32, lo, 0.0f32);
        for (i, &v) in p.iter().enumerate().take(hi).skip(lo) {
            tot += v;
            if v > best {
                best = v;
                bi = i;
            }
        }
        let (a, b, c) = (p[bi - 1] as f64, p[bi] as f64, p[bi + 1] as f64);
        let den = a - 2.0 * b + c;
        let d = if den.abs() > 1e-30 {
            (0.5 * (a - c) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let near = p[bi - 1] + p[bi] + p[bi + 1];
        let coarse = (bi as f64 + d) * bin;
        // On the same bin as the frame before, the phase step of the bin provides the fine frequency.
        let z = self.spec.bins()[bi];
        let mut freq = coarse;
        if self.prev.0 == bi && self.prev.1.norm_sqr() > 0.0 {
            let step = (z * self.prev.1.conj()).arg() as f64 / TAU;
            let per = 8000.0 / HOP as f64;
            let n = (coarse / per - step).round();
            let fine = (step + n) * per;
            if (fine - coarse).abs() < 0.5 * bin {
                freq = fine;
            }
        }
        self.prev = (bi, z);
        self.frames.push_back(Frame {
            freq,
            share: if tot > 0.0 { near / tot } else { 0.0 },
        });
        if self.frames.len() > FRAMES {
            self.frames.pop_front();
        }
        // The waterfall: 70 bins of 50 Hz.
        let p = self.spec.power();
        for (k, w) in self.wf_acc.iter_mut().enumerate() {
            let a = (k as f64 * 50.0 / bin) as usize;
            let b = ((((k + 1) as f64 * 50.0) / bin) as usize).max(a + 1);
            *w += p[a..b.min(N / 2)].iter().sum::<f32>();
        }
        self.wf_n += 1;
        if self.wf_n == 22 {
            let mx = self.wf_acc.iter().cloned().fold(1e-20f32, f32::max);
            for (r, &v) in self.wf_row.iter_mut().zip(&self.wf_acc) {
                *r = 10.0 * (v / mx).max(1e-6).log10();
            }
            self.wf_acc.iter_mut().for_each(|v| *v = 0.0);
            self.wf_n = 0;
        }
    }

    /// Measures the last 16 s.
    pub fn analyse(&mut self) -> Option<RosDetection> {
        if self.frames.len() < 64 {
            return None;
        }
        // Frames where one tone stands out (noise spreads over the band).
        let on: Vec<(usize, Frame)> = self
            .frames
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, f)| f.share > 0.25)
            .collect();
        let time = self.t as f64 / 8000.0;
        let mut d = RosDetection {
            time,
            ..Default::default()
        };
        if on.len() < 32 {
            return None;
        }
        let shares: Vec<f32> = on.iter().map(|(_, f)| f.share).collect();
        let mut s2 = Vec::new();
        d.purity = median(&shares, &mut s2) as f64;
        d.clear = shares.iter().filter(|&&s| s > 0.5).count() as f64 / shares.len() as f64;
        // Tone segments: runs of frames on one tone.
        let mut segs: Vec<(f64, f64, usize)> = vec![]; // (start frame, mean freq, frames)
        let mut cur: Option<(usize, f64, usize)> = None;
        let mut last_i = 0;
        for &(i, f) in &on {
            match cur {
                Some((s, m, n)) if i == last_i + 1 && (f.freq - m / n as f64).abs() < 6.0 => {
                    cur = Some((s, m + f.freq, n + 1))
                }
                _ => {
                    if let Some((s, m, n)) = cur {
                        segs.push((s as f64, m / n as f64, n));
                    }
                    cur = Some((i, f.freq, 1));
                }
            }
            last_i = i;
        }
        if let Some((s, m, n)) = cur {
            segs.push((s as f64, m / n as f64, n));
        }
        let segs: Vec<(f64, f64, usize)> = segs.into_iter().filter(|s| s.2 >= 2).collect();
        if segs.len() < 8 {
            d.why = "too few tone segments".into();
            return Some(d);
        }
        // The tone range.
        let mut fs: Vec<f64> = segs.iter().map(|s| s.1).collect();
        fs.sort_by(|a, b| a.total_cmp(b));
        d.tones_hz = [fs[fs.len() / 50], fs[fs.len() - 1 - fs.len() / 50]];
        d.span_hz = d.tones_hz[1] - d.tones_hz[0];
        d.wide_enough = (1500.0..=2600.0).contains(&d.span_hz);
        // The grid: the spacing at which the tones line up.
        let mut best = (0.0, 0.0);
        let mut sp = 10.0;
        while sp <= 25.0 {
            let (mut c, mut s) = (0.0, 0.0);
            for &f in &fs {
                let p = TAU * f / sp;
                c += p.cos();
                s += p.sin();
            }
            let coh = (c * c + s * s).sqrt() / fs.len() as f64;
            if coh > best.0 {
                best = (coh, sp);
            }
            sp += 0.005;
        }
        d.fit = best.0;
        d.spacing = best.1;
        let base = fs[0];
        let mut pos: Vec<i64> = fs
            .iter()
            .map(|f| ((f - base) / d.spacing).round() as i64)
            .collect();
        pos.sort();
        pos.dedup();
        d.distinct = pos.len();
        // The symbol rate: segment starts (in seconds) line up at the symbol period.
        let starts: Vec<f64> = segs.iter().map(|s| s.0 * HOP as f64 / 8000.0).collect();
        let mut rates = vec![];
        let mut r = 0.5;
        while r <= 40.0 {
            let (mut c, mut s) = (0.0, 0.0);
            for &t in &starts {
                c += (TAU * r * t).cos();
                s += (TAU * r * t).sin();
            }
            rates.push((r, (c * c + s * s).sqrt() / starts.len() as f64));
            r += 0.02;
        }
        let top = rates.iter().map(|x| x.1).fold(0.0, f64::max);
        // The lowest rate that fits about as well as the best (a rate's multiples fit too).
        let (rate, coh) = rates
            .iter()
            .copied()
            .find(|x| x.1 > 0.8 * top && x.0 >= 2.0)
            .unwrap_or((0.0, 0.0));
        // Refine around it.
        let mut rb = (rate, coh);
        let mut q = rate - 0.05;
        while q < rate + 0.05 {
            let (mut c, mut s) = (0.0, 0.0);
            for &t in &starts {
                c += (TAU * q * t).cos();
                s += (TAU * q * t).sin();
            }
            let v = (c * c + s * s).sqrt() / starts.len() as f64;
            if v > rb.1 {
                rb = (q, v);
            }
            q += 0.002;
        }
        d.baud = rb.0;
        let symbols = segs.len();
        d.weak = symbols < 40;
        let grid_ok = (d.spacing - GRID).abs() < 0.3 && d.fit > 0.6;
        let tones_ok = d.distinct >= 16 && d.purity > 0.4;
        d.score = (d.fit * 0.4
            + (1.0 - (d.spacing - GRID).abs().min(1.0)) * 0.2
            + d.purity.min(1.0) * 0.2
            + (d.wide_enough as u8 as f64) * 0.2)
            .clamp(0.0, 1.0);
        let sub = SUBMODES
            .iter()
            .find(|s| s.4 && (d.baud - s.2).abs() < 0.05 * s.2);
        if grid_ok && tones_ok && d.wide_enough {
            match sub {
                Some(s) if !d.weak => {
                    d.sub_mode = Some(s.0.into());
                    d.name = s.1.into();
                    d.detected = true;
                    d.why = format!(
                        "{:.2} Bd, {:.3} Hz grid (fit {:.2}), {} tones over {:.0} Hz",
                        d.baud, d.spacing, d.fit, d.distinct, d.span_hz
                    );
                }
                Some(s) => {
                    d.name = format!("{} (perhaps)", s.1);
                    d.why = format!(
                        "few symbols ({symbols}); {:.2} Bd on the {:.3} Hz grid",
                        d.baud, d.spacing
                    );
                    d.detected = true;
                }
                None => {
                    d.name = format!("ROS-like (rate {:.1} Bd not a reported one)", d.baud);
                    d.why = format!("the {:.3} Hz grid fits, the rate does not", d.spacing);
                    d.weak = true;
                }
            }
        } else {
            d.why = format!(
                "grid {:.3} Hz (fit {:.2}), {} positions, purity {:.2}, span {:.0} Hz",
                d.spacing, d.fit, d.distinct, d.purity, d.span_hz
            );
        }
        Some(d)
    }
}

impl sdrkit_graph::Decoder for RosDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.ros"
    }
    fn outputs(&self) -> Vec<Port> {
        let mut o = decoder::outputs();
        o.push(Port::typed::<RosDetection>("detections").rate_from(RateFrom::Fixed(0.0)));
        o
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Published {
        crate::typed::results("ros")
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.dial = input.tuned_hz;
        self.frames.clear();
        let mut f = decoder::formats();
        f.push(Format::new(
            sdrkit_graph::Kind::typed::<RosDetection>(),
            0.0,
        ));
        Ok(f)
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.frames.clear();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        for &s in x {
            self.hist.push(s);
            self.t += 1;
            self.hop += 1;
            if self.hop == HOP {
                self.hop = 0;
                self.frame();
                self.since += 1;
                if self.since >= 2 * 8000 / HOP {
                    self.since = 0;
                    if let Some(d) = self.analyse() {
                        if d.detected {
                            self.det.good += 1;
                            self.logs.push(format!("{}: {}", d.name, d.why));
                            let freq = self.dial + 0.5 * (d.tones_hz[0] + d.tones_hz[1]);
                            out.event(ROS_DETECTION.event(
                                d.time,
                                freq,
                                &RosDetectionEvent {
                                    sub_mode: d.sub_mode.as_deref(),
                                    name: &d.name,
                                    symbol_rate_hz: Hertz(d.baud),
                                },
                            ));
                            if let Some(ev) = &mut self.det.events {
                                ev.push(d.clone());
                            }
                            self.new_events.push(d.clone());
                        } else {
                            self.det.bad += 1;
                        }
                        self.det.state = if d.detected && !d.weak {
                            2
                        } else if d.wide_enough || d.detected {
                            1
                        } else {
                            0
                        };
                        self.det.offset_hz = 0.5 * (d.tones_hz[0] + d.tones_hz[1]) - CENTRE;
                        self.det.detection = Some(d);
                    } else {
                        self.det.state = 0;
                        self.det.detection = None;
                    }
                }
            }
        }
        if !self.new_events.is_empty() {
            let o = out.items::<RosDetection>(decoder::EXTRA);
            for d in self.new_events.drain(..) {
                o.push(Timed {
                    time: d.time,
                    value: d,
                });
            }
        }
    }
    fn report(&mut self, res: &mut Published) {
        let state = self.det.state;
        let st = &mut self.status;
        st.lock_state =
            [LockState::Searching, LockState::Tracking, LockState::Locked][state as usize];
        st.offset_hz = Some(self.det.offset_hz);
        match (&self.det.detection, state) {
            (Some(d), 2) => put_text(
                &mut st.summary,
                format_args!(
                    "{}, tones {:.0} - {:.0} Hz",
                    d.name, d.tones_hz[0], d.tones_hz[1]
                ),
            ),
            (Some(d), _) if d.detected => put_text(
                &mut st.summary,
                format_args!(
                    "{}, tones {:.0} - {:.0} Hz",
                    d.name, d.tones_hz[0], d.tones_hz[1]
                ),
            ),
            (_, 1) => put_text(
                &mut st.summary,
                format_args!("ROS: a signal of about the right width, not identified"),
            ),
            _ => put_text(&mut st.summary, format_args!("ROS: no signal")),
        }
        res.results_mut().set_status(st);
        let d = self.det.detection.as_ref();
        res.put(&RosFields {
            sub_mode: d.and_then(|d| d.sub_mode.as_deref()),
            name: d.map(|d| d.name.as_str()),
            why: d.map(|d| d.why.as_str()),
            symbol_rate_hz: d.map(|d| Hertz(d.baud)),
            spacing_hz: d.map(|d| Hertz(d.spacing)),
            fit_pct: d.map(|d| (d.fit * 100.0).round()),
            lowest_hz: d.map(|d| Hertz(d.tones_hz[0])),
            highest_hz: d.map(|d| Hertz(d.tones_hz[1])),
            purity_pct: d.map(|d| (d.purity * 100.0).round()),
            distinct: d.map(|d| d.distinct as i64),
            score_pct: d.map(|d| (d.score * 100.0).round()),
            detected: d.is_some_and(|d| d.detected),
            weak: d.is_some_and(|d| d.weak),
            offset_hz: Hertz(self.det.offset_hz),
            detections: self.det.good as i64,
            misfits: self.det.bad as i64,
        });
        if self.waterfall.is_none() {
            self.waterfall = res.results().decl().series_id("waterfall");
        }
        if let Some(id) = self.waterfall {
            let row = &self.wf_row;
            res.results_mut().update_series(id, |sr| {
                sr.points.clear();
                for (k, &v) in row.iter().enumerate() {
                    sr.points.push([(k as f32 + 0.5) * 50.0, v]);
                }
                true
            });
        }
        if let Some(ev) = &self.det.events {
            shaped::log_rows(ev, &ROS_DETECTIONS, 0.0, res);
        }
        flush_logs(&mut self.logs, res);
    }
    fn clear(&mut self) {
        self.det.events = Some(LogRing::new(100));
    }
    fn forget(&mut self, kept: &Kept) {
        self.frames.clear();
        self.det.detection = None;
        self.det.state = 0;
        if !kept.table(ROS_DETECTIONS.key) {
            self.clear();
        }
    }
}

/// What the generator sends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RosSignal {
    /// Random tones of the 144-tone grid at a rate (not ROS encoding).
    Grid {
        /// Symbols per second.
        baud: f64,
    },
    /// Noise only.
    Noise,
    /// A 2-tone FSK like RTTY (170 Hz, 45.45 baud).
    Fsk,
    /// A single carrier.
    Carrier,
    /// A carrier sweeping over the band.
    Sweep,
}

/// The test signal, in the upper or the lower sideband (LSB mirrors).
pub struct RosModulator {
    sig: RosSignal,
    rate: f64,
    lsb: bool,
    osc: Tone,
    t: f64,
    left: f64,
    tone: f64,
    rng: Rng,
}

impl RosModulator {
    /// A test signal. `seed` sets the random tones.
    pub fn new(sig: RosSignal, rate: f64, lsb: bool, seed: u64) -> Self {
        RosModulator {
            sig,
            rate,
            lsb,
            osc: Tone::new(rate),
            t: 0.0,
            left: 0.0,
            tone: CENTRE,
            rng: Rng::new(seed),
        }
    }
}

impl Modulator for RosModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        for o in out {
            let f = match self.sig {
                RosSignal::Grid { baud } => {
                    if self.left <= 0.0 {
                        let k = self.rng.below(144) as f64;
                        self.tone = CENTRE + (k - 71.5) * GRID;
                        self.left += self.rate / baud;
                    }
                    self.left -= 1.0;
                    Some(self.tone)
                }
                RosSignal::Noise => None,
                RosSignal::Fsk => {
                    if self.left <= 0.0 {
                        self.tone = if self.rng.bit() == 1 { 1445.0 } else { 1615.0 };
                        self.left += self.rate / 45.45;
                    }
                    self.left -= 1.0;
                    Some(self.tone)
                }
                RosSignal::Carrier => Some(CENTRE),
                RosSignal::Sweep => Some(450.0 + ((self.t * 400.0) % 2200.0)),
            };
            self.t += 1.0 / self.rate;
            *o = match f {
                Some(f) => {
                    let v = self.osc.next(f, 1.0);
                    if self.lsb {
                        v.conj()
                    } else {
                        v
                    }
                }
                None => Complex32::default(),
            };
        }
    }
}

/// The generator. It reads the settings `demod` (LSB mirrors the signal), `gen_signal` and `gen_baud`.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let v = &cfg.settings;
    let baud = v.get(&GEN_BAUD);
    let sig = match v.get(&GEN_SIGNAL) {
        GenSignal::Noise => RosSignal::Noise,
        GenSignal::Fsk => RosSignal::Fsk,
        GenSignal::Carrier => RosSignal::Carrier,
        GenSignal::Sweep => RosSignal::Sweep,
        GenSignal::Grid => RosSignal::Grid { baud },
    };
    let lsb = v.get(&SIDEBAND) == Sideband::Lsb;
    let m = RosModulator::new(sig, cfg.rate_hz, lsb, cfg.seed);
    Ok(HfGenerator::boxed(
        cfg.rate_hz,
        m,
        SignalInfo::new(2250.0).with_symbol_rate(baud),
    ))
}
