//! CP16: 16 carriers keyed by 16 × 16 dot characters, one dot row per time step, top row first, then a blank row. The receiver
//! finds the comb of carriers and the row clock, draws the rows as a picture and reads the characters against the glyphs of a
//! font. The transmitter sends a text on either sideband.
//!
//! Standard: Chinese Radio Amateurs Club, the design and field-test report of CP16 and the club's article on its transmission
//! (16 carriers, spacing, keying, scan order, 17 rows a character).

use crate::decoder::{self, flush_logs, put_text, Emit, Heard, GEN_SECTION};
use crate::picture::HfPicture;
use crate::typed::{
    Cp16CharEvent, Cp16Fields, Cp16SendArgs, Cp16TxEvent, CP16_CHAR, CP16_SEND, CP16_STOP, CP16_TX,
};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Complex32, ConfigError, GenConfig, Generator, Hz, LockState, Record, Schema, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, StageError, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kept, Kind, Outbox, Port, RateFrom,
    RunContext, TagKind,
};
use sdrkit_proto_glyph::cell::{Cell, Cells};
use std::f64::consts::{PI, TAU};
use std::sync::Arc;

/// Carriers.
pub const CARRIERS: usize = 16;
/// Rows of a character: 16 dot rows and a blank row.
pub const ROWS_PER_CELL: usize = 17;
/// The level of each carrier.
pub const CARRIER_AMP: f32 = 0.055;
/// The rate the carriers are made at, Hz.
pub const SYNTH_RATE: f64 = 16_000.0;
/// The rate of the receiver, Hz.
pub const RATE: f64 = 8000.0;

/// The cells of a text: tab, CR and LF become a blank cell, other control characters are dropped, ASCII 21 to 7E becomes its
/// full-width form U+FF01 to U+FF5E, a space becomes the blank cell U+3000.
pub fn cells(text: &str) -> Vec<char> {
    text.chars()
        .filter_map(|c| match c {
            '\t' | '\r' | '\n' | ' ' => Some('\u{3000}'),
            c if c.is_control() => None,
            '!'..='~' => char::from_u32(u32::from(c) - 0x21 + 0xFF01),
            c => Some(c),
        })
        .collect()
}

/// The character shown for a cell: a full-width form as its ASCII character.
pub fn shown(c: char) -> char {
    match u32::from(c) {
        0xFF01..=0xFF5E => char::from_u32(u32::from(c) - 0xFF01 + 0x21).unwrap_or(c),
        0x3000 => ' ',
        _ => c,
    }
}

/// The dot rows of a list of cells: each cell's 16 rows, then a blank row.
pub fn rows_of(cells: &[char], src: &Cells) -> Vec<u16> {
    let mut out = Vec::with_capacity(cells.len() * ROWS_PER_CELL);
    for &c in cells {
        out.extend_from_slice(&src.cell(c).0);
        out.push(0);
    }
    out
}

/// The parameters of a transmission.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TxParams {
    /// Seconds a character (17 rows).
    pub char_s: f64,
    /// Carrier spacing, Hz.
    pub spacing_hz: f64,
    /// The lowest carrier, Hz from the dial.
    pub base_hz: f64,
    /// Lower sideband: carrier i at −(base + i × spacing).
    pub lower: bool,
}

impl TxParams {
    /// The time of a row, seconds.
    pub fn row_s(&self) -> f64 {
        self.char_s / ROWS_PER_CELL as f64
    }
}

/// Why a transmission is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The spacing is outside 5 to 30 Hz.
    Spacing,
    /// A row is shorter than 25 ms or longer than 500 ms.
    Row,
    /// A carrier lies outside 100 to 3900 Hz.
    Band,
    /// The text has no cell.
    Empty,
    /// A transmission is running.
    Busy,
    /// The transmitter is off.
    Off,
    /// The output rate is below 16 kHz.
    Rate,
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TxError::Spacing => "the spacing must be 5 to 30 Hz",
            TxError::Row => "a row must last 25 to 500 ms",
            TxError::Band => "every carrier must lie within 100 to 3900 Hz",
            TxError::Empty => "the text has no character to send",
            TxError::Busy => "a transmission is running",
            TxError::Off => "the transmitter is off",
            TxError::Rate => "the transmit rate must be at least 16 kHz",
        })
    }
}

impl std::error::Error for TxError {}

/// Checks the parameters and the text of a transmission.
///
/// # Errors
///
/// The [`TxError`] of the first check that fails.
pub fn validate(p: &TxParams, text: &str) -> Result<(), TxError> {
    if !(5.0..=30.0).contains(&p.spacing_hz) {
        return Err(TxError::Spacing);
    }
    let row = p.row_s();
    if !(0.025..=0.5).contains(&row) {
        return Err(TxError::Row);
    }
    if p.base_hz < 100.0 || p.base_hz + 15.0 * p.spacing_hz > 3900.0 {
        return Err(TxError::Band);
    }
    if cells(text).is_empty() {
        return Err(TxError::Empty);
    }
    Ok(())
}

/// The complex baseband of a transmission at 16 kHz: `lead_s` of silence, then the rows; each carrier at its own random phase,
/// its phase running on, keyed on and off with raised-cosine edges of min(6 % of a row, 8 ms).
pub fn synth(rows: &[u16], p: &TxParams, lead_s: f64, seed: u64) -> Vec<Complex32> {
    let row_n = p.row_s() * SYNTH_RATE;
    let edge = ((p.row_s() * 0.06).min(0.008) * SYNTH_RATE)
        .round()
        .max(1.0);
    let lead = (lead_s * SYNTH_RATE) as usize;
    let n = lead + (rows.len() as f64 * row_n).round() as usize;
    let mut out = vec![Complex32::default(); n];
    let mut rng = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    for i in 0..CARRIERS {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        let mut phase = (rng >> 11) as f64 / (1u64 << 53) as f64 * TAU;
        let f = p.base_hz + i as f64 * p.spacing_hz;
        let w = TAU * if p.lower { -f } else { f } / SYNTH_RATE;
        let mut prev = 0.0f64;
        for (r, &bits) in rows.iter().enumerate() {
            let on = if (bits >> (15 - i)) & 1 == 1 {
                1.0
            } else {
                0.0
            };
            let a = lead + (r as f64 * row_n).round() as usize;
            let b = (lead + ((r + 1) as f64 * row_n).round() as usize).min(n);
            for (k, o) in out[a..b].iter_mut().enumerate() {
                let env = if (k as f64) < edge && on != prev {
                    prev + (on - prev) * 0.5 * (1.0 - (PI * k as f64 / edge).cos())
                } else {
                    on
                };
                if env > 0.0 {
                    *o += Complex32::new(phase.cos() as f32, phase.sin() as f32)
                        * (CARRIER_AMP * env as f32);
                }
                phase = (phase + w).rem_euclid(TAU);
            }
            prev = on;
        }
    }
    out
}

/// The silence before a transmission, seconds.
pub const LEAD_S: f64 = 0.5;

/// A transmission of `text` at 16 kHz: [`LEAD_S`] of silence and the text once.
///
/// # Errors
///
/// As [`validate`].
pub fn transmission16(
    text: &str,
    p: &TxParams,
    src: &Cells,
    seed: u64,
) -> Result<Vec<Complex32>, TxError> {
    validate(p, text)?;
    Ok(synth(&rows_of(&cells(text), src), p, LEAD_S, seed))
}

/// Samples made at 16 kHz, resampled to the output rate a piece at a time (so a long transmission at a high rate is never held
/// whole), once or again and again.
pub struct TxSource {
    x16: Vec<Complex32>,
    pos: usize,
    res: Option<Resampler<Complex32>>,
    buf: Vec<Complex32>,
    bpos: usize,
    looped: bool,
    produced: u64,
}

impl TxSource {
    /// The samples `x16` (at 16 kHz) at `rate`, repeated when `looped`.
    pub fn new(x16: Vec<Complex32>, rate: f64, looped: bool) -> TxSource {
        let res = ((rate - SYNTH_RATE).abs() > 1e-6)
            .then(|| Resampler::new(SYNTH_RATE, rate).ok())
            .flatten();
        TxSource {
            x16,
            pos: 0,
            res,
            buf: Vec::new(),
            bpos: 0,
            looped,
            produced: 0,
        }
    }
    /// Samples written so far.
    pub fn produced(&self) -> u64 {
        self.produced
    }
    /// Fills `out` and returns how many samples were written: fewer once a single run has ended.
    pub fn next_into(&mut self, out: &mut [Complex32]) -> usize {
        let mut w = 0;
        while w < out.len() {
            if self.bpos < self.buf.len() {
                let n = (self.buf.len() - self.bpos).min(out.len() - w);
                out[w..w + n].copy_from_slice(&self.buf[self.bpos..self.bpos + n]);
                self.bpos += n;
                w += n;
                continue;
            }
            self.buf.clear();
            self.bpos = 0;
            if self.pos >= self.x16.len() {
                if !self.looped || self.x16.is_empty() {
                    break;
                }
                self.pos = 0;
            }
            let end = (self.pos + 1600).min(self.x16.len());
            let piece = &self.x16[self.pos..end];
            match self.res.as_mut() {
                Some(r) => r.process(piece, &mut self.buf),
                None => self.buf.extend_from_slice(piece),
            }
            self.pos = end;
        }
        self.produced += w as u64;
        w
    }
}

/// A whole transmission of `text` at `rate` (for tests and short texts): [`LEAD_S`] of silence and the text once.
///
/// # Errors
///
/// As [`validate`].
pub fn transmission(
    text: &str,
    p: &TxParams,
    src: &Cells,
    rate: f64,
    seed: u64,
) -> Result<Vec<Complex32>, TxError> {
    let mut s = TxSource::new(transmission16(text, p, src, seed)?, rate, false);
    let mut out = Vec::new();
    let mut piece = vec![Complex32::default(); 8192];
    loop {
        let n = s.next_into(&mut piece);
        out.extend_from_slice(&piece[..n]);
        if n < piece.len() {
            return Ok(out);
        }
    }
}

/// The last output sample of a transmission stopped after the row under way: `produced` samples written at `rate`, rows of
/// `row_s`.
pub fn stop_after_row(produced: u64, rate: f64, row_s: f64) -> u64 {
    let lead = LEAD_S * rate;
    let row_n = (row_s * rate).max(1.0);
    let into = (produced as f64 - lead).max(0.0);
    (lead + (into / row_n).ceil() * row_n).round() as u64
}

/// The glyphs a reading is matched against.
pub struct Recognizer {
    chars: Vec<char>,
    bits: Vec<Cell>,
    centred: Vec<[f32; 256]>,
}

/// What 16 × 16 dots were recognised as.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Match {
    /// The character.
    pub c: char,
    /// The normalised correlation, −1 to 1.
    pub score: f32,
    /// The lead over the second best.
    pub margin: f32,
}

impl Recognizer {
    /// The glyphs of every GB 2312 character but U+3000 that `src` draws with at least 3 inked dots; of characters with the same
    /// glyph, the first in code order.
    pub fn new(src: &Cells) -> Recognizer {
        let mut r = Recognizer {
            chars: Vec::new(),
            bits: Vec::new(),
            centred: Vec::new(),
        };
        let mut seen = std::collections::HashSet::new();
        for (_, _, c) in sdrkit_proto_glyph::gb2312::chars() {
            if c == '\u{3000}' {
                continue;
            }
            let Some(g) = src.get(c) else {
                continue;
            };
            // Of two characters with one glyph, the first in code order is kept.
            if g.ink() < 3 || !seen.insert(g) {
                continue;
            }
            let mean = g.ink() as f32 / 256.0;
            let mut v = [0f32; 256];
            for (k, x) in v.iter_mut().enumerate() {
                *x = f32::from(u8::from(g.dot(k / 16, k % 16))) - mean;
            }
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
            for x in &mut v {
                *x /= norm;
            }
            r.chars.push(c);
            r.bits.push(g);
            r.centred.push(v);
        }
        r
    }
    /// Glyphs held.
    pub fn len(&self) -> usize {
        self.chars.len()
    }
    /// True when no glyph is held.
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }
    /// The best match of soft dots (16 rows of 16, 0 to 1): the 48 glyphs nearest by Hamming distance of the dots at 0.5, then
    /// the normalised correlation with each.
    pub fn recognise(&self, dots: &[[f32; 16]; 16]) -> Option<Match> {
        let mut hard = [0u16; 16];
        for (r, row) in dots.iter().enumerate() {
            for (c, &d) in row.iter().enumerate() {
                if d >= 0.5 {
                    hard[r] |= 1 << (15 - c);
                }
            }
        }
        let mut near: Vec<(u32, usize)> = self
            .bits
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let d: u32 =
                    g.0.iter()
                        .zip(&hard)
                        .map(|(a, b)| (a ^ b).count_ones())
                        .sum();
                (d, i)
            })
            .collect();
        let k = 48.min(near.len());
        if k == 0 {
            return None;
        }
        near.select_nth_unstable(k - 1);
        let mean = dots.iter().flatten().sum::<f32>() / 256.0;
        let mut v = [0f32; 256];
        for (k, x) in v.iter_mut().enumerate() {
            *x = dots[k / 16][k % 16] - mean;
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
        let (mut best, mut second) = ((f32::MIN, 0usize), f32::MIN);
        for &(_, i) in &near[..k] {
            let s = self.centred[i]
                .iter()
                .zip(&v)
                .map(|(a, b)| a * b)
                .sum::<f32>()
                / norm;
            if s > best.0 {
                second = best.0;
                best = (s, i);
            } else if s > second {
                second = s;
            }
        }
        Some(Match {
            c: self.chars[best.1],
            score: best.0,
            margin: best.0 - second.max(-1.0),
        })
    }
}

sdrkit_core::choice_enum! {
    /// The sideband setting.
    pub enum Sideband {
        /// Found from the signal (receive); upper for the transmitter.
        Auto = ("auto", "Automatic"),
        /// Upper sideband.
        Upper = ("upper", "Upper"),
        /// Lower sideband.
        Lower = ("lower", "Lower"),
    }
}

/// Fixed parameters instead of measured ones.
pub const FIXED: Setting<field::Bool> = Setting::new("fixed", "Fixed parameters", "", false)
    .help("On: receive with the four parameters below instead of measuring them. Off: they are the transmitter's.");
/// The lowest carrier.
pub const BASE_HZ: Setting<field::Hz> = Setting::new("base_hz", "Lowest carrier", "Hz", 1000.0)
    .range(100.0, 3900.0, 0.0)
    .help("Every carrier must lie below 3900 Hz.");
/// The spacing.
pub const SPACING_HZ: Setting<field::Hz> =
    Setting::new("spacing_hz", "Spacing", "Hz", 15.0).range(5.0, 30.0, 0.0);
/// Seconds a character.
pub const CHAR_S: Setting<field::F64> =
    Setting::new("char_s", "Seconds a character", "s", 2.0).range(0.425, 8.5, 0.0);
/// The sideband.
pub const SIDEBAND: Setting<field::Choice<Sideband>> =
    Setting::choice("sideband", "Sideband", Sideband::Auto);
/// Read characters.
pub const READ: Setting<field::Bool> = Setting::new("read", "Read characters", "", true);
/// The font file.
pub const FONT: Setting<field::Text> = Setting::new("font", "Font file", "", "")
    .help("An HZK16 file or a TrueType font that the host names. Empty: Latin, Cyrillic and Greek from the shared glyphs.");

sdrkit_core::settings! {
    /// The settings of [`Cp16Decoder`].
    pub enum Cp16Setting {
        /// Fixed parameters.
        Fixed = FIXED,
        /// The lowest carrier.
        Base = BASE_HZ,
        /// The spacing.
        Spacing = SPACING_HZ,
        /// Seconds a character.
        CharS = CHAR_S,
        /// The sideband.
        Sideband = SIDEBAND,
        /// Read characters.
        Read = READ,
        /// The font.
        Font = FONT,
    }
}

sdrkit_core::settings! {
    /// The settings of [`Cp16Sender`]: the transmitter's parameters.
    pub enum Cp16TxSetting {
        /// The lowest carrier.
        Base = BASE_HZ,
        /// The spacing.
        Spacing = SPACING_HZ,
        /// Seconds a character.
        CharS = CHAR_S,
        /// The sideband.
        Sideband = SIDEBAND,
        /// The font of the characters sent.
        Font = FONT,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Cp16Sender`].
    pub enum Cp16Command {
        /// Send.
        Send = CP16_SEND.command,
        /// Stop.
        Stop = CP16_STOP,
    }
}

/// The note of the stack.
pub const NOTE_TEXT: &str = "Not confirmed by a public document: the lowest carrier (1000 Hz here), the font, the blank \
cell for a space, the full-width forms of ASCII and the raised edges of the keying. T/RAC 019-2019 was not read.";

/// How CP16 builds its receiver: the front end's wide stream, Listen, and no demodulator.
pub(crate) const CP16_RX: decoder::RxOptions = decoder::RxOptions {
    id: "cp16",
    input: decoder::InputPort::Wide,
    demod: false,
    sideband_only: false,
    listen: true,
    span: false,
    outputs: &[],
    sender: None,
};

/// The comb of carriers: the lowest comb frequency (Hz, signed) and the spacing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Comb {
    /// Frequency of comb position 0, Hz (negative on the lower sideband).
    pub f0: f64,
    /// Spacing, Hz.
    pub d: f64,
    /// The comb score.
    pub score: f64,
}

impl Comb {
    /// On the lower sideband: its middle is below 0 Hz.
    pub fn lower(&self) -> bool {
        self.f0 + 7.5 * self.d < 0.0
    }
    /// The base frequency: the lowest carrier on the upper sideband, else the highest carrier's distance from the dial.
    pub fn base(&self) -> f64 {
        if self.lower() {
            -(self.f0 + 15.0 * self.d)
        } else {
            self.f0
        }
    }
    /// The base frequency with the 16 columns `o` carriers off the comb.
    pub fn base_at(&self, o: i32) -> f64 {
        self.column_hz(o).abs()
    }
    /// The frequency of picture column `i` (0 the left), Hz signed.
    pub fn column_hz(&self, i: i32) -> f64 {
        if self.lower() {
            self.f0 + f64::from(15 - i) * self.d
        } else {
            self.f0 + f64::from(i) * self.d
        }
    }
}

/// The log spectrum A(f) of the comb search: log10 of the power over the floor, clipped to 0 to 7, interpolated.
struct LogSpec {
    a: Vec<f32>,
    bin: f64,
}

impl LogSpec {
    fn new(x: &[Complex32]) -> Option<LogSpec> {
        let n = 4096;
        if x.len() < n * 5 / 2 {
            return None;
        }
        let mut fft = Fft::forward(n);
        let w: Vec<f32> = (0..n)
            .map(|i| (0.5 - 0.5 * (TAU * i as f64 / n as f64).cos()) as f32)
            .collect();
        let mut acc = vec![0f64; n];
        let mut buf = vec![Complex32::default(); n];
        let mut s = 0;
        while s + n <= x.len() {
            for i in 0..n {
                buf[i] = x[s + i] * w[i];
            }
            fft.process(&mut buf);
            for (k, b) in buf.iter().enumerate() {
                acc[(k + n / 2) % n] += f64::from(b.norm_sqr());
            }
            s += n / 2;
        }
        let mut inner: Vec<f64> = acc[n / 16..n - n / 16].to_vec();
        inner.sort_by(f64::total_cmp);
        let peak = acc.iter().copied().fold(0.0, f64::max);
        let floor = inner[inner.len() / 2].max(peak * 1e-6).max(1e-30);
        Some(LogSpec {
            a: acc
                .iter()
                .map(|&p| (p / floor).log10().clamp(0.0, 7.0) as f32)
                .collect(),
            bin: RATE / n as f64,
        })
    }
    fn at(&self, f: f64) -> f64 {
        let pos = f / self.bin + (self.a.len() / 2) as f64;
        if pos < 0.0 || pos >= (self.a.len() - 1) as f64 {
            return 0.0;
        }
        let i = pos as usize;
        let t = pos - i as f64;
        f64::from(self.a[i]) * (1.0 - t) + f64::from(self.a[i + 1]) * t
    }
    fn score(&self, f0: f64, d: f64) -> f64 {
        let on: f64 = (0..16).map(|j| self.at(f0 + f64::from(j) * d)).sum();
        let off: f64 = (0..15)
            .map(|j| self.at(f0 + (f64::from(j) + 0.5) * d))
            .sum();
        (on - 16.0 / 15.0 * off) / 16.0
    }
}

/// Searches the comb: `f_range` and `d_range` (lowest, highest, step) then a fine search around the best.
fn search(s: &LogSpec, f_range: (f64, f64, f64), d_range: (f64, f64, f64)) -> Option<Comb> {
    let mut best: Option<Comb> = None;
    let mut d = d_range.0;
    while d <= d_range.1 + 1e-9 {
        let mut f = f_range.0;
        while f <= f_range.1 + 1e-9 {
            if f >= -3950.0 && f + 15.0 * d <= 3950.0 {
                let sc = s.score(f, d);
                if best.is_none_or(|b| sc > b.score) {
                    best = Some(Comb {
                        f0: f,
                        d,
                        score: sc,
                    });
                }
            }
            f += f_range.2;
        }
        d += d_range.2;
    }
    let b = best?;
    let mut fine = b;
    for i in -20..=20 {
        let d = b.d + f64::from(i) * 0.01;
        for k in -12..=12 {
            let f = b.f0 + f64::from(k) * 0.05;
            let sc = s.score(f, d);
            if sc > fine.score {
                fine = Comb {
                    f0: f,
                    d,
                    score: sc,
                };
            }
        }
    }
    Some(fine)
}

/// Finds the comb in 8 kHz complex audio. `held`: the comb held from earlier; `base`: a lowest carrier given by hand (with its
/// spacing when `spacing` is given).
pub fn find_comb(
    x: &[Complex32],
    held: Option<Comb>,
    base: Option<f64>,
    spacing: Option<f64>,
) -> Option<Comb> {
    let s = LogSpec::new(x)?;
    let (dlo, dhi) = match spacing {
        Some(d) => (d - 0.3, d + 0.3),
        None => (5.0, 30.0),
    };
    let found = if let Some(h) = held {
        search(
            &s,
            (h.f0 - 0.4 * h.d, h.f0 + 0.4 * h.d, 0.5),
            (h.d - 0.3, h.d + 0.3, 0.2),
        )
    } else if let Some(b) = base {
        let up = search(&s, (b - 4.0, b + 4.0, 0.5), (dlo, dhi, 0.2));
        let down = search(
            &s,
            (-b - 15.0 * dhi - 4.0, -b - 15.0 * dlo + 4.0, 0.5),
            (dlo, dhi, 0.2),
        );
        match (up, down) {
            (Some(u), Some(d)) => Some(if d.score > u.score { d } else { u }),
            (u, d) => u.or(d),
        }
    } else {
        // Around the strongest 300 Hz of the spectrum.
        let mut best = (f64::MIN, 0.0);
        let mut c = -3800.0;
        while c <= 3800.0 {
            let sum: f64 = (-15..=15).map(|k| s.at(c + f64::from(k) * 10.0)).sum();
            if sum > best.0 {
                best = (sum, c);
            }
            c += 10.0;
        }
        let c = best.1;
        search(
            &s,
            (c - 7.5 * dhi - 260.0, c - 7.5 * dlo + 260.0, 0.5),
            (dlo, dhi, 0.2),
        )
    }?;
    (found.score >= 0.12).then_some(found)
}

/// The box-summed magnitude of the audio mixed to `f` Hz over `l` samples, sampled every 4 samples.
fn envelope(x: &[Complex32], f: f64, l: usize) -> Vec<f32> {
    let w = -TAU * f / RATE;
    let mut ring = vec![Complex32::default(); l];
    let mut acc = (0.0f64, 0.0f64);
    let mut out = Vec::with_capacity(x.len() / 4 + 1);
    let (sw, cw) = w.sin_cos();
    let (mut c, mut s) = (1.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        let m = Complex32::new(
            (f64::from(v.re) * c - f64::from(v.im) * s) as f32,
            (f64::from(v.re) * s + f64::from(v.im) * c) as f32,
        );
        let old = std::mem::replace(&mut ring[i % l], m);
        acc.0 += f64::from(m.re) - f64::from(old.re);
        acc.1 += f64::from(m.im) - f64::from(old.im);
        if i % 4 == 3 {
            out.push((acc.0 * acc.0 + acc.1 * acc.1).sqrt() as f32);
        }
        let nc = c * cw - s * sw;
        s = s * cw + c * sw;
        c = nc;
        if i % 4096 == 4095 {
            let r = (c * c + s * s).sqrt();
            c /= r;
            s /= r;
        }
    }
    out
}

/// The row clock found in a window: the row period and the time of a row edge, seconds from the window start, and the score.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowClock {
    /// Row period, seconds.
    pub period: f64,
    /// A row edge, seconds from the window start.
    pub edge: f64,
    /// The concentration of the fold, 0 to 1.
    pub score: f64,
}

/// Finds the row clock from the envelopes (every 4 samples) of the 16 carriers. `l` is the box length (samples) and `hint` a
/// row rate to search near (±2 % given by hand, ±3 % held).
pub fn find_rows(env: &[Vec<f32>], l: usize, hint: Option<(f64, f64)>) -> Option<RowClock> {
    // The slope signal at 500 values a second: the squared change of each envelope over 2 ms, summed.
    let n = env.first()?.len() / 4;
    if n < 1000 {
        return None;
    }
    let mut g = vec![0f32; n];
    for e in env {
        for k in 1..n {
            let d = e[4 * k] - e[4 * (k - 1)];
            g[k] += d * d;
        }
    }
    let gr = 500.0;
    let size = 65536;
    let mut buf = vec![Complex32::default(); size];
    let mean = g.iter().sum::<f32>() / n as f32;
    for (b, &v) in buf.iter_mut().zip(&g) {
        *b = Complex32::new(v - mean, 0.0);
    }
    Fft::forward(size).process(&mut buf);
    let bin = gr / size as f64;
    let mag: Vec<f32> = buf[..size / 2].iter().map(|c| c.norm()).collect();
    let (lo, hi) = match hint {
        Some((rate, tol)) => (rate * (1.0 - tol), rate * (1.0 + tol)),
        None => (2.0, 40.0),
    };
    let (blo, bhi) = (
        (lo / bin).ceil() as usize,
        ((hi / bin).floor() as usize).min(mag.len() - 2),
    );
    let (mlo, mhi) = ((2.0 / bin) as usize, (40.0 / bin) as usize);
    let mut sorted: Vec<f32> = mag[mlo..mhi].to_vec();
    let m = sorted.len() / 2;
    sorted.select_nth_unstable_by(m, f32::total_cmp);
    let med = sorted[m].max(1e-30);
    let mut peaks: Vec<usize> = (blo.max(1)..=bhi)
        .filter(|&k| mag[k] >= mag[k - 1] && mag[k] >= mag[k + 1] && mag[k] >= 4.0 * med)
        .collect();
    peaks.sort_by(|&a, &b| mag[b].total_cmp(&mag[a]));
    peaks.truncate(12);
    let mut cands = peaks.clone();
    for &p in &peaks {
        for div in [2.0, 3.0] {
            let c = (p as f64 / div).round() as usize;
            if c >= 5 && c + 4 < mag.len() {
                let best = (c - 4..=c + 4).max_by(|&a, &b| mag[a].total_cmp(&mag[b]));
                cands.extend(best);
            }
        }
    }
    cands.sort_unstable();
    cands.dedup();
    let floor: Vec<f32> = g.iter().map(|&v| (v - 0.5 * mean).max(0.0)).collect();
    let mut folds: Vec<(f64, f64, f64, f32)> = Vec::new();
    for &k in &cands {
        let rate = k as f64 * bin;
        let p = gr / rate;
        let nb = (p.round() as usize).max(8);
        let mut f = vec![0f64; nb];
        for (i, &v) in floor.iter().enumerate() {
            let ph = (i as f64 / p).fract();
            f[((ph * nb as f64) as usize).min(nb - 1)] += f64::from(v);
        }
        let total: f64 = f.iter().sum();
        if total <= 0.0 {
            continue;
        }
        let width_s = (l as f64 / RATE + 0.004).min(0.6 * p / gr);
        let wb = ((width_s * gr / p) * nb as f64).round().max(1.0) as usize;
        let (mut best, mut at) = (f64::MIN, 0usize);
        for s in 0..nb {
            let sum: f64 = (0..wb).map(|j| f[(s + j) % nb]).sum();
            if sum > best {
                (best, at) = (sum, s);
            }
        }
        let frac = wb as f64 / nb as f64;
        let conc = (best / total - frac) / (1.0 - frac);
        // The circular centroid of the window.
        let (mut cx, mut cy) = (0.0, 0.0);
        for j in 0..wb {
            let b = (at + j) % nb;
            let a = TAU * (b as f64 + 0.5) / nb as f64;
            cx += f[b] * a.cos();
            cy += f[b] * a.sin();
        }
        let centre = cy.atan2(cx).rem_euclid(TAU) / TAU * p / gr;
        let edge = (centre - (l as f64 - 1.0) / 2.0 / RATE - 0.001).rem_euclid(p / gr);
        folds.push((p / gr, edge, conc, mag[k]));
    }
    let top = folds.iter().map(|f| f.2).fold(f64::MIN, f64::max);
    let pick = folds
        .iter()
        .filter(|f| f.2 >= 0.85 * top)
        .max_by(|a, b| a.0.total_cmp(&b.0))?;
    (f64::from(pick.3) >= 6.0 * f64::from(med) && pick.2 >= 0.25).then_some(RowClock {
        period: pick.0,
        edge: pick.1,
        score: pick.2,
    })
}

/// Splits values into blank and inked by the largest between-class variance: the blank and the inked level.
fn levels(values: &[f32]) -> Option<(f32, f32)> {
    if values.len() < 16 {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(f32::total_cmp);
    let n = v.len();
    let total: f64 = v.iter().map(|&x| f64::from(x)).sum();
    let (mut best, mut cut) = (f64::MIN, n / 2);
    let mut left = 0.0f64;
    for k in 1..n {
        left += f64::from(v[k - 1]);
        let (w0, w1) = (k as f64, (n - k) as f64);
        let (m0, m1) = (left / w0, (total - left) / w1);
        let s = w0 * w1 * (m0 - m1) * (m0 - m1);
        if s > best {
            (best, cut) = (s, k);
        }
    }
    let n0 = v[..cut].iter().sum::<f32>() / cut as f32;
    let a1 = v[cut..].iter().sum::<f32>() / (n - cut) as f32;
    Some((n0, a1.max(1.05 * n0)))
}

/// Rows kept for the picture.
const PICTURE_KEEP: usize = 1360;
/// Audio kept, seconds.
const KEEP_S: f64 = 32.0;
/// The window measured, seconds.
const WINDOW_S: f64 = 20.0;

/// The CP16 receiver: 24 kHz complex (the dial at 0) in, the picture rows, the characters and the text out.
pub struct Cp16Decoder {
    fixed: bool,
    base: f64,
    spacing: f64,
    char_s: f64,
    sideband: Sideband,
    read: bool,
    font: String,
    cells: Cells,
    rec: Option<Arc<Recognizer>>,
    lpf: Vec<f32>,
    hist: Vec<Complex32>,
    phase3: usize,
    audio: Vec<Complex32>,
    first: u64,
    n8: u64,
    next_measure: u64,
    comb: Option<Comb>,
    fails: u32,
    clock: Option<(f64, f64)>,
    timing: f64,
    next_row: f64,
    rows: Vec<[f32; 24]>,
    rows_first: usize,
    dots: Vec<[f32; 24]>,
    arr: i32,
    arr_fixed: bool,
    votes: [u32; 9],
    totals: [f32; 9],
    snr: f64,
    reading_at: usize,
    last_end: Option<usize>,
    text: String,
    chars: i64,
    sure: i64,
    unclear: i64,
    heard: Heard,
    dial: f64,
    status: Status,
    logs: Vec<String>,
    pic_dirty: bool,
}

impl Default for Cp16Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Cp16Decoder {
    /// A receiver at the default settings.
    pub fn new() -> Self {
        // A 63-tap Blackman low-pass at 3700 Hz for 24 kHz.
        let taps = 63;
        let fc = 3700.0 / 24000.0;
        let lpf: Vec<f32> = (0..taps)
            .map(|i| {
                let m = i as f64 - (taps - 1) as f64 / 2.0;
                let sinc = if m == 0.0 {
                    2.0 * fc
                } else {
                    (TAU * fc * m).sin() / (PI * m)
                };
                let w = 0.42 - 0.5 * (TAU * i as f64 / (taps - 1) as f64).cos()
                    + 0.08 * (2.0 * TAU * i as f64 / (taps - 1) as f64).cos();
                (sinc * w) as f32
            })
            .collect();
        Cp16Decoder {
            fixed: false,
            base: BASE_HZ.default,
            spacing: SPACING_HZ.default,
            char_s: CHAR_S.default,
            sideband: Sideband::Auto,
            read: true,
            font: String::new(),
            cells: Cells::new(),
            rec: None,
            lpf,
            hist: Vec::new(),
            phase3: 0,
            audio: Vec::new(),
            first: 0,
            n8: 0,
            next_measure: (5.0 * RATE) as u64,
            comb: None,
            fails: 0,
            clock: None,
            timing: 0.0,
            next_row: 0.0,
            rows: Vec::new(),
            rows_first: 0,
            dots: Vec::new(),
            arr: 0,
            arr_fixed: false,
            votes: [0; 9],
            totals: [0.0; 9],
            snr: 0.0,
            reading_at: 0,
            last_end: None,
            text: String::new(),
            chars: 0,
            sure: 0,
            unclear: 0,
            heard: Heard::new("CP16"),
            dial: 0.0,
            status: Status::default(),
            logs: Vec::new(),
            pic_dirty: false,
        }
    }

    fn recognizer(&mut self) -> Arc<Recognizer> {
        self.rec
            .get_or_insert_with(|| Arc::new(Recognizer::new(&self.cells)))
            .clone()
    }

    fn reset(&mut self) {
        self.audio.clear();
        self.hist.clear();
        self.first = self.n8;
        self.next_measure = self.n8 + (5.0 * RATE) as u64;
        self.comb = None;
        self.clock = None;
        self.fails = 0;
    }

    fn fixed_comb(&self) -> Comb {
        let lower = self.sideband == Sideband::Lower;
        let f0 = if lower {
            -(self.base + 15.0 * self.spacing)
        } else {
            self.base
        };
        Comb {
            f0,
            d: self.spacing,
            score: 0.0,
        }
    }

    fn measure(&mut self, out: &mut Outbox) {
        let win = (WINDOW_S * RATE) as usize;
        let start = self.audio.len().saturating_sub(win);
        let x = &self.audio[start..];
        let t0 = (self.first + start as u64) as f64 / RATE;
        let comb = if self.fixed {
            Some(self.fixed_comb())
        } else {
            let base = None;
            let found = find_comb(x, self.comb, base, None);
            match found {
                Some(c) => {
                    if self.comb.is_none() {
                        self.logs.push(format!(
                            "carriers: {:.1} Hz + 15 x {:.2} Hz{}",
                            c.base(),
                            c.d,
                            if c.lower() { ", lower sideband" } else { "" }
                        ));
                    }
                    self.fails = 0;
                    Some(c)
                }
                None => {
                    self.fails += 1;
                    if self.fails >= 4 {
                        self.comb = None;
                        self.clock = None;
                    }
                    self.comb
                }
            }
        };
        let Some(comb) = comb else {
            return;
        };
        self.comb = Some(comb);
        let l = ((RATE / comb.d).round() as usize).max(8);
        let env: Vec<Vec<f32>> = (-4..20)
            .map(|j| envelope(x, comb.column_hz(j), l))
            .collect();
        let hint = if self.fixed {
            Some((ROWS_PER_CELL as f64 / self.char_s, 0.02))
        } else {
            self.clock.map(|(p, _)| (1.0 / p, 0.03))
        };
        if let Some(rc) = find_rows(&env[4..20], l, hint) {
            self.timing = rc.score;
            let edge_abs = t0 + rc.edge;
            match self.clock {
                None => {
                    // Rows start at the first edge in the window.
                    self.clock = Some((rc.period, edge_abs));
                    self.next_row = edge_abs.max(self.next_row);
                    if self.next_row > edge_abs {
                        let k = ((self.next_row - edge_abs) / rc.period).ceil();
                        self.next_row = edge_abs + k * rc.period;
                    }
                }
                Some((p, _)) => {
                    let p2 = 0.7 * p + 0.3 * rc.period;
                    let k = ((self.next_row - edge_abs) / rc.period).round();
                    let predicted = edge_abs + k * rc.period;
                    let diff = predicted - self.next_row;
                    if diff.abs() <= 0.35 * p2 {
                        self.next_row += 0.5 * diff;
                    } else {
                        self.next_row = predicted;
                    }
                    self.clock = Some((p2, edge_abs));
                }
            }
        }
        let Some((period, _)) = self.clock else {
            return;
        };
        // New rows whose end is in the window.
        let t_end = t0 + x.len() as f64 / RATE;
        while self.next_row + period <= t_end {
            let a = ((self.next_row - t0) * RATE).round().max(0.0) as usize;
            let b = (((self.next_row + period) - t0) * RATE).round() as usize;
            let b = b.min(x.len());
            if a >= b {
                self.next_row += period;
                continue;
            }
            let mut row = [0f32; 24];
            for (j, v) in row.iter_mut().enumerate() {
                if b - a > l + 8 {
                    let e = &env[j][(a / 4).min(env[j].len())..(b / 4).min(env[j].len())];
                    *v = if e.is_empty() {
                        0.0
                    } else {
                        e.iter().sum::<f32>() / e.len() as f32
                    };
                } else {
                    let (m0, m1) = (a + (b - a) / 10, b - (b - a) / 10);
                    let w = -TAU * comb.column_hz(j as i32 - 4) / RATE;
                    let mut acc = Complex32::default();
                    for (i, s) in x[m0..m1].iter().enumerate() {
                        let p = w * (m0 + i) as f64;
                        acc += s * Complex32::new(p.cos() as f32, p.sin() as f32);
                    }
                    *v = acc.norm();
                }
            }
            self.rows.push(row);
            self.next_row += period;
        }
        self.dots_and_read(out);
    }

    fn dots_and_read(&mut self, out: &mut Outbox) {
        if self.rows.len() < 8 {
            return;
        }
        let from = self.rows.len().saturating_sub(240);
        let a = (4 + self.arr) as usize;
        let vals: Vec<f32> = self.rows[from..]
            .iter()
            .flat_map(|r| r[a..a + 16].iter().copied())
            .collect();
        let Some((n0, a1)) = levels(&vals) else {
            return;
        };
        self.snr = 20.0 * f64::from((a1 / n0.max(1e-12)).max(1e-6)).log10();
        while self.dots.len() < self.rows.len() {
            let r = &self.rows[self.dots.len()];
            let mut d = [0f32; 24];
            for (i, v) in d.iter_mut().enumerate() {
                *v = ((r[i] - n0) / (a1 - n0)).clamp(0.0, 1.0);
            }
            self.dots.push(d);
            self.pic_dirty = true;
        }
        if self.read {
            self.read_chars(out);
        }
        // The picture keeps its newest rows.
        if self.dots.len() > PICTURE_KEEP + 200 {
            let d = self.dots.len() - PICTURE_KEEP;
            self.dots.drain(..d);
            self.rows.drain(..d);
            self.rows_first += d;
            self.reading_at = self.reading_at.saturating_sub(d);
            self.last_end = self.last_end.map(|e| e.saturating_sub(d));
        }
    }

    /// The 16 rows from `start` in the columns of arrangement `o` (the 16 columns `o` carriers off the comb).
    fn cell_at(&self, start: usize, o: i32) -> [[f32; 16]; 16] {
        let mut b = [[0f32; 16]; 16];
        let a = (4 + o) as usize;
        for (r, row) in b.iter_mut().enumerate() {
            row.copy_from_slice(&self.dots[start + r][a..a + 16]);
        }
        b
    }

    /// The arrangements that fit the ink so far: at most 3 % of the inked dots outside their columns and their middle within
    /// 1.6 columns of the ink's centre (all of them below 50 inked dots; with none, the nearest).
    fn arrangements(&self) -> Vec<i32> {
        if self.arr_fixed || self.fixed {
            return vec![if self.fixed { 0 } else { self.arr }];
        }
        let mut ink = [0f64; 24];
        for d in &self.dots {
            for (k, &v) in d.iter().enumerate() {
                if v >= 0.5 {
                    ink[k] += 1.0;
                }
            }
        }
        let n: f64 = ink.iter().sum();
        if n < 50.0 {
            return (-4..=4).collect();
        }
        let centre = ink
            .iter()
            .enumerate()
            .map(|(k, &v)| (k as f64 - 4.0) * v)
            .sum::<f64>()
            / n;
        let valid: Vec<i32> = (-4..=4)
            .filter(|&o| {
                let inside: f64 = ink[(4 + o) as usize..(20 + o) as usize].iter().sum();
                n - inside <= 0.03 * n && (f64::from(o) + 7.5 - centre).abs() <= 1.6
            })
            .collect();
        if valid.is_empty() {
            let o = (centre - 7.5).round().clamp(-4.0, 4.0) as i32;
            vec![o]
        } else {
            valid
        }
    }

    /// Counts a vote for arrangement `o` with a character's score, and fixes the arrangement when one leads clearly.
    fn vote(&mut self, o: i32, score: f32) {
        if self.arr_fixed || self.fixed {
            return;
        }
        let k = (o + 4) as usize;
        self.votes[k] += 1;
        self.totals[k] += score;
        let all: u32 = self.votes.iter().sum();
        let mut order: Vec<usize> = (0..9).collect();
        order.sort_by(|&a, &b| self.totals[b].total_cmp(&self.totals[a]));
        let (lead, second) = (order[0], order[1]);
        self.arr = lead as i32 - 4;
        let v = self.votes[lead];
        if (v >= 4 && f64::from(v) >= 0.9 * f64::from(all))
            || (all >= 8 && self.totals[lead] >= 1.15 * self.totals[second])
            || all >= 40
        {
            self.arr_fixed = true;
        }
    }

    fn read_chars(&mut self, out: &mut Outbox) {
        let rec = self.recognizer();
        if rec.is_empty() {
            return;
        }
        loop {
            let Some(first) = (self.reading_at..self.dots.len())
                .find(|&r| self.dots[r][4..20].iter().any(|&d| d >= 0.5))
            else {
                self.reading_at = self.dots.len().saturating_sub(16).max(self.reading_at);
                return;
            };
            if first + 16 > self.dots.len() {
                return;
            }
            let lo = first.saturating_sub(15).max(self.reading_at);
            let mut best: Option<(Match, usize, i32)> = None;
            for o in self.arrangements() {
                for s in lo..=first {
                    if s + 16 > self.dots.len() {
                        break;
                    }
                    if let Some(m) = rec.recognise(&self.cell_at(s, o)) {
                        if best.is_none_or(|b| m.score > b.0.score) {
                            best = Some((m, s, o));
                        }
                    }
                }
            }
            let Some((m, s, o)) = best else {
                return;
            };
            if m.score >= 0.40 {
                self.vote(o, m.score);
            }
            if m.score < 0.25 {
                self.reading_at = first + 1;
                continue;
            }
            let time = (self.rows_first + s) as f64 * self.clock.map_or(0.1, |c| c.0);
            if self.last_end.is_some_and(|e| s >= e + 14) {
                self.push_char(' ', time, out);
            }
            let c = if m.score >= 0.40 { shown(m.c) } else { '?' };
            let sure = m.score >= 0.62 && m.margin >= 0.03;
            self.chars += 1;
            if sure {
                self.sure += 1;
            }
            if c == '?' {
                self.unclear += 1;
            }
            self.push_char(c, time, out);
            out.event(CP16_CHAR.event(
                time,
                self.dial + self.comb.map_or(0.0, |c| c.base()),
                &Cp16CharEvent {
                    text: c.to_string().as_str(),
                    score: f64::from(m.score),
                    margin: f64::from(m.margin),
                    sure,
                    family: Family::Message,
                },
            ));
            self.reading_at = s + 16;
            self.last_end = Some(s + 16);
        }
    }

    fn push_char(&mut self, c: char, time: f64, out: &mut Outbox) {
        if c == ' ' {
            if let Some(w) = self.text.split_whitespace().last() {
                if sdrkit_util_text::callsign::is_callsign(w) {
                    let w = w.to_owned();
                    let f = self.dial + self.comb.map_or(0.0, |c| c.base());
                    self.heard.spot(out, &w, time, f, self.snr as f32);
                }
            }
        }
        self.text.push(c);
        if self.text.len() > 8000 {
            let cut = self.text.len() - 8000;
            let at = (cut..self.text.len())
                .find(|&i| self.text.is_char_boundary(i))
                .unwrap_or(0);
            self.text.drain(..at);
        }
    }

    fn picture(&mut self, out: &mut Outbox) {
        if !self.pic_dirty || self.dots.is_empty() {
            return;
        }
        self.pic_dirty = false;
        let rows = &self.dots[self.dots.len().saturating_sub(PICTURE_KEEP)..];
        let mut p = HfPicture::new(1, "CP16", 16, rows.len() as u32, 0.0);
        for (y, r) in rows.iter().enumerate() {
            let a = (4 + self.arr) as usize;
            for (x, &v) in r[a..a + 16].iter().enumerate() {
                let g = (v * 255.0) as u8;
                p.put(x as u32, y as u32, g, g, g);
            }
        }
        p.rows = rows.len() as u32;
        out.picture(self.n8 as f64 / RATE, p.to_picture());
    }

    fn load_font(&mut self) {
        self.cells = Cells::new();
        if !self.font.is_empty() {
            match std::fs::read(&self.font).ok().and_then(Cells::with_font) {
                Some(c) => {
                    self.cells = c;
                    self.logs.push(format!("font {}", self.font));
                }
                None => self
                    .logs
                    .push(format!("the font {} cannot be read", self.font)),
            }
        }
        self.rec = None;
    }
}

impl sdrkit_graph::Decoder for Cp16Decoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.cp16"
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        Cp16Setting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match Cp16Setting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Cp16Setting::Fixed => self.fixed = FIXED.read(id, value)?,
            Cp16Setting::Base => self.base = BASE_HZ.read(id, value)?,
            Cp16Setting::Spacing => self.spacing = SPACING_HZ.read(id, value)?,
            Cp16Setting::CharS => self.char_s = CHAR_S.read(id, value)?,
            Cp16Setting::Sideband => self.sideband = SIDEBAND.read(id, value)?,
            Cp16Setting::Read => self.read = READ.read(id, value)?,
            Cp16Setting::Font => {
                let f = FONT.read(id, value)?.to_owned();
                if f != self.font {
                    self.font = f;
                    self.load_font();
                }
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (Cp16Setting::Fixed, Value::Bool(self.fixed)),
            (Cp16Setting::Base, Value::Hz(self.base)),
            (Cp16Setting::Spacing, Value::Hz(self.spacing)),
            (Cp16Setting::CharS, Value::F64(self.char_s)),
            (Cp16Setting::Sideband, Value::Choice(self.sideband.index())),
            (Cp16Setting::Read, Value::Bool(self.read)),
            (Cp16Setting::Font, Value::Text(self.font.clone())),
        ])
    }
    fn results(&self) -> Published {
        crate::typed::results("cp16")
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.dial = input.tuned_hz;
        self.reset();
        Ok(decoder::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.reset();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        // To 8 kHz: the low-pass, keeping every third output.
        let taps = self.lpf.len();
        for &s in x {
            let s = if s.re.is_finite() && s.im.is_finite() {
                s
            } else {
                Complex32::default()
            };
            self.hist.push(s);
            self.phase3 += 1;
            if self.phase3 == 3 {
                self.phase3 = 0;
                let h = &self.hist[self.hist.len().saturating_sub(taps)..];
                let mut acc = Complex32::default();
                for (v, &c) in h.iter().rev().zip(&self.lpf) {
                    acc += v * c;
                }
                self.audio.push(acc);
                self.n8 += 1;
            }
            if self.hist.len() > 4096 {
                let d = self.hist.len() - taps;
                self.hist.drain(..d);
            }
        }
        let keep = (KEEP_S * RATE) as usize;
        if self.audio.len() > keep + 8000 {
            let d = self.audio.len() - keep;
            self.audio.drain(..d);
            self.first += d as u64;
        }
        while self.n8 >= self.next_measure {
            self.next_measure += RATE as u64;
            self.measure(out);
            self.picture(out);
        }
    }
    fn report(&mut self, res: &mut Published) {
        let state = match (self.comb.is_some(), self.clock.is_some()) {
            (true, true) => 2,
            (true, false) => 1,
            _ => 0,
        };
        let st = &mut self.status;
        st.lock_state = LockState::from_level(state as u8);
        st.snr_db = (state == 2).then_some(self.snr as f32);
        st.offset_hz = Some(0.0);
        match (self.comb, self.clock) {
            (Some(c), Some((p, _))) => put_text(
                &mut st.summary,
                format_args!(
                    "CP16: {:.0} Hz + 15 x {:.1} Hz, {:.2} s a character, {} read, {} unclear",
                    c.base_at(self.arr),
                    c.d,
                    p * ROWS_PER_CELL as f64,
                    self.chars,
                    self.unclear
                ),
            ),
            (Some(c), None) => put_text(
                &mut st.summary,
                format_args!("CP16: carriers at {:.0} Hz + 15 x {:.1} Hz", c.base(), c.d),
            ),
            _ => put_text(&mut st.summary, format_args!("CP16: listening")),
        }
        res.results_mut().set_status(st);
        let arr = self.arr;
        res.put(&Cp16Fields {
            text: self.text.as_str(),
            callsign: self
                .text
                .split_whitespace()
                .rev()
                .find(|w| sdrkit_util_text::callsign::is_callsign(w)),
            state,
            base_hz: self.comb.map(|c| (c.base_at(arr) * 10.0).round() / 10.0),
            spacing_hz: self.comb.map(|c| (c.d * 100.0).round() / 100.0),
            row_ms: self.clock.map(|c| (c.0 * 10000.0).round() / 10.0),
            char_s: self.clock.map(|c| (c.0 * 17.0 * 1000.0).round() / 1000.0),
            mirrored: self.comb.map(|c| c.lower()),
            comb_db: self.comb.map(|c| (c.score * 100.0).round() / 10.0),
            timing: (self.timing * 100.0).round() / 100.0,
            snr_db: (state == 2).then_some((self.snr * 10.0).round() / 10.0),
            picture_rows: (self.rows_first + self.dots.len()) as i64,
            characters: self.chars,
            sure: self.sure,
            unclear: self.unclear,
            font_name: self.cells.has_font().then_some(self.font.as_str()),
            note: NOTE_TEXT,
        });
        self.heard.report(res);
        flush_logs(&mut self.logs, res);
    }
    fn clear(&mut self) {
        self.arr_fixed = false;
        self.votes = [0; 9];
        self.totals = [0.0; 9];
        self.rows_first += self.dots.len();
        self.rows.clear();
        self.dots.clear();
        self.reading_at = 0;
        self.last_end = None;
        self.text.clear();
        self.chars = 0;
        self.sure = 0;
        self.unclear = 0;
        self.heard.clear();
    }
    fn forget(&mut self, kept: &Kept) {
        let text = kept.field("text").then(|| std::mem::take(&mut self.text));
        let counts = (self.chars, self.sure, self.unclear);
        let heard = self.heard.clone();
        self.clear();
        self.reset();
        if let Some(t) = text {
            self.text = t;
        }
        (self.chars, self.sure, self.unclear) = counts;
        self.heard = heard;
        self.heard.forget(kept);
    }
}

/// The transmitter (`hfkbd.cp16tx`): input `in` is the session's samples (their rate and pace), output `out` the complex
/// baseband for `tx.sink`, centred on the dial; output `events` the `cp16.tx` events.
pub struct Cp16Sender {
    base: f64,
    spacing: f64,
    char_s: f64,
    sideband: Sideband,
    rate: f64,
    dial: f64,
    transmit: bool,
    burst: Option<(TxSource, String)>,
    stop_at: Option<u64>,
    font: String,
    cells: Cells,
    events: Vec<sdrkit_core::Event>,
    seed: u64,
}

impl Default for Cp16Sender {
    fn default() -> Self {
        Cp16Sender {
            base: BASE_HZ.default,
            spacing: SPACING_HZ.default,
            char_s: CHAR_S.default,
            sideband: Sideband::Auto,
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            burst: None,
            stop_at: None,
            font: String::new(),
            cells: Cells::new(),
            events: Vec::new(),
            seed: 1,
        }
    }
}

impl Cp16Sender {
    fn params(&self) -> TxParams {
        TxParams {
            char_s: self.char_s,
            spacing_hz: self.spacing,
            base_hz: self.base,
            lower: self.sideband == Sideband::Lower,
        }
    }
    fn event(&mut self, state: &str, reason: &str, text: &str) {
        self.events.push(CP16_TX.event(
            0.0,
            self.dial + self.base,
            &Cp16TxEvent {
                state,
                reason,
                text,
                family: Family::Message,
            },
        ));
    }
    fn send(&mut self, text: &str) -> Result<(), TxError> {
        if !self.transmit {
            return Err(TxError::Off);
        }
        if self.burst.is_some() {
            return Err(TxError::Busy);
        }
        if self.rate < SYNTH_RATE {
            return Err(TxError::Rate);
        }
        let p = self.params();
        let x = transmission16(text, &p, &self.cells, self.seed)?;
        self.seed = self.seed.wrapping_add(1);
        self.burst = Some((TxSource::new(x, self.rate, false), text.to_owned()));
        self.stop_at = None;
        Ok(())
    }
}

impl Component for Cp16Sender {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfkbd.cp16tx",
            vec![Port::iq("in")],
            vec![
                Port::iq("out"),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(Cp16TxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        Ok(vec![
            Format::iq(self.rate, self.dial, self.dial),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        if !ctx.transmit && self.burst.is_some() {
            // The transmission ends with the transmitter.
            let t = self.burst.take().map(|b| b.1).unwrap_or_default();
            self.event("finished", "the transmitter is off", &t);
        }
    }
    fn process(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        if let Some((src, _)) = &mut self.burst {
            let left = self
                .stop_at
                .map_or(u64::MAX, |e| e.saturating_sub(src.produced()));
            let want = (room as u64).min(left) as usize;
            let out = io.iq_out(0);
            let start = out.len();
            out.resize(start + want, Complex32::default());
            let n = src.next_into(&mut out[start..]);
            out.truncate(start + n);
            if n < want || self.stop_at.is_some_and(|e| src.produced() >= e) {
                if let Some(last) = io.iq_out(0).len().checked_sub(1) {
                    io.output(0).tag_here(last, TagKind::BurstEnd);
                }
                let t = self.burst.take().map(|b| b.1).unwrap_or_default();
                self.event("finished", "", &t);
            }
        }
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match Cp16TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Cp16TxSetting::Base => self.base = BASE_HZ.read(id, v)?,
            Cp16TxSetting::Spacing => self.spacing = SPACING_HZ.read(id, v)?,
            Cp16TxSetting::CharS => self.char_s = CHAR_S.read(id, v)?,
            Cp16TxSetting::Sideband => self.sideband = SIDEBAND.read(id, v)?,
            Cp16TxSetting::Font => {
                let f = FONT.read(id, v)?.to_owned();
                if f != self.font {
                    self.cells = if f.is_empty() {
                        Cells::new()
                    } else {
                        std::fs::read(&f)
                            .ok()
                            .and_then(Cells::with_font)
                            .unwrap_or_default()
                    };
                    self.font = f;
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (Cp16TxSetting::Base, Value::Hz(self.base)),
            (Cp16TxSetting::Spacing, Value::Hz(self.spacing)),
            (Cp16TxSetting::CharS, Value::F64(self.char_s)),
            (
                Cp16TxSetting::Sideband,
                Value::Choice(self.sideband.index()),
            ),
            (Cp16TxSetting::Font, Value::Text(self.font.clone())),
        ])
    }
    fn commands(&self) -> &Commands {
        Cp16Command::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match Cp16Command::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            Cp16Command::Send => {
                let a: Cp16SendArgs = CP16_SEND.args(args)?;
                let text = a.text.to_owned();
                match self.send(&text) {
                    Ok(()) => self.event("started", "", &text),
                    Err(e) => {
                        self.event("refused", &e.to_string(), &text);
                        return Err(CommandError::Failed {
                            source: StageError::new(e),
                        });
                    }
                }
            }
            Cp16Command::Stop => {
                if let Some((src, _)) = &self.burst {
                    let row_s = self.params().row_s();
                    self.stop_at = Some(stop_after_row(src.produced(), self.rate, row_s));
                }
            }
        }
        Ok(None)
    }
}

/// The text of the test signal, letters and digits only.
pub const TEST_LETTERS: &str = "CQ CP16 DE BA0TST 73";
/// The text of the test signal with Chinese characters (boxes without a font).
pub const TEST_CHINESE: &str = "CQ CP16 DE BA0TST 业余无线电汉字通信试验 73";

sdrkit_core::choice_enum! {
    /// The text of the test signal.
    pub enum GenText {
        /// Letters and digits only.
        Letters = ("letters", "Letters and digits"),
        /// With Chinese characters.
        Chinese = ("chinese", "Chinese"),
    }
}

/// Seconds a character of the test signal.
pub const GEN_CHAR_S: Setting<field::F64> =
    Setting::new("gen_char_s", "Test signal seconds a character", "s", 2.0)
        .range(1.0, 4.0, 0.0)
        .section(GEN_SECTION)
        .advanced();
/// The spacing of the test signal.
pub const GEN_SPACING_HZ: Setting<field::Hz> =
    Setting::new("gen_spacing_hz", "Test signal spacing", "Hz", 15.0)
        .range(10.0, 20.0, 0.0)
        .section(GEN_SECTION)
        .advanced();
/// The test signal on the lower sideband.
pub const GEN_LSB: Setting<field::Bool> =
    Setting::new("gen_lsb", "Test signal on the lower sideband", "", false)
        .section(GEN_SECTION)
        .advanced();
/// The lowest carrier of the test signal.
pub const GEN_BASE_HZ: Setting<field::Hz> =
    Setting::new("gen_base_hz", "Test signal lowest carrier", "Hz", 1000.0)
        .range(100.0, 3500.0, 0.0)
        .section(GEN_SECTION)
        .advanced();
/// The text of the test signal.
pub const GEN_TEXT: Setting<field::Choice<GenText>> =
    Setting::choice("gen_text", "Test signal text", GenText::Letters)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of the CP16 stack.
    pub enum Cp16GenSetting {
        /// Seconds a character.
        CharS = GEN_CHAR_S,
        /// The spacing.
        Spacing = GEN_SPACING_HZ,
        /// Lower sideband.
        Lsb = GEN_LSB,
        /// The lowest carrier.
        Base = GEN_BASE_HZ,
        /// The text.
        Text = GEN_TEXT,
    }
}

/// The test signal: the transmitter's signal of the text, repeated after 2 s of silence, the dial where
/// [`decoder::test_dial`] puts it.
pub struct Cp16Generator {
    rate: f64,
    dial: f64,
    src: TxSource,
    n: u64,
}

impl Generator for Cp16Generator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // About half the 16 carriers are on: 8 × 0.055².
        Some(SignalInfo::new(2500.0).with_power(8.0 * f64::from(CARRIER_AMP).powi(2)))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        self.dial
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let n = self.src.next_into(out);
        out[n..].fill(Complex32::default());
        if self.dial != 0.0 {
            for o in out.iter_mut() {
                let p = TAU * (self.dial * self.n as f64 / self.rate).fract();
                *o *= Complex32::new(p.cos() as f32, p.sin() as f32);
                self.n += 1;
            }
        }
        out.len()
    }
}

/// The generator of the stack.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let p = TxParams {
        char_s: s.get(&GEN_CHAR_S),
        spacing_hz: s.get(&GEN_SPACING_HZ),
        base_hz: s.get(&GEN_BASE_HZ),
        lower: s.get(&GEN_LSB),
    };
    let text = match s.get(&GEN_TEXT) {
        GenText::Letters => TEST_LETTERS,
        GenText::Chinese => TEST_CHINESE,
    };
    let rate = cfg.rate_hz;
    // The text after 2 s of silence (0.5 s of lead and 1.5 s after the text), again and again.
    let mut x = transmission16(text, &p, &Cells::new(), cfg.seed).unwrap_or_default();
    x.extend(std::iter::repeat_n(
        Complex32::default(),
        (1.5 * SYNTH_RATE) as usize,
    ));
    Ok(Box::new(Cp16Generator {
        rate,
        dial: decoder::test_dial(rate),
        src: TxSource::new(x, rate, true),
        n: 0,
    }))
}

/// The settings of the stack: the receiver's, Listen, and the test signal's.
pub fn schema() -> Arc<Schema> {
    static CELL: std::sync::OnceLock<Arc<Schema>> = std::sync::OnceLock::new();
    crate::typed::schema(&CELL, "cp16")
}

/// The receiver and transmitter of the stack: the family front end, the receiver on its 24 kHz complex stream (both
/// sidebands), and the transmitter on the session's samples into the transmit sink. `send` and `stop` go to the transmitter;
/// the four transmit parameters to both.
///
/// # Errors
///
/// The errors of the graph and of a refused setting.
pub fn graph(
    cfg: &sdrkit_core::StackConfig,
) -> Result<sdrkit_graph::StackGraph, sdrkit_graph::GraphError> {
    use sdrkit_graph::{CommandTarget, GraphBuilder, StackGraph};
    let dec = Cp16Decoder::new();
    let b = GraphBuilder::new()
        .catalog(&sdrkit_graph::Catalog::of(cfg)?)
        .add("front", crate::front::HfFront::new())
        .add("dec", sdrkit_graph::DecoderComponent::new(dec))
        .add("sender", Cp16Sender::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("front")
        .input("sender.in")
        .connect("front.wide", "dec.in")
        .connect("sender.out", "tx.in")
        .output("dec.events")
        .output("dec.pictures")
        .output("sender.events")
        .output("front.listen")
        .status_from("dec")
        .clock(0.1);
    let mut g = StackGraph::of(crate::typed::shape("cp16"), b.build()?)
        .min_rate(
            crate::front::MIN_RATE,
            "HF keyboard stacks need a sample rate of at least 12 ksps",
        )
        .route("listen", &["front.listen"])?;
    for k in ["fixed", "read"] {
        g = g.route(k, &[&format!("dec.{k}")])?;
    }
    for k in ["base_hz", "spacing_hz", "char_s", "sideband", "font"] {
        g = g.route(k, &[&format!("dec.{k}"), &format!("sender.{k}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("send", CommandTarget::of("sender", "send"))
        .command("stop", CommandTarget::of("sender", "stop"))
        .with_settings(&cfg.settings)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_of_a_text() {
        let c = cells("CQ 中\n7");
        assert_eq!(
            c.iter().map(|&c| u32::from(c)).collect::<Vec<_>>(),
            vec![0xFF23, 0xFF31, 0x3000, 0x4E2D, 0x3000, 0xFF17]
        );
        assert_eq!(rows_of(&c, &Cells::new()).len(), 6 * 17);
        assert_eq!(cells("\u{7}"), Vec::<char>::new());
    }

    #[test]
    fn validation() {
        let ok = TxParams {
            char_s: 2.0,
            spacing_hz: 15.0,
            base_hz: 1000.0,
            lower: false,
        };
        assert_eq!(validate(&ok, "CQ"), Ok(()));
        assert_eq!(
            validate(
                &TxParams {
                    spacing_hz: 50.0,
                    ..ok
                },
                "CQ"
            ),
            Err(TxError::Spacing)
        );
        assert_eq!(
            validate(
                &TxParams {
                    base_hz: 3700.0,
                    ..ok
                },
                "CQ"
            ),
            Err(TxError::Band)
        );
        assert_eq!(
            validate(&TxParams { char_s: 0.2, ..ok }, "CQ"),
            Err(TxError::Row)
        );
        assert_eq!(validate(&ok, "\u{1}"), Err(TxError::Empty));
    }

    #[test]
    fn the_sender_refuses_while_off_or_busy() {
        let mut s = Cp16Sender {
            rate: 48_000.0,
            ..Cp16Sender::default()
        };
        assert_eq!(s.send("CQ"), Err(TxError::Off));
        s.transmit = true;
        assert_eq!(s.send("CQ"), Ok(()));
        assert_eq!(s.send("CQ"), Err(TxError::Busy));
        s.burst = None;
        s.spacing = 50.0;
        assert_eq!(s.send("CQ"), Err(TxError::Spacing));
        s.spacing = 15.0;
        s.rate = 12_000.0;
        assert_eq!(s.send("CQ"), Err(TxError::Rate));
    }

    #[test]
    fn a_transmission_at_2_msps_is_made_a_piece_at_a_time() {
        let p = TxParams {
            char_s: 0.5,
            spacing_hz: 15.0,
            base_hz: 1000.0,
            lower: false,
        };
        let mut src = TxSource::new(
            transmission16("CQ", &p, &Cells::new(), 1).unwrap(),
            2e6,
            false,
        );
        let mut piece = vec![Complex32::default(); 65536];
        let mut peak = 0f32;
        loop {
            let n = src.next_into(&mut piece);
            peak = piece[..n].iter().map(|c| c.norm()).fold(peak, f32::max);
            if n < piece.len() {
                break;
            }
        }
        // 0.5 s of silence and 2 characters of 0.5 s.
        assert!(
            (src.produced() as f64 - 1.5 * 2e6).abs() < 2000.0,
            "{}",
            src.produced()
        );
        assert!(peak < 1.0 && peak > 0.05, "{peak}");
        // A stop ends after the row under way.
        let row = p.row_s();
        assert_eq!(stop_after_row(0, 2e6, row), (LEAD_S * 2e6) as u64);
        let at = (LEAD_S * 2e6 + 2.5 * row * 2e6) as u64;
        assert_eq!(
            stop_after_row(at, 2e6, row),
            (LEAD_S * 2e6 + 3.0 * row * 2e6).round() as u64
        );
    }
}
