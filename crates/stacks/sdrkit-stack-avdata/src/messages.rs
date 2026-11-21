//! What the ACARS links of the family (VHF ACARS, VDL Mode 2, HFDL) do with an ACARS message: the one-line reading of its fixed
//! format, the aircraft table row it updates, and the ADS-C report the generators send.

use sdrkit_proto_aero::acars::{reading_of, Message, Reading};
use sdrkit_proto_aero::adsc::{self, Fom, Group, Report, ReportKind, Waypoint};
use sdrkit_proto_aero::aircraft::{Aircraft, Fix, Motion, Source};
use sdrkit_proto_aero::arinc622::{self, Imi};
use sdrkit_util_geo::Position;
use serde::{Deserialize, Serialize};

/// Where a position came from (the serialisable form of [`Source`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    /// An ADS-C report.
    Adsc,
    /// A position written in a message text.
    #[default]
    Text,
    /// A VDL Mode 2 XID location parameter.
    Vdl2Xid,
    /// A VDL Mode 4 synchronisation burst.
    Vdl4Sync,
    /// An HFDL performance or frequency data HFNPDU.
    Hfdl,
}

impl Origin {
    /// The aircraft table's source.
    pub fn source(self) -> Source {
        match self {
            Origin::Adsc => Source::Adsc,
            Origin::Text => Source::Text,
            Origin::Vdl2Xid => Source::Vdl2Xid,
            Origin::Vdl4Sync => Source::Vdl4Sync,
            Origin::Hfdl => Source::Hfdl,
        }
    }
}

/// A position decoded from a frame or message, with the data that came with it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Located {
    /// Degrees north.
    pub lat: f64,
    /// Degrees east.
    pub lon: f64,
    /// Feet, when known.
    pub alt_ft: Option<f64>,
    /// Track, degrees true, when known.
    pub track_deg: Option<f64>,
    /// Ground speed, knots, when known.
    pub gs_kt: Option<f64>,
    /// Vertical rate, feet per minute, when known.
    pub vs_fpm: Option<f64>,
    /// What the position is ("ADS-C basic report", "XID aircraft location" ...).
    pub kind: String,
    /// Where it came from.
    pub origin: Origin,
    /// The time of the report as seconds of the UTC day, when given.
    pub sec_of_day: Option<f64>,
    /// The time of the report as seconds past the hour, when given.
    pub sec_past_hour: Option<f64>,
}

impl Located {
    /// A position of this kind and origin, or `None` when it fails the position test of [`sdrkit_proto_aero::textpos::sane`].
    pub fn new(lat: f64, lon: f64, kind: &str, origin: Origin) -> Option<Located> {
        let p = sdrkit_proto_aero::textpos::sane(lat, lon)?;
        Some(Located {
            lat: p.lat_deg,
            lon: p.lon_deg,
            kind: kind.into(),
            origin,
            ..Default::default()
        })
    }

    /// Returns the position a message carries (ADS-C report with its motion, a text position, or the position of a plain-text
    /// format in a downlink).
    pub fn of_message(m: &Message) -> Option<Located> {
        match &m.reading {
            Reading::Adsc(a) => {
                let r = a.report()?;
                let mut l = Located::new(r.lat_deg, r.lon_deg, "ADS-C report", Origin::Adsc)?;
                l.alt_ft = Some(r.alt_ft);
                l.sec_past_hour = Some(r.sec_past_hour);
                if let Some(mo) = motion_of(a) {
                    l.track_deg = Some(mo.track_deg);
                    l.gs_kt = Some(mo.gs_kt);
                    l.vs_fpm = mo.vs_fpm;
                }
                Some(l)
            }
            Reading::Position(p) => {
                let mut l = Located::new(
                    p.position.lat_deg,
                    p.position.lon_deg,
                    "position in the text",
                    Origin::Text,
                )?;
                l.alt_ft = p.alt_ft;
                l.sec_of_day = p.sec_of_day.map(f64::from);
                Some(l)
            }
            Reading::Format(f) if m.downlink => {
                let p = f.position?;
                let mut l = Located::new(p.lat_deg, p.lon_deg, f.kind, Origin::Text)?;
                l.alt_ft = f.alt_ft;
                l.track_deg = f.heading_deg;
                Some(l)
            }
            _ => None,
        }
    }

    /// The geometry position.
    pub fn position(&self) -> Position {
        Position::new(self.lat, self.lon)
    }

    /// The aircraft table's fix.
    pub fn fix(&self) -> Fix {
        Fix {
            position: self.position(),
            alt_ft: self.alt_ft,
            on_ground: false,
            source: self.origin.source(),
        }
    }

    /// The motion, when the track and the speed are known.
    pub fn motion(&self) -> Option<Motion> {
        Some(Motion {
            track_deg: self.track_deg?,
            gs_kt: self.gs_kt?,
            vs_fpm: self.vs_fpm,
        })
    }

    /// "lat lon alt (kind)" for a detail line.
    pub fn summary(&self) -> String {
        format!(
            "{:.5} {:.5} {} ft ({})",
            self.lat,
            self.lon,
            self.alt_ft.map_or("?".into(), |a| format!("{a:.0}")),
            self.kind
        )
    }
}

/// One line for each ADS-C group of a message: what a frame's detail lists.
pub fn adsc_lines(a: &adsc::Adsc) -> Vec<String> {
    let mut v = Vec::new();
    if !a.crc_ok {
        v.push(format!("ground {}, envelope check FAILED", a.ground));
        return v;
    }
    v.push(format!("ground {}, {}", a.ground, a.imi.as_str()));
    for g in a.groups() {
        v.push(match g {
            Group::Report(r) => format!(
                "report {:.5} {:.5} {:.0} ft at {:.1} s past the hour, accuracy {}",
                r.lat_deg,
                r.lon_deg,
                r.alt_ft,
                r.sec_past_hour,
                adsc::ACCURACY[(r.fom.accuracy & 7) as usize]
            ),
            Group::FlightId { .. } => {
                format!("flight id {}", g.flight_id_text().unwrap_or(""))
            }
            Group::EarthReference {
                track_deg,
                gs_kt,
                vs_fpm,
                ..
            } => {
                format!("earth reference: track {track_deg:.1}, {gs_kt:.0} kt, {vs_fpm:.0} ft/min")
            }
            Group::Airframe { icao } => format!("airframe {icao:06X}"),
            other => adsc::tag_name(other.tag()).to_string(),
        });
    }
    v
}

/// What a message row keeps of a CPDLC envelope.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CpdlcRow {
    /// AT1, CR1, CC1 or DR1.
    pub imi: String,
    /// The ground facility.
    pub ground: String,
    /// The aircraft.
    pub aircraft: String,
    /// The envelope check passed.
    pub crc_ok: bool,
    /// The body decoded completely.
    pub parsed: bool,
    /// The decoded text.
    pub text: String,
}

impl CpdlcRow {
    /// The row of a message's CPDLC envelope, when it has one.
    pub fn of(m: &Message) -> Option<CpdlcRow> {
        let Reading::Cpdlc(c) = &m.reading else {
            return None;
        };
        Some(CpdlcRow {
            imi: c.imi.as_str().into(),
            ground: c.ground.clone(),
            aircraft: c.aircraft.clone(),
            crc_ok: c.crc_ok,
            parsed: c.parsed(),
            text: c.text(),
        })
    }
}

/// The values of the reading columns of a message row: the kind and category of the reading and the CPDLC envelope.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReadingCells<'a> {
    /// The kind of the reading.
    pub kind: Option<&'a str>,
    /// The category of the reading.
    pub category: Option<&'a str>,
    /// The CPDLC envelope.
    pub imi: Option<&'a str>,
    /// The CPDLC ground facility.
    pub ground: Option<&'a str>,
    /// The aircraft named in the CPDLC envelope.
    pub aircraft: Option<&'a str>,
    /// The CPDLC envelope check passed.
    pub crc_ok: Option<bool>,
    /// The CPDLC body decoded completely.
    pub parsed: Option<bool>,
    /// The decoded CPDLC text.
    pub text: Option<&'a str>,
}

/// The reading columns of a kept message; all without a value for a row without a message.
pub fn reading_cells(k: Option<&Kept>) -> ReadingCells<'_> {
    let t = crate::report::text;
    let c = k.and_then(|k| k.cpdlc.as_ref());
    ReadingCells {
        kind: k.and_then(|k| t(&k.reading_kind)),
        category: k.and_then(|k| t(&k.reading_category)),
        imi: c.map(|c| c.imi.as_str()),
        ground: c.and_then(|c| t(&c.ground)),
        aircraft: c.and_then(|c| t(&c.aircraft)),
        crc_ok: c.map(|c| c.crc_ok),
        parsed: c.map(|c| c.parsed),
        text: c.and_then(|c| t(&c.text)),
    }
}

/// The ADS-C report of a message, when it holds one with a good envelope check.
pub fn adsc_of(m: &Message) -> Option<&adsc::Adsc> {
    match &m.reading {
        Reading::Adsc(a) if a.crc_ok => Some(a),
        _ => None,
    }
}

/// Applies a message to its aircraft row at signal time `t`: the flight, the label and text, the ADS-C groups, the text
/// position, or the position of a plain-text format in a downlink. Returns whether the message carried a position.
pub fn apply<X: Default + Clone>(a: &mut Aircraft<X>, m: &Message, t: f64) -> bool {
    let flight = if m.downlink { m.flight.as_str() } else { "" };
    a.message(flight, &m.label, &m.text);
    match &m.reading {
        Reading::Adsc(ad) => a.adsc(t, ad),
        Reading::Position(p) => a
            .position(
                t,
                Fix {
                    position: p.position,
                    alt_ft: p.alt_ft,
                    on_ground: false,
                    source: Source::Text,
                },
                None,
            )
            .is_ok(),
        Reading::Format(f) if m.downlink => match f.position {
            Some(position) => a
                .position(
                    t,
                    Fix {
                        position,
                        alt_ft: f.alt_ft,
                        on_ground: false,
                        source: Source::Text,
                    },
                    None,
                )
                .is_ok(),
            None => false,
        },
        _ => false,
    }
}

/// The row key of `a` in an aircraft table: its address, else its registration as the table keeps it.
pub fn aircraft_key<X>(a: &Aircraft<X>) -> String {
    a.address_text()
        .unwrap_or_else(|| sdrkit_proto_aero::msglog::reg_key(&a.registration))
}

/// Fills the track, ground speed and vertical rate of `a` from its last two positions, `min_dt` to `max_dt` seconds apart,
/// when no message reported its motion.
pub fn fill_motion<X: Default + Clone>(a: &mut Aircraft<X>, min_dt: f64, max_dt: f64) {
    if a.track_reported_at.is_some() {
        return;
    }
    if let Some(d) = a.derived(min_dt, max_dt) {
        a.track_deg = Some(d.track_deg);
        a.gs_kt = Some(d.gs_kt);
        a.vs_fpm = d.vs_fpm.or(a.vs_fpm);
    }
}

/// A message as kept in a list: the fields people read, without the borrowed parts of the reading.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    /// Channel frequency, Hz.
    pub freq_hz: f64,
    /// Level of the carrier, dB below full scale.
    pub level_db: f32,
    /// The registration.
    pub registration: String,
    /// The row key of the aircraft in the aircraft table, when the message has one.
    #[serde(default)]
    pub aircraft: String,
    /// The flight id of a downlink.
    pub flight: String,
    /// The label.
    pub label: String,
    /// The label's meaning.
    pub label_meaning: String,
    /// The mode character.
    pub mode: String,
    /// The block id of the first block.
    pub block_id: String,
    /// The message sequence number of a downlink.
    pub msn: String,
    /// Aircraft to ground.
    pub downlink: bool,
    /// The technical acknowledgement (`NAK`, or the block id acknowledged).
    pub ack: String,
    /// The printable text.
    pub text: String,
    /// The reading of the fixed format.
    pub reading: String,
    /// The kind of the reading.
    #[serde(default)]
    pub reading_kind: String,
    /// The category of the reading.
    #[serde(default)]
    pub reading_category: String,
    /// The CPDLC envelope, when the message has one.
    #[serde(default)]
    pub cpdlc: Option<CpdlcRow>,
    /// Blocks joined.
    pub blocks: u32,
    /// A block is missing.
    pub incomplete: bool,
    /// Bits the repair changed.
    pub bits_fixed: u32,
    /// The position decoded from the message: latitude, longitude, degrees.
    pub position: Option<(f64, f64)>,
    /// The altitude decoded from the message, feet.
    pub alt_ft: Option<f64>,
}

impl Kept {
    /// The kept form of a message heard on `freq_hz` at `level_db`.
    pub fn of(m: &Message, freq_hz: f64, level_db: f32) -> Kept {
        let pos = m.position();
        let (reading, kind, category) = reading_of(m);
        Kept {
            freq_hz,
            level_db,
            registration: m.registration.clone(),
            aircraft: String::new(),
            flight: if m.downlink {
                m.flight.clone()
            } else {
                String::new()
            },
            label: m.label.clone(),
            label_meaning: m.label_meaning.unwrap_or("").to_owned(),
            mode: m.mode.to_string(),
            block_id: m.block_id.to_string(),
            msn: m.msn.clone(),
            downlink: m.downlink,
            ack: match m.tak {
                sdrkit_proto_aero::acars::Tak::Nak => "NAK".into(),
                sdrkit_proto_aero::acars::Tak::Block(c)
                | sdrkit_proto_aero::acars::Tak::Other(c) => (c as char).to_string(),
            },
            text: m.text.clone(),
            reading,
            reading_kind: kind.into(),
            reading_category: category.into(),
            cpdlc: CpdlcRow::of(m),
            blocks: m.blocks,
            incomplete: m.incomplete,
            bits_fixed: m.quality.bits_fixed,
            position: pos.map(|(p, _)| (p.lat_deg, p.lon_deg)),
            alt_ft: pos.and_then(|(_, a)| a),
        }
    }
}

/// Returns the motion of an ADS-C earth reference, when present and valid.
pub fn motion_of(a: &adsc::Adsc) -> Option<Motion> {
    a.earth_reference()
        .map(|(track_deg, gs_kt, vs_fpm)| Motion {
            track_deg,
            gs_kt,
            vs_fpm: Some(vs_fpm),
        })
}

/// A predicted route: the next point (latitude, longitude, altitude in feet), the seconds to it, and the point after.
pub type Route = ((f64, f64, f64), u32, (f64, f64, f64));

/// What a generator puts in an ADS-C downlink.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdscReport {
    /// Degrees north.
    pub lat_deg: f64,
    /// Degrees east.
    pub lon_deg: f64,
    /// Feet.
    pub alt_ft: f64,
    /// Seconds past the hour.
    pub sec_past_hour: f64,
    /// Track, degrees true, and ground speed, knots, for an earth reference group.
    pub track: Option<(f64, f64)>,
    /// The predicted route group: the next point, seconds to it, and the point after.
    pub route: Option<Route>,
    /// A meteorological group: wind speed (kt), wind direction (degrees) and temperature (°C).
    pub meteo: Option<(f64, f64, f64)>,
}

/// The ADS-C envelope text (`GROUND.ADS` address, groups, CRC) of a basic report from `registration` with the flight id
/// `flight`. An error when a value cannot be encoded.
pub fn adsc_envelope(
    ground: &str,
    registration: &str,
    flight: &str,
    r: &AdscReport,
) -> Result<String, sdrkit_proto_aero::Error> {
    let mut data = Vec::new();
    Group::Report(Report {
        kind: ReportKind::Basic,
        lat_deg: r.lat_deg,
        lon_deg: r.lon_deg,
        alt_ft: r.alt_ft,
        sec_past_hour: r.sec_past_hour,
        fom: Fom {
            accuracy: 6,
            redundancy: true,
            tcas: true,
        },
    })
    .encode(&mut data)?;
    if !flight.is_empty() {
        Group::flight_id(flight)?.encode(&mut data)?;
    }
    if let Some((track_deg, gs_kt)) = r.track {
        Group::EarthReference {
            track_deg,
            valid: true,
            gs_kt,
            vs_fpm: 0.0,
        }
        .encode(&mut data)?;
    }
    if let Some((next, eta_s, then)) = r.route {
        let w = |p: (f64, f64, f64)| Waypoint {
            lat_deg: p.0,
            lon_deg: p.1,
            alt_ft: p.2,
        };
        Group::PredictedRoute {
            next: w(next),
            eta_s,
            then: w(then),
        }
        .encode(&mut data)?;
    }
    if let Some((wind_kt, wind_dir_deg, temp_c)) = r.meteo {
        Group::Meteo {
            wind_kt,
            valid: true,
            wind_dir_deg,
            temp_c,
        }
        .encode(&mut data)?;
    }
    let mut out = String::new();
    arinc622::build(ground, Imi::Ads, registration, &data, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_proto_aero::acars::assemble::Quality;
    use sdrkit_proto_aero::acars::{strip_parity, Block, BlockSpec, End};

    #[test]
    fn an_adsc_downlink_reads_back_with_its_position() {
        let r = AdscReport {
            lat_deg: 51.5,
            lon_deg: -0.4,
            alt_ft: 36000.0,
            sec_past_hour: 100.0,
            track: Some((90.0, 450.0)),
            route: None,
            meteo: None,
        };
        let env = adsc_envelope("TESTGND", "G-EUUA", "BA0117", &r).unwrap();
        let text = format!("M01ABA0117/{env}");
        let mut body = Vec::new();
        BlockSpec::new('2', "G-EUUA", b'1', End::Etx)
            .label("B6")
            .text(&text)
            .body(&mut body)
            .unwrap();
        strip_parity(&mut body);
        let m = Message::from_block(&Block::parse(&body).unwrap(), Quality::GOOD);
        let a = adsc_of(&m).unwrap();
        let p = a.position().unwrap();
        assert!((p.lat_deg - 51.5).abs() < 1e-3 && (p.lon_deg + 0.4).abs() < 1e-3);
        assert_eq!(motion_of(a).unwrap().gs_kt, 450.0);
        assert!(reading_of(&m).0.starts_with("ADS-C report 51.5"));
        let k = Kept::of(&m, 131.525e6, -20.0);
        assert_eq!(k.ack, "NAK");
        assert_eq!(k.alt_ft, Some(36000.0));
    }

    #[test]
    fn a_plain_text_position_reaches_the_position_outputs() {
        let mut body = Vec::new();
        BlockSpec::new('2', "VH-QPA", b'1', End::Etx)
            .label("H1")
            .text("M85AQF0073YSSY,KSFO,101621,- 4.9985,-169.9820,35003,290,  35.1, 44100,S05W169,S02W167,1645")
            .body(&mut body)
            .unwrap();
        strip_parity(&mut body);
        let m = Message::from_block(&Block::parse(&body).unwrap(), Quality::GOOD);
        let l = Located::of_message(&m).unwrap();
        assert!((l.lat + 4.9985).abs() < 1e-6 && (l.lon + 169.982).abs() < 1e-6);
        assert_eq!((l.alt_ft, l.origin), (Some(35003.0), Origin::Text));
        let k = Kept::of(&m, 131.525e6, -20.0);
        assert_eq!(
            (k.reading_kind.as_str(), k.reading_category.as_str()),
            ("Position report", "position")
        );
        assert!(k.position.is_some());
    }
}
