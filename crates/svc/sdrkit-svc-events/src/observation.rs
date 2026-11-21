//! An observation: an event with the properties every service needs from it (the wall clock, the family, the object it is about,
//! where, how far from the receiver, how urgent). It borrows the event and copies nothing.

use std::fmt;

use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::{Choice, Event, EventKindId, Value};

use crate::kinds::{FieldRef, Standard};
use crate::Position;

/// What the engine adds to every event: the wall clock and the receiver's position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Context {
    /// The time the event was taken, Unix seconds.
    pub unix_time: f64,
    /// The receiver's position, if the user set one.
    pub receiver: Option<Position>,
}

/// An event with the standard values that the services read. Make it with [`Kinds::observe`](crate::Kinds::observe).
#[derive(Clone, Copy, Debug)]
pub struct Observation<'e> {
    /// The event.
    pub event: &'e Event,
    /// The id of its kind.
    pub kind: EventKindId,
    /// Wall-clock time, Unix seconds.
    pub unix_time: f64,
    /// The family. [`Family::Other`] if the event does not say.
    pub family: Family,
    /// The urgency. [`Urgency::None`] if the event does not say.
    pub urgency: Urgency,
    /// True if the content is private: services never read its words.
    pub private: bool,
    /// The thing the event is about (an address, an MMSI, a call sign).
    pub identity: Option<&'e str>,
    /// The receiver of a message or call.
    pub target: Option<&'e str>,
    /// The channel or talk group of a call.
    pub channel: Option<&'e str>,
    /// The text for people. `None` if the content is private.
    pub text: Option<&'e str>,
    /// The position of the object. Its height is 0 if the event produces none.
    pub position: Option<Position>,
    /// The height of the object, metres, if the event produces one.
    pub height_m: Option<f64>,
    /// The distance from the receiver, km.
    pub distance_km: Option<f64>,
    /// The bearing from the receiver, degrees.
    pub bearing_deg: Option<f64>,
}

/// One value read from an observation, borrowed from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reading<'e> {
    /// A number (numbers, whole numbers, frequencies, times and lengths of time).
    Number(f64),
    /// Text, or the key of a choice.
    Text(&'e str),
    /// Yes or no.
    Bool(bool),
    /// A position, degrees.
    Position(f64, f64),
    /// A raw payload.
    Bytes(&'e [u8]),
}

impl Reading<'_> {
    /// The value as a number: numbers, yes as 1 and no as 0, and text that parses as a number.
    pub fn number(&self) -> Option<f64> {
        match self {
            Reading::Number(x) => Some(*x),
            Reading::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Reading::Text(t) => t.trim().parse().ok(),
            Reading::Position(..) | Reading::Bytes(_) => None,
        }
    }
}

impl fmt::Display for Reading<'_> {
    /// Text for people and for line formats: whole numbers without a fraction, yes and no as `true` and `false`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reading::Number(x) if x.fract() == 0.0 && x.abs() < 1e15 => write!(f, "{}", *x as i64),
            Reading::Number(x) => write!(f, "{x}"),
            Reading::Text(t) => f.write_str(t),
            Reading::Bool(b) => write!(f, "{b}"),
            Reading::Position(lat, lon) => write!(f, "{lat},{lon}"),
            Reading::Bytes(b) => write!(f, "{} bytes", b.len()),
        }
    }
}

impl<'e> Observation<'e> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        event: &'e Event,
        kind: EventKindId,
        ctx: &Context,
        family: Family,
        urgency: Urgency,
        private: bool,
        identity: Option<&'e str>,
        target: Option<&'e str>,
        channel: Option<&'e str>,
        text: Option<&'e str>,
        position: Option<Position>,
        height_m: Option<f64>,
    ) -> Self {
        let (distance_km, bearing_deg) = match (&ctx.receiver, &position) {
            (Some(r), Some(p)) => (
                Some(r.distance_m(p) / 1000.0),
                Some(r.initial_bearing_deg(p)),
            ),
            _ => (None, None),
        };
        Observation {
            event,
            kind,
            unix_time: ctx.unix_time,
            family,
            urgency,
            private,
            identity,
            target,
            channel,
            text,
            position,
            height_m,
            distance_km,
            bearing_deg,
        }
    }

    /// True if the urgency is above [`Urgency::None`].
    pub fn is_urgent(&self) -> bool {
        self.urgency != Urgency::None
    }

    /// The value of `f`. `None` if the observation has no value for it. A private observation produces no text.
    pub fn read(&self, f: &FieldRef) -> Option<Reading<'e>> {
        match f {
            FieldRef::Standard(s) => self.standard(*s),
            FieldRef::Declared(ids) => {
                let ev: &'e Event = self.event;
                let id = (*ids.get(usize::from(self.kind.0))?)?;
                let decl = ev.fields.decl().field(id)?;
                Some(match ev.fields.value(id)? {
                    Value::None => return None,
                    Value::Bool(b) => Reading::Bool(*b),
                    Value::I64(v) => Reading::Number(*v as f64),
                    Value::F64(v) | Value::Hz(v) | Value::Time(v) | Value::Duration(v) => {
                        Reading::Number(*v)
                    }
                    Value::Text(_) if self.private => return None,
                    Value::Text(t) => Reading::Text(t.as_str()),
                    Value::Position { lat, lon } => Reading::Position(*lat, *lon),
                    Value::Choice(i) => {
                        Reading::Text(decl.choices.get(usize::from(*i))?.key.as_ref())
                    }
                    Value::Bytes(b) => Reading::Bytes(b),
                })
            }
        }
    }

    fn standard(&self, s: Standard) -> Option<Reading<'e>> {
        let n = |x: f64| Some(Reading::Number(x));
        match s {
            Standard::Kind => Some(Reading::Text(self.event.kind())),
            Standard::Family => Some(Reading::Text(choice_key(self.family))),
            Standard::Urgency => Some(Reading::Text(choice_key(self.urgency))),
            Standard::Urgent => Some(Reading::Bool(self.is_urgent())),
            Standard::Identity => self.identity.map(Reading::Text),
            Standard::Text => self.text.map(Reading::Text),
            Standard::FreqHz => n(self.event.freq_hz),
            Standard::FreqMhz => n(self.event.freq_hz / 1e6),
            Standard::Lat => self.position.and_then(|p| n(p.lat_deg)),
            Standard::Lon => self.position.and_then(|p| n(p.lon_deg)),
            Standard::DistanceKm => self.distance_km.and_then(n),
            Standard::BearingDeg => self.bearing_deg.and_then(n),
            Standard::UnixTime => n(self.unix_time),
            Standard::SignalTime => n(self.event.time),
        }
    }

    /// Every text value of the event, for searches and word watches. Nothing if the content is private.
    pub fn words(&self) -> impl Iterator<Item = &'e str> + 'e {
        let private = self.private;
        self.event.fields.iter().filter_map(move |(_, v)| match v {
            Value::Text(t) if !private => Some(t.as_str()),
            _ => None,
        })
    }
}

/// The key of choice `c`.
pub fn choice_key<C: Choice>(c: C) -> &'static str {
    C::CHOICES
        .get(usize::from(c.index()))
        .map_or("", |d| match &d.key {
            std::borrow::Cow::Borrowed(k) => k,
            std::borrow::Cow::Owned(_) => "",
        })
}

/// The choice with key `key`. `None` if no choice has it.
pub fn choice_of_key<C: Choice>(key: &str) -> Option<C> {
    C::CHOICES
        .iter()
        .position(|c| c.key == key)
        .and_then(|i| u16::try_from(i).ok())
        .and_then(C::from_index)
}
