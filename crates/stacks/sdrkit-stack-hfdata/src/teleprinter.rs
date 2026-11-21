//! What the teleprinter stacks of this crate share (`arqe`, `autospec`, `gtor`, `efr`, `piccolo`, `coquelet`): the figures case
//! of ITA2, the 8 kHz channel, the decoder that runs a front ([`BinaryFskFront`] or [`ToneSlotFront`]) and hands its symbols to
//! a stack's own [`Layer`], the shared results and events, and the generator of a tone sequence.
//!
//! The decoder takes the 24 kHz channel of the family front end (the dial at 0 Hz, the upper sideband at positive frequencies),
//! sums each three samples to 8000 samples/s, and runs the front and the layer one sample at a time, so the output does not
//! depend on how the input is cut.

use crate::serial::gen::test_dial;
use crate::typed::{
    FskLockEvent, FskLostEvent, TeleprinterFields, TextEvent, FSK_LOCK, FSK_LOST, TEXT,
};
use sdrkit_algo_modem::fskauto::{BinaryFskConfig, BinaryFskFront, FskLock, FskStage};
use sdrkit_algo_modem::toneslots::{Slot, SlotLock, ToneSlotConfig, ToneSlotFront};
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::interface::Fields as _;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Clock, Complex32, ConfigError, Event, Generator, Hz, LockState, Record, ResultsDecl, Schema,
    Setting, SettingError, SettingId, SignalInfo, Status, Value, Values,
};
use sdrkit_graph::{Format, Kind, Outbox, Port, RateFrom, RunContext};
use sdrkit_util_text::ita2;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The input rate.
pub const RATE_IN: f64 = 24_000.0;
/// The rate of the channel the fronts work on.
pub const RATE: f64 = 8000.0;
/// The text kept, characters.
pub const TEXT_KEEP: usize = 6000;

sdrkit_core::choice_enum! {
    /// The figures case of the ITA2 alphabet.
    pub enum Alphabet {
        /// The US teleprinter figures.
        Us = ("us", "US"),
        /// The ITU figures (ITU-T S.1, ITU-R M.476 Table 1).
        Itu = ("itu", "ITU"),
    }
}

impl Alphabet {
    /// The ITA2 table of this figures case.
    pub fn table(self) -> &'static ita2::Table {
        match self {
            Alphabet::Us => &ita2::US_TTY,
            Alphabet::Itu => &ita2::ITU,
        }
    }
}

/// The figures case.
pub const ALPHABET: Setting<field::Choice<Alphabet>> =
    Setting::choice("alphabet", "Figures case", Alphabet::Us)
        .help("The figures of the ITA2 alphabet: US teleprinter or ITU.");

sdrkit_core::choice_enum! {
    /// The polarity of the keying.
    pub enum Polarity {
        /// Both are tried.
        Auto = ("auto", "Automatic"),
        /// The higher tone is mark (1).
        Normal = ("normal", "Normal"),
        /// The lower tone is mark (1).
        Inverted = ("inverted", "Inverted"),
    }
}

/// The polarity.
pub const POLARITY: Setting<field::Choice<Polarity>> =
    Setting::choice("polarity", "Polarity", Polarity::Auto);

impl Polarity {
    /// The polarities to try: `false` normal, `true` inverted.
    pub fn tries(self) -> &'static [bool] {
        match self {
            Polarity::Auto => &[false, true],
            Polarity::Normal => &[false],
            Polarity::Inverted => &[true],
        }
    }
}

/// The `reset` command: drops the tone lock, the word lock and partial frames; keeps the text and the tables.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Searches the signal again; keeps the text and the tables.");
static COMMANDS: [&Command; 1] = [&RESET];

/// The front a layer reads.
#[derive(Clone, Debug)]
pub enum FrontSpec {
    /// 2-FSK: soft bits.
    Fsk(BinaryFskConfig),
    /// One tone at a time in slots: the strongest tone of each slot.
    Slots(ToneSlotConfig),
}

/// What the front locked to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrontLock {
    /// A 2-FSK lock.
    Fsk(FskLock),
    /// A tone-slot lock.
    Slots(SlotLock),
}

impl FrontLock {
    /// The symbol rate, Bd.
    pub fn baud(&self) -> f64 {
        match self {
            FrontLock::Fsk(l) => l.baud,
            FrontLock::Slots(l) => 1.0 / l.slot_s,
        }
    }
    /// The shift of a 2-FSK lock, 0 for tone slots.
    pub fn shift(&self) -> f64 {
        match self {
            FrontLock::Fsk(l) => l.shift(),
            FrontLock::Slots(_) => 0.0,
        }
    }
}

/// The symbols of a front.
pub enum Symbols<'a> {
    /// Soft bits in `[-1, 1]`, positive for the higher tone.
    Soft(&'a [f32]),
    /// Tone slots.
    Slots(&'a [Slot]),
}

/// Where a layer writes what it read.
pub struct Sink {
    /// The time of the newest sample, UTC seconds.
    pub time: f64,
    /// The dial, Hz.
    pub dial: f64,
    /// The characters read.
    pub chars: String,
    /// Events.
    pub events: Vec<Event>,
    /// Good words, characters or frames.
    pub good: u64,
    /// Bad ones.
    pub bad: u64,
    /// Sync taken: the event text and the detail.
    pub locked: Option<(String, String)>,
    /// Sync lost: the reason.
    pub lost: Option<String>,
    /// The front must search the signal again.
    pub restart: bool,
}

impl Sink {
    fn new() -> Self {
        Sink {
            time: 0.0,
            dial: 0.0,
            chars: String::new(),
            events: Vec::new(),
            good: 0,
            bad: 0,
            locked: None,
            lost: None,
            restart: false,
        }
    }
    /// Reports a lock with the event text `text` and the status detail `detail`.
    pub fn lock(&mut self, text: String, detail: String) {
        self.locked = Some((text, detail));
    }
    /// Reports a lost lock.
    pub fn lose(&mut self, reason: &str) {
        self.lost = Some(reason.to_string());
    }
    /// Reports a lost lock and asks the front to search the signal again.
    pub fn lose_signal(&mut self, reason: &str) {
        self.lose(reason);
        self.restart = true;
    }
}

/// The part of a teleprinter stack after the front.
pub trait Layer: Send + 'static {
    /// The registry name, `"hfdata.<name>"`.
    fn name(&self) -> &'static str;
    /// The settings.
    fn schema(&self) -> Arc<Schema>;
    /// The value of every setting.
    fn values(&self) -> Values;
    /// Applies a setting. Returns true when the front must be built again.
    ///
    /// # Errors
    ///
    /// [`SettingError`] for a value it refuses.
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<bool, SettingError>;
    /// The front to build.
    fn front(&self) -> FrontSpec;
    /// Adds its own tables and events to the declaration.
    fn declare(&self, d: ResultsDecl) -> ResultsDecl {
        d
    }
    /// What no public document confirms.
    fn note(&self) -> &'static str;
    /// The front locked.
    fn front_locked(&mut self, lock: FrontLock);
    /// Reads symbols.
    fn symbols(&mut self, s: Symbols<'_>, sink: &mut Sink);
    /// Whether it reads words now.
    fn decoding(&self) -> bool;
    /// What the status shows beside the speed.
    fn detail(&self) -> String;
    /// The centre of the signal for the status, Hz.
    fn centre(&self, lock: &FrontLock) -> f64 {
        match lock {
            FrontLock::Fsk(l) => l.centre(),
            FrontLock::Slots(l) => l.base_hz,
        }
    }
    /// Drops the word lock and partial frames.
    fn reset(&mut self);
    /// Forgets its tables.
    fn clear(&mut self) {}
    /// Writes its tables.
    fn report(&mut self, _res: &mut Published) {}
}

enum Front {
    Fsk(BinaryFskFront),
    Slots(ToneSlotFront),
}

impl Front {
    fn build(spec: &FrontSpec) -> Option<Front> {
        match spec {
            FrontSpec::Fsk(c) => BinaryFskFront::new(c.clone()).ok().map(Front::Fsk),
            FrontSpec::Slots(c) => ToneSlotFront::new(c.clone()).ok().map(Front::Slots),
        }
    }
    fn lock(&self) -> Option<FrontLock> {
        match self {
            Front::Fsk(f) => f.lock().map(FrontLock::Fsk),
            Front::Slots(f) => f.lock().map(FrontLock::Slots),
        }
    }
    fn found(&self) -> bool {
        match self {
            Front::Fsk(f) => f.stage() != FskStage::Searching,
            Front::Slots(f) => f.base_hz().is_some(),
        }
    }
    fn take_new_lock(&mut self) -> bool {
        match self {
            Front::Fsk(f) => f.take_new_lock(),
            Front::Slots(f) => f.take_new_lock(),
        }
    }
    fn restart(&mut self) {
        match self {
            Front::Fsk(f) => f.restart(),
            Front::Slots(f) => f.reset(),
        }
    }
    fn reset(&mut self) {
        match self {
            Front::Fsk(f) => f.reset(),
            Front::Slots(f) => f.reset(),
        }
    }
}

/// Symbols after a front lock with no sync, after which the front searches again.
const NO_SYNC_SYMBOLS: u64 = 3000;

/// The decoder of a teleprinter stack: the 8 kHz channel, the front and the layer `L`.
pub struct TtyDecoder<L: Layer> {
    layer: L,
    front: Option<Front>,
    acc: Complex32,
    n: u64,
    n8: u64,
    power: f64,
    soft: Vec<f32>,
    slots: Vec<Slot>,
    text: String,
    line: String,
    line_at: u64,
    good: i64,
    bad: i64,
    detail: String,
    since_lock: u64,
    clock: Option<Clock>,
    utc0: Option<f64>,
    dial: f64,
    status: Status,
    sink: Sink,
}

impl<L: Layer> TtyDecoder<L> {
    /// The decoder of `layer`.
    pub fn new(layer: L) -> Self {
        let front = Front::build(&layer.front());
        TtyDecoder {
            layer,
            front,
            acc: Complex32::default(),
            n: 0,
            n8: 0,
            power: 0.0,
            soft: Vec::new(),
            slots: Vec::new(),
            text: String::new(),
            line: String::new(),
            line_at: 0,
            good: 0,
            bad: 0,
            detail: String::new(),
            since_lock: 0,
            clock: None,
            utc0: None,
            dial: 0.0,
            status: Status::default(),
            sink: Sink::new(),
        }
    }

    /// The layer.
    pub fn layer(&self) -> &L {
        &self.layer
    }

    fn reset(&mut self) {
        if let Some(f) = &mut self.front {
            f.reset();
        }
        self.layer.reset();
        self.detail.clear();
        self.since_lock = 0;
    }

    fn rebuild(&mut self) {
        self.front = Front::build(&self.layer.front());
        self.layer.reset();
        self.detail.clear();
        self.since_lock = 0;
    }

    /// Runs one 8 kHz sample through the front and the layer.
    fn step8(&mut self, v: Complex32, out: &mut Outbox) {
        let Some(front) = &mut self.front else {
            return;
        };
        self.soft.clear();
        self.slots.clear();
        match front {
            Front::Fsk(f) => f.process(&[v], &mut self.soft),
            Front::Slots(f) => f.process(&[v], &mut self.slots),
        }
        self.n8 += 1;
        if front.take_new_lock() {
            if let Some(l) = front.lock() {
                self.layer.reset();
                self.layer.front_locked(l);
                self.since_lock = 0;
            }
        }
        if self.soft.is_empty() && self.slots.is_empty() {
            return;
        }
        self.sink.time = self.utc0.unwrap_or(0.0) + self.n as f64 / RATE_IN;
        self.sink.dial = self.dial;
        let k = if self.soft.is_empty() {
            self.layer
                .symbols(Symbols::Slots(&self.slots), &mut self.sink);
            self.slots.len()
        } else {
            self.layer
                .symbols(Symbols::Soft(&self.soft), &mut self.sink);
            self.soft.len()
        };
        if std::mem::take(&mut self.sink.restart) {
            if let Some(f) = &mut self.front {
                f.restart();
            }
            self.layer.reset();
            self.since_lock = 0;
        } else if self.layer.decoding() {
            self.since_lock = 0;
        } else {
            self.since_lock += k as u64;
            if self.since_lock > NO_SYNC_SYMBOLS {
                self.since_lock = 0;
                if let Some(f) = &mut self.front {
                    f.restart();
                }
            }
        }
        self.flush(out);
    }

    fn flush(&mut self, out: &mut Outbox) {
        let lock = self.front.as_ref().and_then(Front::lock);
        if let Some((text, detail)) = self.sink.locked.take() {
            self.detail = detail.clone();
            let (b, s, c) = lock.map_or((0.0, 0.0, 0.0), |l| {
                (l.baud(), l.shift(), self.layer.centre(&l))
            });
            out.messages(0).push(FSK_LOCK.event(
                self.sink.time,
                self.dial,
                &FskLockEvent {
                    text: &text,
                    baud: b,
                    shift_hz: s,
                    centre_hz: c,
                    detail: &detail,
                },
            ));
        }
        if let Some(reason) = self.sink.lost.take() {
            out.messages(0).push(FSK_LOST.event(
                self.sink.time,
                self.dial,
                &FskLostEvent {
                    text: &reason,
                    reason: &reason,
                },
            ));
        }
        for e in self.sink.events.drain(..) {
            out.messages(0).push(e);
        }
        self.good += self.sink.good as i64;
        self.bad += self.sink.bad as i64;
        self.sink.good = 0;
        self.sink.bad = 0;
        let chars = std::mem::take(&mut self.sink.chars);
        if !chars.is_empty() {
            self.line_at = self.n;
        }
        for c in chars.chars() {
            self.text.push(c);
            if c == '\n' || c == '\r' {
                if !self.line.trim().is_empty() {
                    self.text_event(out);
                }
                self.line.clear();
            } else {
                self.line.push(c);
                if self.line.chars().count() >= 64 {
                    self.text_event(out);
                    self.line.clear();
                }
            }
        }
        if self.text.len() > TEXT_KEEP + 2000 {
            let cut = self.text.len() - TEXT_KEEP;
            let cut = (cut..self.text.len())
                .find(|&i| self.text.is_char_boundary(i))
                .unwrap_or(0);
            self.text.drain(..cut);
        }
    }

    fn text_event(&self, out: &mut Outbox) {
        out.messages(0).push(TEXT.event(
            self.sink.time,
            self.dial,
            &TextEvent { text: &self.line },
        ));
    }
}

/// The results of every teleprinter stack.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(TeleprinterFields::SHAPE.fields)
        .event(&FSK_LOCK)
        .event(&FSK_LOST)
        .event(&TEXT)
}

impl<L: Layer> sdrkit_graph::Decoder for TtyDecoder<L> {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        self.layer.name()
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        self.layer.schema()
    }
    fn values(&self) -> Values {
        self.layer.values()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        if self.layer.apply(id, value)? {
            self.rebuild();
        }
        Ok(())
    }
    fn commands(&self) -> &'static [&'static Command] {
        &COMMANDS
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        if id.index() == 0 {
            self.reset();
            Ok(None)
        } else {
            Err(CommandError::NoSuchId { id })
        }
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(self.layer.declare(declaration())))
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = Some(ctx.clock.clone());
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        crate::serial::check_rate(input)?;
        self.dial = input.tuned_hz;
        self.reset();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.reset();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        if self.utc0.is_none() {
            let now = self.clock.as_ref().map_or(0.0, Clock::now);
            self.utc0 = Some(now - self.n as f64 / RATE_IN);
        }
        for &v in x {
            let v = if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::default()
            };
            self.n += 1;
            self.power = 0.9999 * self.power + 0.0001 * f64::from(v.norm_sqr());
            self.acc += v;
            if self.n.is_multiple_of(3) {
                let s = std::mem::take(&mut self.acc) / 3.0;
                self.step8(s, out);
            }
            // A line that has had no new character for 2 s is an event too.
            if !self.line.is_empty() && self.n - self.line_at > 2 * RATE_IN as u64 {
                self.sink.time = self.utc0.unwrap_or(0.0) + self.n as f64 / RATE_IN;
                self.text_event(out);
                self.line.clear();
            }
        }
    }
    fn report(&mut self, res: &mut Published) {
        self.layer.report(res);
        let lock = self.front.as_ref().and_then(Front::lock);
        let found = self.front.as_ref().is_some_and(Front::found);
        let state = if self.layer.decoding() {
            2
        } else if found {
            1
        } else {
            0
        };
        let detail = if state == 2 { self.detail.as_str() } else { "" };
        let level = (10.0 * self.power.max(1e-20).log10()) as f32;
        res.put(&TeleprinterFields {
            note: self.layer.note(),
            text: &self.text,
            state,
            baud: lock.map(|l| (l.baud() * 100.0).round() / 100.0),
            centre_hz: lock.map(|l| (self.layer.centre(&l) * 10.0).round() / 10.0),
            shift_hz: lock
                .filter(|l| l.shift() > 0.0)
                .map(|l| (l.shift() * 10.0).round() / 10.0),
            detail,
            good: self.good,
            bad: self.bad,
            level_dbfs: f64::from((level * 10.0).round() / 10.0),
        });
        self.status.lock_state = match state {
            2 => LockState::Locked,
            1 => LockState::Tracking,
            _ => LockState::Searching,
        };
        self.status.level_dbfs = Some(level);
        self.status.summary = match (state, lock) {
            (2, Some(l)) => format!("{:.2} Bd, {}", l.baud(), self.detail),
            (_, Some(l)) => format!("{:.2} Bd, waiting for sync", l.baud()),
            (1, None) => "signal found".to_string(),
            _ => "searching".to_string(),
        };
        res.results_mut().set_status(&self.status);
    }
    fn clear(&mut self) {
        self.text.clear();
        self.line.clear();
        self.good = 0;
        self.bad = 0;
        self.layer.clear();
    }
}

/// A test signal of tones one after the other, phase continuous: `(seconds, Some(Hz above the dial))` or `(seconds, None)` for
/// silence, repeated without end, at unit power while a tone sounds. The dial lies where a receiver of the crate looks
/// ([`test_dial`]).
pub struct ToneSeqGen {
    rate: f64,
    dial: f64,
    ends: Vec<f64>,
    freqs: Vec<Option<f64>>,
    period: f64,
    at: usize,
    n: u64,
    phase: f64,
    bandwidth: f64,
}

impl ToneSeqGen {
    /// The sequence `seq` at `rate`; `bandwidth` Hz is the signal width the generator reports.
    pub fn new(rate: f64, seq: &[(f64, Option<f64>)], bandwidth: f64) -> Self {
        let mut acc = 0.0;
        let ends = seq
            .iter()
            .map(|s| {
                acc += s.0;
                acc
            })
            .collect();
        ToneSeqGen {
            rate,
            dial: test_dial(rate),
            ends,
            freqs: seq.iter().map(|s| s.1).collect(),
            period: acc.max(1e-3),
            at: 0,
            n: 0,
            phase: 0.0,
            bandwidth,
        }
    }
}

impl Generator for ToneSeqGen {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.bandwidth))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        self.dial
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let cycle = (t / self.period).floor();
            let tc = t - cycle * self.period;
            if self.at > 0 && tc < self.ends[self.at - 1] {
                self.at = 0;
            }
            while self.at < self.ends.len() && tc >= self.ends[self.at] {
                self.at += 1;
            }
            self.n += 1;
            match self.freqs.get(self.at).copied().flatten() {
                Some(f) => {
                    self.phase = (self.phase + TAU * (f + self.dial) / self.rate).rem_euclid(TAU);
                    *o = Complex32::new(self.phase.cos() as f32, self.phase.sin() as f32);
                }
                None => {
                    self.phase = (self.phase + TAU * self.dial / self.rate).rem_euclid(TAU);
                    *o = Complex32::default();
                }
            }
        }
        out.len()
    }
}

/// Writes `code` of `table` to `out`, following the shift in `figs`; LTRS and FIGS only shift, null prints nothing, and the
/// bell and who-are-you are dropped.
pub fn put_ita2(table: &ita2::Table, code: u8, figs: &mut bool, out: &mut String) {
    match code {
        ita2::LTRS => *figs = false,
        ita2::FIGS => *figs = true,
        ita2::BLANK => {}
        _ => {
            if let Some(ch) = table.char_of(code, *figs) {
                if ch == '\n' || ch == '\r' || !ch.is_control() {
                    out.push(ch);
                }
            }
        }
    }
}

/// The ITA2 codes of `text` after a LTRS (the US figures case).
pub fn ita2_codes(text: &str) -> Vec<u8> {
    let mut codes = vec![ita2::LTRS];
    let _ = ita2::Encoder::new(&ita2::US_TTY).encode_str(text, &mut codes);
    codes
}
