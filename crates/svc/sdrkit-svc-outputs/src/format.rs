//! What an output writes for an observation, and which observations it takes. Names in a format resolve against the event
//! kinds when the output is added. A line is written into a buffer that the caller reuses.

use std::fmt::Write as _;
use std::time::Instant;

use sdrkit_svc_events::plain::PlainObservation;
use sdrkit_svc_events::{
    split_list, FamilySet, FieldRef, KindSet, Kinds, NameError, Observation, Reading, Template,
};
use sdrkit_util_geo::time::{split_f64, Civil};

use crate::{ax25, OutputError};

sdrkit_core::choice_enum! {
    /// The form of what a stream output (file, UDP, TCP server) writes.
    pub enum LineFormat {
        /// The observation as one JSON object per line.
        Json = ("json", "JSON lines"),
        /// The template, one line per observation.
        Text = ("text", "Text"),
        /// The template read as NMEA 0183 sentences. Only sentences with a good checksum go out, one per line.
        Nmea = ("nmea", "NMEA"),
        /// The template read as an APRS packet in TNC2 text, sent as a KISS frame.
        Kiss = ("kiss", "KISS"),
        /// BaseStation (SBS) lines of an aircraft.
        Sbs = ("sbs", "SBS"),
    }
}

/// The fields of the SBS lines, each resolved from `name=field` in the `fields` text.
#[derive(Clone, Debug, Default, PartialEq)]
struct SbsFields {
    callsign: Option<FieldRef>,
    altitude: Option<FieldRef>,
    speed: Option<FieldRef>,
    track: Option<FieldRef>,
    vrate: Option<FieldRef>,
    squawk: Option<FieldRef>,
    ground: Option<FieldRef>,
}

/// A line format, resolved.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Format {
    kind: LineFormat,
    template: Template,
    sbs: SbsFields,
}

fn name_err(arg: &'static str) -> impl Fn(NameError) -> OutputError {
    move |error| OutputError::Name { arg, error }
}

impl Format {
    /// The format `kind` with the template `template` and the SBS fields `fields` (`altitude=altitude_ft, squawk=squawk`;
    /// the names are callsign, altitude, speed, track, vrate, squawk and ground).
    ///
    /// # Errors
    ///
    /// [`OutputError::Name`] with the position if a name does not resolve within `within`.
    pub fn resolve(
        kind: LineFormat,
        template: &str,
        fields: &str,
        k: &Kinds,
        within: Option<&KindSet>,
    ) -> Result<Format, OutputError> {
        let template = Template::parse(template, 0, k, within).map_err(name_err("template"))?;
        let mut sbs = SbsFields::default();
        for (at, part) in split_list(fields) {
            let (name, field) = part.split_once('=').unwrap_or((part, part));
            let field_at = at + part.find(field.trim()).unwrap_or(0);
            let f = Some(
                k.field(field.trim(), field_at, within)
                    .map_err(name_err("fields"))?,
            );
            let unknown = || {
                name_err("fields")(NameError::UnknownField {
                    name: name.trim().to_owned(),
                    at,
                })
            };
            match name.trim() {
                "callsign" => sbs.callsign = f,
                "altitude" => sbs.altitude = f,
                "speed" => sbs.speed = f,
                "track" => sbs.track = f,
                "vrate" => sbs.vrate = f,
                "squawk" => sbs.squawk = f,
                "ground" => sbs.ground = f,
                _ => return Err(unknown()),
            }
        }
        Ok(Format {
            kind,
            template,
            sbs,
        })
    }

    /// Writes the bytes of `o` into `out` (cleared first), with `text` as a work buffer. False if the format has nothing
    /// for `o`.
    pub fn render(&self, o: &Observation<'_>, text: &mut String, out: &mut Vec<u8>) -> bool {
        out.clear();
        text.clear();
        match self.kind {
            LineFormat::Json => {
                if serde_json::to_writer(&mut *out, &PlainObservation::from(o)).is_err() {
                    return false;
                }
                out.push(b'\n');
            }
            LineFormat::Text => {
                self.template.fill_into(o, text);
                out.extend_from_slice(text.as_bytes());
                out.push(b'\n');
            }
            LineFormat::Nmea => {
                self.template.fill_into(o, text);
                for part in text.split(['\r', '\n']).map(str::trim) {
                    if !part.is_empty() && sdrkit_util_geo::nmea::parse(part).is_ok() {
                        out.extend_from_slice(part.as_bytes());
                        out.extend_from_slice(b"\r\n");
                    }
                }
            }
            LineFormat::Kiss => {
                self.template.fill_into(o, text);
                if let Some(frame) = ax25::from_tnc2(text.trim()) {
                    out.extend_from_slice(&ax25::kiss_frame(&frame, 0));
                }
            }
            LineFormat::Sbs => self.sbs(o, text, out),
        }
        !out.is_empty()
    }

    /// The TNC2 text of `o`: the template filled.
    pub fn text(&self, o: &Observation<'_>, text: &mut String) {
        text.clear();
        self.template.fill_into(o, text);
    }

    fn sbs(&self, o: &Observation<'_>, text: &mut String, out: &mut Vec<u8>) {
        let Some(hex) = o.identity else { return };
        let Some(c) = split_f64(o.unix_time)
            .ok()
            .and_then(|(s, n)| Civil::from_unix(s, n).ok())
        else {
            return;
        };
        let ms = (c.nanos / 1_000_000).min(999);
        let stamp = format!(
            "{:04}/{:02}/{:02},{:02}:{:02}:{:02}.{ms:03}",
            c.year, c.month, c.day, c.hour, c.minute, c.second
        );
        let read =
            |f: &Option<FieldRef>| -> Option<Reading<'_>> { f.as_ref().and_then(|f| o.read(f)) };
        let cell = |f: &Option<FieldRef>| read(f).map(|r| r.to_string()).unwrap_or_default();
        let ground = match read(&self.sbs.ground) {
            Some(Reading::Bool(true)) => "-1",
            Some(Reading::Bool(false)) => "0",
            _ => "",
        };
        let mut line = |ty: u8, cols: [&str; 12]| {
            let _ = write!(
                text,
                "MSG,{ty},1,1,{},1,{stamp},{stamp},{}\r\n",
                hex.to_uppercase(),
                cols.join(",")
            );
        };
        let callsign = cell(&self.sbs.callsign);
        if !callsign.is_empty() {
            line(1, [&callsign, "", "", "", "", "", "", "", "", "", "", ""]);
        }
        let alt = cell(&self.sbs.altitude);
        if let Some(p) = o.position {
            let (lat, lon) = (p.lat_deg.to_string(), p.lon_deg.to_string());
            line(
                3,
                ["", &alt, "", "", &lat, &lon, "", "", "", "", "", ground],
            );
        }
        let (gs, trk, vr) = (
            cell(&self.sbs.speed),
            cell(&self.sbs.track),
            cell(&self.sbs.vrate),
        );
        if !gs.is_empty() || !trk.is_empty() {
            line(4, ["", "", &gs, &trk, "", "", &vr, "", "", "", "", ground]);
        }
        let squawk = cell(&self.sbs.squawk);
        if !squawk.is_empty() {
            let flag = if o.is_urgent() { "-1" } else { "0" };
            line(
                6,
                [
                    "", &alt, "", "", "", "", "", &squawk, "0", flag, "0", ground,
                ],
            );
        }
        out.extend_from_slice(text.as_bytes());
    }
}

/// Which observations an output takes, and how many per second.
#[derive(Clone, Debug, Default)]
pub(crate) struct Filter {
    pub families: FamilySet,
    pub kinds: Option<KindSet>,
    pub urgent_only: bool,
    pub max_per_s: f64,
}

/// A filter with the state of its rate limit.
#[derive(Debug)]
pub(crate) struct Gate {
    filter: Filter,
    tokens: f64,
    last: Option<Instant>,
}

impl Gate {
    /// A gate for `filter`.
    pub fn new(filter: Filter) -> Self {
        let tokens = filter.max_per_s.max(1.0);
        Gate {
            filter,
            tokens,
            last: None,
        }
    }

    /// True if `o` passes the filter, without spending the rate.
    pub fn wants(&self, o: &Observation<'_>) -> bool {
        let f = &self.filter;
        f.families.admits(o.family)
            && f.kinds.as_ref().is_none_or(|k| k.contains(o.kind))
            && (!f.urgent_only || o.is_urgent())
    }

    /// True if `o` passes the filter and the rate limit at `now`.
    pub fn admit(&mut self, o: &Observation<'_>, now: Instant) -> bool {
        if !self.wants(o) {
            return false;
        }
        let rate = self.filter.max_per_s;
        if rate <= 0.0 {
            return true;
        }
        if let Some(l) = self.last {
            self.tokens = (self.tokens + now.saturating_duration_since(l).as_secs_f64() * rate)
                .min(rate.max(1.0));
        }
        self.last = Some(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Writes the MQTT topic of `o` into `out` (cleared first): the template filled, each value with `/`, `+`, `#`, spaces and
/// control characters as `_`, a missing value as `unknown`, and no empty levels.
pub(crate) fn topic(template: &Template, o: &Observation<'_>, out: &mut String) {
    out.clear();
    template.fill_mapped(o, out, &|r, out| {
        let start = out.len();
        if let Some(r) = r {
            let _ = write!(out, "{r}");
        }
        if out.len() == start {
            out.push_str("unknown");
        }
        let safe: String = out[start..]
            .chars()
            .map(|c| {
                if matches!(c, '/' | '+' | '#') || c.is_whitespace() || c.is_control() {
                    '_'
                } else {
                    c
                }
            })
            .collect();
        out.truncate(start);
        out.push_str(&safe);
    });
    while let Some(i) = out.find("//") {
        out.remove(i);
    }
    while out.starts_with('/') {
        out.remove(0);
    }
    while out.ends_with('/') {
        out.pop();
    }
}
