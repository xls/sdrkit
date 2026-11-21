//! One stored message, and its line in the journal.

use std::fmt::Write as _;

use std::borrow::Cow;

use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::record::RecordSeed;
use sdrkit_core::{EventKindId, Record, Value};
use sdrkit_svc_events::plain::{Coords, PlainFields};
use sdrkit_svc_events::{choice_key, choice_of_key, Kinds, Observation, Position};
use serde::de::DeserializeSeed;
use serde::{Deserialize, Serialize};

/// A message in the store.
#[derive(Clone, Debug)]
pub struct Message {
    /// Unique and increasing within a store.
    pub id: u64,
    /// When it was first seen, Unix seconds.
    pub time: f64,
    /// When it was last seen. Later than `time` if repeats were folded in.
    pub last_time: f64,
    /// The key of its event kind.
    pub kind_key: Cow<'static, str>,
    /// The id of its event kind.
    pub kind: EventKindId,
    /// The family.
    pub family: Family,
    /// The urgency.
    pub urgency: Urgency,
    /// The thing it is about.
    pub identity: Option<String>,
    /// Frequency, Hz.
    pub freq_hz: f64,
    /// Position.
    pub position: Option<Position>,
    /// Height, metres, if the event returned one.
    pub height_m: Option<f64>,
    /// Distance from the receiver, km.
    pub distance_km: Option<f64>,
    /// The text for people.
    pub text: Option<String>,
    /// How many later reports the throttle folded into this one.
    pub repeats: u32,
    /// The event's fields (those of the newest report if repeats were folded in). A private message keeps no text.
    pub fields: Record,
    /// Lower-case words for search.
    pub(crate) haystack: String,
    /// The size of its journal line, bytes.
    pub(crate) bytes: usize,
}

impl Message {
    /// A message of an observation, with id 0. [`Message::refresh`] fills the search text and the size.
    pub(crate) fn new(o: &Observation<'_>, kind_key: Cow<'static, str>) -> Self {
        let mut fields = o.event.fields.clone();
        if o.private {
            let ids: Vec<_> = (0..fields.decl().len())
                .filter_map(|i| u16::try_from(i).ok().map(sdrkit_core::FieldId))
                .filter(|id| matches!(fields.value(*id), Some(Value::Text(_))))
                .collect();
            for id in ids {
                let _ = fields.set_value(id, &Value::None);
            }
        }
        Message {
            id: 0,
            time: o.unix_time,
            last_time: o.unix_time,
            kind_key,
            kind: o.kind,
            family: o.family,
            urgency: o.urgency,
            identity: o.identity.map(str::to_owned),
            freq_hz: o.event.freq_hz,
            position: o.position,
            height_m: o.height_m,
            distance_km: o.distance_km,
            text: o.text.map(str::to_owned),
            repeats: 0,
            fields,
            haystack: String::new(),
            bytes: 0,
        }
    }

    /// Rebuilds the search text and the size after a change or a load, and returns the journal line.
    pub(crate) fn refresh(&mut self) -> Result<String, serde_json::Error> {
        let mut h = String::new();
        for s in [
            Some(&*self.kind_key),
            self.identity.as_deref(),
            self.text.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            h.push_str(s);
            h.push(' ');
        }
        for (_, v) in self.fields.iter() {
            match v {
                Value::Text(s) => {
                    h.push_str(s);
                    h.push(' ');
                }
                Value::I64(n) => {
                    let _ = write!(h, "{n} ");
                }
                Value::F64(n) => {
                    let _ = write!(h, "{n} ");
                }
                _ => {}
            }
        }
        self.haystack = h.to_lowercase();
        let line = self.line();
        self.bytes = line.as_ref().map_or(256, |l| l.len() + 1);
        line
    }

    /// The journal line of the message, without the line end.
    pub(crate) fn line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&Line {
            id: self.id,
            time: self.time,
            last_time: self.last_time,
            kind: &self.kind_key,
            family: choice_key(self.family),
            urgency: choice_key(self.urgency),
            identity: self.identity.as_deref(),
            freq_hz: self.freq_hz,
            position: Coords::of(self.position, self.height_m),
            distance_km: self.distance_km,
            text: self.text.as_deref(),
            repeats: self.repeats,
            fields: &self.fields,
        })
    }

    /// The message of journal line `line`. `None` if the line does not parse or its kind is not in the catalog.
    pub(crate) fn from_line(line: &str, kinds: &Kinds) -> Option<Message> {
        let h: Head<'_> = serde_json::from_str(line).ok()?;
        let catalog = kinds.catalog();
        let kind = catalog.id(&h.kind)?;
        let kind_key = catalog.def(kind)?.kind.clone();
        let decl = catalog.fields(kind)?;
        let fields = RecordSeed(decl)
            .deserialize(&mut serde_json::Deserializer::from_str(h.fields.get()))
            .ok()?;
        let mut m = Message {
            id: h.id,
            time: h.time,
            last_time: h.last_time,
            kind_key,
            kind,
            family: choice_of_key(&h.family).unwrap_or(Family::Other),
            urgency: choice_of_key(&h.urgency).unwrap_or(Urgency::None),
            identity: h.identity,
            freq_hz: h.freq_hz,
            position: h.position.map(Coords::position),
            height_m: h.position.and_then(|c| c.height_m),
            distance_km: h.distance_km,
            text: h.text,
            repeats: h.repeats,
            fields,
            haystack: String::new(),
            bytes: 0,
        };
        let _ = m.refresh();
        Some(m)
    }
}

#[derive(Serialize)]
struct Line<'a> {
    id: u64,
    time: f64,
    last_time: f64,
    kind: &'a str,
    family: &'a str,
    urgency: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    identity: Option<&'a str>,
    freq_hz: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<Coords>,
    #[serde(skip_serializing_if = "Option::is_none")]
    distance_km: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
    repeats: u32,
    fields: &'a Record,
}

#[derive(Deserialize)]
struct Head<'a> {
    id: u64,
    time: f64,
    last_time: f64,
    kind: String,
    family: String,
    urgency: String,
    #[serde(default)]
    identity: Option<String>,
    freq_hz: f64,
    #[serde(default)]
    position: Option<Coords>,
    #[serde(default)]
    distance_km: Option<f64>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    repeats: u32,
    #[serde(borrow)]
    fields: &'a serde_json::value::RawValue,
}

/// A message as one plain JSON object, for exports and daily files.
#[derive(Serialize)]
pub struct PlainMessage<'a> {
    /// Its id.
    pub id: u64,
    /// First seen, Unix seconds.
    pub time: f64,
    /// Last seen, Unix seconds.
    pub last_time: f64,
    /// The kind key.
    pub kind: &'a str,
    /// The family key.
    pub family: &'a str,
    /// The urgency key.
    pub urgency: &'a str,
    /// The thing it is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<&'a str>,
    /// Frequency, Hz.
    pub freq_hz: f64,
    /// Position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Coords>,
    /// Distance from the receiver, km.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_km: Option<f64>,
    /// The text for people.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<&'a str>,
    /// Folded repeats.
    pub repeats: u32,
    /// The fields.
    pub fields: PlainFields<'a>,
}

impl<'a> From<&'a Message> for PlainMessage<'a> {
    fn from(m: &'a Message) -> Self {
        PlainMessage {
            id: m.id,
            time: m.time,
            last_time: m.last_time,
            kind: &m.kind_key,
            family: choice_key(m.family),
            urgency: choice_key(m.urgency),
            identity: m.identity.as_deref(),
            freq_hz: m.freq_hz,
            position: Coords::of(m.position, m.height_m),
            distance_km: m.distance_km,
            text: m.text.as_deref(),
            repeats: m.repeats,
            fields: PlainFields::all(&m.fields),
        }
    }
}
