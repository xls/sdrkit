//! Discrete results for logs, automation and other programs. Each event kind is declared once as a `static` [`EventDef`], and
//! the [`EventCatalog`] assigns each kind an id.

use crate::publish::{keys, Declare, Field};
use crate::record::{DeclError, Kind, Record, RecordDecl};
use serde::Serialize;
use std::borrow::Cow;
use std::sync::{Arc, OnceLock};

/// An event kind, declared once as a `static`: its kind key (`"adsb.aircraft"`), a label for people and its fields.
///
/// ```
/// use sdrkit_core::publish::{field, Field};
/// use sdrkit_core::EventDef;
/// const SQUAWK: Field<field::I64> = Field::new("squawk", "Squawk", "");
/// static AIRCRAFT: EventDef = EventDef::new("adsb.aircraft", "Aircraft seen", &[&SQUAWK]);
/// let e = AIRCRAFT.event(12.0, 1090e6).set(&SQUAWK, 7000);
/// assert!(e.is(&AIRCRAFT));
/// assert_eq!(e.get(&SQUAWK), Some(7000));
/// ```
pub struct EventDef {
    /// The kind key.
    pub kind: &'static str,
    /// What people see (`"Aircraft seen"`).
    pub label: &'static str,
    /// The fields an event of this kind can carry.
    pub fields: &'static [&'static dyn Declare],
    decl: OnceLock<Arc<RecordDecl>>,
}

/// The fields of the standard spot event (a station heard): call sign, locator, SNR, frequency and emission type.
pub const SPOT_FIELDS: &[&dyn Declare] = &[
    &keys::CALLSIGN,
    &keys::LOCATOR,
    &keys::SNR_DB,
    &keys::FREQ_HZ,
    &keys::SPOT_MODE,
];

impl EventDef {
    /// An event kind with these fields.
    pub const fn new(
        kind: &'static str,
        label: &'static str,
        fields: &'static [&'static dyn Declare],
    ) -> Self {
        EventDef {
            kind,
            label,
            fields,
            decl: OnceLock::new(),
        }
    }

    /// A spot event kind: a station heard, with the [`SPOT_FIELDS`].
    pub const fn spot(kind: &'static str, label: &'static str) -> Self {
        Self::new(kind, label, SPOT_FIELDS)
    }

    /// The shared declaration of its fields, built on the first call.
    ///
    /// # Errors
    ///
    /// [`DeclError`] if two fields share a key.
    pub fn record_decl(&self) -> Result<&Arc<RecordDecl>, DeclError> {
        if let Some(d) = self.decl.get() {
            return Ok(d);
        }
        let d = Arc::new(RecordDecl::new(
            self.fields.iter().map(|f| f.decl()).collect(),
        )?);
        Ok(self.decl.get_or_init(|| d))
    }

    fn shared(&self) -> &Arc<RecordDecl> {
        match self.record_decl() {
            Ok(d) => d,
            Err(e) => panic!("event kind {}: {e}", self.kind),
        }
    }

    /// Its declaration, as data.
    ///
    /// # Panics
    ///
    /// If two fields share a key.
    pub fn decl(&self) -> EventDecl {
        EventDecl {
            kind: Cow::Borrowed(self.kind),
            label: Cow::Borrowed(self.label),
            fields: Arc::clone(self.shared()),
        }
    }

    /// A new event of this kind at signal time `time` (seconds since the receiver started) on `freq_hz`, with no field values.
    /// Only the value slots are allocated.
    ///
    /// # Panics
    ///
    /// If two fields share a key.
    pub fn event(&'static self, time: f64, freq_hz: f64) -> Event {
        Event {
            kind: Cow::Borrowed(self.kind),
            kind_id: None,
            time,
            freq_hz,
            fields: Record::new(Arc::clone(self.shared())),
        }
    }
}

impl std::fmt::Debug for EventDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventDef")
            .field("kind", &self.kind)
            .finish()
    }
}

/// The declaration of an event kind, as data: its kind key, a label and its fields.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EventDecl {
    /// The kind key.
    pub kind: Cow<'static, str>,
    /// What people see.
    pub label: Cow<'static, str>,
    /// The fields.
    pub fields: Arc<RecordDecl>,
}

/// The index of an event kind in its [`EventCatalog`].
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, serde::Deserialize,
)]
pub struct EventKindId(pub u16);

/// One event: its kind, when and where it was received, and a record of the kind's fields.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Event {
    kind: Cow<'static, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind_id: Option<EventKindId>,
    /// Seconds since the receiver started, in the signal's own time.
    pub time: f64,
    /// The frequency the event was received on, in Hz.
    pub freq_hz: f64,
    /// The fields.
    pub fields: Record,
}

impl Event {
    /// The same event with field `f` written.
    ///
    /// # Panics
    ///
    /// As [`Record::set`] does.
    #[must_use]
    pub fn set<K: Kind>(mut self, f: &Field<K>, v: K::In<'_>) -> Self {
        self.fields.set(f, v);
        self
    }
    /// The same event with field `f` written if `v` is `Some`.
    #[must_use]
    pub fn opt<K: Kind>(mut self, f: &Field<K>, v: Option<K::In<'_>>) -> Self {
        self.fields.opt(f, v);
        self
    }
    /// True if the event is of kind `def`.
    pub fn is(&self, def: &EventDef) -> bool {
        self.kind == def.kind
    }
    /// The value of field `f`. `None` if the kind does not declare it or it holds no value.
    pub fn get<K: Kind>(&self, f: &Field<K>) -> Option<K::Out<'_>> {
        self.fields.get(f)
    }
    /// The kind key.
    /// The text of field `key` for people. Empty if the event has no such field or the field has no value.
    pub fn text(&self, key: &str) -> String {
        self.fields
            .decl()
            .id(key)
            .and_then(|id| self.fields.text(id))
            .map(|t| t.to_string())
            .unwrap_or_default()
    }
    /// The kind of the event.
    pub fn kind(&self) -> &str {
        &self.kind
    }
    /// The id of the kind, if the event was made by an [`EventCatalog`].
    pub fn kind_id(&self) -> Option<EventKindId> {
        self.kind_id
    }
}

/// Why a list of event kinds is not a catalog.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CatalogError {
    /// Two kinds have the same key.
    #[error("the event kind {kind} is declared twice")]
    DuplicateKind {
        /// The kind key.
        kind: Cow<'static, str>,
    },
    /// A kind's fields are not a record declaration.
    #[error("event kind {kind}: {source}")]
    Fields {
        /// The kind key.
        kind: Cow<'static, str>,
        /// What is wrong with the fields.
        source: DeclError,
    },
    /// The list has more kinds than an [`EventKindId`] can count.
    #[error("{count} event kinds is more than 65535")]
    TooMany {
        /// The number of kinds.
        count: usize,
    },
}

/// Every declared event kind, each with an id and one shared field declaration.
#[derive(Debug)]
pub struct EventCatalog {
    kinds: Vec<EventDecl>,
    /// Kind indices sorted by kind key.
    by_kind: Vec<u16>,
}

impl EventCatalog {
    /// A catalog of these kinds. The id of a kind is its position in `defs`.
    ///
    /// # Errors
    ///
    /// [`CatalogError`] if a kind is listed twice, the fields of a kind repeat a key, or there are more than 65535 kinds.
    pub fn new(defs: &[&'static EventDef]) -> Result<Self, CatalogError> {
        let mut decls = Vec::with_capacity(defs.len());
        for d in defs {
            d.record_decl().map_err(|source| CatalogError::Fields {
                kind: Cow::Borrowed(d.kind),
                source,
            })?;
            decls.push(d.decl());
        }
        Self::from_decls(&decls)
    }
    /// A catalog of these declared kinds (what owners give through [`Owner::events`](crate::Owner::events)). The id of a
    /// kind is its position in `decls`; an event is stamped only if its fields share the declaration listed here.
    ///
    /// # Errors
    ///
    /// [`CatalogError`] if a kind is listed twice or there are more than 65535 kinds.
    pub fn from_decls(decls: &[EventDecl]) -> Result<Self, CatalogError> {
        if decls.len() > usize::from(u16::MAX) {
            return Err(CatalogError::TooMany { count: decls.len() });
        }
        let kinds: Vec<EventDecl> = decls.to_vec();
        let mut by_kind: Vec<u16> = (0..kinds.len())
            .filter_map(|i| u16::try_from(i).ok())
            .collect();
        by_kind.sort_by(|a, b| {
            kinds[usize::from(*a)]
                .kind
                .cmp(&kinds[usize::from(*b)].kind)
        });
        if let Some(w) = by_kind
            .windows(2)
            .find(|w| kinds[usize::from(w[0])].kind == kinds[usize::from(w[1])].kind)
        {
            return Err(CatalogError::DuplicateKind {
                kind: kinds[usize::from(w[0])].kind.clone(),
            });
        }
        Ok(EventCatalog { kinds, by_kind })
    }
    /// The id of the kind with key `kind`. This is the only lookup by text. Do it once and keep the id.
    pub fn id(&self, kind: &str) -> Option<EventKindId> {
        self.by_kind
            .binary_search_by(|i| self.kinds[usize::from(*i)].kind.as_ref().cmp(kind))
            .ok()
            .map(|p| EventKindId(self.by_kind[p]))
    }
    /// The id of kind `def`.
    pub fn id_of(&self, def: &EventDef) -> Option<EventKindId> {
        self.id(def.kind)
    }
    /// The declaration of kind `id`.
    pub fn def(&self, id: EventKindId) -> Option<&EventDecl> {
        self.kinds.get(usize::from(id.0))
    }
    /// The field declaration of kind `id`.
    pub fn fields(&self, id: EventKindId) -> Option<&Arc<RecordDecl>> {
        self.kinds.get(usize::from(id.0)).map(|k| &k.fields)
    }
    /// A new event of kind `id`, with no field values. It shares the kind's field declaration. `None` if `id` is not in the
    /// catalog.
    pub fn event(&self, id: EventKindId, time: f64, freq_hz: f64) -> Option<Event> {
        let decl = self.kinds.get(usize::from(id.0))?;
        Some(Event {
            kind: decl.kind.clone(),
            kind_id: Some(id),
            time,
            freq_hz,
            fields: Record::new(Arc::clone(&decl.fields)),
        })
    }
    /// Writes the id of the kind of `event` into it, and returns the id. The engine calls this once, when the event enters.
    /// `None` if the kind is not in the catalog, or the event does not share the catalog's field declaration of its kind.
    pub fn stamp(&self, event: &mut Event) -> Option<EventKindId> {
        let id = self.id(&event.kind)?;
        if !Arc::ptr_eq(event.fields.decl(), self.fields(id)?) {
            return None;
        }
        event.kind_id = Some(id);
        Some(id)
    }
    /// The number of kinds.
    pub fn len(&self) -> usize {
        self.kinds.len()
    }
    /// True if the catalog has no kinds.
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static SPOT: EventDef = EventDef::spot("test.spot", "Spot");
    static OTHER: EventDef = EventDef::spot("test.other", "Other");

    #[test]
    fn the_catalog_stamps_the_kind_id_of_an_event_made_from_its_def() {
        static DEFS: [&EventDef; 1] = [&SPOT];
        let catalog = EventCatalog::new(&DEFS).unwrap();
        let mut e = SPOT.event(1.0, 7.074e6);
        assert_eq!(e.kind_id(), None);
        assert_eq!(catalog.stamp(&mut e), catalog.id_of(&SPOT));
        assert_eq!(e.kind_id(), catalog.id_of(&SPOT));
        let mut other = OTHER.event(1.0, 7.074e6);
        assert_eq!(catalog.stamp(&mut other), None);
        assert_eq!(other.kind_id(), None);
    }
}
