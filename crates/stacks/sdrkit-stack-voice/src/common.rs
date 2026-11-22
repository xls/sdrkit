//! Parts that the single-channel stacks of this crate share: the front end at 12 kHz, the common result fields, the event
//! list and the report of a detector.

use crate::typed::{DetectorEventRow, DetectorFields, DETECTOR_EVENTS};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::{Declare, Published};
use sdrkit_core::{Clock, Complex32, Event, LockState, Results, ResultsDecl, Status, Value};
use sdrkit_graph::components::ShiftResample;
use sdrkit_graph::{GraphBuilder, Kept, RunContext, SampleGuard};
use std::fmt::Write as _;
use std::sync::Arc;

/// The rate of the channel stream.
pub const RATE: f64 = 12_000.0;
/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 250e3;
/// The radio is tuned this far above the dial.
pub const TUNE_OFFSET_HZ: f64 = 20e3;
/// The most events the list keeps.
pub const EVENT_CAP: usize = 60;
/// The longest summary, in characters.
const SUMMARY_MAX: usize = 120;

sdrkit_core::choice_enum! {
    /// How far a detector is.
    pub enum Stage {
        /// Nothing found.
        Searching = ("searching", "Searching"),
        /// A signal is there.
        Signal = ("signal", "Signal found"),
        /// Decoded or confirmed.
        Confirmed = ("confirmed", "Decoded or confirmed"),
    }
}

/// The declaration of a single-channel stack with the shared fields, `own` fields and the event list.
pub fn declaration(own: &[&'static dyn Declare]) -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(DetectorFields::SHAPE.fields)
        .with_fields(own)
        .with_table(DETECTOR_EVENTS.def())
}

/// Adds the input guard and the front end under `guard` and `front`: non-finite samples become 0, the dial goes to 0 Hz and
/// the stream to 12 kHz.
pub fn add_front(b: GraphBuilder) -> GraphBuilder {
    b.add("guard", SampleGuard::default())
        .add("front", ShiftResample::default())
        .set("front.follow", Value::Bool(true))
        .set("front.rate", Value::Hz(RATE))
        .input("guard")
        .connect("guard", "front")
}

/// Moves a test signal made at 0 Hz to where a radio that is tuned with the offset of these stacks sees the dial.
#[derive(Clone, Debug)]
pub struct Placer {
    step: f64,
    phase: f64,
}

impl Placer {
    /// A placer for samples at `rate` that moves the signal by `offset_hz`.
    pub fn new(rate: f64, offset_hz: f64) -> Placer {
        Placer {
            step: offset_hz / rate,
            phase: 0.0,
        }
    }
    /// The placer of a test signal at `rate`: the dial is [`TUNE_OFFSET_HZ`] below the centre.
    pub fn below_centre(rate: f64) -> Placer {
        Placer::new(rate, -TUNE_OFFSET_HZ)
    }
    /// Moves `x` in place.
    pub fn apply(&mut self, x: &mut [Complex32]) {
        if self.step == 0.0 {
            return;
        }
        for s in x {
            self.phase = (self.phase + self.step).rem_euclid(1.0);
            let (sin, cos) = (std::f64::consts::TAU * self.phase).sin_cos();
            *s *= Complex32::new(cos as f32, sin as f32);
        }
    }
}

/// The report of a detector: its results, its event list and the common measures.
pub struct Report {
    /// The results.
    pub p: Published,
    name: &'static str,
    live: bool,
    clock: Clock,
    input_rate: f64,
    samples: u64,
    level_sum: f64,
    level_n: u32,
    level_dbfs: Option<f64>,
    serial: u64,
    pending: Vec<Event>,
    summary: String,
    row: String,
}

impl Report {
    /// A report for the stack `name` that publishes `decl`.
    pub fn new(name: &'static str, decl: ResultsDecl) -> Report {
        let mut p = Published::from_decl(Arc::new(decl));
        p.results_mut().set_status(&Status {
            summary: format!("{name}: waiting for samples"),
            ..Status::default()
        });
        Report {
            p,
            name,
            live: false,
            clock: Clock::fixed(0.0),
            input_rate: 0.0,
            samples: 0,
            level_sum: 0.0,
            level_n: 0,
            level_dbfs: None,
            serial: 0,
            pending: Vec::new(),
            summary: String::new(),
            row: String::new(),
        }
    }

    /// Takes the clock of the host.
    pub fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = ctx.clock.clone();
    }

    /// A new input at `input_rate`: the signal time starts again.
    pub fn configure(&mut self, input_rate: f64) {
        self.input_rate = input_rate;
        self.samples = 0;
        self.level_sum = 0.0;
        self.level_n = 0;
        self.level_dbfs = None;
    }

    /// The signal time, s.
    pub fn time(&self) -> f64 {
        self.samples as f64 / RATE
    }

    /// Counts `x` and measures the level: the mean power over each second.
    pub fn take(&mut self, x: &[Complex32]) {
        for s in x {
            self.level_sum += f64::from(s.norm_sqr());
            self.level_n += 1;
            if self.level_n == RATE as u32 {
                self.level_dbfs = Some(10.0 * (self.level_sum / RATE).max(1e-30).log10());
                self.level_sum = 0.0;
                self.level_n = 0;
            }
        }
        self.samples += x.len() as u64;
    }

    /// The wall-clock time now, when the samples are live.
    pub fn wall(&self) -> Option<f64> {
        self.live.then(|| self.clock.now())
    }

    /// Records an event with `text`: a row of the list and a log line. Returns the signal time of the event, for the event
    /// that the caller makes and passes to [`Report::send`].
    pub fn note(&mut self, text: &str) -> f64 {
        let t = self.time();
        self.serial += 1;
        self.row.clear();
        let _ = write!(self.row, "{}", self.serial);
        let wall = self.wall();
        DETECTOR_EVENTS.put(
            &mut self.p,
            &self.row,
            &DetectorEventRow {
                time_utc: wall.map(Time),
                signal_time_s: Seconds(t),
                text,
            },
        );
        self.p.results_mut().log(format!("{}: {text}", self.name));
        t
    }

    /// Queues `e` for the host.
    pub fn send(&mut self, e: Event) {
        self.pending.push(e);
    }

    /// Moves the queued events to `out`.
    pub fn drain(&mut self, out: &mut Vec<Event>) {
        out.append(&mut self.pending);
    }

    /// Empties the event list.
    pub fn clear(&mut self) {
        self.p.clear_rows(&DETECTOR_EVENTS);
        self.pending.clear();
    }

    /// Forgets the level of the signal before a change of frequency, and the event list if `kept` does not hold it.
    /// The signal time and the input rate stay: they belong to the stream, not to the frequency.
    pub fn forget(&mut self, kept: &Kept) {
        self.level_sum = 0.0;
        self.level_n = 0;
        self.level_dbfs = None;
        if !kept.table(DETECTOR_EVENTS.key) {
            self.clear();
        }
    }

    /// Publishes the common fields and the status. `text` is what was found.
    pub fn publish(
        &mut self,
        stage: Stage,
        text: &str,
        quality_db: Option<f64>,
        good: u64,
        bad: u64,
    ) {
        let p = &mut self.p;
        p.put(&DetectorFields {
            state: stage,
            level_dbfs: self.level_dbfs,
            quality_db: quality_db.filter(|q| q.is_finite()),
            signal_time_s: Seconds(self.samples as f64 / RATE),
            input_rate_hz: Hertz(self.input_rate),
            good: good as i64,
            bad: bad as i64,
        });
        self.summary.clear();
        let _ = write!(self.summary, "{}: {text}", self.name);
        if let Some((i, _)) = self.summary.char_indices().nth(SUMMARY_MAX) {
            self.summary.truncate(i);
        }
        let lock_state = match stage {
            _ if self.samples == 0 => LockState::Off,
            Stage::Searching => LockState::Searching,
            Stage::Signal => LockState::Tracking,
            Stage::Confirmed => LockState::Locked,
        };
        p.results_mut().set_status(&Status {
            lock_state,
            level_dbfs: self.level_dbfs.map(|l| l as f32),
            summary: self.summary.clone(),
            ..Status::default()
        });
    }

    /// The results.
    pub fn results(&self) -> &Results {
        self.p.results()
    }
}
