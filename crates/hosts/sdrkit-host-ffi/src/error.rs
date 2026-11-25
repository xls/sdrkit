//! Error codes, the typed detail of the last error of an engine, and the one map from the engine's errors to codes.

use crate::handle::{run, Handle, Host, Out};
use crate::value::{type_code, Id, Str, SDRKIT_NO_ID, SDRKIT_TYPE_NONE};
use sdrkit_core::record::RecordError;
use sdrkit_core::{CommandError, ConfigError, SettingError, StorageError};
use sdrkit_driver_core::{DriverError, Unavailable};
use sdrkit_graph::{ConnectError, GraphError, Kind};
use sdrkit_host_engine::EngineError;
use std::ffi::{c_char, CStr};
use std::fmt::Write as _;

/// Success.
pub const SDRKIT_OK: i32 = 0;
/// A pointer that the call needs is NULL.
pub const SDRKIT_E_NULL_POINTER: i32 = 1;
/// A string is not UTF-8.
pub const SDRKIT_E_BAD_STRING: i32 = 2;
/// A constant argument or member is not one of its `SDRKIT_*` values.
pub const SDRKIT_E_BAD_ARGUMENT: i32 = 3;
/// The handle is of another type than the call takes, for example a graph where the call takes an object.
pub const SDRKIT_E_WRONG_HANDLE: i32 = 4;
/// The handle was freed, or its object has been removed.
pub const SDRKIT_E_NO_SUCH_OBJECT: i32 = 5;
/// No object, type, setting, command, table or port has the key.
pub const SDRKIT_E_NO_SUCH_KEY: i32 = 6;
/// The string is not an object key.
pub const SDRKIT_E_BAD_KEY: i32 = 7;
/// An id or an index is outside its list.
pub const SDRKIT_E_NO_SUCH_ID: i32 = 8;
/// The library failed inside. The engine stays usable.
pub const SDRKIT_E_PANIC: i32 = 9;
/// A value is of another type than its setting, field or argument.
pub const SDRKIT_E_WRONG_TYPE: i32 = 10;
/// A required value is missing.
pub const SDRKIT_E_REQUIRED: i32 = 11;
/// A number is outside the range of its setting.
pub const SDRKIT_E_OUT_OF_RANGE: i32 = 12;
/// A number is not finite.
pub const SDRKIT_E_NOT_FINITE: i32 = 13;
/// An option index is outside the options of its setting.
pub const SDRKIT_E_NOT_AN_OPTION: i32 = 14;
/// The object does not allow this value now.
pub const SDRKIT_E_NOT_PERMITTED: i32 = 15;
/// A string value does not have the form that its setting needs.
pub const SDRKIT_E_MALFORMED: i32 = 16;
/// The setting cannot change.
pub const SDRKIT_E_FIXED: i32 = 17;
/// The value conflicts with another setting.
pub const SDRKIT_E_CONFLICT: i32 = 18;
/// The object does not support the value.
pub const SDRKIT_E_UNSUPPORTED: i32 = 19;
/// The value needs a station setting of the engine first.
pub const SDRKIT_E_NEEDS_STATION: i32 = 20;
/// A required argument of a command is missing.
pub const SDRKIT_E_MISSING_ARGUMENT: i32 = 30;
/// The command has no such argument.
pub const SDRKIT_E_NOT_AN_ARGUMENT: i32 = 31;
/// The state of the object does not allow the command now.
pub const SDRKIT_E_NOT_NOW: i32 = 32;
/// The table has no row with the key.
pub const SDRKIT_E_NO_SUCH_ROW: i32 = 33;
/// The command failed.
pub const SDRKIT_E_COMMAND_FAILED: i32 = 34;
/// The command transmits, and the transmitter of the session is off: the engine setting `transmit_allowed` is off, or
/// the session has no radio that can transmit. Nothing was sent.
pub const SDRKIT_E_TRANSMIT_OFF: i32 = 35;
/// The file or folder does not exist.
pub const SDRKIT_E_NOT_FOUND: i32 = 40;
/// The file exists already.
pub const SDRKIT_E_ALREADY_EXISTS: i32 = 41;
/// The storage is full.
pub const SDRKIT_E_NO_SPACE: i32 = 42;
/// The storage refused the operation.
pub const SDRKIT_E_DENIED: i32 = 43;
/// The path is not a valid storage path.
pub const SDRKIT_E_INVALID_PATH: i32 = 44;
/// Another input or output error.
pub const SDRKIT_E_IO: i32 = 45;
/// The graph does not build.
pub const SDRKIT_E_GRAPH: i32 = 50;
/// A connection is refused. The error detail holds the components, the ports and the stream types.
pub const SDRKIT_E_CONNECT: i32 = 51;
/// A stack or a component refused its configuration (a rate, a bandwidth, a codec).
pub const SDRKIT_E_CONFIG: i32 = 52;
/// A stage refused its parameters.
pub const SDRKIT_E_STAGE: i32 = 53;
/// The radio failed or refused.
pub const SDRKIT_E_DRIVER: i32 = 60;
/// Sessions read the radio.
pub const SDRKIT_E_RADIO_IN_USE: i32 = 61;
/// The session runs. Stop it first.
pub const SDRKIT_E_STILL_RUNNING: i32 = 62;
/// The session is stopped.
pub const SDRKIT_E_STOPPED: i32 = 63;
/// A radio of this address is open, and its driver opens an address once. The engine table `radios` lists the open
/// radios with their addresses.
pub const SDRKIT_E_RADIO_OPEN: i32 = 64;
/// A session of the feed has not used the samples that it read before.
pub const SDRKIT_E_BEHIND: i32 = 65;
/// The bytes end in a part of a sample.
pub const SDRKIT_E_PART_SAMPLE: i32 = 66;
/// The session does not run a graph.
pub const SDRKIT_E_NO_GRAPH: i32 = 67;
/// The saved settings are refused: they are damaged, are not saved settings, or hold a refused value.
pub const SDRKIT_E_IMPORT: i32 = 68;
/// The saved state is refused.
pub const SDRKIT_E_STATE: i32 = 69;
/// A service failed.
pub const SDRKIT_E_SERVICE: i32 = 70;
/// The engine cannot serve a request of a service or a stack.
pub const SDRKIT_E_REQUEST: i32 = 71;
/// The system did not start a thread.
pub const SDRKIT_E_THREAD: i32 = 72;
/// A declaration of settings, results or events is not valid.
pub const SDRKIT_E_DECLARATION: i32 = 73;
/// The engine could not write its saved state.
pub const SDRKIT_E_ENCODE: i32 = 74;
/// A part of a saved state was not restored.
pub const SDRKIT_E_RESTORE: i32 = 76;
/// The engine sends the transmit samples of the session to its radio, so the caller cannot read them.
pub const SDRKIT_E_TO_RADIO: i32 = 77;
/// The session is a channel of the bank: its audio goes to the mix of the bank's session.
pub const SDRKIT_E_IN_MIX: i32 = 78;
/// A later version of the library wrote the saved settings or the saved state. This version does not read them.
pub const SDRKIT_E_VERSION: i32 = 79;
/// The radio cannot be used on this system for a reason that stays until the user changes something. The member
/// `index` of the error detail holds the reason, an `SDRKIT_UNAVAILABLE_*` constant.
pub const SDRKIT_E_UNAVAILABLE: i32 = 80;
/// The radio has a share: a radio has one. The engine table `shares` lists the shares.
pub const SDRKIT_E_SHARE_OPEN: i32 = 81;

/// The platform gives programs no USB access (iPhone, a web page), or the library was built without it.
pub const SDRKIT_UNAVAILABLE_NO_USB_HERE: i32 = 0;
/// The user may not open the device (Linux: no udev rule; Android: no permission from the system).
pub const SDRKIT_UNAVAILABLE_NO_PERMISSION: i32 = 1;
/// A driver of the operating system holds the device and cannot be detached.
pub const SDRKIT_UNAVAILABLE_KERNEL_DRIVER: i32 = 2;
/// Windows: the device is not bound to the WinUSB driver.
pub const SDRKIT_UNAVAILABLE_NEEDS_WINUSB: i32 = 3;
/// The device is not attached.
pub const SDRKIT_UNAVAILABLE_NOT_FOUND: i32 = 4;

/// No stream type, or the type is not known yet.
pub const SDRKIT_STREAM_NONE: i32 = -1;
/// Complex samples.
pub const SDRKIT_STREAM_IQ: i32 = 0;
/// Real samples.
pub const SDRKIT_STREAM_REAL: i32 = 1;
/// Audio.
pub const SDRKIT_STREAM_AUDIO: i32 = 2;
/// Symbols.
pub const SDRKIT_STREAM_SYMBOLS: i32 = 3;
/// Hard bits.
pub const SDRKIT_STREAM_HARD_BITS: i32 = 4;
/// Soft bits.
pub const SDRKIT_STREAM_LLR: i32 = 5;
/// Bytes.
pub const SDRKIT_STREAM_BYTES: i32 = 6;
/// Messages.
pub const SDRKIT_STREAM_MESSAGES: i32 = 7;
/// Spectrum frames.
pub const SDRKIT_STREAM_SPECTRUM: i32 = 8;
/// Typed items.
pub const SDRKIT_STREAM_ITEM: i32 = 9;

/// The `SDRKIT_STREAM_*` constant of `k`.
pub(crate) fn stream_type(k: Kind) -> i32 {
    match k {
        Kind::Iq => SDRKIT_STREAM_IQ,
        Kind::Real => SDRKIT_STREAM_REAL,
        Kind::Audio => SDRKIT_STREAM_AUDIO,
        Kind::Symbols => SDRKIT_STREAM_SYMBOLS,
        Kind::HardBits => SDRKIT_STREAM_HARD_BITS,
        Kind::Llr => SDRKIT_STREAM_LLR,
        Kind::Bytes => SDRKIT_STREAM_BYTES,
        Kind::Messages => SDRKIT_STREAM_MESSAGES,
        Kind::Spectrum => SDRKIT_STREAM_SPECTRUM,
        Kind::Item(_) => SDRKIT_STREAM_ITEM,
    }
}

/// The detail of the last error of an engine. A member that does not apply holds `SDRKIT_NO_ID` (ids), -1 (`index`), NaN
/// (measures), `SDRKIT_STREAM_NONE` or `SDRKIT_TYPE_NONE`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ErrorDetail {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The error code.
    pub code: i32,
    /// The setting, or the argument of a command.
    pub setting: Id,
    /// The command.
    pub command: Id,
    /// The result field.
    pub field: Id,
    /// The table.
    pub table: Id,
    /// A count or an index: an option index, the number of sessions that read a radio, a byte count, the error code of a
    /// device.
    pub index: i64,
    /// The name of the component at fault, as `sdrkit_graph_add` named it, or empty. For a refused connection, the
    /// component of the input.
    pub component: Str,
    /// The output of a refused connection, `"<component>.<port>"` or `"input"`, or empty.
    pub from: Str,
    /// The input of a refused connection, `"<component>.<port>"`, or empty.
    pub to: Str,
    /// The `SDRKIT_STREAM_*` type of the output.
    pub from_type: i32,
    /// The `SDRKIT_TYPE_*` type that the value must have.
    pub expected: i32,
    /// The `SDRKIT_TYPE_*` type of the value.
    pub got: i32,
    /// The lower limit.
    pub min: f64,
    /// The upper limit.
    pub max: f64,
    /// The number in the call.
    pub value: f64,
    /// For `SDRKIT_E_NO_SUCH_KEY` from the engine: the key, the stack id or the driver key that no object has. Empty
    /// otherwise.
    pub key: Str,
}

impl ErrorDetail {
    const fn new(code: i32) -> Self {
        ErrorDetail {
            size: 0,
            code,
            setting: SDRKIT_NO_ID,
            command: SDRKIT_NO_ID,
            field: SDRKIT_NO_ID,
            table: SDRKIT_NO_ID,
            index: -1,
            component: Str::EMPTY,
            from: Str::EMPTY,
            to: Str::EMPTY,
            from_type: SDRKIT_STREAM_NONE,
            expected: SDRKIT_TYPE_NONE,
            got: SDRKIT_TYPE_NONE,
            min: f64::NAN,
            max: f64::NAN,
            value: f64::NAN,
            key: Str::EMPTY,
        }
    }
}

/// The last error of an engine, or of the calls that take no handle.
pub(crate) struct LastError {
    /// The detail. Its string views point to the static empty string; `sdrkit_error_detail` points them to `names`.
    pub detail: ErrorDetail,
    /// The component, the output, the input and the key that the detail names.
    pub names: [String; 4],
    pub text: String,
}

// SAFETY: the only fields of `LastError` that are not `Send` are the string views of `detail`. They always point to the
// static empty string (`ErrorDetail::new`, `LastError::keep`), which every thread can read.
unsafe impl Send for LastError {}

impl Default for LastError {
    fn default() -> Self {
        LastError {
            detail: ErrorDetail::new(SDRKIT_OK),
            names: Default::default(),
            text: String::new(),
        }
    }
}

impl LastError {
    /// Keeps `err` and returns its code.
    fn keep(&mut self, err: &Error) -> i32 {
        let (d, names) = detail(err);
        self.detail = d;
        for (held, name) in self.names.iter_mut().zip(names) {
            held.clear();
            held.push_str(name);
        }
        self.text.clear();
        let _ = write!(self.text, "{err}");
        d.code
    }

    /// The detail with its string views in `buffers`, which the call copies the names into.
    pub fn view(&self, buffers: &mut [String; 4]) -> ErrorDetail {
        buffers.clone_from(&self.names);
        let mut d = self.detail;
        d.component = Str::of(&buffers[0]);
        d.from = Str::of(&buffers[1]);
        d.to = Str::of(&buffers[2]);
        d.key = Str::of(&buffers[3]);
        d
    }
}

/// The last error of the calls that take no handle: `sdrkit_engine_new`, `sdrkit_engine_new_with_storage`,
/// `sdrkit_free` and `sdrkit_version`.
pub(crate) static PROCESS_LAST: std::sync::Mutex<LastError> = std::sync::Mutex::new(LastError {
    detail: ErrorDetail::new(SDRKIT_OK),
    names: [String::new(), String::new(), String::new(), String::new()],
    text: String::new(),
});

thread_local! {
    /// The buffers behind the views of `sdrkit_error_detail` and `sdrkit_error_message` on handle 0.
    static PROCESS_VIEWS: std::cell::RefCell<([String; 4], String)> = std::cell::RefCell::default();
}

/// Why a call failed.
#[derive(Debug)]
pub(crate) enum Error {
    Engine(EngineError),
    Null,
    BadString,
    BadArgument,
    NoSuchId,
    NotAnOption {
        index: u32,
    },
    WrongHandle,
    WrongType,
    Record(RecordError),
    Storage(StorageError),
    /// The answer is not known yet: the calibration of the device has not finished.
    NotYet,
    Panic,
}

impl Error {
    /// The code alone.
    pub fn code(&self) -> i32 {
        detail(self).0.code
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Engine(e) => write!(f, "{e}"),
            Error::Null => f.write_str("a pointer is NULL"),
            Error::BadString => f.write_str("the string is not UTF-8"),
            Error::BadArgument => f.write_str("a constant is not one of its values"),
            Error::NoSuchId => f.write_str("an id or index is outside its list"),
            Error::NotAnOption { index } => write!(f, "option {index} does not exist"),
            Error::WrongHandle => f.write_str("the handle is of another type"),
            Error::WrongType => f.write_str("the value is of another type"),
            Error::Record(e) => write!(f, "{e}"),
            Error::Storage(e) => write!(f, "{e}"),
            Error::NotYet => f.write_str("the calibration of the device has not finished"),
            Error::Panic => f.write_str("the library failed inside"),
        }
    }
}

impl Host {
    /// Keeps `err` as the last error and returns its code.
    pub fn fail(&self, err: &Error) -> i32 {
        crate::handle::lock(&self.last).keep(err)
    }
}

/// Keeps `err` as the last error of the calls that take no handle and returns its code.
pub(crate) fn fail_process(err: &Error) -> i32 {
    crate::handle::lock(&PROCESS_LAST).keep(err)
}

/// The names that the detail of `err` holds: the component, the output, the input and the key.
type Names<'a> = [&'a str; 4];

/// The code, the detail and the names of `err`.
fn detail(err: &Error) -> (ErrorDetail, Names<'_>) {
    let mut d = ErrorDetail::new(SDRKIT_OK);
    let mut names = [""; 4];
    d.code = match err {
        Error::Engine(e) => engine(e, &mut d, &mut names),
        Error::Null => SDRKIT_E_NULL_POINTER,
        Error::BadString => SDRKIT_E_BAD_STRING,
        Error::BadArgument => SDRKIT_E_BAD_ARGUMENT,
        Error::NoSuchId => SDRKIT_E_NO_SUCH_ID,
        Error::NotAnOption { index } => {
            d.index = i64::from(*index);
            SDRKIT_E_NOT_AN_OPTION
        }
        Error::WrongHandle => SDRKIT_E_WRONG_HANDLE,
        Error::WrongType => SDRKIT_E_WRONG_TYPE,
        Error::Record(e) => record(e, &mut d),
        Error::Storage(e) => storage(*e),
        Error::NotYet => SDRKIT_E_NOT_NOW,
        Error::Panic => SDRKIT_E_PANIC,
    };
    (d, names)
}

/// The one map from [`EngineError`] to a code, with the detail written into `d`.
pub(crate) fn engine<'a>(e: &'a EngineError, d: &mut ErrorDetail, names: &mut Names<'a>) -> i32 {
    match e {
        EngineError::NoSuchOwner(_) => SDRKIT_E_NO_SUCH_OBJECT,
        EngineError::NoSuchKey { key } | EngineError::NoSuchStack { id: key } | EngineError::NoSuchDriver { key } => {
            names[3] = key;
            SDRKIT_E_NO_SUCH_KEY
        }
        EngineError::BadKey(_) => SDRKIT_E_BAD_KEY,
        EngineError::Driver(e) => driver(e, d),
        EngineError::Config(e) => config(e, d),
        EngineError::Graph(e) => graph(e, d, names),
        EngineError::Setting { error, .. } => setting(error, d),
        EngineError::Command { error, .. } => command(error, d),
        EngineError::Storage(e) => storage(*e),
        EngineError::Schema(_) | EngineError::Decl(_) | EngineError::Events(_) => {
            SDRKIT_E_DECLARATION
        }
        EngineError::Thread { .. } => SDRKIT_E_THREAD,
        EngineError::RadioInUse { sessions } => {
            d.index = count(*sessions);
            SDRKIT_E_RADIO_IN_USE
        }
        EngineError::RadioOpen { .. } => SDRKIT_E_RADIO_OPEN,
        EngineError::ShareOpen { key, .. } => {
            names[3] = key;
            SDRKIT_E_SHARE_OPEN
        }
        EngineError::StillRunning => SDRKIT_E_STILL_RUNNING,
        EngineError::Stopped => SDRKIT_E_STOPPED,
        EngineError::NoSuchFeed { .. } => SDRKIT_E_NO_SUCH_OBJECT,
        EngineError::Behind => SDRKIT_E_BEHIND,
        EngineError::PartSample { bytes } => {
            d.index = count(*bytes);
            SDRKIT_E_PART_SAMPLE
        }
        EngineError::AudioChannels { got } => {
            d.value = f64::from(*got);
            SDRKIT_E_UNSUPPORTED
        }
        EngineError::NoGraph => SDRKIT_E_NO_GRAPH,
        EngineError::ToRadio => SDRKIT_E_TO_RADIO,
        EngineError::InMix => SDRKIT_E_IN_MIX,
        EngineError::NoSuchSeries { series: i } | EngineError::NoSuchRaster { raster: i } => {
            d.index = i64::from(*i);
            SDRKIT_E_NO_SUCH_ID
        }
        EngineError::NoSuchTap { tap } => {
            d.index = i64::from(*tap);
            SDRKIT_E_NO_SUCH_ID
        }
        EngineError::Path(_) => SDRKIT_E_INVALID_PATH,
        EngineError::State(_) => SDRKIT_E_STATE,
        EngineError::Memory(_)
        | EngineError::Scan(_)
        | EngineError::Freqdb(_)
        | EngineError::Record(_)
        | EngineError::Store(_)
        | EngineError::Automation(_)
        | EngineError::Outputs(_)
        | EngineError::Watch(_)
        | EngineError::Sweep(_)
        | EngineError::Identify(_)
        | EngineError::Bank(_) => SDRKIT_E_SERVICE,
        EngineError::Request(_)
        | EngineError::RequestsOff
        | EngineError::NoStackFor { .. }
        | EngineError::NoSuchSession { .. }
        | EngineError::NotFollowed { .. }
        | EngineError::NoRadio
        | EngineError::NotInSession { .. }
        | EngineError::Unserved { .. }
        | EngineError::NoSession
        | EngineError::Unlisted
        | EngineError::BankRadio { .. } => SDRKIT_E_REQUEST,
        EngineError::CaptureRadio { radios } => {
            d.index = count(*radios);
            SDRKIT_E_REQUEST
        }
        EngineError::Encode => SDRKIT_E_ENCODE,
        EngineError::Import(sdrkit_host_engine::ImportError::Version { .. }) => SDRKIT_E_VERSION,
        EngineError::Import(e) => {
            if let sdrkit_host_engine::ImportError::Apply { error, .. } = e {
                setting(error, d);
            }
            SDRKIT_E_IMPORT
        }
        EngineError::Restore { .. } => SDRKIT_E_RESTORE,
    }
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn setting(e: &SettingError, d: &mut ErrorDetail) -> i32 {
    let mut at = |id: sdrkit_core::SettingId| d.setting = Id::from(id.0);
    match e {
        SettingError::Unknown { .. } => SDRKIT_E_NO_SUCH_KEY,
        SettingError::NoSuchId { id } => {
            at(*id);
            SDRKIT_E_NO_SUCH_ID
        }
        SettingError::WrongKind { id, expected, got } => {
            at(*id);
            d.expected = type_code(*expected);
            d.got = type_code(*got);
            SDRKIT_E_WRONG_TYPE
        }
        SettingError::Required { id } => {
            at(*id);
            SDRKIT_E_REQUIRED
        }
        SettingError::OutOfRange { id, min, max, got } => {
            at(*id);
            d.min = *min;
            d.max = *max;
            d.value = *got;
            SDRKIT_E_OUT_OF_RANGE
        }
        SettingError::NotFinite { id } => {
            at(*id);
            SDRKIT_E_NOT_FINITE
        }
        SettingError::NotAChoice { id, index } => {
            at(*id);
            d.index = i64::from(*index);
            SDRKIT_E_NOT_AN_OPTION
        }
        SettingError::NotPermitted { id } => {
            at(*id);
            SDRKIT_E_NOT_PERMITTED
        }
        SettingError::Malformed { id, at: pos } => {
            at(*id);
            d.index = pos.map_or(-1, count);
            SDRKIT_E_MALFORMED
        }
        SettingError::Fixed { id } => {
            at(*id);
            SDRKIT_E_FIXED
        }
        SettingError::Conflict { id, with } => {
            at(*id);
            d.index = i64::from(with.0);
            SDRKIT_E_CONFLICT
        }
        SettingError::Unsupported { id } => {
            at(*id);
            SDRKIT_E_UNSUPPORTED
        }
        SettingError::NeedsStation { id, .. } => {
            at(*id);
            SDRKIT_E_NEEDS_STATION
        }
        SettingError::Storage { id, source, .. } => {
            at(*id);
            storage(*source)
        }
        SettingError::Config { id, source } => {
            at(*id);
            config(source, d)
        }
    }
}

fn command(e: &CommandError, d: &mut ErrorDetail) -> i32 {
    match e {
        CommandError::Unknown { .. } | CommandError::NoSuchTable { .. } => SDRKIT_E_NO_SUCH_KEY,
        CommandError::NoSuchId { id } => {
            d.command = Id::from(id.0);
            SDRKIT_E_NO_SUCH_ID
        }
        CommandError::ArgumentsDecl { id } => {
            d.command = Id::from(id.0);
            SDRKIT_E_BAD_ARGUMENT
        }
        CommandError::MissingArgument { arg } => {
            d.setting = Id::from(arg.0);
            SDRKIT_E_MISSING_ARGUMENT
        }
        CommandError::NotAnArgument { .. } => SDRKIT_E_NOT_AN_ARGUMENT,
        CommandError::BadArgument { arg, error } => {
            let c = setting(error, d);
            d.setting = Id::from(arg.0);
            c
        }
        CommandError::NotNow { field, .. } => {
            d.field = Id::from(field.0);
            SDRKIT_E_NOT_NOW
        }
        CommandError::NoSuchRow { table, .. } => {
            d.table = Id::from(table.0);
            SDRKIT_E_NO_SUCH_ROW
        }
        CommandError::NoSuchKey { arg } => {
            d.setting = Id::from(arg.0);
            SDRKIT_E_NO_SUCH_KEY
        }
        CommandError::TransmitOff => SDRKIT_E_TRANSMIT_OFF,
        CommandError::Failed { .. } => SDRKIT_E_COMMAND_FAILED,
    }
}

pub(crate) fn storage(e: StorageError) -> i32 {
    match e {
        StorageError::NotFound => SDRKIT_E_NOT_FOUND,
        StorageError::AlreadyExists => SDRKIT_E_ALREADY_EXISTS,
        StorageError::NoSpace => SDRKIT_E_NO_SPACE,
        StorageError::Denied => SDRKIT_E_DENIED,
        StorageError::InvalidPath(_) => SDRKIT_E_INVALID_PATH,
        StorageError::Io { .. } => SDRKIT_E_IO,
    }
}

fn config(e: &ConfigError, d: &mut ErrorDetail) -> i32 {
    match e {
        ConfigError::RateTooLow { need: limit, got }
        | ConfigError::WrongRate { need: limit, got } => {
            d.min = *limit;
            d.value = *got;
            SDRKIT_E_CONFIG
        }
        ConfigError::RateTooHigh { max, got } => {
            d.max = *max;
            d.value = *got;
            SDRKIT_E_CONFIG
        }
        ConfigError::RateNotMultiple { got, .. } | ConfigError::BandwidthNotSupported { got } => {
            d.value = *got;
            SDRKIT_E_CONFIG
        }
        ConfigError::NoRate | ConfigError::MissingCodec { .. } | ConfigError::BadData { .. } => {
            SDRKIT_E_CONFIG
        }
        ConfigError::Setting(e) => setting(e, d),
        ConfigError::SettingAtRate { id, rate_hz } => {
            d.setting = Id::from(id.0);
            d.value = *rate_hz;
            SDRKIT_E_CONFIG
        }
        ConfigError::Data { source, .. } => storage(*source),
        ConfigError::Stage(_) => SDRKIT_E_STAGE,
    }
}

fn driver(e: &DriverError, d: &mut ErrorDetail) -> i32 {
    match e {
        DriverError::Refused { id } => {
            d.setting = Id::from(id.0);
            SDRKIT_E_DRIVER
        }
        DriverError::Device { code } => {
            d.index = i64::from(*code);
            SDRKIT_E_DRIVER
        }
        DriverError::Setting(e) => setting(e, d),
        DriverError::Storage { source, .. } => storage(*source),
        DriverError::Unavailable { why } => {
            d.index = i64::from(match why {
                Unavailable::NoUsbHere => SDRKIT_UNAVAILABLE_NO_USB_HERE,
                Unavailable::NoPermission => SDRKIT_UNAVAILABLE_NO_PERMISSION,
                Unavailable::KernelDriver => SDRKIT_UNAVAILABLE_KERNEL_DRIVER,
                Unavailable::NeedsWinUsb => SDRKIT_UNAVAILABLE_NEEDS_WINUSB,
                Unavailable::NotFound => SDRKIT_UNAVAILABLE_NOT_FOUND,
            });
            SDRKIT_E_UNAVAILABLE
        }
        _ => SDRKIT_E_DRIVER,
    }
}

fn graph<'a>(e: &'a GraphError, d: &mut ErrorDetail, names: &mut Names<'a>) -> i32 {
    match e {
        GraphError::Description { error, .. } => graph(error, d, names),
        GraphError::Connect { from, to, error } => {
            names[1] = from;
            names[2] = to;
            names[0] = to.split_once('.').map_or(to.as_str(), |(c, _)| c);
            connect(error, d)
        }
        GraphError::Setting { error, .. } => setting(error, d),
        GraphError::Config { component, error } => {
            names[0] = component;
            config(error, d)
        }
        GraphError::UnknownComponent(component) => {
            names[0] = component;
            SDRKIT_E_NO_SUCH_KEY
        }
        GraphError::UnknownPort { component, .. } | GraphError::AmbiguousPort { component, .. } => {
            names[0] = component;
            SDRKIT_E_NO_SUCH_KEY
        }
        GraphError::Role { component, .. }
        | GraphError::OutputFormat { component, .. }
        | GraphError::InputMedia { component, .. }
        | GraphError::OutputCount { component, .. }
        | GraphError::OutputKind { component, .. }
        | GraphError::OutputNotDecoded { component, .. }
        | GraphError::Stopped { component, .. } => {
            names[0] = component;
            SDRKIT_E_GRAPH
        }
        _ => SDRKIT_E_GRAPH,
    }
}

/// The code of `e`, with the stream type of a missing conversion in `d`.
fn connect(e: &ConnectError, d: &mut ErrorDetail) -> i32 {
    match e {
        ConnectError::NoConversion { produces, .. } => {
            d.from_type = stream_type(*produces);
            SDRKIT_E_CONNECT
        }
        ConnectError::NoSuchPort { .. } | ConnectError::NoSuchComponent { .. } => {
            SDRKIT_E_NO_SUCH_KEY
        }
        _ => SDRKIT_E_CONNECT,
    }
}

fn record(e: &RecordError, d: &mut ErrorDetail) -> i32 {
    match e {
        RecordError::NoSuchField { id } => {
            d.setting = Id::from(id.0);
            SDRKIT_E_NO_SUCH_ID
        }
        RecordError::WrongKind { id, expected, got } => {
            d.setting = Id::from(id.0);
            d.expected = type_code(*expected);
            d.got = type_code(*got);
            SDRKIT_E_WRONG_TYPE
        }
        RecordError::NotAChoice { id, index } => {
            d.setting = Id::from(id.0);
            d.index = i64::from(*index);
            SDRKIT_E_NOT_AN_OPTION
        }
    }
}

/// Returns the name of the error code `code`, such as `"SDRKIT_E_NO_SUCH_OBJECT"`, as a static NUL-terminated string. Returns
/// an empty string if `code` is not an error code. Never returns NULL.
#[no_mangle]
pub extern "C" fn sdrkit_code_name(code: i32) -> *const c_char {
    let name: &'static CStr = match code {
        SDRKIT_OK => c"SDRKIT_OK",
        SDRKIT_E_NULL_POINTER => c"SDRKIT_E_NULL_POINTER",
        SDRKIT_E_BAD_STRING => c"SDRKIT_E_BAD_STRING",
        SDRKIT_E_BAD_ARGUMENT => c"SDRKIT_E_BAD_ARGUMENT",
        SDRKIT_E_WRONG_HANDLE => c"SDRKIT_E_WRONG_HANDLE",
        SDRKIT_E_NO_SUCH_OBJECT => c"SDRKIT_E_NO_SUCH_OBJECT",
        SDRKIT_E_NO_SUCH_KEY => c"SDRKIT_E_NO_SUCH_KEY",
        SDRKIT_E_BAD_KEY => c"SDRKIT_E_BAD_KEY",
        SDRKIT_E_NO_SUCH_ID => c"SDRKIT_E_NO_SUCH_ID",
        SDRKIT_E_PANIC => c"SDRKIT_E_PANIC",
        SDRKIT_E_WRONG_TYPE => c"SDRKIT_E_WRONG_TYPE",
        SDRKIT_E_REQUIRED => c"SDRKIT_E_REQUIRED",
        SDRKIT_E_OUT_OF_RANGE => c"SDRKIT_E_OUT_OF_RANGE",
        SDRKIT_E_NOT_FINITE => c"SDRKIT_E_NOT_FINITE",
        SDRKIT_E_NOT_AN_OPTION => c"SDRKIT_E_NOT_AN_OPTION",
        SDRKIT_E_NOT_PERMITTED => c"SDRKIT_E_NOT_PERMITTED",
        SDRKIT_E_MALFORMED => c"SDRKIT_E_MALFORMED",
        SDRKIT_E_FIXED => c"SDRKIT_E_FIXED",
        SDRKIT_E_CONFLICT => c"SDRKIT_E_CONFLICT",
        SDRKIT_E_UNSUPPORTED => c"SDRKIT_E_UNSUPPORTED",
        SDRKIT_E_NEEDS_STATION => c"SDRKIT_E_NEEDS_STATION",
        SDRKIT_E_MISSING_ARGUMENT => c"SDRKIT_E_MISSING_ARGUMENT",
        SDRKIT_E_NOT_AN_ARGUMENT => c"SDRKIT_E_NOT_AN_ARGUMENT",
        SDRKIT_E_NOT_NOW => c"SDRKIT_E_NOT_NOW",
        SDRKIT_E_NO_SUCH_ROW => c"SDRKIT_E_NO_SUCH_ROW",
        SDRKIT_E_COMMAND_FAILED => c"SDRKIT_E_COMMAND_FAILED",
        SDRKIT_E_TRANSMIT_OFF => c"SDRKIT_E_TRANSMIT_OFF",
        SDRKIT_E_NOT_FOUND => c"SDRKIT_E_NOT_FOUND",
        SDRKIT_E_ALREADY_EXISTS => c"SDRKIT_E_ALREADY_EXISTS",
        SDRKIT_E_NO_SPACE => c"SDRKIT_E_NO_SPACE",
        SDRKIT_E_DENIED => c"SDRKIT_E_DENIED",
        SDRKIT_E_INVALID_PATH => c"SDRKIT_E_INVALID_PATH",
        SDRKIT_E_IO => c"SDRKIT_E_IO",
        SDRKIT_E_GRAPH => c"SDRKIT_E_GRAPH",
        SDRKIT_E_CONNECT => c"SDRKIT_E_CONNECT",
        SDRKIT_E_CONFIG => c"SDRKIT_E_CONFIG",
        SDRKIT_E_STAGE => c"SDRKIT_E_STAGE",
        SDRKIT_E_DRIVER => c"SDRKIT_E_DRIVER",
        SDRKIT_E_RADIO_IN_USE => c"SDRKIT_E_RADIO_IN_USE",
        SDRKIT_E_STILL_RUNNING => c"SDRKIT_E_STILL_RUNNING",
        SDRKIT_E_STOPPED => c"SDRKIT_E_STOPPED",
        SDRKIT_E_RADIO_OPEN => c"SDRKIT_E_RADIO_OPEN",
        SDRKIT_E_BEHIND => c"SDRKIT_E_BEHIND",
        SDRKIT_E_PART_SAMPLE => c"SDRKIT_E_PART_SAMPLE",
        SDRKIT_E_NO_GRAPH => c"SDRKIT_E_NO_GRAPH",
        SDRKIT_E_IMPORT => c"SDRKIT_E_IMPORT",
        SDRKIT_E_STATE => c"SDRKIT_E_STATE",
        SDRKIT_E_SERVICE => c"SDRKIT_E_SERVICE",
        SDRKIT_E_REQUEST => c"SDRKIT_E_REQUEST",
        SDRKIT_E_THREAD => c"SDRKIT_E_THREAD",
        SDRKIT_E_DECLARATION => c"SDRKIT_E_DECLARATION",
        SDRKIT_E_ENCODE => c"SDRKIT_E_ENCODE",
        SDRKIT_E_RESTORE => c"SDRKIT_E_RESTORE",
        SDRKIT_E_TO_RADIO => c"SDRKIT_E_TO_RADIO",
        SDRKIT_E_IN_MIX => c"SDRKIT_E_IN_MIX",
        SDRKIT_E_VERSION => c"SDRKIT_E_VERSION",
        SDRKIT_E_UNAVAILABLE => c"SDRKIT_E_UNAVAILABLE",
        SDRKIT_E_SHARE_OPEN => c"SDRKIT_E_SHARE_OPEN",
        _ => c"",
    };
    name.as_ptr()
}

/// Writes the code of the last error of the engine of `handle` to `code`: `SDRKIT_OK` if no call failed. If `handle` is 0,
/// writes the code of the last error of the calls that take no handle.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_error_code(handle: Handle, code: *mut i32) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(code) };
    if handle == 0 {
        return match out {
            Ok(out) => {
                out.put(crate::handle::lock(&PROCESS_LAST).detail.code);
                SDRKIT_OK
            }
            Err(e) => e.code(),
        };
    }
    run(handle, |e| {
        out?.put(crate::handle::lock(&e.host.last).detail.code);
        Ok(())
    })
}

/// Fills `detail` with the detail of the last error of the engine of `handle`. If `handle` is 0, fills it with the detail
/// of the last error of the calls that take no handle (`sdrkit_engine_new`); its strings are then valid until the next
/// call on handle 0 in the same thread.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_error_detail(handle: Handle, detail: *mut ErrorDetail) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(detail) };
    if handle == 0 {
        return match out {
            Ok(out) => {
                let last = crate::handle::lock(&PROCESS_LAST);
                PROCESS_VIEWS.with_borrow_mut(|(names, _)| out.put(last.view(names)));
                SDRKIT_OK
            }
            Err(e) => e.code(),
        };
    }
    run(handle, |e| {
        let out = out?;
        let mut st = e.state();
        out.put(crate::handle::lock(&e.host.last).view(&mut st.error_names));
        Ok(())
    })
}

/// Writes the last error of the engine of `handle` to `message` as one English sentence, for a log. If `handle` is 0,
/// writes the last error of the calls that take no handle (`sdrkit_engine_new`); the string is then valid until the next
/// call on handle 0 in the same thread.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_error_message(handle: Handle, message: *mut Str) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(message) };
    if handle == 0 {
        return match out {
            Ok(out) => {
                let last = crate::handle::lock(&PROCESS_LAST);
                PROCESS_VIEWS.with_borrow_mut(|(_, text)| {
                    text.clone_from(&last.text);
                    out.put(Str::of(text));
                });
                SDRKIT_OK
            }
            Err(e) => e.code(),
        };
    }
    run(handle, |e| {
        let out = out?;
        let mut st = e.state();
        st.string
            .clone_from(&crate::handle::lock(&e.host.last).text);
        out.put(Str::of(&st.string));
        Ok(())
    })
}

crate::handle::sized_structs!(ErrorDetail);
