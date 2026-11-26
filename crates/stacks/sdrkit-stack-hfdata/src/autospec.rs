//! AUTOSPEC and SPREAD: FEC teleprinter broadcasts on 2-FSK. Each ITA2 character is a 10-unit word of the Bauer code (the five
//! elements, then five check units that repeat them for an odd character and invert them for an even one), so one error is
//! corrected and two are detected. SPREAD sends unit `k` of character `n` at stream position `10 n + k S`, so `S - 1` units of other
//! characters follow each unit; the depth `S` (11, 21 or 51) is found. AUTOSPEC is the same with `S = 1`.
//!
//! Standard: ITU-R SM.1052 Annex 1 names AUTOSPEC; no public document gives the code, the speeds or the SPREAD layout.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [autospec: 8 kHz, 2-FSK front, lock (depth, polarity, offset), Bauer decoder] -> text
//! ```

use crate::receiver::GEN_SECTION;
use crate::teleprinter::{
    ita2_codes, put_ita2, Alphabet, FrontLock, FrontSpec, Layer, Polarity, Sink, Symbols,
    ToneSeqGen, TtyDecoder, ALPHABET, POLARITY, RATE,
};
use sdrkit_algo_fec::bauer;
use sdrkit_algo_modem::fskauto::BinaryFskConfig;
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, Choice, ConfigError, GenConfig, Generator, LockProfile, Receiver, Schema, Setting,
    SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, SubMode, Value, Values,
};
use sdrkit_graph::values_of;
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// The submode.
    pub enum Submode {
        /// AUTOSPEC: the units of a word one after the other.
        Autospec = ("autospec", "AUTOSPEC"),
        /// SPREAD: the units of a word spread over 11, 21 or 51 word places.
        Spread = ("spread", "SPREAD"),
    }
}

sdrkit_core::choice_enum! {
    /// The SPREAD depth.
    pub enum Depth {
        /// Found from the signal.
        Auto = ("auto", "Automatic"),
        /// 11 word places.
        D11 = ("11", "11"),
        /// 21 word places.
        D21 = ("21", "21"),
        /// 51 word places.
        D51 = ("51", "51"),
    }
}

impl Depth {
    fn value(self) -> Option<usize> {
        match self {
            Depth::Auto => None,
            Depth::D11 => Some(11),
            Depth::D21 => Some(21),
            Depth::D51 => Some(51),
        }
    }
}

/// The submode.
pub const SUBMODE: Setting<field::Choice<Submode>> =
    Setting::choice("submode", "Signal", Submode::Autospec);
/// The speed; 0 finds it.
pub const BAUD: Setting<field::F64> = Setting::new("baud", "Speed", "Bd", 0.0)
    .range(0.0, 300.0, 0.01)
    .help("0 finds it from the candidates of the submode.");
/// The shift; 0 finds it.
pub const SHIFT: Setting<field::Hz> = Setting::new("shift_hz", "Shift", "Hz", 0.0)
    .range(0.0, 1200.0, 1.0)
    .help("0 finds it from the signal.");
/// The SPREAD depth.
pub const DEPTH: Setting<field::Choice<Depth>> =
    Setting::choice("depth", "Spread depth", Depth::Auto).help("SPREAD only.");
/// The idle character.
pub const IDLE_CODE: Setting<field::I64> = Setting::new("idle_code", "Idle character", "", 21)
    .range(0.0, 31.0, 1.0)
    .help("The ITA2 code sent while idle; a run of three or more is not shown. Not confirmed by a public document.")
    .advanced();
/// The speed of the test signal.
pub const GEN_BAUD: Setting<field::F64> = Setting::new("gen_baud", "Speed", "Bd", 68.5)
    .range(10.0, 300.0, 0.01)
    .section(GEN_SECTION)
    .advanced();
/// The shift of the test signal; 0 sends 270 Hz for AUTOSPEC and 410 Hz for SPREAD.
pub const GEN_SHIFT_HZ: Setting<field::Hz> = Setting::new("gen_shift_hz", "Shift", "Hz", 0.0)
    .range(0.0, 1100.0, 1.0)
    .help("0 sends 270 Hz for AUTOSPEC and 410 Hz for SPREAD.")
    .section(GEN_SECTION)
    .advanced();
/// The SPREAD depth of the test signal.
pub const GEN_DEPTH: Setting<field::Choice<Depth>> =
    Setting::choice("gen_depth", "Spread depth", Depth::D11)
        .help("Automatic sends 11.")
        .section(GEN_SECTION)
        .advanced();
/// The centre of the test signal.
pub const GEN_CENTRE_HZ: Setting<field::Hz> = Setting::new("gen_centre_hz", "Centre", "Hz", 1500.0)
    .range(400.0, 3500.0, 1.0)
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the AUTOSPEC receiver.
    pub enum AutospecRxSetting {
        /// Submode.
        Submode = SUBMODE,
        /// Speed.
        Baud = BAUD,
        /// Shift.
        Shift = SHIFT,
        /// Polarity.
        Polarity = POLARITY,
        /// Spread depth.
        Depth = DEPTH,
        /// Figures case.
        Alphabet = ALPHABET,
        /// Idle character.
        IdleCode = IDLE_CODE,
    }
}

sdrkit_core::settings! {
    /// The settings of the AUTOSPEC stack.
    pub enum AutospecSetting {
        /// Submode.
        Submode = SUBMODE,
        /// Speed.
        Baud = BAUD,
        /// Shift.
        Shift = SHIFT,
        /// Polarity.
        Polarity = POLARITY,
        /// Spread depth.
        Depth = DEPTH,
        /// Figures case.
        Alphabet = ALPHABET,
        /// Idle character.
        IdleCode = IDLE_CODE,
        /// Speed of the test signal.
        GenBaud = GEN_BAUD,
        /// Shift of the test signal.
        GenShiftHz = GEN_SHIFT_HZ,
        /// Spread depth of the test signal.
        GenDepth = GEN_DEPTH,
        /// Centre of the test signal.
        GenCentreHz = GEN_CENTRE_HZ,
    }
}

/// The candidate bauds of AUTOSPEC.
pub const AUTOSPEC_BAUDS: [f64; 4] = [62.3, 68.5, 102.63, 137.0];
/// The candidate bauds of SPREAD.
pub const SPREAD_BAUDS: [f64; 4] = [68.5, 102.7, 137.0, 218.0];

/// What no public document confirms.
pub const NOTE_TEXT: &str = "No public document gives AUTOSPEC or SPREAD beyond their names: not confirmed are the Bauer code \
rule (repeat the elements for an odd character, invert them for an even one), the speeds of both submodes, the SPREAD layout \
(unit k of character n at stream position 10 n + k S, inferred from a description), the idle character (a setting) and the \
US figures case.";

/// Words tested by a lock.
const TEST_WORDS: usize = 90;

/// The stream position of unit `k` of character `n` at depth `s`.
pub fn position(n: usize, k: usize, s: usize) -> usize {
    10 * n + k * s
}

/// The AUTOSPEC layer.
pub struct Autospec {
    submode: Submode,
    baud: f64,
    shift: f64,
    polarity: Polarity,
    depth: Depth,
    alphabet: Alphabet,
    idle: i64,
    lock_info: Option<FrontLock>,
    bits: Vec<u8>,
    lock: Option<(usize, bool)>,
    err: f64,
    figs: bool,
    idle_run: usize,
}

impl Default for Autospec {
    fn default() -> Self {
        Self::new()
    }
}

impl Autospec {
    /// The layer at the default settings.
    pub fn new() -> Self {
        Autospec {
            submode: Submode::Autospec,
            baud: 0.0,
            shift: 0.0,
            polarity: Polarity::Auto,
            depth: Depth::Auto,
            alphabet: Alphabet::Us,
            idle: 21,
            lock_info: None,
            bits: Vec::new(),
            lock: None,
            err: 0.0,
            figs: false,
            idle_run: 0,
        }
    }

    fn depths(&self) -> Vec<usize> {
        match (self.submode, self.depth.value()) {
            (Submode::Autospec, _) => vec![1],
            (Submode::Spread, Some(d)) => vec![d],
            (Submode::Spread, None) => vec![11, 21, 51],
        }
    }

    /// Word `n` at offset `off` and depth `s`, unit 1 in bit 0.
    fn word(&self, off: usize, n: usize, s: usize, inv: bool) -> u16 {
        (0..10).fold(0u16, |w, k| {
            w | (u16::from(self.bits[off + position(n, k, s)] ^ u8::from(inv)) << k)
        })
    }

    fn need(&self) -> usize {
        10 * TEST_WORDS + 9 * self.depths().into_iter().max().unwrap_or(1) + 12
    }

    fn try_lock(&self) -> Option<(usize, bool, usize, f64)> {
        let mut best: Option<(usize, bool, usize, f64, Vec<u8>)> = None;
        for s in self.depths() {
            for &inv in self.polarity.tries() {
                for off in 0..10 {
                    let mut ok = 0;
                    let mut chars = Vec::with_capacity(TEST_WORDS);
                    for n in 0..TEST_WORDS {
                        let w = self.word(off, n, s, inv);
                        if bauer::consistent(w) {
                            ok += 1;
                        }
                        chars.push((w & 31) as u8);
                    }
                    let f = f64::from(ok) / TEST_WORDS as f64;
                    if best.as_ref().is_none_or(|b| f > b.3) {
                        best = Some((s, inv, off, f, chars));
                    }
                }
            }
        }
        let (s, inv, off, f, chars) = best?;
        if f < 0.8 {
            return None;
        }
        let mut counts = [0usize; 32];
        for c in chars {
            counts[usize::from(c)] += 1;
        }
        let distinct = counts.iter().filter(|&&c| c > 0).count();
        let most = counts.iter().copied().max().unwrap_or(0);
        (distinct >= 10 && most * 10 <= 6 * TEST_WORDS).then_some((s, inv, off, f))
    }

    fn label(&self, s: usize) -> String {
        if self.submode == Submode::Autospec {
            "Mk 1".to_string()
        } else {
            format!("SPREAD {s}")
        }
    }

    fn put(&mut self, code: u8, sink: &mut Sink) {
        if i64::from(code) == self.idle {
            self.idle_run += 1;
            return;
        }
        if self.idle_run > 0 && self.idle_run <= 2 {
            for _ in 0..self.idle_run {
                put_ita2(
                    self.alphabet.table(),
                    self.idle as u8,
                    &mut self.figs,
                    &mut sink.chars,
                );
            }
        }
        self.idle_run = 0;
        put_ita2(self.alphabet.table(), code, &mut self.figs, &mut sink.chars);
    }
}

impl Layer for Autospec {
    fn name(&self) -> &'static str {
        "hfdata.autospec"
    }
    fn schema(&self) -> Arc<Schema> {
        AutospecRxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (
                AutospecRxSetting::Submode,
                Value::Choice(self.submode.index()),
            ),
            (AutospecRxSetting::Baud, Value::F64(self.baud)),
            (AutospecRxSetting::Shift, Value::Hz(self.shift)),
            (
                AutospecRxSetting::Polarity,
                Value::Choice(self.polarity.index()),
            ),
            (AutospecRxSetting::Depth, Value::Choice(self.depth.index())),
            (
                AutospecRxSetting::Alphabet,
                Value::Choice(self.alphabet.index()),
            ),
            (AutospecRxSetting::IdleCode, Value::I64(self.idle)),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<bool, SettingError> {
        match AutospecRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AutospecRxSetting::Submode => self.submode = SUBMODE.read(id, value)?,
            AutospecRxSetting::Baud => self.baud = BAUD.read(id, value)?,
            AutospecRxSetting::Shift => self.shift = SHIFT.read(id, value)?,
            AutospecRxSetting::Polarity => self.polarity = POLARITY.read(id, value)?,
            AutospecRxSetting::Depth => self.depth = DEPTH.read(id, value)?,
            AutospecRxSetting::Alphabet => {
                self.alphabet = ALPHABET.read(id, value)?;
                return Ok(false);
            }
            AutospecRxSetting::IdleCode => {
                self.idle = IDLE_CODE.read(id, value)?;
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn front(&self) -> FrontSpec {
        let bauds: &[f64] = match self.submode {
            Submode::Autospec => &AUTOSPEC_BAUDS,
            Submode::Spread => &SPREAD_BAUDS,
        };
        let mut c = BinaryFskConfig::new(RATE, bauds);
        c.baud = (self.baud >= 10.0).then_some(self.baud);
        c.shift_hz = (self.shift >= 50.0).then_some(self.shift);
        FrontSpec::Fsk(c)
    }
    fn note(&self) -> &'static str {
        NOTE_TEXT
    }
    fn front_locked(&mut self, lock: FrontLock) {
        self.lock_info = Some(lock);
    }
    fn symbols(&mut self, s: Symbols<'_>, sink: &mut Sink) {
        let Symbols::Soft(soft) = s else { return };
        self.bits.extend(soft.iter().map(|&v| u8::from(v > 0.0)));
        loop {
            match self.lock {
                None => {
                    if self.bits.len() < self.need() {
                        return;
                    }
                    match self.try_lock() {
                        Some((s, inv, off, f)) => {
                            self.bits.drain(..off);
                            self.lock = Some((s, inv));
                            self.err = 0.0;
                            self.figs = false;
                            self.idle_run = 0;
                            let (b, sh) =
                                self.lock_info.map_or((0.0, 0.0), |l| (l.baud(), l.shift()));
                            let label = self.label(s);
                            let name = if self.submode == Submode::Autospec {
                                "AUTOSPEC".to_string()
                            } else {
                                label.clone()
                            };
                            sink.lock(
                                format!(
                                    "{name} sync: {b:.1} Bd, shift {sh:.0} Hz ({:.0} % of the code words consistent)",
                                    100.0 * f
                                ),
                                label,
                            );
                        }
                        None => {
                            self.bits.drain(..10);
                        }
                    }
                }
                Some((s, inv)) => {
                    if self.bits.len() < position(0, 9, s) + 1 {
                        return;
                    }
                    let w = self.word(0, 0, s, inv);
                    self.bits.drain(..10);
                    let (code, d) = bauer::decode(w);
                    if d <= 1 {
                        self.err *= 0.97;
                        sink.good += 1;
                        self.put(code, sink);
                    } else {
                        self.err = 0.97 * self.err + 1.0;
                        sink.bad += 1;
                        self.idle_run = 0;
                        sink.chars.push('_');
                        if self.err > 12.0 {
                            self.lock = None;
                            sink.lose("sync lost");
                        }
                    }
                }
            }
        }
    }
    fn decoding(&self) -> bool {
        self.lock.is_some()
    }
    fn detail(&self) -> String {
        self.lock.map_or_else(String::new, |(s, _)| self.label(s))
    }
    fn reset(&mut self) {
        self.bits.clear();
        self.lock = None;
        self.err = 0.0;
        self.figs = false;
        self.idle_run = 0;
    }
}

/// The submode of a configuration: the submode named, else the `submode` setting.
fn submode_of(sub: Option<&str>, settings: &Values) -> Submode {
    sub.and_then(Submode::from_key)
        .unwrap_or_else(|| settings.get(&SUBMODE))
}

/// The stream of the test signal at depth `s`: 150 idle characters, the text after a LTRS, 150 idle characters, each a Bauer word
/// with its units at [`position`], padded with `9 s + 1` zero bits.
pub fn test_stream(text: &str, idle: u8, s: usize) -> Vec<u8> {
    let mut chars = vec![idle; 150];
    chars.extend(ita2_codes(text));
    chars.extend(std::iter::repeat_n(idle, 150));
    let mut bits = vec![0u8; 10 * chars.len() + 9 * s + 1];
    for (n, &c) in chars.iter().enumerate() {
        let w = bauer::encode(c);
        for k in 0..10 {
            bits[position(n, k, s)] = ((w >> k) & 1) as u8;
        }
    }
    bits
}

/// The test signal: [`test_stream`] of the test text, the higher tone for 1.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let st = &cfg.settings;
    let sub = submode_of(cfg.submode.as_deref(), st);
    let baud = st.get(&GEN_BAUD);
    let shift = match (st.get(&GEN_SHIFT_HZ), sub) {
        (s, _) if s > 0.0 => s,
        (_, Submode::Autospec) => 270.0,
        (_, Submode::Spread) => 410.0,
    };
    let s = match sub {
        Submode::Autospec => 1,
        Submode::Spread => st.get(&GEN_DEPTH).value().unwrap_or(11),
    };
    let c = st.get(&GEN_CENTRE_HZ);
    let idle = st.get(&IDLE_CODE).clamp(0, 31) as u8;
    let seq: Vec<(f64, Option<f64>)> = test_stream(crate::serial::TEST_TEXT, idle, s)
        .into_iter()
        .map(|b| {
            (
                1.0 / baud,
                Some(if b == 1 {
                    c + shift / 2.0
                } else {
                    c - shift / 2.0
                }),
            )
        })
        .collect();
    Ok(Box::new(ToneSeqGen::new(
        cfg.rate_hz,
        &seq,
        2.0 * baud + shift,
    )))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    AutospecSetting::schema()
}

/// `cfg` with its submode written into the `submode` setting.
fn with_submode(cfg: &StackConfig) -> Result<StackConfig, ConfigError> {
    let mut cfg = cfg.clone();
    if let Some(sub) = cfg.submode.as_deref().and_then(Submode::from_key) {
        if let Ok(id) = schema().id(SUBMODE.field.key) {
            cfg.settings.set(id, &Value::Choice(sub.index()))?;
        }
    }
    Ok(cfg)
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("autospec"),
        "AUTOSPEC",
        TtyDecoder::new(Autospec::new()),
        &with_submode(cfg)?,
    )
}

/// AUTOSPEC and SPREAD.
pub static AUTOSPEC: StackDescriptor = StackDescriptor {
    id: "autospec",
    name: "AUTOSPEC / SPREAD",
    group: "HF utility",
    blurb: "FEC teleprinter on 2-FSK: 10-unit Bauer code words of ITA2 characters, one error corrected; SPREAD spreads each word over 11, 21 or 51 places (found).",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        ..crate::ms110::hf_tuning(crate::ms110::TEST_SIGNAL_HZ)
    },
    submodes: &[
        SubMode {
            id: "autospec",
            name: "AUTOSPEC",
            default_hz: crate::ms110::TEST_SIGNAL_HZ,
            ..SubMode::DEFAULT
        },
        SubMode {
            id: "spread",
            name: "SPREAD",
            default_hz: crate::ms110::TEST_SIGNAL_HZ,
            ..SubMode::DEFAULT
        },
    ],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::AUTOSPEC),
    benchmarks: Some(crate::benchmarks),
};
