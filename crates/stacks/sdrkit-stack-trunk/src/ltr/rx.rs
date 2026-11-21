//! The components of the LTR receiver: `ltr.lowpass`, `ltr.sync` (slicing and sync search), `ltr.check` and `ltr.tables`.

use super::code::{self, LtrFields, WordKind, SYNC};
use crate::front::{Eye, Level};
use crate::grant::Grant;
use crate::typed::*;
use crate::{set_status, typed_format, typed_port};
use sdrkit_algo_dsp::iir::Butterworth4;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_core::interface::{Fields, Hertz, Time};
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, ConfigError, Event, LockState, Record, Results,
    Values,
};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
use sdrkit_util_objects::commands::CLEAR;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{log_rows, rows, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, ObjectTable, Seen, Stamp};
use std::num::NonZeroU32;

/// The bit rate.
pub const BAUD: f64 = 300.0;
/// The slicer's rate: 8 samples per bit.
pub const RATE: f64 = 2400.0;
/// The working rate of the front end, Hz.
pub const WORK_RATE: f64 = 9600.0;
/// The channel half-width of the front end, Hz.
pub const HALF_WIDTH_HZ: f64 = 3500.0;
/// The carrier follow-up searches this far from the channel centre, Hz.
pub const SEARCH_HZ: f64 = 4000.0;
/// The carrier follow-up corrects at most this much, Hz.
pub const MAX_CORRECTION_HZ: f64 = 9000.0;
/// The front end low-passes the lag product at this cut-off before the angle, Hz. It passes the data and the lower voice
/// band, so the product keeps its length.
pub const PRODUCT_LOWPASS_HZ: f64 = 1200.0;
const SPB: u64 = 8;
/// A bit is weak when its mean is nearer to the threshold than this share of half the range.
const WEAK_FRACTION: f32 = 0.15;
/// The most weak bits of a word. The check finds every word with one or two wrong bits, and wrong bits are weak bits.
const MAX_WEAK_BITS: usize = 2;

/// `ltr.lowpass`: a 150 Hz fourth-order Butterworth low-pass at 9600 Hz, decimated by 4 to 2400 Hz.
pub struct LtrLowpass {
    lp: Butterworth4,
    k: u64,
}

impl Default for LtrLowpass {
    fn default() -> Self {
        LtrLowpass {
            lp: Butterworth4::lowpass(WORK_RATE, 150.0),
            k: 0,
        }
    }
}

impl Component for LtrLowpass {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ltr.lowpass",
            vec![Port::real("in")],
            vec![Port::real("out").rate_from(RateFrom::Divided {
                input: 0,
                by: NonZeroU32::new(4).expect("4 is not 0"),
            })],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = LtrLowpass {
            lp: Butterworth4::lowpass(inputs[0].rate, 150.0),
            k: 0,
        };
        Ok(vec![inputs[0].with_rate(inputs[0].rate / 4.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let mut k = self.k;
        let lp = &mut self.lp;
        let out = io.real_out(0);
        for &v in x {
            let y = lp.process_sample(v);
            if k % 4 == 3 {
                out.push(y);
            }
            k += 1;
        }
        self.k = k;
    }
}

/// A sync hit chosen from the phases, before the checks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LtrCandidate {
    /// Signal time of the word's end, s.
    pub time: f64,
    /// The 40 bits (complemented when the sync was inverted).
    pub word: u64,
    /// Found with the inverted sync.
    pub inverted: bool,
    /// The data swing (envelope range), Hz.
    pub swing: f32,
    /// At most two bits have a mean nearer to the threshold than 15 % of half the range.
    pub eye_ok: bool,
}

impl Item for LtrCandidate {
    const KIND: &'static str = "ltr.candidate";
}

/// `ltr.sync`: bit-long integrate-and-dump at every phase against the envelope's middle, 40-bit registers per phase, the
/// 9-bit sync in both polarities, hits on neighbouring phases merged.
pub struct LtrSync {
    n: u64,
    eye: Eye,
    raw: SampleRing<f32>,
    diff: SampleRing<f32>,
    sum: f32,
    regs: [u64; SPB as usize],
    suppress_until: u64,
    hits: Vec<(u64, bool, u64, f32)>,
}

impl Default for LtrSync {
    fn default() -> Self {
        LtrSync {
            n: 0,
            eye: Eye::default(),
            raw: SampleRing::new(16).expect("a ring of 16 samples"),
            diff: SampleRing::new(512).expect("a ring of 512 samples"),
            sum: 0.0,
            regs: [0; SPB as usize],
            suppress_until: 0,
            hits: Vec::with_capacity(8),
        }
    }
}

impl LtrSync {
    /// Takes samples at 2400 Hz, appends candidates.
    pub fn run(&mut self, x: &[f32], out: &mut Vec<Timed<LtrCandidate>>) {
        const MASK: u64 = (1 << 40) - 1;
        for &v in x {
            let n = self.n;
            self.eye.update(v);
            let old = if n >= SPB {
                self.raw.get(n - SPB).unwrap_or(0.0)
            } else {
                0.0
            };
            self.raw.push(v);
            self.sum += v - old;
            if n % 4096 == 4095 {
                // Re-sum now and then so rounding never accumulates.
                self.sum = (0..SPB.min(n + 1))
                    .map(|k| self.raw.get(n - k).unwrap_or(0.0))
                    .sum();
            }
            let d = self.sum / SPB as f32 - self.eye.mid();
            self.diff.push(d);
            let ph = (n % SPB) as usize;
            self.regs[ph] = ((self.regs[ph] << 1) | (d > 0.0) as u64) & MASK;
            if let Some(&(h, ..)) = self.hits.first() {
                if n > h + 4 {
                    let (at, inv, reg, swing) = self.hits[self.hits.len() / 2];
                    self.hits.clear();
                    self.suppress_until = at + 38 * SPB;
                    let half = swing / 2.0;
                    let weak = (0..40u64)
                        .filter(|k| {
                            self.diff.get(at - SPB * (39 - k)).unwrap_or(0.0).abs()
                                < WEAK_FRACTION * half
                        })
                        .count();
                    let eye_ok = weak <= MAX_WEAK_BITS;
                    let word = if inv { !reg & MASK } else { reg };
                    out.push(Timed {
                        time: at as f64 / RATE,
                        value: LtrCandidate {
                            time: at as f64 / RATE,
                            word,
                            inverted: inv,
                            swing,
                            eye_ok,
                        },
                    });
                }
            }
            if n >= 400 && n >= self.suppress_until {
                let reg = self.regs[ph];
                let top = reg >> 31;
                if top == SYNC || top == (!SYNC & 0x1FF) {
                    self.hits.push((n, top != SYNC, reg, self.eye.range()));
                }
            }
            self.n += 1;
        }
    }
    /// The data swing now (envelope range), Hz.
    pub fn swing(&self) -> f32 {
        self.eye.range()
    }
}

impl Component for LtrSync {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ltr.sync",
            vec![Port::real("in")],
            vec![typed_port::<LtrCandidate>("out")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = LtrSync::default();
        Ok(vec![typed_format::<LtrCandidate>(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let mut out = std::mem::take(io.items_out::<LtrCandidate>(0));
        self.run(x, &mut out);
        *io.items_out::<LtrCandidate>(0) = out;
    }
}

/// `ltr.word`: a word that passed the swing and eye checks. It is `good` when its check bits and fields are right too.
#[derive(Clone, Debug, PartialEq)]
pub struct LtrWord {
    /// Signal time, s.
    pub time: f64,
    /// The fields.
    pub fields: LtrFields,
    /// The 7 check bits as received.
    pub check: u8,
    /// Found with the inverted sync.
    pub inverted: bool,
    /// "{AREA}-{HOME}-{ID}".
    pub talkgroup: String,
    /// What the ID says.
    pub kind: WordKind,
    /// The check and the fields are right.
    pub good: bool,
}

impl Item for LtrWord {
    const KIND: &'static str = "ltr.words";
}

/// `ltr.check`: swing at least 80 Hz, the eye check, then the check bits and the field ranges.
#[derive(Default)]
pub struct LtrCheck;

/// Reads a candidate: `None` when it is ignored (swing or eye), else the word, good or bad.
pub fn check(c: &LtrCandidate) -> Option<LtrWord> {
    if c.swing < 80.0 || !c.eye_ok {
        return None;
    }
    let f = code::fields(c.word);
    let good = code::check_ok(c.word, c.inverted) && code::plausible(&f);
    Some(LtrWord {
        time: c.time,
        fields: f,
        check: code::check_bits(c.word),
        inverted: c.inverted,
        talkgroup: code::talkgroup(&f),
        kind: code::kind(f.id),
        good,
    })
}

impl Component for LtrCheck {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ltr.check",
            vec![Port::typed::<LtrCandidate>("in")],
            vec![typed_port::<LtrWord>("out")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![typed_format::<LtrWord>(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        let cands = io.items::<LtrCandidate>(0);
        let out = io.items_out::<LtrWord>(0);
        for c in cands {
            if let Some(w) = check(&c.value) {
                out.push(Timed {
                    time: c.time,
                    value: w,
                });
            }
        }
    }
}

/// A good word within this long keeps the lock, s.
const LOCK_S: f64 = 15.0;

/// One good word of the list.
#[derive(Clone, Debug, PartialEq)]
struct WordRow(LtrWord);

impl Shaped for WordRow {
    type Row = LtrWordRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let w = &self.0;
        let f = &w.fields;
        out.put(&LtrWordRow {
            talkgroup: &w.talkgroup,
            area: i64::from(f.area),
            goto: i64::from(f.goto),
            home: i64::from(f.home),
            id: i64::from(f.id),
            free: i64::from(f.free),
            check: i64::from(w.check),
            kind: w.kind,
            time_s: w.time,
        });
    }
}

/// How often a repeater was named in each field.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RepeaterRow {
    /// The repeater.
    pub repeater: u8,
    /// Times seen as go-to.
    pub as_goto: u32,
    /// As home.
    pub as_home: u32,
    /// As free.
    pub as_free: u32,
}

impl Shaped for RepeaterRow {
    type Row = LtrRepeaterRow;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&LtrRepeaterRow {
            repeater: i64::from(self.repeater),
            as_goto: i64::from(self.as_goto),
            as_home: i64::from(self.as_home),
            as_free: i64::from(self.as_free),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

const REPEATER_ROW: sdrkit_core::settings::Argument<sdrkit_core::publish::field::Text> =
    sdrkit_util_objects::commands::row_of(LTR_REPEATERS.def().key);
const REMOVE: sdrkit_core::command::Command =
    sdrkit_util_objects::commands::remove_command(&[&REPEATER_ROW]);

sdrkit_core::commands! {
    /// The commands of `ltr.tables`.
    pub enum LtrCommand {
        /// Empties the words and the repeaters.
        Clear = CLEAR,
        /// Forgets one repeater.
        Remove = REMOVE,
    }
}

/// `ltr.tables`: the word list, the repeater table, the area, the lock, the events and the voice grants.
pub struct LtrTables {
    p: Published,
    words: LogRing<WordRow>,
    reps: ObjectTable<u8, RepeaterRow>,
    configured: bool,
    last_good: Option<f64>,
    now: f64,
    good: u64,
    bad: u64,
    syncs: u64,
    area: Option<u8>,
    data_valid: bool,
    level_dbfs: f32,
    swing_hz: f32,
    events: Vec<Event>,
    grants: Vec<Timed<Grant>>,
    freq_hz: f64,
    summary: String,
}

impl Default for LtrTables {
    fn default() -> Self {
        LtrTables {
            p: Published::new(crate::typed::LtrFields::SHAPE.fields)
                .with_tables(&[LTR_WORDS.def(), LTR_REPEATERS.def()]),
            words: LogRing::new(400),
            reps: ObjectTable::new(20),
            configured: false,
            last_good: None,
            now: 0.0,
            good: 0,
            bad: 0,
            syncs: 0,
            area: None,
            data_valid: false,
            level_dbfs: -200.0,
            swing_hz: 0.0,
            events: Vec::new(),
            grants: Vec::new(),
            freq_hz: 0.0,
            summary: String::new(),
        }
    }
}

impl LtrTables {
    fn take(&mut self, w: &LtrWord) {
        if !w.good {
            self.bad += 1;
            return;
        }
        self.good += 1;
        let f = w.fields;
        self.area = Some(f.area);
        self.data_valid = true;
        self.last_good = Some(w.time);
        self.words.push(WordRow(w.clone()));
        for (r, which) in [(f.goto, 0), (f.home, 1), (f.free, 2)] {
            if (1..=20).contains(&r) {
                if let Ok(up) = self.reps.upsert(&r, Stamp::at(w.time), || RepeaterRow {
                    repeater: r,
                    ..Default::default()
                }) {
                    match which {
                        0 => up.row.as_goto += 1,
                        1 => up.row.as_home += 1,
                        _ => up.row.as_free += 1,
                    }
                }
            }
        }
        let line = if w.kind == WordKind::Call {
            format!("LTR {}", w.talkgroup)
        } else {
            format!("LTR {} {}", w.talkgroup, choice_label(w.kind))
        };
        self.p.results_mut().log(line);
        self.events.push(LTR_WORD.event(
            w.time,
            self.freq_hz,
            &LtrWordEvent {
                target: &w.talkgroup,
                area: i64::from(f.area),
                goto: i64::from(f.goto),
                home: i64::from(f.home),
                id: i64::from(f.id),
                free: i64::from(f.free),
                kind: w.kind,
                family: keys::Family::Voice,
            },
        ));
        if w.kind == WordKind::Call && (1..=20).contains(&f.goto) {
            self.grants.push(Timed {
                time: w.time,
                value: Grant {
                    protocol: "LTR",
                    talkgroup: w.talkgroup.clone(),
                    talkgroup_id: i64::from(f.id),
                    channel: i32::from(f.goto),
                    group: true,
                    time: w.time,
                    ..Default::default()
                },
            });
        }
    }
    fn publish(&mut self) {
        let locked = self.last_good.is_some_and(|t| self.now - t <= LOCK_S);
        let lock_state = if !self.configured {
            LockState::Off
        } else if locked {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let level = Some(self.level_dbfs);
        let r = self.p.results_mut();
        if !self.configured {
            set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("LTR: waiting for samples"),
            );
        } else if !self.data_valid {
            set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("LTR: listening ({:.1} dBFS)", self.level_dbfs),
            );
        } else {
            set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!(
                    "LTR: {}, {} words, {} repeaters",
                    if locked { "locked" } else { "no lock" },
                    self.good,
                    self.reps.len()
                ),
            );
        }
        let p = &mut self.p;
        p.put(&crate::typed::LtrFields {
            locked,
            words_good: self.good as i64,
            words_bad: self.bad as i64,
            syncs: self.syncs as i64,
            area: self.area.map(i64::from),
            repeaters: self.reps.len() as i64,
            level_dbfs: f64::from(self.level_dbfs),
            swing_hz: Hertz(f64::from(self.swing_hz)),
            data_valid: self.data_valid,
        });
        log_rows(&self.words, &LTR_WORDS, self.now, p);
        rows(&self.reps, &LTR_REPEATERS, self.now, p);
    }
    /// Empties the words and repeaters and drops "words heard" (the counters stay).
    pub fn clear(&mut self) {
        self.words.clear();
        self.reps.clear();
        self.data_valid = false;
        self.p.clear_rows(&LTR_WORDS);
        self.p.clear_rows(&LTR_REPEATERS);
    }
}

/// The label of a choice, for people.
fn choice_label<E: sdrkit_core::Choice>(e: E) -> &'static str {
    E::CHOICES[usize::from(e.index())].label.as_ref()
}

impl Component for LtrTables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ltr.tables",
            vec![
                Port::typed::<LtrCandidate>("candidates"),
                Port::typed::<LtrWord>("words"),
                Port::typed::<Level>("level"),
            ],
            vec![
                typed_port::<Grant>("grants"),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = LtrTables::default();
        self.configured = true;
        self.freq_hz = inputs[0].tuned_hz;
        self.publish();
        Ok(vec![
            typed_format::<Grant>(&inputs[0]),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for c in io.items::<LtrCandidate>(0) {
            self.syncs += 1;
            self.swing_hz = c.value.swing;
            self.now = self.now.max(c.time);
        }
        for w in io.items::<LtrWord>(1) {
            self.now = self.now.max(w.time);
            self.take(&w.value);
        }
        if let Some(l) = io.items::<Level>(2).last() {
            self.level_dbfs = l.value.dbfs;
            self.now = self.now.max(l.time);
        }
        io.items_out::<Grant>(0).append(&mut self.grants);
        io.messages_out(1).append(&mut self.events);
        if io.tick() {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn commands(&self) -> &Commands {
        LtrCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match LtrCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            LtrCommand::Clear => self.clear(),
            LtrCommand::Remove => {
                let row = args.arg(&REPEATER_ROW)?;
                if !sdrkit_util_objects::rows::remove_row(&mut self.reps, row) {
                    return Err(self.p.no_such_row(&LTR_REPEATERS, row));
                }
                self.p.remove_row(&LTR_REPEATERS, row);
            }
        }
        Ok(None)
    }
}
