//! The variant that carries every value, the string and byte views, and their conversions to and from the library's values.

use crate::error::Error;
use crate::handle::slice_in;
use sdrkit_core::{Value, ValueKind};
use std::ffi::c_char;
use std::sync::Arc;

/// The id of a setting, an argument, a field, a column, a table, a series, a raster, a command, an event type, a component
/// or a tap.
pub type Id = u32;

/// No id: the value of an id field of `sdrkit_error_info` that does not apply.
pub const SDRKIT_NO_ID: Id = 0xFFFF_FFFF;

/// A UTF-8 string of `len` bytes at `ptr`, with no NUL at the end. `ptr` is never NULL.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Str {
    /// The first byte.
    pub ptr: *const c_char,
    /// The number of bytes.
    pub len: usize,
}

impl Str {
    /// The empty string.
    pub(crate) const EMPTY: Str = Str {
        ptr: "".as_ptr().cast(),
        len: 0,
    };

    /// A view of `s`.
    pub(crate) fn of(s: &str) -> Self {
        Str {
            ptr: s.as_ptr().cast(),
            len: s.len(),
        }
    }
}

/// `len` bytes at `ptr`. `ptr` is never NULL in a value that the library writes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Bytes {
    /// The first byte.
    pub ptr: *const u8,
    /// The number of bytes.
    pub len: usize,
}

impl Bytes {
    pub(crate) fn of(b: &[u8]) -> Self {
        Bytes {
            ptr: b.as_ptr(),
            len: b.len(),
        }
    }
}

/// No value.
pub const SDRKIT_TYPE_NONE: i32 = 0;
/// Yes or no, in `flag`.
pub const SDRKIT_TYPE_BOOL: i32 = 1;
/// A whole number, in `integer`.
pub const SDRKIT_TYPE_I64: i32 = 2;
/// A number, in `number`.
pub const SDRKIT_TYPE_F64: i32 = 3;
/// A string, in `string`.
pub const SDRKIT_TYPE_STRING: i32 = 4;
/// A frequency in Hz, in `number`.
pub const SDRKIT_TYPE_HZ: i32 = 5;
/// A time in seconds since 1970 (UTC), in `number`.
pub const SDRKIT_TYPE_TIME: i32 = 6;
/// A length of time in seconds, in `number`.
pub const SDRKIT_TYPE_DURATION: i32 = 7;
/// A position in degrees, in `lat` and `lon`.
pub const SDRKIT_TYPE_POSITION: i32 = 8;
/// One of the options of the declaration, by its index in `enum_index`.
pub const SDRKIT_TYPE_ENUM: i32 = 9;
/// Raw bytes, in `bytes`.
pub const SDRKIT_TYPE_BYTES: i32 = 10;
/// Any type: each value carries its own. Only a column or an argument has this type.
pub const SDRKIT_TYPE_ANY: i32 = 11;
/// A set of the flags of the declaration, as a mask in `flags`: bit n is option n.
pub const SDRKIT_TYPE_FLAGS: i32 = 12;

/// One value. `type` is an `SDRKIT_TYPE_*` constant and selects the member that holds the value. The other members are 0.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Variant {
    /// An `SDRKIT_TYPE_*` constant.
    pub r#type: i32,
    /// For `SDRKIT_TYPE_BOOL`.
    pub flag: bool,
    /// For `SDRKIT_TYPE_ENUM`: the index of the option.
    pub enum_index: u32,
    /// For `SDRKIT_TYPE_I64`.
    pub integer: i64,
    /// For `SDRKIT_TYPE_F64`, `SDRKIT_TYPE_HZ`, `SDRKIT_TYPE_TIME` and `SDRKIT_TYPE_DURATION`.
    pub number: f64,
    /// For `SDRKIT_TYPE_POSITION`: degrees north.
    pub lat: f64,
    /// For `SDRKIT_TYPE_POSITION`: degrees east.
    pub lon: f64,
    /// For `SDRKIT_TYPE_STRING`.
    pub string: Str,
    /// For `SDRKIT_TYPE_BYTES`.
    pub bytes: Bytes,
    /// For `SDRKIT_TYPE_FLAGS`: the mask.
    pub flags: u64,
}

/// A place on the Earth.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Position {
    /// Degrees north.
    pub lat: f64,
    /// Degrees east.
    pub lon: f64,
    /// Metres above the WGS 84 ellipsoid. It holds a value only if `has_alt` is true.
    pub alt_m: f64,
    /// True if `alt_m` holds a value.
    pub has_alt: bool,
}

impl Position {
    /// A position without an altitude.
    pub(crate) fn at(lat: f64, lon: f64) -> Self {
        Position {
            lat,
            lon,
            alt_m: 0.0,
            has_alt: false,
        }
    }
}

// The struct of the header is the struct that the layout of the typed structs places.
const _: () = assert!(
    std::mem::size_of::<Position>()
        == std::mem::size_of::<sdrkit_core::interface::layout::Position>()
);

/// A value for the setting or argument `id`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Arg {
    /// The id of the setting or argument.
    pub id: Id,
    /// The value.
    pub value: Variant,
}

/// The `SDRKIT_TYPE_*` constant of `k`.
pub(crate) fn type_code(k: ValueKind) -> i32 {
    match k {
        ValueKind::Bool => SDRKIT_TYPE_BOOL,
        ValueKind::I64 => SDRKIT_TYPE_I64,
        ValueKind::F64 => SDRKIT_TYPE_F64,
        ValueKind::Text => SDRKIT_TYPE_STRING,
        ValueKind::Hz => SDRKIT_TYPE_HZ,
        ValueKind::Time => SDRKIT_TYPE_TIME,
        ValueKind::Duration => SDRKIT_TYPE_DURATION,
        ValueKind::Position => SDRKIT_TYPE_POSITION,
        ValueKind::Choice => SDRKIT_TYPE_ENUM,
        ValueKind::Flags => SDRKIT_TYPE_FLAGS,
        ValueKind::Bytes => SDRKIT_TYPE_BYTES,
        ValueKind::Any => SDRKIT_TYPE_ANY,
    }
}

/// The library's id of the C id `id`. Ids above the library's range return `Error::NoSuchId`.
pub(crate) fn small(id: Id) -> Result<u16, Error> {
    u16::try_from(id).map_err(|_| Error::NoSuchId)
}

/// `n` as a C count.
pub(crate) fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Variant {
    /// A variant of type `t` with every member at 0.
    fn empty(t: i32) -> Self {
        Variant {
            r#type: t,
            flag: false,
            enum_index: 0,
            integer: 0,
            number: 0.0,
            lat: 0.0,
            lon: 0.0,
            string: Str::of(""),
            bytes: Bytes::of(&[]),
            flags: 0,
        }
    }

    /// A view of `v`. Strings and bytes point into `v`.
    pub(crate) fn of(v: &Value) -> Self {
        let num = |t, n| Variant {
            number: n,
            ..Self::empty(t)
        };
        match v {
            Value::None => Self::empty(SDRKIT_TYPE_NONE),
            Value::Bool(b) => Variant {
                flag: *b,
                ..Self::empty(SDRKIT_TYPE_BOOL)
            },
            Value::I64(i) => Variant {
                integer: *i,
                ..Self::empty(SDRKIT_TYPE_I64)
            },
            Value::F64(n) => num(SDRKIT_TYPE_F64, *n),
            Value::Hz(n) => num(SDRKIT_TYPE_HZ, *n),
            Value::Time(n) => num(SDRKIT_TYPE_TIME, *n),
            Value::Duration(n) => num(SDRKIT_TYPE_DURATION, *n),
            Value::Text(t) => Variant {
                string: Str::of(t),
                ..Self::empty(SDRKIT_TYPE_STRING)
            },
            Value::Position { lat, lon } => Variant {
                lat: *lat,
                lon: *lon,
                ..Self::empty(SDRKIT_TYPE_POSITION)
            },
            Value::Choice(c) => Variant {
                enum_index: u32::from(*c),
                ..Self::empty(SDRKIT_TYPE_ENUM)
            },
            Value::Flags(m) => Variant {
                flags: *m,
                ..Self::empty(SDRKIT_TYPE_FLAGS)
            },
            Value::Bytes(b) => Variant {
                bytes: Bytes::of(b),
                ..Self::empty(SDRKIT_TYPE_BYTES)
            },
        }
    }

    /// The library's value of this variant. Strings and bytes are copied.
    ///
    /// For `SDRKIT_TYPE_STRING`, `string` is valid for reads of `string.len` bytes. For `SDRKIT_TYPE_BYTES`, `bytes` is valid
    /// for reads of `bytes.len` bytes.
    pub(crate) unsafe fn to_value(self) -> Result<Value, Error> {
        Ok(match self.r#type {
            SDRKIT_TYPE_NONE => Value::None,
            SDRKIT_TYPE_BOOL => Value::Bool(self.flag),
            SDRKIT_TYPE_I64 => Value::I64(self.integer),
            SDRKIT_TYPE_F64 => Value::F64(self.number),
            SDRKIT_TYPE_HZ => Value::Hz(self.number),
            SDRKIT_TYPE_TIME => Value::Time(self.number),
            SDRKIT_TYPE_DURATION => Value::Duration(self.number),
            SDRKIT_TYPE_POSITION => Value::Position {
                lat: self.lat,
                lon: self.lon,
            },
            SDRKIT_TYPE_ENUM => {
                Value::Choice(
                    u16::try_from(self.enum_index).map_err(|_| Error::NotAnOption {
                        index: self.enum_index,
                    })?,
                )
            }
            SDRKIT_TYPE_FLAGS => Value::Flags(self.flags),
            SDRKIT_TYPE_STRING => {
                // SAFETY: the caller's contract on `string`.
                let b = unsafe { slice_in(self.string.ptr.cast::<u8>(), self.string.len)? };
                Value::Text(
                    std::str::from_utf8(b)
                        .map_err(|_| Error::BadString)?
                        .to_owned(),
                )
            }
            SDRKIT_TYPE_BYTES => {
                // SAFETY: the caller's contract on `bytes`.
                let b = unsafe { slice_in(self.bytes.ptr, self.bytes.len)? };
                Value::Bytes(Arc::from(b))
            }
            _ => return Err(Error::BadArgument),
        })
    }
}
