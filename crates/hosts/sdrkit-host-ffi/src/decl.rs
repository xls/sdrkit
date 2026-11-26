//! Declarations of the values of an object: its settings, the arguments and reply fields of its commands, its result fields
//! and table columns, and the fields of the event types of the engine.

use crate::error::Error;
use crate::handle::{run, str_in, Entry, Handle, Out};
use crate::value::{count, small, type_code, Id, Str, Variant};
use sdrkit_core::{
    CommandId, ContentType, EventKindId, FieldDecl, RecordDecl, Schema, SettingDecl, SettingId,
    TableId, Value,
};
use sdrkit_host_engine::{split_address, EngineError};
use std::ffi::c_char;
use std::sync::Arc;

/// `min`, `max` and `step` hold the range of the value.
pub const SDRKIT_DECL_RANGE: u32 = 1;
/// The argument has no default: the caller must pass it.
pub const SDRKIT_DECL_REQUIRED: u32 = 2;
/// A change takes effect after a restart of the object.
pub const SDRKIT_DECL_RESTART: u32 = 4;
/// The value cannot change.
pub const SDRKIT_DECL_FIXED: u32 = 8;
/// The setting is advanced: a program shows the other settings first, and these on request.
pub const SDRKIT_DECL_ADVANCED: u32 = 16;

/// The setting has no role.
pub const SDRKIT_SETTING_ROLE_NONE: i32 = 0;
/// The setting chooses whose picture the object shows: an instrument, a product.
pub const SDRKIT_SETTING_ROLE_PICTURE_SOURCE: i32 = 1;
/// The setting chooses the view of the picture shown: a channel, a composite.
pub const SDRKIT_SETTING_ROLE_PICTURE_VIEW: i32 = 2;

/// The bytes of the value are not declared, or the value is not bytes.
pub const SDRKIT_CONTENT_NONE: i32 = 0;
/// A picture or media of the codec in `fourcc`.
pub const SDRKIT_CONTENT_MEDIA: i32 = 1;
/// A plain UTF-8 string.
pub const SDRKIT_CONTENT_PLAIN: i32 = 2;
/// Comma-separated values.
pub const SDRKIT_CONTENT_CSV: i32 = 3;
/// JSON.
pub const SDRKIT_CONTENT_JSON: i32 = 4;
/// XML.
pub const SDRKIT_CONTENT_XML: i32 = 5;
/// Bytes of no declared format.
pub const SDRKIT_CONTENT_BINARY: i32 = 6;

/// The declaration of one value: a setting, an argument, a reply field, a result field, a table column or an event field.
/// A member that does not apply to the value is 0 or empty.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Decl {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The key.
    pub key: Str,
    /// The name for people.
    pub label: Str,
    /// One or two sentences for people. Settings and arguments only.
    pub help: Str,
    /// The section that the setting shows in, or empty.
    pub section: Str,
    /// The unit, or empty.
    pub unit: Str,
    /// An `SDRKIT_TYPE_*` constant.
    pub r#type: i32,
    /// `SDRKIT_DECL_*` bits.
    pub flags: u32,
    /// The lowest number, if `flags` has `SDRKIT_DECL_RANGE`.
    pub min: f64,
    /// The highest number, if `flags` has `SDRKIT_DECL_RANGE`.
    pub max: f64,
    /// The step, or 0 for any number, if `flags` has `SDRKIT_DECL_RANGE`.
    pub step: f64,
    /// The default value. Settings and arguments only.
    pub default_value: Variant,
    /// What the special number (`special_value`, 0 unless declared) means when it is not a quantity ("Automatic"), or
    /// empty. With a meaning, the special number is valid outside the range, and a front end shows the meaning in place
    /// of the number. Settings and arguments only.
    pub zero_means: Str,
    /// For `SDRKIT_TYPE_ENUM`: the number of options. For `SDRKIT_TYPE_FLAGS`: the number of flags; option n is the
    /// flag of bit n.
    pub options: u32,
    /// For a column that holds row keys of another table: the key of the object of that table, or empty for the same
    /// object.
    pub refers_object: Str,
    /// The key of that table, or empty.
    pub refers_table: Str,
    /// An `SDRKIT_CONTENT_*` constant: what the bytes of the value hold.
    pub content: i32,
    /// For `SDRKIT_CONTENT_MEDIA`: the four-character code of the codec.
    pub fourcc: [u8; 4],
    /// The number of values of the setting that people normally use, for the present option of the setting that they
    /// depend on. `sdrkit_setting_common()` reads them. Settings only.
    pub common: u32,
    /// An `SDRKIT_SETTING_ROLE_*` constant: what the setting chooses. A front end finds the setting that chooses the
    /// view of the picture by this member and not by the key. Settings only.
    pub role: i32,
    /// With `refers_table`: the key of the column of that table that holds the value to pass, or empty. Empty: the
    /// value is a row key of the table. With a column, a front end offers the rows of the table and passes the cell
    /// of this column of the chosen row; other values stay valid (the addressee of a send command). Arguments and
    /// settings.
    pub refers_column: Str,
    /// The place of the field among the key readings of its object, from 1: what a person looks at first. 0 for a
    /// field that is no key reading. Result fields only.
    pub reading: u32,
    /// The number that `zero_means` names: 0, or another (-1 for a gain stage that follows the gain). Settings and
    /// arguments only.
    pub special_value: f64,
}

/// One option of a value of type `SDRKIT_TYPE_ENUM`, or one flag of a value of type `SDRKIT_TYPE_FLAGS`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct EnumOption {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The key that data files use.
    pub key: Str,
    /// The name for people.
    pub label: Str,
    /// What a flag says, in one line. Empty for an option.
    pub help: Str,
}

/// A declared list of values of an object, or the settings that an open of a radio address takes.
#[derive(Clone, Copy)]
enum Family<'a> {
    Settings,
    Arguments(Id),
    Reply(Id),
    Fields,
    Columns(Id),
    EventFields(Id),
    RadioOpen(&'a str),
}

/// The declaration that the last declaration call of a handle read.
#[derive(Default)]
pub(crate) enum Held {
    #[default]
    Nothing,
    Schema(Arc<Schema>),
    Record(Arc<RecordDecl>),
}

impl Held {
    fn len(&self) -> usize {
        match self {
            Held::Nothing => 0,
            Held::Schema(s) => s.len(),
            Held::Record(r) => r.len(),
        }
    }

    fn field(&self, id: Id) -> Option<&FieldDecl> {
        let i = usize::try_from(id).ok()?;
        match self {
            Held::Nothing => None,
            Held::Schema(s) => s.settings().get(i).map(|d| &d.field),
            Held::Record(r) => r.fields().get(i),
        }
    }

    fn setting(&self, id: Id) -> Option<&SettingDecl> {
        match self {
            Held::Schema(s) => s.settings().get(usize::try_from(id).ok()?),
            Held::Nothing | Held::Record(_) => None,
        }
    }
}

/// The declared list `family` of the object of `e`.
fn hold(e: &Entry, family: Family) -> Result<Held, Error> {
    match family {
        Family::Settings => Ok(Held::Schema(e.owner(|o| Arc::clone(o.schema()))?)),
        Family::Arguments(c) => {
            let id = CommandId(small(c)?);
            e.owner(|o| o.commands().def(id).map(|d| Arc::clone(&d.args)))?
                .map(Held::Schema)
                .ok_or(Error::NoSuchId)
        }
        Family::Reply(c) => {
            let id = CommandId(small(c)?);
            e.owner(|o| o.commands().def(id).map(|d| Arc::clone(&d.reply)))?
                .map(Held::Record)
                .ok_or(Error::NoSuchId)
        }
        Family::Fields => Ok(Held::Record(
            e.owner(|o| Arc::clone(o.results_decl().fields()))?,
        )),
        Family::Columns(t) => {
            let id = TableId(small(t)?);
            e.owner(|o| o.results_decl().table(id).map(|t| Arc::clone(&t.columns)))?
                .map(Held::Record)
                .ok_or(Error::NoSuchId)
        }
        Family::EventFields(k) => e
            .host
            .engine
            .event_catalog()
            .fields(EventKindId(small(k)?))
            .map(|f| Held::Record(Arc::clone(f)))
            .ok_or(Error::NoSuchId),
        Family::RadioOpen(address) => {
            let (driver, at) = split_address(address);
            Ok(Held::Schema(e.host.engine.radio_schema(driver, at)?))
        }
    }
}

/// The declaration of `f`, with the setting members of `s` if it is a setting or an argument.
fn decl_of(f: &FieldDecl, s: Option<&SettingDecl>) -> Decl {
    let (content, fourcc) = match f.content {
        None => (SDRKIT_CONTENT_NONE, [0; 4]),
        Some(ContentType::Media(c)) => (SDRKIT_CONTENT_MEDIA, c.fourcc().0),
        Some(ContentType::Text) => (SDRKIT_CONTENT_PLAIN, [0; 4]),
        Some(ContentType::Csv) => (SDRKIT_CONTENT_CSV, [0; 4]),
        Some(ContentType::Json) => (SDRKIT_CONTENT_JSON, [0; 4]),
        Some(ContentType::Xml) => (SDRKIT_CONTENT_XML, [0; 4]),
        Some(ContentType::Binary) => (SDRKIT_CONTENT_BINARY, [0; 4]),
    };
    let refers = f.refers_to.as_ref();
    let range = s.and_then(|s| s.range);
    let flag = |on: bool, bit: u32| if on { bit } else { 0 };
    let flags = s.map_or(0, |s| {
        flag(s.range.is_some(), SDRKIT_DECL_RANGE)
            | flag(s.required, SDRKIT_DECL_REQUIRED)
            | flag(s.restart, SDRKIT_DECL_RESTART)
            | flag(s.fixed, SDRKIT_DECL_FIXED)
            | flag(s.advanced, SDRKIT_DECL_ADVANCED)
    });
    Decl {
        size: 0,
        key: Str::of(&f.key),
        label: Str::of(&f.label),
        help: Str::of(s.map_or("", |s| &s.help)),
        section: Str::of(s.map_or("", |s| &s.section)),
        unit: Str::of(&f.unit),
        r#type: type_code(f.kind),
        flags,
        min: range.map_or(0.0, |r| r.min),
        max: range.map_or(0.0, |r| r.max),
        step: range.map_or(0.0, |r| r.step),
        default_value: Variant::of(s.map_or(&Value::None, |s| &s.default)),
        zero_means: Str::of(s.map_or("", |s| &s.zero_means)),
        options: count(f.choices.len().max(f.flags.len())),
        refers_object: Str::of(
            refers
                .and_then(|r| r.owner.as_ref())
                .map_or("", |o| o.as_str()),
        ),
        refers_table: Str::of(refers.map_or("", |r| &r.table)),
        content,
        fourcc,
        common: 0,
        role: match s.and_then(|s| s.role) {
            None => SDRKIT_SETTING_ROLE_NONE,
            Some(sdrkit_core::SettingRole::PictureSource) => SDRKIT_SETTING_ROLE_PICTURE_SOURCE,
            Some(sdrkit_core::SettingRole::PictureView) => SDRKIT_SETTING_ROLE_PICTURE_VIEW,
        },
        refers_column: Str::of(refers.map_or("", |r| &r.column)),
        reading: u32::from(f.reading),
        special_value: s.map_or(0.0, |s| s.special_value),
    }
}

/// The common values of setting `id` of `schema` for the present `values` ([`Schema::common`]).
fn common_of<'a>(schema: &'a Schema, id: Id, values: Option<&sdrkit_core::Values>) -> &'a [f64] {
    u16::try_from(id).map_or(&[], |i| schema.common(SettingId(i), values))
}

/// Writes the id of the value with the key `key` in `family` to `id`.
unsafe fn find(h: Handle, family: Family, key: *const c_char, id: *mut Id) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(id)) };
    run(h, |e| {
        let (key, out) = (key?, out?);
        let held = hold(e, family)?;
        let found = (0..count(held.len())).find(|&i| held.field(i).is_some_and(|f| f.key == key));
        out.put(found.ok_or_else(|| {
            Error::Engine(EngineError::NoSuchKey {
                key: key.to_owned(),
            })
        })?);
        Ok(())
    })
}

/// Fills `decl` with the declaration of value `id` of `family`.
unsafe fn decl(h: Handle, family: Family, id: Id, decl: *mut Decl) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(decl) };
    run(h, |e| {
        let out = out?;
        let held = hold(e, family)?;
        let common = match family {
            Family::Settings => e.owner(|o| count(common_of(o.schema(), id, o.values()).len()))?,
            _ => 0,
        };
        let mut st = e.state();
        st.decl = held;
        let f = st.decl.field(id).ok_or(Error::NoSuchId)?;
        out.put(Decl {
            common,
            ..decl_of(f, st.decl.setting(id))
        });
        Ok(())
    })
}

/// Fills `option` with option `n`, or flag `n`, of value `id` of `family`.
unsafe fn option(h: Handle, family: Family, id: Id, n: u32, option: *mut EnumOption) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(option) };
    run(h, |e| {
        let out = out?;
        let held = hold(e, family)?;
        let mut st = e.state();
        st.decl = held;
        let f = st.decl.field(id).ok_or(Error::NoSuchId)?;
        let n = usize::try_from(n).map_err(|_| Error::NoSuchId)?;
        let (key, label, help) = match (f.choices.get(n), f.flags.get(n)) {
            (Some(c), _) => (&c.key, &c.label, ""),
            (None, Some(g)) => (&g.key, &g.label, g.help.as_ref()),
            (None, None) => return Err(Error::NoSuchId),
        };
        out.put(EnumOption {
            size: 0,
            key: Str::of(key),
            label: Str::of(label),
            help: Str::of(help),
        });
        Ok(())
    })
}

/// Writes the number of settings of the object of `handle` to `count`. Setting ids go from 0 to `count` - 1.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_settings_count(handle: Handle, count: *mut u32) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(count) };
    run(handle, |e| {
        let n = hold(e, Family::Settings)?.len();
        out?.put(self::count(n));
        Ok(())
    })
}

/// Writes the id of the setting with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no setting has the key.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_setting_find(
    handle: Handle,
    key: *const c_char,
    id: *mut Id,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { find(handle, Family::Settings, key, id) }
}

/// Fills `decl` with the declaration of setting `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the setting does not exist.
/// The strings of `decl` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_setting_decl(handle: Handle, id: Id, decl: *mut Decl) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { self::decl(handle, Family::Settings, id, decl) }
}

/// Fills `option` with option `n` of the setting `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the setting or the option does not
/// exist.
/// The strings of `option` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_setting_option(
    handle: Handle,
    id: Id,
    n: u32,
    option: *mut EnumOption,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { self::option(handle, Family::Settings, id, n, option) }
}

/// Copies the values of setting `id` that people normally use into `values`, at most `capacity`, and writes the number of
/// values that the setting has to `count`. A list that depends on another setting (the filter widths of a demodulator) is
/// the one for the present value of that setting. A setting without such values has a count of 0. `values` can be NULL
/// with a `capacity` of 0. Returns `SDRKIT_E_NO_SUCH_ID` if the setting does not exist.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_setting_common(
    handle: Handle,
    id: Id,
    values: *mut f64,
    capacity: u32,
    count: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(count) };
    run(handle, |e| {
        let out = out?;
        if values.is_null() && capacity > 0 {
            return Err(Error::Null);
        }
        let list: Vec<f64> = e.owner(|o| {
            let schema = o.schema();
            (usize::try_from(id).is_ok_and(|i| i < schema.len()))
                .then(|| common_of(schema, id, o.values()).to_vec())
        })?
        .ok_or(Error::NoSuchId)?;
        let copied = list.len().min(capacity as usize);
        if copied > 0 {
            // SAFETY: `values` holds `capacity` numbers, and `copied` is at most `capacity`.
            unsafe { std::ptr::copy_nonoverlapping(list.as_ptr(), values, copied) };
        }
        out.put(self::count(list.len()));
        Ok(())
    })
}

/// Runs `f` on the family of the settings that an open of `address` takes, or returns the error of a bad `address` on
/// `engine`.
unsafe fn radio_open(
    engine: Handle,
    address: *const c_char,
    f: impl FnOnce(Family<'_>) -> i32,
) -> i32 {
    // SAFETY: the caller's contract.
    match unsafe { str_in(address) } {
        Ok(a) => f(Family::RadioOpen(a)),
        Err(err) => run(engine, |_| Err(err)),
    }
}

/// Writes to `count` the number of settings that `sdrkit_radio_open()` takes for `address`: those of the object
/// `driver.<key>` of the scheme of the address, with the same ids, then those that the radio at the address adds and
/// that are known before it opens, such as the settings of the test signal of the stack of `synth:<stack>`. Setting ids
/// go from 0 to `count` - 1. Returns the codes of `sdrkit_radio_open()` for an unknown driver or stack.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_radio_open_settings_count(
    engine: Handle,
    address: *const c_char,
    count: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(count) };
    // SAFETY: the caller's contract.
    unsafe {
        radio_open(engine, address, |family| {
            run(engine, |e| {
                let n = hold(e, family)?.len();
                out?.put(self::count(n));
                Ok(())
            })
        })
    }
}

/// Writes to `id` the id of the setting with the key `key` that `sdrkit_radio_open()` takes for `address`. Returns
/// `SDRKIT_E_NO_SUCH_KEY` if no such setting has the key.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_radio_open_setting_find(
    engine: Handle,
    address: *const c_char,
    key: *const c_char,
    id: *mut Id,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { radio_open(engine, address, |family| find(engine, family, key, id)) }
}

/// Fills `decl` with the declaration of setting `id` that `sdrkit_radio_open()` takes for `address`. `common` is 0: the
/// open radio gives the common values with `sdrkit_setting_common()`. Returns `SDRKIT_E_NO_SUCH_ID` if the setting does
/// not exist.
/// The strings of `decl` are views that the library owns, valid until the next call on `engine`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_radio_open_setting_decl(
    engine: Handle,
    address: *const c_char,
    id: Id,
    decl: *mut Decl,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe {
        radio_open(engine, address, |family| {
            self::decl(engine, family, id, decl)
        })
    }
}

/// Fills `option` with option `n` of setting `id` that `sdrkit_radio_open()` takes for `address`. Returns
/// `SDRKIT_E_NO_SUCH_ID` if the setting or the option does not exist.
/// The strings of `option` are views that the library owns, valid until the next call on `engine`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_radio_open_setting_option(
    engine: Handle,
    address: *const c_char,
    id: Id,
    n: u32,
    option: *mut EnumOption,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe {
        radio_open(engine, address, |family| {
            self::option(engine, family, id, n, option)
        })
    }
}

/// Writes the id of the argument with the key `key` of command `command` to `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the
/// command does not exist, `SDRKIT_E_NO_SUCH_KEY` if no argument has the key.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_argument_find(
    handle: Handle,
    command: Id,
    key: *const c_char,
    id: *mut Id,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { find(handle, Family::Arguments(command), key, id) }
}

/// Fills `decl` with the declaration of argument `id` of command `command`. Returns `SDRKIT_E_NO_SUCH_ID` if the command or
/// the argument does not exist.
/// The strings of `decl` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_argument_decl(
    handle: Handle,
    command: Id,
    id: Id,
    decl: *mut Decl,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { self::decl(handle, Family::Arguments(command), id, decl) }
}

/// Fills `option` with option `n` of argument `id` of command `command`. Returns `SDRKIT_E_NO_SUCH_ID` if the command, the
/// argument or the option does not exist.
/// The strings of `option` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_argument_option(
    handle: Handle,
    command: Id,
    id: Id,
    n: u32,
    option: *mut EnumOption,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { self::option(handle, Family::Arguments(command), id, n, option) }
}

/// Writes the id of the reply field with the key `key` of command `command` to `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the
/// command does not exist, `SDRKIT_E_NO_SUCH_KEY` if no reply field has the key.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_reply_field_find(
    handle: Handle,
    command: Id,
    key: *const c_char,
    id: *mut Id,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { find(handle, Family::Reply(command), key, id) }
}

/// Fills `decl` with the declaration of reply field `id` of command `command`. Returns `SDRKIT_E_NO_SUCH_ID` if the command
/// or the field does not exist.
/// The strings of `decl` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_reply_field_decl(
    handle: Handle,
    command: Id,
    id: Id,
    decl: *mut Decl,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { self::decl(handle, Family::Reply(command), id, decl) }
}

/// Fills `option` with option `n` of reply field `id` of command `command`. Returns `SDRKIT_E_NO_SUCH_ID` if the command,
/// the field or the option does not exist.
/// The strings of `option` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_reply_field_option(
    handle: Handle,
    command: Id,
    id: Id,
    n: u32,
    option: *mut EnumOption,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { self::option(handle, Family::Reply(command), id, n, option) }
}

/// Writes the id of the result field with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no field has the key.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_field_find(handle: Handle, key: *const c_char, id: *mut Id) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { find(handle, Family::Fields, key, id) }
}

/// Fills `decl` with the declaration of result field `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the field does not exist.
/// The strings of `decl` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_field_decl(handle: Handle, id: Id, decl: *mut Decl) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { self::decl(handle, Family::Fields, id, decl) }
}

/// Fills `option` with option `n` of result field `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the field or the option does not
/// exist.
/// The strings of `option` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_field_option(
    handle: Handle,
    id: Id,
    n: u32,
    option: *mut EnumOption,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { self::option(handle, Family::Fields, id, n, option) }
}

/// Writes the id of the column with the key `key` of table `table` to `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the table does
/// not exist, `SDRKIT_E_NO_SUCH_KEY` if no column has the key.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_column_find(
    handle: Handle,
    table: Id,
    key: *const c_char,
    id: *mut Id,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { find(handle, Family::Columns(table), key, id) }
}

/// Fills `decl` with the declaration of column `id` of table `table`. Returns `SDRKIT_E_NO_SUCH_ID` if the table or the
/// column does not exist.
/// The strings of `decl` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_column_decl(
    handle: Handle,
    table: Id,
    id: Id,
    decl: *mut Decl,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { self::decl(handle, Family::Columns(table), id, decl) }
}

/// Fills `option` with option `n` of column `id` of table `table`. Returns `SDRKIT_E_NO_SUCH_ID` if the table, the column or
/// the option does not exist.
/// The strings of `option` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_column_option(
    handle: Handle,
    table: Id,
    id: Id,
    n: u32,
    option: *mut EnumOption,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { self::option(handle, Family::Columns(table), id, n, option) }
}

/// Writes the id of the field with the key `key` of event type `event_type` to `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the
/// event type does not exist, `SDRKIT_E_NO_SUCH_KEY` if no field has the key.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_event_field_find(
    handle: Handle,
    event_type: Id,
    key: *const c_char,
    id: *mut Id,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { find(handle, Family::EventFields(event_type), key, id) }
}

/// Fills `decl` with the declaration of field `id` of event type `event_type`. Returns `SDRKIT_E_NO_SUCH_ID` if the event
/// type or the field does not exist.
/// The strings of `decl` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_event_field_decl(
    handle: Handle,
    event_type: Id,
    id: Id,
    decl: *mut Decl,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { self::decl(handle, Family::EventFields(event_type), id, decl) }
}

/// Fills `option` with option `n` of field `id` of event type `event_type`. Returns `SDRKIT_E_NO_SUCH_ID` if the event type,
/// the field or the option does not exist.
/// The strings of `option` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_event_field_option(
    handle: Handle,
    event_type: Id,
    id: Id,
    n: u32,
    option: *mut EnumOption,
) -> i32 {
    // SAFETY: the caller's contract.
    unsafe { self::option(handle, Family::EventFields(event_type), id, n, option) }
}

crate::handle::sized_structs!(Decl, EnumOption);

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::publish::{field, Field};
    use sdrkit_core::settings::DeclareSetting;
    use sdrkit_core::Setting;

    /// The text of `s`.
    fn text(s: Str) -> String {
        // SAFETY: the view is of a string that lives in the declaration of the test.
        let bytes = unsafe { std::slice::from_raw_parts(s.ptr.cast::<u8>(), s.len) };
        String::from_utf8_lossy(bytes).into_owned()
    }

    #[test]
    fn a_key_reading_and_a_value_column_reach_the_declaration() {
        const LEVEL: Field<field::F64> = Field::new("level_db", "Level", "dB").reading(2);
        let d = decl_of(&LEVEL.decl(), None);
        assert_eq!(d.reading, 2);
        assert_eq!(d.refers_column.len, 0);
        const TO: Setting<field::Text> = Setting::new("to", "To", "", "").refers("nodes", "id");
        let s = TO.decl();
        let d = decl_of(&s.field, Some(&s));
        assert_eq!(text(d.refers_table), "nodes");
        assert_eq!(text(d.refers_column), "id");
        assert_eq!(d.reading, 0);
        // A plain reference holds a row key.
        const ROW: Field<field::Text> = Field::new("row", "Row", "").refers_to_table("ships");
        let d = decl_of(&ROW.decl(), None);
        assert_eq!(text(d.refers_table), "ships");
        assert_eq!(d.refers_column.len, 0);
    }
}
