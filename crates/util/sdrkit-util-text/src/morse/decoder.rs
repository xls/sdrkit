//! The Morse timing decoder: it reads the lengths of marks and spaces, follows the sending speed, and returns characters with a
//! confidence and the best other reading. While it does not know the speed, it holds the elements of a word and fits the dot
//! length to the whole word, so a word of dots only or dashes only reads right. A space before the first mark is a pause. Until
//! both classes of spaces are learned, spaces are read against the standard spacing; a stretched space that can be a
//! character or a word space holds the output behind it until a second space length decides it, for at most three characters.
//!
//! Standard: ITU-R M.1677-1 Part I section 2 (a dash is three dots, spaces of one, three and seven dots).

use super::{decode, Code};
use crate::error::{Error, Result};
use sdrkit_algo_dsp::kmeans::kmeans_1d_fixed;

/// The number of bins of the mark and space histograms.
pub const HIST_BINS: usize = 40;
/// The lowest value of the histograms: ln(length / dot length).
pub const HIST_MIN: f64 = -1.6;
/// The span of the histograms in ln(length / dot length). Bin `i` starts at `HIST_MIN + i * HIST_SPAN / HIST_BINS`.
pub const HIST_SPAN: f64 = 4.4;

/// The most elements of one held word. A longer word is decoded when the store is full.
const WORD_CAP: usize = 128;
/// The marks kept for the two-cluster split.
const MARKS: usize = 24;
/// The long spaces kept to tell character spaces from word spaces.
const GAPS: usize = 16;
/// A space is a pause when it is longer than this many dot lengths and [`PAUSE_CHAR_GAPS`] character spaces. A pause gives
/// one word space and is not kept.
const PAUSE_DOTS: f64 = 10.0;
/// See [`PAUSE_DOTS`]. The bound follows the character space, so stretched (Farnsworth) spacing is kept.
const PAUSE_CHAR_GAPS: f64 = 3.0;
/// This many pauses in a row, with no kept space between, mean the spacing has changed: the kept spaces are dropped.
const PAUSE_RUN: u32 = 4;
/// The least ratio of the word space to the character space for two classes of kept spaces.
const CLASS_RATIO: f64 = 1.6;
/// The samples that each class of kept spaces needs before its own thresholds are used.
const MIN_CLASS: usize = 2;
/// Standard spacing (dot lengths): the split between an element space (1) and a character space (3), sqrt(3).
const STD_INTRA: f64 = 1.732_050_8;
/// Standard spacing (dot lengths): the split between a character space (3) and a word space (7), sqrt(21). A space below it
/// is a standard character space; a space above it with no shorter space known can be a stretched (Farnsworth) one.
const STD_WORD: f64 = 4.582_575_7;
/// The most characters held behind a space that is not yet known as a character or a word space. Then the held spaces are
/// taken as character spaces.
const HOLD_CHARS: u32 = 3;
/// The store of held characters and spaces.
const HOLD_CAP: usize = 2 * HOLD_CHARS as usize + 2;

/// How a mark is called a dot or a dash once the speed is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Classifier {
    /// A log-domain classifier that learns the dash-to-dot ratio and the spread of the sender (hand keying).
    #[default]
    Adaptive,
    /// A threshold half way between the two clusters of recent marks.
    Clusters,
}

/// When the decoder holds a word and fits the dot length to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WordFit {
    /// Only while the speed is not known. After that, characters come out as they end.
    #[default]
    Unlocked,
    /// Every word. Use it for identifications that come one word at a time, at any speed.
    Always,
}

/// The settings of a [`Decoder`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecoderConfig {
    /// The speed at the start, words a minute (PARIS timing).
    pub wpm: f64,
    /// If true, the speed stays at `wpm`.
    pub fixed: bool,
    /// The slowest speed followed, words a minute. A held word that fits a slower speed is dropped.
    pub min_wpm: f64,
    /// The fastest speed followed, words a minute. A held word that fits a faster speed is dropped.
    pub max_wpm: f64,
    /// The mark classifier.
    pub classifier: Classifier,
    /// When words are held and fitted.
    pub word_fit: WordFit,
    /// A space of this many seconds always ends a held word.
    pub word_gap_s: f64,
}

impl Default for DecoderConfig {
    fn default() -> Self {
        DecoderConfig::following(20.0)
    }
}

impl DecoderConfig {
    /// Follows the speed from 4 to 60 words a minute, starting at `wpm`.
    pub fn following(wpm: f64) -> Self {
        DecoderConfig {
            wpm,
            fixed: false,
            min_wpm: 4.0,
            max_wpm: 60.0,
            classifier: Classifier::Adaptive,
            word_fit: WordFit::Unlocked,
            word_gap_s: 2.0,
        }
    }

    /// Stays at `wpm` words a minute.
    pub fn fixed(wpm: f64) -> Self {
        DecoderConfig {
            fixed: true,
            min_wpm: wpm,
            max_wpm: wpm,
            ..DecoderConfig::following(wpm)
        }
    }

    /// For identifications: every word is fitted on its own, from 2 to 40 words a minute.
    pub fn idents() -> Self {
        DecoderConfig {
            min_wpm: 2.0,
            max_wpm: 40.0,
            word_fit: WordFit::Always,
            ..DecoderConfig::following(10.0)
        }
    }
}

/// One decoded character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Symbol {
    /// The elements received.
    pub code: Code,
    /// The character or `<XX>` procedure signal. `None` if the table has no such code.
    pub text: Option<&'static str>,
    /// How sure the decoder is, 0 to 9.
    pub confidence: u8,
    /// The reading with the least sure element changed, if the table has it.
    pub alt: Option<&'static str>,
}

impl Symbol {
    /// The text, or `unknown` if the table has no such code.
    pub fn text_or(&self, unknown: &'static str) -> &'static str {
        self.text.unwrap_or(unknown)
    }
}

/// What the decoder returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// A character.
    Char(Symbol),
    /// A word space.
    Space,
}

/// An output that waits for the class of a space.
#[derive(Clone, Copy, Debug)]
enum Held {
    Char(Symbol),
    /// A space of this many dot lengths, character or word space not yet known.
    Gap(f64),
}

/// The streaming Morse decoder. It holds no allocation.
///
/// Feed it the length of every mark and space with [`Decoder::push`], and the time the key stays up with [`Decoder::idle`]. It
/// passes each [`Event`] to a callback.
///
/// ```
/// use sdrkit_util_text::morse::{Decoder, DecoderConfig, Event, Keyer};
///
/// // A word of dots only at 6 words a minute. The decoder starts at 20.
/// let mut d = Decoder::new(DecoderConfig::following(20.0)).unwrap();
/// let mut text = String::new();
/// let mut put = |e: Event| match e {
///     Event::Char(s) => text.push_str(s.text_or("*")),
///     Event::Space => text.push(' '),
/// };
/// for k in Keyer::new(6.0).unwrap().key("EEE") {
///     d.push(k.on, k.seconds, &mut put);
/// }
/// d.flush(&mut put);
/// assert_eq!(text, "EEE");
/// ```
#[derive(Clone, Debug)]
pub struct Decoder {
    cfg: DecoderConfig,
    min_dot: f64,
    max_dot: f64,
    dot: f64,
    ratio: f64,
    spread: f64,
    steadiness: f64,
    marks: [f64; MARKS],
    n_marks: usize,
    gaps: [f64; GAPS],
    n_gaps: usize,
    pauses: u32,
    char_gap: f64,
    word_gap: f64,
    // The kept spaces form two classes.
    two_classes: bool,
    // Two classes with MIN_CLASS samples each: their own thresholds are used.
    learned: bool,
    // A kept space is below STD_WORD: standard spacing.
    std_spacing: bool,
    // A space above STD_WORD was taken as a character space: stretched spacing.
    wide_letter: bool,
    started: bool,
    held: [Held; HOLD_CAP],
    n_held: usize,
    held_chars: u32,
    held_max: f64,
    code: u32,
    len: u32,
    conf: f64,
    weakest: (f64, u32),
    pending: Option<f64>,
    spaced: bool,
    locked: bool,
    seen_dot: bool,
    seen_dash: bool,
    same: u32,
    last_dash: bool,
    mark_hist: [u32; HIST_BINS],
    gap_hist: [u32; HIST_BINS],
    unknown: u64,
    dropped: u64,
    word: [(bool, f64); WORD_CAP],
    n_word: usize,
}

impl Decoder {
    /// A decoder with `cfg`. Errors: speeds outside 1 to 200 words a minute, `min_wpm` over `wpm` or `wpm` over `max_wpm`, or a
    /// word gap that is not a positive number.
    pub fn new(cfg: DecoderConfig) -> Result<Decoder> {
        let ok = (1.0..=200.0).contains(&cfg.min_wpm)
            && (cfg.min_wpm..=cfg.max_wpm).contains(&cfg.wpm)
            && cfg.max_wpm <= 200.0
            && cfg.word_gap_s.is_finite()
            && cfg.word_gap_s > 0.0;
        if !ok {
            return Err(Error::OutOfRange);
        }
        let dot = 1.2 / cfg.wpm;
        Ok(Decoder {
            cfg,
            min_dot: 1.2 / cfg.max_wpm,
            max_dot: 1.2 / cfg.min_wpm,
            dot,
            ratio: 3.0,
            spread: 0.25,
            steadiness: 0.5,
            marks: [0.0; MARKS],
            n_marks: 0,
            gaps: [0.0; GAPS],
            n_gaps: 0,
            pauses: 0,
            char_gap: 3.0,
            word_gap: 7.0,
            two_classes: false,
            learned: false,
            std_spacing: false,
            wide_letter: false,
            started: false,
            held: [Held::Gap(0.0); HOLD_CAP],
            n_held: 0,
            held_chars: 0,
            held_max: 0.0,
            code: 0,
            len: 0,
            conf: 1.0,
            weakest: (1.0, 0),
            pending: None,
            spaced: true,
            locked: cfg.fixed,
            seen_dot: false,
            seen_dash: false,
            same: 0,
            last_dash: false,
            mark_hist: [0; HIST_BINS],
            gap_hist: [0; HIST_BINS],
            unknown: 0,
            dropped: 0,
            word: [(false, 0.0); WORD_CAP],
            n_word: 0,
        })
    }

    /// The settings.
    pub fn config(&self) -> &DecoderConfig {
        &self.cfg
    }

    /// Forgets the speed, the statistics and the character being received.
    pub fn reset(&mut self) {
        if let Ok(d) = Decoder::new(self.cfg) {
            *self = d;
        }
    }

    /// The dot length now followed, seconds.
    pub fn dot(&self) -> f64 {
        self.dot
    }

    /// The speed now followed, words a minute. Before [`Decoder::is_locked`] it is the start speed or the fit of the last word.
    pub fn wpm(&self) -> f64 {
        1.2 / self.dot
    }

    /// Returns true when the decoder has measured the speed (it has seen dots and dashes), or the speed is fixed.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// The dash length over the dot length that the sender uses.
    pub fn dash_ratio(&self) -> f64 {
        self.ratio
    }

    /// The space between characters, dot lengths.
    pub fn char_gap(&self) -> f64 {
        self.char_gap
    }

    /// The space between words, dot lengths.
    pub fn word_gap(&self) -> f64 {
        self.word_gap
    }

    /// True for Farnsworth spacing: the speed is known and the space between characters is over 4.2 dot lengths.
    pub fn farnsworth(&self) -> bool {
        self.locked && self.char_gap > 4.2
    }

    /// How steady the keying is, 0 to 1 (1 is a machine).
    pub fn steadiness(&self) -> f64 {
        self.steadiness
    }

    /// The histogram of ln(mark / dot length). See [`HIST_MIN`] and [`HIST_SPAN`].
    pub fn mark_histogram(&self) -> &[u32; HIST_BINS] {
        &self.mark_hist
    }

    /// The histogram of ln(space / dot length).
    pub fn gap_histogram(&self) -> &[u32; HIST_BINS] {
        &self.gap_hist
    }

    /// The characters decoded with a code the table does not have.
    pub fn unknown(&self) -> u64 {
        self.unknown
    }

    /// The held words dropped because their speed was outside `min_wpm` to `max_wpm`.
    pub fn dropped_words(&self) -> u64 {
        self.dropped
    }

    /// True while a character or a held word waits for its end, or output waits for the class of a space.
    pub fn is_busy(&self) -> bool {
        self.len > 0 || self.pending.is_some() || self.n_word > 0 || self.n_held > 0
    }

    /// One interval that ended: a mark (`on`) or a space, `seconds` long. Events go to `out`. Lengths that are not positive
    /// numbers are ignored.
    pub fn push(&mut self, on: bool, seconds: f64, out: &mut impl FnMut(Event)) {
        if !(seconds.is_finite() && seconds > 0.0) {
            return;
        }
        let glitch = seconds < self.dot / 3.0 && self.locked;
        if on {
            // A spike shorter than a third of a dot is dropped.
            if glitch && !self.cfg.fixed {
                return;
            }
            self.pending = Some(self.pending.unwrap_or(0.0) + seconds);
            return;
        }
        if glitch {
            // A drop-out inside a mark joins the mark.
            if let Some(p) = &mut self.pending {
                *p += seconds;
            }
            return;
        }
        if let Some(m) = self.pending.take() {
            self.mark_in(m, out);
        }
        self.space_in(seconds, out);
    }

    /// The key is up and has been up for `seconds`. Call it while the key stays up: it ends the character and the word in time.
    pub fn idle(&mut self, seconds: f64, out: &mut impl FnMut(Event)) {
        if !(seconds.is_finite() && seconds > 0.0) {
            return;
        }
        if self.holding() && (self.n_word > 0 || self.pending.is_some()) {
            if let Some(m) = self.pending.take() {
                self.mark_in(m, out);
            }
            if self.n_word > 0 && self.word_ends(seconds) {
                self.finish_word(out);
            }
        } else if seconds / self.dot > 1.0 + self.char_gap.max(2.5)
            && (self.len > 0 || self.pending.is_some())
        {
            if let Some(m) = self.pending.take() {
                self.mark(m);
            }
            self.take_char(out);
        }
        let dots = seconds / self.dot;
        if self.n_held > 0 && self.ends_hold(seconds, dots) {
            self.release(out);
        }
        let word =
            seconds >= self.cfg.word_gap_s || self.word_bound().is_some_and(|t| dots > 1.5 * t);
        if word && !self.spaced && !self.is_busy() {
            self.spaced = true;
            out(Event::Space);
        }
    }

    /// Ends the character and the word being received (the end of a transmission). Held output is sent, its open spaces as
    /// character spaces.
    pub fn flush(&mut self, out: &mut impl FnMut(Event)) {
        if let Some(m) = self.pending.take() {
            self.mark_in(m, out);
        }
        self.finish_word(out);
        self.take_char(out);
        self.release(out);
    }

    fn holding(&self) -> bool {
        !self.cfg.fixed && (self.cfg.word_fit == WordFit::Always || !self.locked)
    }

    fn mark_in(&mut self, m: f64, out: &mut impl FnMut(Event)) {
        self.started = true;
        if self.holding() {
            self.word[self.n_word] = (true, m);
            self.n_word += 1;
            if self.n_word == WORD_CAP {
                self.finish_word(out);
            }
        } else {
            self.mark(m);
        }
    }

    /// True if a space of `seconds` ends the held word.
    /// The space is fitted with the word, so a lone character of one kind (S, or TTT) reads at the speed its spaces give.
    fn word_ends(&mut self, seconds: f64) -> bool {
        if seconds >= self.cfg.word_gap_s {
            return true;
        }
        // The store always has room for one more element: a full store is decoded at once.
        self.word[self.n_word] = (false, seconds);
        fit_dot(&self.word[..=self.n_word], self.dot).is_some_and(|u| seconds >= 5.0 * u)
    }

    /// A space that ended. A space before the first mark is a pause: it is not learned and gives no event.
    fn space_in(&mut self, s: f64, out: &mut impl FnMut(Event)) {
        if !self.started {
            return;
        }
        if self.holding() && self.n_word > 0 {
            if !self.word_ends(s) {
                self.word[self.n_word] = (false, s);
                self.n_word += 1;
                if self.n_word == WORD_CAP {
                    self.finish_word(out);
                }
                return;
            }
            self.finish_word(out);
        }
        let dots = s / self.dot;
        Self::hist(&mut self.gap_hist, dots.ln());
        if !self.holding() && self.is_intra(dots) {
            self.learn_dot(s, 0.10);
            return;
        }
        self.take_char(out);
        if self.n_held > 0 && self.ends_hold(s, dots) {
            self.release(out);
        }
        let pause = dots > self.pause_bound() || s >= self.cfg.word_gap_s;
        self.note_gap(dots);
        self.settle(out);
        let word = if pause {
            Some(true)
        } else {
            self.word_bound().map(|t| dots > t)
        };
        match word {
            Some(true) => {
                self.release(out);
                if !self.spaced {
                    self.spaced = true;
                    out(Event::Space);
                }
            }
            Some(false) => {}
            None => self.hold_gap(dots, out),
        }
    }

    fn is_intra(&self, dots: f64) -> bool {
        match self.cfg.classifier {
            // The geometric mean of one dot and the character space, within 1.6 to 2.5 dot lengths.
            Classifier::Adaptive if self.learned => dots < self.char_gap.sqrt().clamp(1.6, 2.5),
            Classifier::Adaptive => dots < STD_INTRA,
            Classifier::Clusters => dots < 2.0,
        }
    }

    /// The space length (dot lengths) above which a space is a word space, or `None` while that is not known: the kept
    /// spaces are all above [`STD_WORD`] and none was taken as a character space.
    ///
    /// With two learned classes it is the geometric mean of the character and word spaces. Before that, a kept standard
    /// character space gives the standard split [`STD_WORD`]; two space lengths (stretched spacing), or stretched spaces
    /// taken as character spaces, give the split from the kept spaces.
    fn word_bound(&self) -> Option<f64> {
        if self.learned {
            Some(self.word_threshold())
        } else if self.std_spacing {
            Some(STD_WORD)
        } else if self.two_classes || self.wide_letter {
            Some(self.word_threshold())
        } else {
            None
        }
    }

    /// True if a key-up of `seconds` (`dots` dot lengths) ends the hold: it reaches the word gap of the settings, or it is
    /// longer than every held space by [`CLASS_RATIO`], so the held spaces are the shorter class.
    fn ends_hold(&self, seconds: f64, dots: f64) -> bool {
        seconds >= self.cfg.word_gap_s || dots >= CLASS_RATIO * self.held_max
    }

    /// Sends a character, or holds it while an earlier space waits for its class. The hold ends after [`HOLD_CHARS`].
    fn emit(&mut self, s: Symbol, out: &mut impl FnMut(Event)) {
        if self.n_held == 0 {
            out(Event::Char(s));
            return;
        }
        if self.n_held == HOLD_CAP {
            self.release(out);
            out(Event::Char(s));
            return;
        }
        self.held[self.n_held] = Held::Char(s);
        self.n_held += 1;
        self.held_chars += 1;
        if self.held_chars >= HOLD_CHARS {
            self.release(out);
        }
    }

    /// Holds a space of `dots` dot lengths whose class is not known, and the output after it.
    fn hold_gap(&mut self, dots: f64, out: &mut impl FnMut(Event)) {
        if self.n_held == HOLD_CAP {
            self.release(out);
        }
        self.held[self.n_held] = Held::Gap(dots);
        self.n_held += 1;
        self.held_max = self.held_max.max(dots);
    }

    /// Sends the held output if the class of its spaces is now known.
    fn settle(&mut self, out: &mut impl FnMut(Event)) {
        if self.n_held > 0 {
            if let Some(t) = self.word_bound() {
                self.send_held(t, out);
            }
        }
    }

    /// Sends the held output with its spaces taken as character spaces (stretched spacing).
    fn release(&mut self, out: &mut impl FnMut(Event)) {
        if self.n_held > 0 {
            self.wide_letter = true;
            self.send_held(f64::INFINITY, out);
        }
    }

    /// Sends the held output in order; a held space over `bound` dot lengths is a word space.
    fn send_held(&mut self, bound: f64, out: &mut impl FnMut(Event)) {
        let n = self.n_held;
        self.n_held = 0;
        self.held_chars = 0;
        self.held_max = 0.0;
        for i in 0..n {
            match self.held[i] {
                Held::Char(s) => {
                    self.spaced = false;
                    out(Event::Char(s));
                }
                Held::Gap(g) if g > bound && !self.spaced => {
                    self.spaced = true;
                    out(Event::Space);
                }
                Held::Gap(_) => {}
            }
        }
    }

    /// Decodes the held word with the dot length fitted to it.
    fn finish_word(&mut self, out: &mut impl FnMut(Event)) {
        let n = self.n_word;
        self.n_word = 0;
        if n == 0 {
            return;
        }
        let u = fit_dot(&self.word[..n], self.dot).unwrap_or(self.dot);
        if !(self.min_dot * 0.999..=self.max_dot * 1.001).contains(&u) {
            self.dropped += 1;
            return;
        }
        self.dot = u;
        // Half way between one and three dot lengths: a dot lengthened by a slow key edge still reads as a dot.
        let t = 2.0 * u;
        for i in 0..n {
            let (on, d) = self.word[i];
            if on {
                let dash = d >= t;
                self.note_mark(d, dash, sureness(d, t));
            } else if d >= t {
                // A space inside a held word is a character space.
                self.take_char(out);
                let g = d / self.dot;
                self.note_gap(g);
                if g >= STD_WORD {
                    self.wide_letter = true;
                }
                self.settle(out);
            } else {
                self.learn_dot(d, 0.10);
            }
        }
        self.take_char(out);
    }

    fn hist(h: &mut [u32; HIST_BINS], x: f64) {
        let i = ((x - HIST_MIN) / HIST_SPAN * HIST_BINS as f64).floor();
        if (0.0..HIST_BINS as f64).contains(&i) {
            h[i as usize] += 1;
        }
    }

    fn learn_dot(&mut self, est: f64, rate: f64) {
        if self.cfg.fixed {
            return;
        }
        let x = (est / self.dot).ln().clamp(-0.7, 0.7);
        self.dot = (self.dot * (rate * x).exp()).clamp(self.min_dot, self.max_dot);
    }

    /// 2-means over the recent marks: the dot and dash means, when both kinds are there.
    fn clusters(&self) -> Option<(f64, f64)> {
        let m = &self.marks[..self.n_marks.min(MARKS)];
        let lo = m.iter().copied().fold(f64::MAX, f64::min);
        let hi = m.iter().copied().fold(0.0, f64::max);
        if m.len() < 2 || hi < 1.8 * lo {
            return None;
        }
        let c = kmeans_1d_fixed(m, [lo, hi], 8)?;
        Some((c.centres[0], c.centres[1]))
    }

    /// Classifies a mark with the known speed and learns from it.
    fn mark(&mut self, d: f64) {
        let (dash, p) = if self.cfg.classifier == Classifier::Adaptive && self.locked {
            let x = (d / self.dot).ln();
            let s = self.spread.max(0.12);
            let lr = self.ratio.ln();
            let d0 = (x * x) / (2.0 * s * s);
            let d1 = ((x - lr) * (x - lr)) / (2.0 * s * s);
            let p_dash = 1.0 / (1.0 + (d1 - d0).exp());
            (p_dash > 0.5, p_dash.max(1.0 - p_dash))
        } else {
            let t = if self.cfg.fixed || self.n_marks < 4 {
                self.dot * self.ratio.sqrt()
            } else {
                // One kind only: judge by the dot length.
                self.clusters()
                    .map_or(self.dot * 2.0, |(lo, hi)| 0.5 * (lo + hi))
            };
            (d > t, sureness(d, t))
        };
        self.note_mark(d, dash, p);
    }

    /// Takes a classified mark into the character and the statistics.
    fn note_mark(&mut self, d: f64, dash: bool, p: f64) {
        self.marks[self.n_marks % MARKS] = d;
        self.n_marks += 1;
        let x = (d / self.dot).ln();
        Self::hist(&mut self.mark_hist, x);
        // Twelve marks of one kind in a row while the marks fall in two clusters: the speed is wrong. Take it from the clusters.
        self.same = if dash == self.last_dash {
            self.same + 1
        } else {
            0
        };
        self.last_dash = dash;
        if self.same >= 12 && !self.cfg.fixed {
            if let Some((lo, hi)) = self.clusters() {
                self.dot = lo.clamp(self.min_dot, self.max_dot);
                self.ratio = (hi / lo).clamp(2.0, 5.0);
                self.same = 0;
            }
        }
        if dash {
            self.seen_dash = true;
            if self.cfg.classifier == Classifier::Adaptive && self.seen_dot {
                let r = d / self.dot;
                self.ratio += 0.04 * (r.clamp(2.0, 5.0) - self.ratio);
            }
            self.learn_dot(d / self.ratio, 0.10);
        } else {
            self.seen_dot = true;
            self.learn_dot(d, 0.10);
            let dev = x.abs();
            self.spread += 0.04 * (dev.max(0.05) - self.spread);
            self.steadiness += 0.04 * ((1.0 - dev * 4.0).clamp(0.0, 1.0) - self.steadiness);
        }
        if !self.locked && self.seen_dot && self.seen_dash && self.n_marks >= 6 {
            self.locked = true;
        }
        if self.len < 16 {
            self.code = (self.code << 1) | dash as u32;
        }
        self.len += 1;
        self.conf *= p;
        if p < self.weakest.0 {
            self.weakest = (p, self.len);
        }
        self.spaced = false;
    }

    /// Returns the character being built.
    fn take_char(&mut self, out: &mut impl FnMut(Event)) {
        if self.len == 0 {
            return;
        }
        let len = self.len.min(16);
        let code = Code {
            bits: self.code as u16,
            len: len as u8,
        };
        let text = if self.len <= 16 { decode(code) } else { None };
        let alt = match self.weakest.1 {
            w if w > 0 && self.len <= 16 => decode(Code {
                bits: code.bits ^ (1 << (self.len - w)),
                len: code.len,
            }),
            _ => None,
        };
        if text.is_none() {
            self.unknown += 1;
        }
        let confidence = (self.conf * 9.0).round().clamp(0.0, 9.0) as u8;
        self.code = 0;
        self.len = 0;
        self.conf = 1.0;
        self.weakest = (1.0, 0);
        let s = Symbol {
            code,
            text,
            confidence,
            alt,
        };
        self.emit(s, out);
    }

    /// Keeps a space between characters or words (dot lengths) and updates the character and word spaces and their classes. A
    /// pause is not kept; after [`PAUSE_RUN`] pauses in a row the kept spaces are dropped and this space starts them again.
    fn note_gap(&mut self, dots: f64) {
        if dots > self.pause_bound() {
            self.pauses += 1;
            if self.pauses < PAUSE_RUN {
                return;
            }
            self.n_gaps = 0;
            self.wide_letter = false;
        }
        self.pauses = 0;
        self.gaps[self.n_gaps % GAPS] = dots;
        self.n_gaps += 1;
        let n = self.n_gaps.min(GAPS);
        let mut v = self.gaps;
        let (c, w, low, high) = spacing(&mut v[..n]);
        (self.char_gap, self.word_gap) = (c, w);
        self.two_classes = low > 0;
        self.learned = low >= MIN_CLASS && high >= MIN_CLASS;
        self.std_spacing = v[0] < STD_WORD;
    }

    /// The space length (dot lengths) above which a space is a pause. Before any space is kept there is no bound, so the first
    /// space of stretched spacing is kept.
    fn pause_bound(&self) -> f64 {
        if self.n_gaps == 0 {
            f64::INFINITY
        } else {
            pause_bound(self.char_gap)
        }
    }

    /// The space length (dot lengths) above which a space ends a word: the geometric mean of the character and word spaces.
    fn word_threshold(&self) -> f64 {
        (self.char_gap * self.word_gap).sqrt()
    }
}

/// The pause bound for a character space of `char_gap` dot lengths. See [`PAUSE_DOTS`].
fn pause_bound(char_gap: f64) -> f64 {
    (PAUSE_CHAR_GAPS * char_gap).max(PAUSE_DOTS)
}

/// The character and word spaces (dot lengths) of the kept spaces `v`, which must not be empty, and the sizes of the lower and
/// upper class (0 and the count for one class). `v` is sorted in place.
///
/// The kept spaces are split into two classes; the lower class gives the character space and with it the pause bound. A lone
/// space below two or more others is left out if it is longer than a standard character space or the others still form two
/// classes; spaces over the bound are left out; then the split is made again. So neither a pause nor one short stretched space
/// among the kept spaces moves the estimates, and a lone standard character space is never lost.
fn spacing(v: &mut [f64]) -> (f64, f64, usize, usize) {
    v.sort_unstable_by(f64::total_cmp);
    let (mut a, mut b) = (0, v.len());
    let (mut c, mut w, mut low) = classes(&v[a..b]);
    // Each pass drops one space or more, and keeps the lowest space under the bound.
    for _ in 0..GAPS {
        if low == 1 && b - a >= 3 && (v[a] >= STD_WORD || classes(&v[a + 1..b]).2 > 0) {
            a += 1;
        } else {
            let m = a + v[a..b].partition_point(|&g| g <= pause_bound(c));
            if m == b {
                break;
            }
            b = m;
        }
        (c, w, low) = classes(&v[a..b]);
    }
    let high = b - a - low;
    (c, w, low, high)
}

/// The character and word spaces of the sorted spaces `v` (not empty), and the size of the lower class (0 for one class).
///
/// The split is the one with the least spread in the log domain; it is taken if the two classes differ by [`CLASS_RATIO`] or
/// more. One class is taken as character spaces (most spaces are), and the word space as 7/3 of it.
fn classes(v: &[f64]) -> (f64, f64, usize) {
    let n = v.len();
    let mut ls = [0.0; GAPS];
    for (l, &g) in ls.iter_mut().zip(v) {
        *l = g.ln();
    }
    let ls = &ls[..n];
    let mean = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;
    let spread = |s: &[f64]| {
        let m = mean(s);
        s.iter().map(|x| (x - m) * (x - m)).sum::<f64>()
    };
    let mut best: Option<(f64, usize)> = None;
    for i in 1..n {
        let e = spread(&ls[..i]) + spread(&ls[i..]);
        if best.is_none_or(|(b, _)| e < b) {
            best = Some((e, i));
        }
    }
    if let Some((_, i)) = best {
        let (lo, hi) = (mean(&ls[..i]), mean(&ls[i..]));
        if hi - lo >= CLASS_RATIO.ln() {
            return (lo.exp(), hi.exp(), i);
        }
    }
    let m = mean(ls).exp();
    (m, m * 7.0 / 3.0, 0)
}

/// How sure a decision is for length `d` against threshold `t`: 0.5 at the threshold, near 1 far from it.
fn sureness(d: f64, t: f64) -> f64 {
    ((d / t).ln().abs() * 3.0).tanh() * 0.5 + 0.5
}

/// The dot length that fits the marks and spaces of one word best, seconds.
///
/// Each mark is one or three dot lengths and each space one, three or seven, on a log scale (ITU-R M.1677-1 Part I section 2).
/// When two dot lengths fit equally well (a word of dashes and character spaces only), the one nearer `prior` wins. `elements`
/// holds (key down, seconds). Returns `None` with fewer than two elements of a positive length.
///
/// ```
/// use sdrkit_util_text::morse::{fit_dot, Keyer};
/// let k: Vec<(bool, f64)> = Keyer::new(12.0).unwrap().key("TOM").map(|k| (k.on, k.seconds)).collect();
/// assert!((fit_dot(&k, 0.06).unwrap() - 0.1).abs() < 1e-9);
/// ```
pub fn fit_dot(elements: &[(bool, f64)], prior: f64) -> Option<f64> {
    let ok = |d: f64| d.is_finite() && d > 0.0;
    if elements.iter().filter(|e| ok(e.1)).count() < 2 {
        return None;
    }
    let cost = |u: f64| -> f64 {
        let lu = u.ln();
        elements
            .iter()
            .filter(|e| ok(e.1))
            .map(|&(on, d)| {
                let l = d.ln() - lu;
                let a = l * l;
                let b = (l - 3f64.ln()) * (l - 3f64.ln());
                let c = if on {
                    f64::MAX
                } else {
                    (l - 7f64.ln()) * (l - 7f64.ln())
                };
                a.min(b).min(c)
            })
            .sum()
    };
    let lp = if ok(prior) { prior.ln() } else { 0.0 };
    let mut best = (f64::MAX, 0.0f64);
    for &(on, d) in elements.iter().filter(|e| ok(e.1)) {
        let units: &[f64] = if on { &[1.0, 3.0] } else { &[1.0, 3.0, 7.0] };
        for &k in units {
            let u = d / k;
            let c = cost(u);
            let tie = (c - best.0).abs() <= 1e-9 * (1.0 + best.0);
            if (!tie && c < best.0) || (tie && (u.ln() - lp).abs() < (best.1.ln() - lp).abs()) {
                best = (c, u);
            }
        }
    }
    // The mean of the elements, each divided by the units it was read as.
    let u0 = best.1;
    let (mut s, mut n) = (0.0, 0);
    for &(on, d) in elements.iter().filter(|e| ok(e.1)) {
        let r = d / u0;
        let k = if r < 2.0 {
            1.0
        } else if on || r < 5.0 {
            3.0
        } else {
            7.0
        };
        s += d / k;
        n += 1;
    }
    Some(s / n as f64)
}

#[cfg(test)]
mod tests {
    use super::super::{Key, Keyer};
    use super::*;

    fn text_of(d: &mut Decoder, keys: impl Iterator<Item = Key>, jitter: f64, seed: u32) -> String {
        let mut s = String::new();
        let mut x = seed;
        let mut put = |e: Event| match e {
            Event::Char(c) => s.push_str(c.text_or("*")),
            Event::Space => s.push(' '),
        };
        for k in keys {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let r = ((x >> 8) as f64 / (1u32 << 24) as f64 - 0.5) * 2.0 * jitter;
            d.push(k.on, k.seconds * (1.0 + r), &mut put);
        }
        d.idle(5.0, &mut put);
        d.flush(&mut put);
        s.trim_end().to_string()
    }

    fn cfg(wpm: f64) -> DecoderConfig {
        DecoderConfig::following(wpm)
    }

    /// Decodes `keys`, each length scaled by a uniform factor in 1 ± `jitter`. With `idle_step`, the decoder also gets
    /// [`Decoder::idle`] calls every `idle_step` seconds of each space before the space itself, as a live key would give.
    fn decode(
        d: &mut Decoder,
        keys: &[Key],
        jitter: f64,
        seed: u32,
        idle_step: Option<f64>,
    ) -> String {
        let mut s = String::new();
        let mut x = seed;
        let mut put = |e: Event| match e {
            Event::Char(c) => s.push_str(c.text_or("*")),
            Event::Space => s.push(' '),
        };
        for k in keys {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let r = ((x >> 8) as f64 / (1u32 << 24) as f64 - 0.5) * 2.0 * jitter;
            let len = k.seconds * (1.0 + r);
            if let (false, Some(step)) = (k.on, idle_step) {
                let mut t = step;
                while t < len {
                    d.idle(t, &mut put);
                    t += step;
                }
            }
            d.push(k.on, len, &mut put);
        }
        d.idle(60.0, &mut put);
        d.flush(&mut put);
        s.trim_end().to_string()
    }

    /// The key intervals of `parts`: (keyer, text) in turn, joined by key-up spaces of `pause` seconds.
    fn keyed(parts: &[(Keyer, &str)], pause: f64) -> Vec<Key> {
        let mut v = Vec::new();
        for (i, (k, t)) in parts.iter().enumerate() {
            if i > 0 {
                v.push(Key {
                    on: false,
                    seconds: pause,
                });
            }
            v.extend(k.key(t));
        }
        v
    }

    const CALL: &str = "TEST DE N0XYZ 599";

    /// The decoder settings a stack uses: following from 20 WPM and fixed at `wpm`, with each classifier.
    fn configs(wpm: f64) -> [DecoderConfig; 4] {
        let with = |c: DecoderConfig, classifier| DecoderConfig { classifier, ..c };
        [
            with(cfg(20.0), Classifier::Adaptive),
            with(cfg(20.0), Classifier::Clusters),
            with(DecoderConfig::fixed(wpm), Classifier::Adaptive),
            with(DecoderConfig::fixed(wpm), Classifier::Clusters),
        ]
    }

    /// Decodes `text` from `keyer`, after a key-up of `lead` dot lengths, for every setting of [`configs`], fed as whole
    /// intervals and live, without jitter and with ±15 % over 20 seeds. Returns the cases that do not give `text` exactly.
    fn exact_from_start(keyer: Keyer, char_wpm: f64, text: &str, lead: Option<f64>) -> Vec<String> {
        let mut keys = Vec::new();
        if let Some(l) = lead {
            keys.push(Key {
                on: false,
                seconds: l * keyer.dot(),
            });
        }
        keys.extend(keyer.key(text));
        let mut bad = Vec::new();
        for c in configs(char_wpm) {
            for idle_step in [None, Some(0.01)] {
                for (jitter, seed) in std::iter::once((0.0, 1)).chain((1..=20).map(|s| (0.15, s))) {
                    let mut d = Decoder::new(c).unwrap();
                    let got = decode(&mut d, &keys, jitter, seed, idle_step);
                    if got != text {
                        bad.push(format!(
                            "{:?} fixed {}, lead {lead:?}, idle {idle_step:?}, jitter {jitter}, seed {seed}: {got:?}",
                            c.classifier, c.fixed
                        ));
                    }
                }
            }
        }
        bad
    }

    /// The start of a transmission as a CW stack feeds it: a key-up of 9.87 dot lengths before the first mark, then "CQ" with
    /// element spaces of 1.0 and a character space of 2.98, a word space of 6.92, then "DE". The leading key-up is not a
    /// sample of the spacing.
    #[test]
    fn leading_pause_is_not_learned() {
        let dot = 0.06;
        let code = |c: char| match c {
            'C' => "-.-.",
            'Q' => "--.-",
            'D' => "-..",
            _ => ".",
        };
        let mut keys = vec![(false, 9.87)];
        for (w, word) in ["CQ", "DE"].iter().enumerate() {
            if w > 0 {
                keys.push((false, 6.92));
            }
            for (i, c) in word.chars().enumerate() {
                if i > 0 {
                    keys.push((false, 2.98));
                }
                for (j, e) in code(c).chars().enumerate() {
                    if j > 0 {
                        keys.push((false, 1.0));
                    }
                    keys.push((true, if e == '-' { 3.0 } else { 1.0 }));
                }
            }
        }
        let keys: Vec<Key> = keys
            .into_iter()
            .map(|(on, n)| Key {
                on,
                seconds: n * dot,
            })
            .collect();
        let mut bad = Vec::new();
        for c in configs(20.0) {
            for idle_step in [None, Some(0.005)] {
                let mut d = Decoder::new(c).unwrap();
                let got = decode(&mut d, &keys, 0.0, 1, idle_step);
                if got != "CQ DE" {
                    bad.push(format!("{c:?} idle {idle_step:?}: {got:?}"));
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Standard spacing is exact from the first character at 12, 20 and 40 WPM, with no leading key-up and with one of 7,
    /// 10 and 30 dot lengths.
    #[test]
    fn standard_exact_from_start() {
        let mut bad = Vec::new();
        for wpm in [12.0, 20.0, 40.0] {
            for lead in [None, Some(7.0), Some(10.0), Some(30.0)] {
                bad.extend(exact_from_start(Keyer::new(wpm).unwrap(), wpm, CALL, lead));
            }
        }
        assert!(bad.is_empty(), "{} cases:\n{}", bad.len(), bad.join("\n"));
    }

    /// A lone standard character space among word spaces stays the character space: "AN I AM" keeps 3, 7 and 7 dot lengths
    /// before the next character space.
    #[test]
    fn lone_character_space_is_kept() {
        let mut bad = Vec::new();
        for wpm in [12.0, 20.0, 40.0] {
            bad.extend(exact_from_start(
                Keyer::new(wpm).unwrap(),
                wpm,
                "AN I AM TEST",
                Some(10.0),
            ));
        }
        assert!(bad.is_empty(), "{} cases:\n{}", bad.len(), bad.join("\n"));
    }

    /// A lone low space is left out only if it is a stretched space or the rest still form two classes.
    #[test]
    fn spacing_keeps_two_classes() {
        // A standard character space below word spaces and a long word space.
        let (c, _, low, high) = spacing(&mut [2.98, 6.92, 9.87]);
        assert_eq!((low, high), (1, 2));
        assert!((c - 2.98).abs() < 1e-9, "{c}");
        let (c, _, low, high) = spacing(&mut [3.0, 7.0, 7.0]);
        assert_eq!((low, high), (1, 2));
        assert!((c - 3.0).abs() < 1e-9, "{c}");
        // One short stretched space below stretched character spaces.
        let (c, _, low, high) = spacing(&mut [5.5, 9.9, 9.9]);
        assert_eq!((low, high), (0, 2));
        assert!((c - 9.9).abs() < 1e-9, "{c}");
    }

    /// Farnsworth spacing (20 WPM characters at 10 WPM, 18 at 5) is exact from the first character: no word space after the
    /// first stretched character space.
    #[test]
    fn farnsworth_exact_from_start() {
        let mut bad = Vec::new();
        for (c, w) in [(20.0, 10.0), (18.0, 5.0)] {
            for lead in [None, Some(7.0), Some(30.0)] {
                let k = Keyer::farnsworth(c, w).unwrap();
                bad.extend(exact_from_start(k, c, "TEST DE N0XYZ", lead));
            }
        }
        assert!(bad.is_empty(), "{} cases:\n{}", bad.len(), bad.join("\n"));
    }

    /// A long pause between transmissions gives one word space and does not change how later word gaps read.
    #[test]
    fn pauses_keep_word_spaces() {
        let want = [CALL; 3].join(" ");
        let mut bad = Vec::new();
        for wpm in [12.0, 20.0, 40.0] {
            let k = Keyer::new(wpm).unwrap();
            for pause in [1.0, 2.0, 5.0, 30.0] {
                let keys = keyed(&[(k, CALL), (k, CALL), (k, CALL)], pause);
                for idle_step in [None, Some(0.01)] {
                    let mut d = Decoder::new(cfg(20.0)).unwrap();
                    let got = decode(&mut d, &keys, 0.0, 1, idle_step);
                    if got != want || (d.wpm() - wpm).abs() > 0.03 * wpm || d.word_gap() > 10.0 {
                        bad.push(format!(
                            "{wpm} WPM, pause {pause} s, idle {idle_step:?}: {got:?}, {:.1} WPM, word gap {:.1}",
                            d.wpm(),
                            d.word_gap()
                        ));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// One short space among the kept spaces does not become the character space: Farnsworth spacing (15 WPM characters at
    /// 8 WPM) whose first character space is read at 5.5 dot lengths, as a receive chain can give it. The next space comes
    /// with only that one kept, so it may read as a word space; the text is checked from the third character on.
    #[test]
    fn one_short_space_is_left_out() {
        let k = Keyer::farnsworth(15.0, 8.0).unwrap();
        let mut keys = keyed(&[(k, CALL), (k, CALL), (k, CALL)], 5.0);
        let first = keys
            .iter()
            .position(|x| !x.on && x.seconds > 2.0 * k.dot())
            .unwrap();
        keys[first].seconds = 5.5 * k.dot();
        let mut d = Decoder::new(cfg(15.0)).unwrap();
        let got = decode(&mut d, &keys, 0.0, 1, None);
        assert!(got.ends_with(&[CALL; 3].join(" ")[2..]), "{got}");
    }

    /// A change between standard and stretched spacing in one stream, both ways: after one transmission in the new spacing the
    /// text is exact again.
    #[test]
    fn spacing_change_is_followed() {
        let (std, wide) = (
            Keyer::new(20.0).unwrap(),
            Keyer::farnsworth(20.0, 10.0).unwrap(),
        );
        let two = [CALL; 2].join(" ");
        for (a, b) in [(std, wide), (wide, std)] {
            let keys = keyed(
                &[(a, CALL), (a, CALL), (b, CALL), (b, CALL), (b, CALL)],
                2.0,
            );
            let mut d = Decoder::new(cfg(20.0)).unwrap();
            let got = decode(&mut d, &keys, 0.0, 1, Some(0.01));
            assert!(got.ends_with(&format!(" {two}")), "{got}");
        }
    }

    /// Hand keying: every mark and space off by up to ±25 %, with pauses between the transmissions, over 20 seeds. The decoder
    /// starts at 20 WPM; the first transmission is where it finds the speed, so the text is checked from the second on.
    #[test]
    fn uneven_keying_keeps_word_spaces() {
        let tail = [CALL; 2].join(" ");
        let mut bad = Vec::new();
        for wpm in [12.0, 20.0, 40.0] {
            let k = Keyer::new(wpm).unwrap();
            for pause in [2.0, 30.0] {
                let keys = keyed(&[(k, CALL), (k, CALL), (k, CALL)], pause);
                for seed in 1..=20 {
                    let mut d = Decoder::new(cfg(20.0)).unwrap();
                    let got = decode(&mut d, &keys, 0.25, seed, Some(0.01));
                    if !got.ends_with(&format!(" {tail}")) {
                        bad.push(format!("{wpm} WPM, pause {pause} s, seed {seed}: {got:?}"));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Farnsworth spacing: characters at 20 WPM, spaces stretched to 10 WPM, with pauses, ±10 % on every interval, 20 seeds,
    /// fed live and as whole intervals: the text is exact from the start.
    #[test]
    fn farnsworth_with_pauses() {
        let k = Keyer::farnsworth(20.0, 10.0).unwrap();
        let want = [CALL; 3].join(" ");
        let mut bad = Vec::new();
        for pause in [5.0, 30.0] {
            let keys = keyed(&[(k, CALL), (k, CALL), (k, CALL)], pause);
            for seed in 1..=20 {
                for idle_step in [None, Some(0.01)] {
                    let mut d = Decoder::new(cfg(20.0)).unwrap();
                    let got = decode(&mut d, &keys, 0.1, seed, idle_step);
                    if got != want || !d.farnsworth() || (d.wpm() - 20.0).abs() > 1.0 {
                        bad.push(format!(
                            "pause {pause} s, seed {seed}, idle {idle_step:?}: {got:?}, {:.1} WPM",
                            d.wpm()
                        ));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// A speed change from 15 to 25 WPM in mid text, with standard word spaces: the text at 15 WPM is exact, and after one
    /// transmission at 25 WPM the text is exact again. ±10 % and ±25 % on every interval, 20 seeds each. The speed is within
    /// 10 %: three times the spread of a dot estimate that learns at 0.1 from lengths with ±25 % uniform error.
    #[test]
    fn speed_change_keeps_word_spaces() {
        let (slow, fast) = (Keyer::new(15.0).unwrap(), Keyer::new(25.0).unwrap());
        let keys = keyed(
            &[
                (slow, CALL),
                (slow, CALL),
                (fast, CALL),
                (fast, CALL),
                (fast, CALL),
            ],
            7.0 * slow.dot(),
        );
        let two = [CALL; 2].join(" ");
        let mut bad = Vec::new();
        for jitter in [0.0, 0.1, 0.25] {
            for seed in 1..=20 {
                let mut d = Decoder::new(cfg(15.0)).unwrap();
                let got = decode(&mut d, &keys, jitter, seed, Some(0.01));
                if !got.starts_with(&format!("{two} "))
                    || !got.ends_with(&format!(" {two}"))
                    || (d.wpm() - 25.0).abs() > 2.5
                {
                    bad.push(format!(
                        "jitter {jitter}, seed {seed}: {got:?}, {:.1} WPM",
                        d.wpm()
                    ));
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// ITU-R M.1677-1 Part I section 2: words of dots only or dashes only, sent once, at speeds away from the start speed.
    #[test]
    fn one_kind_words_read_right() {
        for w in ["EEE", "SSS", "HHH", "IS", "OMO", "MOM", "TOM", "OO", "SOS"] {
            for wpm in [6.0, 12.0, 25.0] {
                let mut d = Decoder::new(cfg(20.0)).unwrap();
                let got = text_of(&mut d, Keyer::new(wpm).unwrap().key(w), 0.0, 1);
                assert_eq!(got, w, "{w} at {wpm}");
                assert!(
                    (d.wpm() - wpm).abs() < 0.03 * wpm,
                    "{w} at {wpm}: {}",
                    d.wpm()
                );
            }
        }
        // Dashes with character spaces only fit two speeds exactly (TTT, or S three times slower): the start speed decides.
        let mut d = Decoder::new(cfg(15.0)).unwrap();
        assert_eq!(
            text_of(&mut d, Keyer::new(12.0).unwrap().key("TTT"), 0.0, 1),
            "TTT"
        );
        let mut d = Decoder::new(cfg(6.0)).unwrap();
        assert_eq!(
            text_of(&mut d, Keyer::new(12.0).unwrap().key("TTT"), 0.0, 1),
            "S"
        );
    }

    #[test]
    fn idents_at_any_speed() {
        for (w, wpm) in [
            ("SYD", 7.0),
            ("TOM", 10.0),
            ("EEE", 12.0),
            ("OMO", 6.0),
            ("IGN", 15.0),
        ] {
            let mut d = Decoder::new(DecoderConfig::idents()).unwrap();
            let k = Keyer::new(wpm).unwrap();
            let keys = k
                .key(w)
                .chain([Key {
                    on: false,
                    seconds: 3.0,
                }])
                .chain(k.key(w))
                .chain([Key {
                    on: false,
                    seconds: 3.0,
                }]);
            assert_eq!(text_of(&mut d, keys, 0.05, 7), format!("{w} {w}"), "{w}");
            assert!((d.wpm() - wpm).abs() < 0.1 * wpm, "{w}: {}", d.wpm());
        }
        // A word outside the speed range is dropped.
        let mut d = Decoder::new(DecoderConfig::idents()).unwrap();
        assert_eq!(
            text_of(&mut d, Keyer::new(60.0).unwrap().key("ABC"), 0.0, 1),
            ""
        );
        assert_eq!(d.dropped_words(), 1);
    }

    #[test]
    fn both_classifiers_read_machine_keying() {
        for classifier in [Classifier::Adaptive, Classifier::Clusters] {
            for wpm in [5.0, 12.0, 20.0, 35.0, 50.0] {
                let mut d = Decoder::new(DecoderConfig {
                    classifier,
                    ..cfg(20.0)
                })
                .unwrap();
                let text = "CQ CQ DE K0TST K0TST PSE K TEST 599";
                let got = text_of(&mut d, Keyer::new(wpm).unwrap().key(text), 0.0, 1);
                assert_eq!(got, text, "{classifier:?} {wpm}");
                assert!((d.wpm() - wpm).abs() < 0.05 * wpm, "{} vs {wpm}", d.wpm());
                assert!(d.is_locked() && !d.farnsworth());
            }
        }
    }

    #[test]
    fn follows_speed_changes_and_farnsworth() {
        let text = "CQ CQ DE K0TST K0TST PSE K";
        // Starting at 20 wpm, the signal is at 30 wpm with 15 % jitter.
        let mut d = Decoder::new(cfg(20.0)).unwrap();
        assert_eq!(
            text_of(&mut d, Keyer::new(30.0).unwrap().key(text), 0.15, 3),
            text
        );
        assert!((d.wpm() - 30.0).abs() < 3.0, "{}", d.wpm());
        // Then slows to 12 wpm.
        let out = text_of(&mut d, Keyer::new(12.0).unwrap().key(text), 0.1, 4);
        assert!(out.ends_with("K0TST PSE K"), "{out}");
        // Farnsworth 20/8.
        let mut d = Decoder::new(cfg(20.0)).unwrap();
        let k = Keyer::farnsworth(20.0, 8.0).unwrap();
        assert_eq!(text_of(&mut d, k.key(text), 0.05, 5), text);
        assert!(d.farnsworth());
        // Prosigns, a fixed speed, and bad input.
        let mut d = Decoder::new(DecoderConfig::fixed(20.0)).unwrap();
        assert_eq!(
            text_of(&mut d, Keyer::new(20.0).unwrap().key("TU <SK>"), 0.0, 1),
            "TU <SK>"
        );
        d.push(true, f64::NAN, &mut |_| panic!("no event"));
        assert!(Decoder::new(cfg(500.0)).is_err());
        assert!(Decoder::new(DecoderConfig {
            min_wpm: 30.0,
            ..cfg(20.0)
        })
        .is_err());
    }

    #[test]
    fn confidence_and_alternative() {
        let mut d = Decoder::new(DecoderConfig::fixed(20.0)).unwrap();
        let dot = 0.06;
        let mut v = Vec::new();
        // A: a dot, then a mark half way between a dot and a dash.
        d.push(true, dot, &mut |e| v.push(e));
        d.push(false, dot, &mut |e| v.push(e));
        d.push(true, dot * 3f64.sqrt() * 1.02, &mut |e| v.push(e));
        d.flush(&mut |e| v.push(e));
        let Event::Char(s) = v[0] else {
            panic!("{v:?}")
        };
        assert_eq!((s.text, s.alt), (Some("A"), Some("I")));
        assert!(s.confidence <= 5, "{}", s.confidence);
        // A code the table does not have.
        let mut v = Vec::new();
        for _ in 0..10 {
            d.push(true, dot, &mut |e| v.push(e));
            d.push(false, dot, &mut |e| v.push(e));
        }
        d.flush(&mut |e| v.push(e));
        assert!(matches!(v[0], Event::Char(Symbol { text: None, .. })));
        assert_eq!(d.unknown(), 1);
    }

    #[test]
    fn fit_dot_known_answers() {
        // PARIS at 12 wpm: dot 0.1 s; word space 0.7 s included.
        let mut k: Vec<(bool, f64)> = Keyer::new(12.0)
            .unwrap()
            .key("PARIS PARIS")
            .map(|k| (k.on, k.seconds))
            .collect();
        assert!((fit_dot(&k, 1.0).unwrap() - 0.1).abs() < 1e-12);
        k.truncate(1);
        assert_eq!(fit_dot(&k, 0.1), None);
    }
}
