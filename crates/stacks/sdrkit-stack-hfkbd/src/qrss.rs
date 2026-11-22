//! QRSS and DFCW: very slow Morse read from a narrow spectrogram. The receiver finds the steady tone, mixes it to a stream of 100
//! samples a second, draws the spectrogram as picture rows, tracks the strongest line, and reads the keying with the element
//! length found from the runs.
//!
//! Standard: ITU-R M.1677-1, International Morse code. The slow forms have no formal document.

use crate::decoder::{self, flush_logs, put_text, Emit, Modulator, GEN_SECTION};
use crate::picture::HfPicture;
use crate::typed::{QrssFields, QrssTextEvent, QRSS_TEXT};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_modem::slowmorse::{self, Key, Keying, Run, Symbol, BIAS};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Complex32, ConfigError, GenConfig, Generator, LockState, Schema, Setting, SettingError,
    SettingId, SettingSet, SignalInfo, StackConfig, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kept, Outbox, Port};
use std::f64::consts::TAU;
use std::sync::Arc;

/// The rate of the narrow stream, Hz.
pub const NARROW_RATE: f64 = 100.0;
/// Samples of 8 kHz audio in one sample of the narrow stream.
const GROUP_N: usize = 80;
/// The element lengths tried, seconds.
pub const CANDIDATES: [f64; 16] = [
    1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 15.0, 20.0, 30.0, 45.0, 60.0, 90.0, 120.0,
];

sdrkit_core::choice_enum! {
    /// The keying: the submode.
    pub enum Submode {
        /// On-off keying.
        Qrss = ("qrss", "QRSS"),
        /// Dots and dashes on two frequencies.
        Dfcw = ("dfcw", "DFCW"),
    }
}

impl Submode {
    fn keying(self) -> Keying {
        match self {
            Submode::Qrss => Keying::Qrss,
            Submode::Dfcw => Keying::Dfcw,
        }
    }
}

/// The averaged power spectrum of `x` (8 kHz complex) over windows of the next power of two at or above 6 s, Hann, half
/// overlapping. Returns the spectrum and the windows averaged.
fn long_spectrum(x: &[Complex32]) -> (Vec<f64>, usize) {
    let n = ((6.0 * 8000.0) as usize).next_power_of_two();
    let mut fft = Fft::forward(n);
    let w: Vec<f32> = (0..n)
        .map(|i| (0.5 - 0.5 * (TAU * i as f64 / n as f64).cos()) as f32)
        .collect();
    let mut acc = vec![0f64; n];
    let mut buf = vec![Complex32::default(); n];
    let mut count = 0;
    let mut s = 0;
    while s + n <= x.len() {
        for i in 0..n {
            buf[i] = x[s + i] * w[i];
        }
        fft.process(&mut buf);
        for (a, b) in acc.iter_mut().zip(&buf) {
            *a += f64::from(b.norm_sqr());
        }
        count += 1;
        s += n / 2;
    }
    (acc, count)
}

/// The steady tone of 8 kHz complex audio: the strongest bin of 300 to 3500 Hz when it is more than 30 times the median of the
/// band and at least two windows were averaged.
pub fn find_centre(x: &[Complex32]) -> Option<f64> {
    let (spec, count) = long_spectrum(x);
    if count < 2 {
        return None;
    }
    let bin = 8000.0 / spec.len() as f64;
    let (lo, hi) = (
        (300.0 / bin).ceil() as usize,
        (3500.0 / bin).floor() as usize,
    );
    let mut band: Vec<f64> = spec[lo..=hi].to_vec();
    let (k, &peak) = band.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
    band.sort_by(f64::total_cmp);
    let med = band[band.len() / 2];
    (peak > 30.0 * med && peak > 0.0).then_some((lo + k) as f64 * bin)
}

/// A spectrogram of the narrow stream.
#[derive(Clone, Debug)]
pub struct Spectrogram {
    /// Seconds between rows.
    pub hop_s: f64,
    /// The width of a bin, Hz.
    pub bin_hz: f64,
    /// Bins each side of the centre.
    pub half: usize,
    /// Each row: the span's bins, power.
    pub rows: Vec<Vec<f32>>,
    /// The noise power of a bin.
    pub noise: f32,
}

impl Spectrogram {
    /// The spectrogram of the narrow stream `z` for element length `t` (window of half an element, hop a quarter window), the
    /// bins within ±`span_hz`/2 (at least ±2 bins).
    pub fn new(z: &[Complex32], t: f64, span_hz: f64) -> Spectrogram {
        let w = ((0.5 * t * NARROW_RATE).round() as usize).max(32);
        let n = (2 * w).next_power_of_two();
        let bin = NARROW_RATE / n as f64;
        let half = ((span_hz / 2.0 / bin).round() as usize)
            .max(2)
            .min(n / 2 - 1);
        let hop = w as f64 / 4.0;
        let win: Vec<f32> = (0..w)
            .map(|i| (0.5 - 0.5 * (TAU * i as f64 / w as f64).cos()) as f32)
            .collect();
        let mut fft = Fft::forward(n);
        let mut buf = vec![Complex32::default(); n];
        let mut rows = Vec::new();
        let mut cells = Vec::new();
        let mut k = 0usize;
        loop {
            let s = (k as f64 * hop).round() as usize;
            if s + w > z.len() {
                break;
            }
            buf.fill(Complex32::default());
            for i in 0..w {
                buf[i] = z[s + i] * win[i];
            }
            fft.process(&mut buf);
            let row: Vec<f32> = (0..=2 * half)
                .map(|j| buf[(n + j - half) % n].norm_sqr())
                .collect();
            cells.extend(buf.iter().step_by(2).map(Complex32::norm_sqr));
            rows.push(row);
            k += 1;
        }
        let noise = if cells.is_empty() {
            1e-30
        } else {
            let m = cells.len() / 2;
            cells.select_nth_unstable_by(m, f32::total_cmp);
            (cells[m] / 0.69).max(1e-30)
        };
        Spectrogram {
            hop_s: hop / NARROW_RATE,
            bin_hz: bin,
            half,
            rows,
            noise,
        }
    }

    /// The strongest line of each row: its level over the noise (dB) and its frequency from the centre (Hz), with a parabolic
    /// peak.
    pub fn track(&self) -> Vec<(f64, f64)> {
        self.rows
            .iter()
            .map(|r| {
                let (k, &p) = r
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .unwrap_or((0, &0.0));
                let frac = if k > 0 && k + 1 < r.len() {
                    let (a, b, c) = (f64::from(r[k - 1]), f64::from(p), f64::from(r[k + 1]));
                    let d = a - 2.0 * b + c;
                    if d < 0.0 {
                        0.5 * (a - c) / d
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
                let level = 10.0 * (f64::from(p) / f64::from(self.noise)).max(1e-12).log10();
                (level, (k as f64 + frac - self.half as f64) * self.bin_hz)
            })
            .collect()
    }
}

/// The key state of each row of a track, and the SNR (the 95th over the 20th percentile of the levels, dB). `None` when the
/// SNR is below 6 dB.
pub fn key_states(track: &[(f64, f64)], keying: Keying) -> Option<(Vec<Key>, f64)> {
    if track.len() < 8 {
        return None;
    }
    let mut lv: Vec<f64> = track.iter().map(|t| t.0).collect();
    lv.sort_by(f64::total_cmp);
    let pct = |p: f64| lv[((lv.len() - 1) as f64 * p).round() as usize];
    let (lo, hi) = (pct(0.2), pct(0.95));
    if hi - lo < 6.0 {
        return None;
    }
    let thr = 0.5 * (lo + hi);
    let down: Vec<bool> = track.iter().map(|t| t.0 > thr).collect();
    let mut keys: Vec<u8> = match keying {
        Keying::Qrss => down.iter().map(|&d| u8::from(d)).collect(),
        Keying::Dfcw => {
            let f: Vec<f64> = track
                .iter()
                .zip(&down)
                .filter(|(_, &d)| d)
                .map(|(t, _)| t.1)
                .collect();
            if f.is_empty() {
                return None;
            }
            let (mut a, mut b) = (
                f.iter().copied().fold(f64::MAX, f64::min),
                f.iter().copied().fold(f64::MIN, f64::max),
            );
            for _ in 0..12 {
                let mid = 0.5 * (a + b);
                let (lo_v, hi_v): (Vec<f64>, Vec<f64>) = f.iter().partition(|&&x| x < mid);
                if !lo_v.is_empty() {
                    a = lo_v.iter().sum::<f64>() / lo_v.len() as f64;
                }
                if !hi_v.is_empty() {
                    b = hi_v.iter().sum::<f64>() / hi_v.len() as f64;
                }
            }
            let mid = 0.5 * (a + b);
            track
                .iter()
                .zip(&down)
                .map(|(t, &d)| match (d, t.1 < mid) {
                    (false, _) => 0,
                    (true, true) => 1,
                    (true, false) => 2,
                })
                .collect()
        }
    };
    // A median of three frames: a frame across two elements takes the state of its neighbours.
    let raw = keys.clone();
    for i in 1..raw.len().saturating_sub(1) {
        let mut w = [raw[i - 1], raw[i], raw[i + 1]];
        w.sort_unstable();
        keys[i] = w[1];
    }
    let k = keys
        .into_iter()
        .map(|v| match (keying, v) {
            (_, 0) => Key::Up,
            (Keying::Qrss, _) => Key::Down,
            (Keying::Dfcw, 1) => Key::Lower,
            _ => Key::Upper,
        })
        .collect();
    Some((k, hi - lo))
}

/// The runs of a key-state sequence of rows `hop_s` apart, without the first and the last (cut by the ends of the record).
pub fn runs(keys: &[Key], hop_s: f64) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for &k in keys {
        match out.last_mut() {
            Some(r) if r.key == k => r.seconds += hop_s,
            _ => out.push(Run {
                key: k,
                seconds: hop_s,
            }),
        }
    }
    if out.len() >= 2 {
        out.remove(0);
        out.pop();
    } else {
        out.clear();
    }
    out
}

/// What a reading of the record found.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    /// The text, trailing spaces removed.
    pub text: String,
    /// The element length, seconds.
    pub element_s: f64,
    /// The SNR: the 95th over the 20th percentile of the levels, dB.
    pub snr_db: f64,
}

fn text_of(symbols: &[Symbol]) -> String {
    let mut s = String::new();
    for sym in symbols {
        match *sym {
            Symbol::Space => s.push(' '),
            Symbol::Letter { bits, len } => {
                let code = sdrkit_util_text::morse::Code { bits, len };
                s.push_str(sdrkit_util_text::morse::decode(code).unwrap_or("?"));
            }
        }
    }
    s.trim_end().to_owned()
}

/// The runs and the SNR of the narrow stream read at element length `t`.
fn runs_at(z: &[Complex32], t: f64, span_hz: f64, keying: Keying) -> Option<(Vec<Run>, f64)> {
    let sg = Spectrogram::new(z, t, span_hz);
    let (keys, snr) = key_states(&sg.track(), keying)?;
    Some((runs(&keys, sg.hop_s), snr))
}

/// The element length found in the narrow stream `z`: the candidate whose refined length fits the runs best, below an error of
/// 0.2. `None` when no candidate fits or the record is shorter than 12 elements of every candidate.
pub fn find_element(z: &[Complex32], span_hz: f64, keying: Keying) -> Option<f64> {
    let len_s = z.len() as f64 / NARROW_RATE;
    let mut best: Option<(f64, f64)> = None;
    for &c in CANDIDATES.iter().filter(|&&c| len_s >= 12.0 * c) {
        let Some((r, _)) = runs_at(z, c, span_hz, keying) else {
            continue;
        };
        if r.len() < 5 {
            continue;
        }
        let t = slowmorse::refine(&r, keying, c, BIAS);
        let e = slowmorse::error(&r, keying, t, BIAS);
        if best.is_none_or(|b| e < b.0) {
            best = Some((e, t));
        }
    }
    best.filter(|b| b.0 < 0.2).map(|b| b.1)
}

/// Reads the narrow stream `z` at element length `t` (refined on the runs). `None` with fewer than 4 runs or an SNR below
/// 6 dB.
pub fn read(
    z: &[Complex32],
    t: f64,
    span_hz: f64,
    keying: Keying,
    dash_below: bool,
) -> Option<Reading> {
    let (r, snr) = runs_at(z, t, span_hz, keying)?;
    if r.len() < 4 {
        return None;
    }
    let t = slowmorse::refine(&r, keying, t, BIAS);
    let el = slowmorse::elements(&r, keying, t, BIAS, dash_below);
    Some(Reading {
        text: text_of(&slowmorse::codes(&el)),
        element_s: t,
        snr_db: snr,
    })
}

/// The submode.
pub const SUBMODE: Setting<field::Choice<Submode>> =
    Setting::choice("submode", "Keying", Submode::Qrss)
        .help("QRSS: on-off keying. DFCW: dots and dashes on two frequencies.");
/// Automatic centre and element length.
pub const AUTO: Setting<field::Bool> =
    Setting::new("auto", "Automatic centre and element length", "", true);
/// The tone.
pub const CENTRE: Setting<field::Hz> = Setting::new("centre_hz", "Tone", "Hz", 1000.0)
    .range(300.0, 3500.0, 0.0)
    .help("Setting it turns the automatic centre off.");
/// The element length (0: found from the signal).
pub const ELEMENT_S: Setting<field::F64> = Setting::new("element_s", "Element length", "s", 0.0)
    .range(0.0, 120.0, 0.0)
    .help("0 finds the length from the keying; else 0.5 to 120 s.");
/// The width of the window.
pub const SPAN_HZ: Setting<field::Hz> =
    Setting::new("span_hz", "Window width", "Hz", 12.0).range(2.0, 50.0, 0.0);
/// DFCW: the dash below the dot frequency.
pub const DASH_BELOW: Setting<field::Bool> =
    Setting::new("dash_below", "Dash below the dot frequency", "", false);

sdrkit_core::settings! {
    /// The settings of [`QrssDecoder`].
    pub enum QrssSetting {
        /// The submode.
        Submode = SUBMODE,
        /// Automatic centre.
        Auto = AUTO,
        /// The tone.
        Centre = CENTRE,
        /// The element length.
        Element = ELEMENT_S,
        /// The window width.
        Span = SPAN_HZ,
        /// Dash below.
        DashBelow = DASH_BELOW,
    }
}

/// The note of the stack.
pub const NOTE_TEXT: &str = "Not confirmed by a public document: the DFCW gap lengths and the dot on the lower \
frequency are common practice; the 0.13 element by which the half-element window lengthens marks was measured on generated \
signals.";

/// Picture rows kept.
const ROWS_KEPT: usize = 400;
/// The narrow stream kept, seconds.
const KEEP_S: f64 = 3.0 * 3600.0;

/// The QRSS and DFCW receiver: 8 kHz complex sideband audio in, the picture and the text out.
pub struct QrssDecoder {
    submode: Submode,
    auto: bool,
    centre: f64,
    manual: bool,
    element_set: f64,
    span: f64,
    dash_below: bool,
    wait: Vec<Complex32>,
    found: Option<f64>,
    n: u64,
    next_try: u64,
    phase: f64,
    acc: Complex32,
    acc_n: usize,
    narrow: Vec<Complex32>,
    since_read: usize,
    element: Option<f64>,
    searched_at: usize,
    reading: Option<Reading>,
    rows_total: i64,
    dial: f64,
    status: Status,
    logs: Vec<String>,
    picture_id: u64,
}

impl Default for QrssDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl QrssDecoder {
    /// A receiver at the default settings.
    pub fn new() -> Self {
        QrssDecoder {
            submode: Submode::Qrss,
            auto: true,
            centre: CENTRE.default,
            manual: false,
            element_set: 0.0,
            span: SPAN_HZ.default,
            dash_below: false,
            wait: Vec::new(),
            found: None,
            n: 0,
            next_try: 0,
            phase: 0.0,
            acc: Complex32::default(),
            acc_n: 0,
            narrow: Vec::new(),
            since_read: 0,
            element: None,
            searched_at: 0,
            reading: None,
            rows_total: 0,
            dial: 0.0,
            status: Status::default(),
            logs: Vec::new(),
            picture_id: 0,
        }
    }

    /// The tone the narrow stream is made around: set by hand, or found.
    fn tone(&self) -> Option<f64> {
        if self.auto && !self.manual {
            self.found
        } else {
            Some(self.centre)
        }
    }

    fn restart(&mut self) {
        self.wait.clear();
        self.found = None;
        self.narrow.clear();
        self.element = None;
        self.searched_at = 0;
        self.reading = None;
        self.next_try = self.n + (30.0 * 8000.0) as u64;
        self.phase = 0.0;
        self.acc = Complex32::default();
        self.acc_n = 0;
    }

    fn narrow_in(&mut self, x: &[Complex32]) {
        let Some(c) = self.tone() else {
            return;
        };
        let w = -TAU * c / 8000.0;
        for s in x {
            let (sn, cs) = self.phase.sin_cos();
            self.acc += s * Complex32::new(cs as f32, sn as f32);
            self.phase = (self.phase + w).rem_euclid(TAU);
            self.acc_n += 1;
            if self.acc_n == GROUP_N {
                self.narrow.push(self.acc / GROUP_N as f32);
                self.acc = Complex32::default();
                self.acc_n = 0;
                self.since_read += 1;
            }
        }
        let keep = (KEEP_S * NARROW_RATE) as usize;
        if self.narrow.len() > keep {
            let d = self.narrow.len() - keep;
            self.narrow.drain(..d);
        }
    }

    /// The element length: set by hand, found, or 3 s before one is known.
    fn element_now(&self) -> f64 {
        if self.element_set > 0.0 {
            self.element_set
        } else {
            self.element.unwrap_or(3.0)
        }
    }

    fn read_text(&mut self, out: &mut Outbox) {
        let keying = self.submode.keying();
        let len = self.narrow.len();
        if self.element_set <= 0.0 {
            let grown = len.saturating_sub(self.searched_at) as f64 / NARROW_RATE;
            let need = (30.0f64).max(0.25 * self.searched_at as f64 / NARROW_RATE);
            if self.element.is_none() || grown >= need {
                self.searched_at = len;
                if let Some(t) = find_element(&self.narrow, self.span, keying) {
                    if self.element.is_none() {
                        self.logs.push(format!("elements of {t:.1} s"));
                    }
                    self.element = Some(t);
                }
            }
        }
        let t = self.element_now();
        let r = read(&self.narrow, t, self.span, keying, self.dash_below);
        if self.element_set <= 0.0 {
            if let Some(r) = &r {
                if self.element.is_some() {
                    self.element = Some(r.element_s);
                }
            }
        }
        let changed =
            r.as_ref().map(|r| r.text.as_str()) != self.reading.as_ref().map(|r| r.text.as_str());
        if changed {
            if let Some(r) = &r {
                let time = self.n as f64 / 8000.0;
                let c = self.tone().unwrap_or(0.0);
                out.event(QRSS_TEXT.event(
                    time,
                    self.dial + c,
                    &QrssTextEvent {
                        text: r.text.as_str(),
                        element_s: r.element_s,
                        centre_hz: Hertz(c),
                        family: Family::Message,
                    },
                ));
            }
        }
        if r.is_some() || self.reading.is_none() {
            self.reading = r;
        }
    }

    /// Sends the newest rows of the spectrogram as the picture, in the state `state`. The picture is one strip that
    /// grows, with one number.
    fn picture(&mut self, out: &mut Outbox, state: sdrkit_graph::PictureState) {
        let t = self.element_now();
        let w = ((0.5 * t * NARROW_RATE).round() as usize).max(32);
        // Enough of the newest stream for the rows kept.
        let need = (ROWS_KEPT as f64 * w as f64 / 4.0) as usize + w;
        let z = &self.narrow[self.narrow.len().saturating_sub(need)..];
        let sg = Spectrogram::new(z, t, self.span);
        if sg.rows.is_empty() {
            return;
        }
        let rows = &sg.rows[sg.rows.len().saturating_sub(ROWS_KEPT)..];
        let width = rows[0].len() as u32;
        self.picture_id = 1;
        let mut p = HfPicture::new(self.picture_id, "QRSS", width, rows.len() as u32, 0.0);
        for (y, r) in rows.iter().enumerate() {
            for (x, &v) in r.iter().enumerate() {
                let db = 10.0 * (v / sg.noise).max(1e-6).log10();
                let g = ((db + 5.0) / 35.0 * 255.0).clamp(0.0, 255.0) as u8;
                p.put(x as u32, y as u32, g, g, g);
            }
        }
        p.rows = rows.len() as u32;
        self.rows_total = rows.len() as i64;
        out.picture(self.n as f64 / 8000.0, p.to_picture(state));
    }
}

impl sdrkit_graph::Decoder for QrssDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.qrss"
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        QrssSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match QrssSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            QrssSetting::Submode => {
                let s = SUBMODE.read(id, value)?;
                if s != self.submode {
                    self.submode = s;
                    self.element = None;
                    self.reading = None;
                }
            }
            QrssSetting::Auto => {
                let a = AUTO.read(id, value)?;
                if a && !self.auto {
                    self.manual = false;
                    self.restart();
                }
                self.auto = a;
            }
            QrssSetting::Centre => {
                let c = CENTRE.read(id, value)?;
                if c != self.centre {
                    self.centre = c;
                    self.manual = true;
                    self.restart();
                }
            }
            QrssSetting::Element => {
                let e = ELEMENT_S.read(id, value)?;
                if e > 0.0 && e < 0.5 {
                    return Err(SettingError::OutOfRange {
                        id,
                        min: 0.5,
                        max: 120.0,
                        got: e,
                    });
                }
                self.element_set = e;
            }
            QrssSetting::Span => self.span = SPAN_HZ.read(id, value)?,
            QrssSetting::DashBelow => self.dash_below = DASH_BELOW.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (QrssSetting::Submode, Value::Choice(self.submode.index())),
            (QrssSetting::Auto, Value::Bool(self.auto && !self.manual)),
            (QrssSetting::Centre, Value::Hz(self.centre)),
            (QrssSetting::Element, Value::F64(self.element_set)),
            (QrssSetting::Span, Value::Hz(self.span)),
            (QrssSetting::DashBelow, Value::Bool(self.dash_below)),
        ])
    }
    fn results(&self) -> Published {
        crate::typed::results("qrss")
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.dial = input.tuned_hz;
        self.restart();
        Ok(decoder::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.restart();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.n += x.len() as u64;
        if self.tone().is_none() {
            // Wait for a steady tone: keep 30 to 60 s, try every 5 s.
            self.wait.extend(x.iter().map(|s| {
                if s.re.is_finite() && s.im.is_finite() {
                    *s
                } else {
                    Complex32::default()
                }
            }));
            let keep = (60.0 * 8000.0) as usize;
            if self.wait.len() > keep {
                let d = self.wait.len() - keep;
                self.wait.drain(..d);
            }
            if self.n >= self.next_try {
                self.next_try = self.n + (5.0 * 8000.0) as u64;
                if let Some(c) = find_centre(&self.wait) {
                    self.found = Some(c);
                    self.logs.push(format!("tone at {c:.1} Hz"));
                    let kept = std::mem::take(&mut self.wait);
                    self.narrow_in(&kept);
                }
            }
            return;
        }
        self.narrow_in(x);
        // Every 2 s of signal: the picture and the text.
        if self.since_read >= (2.0 * NARROW_RATE) as usize {
            self.since_read = 0;
            self.picture(out, sdrkit_graph::PictureState::Growing);
            self.read_text(out);
        }
    }
    fn finish(&mut self, out: &mut Outbox) {
        if self.tone().is_some() && !self.narrow.is_empty() {
            // The end of the record: the element length is searched again over all of it.
            self.searched_at = 0;
            self.element = None;
            self.picture(out, sdrkit_graph::PictureState::Finished);
            self.read_text(out);
        }
    }
    fn report(&mut self, res: &mut Published) {
        let tone = self.tone();
        let st = &mut self.status;
        st.lock_state = if tone.is_some() {
            LockState::Locked
        } else {
            LockState::Searching
        };
        st.snr_db = self.reading.as_ref().map(|r| r.snr_db as f32);
        st.offset_hz = Some(0.0);
        let label = match self.submode {
            Submode::Qrss => "QRSS",
            Submode::Dfcw => "DFCW",
        };
        match (
            tone,
            self.element
                .or((self.element_set > 0.0).then_some(self.element_set)),
        ) {
            (None, _) => put_text(
                &mut st.summary,
                format_args!("{label}: looking for a steady tone"),
            ),
            (Some(c), None) => put_text(
                &mut st.summary,
                format_args!("{label}: tone {c:.1} Hz, reading the keying"),
            ),
            (Some(c), Some(t)) => put_text(
                &mut st.summary,
                format_args!("{label}: tone {c:.1} Hz, elements {t:.0} s"),
            ),
        }
        res.results_mut().set_status(st);
        let text = self.reading.as_ref().map_or("", |r| r.text.as_str());
        // The element length and the SNR are of the present tone.
        let reading = self.reading.as_ref().filter(|_| tone.is_some());
        res.put(&QrssFields {
            text,
            callsign: text
                .split_whitespace()
                .rev()
                .find(|w| sdrkit_util_text::callsign::is_callsign(w)),
            centre_hz: tone.map(Hertz),
            element_s: reading.map(|r| (r.element_s * 100.0).round() / 100.0),
            snr_db: reading.map(|r| (r.snr_db * 10.0).round() / 10.0),
            locked: tone.is_some(),
            picture_rows: self.rows_total,
            note: NOTE_TEXT,
        });
        flush_logs(&mut self.logs, res);
    }
    fn clear(&mut self) {
        // The record goes; the tone and the element length stay.
        self.narrow.clear();
        self.since_read = 0;
        self.searched_at = 0;
        self.reading = None;
        self.rows_total = 0;
    }
    fn forget(&mut self, kept: &Kept) {
        let reading = self.reading.take().filter(|_| kept.field("text"));
        self.restart();
        self.reading = reading;
        self.since_read = 0;
        self.rows_total = 0;
    }
}

/// The element length of the test signal (0: 3 s for QRSS, 6 s for DFCW).
pub const GEN_ELEMENT_S: Setting<field::F64> =
    Setting::new("gen_element_s", "Test signal element length", "s", 0.0)
        .range(0.0, 120.0, 0.0)
        .section(GEN_SECTION)
        .help("0: 3 s for QRSS, 6 s for DFCW.")
        .advanced();
/// The DFCW shift of the test signal.
pub const GEN_SHIFT_HZ: Setting<field::Hz> =
    Setting::new("gen_shift_hz", "Test signal DFCW shift", "Hz", 1.5)
        .range(-10.0, 10.0, 0.0)
        .section(GEN_SECTION)
        .help("The dash frequency above the dot frequency (negative: below).")
        .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of the QRSS stack.
    pub enum QrssGenSetting {
        /// The text.
        Message = decoder::GEN_MESSAGE,
        /// The element length.
        Element = GEN_ELEMENT_S,
        /// The DFCW shift.
        Shift = GEN_SHIFT_HZ,
    }
}

/// The key of each element of `text` sent in `keying`: `None` up, `Some(false)` the dot frequency (or the carrier),
/// `Some(true)` the dash frequency.
pub fn keying_of(text: &str, keying: Keying) -> Vec<Option<bool>> {
    let mut out = Vec::new();
    let (letter, word) = match keying {
        Keying::Qrss => (3, 7),
        Keying::Dfcw => (1, 3),
    };
    for (wi, w) in text.split_whitespace().enumerate() {
        if wi > 0 {
            out.extend(std::iter::repeat_n(None, word));
        }
        for (ci, s) in sdrkit_util_text::morse::symbols(w).enumerate() {
            let Ok(code) = sdrkit_util_text::morse::encode(s) else {
                continue;
            };
            if ci > 0 {
                out.extend(std::iter::repeat_n(None, letter));
            }
            for (ei, dash) in code.elements().enumerate() {
                match keying {
                    Keying::Qrss => {
                        if ei > 0 {
                            out.push(None);
                        }
                        out.extend(std::iter::repeat_n(Some(false), if dash { 3 } else { 1 }));
                    }
                    Keying::Dfcw => out.push(Some(dash)),
                }
            }
        }
    }
    out
}

/// The QRSS and DFCW transmitter: 5 s of silence, then the text with a word gap after it, repeated; a raised edge of a tenth of
/// an element.
pub struct QrssModulator {
    rate: f64,
    centre: f64,
    shift: f64,
    element: f64,
    keys: Vec<Option<bool>>,
    n: u64,
    phase: f64,
}

impl QrssModulator {
    /// Sends `text` in `keying` at `rate`, elements of `element` s, the tone at `centre`, DFCW dashes `shift` Hz above.
    pub fn new(
        rate: f64,
        keying: Keying,
        text: &str,
        centre: f64,
        element: f64,
        shift: f64,
    ) -> Self {
        let mut keys = keying_of(text, keying);
        keys.extend(std::iter::repeat_n(None, 7));
        QrssModulator {
            rate,
            centre,
            shift,
            element,
            keys,
            n: 0,
            phase: 0.0,
        }
    }
}

impl Modulator for QrssModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        let lead = 5.0;
        for o in out.iter_mut() {
            let t = self.n as f64 / self.rate - lead;
            self.n += 1;
            if t < 0.0 {
                *o = Complex32::default();
                continue;
            }
            let u = t / self.element;
            let k = u.floor() as usize % self.keys.len();
            let Some(dash) = self.keys[k] else {
                *o = Complex32::default();
                continue;
            };
            let f = self.centre + if dash { self.shift } else { 0.0 };
            self.phase = (self.phase + TAU * f / self.rate).rem_euclid(TAU);
            let edge = |i: usize| self.keys[i % self.keys.len()] != self.keys[k];
            let x = u - u.floor();
            let mut a = 1.0;
            if x < 0.1 && edge(k + self.keys.len() - 1) {
                a = 0.5 - 0.5 * (std::f64::consts::PI * x / 0.1).cos();
            } else if x > 0.9 && edge(k + 1) {
                a = 0.5 - 0.5 * (std::f64::consts::PI * (1.0 - x) / 0.1).cos();
            }
            *o = Complex32::new(self.phase.cos() as f32, self.phase.sin() as f32) * a as f32;
        }
    }
}

/// The test text.
pub const TEST_TEXT: &str = "CQ";
/// The tone of the test signal, Hz.
pub const GEN_CENTRE: f64 = 1000.0;

/// The generator: "CQ" (or `gen_message`) at 1000 Hz after 5 s of silence; QRSS with 3 s elements, or with the submode `dfcw`
/// DFCW with 6 s elements and dashes `gen_shift_hz` above.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let sub = match cfg.submode.as_deref() {
        Some("dfcw") => Submode::Dfcw,
        Some(_) | None => cfg.settings.get(&SUBMODE),
    };
    let e = cfg.settings.get(&GEN_ELEMENT_S);
    let element = if e > 0.0 {
        e
    } else if sub == Submode::Dfcw {
        6.0
    } else {
        3.0
    };
    let m = QrssModulator::new(
        cfg.rate_hz,
        sub.keying(),
        &decoder::message(cfg, TEST_TEXT),
        GEN_CENTRE,
        element,
        cfg.settings.get(&GEN_SHIFT_HZ),
    );
    let info = SignalInfo::new(2500.0)
        .with_symbol_rate(1.0 / element)
        .with_power(1.0);
    Ok(decoder::placed(cfg.rate_hz, m, info))
}

/// The receiver options of the stack, with a submode the host names written into the `submode` setting.
pub fn with_submode(cfg: &StackConfig) -> Result<StackConfig, ConfigError> {
    let mut cfg = cfg.clone();
    if let Some(sub) = cfg.submode.as_deref().and_then(Submode::from_key) {
        let schema = crate::qrss_schema_of();
        if let Ok(id) = schema.id(SUBMODE.field.key) {
            cfg.settings.set(id, &Value::Choice(sub.index()))?;
        }
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keying_of_cq() {
        let k = keying_of("CQ", Keying::Qrss);
        // C: -.-.  (3+1+1+1+3+1+1) = 11 units, a letter gap of 3, Q: --.- 13 units.
        assert_eq!(k.len(), 11 + 3 + 13);
        let d = keying_of("CQ", Keying::Dfcw);
        assert_eq!(
            d,
            vec![
                Some(true),
                Some(false),
                Some(true),
                Some(false),
                None,
                Some(true),
                Some(true),
                Some(false),
                Some(true)
            ]
        );
    }
}
