//! What the HF sender stacks share (`xpa`, `crowd36`, `fsk200`, `gwfsk`, `fsk_modems`, `uvb76`, `vlf_msk`): the decoder that
//! takes the 24 kHz channel of the family front end, replaces a sample that is not finite by zero, filters and decimates it to
//! the channel of the stack, and runs the stack's [`Layer`] in pieces of a fixed count of channel samples, so the output does
//! not depend on how the input is cut; the log of the rows of a table, the text kept, and a tone-sequence generator.

use crate::serial::gen::test_dial;
use sdrkit_algo_dsp::decim::Decimator;
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Clock, Complex32, ConfigError, Event, Generator, Hz, LockState, Record, ResultsDecl, Schema,
    SettingError, SettingId, SignalInfo, Status, Value, Values,
};
use sdrkit_graph::{Format, Kept, Kind, Outbox, Port, RateFrom, RunContext};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The input rate.
pub const RATE_IN: f64 = 24_000.0;
/// The rows a table keeps.
pub const ROWS_KEPT: usize = 500;
/// The input samples over which the level is measured (0.1 s).
const LEVEL_BLOCK: u64 = 2400;
/// The characters of text kept.
pub const TEXT_KEPT: usize = 6000;

/// What a layer writes while it reads: the time of the newest sample and the events.
pub struct Cx<'a> {
    /// The UTC time of the newest sample, seconds.
    pub time: f64,
    /// The dial, Hz.
    pub dial: f64,
    /// The events.
    pub events: &'a mut Vec<Event>,
}

/// The state that a layer reports for the status of the stack.
#[derive(Clone, Debug, Default)]
pub struct StatusLine {
    /// 0 searching, 1 signal found, 2 decoding.
    pub state: u8,
    /// The headline.
    pub summary: String,
    /// The SNR, dB.
    pub snr_db: Option<f64>,
    /// The frequency error or offset, Hz.
    pub offset_hz: Option<f64>,
}

/// The reader of one stack on its channel.
pub trait Layer: Send + 'static {
    /// The registry name of the decoder.
    const NAME: &'static str;
    /// The decimation from 24 kHz.
    const DECIM: usize;
    /// The flat pass band of the channel, Hz from 0.
    const PASS_HZ: f64;
    /// The settings the decoder takes.
    fn schema() -> Arc<Schema>;
    /// The results it declares.
    fn declaration() -> ResultsDecl;
    /// The present value of every setting.
    fn values(&self) -> Values;
    /// Applies a setting.
    ///
    /// # Errors
    ///
    /// [`SettingError`] for a value the layer refuses.
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError>;
    /// Reads channel samples.
    fn push(&mut self, x: &[Complex32], cx: &mut Cx);
    /// The input ended.
    fn finish(&mut self, cx: &mut Cx) {
        let _ = cx;
    }
    /// Drops the lock, the clocks, the readings and partial lines; keeps the text and the tables.
    fn reset(&mut self);
    /// Empties the text, the tables, the events and the counters; keeps the settings and the measured tones.
    fn clear(&mut self);
    /// The host changed the frequency: drops the lock, and clears what `kept` does not name.
    fn forget(&mut self, kept: &Kept);
    /// The dial is known or changed.
    fn tuned(&mut self, dial_hz: f64) {
        let _ = dial_hz;
    }
    /// Writes the results; `level_dbfs` is the level of the channel.
    fn report(&mut self, res: &mut Published, level_dbfs: f64) -> StatusLine;
}

/// The command that drops the lock.
pub static COMMANDS: [&Command; 1] = [&crate::teleprinter::RESET];

/// A layer as a decoder (module documentation).
pub struct Sender<L: Layer> {
    layer: L,
    decim: Decimator<Complex32>,
    piece: usize,
    chan: Vec<Complex32>,
    out: Vec<Complex32>,
    n_chan: u64,
    clock: Option<Clock>,
    utc0: Option<f64>,
    dial: f64,
    power: f64,
    count: u64,
    level: f64,
    events: Vec<Event>,
    status: Status,
}

impl<L: Layer> Sender<L> {
    /// The decoder of `layer`.
    pub fn new(layer: L) -> Self {
        let rate = RATE_IN / L::DECIM as f64;
        Sender {
            layer,
            decim: Decimator::new(RATE_IN, L::DECIM, L::PASS_HZ, 60.0),
            piece: (rate / 100.0) as usize,
            chan: Vec::new(),
            out: Vec::new(),
            n_chan: 0,
            clock: None,
            utc0: None,
            dial: 0.0,
            power: 0.0,
            count: 0,
            level: -150.0,
            events: Vec::new(),
            status: Status::default(),
        }
    }

    /// The layer.
    pub fn layer(&self) -> &L {
        &self.layer
    }

    fn rate(&self) -> f64 {
        RATE_IN / L::DECIM as f64
    }

    fn run(&mut self, out: &mut Outbox) {
        let mut at = 0;
        while self.chan.len() - at >= self.piece {
            self.n_chan += self.piece as u64;
            let mut cx = Cx {
                time: self.utc0.unwrap_or(0.0) + self.n_chan as f64 / self.rate(),
                dial: self.dial,
                events: &mut self.events,
            };
            self.layer.push(&self.chan[at..at + self.piece], &mut cx);
            at += self.piece;
        }
        self.chan.drain(..at);
        for e in self.events.drain(..) {
            out.messages(0).push(e);
        }
    }
}

impl<L: Layer> sdrkit_graph::Decoder for Sender<L> {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        L::NAME
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        L::schema()
    }
    fn values(&self) -> Values {
        self.layer.values()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        self.layer.apply(id, value)
    }
    fn commands(&self) -> &'static [&'static Command] {
        &COMMANDS
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        if id.index() == 0 {
            self.layer.reset();
            Ok(None)
        } else {
            Err(CommandError::NoSuchId { id })
        }
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(L::declaration()))
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = Some(ctx.clock.clone());
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        crate::serial::check_rate(input)?;
        self.dial = input.tuned_hz;
        self.decim.reset();
        self.chan.clear();
        self.layer.reset();
        self.layer.tuned(self.dial);
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.layer.reset();
        self.layer.tuned(self.dial);
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        if self.utc0.is_none() {
            self.utc0 = Some(self.clock.as_ref().map_or(0.0, Clock::now));
        }
        self.out.clear();
        for &v in x {
            let v = if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::default()
            };
            self.power += f64::from(v.norm_sqr());
            self.count += 1;
            if self.count == LEVEL_BLOCK {
                let p = self.power / LEVEL_BLOCK as f64;
                self.level = ((100.0 * p.max(1e-15).log10()).round() / 10.0).max(-150.0);
                self.power = 0.0;
                self.count = 0;
            }
            self.out.push(v);
        }
        let mut y = std::mem::take(&mut self.chan);
        self.decim.process(&self.out, &mut y);
        self.chan = y;
        self.run(out);
    }
    fn finish(&mut self, out: &mut Outbox) {
        let mut cx = Cx {
            time: self.utc0.unwrap_or(0.0) + self.n_chan as f64 / self.rate(),
            dial: self.dial,
            events: &mut self.events,
        };
        self.layer.finish(&mut cx);
        for e in self.events.drain(..) {
            out.messages(0).push(e);
        }
    }
    fn report(&mut self, res: &mut Published) {
        let s = self.layer.report(res, self.level);
        self.status.lock_state = match s.state {
            2 => LockState::Locked,
            1 => LockState::Tracking,
            _ => LockState::Searching,
        };
        self.status.snr_db = s.snr_db.filter(|v| v.is_finite()).map(|v| v as f32);
        self.status.offset_hz = s.offset_hz.filter(|v| v.is_finite());
        self.status.summary = s.summary;
        res.results_mut().set_status(&self.status);
    }
    fn clear(&mut self) {
        self.layer.clear();
    }
    fn forget(&mut self, kept: &Kept) {
        self.layer.forget(kept);
    }
}

/// The rows of one table, newest last, with the count of rows not yet written.
#[derive(Clone, Debug)]
pub struct RowLog<T> {
    rows: VecDeque<T>,
    serial: u64,
    new: usize,
    cleared: bool,
}

impl<T> Default for RowLog<T> {
    fn default() -> Self {
        RowLog {
            rows: VecDeque::new(),
            serial: 0,
            new: 0,
            cleared: false,
        }
    }
}

impl<T> RowLog<T> {
    /// Adds a row.
    pub fn push(&mut self, row: T) {
        self.rows.push_back(row);
        self.new += 1;
        if self.rows.len() > ROWS_KEPT {
            self.rows.pop_front();
        }
    }

    /// The rows, oldest first.
    pub fn rows(&self) -> impl Iterator<Item = &T> {
        self.rows.iter()
    }

    /// The count of rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether there is no row.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Forgets every row; the table is emptied at the next report.
    pub fn clear(&mut self) {
        self.rows.clear();
        self.new = 0;
        self.cleared = true;
    }

    /// Writes the new rows: `empty` empties the table after a clear, `put` writes one row under its key.
    pub fn report(
        &mut self,
        res: &mut Published,
        empty: impl FnOnce(&mut Published),
        mut put: impl FnMut(&mut Published, &str, &T),
    ) {
        if std::mem::take(&mut self.cleared) {
            empty(res);
        }
        let new = self.new.min(self.rows.len());
        let start = self.rows.len() - new;
        for (k, r) in self.rows.iter().skip(start).enumerate() {
            put(res, &format!("{:06}", self.serial + k as u64), r);
        }
        self.serial += new as u64;
        self.new = 0;
    }
}

/// Text kept to its last [`TEXT_KEPT`] characters.
#[derive(Clone, Debug, Default)]
pub struct TextLog {
    text: String,
}

impl TextLog {
    /// Adds a line.
    pub fn line(&mut self, s: &str) {
        self.text.push_str(s);
        self.text.push('\n');
        let n = self.text.chars().count();
        if n > TEXT_KEPT {
            let cut = self
                .text
                .char_indices()
                .nth(n - TEXT_KEPT)
                .map_or(0, |(i, _)| i);
            self.text.drain(..cut);
        }
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Forgets the text.
    pub fn clear(&mut self) {
        self.text.clear();
    }
}

/// One piece of a tone sequence: its length in seconds, its frequency (none for silence) and its amplitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tone {
    /// The length, seconds.
    pub secs: f64,
    /// The frequency above the dial, Hz; `None` for silence.
    pub hz: Option<f64>,
    /// The amplitude.
    pub amp: f32,
}

impl Tone {
    /// A tone of amplitude 1.
    pub fn new(secs: f64, hz: f64) -> Self {
        Tone {
            secs,
            hz: Some(hz),
            amp: 1.0,
        }
    }

    /// Silence.
    pub fn silence(secs: f64) -> Self {
        Tone {
            secs,
            hz: None,
            amp: 0.0,
        }
    }
}

/// The samples of `seq` at `rate`, phase continuous, the dial at `dial_hz` from the centre of the samples. A piece starts at
/// the first sample at or after the end of the one before, so the times do not drift.
pub fn render(seq: &[Tone], rate: f64, dial_hz: f64) -> Vec<Complex32> {
    let total: f64 = seq.iter().map(|t| t.secs).sum();
    let mut x = Vec::with_capacity((total * rate) as usize + 1);
    let mut phase = 0.0f64;
    let mut end = 0.0f64;
    for t in seq {
        end += t.secs;
        let stop = (end * rate).round() as usize;
        while x.len() < stop {
            let f = t.hz.unwrap_or(0.0) + dial_hz;
            let v = match t.hz {
                Some(_) => Complex32::from_polar(t.amp, (TAU * phase) as f32),
                None => Complex32::default(),
            };
            phase = (phase + f / rate).rem_euclid(1.0);
            x.push(v);
        }
    }
    x
}

/// A generator that plays a rendered signal over and over, the dial where the stacks of the crate look.
pub struct Playback {
    x: Arc<Vec<Complex32>>,
    rate: f64,
    pos: usize,
    info: SignalInfo,
}

impl Playback {
    /// Plays `seq` at `rate`; `info` is what the signal is (its bandwidth and power while it is on).
    pub fn new(seq: &[Tone], rate: f64, info: SignalInfo) -> Self {
        Self::of(render(seq, rate, test_dial(rate)), rate, info)
    }

    /// Plays the samples `x`, rendered at `rate` with the dial of [`test_dial`].
    pub fn of(x: Vec<Complex32>, rate: f64, info: SignalInfo) -> Self {
        Playback {
            x: Arc::new(x),
            rate,
            pos: 0,
            info,
        }
    }
}

impl Generator for Playback {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(self.info)
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        test_dial(self.rate)
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        if self.x.is_empty() {
            out.iter_mut().for_each(|o| *o = Complex32::default());
            return out.len();
        }
        for o in out.iter_mut() {
            *o = self.x[self.pos];
            self.pos = (self.pos + 1) % self.x.len();
        }
        out.len()
    }
}

/// A small seeded generator for the content of the test signals.
#[derive(Clone, Debug)]
pub struct Lcg(pub u64);

impl Lcg {
    /// The next value.
    pub fn step(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }

    /// A value below `n`.
    pub fn below(&mut self, n: u32) -> u32 {
        self.step() % n.max(1)
    }
}
