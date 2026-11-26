//! CROWD36 (also called CIS-36): MFSK of one tone at a time out of 34 tones 40 Hz apart from 1410 Hz above an upper-sideband
//! dial, at 40 Bd (10 and 20 Bd are also described). Two sync tones alternate; then characters follow in a letters case and a
//! figures case with Cyrillic letters. The traffic is believed to be enciphered; nothing is deciphered.
//!
//! Standard: no issuing body publishes the signal. Public descriptions (the Wavecom decoder help on CIS-36, the Shoc signal
//! database, the hoka.it page on CROWD36) give 36 numbered positions with 32 active tones in groups of 10, 11 and 11, 40 Hz
//! apart within a group and 80 Hz between groups, and the speeds. The 34 positions read here are numbers 2 to 35 of them;
//! positions 10 and 22 of this grid are the gaps between the groups.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [crowd36: low pass to 8 kHz, tone-set search and replay, 42 matched filters and the symbol
//!        clock, the quality gate, the sync and the character tables] -> rows, text
//! ```

use crate::receiver::GEN_SECTION;
use crate::senders::{Cx, Layer, Lcg, Playback, RowLog, Sender, StatusLine, TextLog, Tone};
use crate::typed::{
    Crowd36Fields, SenderRow, TextEvent, TonesFoundEvent, TonesLostEvent, SENDER_ROWS, TEXT,
    TONES_FOUND, TONES_LOST,
};
use sdrkit_algo_modem::fsk::{MfskConfig, MfskDemod};
use sdrkit_algo_modem::toneset::{
    CombSpec, QualityGate, SearchConfig, SearchEvent, ToneOffset, ToneSetSearch,
};
use sdrkit_core::interface::{Fields as _, Hertz, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, Value, Values,
};
use sdrkit_graph::{values_of, Kept};
use std::collections::VecDeque;
use std::sync::Arc;

/// The channel rate.
pub const RATE: f64 = 8000.0;
/// The tones.
pub const TONES: usize = 34;
/// The lowest tone, Hz.
pub const FIRST_HZ: f64 = 1410.0;
/// The spacing, Hz.
pub const SPACING_HZ: f64 = 40.0;
/// The matched filters beyond each end.
const GUARD_FILTERS: usize = 4;
/// The row id.
const ID: &str = "CROWD36";

/// The frequency of tone `k`, Hz above the dial.
pub fn tone_hz(k: i64) -> f64 {
    FIRST_HZ + SPACING_HZ * k as f64
}

/// What a tone means in one case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sym {
    /// A character.
    Char(&'static str),
    /// The figure shift.
    Figs,
    /// The letter shift.
    Ltrs,
    /// A new line.
    Cr,
    /// Nothing.
    Nothing,
    /// A tone of unknown meaning: shown with its number.
    Unknown,
}

use Sym::{Char as C, Cr, Figs, Ltrs, Nothing, Unknown as U};

/// The letters case, by tone.
pub const LETTERS: [Sym; TONES] = [
    C("Q"),
    C("X"),
    C("W"),
    C("V"),
    C("E"),
    C("K"),
    C(" "),
    C("B"),
    C("R"),
    C("J"),
    U,
    C("G"),
    C("T"),
    C("F"),
    Figs,
    C("M"),
    C("Y"),
    C("C"),
    Cr,
    C("Z"),
    C("U"),
    C("L"),
    U,
    C("D"),
    C("I"),
    C("H"),
    Ltrs,
    C("S"),
    C("O"),
    C("N"),
    U,
    C("A"),
    C("P"),
    U,
];

/// The figures case, by tone.
pub const FIGURES: [Sym; TONES] = [
    C("1"),
    C("Щ"),
    C("2"),
    C("="),
    C("3"),
    C("("),
    C(" "),
    C("?"),
    C("4"),
    C("Ю"),
    U,
    C("8"),
    C("5"),
    C("Э"),
    Figs,
    C("."),
    C("6"),
    C(":"),
    Cr,
    C("+"),
    C("7"),
    C(")"),
    U,
    C(" "),
    C("8"),
    C("щ"),
    Ltrs,
    Nothing,
    C("9"),
    C(","),
    U,
    C("-"),
    C("0"),
    U,
];

/// The tone of a letter A to Z (or space) in the letters case.
pub fn letter_tone(c: char) -> Option<i64> {
    LETTERS
        .iter()
        .position(|t| matches!(t, C(s) if s.chars().eq(std::iter::once(c))))
        .map(|k| k as i64)
}

sdrkit_core::choice_enum! {
    /// The sideband the tones are read in.
    pub enum Sideband {
        /// Upper sideband.
        Upper = ("upper", "Upper"),
        /// Lower sideband: the tone numbering is mirrored.
        Lower = ("lower", "Lower"),
    }
}

sdrkit_core::choice_enum! {
    /// The speed.
    pub enum Speed {
        /// 40 Bd.
        B40 = ("40", "40 Bd"),
        /// 20 Bd.
        B20 = ("20", "20 Bd"),
        /// 10 Bd.
        B10 = ("10", "10 Bd"),
    }
}

impl Speed {
    /// The speed, Bd.
    pub fn baud(self) -> f64 {
        match self {
            Speed::B40 => 40.0,
            Speed::B20 => 20.0,
            Speed::B10 => 10.0,
        }
    }
}

/// The tone the higher sync tone is taken as.
pub const SYNC_HIGH_TONE: Setting<field::I64> =
    Setting::new("sync_high_tone", "Sync high tone", "tone", 24)
        .range(0.0, 33.0, 1.0)
        .help("The tone number the higher sync tone is taken as; it sets the numbering of every tone. It must match the station.");
/// The sideband.
pub const SIDEBAND: Setting<field::Choice<Sideband>> =
    Setting::choice("sideband", "Sideband", Sideband::Upper)
        .help("lower mirrors the numbering; the sync cannot tell the sidebands apart.");
/// The speed.
pub const SPEED: Setting<field::Choice<Speed>> = Setting::choice("speed", "Speed", Speed::B40)
    .help("The stack and the test signal use this speed.");
/// The search width.
pub const SEARCH_HZ: Setting<field::Hz> = Setting::new("search_hz", "Search width", "Hz", 4500.0)
    .range(500.0, 4500.0, 10.0)
    .help(
        "The tones are searched up to this far from their nominal place (and within the channel).",
    );
/// The groups of the test signal.
pub const GEN_GROUPS: Setting<field::I64> = Setting::new("gen_groups", "Groups", "", 40)
    .range(1.0, 1000.0, 1.0)
    .help("Random five-letter groups from the seed.")
    .section(GEN_SECTION)
    .advanced();
/// The higher sync tone of the test signal.
pub const GEN_SYNC_HIGH: Setting<field::I64> =
    Setting::new("gen_sync_high", "Sync high tone", "tone", 24)
        .range(0.0, 33.0, 1.0)
        .section(GEN_SECTION)
        .advanced();
/// The lower sync tone of the test signal.
pub const GEN_SYNC_LOW: Setting<field::I64> =
    Setting::new("gen_sync_low", "Sync low tone", "tone", 10)
        .range(0.0, 33.0, 1.0)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the CROWD36 receiver.
    pub enum Crowd36RxSetting {
        /// Sync high tone.
        SyncHighTone = SYNC_HIGH_TONE,
        /// Sideband.
        Sideband = SIDEBAND,
        /// Speed.
        Speed = SPEED,
        /// Search width.
        SearchHz = SEARCH_HZ,
    }
}

sdrkit_core::settings! {
    /// The settings of the CROWD36 stack.
    pub enum Crowd36Setting {
        /// Sync high tone.
        SyncHighTone = SYNC_HIGH_TONE,
        /// Sideband.
        Sideband = SIDEBAND,
        /// Speed.
        Speed = SPEED,
        /// Search width.
        SearchHz = SEARCH_HZ,
        /// Groups of the test signal.
        GenGroups = GEN_GROUPS,
        /// Sync high tone of the test signal.
        GenSyncHigh = GEN_SYNC_HIGH,
        /// Sync low tone of the test signal.
        GenSyncLow = GEN_SYNC_LOW,
    }
}

/// What no public document confirms.
pub const NOTE_TEXT: &str = "No issuing body publishes CROWD36. Public descriptions give 36 positions (32 active tones in \
groups of 10, 11 and 11, 80 Hz between the groups) and 10, 20 or 40 Bd. Not confirmed: the lowest tone 1410 Hz, the upper \
sideband, the sync high tone 24 and the sync low tone 10 of the test signal, both character tables and their oddities (8 at \
two tones, a space at tone 23, nothing at tone 27, a lower-case Cyrillic letter at tone 25), traffic in groups of five characters (a line is shown only when it holds one; \
the others are counted as refused). The traffic is not deciphered.";

/// A row as kept.
#[derive(Clone, Debug)]
struct Row {
    time: f64,
    kind: &'static str,
    text: String,
    snr: Option<f64>,
}

/// The character layer: the sync rule and the two cases over a stream of tone numbers.
#[derive(Clone, Debug)]
pub struct Reader {
    sync_high: i64,
    hist: VecDeque<i64>,
    /// The state of the line before each of the last five symbols: its length, the word and the groups.
    lens: VecDeque<(usize, Word, usize)>,
    /// The word being read.
    word: Word,
    /// Groups of five table characters in the line.
    groups: usize,
    stretch: u32,
    synced: bool,
    c: i64,
    figs: bool,
    line: String,
    /// The sync tones of the last sync, after the correction.
    pub sync: Option<(i64, i64)>,
    /// Characters read.
    pub chars: i64,
    /// Tones outside the tables.
    pub bad: i64,
    /// Lines refused by the check of their form.
    pub refused: i64,
}

/// The word being read: table characters since the last space, or a word that holds a tone of unknown meaning or outside
/// the tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Word {
    Chars(usize),
    Spoilt,
}

/// What the character layer gives.
#[derive(Clone, Debug, PartialEq)]
pub enum Read {
    /// A sync: the high and the low tone after the correction, and the correction.
    Sync(i64, i64, i64),
    /// A line of text.
    Text(String),
}

impl Reader {
    /// A reader taking the higher sync tone as `sync_high`.
    pub fn new(sync_high: i64) -> Self {
        Reader {
            sync_high,
            hist: VecDeque::new(),
            lens: VecDeque::new(),
            word: Word::Chars(0),
            groups: 0,
            stretch: 0,
            synced: false,
            c: 0,
            figs: false,
            line: String::new(),
            sync: None,
            chars: 0,
            bad: 0,
            refused: 0,
        }
    }

    /// The correction of the numbering.
    pub fn correction(&self) -> i64 {
        self.c
    }

    /// Takes the correction into the tone numbers: the numbers that follow are already corrected.
    pub fn recentre(&mut self) {
        for h in &mut self.hist {
            *h -= self.c;
        }
        self.c = 0;
    }

    /// Whether a sync was seen.
    pub fn synced(&self) -> bool {
        self.synced
    }

    /// Publishes the open line when it holds a group: five characters of the tables between spaces or the ends of the
    /// line (the traffic is sent in groups of five). A line without one is refused and counted.
    pub fn flush(&mut self, out: &mut Vec<Read>) {
        self.end_word();
        let t = self.line.trim().to_string();
        self.line.clear();
        let groups = std::mem::take(&mut self.groups);
        if groups > 0 {
            out.push(Read::Text(t));
        } else if !t.is_empty() {
            self.refused += 1;
        }
    }

    fn end_word(&mut self) {
        if self.word == Word::Chars(5) {
            self.groups += 1;
        }
        self.word = Word::Chars(0);
    }

    /// Takes tone number `n`.
    pub fn push(&mut self, n: i64, out: &mut Vec<Read>) {
        self.hist.push_back(n);
        if self.hist.len() > 6 {
            self.hist.pop_front();
        }
        self.lens
            .push_back((self.line.len(), self.word, self.groups));
        if self.lens.len() > 5 {
            self.lens.pop_front();
        }
        let h: Vec<i64> = self.hist.iter().copied().collect();
        let rule = h.len() == 6
            && h[0] == h[2]
            && h[2] == h[4]
            && h[1] == h[3]
            && h[3] == h[5]
            && (h[0] - h[1]).abs() >= 3;
        if rule {
            if self.stretch == 0 {
                let (keep, word, groups) =
                    self.lens.front().copied().unwrap_or((0, Word::Chars(0), 0));
                if keep <= self.line.len() {
                    self.line.truncate(keep);
                    (self.word, self.groups) = (word, groups);
                }
                self.flush(out);
                let (hi, lo) = (h[0].max(h[1]), h[0].min(h[1]));
                self.c = hi - self.sync_high;
                self.synced = true;
                self.figs = false;
                self.sync = Some((hi - self.c, lo - self.c));
                out.push(Read::Sync(hi - self.c, lo - self.c, self.c));
            }
            self.stretch = 6;
            return;
        }
        self.stretch = self.stretch.saturating_sub(1);
        if !self.synced {
            return;
        }
        let k = n - self.c;
        if !(0..TONES as i64).contains(&k) {
            self.append("?");
            self.word = Word::Spoilt;
            self.bad += 1;
            return;
        }
        let table = if self.figs { &FIGURES } else { &LETTERS };
        match table[k as usize] {
            Ltrs => self.figs = false,
            Figs => self.figs = true,
            Cr => {
                self.figs = false;
                self.flush(out);
            }
            Nothing => {}
            U => {
                self.append(&format!("<*{k}>"));
                self.word = Word::Spoilt;
            }
            C(" ") => {
                self.append(" ");
                self.end_word();
            }
            C(s) => {
                self.append(s);
                if let Word::Chars(n) = self.word {
                    self.word = Word::Chars(n + 1);
                }
            }
        }
        if self.line.chars().count() >= 80 {
            self.flush(out);
        }
    }

    fn append(&mut self, s: &str) {
        self.line.push_str(s);
        self.chars += 1;
    }
}

/// The CROWD36 layer.
pub struct Crowd36 {
    sync_high: i64,
    lower: bool,
    speed: Speed,
    search_hz: f64,
    search: ToneSetSearch,
    demod: MfskDemod,
    gate: QualityGate,
    follow: ToneOffset,
    reader: Reader,
    strobes: u64,
    mixed: Vec<Complex32>,
    energies: Vec<f32>,
    reads: Vec<Read>,
    rows: RowLog<Row>,
    text: TextLog,
    good: i64,
    read_in_lock: i64,
    snr: Option<f64>,
}

impl Default for Crowd36 {
    fn default() -> Self {
        Self::new()
    }
}

impl Crowd36 {
    /// A layer at the default settings.
    pub fn new() -> Self {
        let mut s = Crowd36 {
            sync_high: 24,
            lower: false,
            speed: Speed::B40,
            search_hz: 4500.0,
            search: ToneSetSearch::new(RATE, search_config(4500.0)).expect("valid search"),
            demod: demod(Speed::B40),
            gate: QualityGate::new(),
            follow: ToneOffset::new(SPACING_HZ, 16),
            reader: Reader::new(24),
            strobes: 0,
            mixed: Vec::new(),
            energies: Vec::new(),
            reads: Vec::new(),
            rows: RowLog::default(),
            text: TextLog::default(),
            good: 0,
            read_in_lock: 0,
            snr: None,
        };
        s.rebuild();
        s
    }

    fn rebuild(&mut self) {
        self.search =
            ToneSetSearch::new(RATE, search_config(self.search_hz)).expect("valid search");
        self.restart();
    }

    fn restart(&mut self) {
        self.demod = demod(self.speed);
        self.gate.reset();
        self.follow.reset();
        self.reader = Reader {
            refused: self.reader.refused,
            ..Reader::new(self.sync_high)
        };
        self.strobes = 0;
    }

    fn take(&mut self, cx: &mut Cx) {
        for r in self.reads.drain(..) {
            let (kind, text) = match r {
                Read::Sync(h, l, _) => {
                    // The mixer has moved by the correction already.
                    let off = self.search.offset_hz().unwrap_or(0.0);
                    ("Sync", format!("sync tones {h} / {l}, offset {off:+.0} Hz"))
                }
                Read::Text(t) => {
                    self.good += 1;
                    self.text.line(&t);
                    cx.events
                        .push(TEXT.event(cx.time, cx.dial, &TextEvent { text: &t }));
                    ("Text", t)
                }
            };
            self.rows.push(Row {
                time: cx.time,
                kind,
                text,
                snr: self.snr,
            });
        }
    }

    fn symbols(&mut self, cx: &mut Cx) {
        let m = TONES + 2 * GUARD_FILTERS;
        let e = std::mem::take(&mut self.energies);
        for sym in e.chunks_exact(m) {
            self.strobes += 1;
            let open = self.gate.push(sym);
            if self.gate.take_closed() {
                self.reader.flush(&mut self.reads);
            }
            if !open || self.strobes < 3 {
                continue;
            }
            // The comb fit leaves an error within the spacing; the filters beside the strongest follow it.
            if let Some(d) = self.follow.push(sym) {
                self.search.adjust(0.5 * d);
            }
            let idx = sym
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map_or(0, |(i, _)| i) as i64;
            let n = if self.lower {
                TONES as i64 - 1 - (idx - GUARD_FILTERS as i64)
            } else {
                idx - GUARD_FILTERS as i64
            };
            let before = self.reader.chars;
            self.reader.push(n, &mut self.reads);
            // A sync names the numbering: the mixer moves by the whole tones, so every tone sits on its own filter.
            let c = self.reader.correction();
            if c != 0 {
                self.search.adjust(c as f64 * SPACING_HZ);
                self.reader.recentre();
            }
            self.read_in_lock += self.reader.chars - before;
        }
        self.energies = e;
        self.energies.clear();
        self.take(cx);
    }
}

fn search_config(search_hz: f64) -> SearchConfig {
    let tones: Vec<f64> = (0..TONES as i64).map(tone_hz).collect();
    let comb = CombSpec::new(&tones, 16.0, -search_hz, search_hz)
        .with_guards(SPACING_HZ, 2)
        .periodic(SPACING_HZ);
    // The spectrum of 40 Bd tones 40 Hz apart is flat over the comb: the offset is followed from the symbols, not the fits.
    SearchConfig::new(comb, Some(SPACING_HZ), 0.0)
}

fn demod(speed: Speed) -> MfskDemod {
    let g = GUARD_FILTERS as i64;
    let tones: Vec<f64> = (-g..TONES as i64 + g).map(tone_hz).collect();
    let mut cfg = MfskConfig::from_hz(&tones, speed.baud(), RATE);
    cfg.average_symbols = 12.0;
    MfskDemod::new(cfg).expect("valid demodulator")
}

impl Layer for Crowd36 {
    const NAME: &'static str = "hfdata.crowd36";
    const DECIM: usize = 3;
    const PASS_HZ: f64 = 3400.0;
    fn schema() -> Arc<Schema> {
        Crowd36RxSetting::schema()
    }
    fn declaration() -> ResultsDecl {
        ResultsDecl::new()
            .with_fields(Crowd36Fields::SHAPE.fields)
            .with_table(&SENDER_ROWS)
            .event(&TEXT)
            .event(&TONES_FOUND)
            .event(&TONES_LOST)
    }
    fn values(&self) -> Values {
        values_of([
            (Crowd36RxSetting::SyncHighTone, Value::I64(self.sync_high)),
            (
                Crowd36RxSetting::Sideband,
                Value::Choice(if self.lower { 1 } else { 0 }),
            ),
            (Crowd36RxSetting::Speed, Value::Choice(self.speed.index())),
            (Crowd36RxSetting::SearchHz, Value::Hz(self.search_hz)),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match Crowd36RxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Crowd36RxSetting::SyncHighTone => self.sync_high = SYNC_HIGH_TONE.read(id, value)?,
            Crowd36RxSetting::Sideband => {
                self.lower = SIDEBAND.read(id, value)? == Sideband::Lower;
            }
            Crowd36RxSetting::Speed => self.speed = SPEED.read(id, value)?,
            Crowd36RxSetting::SearchHz => self.search_hz = SEARCH_HZ.read(id, value)?,
        }
        self.rebuild();
        Ok(())
    }
    fn push(&mut self, x: &[Complex32], cx: &mut Cx) {
        for &v in x {
            match self.search.push(v) {
                Some(SearchEvent::Found { df_hz, snr_db }) => {
                    self.restart();
                    self.read_in_lock = 0;
                    self.snr = Some(snr_db);
                    cx.events.push(TONES_FOUND.event(
                        cx.time,
                        cx.dial,
                        &TonesFoundEvent {
                            text: &format!("CROWD36 tones found, offset {df_hz:+.1} Hz"),
                            offset: Hertz(df_hz),
                            snr: snr_db,
                        },
                    ));
                }
                Some(SearchEvent::Lost { df_hz }) => {
                    self.reader.flush(&mut self.reads);
                    self.take(cx);
                    cx.events.push(TONES_LOST.event(
                        cx.time,
                        cx.dial,
                        &TonesLostEvent {
                            text: "CROWD36 tones lost",
                            offset: Hertz(df_hz),
                            read: self.read_in_lock,
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
            // Pieces of 10 ms, so a correction of the mixer applies to the samples that follow it.
            loop {
                self.mixed.clear();
                if self.search.pull(80, &mut self.mixed) == 0 {
                    break;
                }
                let mixed = std::mem::take(&mut self.mixed);
                self.demod.process(&mixed, &mut self.energies);
                self.mixed = mixed;
                self.symbols(cx);
            }
        }
    }
    fn finish(&mut self, cx: &mut Cx) {
        self.reader.flush(&mut self.reads);
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
        self.good = 0;
        self.reader.chars = 0;
        self.reader.bad = 0;
        self.reader.refused = 0;
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        if !kept.table(SENDER_ROWS.key) {
            self.rows.clear();
        }
        if !kept.field("text") {
            self.text.clear();
        }
        self.good = 0;
        self.reader.refused = 0;
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
                        kind: row.kind,
                        id: ID,
                        text: &row.text,
                        snr: row.snr,
                    },
                );
            },
        );
        let df = self.search.offset_hz();
        let locked = df.is_some();
        let sync = self
            .reader
            .sync
            .map(|(h, l)| format!("{h} and {l} (high tone taken as tone {})", self.sync_high));
        let state: u8 = match (locked, self.reader.synced()) {
            (true, true) => 2,
            (true, false) => 1,
            _ => 0,
        };
        let snr = if locked { self.snr } else { None };
        res.put(&Crowd36Fields {
            note: NOTE_TEXT,
            text: self.text.as_str(),
            state: i64::from(state),
            offset_hz: df.map(Hertz),
            snr_db: snr.map(|v| (v * 10.0).round() / 10.0),
            sync_tones: sync.as_deref(),
            characters: self.reader.chars,
            good: self.good,
            bad: self.reader.bad,
            level_dbfs,
            refused: self.reader.refused,
        });
        let summary = match state {
            0 => "searching for CROWD36 tones".to_string(),
            1 => format!(
                "CROWD36 tones found (offset {:+.0} Hz), waiting for the sync tones",
                df.unwrap_or(0.0)
            ),
            _ => format!("CROWD36: {} characters", self.reader.chars),
        };
        StatusLine {
            state,
            summary,
            snr_db: snr,
            offset_hz: df,
        }
    }
}

/// The schema of the stack.
pub fn schema() -> Arc<Schema> {
    Crowd36Setting::schema()
}

/// The groups of the test signal: `n` random five-letter groups from `seed`.
pub fn test_groups(n: usize, seed: u64) -> Vec<String> {
    let mut r = Lcg(seed ^ 0x36);
    (0..n)
        .map(|_| (0..5).map(|_| (b'A' + r.below(26) as u8) as char).collect())
        .collect()
}

/// The tone numbers of the test signal: 20 pairs of the sync tones, the letter shift, the groups
/// each followed by a space and every tenth by a new line, and a final new line.
pub fn test_tones(groups: &[String], high: i64, low: i64) -> Vec<i64> {
    let mut t = Vec::new();
    for _ in 0..20 {
        t.push(high);
        t.push(low);
    }
    t.push(26);
    for (i, g) in groups.iter().enumerate() {
        for c in g.chars() {
            t.push(letter_tone(c).unwrap_or(6));
        }
        t.push(if (i + 1) % 10 == 0 { 18 } else { 6 });
    }
    t.push(18);
    t
}

/// The test signal: 1 s of silence, the tones of [`test_tones`], 1.5 s of silence.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let n = s.get(&GEN_GROUPS).clamp(1, 1000) as usize;
    let baud = s.get(&SPEED).baud();
    let groups = test_groups(n, cfg.seed);
    let mut seq = vec![Tone::silence(1.0)];
    seq.extend(
        test_tones(&groups, s.get(&GEN_SYNC_HIGH), s.get(&GEN_SYNC_LOW))
            .into_iter()
            .map(|k| Tone::new(1.0 / baud, tone_hz(k))),
    );
    seq.push(Tone::silence(1.5));
    Ok(Box::new(Playback::new(
        &seq,
        cfg.rate_hz,
        SignalInfo {
            symbol_rate_hz: Some(baud),
            ..SignalInfo::new(3000.0)
        },
    )))
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("crowd36"),
        "CROWD36",
        Sender::new(Crowd36::new()),
        cfg,
    )
}

/// The dial of the test signal. It is not a station frequency: the host gives the frequency.
pub const TEST_SIGNAL_HZ: f64 = 6.5e6;

/// CROWD36.
pub static CROWD36: StackDescriptor = StackDescriptor {
    id: "crowd36",
    name: "CROWD36",
    group: "HF utility",
    blurb: "CROWD36 34-tone MFSK at 40 Bd: the tones found anywhere in the channel, the sync tones, the characters of the letters and figures cases. Not deciphered.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        ..crate::ms110::hf_tuning(TEST_SIGNAL_HZ)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::CROWD36),
    benchmarks: Some(crate::benchmarks),
};
