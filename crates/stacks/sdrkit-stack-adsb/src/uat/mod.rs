//! UAT on 978 MHz with FIS-B: the demodulator, the payloads, the FIS-B products, the table of targets and the test signal.
//!
//! Standard: RTCA DO-282B; RTCA DO-358B

pub mod demod;
pub mod fisb;
pub mod generator;
pub mod payload;

use crate::codes::parse_squawk;
use crate::geo::{bearing_deg, distance_nm};
use crate::settings::EXPIRY_S;
use crate::table::emergency_reason;
use crate::typed::{
    self, FisbBlockRow, FisbProductRow, FisbRecordRow, FisbVertexRow, GeometryKind, RecordKind,
    UatAircraftRow, UatFields, UatLogRow, UatMessage, UatMessageKind, FISB_BLOCKS, FISB_PRODUCTS,
    FISB_RECORDS, FISB_VERTICES, UAT_AIRCRAFT, UAT_LOG, UAT_MESSAGE,
};
use demod::{UatDemod, UatFrame, UatKind, UatOut};
use payload::{decode_downlink, decode_uplink};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, Time};
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{
    ConfigError, EventDef, LockState, Record, Results, SettingError, SettingId, SettingSet,
    StackConfig, StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError,
    Io, Item, Kind, Port, RateFrom, RunContext, StackGraph, Timed,
};
use sdrkit_proto_aero::aircraft::{Aircraft, AircraftTable, Fix, Source, TrailSettings};
use sdrkit_util_geo::{Position, Text};
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Spacing, Stamp, Tables};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 1.9e6;

/// The UAT data of a target, beside the shared aircraft row ([`Aircraft`]): the flight id is the callsign, the fix the
/// position, the track the track or heading, and the vertical rate the shared one.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UatExtra {
    /// Address qualifier.
    pub address_qualifier: u8,
    /// Squawk (when the mode status says the characters are one).
    pub squawk: Option<String>,
    /// Barometric altitude.
    pub baro_alt_ft: Option<i32>,
    /// Geometric altitude.
    pub geo_alt_ft: Option<i32>,
    /// On the ground.
    pub on_ground: bool,
    /// TIS-B.
    pub tisb: bool,
    /// NIC.
    pub nic: u8,
    /// Emitter category.
    pub category: Option<u8>,
    /// Emergency status.
    pub emergency: u8,
    /// Level, dBFS.
    pub level_dbfs: f32,
    /// Distance (m) and bearing from the station, at the last report.
    pub range: Option<(f64, f64)>,
}

impl Default for UatExtra {
    fn default() -> Self {
        UatExtra {
            address_qualifier: 0,
            squawk: None,
            baro_alt_ft: None,
            geo_alt_ft: None,
            on_ground: false,
            tisb: false,
            nic: 0,
            category: None,
            emergency: 0,
            level_dbfs: -120.0,
            range: None,
        }
    }
}

impl Shift for UatExtra {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// One UAT target.
pub type UatAircraft = Aircraft<UatExtra>;

/// The number of trail points of a UAT target.
const TRAIL_POINTS: usize = 64;

/// The six hex digits of the address of `a`.
fn address_hex(a: &UatAircraft) -> Text<6> {
    let mut hex = Text::<6>::new();
    // Six hex digits fit.
    let _ = write!(hex, "{:06X}", a.icao.or(a.non_icao).unwrap_or(0));
    hex
}

impl UatExtra {
    /// The row of [`UAT_AIRCRAFT`].
    fn row_write(
        a: &UatAircraft,
        seen: Option<&Seen>,
        out: RowOut<'_, '_, UatAircraftRow<'static>>,
    ) {
        let x = &a.extra;
        let reg = a.icao.and_then(sdrkit_util_ids::icao::registration);
        let row = UatAircraftRow {
            icao: &address_hex(a),
            address_qualifier: i64::from(x.address_qualifier),
            callsign: (!a.flight.is_empty()).then_some(a.flight.as_str()),
            registration: reg.as_ref().map(|r| r.as_str()),
            country: a.country,
            squawk: x.squawk.as_deref(),
            position: a.fix.map(|(_, f)| (f.position.lat_deg, f.position.lon_deg)),
            alt_ft: x.baro_alt_ft.map(i64::from),
            geo_alt_ft: x.geo_alt_ft.map(i64::from),
            speed_kt: a.gs_kt,
            heading_deg: a.track_deg,
            vertical_rate_fpm: a.vs_fpm.map(|v| v as i64),
            on_ground: x.on_ground,
            tisb: x.tisb,
            nic: i64::from(x.nic),
            category: x.category.map(i64::from),
            emergency: i64::from(x.emergency),
            level_dbfs: f64::from(x.level_dbfs),
            distance_m: x.range.map(|r| r.0),
            bearing_deg: x.range.map(|r| r.1),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        };
        out.put(&row);
    }

    /// The last position, labelled with the callsign, else the address.
    fn row_point(a: &UatAircraft) -> Option<Point<'_>> {
        let (_, fix) = a.fix?;
        let label = if a.flight.is_empty() {
            std::borrow::Cow::Owned(address_hex(a).as_str().to_owned())
        } else {
            std::borrow::Cow::Borrowed(a.flight.as_str())
        };
        let p = Point::new(fix.position, label);
        Some(match a.track_deg {
            Some(h) => p.heading(h as f32),
            None => p,
        })
    }
}

/// A FIS-B product held.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FisbEntry {
    /// The number of the product, counted from the first product heard. Its text in six digits is the row key.
    #[serde(default)]
    pub seq: u64,
    /// Signal time received.
    pub time: f64,
    /// The product's own time ("HH:MM", "HH:MM:SS", with "MM-DD " in front when given).
    pub stamp: String,
    /// The decoded product.
    pub product: fisb::Product,
}

impl Shift for FisbEntry {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl FisbEntry {
    /// The row of [`FISB_PRODUCTS`].
    fn row(&self) -> FisbProductRow<'_> {
        let p = &self.product;
        FisbProductRow {
            time_s: self.time,
            product_id: i64::from(p.apdu.product_id),
            name: &p.name,
            stamp: &self.stamp,
            text: &p.text,
            records: p.records.len() as i64,
            blocks: p.blocks.len() as i64,
        }
    }
}

impl Shaped for FisbEntry {
    type Row = FisbProductRow<'static>;
    /// The row of [`FISB_PRODUCTS`].
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&self.row());
    }
}

/// A log line of the UAT monitor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UatLogLine {
    /// Signal time.
    pub time: f64,
    /// What the message is.
    pub kind: UatMessageKind,
    /// Address.
    pub address: String,
    /// Level.
    pub level_dbfs: f32,
    /// Summary.
    pub summary: String,
    /// The position decoded from the message, if it carries one.
    pub pos: Option<(f64, f64)>,
    /// The squawk of the aircraft, and its emergency status (0 none), when the message is from an aircraft.
    #[serde(default)]
    pub emergency: Option<(Option<u16>, u8)>,
}

impl Shift for UatLogLine {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for UatLogLine {
    type Row = UatLogRow<'static>;
    /// The row of [`UAT_LOG`].
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let row = UatLogRow {
            time_s: self.time,
            kind: self.kind,
            address: (!self.address.is_empty()).then_some(self.address.as_str()),
            level_dbfs: f64::from(self.level_dbfs),
            summary: &self.summary,
        };
        out.put(&row);
    }
}

/// A decoded UAT message, as an event of [`UAT_MESSAGE`].
#[derive(Clone, Debug)]
pub struct UatEvent {
    /// The log line.
    pub line: UatLogLine,
}

impl Item for UatEvent {
    const KIND: &'static str = "uat.message";
    fn event_def() -> Option<&'static EventDef> {
        Some(UAT_MESSAGE.def())
    }
    fn write_fields(&self, r: &mut Record) {
        let l = &self.line;
        let urgent = l
            .emergency
            .and_then(|(squawk, e)| emergency_reason(squawk, e))
            .is_some();
        UatMessage {
            text: &l.summary,
            kind: l.kind,
            source: (!l.address.is_empty()).then_some(l.address.as_str()),
            level_dbfs: f64::from(l.level_dbfs),
            position: l.pos,
            urgency: if urgent {
                keys::Urgency::Emergency
            } else {
                keys::Urgency::None
            },
            family: keys::Family::Aircraft,
        }
        .write(r);
    }
}

/// What the UAT tracker hears and keeps across a restart.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UatHeld {
    /// Targets.
    pub aircraft: AircraftTable<UatExtra>,
    /// FIS-B products, oldest first.
    pub fisb: LogRing<FisbEntry>,
    /// The message log, oldest first.
    pub log: LogRing<UatLogLine>,
}

impl Default for UatHeld {
    fn default() -> Self {
        UatHeld {
            aircraft: AircraftTable::new(2000).with_trail(TrailSettings {
                points: TRAIL_POINTS,
                spacing: Spacing::default(),
            }),
            fisb: LogRing::new(64),
            log: LogRing::new(64),
        }
    }
}

impl Shift for UatHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.aircraft.shift_times(by_s);
        self.fisb.shift_times(by_s);
        self.log.shift_times(by_s);
    }
}

impl Tables for UatHeld {
    const FORMAT: StateFormat = StateFormat::new("adsb.uat_targets", 1);
    fn clear(&mut self) {
        self.aircraft.clear();
        self.fisb.clear();
        self.log.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.aircraft.adopt(saved.aircraft);
        self.fisb.adopt(saved.fisb);
        self.log.adopt(saved.log);
    }
}

/// The UAT table and results.
pub struct UatTracker {
    /// The station's position, from the run context.
    pub station: Option<(f64, f64)>,
    expiry_s: f64,
    held: UatHeld,
    segments: fisb::Reassembler,
    frames: u64,
    times: VecDeque<f64>,
    now: f64,
    snr: Option<f32>,
    last_good: f64,
    state: u8,
    report: u64,
    published: Published,
    events: Vec<UatEvent>,
    parts: FisbParts,
    fisb_seq: u64,
}

impl Default for UatTracker {
    fn default() -> Self {
        UatTracker {
            station: None,
            expiry_s: 60.0,
            held: UatHeld::default(),
            segments: fisb::Reassembler::default(),
            frames: 0,
            times: VecDeque::new(),
            now: 0.0,
            snr: None,
            last_good: f64::MIN,
            state: 0,
            report: 0,
            published: Published::new(UatFields::SHAPE.fields).with_tables(&typed::uat_tables()),
            events: Vec::new(),
            parts: FisbParts::default(),
            fisb_seq: 0,
        }
    }
}

fn stamp(a: &fisb::Apdu) -> String {
    let mut s = String::new();
    if let Some((m, d)) = a.month_day {
        s.push_str(&format!("{m:02}-{d:02} "));
    }
    s.push_str(&format!("{:02}:{:02}", a.hours, a.minutes));
    if let Some(x) = a.seconds {
        s.push_str(&format!(":{x:02}"));
    }
    s
}

fn twgo_time(t: &fisb::TwgoTime) -> String {
    let mut s = String::new();
    if let Some(m) = t.month {
        s.push_str(&format!("{m:02}-"));
    }
    if let Some(d) = t.day {
        s.push_str(&format!("{d:02} "));
    }
    s.push_str(&format!("{:02}:{:02}", t.hours, t.minutes));
    s
}

impl UatTracker {
    fn push_log(&mut self, l: UatLogLine) {
        self.events.push(UatEvent { line: l.clone() });
        self.held.log.push(l);
    }

    /// Takes one decoded frame.
    pub fn take(&mut self, f: &UatFrame) {
        self.now = self.now.max(f.time);
        self.frames += 1;
        self.last_good = f.time;
        self.times.push_back(f.time);
        let snr = f.level_dbfs - f.noise_dbfs;
        self.snr = Some(self.snr.map_or(snr, |s| s + 0.1 * (snr - s)));
        match f.kind {
            UatKind::Uplink => self.take_uplink(f),
            _ => self.take_downlink(f),
        }
    }

    fn add_product(&mut self, f: &UatFrame, p: fisb::Product) {
        let text: String = p.text.chars().take(120).collect();
        let e = FisbEntry {
            seq: self.fisb_seq,
            time: f.time,
            stamp: stamp(&p.apdu),
            product: p,
        };
        self.push_log(UatLogLine {
            time: f.time,
            kind: UatMessageKind::Fisb,
            address: String::new(),
            level_dbfs: f.level_dbfs,
            summary: format!("{} {}", e.product.name, text),
            pos: None,
            emergency: None,
        });
        self.fisb_seq += 1;
        self.held.fisb.push(e);
    }

    fn take_uplink(&mut self, f: &UatFrame) {
        let Some((h, frames)) = decode_uplink(&f.bytes) else {
            return;
        };
        let mut any = false;
        for fr in frames {
            if fr.frame_type != 0 || fr.data.len() < 4 {
                continue;
            }
            let Some(p) = fisb::decode_product(&fr.data) else {
                continue;
            };
            any = true;
            let whole = if p.apdu.s_flag {
                self.segments.push(f.time, &fr.data)
            } else {
                None
            };
            self.add_product(f, p);
            // A segment that completed its file adds the whole product after it.
            if let Some(w) = whole {
                self.add_product(f, w);
            }
        }
        if !any {
            self.push_log(UatLogLine {
                time: f.time,
                kind: UatMessageKind::Uat,
                address: String::new(),
                level_dbfs: f.level_dbfs,
                summary: format!("UAT uplink, slot {}", h.slot_id),
                pos: None,
                emergency: None,
            });
        }
    }

    fn take_downlink(&mut self, f: &UatFrame) {
        let Some(d) = decode_downlink(&f.bytes) else {
            return;
        };
        let (kind, kind_text) = if d.tisb {
            (UatMessageKind::Tisb, "TIS-B")
        } else {
            (UatMessageKind::Adsb, "ADS-B")
        };
        let hex = format!("{:06X}", d.address);
        if d.payload_type > 1 {
            self.push_log(UatLogLine {
                time: f.time,
                kind: UatMessageKind::Uat,
                address: hex,
                level_dbfs: f.level_dbfs,
                summary: format!(
                    "UAT MDB type {} ({} frame)",
                    d.payload_type,
                    if f.kind == UatKind::Long {
                        "long"
                    } else {
                        "short"
                    }
                ),
                pos: None,
                emergency: None,
            });
            return;
        }
        // Address qualifier 0 is an ICAO address (DO-282B 2.2.4.5.1.2).
        let non_icao = d.address_qualifier != 0;
        let Some(a) = self
            .held
            .aircraft
            .heard_address(d.address, non_icao, Stamp::at(f.time))
        else {
            return;
        };
        let x = &mut a.extra;
        x.address_qualifier = d.address_qualifier;
        x.tisb = d.tisb;
        x.level_dbfs = f.level_dbfs;
        x.nic = d.sv.nic;
        for alt in [d.sv.alt, d.aux_alt].into_iter().flatten() {
            if alt.1 {
                x.geo_alt_ft = Some(alt.0);
            } else {
                x.baro_alt_ft = Some(alt.0);
            }
        }
        x.on_ground = d.sv.air_ground == 2;
        let alt_ft = x.baro_alt_ft.or(x.geo_alt_ft);
        if let Some((lat, lon)) = d.sv.pos {
            let fix = Fix {
                position: Position::new(lat, lon),
                alt_ft: alt_ft.map(f64::from),
                on_ground: x.on_ground,
                source: Source::Other,
            };
            let _ = a.position(f.time, fix, None);
        }
        if d.sv.speed_kt.is_some() {
            a.gs_kt = d.sv.speed_kt;
        }
        if let Some(t) = d.sv.track_deg {
            a.track_deg = Some(t);
            a.track_reported_at = Some(f.time);
        }
        if let Some((v, _)) = d.sv.vrate {
            a.vs_fpm = Some(f64::from(v));
        }
        let x = &mut a.extra;
        if let Some(ms) = &d.ms {
            if ms.is_callsign {
                if !ms.ident.is_empty() {
                    a.flight.clone_from(&ms.ident);
                }
            } else if parse_squawk(&ms.ident).is_some() {
                x.squawk = Some(ms.ident.clone());
            }
            x.category = Some(ms.category);
            x.emergency = ms.emergency;
        }
        let pos = a.fix.map(|(_, f)| (f.position.lat_deg, f.position.lon_deg));
        let summary = match (a.flight.as_str(), pos) {
            ("", Some((la, lo))) => format!(
                "UAT {kind_text} {hex} {la:.4} {lo:.4} {} ft",
                alt_ft.unwrap_or(0)
            ),
            (c, Some((la, lo))) => format!(
                "UAT {kind_text} {hex} {c} {la:.4} {lo:.4} {} ft",
                alt_ft.unwrap_or(0)
            ),
            _ => format!("UAT {kind_text} {hex} type {}", d.payload_type),
        };
        let x = &a.extra;
        let squawk = x.squawk.as_deref().and_then(parse_squawk);
        let line = UatLogLine {
            time: f.time,
            kind,
            emergency: Some((squawk, x.emergency)),
            address: hex,
            level_dbfs: f.level_dbfs,
            summary: if d.tisb {
                summary.replacen("UAT TIS-B", "TIS-B", 1)
            } else {
                summary
            },
            pos: d.sv.pos,
        };
        self.push_log(line);
    }

    /// A report at signal time `now`, with the demodulator's noise figure.
    pub fn report_at(&mut self, now: f64, noise: f32) {
        self.now = self.now.max(now);
        let now = self.now;
        self.report += 1;
        while self.times.front().is_some_and(|&t| now - t > 1.0) {
            self.times.pop_front();
        }
        self.held.aircraft.expire(now, self.expiry_s);
        let old = self.state;
        self.state = if now - self.last_good <= 5.0 { 2 } else { 0 };
        let rx = self.station;
        for a in self.held.aircraft.records_mut() {
            let pos = a.fix.map(|(_, f)| (f.position.lat_deg, f.position.lon_deg));
            a.extra.range = match (rx, pos) {
                (Some((rl, ro)), Some((la, lo))) => Some((
                    distance_nm(rl, ro, la, lo) * sdrkit_util_geo::NAUTICAL_MILE_M,
                    bearing_deg(rl, ro, la, lo),
                )),
                _ => None,
            };
        }
        let count = self.held.aircraft.len();
        let with_pos = self
            .held
            .aircraft
            .rows()
            .iter()
            .filter(|r| r.record.fix.is_some())
            .count();
        let tisb = self
            .held
            .log
            .iter()
            .filter(|l| l.kind == UatMessageKind::Tisb)
            .count();
        let mps = self.times.len() as f64;
        let snr = self.snr.unwrap_or(0.0);
        let mut summary = String::new();
        let _ = write!(
            summary,
            "{count} aircraft ({with_pos} with position), {mps:.0} msg/s, {} FIS-B",
            self.held.fisb.len()
        );
        let status = Status {
            lock_state: if self.state == 2 {
                LockState::Locked
            } else {
                LockState::Searching
            },
            snr_db: (self.state == 2).then_some(snr),
            level_dbfs: Some(noise),
            summary,
            ..Status::new(LockState::Searching)
        };
        let p = &mut self.published;
        {
            let r = p.results_mut();
            if old != 2 && self.state == 2 {
                r.log("UAT messages are coming in");
            } else if old == 2 && self.state != 2 {
                r.log("no UAT messages for 5 s");
            }
            r.set_status(&status);
        }
        p.put(&UatFields {
            aircraft_count: count as i64,
            with_position: with_pos as i64,
            message_rate: Hertz(mps),
            fisb_products: self.held.fisb.len() as i64,
            tisb_recent: tisb as i64,
            fisb_pending: self.segments.pending() as i64,
            frames_ok: self.frames as i64,
            snr_db: (self.state == 2).then_some(snr as f64),
            noise_dbfs: noise as f64,
            report: self.report as i64,
            time_s: now,
        });
        sdrkit_util_objects::shaped::rows_by(
            self.held.aircraft.rows(),
            &UAT_AIRCRAFT,
            p,
            |_| true,
            |r, out| UatExtra::row_write(r.record, Some(r.seen), out),
        );
        sdrkit_util_objects::shaped::row_points_by(
            self.held.aircraft.rows(),
            UAT_AIRCRAFT.def(),
            "",
            p.results_mut(),
            UatExtra::row_point,
        );
        sdrkit_util_objects::shaped::log_rows(&self.held.log, &UAT_LOG, now, p);
        self.parts.publish(&self.held.fisb, p);
    }

    fn clear(&mut self) {
        self.held.clear();
        self.segments.clear();
        self.frames = 0;
        self.parts.clear(&mut self.published);
    }
}

/// The rows of the parts of the FIS-B products held: the TWGO records, their vertices and the grid blocks. The rows of a product
/// are written once, when it comes, and removed when it leaves the products held.
#[derive(Debug, Default)]
struct FisbParts {
    /// The products written: (sequence number in the log, records, blocks, vertices of each record).
    written: VecDeque<(u64, usize, usize, Vec<usize>)>,
}

impl FisbParts {
    fn publish(&mut self, fisb: &LogRing<FisbEntry>, p: &mut Published) {
        let oldest = fisb.iter().next().map(|e| e.seq);
        while let Some(&(seq, ..)) = self.written.front() {
            if oldest.is_some_and(|o| seq >= o) {
                break;
            }
            if let Some((seq, records, blocks, vertices)) = self.written.pop_front() {
                remove_parts(p, seq, records, blocks, &vertices);
            }
        }
        let newest = self.written.back().map(|w| w.0);
        let mut key = String::new();
        for e in fisb.iter() {
            if newest.is_some_and(|n| e.seq <= n) {
                continue;
            }
            key.clear();
            let _ = write!(key, "{:06}", e.seq);
            FISB_PRODUCTS.put(p, &key, &e.row());
            let vertices = write_parts(p, e.seq, e);
            self.written.push_back((
                e.seq,
                e.product.records.len(),
                e.product.blocks.len(),
                vertices,
            ));
        }
    }

    fn clear(&mut self, p: &mut Published) {
        self.written.clear();
        for def in [
            FISB_PRODUCTS.def(),
            FISB_RECORDS.def(),
            FISB_BLOCKS.def(),
            FISB_VERTICES.def(),
        ] {
            p.clear_rows(def);
        }
    }
}

/// The row key of part `j` of the product with sequence number `seq`.
fn part_key(key: &mut String, seq: u64, j: usize) -> &str {
    key.clear();
    let _ = write!(key, "{seq:06}.{j:04}");
    key
}

/// Removes the rows of the parts of a product.
fn remove_parts(p: &mut Published, seq: u64, records: usize, blocks: usize, vertices: &[usize]) {
    let mut key = String::new();
    let _ = write!(key, "{seq:06}");
    p.remove_row(&FISB_PRODUCTS, &key);
    for j in 0..records {
        p.remove_row(&FISB_RECORDS, part_key(&mut key, seq, j));
        for k in 0..vertices.get(j).copied().unwrap_or(0) {
            key.clear();
            let _ = write!(key, "{seq:06}.{j:04}.{k:04}");
            p.remove_row(&FISB_VERTICES, &key);
        }
    }
    for j in 0..blocks {
        p.remove_row(&FISB_BLOCKS, part_key(&mut key, seq, j));
    }
}

/// Writes the rows of the parts of product `e`, sequence number `seq`. Returns the number of vertices of each record.
fn write_parts(p: &mut Published, seq: u64, e: &FisbEntry) -> Vec<usize> {
    let pr = &e.product;
    let mut product = String::new();
    let _ = write!(product, "{seq:06}");
    let mut key = String::new();
    let mut counts = Vec::with_capacity(pr.records.len());
    let product_id = i64::from(pr.apdu.product_id);
    for (j, r) in pr.records.iter().enumerate() {
        let mut row = FisbRecordRow {
            product: &product,
            time_s: e.time,
            product_id,
            name: &pr.name,
            report: 0,
            kind: RecordKind::Text,
            active: None,
            label: None,
            overlay: None,
            start: None,
            end: None,
            geometry: None,
            position: None,
            vertices: None,
            radius_m: None,
            alt_bottom_ft: None,
            alt_top_ft: None,
            text: None,
        };
        let mut vertices: &[(f64, f64, i32)] = &[];
        let validity;
        match r {
            fisb::TwgoRecord::Text {
                report,
                active,
                text,
                ..
            } => {
                row.report = i64::from(*report);
                row.active = Some(*active);
                row.text = Some(text);
            }
            fisb::TwgoRecord::Graphic {
                report,
                overlay,
                label,
                times,
                geometry,
            } => {
                row.report = i64::from(*report);
                row.kind = RecordKind::Graphic;
                row.label = Some(label);
                row.overlay = Some(i64::from(*overlay));
                validity = times.as_ref().map(|(a, b)| (twgo_time(a), twgo_time(b)));
                row.start = validity.as_ref().map(|v| v.0.as_str());
                row.end = validity.as_ref().map(|v| v.1.as_str());
                let (kind, at, radius, alts) = match geometry {
                    fisb::Geometry::Path {
                        polygon,
                        vertices: v,
                    } => {
                        vertices = v;
                        let lo = v.iter().map(|v| v.2).min();
                        let hi = v.iter().map(|v| v.2).max();
                        (
                            if *polygon {
                                GeometryKind::Polygon
                            } else {
                                GeometryKind::Polyline
                            },
                            v.first().map(|v| (v.0, v.1)),
                            None,
                            lo.zip(hi),
                        )
                    }
                    fisb::Geometry::Circle {
                        centre,
                        radius_nm,
                        alt,
                    } => (
                        GeometryKind::Circle,
                        Some(*centre),
                        Some(radius_nm * sdrkit_util_geo::NAUTICAL_MILE_M),
                        Some(*alt),
                    ),
                    fisb::Geometry::Point((la, lo, alt)) => (
                        GeometryKind::Point,
                        Some((*la, *lo)),
                        None,
                        Some((*alt, *alt)),
                    ),
                    fisb::Geometry::Other(_) => (GeometryKind::Other, None, None, None),
                };
                row.geometry = Some(kind);
                row.position = at;
                row.vertices = Some(vertices.len() as i64);
                row.radius_m = radius;
                row.alt_bottom_ft = alts.map(|a| i64::from(a.0));
                row.alt_top_ft = alts.map(|a| i64::from(a.1));
            }
        }
        FISB_RECORDS.put(p, part_key(&mut key, seq, j), &row);
        let record = part_key(&mut key, seq, j).to_owned();
        for (k, v) in vertices.iter().enumerate() {
            key.clear();
            let _ = write!(key, "{record}.{k:04}");
            let vertex = FisbVertexRow {
                record: &record,
                index: k as i64,
                position: (v.0, v.1),
                alt_ft: i64::from(v.2),
            };
            FISB_VERTICES.put(p, &key, &vertex);
        }
        counts.push(vertices.len());
    }
    for (j, b) in pr.blocks.iter().enumerate() {
        let bins: Option<String> = b.bins.as_ref().map(|bins| {
            bins.iter()
                .map(|&v| char::from_digit(u32::from(v & 0xF), 16).unwrap_or('0'))
                .collect()
        });
        let max_bin = b
            .bins
            .as_ref()
            .map_or(0, |bins| i64::from(bins.iter().copied().max().unwrap_or(0)));
        let row = FisbBlockRow {
            product: &product,
            time_s: e.time,
            product_id,
            name: &pr.name,
            block: i64::from(b.block),
            south: b.south,
            scale: i64::from(b.scale),
            corner: (b.area.0, b.area.1),
            lat_size_deg: b.area.2,
            lon_size_deg: b.area.3,
            max_bin,
            bins: bins.as_deref(),
        };
        FISB_BLOCKS.put(p, part_key(&mut key, seq, j), &row);
    }
    counts
}

sdrkit_core::settings! {
    /// The settings of [`UatTracker`].
    pub enum UatTrackerSetting {
        /// Expiry time.
        ExpiryS = EXPIRY_S,
    }
}

const AIRCRAFT_ROW: sdrkit_core::settings::Argument<sdrkit_core::publish::field::Text> =
    sdrkit_util_objects::commands::row_of(UAT_AIRCRAFT.def().key);
const REMOVE: sdrkit_core::command::Command =
    sdrkit_util_objects::commands::remove_command(&[&AIRCRAFT_ROW]);

sdrkit_core::commands! {
    /// The commands of [`UatTracker`].
    pub enum UatTrackerCommand {
        /// Forgets the targets, the FIS-B products and the log.
        Clear = sdrkit_util_objects::commands::CLEAR,
        /// Forgets one target.
        Remove = REMOVE,
    }
}

impl Component for UatTracker {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "adsb.uat_tracker",
            vec![Port::typed::<UatOut>("in")],
            vec![Port::typed::<UatEvent>("events").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(UatTrackerSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<UatEvent>(), 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.station = ctx.station.position.map(|p| (p.lat_deg, p.lon_deg));
    }
    fn process(&mut self, io: &mut Io) {
        for it in io.items::<UatOut>(0) {
            match &it.value {
                UatOut::Frame(f) => self.take(f),
                UatOut::Tick(t) => self.report_at(t.time, t.noise_dbfs),
            }
        }
        let now = self.now;
        let o = io.items_out::<UatEvent>(0);
        for e in self.events.drain(..) {
            o.push(Timed {
                time: now,
                value: e,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match UatTrackerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            UatTrackerSetting::ExpiryS => self.expiry_s = EXPIRY_S.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(UatTrackerSetting::ExpiryS, Value::F64(self.expiry_s))])
    }
    fn commands(&self) -> &Commands {
        UatTrackerCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match UatTrackerCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            UatTrackerCommand::Clear => self.clear(),
            UatTrackerCommand::Remove => {
                let row = args.arg(&AIRCRAFT_ROW)?;
                let aircraft = &mut self.held.aircraft;
                let key = sdrkit_util_objects::rows::key_of_row(aircraft.rows(), row)
                    .ok_or_else(|| self.published.no_such_row(&UAT_AIRCRAFT, row))?;
                aircraft.remove(&key);
            }
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.now)?;
        self.fisb_seq = self.held.fisb.iter().map(|e| e.seq + 1).max().unwrap_or(0);
        self.parts.clear(&mut self.published);
        Ok(())
    }
}

/// Builds the UAT graph.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`].
pub fn build_graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("demod", UatDemod::default())
        .add("tracker", UatTracker::default())
        .input("demod")
        .connect("demod", "tracker")
        .output("tracker.events")
        .clock(0.25)
        .status_from("tracker")
        .catalog(catalog)
        .build()
}

/// The UAT receiver: its graph in a [`StackGraph`].
///
/// # Errors
///
/// The errors of [`build_graph`] and of [`sdrkit_graph::StackGraphBuilder::build`].
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(typed::shape("uat"), build_graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "UAT needs a sample rate of at least 1.9 Msps")
        .route("expiry_s", &["tracker.expiry_s"])?
        .command("clear", sdrkit_graph::CommandTarget::of("tracker", "clear"))
        .on_retune(sdrkit_graph::CommandTarget::of("tracker", "clear"))?
        .with_settings(&cfg.settings)
        .build()
}

#[cfg(test)]
mod tests {
    use super::generator::{UatBurst, UatGenerator, UatSchedule, EXAMPLE_PAYLOAD};
    use super::*;
    use sdrkit_core::{Complex32, Generator};

    #[test]
    fn the_example_frame_decodes_from_a_signal() {
        let mut g = UatGenerator::new(2.083334e6, UatSchedule::Example { bursts: 2 }, 1);
        let mut x = vec![Complex32::new(0.0, 0.0); 2_083_334 * 3];
        g.generate(&mut x);
        let mut d = demod::UatDemodulator::new(2.083334e6);
        let mut out = Vec::new();
        for c in x.chunks(5000) {
            d.feed(c, &mut out);
        }
        assert!(out.len() >= 2, "{}", out.len());
        assert_eq!(out[0].bytes, EXAMPLE_PAYLOAD.to_vec());
        assert_eq!(out[0].kind, UatKind::Short);
        let _ = UatBurst::downlink(0.0, &EXAMPLE_PAYLOAD, 0.1, 0.0, 0.0);
    }
}
