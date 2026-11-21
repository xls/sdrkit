//! Exports of messages: CSV (RFC 4180, UTF-8), JSON, JSON lines, ADIF, and for messages with a position KML, GPX and
//! GeoJSON.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use sdrkit_core::publish::keys::{self, Family};
use sdrkit_svc_events::choice_key;
use serde::Serialize;

use crate::message::{Message, PlainMessage};
use crate::time::{civil, iso as iso8601};
use sdrkit_svc_events::plain::PlainFields;

sdrkit_core::choice_enum! {
    /// An export format.
    pub enum Export {
        /// Comma-separated values: fixed columns, then every field that a message has, in key order.
        Csv = ("csv", "CSV"),
        /// One JSON list.
        Json = ("json", "JSON"),
        /// One JSON object per line.
        JsonLines = ("jsonl", "JSON lines"),
        /// Amateur radio log interchange: messages with a call sign only.
        Adif = ("adif", "ADIF"),
        /// Placemarks: messages with a position only.
        Kml = ("kml", "KML"),
        /// Waypoints: messages with a position only.
        Gpx = ("gpx", "GPX"),
        /// A feature collection of points: messages with a position only.
        GeoJson = ("geojson", "GeoJSON"),
    }
}

impl Export {
    /// The usual file extension.
    pub fn extension(self) -> &'static str {
        match self {
            Export::Csv => "csv",
            Export::Json => "json",
            Export::JsonLines => "jsonl",
            Export::Adif => "adi",
            Export::Kml => "kml",
            Export::Gpx => "gpx",
            Export::GeoJson => "geojson",
        }
    }
}

/// The messages `records` in `format`.
pub fn export<'a>(records: impl IntoIterator<Item = &'a Message>, format: Export) -> String {
    let records: Vec<&Message> = records.into_iter().collect();
    match format {
        Export::Csv => csv(&records),
        Export::Json => serde_json::to_string_pretty(
            &records
                .iter()
                .map(|m| PlainMessage::from(*m))
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default(),
        Export::JsonLines => records
            .iter()
            .filter_map(|r| serde_json::to_string(&PlainMessage::from(*r)).ok())
            .map(|l| l + "\n")
            .collect(),
        Export::Adif => adif(&records),
        Export::Kml => kml(&records),
        Export::Gpx => gpx(&records),
        Export::GeoJson => geojson(&records),
    }
}

/// Returns one CSV cell, quoted if it must be.
pub(crate) fn csv_cell(s: &str) -> String {
    sdrkit_util_text::csv::field(s, ',')
}

const CSV_COLUMNS: [&str; 11] = [
    "id",
    "time",
    "last_time",
    "kind",
    "family",
    "identity",
    "freq_hz",
    "lat",
    "lon",
    "distance_km",
    "text",
];

fn csv(records: &[&Message]) -> String {
    let fields: BTreeSet<&str> = records
        .iter()
        .flat_map(|r| {
            r.fields
                .iter()
                .filter(|(_, v)| !matches!(v, sdrkit_core::Value::None))
                .map(|(d, _)| d.key.as_ref())
        })
        .collect();
    let mut out = String::new();
    let header: Vec<String> = CSV_COLUMNS
        .iter()
        .map(|c| c.to_string())
        .chain(fields.iter().map(|f| csv_cell(f)))
        .collect();
    out.push_str(&header.join(","));
    out.push_str("\r\n");
    for r in records {
        let opt = |x: Option<f64>| x.map(|v| v.to_string()).unwrap_or_default();
        let mut row = vec![
            r.id.to_string(),
            iso8601(r.time),
            iso8601(r.last_time),
            csv_cell(&r.kind_key),
            csv_cell(choice_key(r.family)),
            csv_cell(r.identity.as_deref().unwrap_or("")),
            r.freq_hz.to_string(),
            opt(r.position.map(|p| p.lat_deg)),
            opt(r.position.map(|p| p.lon_deg)),
            opt(r.distance_km),
            csv_cell(r.text.as_deref().unwrap_or("")),
        ];
        for f in &fields {
            let cell = r
                .fields
                .decl()
                .id(f)
                .and_then(|id| r.fields.text(id))
                .map(|t| t.to_string())
                .unwrap_or_default();
            row.push(csv_cell(&cell));
        }
        out.push_str(&row.join(","));
        out.push_str("\r\n");
    }
    out
}

fn adif_field(out: &mut String, name: &str, value: &str) {
    if !value.is_empty() {
        let _ = write!(out, "<{}:{}>{} ", name, value.len(), value);
    }
}

/// The ADIF mode of a message: its spot mode field, or `PKT` for APRS.
fn adif_mode(r: &Message) -> Option<String> {
    if let Some(m) = r.fields.get(&keys::SPOT_MODE) {
        return Some(m.to_uppercase());
    }
    (r.family == Family::Aprs).then(|| "PKT".to_owned())
}

fn adif(records: &[&Message]) -> String {
    let mut out = String::from("sdrkit message store export\n");
    adif_field(&mut out, "ADIF_VER", "3.1.4");
    adif_field(&mut out, "PROGRAMID", "sdrkit");
    out.push_str("<EOH>\n");
    for r in records {
        let call = r
            .fields
            .get(&keys::CALLSIGN)
            .map(str::to_owned)
            .or_else(|| {
                (r.family == Family::Aprs)
                    .then(|| r.identity.clone())
                    .flatten()
            });
        let Some(call) = call else { continue };
        let Some(c) = civil(r.time, 0) else { continue };
        adif_field(&mut out, "CALL", &call.to_uppercase());
        adif_field(
            &mut out,
            "QSO_DATE",
            &format!("{:04}{:02}{:02}", c.year, c.month, c.day),
        );
        adif_field(
            &mut out,
            "TIME_ON",
            &format!("{:02}{:02}{:02}", c.hour, c.minute, c.second),
        );
        if r.freq_hz > 0.0 {
            adif_field(&mut out, "FREQ", &format!("{:.6}", r.freq_hz / 1e6));
        }
        if let Some(m) = adif_mode(r) {
            adif_field(&mut out, "MODE", &m);
        }
        if let Some(grid) = r
            .position
            .and_then(|p| sdrkit_util_geo::maidenhead::encode(&p, 6).ok())
        {
            adif_field(&mut out, "GRIDSQUARE", grid.as_str());
        }
        if let Some(snr) = r.fields.get(&keys::SNR_DB) {
            adif_field(&mut out, "RST_RCVD", &format!("{snr:.0}"));
        }
        let comment = format!("{} {}", r.kind_key, r.text.as_deref().unwrap_or(""))
            .trim()
            .replace(['\r', '\n'], " ");
        adif_field(&mut out, "COMMENT", &comment);
        out.push_str("<EOR>\n");
    }
    out
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn name_of(r: &Message) -> String {
    r.identity.clone().unwrap_or_else(|| r.kind_key.to_string())
}

fn kml(records: &[&Message]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n<Document>\n",
    );
    for r in records {
        let Some(p) = r.position else { continue };
        let alt = r.height_m.unwrap_or(0.0);
        let _ = writeln!(
            out,
            "<Placemark><name>{}</name><description>{}</description><TimeStamp><when>{}</when></TimeStamp><Point><coordinates>{},{},{}</coordinates></Point></Placemark>",
            xml(&name_of(r)),
            xml(format!("{} {}", r.kind_key, r.text.as_deref().unwrap_or("")).trim()),
            iso8601(r.last_time),
            p.lon_deg,
            p.lat_deg,
            alt
        );
    }
    out.push_str("</Document>\n</kml>\n");
    out
}

fn gpx(records: &[&Message]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"sdrkit\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n",
    );
    for r in records {
        let Some(p) = r.position else { continue };
        let ele = r
            .height_m
            .map(|h| format!("<ele>{h}</ele>"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "<wpt lat=\"{}\" lon=\"{}\">{}<time>{}</time><name>{}</name><desc>{}</desc></wpt>",
            p.lat_deg,
            p.lon_deg,
            ele,
            iso8601(r.last_time),
            xml(&name_of(r)),
            xml(&r.kind_key)
        );
    }
    out.push_str("</gpx>\n");
    out
}

#[derive(Serialize)]
struct Collection<'a> {
    #[serde(rename = "type")]
    ty: &'static str,
    features: Vec<Feature<'a>>,
}

#[derive(Serialize)]
struct Feature<'a> {
    #[serde(rename = "type")]
    ty: &'static str,
    geometry: Point,
    properties: Properties<'a>,
}

#[derive(Serialize)]
struct Point {
    #[serde(rename = "type")]
    ty: &'static str,
    coordinates: Vec<f64>,
}

#[derive(Serialize)]
struct Properties<'a> {
    id: u64,
    time: String,
    kind: &'a str,
    identity: Option<&'a str>,
    text: Option<&'a str>,
    fields: PlainFields<'a>,
}

fn geojson(records: &[&Message]) -> String {
    let features = records
        .iter()
        .filter_map(|r| {
            let p = r.position?;
            let mut coordinates = vec![p.lon_deg, p.lat_deg];
            coordinates.extend(r.height_m);
            Some(Feature {
                ty: "Feature",
                geometry: Point {
                    ty: "Point",
                    coordinates,
                },
                properties: Properties {
                    id: r.id,
                    time: iso8601(r.last_time),
                    kind: &r.kind_key,
                    identity: r.identity.as_deref(),
                    text: r.text.as_deref(),
                    fields: PlainFields::all(&r.fields),
                },
            })
        })
        .collect();
    serde_json::to_string(&Collection {
        ty: "FeatureCollection",
        features,
    })
    .unwrap_or_default()
}
