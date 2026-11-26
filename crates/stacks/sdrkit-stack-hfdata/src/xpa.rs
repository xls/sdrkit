//! XPA and XPA2: MFSK number-group senders on HF. XPA (submode `xpa`): 20 tone positions 40 Hz apart from 520 Hz above an
//! upper-sideband dial, 17 of them used, at 10 or 20 Bd. XPA2 (submode `xpa2`): 16 tones 15.625 Hz apart from 1000 Hz, 14 used,
//! at 7.8125 Bd. The stack finds the tones anywhere in the channel, finds the speed, the sideband and the numbering of the tones
//! by reading the symbols in parallel, and shows the start tones, the message start, the five-figure groups ten to a line, the
//! block sync, the break and the end of a message.
//!
//! Standard: no issuing body publishes these signals. The tone maps and the broadcast sequences are those of the public
//! protocol pages of priyom.org (XPA, XPA2) and the Signal Identification Wiki (CIS MFSK-20 XPA, CIS MFSK-16 XPA2).
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [xpa: low pass to 8 kHz, tone-set search and replay, one matched-filter bank, symbol clock and
//!        quality gate per speed, 14 readings per speed (two sidebands, seven numberings), the winner] -> rows, text
//! ```

use crate::receiver::GEN_SECTION;
use crate::senders::{Cx, Layer, Lcg, Playback, RowLog, Sender, StatusLine, TextLog, Tone};
use crate::typed::{
    SenderRow, TextEvent, TonesFoundEvent, TonesLostEvent, XpaFields, SENDER_ROWS, TEXT,
    TONES_FOUND, TONES_LOST,
};
use sdrkit_algo_modem::fsk::{MfskConfig, MfskDemod};
use sdrkit_algo_modem::toneset::{
    Chooser, CombSpec, QualityGate, SearchConfig, SearchEvent, ToneSetSearch,
};
use sdrkit_core::interface::{Fields as _, Hertz, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, SubMode, Value, Values,
};
use sdrkit_graph::{values_of, Kept};
use std::collections::VecDeque;
use std::sync::Arc;

/// The channel rate.
pub const RATE: f64 = 8000.0;
/// The matched filters beyond each end of the tones.
const GUARD_FILTERS: i64 = 3;
/// The numbering corrections tried.
const CORRECTIONS: i64 = 3;
/// The block sync string.
pub const BLOCK_SYNC: &str = "6666622662626";
/// The break string.
pub const BREAK: &str = "4444444444";
/// The groups that separate two messages of a broadcast.
pub const SEPARATOR: &str = "00000 00000";
/// The fixed string of the XPA header: block sync, ten 4s, block sync.
pub const HEADER_STRING: &str = "666662266262644444444446666622662626";

sdrkit_core::choice_enum! {
    /// The signal.
    pub enum Submode {
        /// XPA: 20 positions 40 Hz apart, 10 or 20 Bd.
        Xpa = ("xpa", "XPA"),
        /// XPA2: 16 tones 15.625 Hz apart, 7.8125 Bd.
        Xpa2 = ("xpa2", "XPA2"),
    }
}

impl Submode {
    /// The tone positions.
    pub fn tones(self) -> i64 {
        match self {
            Submode::Xpa => 20,
            Submode::Xpa2 => 16,
        }
    }

    /// The frequency of tone `k`, Hz above the dial.
    pub fn tone_hz(self, k: i64) -> f64 {
        match self {
            Submode::Xpa => 520.0 + 40.0 * k as f64,
            Submode::Xpa2 => 1000.0 + 15.625 * k as f64,
        }
    }

    /// The tone spacing, Hz.
    pub fn spacing(self) -> f64 {
        match self {
            Submode::Xpa => 40.0,
            Submode::Xpa2 => 15.625,
        }
    }

    /// The higher start tone.
    pub fn start_high(self) -> i64 {
        match self {
            Submode::Xpa => 19,
            Submode::Xpa2 => 15,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Submode::Xpa => "XPA",
            Submode::Xpa2 => "XPA2",
        }
    }
}

sdrkit_core::choice_enum! {
    /// The speed of XPA.
    pub enum Speed {
        /// Both speeds are read; the one that fits wins.
        Auto = ("auto", "Find it"),
        /// 10 Bd.
        B10 = ("10", "10 Bd"),
        /// 20 Bd.
        B20 = ("20", "20 Bd"),
    }
}

sdrkit_core::choice_enum! {
    /// The sideband.
    pub enum Sideband {
        /// Both are read; the one that fits wins.
        Auto = ("auto", "Find it"),
        /// Upper sideband.
        Upper = ("upper", "Upper"),
        /// Lower sideband: the tones are reversed.
        Lower = ("lower", "Lower"),
    }
}

sdrkit_core::choice_enum! {
    /// The correction of the tone numbering.
    pub enum Numbering {
        /// Seven corrections are read; the one that fits wins.
        Auto = ("auto", "Find it"),
        /// Three tones down.
        Minus3 = ("minus3", "-3 tones"),
        /// Two tones down.
        Minus2 = ("minus2", "-2 tones"),
        /// One tone down.
        Minus1 = ("minus1", "-1 tone"),
        /// As found.
        Zero = ("none", "As found"),
        /// One tone up.
        Plus1 = ("plus1", "+1 tone"),
        /// Two tones up.
        Plus2 = ("plus2", "+2 tones"),
        /// Three tones up.
        Plus3 = ("plus3", "+3 tones"),
    }
}

sdrkit_core::choice_enum! {
    /// The speed of the XPA test signal.
    pub enum GenBaud {
        /// 10 Bd.
        B10 = ("10", "10 Bd"),
        /// 20 Bd.
        B20 = ("20", "20 Bd"),
    }
}

/// The submode.
pub const SUBMODE: Setting<field::Choice<Submode>> =
    Setting::choice("submode", "Signal", Submode::Xpa);
/// The speed.
pub const SPEED: Setting<field::Choice<Speed>> = Setting::choice("speed", "Speed", Speed::Auto)
    .help("XPA only: Find it reads 10 and 20 Bd side by side; a value reads one.");
/// The sideband.
pub const SIDEBAND: Setting<field::Choice<Sideband>> =
    Setting::choice("sideband", "Sideband", Sideband::Auto)
        .help("Find it reads both sidebands side by side; a value halves the readings.");
/// The numbering correction.
pub const NUMBERING: Setting<field::Choice<Numbering>> =
    Setting::choice("numbering", "Tone numbering correction", Numbering::Auto)
        .help("Find it reads the corrections -3 to +3 tones side by side; a value reads one.");
/// The search width.
pub const SEARCH_HZ: Setting<field::Hz> = Setting::new("search_hz", "Search width", "Hz", 4500.0)
    .range(500.0, 4500.0, 10.0)
    .help(
        "The tones are searched up to this far from their nominal place (and within the channel).",
    );
/// The groups of the test signal.
pub const GEN_GROUPS: Setting<field::I64> = Setting::new("gen_groups", "Groups", "", 0)
    .range(0.0, 1000.0, 1.0)
    .help("Random five-figure groups from the seed; 0 takes 30 for XPA and 24 for XPA2.")
    .section(GEN_SECTION)
    .advanced();
/// The speed of the XPA test signal.
pub const GEN_BAUD: Setting<field::Choice<GenBaud>> =
    Setting::choice("gen_baud", "XPA speed", GenBaud::B10)
        .section(GEN_SECTION)
        .advanced();
/// The test signal in the lower sideband.
pub const GEN_LOWER: Setting<field::Bool> = Setting::new("gen_lower", "Lower sideband", "", false)
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the XPA receiver.
    pub enum XpaRxSetting {
        /// Signal.
        Submode = SUBMODE,
        /// Speed.
        Speed = SPEED,
        /// Sideband.
        Sideband = SIDEBAND,
        /// Numbering.
        Numbering = NUMBERING,
        /// Search width.
        SearchHz = SEARCH_HZ,
    }
}

sdrkit_core::settings! {
    /// The settings of the XPA stack.
    pub enum XpaSetting {
        /// Signal.
        Submode = SUBMODE,
        /// Speed.
        Speed = SPEED,
        /// Sideband.
        Sideband = SIDEBAND,
        /// Numbering.
        Numbering = NUMBERING,
        /// Search width.
        SearchHz = SEARCH_HZ,
        /// Groups of the test signal.
        GenGroups = GEN_GROUPS,
        /// Speed of the test signal.
        GenBaud = GEN_BAUD,
        /// Lower sideband test signal.
        GenLower = GEN_LOWER,
    }
}

/// What no public document confirms.
pub const NOTE_TEXT: &str = "No issuing body publishes XPA or XPA2; the tone maps and broadcast sequences follow the protocol \
pages of priyom.org. Not confirmed: the end of an XPA message (end tone, then repeat), ten groups to a line, the rule that a \
character equal to the one before is sent as the repeat tone (the header string has runs of equal tones), the block sync \
inside XPA2 traffic, the break string outside the XPA header, the order of the alternating XPA2 sync tones, whether senders \
keep the phase continuous, the content of the XPA schedule ID (not sent by the test signal), and the null message \
'0____ 00001 00000 _____' read with any digit at each underscore.";

/// A token of the reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    /// Nothing before.
    Empty,
    /// The low start tone.
    StartLow,
    /// The high start tone.
    StartHigh,
    /// The low sync tone (XPA header marker).
    SyncLow,
    /// The high sync tone.
    SyncHigh,
    /// A space.
    Space,
    /// The end tone.
    End,
    /// The message start.
    MsgStart,
    /// The repeat tone.
    Repeat,
    /// A digit.
    Digit(u8),
    /// A tone without a meaning.
    Unknown,
}

/// The token of tone `k` after the token `prev`.
pub fn token(sub: Submode, k: i64, prev: Token) -> Token {
    match sub {
        Submode::Xpa => match k {
            0 => Token::StartLow,
            2 => Token::SyncLow,
            4 => Token::Space,
            5 => Token::End,
            6..=14 => Token::Digit((k - 6) as u8),
            15 if prev == Token::SyncLow => Token::SyncHigh,
            15 => Token::Digit(9),
            16 => Token::MsgStart,
            17 => Token::Repeat,
            19 => Token::StartHigh,
            _ => Token::Unknown,
        },
        Submode::Xpa2 => match k {
            0 => Token::Space,
            2 => Token::MsgStart,
            3 => Token::End,
            5 => Token::Repeat,
            6..=15 => Token::Digit((k - 6) as u8),
            _ => Token::Unknown,
        },
    }
}

/// A row of a reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// The start tones.
    Start,
    /// The message start.
    MessageStart,
    /// Up to ten groups.
    Groups(String),
    /// The block sync string.
    BlockSync,
    /// The break string.
    Break,
    /// The end of a message.
    End,
    /// The groups that separate two XPA messages.
    Separator,
    /// An XPA null message (`0____ 00001 00000 _____`, each `_` any digit): its groups.
    NullMessage(String),
}

impl Line {
    /// The kind of the row.
    pub fn kind(&self) -> &'static str {
        match self {
            Line::Start => "Start",
            Line::MessageStart => "Message start",
            Line::Groups(_) => "Groups",
            Line::BlockSync => "Block sync",
            Line::Break => "Break",
            Line::End => "End",
            Line::Separator => "Message separator",
            Line::NullMessage(_) => "Null message",
        }
    }

    /// The text of the row.
    pub fn text(&self) -> &str {
        match self {
            Line::Start => "start tones",
            Line::MessageStart => "message start",
            Line::Groups(g) => g,
            Line::BlockSync => BLOCK_SYNC,
            Line::Break => BREAK,
            Line::End => "message complete",
            Line::Separator => SEPARATOR,
            Line::NullMessage(g) => g,
        }
    }
}

/// One reading of a symbol stream: the tones to text of the XPA tone maps.
#[derive(Clone, Debug)]
pub struct Reading {
    sub: Submode,
    prev: Token,
    line: String,
    groups: u32,
    /// Well-formed groups read.
    pub score: u32,
    in_message: bool,
    in_start: bool,
    last: VecDeque<i64>,
    /// The first five words of the open message.
    message: Vec<String>,
    /// Words in the open message.
    words: usize,
    /// The word before was `00000`.
    zero_before: bool,
}

impl Reading {
    /// A reading of `sub`.
    pub fn new(sub: Submode) -> Self {
        Reading {
            sub,
            prev: Token::Empty,
            line: String::new(),
            groups: 0,
            score: 0,
            in_message: false,
            in_start: false,
            last: VecDeque::new(),
            message: Vec::new(),
            words: 0,
            zero_before: false,
        }
    }

    /// Ends the open message: when it is a null message and the line holds it whole, the line becomes its row.
    fn close_message(&mut self, out: &mut Vec<Line>) {
        let m = std::mem::take(&mut self.message);
        let words = std::mem::take(&mut self.words);
        self.zero_before = false;
        let null = self.sub == Submode::Xpa
            && words == 4
            && m.iter()
                .all(|g| g.len() == 5 && g.bytes().all(|b| b.is_ascii_digit()))
            && m[0].starts_with('0')
            && m[1] == "00001"
            && m[2] == "00000"
            && self.line.trim_end() == m.join(" ");
        if null {
            self.line.clear();
            out.push(Line::NullMessage(m.join(" ")));
        }
    }

    /// Drops the words of the open message (the line was not traffic).
    fn forget_message(&mut self) {
        self.message.clear();
        self.words = 0;
        self.zero_before = false;
    }

    /// Publishes the open line.
    pub fn flush(&mut self, out: &mut Vec<Line>) {
        let t = self.line.trim_end().to_string();
        self.line.clear();
        if !t.is_empty() {
            out.push(Line::Groups(t));
        }
    }

    /// Takes tone number `k`.
    pub fn push(&mut self, k: i64, out: &mut Vec<Line>) {
        self.last.push_back(k);
        if self.last.len() > 4 {
            self.last.pop_front();
        }
        let high = self.sub.start_high();
        if self.in_start && (k == 0 || k == high) {
            return;
        }
        self.in_start = false;
        self.read(k, out);
        if self.last.len() == 4 && self.last.iter().eq([0, 0, high, high].iter()) {
            self.line.clear();
            self.forget_message();
            self.groups = 0;
            self.in_start = true;
            if !self.in_message {
                out.push(Line::Start);
                self.in_message = true;
            }
        }
        if self.prev == Token::SyncHigh {
            self.line.clear();
            self.forget_message();
            self.groups = 0;
        }
    }

    /// Two groups `00000` in a row end a message: they are taken out of the line, the message before is closed, the line is
    /// flushed and a separator row follows. False when the first of them was flushed with an earlier line.
    fn separator(&mut self, out: &mut Vec<Line>) -> bool {
        let with_space = format!("{SEPARATOR} ");
        if !self.line.ends_with(&with_space) {
            return false;
        }
        self.line.truncate(self.line.len() - with_space.len());
        self.words -= 2;
        self.message.truncate(self.words.min(5));
        self.close_message(out);
        self.flush(out);
        out.push(Line::Separator);
        self.groups = 0;
        true
    }

    fn read(&mut self, k: i64, out: &mut Vec<Line>) {
        let mut t = token(self.sub, k, self.prev);
        if t == Token::Repeat && self.prev == Token::End {
            self.close_message(out);
            self.flush(out);
            if self.in_message {
                out.push(Line::End);
            }
            self.prev = Token::Empty;
            self.in_message = false;
            return;
        }
        if t == Token::Repeat {
            t = self.prev;
        }
        if (t == Token::MsgStart && self.prev == Token::MsgStart)
            || (t == Token::Space && self.prev == Token::Space)
        {
            return;
        }
        if t == Token::Space && self.line.is_empty() {
            self.prev = t;
            return;
        }
        let before = self.prev;
        self.prev = t;
        match t {
            Token::MsgStart => {
                self.close_message(out);
                self.flush(out);
                out.push(Line::MessageStart);
                self.groups = 0;
                self.in_message = true;
            }
            Token::End => {
                self.close_message(out);
                self.flush(out);
                self.groups = 0;
            }
            Token::SyncHigh
            | Token::SyncLow
            | Token::StartHigh
            | Token::StartLow
            | Token::Empty => {}
            Token::Digit(_) | Token::Unknown | Token::Space => {
                match t {
                    Token::Digit(d) => self.line.push((b'0' + d) as char),
                    Token::Unknown => self.line.push('?'),
                    _ => self.line.push(' '),
                }
                if self.line.len() > 160 {
                    self.flush(out);
                    return;
                }
                if t == Token::Space {
                    let word = self.line.trim_end().rsplit(' ').next().unwrap_or("");
                    let group = word.len() == 5 && word.bytes().all(|b| b.is_ascii_digit());
                    let zero = word == "00000";
                    if self.message.len() < 5 {
                        self.message.push(word.to_string());
                    }
                    self.words += 1;
                    let sep = zero && self.zero_before;
                    self.zero_before = zero;
                    if group {
                        self.score += 1;
                    }
                    if sep && self.sub == Submode::Xpa && self.separator(out) {
                        return;
                    }
                    let _ = before;
                    self.groups += 1;
                    if self.groups >= 10 {
                        self.flush(out);
                        self.groups = 0;
                    }
                } else if self.line.ends_with(BLOCK_SYNC) {
                    self.line.truncate(self.line.len() - BLOCK_SYNC.len());
                    self.flush(out);
                    out.push(Line::BlockSync);
                    self.groups = 0;
                    self.prev = Token::Empty;
                } else if self.line.ends_with(BREAK) {
                    self.line.truncate(self.line.len() - BREAK.len());
                    self.flush(out);
                    out.push(Line::Break);
                    self.groups = 0;
                    self.prev = Token::Empty;
                }
            }
            Token::Repeat => {}
        }
    }
}

/// The readings of one speed: its filter bank, clock and gate.
struct Chain {
    baud: f64,
    demod: MfskDemod,
    gate: QualityGate,
    strobes: u64,
    readings: Vec<(bool, i64, Reading)>,
    energies: Vec<f32>,
}

/// A row held with its reading.
#[derive(Clone, Debug)]
struct Row {
    time: f64,
    id: &'static str,
    line: Line,
    snr: Option<f64>,
}

/// The XPA layer.
pub struct Xpa {
    sub: Submode,
    speed: Speed,
    sideband: Sideband,
    numbering: Numbering,
    search_hz: f64,
    search: ToneSetSearch,
    chains: Vec<Chain>,
    chooser: Chooser<Row>,
    winner: Option<(f64, bool, i64)>,
    mixed: Vec<Complex32>,
    lines: Vec<Line>,
    publish: Vec<Row>,
    rows: RowLog<Row>,
    text: TextLog,
    groups: i64,
    good: i64,
    bad: i64,
    snr: Option<f64>,
    fits: u64,
    elsewhere: u32,
}

impl Default for Xpa {
    fn default() -> Self {
        Self::new()
    }
}

fn search_config(sub: Submode, search_hz: f64) -> SearchConfig {
    let tones: Vec<f64> = (0..sub.tones()).map(|k| sub.tone_hz(k)).collect();
    let half = match sub {
        Submode::Xpa => 10.0,
        Submode::Xpa2 => 6.25,
    };
    let comb = CombSpec::new(&tones, half, -search_hz, search_hz)
        .with_guards(sub.spacing(), 2)
        .periodic(sub.spacing());
    SearchConfig::new(comb, Some(sub.spacing()), 0.3 * sub.spacing())
}

impl Xpa {
    /// A layer at the default settings.
    pub fn new() -> Self {
        let mut x = Xpa {
            sub: Submode::Xpa,
            speed: Speed::Auto,
            sideband: Sideband::Auto,
            numbering: Numbering::Auto,
            search_hz: 4500.0,
            search: ToneSetSearch::new(RATE, search_config(Submode::Xpa, 4500.0))
                .expect("valid search"),
            chains: Vec::new(),
            chooser: Chooser::new(0),
            winner: None,
            mixed: Vec::new(),
            lines: Vec::new(),
            publish: Vec::new(),
            rows: RowLog::default(),
            text: TextLog::default(),
            groups: 0,
            good: 0,
            bad: 0,
            snr: None,
            fits: 0,
            elsewhere: 0,
        };
        x.rebuild();
        x
    }

    fn rebuild(&mut self) {
        self.search = ToneSetSearch::new(RATE, search_config(self.sub, self.search_hz))
            .expect("valid search");
        self.restart();
    }

    /// The speeds read.
    fn bauds(&self) -> Vec<f64> {
        match (self.sub, self.speed) {
            (Submode::Xpa2, _) => vec![7.8125],
            (Submode::Xpa, Speed::Auto) => vec![10.0, 20.0],
            (Submode::Xpa, Speed::B10) => vec![10.0],
            (Submode::Xpa, Speed::B20) => vec![20.0],
        }
    }

    fn restart(&mut self) {
        let sides: &[bool] = match self.sideband {
            Sideband::Auto => &[false, true],
            Sideband::Upper => &[false],
            Sideband::Lower => &[true],
        };
        let corr: Vec<i64> = match self.numbering {
            Numbering::Auto => (-CORRECTIONS..=CORRECTIONS).collect(),
            n => vec![n.index() as i64 - 4],
        };
        let sub = self.sub;
        let tones: Vec<f64> = (-GUARD_FILTERS..sub.tones() + GUARD_FILTERS)
            .map(|k| sub.tone_hz(k))
            .collect();
        self.chains = self
            .bauds()
            .into_iter()
            .map(|baud| {
                let mut cfg = MfskConfig::from_hz(&tones, baud, RATE);
                cfg.average_symbols = 12.0;
                Chain {
                    baud,
                    demod: MfskDemod::new(cfg).expect("valid demodulator"),
                    gate: QualityGate::new(),
                    strobes: 0,
                    readings: sides
                        .iter()
                        .flat_map(|&l| corr.iter().map(move |&c| (l, c, Reading::new(sub))))
                        .collect(),
                    energies: Vec::new(),
                }
            })
            .collect();
        let n = self.chains.iter().map(|c| c.readings.len()).sum();
        self.chooser = Chooser::new(n);
        self.winner = None;
    }

    fn id_of(&self, baud: f64) -> &'static str {
        match (self.sub, baud as i64) {
            (Submode::Xpa2, _) => "XPA2 7.8125 Bd",
            (_, 20) => "XPA 20 Bd",
            _ => "XPA 10 Bd",
        }
    }

    /// Offers the lines of reading `r` of chain `c` to the chooser.
    fn offer(&mut self, index: usize, baud: f64, time: f64) {
        let id = self.id_of(baud);
        for line in std::mem::take(&mut self.lines) {
            let row = Row {
                time,
                id,
                line,
                snr: self.snr,
            };
            self.chooser.offer(index, row, &mut self.publish);
        }
    }

    fn symbols(&mut self, ci: usize, time: f64) {
        let m = (self.sub.tones() + 2 * GUARD_FILTERS) as usize;
        let k = self.sub.tones();
        let base: usize = self.chains[..ci].iter().map(|c| c.readings.len()).sum();
        let e = std::mem::take(&mut self.chains[ci].energies);
        for sym in e.chunks_exact(m) {
            let ch = &mut self.chains[ci];
            ch.strobes += 1;
            let open = ch.gate.push(sym);
            let closed = ch.gate.take_closed();
            let baud = ch.baud;
            if closed {
                for r in 0..self.chains[ci].readings.len() {
                    self.chains[ci].readings[r].2.flush(&mut self.lines);
                    self.offer(base + r, baud, time);
                }
            }
            if !open || self.chains[ci].strobes < 3 {
                continue;
            }
            let b = sym
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map_or(0, |(i, _)| i) as i64;
            for r in 0..self.chains[ci].readings.len() {
                let (lower, c, ref mut reading) = self.chains[ci].readings[r];
                let n = if lower {
                    (k - 1) - (b - GUARD_FILTERS)
                } else {
                    b - GUARD_FILTERS
                };
                reading.push(n - c, &mut self.lines);
                self.offer(base + r, baud, time);
            }
            self.choose();
        }
        self.chains[ci].energies = e;
        self.chains[ci].energies.clear();
    }

    /// Chooses a winner: the most well-formed groups, at least 3 and at least twice the next plus 1.
    fn choose(&mut self) {
        if self.chooser.winner().is_some() {
            return;
        }
        let mut all = Vec::new();
        for ch in &self.chains {
            for (l, c, r) in &ch.readings {
                all.push((r.score, ch.baud, *l, *c));
            }
        }
        let Some((bi, best)) = all.iter().enumerate().max_by_key(|(_, a)| a.0) else {
            return;
        };
        let second = all
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != bi)
            .map(|(_, a)| a.0)
            .max()
            .unwrap_or(0);
        if best.0 >= 3 && best.0 > 2 * second {
            self.winner = Some((best.1, best.2, best.3));
            self.chooser.choose(bi, &mut self.publish);
        }
    }

    fn take(&mut self, cx: &mut Cx) {
        for row in std::mem::take(&mut self.publish) {
            if let Line::Groups(g) = &row.line {
                self.good += 1;
                self.groups += g.split(' ').count() as i64;
                self.bad += g.matches('?').count() as i64;
                self.text.line(g);
                cx.events
                    .push(TEXT.event(cx.time, cx.dial, &TextEvent { text: g }));
            }
            self.rows.push(row);
        }
    }

    /// Before a reading has won, two fits in a row that name an offset at least half a spacing from the lock lock again
    /// there (the start tones alone leave the whole-tone place of the comb open).
    fn relock(&mut self) -> Option<SearchEvent> {
        if self.search.fits() == self.fits {
            return None;
        }
        self.fits = self.search.fits();
        let (Some(df), Some(f)) = (self.search.offset_hz(), self.search.last_fit()) else {
            self.elsewhere = 0;
            return None;
        };
        if self.chooser.winner().is_some()
            || !f.found
            || (f.df_hz - df).abs() <= self.sub.spacing() / 2.0
        {
            self.elsewhere = 0;
            return None;
        }
        self.elsewhere += 1;
        if self.elsewhere < 2 {
            return None;
        }
        self.elsewhere = 0;
        self.search.relock()
    }

    fn lost(&mut self, time: f64) {
        for ci in 0..self.chains.len() {
            let base: usize = self.chains[..ci].iter().map(|c| c.readings.len()).sum();
            let baud = self.chains[ci].baud;
            for r in 0..self.chains[ci].readings.len() {
                self.chains[ci].readings[r].2.flush(&mut self.lines);
                self.offer(base + r, baud, time);
            }
        }
    }
}

impl Layer for Xpa {
    const NAME: &'static str = "hfdata.xpa";
    const DECIM: usize = 3;
    const PASS_HZ: f64 = 3400.0;
    fn schema() -> Arc<Schema> {
        XpaRxSetting::schema()
    }
    fn declaration() -> ResultsDecl {
        ResultsDecl::new()
            .with_fields(XpaFields::SHAPE.fields)
            .with_table(&SENDER_ROWS)
            .event(&TEXT)
            .event(&TONES_FOUND)
            .event(&TONES_LOST)
    }
    fn values(&self) -> Values {
        values_of([
            (XpaRxSetting::Submode, Value::Choice(self.sub.index())),
            (XpaRxSetting::Speed, Value::Choice(self.speed.index())),
            (XpaRxSetting::Sideband, Value::Choice(self.sideband.index())),
            (
                XpaRxSetting::Numbering,
                Value::Choice(self.numbering.index()),
            ),
            (XpaRxSetting::SearchHz, Value::Hz(self.search_hz)),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match XpaRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            XpaRxSetting::Submode => self.sub = SUBMODE.read(id, value)?,
            XpaRxSetting::Speed => self.speed = SPEED.read(id, value)?,
            XpaRxSetting::Sideband => self.sideband = SIDEBAND.read(id, value)?,
            XpaRxSetting::Numbering => self.numbering = NUMBERING.read(id, value)?,
            XpaRxSetting::SearchHz => self.search_hz = SEARCH_HZ.read(id, value)?,
        }
        self.rebuild();
        Ok(())
    }
    fn push(&mut self, x: &[Complex32], cx: &mut Cx) {
        for &v in x {
            let mut event = self.search.push(v);
            if event.is_none() {
                event = self.relock();
            }
            match event {
                Some(SearchEvent::Found { df_hz, snr_db }) => {
                    self.restart();
                    self.snr = Some(snr_db);
                    cx.events.push(TONES_FOUND.event(
                        cx.time,
                        cx.dial,
                        &TonesFoundEvent {
                            text: &format!(
                                "{} tones found, offset {df_hz:+.1} Hz",
                                self.sub.label()
                            ),
                            offset: Hertz(df_hz),
                            snr: snr_db,
                        },
                    ));
                }
                Some(SearchEvent::Lost { df_hz }) => {
                    self.lost(cx.time);
                    self.take(cx);
                    cx.events.push(TONES_LOST.event(
                        cx.time,
                        cx.dial,
                        &TonesLostEvent {
                            text: &format!("{} tones lost", self.sub.label()),
                            offset: Hertz(df_hz),
                            read: self.groups,
                        },
                    ));
                    self.restart();
                    self.snr = None;
                }
                None => {}
            }
            if let Some(f) = self.search.last_fit().filter(|_| self.search.locked()) {
                self.snr = Some(f.snr_db);
            }
            loop {
                self.mixed.clear();
                if self.search.pull(80, &mut self.mixed) == 0 {
                    break;
                }
                let mixed = std::mem::take(&mut self.mixed);
                for ci in 0..self.chains.len() {
                    let mut e = std::mem::take(&mut self.chains[ci].energies);
                    self.chains[ci].demod.process(&mixed, &mut e);
                    self.chains[ci].energies = e;
                    self.symbols(ci, cx.time);
                }
                self.mixed = mixed;
                self.take(cx);
            }
        }
    }
    fn finish(&mut self, cx: &mut Cx) {
        self.lost(cx.time);
        self.take(cx);
    }
    fn reset(&mut self) {
        self.search.reset();
        self.restart();
        self.snr = None;
    }
    fn clear(&mut self) {
        self.rows.clear();
        self.text.clear();
        self.groups = 0;
        self.good = 0;
        self.bad = 0;
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        if !kept.table(SENDER_ROWS.key) {
            self.rows.clear();
        }
        if !kept.field("text") {
            self.text.clear();
        }
        self.groups = 0;
        self.good = 0;
        self.bad = 0;
    }
    fn report(&mut self, res: &mut Published, level_dbfs: f64) -> StatusLine {
        self.rows.report(
            res,
            |r| r.clear_rows(&SENDER_ROWS),
            |r, key, row| {
                SENDER_ROWS.put(
                    r,
                    key,
                    &SenderRow {
                        time: Time(row.time),
                        kind: row.line.kind(),
                        id: row.id,
                        text: row.line.text(),
                        snr: row.snr,
                    },
                );
            },
        );
        let df = self.search.offset_hz();
        let snr = df.and(self.snr);
        let w = self.winner.filter(|_| df.is_some());
        let state: u8 = match (df, w) {
            (Some(_), Some(_)) => 2,
            (Some(_), None) => 1,
            _ => 0,
        };
        let numbering = w.map(|(_, _, c)| {
            if c == 0 {
                "as found".to_string()
            } else {
                format!("moved by {c:+} tones")
            }
        });
        res.put(&XpaFields {
            note: NOTE_TEXT,
            text: self.text.as_str(),
            state: i64::from(state),
            offset_hz: df.map(Hertz),
            snr_db: snr.map(|v| (v * 10.0).round() / 10.0),
            speed: w.map(|(b, _, _)| b),
            sideband: w.map(|(_, l, _)| if l { "lower (tones reversed)" } else { "upper" }),
            numbering: numbering.as_deref(),
            groups: self.groups,
            good: self.good,
            bad: self.bad,
            level_dbfs,
        });
        let name = self.sub.label();
        let summary = match (df, w) {
            (None, _) => format!("searching for {name} tones"),
            (Some(d), None) => format!("{name} tones found (offset {d:+.0} Hz), reading"),
            (Some(_), Some((b, _, _))) => format!("{}: {} groups", self.id_of(b), self.groups),
        };
        StatusLine {
            state,
            summary,
            snr_db: snr,
            offset_hz: df,
        }
    }
}

/// The submode of a configuration: the submode named, else the `submode` setting.
pub fn submode_of(sub: Option<&str>, settings: &Values) -> Submode {
    sub.and_then(Submode::from_key)
        .unwrap_or_else(|| settings.get(&SUBMODE))
}

/// The schema of the stack.
pub fn schema() -> Arc<Schema> {
    XpaSetting::schema()
}

/// `n` random five-figure groups from `seed`.
pub fn test_groups(n: usize, seed: u64) -> Vec<String> {
    let mut r = Lcg(seed ^ 0x7A);
    (0..n)
        .map(|_| (0..5).map(|_| (b'0' + r.below(10) as u8) as char).collect())
        .collect()
}

/// The tones of `digits` with the repeat rule: a character equal to the one before (when that one was not itself sent as the
/// repeat tone) is sent as `repeat`.
fn with_repeats(chars: &[i64], repeat: i64, out: &mut Vec<i64>) {
    let mut prev: Option<i64> = None;
    let mut was_repeat = false;
    for &c in chars {
        if Some(c) == prev && !was_repeat {
            out.push(repeat);
            was_repeat = true;
        } else {
            out.push(c);
            was_repeat = false;
        }
        prev = Some(c);
    }
}

/// The tone of digit `d`.
fn digit_tone(d: u8) -> i64 {
    6 + i64::from(d)
}

/// The traffic tones of `groups`: each group and a space with the repeat rule, the block sync after every 64 groups.
fn traffic(sub: Submode, groups: &[String], out: &mut Vec<i64>) {
    let (space, repeat) = match sub {
        Submode::Xpa => (4, 17),
        Submode::Xpa2 => (0, 5),
    };
    let mut chars = Vec::new();
    for (i, g) in groups.iter().enumerate() {
        chars.extend(g.bytes().map(|b| digit_tone(b - b'0')));
        chars.push(space);
        if (i + 1) % 64 == 0 && i + 1 < groups.len() {
            with_repeats(&chars, repeat, out);
            chars.clear();
            out.extend(BLOCK_SYNC.bytes().map(|b| digit_tone(b - b'0')));
        }
    }
    with_repeats(&chars, repeat, out);
}

/// The tones of the test signal at `baud` (timed pieces: seconds and tone). XPA: three cycles of 400 ms of tone 0 and 1600 ms of
/// tone 19; the header marker and digit 9 alternating (20 pairs), the header string, 20 pairs again; repeat and message start
/// alternating (20 tones), ten spaces; the traffic; end tone and repeat. XPA2: three cycles of 400 ms of the digit 9 tone and
/// 1600 ms of the space tone; the block sync string; message start and repeat alternating (20 tones); ten spaces; the traffic;
/// end and repeat alternating (20 tones).
pub fn test_pieces(sub: Submode, groups: &[String], baud: f64) -> Vec<(f64, i64)> {
    let sym = 1.0 / baud;
    let mut p = Vec::new();
    let mut tones = Vec::new();
    let (a, b) = match sub {
        Submode::Xpa => (0, 19),
        Submode::Xpa2 => (15, 0),
    };
    for _ in 0..3 {
        p.push((0.4, a));
        p.push((1.6, b));
    }
    match sub {
        Submode::Xpa => {
            for _ in 0..20 {
                tones.extend([2, 15]);
            }
            tones.extend(HEADER_STRING.bytes().map(|c| digit_tone(c - b'0')));
            for _ in 0..20 {
                tones.extend([2, 15]);
            }
            for _ in 0..10 {
                tones.extend([17, 16]);
            }
            tones.extend([4; 10]);
            traffic(sub, groups, &mut tones);
            tones.extend([5, 17]);
        }
        Submode::Xpa2 => {
            tones.extend(BLOCK_SYNC.bytes().map(|c| digit_tone(c - b'0')));
            for _ in 0..10 {
                tones.extend([2, 5]);
            }
            tones.extend([0; 10]);
            traffic(sub, groups, &mut tones);
            for _ in 0..10 {
                tones.extend([3, 5]);
            }
        }
    }
    p.extend(tones.into_iter().map(|t| (sym, t)));
    p
}

/// The tone sequence of the test signal: 1 s of silence, [`test_pieces`] (mirrored in the lower sideband), 1.5 s of silence.
pub fn test_sequence(sub: Submode, groups: &[String], baud: f64, lower: bool) -> Vec<Tone> {
    let mut seq = vec![Tone::silence(1.0)];
    for (secs, k) in test_pieces(sub, groups, baud) {
        let f = sub.tone_hz(k);
        seq.push(Tone::new(secs, if lower { -f } else { f }));
    }
    seq.push(Tone::silence(1.5));
    seq
}

/// The test signal of the submode.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let sub = submode_of(cfg.submode.as_deref(), s);
    let n = match s.get(&GEN_GROUPS) {
        n if n > 0 => n.min(1000) as usize,
        _ if sub == Submode::Xpa2 => 24,
        _ => 30,
    };
    let baud = match (sub, s.get(&GEN_BAUD)) {
        (Submode::Xpa2, _) => 7.8125,
        (_, GenBaud::B10) => 10.0,
        (_, GenBaud::B20) => 20.0,
    };
    let seq = test_sequence(sub, &test_groups(n, cfg.seed), baud, s.get(&GEN_LOWER));
    Ok(Box::new(Playback::new(
        &seq,
        cfg.rate_hz,
        SignalInfo {
            symbol_rate_hz: Some(baud),
            ..SignalInfo::new(3000.0)
        },
    )))
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
        crate::typed::shape("xpa"),
        "XPA",
        Sender::new(Xpa::new()),
        &with_submode(cfg)?,
    )
}

/// The dial of the test signal. It is not a station frequency: the host gives the frequency.
pub const TEST_SIGNAL_HZ: f64 = 6.5e6;

/// XPA and XPA2.
pub static XPA: StackDescriptor = StackDescriptor {
    id: "xpa",
    name: "XPA / XPA2 number senders",
    group: "HF utility",
    blurb: "XPA (20 positions, 10 or 20 Bd) and XPA2 (16 tones, 7.8125 Bd) MFSK number senders: the tones found anywhere in the channel, the speed, sideband and numbering found, the five-figure groups.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        ..crate::ms110::hf_tuning(TEST_SIGNAL_HZ)
    },
    submodes: &[
        SubMode {
            id: "xpa",
            name: "XPA",
            default_hz: TEST_SIGNAL_HZ,
            ..SubMode::DEFAULT
        },
        SubMode {
            id: "xpa2",
            name: "XPA2",
            default_hz: TEST_SIGNAL_HZ,
            ..SubMode::DEFAULT
        },
    ],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::XPA),
    benchmarks: Some(crate::benchmarks),
};
