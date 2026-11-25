//! A session: one receiver (a stack's graph or a graph of the host) on one feed, run on its own thread. The thread
//! publishes complete copies of the receiver, of the components of its graph and of its run state
//! ([`crate::published`]). The host reads its copies: [`SessionControl`] for the session, [`Proxy`] for the receiver and
//! its components. A read never waits for the thread, and the thread never waits for a read. What the host writes is
//! queued for the thread, which carries it out between two blocks. The thread writes the session's streams for the host.

use crate::feed::{lock, Block, Demand, Feed};
use crate::mix::{Gains, MixIn, Mixer};
use crate::published::{Mirror, Publisher, Snapshot};
use crate::stages::Placement;
use crate::streams::{Readers, Streams};
use crate::transmit::{TxReport, TxSide, TX_FAILED, TX_LAST, TX_SENT};
use sdrkit_core::publish::{field, keys};
use sdrkit_core::{CommandError, CommandId, ConfigError};
use sdrkit_core::{
    Commands, Elementary, Event, EventCatalog, Field, HostRequest, InputInfo, Operator, Output,
    Owner, Packet, Receiver, Record, Results, ResultsDecl, Schema, Setting, SettingError,
    SettingId, SettingSet, StackDescriptor, StreamKind, TableOperator, Value, Values,
};
use sdrkit_graph::components::{SpectrumScale, SpectrumSize, SpectrumWindow};
use sdrkit_graph::{FrameRule, SizeRule, SpectrumSpec};
use sdrkit_svc_record::{transport_packets, Feed as RecordFeed};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver as Inbound, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

sdrkit_core::choice_enum! {
    /// Where a session is.
    pub enum SessionState {
        /// It waits for its first samples.
        Waiting = ("waiting", "Waiting"),
        /// Its receiver runs.
        Running = ("running", "Running"),
        /// Its receiver refused the input. A new tuned frequency or rate tries again.
        Failed = ("failed", "Failed"),
        /// It was stopped.
        Stopped = ("stopped", "Stopped"),
    }
}

/// The frequency that the session listens to, inside the samples of its input. 0 follows the centre of the input.
pub const TUNED: Setting<field::Hz> = Setting::new("tuned_hz", "Frequency", "Hz", 0.0)
    .range(0.0, 1e12, 1.0)
    .zero_means("The centre of the input")
    .help("The frequency that the session listens to. A radio moves its centre when the frequency is outside its samples. The session stays the same and keeps running. From the change until the receiver has taken the frequency, the session shows no result of its stack. Then it shows what the receiver holds: a stack clears the results that belong to one frequency, and a result that it keeps is shown again. The result session.retunes counts the changes that the receiver has taken.");
/// The session requests a spectrum from its input.
pub const SPECTRUM: Setting<field::Bool> = Setting::new("spectrum", "Spectrum", "", false);
/// The bins of the spectrum.
pub const SPECTRUM_SIZE: Setting<field::Choice<SpectrumSize>> =
    Setting::choice("spectrum_size", "Spectrum size", SpectrumSize::S1024);
/// The window of the spectrum.
pub const SPECTRUM_WINDOW: Setting<field::Choice<SpectrumWindow>> = Setting::choice(
    "spectrum_window",
    "Spectrum window",
    SpectrumWindow::BlackmanHarris,
);
/// The spectrum frames per second.
pub const SPECTRUM_FPS: Setting<field::F64> =
    Setting::new("spectrum_fps", "Spectrum frames", "1/s", 10.0).range(1.0, 60.0, 0.0);
/// The factor on the session's audio.
pub const VOLUME: Setting<field::F64> =
    Setting::new("volume", "Volume", "", 1.0).range(0.0, 4.0, 0.0);
/// The position of the session's audio in the bank's mix: -1 left, 0 centre, 1 right.
pub const PAN: Setting<field::F64> = Setting::new("pan", "Pan", "", 0.0).range(-1.0, 1.0, 0.0);
/// The session's audio is silent.
pub const MUTE: Setting<field::Bool> = Setting::new("mute", "Mute", "", false);
/// Only the solo sessions sound in the bank's mix.
pub const SOLO: Setting<field::Bool> = Setting::new("solo", "Solo", "", false);

sdrkit_core::settings! {
    /// The settings of every session. The settings of its receiver follow them.
    pub enum SessionSetting {
        /// [`TUNED`].
        Tuned = TUNED,
        /// [`SPECTRUM`].
        Spectrum = SPECTRUM,
        /// [`SPECTRUM_SIZE`].
        SpectrumSize = SPECTRUM_SIZE,
        /// [`SPECTRUM_WINDOW`].
        SpectrumWindow = SPECTRUM_WINDOW,
        /// [`SPECTRUM_FPS`].
        SpectrumFps = SPECTRUM_FPS,
        /// [`VOLUME`].
        Volume = VOLUME,
        /// [`PAN`].
        Pan = PAN,
        /// [`MUTE`].
        Mute = MUTE,
        /// [`SOLO`].
        Solo = SOLO,
    }
}

/// Where the session is.
pub const STATE: Field<field::Choice<SessionState>> = Field::new("session.state", "Session", "");
/// Samples the receiver was fed.
pub const SAMPLES: Field<field::I64> = Field::new("session.samples", "Samples", "");
/// Samples the session lost because it was behind its input.
pub const DROPPED: Field<field::I64> = Field::new("session.dropped", "Dropped", "");
/// Events whose kind the engine's catalog does not list. They are dropped.
pub const UNLISTED: Field<field::I64> =
    Field::new("session.events_unlisted", "Unlisted events", "");
/// Why the receiver refused its input, or the last setting it refused.
pub const PROBLEM: Field<field::Text> = Field::new("session.problem", "Problem", "");
/// Why the engine refused the last request of the session's receiver.
pub const REQUEST_PROBLEM: Field<field::Text> =
    Field::new("session.request_problem", "Refused request", "");
/// Audio samples lost because the host did not take them in time.
pub const AUDIO_LOST: Field<field::I64> = Field::new("session.audio_lost", "Audio lost", "");
/// Media packets and pictures lost because the host did not take them in time.
pub const MEDIA_LOST: Field<field::I64> = Field::new("session.media_lost", "Media lost", "");
/// Samples of the audio that the host gave to the session ([`Engine::write_audio`](crate::Engine::write_audio)) that
/// the graph did not take in time.
pub const AUDIO_IN_LOST: Field<field::I64> =
    Field::new("session.audio_in_lost", "Host audio lost", "");
/// The rate of the audio that the graph of the session takes from the host without conversion. It has a value while
/// the graph has a host audio input (`host.in`) and the session can transmit (its radio can, or its transmit samples
/// go to the host): the host gives audio with [`Engine::write_audio`](crate::Engine::write_audio) while
/// [`AUDIO_IN_WANTED`] is true.
pub const AUDIO_IN_RATE: Field<field::Hz> =
    Field::new("session.audio_in_rate_hz", "Host audio rate", "Hz");
/// True while a transmission that sends the host audio runs, or a command that starts one (a command that transmits
/// speech, such as push-to-talk) was accepted: the host opens its microphone only then. False while
/// [`AUDIO_IN_RATE`] has no value.
pub const AUDIO_IN_WANTED: Field<field::Bool> =
    Field::new("session.audio_in_wanted", "Host audio wanted", "");
/// Transmit samples lost because the host did not take them in time.
pub const TRANSMIT_LOST: Field<field::I64> =
    Field::new("session.transmit_lost", "Transmit samples lost", "");

/// The number of changes of frequency that the receiver has taken: those of the host, of a service and of the
/// receiver itself.
pub const RETUNES: Field<field::I64> = Field::new("session.retunes", "Retunes", "");

/// The signal to noise ratio that the receiver measures, from the status of its results.
pub const SNR: Field<field::F64> = Field::new("session.snr_db", "SNR", "dB");
/// The level that the receiver measures, from the status of its results.
pub const LEVEL: Field<field::F64> = Field::new("session.level_dbfs", "Level", "dBFS");
/// The offset of the signal from the tuned frequency that the receiver measures, from the status of its results.
pub const OFFSET: Field<field::Hz> = Field::new("session.offset_hz", "Offset", "Hz");

/// The share of one core that the stack of the session takes now, all its threads together.
pub const SHARE: Field<field::F64> = Field::new("session.share", "Processor share", "");
/// The cores that the stack needs at its rate, measured over the last second.
pub const CORES_NEEDED: Field<field::F64> = Field::new("session.cores_needed", "Cores needed", "");
/// The cores that the stack declared before the session started. Absent for a stack with no recorded cost.
pub const CORES_DECLARED: Field<field::F64> =
    Field::new("session.cores_declared", "Cores declared", "");
/// The cores that were free for the session when it started.
pub const CORES_FREE: Field<field::F64> = Field::new("session.cores_free", "Cores free", "");
/// The threads that run the stack of the session.
pub const THREADS: Field<field::I64> = Field::new("session.threads", "Threads", "");
/// The times that a stage of the stack found the queue after it full and waited.
pub const STAGE_BEHIND: Field<field::I64> = Field::new("session.stage_behind", "Stage behind", "");
/// The places where samples are missing that the stack was told of.
pub const GAPS: Field<field::I64> = Field::new("session.gaps", "Gaps", "");
/// Why the session may lose samples: its stack needs more cores than are free.
pub const LOAD_PROBLEM: Field<field::Text> = Field::new("session.load_problem", "Load", "");

/// Columns of the load tables.
pub const LOAD_SECTION: Field<field::Text> = Field::new("section", "Section", "");
/// The stage of a component.
pub const LOAD_STAGE: Field<field::Text> = Field::new("stage", "Stage", "");
/// The thread of a component or a stage: 0 is the session thread.
pub const LOAD_THREAD: Field<field::I64> = Field::new("thread", "Thread", "");
/// The share of one core over the last second.
pub const LOAD_SHARE: Field<field::F64> = Field::new("share", "Share", "");
/// The share of one core since the session was configured.
pub const LOAD_SHARE_TOTAL: Field<field::F64> = Field::new("share_total", "Share since start", "");
/// The calls of a component or a section.
pub const LOAD_CALLS: Field<field::I64> = Field::new("calls", "Calls", "");
/// The items that a component read.
pub const LOAD_ITEMS_IN: Field<field::I64> = Field::new("items_in", "Items in", "");
/// The items that a component wrote.
pub const LOAD_ITEMS_OUT: Field<field::I64> = Field::new("items_out", "Items out", "");
/// The mean time of one call.
pub const LOAD_MEAN_US: Field<field::F64> = Field::new("mean_us", "Mean call", "us");
/// The longest call.
pub const LOAD_WORST_US: Field<field::F64> = Field::new("worst_us", "Worst call", "us");
/// The stage has a thread of its own now.
pub const LOAD_CUT: Field<field::Bool> = Field::new("cut", "Own thread", "");
/// The cores that a stage needs at the rate of the stack, over the last second.
pub const LOAD_COST: Field<field::F64> = Field::new("cost", "Cost", "");
/// The recorded cost of a stage.
pub const LOAD_DECLARED: Field<field::F64> = Field::new("declared", "Declared cost", "");
/// The cost in calibration units per second of signal: the same number on every device.
pub const LOAD_UNITS: Field<field::F64> = Field::new("units", "Units", "");
/// The recorded cost of a stage in calibration units per second of signal.
pub const LOAD_DECLARED_UNITS: Field<field::F64> =
    Field::new("declared_units", "Declared units", "");
/// The calibration units per second that one fast core of this device runs.
pub const CORE_UNITS: Field<field::F64> = Field::new("session.core_units", "Units per core", "");
/// The fill of the queue before a stage, from 0 to 1.
pub const LOAD_QUEUE_FILL: Field<field::F64> = Field::new("queue_fill", "Queue fill", "");
/// The units that wait before a stage, and the most that waited.
pub const LOAD_QUEUE_UNITS: Field<field::I64> = Field::new("queue_units", "Queue units", "");
/// The most units that waited before a stage.
pub const LOAD_QUEUE_MAX: Field<field::I64> = Field::new("queue_units_max", "Queue most", "");
/// The times that the stage before found the queue full.
pub const LOAD_BEHIND: Field<field::I64> = Field::new("behind", "Behind", "");

/// Columns of the table of connections.
pub const CONN_FROM: Field<field::Text> = Field::new("from", "From", "");
pub const CONN_OUTPUT: Field<field::Text> = Field::new("output", "Output", "");
pub const CONN_TO: Field<field::Text> = Field::new("to", "To", "");
pub const CONN_INPUT: Field<field::Text> = Field::new("input", "Input", "");
/// The stream format.
pub const CONN_FORMAT: Field<field::Text> = Field::new("format", "Format", "");
pub const CONN_ITEMS: Field<field::F64> = Field::new("items_per_s", "Items", "1/s");
pub const CONN_BYTES: Field<field::F64> = Field::new("bytes_per_s", "Bytes", "B/s");
/// The bytes that the buffer or queue holds now, on the mean, and at most since the start.
pub const CONN_HELD: Field<field::I64> = Field::new("held_bytes", "Held", "B");
pub const CONN_HELD_MEAN: Field<field::F64> = Field::new("held_mean_bytes", "Held mean", "B");
pub const CONN_HELD_MAX: Field<field::I64> = Field::new("held_max_bytes", "Held most", "B");
/// The bound of the buffer or queue. Absent when it has no fixed bound.
pub const CONN_CAPACITY: Field<field::I64> = Field::new("capacity_bytes", "Capacity", "B");
/// The bytes held now as a share of the bound, from 0 to 1.
pub const CONN_FILL: Field<field::F64> = Field::new("fill", "Fill", "");
/// The sampled time that an item waits from the end of its writer's call to the start of its reader's call.
pub const CONN_WAIT_MEAN: Field<field::F64> = Field::new("wait_mean_us", "Wait", "us");
pub const CONN_WAIT_WORST: Field<field::F64> = Field::new("wait_worst_us", "Wait worst", "us");
/// The times the writer found the buffer or queue full, and the reader found it empty.
pub const CONN_FULL: Field<field::I64> = Field::new("full", "Found full", "");
pub const CONN_EMPTY: Field<field::I64> = Field::new("empty", "Found empty", "");
/// The connection joins two stages, and the two run on different threads now.
pub const CONN_BOUNDARY: Field<field::Bool> = Field::new("boundary", "Stage boundary", "");
pub const CONN_CUT: Field<field::Bool> = Field::new("cut", "Cut", "");
/// Input samples that the session lost at this place.
pub const CONN_LOST: Field<field::I64> = Field::new("lost", "Lost", "");

/// The connections of a session: first its queue from the input to the stack, then each connection from an output port
/// of a component to an input port of the next, in the run order of the readers. The row key is
/// `<from>.<output>><to>.<input>`, and `queue` for the queue.
pub static CONNECTIONS: sdrkit_core::TableDef = sdrkit_core::TableDef {
    key: "session.connections",
    label: "Connections",
    columns: &[
        &CONN_FROM,
        &CONN_OUTPUT,
        &CONN_TO,
        &CONN_INPUT,
        &CONN_FORMAT,
        &LOAD_STAGE,
        &CONN_ITEMS,
        &CONN_BYTES,
        &CONN_HELD,
        &CONN_HELD_MEAN,
        &CONN_HELD_MAX,
        &CONN_CAPACITY,
        &CONN_FILL,
        &CONN_WAIT_MEAN,
        &CONN_WAIT_WORST,
        &CONN_FULL,
        &CONN_EMPTY,
        &CONN_BOUNDARY,
        &CONN_CUT,
        &CONN_LOST,
    ],
    cap: 1024,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The components of the stack of a session in run order, each followed by its sections. The row key is the id of the
/// component, or `<component>/<section>`.
pub static COMPONENTS: sdrkit_core::TableDef = sdrkit_core::TableDef {
    key: "session.components",
    label: "Components",
    columns: &[
        &LOAD_SECTION,
        &LOAD_STAGE,
        &LOAD_THREAD,
        &LOAD_SHARE,
        &LOAD_SHARE_TOTAL,
        &LOAD_CALLS,
        &LOAD_ITEMS_IN,
        &LOAD_ITEMS_OUT,
        &LOAD_MEAN_US,
        &LOAD_WORST_US,
        &LOAD_UNITS,
    ],
    cap: 1024,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The stages of the stack of a session in run order. The row key is the name of the stage.
pub static STAGES: sdrkit_core::TableDef = sdrkit_core::TableDef {
    key: "session.stages",
    label: "Stages",
    columns: &[
        &LOAD_THREAD,
        &LOAD_CUT,
        &LOAD_SHARE,
        &LOAD_COST,
        &LOAD_DECLARED,
        &LOAD_QUEUE_FILL,
        &LOAD_QUEUE_UNITS,
        &LOAD_QUEUE_MAX,
        &LOAD_BEHIND,
        &LOAD_UNITS,
        &LOAD_DECLARED_UNITS,
    ],
    cap: 64,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The result fields of every session, after those of its receiver.
pub(crate) const OWN_FIELDS: [&dyn sdrkit_core::Declare; 29] = [
    &STATE,
    &SAMPLES,
    &DROPPED,
    &UNLISTED,
    &PROBLEM,
    &REQUEST_PROBLEM,
    &AUDIO_LOST,
    &TRANSMIT_LOST,
    &RETUNES,
    &TX_SENT,
    &TX_FAILED,
    &TX_LAST,
    &crate::transmit::TX_BLOCKED,
    &SNR,
    &LEVEL,
    &OFFSET,
    &SHARE,
    &CORES_NEEDED,
    &CORES_DECLARED,
    &CORES_FREE,
    &THREADS,
    &STAGE_BEHIND,
    &GAPS,
    &LOAD_PROBLEM,
    &CORE_UNITS,
    &MEDIA_LOST,
    &AUDIO_IN_LOST,
    &AUDIO_IN_RATE,
    &AUDIO_IN_WANTED,
];

/// The events a session keeps for the host. The oldest go first.
const EVENT_CAP: usize = 1024;
/// The fewest blocks that the queue of a session holds: the samples of the smallest ring of a radio. A feed of a radio
/// with a longer ring has a longer queue ([`crate::feed::Feed::queue`]).
pub(crate) const QUEUE: usize = 128;
/// The longest time between two copies of the results.
const PUBLISH: Duration = Duration::from_millis(100);
/// The results that changed are copied for the host at most this often.
const PUBLISH_MIN: Duration = Duration::from_millis(15);
/// The time between two passes of a session that its graph drives from inside.
const PACE: Duration = Duration::from_millis(20);
/// The media packets that wait on the session thread while the host reads the queue.
const PENDING: usize = 64;

/// Who started a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum By {
    /// The host.
    Host,
    /// The engine, for a service or a request. A saved state leaves it out.
    Engine,
    /// The engine, for a share. The session takes the samples as the radio gives them and never steers the radio. A
    /// saved state leaves it out.
    Share,
}

/// What the session reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    /// An open radio.
    Radio(crate::RadioId),
    /// A host feed: the samples that the host pushes with [`Engine::push`](crate::Engine::push).
    Feed(crate::FeedId),
    /// No samples: a component of the graph makes the signal (a stack's test signal, `gen.stack`). The session runs a pass
    /// of its graph every 20 ms.
    Inside,
}

enum Control {
    Set(SettingId, Value),
    /// The frequency, the number of this retune of the session, and true if the host ordered it: the receiver then
    /// forgets the results of the frequency before.
    Tune(f64, u64, bool),
    /// A receiver in place of the one that runs, the tune offset of its stack on the input of the session, and the
    /// number of the receiver.
    Replace(Box<dyn Receiver>, f64, u64),
    /// Work of a thread of the host on the run state. The host waits for it.
    Call(Arc<Mutex<Option<Job>>>, Ack),
    /// A recording of the record service, with the id of the stack that the session runs.
    Record(RecordFeed, &'static str),
    /// The channel sessions whose audio the session mixes into its own.
    Mix(Vec<MixIn>),
}

/// Work on the run state of a session, queued by a thread of the host.
type Job = Box<dyn FnOnce(&mut Run) + Send>;

/// Tells the thread of the host that waits for a [`Control::Call`] to go on, when dropped.
struct Ack(Arc<(Mutex<bool>, Condvar)>);

impl Drop for Ack {
    fn drop(&mut self) {
        *lock(&self.0 .0) = true;
        self.0 .1.notify_all();
    }
}

/// A recording that the session thread feeds.
struct Recording {
    feed: RecordFeed,
    stack: &'static str,
    /// The rate and centre of the samples it was given last.
    input: (f64, f64),
}

impl Recording {
    fn ended(&self) -> bool {
        match &self.feed {
            RecordFeed::Audio(f) => f.is_ended(),
            RecordFeed::Iq(f) => f.is_ended(),
            RecordFeed::Ts(f) => f.is_ended(),
            RecordFeed::TimeShift(f) => f.is_ended(),
            RecordFeed::Rewind(f) => f.is_ended(),
        }
    }

    /// Writes the block `b` and the audio of `out` to the recording. `squelch` is the receiver's
    /// [`SQUELCH_OPEN`](keys::SQUELCH_OPEN): the audio recorder's gate reads it. A transport stream recording reads the
    /// media packets of [`StreamKind::MpegTs`] in `out`.
    fn take(&mut self, b: &Block, out: &Output, squelch: bool) {
        let input = (b.rate_hz, b.centre_hz);
        let iq = b.samples();
        match &mut self.feed {
            RecordFeed::Audio(f) => {
                for p in out.audio_runs() {
                    f.push(p.samples, p.channels, p.rate_hz, squelch);
                }
            }
            RecordFeed::Iq(f) if !iq.is_empty() => {
                if self.input != input {
                    f.set_format(b.rate_hz, b.centre_hz);
                }
                f.push(iq);
            }
            RecordFeed::Rewind(f) if !iq.is_empty() => {
                if self.input.0 != b.rate_hz {
                    f.configure(b.rate_hz, b.centre_hz, self.stack);
                } else if self.input.1 != b.centre_hz {
                    f.retuned(b.centre_hz);
                }
                f.push(iq);
            }
            RecordFeed::Ts(f) => {
                for p in transport_stream(out).flat_map(transport_packets) {
                    f.push(&p);
                }
            }
            RecordFeed::TimeShift(f) => {
                for p in transport_stream(out).flat_map(transport_packets) {
                    f.push(&p);
                }
            }
            RecordFeed::Iq(_) | RecordFeed::Rewind(_) => return,
        }
        self.input = input;
    }
}

/// How long a setting that the receiver refused at the rate of its input waits for another rate, in seconds of samples.
const AT_RATE_WAIT_S: f64 = 0.5;

/// The settings that can wait for another rate at once without an allocation on the session thread.
const AT_RATE_SLOTS: usize = 8;

/// True if `e` refuses a value because of the sample rate of the input, which the engine may be about to change.
fn refused_at_rate(e: &SettingError) -> bool {
    matches!(
        e,
        SettingError::Config { source, .. } if matches!(
            **source,
            ConfigError::RateTooLow { .. }
                | ConfigError::RateTooHigh { .. }
                | ConfigError::WrongRate { .. }
                | ConfigError::RateNotMultiple { .. }
                | ConfigError::SettingAtRate { .. }
        )
    )
}

/// The media packets of [`StreamKind::MpegTs`] in `out`, in order.
fn transport_stream(out: &Output) -> impl Iterator<Item = &Packet> + '_ {
    out.media
        .iter()
        .filter(|e| e.kind == StreamKind::MpegTs)
        .map(|e| &e.packet)
}

/// The receiver of a session, which only its thread feeds.
pub(crate) struct Run {
    pub(crate) rx: Box<dyn Receiver>,
    /// The rate, centre and tuned frequency of the last configure, if it worked.
    input: Option<(f64, f64, f64)>,
    tuned_hz: f64,
    /// How far below the centre of its input the session listens while it has no frequency of its own.
    offset_hz: f64,
    failure: Option<ConfigError>,
    /// The input that the receiver refused last. It is not tried again until the input changes.
    last_failed: Option<(f64, f64, f64)>,
    /// A setting was refused on the thread, so the host's copy of the values is out of date.
    values_stale: bool,
    /// The last refusal of a setting on the thread.
    refused: Option<SettingError>,
    /// The settings that the receiver refused at the rate of its input, the newest value of each. The engine moves the
    /// rate of the radio after a setting that gives the rate, so they are given again at the next change of the rate,
    /// and refused for good after [`AT_RATE_WAIT_S`] of samples at the same rate.
    at_rate: Vec<(SettingId, Value)>,
    /// The seconds of samples fed since the first setting of `at_rate` began to wait.
    at_rate_waited: f64,
    /// Moves on when `failure` or `refused` changes.
    problem_seq: u64,
    /// The number of the last change of frequency that the receiver has taken.
    retunes: u64,
    /// The requests of the receiver that wait for the next copy to the owner.
    requests: Vec<HostRequest>,
    /// The recordings of the record service that the session feeds.
    records: Vec<Recording>,
    /// The mix of the channel sessions, for the session of the bank.
    mixer: Mixer,
    /// The transmit switch of the engine.
    transmit: Arc<AtomicBool>,
    /// Events of a kind that the catalog does not list.
    unlisted: u64,
    /// The last report of the transmit side.
    tx: Option<TxReport>,
    /// The number of the receiver. It moves on when another receiver takes its place.
    generation: u64,
    /// The text of `failure` or `refused`, made at `problem_seq` of `problem_at`.
    problem: Option<String>,
    problem_at: u64,
    /// The number of the last copy for which the session's copy of the receiver's values was out of date.
    stale_at: u64,
    publisher: Publisher,
    /// The calls of the host that were carried out since the last copy. The host goes on after the copy.
    acks: Vec<Ack>,
    /// The threads of the stack and its load.
    place: Placement,
    /// The samples that the session had lost when the last block was fed.
    dropped_seen: u64,
}

impl Run {
    /// Copies the receiver and the run state for the host and swaps the copy with the front buffer of `link`. `all`
    /// takes every part as changed. Returns false if a thread of the host reads the front buffer now and `wait` is
    /// false: the next call offers the copy again.
    fn publish(&mut self, link: &Link, all: bool, wait: bool) -> bool {
        let seq = self.publish_to(&link.front, all, wait);
        if let Some(seq) = seq {
            link.front_generation
                .store(self.generation, Ordering::Release);
            link.front_seq.store(seq, Ordering::Release);
        }
        if wait {
            self.acks.clear();
        }
        seq.is_some()
    }

    fn publish_to(&mut self, front: &Mutex<Snapshot>, all: bool, wait: bool) -> Option<u64> {
        let stale = std::mem::take(&mut self.values_stale);
        if self.problem_at != self.problem_seq {
            self.problem_at = self.problem_seq;
            self.problem = match (&self.failure, &self.refused) {
                (Some(e), _) => Some(e.to_string()),
                (None, Some(e)) => Some(e.to_string()),
                (None, None) => None,
            };
        }
        let b = self.publisher.fill(self.rx.as_mut(), all || stale);
        b.generation = self.generation;
        let st = &mut b.state;
        st.failed = self.failure.is_some();
        st.has_input = self.input.is_some();
        st.problem.clone_from(&self.problem);
        st.problem_seq = self.problem_seq;
        st.retunes = self.retunes;
        st.unlisted = self.unlisted;
        if stale {
            self.stale_at = b.seq;
        }
        st.stale_at = self.stale_at;
        st.tx.clone_from(&self.tx);
        if st.load.seq != self.place.load.seq {
            st.load.copy_from(&self.place.load);
        }
        self.publisher.swap(front, wait)
    }

    /// The host reached the receiver or one of its components directly, so the session's copy of the values is read again.
    pub(crate) fn touched(&mut self) {
        self.values_stale = true;
    }

    /// Carries out the controls of `inbox`, then passes the receiver the state of the transmitter: on if the transmit
    /// switch of the engine is on and the session has a transmit stream.
    fn apply(&mut self, inbox: &mut Vec<Control>, live: bool, streams: &Arc<Streams>) {
        self.controls(inbox, live, streams);
        self.pass_transmit(streams);
    }

    fn pass_transmit(&mut self, streams: &Arc<Streams>) {
        let on = self.transmit_on(streams);
        self.rx.set_transmit(on);
    }

    /// The state of the transmitter of the session: on if the transmit switch of the engine is on and the session has a
    /// transmit stream.
    fn transmit_on(&self, streams: &Streams) -> bool {
        streams.has_transmit() && self.transmit.load(Ordering::Acquire)
    }

    fn controls(&mut self, inbox: &mut Vec<Control>, live: bool, streams: &Arc<Streams>) {
        for c in inbox.drain(..) {
            match c {
                Control::Set(id, v) => {
                    self.at_rate.retain(|(w, _)| *w != id);
                    match self.rx.apply(id, &v) {
                        Ok(()) => self.taken(),
                        Err(e) if self.input.is_some() && refused_at_rate(&e) => {
                            if self.at_rate.is_empty() {
                                self.at_rate_waited = 0.0;
                            }
                            self.at_rate.push((id, v));
                        }
                        Err(e) => self.refuse(e),
                    }
                }
                Control::Tune(hz, number, by_host) => {
                    self.tuned_hz = hz;
                    if let Some((rate, centre, _)) = self.input {
                        self.retune(rate, centre, live);
                    }
                    if by_host {
                        self.rx.forget();
                    }
                    self.retunes = number;
                }
                Control::Call(job, ack) => {
                    if let Some(job) = lock(&job).take() {
                        // The call finds the receiver as a block would.
                        self.pass_transmit(streams);
                        job(self);
                    }
                    self.acks.push(ack);
                }
                Control::Replace(mut rx, offset_hz, generation) => {
                    give_streams(rx.as_mut(), streams);
                    self.place.end(self.rx.as_mut());
                    streams.end_pictures();
                    self.rx = rx;
                    self.place.start(self.rx.as_mut());
                    self.generation = generation;
                    self.offset_hz = offset_hz;
                    self.input = None;
                    self.failure = None;
                    self.last_failed = None;
                    self.refused = None;
                    self.at_rate.clear();
                    self.values_stale = false;
                    self.problem_seq += 1;
                }
                Control::Record(feed, stack) => self.records.push(Recording {
                    feed,
                    stack,
                    input: (0.0, 0.0),
                }),
                Control::Mix(inputs) => self.mixer.set(inputs),
            }
        }
    }

    /// The frequency that the session listens to on an input with the centre `centre`.
    fn listens(&self, centre: f64) -> f64 {
        sdrkit_core::listened_hz(self.tuned_hz, centre, self.offset_hz)
    }

    fn info(&self, rate: f64, centre: f64, live: bool) -> InputInfo {
        let tuned = self.listens(centre);
        InputInfo {
            live,
            ..InputInfo::new(rate, centre, tuned)
        }
    }

    /// A setting was taken: a refusal before it is shown no more.
    fn taken(&mut self) {
        if self.refused.take().is_some() {
            self.problem_seq += 1;
        }
    }

    /// A setting was refused: the host's copy of the values is read again and the refusal is shown.
    fn refuse(&mut self, e: SettingError) {
        self.values_stale = true;
        self.refused = Some(e);
        self.problem_seq += 1;
    }

    /// Gives the receiver the settings of `at_rate` again. A refusal now is final.
    fn give_at_rate(&mut self) {
        let mut waiting = std::mem::take(&mut self.at_rate);
        for (id, v) in waiting.drain(..) {
            match self.rx.apply(id, &v) {
                Ok(()) => self.taken(),
                Err(e) => self.refuse(e),
            }
        }
        self.at_rate = waiting;
    }

    fn retune(&mut self, rate: f64, centre: f64, live: bool) {
        let info = self.info(rate, centre, live);
        match self.input {
            Some((r, _, _)) if r == rate && self.failure.is_none() => {
                if self.input != Some((rate, centre, info.tuned_hz)) {
                    self.rx.retuned(&info);
                    self.input = Some((rate, centre, info.tuned_hz));
                }
            }
            _ => match self.rx.configure(&info) {
                Ok(()) => {
                    if self.failure.take().is_some() {
                        self.problem_seq += 1;
                    }
                    self.input = Some((rate, centre, info.tuned_hz));
                    if !self.at_rate.is_empty() {
                        self.give_at_rate();
                    }
                }
                Err(e) => {
                    self.failure = Some(e);
                    self.problem_seq += 1;
                    self.input = None;
                }
            },
        }
    }

    /// Feeds the block `b`. `dropped` is the count of samples that the session has lost before its stack so far.
    fn feed(&mut self, b: &Block, live: bool, dropped: u64, out: &mut Output) {
        let wanted = (b.rate_hz, b.centre_hz, self.listens(b.centre_hz));
        if self.input != Some(wanted) {
            if self.last_failed == Some(wanted) {
                return;
            }
            // A session without a frequency of its own follows the centre of its input. A new centre is then a
            // change of frequency that the receiver did not order.
            let moved = self.tuned_hz <= 0.0
                && self
                    .input
                    .is_some_and(|(_, centre, _)| centre != b.centre_hz);
            self.retune(b.rate_hz, b.centre_hz, live);
            if moved {
                self.rx.forget();
            }
            if self.failure.is_some() {
                self.last_failed = Some(wanted);
                return;
            }
            self.last_failed = None;
        }
        let dropped = dropped.max(self.dropped_seen);
        let lost = dropped - std::mem::replace(&mut self.dropped_seen, dropped);
        // A gap that the source made on purpose (a retune, a step of a hop) is told to the stack, and is no loss.
        let gaps: u64 = b
            .gaps()
            .iter()
            .filter(|g| !g.deliberate)
            .map(|g| g.lost)
            .sum();
        self.place.lose(self.rx.as_mut(), lost, gaps);
        self.place.gaps += feed_block(self.rx.as_mut(), b, lost, out);
        if !self.at_rate.is_empty() && b.rate_hz > 0.0 {
            self.at_rate_waited += b.samples().len() as f64 / b.rate_hz;
            if self.at_rate_waited >= AT_RATE_WAIT_S {
                self.give_at_rate();
            }
        }
    }

    /// Writes the block `b` and the audio of `out` to every recording, and drops the recordings that ended.
    fn record(&mut self, b: &Block, out: &Output) {
        if self.records.is_empty() {
            return;
        }
        // A receiver without a squelch is always open.
        let squelch = self
            .rx
            .results()
            .fields()
            .get(&keys::SQUELCH_OPEN)
            .unwrap_or(true);
        for r in &mut self.records {
            r.take(b, out, squelch);
        }
        self.records.retain(|r| !r.ended());
    }
}

/// Whether the thread of a session takes the controls that are queued.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Taken {
    /// The thread has not started. A call of the host works on the run state itself.
    NotYet,
    /// The thread takes the controls between two blocks.
    ByThread,
    /// The thread has ended. A call of the host works on the run state itself.
    NoMore,
}

/// The controls that wait for the session thread.
struct Inbox {
    controls: Vec<Control>,
    taken: Taken,
}

/// The events and requests of the session thread that the control thread has not taken.
#[derive(Default)]
struct Fresh {
    events: Vec<Event>,
    requests: Vec<HostRequest>,
}

/// What the host and the session thread share.
pub(crate) struct Link {
    /// The receiver. The session thread holds it while it works on a block. A thread of the host takes it only while
    /// no session thread runs.
    run: Mutex<Run>,
    inbox: Mutex<Inbox>,
    /// The newest copy of the session for the host. Both sides hold it for a swap or a copy only.
    front: Mutex<Snapshot>,
    /// The number of the copy in `front`, and the number of its receiver.
    front_seq: AtomicU64,
    front_generation: AtomicU64,
    /// The number of the receiver that the host gave the session last.
    generation: AtomicU64,
    fresh: Mutex<Fresh>,
    samples: AtomicU64,
    pub(crate) dropped: Arc<AtomicU64>,
    /// The traffic of the queue of blocks.
    pub(crate) queue: Arc<crate::feed::QueueMeter>,
    stop: AtomicBool,
    pub(crate) demand: Arc<Demand>,
    pub(crate) feed: Arc<Feed>,
    /// The streams for the host.
    pub(crate) streams: Arc<Streams>,
    /// The volume, pan, mute and solo of the session.
    pub(crate) gains: Arc<Gains>,
    /// The engine sends the session's transmit stream to its radio, so the host does not read it.
    pub(crate) to_radio: bool,
}

impl Link {
    /// Runs `f` on the run state between two blocks of the session, after the controls that wait, and returns what it
    /// returns once the copy for the host shows its effect. The session thread is held up for `f` and that copy only.
    /// While the session thread works on a block, `f` is queued for it and the call waits for the end of that block. Else
    /// `f` runs on the calling thread. `None` if the receiver stopped on a fault while it ran `f`.
    pub(crate) fn with_run<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Run) -> R + Send + 'static,
    ) -> Option<R> {
        let out = Arc::new(Mutex::new(None));
        let o = Arc::clone(&out);
        let job: Job = Box::new(move |run| {
            let r = f(run);
            *lock(&o) = Some(r);
        });
        // The session thread waits for its next block, or does not run: the work is done here.
        let job = match self.run.try_lock() {
            Ok(run) => {
                self.do_here(run, job);
                return lock(&out).take();
            }
            Err(std::sync::TryLockError::Poisoned(p)) => {
                self.do_here(p.into_inner(), job);
                return lock(&out).take();
            }
            Err(std::sync::TryLockError::WouldBlock) => Arc::new(Mutex::new(Some(job))),
        };
        let done = Arc::new((Mutex::new(false), Condvar::new()));
        let queued = {
            let mut i = lock(&self.inbox);
            let queued = i.taken == Taken::ByThread;
            if queued {
                i.controls
                    .push(Control::Call(Arc::clone(&job), Ack(Arc::clone(&done))));
            }
            queued
        };
        if queued {
            let mut d = lock(&done.0);
            while !*d {
                d = done.1.wait(d).unwrap_or_else(|p| p.into_inner());
            }
        }
        // No thread took the work: it runs here.
        let left = lock(&job).take();
        if let Some(job) = left {
            self.do_here(lock(&self.run), job);
        }
        let r = lock(&out).take();
        r
    }

    /// Carries out the controls that wait, then `job`, on the calling thread, and copies the result for the host.
    fn do_here(&self, mut run: MutexGuard<'_, Run>, job: Job) {
        let mut rest = Vec::new();
        self.take_inbox(&mut rest);
        run.apply(&mut rest, self.feed.live(), &self.streams);
        job(&mut run);
        run.publish(self, true, true);
    }

    /// The run state, if the session thread does not work on a block now. For the control thread, which holds it for a
    /// short copy.
    pub(crate) fn try_run(&self) -> Option<MutexGuard<'_, Run>> {
        self.run.try_lock().ok()
    }

    /// The session thread takes the controls from now on.
    fn started(&self) {
        lock(&self.inbox).taken = Taken::ByThread;
    }

    /// The session thread takes no more controls. Moves those that wait into `into`.
    fn ended(&self, into: &mut Vec<Control>) {
        let mut i = lock(&self.inbox);
        i.taken = Taken::NoMore;
        into.append(&mut i.controls);
    }

    /// The session thread stopped on a fault: the calls of the host that wait go on, on their own threads.
    pub(crate) fn abandon(&self) {
        let mut rest = Vec::new();
        self.ended(&mut rest);
        drop(rest);
        lock(&self.run).acks.clear();
    }

    /// Hands a recording of the record service to the session thread. `stack` is the id of the stack the session runs.
    pub(crate) fn record(&self, feed: RecordFeed, stack: &'static str) {
        self.send(Control::Record(feed, stack));
    }

    /// Makes the session mix the audio of `inputs` into its own.
    pub(crate) fn mix(&self, inputs: Vec<MixIn>) {
        self.send(Control::Mix(inputs));
    }

    fn send(&self, c: Control) {
        lock(&self.inbox).controls.push(c);
    }

    fn take_inbox(&self, into: &mut Vec<Control>) {
        std::mem::swap(&mut lock(&self.inbox).controls, into);
    }

    /// Moves `events` and `requests` of the session thread to where the control thread takes them. The oldest events go
    /// first when more than [`EVENT_CAP`] wait. If the control thread takes them at this moment and `wait` is false,
    /// they stay for the next call.
    fn offer(&self, events: &mut Vec<Event>, requests: &mut Vec<HostRequest>, wait: bool) {
        if events.is_empty() && requests.is_empty() {
            return;
        }
        let mut f = if wait {
            lock(&self.fresh)
        } else {
            match self.fresh.try_lock() {
                Ok(f) => f,
                Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => {
                    let over = events.len().saturating_sub(EVENT_CAP);
                    events.drain(..over);
                    return;
                }
            }
        };
        let over = (f.events.len() + events.len()).saturating_sub(EVENT_CAP);
        let held = over.min(f.events.len());
        f.events.drain(..held);
        events.drain(..over - held);
        f.events.append(events);
        f.requests.append(requests);
    }

    /// Moves the events and requests that came from the session thread since the last call into `events` and `requests`.
    pub(crate) fn take_fresh(&self, events: &mut Vec<Event>, requests: &mut Vec<HostRequest>) {
        let mut f = lock(&self.fresh);
        events.append(&mut f.events);
        requests.append(&mut f.requests);
    }
}

/// The receiver of a session, or one component of its graph, for a thread of the host: the settings, values, results and
/// commands are those of the last copy. A new value and a command go to the session thread, and the call returns what the
/// receiver answered.
pub(crate) struct Proxy<'a> {
    mirror: &'a mut Mirror,
    link: &'a Link,
    /// The component. `None` for the receiver itself.
    component: Option<sdrkit_graph::ComponentHandle>,
}

impl Proxy<'_> {
    /// Runs `f` on the owner between two blocks of the session, with the state of the transmitter of the session, then
    /// reads the copy again. `None` if the owner is gone.
    fn on_thread<R: Send + 'static>(
        &mut self,
        f: impl FnOnce(&mut dyn Owner, bool) -> R + Send + 'static,
    ) -> Option<R> {
        let component = self.component;
        let streams = Arc::clone(&self.link.streams);
        let r = self
            .link
            .with_run(move |run| {
                let on = run.transmit_on(&streams);
                let r = match component {
                    None => f(run.rx.as_mut(), on),
                    Some(h) => {
                        let graph = run.rx.as_any_mut()?.downcast_mut::<sdrkit_graph::Graph>()?;
                        f(&mut graph.owner(h)?, on)
                    }
                };
                run.touched();
                Some(r)
            })
            .flatten();
        let front = lock(&self.link.front);
        let copy = match self.component {
            None => Some(&front.stack),
            Some(h) => h.index().and_then(|i| front.components.get(i)),
        };
        if let Some(copy) = copy {
            self.mirror.sync(copy, front.seq);
        }
        r
    }
}

impl Owner for Proxy<'_> {
    fn schema(&self) -> &Arc<Schema> {
        self.mirror.schema()
    }

    fn values(&self) -> Option<&Values> {
        self.mirror.values()
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let v = value.clone();
        self.on_thread(move |o, _| o.apply(id, &v))
            .unwrap_or(Err(SettingError::NotPermitted { id }))
    }

    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.mirror.results().decl()
    }

    fn results(&self) -> &Results {
        self.mirror.results()
    }

    fn events(&self) -> &[sdrkit_core::EventDecl] {
        self.mirror.events()
    }

    fn commands(&self) -> &Commands {
        self.mirror.commands()
    }

    /// Runs the command on the owner. A command that transmits is refused with [`CommandError::TransmitOff`] while the
    /// transmitter of the session is off, as on the session.
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let a = args.clone();
        let transmits = self.mirror.commands().def(id).and_then(|d| d.transmits);
        let streams = Arc::clone(&self.link.streams);
        self.on_thread(move |o, on| {
            if transmits.is_some() && !on {
                return Err(CommandError::TransmitOff);
            }
            let r = o.invoke(id, &a);
            if let (Ok(_), Some(c)) = (&r, transmits) {
                streams.transmit_command(c);
            }
            r
        })
        .unwrap_or(Err(CommandError::NoSuchId { id }))
    }

    fn problem(&mut self, text: &str) {
        let t = text.to_owned();
        let _ = self.on_thread(move |o, _| o.problem(&t));
    }
}

/// A session as an owner: its settings, then those of its receiver, the results of its receiver with its own counts, and
/// the commands of its receiver.
pub(crate) struct SessionControl {
    pub(crate) link: Arc<Link>,
    schema: Arc<Schema>,
    values: Values,
    own: u16,
    graph_decl: Arc<ResultsDecl>,
    results: Results,
    /// The number of changes of frequency that were ordered: by the host, by a service or by the receiver itself.
    retunes: u64,
    /// The number of the last change that the host ordered. While the receiver has taken fewer changes, its results
    /// are those of the frequency before, and the session shows none of them.
    hidden_until: u64,
    /// The `problem_seq` of the run whose problem the results show.
    problem_seq: u64,
    /// The `problem_seq` of the transmit report whose problem the results show.
    tx_problem_seq: u64,
    /// The transmit problem that the summary of the status shows. Empty if none.
    tx_problem: String,
    commands: Commands,
    /// The events for the host, after the services saw them.
    events: VecDeque<Event>,
    /// The number of the copy of the session thread that the results show.
    synced: u64,
    /// The results show no result of the receiver, so the next copy takes every table.
    hidden: bool,
    /// The number of the load measurement that the tables show.
    load_seq: u64,
    /// The number of the receiver whose settings and commands the session shows.
    generation: u64,
    /// The thread stopped on a fault. The results stay as they are.
    crashed: bool,
    pub(crate) state: SessionState,
    pub(crate) stack: Option<&'static StackDescriptor>,
    /// The submode of the stack. `None` for its default.
    pub(crate) submode: Option<String>,
    /// The operator of the session: what it demands of the radio is all that the engine does to the radio for the
    /// session.
    pub(crate) operator: Box<dyn Operator>,
    /// The sample rate that the stack asked for with a request, and the rate that its settings gave then. It counts
    /// until the settings give another rate, the frequency changes or another stack comes.
    pub(crate) asked_rate: Option<(f64, f64)>,
}

/// The operator of a session that runs `stack` in `submode`: that of the stack's table, or the manual one for a session
/// without a stack.
fn operator_of(
    stack: Option<&'static StackDescriptor>,
    submode: Option<&str>,
) -> Box<dyn Operator> {
    Box::new(stack.map_or_else(TableOperator::manual, |d| {
        TableOperator::of_stack(d, submode)
    }))
}

/// How far below the centre of `input` a session of `operator` listens while it has no frequency: the tune offset of the
/// operator on a radio, a recording and a host feed. An inside input is a channel that the engine cut around its station,
/// so the session listens at its centre.
pub(crate) fn follow_offset(input: Input, operator: &dyn Operator) -> f64 {
    match input {
        Input::Radio(_) | Input::Feed(_) => operator.tune_offset_hz(),
        Input::Inside => 0.0,
    }
}

/// A session: its owner, the state its thread shares, and the thread.
pub(crate) struct Session {
    pub(crate) control: Mutex<SessionControl>,
    pub(crate) link: Arc<Link>,
    pub(crate) thread: Mutex<Option<JoinHandle<()>>>,
    /// What the session reads.
    pub(crate) input: Input,
    /// Who started it.
    pub(crate) by: By,
    /// The host's positions in the streams, and its taps.
    pub(crate) readers: Mutex<Readers>,
    /// The copy of the receiver that the host reads, and those of the components of its graph by handle.
    stack: Mutex<Mirror>,
    components: Mutex<Vec<Mirror>>,
}

/// The schema of a session of `rx`: the session's own settings, then those of `rx`, with the values of `rx`. Also returns the
/// number of the session's own settings.
fn session_schema(rx: &dyn Receiver) -> Result<(Arc<Schema>, Values, u16), crate::EngineError> {
    let own = SessionSetting::schema();
    let n = own.len() as u16;
    let mut decls = own.settings().to_vec();
    decls.extend_from_slice(rx.schema().settings());
    let schema = Arc::new(Schema::new(decls)?);
    let mut values = Values::defaults(&schema);
    if let Some(v) = rx.values() {
        for i in 0..v.schema().len() {
            let id = SettingId(i as u16);
            if let Some(x) = v.value(id) {
                crate::EngineError::set(&mut values, SettingId(n + id.0), x)?;
            }
        }
    }
    Ok((schema, values, n))
}

/// The results that a session with a receiver of results `graph` shows: those of the receiver and those of the session.
pub(crate) fn merged_decl(graph: &ResultsDecl) -> ResultsDecl {
    let mut decl = graph.clone();
    for f in OWN_FIELDS {
        if decl.field_id(f.key()).is_none() {
            decl = decl.with_fields(&[f]);
        }
    }
    for t in [&COMPONENTS, &STAGES, &CONNECTIONS] {
        if decl.table_id(t.key).is_none() {
            decl = decl.with_table(t);
        }
    }
    decl
}

impl Session {
    /// A session of `rx` on `feed`, which reads `input`, with the streams `streams`. `stack` and `submode` name what `rx`
    /// runs.
    /// `to_radio` says that the engine sends the transmit stream to the radio. `transmit` is the transmit switch of the
    /// engine.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        mut rx: Box<dyn Receiver>,
        feed: Arc<Feed>,
        streams: Arc<Streams>,
        input: Input,
        stack: Option<&'static StackDescriptor>,
        submode: Option<&str>,
        by: By,
        to_radio: bool,
        transmit: Arc<AtomicBool>,
        cores: Arc<crate::stages::Cores>,
    ) -> Result<Session, crate::EngineError> {
        streams.set_present(feed.present());
        give_streams(rx.as_mut(), &streams);
        let (schema, values, own) = session_schema(rx.as_ref())?;
        let graph_decl = Arc::clone(rx.results_decl());
        let mut results = Results::new(Arc::new(merged_decl(&graph_decl)));
        results.fields_mut().set(&STATE, SessionState::Waiting);
        let operator = operator_of(stack, submode);
        let commands = rx.commands().clone();
        let mut run = Run {
            rx,
            input: None,
            tuned_hz: 0.0,
            offset_hz: follow_offset(input, operator.as_ref()),
            failure: None,
            values_stale: false,
            refused: None,
            at_rate: Vec::with_capacity(AT_RATE_SLOTS),
            at_rate_waited: 0.0,
            problem_seq: 0,
            retunes: 0,
            last_failed: None,
            requests: Vec::new(),
            records: Vec::new(),
            mixer: Mixer::default(),
            transmit,
            unlisted: 0,
            tx: None,
            generation: 0,
            problem: None,
            problem_at: 0,
            stale_at: 0,
            publisher: Publisher::default(),
            acks: Vec::new(),
            place: Placement::new(cores),
            dropped_seen: 0,
        };
        // The host reads the receiver before the thread starts.
        let front = Mutex::default();
        let seq = run.publish_to(&front, true, true).unwrap_or(0);
        let link = Arc::new(Link {
            run: Mutex::new(run),
            inbox: Mutex::new(Inbox {
                controls: Vec::new(),
                taken: Taken::NotYet,
            }),
            front,
            front_seq: AtomicU64::new(seq),
            front_generation: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            fresh: Mutex::default(),
            samples: AtomicU64::new(0),
            dropped: Arc::new(AtomicU64::new(0)),
            queue: Arc::default(),
            stop: AtomicBool::new(false),
            demand: Arc::new(Demand::default()),
            feed,
            gains: Arc::clone(&streams.gains),
            streams,
            to_radio,
        });
        Ok(Session {
            control: Mutex::new(SessionControl {
                link: Arc::clone(&link),
                schema,
                values,
                own,
                graph_decl,
                results,
                retunes: 0,
                hidden_until: 0,
                problem_seq: 0,
                tx_problem_seq: 0,
                tx_problem: String::new(),
                commands,
                events: VecDeque::with_capacity(EVENT_CAP),
                synced: 0,
                hidden: false,
                load_seq: 0,
                generation: 0,
                crashed: false,
                state: SessionState::Waiting,
                stack,
                submode: submode.map(str::to_owned),
                operator,
                asked_rate: None,
            }),
            link,
            thread: Mutex::new(None),
            input,
            by,
            readers: Mutex::default(),
            stack: Mutex::default(),
            components: Mutex::default(),
        })
    }

    /// The owner of the session, with the results of the newest copy of the session thread.
    pub(crate) fn owner(&self) -> MutexGuard<'_, SessionControl> {
        let mut c = lock(&self.control);
        c.sync();
        c
    }

    /// The newest copy of the session thread. After [`Session::replace`] the call waits until the thread has taken the
    /// new receiver, one block at most.
    fn front(&self) -> MutexGuard<'_, Snapshot> {
        if !self.current() {
            let _ = self.link.with_run(|_| ());
        }
        lock(&self.link.front)
    }

    /// True if the newest copy is of the receiver that the host gave the session last.
    fn current(&self) -> bool {
        let l = &self.link;
        l.front_generation.load(Ordering::Acquire) == l.generation.load(Ordering::Acquire)
    }

    /// True if `mirror` holds the newest copy.
    fn holds(&self, mirror: &Mirror) -> bool {
        self.current() && mirror.synced() == self.link.front_seq.load(Ordering::Acquire)
    }

    /// Runs `f` on the receiver of the session, as the host sees it.
    pub(crate) fn on_stack<R>(&self, f: impl FnOnce(&mut dyn Owner) -> R) -> R {
        let mut mirror = lock(&self.stack);
        if !self.holds(&mirror) {
            let front = self.front();
            mirror.sync(&front.stack, front.seq);
        }
        f(&mut Proxy {
            mirror: &mut mirror,
            link: &self.link,
            component: None,
        })
    }

    /// Runs `f` on component `handle` of the graph of the session, as the host sees it. `None` if the session runs no
    /// graph or the graph has no such component.
    pub(crate) fn on_component<R>(
        &self,
        handle: sdrkit_graph::ComponentHandle,
        f: impl FnOnce(&mut dyn Owner) -> R,
    ) -> Option<R> {
        let i = handle.index()?;
        let mut mirrors = lock(&self.components);
        if !mirrors.get(i).is_some_and(|m| self.holds(m)) {
            let front = self.front();
            let copy = front.components.get(i)?;
            if mirrors.len() <= i {
                mirrors.resize_with(i + 1, Mirror::default);
            }
            mirrors[i].sync(copy, front.seq);
        }
        Some(f(&mut Proxy {
            mirror: &mut mirrors[i],
            link: &self.link,
            component: Some(handle),
        }))
    }

    /// The handle of the component named `name` of the graph of the session. `None` if the session runs no graph,
    /// `Some(None)` if its graph has no component of that name.
    pub(crate) fn component(&self, name: &str) -> Option<Option<sdrkit_graph::ComponentHandle>> {
        let front = self.front();
        front.graph.then(|| {
            let i = front.names.iter().position(|n| n == name)?;
            u32::try_from(i).ok().map(sdrkit_graph::ComponentHandle)
        })
    }

    /// Tells the thread to stop. It stops after the block it works on.
    pub(crate) fn stop(&self) {
        self.link.stop.store(true, Ordering::Release);
        let mut c = lock(&self.control);
        c.state = SessionState::Stopped;
        c.results.fields_mut().set(&STATE, SessionState::Stopped);
    }

    /// Puts `rx` in place of the session's receiver between two blocks. The session's settings and commands become those of
    /// `rx` at once, and its results keep their change counters. The session thread takes `rx` before its next block, after
    /// the settings that wait, and configures it with that block. The call does not wait for the session thread.
    pub(crate) fn replace(
        &self,
        rx: Box<dyn Receiver>,
        stack: Option<&'static StackDescriptor>,
        submode: Option<&str>,
    ) -> Result<(), crate::EngineError> {
        let (schema, mut values, own) = session_schema(rx.as_ref())?;
        let mut c = lock(&self.control);
        for i in 0..own {
            if let Some(v) = c.values.value(SettingId(i)) {
                crate::EngineError::set(&mut values, SettingId(i), v)?;
            }
        }
        c.graph_decl = Arc::clone(rx.results_decl());
        c.commands = rx.commands().clone();
        let decl = Arc::new(merged_decl(&c.graph_decl));
        c.results = std::mem::take(&mut c.results).with_decl(decl);
        c.problem_seq = u64::MAX;
        (c.schema, c.values, c.own, c.stack) = (schema, values, own, stack);
        c.submode = submode.map(str::to_owned);
        // The session stays on its frequency under the operator of the new stack.
        let hz = c.operator.frequency();
        c.operator = operator_of(stack, submode);
        c.operator.set_frequency(hz);
        c.asked_rate = None;
        let offset_hz = follow_offset(self.input, c.operator.as_ref());
        c.generation += 1;
        self.link.generation.store(c.generation, Ordering::Release);
        self.link
            .send(Control::Replace(rx, offset_hz, c.generation));
        Ok(())
    }

    /// True once the session's thread has ended.
    pub(crate) fn finished(&self) -> bool {
        lock(&self.thread)
            .as_ref()
            .is_none_or(JoinHandle::is_finished)
    }

    pub(crate) fn join(&self) {
        if let Some(t) = lock(&self.thread).take() {
            let _ = t.join();
        }
    }
}

impl SessionControl {
    /// Takes the test-signal settings of `given`, the stack settings that the session started with: a receiver does not
    /// hold them ([`crate::is_signal_setting`]), so the session keeps them for the test signal.
    pub(crate) fn keep_signal_settings(&mut self, given: &Values) {
        for (i, decl) in given.schema().settings().iter().enumerate() {
            if !crate::is_signal_setting(&decl.field.key) {
                continue;
            }
            let Ok(at) = self.schema.id(&decl.field.key) else {
                continue;
            };
            if let Some(v) = given.value(SettingId(i as u16)) {
                let _ = self.values.set(at, v);
            }
        }
    }

    /// Moves the session's events into `out`.
    pub(crate) fn take_events(&mut self, out: &mut Vec<Event>) {
        out.extend(self.events.drain(..));
    }

    /// Passes `events` to the host, after the services saw them. The oldest go first when the queue is full.
    pub(crate) fn pass_on(&mut self, events: &mut Vec<Event>) {
        for e in events.drain(..) {
            if self.events.len() == EVENT_CAP {
                self.events.pop_front();
            }
            self.events.push_back(e);
        }
    }

    /// Clears the refusal of the last request of the receiver: the engine carried out the request that followed it.
    pub(crate) fn request_served(&mut self) {
        self.results.fields_mut().none(&REQUEST_PROBLEM);
    }

    /// Tunes the session to `hz` (0 follows the centre of the input), for a service or for the receiver itself. The
    /// results of the receiver stay in view.
    pub(crate) fn tune(&mut self, hz: f64) -> Result<(), crate::EngineError> {
        let id = SessionSetting::Tuned.id();
        let before = self.values.get(&TUNED);
        crate::EngineError::set(&mut self.values, id, &Value::Hz(hz))?;
        self.retune(before, hz, false);
        Ok(())
    }

    /// Tunes the session to `hz` for a scan. The stack forgets the results of the frequency before
    /// ([`Receiver::forget`]), and the session shows no result of the stack until the stack has taken the change. A
    /// tune to the frequency that the session has does the same.
    pub(crate) fn tune_fresh(&mut self, hz: f64) -> Result<(), crate::EngineError> {
        let id = SessionSetting::Tuned.id();
        crate::EngineError::set(&mut self.values, id, &Value::Hz(hz))?;
        self.retunes += 1;
        self.link.send(Control::Tune(hz, self.retunes, true));
        self.hidden_until = self.retunes;
        self.hidden = true;
        self.hide_receiver_results();
        Ok(())
    }

    /// Orders the change of frequency from `before` to `hz`, and returns true if the frequency changes. `by_host` is
    /// true for a change that the host made: the receiver then forgets the results of the frequency before
    /// ([`Receiver::forget`]).
    fn retune(&mut self, before: f64, hz: f64, by_host: bool) -> bool {
        let changes = hz != before;
        if changes {
            self.retunes += 1;
        }
        self.link
            .send(Control::Tune(hz, self.retunes, by_host && changes));
        changes
    }

    /// Makes the results hold no result of the receiver: its fields have no value, and its tables, series, rasters,
    /// points and log are empty. The fields of the session itself stay.
    fn hide_receiver_results(&mut self) {
        self.results
            .copy_from(&Results::new(Arc::clone(&self.graph_decl)));
    }

    /// The setting of the session's receiver with key `key`.
    pub(crate) fn receiver_setting(&self, key: &str) -> Option<SettingId> {
        let id = self.schema.id(key).ok()?;
        (id.0 >= self.own).then_some(id)
    }

    /// The thread of the session ended on a fault of the library with the message `text`: the session has failed and its
    /// problem says so.
    pub(crate) fn crashed(&mut self, text: &str) {
        self.sync();
        self.crashed = true;
        self.state = SessionState::Failed;
        let problem = format!("the receiver stopped on a fault in the library: {text}");
        self.results.fields_mut().set(&PROBLEM, problem.as_str());
        self.results.fields_mut().set(&STATE, SessionState::Failed);
    }

    fn spectrum_spec(&self) -> Option<SpectrumSpec> {
        let on = self.values.get(&SPECTRUM);
        on.then(|| SpectrumSpec {
            size: SizeRule::Bins(self.values.get(&SPECTRUM_SIZE)),
            window: self.values.get(&SPECTRUM_WINDOW),
            average: 1,
            frames: FrameRule::PerSecond(self.values.get(&SPECTRUM_FPS)),
            scale: SpectrumScale::Db,
        })
    }

    /// Brings the results up to date with the newest copy of the session thread: the receiver's results, the session's
    /// counts, the problem and the transmit report.
    pub(crate) fn sync(&mut self) {
        let link = Arc::clone(&self.link);
        if link.front_seq.load(Ordering::Acquire) == self.synced || self.crashed {
            return;
        }
        let front = lock(&link.front);
        // The copy is of the receiver before: the session shows the new one already.
        if front.generation != self.generation {
            return;
        }
        let (src, run) = (&front.stack.results, &front.state);
        let mut all = self.hidden;
        if !Arc::ptr_eq(src.decl(), &self.graph_decl) {
            self.graph_decl = Arc::clone(src.decl());
            let decl = Arc::new(merged_decl(&self.graph_decl));
            self.results = std::mem::take(&mut self.results).with_decl(decl);
            self.problem_seq = u64::MAX;
            all = true;
        }
        self.hidden = run.retunes < self.hidden_until;
        if self.hidden {
            self.hide_receiver_results();
        } else {
            let (at, since) = (&front.stack.changed_at, self.synced);
            self.results.copy_from_where(src, |t| {
                all || at.get(usize::from(t.0)).is_none_or(|&c| c > since)
            });
        }
        if self.state != SessionState::Stopped {
            self.state = match (run.failed, run.has_input) {
                (true, _) => SessionState::Failed,
                (false, true) => SessionState::Running,
                (false, false) => SessionState::Waiting,
            };
        }
        let samples = self.link.samples.load(Ordering::Relaxed);
        let dropped = self.link.dropped.load(Ordering::Relaxed);
        let status = self.results.status();
        let (snr, level, offset) = (status.snr_db, status.level_dbfs, status.offset_hz);
        let mut f = self.results.fields_mut();
        f.opt(&SNR, snr.map(f64::from));
        f.opt(&LEVEL, level.map(f64::from));
        f.opt(&OFFSET, offset);
        f.set(&STATE, self.state);
        f.set(&SAMPLES, i64::try_from(samples).unwrap_or(i64::MAX));
        // The samples lost at the queue and before a stage, and those that the driver lost in the gaps it marked.
        let dropped = dropped + run.load.gap_samples;
        f.set(&DROPPED, i64::try_from(dropped).unwrap_or(i64::MAX));
        f.set(&UNLISTED, i64::try_from(run.unlisted).unwrap_or(i64::MAX));
        f.set(&RETUNES, i64::try_from(run.retunes).unwrap_or(i64::MAX));
        let load = &run.load;
        if load.seq > 0 {
            f.set(&SHARE, load.share);
            f.set(&CORES_NEEDED, load.need);
            f.opt(
                &CORES_DECLARED,
                (load.declared > 0.0).then_some(load.declared),
            );
            f.set(&CORES_FREE, load.free);
            f.set(&CORE_UNITS, load.core_units);
            f.set(&THREADS, i64::from(load.threads));
            f.set(
                &STAGE_BEHIND,
                i64::try_from(load.behind).unwrap_or(i64::MAX),
            );
            f.set(&GAPS, i64::try_from(load.gaps).unwrap_or(i64::MAX));
            let short = load.short.then(|| {
                format!(
                    "needs about {:.1} cores, {:.0} free",
                    load.declared,
                    load.free.floor()
                )
            });
            f.opt(&LOAD_PROBLEM, short.as_deref());
        }
        let s = &self.link.streams;
        f.set(
            &AUDIO_LOST,
            i64::try_from(s.audio_lost()).unwrap_or(i64::MAX),
        );
        f.set(
            &TRANSMIT_LOST,
            i64::try_from(s.transmit_lost()).unwrap_or(i64::MAX),
        );
        f.set(
            &MEDIA_LOST,
            i64::try_from(s.media_lost()).unwrap_or(i64::MAX),
        );
        f.set(
            &AUDIO_IN_LOST,
            i64::try_from(s.audio_in_lost()).unwrap_or(i64::MAX),
        );
        // Host audio is for the transmitter: a session that cannot transmit does not ask for it.
        let takes = s.wants_audio_in() && s.has_transmit();
        f.opt(
            &AUDIO_IN_RATE,
            takes.then_some(sdrkit_graph::components::HOST_IN_RATE),
        );
        // Wanted while a sender keeps the host audio; a speech command asks at once, before the sender's first pass.
        f.set(
            &AUDIO_IN_WANTED,
            takes && (s.voice_wanted() || s.audio_in_kept()),
        );
        let tx = run.tx.as_ref();
        if let Some(t) = tx {
            f.set(&TX_SENT, i64::try_from(t.sent).unwrap_or(i64::MAX));
            f.set(&TX_FAILED, i64::try_from(t.failed).unwrap_or(i64::MAX));
            f.opt(&TX_LAST, t.last);
            f.opt(&crate::transmit::TX_BLOCKED, t.blocked.as_deref());
        }
        if std::mem::replace(&mut self.problem_seq, run.problem_seq) != run.problem_seq {
            f.opt(&PROBLEM, run.problem.as_deref());
        }
        if let Some(t) = tx.filter(|t| t.problem_seq != self.tx_problem_seq) {
            self.tx_problem_seq = t.problem_seq;
            match &t.problem {
                Some(p) => {
                    self.results.problem(p);
                    p.clone_into(&mut self.tx_problem);
                }
                // The burst after the failed one went on air: the summary no longer names the failure.
                None => {
                    let shown = std::mem::take(&mut self.tx_problem);
                    self.results.update_status(|s| {
                        let c = !shown.is_empty() && s.summary == shown;
                        if c {
                            s.summary.clear();
                        }
                        c
                    });
                }
            }
        }
        // A setting was refused on the thread, or the host reached the receiver itself: the values are read again, except
        // those of the test signal, which the session keeps.
        if run.stale_at > self.synced {
            if let Some(v) = &front.stack.values {
                for (i, decl) in v.schema().settings().iter().enumerate() {
                    if crate::is_signal_setting(&decl.field.key) {
                        continue;
                    }
                    if let Some(x) = v.value(SettingId(i as u16)) {
                        let _ = self.values.set(SettingId(self.own + i as u16), x);
                    }
                }
            }
        }
        if run.load.seq != self.load_seq {
            self.load_seq = run.load.seq;
            write_load(&mut self.results, &run.load);
        }
        self.synced = front.seq;
    }
}

/// Writes the rows of the tables [`COMPONENTS`], [`STAGES`] and [`CONNECTIONS`].
fn write_load(results: &mut Results, load: &crate::stages::Load) {
    let us = |ns: u64| ns as f64 / 1e3;
    let int = |n: u64| i64::try_from(n).unwrap_or(i64::MAX);
    if let Some(mut t) = results
        .decl()
        .table_id(COMPONENTS.key)
        .and_then(|id| results.table_mut(id))
    {
        t.clear();
        let mut key = String::new();
        for c in &load.components {
            key.clear();
            key.push_str(&c.load.component);
            if !c.load.section.is_empty() {
                key.push('/');
                key.push_str(c.load.section);
            }
            let mut r = t.row_mut(&key);
            r.set(&LOAD_SECTION, c.load.section);
            r.set(&LOAD_STAGE, &c.stage);
            r.set(&LOAD_THREAD, i64::from(c.thread));
            r.set(&LOAD_SHARE, c.share);
            r.set(&LOAD_SHARE_TOTAL, c.share_total);
            r.set(&LOAD_CALLS, int(c.load.calls));
            r.set(&LOAD_ITEMS_IN, int(c.load.items_in));
            r.set(&LOAD_ITEMS_OUT, int(c.load.items_out));
            r.set(
                &LOAD_MEAN_US,
                us(c.load.busy_ns) / c.load.calls.max(1) as f64,
            );
            r.set(&LOAD_WORST_US, us(c.load.worst_ns));
            r.set(&LOAD_UNITS, c.units);
        }
    }
    if let Some(mut t) = results
        .decl()
        .table_id(STAGES.key)
        .and_then(|id| results.table_mut(id))
    {
        t.clear();
        for s in &load.stages {
            let mut r = t.row_mut(&s.load.name);
            r.set(&LOAD_THREAD, i64::from(s.load.thread));
            r.set(&LOAD_CUT, s.load.cut);
            r.set(&LOAD_SHARE, s.share);
            r.set(&LOAD_COST, s.cost);
            r.opt(&LOAD_DECLARED, s.declared);
            r.set(&LOAD_QUEUE_FILL, s.load.queue_fill());
            r.set(&LOAD_QUEUE_UNITS, int(s.load.queue_units as u64));
            r.set(&LOAD_QUEUE_MAX, int(s.load.queue_units_max as u64));
            r.set(&LOAD_BEHIND, int(s.load.behind));
            r.set(&LOAD_UNITS, s.units);
            r.opt(&LOAD_DECLARED_UNITS, s.declared_units);
        }
    }
    if let Some(mut t) = results
        .decl()
        .table_id(CONNECTIONS.key)
        .and_then(|id| results.table_mut(id))
    {
        t.clear();
        let mut key = String::new();
        for (k, c) in load.connections.iter().enumerate() {
            let l = &c.load;
            key.clear();
            if k == 0 && l.from == "input" {
                key.push_str("queue");
            } else {
                use std::fmt::Write;
                let _ = write!(key, "{}.{}>{}.{}", l.from, l.output, l.to, l.input);
            }
            let mut r = t.row_mut(&key);
            r.set(&CONN_FROM, &l.from);
            r.set(&CONN_OUTPUT, &l.output);
            r.set(&CONN_TO, &l.to);
            r.set(&CONN_INPUT, &l.input);
            r.set(&CONN_FORMAT, &l.format);
            r.set(&LOAD_STAGE, &c.stage);
            r.set(&CONN_ITEMS, c.items_per_s);
            r.set(&CONN_BYTES, c.bytes_per_s);
            r.set(&CONN_HELD, int(l.held_bytes));
            r.set(
                &CONN_HELD_MEAN,
                l.held_sum as f64 / l.held_samples.max(1) as f64,
            );
            r.set(&CONN_HELD_MAX, int(l.held_max));
            let cap = (l.capacity_bytes > 0).then_some(l.capacity_bytes);
            r.opt(&CONN_CAPACITY, cap.map(int));
            r.opt(
                &CONN_FILL,
                cap.map(|c| (l.held_bytes as f64 / c as f64).min(1.0)),
            );
            let waited = (l.wait_samples > 0).then_some(l.wait_samples);
            r.opt(
                &CONN_WAIT_MEAN,
                waited.map(|n| us(l.wait_ns_sum) / n as f64),
            );
            r.opt(&CONN_WAIT_WORST, waited.map(|_| us(l.wait_worst_ns)));
            r.set(&CONN_FULL, int(l.full));
            r.set(&CONN_EMPTY, int(l.empty));
            r.set(&CONN_BOUNDARY, l.boundary);
            r.set(&CONN_CUT, l.cut);
            r.set(&CONN_LOST, int(c.lost));
        }
    }
}

/// Feeds the samples of `b` to `rx`, and tells it where samples are missing: `lost` samples before the block, and each
/// gap of the block at its place. Returns the count of the places.
pub(crate) fn feed_block(rx: &mut dyn Receiver, b: &Block, lost: u64, out: &mut Output) -> u64 {
    let iq = b.samples();
    let mut places = 0;
    if lost > 0 {
        rx.discontinuity(lost);
        places += 1;
    }
    let mut at = 0;
    for g in b.gaps() {
        let (before, lost) = (g.at.min(iq.len()), g.lost);
        if before > at {
            rx.feed(&iq[at..before], out);
            at = before;
        }
        rx.discontinuity(lost);
        places += 1;
    }
    if at < iq.len() || iq.is_empty() {
        rx.feed(&iq[at..], out);
    }
    places
}

impl Owner for SessionControl {
    fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }

    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }

    /// Takes the value at once. A setting of the receiver reaches the receiver before its next block. If the receiver
    /// refuses it there, the value goes back and the results show the receiver's own value; a setting of the test signal
    /// ([`crate::is_signal_setting`]) keeps the value, as the receiver does not hold it.
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        if self.state == SessionState::Stopped {
            return Err(SettingError::NotPermitted { id });
        }
        let before = self.values.get(&TUNED);
        self.values.set(id, value)?;
        match SessionSetting::from_id(id) {
            // The host changes the frequency: from now until the receiver has taken the change, the session shows no
            // result of the receiver.
            Some(SessionSetting::Tuned) => {
                if self.retune(before, TUNED.read(id, value)?, true) {
                    self.hidden_until = self.retunes;
                    self.hidden = true;
                    self.hide_receiver_results();
                }
            }
            Some(
                SessionSetting::Volume
                | SessionSetting::Pan
                | SessionSetting::Mute
                | SessionSetting::Solo,
            ) => {
                let v = &self.values;
                self.link
                    .gains
                    .set(v.get(&VOLUME), v.get(&PAN), v.get(&MUTE), v.get(&SOLO));
            }
            Some(
                SessionSetting::Spectrum
                | SessionSetting::SpectrumSize
                | SessionSetting::SpectrumWindow
                | SessionSetting::SpectrumFps,
            ) => {
                *lock(&self.link.demand.spec) = self.spectrum_spec();
                self.link.feed.demand_changed();
            }
            None => self
                .link
                .send(Control::Set(SettingId(id.0 - self.own), value.clone())),
        }
        Ok(())
    }

    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.results.decl()
    }

    fn results(&self) -> &Results {
        &self.results
    }

    fn commands(&self) -> &Commands {
        &self.commands
    }

    /// Shows `text` in the field [`REQUEST_PROBLEM`]: why the engine refused the last request of the receiver.
    fn problem(&mut self, text: &str) {
        self.results.fields_mut().set(&REQUEST_PROBLEM, text);
    }

    /// Runs the command on the receiver between two blocks, after the settings that wait. A command that transmits is
    /// refused with [`CommandError::TransmitOff`] while the transmitter of the session is off, the state that the receiver
    /// is given ([`Run::pass_transmit`]); the receiver does not see it.
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let args = args.clone();
        let transmits = self.commands.def(id).and_then(|d| d.transmits);
        let streams = Arc::clone(&self.link.streams);
        self.link
            .with_run(move |run| {
                if transmits.is_some() && !run.transmit_on(&streams) {
                    return Err(CommandError::TransmitOff);
                }
                let r = run.rx.invoke(id, &args);
                if let (Ok(_), Some(c)) = (&r, transmits) {
                    streams.transmit_command(c);
                }
                r
            })
            .unwrap_or(Err(CommandError::NoSuchId { id }))
    }
}

/// The loop of a session's thread: blocks from `blocks` into the receiver, the results and events out to the owner.
///
/// `tx_side` sends the session's transmit stream to its radio on a thread of its own ([`TxWorker`]), so that a burst
/// does not hold up the receive side. If that thread does not start, the session does not transmit.
pub(crate) fn run(
    session: Arc<Session>,
    blocks: Inbound<Arc<Block>>,
    catalog: Arc<EventCatalog>,
    tx_side: Option<TxSide>,
) {
    let link = Arc::clone(&session.link);
    let mut tx = tx_side.and_then(|s| {
        crate::transmit::TxWorker::start(s, Arc::clone(&link.streams), Arc::clone(&catalog)).ok()
    });
    let live = link.feed.live();
    let inside = session.input == Input::Inside;
    let idle = Block::default();
    let mut inbox: Vec<Control> = Vec::new();
    let mut events: Vec<Event> = Vec::with_capacity(EVENT_CAP);
    let mut pending: Vec<Elementary> = Vec::with_capacity(PENDING);
    let mut out = Output::default();
    let mut seen = u64::MAX;
    let mut load_seen = 0;
    let mut last = Instant::now();
    let mut next = Instant::now();
    link.started();
    {
        let mut run = lock(&link.run);
        let run = &mut *run;
        if !inside {
            run.place.queue = Some(Arc::clone(&link.queue));
        }
        run.place.start(run.rx.as_mut());
    }
    loop {
        let wait = if inside {
            next.saturating_duration_since(Instant::now())
        } else {
            Duration::from_millis(50)
        };
        // The reader that finds the queue empty is counted, then waits.
        let block = match blocks.try_recv() {
            Ok(b) => Some(b),
            Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {
                if !inside {
                    link.queue.empty.fetch_add(1, Ordering::Relaxed);
                }
                match blocks.recv_timeout(wait) {
                    Ok(b) => Some(b),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        };
        if let Some(b) = &block {
            link.queue.take(b);
            link.feed.took();
        }
        if link.stop.load(Ordering::Acquire) {
            break;
        }
        let mut run = lock(&link.run);
        link.take_inbox(&mut inbox);
        let changed = !inbox.is_empty();
        run.apply(&mut inbox, live, &link.streams);
        if changed {
            // A thread of the host that waits for its call goes on once the copy shows what the call did.
            let wait = !run.acks.is_empty();
            run.publish(&link, true, wait);
        }
        if let Some(b) = block {
            run.feed(&b, live, link.dropped.load(Ordering::Relaxed), &mut out);
            link.streams.pass_done();
            run.record(&b, &out);
            link.samples
                .fetch_add(b.samples().len() as u64, Ordering::Relaxed);
        } else if inside && Instant::now() >= next {
            run.feed(&idle, false, 0, &mut out);
            link.streams.pass_done();
            run.record(&idle, &out);
            next += PACE;
            // A session that fell far behind starts its pace again from now.
            let now = Instant::now();
            if now.saturating_duration_since(next) > PUBLISH {
                next = now;
            }
        } else if let Some(st) = run.rx.staged() {
            // No block: what the stage threads finished meanwhile still goes out.
            st.poll(&mut out);
        }
        for p in out.audio_runs() {
            link.streams
                .write_audio_layout(p.samples, p.rate_hz, p.channels, p.layout);
        }
        run.mixer.step(&link.streams);
        pending.append(&mut out.media);
        link.streams.offer_media(&mut pending);
        run.requests.append(&mut out.requests);
        for mut e in out.events.drain(..) {
            if catalog.stamp(&mut e).is_some() {
                events.push(e);
            } else {
                run.unlisted += 1;
            }
        }
        out.clear();
        if let Some(t) = &tx {
            run.tx = Some(t.take(&mut events));
        }
        link.offer(&mut events, &mut run.requests, false);
        {
            let run = &mut *run;
            run.place
                .look(run.rx.as_mut(), link.dropped.load(Ordering::Relaxed));
        }
        let counter = run.rx.results().change_counter();
        let age = last.elapsed();
        let due = run.publisher.waits
            || run.values_stale
            || (run.place.load.seq != load_seen && age >= PUBLISH_MIN)
            || (counter != seen && age >= PUBLISH_MIN)
            || age >= PUBLISH;
        if due && run.publish(&link, false, false) {
            seen = counter;
            load_seen = run.place.load.seq;
            last = Instant::now();
        }
        drop(run);
    }
    // The feed sends no more: a block after the end counts as no loss, and the last copy holds the final count.
    drop(blocks);
    // The burst that runs ends, and the transmitter goes off air, before the last copy.
    let report = tx.take().map(|t| t.finish(&mut events));
    link.ended(&mut inbox);
    {
        let mut run = lock(&link.run);
        run.apply(&mut inbox, live, &link.streams);
        // The work that waits between the stages is finished, and the stage threads end.
        {
            let run = &mut *run;
            if let Some(st) = run.rx.staged() {
                st.drain(&mut out);
            }
            run.place.end(run.rx.as_mut());
            run.place
                .publish_last(run.rx.as_mut(), link.dropped.load(Ordering::Relaxed));
        }
        for p in out.audio_runs() {
            link.streams
                .write_audio_layout(p.samples, p.rate_hz, p.channels, p.layout);
        }
        pending.append(&mut out.media);
        link.streams.offer_media(&mut pending);
        // The session ends: a picture that still grows goes to the host once more, finished.
        link.streams.end_pictures();
        for mut e in out.events.drain(..) {
            if catalog.stamp(&mut e).is_some() {
                events.push(e);
            }
        }
        if report.is_some() {
            run.tx = report;
        }
        link.offer(&mut events, &mut run.requests, true);
        run.publish(&link, true, true);
    }
    let mut c = lock(&session.control);
    c.state = SessionState::Stopped;
    c.sync();
    c.results.fields_mut().set(&STATE, SessionState::Stopped);
}

/// Sets on `rx`, if it is a graph, the session's streams for its endpoints (`host.out`, `tx.sink`), and whether its
/// signal is made as it plays.
fn give_streams(rx: &mut dyn Receiver, streams: &Arc<Streams>) {
    if let Some(g) = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<sdrkit_graph::Graph>())
    {
        let mut ctx = g.context().clone();
        ctx.present = streams.present();
        ctx.streams = Some(Arc::clone(streams) as Arc<dyn sdrkit_graph::components::HostStreams>);
        g.set_context(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::Complex32;

    /// Notes the calls of the session thread.
    #[derive(Default)]
    struct Notes(Vec<String>);

    impl Owner for Notes {
        fn apply(&mut self, id: SettingId, _: &Value) -> Result<(), SettingError> {
            Err(SettingError::NoSuchId { id })
        }
        fn results(&self) -> &Results {
            unreachable!("the test reads no results")
        }
    }

    impl Receiver for Notes {
        fn configure(&mut self, _: &InputInfo) -> Result<(), ConfigError> {
            Ok(())
        }
        fn feed(&mut self, iq: &[Complex32], _: &mut Output) {
            self.0.push(format!("feed {}", iq.len()));
        }
        fn discontinuity(&mut self, lost: u64) {
            self.0.push(format!("gap {lost}"));
        }
    }

    #[test]
    fn a_gap_of_a_block_reaches_the_stack_between_the_samples_around_it() {
        let iq = [Complex32::default(); 10];
        let mut out = Output::default();
        let mut rx = Notes::default();
        // 7 samples were lost before the block, 5 after its fourth sample and 3 after its last.
        let deliberate = sdrkit_driver_core::Gap {
            deliberate: true,
            ..sdrkit_driver_core::Gap::lost_at(10, 3)
        };
        let b = Block::of(&iq, &[sdrkit_driver_core::Gap::lost_at(4, 5), deliberate]);
        assert_eq!(feed_block(&mut rx, &b, 7, &mut out), 3);
        assert_eq!(rx.0, ["gap 7", "feed 4", "gap 5", "feed 6", "gap 3"]);
        // A block without a gap is fed whole, and an empty block still runs a pass.
        let mut rx = Notes::default();
        assert_eq!(feed_block(&mut rx, &Block::of(&iq, &[]), 0, &mut out), 0);
        assert_eq!(feed_block(&mut rx, &Block::default(), 0, &mut out), 0);
        assert_eq!(rx.0, ["feed 10", "feed 0"]);
    }
}
