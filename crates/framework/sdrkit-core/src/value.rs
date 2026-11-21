//! The one value type for result fields, table cells, event fields and settings, and the `Choice` trait for Rust enums.

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
    /// The text form for people. `choices` is the choice list of the field's declaration.
    pub fn display<'a>(&'a self, choices: &'a [ChoiceDecl]) -> FieldText<'a> {
        FieldText {
            value: self,
            choices,
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

/// The text form of a [`Value`] for people: numbers with at most six decimals and no trailing zeros, yes and no, a choice by its
/// label, a position as "lat, lon", a payload by its length. `None` is empty.
#[derive(Clone, Copy, Debug)]
pub struct FieldText<'a> {
    value: &'a Value,
    choices: &'a [ChoiceDecl],
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
