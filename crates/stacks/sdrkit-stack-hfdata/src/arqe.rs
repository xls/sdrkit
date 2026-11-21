//! ARQ-E and ARQ-N: duplex ARQ teleprinter links on HF. Each direction is a continuous 2-FSK signal of 7-unit words; ARQ-E sends
//! every 4th or 8th word with all seven units inverted, ARQ-N inverts none. The receiver finds the speed, the shift, the word
//! alignment, the polarity and the inversion cycle, and only locks on real traffic (an idle pattern passes the word check at any
//! rotation).
//!
//! A signal of 2 or 4 time-division channels of ITU-R F.342-2 Annex I is read too (the setting `channels`): 2 channels send their
//! characters in turn (A, B); 4 channels send A and C element by element, then B and D element by element. In each cycle channels
//! A and D invert one character and B and C all the others. The setting `channel` picks the channel whose text is shown.
//!
//! Two word codes can be read (the setting `word_code`): an identification unit, the five ITA2 elements and an odd-parity unit;
//! or the 7-unit constant-ratio code of ITU-R F.342-2 Table I (three Z units in every word; signal repetition, alpha and beta).
//!
//! Standard: ITU-R F.342-2 (the 7-unit code and its repetition cycles of 4 and 8 characters, 96 and 192 Bd, 85 5/7 Bd), ITU-R
//! F.519 (single channel at 48, 72 and 96 Bd), ITU-R SM.1052 Annex 1 (the names ARQ-E3 and ARQ-N). Which word code the signals
//! named ARQ-E use is not established by a public document.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [arqe: 8 kHz, 2-FSK front, word lock (polarity, alignment, channels, cycle), reader] -> text
//! ```

use crate::receiver::GEN_SECTION;
use crate::teleprinter::{
    ita2_codes, put_ita2, Alphabet, FrontLock, FrontSpec, Layer, Polarity, Sink, Symbols,
    ToneSeqGen, TtyDecoder, ALPHABET, POLARITY, RATE,
};
use sdrkit_algo_modem::fskauto::BinaryFskConfig;
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, Choice, ConfigError, GenConfig, Generator, LockProfile, Receiver, Schema, Setting,
    SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, SubMode, Value, Values,
};
use sdrkit_graph::values_of;
use sdrkit_util_text::sevenunit::{f342, ident_parity, three_of_seven, F342Signal};
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// The submode.
    pub enum Submode {
        /// ARQ-E: every 4th or 8th word inverted.
        Arqe = ("arqe", "ARQ-E"),
        /// ARQ-N: no inversion.
        Arqn = ("arqn", "ARQ-N"),
    }
}

sdrkit_core::choice_enum! {
    /// The inversion cycle.
    pub enum Cycle {
        /// Found from the signal.
        Auto = ("auto", "Automatic"),
        /// No word inverted.
        None = ("none", "No inversion"),
        /// Every 4th word inverted.
        C4 = ("4", "Every 4th word"),
        /// Every 8th word inverted.
        C8 = ("8", "Every 8th word"),
    }
}

sdrkit_core::choice_enum! {
    /// The time-division channels of an ITU-R F.342 signal.
    pub enum Channels {
        /// Found from the signal.
        Auto = ("auto", "Automatic"),
        /// One channel.
        One = ("1", "One channel"),
        /// Two channels, their characters in turn.
        Two = ("2", "Two channels"),
        /// Four channels, A with C and B with D element by element.
        Four = ("4", "Four channels"),
    }
}

impl Channels {
    /// The channel counts to try.
    fn counts(self) -> &'static [usize] {
        match self {
            Channels::Auto => &[1, 2, 4],
            Channels::One => &[1],
            Channels::Two => &[2],
            Channels::Four => &[4],
        }
    }
}

sdrkit_core::choice_enum! {
    /// A time-division channel.
    pub enum Channel {
        /// Channel A.
        A = ("a", "A"),
        /// Channel B.
        B = ("b", "B"),
        /// Channel C.
        C = ("c", "C"),
        /// Channel D.
        D = ("d", "D"),
    }
}

sdrkit_core::choice_enum! {
    /// The 7-unit word code.
    pub enum WordCode {
        /// An identification unit, the five ITA2 elements and an odd-parity unit.
        Parity = ("parity", "Identification and parity"),
        /// The constant-ratio code of ITU-R F.342-2.
        F342 = ("f342", "ITU-R F.342 constant ratio"),
    }
}

/// The submode.
pub const SUBMODE: Setting<field::Choice<Submode>> =
    Setting::choice("submode", "Signal", Submode::Arqe);
/// The speed; 0 finds it.
pub const BAUD: Setting<field::F64> = Setting::new("baud", "Speed", "Bd", 0.0)
    .range(0.0, 300.0, 0.01)
    .help("0 finds it from the candidates of the submode.");
/// The shift; 0 finds it.
pub const SHIFT: Setting<field::Hz> = Setting::new("shift_hz", "Shift", "Hz", 0.0)
    .range(0.0, 1200.0, 1.0)
    .help("0 finds it from the signal.");
/// The inversion cycle.
pub const CYCLE: Setting<field::Choice<Cycle>> =
    Setting::choice("cycle", "Inversion cycle", Cycle::Auto)
        .help("ARQ-E only; ARQ-N inverts no word.");
/// The word code.
pub const WORD_CODE: Setting<field::Choice<WordCode>> =
    Setting::choice("word_code", "Word code", WordCode::Parity).help(
        "Which 7-unit code the signal uses is not confirmed by a public document: an identification unit, ITA2 and odd parity, or ITU-R F.342.",
    );
/// The time-division channels.
pub const CHANNELS: Setting<field::Choice<Channels>> =
    Setting::choice("channels", "Channels", Channels::Auto).help(
        "ITU-R F.342: 2 channels send their characters in turn; 4 channels send A and C, then B and D, element by element.",
    );
/// The channel shown.
pub const CHANNEL: Setting<field::Choice<Channel>> =
    Setting::choice("channel", "Channel shown", Channel::A)
        .help("The channel whose text is shown when the signal has 2 or 4 channels.");
/// The speed of the test signal.
pub const GEN_BAUD: Setting<field::F64> = Setting::new("gen_baud", "Speed", "Bd", 96.0)
    .range(10.0, 300.0, 0.01)
    .section(GEN_SECTION)
    .advanced();
/// The shift of the test signal.
pub const GEN_SHIFT_HZ: Setting<field::Hz> = Setting::new("gen_shift_hz", "Shift", "Hz", 400.0)
    .range(60.0, 1100.0, 1.0)
    .section(GEN_SECTION)
    .advanced();
/// The inversion cycle of the test signal (ARQ-N sends none).
pub const GEN_CYCLE: Setting<field::Choice<Cycle>> =
    Setting::choice("gen_cycle", "Inversion cycle", Cycle::C8)
        .help("Automatic sends 8. ARQ-N inverts no word.")
        .section(GEN_SECTION)
        .advanced();
/// The time-division channels of the test signal.
pub const GEN_CHANNELS: Setting<field::Choice<Channels>> =
    Setting::choice("gen_channels", "Channels", Channels::One)
        .help("Automatic sends one. Channel A sends the test text, the others the test text after their name.")
        .section(GEN_SECTION)
        .advanced();
/// The word code of the test signal.
pub const GEN_WORD_CODE: Setting<field::Choice<WordCode>> =
    Setting::choice("gen_word_code", "Word code", WordCode::Parity)
        .section(GEN_SECTION)
        .advanced();
/// The centre of the test signal.
pub const GEN_CENTRE_HZ: Setting<field::Hz> = Setting::new("gen_centre_hz", "Centre", "Hz", 1500.0)
    .range(400.0, 3500.0, 1.0)
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the ARQ-E receiver.
    pub enum ArqeRxSetting {
        /// Submode.
        Submode = SUBMODE,
        /// Speed.
        Baud = BAUD,
        /// Shift.
        Shift = SHIFT,
        /// Polarity.
        Polarity = POLARITY,
        /// Inversion cycle.
        Cycle = CYCLE,
        /// Figures case.
        Alphabet = ALPHABET,
        /// Word code.
        WordCode = WORD_CODE,
        /// Time-division channels.
        Channels = CHANNELS,
        /// Channel shown.
        Channel = CHANNEL,
    }
}

sdrkit_core::settings! {
    /// The settings of the ARQ-E stack.
    pub enum ArqeSetting {
        /// Submode.
        Submode = SUBMODE,
        /// Speed.
        Baud = BAUD,
        /// Shift.
        Shift = SHIFT,
        /// Polarity.
        Polarity = POLARITY,
        /// Inversion cycle.
        Cycle = CYCLE,
        /// Figures case.
        Alphabet = ALPHABET,
        /// Word code.
        WordCode = WORD_CODE,
        /// Time-division channels.
        Channels = CHANNELS,
        /// Channel shown.
        Channel = CHANNEL,
        /// Speed of the test signal.
        GenBaud = GEN_BAUD,
        /// Shift of the test signal.
        GenShiftHz = GEN_SHIFT_HZ,
        /// Inversion cycle of the test signal.
        GenCycle = GEN_CYCLE,
        /// Time-division channels of the test signal.
        GenChannels = GEN_CHANNELS,
        /// Word code of the test signal.
        GenWordCode = GEN_WORD_CODE,
        /// Centre of the test signal.
        GenCentreHz = GEN_CENTRE_HZ,
    }
}

/// The candidate bauds of ARQ-E.
pub const ARQE_BAUDS: [f64; 11] = [
    46.2, 48.0, 50.0, 64.0, 72.0, 86.0, 96.0, 144.0, 184.6, 192.0, 288.0,
];
/// The rates of ITU-R F.342-2 Table IV and F.519 (85 5/7, 171 3/7): the candidates when the word code is F.342.
pub const F342_BAUDS: [f64; 8] = [
    48.0,
    72.0,
    600.0 / 7.0,
    96.0,
    100.0,
    1200.0 / 7.0,
    192.0,
    200.0,
];
/// The candidate bauds of ARQ-N.
pub const ARQN_BAUDS: [f64; 6] = [48.0, 64.0, 72.0, 86.0, 96.0, 192.0];

/// What no public document confirms.
pub const NOTE_TEXT: &str = "Which 7-unit word the signals named ARQ-E and ARQ-N use is not confirmed by a public document: \
the identification unit, ITA2 and odd-parity word (the default) or the constant-ratio code of ITU-R F.342-2 (the setting \
word_code). With the F.342 code the candidate speeds are those of F.342-2 Table IV and F.519 (48, 72, 85 5/7, 96, 100, 171 3/7, \
192 and 200 Bd); with the parity word they are 46.2, 48, 50, 64, 72, 86, 96, 144, 184.6, 192 and 288 Bd, of which 46.2, 50, 64, 86, \
144, 184.6 and 288 Bd are not confirmed by a public document. Not confirmed either: all of ARQ-N, the idle word of the parity \
code and the US figures case. The repetition cycle of the ARQ link is not followed (each direction is read as a continuous \
stream). Of the 2- and 4-channel signals of F.342-2 Annex I, which element of A or C is sent first is not confirmed (A is taken \
first). On a 4-channel signal that idles, A and C (B and D) send complementary units and the automatic speed can find half \
the rate: give the speed.";

/// Words tested by a lock, over all channels.
const TEST_WORDS: usize = 120;
/// Words kept before a lock is tried: the words tested and the offsets of a frame of 4 channels.
const LOCK_WORDS: usize = TEST_WORDS + 4;

/// The place of unit `u` (0 to 6) of channel `ch` (A = 0 to D = 3) in a frame of `n` channels (7 n units): the channels in turn,
/// or for 4 channels A and C element by element (A first), then B and D element by element (ITU-R F.342-2 Annex I).
pub fn frame_place(n: usize, ch: usize, u: usize) -> usize {
    if n == 4 {
        14 * (ch % 2) + 2 * u + ch / 2
    } else {
        7 * ch + u
    }
}

/// A word lock: polarity, channels, inversion cycle and its phase.
#[derive(Clone, Copy, Debug, PartialEq)]
struct WordLock {
    inverted: bool,
    channels: usize,
    cycle: usize,
    phase: usize,
}

impl WordLock {
    /// Whether word `k` (counted from the lock) of channel `ch` is in an inverted place: channels A and D invert the first
    /// character of each cycle, B and C the others.
    fn inverted_place(&self, ch: usize, k: usize) -> bool {
        inverted_place(
            self.cycle,
            ch,
            (k + self.phase).is_multiple_of(self.cycle.max(1)),
        )
    }
}

/// Whether a character of channel `ch` is sent inverted in a cycle of `cycle` characters (0: none), `first` when it is the first
/// of its cycle.
fn inverted_place(cycle: usize, ch: usize, first: bool) -> bool {
    cycle > 0 && (first != matches!(ch, 1 | 2))
}

/// The ARQ-E layer.
pub struct Arqe {
    submode: Submode,
    baud: f64,
    shift: f64,
    polarity: Polarity,
    cycle: Cycle,
    alphabet: Alphabet,
    code: WordCode,
    channels: Channels,
    channel: Channel,
    lock_info: Option<FrontLock>,
    bits: Vec<u8>,
    lock: Option<WordLock>,
    word: usize,
    err: f64,
    figs: bool,
}

impl Default for Arqe {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether `w` passes the word check of `code`.
fn passes(code: WordCode, w: u8) -> bool {
    match code {
        WordCode::Parity => ident_parity::passes(w),
        WordCode::F342 => three_of_seven(w) && f342::signal(w).is_some(),
    }
}

/// Whether `w` (passing) is a traffic word of `code`; its ITA2 code.
fn traffic(code: WordCode, w: u8) -> Option<u8> {
    match code {
        WordCode::Parity => (!ident_parity::is_service(w)).then(|| ident_parity::code(w)),
        WordCode::F342 => match f342::signal(w) {
            Some(F342Signal::Traffic(c)) => Some(c),
            _ => None,
        },
    }
}

impl Arqe {
    /// The layer at the default settings.
    pub fn new() -> Self {
        Arqe {
            submode: Submode::Arqe,
            baud: 0.0,
            shift: 0.0,
            polarity: Polarity::Auto,
            cycle: Cycle::Auto,
            alphabet: Alphabet::Us,
            code: WordCode::Parity,
            channels: Channels::Auto,
            channel: Channel::A,
            lock_info: None,
            bits: Vec::new(),
            lock: None,
            word: 0,
            err: 0.0,
            figs: false,
        }
    }

    fn name_of(&self) -> &'static str {
        match self.submode {
            Submode::Arqe => "ARQ-E",
            Submode::Arqn => "ARQ-N",
        }
    }

    /// The word `k` of channel `ch` of `n` in the buffer at bit offset `off`, unit 1 in bit 0.
    fn word_at(&self, off: usize, n: usize, ch: usize, k: usize, inv: bool) -> u8 {
        let s = off + 7 * n * k;
        (0..7).fold(0u8, |w, u| {
            w | ((self.bits[s + frame_place(n, ch, u)] ^ u8::from(inv)) << u)
        })
    }

    /// The word of channel `ch` in place `k` of a lock, erect.
    fn erect(wl: &WordLock, ch: usize, k: usize, w: u8) -> u8 {
        if wl.inverted_place(ch, k) {
            !w & 0x7F
        } else {
            w
        }
    }

    fn cycles(&self) -> &'static [usize] {
        match (self.submode, self.cycle) {
            (Submode::Arqn, _) | (_, Cycle::None) => &[0],
            (_, Cycle::C4) => &[4],
            (_, Cycle::C8) => &[8],
            (_, Cycle::Auto) => &[0, 4, 8],
        }
    }

    /// The best lock over the oldest 120 words (of all channels), its offset, if it passes the word and traffic checks. On equal
    /// scores the fewer channels win.
    fn try_lock(&self) -> Option<(WordLock, usize, f64)> {
        let mut best: Option<(WordLock, usize, f64)> = None;
        for &n in self.channels.counts() {
            let m = TEST_WORDS / n;
            for &inv in self.polarity.tries() {
                for off in 0..7 * n {
                    let words: Vec<[u8; 4]> = (0..m)
                        .map(|k| {
                            let mut f = [0u8; 4];
                            for (ch, w) in f.iter_mut().enumerate().take(n) {
                                *w = self.word_at(off, n, ch, k, inv);
                            }
                            f
                        })
                        .collect();
                    for &cycle in self.cycles() {
                        for phase in 0..cycle.max(1) {
                            let wl = WordLock {
                                inverted: inv,
                                channels: n,
                                cycle,
                                phase,
                            };
                            let ok = words
                                .iter()
                                .enumerate()
                                .flat_map(|(k, f)| (0..n).map(move |ch| (k, ch, f[ch])))
                                .filter(|&(k, ch, w)| passes(self.code, Self::erect(&wl, ch, k, w)))
                                .count();
                            let score = ok as f64 / (n * m) as f64
                                - if cycle == 0 && self.submode == Submode::Arqe {
                                    0.02
                                } else {
                                    0.0
                                };
                            if best.is_none_or(|b| score > b.2) {
                                best = Some((wl, off, score));
                            }
                        }
                    }
                }
            }
        }
        let (wl, off, score) = best?;
        if score < 0.93 {
            return None;
        }
        let n = wl.channels;
        let mut seen = [false; 128];
        let mut count = 0;
        for k in 0..TEST_WORDS / n {
            for ch in 0..n {
                let w = Self::erect(&wl, ch, k, self.word_at(off, n, ch, k, wl.inverted));
                if passes(self.code, w) && traffic(self.code, w).is_some() {
                    count += 1;
                    seen[usize::from(w)] = true;
                }
            }
        }
        let distinct = seen.iter().filter(|&&s| s).count();
        (count >= 24 && distinct >= 10).then_some((wl, off, score))
    }

    /// Reads one frame (a word of each channel); false when the lock is lost. The text of the channel shown goes out.
    fn read(&mut self, wl: WordLock, sink: &mut Sink) -> bool {
        let n = wl.channels;
        let shown = self.channel.index() as usize;
        let shown = if shown < n { shown } else { 0 };
        let mut kept = true;
        for ch in 0..n {
            let w = Self::erect(&wl, ch, self.word, self.word_at(0, n, ch, 0, wl.inverted));
            if !passes(self.code, w) {
                sink.bad += 1;
                self.err = 0.95 * self.err + 1.0;
                kept &= self.err <= 8.0;
                continue;
            }
            self.err *= 0.95;
            sink.good += 1;
            if ch == shown {
                if let Some(c) = traffic(self.code, w) {
                    put_ita2(self.alphabet.table(), c, &mut self.figs, &mut sink.chars);
                }
            }
        }
        self.bits.drain(..7 * n);
        self.word += 1;
        kept
    }

    fn detail_of(&self) -> String {
        let (cycle, n) = self.lock.map_or((0, 1), |l| (l.cycle, l.channels));
        let mut d = if cycle > 0 {
            format!("cycle {cycle}")
        } else {
            "no inversion".to_string()
        };
        if n > 1 {
            d.push_str(&format!(", {n} channels"));
        }
        d
    }
}

impl Layer for Arqe {
    fn name(&self) -> &'static str {
        "hfdata.arqe"
    }
    fn schema(&self) -> Arc<Schema> {
        ArqeRxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (ArqeRxSetting::Submode, Value::Choice(self.submode.index())),
            (ArqeRxSetting::Baud, Value::F64(self.baud)),
            (ArqeRxSetting::Shift, Value::Hz(self.shift)),
            (
                ArqeRxSetting::Polarity,
                Value::Choice(self.polarity.index()),
            ),
            (ArqeRxSetting::Cycle, Value::Choice(self.cycle.index())),
            (
                ArqeRxSetting::Alphabet,
                Value::Choice(self.alphabet.index()),
            ),
            (ArqeRxSetting::WordCode, Value::Choice(self.code.index())),
            (
                ArqeRxSetting::Channels,
                Value::Choice(self.channels.index()),
            ),
            (ArqeRxSetting::Channel, Value::Choice(self.channel.index())),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<bool, SettingError> {
        match ArqeRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ArqeRxSetting::Submode => self.submode = SUBMODE.read(id, value)?,
            ArqeRxSetting::Baud => self.baud = BAUD.read(id, value)?,
            ArqeRxSetting::Shift => self.shift = SHIFT.read(id, value)?,
            ArqeRxSetting::Polarity => self.polarity = POLARITY.read(id, value)?,
            ArqeRxSetting::Cycle => self.cycle = CYCLE.read(id, value)?,
            ArqeRxSetting::Alphabet => {
                self.alphabet = ALPHABET.read(id, value)?;
                return Ok(false);
            }
            ArqeRxSetting::WordCode => self.code = WORD_CODE.read(id, value)?,
            ArqeRxSetting::Channels => self.channels = CHANNELS.read(id, value)?,
            ArqeRxSetting::Channel => {
                self.channel = CHANNEL.read(id, value)?;
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn front(&self) -> FrontSpec {
        let bauds: &[f64] = match (self.code, self.submode) {
            (WordCode::F342, _) => &F342_BAUDS,
            (_, Submode::Arqe) => &ARQE_BAUDS,
            (_, Submode::Arqn) => &ARQN_BAUDS,
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
                    if self.bits.len() < 7 * LOCK_WORDS {
                        return;
                    }
                    match self.try_lock() {
                        Some((wl, off, score)) => {
                            self.bits.drain(..off);
                            self.lock = Some(wl);
                            self.word = 0;
                            self.err = 0.0;
                            self.figs = false;
                            let (b, sh) =
                                self.lock_info.map_or((0.0, 0.0), |l| (l.baud(), l.shift()));
                            let detail = self.detail_of();
                            sink.lock(
                                format!(
                                    "{} word sync: {b:.1} Bd, shift {sh:.0} Hz, {detail} ({:.0} % of the words pass)",
                                    self.name_of(),
                                    100.0 * score.min(1.0)
                                ),
                                detail,
                            );
                        }
                        None => {
                            self.bits.drain(..7);
                        }
                    }
                }
                Some(wl) => {
                    if self.bits.len() < 7 * wl.channels {
                        return;
                    }
                    if !self.read(wl, sink) {
                        self.lock = None;
                        sink.lose("word sync lost");
                    }
                }
            }
        }
    }
    fn decoding(&self) -> bool {
        self.lock.is_some()
    }
    fn detail(&self) -> String {
        self.detail_of()
    }
    fn reset(&mut self) {
        self.bits.clear();
        self.lock = None;
        self.err = 0.0;
        self.figs = false;
    }
}

/// The submode of a configuration: the submode named, else the `submode` setting.
fn submode_of(sub: Option<&str>, settings: &Values) -> Submode {
    sub.and_then(Submode::from_key)
        .unwrap_or_else(|| settings.get(&SUBMODE))
}

/// The idle word of `code`: with the parity code a service signal of elements 10101, with F.342 beta.
fn idle_word(code: WordCode) -> u8 {
    match code {
        WordCode::Parity => ident_parity::word(21, true),
        WordCode::F342 => f342::BETA,
    }
}

/// The words of the test signal: `idle` idle words, `text` after a LTRS, `idle` idle words.
pub fn test_words(text: &str, code: WordCode, idle: usize) -> Vec<u8> {
    let i = idle_word(code);
    let mut w = vec![i; idle];
    w.extend(ita2_codes(text).into_iter().map(|c| match code {
        WordCode::Parity => ident_parity::word(c, false),
        WordCode::F342 => f342::word(c),
    }));
    w.extend(std::iter::repeat_n(i, idle));
    w
}

/// The text of channel `ch` (A = 0) of the test signal: the test text on A, the channel's name and the test text on the others.
pub fn channel_text(ch: usize) -> String {
    let t = crate::serial::TEST_TEXT;
    match ch {
        0 => t.to_string(),
        _ => format!("CHANNEL {} {t}", ["A", "B", "C", "D"][ch.min(3)]),
    }
}

/// The units of the test signal on `n` channels, unit 1 of each word first: [`test_words`] of each channel's text with 200 / `n`
/// idle words on each side, the shorter channels filled with idle words; in ARQ-E each channel inverts its words in its places of
/// a cycle of `cycle` (the first character of a cycle is the last of each group).
pub fn test_units(n: usize, code: WordCode, cycle: usize) -> Vec<u8> {
    let mut ch: Vec<Vec<u8>> = (0..n)
        .map(|c| test_words(&channel_text(c), code, 200 / n))
        .collect();
    let len = ch.iter().map(Vec::len).max().unwrap_or(0);
    for w in &mut ch {
        w.resize(len, idle_word(code));
    }
    let mut units = Vec::with_capacity(7 * n * len);
    let mut frame = vec![0u8; 7 * n];
    for k in 0..len {
        let first = cycle > 0 && k % cycle == cycle - 1;
        for (c, words) in ch.iter().enumerate() {
            let w = if inverted_place(cycle, c, first) {
                !words[k] & 0x7F
            } else {
                words[k]
            };
            for u in 0..7 {
                frame[frame_place(n, c, u)] = (w >> u) & 1;
            }
        }
        units.extend_from_slice(&frame);
    }
    units
}

/// The test signal: [`test_units`] of the test text on `gen_channels` channels, unit 1 first, the higher tone for 1.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let sub = submode_of(cfg.submode.as_deref(), s);
    let baud = s.get(&GEN_BAUD);
    let shift = s.get(&GEN_SHIFT_HZ);
    let c = s.get(&GEN_CENTRE_HZ);
    let cycle = match (sub, s.get(&GEN_CYCLE)) {
        (Submode::Arqn, _) | (_, Cycle::None) => 0,
        (_, Cycle::C4) => 4,
        _ => 8,
    };
    let n = match s.get(&GEN_CHANNELS) {
        Channels::Two => 2,
        Channels::Four => 4,
        _ => 1,
    };
    let seq: Vec<_> = test_units(n, s.get(&GEN_WORD_CODE), cycle)
        .into_iter()
        .map(|b| {
            let f = if b == 1 {
                c + shift / 2.0
            } else {
                c - shift / 2.0
            };
            (1.0 / baud, Some(f))
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
    ArqeSetting::schema()
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
        crate::typed::shape("arqe"),
        "ARQ-E",
        TtyDecoder::new(Arqe::new()),
        &with_submode(cfg)?,
    )
}

/// ARQ-E and ARQ-N.
pub static ARQE: StackDescriptor = StackDescriptor {
    id: "arqe",
    name: "ARQ-E / ARQ-N",
    group: "HF utility",
    blurb: "Duplex ARQ teleprinter on 2-FSK with 7-unit words: speed, shift, word alignment, polarity and the inversion cycle found; ITA2 text.",
    tuning: crate::ms110::hf_tuning(crate::ms110::TEST_SIGNAL_HZ),
    submodes: &[
        SubMode {
            id: "arqe",
            name: "ARQ-E",
            default_hz: crate::ms110::TEST_SIGNAL_HZ,
        },
        SubMode {
            id: "arqn",
            name: "ARQ-N",
            default_hz: crate::ms110::TEST_SIGNAL_HZ,
        },
    ],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::ARQE),
    benchmarks: Some(crate::benchmarks),
};
