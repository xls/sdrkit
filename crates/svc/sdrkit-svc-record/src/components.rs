//! The recorders as graph components (sinks) and the transport stream player (a generator). Each recorder starts and stops by
//! command and takes the settings of the record service.

use crate::audio::{self, AudioControl, AudioFeed};
use crate::iq::{self, IqControl, IqFeed};
use crate::rewind::{self, RewindControl, RewindFeed};
use crate::service::{
    self, Problem, RunState, AUDIO_CONTAINER, A_LONG, A_POST, A_PRE, A_REASON, CHANNEL_IQ,
    CLOSE_DB, COMMENT, FOLDER, GATE, HANG, IQ_CONTAINER, IQ_SAMPLE, LOG, MAX_FILE, MIN_LENGTH,
    MONO, OPEN_DB, PREFIX, PREROLL, QUEUE, REWIND_BUDGET, REWIND_LONG, REWIND_LONG_BUDGET, SPLIT,
    TITLE, TS_DROP_NULL, TS_MAX, TS_SIDECAR, UTC_OFFSET,
};
use crate::spool::{Live, Note, Writer};
use crate::ts::{self, TsControl, TsFeed};
use crate::tsplay::{FileStore, ReadState, TsPlayer};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::media::Packet;
use sdrkit_core::publish::{field, Field, Published};
use sdrkit_core::settings::{Argument, SettingError, SettingSet};
use sdrkit_core::stack::StageError;
use sdrkit_core::{
    Clock, ConfigError, Record, Results, Setting, SettingId, Storage, StorePath, Value, Values,
};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Item, Port, Registry};
use sdrkit_proto_mpegts::packet::TS_SIZE;
use sdrkit_proto_mpegts::stream::{self, TsPacket};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Packets the player reads at most per call.
const PLAY_BATCH: usize = 512;

/// Registers the recorder and player components, which write and read through `storage` and name files by `clock`.
pub fn register(r: &mut Registry, storage: Arc<dyn Storage>, clock: Clock) {
    let (s, c) = (storage.clone(), clock.clone());
    r.add("record.audio", move || {
        Box::new(AudioRecorder::new(s.clone(), c.clone()))
    });
    let (s, c) = (storage.clone(), clock.clone());
    r.add("record.iq", move || {
        Box::new(IqRecorder::new(s.clone(), c.clone()))
    });
    let (s, c) = (storage.clone(), clock.clone());
    r.add("record.ts", move || {
        Box::new(TsRecorder::new(s.clone(), c.clone()))
    });
    let (s, c) = (storage.clone(), clock.clone());
    r.add("record.rewind", move || {
        Box::new(RewindRecorder::new(s.clone(), c.clone()))
    });
    r.add("record.ts_play", move || {
        Box::new(TsFilePlayer::new(storage.clone(), clock.clone()))
    });
}

/// One squelch decision for the audio recorder, timed at the end of its block.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GateBlock {
    /// The squelch condition is met in this block, before its own hang (the recorder applies its own hang).
    pub open: bool,
}

impl Item for GateBlock {
    const KIND: &'static str = "record.gate";
}

const STATE: Field<field::Choice<RunState>> = Field::new("state", "State", "");
const FILE: Field<field::Text> = Field::new("file", "File", "");
const FILES: Field<field::I64> = Field::new("files", "Files kept", "");
const FRAMES: Field<field::I64> = Field::new("frames", "Written", "");
const DROPPED: Field<field::I64> = Field::new("dropped", "Left out", "");
const PROBLEM: Field<field::Choice<Problem>> = Field::new("problem", "Problem", "");

const C_START: Command = Command::new("start", "Start recording");
const C_STOP: Command = Command::new("stop", "Stop recording");

sdrkit_core::commands! {
    /// The commands of a recorder component.
    pub enum RecorderCommand {
        /// Starts recording with the settings in force.
        Start = C_START,
        /// Stops recording.
        Stop = C_STOP,
    }
}

fn count(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn failed(e: crate::error::RecordError) -> CommandError {
    CommandError::Failed {
        source: StageError::new(e),
    }
}

/// The control side of a recording that a component runs, and its results.
struct Shown {
    p: Published,
    files: u64,
    file: Option<StorePath>,
    problem: Problem,
}

impl Shown {
    fn new() -> Shown {
        let mut p = Published::new(&[&STATE, &FILE, &FILES, &FRAMES, &DROPPED, &PROBLEM]);
        p.set(&STATE, RunState::Waiting);
        p.set(&PROBLEM, Problem::None);
        Shown {
            p,
            files: 0,
            file: None,
            problem: Problem::None,
        }
    }

    /// Reads the notes of `writer` and writes the results.
    fn poll<B: crate::spool::Block>(&mut self, writer: &mut Writer<B>, live: &Live) {
        while let Some(n) = writer.note() {
            match n {
                Note::Opened { slot: 0, path } => self.file = Some(path),
                Note::Closed {
                    slot: 0,
                    kept: true,
                    ..
                } => self.files += 1,
                Note::Failed { error, .. } => {
                    self.problem = Problem::of(&error);
                    live.stop.store(true, Ordering::Relaxed);
                }
                Note::Opened { .. } | Note::Closed { .. } | Note::Done => {}
            }
        }
        let state = if self.problem != Problem::None {
            RunState::Failed
        } else if live.ended.load(Ordering::Relaxed) {
            RunState::Ending
        } else if live.gate_open.load(Ordering::Relaxed) || self.file.is_some() {
            RunState::Recording
        } else {
            RunState::Waiting
        };
        self.p.set(&STATE, state);
        self.p.opt(&FILE, self.file.as_ref().map(StorePath::as_str));
        self.p.set(&FILES, count(self.files));
        self.p
            .set(&FRAMES, count(live.frames.load(Ordering::Relaxed)));
        self.p
            .set(&DROPPED, count(live.dropped.load(Ordering::Relaxed)));
        self.p.set(&PROBLEM, self.problem);
    }
}

sdrkit_core::settings! {
    /// The settings of the audio recorder component.
    pub enum AudioRecorderSetting {
        /// The folder.
        Folder = FOLDER,
        /// The start of file names.
        Prefix = PREFIX,
        /// The local time offset.
        UtcOffset = UTC_OFFSET,
        /// The write queue.
        Queue = QUEUE,
        /// The container.
        Container = AUDIO_CONTAINER,
        /// Mono.
        Mono = MONO,
        /// Whole or per transmission.
        Split = SPLIT,
        /// The transmission gate.
        Gate = GATE,
        /// The open level.
        OpenDb = OPEN_DB,
        /// The close level.
        CloseDb = CLOSE_DB,
        /// The pre-roll.
        Preroll = PREROLL,
        /// The hang.
        Hang = HANG,
        /// The shortest transmission kept.
        MinLength = MIN_LENGTH,
        /// The longest file.
        MaxFile = MAX_FILE,
        /// The log of kept files.
        Log = LOG,
        /// The title tag.
        Title = TITLE,
        /// The comment tag.
        Comment = COMMENT,
        /// The channel IQ beside each file.
        ChannelIq = CHANNEL_IQ,
    }
}

/// The audio recorder (`record.audio`): audio in (`audio`), with the squelch decisions (`gate`, optional) and the channel IQ
/// (`iq`, optional).
pub struct AudioRecorder {
    storage: Arc<dyn Storage>,
    clock: Clock,
    values: Values,
    run: Option<(AudioFeed, AudioControl)>,
    shown: Shown,
    rate: f64,
    iq_rate: f64,
    squelch: bool,
}

impl AudioRecorder {
    /// A recorder that writes through `storage` and names files by `clock`. It records after the command start.
    pub fn new(storage: Arc<dyn Storage>, clock: Clock) -> Self {
        AudioRecorder {
            storage,
            clock,
            values: Values::defaults(&AudioRecorderSetting::schema()),
            run: None,
            shown: Shown::new(),
            rate: 0.0,
            iq_rate: 0.0,
            squelch: false,
        }
    }
}

impl Component for AudioRecorder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "record.audio",
            vec![
                Port::audio("audio"),
                Port::typed::<GateBlock>("gate").optional("the squelch is open"),
                Port::iq("iq").optional("no channel IQ"),
            ],
        )
        .with_settings(AudioRecorderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs.first().map_or(0.0, |f| f.rate);
        self.iq_rate = inputs.get(2).map_or(0.0, |f| f.rate);
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let Some((feed, control)) = &mut self.run else {
            return;
        };
        if io.is_connected(1) {
            if let Some(g) = io.items::<GateBlock>(1).last() {
                self.squelch = g.value.open;
            }
        } else {
            self.squelch = true;
        }
        if io.is_connected(2) {
            feed.push_iq(io.iq(2), self.iq_rate);
        }
        let a = io.audio(0);
        feed.push(&a.samples, a.channels, self.rate, self.squelch);
        self.shown.poll(&mut control.writer, &control.live);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        if AudioRecorderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?
            == AudioRecorderSetting::Folder
        {
            service::check_folder(id, v)?;
        }
        self.values.set(id, v)?;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.values.clone()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.shown.p.results())
    }
    fn commands(&self) -> &Commands {
        RecorderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match RecorderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RecorderCommand::Start => {
                let spec = service::audio_spec(&self.values, "", 0.0).map_err(failed)?;
                self.run = Some(
                    audio::start(spec, "", self.storage.clone(), self.clock.clone())
                        .map_err(failed)?,
                );
                self.shown = Shown::new();
            }
            // Dropping the feed closes its file and ends its writer, which finishes on its own thread.
            RecorderCommand::Stop => {
                self.run = None;
                self.shown.p.set(&STATE, RunState::Ending);
            }
        }
        Ok(None)
    }
}

sdrkit_core::settings! {
    /// The settings of the IQ recorder component.
    pub enum IqRecorderSetting {
        /// The folder.
        Folder = FOLDER,
        /// The start of file names.
        Prefix = PREFIX,
        /// The local time offset.
        UtcOffset = UTC_OFFSET,
        /// The write queue.
        Queue = QUEUE,
        /// The container.
        Container = IQ_CONTAINER,
        /// The sample type.
        Sample = IQ_SAMPLE,
    }
}

/// The IQ recorder (`record.iq`): IQ in (`iq`).
pub struct IqRecorder {
    storage: Arc<dyn Storage>,
    clock: Clock,
    values: Values,
    run: Option<(IqFeed, IqControl)>,
    shown: Shown,
    format: (f64, f64),
}

impl IqRecorder {
    /// A recorder that writes through `storage` and names files by `clock`. It records after the command start.
    pub fn new(storage: Arc<dyn Storage>, clock: Clock) -> Self {
        IqRecorder {
            storage,
            clock,
            values: Values::defaults(&IqRecorderSetting::schema()),
            run: None,
            shown: Shown::new(),
            format: (0.0, 0.0),
        }
    }
}

impl Component for IqRecorder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("record.iq", vec![Port::iq("iq")])
            .with_settings(IqRecorderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.format = inputs.first().map_or((0.0, 0.0), |f| (f.rate, f.centre_hz));
        if let Some((feed, _)) = &mut self.run {
            feed.set_format(self.format.0, self.format.1);
        }
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some((feed, control)) = &mut self.run {
            feed.push(io.iq(0));
            self.shown.poll(&mut control.writer, &control.live);
        }
    }
    fn retuned(&mut self, centre_hz: f64, _tuned_hz: f64) {
        self.format.1 = centre_hz;
        if let Some((feed, _)) = &mut self.run {
            feed.set_format(self.format.0, centre_hz);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        if IqRecorderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?
            == IqRecorderSetting::Folder
        {
            service::check_folder(id, v)?;
        }
        self.values.set(id, v)?;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.values.clone()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.shown.p.results())
    }
    fn commands(&self) -> &Commands {
        RecorderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match RecorderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RecorderCommand::Start => {
                let spec = service::iq_spec(&self.values, 0.0).map_err(failed)?;
                let (mut feed, control) =
                    iq::start(spec, "", self.storage.clone(), self.clock.clone())
                        .map_err(failed)?;
                feed.set_format(self.format.0, self.format.1);
                self.run = Some((feed, control));
                self.shown = Shown::new();
            }
            // Dropping the feed closes its file and ends its writer, which finishes on its own thread.
            RecorderCommand::Stop => {
                self.run = None;
                self.shown.p.set(&STATE, RunState::Ending);
            }
        }
        Ok(None)
    }
}

const SERVICES: Setting<field::Text> = Setting::new("services", "Services", "", "")
    .help("Service ids separated by commas, one file each. Empty records the multiplex.");

sdrkit_core::settings! {
    /// The settings of the transport stream recorder component.
    pub enum TsRecorderSetting {
        /// The folder.
        Folder = FOLDER,
        /// The start of file names.
        Prefix = PREFIX,
        /// The local time offset.
        UtcOffset = UTC_OFFSET,
        /// The write queue.
        Queue = QUEUE,
        /// Null packets left out.
        DropNull = TS_DROP_NULL,
        /// The largest file.
        MaxBytes = TS_MAX,
        /// The sidecar.
        Sidecar = TS_SIDECAR,
        /// The services.
        Services = SERVICES,
    }
}

/// The transport stream recorder (`record.ts`): packets in (`ts`).
pub struct TsRecorder {
    storage: Arc<dyn Storage>,
    clock: Clock,
    values: Values,
    services: Vec<u16>,
    run: Option<(TsFeed, TsControl)>,
    shown: Shown,
}

impl TsRecorder {
    /// A recorder that writes through `storage` and names files by `clock`. It records after the command start.
    pub fn new(storage: Arc<dyn Storage>, clock: Clock) -> Self {
        TsRecorder {
            storage,
            clock,
            values: Values::defaults(&TsRecorderSetting::schema()),
            services: Vec::new(),
            run: None,
            shown: Shown::new(),
        }
    }
}

/// Reads service ids separated by commas. The error is the byte position of the first id that does not read.
fn service_ids(text: &str) -> Result<Vec<u16>, usize> {
    let mut out = Vec::new();
    let mut at = 0;
    for part in text.split(',') {
        let t = part.trim();
        if !t.is_empty() {
            out.push(t.parse::<u16>().map_err(|_| at)?);
        }
        at += part.len() + 1;
    }
    Ok(out)
}

impl Component for TsRecorder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("record.ts", vec![stream::port("ts")])
            .with_settings(TsRecorderSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some((feed, control)) = &mut self.run {
            for p in io.items::<TsPacket>(0) {
                feed.push(&p.value);
            }
            self.shown.poll(&mut control.writer, &control.live);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TsRecorderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TsRecorderSetting::Services => {
                let text = SERVICES.read(id, v)?;
                self.services =
                    service_ids(text).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
            }
            TsRecorderSetting::Folder => service::check_folder(id, v)?,
            TsRecorderSetting::Prefix
            | TsRecorderSetting::UtcOffset
            | TsRecorderSetting::Queue
            | TsRecorderSetting::DropNull
            | TsRecorderSetting::MaxBytes
            | TsRecorderSetting::Sidecar => {}
        }
        self.values.set(id, v)?;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.values.clone()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.shown.p.results())
    }
    fn commands(&self) -> &Commands {
        RecorderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match RecorderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RecorderCommand::Start => {
                let spec = service::ts_spec(&self.values, self.services.clone()).map_err(failed)?;
                self.run = Some(
                    ts::start(spec, "", self.storage.clone(), self.clock.clone())
                        .map_err(failed)?,
                );
                self.shown = Shown::new();
            }
            // Dropping the feed closes its file and ends its writer, which finishes on its own thread.
            RecorderCommand::Stop => {
                self.run = None;
                self.shown.p.set(&STATE, RunState::Ending);
            }
        }
        Ok(None)
    }
}

sdrkit_core::settings! {
    /// The settings of the rewind component.
    pub enum RewindSetting {
        /// The folder of the saves.
        Folder = FOLDER,
        /// The start of file names.
        Prefix = PREFIX,
        /// The local time offset.
        UtcOffset = UTC_OFFSET,
        /// The memory of the history.
        Budget = REWIND_BUDGET,
        /// The centre history factor.
        LongFactor = REWIND_LONG,
        /// The memory of the centre history.
        LongBudget = REWIND_LONG_BUDGET,
    }
}

const C_SAVE: Command = Command::new("save", "Save")
    .help("Saves the last seconds and the seconds that follow as a SigMF recording.")
    .args(&[&A_PRE, &A_POST, &A_LONG, &A_REASON]);

sdrkit_core::commands! {
    /// The commands of the rewind component.
    pub enum RewindCommand {
        /// Saves the last seconds and the seconds that follow.
        Save = C_SAVE,
    }
}

const REWIND_S: Field<field::Duration> = Field::new("rewind_s", "Rewind held", "s");
const SAVES: Field<field::I64> = Field::new("saves", "Saves", "");

/// The rewind buffer (`record.rewind`): IQ in (`iq`). A retune empties it.
pub struct RewindRecorder {
    storage: Arc<dyn Storage>,
    clock: Clock,
    values: Values,
    feed: Option<(RewindFeed, RewindControl)>,
    format: (f64, f64),
    p: Published,
    saves: u64,
    next_id: u64,
}

impl RewindRecorder {
    /// A rewind buffer that saves through `storage` and names files by `clock`.
    pub fn new(storage: Arc<dyn Storage>, clock: Clock) -> Self {
        RewindRecorder {
            storage,
            clock,
            values: Values::defaults(&RewindSetting::schema()),
            feed: None,
            format: (0.0, 0.0),
            p: Published::new(&[&REWIND_S, &SAVES, &PROBLEM]),
            saves: 0,
            next_id: 1,
        }
    }

    fn make(&mut self) {
        let (mut feed, control) =
            rewind::start(service::rewind_spec(&self.values), self.storage.clone());
        if self.format.0 > 0.0 {
            feed.configure(self.format.0, self.format.1, "");
        }
        self.feed = Some((feed, control));
    }
}

impl Component for RewindRecorder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("record.rewind", vec![Port::iq("iq")])
            .with_settings(RewindSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.format = inputs.first().map_or((0.0, 0.0), |f| (f.rate, f.centre_hz));
        self.make();
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let Some((feed, control)) = &mut self.feed else {
            return;
        };
        feed.push(io.iq(0));
        for n in control.notes.try_iter() {
            match n {
                rewind::SaveNote::Done { .. } => self.saves += 1,
                rewind::SaveNote::Failed { error, .. } => self.p.set(&PROBLEM, Problem::of(&error)),
                rewind::SaveNote::Opened { .. } => {}
            }
        }
        let rate = control.live.rate().max(1.0);
        self.p.set(
            &REWIND_S,
            control.live.held.load(Ordering::Relaxed) as f64 / rate,
        );
        self.p.set(&SAVES, count(self.saves));
    }
    fn retuned(&mut self, centre_hz: f64, _tuned_hz: f64) {
        self.format.1 = centre_hz;
        if let Some((feed, _)) = &mut self.feed {
            feed.retuned(centre_hz);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let s = RewindSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        if s == RewindSetting::Folder {
            service::check_folder(id, v)?;
        }
        self.values.set(id, v)?;
        if matches!(
            s,
            RewindSetting::Budget | RewindSetting::LongFactor | RewindSetting::LongBudget
        ) && self.feed.is_some()
        {
            self.make();
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.values.clone()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn commands(&self) -> &Commands {
        RewindCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match RewindCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RewindCommand::Save => {
                let Some((_, control)) = &self.feed else {
                    return Err(failed(crate::error::RecordError::NothingHeld));
                };
                let order = rewind::SaveOrder {
                    id: self.next_id,
                    now_unix: self.clock.now(),
                    ..service::save_order(&self.values, args).map_err(failed)?
                };
                self.next_id += 1;
                rewind::order(control, order).map_err(failed)?;
            }
        }
        Ok(None)
    }
}

const PLAY_FILE: Setting<field::Text> = Setting::new("file", "File", "", "")
    .help("The transport stream file in the storage of the host.");

sdrkit_core::settings! {
    /// The settings of the transport stream player.
    pub enum TsPlaySetting {
        /// The file.
        File = PLAY_FILE,
    }
}

const A_PAUSED: Setting<field::Bool> = Setting::new("paused", "Paused", "", true);
const A_AT: Argument<field::Duration> = Argument::new(Field::new("at_s", "Position", "s"));
const C_PAUSE: Command = Command::new("pause", "Pause").args(&[&A_PAUSED]);
const C_SEEK: Command = Command::new("seek", "Seek")
    .help("Plays from this many seconds of stream time after the start.")
    .args(&[&A_AT]);

sdrkit_core::commands! {
    /// The commands of the transport stream player.
    pub enum TsPlayCommand {
        /// Pauses or resumes.
        Pause = C_PAUSE,
        /// Moves to a position.
        Seek = C_SEEK,
    }
}

const POSITION: Field<field::Duration> = Field::new("position_s", "Position", "s");
const PLAY_DURATION: Field<field::Duration> = Field::new("duration_s", "Length", "s");
const PAUSED: Field<field::Bool> = Field::new("paused", "Paused", "");
const ENDED: Field<field::Bool> = Field::new("ended", "Ended", "");

/// The transport stream player (`record.ts_play`): a `.ts` file in storage, played paced by its PCR on the wall clock, out as
/// packets (`ts`).
pub struct TsFilePlayer {
    storage: Arc<dyn Storage>,
    clock: Clock,
    values: Values,
    player: Option<TsPlayer<FileStore>>,
    wait_until: f64,
    batch: Vec<[u8; TS_SIZE]>,
    p: Published,
}

impl TsFilePlayer {
    /// A player that reads through `storage` and paces by `clock`.
    pub fn new(storage: Arc<dyn Storage>, clock: Clock) -> Self {
        TsFilePlayer {
            storage,
            clock,
            values: Values::defaults(&TsPlaySetting::schema()),
            player: None,
            wait_until: 0.0,
            batch: Vec::with_capacity(PLAY_BATCH),
            p: Published::new(&[&POSITION, &PLAY_DURATION, &PAUSED, &ENDED, &PROBLEM]),
        }
    }

    fn open(&mut self, text: &str) -> Result<(), SettingError> {
        self.player = None;
        if text.is_empty() {
            return Ok(());
        }
        let id = TsPlaySetting::File.id();
        let path = StorePath::new(text).map_err(|_| SettingError::Malformed { id, at: None })?;
        match FileStore::open(&*self.storage, &path)
            .and_then(|s| TsPlayer::new(s).map_err(Into::into))
        {
            Ok(mut p) => {
                self.p.opt(&PLAY_DURATION, p.duration());
                self.player = Some(p);
                self.p.set(&PROBLEM, Problem::None);
            }
            Err(e) => self.p.set(&PROBLEM, Problem::of(&e)),
        }
        Ok(())
    }
}

impl Component for TsFilePlayer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::generator("record.ts_play", vec![stream::port("ts")])
            .with_settings(TsPlaySetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(stream::kind(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(player) = &mut self.player else {
            return;
        };
        let now = self.clock.now();
        if now < self.wait_until {
            return;
        }
        self.batch.clear();
        let state = player.read(now, &mut self.batch, PLAY_BATCH);
        self.wait_until = match state {
            ReadState::WaitUntil(t) => t,
            ReadState::More | ReadState::Paused | ReadState::End => 0.0,
        };
        if !self.batch.is_empty() {
            // One shared block per call: the packets are slices of it.
            let data: Arc<[u8]> = self.batch.as_flattened().into();
            let out = io.items_out::<TsPacket>(0);
            for k in 0..self.batch.len() {
                if let Some(p) = TsPacket::new(Packet::slice(&data, k * TS_SIZE, TS_SIZE, 0.0)) {
                    out.push(sdrkit_graph::Timed {
                        time: 0.0,
                        value: p,
                    });
                }
            }
        }
        let s = player.status();
        self.p.set(&POSITION, s.position_s);
        self.p.set(&PAUSED, s.paused);
        self.p.set(&ENDED, state == ReadState::End);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TsPlaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TsPlaySetting::File => {
                let text = PLAY_FILE.read(id, v)?.to_string();
                self.open(&text)?;
            }
        }
        self.values.set(id, v)?;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.values.clone()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn commands(&self) -> &Commands {
        TsPlayCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let cmd = TsPlayCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        let Some(player) = &mut self.player else {
            return Err(CommandError::NotNow {
                field: sdrkit_core::FieldId(0),
                state: Value::None,
            });
        };
        match cmd {
            TsPlayCommand::Pause => {
                if args.get(&A_PAUSED) {
                    player.pause();
                } else {
                    player.resume();
                }
            }
            TsPlayCommand::Seek => player.seek(args.arg(&A_AT)?),
        }
        self.wait_until = 0.0;
        Ok(None)
    }
}
