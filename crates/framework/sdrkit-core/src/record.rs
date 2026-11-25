//! Field declarations, record declarations and records: one [`Value`] slot per declared field, written through [`Field`]
//! constants.

use crate::media::Codec;
use crate::owner::OwnerKey;
use crate::publish::{field, sealed, CatalogueField, Field};
use crate::value::{Choice, ChoiceDecl, FlagDecl, FlagSet, Value, ValueKind};
use serde::de::{DeserializeSeed, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// The declaration of one field or table column: its key, what people see, its unit, its kind and, for a choice or a
/// flag set, its list.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FieldDecl {
    /// The key. The key rules of [`crate::publish`] apply.
    pub key: Cow<'static, str>,
    /// What people see (`"SNR"`).
    pub label: Cow<'static, str>,
    /// The unit people see (`"dB"`). Empty for none.
    pub unit: Cow<'static, str>,
    /// The kind of the values.
    pub kind: ValueKind,
    /// The choice list of a [`ValueKind::Choice`] field. Empty for other kinds.
    #[serde(default, skip_serializing_if = "<[ChoiceDecl]>::is_empty")]
    pub choices: Cow<'static, [ChoiceDecl]>,
    /// The flag list of a [`ValueKind::Flags`] field: entry n is bit n of the mask. Empty for other kinds.
    #[serde(default, skip_serializing_if = "<[FlagDecl]>::is_empty")]
    pub flags: Cow<'static, [FlagDecl]>,
    /// The table whose row keys this column holds, or whose rows give values for it. `None` for most fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refers_to: Option<RefersTo>,
    /// The content type of a [`ValueKind::Bytes`] field. `None` for other kinds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<ContentType>,
    /// The order of the field among the key readings of its owner, from 1: what a person looks at first. 0 for a
    /// field that is no key reading.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub reading: u8,
}

// The derived form, with the flag list only on a field that has one: the text of the other fields stays as it was.
impl std::fmt::Debug for FieldDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("FieldDecl");
        s.field("key", &self.key)
            .field("label", &self.label)
            .field("unit", &self.unit)
            .field("kind", &self.kind)
            .field("choices", &self.choices);
        if !self.flags.is_empty() {
            s.field("flags", &self.flags);
        }
        s.field("refers_to", &self.refers_to)
            .field("content", &self.content)
            .field("reading", &self.reading)
            .finish()
    }
}

/// True for 0, for the serialiser.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(n: &u8) -> bool {
    *n == 0
}

/// What the bytes of a [`ValueKind::Bytes`] field hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    /// A picture or media of this codec.
    Media(Codec),
    /// Plain text in UTF-8.
    Text,
    /// Comma-separated values in UTF-8.
    Csv,
    /// JSON.
    Json,
    /// XML.
    Xml,
    /// Bytes of no declared type.
    Binary,
}

/// The table that a field refers to: the owner that holds it and the table key. Without a column, a value of the
/// field is a row key of the table. With a column, a value can be taken from that column of a row, and other values
/// stay valid: the addressee of a send command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefersTo {
    /// The owner of the table. `None` for a table of the owner that declares the column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<OwnerKey>,
    /// The key of the table in the owner's results.
    pub table: Cow<'static, str>,
    /// The key of the column that holds the value to pass. Empty: the value is the row key.
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub column: Cow<'static, str>,
}

impl FieldDecl {
    /// The most key readings of one owner.
    pub const MAX_READINGS: u8 = 6;
    /// A field of `kind` with no choice list.
    pub fn new(
        key: impl Into<Cow<'static, str>>,
        label: impl Into<Cow<'static, str>>,
        unit: impl Into<Cow<'static, str>>,
        kind: ValueKind,
    ) -> Self {
        FieldDecl {
            key: key.into(),
            label: label.into(),
            unit: unit.into(),
            kind,
            choices: Cow::Borrowed(&[]),
            flags: Cow::Borrowed(&[]),
            refers_to: None,
            content: None,
            reading: 0,
        }
    }
    /// A choice field with this list, for a list built at run time.
    pub fn with_choices(
        key: impl Into<Cow<'static, str>>,
        label: impl Into<Cow<'static, str>>,
        choices: impl Into<Cow<'static, [ChoiceDecl]>>,
    ) -> Self {
        FieldDecl {
            key: key.into(),
            label: label.into(),
            unit: Cow::Borrowed(""),
            kind: ValueKind::Choice,
            choices: choices.into(),
            flags: Cow::Borrowed(&[]),
            refers_to: None,
            content: None,
            reading: 0,
        }
    }
    /// The same field, holding row keys of table `table` of owner `owner`.
    #[must_use]
    pub fn with_refers_to(mut self, owner: OwnerKey, table: impl Into<Cow<'static, str>>) -> Self {
        self.refers_to = Some(RefersTo {
            owner: Some(owner),
            table: table.into(),
            column: Cow::Borrowed(""),
        });
        self
    }
    /// The same field, holding row keys of table `table` of the same owner.
    #[must_use]
    pub fn with_refers_to_table(mut self, table: impl Into<Cow<'static, str>>) -> Self {
        self.refers_to = Some(RefersTo {
            owner: None,
            table: table.into(),
            column: Cow::Borrowed(""),
        });
        self
    }
    /// The same field with content type `content`. [`RecordDecl::new`] refuses it on a field that is not
    /// [`ValueKind::Bytes`].
    #[must_use]
    pub fn with_content(mut self, content: ContentType) -> Self {
        self.content = Some(content);
        self
    }
    /// The mask of the flags that the field declares.
    pub fn flag_mask(&self) -> u64 {
        FlagDecl::all(self.flags.len())
    }
    /// The index of the choice with key `key`. `None` if the list has no such key.
    pub fn choice_index(&self, key: &str) -> Option<u16> {
        self.choices
            .iter()
            .position(|c| c.key == key)
            .and_then(|i| u16::try_from(i).ok())
    }
}

/// The index of a field in its [`RecordDecl`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FieldId(pub u16);

impl FieldId {
    /// The slot index.
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// Why a list of field declarations is not a record declaration.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DeclError {
    /// Two fields have the same key.
    #[error("the key {key} is declared twice")]
    DuplicateKey {
        /// The key.
        key: String,
    },
    /// The list has more fields than a [`FieldId`] can count.
    #[error("{count} fields is more than 65535")]
    TooMany {
        /// The number of fields.
        count: usize,
    },
    /// A field that is not [`ValueKind::Bytes`] declares a content type.
    #[error("the field {key} is not bytes and declares a content type")]
    ContentNotBytes {
        /// The key of the field.
        key: String,
    },
    /// A flag list has more than 64 entries or a key twice, or a field that is not [`ValueKind::Flags`] has one.
    #[error("the field {key} has a wrong flag list: at most 64 flags, each key once, on a flag field only")]
    BadFlags {
        /// The key of the field.
        key: String,
    },
}

/// True if the flag list of `f` is legal: empty on a field of another kind, else at most 64 entries with unique keys.
fn flags_ok(f: &FieldDecl) -> bool {
    let l = &f.flags;
    if f.kind != ValueKind::Flags {
        return l.is_empty();
    }
    l.len() <= FlagDecl::MAX && (1..l.len()).all(|i| l[..i].iter().all(|o| o.key != l[i].key))
}

/// An ordered list of field declarations with unique keys. It is built once and shared through an `Arc`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<FieldDecl>", into = "Vec<FieldDecl>")]
pub struct RecordDecl {
    fields: Vec<FieldDecl>,
    /// Field indices sorted by key.
    by_key: Vec<u16>,
}

impl RecordDecl {
    /// A declaration of these fields, in this order.
    ///
    /// # Errors
    ///
    /// [`DeclError::DuplicateKey`] if two fields share a key, [`DeclError::TooMany`] if there are more than 65535 fields,
    /// [`DeclError::ContentNotBytes`] if a field that is not bytes declares a content type, [`DeclError::BadFlags`] if a
    /// flag list has more than 64 entries or a key twice or is on a field of another kind.
    pub fn new(fields: Vec<FieldDecl>) -> Result<Self, DeclError> {
        let count = fields.len();
        if count > usize::from(u16::MAX) {
            return Err(DeclError::TooMany { count });
        }
        if let Some(f) = fields
            .iter()
            .find(|f| f.content.is_some() && f.kind != ValueKind::Bytes)
        {
            return Err(DeclError::ContentNotBytes {
                key: f.key.to_string(),
            });
        }
        if let Some(f) = fields.iter().find(|f| !flags_ok(f)) {
            return Err(DeclError::BadFlags {
                key: f.key.to_string(),
            });
        }
        let mut by_key: Vec<u16> = (0..count).filter_map(|i| u16::try_from(i).ok()).collect();
        by_key.sort_by(|a, b| {
            fields[usize::from(*a)]
                .key
                .cmp(&fields[usize::from(*b)].key)
        });
        if let Some(w) = by_key
            .windows(2)
            .find(|w| fields[usize::from(w[0])].key == fields[usize::from(w[1])].key)
        {
            return Err(DeclError::DuplicateKey {
                key: fields[usize::from(w[0])].key.to_string(),
            });
        }
        Ok(RecordDecl { fields, by_key })
    }
    /// A declaration of the fields of a constant list.
    ///
    /// # Panics
    ///
    /// If two fields share a key. A constant list with a repeated key is a programming error.
    pub fn of(fields: &[&dyn crate::publish::Declare]) -> Self {
        match Self::new(fields.iter().map(|f| f.decl()).collect()) {
            Ok(d) => d,
            Err(e) => panic!("{e}"),
        }
    }
    /// The id of the field with key `key`. This is the only lookup by text. Do it once and keep the id.
    pub fn id(&self, key: &str) -> Option<FieldId> {
        self.by_key
            .binary_search_by(|i| self.fields[usize::from(*i)].key.as_ref().cmp(key))
            .ok()
            .map(|p| FieldId(self.by_key[p]))
    }
    /// The declaration of field `id`.
    pub fn field(&self, id: FieldId) -> Option<&FieldDecl> {
        self.fields.get(id.index())
    }
    /// The declarations, in order.
    pub fn fields(&self) -> &[FieldDecl] {
        &self.fields
    }
    /// The number of fields.
    pub fn len(&self) -> usize {
        self.fields.len()
    }
    /// True if no field is declared.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

impl TryFrom<Vec<FieldDecl>> for RecordDecl {
    type Error = DeclError;
    fn try_from(v: Vec<FieldDecl>) -> Result<Self, DeclError> {
        Self::new(v)
    }
}

impl From<RecordDecl> for Vec<FieldDecl> {
    fn from(d: RecordDecl) -> Self {
        d.fields
    }
}

/// A field kind: how a value of it is written to a slot and read back. Implemented by the markers of [`field`] only.
pub trait Kind: sealed::Sealed + 'static {
    /// What a value is given as.
    type In<'a>;
    /// What a value is read as.
    type Out<'a>;
    /// The declared kind.
    const VALUE_KIND: ValueKind;
    /// The choice list of a field of this kind. Empty for kinds other than [`field::Choice`].
    fn choices() -> Cow<'static, [ChoiceDecl]> {
        Cow::Borrowed(&[])
    }
    /// The flag list of a field of this kind. Empty for kinds other than [`field::Flags`].
    fn flags() -> Cow<'static, [FlagDecl]> {
        Cow::Borrowed(&[])
    }
    /// Writes `v` into `slot`, in place.
    fn store(slot: &mut Value, v: Self::In<'_>);
    /// Writes `v` into `slot`, in place, and returns true if the slot changed. The default counts every write as a change.
    fn store_if_changed(slot: &mut Value, v: Self::In<'_>) -> bool {
        Self::store(slot, v);
        true
    }
    /// Reads a slot. `None` if it holds no value of this kind.
    fn load(slot: &Value) -> Option<Self::Out<'_>>;
}

/// Equality of two numbers by their bits, so that a NaN written twice is no change.
trait SameBits: Copy {
    fn same(self, o: Self) -> bool;
}
impl SameBits for f64 {
    fn same(self, o: f64) -> bool {
        self.to_bits() == o.to_bits()
    }
}
impl SameBits for i64 {
    fn same(self, o: i64) -> bool {
        self == o
    }
}
impl SameBits for bool {
    fn same(self, o: bool) -> bool {
        self == o
    }
}

macro_rules! number_kind {
    ($marker:ty, $variant:ident, $t:ty) => {
        impl Kind for $marker {
            type In<'a> = $t;
            type Out<'a> = $t;
            const VALUE_KIND: ValueKind = ValueKind::$variant;
            fn store(slot: &mut Value, v: $t) {
                *slot = Value::$variant(v);
            }
            fn store_if_changed(slot: &mut Value, v: $t) -> bool {
                match slot {
                    Value::$variant(o) if o.same(v) => false,
                    _ => {
                        *slot = Value::$variant(v);
                        true
                    }
                }
            }
            fn load(slot: &Value) -> Option<$t> {
                match slot {
                    Value::$variant(v) => Some(*v),
                    _ => None,
                }
            }
        }
    };
}

number_kind!(field::F64, F64, f64);
number_kind!(field::I64, I64, i64);
number_kind!(field::Bool, Bool, bool);
number_kind!(field::Hz, Hz, f64);
number_kind!(field::Time, Time, f64);
number_kind!(field::Duration, Duration, f64);

impl Kind for field::Position {
    type In<'a> = (f64, f64);
    type Out<'a> = (f64, f64);
    const VALUE_KIND: ValueKind = ValueKind::Position;
    fn store(slot: &mut Value, (lat, lon): (f64, f64)) {
        *slot = Value::Position { lat, lon };
    }
    fn store_if_changed(slot: &mut Value, (lat, lon): (f64, f64)) -> bool {
        match slot {
            Value::Position { lat: a, lon: b } if a.same(lat) && b.same(lon) => false,
            _ => {
                *slot = Value::Position { lat, lon };
                true
            }
        }
    }
    fn load(slot: &Value) -> Option<(f64, f64)> {
        match slot {
            Value::Position { lat, lon } => Some((*lat, *lon)),
            _ => None,
        }
    }
}

impl Kind for field::Text {
    type In<'a> = &'a str;
    type Out<'a> = &'a str;
    const VALUE_KIND: ValueKind = ValueKind::Text;
    fn store(slot: &mut Value, v: &str) {
        match slot {
            Value::Text(t) => {
                if t != v {
                    t.clear();
                    t.push_str(v);
                }
            }
            other => *other = Value::Text(v.to_owned()),
        }
    }
    fn store_if_changed(slot: &mut Value, v: &str) -> bool {
        match slot {
            Value::Text(t) if t == v => false,
            _ => {
                Self::store(slot, v);
                true
            }
        }
    }
    fn load(slot: &Value) -> Option<&str> {
        match slot {
            Value::Text(t) => Some(t),
            _ => None,
        }
    }
}

impl<E: Choice> Kind for field::Choice<E> {
    type In<'a> = E;
    type Out<'a> = E;
    const VALUE_KIND: ValueKind = ValueKind::Choice;
    fn choices() -> Cow<'static, [ChoiceDecl]> {
        Cow::Borrowed(E::CHOICES)
    }
    fn store(slot: &mut Value, v: E) {
        *slot = Value::Choice(v.index());
    }
    fn store_if_changed(slot: &mut Value, v: E) -> bool {
        let i = v.index();
        match slot {
            Value::Choice(o) if *o == i => false,
            _ => {
                *slot = Value::Choice(i);
                true
            }
        }
    }
    fn load(slot: &Value) -> Option<E> {
        match slot {
            Value::Choice(i) => E::from_index(*i),
            _ => None,
        }
    }
}

impl<F: FlagSet> Kind for field::Flags<F> {
    type In<'a> = F;
    type Out<'a> = F;
    const VALUE_KIND: ValueKind = ValueKind::Flags;
    fn flags() -> Cow<'static, [FlagDecl]> {
        Cow::Borrowed(F::FLAGS)
    }
    fn store(slot: &mut Value, v: F) {
        *slot = Value::Flags(v.bits());
    }
    fn store_if_changed(slot: &mut Value, v: F) -> bool {
        let m = v.bits();
        match slot {
            Value::Flags(o) if *o == m => false,
            _ => {
                *slot = Value::Flags(m);
                true
            }
        }
    }
    fn load(slot: &Value) -> Option<F> {
        match slot {
            Value::Flags(m) => Some(F::from_bits(*m)),
            _ => None,
        }
    }
}

impl Kind for field::Bytes {
    type In<'a> = Arc<[u8]>;
    type Out<'a> = &'a [u8];
    const VALUE_KIND: ValueKind = ValueKind::Bytes;
    fn store(slot: &mut Value, v: Arc<[u8]>) {
        *slot = Value::Bytes(v);
    }
    fn store_if_changed(slot: &mut Value, v: Arc<[u8]>) -> bool {
        match slot {
            Value::Bytes(o) if Arc::ptr_eq(o, &v) => false,
            _ => {
                *slot = Value::Bytes(v);
                true
            }
        }
    }
    fn load(slot: &Value) -> Option<&[u8]> {
        match slot {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }
}

impl Kind for field::Any {
    type In<'a> = &'a Value;
    type Out<'a> = &'a Value;
    const VALUE_KIND: ValueKind = ValueKind::Any;
    fn store(slot: &mut Value, v: &Value) {
        slot.clone_from(v);
    }
    fn store_if_changed(slot: &mut Value, v: &Value) -> bool {
        let same = match (&*slot, v) {
            (Value::Bytes(a), Value::Bytes(b)) => Arc::ptr_eq(a, b),
            (a, b) => a == b,
        };
        if !same {
            slot.clone_from(v);
        }
        !same
    }
    fn load(slot: &Value) -> Option<&Value> {
        (!slot.is_none()).then_some(slot)
    }
}

static DROPPED_WRITES: AtomicU64 = AtomicU64::new(0);

/// The number of record writes dropped since the start because the field was not declared or was declared with another kind.
/// Only release builds drop. Debug builds panic.
pub fn dropped_writes() -> u64 {
    DROPPED_WRITES.load(Ordering::Relaxed)
}

/// Refuses a write that is a programming error: a field or setting `key` that does not take the value, for the reason `why`.
/// Panics in debug builds. In release builds, counts the dropped write in [`dropped_writes`].
pub fn refuse_write(key: &str, why: &str) {
    if cfg!(debug_assertions) {
        panic!("{key}: {why}");
    }
    DROPPED_WRITES.fetch_add(1, Ordering::Relaxed);
}

/// Why a write by id was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RecordError {
    /// The record declares no field with this id.
    #[error("no field number {}", .id.0)]
    NoSuchField {
        /// The id.
        id: FieldId,
    },
    /// The value is of another kind than the field.
    #[error("field number {} is {expected:?}, the value is {got:?}", .id.0)]
    WrongKind {
        /// The field.
        id: FieldId,
        /// The declared kind.
        expected: ValueKind,
        /// The kind of the value.
        got: ValueKind,
    },
    /// The choice index is outside the field's list.
    #[error("field number {} has no choice {index}", .id.0)]
    NotAChoice {
        /// The field.
        id: FieldId,
        /// The index.
        index: u16,
    },
    /// The mask has a bit outside the field's flag list.
    #[error("field number {} has no flag for a bit of the mask {mask:#x}", .id.0)]
    NotAFlag {
        /// The field.
        id: FieldId,
        /// The mask.
        mask: u64,
    },
}

/// One value slot per field of a [`RecordDecl`]. A new record holds [`Value::None`] in every slot.
///
/// Each write returns true if it changed a slot. A write of an equal number, flag, choice, position or text is no change.
#[derive(Debug)]
pub struct Record {
    decl: Arc<RecordDecl>,
    slots: Vec<Value>,
}

impl Record {
    /// A record of `decl` with no values.
    pub fn new(decl: Arc<RecordDecl>) -> Self {
        let slots = vec![Value::None; decl.len()];
        Record { decl, slots }
    }
    /// Its declaration.
    pub fn decl(&self) -> &Arc<RecordDecl> {
        &self.decl
    }
    fn slot_of(&self, key: &str, kind: ValueKind) -> Option<usize> {
        match self.decl.id(key) {
            Some(id) if self.decl.fields[id.index()].kind == kind => Some(id.index()),
            Some(_) => {
                refuse_write(key, "a field declared with another kind");
                None
            }
            None => {
                refuse_write(key, "a field that is not declared");
                None
            }
        }
    }
    /// Writes field `f` and returns true if the slot changed. Nothing is allocated, except for a text longer than the one in
    /// the slot.
    ///
    /// # Panics
    ///
    /// In debug builds, if the record does not declare `f` with its kind. In release builds the write is dropped and counted
    /// ([`dropped_writes`]).
    pub fn set<K: Kind>(&mut self, f: &Field<K>, v: K::In<'_>) -> bool {
        match self.slot_of(f.key, K::VALUE_KIND) {
            Some(i) => K::store_if_changed(&mut self.slots[i], v),
            None => false,
        }
    }
    /// Marks field `f` as having no value now, and returns true if the slot changed. Panics and counts as [`Record::set`]
    /// does.
    pub fn none<K: Kind>(&mut self, f: &Field<K>) -> bool {
        match self.slot_of(f.key, K::VALUE_KIND) {
            Some(i) if !matches!(self.slots[i], Value::None) => {
                self.slots[i] = Value::None;
                true
            }
            _ => false,
        }
    }
    /// Writes field `f` if `v` is `Some`, else marks it as having no value. Returns true if the slot changed.
    pub fn opt<K: Kind>(&mut self, f: &Field<K>, v: Option<K::In<'_>>) -> bool {
        match v {
            Some(v) => self.set(f, v),
            None => self.none(f),
        }
    }
    /// The value of field `f`. `None` if the record does not declare it or it holds no value.
    pub fn get<K: Kind>(&self, f: &Field<K>) -> Option<K::Out<'_>> {
        let id = self.decl.id(f.key)?;
        K::load(&self.slots[id.index()])
    }
    /// Writes the catalogue entry `index` into field `f` and returns true if the slot changed. Panics and counts as
    /// [`Record::set`] does, and also for an index outside the catalogue.
    pub fn set_entry(&mut self, f: &CatalogueField, index: u16) -> bool {
        let Some(i) = self.slot_of(f.key, ValueKind::Choice) else {
            return false;
        };
        if usize::from(index) >= self.decl.fields[i].choices.len() {
            refuse_write(f.key, "a catalogue index outside the list");
            return false;
        }
        match self.slots[i] {
            Value::Choice(o) if o == index => false,
            _ => {
                self.slots[i] = Value::Choice(index);
                true
            }
        }
    }
    /// The catalogue entry of field `f`. `None` if it holds none.
    pub fn entry(&self, f: &CatalogueField) -> Option<u16> {
        match self.slots[self.decl.id(f.key)?.index()] {
            Value::Choice(i) => Some(i),
            _ => None,
        }
    }
    /// The value of field `id`.
    pub fn value(&self, id: FieldId) -> Option<&Value> {
        self.slots.get(id.index())
    }
    /// Writes field `id` from a value, in place, and returns true if the slot changed.
    ///
    /// # Errors
    ///
    /// [`RecordError`] if the field does not exist, the kind differs, the choice index is outside the list or the mask
    /// has a bit outside the flag list.
    pub fn set_value(&mut self, id: FieldId, v: &Value) -> Result<bool, RecordError> {
        let decl = self.decl.field(id).ok_or(RecordError::NoSuchField { id })?;
        let any = decl.kind == ValueKind::Any;
        if let Some(got) = v.kind().filter(|_| !any) {
            if got != decl.kind {
                return Err(RecordError::WrongKind {
                    id,
                    expected: decl.kind,
                    got,
                });
            }
        }
        if let (false, Value::Choice(index)) = (any, v) {
            if usize::from(*index) >= decl.choices.len() {
                return Err(RecordError::NotAChoice { id, index: *index });
            }
        }
        if let (false, Value::Flags(mask)) = (any, v) {
            if mask & !decl.flag_mask() != 0 {
                return Err(RecordError::NotAFlag { id, mask: *mask });
            }
        }
        let slot = &mut self.slots[id.index()];
        let same = match (&*slot, v) {
            (Value::Bytes(a), Value::Bytes(b)) => Arc::ptr_eq(a, b),
            (a, b) => a == b,
        };
        if !same {
            slot.clone_from(v);
        }
        Ok(!same)
    }
    /// The text form of field `id` for people.
    pub fn text(&self, id: FieldId) -> Option<crate::value::FieldText<'_>> {
        let d = self.decl.field(id)?;
        Some(self.slots[id.index()].display(&d.choices).with_flags(&d.flags))
    }
    /// The declared fields with their values, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&FieldDecl, &Value)> {
        self.decl.fields.iter().zip(&self.slots)
    }
    /// Sets every slot to [`Value::None`], and returns true if a slot changed.
    pub fn clear(&mut self) -> bool {
        let changed = self.slots.iter().any(|v| !matches!(v, Value::None));
        if changed {
            self.slots.fill(Value::None);
        }
        changed
    }
}

impl Clone for Record {
    fn clone(&self) -> Self {
        Record {
            decl: Arc::clone(&self.decl),
            slots: self.slots.clone(),
        }
    }
    fn clone_from(&mut self, s: &Self) {
        if !Arc::ptr_eq(&self.decl, &s.decl) {
            self.decl = Arc::clone(&s.decl);
        }
        self.slots.clone_from(&s.slots);
    }
}

/// Two records are equal if their declarations and values are equal.
impl PartialEq for Record {
    fn eq(&self, o: &Self) -> bool {
        (Arc::ptr_eq(&self.decl, &o.decl) || self.decl == o.decl) && self.slots == o.slots
    }
}

/// A record serialises as a map from each declared key to its value.
impl Serialize for Record {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.slots.len()))?;
        for (d, v) in self.iter() {
            m.serialize_entry(d.key.as_ref(), v)?;
        }
        m.end()
    }
}

/// Reads a [`Record`] of a known declaration from the map form that [`Record`] serialises to.
///
/// The read is strict: an undeclared key, a value of the wrong kind or a choice index outside the list is an error.
#[derive(Clone, Copy, Debug)]
pub struct RecordSeed<'a>(pub &'a Arc<RecordDecl>);

impl<'de> DeserializeSeed<'de> for RecordSeed<'_> {
    type Value = Record;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Record, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for RecordSeed<'_> {
    type Value = Record;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a map of declared keys to values")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Record, A::Error> {
        use serde::de::Error;
        let mut r = Record::new(Arc::clone(self.0));
        while let Some(key) = map.next_key::<Cow<'de, str>>()? {
            let id = self
                .0
                .id(&key)
                .ok_or_else(|| A::Error::custom(format_args!("undeclared key {key}")))?;
            let v: Value = map.next_value()?;
            r.set_value(id, &v).map_err(A::Error::custom)?;
        }
        Ok(r)
    }
}
