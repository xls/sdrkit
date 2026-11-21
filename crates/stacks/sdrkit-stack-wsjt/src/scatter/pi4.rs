//! PI4: the beacon stack. Four tones, 146 symbols of 1/6 s each carrying a sync bit and a bit of a K = 32 convolutional code
//! of a message of up to 8 characters, sent each minute before a CW identification and a plain carrier.
//!
//! Standard: B. Hansen OZ2M, A. Talbot G4JNT, K. Schmidt DJ5HG, "PI4 - PharusIgnis4 - a digital modulation (MGM) for beacon
//! purposes", section 6.1 (tones, timing, code, interleaver, sync vector). The code and the interleaver: A. Talbot G4JNT, "The
//! WSPR Coding Process".

use super::{Board, Kept, ScatterDecode, Shown, GEN_TEXT};
use crate::front::AUDIO_RATE;
use crate::typed::{Pi4DecodeEvent, Pi4DecodeRow, PI4_DECODE, PI4_DECODES};
use num_complex::Complex32;
use sdrkit_algo_fec::conv::{codes::WSPR_K32, Encoder, Fano, FanoParams};
use sdrkit_algo_modem::tonegrid::{GridSpec, GridWindow, ToneGrid};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published, TableDef};
use sdrkit_core::{
    Choice, ConfigError, Event, GenConfig, Generator, Hz, LockState, Schema, Setting, SettingError,
    SettingId, SettingSet, SignalInfo, Value, Values,
};
use sdrkit_graph::{values_of, Format, Outbox, Port, RunContext};
use sdrkit_util_bits::interleave::Permutation;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The character set: value = index.
pub const CHARSET: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ /";
/// Characters in a message.
pub const CHARS: usize = 8;
/// Bits of the source value.
pub const BITS: usize = 42;
/// Tail bits of the code.
pub const TAIL: usize = 31;
/// Symbols.
pub const SYMBOLS: usize = 146;
/// The symbol length, seconds.
pub const SYMBOL_S: f64 = 1.0 / 6.0;
/// The transmission, seconds.
pub const TX_S: f64 = SYMBOLS as f64 * SYMBOL_S;

/// The sync vector.
pub const SYNC: [u8; SYMBOLS] = [
    0, 0, 1, 0, 0, 1, 1, 1, 1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1,
    1, 0, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 1, 1, 1, 1, 0, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0, 0, 0, 0, 0,
    1, 1, 1, 1, 1, 0, 1, 0, 1, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0,
    0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 0,
    1, 0, 1, 0, 1, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1,
];

/// The tone spacing of tone-spacing factor `k`: k × 12000 / 2048 Hz.
pub fn spacing_hz(k: u32) -> f64 {
    f64::from(k) * 12000.0 / 2048.0
}

/// Why a message cannot be sent.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PackError {
    /// More than 8 characters.
    #[error("a PI4 message has at most 8 characters")]
    TooLong,
    /// A character outside the set.
    #[error("PI4 cannot send {0:?}")]
    BadChar(char),
}

/// The source value of a message: up to 8 characters, padded with spaces, each the next base-38 digit.
///
/// # Errors
///
/// [`PackError`] for a message of more than 8 characters or with a character outside the set.
pub fn pack(text: &str) -> Result<u64, PackError> {
    if text.chars().count() > CHARS {
        return Err(PackError::TooLong);
    }
    let mut n = 0u64;
    for c in text.chars().chain(std::iter::repeat(' ')).take(CHARS) {
        let u = c.to_ascii_uppercase();
        let v = CHARSET.find(u).ok_or(PackError::BadChar(c))?;
        n = n * 38 + v as u64;
    }
    Ok(n)
}

/// The message of a source value: 8 characters, trailing spaces kept.
pub fn unpack(mut n: u64) -> String {
    let mut c = [b' '; CHARS];
    for x in c.iter_mut().rev() {
        *x = CHARSET.as_bytes()[(n % 38) as usize];
        n /= 38;
    }
    String::from_utf8_lossy(&c).into_owned()
}

/// The 42 bits of a source value, most significant first.
pub fn bits_of(n: u64) -> Vec<u8> {
    (0..BITS).rev().map(|i| ((n >> i) & 1) as u8).collect()
}

fn interleaver() -> Permutation {
    Permutation::bit_reverse8(SYMBOLS).expect("146 channel bits fit the 8-bit reversal")
}

/// The 146 coded bits of the source bits, before interleaving.
pub fn code(bits: &[u8]) -> Vec<u8> {
    let mut enc = Encoder::new(WSPR_K32);
    let mut cw = Vec::with_capacity(SYMBOLS);
    enc.encode(bits, &mut cw);
    enc.terminate(&mut cw);
    cw
}

/// The 146 tones (0 to 3) of a message.
///
/// # Errors
///
/// As [`pack`].
pub fn symbols(text: &str) -> Result<Vec<u8>, PackError> {
    let cw = code(&bits_of(pack(text)?));
    let mut il = vec![0u8; SYMBOLS];
    interleaver()
        .apply(&cw, &mut il)
        .expect("the code has 146 bits");
    Ok(SYNC.iter().zip(&il).map(|(&s, &d)| s + 2 * d).collect())
}

/// Decodes 146 soft data bits in transmission order (positive means a coded 1) into the source value and the count of channel
/// bits whose hard decision agrees with the code of the result. `None` when the decoder gives up.
pub fn decode_soft(soft: &[f32]) -> Option<(u64, usize)> {
    if soft.len() != SYMBOLS {
        return None;
    }
    let mut deint = vec![0f32; SYMBOLS];
    interleaver().invert(soft, &mut deint).ok()?;
    // The decoder takes LLRs where positive means 0.
    let llr: Vec<f32> = deint.iter().map(|&v| -v).collect();
    let params = FanoParams {
        delta: 16,
        max_cycles_per_bit: 400_000 / (BITS + TAIL) as u32,
        ..FanoParams::default()
    };
    let mut fano = Fano::new(WSPR_K32, BITS + TAIL, params).ok()?;
    let mut bits = vec![0u8; BITS];
    fano.decode(&llr, &mut bits).ok()?;
    let n = bits.iter().fold(0u64, |n, &b| (n << 1) | u64::from(b));
    let cw = code(&bits);
    let agree = cw
        .iter()
        .zip(&deint)
        .filter(|(&b, &v)| (b == 1) == (v > 0.0))
        .count();
    Some((n, agree))
}

/// What the reader found.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    /// The message, trailing spaces removed.
    pub text: String,
    /// The start of the transmission, seconds from the start of the audio.
    pub start_s: f64,
    /// The frequency of tone 0, Hz.
    pub tone0_hz: f64,
    /// The sync value c.
    pub sync: f64,
    /// SNR in 2500 Hz, dB.
    pub snr_db: f64,
}

/// Reads audio at `rate` for a PI4 transmission of tone-spacing factor `k` with tone 0 between `f_lo` and `f_hi` Hz. `None`
/// when the sync is below 0.2, when one of the four tones holds less than a tenth of the energy of the most used one (a
/// carrier), or when no message decodes with 70 % of its channel bits agreeing.
pub fn read(x: &[f32], rate: f64, k: u32, f_lo: f64, f_hi: f64) -> Option<Reading> {
    let sym = SYMBOL_S * rate;
    if (x.len() as f64) < (SYMBOLS + 1) as f64 * sym {
        return None;
    }
    let sp = spacing_hz(k);
    let spec = GridSpec {
        rate,
        symbol: sym,
        pad: 2,
        window: GridWindow::Rectangular,
        step: sym / 4.0,
        f_lo: 0.0,
        f_hi: (3800.0f64).min(rate / 2.0),
    };
    let grid = ToneGrid::new(x, &spec).ok()?;
    let bin = grid.bin_hz();
    let nb = grid.bins();
    let off = |m: i32| (f64::from(m) * sp / bin).round() as i64;
    let lo_ok = |b: i64| b >= 0 && (b as f64) * bin >= 200.0;
    let hi_ok = |b: i64| (b as usize) < nb && (b as f64) * bin <= 3800.0;
    let b_lo = ((f_lo.max(200.0)) / bin).ceil() as i64;
    let b_hi = ((f_hi.min(3800.0 - 3.0 * sp)) / bin).floor() as i64;
    if b_hi < b_lo {
        return None;
    }
    let cands = (b_hi - b_lo + 1) as usize;
    let cols = grid.columns();
    // For each column and candidate: the energy of tones 0 and 2, of tones 1 and 3, and of the guard tones.
    let mut s02 = vec![0f32; cols * cands];
    let mut s13 = vec![0f32; cols * cands];
    let mut sg = vec![0f32; cols * cands];
    let at = |col: &[f32], b: i64| -> f32 {
        if lo_ok(b) && hi_ok(b) {
            col[b as usize]
        } else {
            0.0
        }
    };
    for c in 0..cols {
        let col = grid.column(c);
        for i in 0..cands {
            let b0 = b_lo + i as i64;
            let j = c * cands + i;
            s02[j] = at(col, b0) + at(col, b0 + off(2));
            s13[j] = at(col, b0 + off(1)) + at(col, b0 + off(3));
            sg[j] = at(col, b0 + off(-2))
                + at(col, b0 + off(-1))
                + at(col, b0 + off(4))
                + at(col, b0 + off(5));
        }
    }
    let starts = cols.saturating_sub(4 * (SYMBOLS - 1));
    let mut best = (f64::MIN, 0usize, 0usize);
    for st in 0..starts {
        for i in 0..cands {
            let (mut a, mut b, mut t) = (0.0f64, 0.0f64, 0.0f64);
            for (n, &s) in SYNC.iter().enumerate() {
                let j = (st + 4 * n) * cands + i;
                let (on, off) = if s == 0 {
                    (s02[j], s13[j])
                } else {
                    (s13[j], s02[j])
                };
                a += f64::from(on);
                b += f64::from(off);
                t += f64::from(sg[j]);
            }
            let c = (a - b) / (a + b + t).max(1e-30);
            if c > best.0 {
                best = (c, st, i);
            }
        }
    }
    let (sync, st, i) = best;
    if sync < 0.2 {
        return None;
    }
    let b0 = b_lo + i as i64;
    let e = |c: usize, m: i32| at(grid.column(c), b0 + off(m));
    // A transmission uses all four tones about equally; a carrier or a keyed carrier at the lock does not.
    let use_of: Vec<f64> = (0..4)
        .map(|m| (0..SYMBOLS).map(|n| f64::from(e(st + 4 * n, m))).sum())
        .collect();
    let most = use_of.iter().copied().fold(0.0, f64::max);
    if use_of.iter().any(|&u| u < 0.1 * most) {
        return None;
    }
    for dst in [0i64, -1, 1] {
        let s0 = st as i64 + dst;
        if s0 < 0 || (s0 as usize + 4 * (SYMBOLS - 1)) >= cols {
            continue;
        }
        let s0 = s0 as usize;
        let d: Vec<f32> = SYNC
            .iter()
            .enumerate()
            .map(|(n, &s)| {
                let c = s0 + 4 * n;
                let s = i32::from(s);
                e(c, s + 2).sqrt() - e(c, s).sqrt()
            })
            .collect();
        let rms = (d.iter().map(|v| v * v).sum::<f32>() / SYMBOLS as f32).sqrt();
        if rms <= 0.0 {
            continue;
        }
        for scale in [3.0f32, 6.0, 1.5] {
            let soft: Vec<f32> = d.iter().map(|v| scale * v / rms).collect();
            let Some((n, agree)) = decode_soft(&soft) else {
                continue;
            };
            if agree * 10 < SYMBOLS * 7 || n >= 38u64.pow(CHARS as u32) {
                continue;
            }
            // The SNR: the sent tone over the noise per bin, in 2500 Hz.
            let mut cells: Vec<f32> = (0..cols)
                .step_by(7)
                .flat_map(|c| grid.column(c).iter().step_by(13).copied())
                .collect();
            let noise = f64::from(super::median(&mut cells)) / 0.69;
            let sig = SYNC
                .iter()
                .enumerate()
                .map(|(n, &s)| {
                    let c = s0 + 4 * n;
                    let s = i32::from(s);
                    f64::from(e(c, s).max(e(c, s + 2)))
                })
                .sum::<f64>()
                / SYMBOLS as f64;
            let snr = ((sig - noise).max(1e-30) / noise.max(1e-30) * bin / 2500.0).log10() * 10.0;
            return Some(Reading {
                text: unpack(n).trim_end().to_owned(),
                start_s: grid.start_of(s0) as f64 / rate,
                tone0_hz: b0 as f64 * bin,
                sync,
                snr_db: snr,
            });
        }
    }
    None
}

sdrkit_core::choice_enum! {
    /// The tone-spacing factor.
    pub enum Spacing {
        /// PI4: 234.375 Hz.
        K40 = ("40", "PI4 (K = 40)"),
        /// PI4-80: 468.75 Hz.
        K80 = ("80", "PI4-80 (K = 80)"),
        /// PI4-96: 562.5 Hz.
        K96 = ("96", "PI4-96 (K = 96)"),
        /// PI4-120: 703.125 Hz.
        K120 = ("120", "PI4-120 (K = 120)"),
    }
}

impl Spacing {
    /// The factor K.
    pub fn k(self) -> u32 {
        match self {
            Spacing::K40 => 40,
            Spacing::K80 => 80,
            Spacing::K96 => 96,
            Spacing::K120 => 120,
        }
    }
}

/// The tone-spacing factor.
pub const K: Setting<field::Choice<Spacing>> = Setting::choice("k", "Tone spacing factor", Spacing::K40)
    .help("PI4, PI4-80, PI4-96 or PI4-120. Which one a beacon uses depends on its band and channel spacing.");
/// The lowest tone-0 frequency searched.
pub const F_LO_HZ: Setting<field::Hz> =
    Setting::new("f_lo_hz", "Lowest tone-0 frequency", "Hz", 400.0).range(200.0, 3000.0, 0.0);
/// The highest tone-0 frequency searched (clipped so tone 3 stays below 3800 Hz).
pub const F_HI_HZ: Setting<field::Hz> =
    Setting::new("f_hi_hz", "Highest tone-0 frequency", "Hz", 2600.0)
        .range(300.0, 3800.0, 0.0)
        .help("Clipped so that tone 3 lies below 3800 Hz.");

sdrkit_core::settings! {
    /// The settings of [`Pi4Decoder`].
    pub enum Pi4Setting {
        /// The tone-spacing factor.
        K = K,
        /// The lowest tone 0.
        FLo = F_LO_HZ,
        /// The highest tone 0.
        FHi = F_HI_HZ,
    }
}

/// The values that a decode of PI4 adds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pi4Extra {
    /// The start against the minute, s.
    pub dt_s: f64,
    /// The carrier frequency: tone 0 plus half a spacing, Hz.
    pub carrier_hz: f64,
    /// The sync value c.
    pub sync: f64,
    /// The tone spacing factor.
    pub k: i64,
}

impl Shown for Pi4Extra {
    fn table() -> &'static TableDef {
        &PI4_DECODES
    }
    fn row(d: &ScatterDecode<Self>, utc: f64, key: &str, p: &mut Published) {
        PI4_DECODES.put(
            p,
            key,
            &Pi4DecodeRow {
                utc: Time(utc),
                time_s: d.time_s,
                snr_db: d.snr_db,
                audio_hz: Hertz(d.hz),
                mode: &d.label,
                message: &d.text,
                dt_s: d.extra.dt_s,
                carrier_hz: d.extra.carrier_hz,
                sync: d.extra.sync,
                k: d.extra.k,
            },
        );
    }
    fn event(d: &ScatterDecode<Self>, utc: f64, rf_hz: f64, callsign: Option<&str>) -> Event {
        PI4_DECODE.event(
            utc,
            rf_hz,
            &Pi4DecodeEvent {
                mode: &d.label,
                text: &d.text,
                snr_db: d.snr_db,
                audio_hz: Hertz(d.hz),
                callsign,
                source: callsign,
                family: Family::Message,
                dt_s: d.extra.dt_s,
                carrier_hz: d.extra.carrier_hz,
                sync: d.extra.sync,
                k: d.extra.k,
            },
        )
    }
}

/// What no public document confirms.
pub const NOTE_TEXT: &str =
    "Every part of the signal follows the published PI4 description. The CW identification and \
the carrier of the minute are not decoded.";

/// The window, seconds.
const WINDOW_S: f64 = 30.0;
/// A decode every, seconds.
const EVERY_S: f64 = 6.0;

/// The PI4 receiver: 8 kHz audio in, a row and an event for each transmission.
pub struct Pi4Decoder {
    spacing: Spacing,
    f_lo: f64,
    f_hi: f64,
    board: Board<Pi4Extra>,
    kept: Kept,
    next: u64,
    last: Option<(String, f64)>,
    tone0: Option<f64>,
}

impl Default for Pi4Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Pi4Decoder {
    /// A receiver at the default settings.
    pub fn new() -> Self {
        Pi4Decoder {
            spacing: Spacing::K40,
            f_lo: F_LO_HZ.default,
            f_hi: F_HI_HZ.default,
            board: Board::new(),
            kept: Kept::default(),
            next: (WINDOW_S * AUDIO_RATE) as u64,
            last: None,
            tone0: None,
        }
    }

    fn reset(&mut self) {
        let at = self.kept.end();
        self.kept.reset(at);
        self.next = at + (WINDOW_S * AUDIO_RATE) as u64;
    }

    fn decode(&mut self, out: &mut Outbox) {
        let k = self.spacing.k();
        let Some(r) = read(self.kept.all(), AUDIO_RATE, k, self.f_lo, self.f_hi) else {
            return;
        };
        let t = self.kept.first() as f64 / AUDIO_RATE + r.start_s;
        // The windows overlap: one transmission is reported once, the next a minute later again.
        if let Some((text, at)) = &self.last {
            if *text == r.text && (t - at).abs() < 30.0 {
                return;
            }
        }
        self.last = Some((r.text.clone(), t));
        self.tone0 = Some(r.tone0_hz);
        let utc = self.board.utc(t);
        let dt = utc - 60.0 * (utc / 60.0).round();
        let sp = spacing_hz(k);
        self.board.decode(
            out,
            ScatterDecode {
                time_s: t,
                label: "PI4".into(),
                text: r.text,
                hz: (r.tone0_hz * 10.0).round() / 10.0,
                snr_db: r.snr_db.round(),
                extra: Pi4Extra {
                    dt_s: (dt * 10.0).round() / 10.0,
                    carrier_hz: ((r.tone0_hz + 0.5 * sp) * 10.0).round() / 10.0,
                    sync: (r.sync * 1000.0).round() / 1000.0,
                    k: i64::from(k),
                },
            },
        );
    }
}

impl sdrkit_graph::Decoder for Pi4Decoder {
    type Input = f32;
    fn name(&self) -> &'static str {
        "wsjt.pi4"
    }
    fn outputs(&self) -> Vec<Port> {
        super::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        Pi4Setting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match Pi4Setting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Pi4Setting::K => self.spacing = K.read(id, value)?,
            Pi4Setting::FLo => self.f_lo = F_LO_HZ.read(id, value)?,
            Pi4Setting::FHi => self.f_hi = F_HI_HZ.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (Pi4Setting::K, Value::Choice(self.spacing.index())),
            (Pi4Setting::FLo, Value::Hz(self.f_lo)),
            (Pi4Setting::FHi, Value::Hz(self.f_hi)),
        ])
    }
    fn results(&self) -> Published {
        crate::typed::results("pi4")
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
        let mut rest = x;
        while !rest.is_empty() {
            let room = (self.next.saturating_sub(self.kept.end()) as usize).max(1);
            let (now, later) = rest.split_at(room.min(rest.len()));
            self.kept.push(now);
            rest = later;
            if self.kept.end() >= self.next {
                let keep = self
                    .kept
                    .end()
                    .saturating_sub((WINDOW_S * AUDIO_RATE) as u64);
                self.kept.drop_before(keep);
                self.decode(out);
                self.next += (EVERY_S * AUDIO_RATE) as u64;
            }
        }
    }
    fn finish(&mut self, out: &mut Outbox) {
        let window = (WINDOW_S * AUDIO_RATE) as u64;
        if self.kept.end() + window > self.next + (EVERY_S * AUDIO_RATE) as u64 {
            self.decode(out);
        }
    }
    fn report(&mut self, res: &mut Published) {
        let total = self.board.total();
        let state = if total > 0 {
            LockState::Locked
        } else {
            LockState::Searching
        };
        match self.tone0 {
            Some(f) => self
                .board
                .set_state(state, format_args!("PI4 at {f:.0} Hz")),
            None => self.board.set_state(state, format_args!("PI4: listening")),
        }
        self.board.status.snr_db = self.board.rows().last().map(|d| d.snr_db as f32);
        self.board.report(res, NOTE_TEXT);
    }
    fn clear(&mut self) {
        self.board.clear();
        self.last = None;
        self.tone0 = None;
    }
    /// The tone is of the frequency before. The decodes go unless the stack keeps them.
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        if !kept.table(PI4_DECODES.key) {
            self.board.clear();
        }
        self.last = None;
        self.tone0 = None;
    }
}

/// The text of the test signal.
pub const TEST_TEXT: &str = "K0TST";
/// The carrier of the test signal: tone 0 lies half a spacing below it, Hz.
pub const GEN_CARRIER_HZ: f64 = 800.0;
/// The CW FSK space of the test signal, Hz from the carrier.
pub const GEN_CW_SPACE_HZ: f64 = -250.0;

/// The tone-spacing factor of the test signal.
pub const GEN_K: Setting<field::Choice<Spacing>> =
    Setting::choice("gen_k", "Test signal tone spacing factor", Spacing::K40)
        .section(super::GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of the PI4 stack.
    pub enum Pi4GenSetting {
        /// The text sent.
        Text = GEN_TEXT,
        /// The tone-spacing factor.
        K = GEN_K,
    }
}

/// The parts of the minute of a PI4 beacon.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Part {
    Silent,
    Tone(f64),
}

/// The test signal: each minute the message (0 to 24.333 s), a pause to 25 s, the CW identification of the text at 12 words a
/// minute keyed as FSK with the space 250 Hz below the carrier, the carrier from 0.5 s after it until 59.5 s, a pause to 60 s.
/// Complex with the dial where [`super::dial_in_samples`] puts it, at unit power, phase continuous.
pub struct Pi4Generator {
    rate: f64,
    dial: f64,
    sp: f64,
    tones: Vec<u8>,
    cw: Vec<bool>,
    n: usize,
    phase: f64,
}

impl Pi4Generator {
    /// The test signal of `text` with factor `k` at `rate`.
    ///
    /// # Errors
    ///
    /// [`PackError`] for a text PI4 cannot send.
    pub fn new(rate: f64, k: u32, text: &str) -> Result<Self, PackError> {
        let mut cw = Vec::new();
        for (wi, word) in text.split_whitespace().enumerate() {
            if wi > 0 {
                cw.extend([false; 4]);
            }
            for (ci, c) in word.chars().enumerate() {
                if ci > 0 {
                    cw.extend([false; 2]);
                }
                let s = c.to_ascii_uppercase().to_string();
                let Ok(code) = sdrkit_util_text::morse::encode(&s) else {
                    continue;
                };
                for (i, dash) in code.elements().enumerate() {
                    if i > 0 {
                        cw.push(false);
                    }
                    cw.extend(std::iter::repeat_n(true, if dash { 3 } else { 1 }));
                }
            }
        }
        Ok(Pi4Generator {
            rate,
            dial: super::dial_in_samples(rate),
            sp: spacing_hz(k),
            tones: symbols(text)?,
            cw,
            n: 0,
            phase: 0.0,
        })
    }

    fn part(&self, t: f64) -> Part {
        let carrier = GEN_CARRIER_HZ;
        if t < TX_S {
            let s = self.tones[((t / SYMBOL_S) as usize).min(SYMBOLS - 1)];
            return Part::Tone(carrier + (f64::from(s) - 0.5) * self.sp);
        }
        let cw0 = 25.0;
        let dot = 0.1;
        let cw_end = cw0 + self.cw.len() as f64 * dot;
        if t < cw0 {
            Part::Silent
        } else if t < cw_end {
            let mark = self.cw[((t - cw0) / dot) as usize];
            Part::Tone(if mark {
                carrier
            } else {
                carrier + GEN_CW_SPACE_HZ
            })
        } else if t < cw_end + 0.5 {
            Part::Silent
        } else if t < 59.5 {
            Part::Tone(carrier)
        } else {
            Part::Silent
        }
    }
}

impl Generator for Pi4Generator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(2500.0)
                .with_symbol_rate(1.0 / SYMBOL_S)
                .with_power(1.0),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        self.dial
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let minute = (60.0 * self.rate) as usize;
        for o in out.iter_mut() {
            let t = (self.n % minute) as f64 / self.rate;
            self.n += 1;
            *o = match self.part(t) {
                Part::Silent => Complex32::default(),
                Part::Tone(f) => {
                    self.phase = (self.phase + TAU * (self.dial + f) / self.rate).rem_euclid(TAU);
                    Complex32::new(self.phase.cos() as f32, self.phase.sin() as f32)
                }
            };
        }
        out.len()
    }
}

/// The generator of the stack: [`TEST_TEXT`] or `gen_text` (a text PI4 cannot send makes the test text).
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let t = cfg.settings.get(&GEN_TEXT);
    let text = if t.trim().is_empty() || pack(t.trim()).is_err() {
        TEST_TEXT
    } else {
        t.trim()
    };
    let k = cfg.settings.get(&GEN_K).k();
    let g = Pi4Generator::new(cfg.rate_hz, k, text)
        .or_else(|_| Pi4Generator::new(cfg.rate_hz, k, TEST_TEXT))
        .expect("the test text packs");
    Ok(Box::new(g))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    static CELL: std::sync::OnceLock<Arc<Schema>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| Arc::new(Schema::of(&settings())))
        .clone()
}

/// The settings of the stack, in the order of its schema.
pub fn settings() -> Vec<&'static dyn sdrkit_core::settings::DeclareSetting> {
    super::settings(Pi4Setting::SETTINGS, Pi4GenSetting::SETTINGS)
}

/// The receiver of the stack.
pub fn make_receiver(
    cfg: &sdrkit_core::StackConfig,
) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    super::receiver("pi4", Pi4Decoder::new(), cfg)
}

/// The real audio of one transmission of `text` with factor `k` at `rate`, tone 0 at `tone0_hz`, amplitude `amp`, phase
/// continuous.
///
/// # Errors
///
/// As [`pack`].
pub fn audio(
    text: &str,
    k: u32,
    rate: f64,
    tone0_hz: f64,
    amp: f32,
) -> Result<Vec<f32>, PackError> {
    let s = symbols(text)?;
    let sym = SYMBOL_S * rate;
    let n = (SYMBOLS as f64 * sym).round() as usize;
    let sp = spacing_hz(k);
    let mut phase = 0.0f64;
    Ok((0..n)
        .map(|i| {
            let j = ((i as f64 / sym) as usize).min(SYMBOLS - 1);
            let v = amp * phase.cos() as f32;
            phase += TAU * (tone0_hz + f64::from(s[j]) * sp) / rate;
            v
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digits(s: &[u8]) -> String {
        s.iter().map(|d| char::from(b'0' + d)).collect()
    }

    #[test]
    fn the_published_example() {
        assert_eq!(pack("OZ7IGY"), Ok(2_851_949_862_724));
        let s = symbols("OZ7IGY").unwrap();
        assert_eq!(
            digits(&s[..40]),
            "2 0 1 0 0 3 3 3 3 2 3 2 1 2 1 2 0 3 2 2 0 3 2 2 0 1 1 0 0 1 3 1 3 0 2 1 1 3 3 1"
                .replace(' ', "")
        );
    }

    #[test]
    fn every_symbol_of_two_messages() {
        assert_eq!(
            digits(&symbols("OZ7IGY").unwrap()),
            "20100333323212120322032201100131302113312013213332123121103202001331323230200213331230030232102021001102022332222310013301312130303012202313200211"
        );
        let n = pack("PI4ABC").unwrap();
        assert_eq!(n, 2_914_924_834_068);
        assert_eq!(n, 0x2A6_AF0F_2D14);
        let cw = code(&bits_of(n));
        assert_eq!(digits(&cw[..32]), "11011001000100110110100000001100");
        assert_eq!(
            digits(&symbols("PI4ABC").unwrap()),
            "20100131103030100100032221300311322113330231013310301303321220201131121210220033311210010230122023201322202332220332013123310310303032222333200211"
        );
    }

    #[test]
    fn packing() {
        assert_eq!(unpack(pack("PI4ABC/P").unwrap()), "PI4ABC/P");
        assert_eq!(unpack(pack("k1jt").unwrap()), "K1JT    ");
        assert_eq!(pack("TOOLONGMSG"), Err(PackError::TooLong));
        assert_eq!(pack("A-B"), Err(PackError::BadChar('-')));
        assert_eq!(38u64.pow(8), 4_347_792_138_496);
        assert!(38u64.pow(8) < 1 << 42);
    }

    #[test]
    fn random_messages_round_trip_through_the_code() {
        let mut x = 0x1234_5678_9abc_u64;
        for _ in 0..50 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let n = x % 38u64.pow(8);
            let mut il = vec![0u8; SYMBOLS];
            interleaver().apply(&code(&bits_of(n)), &mut il).unwrap();
            let soft: Vec<f32> = il
                .iter()
                .map(|&b| if b == 1 { 3.0 } else { -3.0 })
                .collect();
            assert_eq!(decode_soft(&soft), Some((n, SYMBOLS)));
        }
    }
}
