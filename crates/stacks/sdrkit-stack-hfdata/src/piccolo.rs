//! Piccolo Mark VI: a 6-tone MFSK teleprinter. One tone at a time, 20 Hz apart, 50 ms each, continuous phase; each ITA2
//! character is a pair of tones from a 6 x 6 matrix. While idle the signal sends SYNC and its barred form (D E, E D); the pair D D
//! is never sent, which tells which tone starts a pair.
//!
//! Standard: ITU-R SM.1052 Annex 1 names "Piccolo mk 6"; no public document gives the tones or the matrix.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [piccolo: 8 kHz, tone-slot front (base, 50 ms slots), pair phase, matrix] -> text
//! ```

use crate::receiver::GEN_SECTION;
use crate::teleprinter::{
    ita2_codes, put_ita2, Alphabet, FrontLock, FrontSpec, Layer, Sink, Symbols, ToneSeqGen,
    TtyDecoder, ALPHABET, RATE,
};
use crate::typed::{PiccoloPairsEvent, PICCOLO_PAIRS};
use sdrkit_algo_modem::toneslots::ToneSlotConfig;
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, Choice, ConfigError, GenConfig, Generator, LockProfile, Receiver, Schema, Setting,
    SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, Value, Values,
};
use sdrkit_graph::values_of;
use std::sync::Arc;

/// The tone spacing.
pub const SPACING: Setting<field::Hz> = Setting::new("spacing_hz", "Tone spacing", "Hz", 20.0)
    .range(5.0, 200.0, 0.01)
    .help("Not confirmed by a public document.")
    .advanced();
/// The tone length.
pub const TONE_MS: Setting<field::F64> = Setting::new("tone_ms", "Tone length", "ms", 50.0)
    .range(5.0, 500.0, 0.1)
    .help("Not confirmed by a public document.")
    .advanced();
/// The lowest tone (A) of the test signal.
pub const GEN_BASE_HZ: Setting<field::Hz> = Setting::new("gen_base_hz", "Tone A", "Hz", 460.0)
    .range(300.0, 2800.0, 1.0)
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the Piccolo receiver.
    pub enum PiccoloRxSetting {
        /// Figures case.
        Alphabet = ALPHABET,
        /// Tone spacing.
        Spacing = SPACING,
        /// Tone length.
        ToneMs = TONE_MS,
    }
}

sdrkit_core::settings! {
    /// The settings of the Piccolo stack.
    pub enum PiccoloSetting {
        /// Figures case.
        Alphabet = ALPHABET,
        /// Tone spacing.
        Spacing = SPACING,
        /// Tone length.
        ToneMs = TONE_MS,
        /// Tone A of the test signal.
        GenBaseHz = GEN_BASE_HZ,
    }
}

/// What no public document confirms.
pub const NOTE_TEXT: &str = "No public document gives the Piccolo Mark VI signal beyond its name: not confirmed are the 6 tones, \
their 20 Hz spacing and 50 ms length (settings), the tone-pair matrix (a published museum description), the SYNC idle pattern \
and the US figures case. The ITA5 matrix of Mark XII is not covered.";

/// What a pair of tones stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairCode {
    /// An ITA2 code.
    Ita2(u8),
    /// SYNC or its barred form.
    Sync,
    /// MI.
    Mi,
    /// The pair D D, never sent.
    Never,
    /// A pair that stands for nothing.
    Unused,
}

use PairCode::{Ita2 as C, Mi, Never, Sync, Unused};

/// The matrix: the first tone (A to F) by row, the second by column, as ITA2 codes (element 1 in bit 0).
///
/// | | A | B | C | D | E | F |
/// |---|---|---|---|---|---|---|
/// | A | unused | E | LF | space | S | A |
/// | B | CR | D | R | N | F | J |
/// | C | T | Z | L | H | Y | W |
/// | D | I | U | P | never | SYNC | Q |
/// | E | C | K | V | SYNC | MI | LTRS |
/// | F | O | B | G | M | X | FIGS |
pub const MATRIX: [[PairCode; 6]; 6] = [
    [Unused, C(1), C(2), C(4), C(5), C(3)],
    [C(8), C(9), C(10), C(12), C(13), C(11)],
    [C(16), C(17), C(18), C(20), C(21), C(19)],
    [C(6), C(7), C(22), Never, Sync, C(23)],
    [C(14), C(15), C(30), Sync, Mi, C(31)],
    [C(24), C(25), C(26), C(28), C(29), C(27)],
];

/// What the pair (`first`, `second`) of tones (0 = A) stands for.
pub fn pair_code(first: usize, second: usize) -> PairCode {
    MATRIX
        .get(first)
        .and_then(|r| r.get(second))
        .copied()
        .unwrap_or(Unused)
}

/// The pair of tones of ITA2 code `code`.
pub fn pair_of(code: u8) -> Option<(usize, usize)> {
    (0..6)
        .flat_map(|a| (0..6).map(move |b| (a, b)))
        .find(|&(a, b)| pair_code(a, b) == PairCode::Ita2(code))
}

/// Whether the slots end the lock: the running mean of the share of the strongest tone (weight 0.1 a slot) falls below 0.6.
/// Noise gives about 0.4 over six tones, a tone about 0.9.
pub(crate) fn lost(share: &mut f64, slots: &[sdrkit_algo_modem::toneslots::Slot]) -> bool {
    for s in slots {
        *share += 0.1 * (f64::from(s.share) - *share);
        if *share < 0.6 {
            return true;
        }
    }
    false
}

/// The tone D.
const D: usize = 3;
/// The tone E.
const E: usize = 4;

/// The Piccolo layer.
pub struct Piccolo {
    alphabet: Alphabet,
    spacing: f64,
    tone_ms: f64,
    slots: Vec<u8>,
    phase: Option<usize>,
    share: f64,
    figs: bool,
}

impl Default for Piccolo {
    fn default() -> Self {
        Self::new()
    }
}

impl Piccolo {
    /// The layer at the default settings.
    pub fn new() -> Self {
        Piccolo {
            alphabet: Alphabet::Us,
            spacing: 20.0,
            tone_ms: 50.0,
            slots: Vec::new(),
            phase: None,
            share: 1.0,
            figs: false,
        }
    }

    fn score(&self, phase: usize) -> i64 {
        self.slots[phase..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| match (usize::from(p[0]), usize::from(p[1])) {
                (D, D) => -5,
                (D, E) | (E, D) => 1,
                _ => 0,
            })
            .sum()
    }
}

impl Layer for Piccolo {
    fn name(&self) -> &'static str {
        "hfdata.piccolo"
    }
    fn schema(&self) -> Arc<Schema> {
        PiccoloRxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (
                PiccoloRxSetting::Alphabet,
                Value::Choice(self.alphabet.index()),
            ),
            (PiccoloRxSetting::Spacing, Value::Hz(self.spacing)),
            (PiccoloRxSetting::ToneMs, Value::F64(self.tone_ms)),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<bool, SettingError> {
        match PiccoloRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PiccoloRxSetting::Alphabet => {
                self.alphabet = ALPHABET.read(id, value)?;
                return Ok(false);
            }
            PiccoloRxSetting::Spacing => self.spacing = SPACING.read(id, value)?,
            PiccoloRxSetting::ToneMs => self.tone_ms = TONE_MS.read(id, value)?,
        }
        Ok(true)
    }
    fn front(&self) -> FrontSpec {
        let mut c = ToneSlotConfig::new(RATE, 6, self.spacing, &[self.tone_ms / 1000.0]);
        c.idle_pair = Some(D);
        FrontSpec::Slots(c)
    }
    fn declare(&self, d: sdrkit_core::ResultsDecl) -> sdrkit_core::ResultsDecl {
        d.event(&PICCOLO_PAIRS)
    }
    fn note(&self) -> &'static str {
        NOTE_TEXT
    }
    fn centre(&self, lock: &FrontLock) -> f64 {
        match lock {
            FrontLock::Slots(l) => l.base_hz + 2.5 * self.spacing,
            FrontLock::Fsk(l) => l.centre(),
        }
    }
    fn front_locked(&mut self, _lock: FrontLock) {}
    fn symbols(&mut self, s: Symbols<'_>, sink: &mut Sink) {
        let Symbols::Slots(slots) = s else { return };
        if self.phase.is_some() && lost(&mut self.share, slots) {
            sink.lose_signal("tone pairs lost");
            return;
        }
        self.slots.extend(slots.iter().map(|s| s.tone));
        if self.phase.is_none() {
            if self.slots.len() < 64 {
                return;
            }
            let p = if self.score(1) > self.score(0) { 1 } else { 0 };
            self.phase = Some(p);
            self.share = 1.0;
            self.slots.drain(..p);
            self.figs = false;
            sink.events.push(PICCOLO_PAIRS.event(
                sink.time,
                sink.dial,
                &PiccoloPairsEvent {
                    text: "Piccolo tone pairs found",
                },
            ));
            sink.lock(
                format!("Piccolo tone pairs found: {:.0} ms tones", self.tone_ms),
                format!("{:.0} ms tones", self.tone_ms),
            );
        }
        let n = self.slots.len() / 2 * 2;
        let table = self.alphabet.table();
        for p in self.slots[..n].as_chunks::<2>().0 {
            match pair_code(usize::from(p[0]), usize::from(p[1])) {
                PairCode::Ita2(c) => {
                    sink.good += 1;
                    put_ita2(table, c, &mut self.figs, &mut sink.chars);
                }
                PairCode::Sync | PairCode::Mi | PairCode::Never => {}
                PairCode::Unused => sink.bad += 1,
            }
        }
        self.slots.drain(..n);
    }
    fn decoding(&self) -> bool {
        self.phase.is_some()
    }
    fn detail(&self) -> String {
        format!("{:.0} ms tones", self.tone_ms)
    }
    fn reset(&mut self) {
        self.slots.clear();
        self.phase = None;
        self.figs = false;
    }
}

/// The tones of the test signal (0 = A): 40 idle pairs (D E, E D), the text after a LTRS, 40 idle pairs.
pub fn test_tones(text: &str) -> Vec<usize> {
    let idle = |n: usize| (0..n).flat_map(|i| if i % 2 == 0 { [D, E] } else { [E, D] });
    let mut t: Vec<usize> = idle(40).collect();
    for c in ita2_codes(text) {
        if let Some((a, b)) = pair_of(c) {
            t.extend([a, b]);
        }
    }
    t.extend(idle(40));
    t
}

/// The test signal: [`test_tones`] of the test text, 50 ms each, continuous phase.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let base = s.get(&GEN_BASE_HZ);
    let sp = s.get(&SPACING);
    let len = s.get(&TONE_MS) / 1000.0;
    let seq: Vec<(f64, Option<f64>)> = test_tones(crate::serial::TEST_TEXT)
        .into_iter()
        .map(|k| (len, Some(base + sp * k as f64)))
        .collect();
    Ok(Box::new(ToneSeqGen::new(
        cfg.rate_hz,
        &seq,
        6.0 * sp + 2.0 / len,
    )))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    PiccoloSetting::schema()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("piccolo"),
        "Piccolo",
        TtyDecoder::new(Piccolo::new()),
        cfg,
    )
}

/// Piccolo Mark VI.
pub static PICCOLO: StackDescriptor = StackDescriptor {
    id: "piccolo",
    name: "Piccolo Mk VI",
    group: "HF utility",
    blurb: "6-tone MFSK teleprinter: each ITA2 character a pair of 50 ms tones 20 Hz apart from a 6 x 6 matrix; the tones and the pair phase found.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        ..crate::ms110::hf_tuning(crate::ms110::TEST_SIGNAL_HZ)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::PICCOLO),
    benchmarks: Some(crate::benchmarks),
};
