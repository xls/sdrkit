//! The event kinds that the services read: the standard fields of each kind, found once, and the resolution of names in user
//! text (a rule, a line format, a watch) to kind sets and field references.

use std::sync::Arc;

use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::publish::Declare;
use sdrkit_core::{Event, EventCatalog, EventKindId, FieldId};

use crate::observation::{Context, Observation};
use crate::Position;

/// Why a name in user text does not resolve.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    /// No event kind matches the kind or pattern.
    #[error("no event kind matches {name} (at {at})")]
    UnknownKind {
        /// The name or pattern.
        name: String,
        /// The byte position of the name in the text.
        at: usize,
    },
    /// No event kind that the rule reads has a field with this name.
    #[error("no event kind has the field {name} (at {at})")]
    UnknownField {
        /// The name.
        name: String,
        /// The byte position of the name in the text.
        at: usize,
    },
    /// The family name is not one of [`Family`].
    #[error("there is no family {name} (at {at})")]
    UnknownFamily {
        /// The name.
        name: String,
        /// The byte position of the name in the text.
        at: usize,
    },
}

/// A value that every observation has, whatever its kind. User text names it by its key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Standard {
    /// The kind key (`"kind"`).
    Kind,
    /// The family key (`"family"`).
    Family,
    /// The urgency key (`"urgency"`).
    Urgency,
    /// True if the urgency is above none (`"urgent"`).
    Urgent,
    /// The thing the event is about (`"source"`).
    Identity,
    /// The text for people (`"text"`).
    Text,
    /// The frequency, Hz (`"freq_hz"`).
    FreqHz,
    /// The frequency, MHz (`"freq_mhz"`).
    FreqMhz,
    /// The latitude, degrees (`"lat"`).
    Lat,
    /// The longitude, degrees (`"lon"`).
    Lon,
    /// The distance from the receiver, km (`"distance_km"`).
    DistanceKm,
    /// The bearing from the receiver, degrees (`"bearing_deg"`).
    BearingDeg,
    /// The wall-clock time, Unix seconds (`"unix_time"`).
    UnixTime,
    /// The signal time, seconds since the receiver started (`"signal_time"`).
    SignalTime,
}

impl Standard {
    /// Every standard value with its key.
    pub const ALL: [(&'static str, Standard); 14] = [
        ("kind", Standard::Kind),
        ("family", Standard::Family),
        ("urgency", Standard::Urgency),
        ("urgent", Standard::Urgent),
        ("source", Standard::Identity),
        ("text", Standard::Text),
        ("freq_hz", Standard::FreqHz),
        ("freq_mhz", Standard::FreqMhz),
        ("lat", Standard::Lat),
        ("lon", Standard::Lon),
        ("distance_km", Standard::DistanceKm),
        ("bearing_deg", Standard::BearingDeg),
        ("unix_time", Standard::UnixTime),
        ("signal_time", Standard::SignalTime),
    ];

    /// The standard value with key `key`.
    pub fn of(key: &str) -> Option<Standard> {
        Self::ALL.iter().find(|(k, _)| *k == key).map(|(_, s)| *s)
    }

    /// Its key.
    pub fn key(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, s)| *s == self)
            .map_or("", |(k, _)| k)
    }
}

/// A field that user text names, resolved: a standard value, or a declared field with its id in each kind.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldRef {
    /// A value every observation has.
    Standard(Standard),
    /// A declared field. The slot of kind `k` is at index `k`. `None` where the kind does not declare it.
    Declared(Arc<[Option<FieldId>]>),
}

/// A set of event kinds, resolved from user text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KindSet {
    bits: Box<[u64]>,
}

impl KindSet {
    fn empty(len: usize) -> Self {
        KindSet {
            bits: vec![0; len.div_ceil(64)].into_boxed_slice(),
        }
    }
    fn insert(&mut self, id: EventKindId) {
        let i = usize::from(id.0);
        if let Some(w) = self.bits.get_mut(i / 64) {
            *w |= 1 << (i % 64);
        }
    }
    /// True if kind `id` is in the set.
    pub fn contains(&self, id: EventKindId) -> bool {
        let i = usize::from(id.0);
        self.bits
            .get(i / 64)
            .is_some_and(|w| w & (1 << (i % 64)) != 0)
    }
    /// True if the set holds no kind.
    pub fn is_empty(&self) -> bool {
        self.bits.iter().all(|w| *w == 0)
    }
    /// Adds every kind of `other`.
    pub fn add(&mut self, other: &KindSet) {
        for (a, b) in self.bits.iter_mut().zip(other.bits.iter()) {
            *a |= b;
        }
    }
}

/// A set of families. The empty set stands for every family where a filter uses it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FamilySet(u32);

impl FamilySet {
    /// Adds `f`.
    pub fn insert(&mut self, f: Family) {
        self.0 |= 1 << <Family as sdrkit_core::Choice>::index(f);
    }
    /// True if the set holds `f`.
    pub fn contains(self, f: Family) -> bool {
        self.0 & (1 << <Family as sdrkit_core::Choice>::index(f)) != 0
    }
    /// True if the set holds no family.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    /// True if the set is empty or holds `f`.
    pub fn admits(self, f: Family) -> bool {
        self.is_empty() || self.contains(f)
    }
}

/// The parts of a list separated by commas, trimmed, each with its byte position. Empty parts are left out.
pub fn split_list(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut start = 0;
    text.split(',').filter_map(move |part| {
        let at = start + (part.len() - part.trim_start().len());
        start += part.len() + 1;
        let p = part.trim();
        (!p.is_empty()).then_some((at, p))
    })
}

/// The slots of the standard fields in one kind.
#[derive(Clone, Copy, Debug, Default)]
struct Slots {
    urgency: Option<FieldId>,
    family: Option<FieldId>,
    private: Option<FieldId>,
    source: Option<FieldId>,
    text: Option<FieldId>,
    position: Option<FieldId>,
    height: Option<FieldId>,
    target: Option<FieldId>,
    channel: Option<FieldId>,
}

/// Every event kind of the engine's catalog, with the slots of its standard fields. Build it once when the catalog is made.
#[derive(Debug)]
pub struct Kinds {
    catalog: Arc<EventCatalog>,
    slots: Vec<Slots>,
}

impl Kinds {
    /// The kinds of `catalog`.
    pub fn new(catalog: Arc<EventCatalog>) -> Self {
        let slots = (0..catalog.len())
            .filter_map(|i| u16::try_from(i).ok())
            .map(|i| {
                let Some(decl) = catalog.fields(EventKindId(i)) else {
                    return Slots::default();
                };
                let slot = |f: &dyn Declare| {
                    let d = f.decl();
                    decl.id(&d.key)
                        .filter(|id| decl.field(*id).is_some_and(|x| x.kind == d.kind))
                };
                Slots {
                    urgency: slot(&keys::URGENCY),
                    family: slot(&keys::FAMILY),
                    private: slot(&keys::PRIVATE),
                    source: slot(&keys::SOURCE),
                    text: slot(&keys::TEXT),
                    position: slot(&keys::POSITION),
                    height: slot(&keys::HEIGHT_M),
                    target: slot(&keys::TARGET),
                    channel: slot(&keys::CHANNEL),
                }
            })
            .collect();
        Kinds { catalog, slots }
    }

    /// The catalog.
    pub fn catalog(&self) -> &Arc<EventCatalog> {
        &self.catalog
    }

    /// The observation of `event` with the wall clock and the receiver of `ctx`. `None` if the event carries no kind id
    /// ([`EventCatalog::stamp`] assigns one), or was not made from the catalog's definition of its kind.
    pub fn observe<'e>(&self, event: &'e Event, ctx: &Context) -> Option<Observation<'e>> {
        let kind = event.kind_id()?;
        self.catalog.def(kind)?;
        if !Arc::ptr_eq(event.fields.decl(), self.catalog.fields(kind)?) {
            return None;
        }
        let s = self.slots.get(usize::from(kind.0))?;
        let val = |id: Option<FieldId>| id.and_then(|id| event.fields.value(id));
        let choice = |id| match val(id) {
            Some(sdrkit_core::Value::Choice(i)) => Some(*i),
            _ => None,
        };
        let text = |id| match val(id) {
            Some(sdrkit_core::Value::Text(t)) if !t.trim().is_empty() => Some(t.trim()),
            _ => None,
        };
        let urgency = choice(s.urgency)
            .and_then(<Urgency as sdrkit_core::Choice>::from_index)
            .unwrap_or(Urgency::None);
        let family = choice(s.family)
            .and_then(<Family as sdrkit_core::Choice>::from_index)
            .unwrap_or(Family::Other);
        let private = matches!(val(s.private), Some(sdrkit_core::Value::Bool(true)));
        let height_m = match val(s.height) {
            Some(sdrkit_core::Value::F64(h)) if h.is_finite() => Some(*h),
            _ => None,
        };
        let position = match val(s.position) {
            Some(sdrkit_core::Value::Position { lat, lon }) => {
                Position::checked(*lat, *lon, height_m.unwrap_or(0.0)).ok()
            }
            _ => None,
        };
        Some(Observation::new(
            event,
            kind,
            ctx,
            family,
            urgency,
            private,
            text(s.source),
            text(s.target),
            text(s.channel),
            if private { None } else { text(s.text) },
            position,
            height_m,
        ))
    }

    /// The kinds that `pattern` names: `"*"` (all), a kind key, or a prefix that ends in `*` (`"adsb.*"`). `at` is the byte
    /// position of the pattern in the user's text.
    ///
    /// # Errors
    ///
    /// [`NameError::UnknownKind`] if no kind matches.
    pub fn kinds(&self, pattern: &str, at: usize) -> Result<KindSet, NameError> {
        let p = pattern.trim();
        let mut set = KindSet::empty(self.catalog.len());
        for i in 0..self.catalog.len() {
            let Some(id) = u16::try_from(i).ok().map(EventKindId) else {
                continue;
            };
            let Some(def) = self.catalog.def(id) else {
                continue;
            };
            let hit = match p.strip_suffix('*') {
                Some(prefix) => def.kind.starts_with(prefix),
                None => def.kind == p,
            };
            if hit {
                set.insert(id);
            }
        }
        if set.is_empty() {
            return Err(NameError::UnknownKind {
                name: p.to_owned(),
                at,
            });
        }
        Ok(set)
    }

    /// The kinds that a list of patterns names, separated by commas (`"adsb.*, ais.vessel"`). `None` for empty text (every
    /// kind). `at` is the byte position of `text` in the user's text.
    ///
    /// # Errors
    ///
    /// [`NameError::UnknownKind`] with the position of the first pattern that matches no kind.
    pub fn kind_list(&self, text: &str, at: usize) -> Result<Option<KindSet>, NameError> {
        let mut set: Option<KindSet> = None;
        for (i, part) in split_list(text) {
            let one = self.kinds(part, at + i)?;
            match &mut set {
                Some(s) => s.add(&one),
                None => set = Some(one),
            }
        }
        Ok(set)
    }

    /// Every kind.
    pub fn all(&self) -> KindSet {
        let mut set = KindSet::empty(self.catalog.len());
        for i in 0..self.catalog.len() {
            if let Ok(i) = u16::try_from(i) {
                set.insert(EventKindId(i));
            }
        }
        set
    }

    /// The family with key `name`. `at` is the byte position of the name in the user's text.
    ///
    /// # Errors
    ///
    /// [`NameError::UnknownFamily`] if no family has this key.
    pub fn family(name: &str, at: usize) -> Result<Family, NameError> {
        let n = name.trim();
        <Family as sdrkit_core::Choice>::CHOICES
            .iter()
            .position(|c| c.key == n)
            .and_then(|i| u16::try_from(i).ok())
            .and_then(<Family as sdrkit_core::Choice>::from_index)
            .ok_or_else(|| NameError::UnknownFamily {
                name: n.to_owned(),
                at,
            })
    }

    /// The families that a list of family keys names, separated by commas. The empty set for empty text. `at` is the byte
    /// position of `text` in the user's text.
    ///
    /// # Errors
    ///
    /// [`NameError::UnknownFamily`] with the position of the first key that is not a family.
    pub fn family_list(text: &str, at: usize) -> Result<FamilySet, NameError> {
        let mut set = FamilySet::default();
        for (i, part) in split_list(text) {
            set.insert(Kinds::family(part, at + i)?);
        }
        Ok(set)
    }

    /// The field that `name` names: a [`Standard`] value, or a field that a kind of `within` declares (every kind if `within`
    /// is `None`). `at` is the byte position of the name in the user's text.
    ///
    /// # Errors
    ///
    /// [`NameError::UnknownField`] if no kind of `within` declares the name and it is not a standard value.
    pub fn field(
        &self,
        name: &str,
        at: usize,
        within: Option<&KindSet>,
    ) -> Result<FieldRef, NameError> {
        if let Some(s) = Standard::of(name) {
            return Ok(FieldRef::Standard(s));
        }
        let mut any = false;
        let ids: Arc<[Option<FieldId>]> = (0..self.catalog.len())
            .map(|i| {
                let id = EventKindId(u16::try_from(i).ok()?);
                if within.is_some_and(|w| !w.contains(id)) {
                    return None;
                }
                let f = self.catalog.fields(id)?.id(name);
                any |= f.is_some();
                f
            })
            .collect();
        if !any {
            return Err(NameError::UnknownField {
                name: name.to_owned(),
                at,
            });
        }
        Ok(FieldRef::Declared(ids))
    }
}
