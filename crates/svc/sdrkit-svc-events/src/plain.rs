//! Observations and records as plain JSON: numbers, text, yes and no, a choice by its key, a position as `{"lat", "lon"}`.

use sdrkit_core::{Record, Value};
use serde::{Deserialize, Serialize};

use crate::observation::{choice_key, Observation};
use crate::Position;

/// A position as plain JSON: `{"lat", "lon"}`, with `"height_m"` if known.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Coords {
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude, degrees.
    pub lon: f64,
    /// Height, metres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_m: Option<f64>,
}

impl Coords {
    /// The coordinates of `p`, with the height `height_m`.
    pub fn of(p: Option<Position>, height_m: Option<f64>) -> Option<Coords> {
        p.map(|p| Coords {
            lat: p.lat_deg,
            lon: p.lon_deg,
            height_m,
        })
    }
    /// The position, height 0 if none.
    pub fn position(self) -> Position {
        Position::with_alt(self.lat, self.lon, self.height_m.unwrap_or(0.0))
    }
}

/// The fields of a record as plain JSON values: numbers, text, yes and no, a choice by its key, a position as
/// `{"lat", "lon"}`, a payload as a list of bytes. If `hide_text` is true, text values are left out.
pub struct PlainFields<'a> {
    /// The record.
    pub record: &'a Record,
    /// Leave text values out (private content).
    pub hide_text: bool,
}

impl<'a> PlainFields<'a> {
    /// Every field of `record`.
    pub fn all(record: &'a Record) -> Self {
        PlainFields {
            record,
            hide_text: false,
        }
    }
}

impl Serialize for PlainFields<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(None)?;
        for (d, v) in self.record.iter() {
            match v {
                Value::None => {}
                Value::Bool(b) => m.serialize_entry(d.key.as_ref(), b)?,
                Value::I64(n) => m.serialize_entry(d.key.as_ref(), n)?,
                Value::F64(n) | Value::Hz(n) | Value::Time(n) | Value::Duration(n) => {
                    m.serialize_entry(d.key.as_ref(), n)?;
                }
                Value::Text(_) if self.hide_text => {}
                Value::Text(t) => m.serialize_entry(d.key.as_ref(), t)?,
                Value::Position { lat, lon } => {
                    m.serialize_entry(
                        d.key.as_ref(),
                        &Coords {
                            lat: *lat,
                            lon: *lon,
                            height_m: None,
                        },
                    )?;
                }
                Value::Choice(i) => m.serialize_entry(
                    d.key.as_ref(),
                    d.choices
                        .get(usize::from(*i))
                        .map_or("", |c| c.key.as_ref()),
                )?,
                Value::Bytes(b) => m.serialize_entry(d.key.as_ref(), &b[..])?,
            }
        }
        m.end()
    }
}

/// An observation as one plain JSON object: time, kind, family, urgency, source, frequency, position, distance, text and
/// fields. A private observation produces no text.
#[derive(Serialize)]
pub struct PlainObservation<'a> {
    /// Wall-clock time, Unix seconds.
    pub unix_time: f64,
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
    /// The fields.
    pub fields: PlainFields<'a>,
}

impl<'a> From<&Observation<'a>> for PlainObservation<'a> {
    fn from(o: &Observation<'a>) -> Self {
        PlainObservation {
            unix_time: o.unix_time,
            kind: o.event.kind(),
            family: choice_key(o.family),
            urgency: choice_key(o.urgency),
            identity: o.identity,
            freq_hz: o.event.freq_hz,
            position: Coords::of(o.position, o.height_m),
            distance_km: o.distance_km,
            text: o.text,
            fields: PlainFields {
                record: &o.event.fields,
                hide_text: o.private,
            },
        }
    }
}
