//! ISCAT: 42-FSK in frames of 24 symbols, four Costas sync symbols and two length symbols each; the message is folded over all
//! frames. Two variants: ISCAT-A (21.5 baud) and ISCAT-B (43.1 baud).
//!
//! Standard: WSJT-X user guide, section ISCAT.

use super::{Board, Kept, ScatterDecode, Shown, GEN_TEXT, TEST_TEXT};
use crate::front::AUDIO_RATE;
use crate::typed::{IscatDecodeEvent, IscatDecodeRow, IscatFields, ISCAT_DECODE, ISCAT_DECODES};
use num_complex::Complex32;
use sdrkit_algo_modem::tonegrid::{Fold, GridSpec, GridWindow, ToneGrid};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published, TableDef};
use sdrkit_core::{
    Choice, ConfigError, Event, GenConfig, Generator, Hz, LockState, Schema, Setting, SettingError,
    SettingId, SettingSet, SignalInfo, StackConfig, Value, Values,
};
use sdrkit_graph::{values_of, Format, Outbox, Port, RunContext};
use std::f64::consts::TAU;
use std::sync::Arc;

/// Tones.
pub const TONES: usize = 42;
/// Symbols in a frame.
pub const FRAME: usize = 24;
/// The Costas sync tones of symbols 0 to 3.
pub const COSTAS: [usize; 4] = [0, 1, 3, 2];
/// Characters in a frame.
pub const DATA: usize = 18;
/// The character set: tone n sends character n.
pub const CHARSET: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ /.?@-";
/// The tone of `@`, which starts the sent string.
pub const AT: usize = 40;

/// A variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// ISCAT-A: 512 samples at 11025 Hz a symbol.
    A,
    /// ISCAT-B: 256 samples at 11025 Hz a symbol.
    B,
}

impl Variant {
    /// The symbol length, seconds.
    pub fn symbol_s(self) -> f64 {
        match self {
            Variant::A => 512.0 / 11025.0,
            Variant::B => 256.0 / 11025.0,
        }
    }
    /// The tone spacing, Hz.
    pub fn spacing_hz(self) -> f64 {
        1.0 / self.symbol_s()
    }
    /// The frequency of tone 0, Hz: 47 spacings (A) or 13 spacings (B).
    pub fn tone0_hz(self) -> f64 {
        match self {
            Variant::A => 47.0 * self.spacing_hz(),
            Variant::B => 13.0 * self.spacing_hz(),
        }
    }
    /// Its name.
    pub fn name(self) -> &'static str {
        match self {
            Variant::A => "ISCAT-A",
            Variant::B => "ISCAT-B",
        }
    }
}

sdrkit_core::choice_enum! {
    /// The variant setting.
    pub enum Submode {
        /// Both variants are tried; the stronger sync wins.
        Auto = ("auto", "Automatic"),
        /// ISCAT-A.
        A = ("a", "ISCAT-A"),
        /// ISCAT-B.
        B = ("b", "ISCAT-B"),
    }
}

/// The tone of a character (lower case folded); a character outside the set is sent as a space.
pub fn tone_of(c: char) -> usize {
    CHARSET.find(c.to_ascii_uppercase()).unwrap_or(36)
}

/// The first `n` symbols (tones) of the transmission of `message`.
pub fn symbols(message: &str, n: usize) -> Vec<usize> {
    let sent: Vec<usize> = std::iter::once(AT)
        .chain(message.chars().map(tone_of))
        .collect();
    let l = sent.len();
    let mut out = Vec::with_capacity(n);
    let mut next = 0usize;
    for k in 0..n {
        let i = k % FRAME;
        out.push(match i {
            0..=3 => COSTAS[i],
            4 => l % TONES,
            5 => (l + 5) % TONES,
            _ => {
                let t = sent[next % l];
                next += 1;
                t
            }
        });
    }
    out
}

/// What the reader found.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    /// The variant.
    pub variant: Variant,
    /// The message (without the `@`).
    pub text: String,
    /// Signal time of the first whole frame, seconds from the start of the audio.
    pub start_s: f64,
    /// The offset of tone 0 from its nominal frequency, Hz.
    pub df_hz: f64,
    /// The sync ratio r: the geometric mean of the Costas tones over the symbol means, minus 1.
    pub sync: f64,
    /// Whole frames used.
    pub frames: usize,
    /// Characters shown as `?`.
    pub unclear: usize,
}

impl Reading {
    /// The SNR: 10·log10(r − 1), dB (not in 2500 Hz).
    pub fn snr_db(&self) -> f64 {
        10.0 * (self.sync - 1.0).max(1e-3).log10()
    }
}

/// Reads audio at `rate` as `variant` with the tone-0 tolerance ±`tol_hz`. `None` when there is no sync, no length, no `@` or
/// too many unclear characters.
pub fn read(x: &[f32], rate: f64, variant: Variant, tol_hz: f64) -> Option<Reading> {
    let sym = variant.symbol_s() * rate;
    if (x.len() as f64) < 49.0 * sym {
        return None;
    }
    let sp = variant.spacing_hz();
    let qs = sp / 4.0;
    let shifts = (2.0 * tol_hz / qs).floor() as usize + 1;
    let f0 = variant.tone0_hz() - tol_hz;
    let spec = GridSpec {
        rate,
        symbol: sym,
        pad: 4,
        window: GridWindow::Hann,
        step: sym / 4.0,
        f_lo: (f0 - sp).max(0.0),
        f_hi: (f0 + 2.0 * tol_hz + (TONES as f64 + 1.0) * sp).min(rate / 2.0),
    };
    let grid = ToneGrid::new(x, &spec).ok()?;
    let cols = grid.columns();
    let mut best: Option<(f64, usize, usize, usize, f64, usize)> = None;
    let mut e = Vec::new();
    let mut means = Vec::new();
    for q in 0..4 {
        let nsym = cols.saturating_sub(q).div_ceil(4);
        for s in 0..shifts {
            energies(&grid, q, nsym, f0 + s as f64 * qs, sp, &mut e, &mut means);
            for j in 0..FRAME {
                let frames = nsym.saturating_sub(j) / FRAME;
                if frames < 2 {
                    continue;
                }
                let mut acc = 0.0f64;
                for m in 0..frames {
                    for (i, &t) in COSTAS.iter().enumerate() {
                        let k = j + m * FRAME + i;
                        acc += f64::from(1.0 + e[k * TONES + t] / means[k]).ln();
                    }
                }
                let r = (acc / (4 * frames) as f64).exp() - 1.0;
                let score = (r - 1.0) * (frames as f64).sqrt();
                if best.is_none_or(|b| score > b.0) {
                    best = Some((score, q, s, j, r, frames));
                }
            }
        }
    }
    let (_, q, s, j, r, frames) = best?;
    if r < 2.2 || frames < 2 {
        return None;
    }
    let nsym = cols.saturating_sub(q).div_ceil(4);
    energies(&grid, q, nsym, f0 + s as f64 * qs, sp, &mut e, &mut means);
    let ratio = |k: usize, t: usize| e[k * TONES + t] / means[k];
    // The length.
    let mut len = (f32::MIN, 0usize);
    for l in 2..TONES {
        let sum: f32 = (0..frames)
            .map(|m| {
                let k = j + m * FRAME;
                ratio(k + 4, l % TONES) + ratio(k + 5, (l + 5) % TONES)
            })
            .sum();
        if sum > len.0 {
            len = (sum, l);
        }
    }
    if len.0 < 3.0 * frames as f32 {
        return None;
    }
    let l = len.1;
    // The fold.
    let mut fold = Fold::new(l, TONES);
    let mut v = [0f32; TONES];
    for m in 0..frames {
        for i in 0..DATA {
            let k = j + m * FRAME + 6 + i;
            for (t, x) in v.iter_mut().enumerate() {
                *x = ratio(k, t);
            }
            fold.add(DATA * m + i, &v);
        }
    }
    let conf = |p: usize, t: usize| {
        let sums = fold.sums(p);
        let others = (sums.iter().sum::<f32>() - sums[t]) / (TONES - 1) as f32;
        sums[t] / others.max(1e-12)
    };
    let at = (0..l).max_by(|&a, &b| fold.sums(a)[AT].total_cmp(&fold.sums(b)[AT]))?;
    if conf(at, AT) < 3.0 {
        return None;
    }
    let mut text = String::new();
    let mut unclear = 0;
    for k in 1..l {
        let p = (at + k) % l;
        let (t, _, _) = fold.best(p)?;
        if conf(p, t) < 3.0 {
            text.push('?');
            unclear += 1;
        } else {
            text.push(CHARSET.as_bytes()[t] as char);
        }
    }
    if unclear * 5 > 2 * (l - 1) {
        return None;
    }
    let first = q + 4 * j;
    Some(Reading {
        variant,
        text,
        start_s: grid.start_of(first) as f64 / rate,
        df_hz: s as f64 * qs - tol_hz,
        sync: r,
        frames,
        unclear,
    })
}

/// The 42 tone energies of each symbol of symbol phase `q` (column `q + 4k`), tone 0 at `f0`, and each symbol's mean plus a
/// floor of 5 % of the median of the means.
fn energies(
    grid: &ToneGrid,
    q: usize,
    nsym: usize,
    f0: f64,
    sp: f64,
    e: &mut Vec<f32>,
    means: &mut Vec<f32>,
) {
    e.clear();
    means.clear();
    for k in 0..nsym {
        let c = q + 4 * k;
        let mut sum = 0.0;
        for t in 0..TONES {
            let p = grid.power(c, f0 + t as f64 * sp);
            e.push(p);
            sum += p;
        }
        means.push(sum / TONES as f32);
    }
    let mut sorted = means.clone();
    let floor = 0.05 * super::median(&mut sorted);
    for m in means.iter_mut() {
        *m = (*m + floor).max(1e-30);
    }
}

/// The variant setting.
pub const SUBMODE: Setting<field::Choice<Submode>> =
    Setting::choice("submode", "Variant", Submode::Auto)
        .help("Automatic tries ISCAT-A and ISCAT-B on each reading and keeps the stronger sync.");
/// The frequency tolerance.
pub const TOL_HZ: Setting<field::Hz> = Setting::new("tol_hz", "Frequency tolerance", "Hz", 100.0)
    .range(25.0, 300.0, 0.0)
    .help("How far tone 0 may lie from its nominal frequency.");

sdrkit_core::settings! {
    /// The settings of [`IscatDecoder`].
    pub enum IscatSetting {
        /// The variant.
        Submode = SUBMODE,
        /// The frequency tolerance.
        Tol = TOL_HZ,
    }
}

/// The values that a decode of ISCAT adds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IscatExtra {
    /// The variant.
    pub variant: &'static str,
    /// The sync ratio r.
    pub sync_ratio: f64,
    /// The whole frames used.
    pub frames: i64,
    /// The characters shown as `?`.
    pub unclear: i64,
}

impl Shown for IscatExtra {
    fn table() -> &'static TableDef {
        &ISCAT_DECODES
    }
    fn row(d: &ScatterDecode<Self>, utc: f64, key: &str, p: &mut Published) {
        ISCAT_DECODES.put(
            p,
            key,
            &IscatDecodeRow {
                utc: Time(utc),
                time_s: d.time_s,
                snr_db: d.snr_db,
                audio_hz: Hertz(d.hz),
                mode: &d.label,
                message: &d.text,
                variant: d.extra.variant,
                sync_ratio: d.extra.sync_ratio,
                frames: d.extra.frames,
                unclear: d.extra.unclear,
            },
        );
    }
    fn event(d: &ScatterDecode<Self>, utc: f64, rf_hz: f64, callsign: Option<&str>) -> Event {
        ISCAT_DECODE.event(
            utc,
            rf_hz,
            &IscatDecodeEvent {
                mode: &d.label,
                text: &d.text,
                snr_db: d.snr_db,
                audio_hz: Hertz(d.hz),
                callsign,
                source: callsign,
                family: Family::Message,
                variant: d.extra.variant,
                sync_ratio: d.extra.sync_ratio,
                frames: d.extra.frames,
                unclear: d.extra.unclear,
            },
        )
    }
}

/// What no public document confirms.
pub const NOTE_TEXT: &str =
    "Not confirmed by a public document: the positions of tone 0 (47 and 13 spacings), the rule \
of the length symbols, the character set and the longest message (41 characters with the @ here).";

/// Audio kept, seconds.
const KEEP_S: f64 = 24.0;
/// A decode every, seconds.
const EVERY_S: f64 = 4.0;
/// Audio needed for the first decode, seconds.
const FIRST_S: f64 = 6.0;

/// The ISCAT receiver: 8 kHz audio in, a row and an event for each new message.
pub struct IscatDecoder {
    submode: Submode,
    tol: f64,
    board: Board<IscatExtra>,
    kept: Kept,
    next: u64,
    last: Option<String>,
    variant: Option<Variant>,
    tuning: Option<f64>,
}

impl Default for IscatDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl IscatDecoder {
    /// A receiver at the default settings.
    pub fn new() -> Self {
        IscatDecoder {
            submode: Submode::Auto,
            tol: TOL_HZ.default,
            board: Board::new(),
            kept: Kept::default(),
            next: (FIRST_S * AUDIO_RATE) as u64,
            last: None,
            variant: None,
            tuning: None,
        }
    }

    fn reset(&mut self) {
        let at = self.kept.end();
        self.kept.reset(at);
        self.next = at + (FIRST_S * AUDIO_RATE) as u64;
    }

    fn decode(&mut self, out: &mut Outbox) {
        let variants: &[Variant] = match self.submode {
            Submode::Auto => &[Variant::A, Variant::B],
            Submode::A => &[Variant::A],
            Submode::B => &[Variant::B],
        };
        let x = self.kept.all();
        let best = variants
            .iter()
            .filter_map(|&v| read(x, AUDIO_RATE, v, self.tol))
            .max_by(|a, b| a.sync.total_cmp(&b.sync));
        let Some(r) = best else {
            return;
        };
        if self.last.as_deref() == Some(r.text.as_str()) {
            return;
        }
        self.last = Some(r.text.clone());
        self.variant = Some(r.variant);
        self.tuning = Some(r.df_hz);
        let t0 = self.kept.first() as f64 / AUDIO_RATE;
        self.board.decode(
            out,
            ScatterDecode {
                time_s: t0 + r.start_s,
                label: r.variant.name().into(),
                text: r.text.clone(),
                hz: (r.df_hz * 10.0).round() / 10.0,
                snr_db: (r.snr_db() * 10.0).round() / 10.0,
                extra: IscatExtra {
                    variant: r.variant.name(),
                    sync_ratio: (r.sync * 100.0).round() / 100.0,
                    frames: r.frames as i64,
                    unclear: r.unclear as i64,
                },
            },
        );
    }
}

impl sdrkit_graph::Decoder for IscatDecoder {
    type Input = f32;
    fn name(&self) -> &'static str {
        "wsjt.iscat"
    }
    fn outputs(&self) -> Vec<Port> {
        super::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        IscatSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match IscatSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            IscatSetting::Submode => self.submode = SUBMODE.read(id, value)?,
            IscatSetting::Tol => self.tol = TOL_HZ.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (IscatSetting::Submode, Value::Choice(self.submode.index())),
            (IscatSetting::Tol, Value::Hz(self.tol)),
        ])
    }
    fn results(&self) -> Published {
        crate::typed::results("iscat")
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
                self.decode(out);
                self.next += (EVERY_S * AUDIO_RATE) as u64;
                let keep = self.kept.end().saturating_sub((KEEP_S * AUDIO_RATE) as u64);
                self.kept.drop_before(keep);
            }
        }
    }
    fn finish(&mut self, out: &mut Outbox) {
        if self.kept.end() > self.next.saturating_sub((EVERY_S * AUDIO_RATE) as u64) {
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
        match (self.variant, self.tuning) {
            (Some(v), Some(t)) => self
                .board
                .set_state(state, format_args!("{}, tuning {t:.0} Hz", v.name())),
            _ => self
                .board
                .set_state(state, format_args!("ISCAT: listening")),
        }
        self.board.status.offset_hz = self.tuning;
        self.board.status.snr_db = self.board.rows().last().map(|d| d.snr_db as f32);
        res.put(&IscatFields {
            variant: self.variant.map(Variant::name),
        });
        self.board.report(res, NOTE_TEXT);
    }
    fn clear(&mut self) {
        self.board.clear();
        self.last = None;
        self.variant = None;
        self.tuning = None;
    }
    /// The variant and the tuning are of the frequency before. The decodes go unless the stack keeps them.
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        if !kept.table(ISCAT_DECODES.key) {
            self.board.clear();
        }
        self.last = None;
        self.variant = None;
        self.tuning = None;
    }
}

/// The variant of the test signal.
pub const GEN_VARIANT: Setting<field::Choice<GenVariant>> =
    Setting::choice("gen_variant", "Test signal variant", GenVariant::A)
        .section(super::GEN_SECTION)
        .advanced();

sdrkit_core::choice_enum! {
    /// The variant of the test signal.
    pub enum GenVariant {
        /// ISCAT-A.
        A = ("a", "ISCAT-A"),
        /// ISCAT-B.
        B = ("b", "ISCAT-B"),
    }
}

sdrkit_core::settings! {
    /// The test-signal settings of the ISCAT stack.
    pub enum IscatGenSetting {
        /// The text sent.
        Text = GEN_TEXT,
        /// The variant.
        Variant = GEN_VARIANT,
    }
}

/// The test signal: the message sent without a break, phase continuous, complex with the dial where
/// [`super::dial_in_samples`] puts it, at unit power.
pub struct IscatGenerator {
    rate: f64,
    dial: f64,
    variant: Variant,
    symbols: Vec<usize>,
    n: usize,
    phase: f64,
}

impl IscatGenerator {
    /// The test signal of `message` as `variant` at `rate`.
    pub fn new(rate: f64, variant: Variant, message: &str) -> Self {
        let l = message.chars().count() + 1;
        // A whole number of messages and frames: the loop is seamless.
        let frames = l / gcd(l, DATA);
        IscatGenerator {
            rate,
            dial: super::dial_in_samples(rate),
            variant,
            symbols: symbols(message, frames * FRAME),
            n: 0,
            phase: 0.0,
        }
    }
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

impl Generator for IscatGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(2500.0)
                .with_symbol_rate(self.variant.spacing_hz())
                .with_power(1.0),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        self.dial
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let sym = self.variant.symbol_s() * self.rate;
        let sp = self.variant.spacing_hz();
        for o in out.iter_mut() {
            let k = (self.n as f64 / sym) as usize % self.symbols.len();
            let f = self.dial + self.variant.tone0_hz() + self.symbols[k] as f64 * sp;
            self.phase = (self.phase + TAU * f / self.rate).rem_euclid(TAU);
            *o = Complex32::new(self.phase.cos() as f32, self.phase.sin() as f32);
            self.n += 1;
        }
        out.len()
    }
}

/// The generator of the stack: [`TEST_TEXT`] or `gen_text`, as the variant of `gen_variant`.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let t = cfg.settings.get(&GEN_TEXT);
    let text = if t.trim().is_empty() { TEST_TEXT } else { t };
    // A submode the host names wins over the setting.
    let v = match (cfg.submode.as_deref(), cfg.settings.get(&GEN_VARIANT)) {
        (Some("b"), _) | (None | Some("auto"), GenVariant::B) => Variant::B,
        _ => Variant::A,
    };
    Ok(Box::new(IscatGenerator::new(cfg.rate_hz, v, text)))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    static CELL: std::sync::OnceLock<Arc<Schema>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| Arc::new(Schema::of(&settings())))
        .clone()
}

/// The settings of the stack, in the order of its schema.
pub fn settings() -> Vec<&'static dyn sdrkit_core::settings::DeclareSetting> {
    super::settings(IscatSetting::SETTINGS, IscatGenSetting::SETTINGS)
}

/// The receiver of the stack. A submode the host names (`a`, `b`, `auto`) wins over the `submode` setting.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    let mut cfg = cfg.clone();
    if let Some(sub) = cfg.submode.as_deref().and_then(Submode::from_key) {
        let schema = schema();
        if let Ok(id) = schema.id(SUBMODE.field.key) {
            cfg.settings.set(id, &Value::Choice(sub.index()))?;
        }
    }
    super::receiver("iscat", IscatDecoder::new(), &cfg)
}

/// The real audio of `seconds` of the transmission of `message` as `variant` at `rate`, tone 0 `df_hz` from its nominal
/// frequency, amplitude `amp`, phase continuous.
pub fn audio(
    message: &str,
    variant: Variant,
    rate: f64,
    seconds: f64,
    df_hz: f64,
    amp: f32,
) -> Vec<f32> {
    let sym = variant.symbol_s() * rate;
    let n = (seconds * rate) as usize;
    let s = symbols(message, (n as f64 / sym) as usize + 1);
    let mut phase = 0.0f64;
    (0..n)
        .map(|i| {
            let k = (i as f64 / sym) as usize;
            let v = amp * phase.cos() as f32;
            phase += TAU * (variant.tone0_hz() + df_hz + s[k] as f64 * variant.spacing_hz()) / rate;
            v
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frames_of_test() {
        let s = symbols("TEST", 48);
        assert_eq!(
            s,
            vec![
                0, 1, 3, 2, 5, 10, 40, 29, 14, 28, 29, 40, 29, 14, 28, 29, 40, 29, 14, 28, 29, 40,
                29, 14, 0, 1, 3, 2, 5, 10, 28, 29, 40, 29, 14, 28, 29, 40, 29, 14, 28, 29, 40, 29,
                14, 28, 29, 40
            ]
        );
        assert!((Variant::A.tone0_hz() - 1012.06).abs() < 0.01);
        assert!((Variant::B.tone0_hz() - 559.86).abs() < 0.01);
        assert_eq!(tone_of('t'), 29);
        assert_eq!(tone_of('&'), 36);
    }
}
