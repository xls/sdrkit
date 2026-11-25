//! The one value type for result fields, table cells, event fields and settings, the `Choice` trait for Rust enums and
//! the `FlagSet` trait for sets of named flags.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::Arc;

/// One value of a declared field.
///
/// `None` means "declared, no value now". It is legal in every field. A choice is an index into the choice list of the field's
/// declaration, never text.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum Value {
    /// No value now.
    #[default]
    None,
    /// Yes or no.
    Bool(bool),
    /// A whole number.
    I64(i64),
    /// A number. The unit is in the key's suffix.
    F64(f64),
    /// Text for people.
    Text(String),
    /// A frequency in Hz.
    Hz(f64),
    /// A time, seconds since 1970 (UTC).
    Time(f64),
    /// A length of time in seconds.
    Duration(f64),
    /// A position in degrees.
    Position {
        /// Latitude, degrees north.
        lat: f64,
        /// Longitude, degrees east.
        lon: f64,
    },
    /// An index into the choice list of the field's declaration.
    Choice(u16),
    /// A set of the flags of the field's declaration: bit n is entry n of the flag list.
    Flags(u64),
    /// A raw payload.
    Bytes(#[serde(with = "bytes_serde")] Arc<[u8]>),
}

/// The kind of a declared field. It has the variants of [`Value`] without `None`.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum ValueKind {
    /// Yes or no.
    Bool,
    /// A whole number.
    I64,
    /// A number.
    F64,
    /// Text.
    #[default]
    Text,
    /// A frequency in Hz.
    Hz,
    /// A time, seconds since 1970 (UTC).
    Time,
    /// A length of time in seconds.
    Duration,
    /// A position in degrees.
    Position,
    /// One of a declared list.
    Choice,
    /// A set of the flags of a declared list.
    Flags,
    /// A raw payload.
    Bytes,
    /// Any kind: each value carries its own. Only a table column or a command argument has this kind, for a stored setting
    /// of another owner. A value never has it.
    Any,
}

impl Value {
    /// The kind of the value. `None` for [`Value::None`].
    pub fn kind(&self) -> Option<ValueKind> {
        Some(match self {
            Value::None => return None,
            Value::Bool(_) => ValueKind::Bool,
            Value::I64(_) => ValueKind::I64,
            Value::F64(_) => ValueKind::F64,
            Value::Text(_) => ValueKind::Text,
            Value::Hz(_) => ValueKind::Hz,
            Value::Time(_) => ValueKind::Time,
            Value::Duration(_) => ValueKind::Duration,
            Value::Position { .. } => ValueKind::Position,
            Value::Choice(_) => ValueKind::Choice,
            Value::Flags(_) => ValueKind::Flags,
            Value::Bytes(_) => ValueKind::Bytes,
        })
    }
    /// True if a field of `kind` can hold this value. `None` fits every kind, and a field of [`ValueKind::Any`] holds every
    /// value.
    pub fn fits(&self, kind: ValueKind) -> bool {
        kind == ValueKind::Any || self.kind().is_none_or(|k| k == kind)
    }
    /// True for [`Value::None`].
    pub fn is_none(&self) -> bool {
        matches!(self, Value::None)
    }
    /// The text form for people. `choices` is the choice list of the field's declaration. A flag set shows as its mask:
    /// [`FieldText::with_flags`] gives it the labels.
    pub fn display<'a>(&'a self, choices: &'a [ChoiceDecl]) -> FieldText<'a> {
        FieldText {
            value: self,
            choices,
            flags: &[],
        }
    }
}

// `clone_from` reuses the text buffer, so copies of records allocate nothing once they have their size.
impl Clone for Value {
    fn clone(&self) -> Self {
        match self {
            Value::None => Value::None,
            Value::Bool(v) => Value::Bool(*v),
            Value::I64(v) => Value::I64(*v),
            Value::F64(v) => Value::F64(*v),
            Value::Text(t) => Value::Text(t.clone()),
            Value::Hz(v) => Value::Hz(*v),
            Value::Time(v) => Value::Time(*v),
            Value::Duration(v) => Value::Duration(*v),
            Value::Position { lat, lon } => Value::Position {
                lat: *lat,
                lon: *lon,
            },
            Value::Choice(i) => Value::Choice(*i),
            Value::Flags(m) => Value::Flags(*m),
            Value::Bytes(b) => Value::Bytes(Arc::clone(b)),
        }
    }
    fn clone_from(&mut self, s: &Self) {
        match (self, s) {
            (Value::Text(a), Value::Text(b)) => a.clone_from(b),
            (me, s) => *me = s.clone(),
        }
    }
}

/// One entry of a choice list: the key that data files and the C API use, and the label people see.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceDecl {
    /// The key (`"locked"`).
    pub key: Cow<'static, str>,
    /// What people see (`"Locked"`).
    pub label: Cow<'static, str>,
}

impl ChoiceDecl {
    /// An entry with this key and label, for a constant list.
    pub const fn new(key: &'static str, label: &'static str) -> Self {
        ChoiceDecl {
            key: Cow::Borrowed(key),
            label: Cow::Borrowed(label),
        }
    }
}

/// A Rust enum that a choice field holds. The field stores the index of the variant.
///
/// Implement it with [`choice_enum!`](crate::choice_enum), not by hand.
pub trait Choice: Copy + 'static {
    /// The declared list, in variant order.
    const CHOICES: &'static [ChoiceDecl];
    /// The index of this variant in [`Choice::CHOICES`].
    fn index(self) -> u16;
    /// The variant at `index`. `None` if the index is outside the list.
    fn from_index(index: u16) -> Option<Self>;
    /// The key of this variant.
    fn key(self) -> &'static str {
        &Self::CHOICES[usize::from(self.index())].key
    }
    /// The label of this variant, for people.
    fn label(self) -> &'static str {
        &Self::CHOICES[usize::from(self.index())].label
    }
    /// The variant with key `key`. `None` if no variant has it.
    fn from_key(key: &str) -> Option<Self> {
        let i = Self::CHOICES.iter().position(|c| c.key == key)?;
        Self::from_index(u16::try_from(i).ok()?)
    }
}

/// Declares an enum and implements [`Choice`] for it from one list of variants, keys and labels.
///
/// ```
/// sdrkit_core::choice_enum! {
///     /// A frequency plan.
///     pub enum Region {
///         One = ("r1", "Region 1"),
///         Two = ("r2", "Region 2"),
///     }
/// }
/// use sdrkit_core::Choice;
/// assert_eq!(Region::Two.index(), 1);
/// assert_eq!(Region::from_index(0), Some(Region::One));
/// assert_eq!(Region::CHOICES[1].label, "Region 2");
/// ```
#[macro_export]
macro_rules! choice_enum {
    ($(#[$m:meta])* $vis:vis enum $name:ident {
        $($(#[$vm:meta])* $var:ident = ($key:literal, $label:literal)),+ $(,)?
    }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        $vis enum $name {
            $($(#[$vm])* $var),+
        }
        impl $crate::value::Choice for $name {
            const CHOICES: &'static [$crate::value::ChoiceDecl] =
                &[$($crate::value::ChoiceDecl::new($key, $label)),+];
            fn index(self) -> u16 {
                self as u16
            }
            fn from_index(index: u16) -> Option<Self> {
                const ALL: &[$name] = &[$($name::$var),+];
                ALL.get(usize::from(index)).copied()
            }
        }
    };
}

/// One entry of a flag list: the key that data files and the C API use, the label people see and one line of help.
/// Entry n of the list is bit n of the mask.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagDecl {
    /// The key (`"teleport"`).
    pub key: Cow<'static, str>,
    /// What people see (`"Teleport"`).
    pub label: Cow<'static, str>,
    /// What the flag says, in one line.
    pub help: Cow<'static, str>,
}

impl FlagDecl {
    /// The most entries of one list: the bits of the mask.
    pub const MAX: usize = 64;
    /// An entry with this key, label and help line, for a constant list.
    pub const fn new(key: &'static str, label: &'static str, help: &'static str) -> Self {
        FlagDecl {
            key: Cow::Borrowed(key),
            label: Cow::Borrowed(label),
            help: Cow::Borrowed(help),
        }
    }
    /// The mask of every entry of a list of `count` entries.
    pub const fn all(count: usize) -> u64 {
        if count >= Self::MAX {
            u64::MAX
        } else {
            (1 << count) - 1
        }
    }
}

/// A Rust type that a flag field holds: a set of named flags. The field stores the mask.
///
/// Implement it with [`flag_set!`](crate::flag_set), not by hand.
pub trait FlagSet: Copy + 'static {
    /// The declared list. Entry n is bit n. At most [`FlagDecl::MAX`] entries.
    const FLAGS: &'static [FlagDecl];
    /// The mask.
    fn bits(self) -> u64;
    /// The set of mask `bits`. Bits outside the list are dropped.
    fn from_bits(bits: u64) -> Self;
    /// The entries of the flags in the set, in list order.
    fn entries(self) -> impl Iterator<Item = &'static FlagDecl> {
        let bits = self.bits();
        Self::FLAGS
            .iter()
            .enumerate()
            .filter(move |(i, _)| bits >> i & 1 == 1)
            .map(|(_, f)| f)
    }
    /// The set of the one flag with key `key`. `None` if no flag has it.
    fn from_key(key: &str) -> Option<Self> {
        let i = Self::FLAGS.iter().position(|f| f.key == key)?;
        Some(Self::from_bits(1 << i))
    }
}

/// Declares a set of named flags and implements [`FlagSet`] for it from one list of names, keys, labels and help lines.
/// The type holds a `u64` mask. Each name is a constant with one flag. The list has at most 64 entries.
///
/// ```
/// sdrkit_core::flag_set! {
///     /// What is wrong with a frame.
///     pub struct Faults {
///         CRC = ("crc", "CRC", "The checksum does not match."),
///         SHORT = ("short", "Short", "The frame ends before its length."),
///     }
/// }
/// use sdrkit_core::FlagSet;
/// let f = Faults::CRC | Faults::SHORT;
/// assert_eq!(f.bits(), 3);
/// assert!(f.contains(Faults::SHORT) && !Faults::CRC.contains(f));
/// assert_eq!(Faults::FLAGS[1].key, "short");
/// assert_eq!(Faults::from_bits(0b110), Faults::SHORT);
/// ```
#[macro_export]
macro_rules! flag_set {
    ($(#[$m:meta])* $vis:vis struct $name:ident {
        $($(#[$fm:meta])* $flag:ident = ($key:literal, $label:literal, $help:literal)),+ $(,)?
    }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        $vis struct $name(u64);
        const _: () = assert!(
            <$name as $crate::value::FlagSet>::FLAGS.len() <= $crate::value::FlagDecl::MAX,
            "a flag set has at most 64 flags"
        );
        impl $name {
            $crate::flag_set!(@consts 0u32; $($(#[$fm])* $flag = $help;)+);
            /// The set with no flag.
            pub const fn empty() -> Self {
                $name(0)
            }
            /// True if the set has no flag.
            pub const fn is_empty(self) -> bool {
                self.0 == 0
            }
            /// True if every flag of `other` is in the set.
            pub const fn contains(self, other: Self) -> bool {
                self.0 & other.0 == other.0
            }
            /// Adds the flags of `other`.
            pub fn insert(&mut self, other: Self) {
                self.0 |= other.0;
            }
        }
        impl ::core::ops::BitOr for $name {
            type Output = Self;
            fn bitor(self, other: Self) -> Self {
                $name(self.0 | other.0)
            }
        }
        impl $crate::value::FlagSet for $name {
            const FLAGS: &'static [$crate::value::FlagDecl] =
                &[$($crate::value::FlagDecl::new($key, $label, $help)),+];
            fn bits(self) -> u64 {
                self.0
            }
            fn from_bits(bits: u64) -> Self {
                $name(bits & $crate::value::FlagDecl::all(Self::FLAGS.len()))
            }
        }
        impl<'a> $crate::interface::Cell<'a> for $name {
            type Kind = $crate::publish::field::Flags<$name>;
            fn give(&self) -> $name {
                *self
            }
            fn take(v: $name) -> Self {
                v
            }
        }
    };
    (@consts $n:expr;) => {};
    (@consts $n:expr; $(#[$fm:meta])* $flag:ident = $help:literal; $($rest:tt)*) => {
        $(#[$fm])*
        #[doc = $help]
        pub const $flag: Self = Self(1u64 << $n);
        $crate::flag_set!(@consts $n + 1u32; $($rest)*);
    };
}

/// The text form of a [`Value`] for people: numbers with at most six decimals and no trailing zeros, yes and no, a choice by its
/// label, a flag set by its labels with ", " between them, a position as "lat, lon", a payload by its length. `None` is empty.
#[derive(Clone, Copy, Debug)]
pub struct FieldText<'a> {
    value: &'a Value,
    choices: &'a [ChoiceDecl],
    flags: &'a [FlagDecl],
}

impl<'a> FieldText<'a> {
    /// The same text, with `flags` as the flag list of the field's declaration. Without it a flag set shows as its mask.
    #[must_use]
    pub fn with_flags(mut self, flags: &'a [FlagDecl]) -> Self {
        self.flags = flags;
        self
    }
}

impl std::fmt::Display for FieldText<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::results::write_number;
        match self.value {
            Value::None => Ok(()),
            Value::Bool(b) => f.write_str(if *b { "yes" } else { "no" }),
            Value::I64(v) => write!(f, "{v}"),
            Value::F64(v) | Value::Hz(v) | Value::Time(v) | Value::Duration(v) => {
                write_number(f, *v)
            }
            Value::Text(t) => f.write_str(t),
            Value::Position { lat, lon } => {
                write_number(f, *lat)?;
                f.write_str(", ")?;
                write_number(f, *lon)
            }
            Value::Choice(i) => match self.choices.get(usize::from(*i)) {
                Some(c) => f.write_str(&c.label),
                None => write!(f, "{i}"),
            },
            Value::Flags(m) if self.flags.is_empty() => write!(f, "{m:#x}"),
            Value::Flags(m) => {
                let mut first = true;
                for (i, d) in self.flags.iter().enumerate() {
                    if m >> i & 1 == 1 {
                        if !first {
                            f.write_str(", ")?;
                        }
                        first = false;
                        f.write_str(&d.label)?;
                    }
                }
                Ok(())
            }
            Value::Bytes(b) => write!(f, "{} bytes", b.len()),
        }
    }
}

/// Bytes travel as padded standard base64 text (RFC 4648 section 4).
mod bytes_serde {
    use crate::base64::{decode_into, encode_into, Alphabet};
    use serde::{de::Error, Deserialize, Deserializer, Serializer};
    use std::sync::Arc;

    pub fn serialize<S: Serializer>(b: &Arc<[u8]>, s: S) -> Result<S::Ok, S::Error> {
        let mut text = String::with_capacity(b.len().div_ceil(3) * 4);
        encode_into(b, Alphabet::Standard, true, &mut text);
        s.serialize_str(&text)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Arc<[u8]>, D::Error> {
        let text = String::deserialize(d)?;
        let mut bytes = Vec::with_capacity(text.len() / 4 * 3);
        decode_into(&text, Alphabet::Standard, &mut bytes).map_err(D::Error::custom)?;
        Ok(Arc::from(bytes))
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::I64(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::F64(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_owned())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}
