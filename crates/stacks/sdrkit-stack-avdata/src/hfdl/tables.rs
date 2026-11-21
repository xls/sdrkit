//! The HFDL results: the message list, the ground stations, the aircraft (by address, or by ground station and aircraft id
//! until the address is known), the channels, the counters, the status line, the events and the band hopper's requests.

use super::pdu::Message;
use super::stations::{self, Hop, Hopper};
use super::FOLLOW_BEST_BAND;
use super::{BurstOut, ChannelState, FrontState};
use crate::messages::Located;
use crate::report::{self, Heard};
use crate::typed::{
    HfdlAircraftRow, HfdlChannelRow, HfdlFields, HfdlMessageEvent, HfdlMessageRow, HfdlStationRow,
    HFDL_AIRCRAFT, HFDL_AIRCRAFT_ROW, HFDL_CHANNELS, HFDL_MESSAGE, HFDL_MESSAGES, HFDL_REMOVE,
    HFDL_STATIONS,
};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Clock, CommandError, CommandId, CommandSet, Commands, ConfigError, Event, HostRequest, Record,
    Results, SettingError, SettingId, SettingSet, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RunContext,
    Timed,
};
use sdrkit_util_geo::Position;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Spacing, Stamp, Tables as _, Trail,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Messages kept.
pub const MESSAGE_CAP: usize = 400;
/// Aircraft kept.
pub const AIRCRAFT_CAP: usize = 200;
/// Ground stations kept.
pub const STATION_CAP: usize = 64;
/// Channels shown.
pub const CHANNEL_CAP: usize = 512;

/// A message as kept in the list, with its burst.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Kept {
    /// Dial.
    pub dial_hz: f64,
    /// Level.
    pub level_db: f32,
    /// SNR.
    pub snr_db: f32,
    /// Offset.
    pub offset_hz: f64,
    /// bit/s.
    pub bps: u32,
    /// The ground station as its row key.
    pub gs_row: String,
    /// Every decoded layer, joined by " | ".
    pub detail: String,
    /// The ACARS fields, when it carries a block.
    pub acars: Option<crate::messages::Kept>,
    /// The message.
    pub m: Message,
}

impl Shaped for Heard<Kept> {
    type Row = HfdlMessageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let k = &self.item;
        let m = &k.m;
        let t = report::text;
        let a = k.acars.as_ref();
        let r = crate::messages::reading_cells(a);
        out.put(&HfdlMessageRow {
            time: self.wall.map(Time),
            signal_s: self.at,
            dial: Hertz(k.dial_hz),
            level_db: f64::from(k.level_db),
            snr_db: f64::from(k.snr_db),
            offset_hz: Hertz(k.offset_hz),
            bps: i64::from(k.bps),
            gs_id: i64::from(m.gs_id),
            ground_station: &k.gs_row,
            ac_id: m.ac_id.map(i64::from),
            kind: t(&m.kind),
            hfnpdu: t(&m.hfnpdu),
            uplink: m.uplink,
            summary: t(&m.summary),
            registration: a.and_then(|x| t(&x.registration)),
            flight: t(&m.flight),
            label: a.and_then(|x| t(&x.label)),
            block_id: a.and_then(|x| t(&x.block_id)),
            msn: a.and_then(|x| t(&x.msn)),
            text: a.and_then(|x| t(&x.text)),
            acars_ok: m.acars_ok,
            reading_kind: r.kind,
            reading_category: r.category,
            cpdlc_imi: r.imi,
            cpdlc_ground: r.ground,
            cpdlc_aircraft: r.aircraft,
            cpdlc_crc_ok: r.crc_ok,
            cpdlc_parsed: r.parsed,
            cpdlc_text: r.text,
            position: m.position.as_ref().map(|p| (p.lat, p.lon)),
            detail: t(&k.detail),
        });
    }
}

/// An aircraft.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Aircraft {
    /// The row key: the address, or the ground station and aircraft id while the address is not known.
    #[serde(default)]
    pub key: String,
    /// ICAO address.
    pub icao: Option<u32>,
    /// Registration.
    pub reg: String,
    /// Flight.
    pub flight: String,
    /// Ground station.
    pub gs_id: u8,
    /// Aircraft id.
    pub ac_id: Option<u8>,
    /// Messages.
    pub messages: u64,
    /// Channel.
    pub dial_hz: f64,
    /// Level of downlinks.
    pub level_db: f32,
    /// Logged on.
    pub logged_on: bool,
    /// Last position.
    pub position: Option<Located>,
    /// The trail.
    pub trail: Trail<Option<f64>>,
}

impl Default for Aircraft {
    fn default() -> Self {
        Aircraft {
            key: String::new(),
            icao: None,
            reg: String::new(),
            flight: String::new(),
            gs_id: 0,
            ac_id: None,
            messages: 0,
            dial_hz: 0.0,
            level_db: 0.0,
            logged_on: false,
            position: None,
            trail: Trail::new(
                sdrkit_proto_aero::aircraft::TRAIL_POINTS,
                Spacing::default(),
            ),
        }
    }
}

impl Shift for Aircraft {
    fn shift_times(&mut self, by_s: f64) {
        self.trail.shift_times(by_s);
    }
}

/// The text of a ground station id: its row key in the ground station table.
fn gs_text(id: u8, buf: &mut [u8; 3]) -> &str {
    let mut n = 0;
    let digits = [id / 100, id / 10 % 10, id % 10];
    for (i, d) in digits.iter().enumerate() {
        if *d > 0 || n > 0 || i == 2 {
            buf[n] = b'0' + d;
            n += 1;
        }
    }
    std::str::from_utf8(&buf[..n]).unwrap_or("")
}

impl Shaped for Aircraft {
    type Row = HfdlAircraftRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let t = report::text;
        let mut hex = [0; 6];
        let mut gs = [0; 3];
        out.put(&HfdlAircraftRow {
            key: &self.key,
            address: self.icao.map(|i| report::hex6(i, &mut hex)),
            registration: t(&self.reg),
            flight: t(&self.flight),
            gs_id: i64::from(self.gs_id),
            ground_station: gs_text(self.gs_id, &mut gs),
            count: i64::try_from(self.messages).unwrap_or(i64::MAX),
            dial: Hertz(self.dial_hz),
            last_heard: columns::last_heard(seen).map(Time),
            logged_on: self.logged_on,
            position: self.position.as_ref().map(|p| (p.lat, p.lon)),
            alt_ft: report::finite(self.position.as_ref().and_then(|p| p.alt_ft)),
        });
    }

    fn point(&self) -> Option<Point<'_>> {
        let p = self.position.as_ref()?;
        let label = if self.reg.is_empty() {
            self.flight.as_str()
        } else {
            self.reg.as_str()
        };
        let pt = Point::new(p.position(), label);
        Some(match p.track_deg {
            Some(t) => pt.heading(t as f32),
            None => pt,
        })
    }
}

/// A ground station.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Station {
    /// The station id.
    pub id: u8,
    /// Its name, from the station table, or "station N".
    pub name: String,
    /// Squitters.
    pub squitters: u64,
    /// Messages naming it.
    pub messages: u64,
    /// Heard directly (a squitter or an uplink).
    pub heard_direct: bool,
    /// Channel it was heard on.
    pub dial_hz: f64,
    /// Level.
    pub level_db: f32,
    /// UTC synchronised.
    pub utc_sync: bool,
    /// Frame index.
    pub frame_index: Option<u16>,
    /// Frequencies in use (last squitter), kHz, separated by spaces.
    pub active_khz: String,
}

impl Shift for Station {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Station {
    type Row = HfdlStationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&HfdlStationRow {
            gs_id: i64::from(self.id),
            name: &self.name,
            heard_direct: self.heard_direct,
            utc_sync: self.utc_sync,
            squitters: i64::try_from(self.squitters).unwrap_or(i64::MAX),
            count: i64::try_from(self.messages).unwrap_or(i64::MAX),
            dial: Hertz(self.dial_hz),
            last_heard: columns::last_heard(seen).map(Time),
            frame_index: self.frame_index.map(i64::from),
            active_khz: report::text(&self.active_khz),
        });
    }

    fn point(&self) -> Option<Point<'_>> {
        let st = stations::by_id(self.id)?;
        Some(Point::new(Position::new(st.lat, st.lon), st.name))
    }
}

impl Shaped for ChannelState {
    type Row = HfdlChannelRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let n = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        let mut gs = [0; 3];
        out.put(&HfdlChannelRow {
            dial: Hertz(self.dial_hz),
            name: self.station.and_then(stations::by_id).map(|s| s.name),
            ground_station: self.station.map(|id| gs_text(id, &mut gs)),
            in_band: self.in_band,
            found: n(self.preambles),
            good: n(self.good),
            bad: n(self.bad),
            snr_db: f64::from(self.snr_db),
            offset_hz: Hertz(f64::from(self.last_offset_hz)),
            bps: i64::from(self.last_bps),
        });
    }
}

/// Counters of the current input.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Counters {
    /// Good PDUs (squitters and MPDUs, one per burst).
    pub pdus_ok: u64,
    /// Bad bursts.
    pub bursts_bad: u64,
    /// Squitters.
    pub squitters: u64,
    /// LPDUs.
    pub lpdus: u64,
    /// ACARS messages.
    pub acars: u64,
    /// Positions.
    pub positions: u64,
    /// Good bursts at 300 / 600 / 1200 / 1800 bit/s.
    pub by_rate: [u64; 4],
    /// Preamble rejected, short, squitter FCS, header FCS, LPDU FCS, LPDU type.
    pub why: [u64; 6],
}

/// What is kept across sessions.
#[derive(Serialize, Deserialize)]
pub struct Store {
    messages: LogRing<Heard<Kept>>,
    aircraft: ObjectTable<String, Aircraft>,
    stations: ObjectTable<u8, Station>,
    /// Logon confirms: "gs/ac" to the address.
    logons: BTreeMap<String, u32>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            messages: LogRing::new(MESSAGE_CAP),
            aircraft: ObjectTable::new(AIRCRAFT_CAP),
            stations: ObjectTable::new(STATION_CAP),
            logons: BTreeMap::new(),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.messages.shift_times(by_s);
        self.aircraft.shift_times(by_s);
        self.stations.shift_times(by_s);
    }
}

impl sdrkit_util_objects::Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("avdata.hfdl", 2);
    fn clear(&mut self) {
        self.messages.clear();
        self.aircraft.clear();
        self.stations.clear();
        self.logons.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.messages.adopt(saved.messages);
        self.aircraft.adopt(saved.aircraft);
        self.stations.adopt(saved.stations);
        self.logons = saved.logons;
    }
}

/// The key of an aircraft whose address is not known yet.
fn unknown_key(gs: u8, ac: u8) -> String {
    format!("ground station {gs}, aircraft id {ac} (address not known)")
}

sdrkit_core::settings! {
    /// The settings of [`Tables`].
    pub enum TablesSetting {
        /// Follow the band with the most traffic.
        FollowBestBand = FOLLOW_BEST_BAND,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Tables`].
    pub enum TablesCommand {
        /// Forget every message, aircraft and ground station.
        Clear = objcmd::CLEAR,
        /// Forget one aircraft.
        Remove = HFDL_REMOVE.command,
    }
}

/// The results component (`avdata.hfdl_tables`).
pub struct Tables {
    published: Published,
    store: Store,
    counters: Counters,
    channels: Vec<ChannelState>,
    events: Vec<Event>,
    requests: Vec<HostRequest>,
    last_good: f64,
    last_pre: f64,
    preambles: u64,
    follow: bool,
    hopper: Hopper,
    hopper_status: String,
    announced: Vec<u32>,
    last_hop_t: f64,
    now_s: f64,
    summary: String,
    live: bool,
    clock: Clock,
    configured: bool,
}

impl Default for Tables {
    fn default() -> Self {
        Tables {
            published: crate::typed::results(super::ID),
            store: Store::default(),
            counters: Counters::default(),
            channels: Vec::new(),
            events: Vec::new(),
            requests: Vec::new(),
            last_good: f64::NEG_INFINITY,
            last_pre: f64::NEG_INFINITY,
            preambles: 0,
            follow: FOLLOW_BEST_BAND.default,
            hopper: Hopper::default(),
            hopper_status: String::new(),
            announced: Vec::new(),
            last_hop_t: f64::NEG_INFINITY,
            now_s: 0.0,
            summary: String::new(),
            live: false,
            clock: Clock::fixed(0.0),
            configured: false,
        }
    }
}

impl Tables {
    fn burst(&mut self, b: &BurstOut) {
        let t = b.time_s;
        self.last_good = t;
        let ri = match b.bps {
            300 => 0,
            600 => 1,
            1200 => 2,
            _ => 3,
        };
        self.counters.by_rate[ri] += 1;
        // A good PDU is a burst's squitter or MPDU (the LPDUs inside are counted apart).
        self.counters.pdus_ok += 1;
        for m in &b.messages {
            self.message(b, m.clone(), t);
        }
    }

    fn message(&mut self, b: &BurstOut, mut m: Message, t: f64) {
        let wall = report::wall(self.live, &self.clock);
        let mut at = Stamp::at(t);
        if let Some(w) = wall {
            at = at.with_wall(w);
        }
        if m.squitter {
            self.counters.squitters += 1;
        } else {
            self.counters.lpdus += 1;
        }
        if m.acars.is_some() {
            self.counters.acars += 1;
        }
        if m.position.is_some() {
            self.counters.positions += 1;
        }
        let key = |gs: u8, ac: u8| format!("{gs}/{ac}");
        // The address from the table of logon confirms.
        if let (Some(icao), Some(ac)) = (m.icao, m.assigned_id) {
            self.store.logons.insert(key(m.gs_id, ac), icao);
        }
        if m.icao.is_none() {
            if let Some(ac) = m.ac_id {
                m.icao = self.store.logons.get(&key(m.gs_id, ac)).copied();
            }
        }
        // Ground stations.
        let direct = m.squitter || m.uplink;
        let new = self.store.stations.get(&m.gs_id).is_none();
        if let Ok(up) = self.store.stations.upsert(&m.gs_id, at, || Station {
            id: m.gs_id,
            name: stations::name(m.gs_id),
            ..Station::default()
        }) {
            let s = up.row;
            s.messages += 1;
            if direct {
                s.heard_direct = true;
                s.dial_hz = b.dial_hz;
                s.level_db = b.level_db;
            }
            if m.squitter {
                s.squitters += 1;
                s.utc_sync = m.utc_sync;
                s.frame_index = m.frame_index;
                s.active_khz = m.active_khz.join(" ");
            }
        }
        if new && direct {
            self.published.results_mut().log(format!(
                "ground station {} {} heard on {:.3} MHz",
                m.gs_id,
                stations::name(m.gs_id),
                b.dial_hz / 1e6
            ));
        }
        if m.squitter {
            for k in m
                .active_khz
                .iter()
                .chain(m.others.iter().flat_map(|o| o.2.iter()))
            {
                if let Ok(v) = k.parse::<u32>() {
                    self.announced.push(v);
                }
            }
        }
        // Aircraft.
        let reg = m
            .acars
            .as_ref()
            .map(|a| a.registration.clone())
            .unwrap_or_default();
        let mut aircraft_id = String::new();
        if !m.squitter && (m.icao.is_some() || m.ac_id.is_some() || !reg.is_empty()) {
            let ak = match (m.icao, m.ac_id) {
                (Some(i), _) => format!("{i:06X}"),
                (None, Some(ac)) => unknown_key(m.gs_id, ac),
                (None, None) => reg.clone(),
            };
            // The (station, id) row folds into the address row when the address becomes known.
            if let (Some(_), Some(ac)) = (m.icao, m.ac_id) {
                if let Some(old) = self.store.aircraft.remove(&unknown_key(m.gs_id, ac)) {
                    if self.store.aircraft.get(&ak).is_none() {
                        let _ = self.store.aircraft.upsert(&ak, at, || old.record);
                    }
                }
            }
            if let Ok(up) = self.store.aircraft.upsert(&ak, at, Aircraft::default) {
                let a = up.row;
                if a.key != ak {
                    a.key.clone_from(&ak);
                }
                a.icao = a.icao.or(m.icao);
                a.gs_id = m.gs_id;
                if m.ac_id.is_some() {
                    a.ac_id = m.ac_id;
                }
                a.messages += 1;
                a.dial_hz = b.dial_hz;
                if !m.uplink {
                    a.level_db = b.level_db;
                }
                if !reg.is_empty() {
                    a.reg.clone_from(&reg);
                }
                if !m.flight.is_empty() && !m.uplink {
                    a.flight.clone_from(&m.flight);
                }
                if m.assigned_id.is_some() {
                    a.logged_on = true;
                }
                if m.logoff {
                    a.logged_on = false;
                }
                if let Some(p) = m.position.as_ref().filter(|_| !m.uplink) {
                    let _ = a.trail.push(t, p.position(), p.alt_ft);
                    a.position = Some(p.clone());
                }
            }
            aircraft_id = match m.icao {
                Some(i) => format!("{i:06X}"),
                None if !reg.is_empty() => reg.clone(),
                None => ak,
            };
        }
        let station = stations::name(m.gs_id);
        let (source, target) = if m.squitter {
            (station.as_str(), "")
        } else if m.uplink {
            (station.as_str(), aircraft_id.as_str())
        } else {
            (aircraft_id.as_str(), station.as_str())
        };
        self.events.push(HFDL_MESSAGE.event(
            t,
            b.dial_hz,
            &HfdlMessageEvent {
                source,
                target: report::text(target),
                text: &m.summary,
                snr_db: Some(f64::from(b.snr_db)).filter(|s| s.is_finite()),
                family: Family::Aircraft,
                position: m.position.as_ref().map(|p| (p.lat, p.lon)),
                kind: &m.kind,
                uplink: m.uplink,
                gs_id: i64::from(m.gs_id),
            },
        ));
        let acars = m
            .acars
            .as_ref()
            .map(|a| crate::messages::Kept::of(a, b.dial_hz, b.level_db));
        self.store.messages.push(Heard {
            at: t,
            wall,
            item: Kept {
                dial_hz: b.dial_hz,
                level_db: b.level_db,
                snr_db: b.snr_db,
                offset_hz: b.offset_hz,
                bps: b.bps,
                gs_row: m.gs_id.to_string(),
                detail: m.detail.join(" | "),
                acars,
                m,
            },
        });
    }

    fn report(&mut self, st: &FrontState) {
        let now = st.time_s;
        self.now_s = now;
        let pre: u64 = st.channels.iter().map(|c| c.preambles).sum();
        if st
            .channels
            .iter()
            .any(|c| c.preambles > 0 && now - c.last_heard_s < 60.0)
            || pre > self.preambles
        {
            self.last_pre = now;
        }
        self.preambles = pre;
        self.channels.clone_from(&st.channels);
        self.counters.why = st.why;
        self.counters.bursts_bad = st.channels.iter().map(|c| c.bad).sum();
        // The hopper, about once a second.
        if self.follow && now - self.last_hop_t >= 1.0 {
            self.last_hop_t = now;
            if let Hop::To(b) =
                self.hopper
                    .step(now, st.centre_hz, self.counters.pdus_ok, &self.announced)
            {
                self.requests.push(HostRequest::retune(
                    stations::BANDS[b].centre_mhz * 1e6,
                    "HFDL: the band with the most traffic",
                ));
            }
            self.announced.clear();
            self.hopper_status.clone_from(&self.hopper.status);
        }
        self.publish();
    }

    fn publish(&mut self) {
        let now = self.now_s;
        let state = if now - self.last_good <= 120.0 {
            2
        } else if now - self.last_pre <= 60.0 {
            1
        } else {
            0
        };
        let inb = self.channels.iter().filter(|c| c.in_band).count();
        let active = self
            .channels
            .iter()
            .filter(|c| c.good > 0 && now - c.last_heard_s <= 40.0)
            .count();
        let direct = self
            .store
            .stations
            .iter()
            .filter(|s| s.record.heard_direct)
            .count();
        let c = &self.counters;
        self.summary.clear();
        let _ = if inb == 0 {
            write!(
                self.summary,
                "HFDL: no channel inside the band, tune to an HFDL band (8.9, 10.06, 11.28, 13.3, 17.94, 21.96 MHz)"
            )
        } else if state == 0 {
            write!(self.summary, "HFDL: listening to {inb} channels")
        } else {
            write!(
                self.summary,
                "HFDL: {}, {active} of {inb} channels active, {} PDUs, {} bad, {direct} ground stations, {} aircraft",
                if state == 2 { "decoding" } else { "preambles" },
                c.pdus_ok,
                c.bursts_bad,
                self.store.aircraft.len()
            )
        };
        let last = self
            .channels
            .iter()
            .filter(|c| c.good > 0)
            .max_by(|a, b| a.last_heard_s.total_cmp(&b.last_heard_s));
        let p = &mut self.published;
        report::status(
            p.results_mut(),
            state,
            last.map(|c| c.snr_db),
            None,
            last.map(|c| f64::from(c.last_offset_hz)),
            &self.summary,
        );
        let n = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        let w = c.why;
        p.put(&HfdlFields {
            pdus: n(c.pdus_ok),
            bursts_bad: n(c.bursts_bad),
            squitters: n(c.squitters),
            lpdus: n(c.lpdus),
            acars: n(c.acars),
            positions: n(c.positions),
            bursts_300: n(c.by_rate[0]),
            bursts_600: n(c.by_rate[1]),
            bursts_1200: n(c.by_rate[2]),
            bursts_1800: n(c.by_rate[3]),
            lost_preamble: n(w[0]),
            lost_short: n(w[1]),
            lost_squitter_fcs: n(w[2]),
            lost_header_fcs: n(w[3]),
            lost_lpdu_fcs: n(w[4]),
            lost_lpdu_type: n(w[5]),
            hopper: &self.hopper_status,
            aircraft: n(self.store.aircraft.len() as u64),
        });
        shaped::log_rows(&self.store.messages, &HFDL_MESSAGES, now, p);
        shaped::rows(&self.store.aircraft, &HFDL_AIRCRAFT, now, p);
        shaped::points(&self.store.aircraft, "aircraft/", p.results_mut());
        shaped::rows(&self.store.stations, &HFDL_STATIONS, now, p);
        shaped::points(&self.store.stations, "station/", p.results_mut());
        shaped::rows_of(
            &self.channels,
            |c, w| write!(w, "{}", c.dial_hz.round() as u64),
            &HFDL_CHANNELS,
            now,
            p,
        );
    }
}

impl Component for Tables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.hfdl_tables",
            vec![
                Port::typed::<BurstOut>("bursts"),
                Port::typed::<FrontState>("state"),
            ],
            vec![
                Port::new("out", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(TablesSetting::schema())
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A restart forgets the signal and keeps the lists: their times move back by the old signal time.
        if self.configured {
            self.store.shift_times(-self.now_s);
        }
        self.configured = true;
        self.now_s = 0.0;
        self.last_good = f64::NEG_INFINITY;
        self.last_pre = f64::NEG_INFINITY;
        self.preambles = 0;
        self.counters = Counters::default();
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.clock = ctx.clock.clone();
    }

    fn process(&mut self, io: &mut Io) {
        for b in io.items::<BurstOut>(0) {
            self.burst(&b.value);
        }
        for s in io.items::<FrontState>(1) {
            self.report(&s.value);
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
        if !self.requests.is_empty() {
            let now = self.now_s;
            let o = io.items_out::<HostRequest>(1);
            for r in self.requests.drain(..) {
                o.push(Timed {
                    time: now,
                    value: r,
                });
            }
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TablesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TablesSetting::FollowBestBand => self.follow = FOLLOW_BEST_BAND.read(id, v)?,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([(TablesSetting::FollowBestBand, Value::Bool(self.follow))])
    }

    fn commands(&self) -> &Commands {
        TablesCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TablesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TablesCommand::Clear => {
                self.store.clear();
                self.counters = Counters::default();
            }
            TablesCommand::Remove => {
                let table = self
                    .published
                    .results()
                    .decl()
                    .table_id(HFDL_AIRCRAFT.key)
                    .expect("the declaration holds the aircraft table");
                objcmd::remove(
                    &mut self.store.aircraft,
                    table,
                    &mut self.published,
                    args,
                    &HFDL_AIRCRAFT_ROW,
                )?;
            }
        }
        self.publish();
        Ok(None)
    }

    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        self.channels.clear();
        self.counters = Counters::default();
        self.last_good = f64::NEG_INFINITY;
        self.last_pre = f64::NEG_INFINITY;
        self.preambles = 0;
        if !kept.table(HFDL_MESSAGES.key) {
            self.store.messages.clear();
        }
        if !kept.table(HFDL_AIRCRAFT.key) {
            self.store.aircraft.clear();
        }
        if !kept.table(HFDL_STATIONS.key) {
            self.store.stations.clear();
        }
        self.publish();
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now_s).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now_s)?;
        self.publish();
        Ok(())
    }
}
