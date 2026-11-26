//! Safe Rust over the C API: handles that free themselves, errors with their code and message, and values copied out of
//! the views of the library.

use crate::sys;
use std::ffi::{c_char, CStr, CString};
use std::fmt;
use std::path::Path;

/// The id of a setting, an argument, a field, a column, a table, a command or an event type.
pub type Id = sys::sdrkit_id;

/// An error code of the library with the message that the engine wrote for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    /// The `SDRKIT_E_*` code.
    pub code: i32,
    /// One sentence from the engine, or a fixed sentence if the engine cannot say more.
    pub message: String,
}

impl Error {
    /// The name of the code, such as `SDRKIT_E_OUT_OF_RANGE`.
    pub fn name(&self) -> &'static str {
        code_name(self.code)
    }

    /// True if the handle was freed or its object has been removed.
    pub fn is_dead_handle(&self) -> bool {
        self.code == sys::SDRKIT_E_NO_SUCH_OBJECT
    }

    /// Makes the error of `code` with the message of the last error of the engine of `handle`, or of the calls without a
    /// handle if `handle` is 0.
    fn of(handle: sys::SDRKIT_HANDLE, code: i32) -> Error {
        let mut message = String::new();
        if code == sys::SDRKIT_E_NO_SUCH_OBJECT && handle != 0 {
            message.push_str("the handle is closed, or its object no longer exists");
        } else {
            let mut s = EMPTY;
            // SAFETY: `s` is a valid out parameter; the view is copied before the next call on the handle.
            if unsafe { sys::sdrkit_error_message(handle, &mut s) } == sys::SDRKIT_OK {
                copy_string(s, &mut message);
            }
        }
        Error { code, message }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.message.is_empty() {
            f.write_str(self.name())
        } else {
            write!(f, "{}: {}", self.name(), self.message)
        }
    }
}

impl std::error::Error for Error {}

/// The result of a call of the C API.
pub type Result<T> = std::result::Result<T, Error>;

/// Returns the name of the error code `code`, or an empty string if `code` is not an error code.
pub fn code_name(code: i32) -> &'static str {
    // SAFETY: the library returns a static NUL-terminated string and never NULL.
    let name = unsafe { CStr::from_ptr(sys::sdrkit_code_name(code)) };
    name.to_str().unwrap_or("")
}

const EMPTY: sys::sdrkit_string = sys::sdrkit_string {
    ptr: std::ptr::null(),
    len: 0,
};

const NO_BYTES: sys::sdrkit_bytes = sys::sdrkit_bytes {
    ptr: std::ptr::null(),
    len: 0,
};

const ZERO_VARIANT: sys::sdrkit_variant = sys::sdrkit_variant {
    r#type: sys::SDRKIT_TYPE_NONE,
    flag: false,
    enum_index: 0,
    integer: 0,
    number: 0.0,
    lat: 0.0,
    lon: 0.0,
    string: EMPTY,
    bytes: NO_BYTES,
    flags: 0,
};

/// Copies the string view `s` into `out`, replacing what `out` held. Bytes that are not UTF-8 become U+FFFD.
fn copy_string(s: sys::sdrkit_string, out: &mut String) {
    out.clear();
    if s.ptr.is_null() || s.len == 0 {
        return;
    }
    // SAFETY: the library returned `len` readable bytes at `ptr`, valid until the next call on the handle.
    let bytes = unsafe { std::slice::from_raw_parts(s.ptr.cast::<u8>(), s.len) };
    out.push_str(&String::from_utf8_lossy(bytes));
}

/// Returns the string view `s` as an owned string.
fn owned(s: sys::sdrkit_string) -> String {
    let mut out = String::new();
    copy_string(s, &mut out);
    out
}

/// Copies the byte view `b` into `out`, replacing what `out` held.
fn copy_bytes(b: sys::sdrkit_bytes, out: &mut Vec<u8>) {
    out.clear();
    if b.ptr.is_null() || b.len == 0 {
        return;
    }
    // SAFETY: the library returned `len` readable bytes at `ptr`, valid until the next call on the handle.
    out.extend_from_slice(unsafe { std::slice::from_raw_parts(b.ptr, b.len) });
}

/// Makes a NUL-terminated copy of `s`. Returns `SDRKIT_E_BAD_STRING` if `s` holds a NUL.
fn c_string(s: &str) -> Result<CString> {
    CString::new(s).map_err(|_| Error {
        code: sys::SDRKIT_E_BAD_STRING,
        message: "the string holds a NUL character".into(),
    })
}

/// Returns the version of the library, `major.minor.patch`.
///
/// # Errors
///
/// The code of the library.
pub fn version() -> Result<String> {
    let mut s = EMPTY;
    // SAFETY: `s` is a valid out parameter.
    let code = unsafe { sys::sdrkit_version(&mut s) };
    check(0, code)?;
    Ok(owned(s))
}

/// An open data folder of the presets part of the library. Dropping it closes the handle.
#[derive(Debug)]
pub struct Presets(sys::SDRKIT_HANDLE);

impl Drop for Presets {
    fn drop(&mut self) {
        sys::sdrkit_presets_close(self.0);
    }
}

impl Presets {
    /// Opens the data folder `folder`.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NOT_FOUND` for a folder that does not exist, `SDRKIT_E_MALFORMED` for a plan file that does not have
    /// its format, with the sentence of the library.
    pub fn open(folder: &Path) -> Result<Presets> {
        let path = c_string(&folder.to_string_lossy())?;
        let mut h = 0;
        // SAFETY: `path` is NUL-terminated and `h` is a valid out parameter.
        let code = unsafe { sys::sdrkit_presets_open(path.as_ptr(), &mut h) };
        if code != sys::SDRKIT_OK {
            return Err(Presets(0).error(code));
        }
        Ok(Presets(h))
    }

    /// The error of `code` with the sentence of the last error of the handle.
    fn error(&self, code: i32) -> Error {
        let mut m = EMPTY;
        // SAFETY: `m` is a valid out parameter, and the view is copied before the next call.
        let message = match unsafe { sys::sdrkit_presets_error_message(self.0, &mut m) } {
            sys::SDRKIT_OK => owned(m),
            _ => String::new(),
        };
        Error { code, message }
    }

    /// The settings that the data has for `stack` in the country `country` and the city `city`, on the frequency
    /// `freq_hz` (else that of the data) in the submode `submode`, as values of the settings of the stack type in
    /// `engine`, by setting id. Empty if the data has nothing for the stack.
    ///
    /// # Errors
    ///
    /// The code and the sentence of the library: a stack or a setting that the engine does not have, or a value that
    /// does not fit its setting.
    pub fn settings(
        &self,
        engine: &Handle,
        stack: &str,
        country: &str,
        city: Option<&str>,
        freq_hz: Option<f64>,
        submode: Option<&str>,
    ) -> Result<Vec<(Id, Value)>> {
        let (stack, country) = (c_string(stack)?, c_string(country)?);
        let city = city.map(c_string).transpose()?;
        let submode = submode.map(c_string).transpose()?;
        let query = sys::PresetsQuery {
            stack: stack.as_ptr(),
            country: country.as_ptr(),
            city: city.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            submode: submode.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            has_frequency_hz: freq_hz.is_some(),
            frequency_hz: freq_hz.unwrap_or(0.0),
            ..sys::PresetsQuery::default()
        };
        let mut n = 0;
        // SAFETY: the strings of `query` are NUL-terminated and live to the end of the call; `n` is a valid out
        // parameter.
        match unsafe { sys::sdrkit_presets_settings(self.0, engine.0, &query, &mut n) } {
            sys::SDRKIT_OK => {}
            sys::SDRKIT_E_NOT_FOUND => return Ok(Vec::new()),
            code => return Err(self.error(code)),
        }
        (0..n)
            .map(|i| {
                let mut s = sys::PresetsSetting::default();
                // SAFETY: `s` is a valid out parameter, and its value is copied before the next call.
                match unsafe { sys::sdrkit_presets_setting_read(self.0, i, &mut s) } {
                    sys::SDRKIT_OK => {
                        let mut v = Value::None;
                        v.copy_from(&s.arg.value);
                        Ok((s.arg.id, v))
                    }
                    code => Err(self.error(code)),
                }
            })
            .collect()
    }
}

/// Returns `Ok` for `SDRKIT_OK`, else the error of `code` with the message of the engine of `handle`.
fn check(handle: sys::SDRKIT_HANDLE, code: i32) -> Result<()> {
    if code == sys::SDRKIT_OK {
        Ok(())
    } else {
        Err(Error::of(handle, code))
    }
}

/// The type of a value or a declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Type {
    /// No value.
    #[default]
    None,
    /// Yes or no.
    Bool,
    /// A whole number.
    I64,
    /// A number.
    F64,
    /// A string.
    String,
    /// A frequency in Hz.
    Hz,
    /// Seconds since 1970 (UTC).
    Time,
    /// A length of time in seconds.
    Duration,
    /// A position in degrees.
    Position,
    /// One of the options of the declaration.
    Enum,
    /// Raw bytes.
    Bytes,
    /// Any type: each value carries its own.
    Any,
}

impl Type {
    /// Returns the type of the `SDRKIT_TYPE_*` constant `raw`, or [`Type::None`] for a constant that this application
    /// does not know.
    fn from_raw(raw: i32) -> Type {
        match raw {
            sys::SDRKIT_TYPE_BOOL => Type::Bool,
            sys::SDRKIT_TYPE_I64 => Type::I64,
            sys::SDRKIT_TYPE_F64 => Type::F64,
            sys::SDRKIT_TYPE_STRING => Type::String,
            sys::SDRKIT_TYPE_HZ => Type::Hz,
            sys::SDRKIT_TYPE_TIME => Type::Time,
            sys::SDRKIT_TYPE_DURATION => Type::Duration,
            sys::SDRKIT_TYPE_POSITION => Type::Position,
            sys::SDRKIT_TYPE_ENUM => Type::Enum,
            sys::SDRKIT_TYPE_BYTES => Type::Bytes,
            sys::SDRKIT_TYPE_ANY => Type::Any,
            _ => Type::None,
        }
    }

    /// Returns a short name for a column of types.
    pub fn name(self) -> &'static str {
        match self {
            Type::None => "none",
            Type::Bool => "bool",
            Type::I64 => "integer",
            Type::F64 => "number",
            Type::String => "string",
            Type::Hz => "frequency",
            Type::Time => "time",
            Type::Duration => "duration",
            Type::Position => "position",
            Type::Enum => "enum",
            Type::Bytes => "bytes",
            Type::Any => "any",
        }
    }
}

/// One value, copied out of an `sdrkit_variant`.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
    /// No value.
    #[default]
    None,
    /// Yes or no.
    Bool(bool),
    /// A whole number.
    I64(i64),
    /// A number.
    F64(f64),
    /// A string.
    String(String),
    /// A frequency in Hz.
    Hz(f64),
    /// Seconds since 1970 (UTC).
    Time(f64),
    /// A length of time in seconds.
    Duration(f64),
    /// A position: degrees north, degrees east.
    Position(f64, f64),
    /// The index of an option.
    Enum(u32),
    /// Raw bytes.
    Bytes(Vec<u8>),
}

impl Value {
    /// Returns the type of the value.
    pub fn type_of(&self) -> Type {
        match self {
            Value::None => Type::None,
            Value::Bool(_) => Type::Bool,
            Value::I64(_) => Type::I64,
            Value::F64(_) => Type::F64,
            Value::String(_) => Type::String,
            Value::Hz(_) => Type::Hz,
            Value::Time(_) => Type::Time,
            Value::Duration(_) => Type::Duration,
            Value::Position(..) => Type::Position,
            Value::Enum(_) => Type::Enum,
            Value::Bytes(_) => Type::Bytes,
        }
    }

    /// Copies `v` into the value. A string or bytes value keeps its buffer if the value held the same type.
    fn copy_from(&mut self, v: &sys::sdrkit_variant) {
        match Type::from_raw(v.r#type) {
            Type::String => {
                if !matches!(self, Value::String(_)) {
                    *self = Value::String(String::new());
                }
                if let Value::String(s) = self {
                    copy_string(v.string, s);
                }
            }
            Type::Bytes => {
                if !matches!(self, Value::Bytes(_)) {
                    *self = Value::Bytes(Vec::new());
                }
                if let Value::Bytes(b) = self {
                    copy_bytes(v.bytes, b);
                }
            }
            Type::Bool => *self = Value::Bool(v.flag),
            Type::I64 => *self = Value::I64(v.integer),
            Type::F64 => *self = Value::F64(v.number),
            Type::Hz => *self = Value::Hz(v.number),
            Type::Time => *self = Value::Time(v.number),
            Type::Duration => *self = Value::Duration(v.number),
            Type::Position => *self = Value::Position(v.lat, v.lon),
            Type::Enum => *self = Value::Enum(v.enum_index),
            Type::None | Type::Any => *self = Value::None,
        }
    }

    /// Returns the variant of the value. Its string and bytes point into `self`.
    fn variant(&self) -> sys::sdrkit_variant {
        let mut v = ZERO_VARIANT;
        match self {
            Value::None => {}
            Value::Bool(b) => {
                v.r#type = sys::SDRKIT_TYPE_BOOL;
                v.flag = *b;
            }
            Value::I64(i) => {
                v.r#type = sys::SDRKIT_TYPE_I64;
                v.integer = *i;
            }
            Value::F64(x) => {
                v.r#type = sys::SDRKIT_TYPE_F64;
                v.number = *x;
            }
            Value::String(s) => {
                v.r#type = sys::SDRKIT_TYPE_STRING;
                v.string = sys::sdrkit_string {
                    ptr: s.as_ptr().cast::<c_char>(),
                    len: s.len(),
                };
            }
            Value::Hz(x) => {
                v.r#type = sys::SDRKIT_TYPE_HZ;
                v.number = *x;
            }
            Value::Time(x) => {
                v.r#type = sys::SDRKIT_TYPE_TIME;
                v.number = *x;
            }
            Value::Duration(x) => {
                v.r#type = sys::SDRKIT_TYPE_DURATION;
                v.number = *x;
            }
            Value::Position(lat, lon) => {
                v.r#type = sys::SDRKIT_TYPE_POSITION;
                v.lat = *lat;
                v.lon = *lon;
            }
            Value::Enum(i) => {
                v.r#type = sys::SDRKIT_TYPE_ENUM;
                v.enum_index = *i;
            }
            Value::Bytes(b) => {
                v.r#type = sys::SDRKIT_TYPE_BYTES;
                v.bytes = sys::sdrkit_bytes {
                    ptr: b.as_ptr(),
                    len: b.len(),
                };
            }
        }
        v
    }
}

/// One option of a value of type [`Type::Enum`].
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Opt {
    /// The key that data files use.
    pub key: String,
    /// The name for people.
    pub label: String,
}

/// The declaration of a setting, an argument, a reply field, a result field, a column or an event field.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Decl {
    /// The key.
    pub key: String,
    /// The name for people.
    pub label: String,
    /// One or two sentences for people, or empty.
    pub help: String,
    /// The section that a setting shows in, or empty.
    pub section: String,
    /// The unit, or empty.
    pub unit: String,
    /// The type.
    pub ty: Type,
    /// The lowest number, the highest number and the step (0 for any number), if the value has a range.
    pub range: Option<(f64, f64, f64)>,
    /// The value must be given.
    pub required: bool,
    /// A change applies after a restart of the object.
    pub restart: bool,
    /// The value cannot change.
    pub fixed: bool,
    /// The setting is advanced: it is shown on request.
    pub advanced: bool,
    /// The default value of a setting or an argument.
    pub default: Value,
    /// What the special number ([`Decl::special_value`]) means when it is not a quantity ("Automatic"), or empty.
    pub zero_means: String,
    /// The number that `zero_means` names: 0, or another.
    pub special_value: f64,
    /// The options of a value of type [`Type::Enum`].
    pub options: Vec<Opt>,
    /// For a value that holds a row key: the key of the object of the table, or empty for the same object.
    pub refers_object: String,
    /// For a value that holds a row key: the key of the table that the keys belong to, or empty.
    pub refers_table: String,
    /// The column of `refers_table` that holds a value to pass, or empty. With a column, the value is free: a row of
    /// the table only offers one.
    pub refers_column: String,
}

impl Decl {
    /// Copies the declaration `d`. The options are read after.
    fn from_raw(d: &sys::sdrkit_decl) -> Decl {
        let mut default = Value::None;
        default.copy_from(&d.default_value);
        Decl {
            key: owned(d.key),
            label: owned(d.label),
            help: owned(d.help),
            section: owned(d.section),
            unit: owned(d.unit),
            ty: Type::from_raw(d.r#type),
            range: (d.flags & sys::SDRKIT_DECL_RANGE != 0).then_some((d.min, d.max, d.step)),
            required: d.flags & sys::SDRKIT_DECL_REQUIRED != 0,
            restart: d.flags & sys::SDRKIT_DECL_RESTART != 0,
            fixed: d.flags & sys::SDRKIT_DECL_FIXED != 0,
            advanced: d.flags & sys::SDRKIT_DECL_ADVANCED != 0,
            default,
            zero_means: owned(d.zero_means),
            special_value: d.special_value,
            options: Vec::new(),
            refers_object: owned(d.refers_object),
            refers_table: owned(d.refers_table),
            refers_column: owned(d.refers_column),
        }
    }
}

/// The declaration of one command.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CommandDecl {
    /// The id.
    pub id: Id,
    /// The key.
    pub key: String,
    /// The name for people.
    pub label: String,
    /// One or two sentences for people.
    pub help: String,
    /// The arguments, by id.
    pub args: Vec<Decl>,
    /// The reply fields, by id.
    pub reply: Vec<Decl>,
}

/// The declaration of one table.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TableDecl {
    /// The key.
    pub key: String,
    /// The name for people.
    pub label: String,
    /// The columns, by id.
    pub columns: Vec<Decl>,
    /// The most rows that the table keeps.
    pub cap: usize,
    /// The column that names a row for people, if the table states one.
    pub label_column: Option<usize>,
}

/// The declaration of the results of an object.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ResultsDecl {
    /// The result fields, by id.
    pub fields: Vec<Decl>,
    /// The tables, by id.
    pub tables: Vec<TableDecl>,
}

/// One event type of the catalog of the engine.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EventType {
    /// The key.
    pub key: String,
    /// The name for people.
    pub label: String,
    /// The fields, by id.
    pub fields: Vec<Decl>,
}

/// The state of an object in a snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum State {
    /// The object does not run.
    #[default]
    Off,
    /// The object looks for a signal.
    Searching,
    /// The object has a signal and does not decode it yet.
    Tracking,
    /// The object decodes the signal.
    Locked,
}

/// What `snapshot_read` wrote besides the summary.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SnapshotInfo {
    /// The change counter to pass as `since` next time.
    pub counter: u64,
    /// The declaration changed and the snapshot holds everything.
    pub declaration_changed: bool,
    /// A field changed and the snapshot holds every field.
    pub fields_changed: bool,
    /// The state.
    pub state: State,
    /// The signal to noise ratio in dB, or NaN.
    pub snr_db: f32,
    /// The level in dB full scale, or NaN.
    pub level_dbfs: f32,
    /// The offset from the tuned frequency in Hz, or NaN.
    pub offset_hz: f64,
    /// The number of log lines in the snapshot.
    pub log_lines: u32,
}

/// The rows of one table in a snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct RowsInfo {
    /// The number of rows that changed, newest first.
    pub rows: u32,
    /// The number of rows in the table.
    pub total: u32,
}

/// One event of the last take.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct EventInfo {
    /// The id of its event type.
    pub ty: Id,
    /// Seconds since the object started.
    pub time: f64,
    /// The frequency of the signal in Hz.
    pub freq_hz: f64,
    /// The number of fields.
    pub fields: u32,
}

/// The audio that `audio_read` copied.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct AudioInfo {
    /// The number of frames copied, or 0.
    pub frames: usize,
    /// Frames per second.
    pub rate_hz: f64,
    /// Samples per frame.
    pub channels: u16,
}

/// The spectrum frame that `spectrum_read` copied.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SpectrumInfo {
    /// The width of the frame in Hz.
    pub span_hz: f64,
    /// The frequency at the middle bin.
    pub centre_hz: f64,
}

/// A handle of an engine, of an object of an engine, or of a session. Dropping it frees the handle; dropping the engine
/// handle stops every session of the engine.
#[derive(Debug, PartialEq, Eq)]
pub struct Handle(sys::SDRKIT_HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        sys::sdrkit_free(self.0);
    }
}

impl Handle {
    /// Makes an engine that keeps its files in `folder`.
    ///
    /// # Errors
    ///
    /// A storage code if the folder cannot be made.
    pub fn engine_in(folder: &Path) -> Result<Handle> {
        let path = c_string(&folder.to_string_lossy())?;
        let mut h = 0;
        // SAFETY: `path` is NUL-terminated and `h` is a valid out parameter.
        let code = unsafe { sys::sdrkit_engine_new(path.as_ptr(), &mut h) };
        check(0, code)?;
        Ok(Handle(h))
    }

    /// Makes an engine that keeps its files in memory. They are lost when the engine is freed.
    ///
    /// # Errors
    ///
    /// The code of the library.
    pub fn engine_in_memory() -> Result<Handle> {
        let mut h = 0;
        // SAFETY: a NULL folder selects memory storage, and `h` is a valid out parameter.
        let code = unsafe { sys::sdrkit_engine_new(std::ptr::null(), &mut h) };
        check(0, code)?;
        Ok(Handle(h))
    }

    /// Gives the engine the plain values that the data folder `folder` has for the country `country`: the channels of
    /// a scan from the plans that the country uses, and the frequency entries. A folder without frequency entries
    /// gives none.
    ///
    /// # Errors
    ///
    /// The code of the presets call that failed, with its sentence: `SDRKIT_E_NOT_FOUND` for a folder that does not
    /// exist, `SDRKIT_E_MALFORMED` for a plan file that does not have its format.
    pub fn load_presets(&self, folder: &Path, country: &str) -> Result<()> {
        let presets = Presets::open(folder)?;
        let country = c_string(country)?;
        let load = || -> std::result::Result<(), i32> {
            let ok = |code: i32| if code == sys::SDRKIT_OK { Ok(()) } else { Err(code) };
            let mut n = 0;
            // SAFETY: `country` is NUL-terminated and `n` is a valid out parameter.
            ok(unsafe { sys::sdrkit_presets_plan_ids(presets.0, country.as_ptr(), &mut n) })?;
            let mut ids = Vec::new();
            for i in 0..n {
                let mut id = EMPTY;
                // SAFETY: `id` is a valid out parameter, and the view is copied before the next call.
                ok(unsafe { sys::sdrkit_presets_plan_id(presets.0, i, &mut id) })?;
                ids.push(CString::new(owned(id)).map_err(|_| sys::SDRKIT_E_BAD_STRING)?);
            }
            let list: Vec<*const std::ffi::c_char> = ids.iter().map(|s| s.as_ptr()).collect();
            // SAFETY: `list` holds `list.len()` NUL-terminated strings that live to the end of the call.
            ok(unsafe {
                sys::sdrkit_presets_scan_channels_set(
                    presets.0,
                    self.0,
                    list.as_ptr(),
                    list.len(),
                    std::ptr::null_mut(),
                )
            })?;
            // SAFETY: the count can be NULL.
            match unsafe {
                sys::sdrkit_presets_frequency_entries_set(presets.0, self.0, std::ptr::null_mut())
            } {
                sys::SDRKIT_OK | sys::SDRKIT_E_NOT_FOUND => Ok(()),
                code => Err(code),
            }
        };
        load().map_err(|code| presets.error(code))
    }

    /// Returns the number of the handle.
    pub fn raw(&self) -> u64 {
        self.0
    }

    fn check(&self, code: i32) -> Result<()> {
        check(self.0, code)
    }

    /// Returns a new handle of the object with the key `key`.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_BAD_KEY`, `SDRKIT_E_NO_SUCH_KEY`.
    pub fn find(&self, key: &str) -> Result<Handle> {
        let k = c_string(key)?;
        let mut h = 0;
        // SAFETY: `k` is NUL-terminated and `h` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_object_find(self.0, k.as_ptr(), &mut h) })?;
        Ok(Handle(h))
    }

    /// Returns the key of the object, empty for the stack of a session.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_OBJECT`.
    pub fn key(&self) -> Result<String> {
        let mut s = EMPTY;
        // SAFETY: `s` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_object_key(self.0, &mut s) })?;
        Ok(owned(s))
    }

    /// Returns the `SDRKIT_OBJECT_*` type of the object.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_OBJECT`.
    pub fn object_type(&self) -> Result<i32> {
        let mut t = 0;
        // SAFETY: `t` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_object_type(self.0, &mut t) })?;
        Ok(t)
    }

    /// Returns the keys and the `SDRKIT_OBJECT_*` types of the live objects of the engine.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_OBJECT`.
    pub fn objects(&self) -> Result<Vec<(String, i32)>> {
        let mut count = 0;
        // SAFETY: `count` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_objects(self.0, &mut count) })?;
        let mut out = Vec::with_capacity(count as usize);
        for n in 0..count {
            let (mut s, mut t) = (EMPTY, 0);
            // SAFETY: `s` and `t` are valid out parameters.
            self.check(unsafe { sys::sdrkit_object(self.0, n, &mut s, &mut t) })?;
            out.push((owned(s), t));
        }
        Ok(out)
    }

    /// Opens the radio at `address` with the values `settings` of the settings of `driver.<scheme>`.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_KEY` for an unknown driver, a setting code, `SDRKIT_E_DRIVER`.
    pub fn radio_open(&self, address: &str, settings: &[(Id, Value)]) -> Result<Handle> {
        let a = c_string(address)?;
        let args: Vec<sys::sdrkit_arg> = args(settings);
        let options = sys::sdrkit_radio_options {
            settings: args.as_ptr(),
            count: args.len(),
            ..Default::default()
        };
        let mut h = 0;
        // SAFETY: `a` is NUL-terminated, `options` points at `args`, whose views point into `settings`, all alive here.
        self.check(unsafe { sys::sdrkit_radio_open(self.0, a.as_ptr(), &options, &mut h) })?;
        Ok(Handle(h))
    }

    /// Closes the radio of this handle.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_RADIO_IN_USE` while sessions read it, `SDRKIT_E_NO_SUCH_OBJECT`.
    pub fn radio_close(&self) -> Result<()> {
        self.check(sys::sdrkit_radio_close(self.0))
    }

    /// Starts a session of the stack `stack` on the radio `radio`, in the submode `submode` (`None` for the default) and
    /// with the values `settings` of the settings of `stack.<id>` over the defaults. The engine tunes the session to
    /// `centre_hz`, or to the stack's default frequency if it is `None`.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_KEY` for an unknown stack, a setting code, `SDRKIT_E_CONFIG`, `SDRKIT_E_NO_SUCH_OBJECT`.
    pub fn stack_start(
        &self,
        stack: &str,
        submode: Option<&str>,
        centre_hz: Option<f64>,
        settings: &[(Id, Value)],
        radio: &Handle,
    ) -> Result<Handle> {
        let s = c_string(stack)?;
        let m = submode.map(c_string).transpose()?;
        let args: Vec<sys::sdrkit_arg> = args(settings);
        let options = sys::sdrkit_stack_options {
            submode: m.as_ref().map_or(std::ptr::null(), |m| m.as_ptr()),
            settings: args.as_ptr(),
            count: args.len(),
            centre_hz: centre_hz.unwrap_or(0.0),
            ..Default::default()
        };
        let input = sys::sdrkit_input_radio(radio.0);
        let mut h = 0;
        // SAFETY: `s` and `m` are NUL-terminated and alive here, `options` points at `args`, whose views point into
        // `settings`, and `input` is valid.
        let code = unsafe { sys::sdrkit_stack_start(self.0, s.as_ptr(), &options, &input, &mut h) };
        self.check(code)?;
        Ok(Handle(h))
    }

    /// Returns a new handle of the stack of this session.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_WRONG_HANDLE` if the handle is not a session.
    pub fn stack(&self) -> Result<Handle> {
        let mut h = 0;
        // SAFETY: `h` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_object_stack(self.0, &mut h) })?;
        Ok(Handle(h))
    }

    /// Stops this session.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_OBJECT`, `SDRKIT_E_WRONG_HANDLE`.
    pub fn session_stop(&self) -> Result<()> {
        self.check(sys::sdrkit_session_stop(self.0))
    }

    /// Closes this stopped session.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_STILL_RUNNING` until the session has stopped.
    pub fn session_close(&self) -> Result<()> {
        self.check(sys::sdrkit_session_close(self.0))
    }

    /// Reads a declaration with `decl` and its options with `option`, both called on this handle.
    fn decl(
        &self,
        decl: impl Fn(*mut sys::sdrkit_decl) -> i32,
        option: impl Fn(u32, *mut sys::sdrkit_option) -> i32,
    ) -> Result<Decl> {
        let mut raw = sys::sdrkit_decl::default();
        self.check(decl(&mut raw))?;
        let mut d = Decl::from_raw(&raw);
        for n in 0..raw.options {
            let mut o = sys::sdrkit_option::default();
            self.check(option(n, &mut o))?;
            d.options.push(Opt {
                key: owned(o.key),
                label: owned(o.label),
            });
        }
        Ok(d)
    }

    /// Returns the declarations of the settings, by id.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_OBJECT`.
    pub fn settings(&self) -> Result<Vec<Decl>> {
        let mut count = 0;
        // SAFETY: `count` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_settings_count(self.0, &mut count) })?;
        (0..count)
            .map(|id| {
                self.decl(
                    // SAFETY: `d` points at a live `sdrkit_decl`.
                    |d| unsafe { sys::sdrkit_setting_decl(self.0, id, d) },
                    // SAFETY: `o` points at a live `sdrkit_option`.
                    |n, o| unsafe { sys::sdrkit_setting_option(self.0, id, n, o) },
                )
            })
            .collect()
    }

    /// Returns the id of the setting with the key `key`.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_KEY`.
    pub fn setting_find(&self, key: &str) -> Result<Id> {
        let k = c_string(key)?;
        let mut id = 0;
        // SAFETY: `k` is NUL-terminated and `id` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_setting_find(self.0, k.as_ptr(), &mut id) })?;
        Ok(id)
    }

    /// Copies the value of setting `id` into `out`.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_ID`.
    pub fn get(&self, id: Id, out: &mut Value) -> Result<()> {
        let mut v = ZERO_VARIANT;
        // SAFETY: `v` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_setting_get(self.0, id, &mut v) })?;
        out.copy_from(&v);
        Ok(())
    }

    /// Sets setting `id` to `value`. The library checks the type, the range and the options.
    ///
    /// # Errors
    ///
    /// A setting code such as `SDRKIT_E_WRONG_TYPE` or `SDRKIT_E_OUT_OF_RANGE`.
    pub fn set(&self, id: Id, value: &Value) -> Result<()> {
        let v = value.variant();
        // SAFETY: the views of `v` point into `value`, which outlives the call.
        self.check(unsafe { sys::sdrkit_setting_set(self.0, id, &v) })
    }

    /// Returns the declarations of the commands with their arguments and reply fields, by id.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_OBJECT`.
    pub fn commands(&self) -> Result<Vec<CommandDecl>> {
        let mut count = 0;
        // SAFETY: `count` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_commands_count(self.0, &mut count) })?;
        let mut out = Vec::with_capacity(count as usize);
        for id in 0..count {
            let mut c = sys::sdrkit_command_info::default();
            // SAFETY: `c` is a valid out parameter.
            self.check(unsafe { sys::sdrkit_command_decl(self.0, id, &mut c) })?;
            let mut cmd = CommandDecl {
                id,
                key: owned(c.key),
                label: owned(c.label),
                help: owned(c.help),
                args: Vec::new(),
                reply: Vec::new(),
            };
            for a in 0..c.arguments {
                cmd.args.push(self.decl(
                    // SAFETY: `d` points at a live `sdrkit_decl`.
                    |d| unsafe { sys::sdrkit_argument_decl(self.0, id, a, d) },
                    // SAFETY: `o` points at a live `sdrkit_option`.
                    |n, o| unsafe { sys::sdrkit_argument_option(self.0, id, a, n, o) },
                )?);
            }
            for f in 0..c.reply_fields {
                cmd.reply.push(self.decl(
                    // SAFETY: `d` points at a live `sdrkit_decl`.
                    |d| unsafe { sys::sdrkit_reply_field_decl(self.0, id, f, d) },
                    // SAFETY: `o` points at a live `sdrkit_option`.
                    |n, o| unsafe { sys::sdrkit_reply_field_option(self.0, id, f, n, o) },
                )?);
            }
            out.push(cmd);
        }
        Ok(out)
    }

    /// Returns the id of the command with the key `key`.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_KEY`.
    pub fn command_find(&self, key: &str) -> Result<Id> {
        let k = c_string(key)?;
        let mut id = 0;
        // SAFETY: `k` is NUL-terminated and `id` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_command_find(self.0, k.as_ptr(), &mut id) })?;
        Ok(id)
    }

    /// Runs command `id` with the arguments `args` and returns the reply fields. An argument that is not in `args` has
    /// its default value.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_MISSING_ARGUMENT`, a setting code for a refused argument, `SDRKIT_E_NOT_NOW`, `SDRKIT_E_NO_SUCH_ROW`,
    /// `SDRKIT_E_COMMAND_FAILED`.
    pub fn invoke(&self, id: Id, arguments: &[(Id, Value)]) -> Result<Vec<Value>> {
        let a = args(arguments);
        let mut fields = 0;
        // SAFETY: `a` holds `a.len()` arguments whose views point into `arguments`, alive for the call.
        let code =
            unsafe { sys::sdrkit_command_invoke(self.0, id, a.as_ptr(), a.len(), &mut fields) };
        self.check(code)?;
        let mut reply = vec![Value::None; fields as usize];
        for (n, slot) in (0..fields).zip(reply.iter_mut()) {
            let mut v = ZERO_VARIANT;
            // SAFETY: `v` is a valid out parameter.
            self.check(unsafe { sys::sdrkit_reply(self.0, n, &mut v) })?;
            slot.copy_from(&v);
        }
        Ok(reply)
    }

    /// Returns the declaration of the results: the fields and the tables with their columns.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_OBJECT`.
    pub fn results_decl(&self) -> Result<ResultsDecl> {
        let mut info = sys::sdrkit_results_info::default();
        // SAFETY: `info` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_results_decl(self.0, &mut info) })?;
        let mut out = ResultsDecl::default();
        for id in 0..info.fields {
            out.fields.push(self.decl(
                // SAFETY: `d` points at a live `sdrkit_decl`.
                |d| unsafe { sys::sdrkit_field_decl(self.0, id, d) },
                // SAFETY: `o` points at a live `sdrkit_option`.
                |n, o| unsafe { sys::sdrkit_field_option(self.0, id, n, o) },
            )?);
        }
        for t in 0..info.tables {
            let mut ti = sys::sdrkit_table_info::default();
            // SAFETY: `ti` is a valid out parameter.
            self.check(unsafe { sys::sdrkit_table_decl(self.0, t, &mut ti) })?;
            let mut table = TableDecl {
                key: owned(ti.key),
                label: owned(ti.label),
                columns: Vec::with_capacity(ti.columns as usize),
                cap: ti.cap,
                label_column: (ti.label_column != sys::SDRKIT_NO_ID)
                    .then_some(ti.label_column as usize),
            };
            for c in 0..ti.columns {
                table.columns.push(self.decl(
                    // SAFETY: `d` points at a live `sdrkit_decl`.
                    |d| unsafe { sys::sdrkit_column_decl(self.0, t, c, d) },
                    // SAFETY: `o` points at a live `sdrkit_option`.
                    |n, o| unsafe { sys::sdrkit_column_option(self.0, t, c, n, o) },
                )?);
            }
            out.tables.push(table);
        }
        Ok(out)
    }

    /// Takes a snapshot of the results that changed after the counter `since`, copies its summary into `summary` and
    /// returns the rest of its state.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_OBJECT`.
    pub fn snapshot_read(&self, since: u64, summary: &mut String) -> Result<SnapshotInfo> {
        let mut i = sys::sdrkit_snapshot_info::default();
        // SAFETY: `i` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_snapshot_read(self.0, since, &mut i) })?;
        copy_string(i.summary, summary);
        Ok(SnapshotInfo {
            counter: i.counter,
            declaration_changed: i.declaration_changed,
            fields_changed: i.fields_changed,
            state: match i.state {
                sys::SDRKIT_STATE_SEARCHING => State::Searching,
                sys::SDRKIT_STATE_TRACKING => State::Tracking,
                sys::SDRKIT_STATE_LOCKED => State::Locked,
                _ => State::Off,
            },
            snr_db: i.snr_db,
            level_dbfs: i.level_dbfs,
            offset_hz: i.offset_hz,
            log_lines: i.log_lines,
        })
    }

    /// Copies result field `id` of the snapshot into `out`.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_ID`.
    pub fn snapshot_field(&self, id: Id, out: &mut Value) -> Result<()> {
        let mut v = ZERO_VARIANT;
        // SAFETY: `v` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_snapshot_field(self.0, id, &mut v) })?;
        out.copy_from(&v);
        Ok(())
    }

    /// Returns the row counts of table `table` in the snapshot.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_ID`.
    pub fn snapshot_table(&self, table: Id) -> Result<RowsInfo> {
        let mut r = sys::sdrkit_rows_info::default();
        // SAFETY: `r` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_snapshot_table(self.0, table, &mut r) })?;
        Ok(RowsInfo {
            rows: r.rows,
            total: r.total,
        })
    }

    /// Copies the key of changed row `n` of table `table` into `out`.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_ID`.
    pub fn snapshot_row(&self, table: Id, n: u32, out: &mut String) -> Result<()> {
        let mut s = EMPTY;
        // SAFETY: `s` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_snapshot_row(self.0, table, n, &mut s) })?;
        copy_string(s, out);
        Ok(())
    }

    /// Copies column `column` of changed row `n` of table `table` into `out`.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_ID`.
    pub fn snapshot_cell(&self, table: Id, n: u32, column: Id, out: &mut Value) -> Result<()> {
        let mut v = ZERO_VARIANT;
        // SAFETY: `v` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_snapshot_cell(self.0, table, n, column, &mut v) })?;
        out.copy_from(&v);
        Ok(())
    }

    /// Copies log line `n` of the snapshot into `out`. Line 0 is the oldest.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_ID`.
    pub fn snapshot_log(&self, n: u32, out: &mut String) -> Result<()> {
        let mut s = EMPTY;
        // SAFETY: `s` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_snapshot_log(self.0, n, &mut s) })?;
        copy_string(s, out);
        Ok(())
    }

    /// Returns the event types of the catalog of the engine, by id.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_OBJECT`.
    pub fn event_types(&self) -> Result<Vec<EventType>> {
        let mut count = 0;
        // SAFETY: `count` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_event_types_count(self.0, &mut count) })?;
        let mut out = Vec::with_capacity(count as usize);
        for t in 0..count {
            let mut i = sys::sdrkit_event_type_info::default();
            // SAFETY: `i` is a valid out parameter.
            self.check(unsafe { sys::sdrkit_event_type_decl(self.0, t, &mut i) })?;
            let mut et = EventType {
                key: owned(i.key),
                label: owned(i.label),
                fields: Vec::with_capacity(i.fields as usize),
            };
            for f in 0..i.fields {
                et.fields.push(self.decl(
                    // SAFETY: `d` points at a live `sdrkit_decl`.
                    |d| unsafe { sys::sdrkit_event_field_decl(self.0, t, f, d) },
                    // SAFETY: `o` points at a live `sdrkit_option`.
                    |n, o| unsafe { sys::sdrkit_event_field_option(self.0, t, f, n, o) },
                )?);
            }
            out.push(et);
        }
        Ok(out)
    }

    /// Takes the waiting events of the object and returns their number. The events of the previous take are dropped.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_OBJECT`.
    pub fn events_take(&self) -> Result<u32> {
        let mut count = 0;
        // SAFETY: `count` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_events_take(self.0, &mut count) })?;
        Ok(count)
    }

    /// Returns event `n` of the last take.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_ID`.
    pub fn event(&self, n: u32) -> Result<EventInfo> {
        let mut e = sys::sdrkit_event_info::default();
        // SAFETY: `e` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_event(self.0, n, &mut e) })?;
        Ok(EventInfo {
            ty: e.r#type,
            time: e.time,
            freq_hz: e.freq_hz,
            fields: e.fields,
        })
    }

    /// Copies field `id` of event `n` of the last take into `out`.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_ID`.
    pub fn event_field(&self, n: u32, id: Id, out: &mut Value) -> Result<()> {
        let mut v = ZERO_VARIANT;
        // SAFETY: `v` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_event_field(self.0, n, id, &mut v) })?;
        out.copy_from(&v);
        Ok(())
    }

    /// Copies the waiting audio of this session into `out`, whole interleaved frames.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_OBJECT`, `SDRKIT_E_WRONG_HANDLE`.
    pub fn audio_read(&self, out: &mut [f32]) -> Result<AudioInfo> {
        let mut i = sys::sdrkit_audio_info::default();
        // SAFETY: `out` is valid for `out.len()` floats and `i` is a valid out parameter.
        self.check(unsafe { sys::sdrkit_audio_read(self.0, out.as_mut_ptr(), out.len(), &mut i) })?;
        Ok(AudioInfo {
            frames: i.frames,
            rate_hz: i.rate_hz,
            channels: i.channels,
        })
    }

    /// Copies the newest spectrum frame of this session into `bins`, if a frame came after the last call. `bins` stays
    /// as it is if no frame came.
    ///
    /// # Errors
    ///
    /// `SDRKIT_E_NO_SUCH_OBJECT`, `SDRKIT_E_WRONG_HANDLE`.
    pub fn spectrum_read(&self, bins: &mut Vec<f32>) -> Result<Option<SpectrumInfo>> {
        let mut i = sys::sdrkit_spectrum_frame::default();
        let mut copy = vec![0.0f32; bins.capacity().max(1024)];
        // SAFETY: `copy` holds as many values as the capacity passed, and `i` is a valid out parameter.
        self.check(unsafe {
            sys::sdrkit_spectrum_read(self.0, copy.as_mut_ptr(), copy.len() as u32, &mut i)
        })?;
        if !i.found {
            return Ok(None);
        }
        if i.total > i.bins {
            // The frame has more bins than the buffer: the next frame is read with a buffer of its size.
            bins.reserve((i.total as usize).saturating_sub(bins.len()));
            return Ok(None);
        }
        bins.clear();
        bins.extend_from_slice(&copy[..i.bins as usize]);
        Ok(Some(SpectrumInfo {
            span_hz: i.span_hz,
            centre_hz: i.centre_hz,
        }))
    }
}

/// Returns the C arguments of `values`. Their views point into `values`.
fn args(values: &[(Id, Value)]) -> Vec<sys::sdrkit_arg> {
    values
        .iter()
        .map(|(id, v)| sys::sdrkit_arg {
            id: *id,
            value: v.variant(),
        })
        .collect()
}
