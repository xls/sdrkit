//! Coquelet-8: an 8-tone MFSK teleprinter. One tone at a time, 26.67 Hz apart, 37.5, 50 or 75 ms each (found); each ITA2
//! character is a pair of tones: the first (tone 1 to 8) carries elements 3, 4 and 5, the second (tone 5 to 8) elements 1 and 2.
//! While idle the signal alternates tones 1 and 8, which is also the pair of M: a run of one or two such pairs is shown as M, a
//! longer run is idle.
//!
//! Standard: ITU-R SM.1052 Annex 1 names "Coquelet mk 1, 2"; no public document gives the tones or the code.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [coquelet: 8 kHz, tone-slot front (base, slot length), pair phase, tone table] -> text
//! ```

use crate::receiver::GEN_SECTION;
use crate::teleprinter::{
    ita2_codes, put_ita2, Alphabet, FrontLock, FrontSpec, Layer, Sink, Symbols, ToneSeqGen,
    TtyDecoder, ALPHABET, RATE,
};
use crate::typed::{CoqueletPairsEvent, COQUELET_PAIRS};
use sdrkit_algo_modem::toneslots::ToneSlotConfig;
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, Choice, ConfigError, GenConfig, Generator, LockProfile, Receiver, ResultsDecl,
    Schema, Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, Value,
    Values,
};
use sdrkit_graph::values_of;
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// The tone length.
    pub enum ToneLength {
        /// Found from the signal.
        Auto = ("auto", "Automatic"),
        /// 37.5 ms.
        Ms37 = ("37.5", "37.5 ms"),
        /// 50 ms.
        Ms50 = ("50", "50 ms"),
        /// 75 ms.
        Ms75 = ("75", "75 ms"),
    }
}

impl ToneLength {
    fn seconds(self) -> Option<f64> {
        match self {
            ToneLength::Auto => None,
            ToneLength::Ms37 => Some(0.0375),
            ToneLength::Ms50 => Some(0.05),
            ToneLength::Ms75 => Some(0.075),
        }
    }
}

/// The tone length.
pub const TONE_MS: Setting<field::Choice<ToneLength>> =
    Setting::choice("tone_ms", "Tone length", ToneLength::Auto);
/// The tone spacing.
pub const SPACING: Setting<field::Hz> =
    Setting::new("spacing_hz", "Tone spacing", "Hz", 80.0 / 3.0)
        .range(5.0, 200.0, 0.01)
        .help("Not confirmed by a public document.")
        .advanced();
/// The tone length of the test signal.
pub const GEN_TONE_MS: Setting<field::Choice<ToneLength>> =
    Setting::choice("gen_tone_ms", "Tone length", ToneLength::Ms50)
        .help("Automatic sends 50 ms.")
        .section(GEN_SECTION)
        .advanced();
/// Tone 1 of the test signal.
pub const GEN_BASE_HZ: Setting<field::Hz> = Setting::new("gen_base_hz", "Tone 1", "Hz", 773.0)
    .range(300.0, 2700.0, 1.0)
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the Coquelet receiver.
    pub enum CoqueletRxSetting {
        /// Tone length.
        ToneMs = TONE_MS,
        /// Figures case.
        Alphabet = ALPHABET,
        /// Tone spacing.
        Spacing = SPACING,
    }
}

sdrkit_core::settings! {
    /// The settings of the Coquelet stack.
    pub enum CoqueletSetting {
        /// Tone length.
        ToneMs = TONE_MS,
        /// Figures case.
        Alphabet = ALPHABET,
        /// Tone spacing.
        Spacing = SPACING,
        /// Tone length of the test signal.
        GenToneMs = GEN_TONE_MS,
        /// Tone 1 of the test signal.
        GenBaseHz = GEN_BASE_HZ,
    }
}

/// What no public document confirms.
pub const NOTE_TEXT: &str = "No public document gives the Coquelet-8 signal beyond its name: not confirmed are the 8 tones, their \
26.67 Hz spacing (a setting) and lengths, the tone table (a maker's published table), the idle pattern and the reading of a \
run of one or two idle pairs as M, and the US figures case. Coquelet-13 and Coquelet-80 are not covered.";

/// The first tone (1 to 8) of elements 3, 4, 5 (element 3 in bit 0).
const FIRST: [u8; 8] = [8, 4, 7, 6, 3, 5, 2, 1];
/// The second tone (5 to 8) of elements 1, 2 (element 1 in bit 0).
const SECOND: [u8; 4] = [8, 6, 7, 5];

/// The tone pair (1 to 8) of ITA2 code `code` (element 1 in bit 0).
pub fn pair_of(code: u8) -> (u8, u8) {
    (
        FIRST[usize::from((code >> 2) & 7)],
        SECOND[usize::from(code & 3)],
    )
}

/// The ITA2 code of the tone pair (1 to 8); `None` when the second tone is below 5 or a tone is out of range.
pub fn code_of(first: u8, second: u8) -> Option<u8> {
    let hi = FIRST.iter().position(|&t| t == first)? as u8;
    let lo = SECOND.iter().position(|&t| t == second)? as u8;
    Some((hi << 2) | lo)
}

/// The pair of M and of the idle.
const IDLE: (u8, u8) = (1, 8);

/// The Coquelet layer.
pub struct Coquelet {
    tone: ToneLength,
    alphabet: Alphabet,
    spacing: f64,
    slot_s: f64,
    slots: Vec<u8>,
    phase: Option<usize>,
    share: f64,
    figs: bool,
    idle_run: usize,
}

impl Default for Coquelet {
    fn default() -> Self {
        Self::new()
    }
}

impl Coquelet {
    /// The layer at the default settings.
    pub fn new() -> Self {
        Coquelet {
            tone: ToneLength::Auto,
            alphabet: Alphabet::Us,
            spacing: 80.0 / 3.0,
            slot_s: 0.05,
            slots: Vec::new(),
            phase: None,
            share: 1.0,
            figs: false,
            idle_run: 0,
        }
    }

    fn label(&self) -> String {
        format!("{:.0} ms tones", self.slot_s * 1000.0)
    }
}

impl Layer for Coquelet {
    fn name(&self) -> &'static str {
        "hfdata.coquelet"
    }
    fn schema(&self) -> Arc<Schema> {
        CoqueletRxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (CoqueletRxSetting::ToneMs, Value::Choice(self.tone.index())),
            (
                CoqueletRxSetting::Alphabet,
                Value::Choice(self.alphabet.index()),
            ),
            (CoqueletRxSetting::Spacing, Value::Hz(self.spacing)),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<bool, SettingError> {
        match CoqueletRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CoqueletRxSetting::ToneMs => self.tone = TONE_MS.read(id, value)?,
            CoqueletRxSetting::Alphabet => {
                self.alphabet = ALPHABET.read(id, value)?;
                return Ok(false);
            }
            CoqueletRxSetting::Spacing => self.spacing = SPACING.read(id, value)?,
        }
        Ok(true)
    }
    fn front(&self) -> FrontSpec {
        let slots: Vec<f64> = match self.tone.seconds() {
            Some(s) => vec![s],
            None => vec![0.0375, 0.05, 0.075],
        };
        FrontSpec::Slots(ToneSlotConfig::new(RATE, 8, self.spacing, &slots))
    }
    fn declare(&self, d: ResultsDecl) -> ResultsDecl {
        d.event(&COQUELET_PAIRS)
    }
    fn note(&self) -> &'static str {
        NOTE_TEXT
    }
    fn centre(&self, lock: &FrontLock) -> f64 {
        match lock {
            FrontLock::Slots(l) => l.base_hz + 3.5 * self.spacing,
            FrontLock::Fsk(l) => l.centre(),
        }
    }
    fn front_locked(&mut self, lock: FrontLock) {
        if let FrontLock::Slots(l) = lock {
            self.slot_s = l.slot_s;
        }
    }
    fn symbols(&mut self, s: Symbols<'_>, sink: &mut Sink) {
        let Symbols::Slots(slots) = s else { return };
        if self.phase.is_some() && crate::piccolo::lost(&mut self.share, slots) {
            sink.lose_signal("tone pairs lost");
            return;
        }
        // Tone 1 is the lowest: tone index 0.
        self.slots.extend(slots.iter().map(|s| s.tone + 1));
        if self.phase.is_none() {
            if self.slots.len() < 64 {
                return;
            }
            let score = |p: usize| {
                self.slots[p..]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .filter(|q| q[1] >= 5)
                    .count()
            };
            let p = if score(1) > score(0) { 1 } else { 0 };
            self.phase = Some(p);
            self.share = 1.0;
            self.slots.drain(..p);
            self.figs = false;
            self.idle_run = 0;
            sink.events.push(COQUELET_PAIRS.event(
                sink.time,
                sink.dial,
                &CoqueletPairsEvent {
                    text: "Coquelet tone pairs found",
                },
            ));
            sink.lock(
                format!("Coquelet tone pairs found: {}", self.label()),
                self.label(),
            );
        }
        let n = self.slots.len() / 2 * 2;
        let table = self.alphabet.table();
        for i in (0..n).step_by(2) {
            let (a, b) = (self.slots[i], self.slots[i + 1]);
            if (a, b) == IDLE {
                self.idle_run += 1;
                continue;
            }
            if (1..=2).contains(&self.idle_run) {
                for _ in 0..self.idle_run {
                    put_ita2(table, 28, &mut self.figs, &mut sink.chars);
                    sink.good += 1;
                }
            }
            self.idle_run = 0;
            match code_of(a, b) {
                Some(c) => {
                    sink.good += 1;
                    put_ita2(table, c, &mut self.figs, &mut sink.chars);
                }
                None => sink.bad += 1,
            }
        }
        self.slots.drain(..n);
    }
    fn decoding(&self) -> bool {
        self.phase.is_some()
    }
    fn detail(&self) -> String {
        self.label()
    }
    fn reset(&mut self) {
        self.slots.clear();
        self.phase = None;
        self.figs = false;
        self.idle_run = 0;
    }
}

/// The tones (1 to 8) of the test signal: 40 idle pairs (1, 8), the text after a LTRS, 40 idle pairs.
pub fn test_tones(text: &str) -> Vec<u8> {
    let mut t: Vec<u8> = (0..40).flat_map(|_| [IDLE.0, IDLE.1]).collect();
    for c in ita2_codes(text) {
        let (a, b) = pair_of(c);
        t.extend([a, b]);
    }
    t.extend((0..40).flat_map(|_| [IDLE.0, IDLE.1]));
    t
}

/// The test signal: [`test_tones`] of the test text, continuous phase.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let len = s.get(&GEN_TONE_MS).seconds().unwrap_or(0.05);
    let base = s.get(&GEN_BASE_HZ);
    let sp = s.get(&SPACING);
    let seq: Vec<(f64, Option<f64>)> = test_tones(crate::serial::TEST_TEXT)
        .into_iter()
        .map(|k| (len, Some(base + sp * f64::from(k - 1))))
        .collect();
    Ok(Box::new(ToneSeqGen::new(
        cfg.rate_hz,
        &seq,
        8.0 * sp + 2.0 / len,
    )))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    CoqueletSetting::schema()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("coquelet"),
        "Coquelet",
        TtyDecoder::new(Coquelet::new()),
        cfg,
    )
}

/// Coquelet-8.
pub static COQUELET: StackDescriptor = StackDescriptor {
    id: "coquelet",
    name: "Coquelet-8",
    group: "HF utility",
    blurb: "8-tone MFSK teleprinter: each ITA2 character a pair of tones 26.67 Hz apart; the tones, the tone length (37.5, 50 or 75 ms) and the pair phase found.",
    tuning: crate::ms110::hf_tuning(crate::ms110::TEST_SIGNAL_HZ),
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::COQUELET),
    benchmarks: Some(crate::benchmarks),
};
