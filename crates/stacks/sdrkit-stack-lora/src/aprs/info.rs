//! The APRS information field as one flat record for the tables of this family: the kind, the position, the message parts and
//! a one-line summary. The parsing itself is `sdrkit_proto_link::aprs`.
//!
//! Standard: APRS 1.0.1 (the APRS101 document of 2000), with the 1.1 and 1.2 addenda.

use crate::proto::error::ParseError;
use sdrkit_proto_link::aprs::{self, Body, Content, Located, NmeaCheck, Timestamp, Weather};
use serde::{Deserialize, Serialize};

/// A decoded information field, flattened for the tables.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AprsInfo {
    /// What the packet is: "Position", "Mic-E", "Object", "Item", "Status", "Message", "Bulletin", "Announcement", "Weather",
    /// "Telemetry", "Raw GPS", "Capabilities", "Query", "Third-party", "User-defined", "Maidenhead" or "Other".
    pub kind: String,
    /// Latitude, degrees north.
    pub lat: Option<f64>,
    /// Longitude, degrees east.
    pub lon: Option<f64>,
    /// Symbol table.
    pub symbol_table: Option<char>,
    /// Symbol code.
    pub symbol_code: Option<char>,
    /// Altitude, metres.
    pub alt_m: Option<f64>,
    /// Course, degrees.
    pub course: Option<u16>,
    /// Speed, knots.
    pub speed_kn: Option<f64>,
    /// Object or item name.
    pub name: Option<String>,
    /// The object or item is alive.
    pub live: Option<bool>,
    /// Comment or status text.
    pub comment: String,
    /// Message addressee.
    pub addressee: Option<String>,
    /// Message text.
    pub text: Option<String>,
    /// Message number.
    pub msg_no: Option<String>,
    /// The reply-ack number of "{MM}AA".
    pub reply_ack: Option<String>,
    /// An acknowledgement.
    pub is_ack: bool,
    /// A reject.
    pub is_rej: bool,
    /// Mic-E message status.
    pub mice_status: Option<String>,
    /// Mic-E radio.
    pub device: Option<String>,
    /// The weather values as one line.
    pub weather: Option<String>,
    /// The time stamp as sent.
    pub timestamp: Option<String>,
    /// The station can receive messages (`=` and `@`).
    pub messaging: bool,
    /// The `SOURCE>DEST,PATH` header of the inner packet of a third-party frame.
    pub third_party: Option<String>,
    /// The inner packet of a third-party frame.
    pub inner: Option<Box<AprsInfo>>,
    /// The NMEA checksum of a raw GPS packet is wrong.
    pub nmea_bad: bool,
    /// The one-line summary.
    pub summary: String,
}

/// "25.2000N".
pub fn fmt_lat(v: f64) -> String {
    format!("{:.4}{}", v.abs(), if v < 0.0 { 'S' } else { 'N' })
}

/// "55.3000E".
pub fn fmt_lon(v: f64) -> String {
    format!("{:.4}{}", v.abs(), if v < 0.0 { 'W' } else { 'E' })
}

/// Text from bytes: control characters, bytes that are not UTF-8 and the replacement character U+FFFD become "?".
pub fn clean_text(b: &[u8]) -> String {
    let mut out = String::with_capacity(b.len());
    for chunk in b.utf8_chunks() {
        for c in chunk.valid().chars() {
            out.push(if c.is_control() || c == '\u{FFFD}' {
                '?'
            } else {
                c
            });
        }
        if !chunk.invalid().is_empty() {
            out.push('?');
        }
    }
    out
}

fn fmt_timestamp(t: &Timestamp) -> String {
    match *t {
        Timestamp::DayHourMinute {
            day,
            hour,
            minute,
            utc,
        } => format!(
            "{day:02}{hour:02}{minute:02}{}",
            if utc { 'z' } else { '/' }
        ),
        Timestamp::HourMinuteSecond {
            hour,
            minute,
            second,
        } => format!("{hour:02}{minute:02}{second:02}h"),
        Timestamp::MonthDayHourMinute {
            month,
            day,
            hour,
            minute,
        } => format!("{month:02}{day:02}{hour:02}{minute:02}"),
    }
}

/// The weather values that were sent, as one line: "220 deg 4 mph gust 5 mph 77 F ...".
pub fn weather_line(w: &Weather) -> String {
    let mut p = Vec::new();
    if let Some(d) = w.wind_dir_deg {
        p.push(format!("{d} deg"));
    }
    if let Some(s) = w.wind_mph {
        p.push(format!("{s:.0} mph"));
    }
    if let Some(g) = w.gust_mph {
        p.push(format!("gust {g:.0} mph"));
    }
    if let Some(t) = w.temp_f {
        p.push(format!("{t:.0} F"));
    }
    if w.rain_1h_in.is_some() || w.rain_24h_in.is_some() || w.rain_midnight_in.is_some() {
        let f = |v: Option<f64>| v.map_or("-".into(), |x| format!("{x:.2}"));
        p.push(format!(
            "rain {}/{}/{} in",
            f(w.rain_1h_in),
            f(w.rain_24h_in),
            f(w.rain_midnight_in)
        ));
    }
    if let Some(h) = w.humidity_pct {
        p.push(format!("{h} %"));
    }
    if let Some(b) = w.pressure_hpa {
        p.push(format!("{b:.1} hPa"));
    }
    if let Some(l) = w.luminosity_w_m2 {
        p.push(format!("{l} W/m2"));
    }
    if let Some(s) = w.snow_in {
        p.push(format!("snow {s:.1} in"));
    }
    p.join(" ")
}

fn located(i: &mut AprsInfo, l: &Located) {
    i.lat = Some(l.position.lat_deg);
    i.lon = Some(l.position.lon_deg);
    i.symbol_table = Some(l.symbol.table);
    i.symbol_code = Some(l.symbol.code);
    i.alt_m = l.altitude_m;
    i.course = l.course_deg.filter(|&c| c != 0);
    i.speed_kn = l.speed_kn;
    i.comment = clean_text(l.comment.as_bytes());
    if let Some(w) = &l.weather {
        i.weather = Some(weather_line(w));
        i.kind = "Weather".into();
    }
}

fn fill(i: &mut AprsInfo, p: &aprs::Packet) {
    match &p.body {
        Body::Position {
            timestamp,
            messaging,
            report,
        } => {
            i.kind = "Position".into();
            i.messaging = *messaging;
            i.timestamp = timestamp.as_ref().map(fmt_timestamp);
            located(i, report);
        }
        Body::MicE(m) => {
            i.kind = "Mic-E".into();
            i.lat = Some(m.position.lat_deg);
            i.lon = Some(m.position.lon_deg);
            i.symbol_table = Some(m.symbol.table);
            i.symbol_code = Some(m.symbol.code);
            i.speed_kn = Some(m.speed_kn);
            i.course = (m.course_deg != 0).then_some(m.course_deg);
            i.alt_m = m.altitude_m;
            i.mice_status = Some(m.message.name());
            i.device = m.device.as_ref().map(|d| d.to_string());
            i.comment = clean_text(m.comment.as_bytes());
        }
        Body::Object {
            name,
            live,
            timestamp,
            report,
        } => {
            located(i, report);
            i.kind = "Object".into();
            i.name = Some(name.clone());
            i.live = Some(*live);
            i.timestamp = timestamp.as_ref().map(fmt_timestamp);
        }
        Body::Item { name, live, report } => {
            located(i, report);
            i.kind = "Item".into();
            i.name = Some(name.clone());
            i.live = Some(*live);
        }
        Body::Status {
            timestamp,
            locator,
            text,
        } => {
            i.kind = "Status".into();
            i.timestamp = timestamp.as_ref().map(fmt_timestamp);
            let text = clean_text(text.as_bytes());
            i.comment = match locator {
                Some((loc, _)) if text.is_empty() => loc.clone(),
                Some((loc, _)) => format!("{loc} {text}"),
                None => text,
            };
        }
        Body::Message(m) => {
            i.kind = "Message".into();
            i.addressee = Some(m.addressee.clone());
            match &m.content {
                Content::Text {
                    text,
                    id,
                    reply_ack,
                } => {
                    i.text = Some(clean_text(text.as_bytes()));
                    i.msg_no = id.clone();
                    i.reply_ack = reply_ack.clone();
                }
                Content::Ack { id, reply_ack } => {
                    i.is_ack = true;
                    i.msg_no = Some(id.clone());
                    i.reply_ack = reply_ack.clone();
                }
                Content::Rej { id } => {
                    i.is_rej = true;
                    i.msg_no = Some(id.clone());
                }
                Content::Bulletin { text, .. } => {
                    i.kind = "Bulletin".into();
                    i.text = Some(text.clone());
                }
                Content::Announcement { text, .. } => {
                    i.kind = "Announcement".into();
                    i.text = Some(text.clone());
                }
                Content::Weather { text } => {
                    i.kind = "Bulletin".into();
                    i.text = Some(text.clone());
                }
                Content::Telemetry(_) => {
                    i.kind = "Telemetry".into();
                    i.comment = format!("set-up for {}", m.addressee);
                }
                Content::Query { query } => {
                    i.kind = "Query".into();
                    i.comment = query.clone();
                }
            }
        }
        Body::Weather { timestamp, weather } => {
            i.kind = "Weather".into();
            i.timestamp = timestamp.as_ref().map(fmt_timestamp);
            i.weather = Some(weather_line(weather));
        }
        Body::Telemetry(t) => {
            i.kind = "Telemetry".into();
            i.comment = format!("{t:?}");
        }
        Body::Query { query, target } => {
            i.kind = "Query".into();
            i.comment = if target.is_empty() {
                query.clone()
            } else {
                format!("{query} {target}")
            };
        }
        Body::Capabilities(list) => {
            i.kind = "Capabilities".into();
            i.comment = list
                .iter()
                .map(|(k, v)| match v {
                    Some(v) => format!("{k}={v}"),
                    None => k.clone(),
                })
                .collect::<Vec<_>>()
                .join(",");
        }
        Body::RawGps {
            sentence,
            check,
            position,
            speed_kn,
            course_deg,
            altitude_m,
        } => {
            i.kind = "Raw GPS".into();
            i.comment = clean_text(sentence.as_bytes());
            i.nmea_bad = *check == NmeaCheck::Bad;
            if let Some(p) = position {
                i.lat = Some(p.lat_deg);
                i.lon = Some(p.lon_deg);
            }
            i.speed_kn = *speed_kn;
            i.course = course_deg.map(|c| c.round() as u16);
            i.alt_m = *altitude_m;
        }
        Body::ThirdParty { header, packet } => {
            i.kind = "Third-party".into();
            let mut inner = AprsInfo::default();
            fill(&mut inner, packet);
            i.comment = format!("{header}: {}", inner.summary);
            i.third_party = Some(header.clone());
            i.inner = Some(Box::new(inner));
        }
        Body::UserDefined { user, kind, data } => {
            i.kind = "User-defined".into();
            i.comment = format!("{user}{kind}{data}");
        }
        Body::Maidenhead { locator, comment } => {
            i.kind = "Maidenhead".into();
            i.comment = if comment.is_empty() {
                locator.clone()
            } else {
                format!("{locator} {comment}")
            };
        }
        Body::Other(s) => {
            i.kind = "Other".into();
            i.comment = clean_text(s.as_bytes());
        }
    }
    i.summary = i.summary_text();
}

impl AprsInfo {
    fn position_text(&self) -> String {
        let mut p = Vec::new();
        if let (Some(a), Some(b)) = (self.lat, self.lon) {
            p.push(format!("{} {}", fmt_lat(a), fmt_lon(b)));
        }
        if let (Some(t), Some(c)) = (self.symbol_table, self.symbol_code) {
            p.push(format!("[{t}{c}]"));
        }
        if let Some(s) = self.speed_kn {
            p.push(format!("{s:.0} kn"));
        }
        if let Some(c) = self.course {
            p.push(format!("{c} deg"));
        }
        if let Some(a) = self.alt_m {
            p.push(format!("{a:.0} m"));
        }
        if let Some(s) = &self.mice_status {
            p.push(s.clone());
        }
        if !self.comment.is_empty() {
            p.push(self.comment.clone());
        }
        p.join(" ")
    }

    fn summary_text(&self) -> String {
        let name = self.name.clone().unwrap_or_default();
        match self.kind.as_str() {
            "Position" | "Mic-E" => self.position_text(),
            "Object" => format!("Object {name} {}", self.position_text()),
            "Item" => format!("Item {name} {}", self.position_text()),
            "Status" => format!("Status: {}", self.comment),
            "Message" | "Bulletin" | "Announcement" => {
                let a = self.addressee.clone().unwrap_or_default();
                let no = self.msg_no.clone().unwrap_or_default();
                if self.is_ack {
                    format!("Ack {no} to {a}")
                } else if self.is_rej {
                    format!("Reject {no} to {a}")
                } else {
                    let mut s =
                        format!("Message to {a}: {}", self.text.clone().unwrap_or_default());
                    if let Some(n) = &self.msg_no {
                        s += &format!(" {{{n}}}");
                    }
                    s
                }
            }
            "Weather" => {
                let w = self.weather.clone().unwrap_or_default();
                match (self.lat, self.lon) {
                    (Some(a), Some(b)) => format!("Weather: {w} at {} {}", fmt_lat(a), fmt_lon(b)),
                    _ => format!("Weather: {w}"),
                }
            }
            "Raw GPS" if self.nmea_bad => format!("Raw GPS (checksum wrong) {}", self.comment),
            "Raw GPS" => format!("Raw GPS {}", self.position_text()),
            "Telemetry" | "Capabilities" | "Query" | "Third-party" => {
                format!("{} {}", self.kind, self.comment)
            }
            _ => self.comment.clone(),
        }
    }
}

/// Parses an information field. `dest` is the AX.25 destination callsign without its SSID (Mic-E keeps data there). A field
/// that is not APRS returns the reason.
pub fn parse_info(info: &[u8], dest: &str) -> Result<AprsInfo, ParseError> {
    let dest = dest.split('-').next().unwrap_or(dest);
    let p = aprs::parse(info, dest).map_err(ParseError::Aprs)?;
    let mut i = AprsInfo::default();
    fill(&mut i, &p);
    Ok(i)
}
