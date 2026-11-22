//! The sender that the frame stacks of the crate share (`<stack>.tx`). The command `send` of a stack builds one frame as
//! complex baseband at the session rate with the frame builder and the modulator of the stack's test signal; the sender
//! moves it to the offset of its channel from the centre of the samples and writes it into the transmit sink as one
//! burst, with a raised-cosine rise and fall of the length the stack states. Frames sent while a burst is on air wait and
//! go out one burst each, in order. `halt` drops the waiting frames and ends the burst on air with its fall.
//!
//! Input `in` is the session's samples: their rate, centre and dial. Output `out` is complex baseband at the session rate
//! for `tx.sink`. Output `events` carries the transmit events of the stack. Each burst ends with an end mark: at its end,
//! after `halt`, the transmitter going off and a new input format. The engine's transmit setting is the only switch.

use crate::typed::{IsmTxEvent, IsmTxFields, TX_HALT};
use sdrkit_components::tx::{end_mark, BurstOut, BurstTiming, Ramp};
use sdrkit_core::command::{Command, CommandError, CommandId, Commands};
use sdrkit_core::interface::{EventKind, Fields as _};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::{DeclareSetting, Schema};
use sdrkit_core::{
    Complex32, ConfigError, Event, Record, Results, Setting, SettingError, SettingId, StageError,
    Value, Values,
};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RunContext};
use std::collections::VecDeque;
use std::sync::Arc;

/// The section of the transmit settings.
pub const TX: &str = "Transmit";
/// The most frames that wait for the transmitter.
pub const MAX_WAITING: usize = 16;
/// The samples shaped at one time.
const CHUNK: usize = 1024;

/// The peak of the signal sent.
pub const TX_LEVEL: Setting<field::F64> = Setting::new("tx_level", "Level", "of full scale", 0.5)
    .range(0.05, 0.9, 0.0)
    .help("The magnitude of the signal sent, of full scale.")
    .section(TX);

/// Where a frame is made: the session rate, the centre of the samples and the tuned frequency.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Place {
    /// The sample rate, Hz.
    pub rate: f64,
    /// The centre of the samples, Hz.
    pub centre_hz: f64,
    /// The tuned frequency, Hz.
    pub tuned_hz: f64,
}

/// One frame to send.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// The samples at the session rate around 0 Hz, peak magnitude 1.
    pub samples: Vec<Complex32>,
    /// The frequency of the channel from the centre of the samples, Hz.
    pub offset_hz: f64,
    /// The rise and the fall of the carrier, seconds.
    pub ramp_s: f64,
    /// The addressee or the identity of the frame, for the events.
    pub target: String,
    /// What the frame carries, in one line, for the events.
    pub text: String,
}

/// Why a frame is not sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused(pub String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

/// The command error of a refusal with the reason `why`.
pub fn refused(why: impl Into<String>) -> CommandError {
    CommandError::Failed {
        source: StageError::new(Refused(why.into())),
    }
}

/// The bytes of hexadecimal text with spaces allowed, or a refusal that names the argument `what`.
///
/// # Errors
///
/// A refusal for text that is not pairs of hex digits.
pub fn hex_arg(what: &str, text: &str) -> Result<Vec<u8>, CommandError> {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    crate::family::from_hex(&t).ok_or_else(|| refused(format!("{what} is not pairs of hex digits")))
}

/// What a stack gives the sender: its `send` command, its event and how the arguments make a frame.
pub trait FrameSource: Default + Send + 'static {
    /// The component name, `<stack>.tx`.
    const NAME: &'static str;
    /// The `send` command of the stack.
    const SEND: &'static Command;
    /// The event kind of the transmissions.
    fn event() -> &'static EventKind<IsmTxEvent<'static>>;
    /// The settings of the stack that the frames read, besides [`TX_LEVEL`].
    fn settings() -> &'static [&'static dyn DeclareSetting] {
        &[]
    }
    /// Takes the setting `key` (id `id` in the sender's schema).
    ///
    /// # Errors
    ///
    /// [`SettingError`] for a value the stack refuses or a setting it does not have.
    fn apply(&mut self, key: &str, id: SettingId, v: &Value) -> Result<(), SettingError> {
        let _ = (key, v);
        Err(SettingError::NoSuchId { id })
    }
    /// The value of the setting `key`.
    fn value(&self, key: &str) -> Option<Value> {
        let _ = key;
        None
    }
    /// The frame of the arguments `args` of `send` at `at`.
    ///
    /// # Errors
    ///
    /// The errors of the arguments, and a [`refused`] error for a frame that cannot be sent.
    fn frame(&self, args: &Values, at: &Place) -> Result<Frame, CommandError>;
}

/// A frame waiting: its samples at their offset, its ramp and its target.
struct Waiting {
    samples: Vec<Complex32>,
    ramp_s: f64,
    target: String,
}

/// The burst on air.
struct Burst {
    samples: Vec<Complex32>,
    made: usize,
    ramp: Ramp,
    halt: bool,
    out: BurstOut,
    target: String,
}

impl Burst {
    /// Shapes samples until `room` output samples wait, or the rest of the frame with its fall.
    fn fill(&mut self, room: usize, level: f32, chunk: &mut Vec<Complex32>) {
        let total = self.samples.len();
        let fall = self.ramp.samples();
        while self.out.wants(room) {
            chunk.clear();
            let end = (self.made + CHUNK).min(total);
            let mut ended = false;
            for n in self.made..end {
                if !self.ramp.falling() && (n + fall >= total || self.halt) {
                    self.ramp.fall();
                }
                match self.ramp.gain(level) {
                    Some(g) => chunk.push(self.samples[n] * g),
                    None => {
                        ended = true;
                        break;
                    }
                }
            }
            self.made += chunk.len();
            self.out.push(chunk);
            if ended || self.made >= total {
                self.out.finish(chunk);
                break;
            }
        }
    }
}

/// The sender of the stack of `S`. See the module.
pub struct FrameSender<S: FrameSource> {
    source: S,
    schema: Arc<Schema>,
    commands: Commands,
    level: f64,
    place: Place,
    transmit: bool,
    timing: BurstTiming,
    queue: VecDeque<Waiting>,
    burst: Option<Burst>,
    end_owed: bool,
    sent: u64,
    events: Vec<Event>,
    results: Published,
    shown: Option<(bool, u64, usize)>,
    chunk: Vec<Complex32>,
}

impl<S: FrameSource> Default for FrameSender<S> {
    fn default() -> Self {
        let mut list: Vec<&'static dyn DeclareSetting> = S::settings().to_vec();
        list.push(&TX_LEVEL);
        let mut s = FrameSender {
            source: S::default(),
            schema: Arc::new(Schema::of(&list)),
            commands: Commands::of(&[S::SEND, &TX_HALT]),
            level: TX_LEVEL.default,
            place: Place::default(),
            transmit: false,
            timing: BurstTiming::default(),
            queue: VecDeque::new(),
            burst: None,
            end_owed: false,
            sent: 0,
            events: Vec::new(),
            results: Published::new(IsmTxFields::SHAPE.fields),
            shown: None,
            chunk: Vec::new(),
        };
        s.publish();
        s
    }
}

impl<S: FrameSource> FrameSender<S> {
    fn event(&mut self, state: &str, reason: &str, target: &str, text: &str) {
        self.events.push(S::event().event(
            0.0,
            self.place.tuned_hz,
            &IsmTxEvent {
                state,
                reason,
                target,
                text,
                family: Family::Sensor,
            },
        ));
    }

    /// Drops the waiting frames and the burst on air; its end mark goes out on the next pass when samples of it were
    /// written.
    fn cut(&mut self, reason: &str) {
        self.queue.clear();
        if let Some(b) = self.burst.take() {
            self.end_owed |= b.out.sent() > 0;
            self.event("finished", reason, &b.target, "");
        }
    }

    /// Starts the burst of the next waiting frame.
    fn start(&mut self) {
        let Some(w) = self.queue.pop_front() else {
            return;
        };
        let rate = self.place.rate;
        match BurstOut::new(rate, rate, 1.0) {
            Ok(out) => {
                self.burst = Some(Burst {
                    samples: w.samples,
                    made: 0,
                    ramp: Ramp::of_seconds(w.ramp_s, rate),
                    halt: false,
                    out,
                    target: w.target,
                });
            }
            Err(e) => self.event("refused", &e.to_string(), &w.target, ""),
        }
    }

    /// Makes the frame of `args` and queues it; starts a burst when none is on air. True when it started one.
    fn send(&mut self, args: &Values) -> Result<(bool, String, String), CommandError> {
        if !self.transmit {
            return Err(refused("the transmitter is off"));
        }
        if self.place.rate <= 0.0 {
            return Err(refused("the stack has no input yet"));
        }
        if self.queue.len() >= MAX_WAITING {
            return Err(refused(format!("{MAX_WAITING} frames wait already")));
        }
        let f = self.source.frame(args, &self.place)?;
        if f.samples.is_empty() {
            return Err(refused("the frame has no samples at this rate"));
        }
        let mut samples = f.samples;
        if f.offset_hz != 0.0 {
            let step = std::f64::consts::TAU * f.offset_hz / self.place.rate;
            for (n, z) in samples.iter_mut().enumerate() {
                let (s, c) = (step * n as f64).sin_cos();
                *z *= Complex32::new(c as f32, s as f32);
            }
        }
        self.queue.push_back(Waiting {
            samples,
            ramp_s: f.ramp_s,
            target: f.target.clone(),
        });
        let started = self.burst.is_none();
        if started {
            self.start();
        }
        Ok((started, f.target, f.text))
    }

    /// Drops the waiting frames and ends the burst on air with its fall.
    fn halt(&mut self) {
        self.queue.clear();
        let Some(b) = &mut self.burst else {
            return;
        };
        if b.made == 0 {
            let b = self.burst.take().expect("a burst");
            self.event("finished", "halted", &b.target, "");
            return;
        }
        b.halt = true;
    }

    /// Writes the result fields when they changed.
    fn publish(&mut self) {
        let now = (self.burst.is_some(), self.sent, self.queue.len());
        if self.shown == Some(now) {
            return;
        }
        self.shown = Some(now);
        self.results.put(&IsmTxFields {
            tx_sending: now.0,
            tx_sent: now.1 as i64,
            tx_waiting: now.2 as i64,
        });
    }

    /// One pass: the samples of the burst, the end marks and the next burst.
    fn run(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        if std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
        }
        if self.burst.is_none() && !self.queue.is_empty() && self.transmit {
            self.start();
        }
        if let Some(b) = &mut self.burst {
            b.fill(room, self.level as f32, &mut self.chunk);
            if b.out.write(io, 0, room, &self.timing) {
                let reason = if b.halt { "halted" } else { "" };
                let target = std::mem::take(&mut b.target);
                self.burst = None;
                self.sent += 1;
                self.event("finished", reason, &target, "");
            }
        }
        self.publish();
        io.messages_out(1).append(&mut self.events);
    }
}

impl<S: FrameSource> Component for FrameSender<S> {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            S::NAME,
            vec![Port::iq("in")],
            vec![
                Port::iq("out"),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(self.schema.clone())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let tuned_hz = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        self.place = Place {
            rate: f.rate,
            centre_hz: f.centre_hz,
            tuned_hz,
        };
        if self.burst.is_some() || !self.queue.is_empty() {
            self.cut("the format changed");
        }
        Ok(vec![
            Format::iq(f.rate, f.centre_hz, tuned_hz),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.timing.attach(ctx);
        if !ctx.transmit && (self.burst.is_some() || !self.queue.is_empty()) {
            self.cut("the transmitter is off");
        }
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io);
    }
    fn finish(&mut self, io: &mut Io) {
        self.run(io);
        if let Some(b) = self.burst.take() {
            self.end_owed |= b.out.sent() > 0;
            self.event("finished", "the input ended", &b.target, "");
        }
        if std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
        }
        self.publish();
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let key = self
            .schema
            .setting(id)
            .map(|d| d.field.key.to_string())
            .ok_or(SettingError::NoSuchId { id })?;
        if key == TX_LEVEL.key() {
            self.level = TX_LEVEL.read(id, v)?;
        } else {
            self.source.apply(&key, id, v)?;
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let mut out = Values::defaults(&self.schema);
        for (i, d) in self.schema.settings().iter().enumerate() {
            let v = if d.field.key == TX_LEVEL.key() {
                Some(Value::F64(self.level))
            } else {
                self.source.value(&d.field.key)
            };
            if let Some(v) = v {
                // The value is one the schema declared, so it is of the right kind.
                let _ = out.set(SettingId(i as u16), &v);
            }
        }
        out
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        &self.commands
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let r = match id.index() {
            0 => match self.send(args) {
                Ok((started, target, text)) => {
                    let state = if started { "started" } else { "queued" };
                    self.event(state, "", &target, &text);
                    Ok(())
                }
                Err(e) => {
                    self.event("refused", &e.to_string(), "", "");
                    Err(e)
                }
            },
            1 => {
                self.halt();
                Ok(())
            }
            _ => Err(CommandError::NoSuchId { id }),
        };
        self.publish();
        r.map(|()| None)
    }
}
