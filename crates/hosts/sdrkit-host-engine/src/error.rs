//! The one error type of the engine. Each variant stands for one kind of failure, so a host can map it to one error code.

use crate::owners::OwnerRef;
use crate::RadioId;
use sdrkit_core::event::CatalogError;
use sdrkit_core::record::DeclError;
use sdrkit_core::settings::SchemaError;
use sdrkit_core::settings::ValuesError;
use sdrkit_core::storage::PathError;
use sdrkit_core::{
    CommandError, ConfigError, OwnerKeyError, RequestError, SettingError, SignalType, StateError,
    StorageError,
};
use sdrkit_driver_core::DriverError;
use sdrkit_graph::GraphError;

/// Why the engine refused a call.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The handle refers to no owner: it was never issued, or its radio or session is closed.
    #[error("there is no owner {0:?}")]
    NoSuchOwner(OwnerRef),
    /// No owner has this key.
    #[error("there is no owner with the key {key}")]
    NoSuchKey {
        /// The requested key.
        key: String,
    },
    /// The text is not an owner key.
    #[error(transparent)]
    BadKey(#[from] OwnerKeyError),
    /// No stack has this id.
    #[error("there is no stack {id}")]
    NoSuchStack {
        /// The requested id.
        id: String,
    },
    /// No driver kind has this key.
    #[error("there is no driver {key}")]
    NoSuchDriver {
        /// The requested key.
        key: String,
    },
    /// The radio refused or failed.
    #[error(transparent)]
    Driver(#[from] DriverError),
    /// A receiver refused its configuration.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// A graph did not build, connect or validate.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// A setting was refused. Made by [`EngineError::setting`], which has the declaration that names the setting.
    #[error("{text}")]
    Setting {
        /// The sentence, with the label and key of the setting.
        text: String,
        /// What was refused.
        error: SettingError,
    },
    /// A command was refused or failed. Made by [`EngineError::command`], which has the declarations that name the
    /// command and its arguments.
    #[error("{text}")]
    Command {
        /// The sentence, with the labels and keys of the command and its arguments.
        text: String,
        /// What was refused.
        error: CommandError,
    },
    /// The host's storage failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// The settings of a session do not make one schema.
    #[error(transparent)]
    Schema(#[from] SchemaError),
    /// The results of a session do not make one declaration.
    #[error(transparent)]
    Decl(#[from] DeclError),
    /// The event kinds do not make one catalog.
    #[error(transparent)]
    Events(#[from] CatalogError),
    /// The system refused to start a thread.
    #[error("the system refused a thread: {kind}")]
    Thread {
        /// What the system said.
        kind: std::io::ErrorKind,
    },
    /// The radio feeds sessions. Close them first.
    #[error("sessions read the radio; close them first (sessions: {sessions})")]
    RadioInUse {
        /// The number of sessions that read the radio.
        sessions: usize,
    },
    /// A radio of this address is open, and its driver opens an address once ([`Opens::Once`](sdrkit_driver_core::Opens)).
    /// A session starts on the open radio.
    #[error("the radio is open as {key}; start the session on it, or close it first")]
    RadioOpen {
        /// The open radio.
        radio: RadioId,
        /// The key of the open radio.
        key: String,
    },
    /// The radio has a share: a radio has one.
    #[error("the radio is shared as {key}")]
    ShareOpen {
        /// The share.
        share: crate::ShareId,
        /// Its key.
        key: String,
    },
    /// The session runs. Stop it first.
    #[error("the session still runs")]
    StillRunning,
    /// The session has stopped and takes no samples.
    #[error("the session has stopped")]
    Stopped,
    /// No host feed has this handle now.
    #[error("there is no host feed {feed:?}")]
    NoSuchFeed {
        /// The handle given.
        feed: crate::FeedId,
    },
    /// A session of the feed has not used the samples it read before. Nothing was written. Push again later.
    #[error("a session of the feed is behind")]
    Behind,
    /// Raw samples whose bytes end in a part of a sample. Nothing was taken.
    #[error("{bytes} bytes are not a whole number of samples")]
    PartSample {
        /// The bytes given.
        bytes: usize,
    },
    /// Host audio for a session has a channel count that is not 1 or 2.
    #[error("host audio of {got} channels: a session takes 1 or 2")]
    AudioChannels {
        /// The channel count given.
        got: u16,
    },
    /// The session's receiver is not a graph, so it has no ports to tap.
    #[error("the session's receiver has no graph")]
    NoGraph,
    /// The engine sends the session's transmit samples to its radio, so the host cannot read them.
    #[error("the engine sends the session's transmit samples to its radio")]
    ToRadio,
    /// The session is a channel of the bank: its audio goes to the mix of the bank's session, which the host reads.
    #[error("the session's audio goes to the mix of the bank")]
    InMix,
    /// The owner declares no series of this id.
    #[error("the owner has no series {series}")]
    NoSuchSeries {
        /// The id given.
        series: u16,
    },
    /// The owner declares no raster of this id.
    #[error("the owner has no raster {raster}")]
    NoSuchRaster {
        /// The id given.
        raster: u16,
    },
    /// The session has no tap of this id.
    #[error("the session has no tap {tap}")]
    NoSuchTap {
        /// The id given.
        tap: u32,
    },
    /// A file path of the host's storage is not valid.
    #[error(transparent)]
    Path(#[from] PathError),
    /// A saved state was refused.
    #[error(transparent)]
    State(#[from] StateError),
    /// The memory service failed.
    #[error(transparent)]
    Memory(#[from] sdrkit_svc_memory::MemoryError),
    /// The scan service failed.
    #[error(transparent)]
    Scan(#[from] sdrkit_svc_scan::ScanError),
    /// The frequency database failed.
    #[error(transparent)]
    Freqdb(#[from] sdrkit_svc_freqdb::DbError),
    /// The record service failed.
    #[error(transparent)]
    Record(#[from] sdrkit_svc_record::RecordError),
    /// The message store failed.
    #[error(transparent)]
    Store(#[from] sdrkit_svc_store::StoreError),
    /// The automation failed.
    #[error(transparent)]
    Automation(#[from] sdrkit_svc_automation::AutomationError),
    /// The outputs failed.
    #[error(transparent)]
    Outputs(#[from] sdrkit_svc_outputs::OutputError),
    /// The watch list failed.
    #[error(transparent)]
    Watch(#[from] sdrkit_svc_watch::WatchError),
    /// The sweep failed.
    #[error(transparent)]
    Sweep(#[from] sdrkit_svc_sweep::SweepError),
    /// The identify service failed.
    #[error(transparent)]
    Identify(#[from] sdrkit_svc_identify::IdentifyError),
    /// The channel bank failed.
    #[error(transparent)]
    Bank(#[from] sdrkit_svc_bank::BankError),
    /// A request is not one that can be carried out (a frequency below 0, a hop with no channels).
    #[error(transparent)]
    Request(#[from] RequestError),
    /// The engine's setting `requests_allowed` is off.
    #[error("requests are not allowed")]
    RequestsOff,
    /// No stack receives the signal type of a request.
    #[error("no stack receives {}", sdrkit_core::Choice::key(*.signal))]
    NoStackFor {
        /// The signal type.
        signal: SignalType,
    },
    /// A request refers to no running session (by its row key in the `sessions` table).
    #[error("there is no session {key}")]
    NoSuchSession {
        /// The row key given.
        key: String,
    },
    /// A release names a channel that is not followed.
    #[error("{freq_hz} Hz is not followed")]
    NotFollowed {
        /// The channel.
        freq_hz: f64,
    },
    /// A follow or a channel needs a session on a radio. A session that the host feeds cannot share its samples.
    #[error("the session does not read a radio")]
    NoRadio,
    /// A setting of a request belongs to an owner that the session does not run.
    #[error("the session has no setting {owner}/{key}")]
    NotInSession {
        /// The owner key of the setting.
        owner: String,
        /// The setting key.
        key: String,
    },
    /// The engine cannot carry out this kind of request.
    #[error("the engine cannot carry out a {} request", sdrkit_core::Choice::key(*.kind))]
    Unserved {
        /// The kind of request.
        kind: crate::RequestKind,
    },
    /// Another session steers the radio: it hops the radio, or reads it too. Only the one session that steers a radio
    /// hops it.
    #[error("another session steers the radio; only the one session that reads a radio hops it")]
    NotSteering,
    /// A saved state could not be written as JSON.
    #[error("the saved state could not be written")]
    Encode,
    /// Saved settings or a state file were refused. Nothing changed.
    #[error(transparent)]
    Import(#[from] ImportError),
    /// A request needs the session whose event set it off, and no session raised that event.
    #[error("the request has no session")]
    NoSession,
    /// The event catalog does not list an event that a request needs.
    #[error("the event kind is not listed")]
    Unlisted,
    /// The link to a network radio carries less than the least rate of the stack. The radio is left as it was.
    #[error(
        "{stack} needs {} Msps; the link to this radio carries about {} Msps",
        sdrkit_driver_core::fmt_m(*need_hz),
        sdrkit_driver_core::fmt_m(*carries_hz)
    )]
    LinkTooSlow {
        /// The name of the stack.
        stack: &'static str,
        /// The least rate of the stack, Hz.
        need_hz: f64,
        /// What the link carries, Hz.
        carries_hz: f64,
    },
    /// The channel bank reads one radio, and a channel names another one.
    #[error("the channel bank reads another radio than {radio}")]
    BankRadio {
        /// The row key of the radio that the channel names.
        radio: String,
    },
    /// A capture of the identify service needs exactly one open radio.
    #[error("a capture needs one open radio, and {radios} are open")]
    CaptureRadio {
        /// The number of open radios.
        radios: usize,
    },
    /// A saved state was not restored. What came before it in the state was restored.
    #[error("{item} was not restored: {source}")]
    Restore {
        /// The first thing that was not restored.
        item: RestoreItem,
        /// Why.
        source: Box<EngineError>,
    },
}

/// A thing of a saved state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestoreItem {
    /// The radio at this address.
    Radio {
        /// The address the radio was opened at.
        address: String,
    },
    /// The session at this place in the order of the saved sessions, from 0.
    Session {
        /// Its place.
        order: usize,
    },
    /// The service with this owner key.
    Service {
        /// The owner key.
        key: String,
    },
}

impl std::fmt::Display for RestoreItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestoreItem::Radio { address } => write!(f, "the radio at {address}"),
            RestoreItem::Session { order } => write!(f, "session {order}"),
            RestoreItem::Service { key } => write!(f, "the service {key}"),
        }
    }
}

/// Why a settings blob or a state file was refused.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// The state file is not JSON.
    #[error("not JSON at line {line}, column {column}")]
    NotJson {
        /// The line of the fault.
        line: usize,
        /// The column of the fault.
        column: usize,
    },
    /// The bytes are not a settings blob, or the state file has no version number.
    #[error("the bytes are not saved settings")]
    NotSettings,
    /// The bytes are of a version that this engine does not read: a later version of the library wrote them.
    #[error("saved settings of version {found} are not read")]
    Version {
        /// The version in the bytes.
        found: u64,
    },
    /// The settings blob is cut short, fails its checksum, or does not decode.
    #[error("the saved settings are damaged at byte {at}")]
    Damaged {
        /// The byte offset of the fault.
        at: usize,
    },
    /// The setting is never read from saved settings: `transmit_allowed` of the engine.
    #[error("{owner}: the setting {setting} is not read from saved settings")]
    NotSaved {
        /// The owner key.
        owner: String,
        /// The setting key.
        setting: String,
    },
    /// The owner refused a value.
    #[error("{owner}: {error}")]
    Value {
        /// The owner key.
        owner: String,
        /// The setting key, if the fault is of one setting.
        setting: Option<String>,
        /// What was refused.
        error: ValuesError,
    },
    /// The owner refused a value that its schema accepted.
    #[error("{owner}: {error}")]
    Apply {
        /// The owner key.
        owner: String,
        /// What was refused.
        error: SettingError,
    },
}

impl EngineError {
    /// `error` of a setting of `schema`, with the sentence that names the setting.
    pub fn setting(schema: &sdrkit_core::Schema, error: SettingError) -> Self {
        EngineError::Setting {
            text: sdrkit_core::owner::setting_error_text(schema, "setting", &error),
            error,
        }
    }
    /// Applies `value` to setting `id` of `owner` ([`sdrkit_core::owner::apply`]). A refusal names the setting.
    pub fn apply<O: sdrkit_core::Owner + ?Sized>(
        owner: &mut O,
        id: sdrkit_core::SettingId,
        value: &sdrkit_core::Value,
    ) -> Result<(), EngineError> {
        sdrkit_core::owner::apply(owner, id, value)
            .map_err(|e| EngineError::setting(owner.schema(), e))
    }
    /// The id of setting `key` of `schema`.
    pub fn id_of(
        schema: &sdrkit_core::Schema,
        key: &str,
    ) -> Result<sdrkit_core::SettingId, EngineError> {
        schema.id(key).map_err(|e| EngineError::setting(schema, e))
    }
    /// Sets slot `id` of `values`. A refusal names the setting.
    pub fn set(
        values: &mut sdrkit_core::Values,
        id: sdrkit_core::SettingId,
        value: &sdrkit_core::Value,
    ) -> Result<(), EngineError> {
        values
            .set(id, value)
            .map_err(|e| EngineError::setting(values.schema(), e))
    }
    /// Runs command `id` of `owner` with the arguments `given` ([`sdrkit_core::owner::invoke`]). A refusal names the
    /// command and its arguments.
    pub fn invoke<O: sdrkit_core::Owner + ?Sized>(
        owner: &mut O,
        id: sdrkit_core::CommandId,
        given: &sdrkit_core::Record,
    ) -> Result<Option<sdrkit_core::Record>, EngineError> {
        sdrkit_core::owner::invoke(owner, id, given).map_err(|e| EngineError::command(owner, id, e))
    }
    /// The error for a command key that `owner` does not have.
    pub fn no_command(key: &str) -> Self {
        let error = CommandError::Unknown {
            key: key.to_owned(),
        };
        EngineError::Command {
            text: error.to_string(),
            error,
        }
    }
    /// `error` of command `id` of `owner`, with the sentence that names the command and its arguments.
    pub fn command<O: sdrkit_core::Owner + ?Sized>(
        owner: &O,
        id: sdrkit_core::CommandId,
        error: CommandError,
    ) -> Self {
        EngineError::Command {
            text: sdrkit_core::owner::command_error_text(owner, id, &error),
            error,
        }
    }
    pub(crate) fn thread(e: &std::io::Error) -> Self {
        EngineError::Thread { kind: e.kind() }
    }
}

