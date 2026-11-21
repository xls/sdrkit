//! FSK441: meteor-scatter pings of 4-FSK at 441 baud, three tones a character. A burst finder on the audio hands each burst to
//! a reader that finds the tone offset, the symbol phase and the character boundary.
//!
//! Standard: WSJT user guide, section FSK441; J. Taylor, "WSJT: New Software for VHF Meteor-Scatter Communication", QST,
//! December 2001.

use super::{median, Board, Kept, ScatterDecode, Shown, GEN_TEXT, TEST_TEXT};
use crate::front::AUDIO_RATE;
use crate::typed::{
    Fsk441DecodeEvent, Fsk441DecodeRow, Fsk441Fields, FSK441_DECODE, FSK441_DECODES,
};
use num_complex::Complex32;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published, TableDef};
use sdrkit_core::{
    ConfigError, Event, GenConfig, Generator, Hz, LockState, Schema, Setting, SettingError,
    SettingId, SettingSet, SignalInfo, Value, Values,
};
use sdrkit_graph::{values_of, Format, Outbox, Port, RunContext};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::sync::Arc;

/// Symbols a second.
pub const BAUD: f64 = 441.0;
/// The tone spacing, Hz.
pub const SPACING_HZ: f64 = 441.0;

/// The audio frequency of tone `k` (0 to 3): (k + 2) × 441 Hz.
pub fn tone_hz(k: u8) -> f64 {
    (f64::from(k) + 2.0) * SPACING_HZ
}

/// The character of each code 0 to 47 (code = 16·t1 + 4·t2 + t3). Codes 0, 15, 21 and 42 read as a space.
pub const CHARS: [char; 48] = [
    ' ', '1', '2', '3', '4', '5', '6', '7', '8', '9', '.', ',', '?', '/', '#', ' ', '$', 'A', 'B',
    'C', 'D', ' ', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U',
    'V', 'W', 'X', 'Y', ' ', '0', 'E', 'Z', '*', '!',
];

/// The code sent for a space: tones 0, 3, 3.
pub const SPACE_CODE: u8 = 15;

/// The code sent for `c`: its code in [`CHARS`] (lower case folded), [`SPACE_CODE`] for a space and for a character the table
/// does not have.
pub fn code_of(c: char) -> u8 {
    let c = c.to_ascii_uppercase();
    if c == ' ' {
        return SPACE_CODE;
    }
    CHARS
        .iter()
        .position(|&x| x == c)
        .map_or(SPACE_CODE, |p| p as u8)
}

/// The tones of `text`, three a character, first tone first.
pub fn tones(text: &str) -> Vec<u8> {
    text.chars()
        .map(code_of)
        .flat_map(|c| [c >> 4, (c >> 2) & 3, c & 3])
        .collect()
}

/// The real audio of `text` sent once at `rate`, the tones `df_hz` from their nominal frequencies, amplitude `amp`, phase
/// continuous.
pub fn audio(text: &str, rate: f64, df_hz: f64, amp: f32) -> Vec<f32> {
    let t = tones(text);
    let ns = rate / BAUD;
    let n = (t.len() as f64 * ns).round() as usize;
    let mut phase = 0.0f64;
    (0..n)
        .map(|i| {
            let k = ((i as f64 / ns) as usize).min(t.len() - 1);
            phase = (phase + TAU * (tone_hz(t[k]) + df_hz) / rate).rem_euclid(TAU);
            amp * phase.cos() as f32
        })
        .collect()
}

/// What the reader found in a burst.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    /// The text, single spaces, none at the ends.
    pub text: String,
    /// The tones' offset from 882 to 2205 Hz, Hz.
    pub df_hz: f64,
    /// Strongest tone over the mean of the other three, summed over the symbols, dB.
    pub contrast_db: f64,
    /// Symbols read.
    pub symbols: usize,
}

/// The four tones mixed down and summed from the start of a piece of audio, so the sum over any span is one subtraction.
struct Sums {
    acc: [Vec<(f64, f64)>; 4],
}

impl Sums {
    fn new(x: &[f32], rate: f64, df: f64) -> Sums {
        let mut acc: [Vec<(f64, f64)>; 4] = Default::default();
        for (m, a) in acc.iter_mut().enumerate() {
            let w = TAU * (tone_hz(m as u8) + df) / rate;
            let (sw, cw) = w.sin_cos();
            let (mut c, mut s) = (1.0f64, 0.0f64);
            let (mut re, mut im) = (0.0, 0.0);
            a.reserve(x.len() + 1);
            a.push((0.0, 0.0));
            for &v in x {
                re += f64::from(v) * c;
                im -= f64::from(v) * s;
                a.push((re, im));
                let nc = c * cw - s * sw;
                s = s * cw + c * sw;
                c = nc;
            }
        }
        Sums { acc }
    }
    fn len(&self) -> usize {
        self.acc[0].len() - 1
    }
    /// The energy of tone `m` over samples `a..b`.
    fn energy(&self, m: usize, a: usize, b: usize) -> f64 {
        let (p, q) = (self.acc[m][a], self.acc[m][b]);
        let (re, im) = (q.0 - p.0, q.1 - p.1);
        re * re + im * im
    }
    /// The tone energies of every whole symbol from sample `phase` on, `ns` samples a symbol.
    fn symbols(&self, phase: f64, ns: f64, out: &mut Vec<[f64; 4]>) {
        out.clear();
        let mut k = 0usize;
        loop {
            let a = (phase + k as f64 * ns).round() as usize;
            let b = (phase + (k + 1) as f64 * ns).round() as usize;
            if b > self.len() {
                break;
            }
            out.push(std::array::from_fn(|m| self.energy(m, a, b)));
            k += 1;
        }
    }
    /// The sum over the symbols of the strongest tone.
    fn metric(&self, phase: f64, ns: f64, scratch: &mut Vec<[f64; 4]>) -> f64 {
        self.symbols(phase, ns, scratch);
        scratch
            .iter()
            .map(|e| e.iter().copied().fold(0.0, f64::max))
            .sum()
    }
}

/// Reads one burst (or a piece of a continuous signal) of audio at `rate`: the offset of the tones within ±`tol_hz`, the
/// symbol phase, the character boundary and the text. `None` when the audio does not hold a readable FSK441 signal.
pub fn read(x: &[f32], rate: f64, tol_hz: f64) -> Option<Reading> {
    let ns = rate / BAUD;
    if (x.len() as f64) < 12.0 * ns {
        return None;
    }
    // The 1.2 s with the most energy, on a 0.05 s grid.
    let part_len = (1.2 * rate) as usize;
    let s0 = if x.len() > part_len {
        let step = ((0.05 * rate) as usize).max(1);
        let mut cum = Vec::with_capacity(x.len() + 1);
        cum.push(0.0f64);
        for &v in x {
            cum.push(cum[cum.len() - 1] + f64::from(v) * f64::from(v));
        }
        let mut best = (f64::MIN, 0usize);
        let mut s = 0;
        while s + part_len <= x.len() {
            let e = cum[s + part_len] - cum[s];
            if e > best.0 {
                best = (e, s);
            }
            s += step;
        }
        best.1
    } else {
        0
    };
    let part = &x[s0..(s0 + part_len).min(x.len())];
    let mut scratch = Vec::new();
    let mut best = (f64::MIN, 0.0, 0.0);
    let mut df = -tol_hz;
    while df <= tol_hz + 1e-9 {
        let s = Sums::new(part, rate, df);
        for q in 0..4 {
            let ph = f64::from(q) * ns / 4.0;
            let m = s.metric(ph, ns, &mut scratch);
            if m > best.0 {
                best = (m, df, ph);
            }
        }
        df += 20.0;
    }
    let (coarse_df, coarse_ph) = (best.1, best.2);
    for j in -3..=3 {
        let df = coarse_df + 5.0 * f64::from(j);
        let s = Sums::new(part, rate, df);
        for k in -3..=4 {
            let ph = (coarse_ph + f64::from(k) * ns / 16.0).rem_euclid(ns);
            let m = s.metric(ph, ns, &mut scratch);
            if m > best.0 {
                best = (m, df, ph);
            }
        }
    }
    let (df, ph) = (best.1, (s0 as f64 + best.2).rem_euclid(ns));
    let s = Sums::new(x, rate, df);
    let mut e = Vec::new();
    s.symbols(ph, ns, &mut e);
    // Trim the quiet ends of a strong ping.
    let totals: Vec<f64> = e.iter().map(|v| v.iter().sum()).collect();
    let peak = totals.iter().copied().fold(0.0, f64::max);
    let mut sorted: Vec<f32> = totals.iter().map(|&t| t as f32).collect();
    let med = f64::from(median(&mut sorted));
    let (mut lo, mut hi) = (0usize, e.len());
    // Quiet ends (noise or silence before and after a burst that fills most of the block) are never read: a symbol at either
    // end below a tenth of the 75th percentile of the totals goes.
    let mut by = totals.clone();
    by.sort_by(f64::total_cmp);
    let upper = by[(by.len() * 3) / 4];
    let quiet = (0.1 * upper).max(1e-4 * peak);
    while lo < hi && totals[lo] < quiet {
        lo += 1;
    }
    while hi > lo && totals[hi - 1] < quiet {
        hi -= 1;
    }
    if peak > 20.0 * med {
        while lo < hi && totals[lo] < 0.3 * peak {
            lo += 1;
        }
        while hi > lo && totals[hi - 1] < 0.3 * peak {
            hi -= 1;
        }
    }
    let e = &e[lo..hi];
    if e.len() < 12 {
        return None;
    }
    let tones: Vec<u8> = e
        .iter()
        .map(|v| {
            (1..4).fold(0u8, |best, m| {
                if v[m] > v[usize::from(best)] {
                    m as u8
                } else {
                    best
                }
            })
        })
        .collect();
    let mut n3 = [0f64; 3];
    for (i, &t) in tones.iter().enumerate() {
        if t == 3 {
            n3[i % 3] += 1.0;
        }
    }
    let mean = n3.iter().sum::<f64>() / 3.0;
    let least = n3.iter().copied().fold(f64::MAX, f64::min);
    let (mut strong, mut rest) = (0.0, 0.0);
    for v in e {
        let m = v.iter().copied().fold(0.0, f64::max);
        strong += m;
        rest += (v.iter().sum::<f64>() - m) / 3.0;
    }
    let contrast_db = 10.0 * (strong / rest.max(1e-30)).log10();
    if mean < 2.0 || least > 0.4 * mean || contrast_db < 4.5 {
        return None;
    }
    let boundary = (0..3).find(|&p| n3[p] == least).unwrap_or(0);
    let (mut chars, mut bad, mut n) = (String::new(), 0usize, 0usize);
    let mut i = boundary;
    while i + 2 < tones.len() {
        let c = 16 * tones[i] + 4 * tones[i + 1] + tones[i + 2];
        n += 1;
        match CHARS.get(usize::from(c)) {
            Some(&ch) => chars.push(ch),
            None => {
                chars.push(' ');
                bad += 1;
            }
        }
        i += 3;
    }
    if n < 4 || bad * 4 > n {
        return None;
    }
    let text = chars.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() < 2 {
        return None;
    }
    Some(Reading {
        text,
        df_hz: df,
        contrast_db,
        symbols: e.len(),
    })
}

/// The frequency tolerance of the reader.
pub const TOL_HZ: Setting<field::Hz> = Setting::new("tol_hz", "Frequency tolerance", "Hz", 300.0)
    .range(50.0, 500.0, 0.0)
    .help("How far the tones may lie from 882 to 2205 Hz.");
/// The threshold of the burst finder.
pub const BURST_DB: Setting<field::F64> = Setting::new("burst_db", "Burst threshold", "dB", 6.0)
    .range(3.0, 15.0, 0.0)
    .help("A frame of audio this far above the noise starts a burst.");

sdrkit_core::settings! {
    /// The settings of [`Fsk441Decoder`].
    pub enum Fsk441Setting {
        /// The frequency tolerance.
        Tol = TOL_HZ,
        /// The burst threshold.
        Burst = BURST_DB,
    }
}

/// The values that a decode of FSK441 adds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fsk441Extra {
    /// The contrast, dB (not a calibrated SNR).
    pub contrast_db: f64,
    /// The length of the burst, ms.
    pub duration_ms: f64,
}

impl Shown for Fsk441Extra {
    fn table() -> &'static TableDef {
        &FSK441_DECODES
    }
    fn row(d: &ScatterDecode<Self>, utc: f64, key: &str, p: &mut Published) {
        FSK441_DECODES.put(
            p,
            key,
            &Fsk441DecodeRow {
                utc: Time(utc),
                time_s: d.time_s,
                snr_db: d.snr_db,
                audio_hz: Hertz(d.hz),
                mode: &d.label,
                message: &d.text,
                contrast_db: d.extra.contrast_db,
                duration_ms: d.extra.duration_ms,
            },
        );
    }
    fn event(d: &ScatterDecode<Self>, utc: f64, rf_hz: f64, callsign: Option<&str>) -> Event {
        FSK441_DECODE.event(
            utc,
            rf_hz,
            &Fsk441DecodeEvent {
                mode: &d.label,
                text: &d.text,
                snr_db: d.snr_db,
                audio_hz: Hertz(d.hz),
                callsign,
                source: callsign,
                family: Family::Message,
                contrast_db: d.extra.contrast_db,
                duration_ms: d.extra.duration_ms,
            },
        )
    }
}

/// What no public document confirms.
pub const NOTE_TEXT: &str =
    "Not confirmed by a public document: the rows of the character table, that codes 0, 21 and \
42 read as a space, and the two spaces the test signal sends between repeats.";

/// Frames that only fill the noise list at the start.
const WARMUP_FRAMES: u64 = 24;
/// Quiet frames kept for the noise level.
const NOISE_KEEP: usize = 600;
/// Frames dropped from the noise list when it is full.
const NOISE_DROP: usize = 100;
/// Frames that are not hot that end a burst.
const COLD_END: u32 = 4;
/// The longest piece of a burst read at once, seconds.
const MAX_BURST_S: f64 = 2.0;
/// Audio added on each side of a burst, seconds.
const PAD_S: f64 = 0.03;
/// The decay of the strongest frame power, per frame.
const PEAK_DECAY: f32 = 0.999;
/// Audio kept, seconds.
const KEEP_S: f64 = 4.0;

/// The FSK441 receiver: 8 kHz audio in, a decode row and event for each burst read.
pub struct Fsk441Decoder {
    tol: f64,
    burst_db: f64,
    board: Board<Fsk441Extra>,
    kept: Kept,
    frame_len: usize,
    fft: Fft,
    win: Vec<f32>,
    buf: Vec<Complex32>,
    band: (usize, usize),
    next_frame: u64,
    frames: u64,
    noise: VecDeque<f32>,
    scratch: Vec<f32>,
    level_db: f64,
    peak: f32,
    open: Option<u64>,
    last_hot_end: u64,
    cold: u32,
    pending: Vec<(u64, u64)>,
    bursts: i64,
    decoded: bool,
}

impl Default for Fsk441Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Fsk441Decoder {
    /// A receiver at the default settings.
    pub fn new() -> Self {
        let frame_len = ((0.02133 * AUDIO_RATE).round() as usize).max(64);
        let n = super::next_pow2(frame_len);
        let bin = AUDIO_RATE / n as f64;
        Fsk441Decoder {
            tol: TOL_HZ.default,
            burst_db: BURST_DB.default,
            board: Board::new(),
            kept: Kept::default(),
            frame_len,
            fft: Fft::forward(n),
            win: (0..frame_len)
                .map(|i| (0.5 - 0.5 * (TAU * i as f64 / frame_len as f64).cos()) as f32)
                .collect(),
            buf: vec![Complex32::default(); n],
            band: (
                (700.0 / bin).ceil() as usize,
                (2500.0 / bin).floor() as usize,
            ),
            next_frame: 0,
            frames: 0,
            noise: VecDeque::new(),
            scratch: Vec::new(),
            level_db: 0.0,
            peak: 0.0,
            open: None,
            last_hot_end: 0,
            cold: 0,
            pending: Vec::new(),
            bursts: 0,
            decoded: false,
        }
    }

    fn reset(&mut self) {
        let at = self.kept.end();
        self.kept.reset(at);
        self.next_frame = at;
        self.frames = 0;
        self.noise.clear();
        self.open = None;
        self.cold = 0;
        self.pending.clear();
    }

    fn frame_power(&mut self, a: u64) -> f32 {
        let x = self.kept.span(a, a + self.frame_len as u64);
        for (i, b) in self.buf.iter_mut().enumerate() {
            *b = match x.get(i) {
                Some(&v) => Complex32::new(v * self.win[i], 0.0),
                None => Complex32::default(),
            };
        }
        self.fft.process(&mut self.buf);
        self.buf[self.band.0..=self.band.1]
            .iter()
            .map(Complex32::norm_sqr)
            .sum()
    }

    fn close(&mut self, end: u64) {
        if let Some(start) = self.open.take() {
            let pad = (PAD_S * AUDIO_RATE) as u64;
            self.pending.push((start.saturating_sub(pad), end + pad));
            self.bursts += 1;
        }
    }

    fn frames(&mut self) {
        let fl = self.frame_len as u64;
        while self.next_frame + fl <= self.kept.end() {
            let a = self.next_frame;
            self.next_frame += fl;
            let p = self.frame_power(a);
            self.frames += 1;
            self.peak = (self.peak * PEAK_DECAY).max(p);
            if self.frames <= WARMUP_FRAMES {
                self.noise.push_back(p);
                continue;
            }
            self.scratch.clear();
            self.scratch.extend(self.noise.iter().copied());
            // A signal with no noise at all (a clean test signal) still ends its bursts: the floor stays 50 dB below the
            // strongest recent frame.
            let noise = median(&mut self.scratch).max(self.peak * 1e-5).max(1e-30);
            self.level_db = 10.0 * (f64::from(p) / f64::from(noise)).max(1e-12).log10();
            let hot = f64::from(p) > f64::from(noise) * 10f64.powf(self.burst_db / 10.0);
            if hot {
                self.cold = 0;
                self.last_hot_end = a + fl;
                if self.open.is_none() {
                    self.open = Some(a);
                }
            } else {
                if self.noise.len() >= NOISE_KEEP {
                    self.noise.drain(..NOISE_DROP);
                }
                self.noise.push_back(p);
                if self.open.is_some() {
                    self.cold += 1;
                    if self.cold >= COLD_END {
                        let end = self.last_hot_end;
                        self.close(end);
                    }
                }
            }
            if let Some(start) = self.open {
                if (a + fl - start) as f64 >= MAX_BURST_S * AUDIO_RATE {
                    self.close(a + fl);
                    self.open = Some(a + fl);
                }
            }
        }
    }

    fn read_pending(&mut self, out: &mut Outbox, all: bool) {
        let mut i = 0;
        while i < self.pending.len() {
            let (a, b) = self.pending[i];
            if b > self.kept.end() && !all {
                i += 1;
                continue;
            }
            self.pending.remove(i);
            let x = self.kept.span(a, b);
            if let Some(r) = read(x, AUDIO_RATE, self.tol) {
                let start = a.max(self.kept.first());
                let dur_ms = r.symbols as f64 / BAUD * 1000.0;
                let ratio = 10f64.powf(r.contrast_db / 10.0);
                self.decoded = true;
                self.board.decode(
                    out,
                    ScatterDecode {
                        time_s: start as f64 / AUDIO_RATE,
                        label: "FSK441".into(),
                        text: r.text,
                        hz: r.df_hz,
                        snr_db: super::snr_2500(ratio, SPACING_HZ).round(),
                        extra: Fsk441Extra {
                            contrast_db: (r.contrast_db * 10.0).round() / 10.0,
                            duration_ms: dur_ms.round(),
                        },
                    },
                );
            }
        }
    }
}

impl sdrkit_graph::Decoder for Fsk441Decoder {
    type Input = f32;
    fn name(&self) -> &'static str {
        "wsjt.fsk441"
    }
    fn outputs(&self) -> Vec<Port> {
        super::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        Fsk441Setting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match Fsk441Setting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Fsk441Setting::Tol => self.tol = TOL_HZ.read(id, value)?,
            Fsk441Setting::Burst => self.burst_db = BURST_DB.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (Fsk441Setting::Tol, Value::Hz(self.tol)),
            (Fsk441Setting::Burst, Value::F64(self.burst_db)),
        ])
    }
    fn results(&self) -> Published {
        crate::typed::results("fsk441")
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.board.attach(&ctx.clock);
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        super::check_rate(input)?;
        self.board.configure(input);
        self.reset();
        Ok(super::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.board.configure(input);
        self.reset();
    }
    fn process(&mut self, x: &[f32], _first: u64, out: &mut Outbox) {
        self.board.saw(self.kept.end() as f64 / AUDIO_RATE);
        self.kept.push(x);
        self.frames();
        self.read_pending(out, false);
        let keep_from = self.kept.end().saturating_sub((KEEP_S * AUDIO_RATE) as u64);
        let keep_from = self
            .open
            .map_or(keep_from, |s| keep_from.min(s.saturating_sub(800)));
        let keep_from = self.pending.iter().fold(keep_from, |k, &(a, _)| k.min(a));
        self.kept.drop_before(keep_from.min(self.next_frame));
    }
    fn finish(&mut self, out: &mut Outbox) {
        let end = self.last_hot_end.max(self.open.unwrap_or(0));
        self.close(end);
        self.read_pending(out, true);
    }
    fn report(&mut self, res: &mut Published) {
        let state = if self.decoded {
            LockState::Locked
        } else if self.level_db > 3.0 {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        let total = self.board.total();
        self.board.set_state(
            state,
            format_args!("FSK441: {} bursts, {} decodes", self.bursts, total),
        );
        self.board.status.snr_db = self.board.rows().last().map(|d| d.snr_db as f32);
        res.put(&Fsk441Fields {
            burst_level_db: (self.level_db * 10.0).round() / 10.0,
            bursts: self.bursts,
        });
        self.board.report(res, NOTE_TEXT);
    }
    fn clear(&mut self) {
        self.board.clear();
        self.bursts = 0;
        self.decoded = false;
    }
    /// The bursts are of the frequency before. The decodes go unless the stack keeps them.
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        if !kept.table(FSK441_DECODES.key) {
            self.board.clear();
        }
        self.bursts = 0;
        self.decoded = false;
    }
}

/// Starts of the test bursts in the 24 s loop, seconds.
pub const GEN_STARTS_S: [f64; 4] = [2.0, 8.5, 14.0, 19.5];
/// The longest test burst, seconds.
pub const GEN_BURST_S: f64 = 0.5;
/// The loop of the test signal, seconds.
pub const GEN_LOOP_S: f64 = 24.0;

sdrkit_core::settings! {
    /// The test-signal settings of the FSK441 stack.
    pub enum Fsk441GenSetting {
        /// The text sent.
        Text = GEN_TEXT,
    }
}

/// The test signal: in a loop of 24 s, bursts at 2.0, 8.5, 14.0 and 19.5 s of the text sent 1, 2, 3 and 1 times (each burst at
/// most 0.5 s), complex with the dial where [`super::dial_in_samples`] puts it and the tones above it, at unit power while
/// sending.
pub struct Fsk441Generator {
    rate: f64,
    dial: f64,
    bursts: Vec<(usize, Vec<u8>)>,
    n: usize,
    loop_len: usize,
    phase: f64,
}

impl Fsk441Generator {
    /// The test signal of `text` at `rate`.
    pub fn new(rate: f64, text: &str) -> Self {
        let unit = format!("{}  ", text.trim());
        let max_sym = (GEN_BURST_S * BAUD) as usize;
        let bursts = GEN_STARTS_S
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                let mut t = tones(&unit.repeat(i % 3 + 1));
                t.truncate(max_sym);
                ((s * rate) as usize, t)
            })
            .collect();
        Fsk441Generator {
            rate,
            dial: super::dial_in_samples(rate),
            bursts,
            n: 0,
            loop_len: (GEN_LOOP_S * rate) as usize,
            phase: 0.0,
        }
    }

    fn tone_at(&self, i: usize) -> Option<u8> {
        let ns = self.rate / BAUD;
        self.bursts.iter().find_map(|(s, t)| {
            let k = ((i.checked_sub(*s)?) as f64 / ns) as usize;
            t.get(k).copied()
        })
    }
}

impl Generator for Fsk441Generator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(2500.0)
                .with_symbol_rate(BAUD)
                .with_power(1.0),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        self.dial
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            *o = match self.tone_at(self.n % self.loop_len) {
                Some(t) => {
                    self.phase =
                        (self.phase + TAU * (self.dial + tone_hz(t)) / self.rate).rem_euclid(TAU);
                    let (s, c) = self.phase.sin_cos();
                    Complex32::new(c as f32, s as f32)
                }
                None => Complex32::default(),
            };
            self.n += 1;
        }
        out.len()
    }
}

/// The generator of the stack: [`TEST_TEXT`] or `gen_text`.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let t = cfg.settings.get(&GEN_TEXT);
    let text = if t.trim().is_empty() { TEST_TEXT } else { t };
    Ok(Box::new(Fsk441Generator::new(cfg.rate_hz, text)))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    static CELL: std::sync::OnceLock<Arc<Schema>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| Arc::new(Schema::of(&settings())))
        .clone()
}

/// The settings of the stack, in the order of its schema.
pub fn settings() -> Vec<&'static dyn sdrkit_core::settings::DeclareSetting> {
    super::settings(Fsk441Setting::SETTINGS, Fsk441GenSetting::SETTINGS)
}

/// The receiver of the stack.
pub fn make_receiver(
    cfg: &sdrkit_core::StackConfig,
) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    super::receiver("fsk441", Fsk441Decoder::new(), cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_codes_of_k1jt() {
        let t = tones("K1JT ");
        assert_eq!(t, vec![1, 2, 3, 0, 0, 1, 1, 2, 2, 2, 1, 0, 0, 3, 3]);
        assert_eq!(
            [
                code_of('K'),
                code_of('1'),
                code_of('J'),
                code_of('T'),
                code_of(' ')
            ],
            [27, 1, 26, 36, 15]
        );
        assert_eq!(
            [tone_hz(1), tone_hz(2), tone_hz(3)],
            [1323.0, 1764.0, 2205.0]
        );
        assert_eq!(code_of('k'), 27);
        assert_eq!(code_of('%'), SPACE_CODE);
        // No character starts with tone 3.
        assert!(CHARS.iter().enumerate().all(|(c, _)| c >> 4 < 3));
    }
}
