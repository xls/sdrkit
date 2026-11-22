//! The drone table of Remote ID: drones by broadcast address, the message log, the counters, the published results, the events,
//! the commands, the saved state and the band request to the host.

use super::messages::{summary, Location, Message, MsgType, System, Transport, MSG_LEN};
use super::phy::{Phy, PhyStats};
use super::{plan, Band, Plan, RidItem, RidPacket, BAND};
use crate::family::{publish_tracks, wall, SignalClock, StatusLine};
use crate::typed::{
    DroneTrackRow, RemoteIdDroneEvent, RemoteIdDroneRow, RemoteIdFields, RemoteIdMessageRow,
    DRONE_REMOVE, DRONE_ROW, REMOTE_ID_DRONE, REMOTE_ID_DRONES, REMOTE_ID_MESSAGES,
    REMOTE_ID_TRACKS,
};
use remoteid::{Observation, TrustConfig, TrustState, Verdict};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Clock, ConfigError, Event, HostRequest, LockState, Record, Results, ResultsDecl,
    SettingError, SettingId, SettingSet, StateBlob, StateError, TableId, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RunContext,
    Timed,
};
use sdrkit_util_geo::{Position, KNOT_M_S};
use sdrkit_util_objects::commands::{self, Tables};
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Spacing, Stamp, Trail,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Most drones kept.
pub const DRONE_CAP: usize = 200;
/// Most log lines kept.
pub const LOG_CAP: usize = 400;
/// Trail points kept for each drone.
pub const TRAIL_CAP: usize = 120;
/// The least distance between two trail points, metres.
pub const TRAIL_SPACING_M: f64 = 0.1;
/// The least time between two events of one drone, seconds.
pub const EVENT_EVERY_S: f64 = 5.0;
/// The time after the last Remote ID message for which the lock state stays locked, seconds.
pub const HEARD_S: f64 = 5.0;

sdrkit_core::choice_enum! {
    /// How a drone was heard.
    pub enum Via {
        /// Bluetooth LE only.
        Ble = ("ble", "BLE"),
        /// Wi-Fi only.
        Wifi = ("wifi", "Wi-Fi"),
        /// Both.
        Both = ("both", "BLE + Wi-Fi"),
    }
}

/// What `remote_id` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("remote_id").results_decl()
}

/// The emergency status of a Location message (ASTM F3411-22a, operational status 3).
const STATUS_EMERGENCY: u8 = 3;

/// One drone: everything heard from one broadcast address.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Drone {
    /// The broadcast address (MAC text).
    pub address: String,
    /// Heard through BLE.
    pub via_ble: bool,
    /// Heard through Wi-Fi.
    pub via_wifi: bool,
    /// The last packet's level, dBFS.
    pub level: Option<f64>,
    /// The last message counter.
    pub counter: u8,
    /// Messages heard of each type.
    pub per_type: [u64; 6],
    /// UA type, once a Basic ID was heard.
    pub ua_type: Option<u8>,
    /// Serial number.
    pub serial: String,
    /// Specific session ID.
    pub session_id: String,
    /// CAA registration.
    pub caa: String,
    /// UTM assigned UUID.
    pub utm_uuid: String,
    /// The last Location message.
    pub location: Option<Location>,
    /// The track.
    pub trail: Trail,
    /// Authentication type, once heard.
    pub auth_type: Option<u8>,
    /// Authentication pages (the last page plus 1, from page 0).
    pub auth_pages: Option<u8>,
    /// Self-ID text.
    pub self_id: String,
    /// The last System message.
    pub system: Option<System>,
    /// Operator ID text.
    pub operator_id: String,
    /// The last BLE channel heard on.
    pub ble_channel: u8,
    /// The last Wi-Fi channel heard on.
    pub wifi_channel: u8,
    /// Signal time of the last event.
    #[serde(skip)]
    event_t: Option<f64>,
}

impl Default for Drone {
    fn default() -> Self {
        Drone {
            address: String::new(),
            via_ble: false,
            via_wifi: false,
            level: None,
            counter: 0,
            per_type: [0; 6],
            ua_type: None,
            serial: String::new(),
            session_id: String::new(),
            caa: String::new(),
            utm_uuid: String::new(),
            location: None,
            trail: Trail::new(TRAIL_CAP, Spacing::metres(TRAIL_SPACING_M)),
            auth_type: None,
            auth_pages: None,
            self_id: String::new(),
            system: None,
            operator_id: String::new(),
            ble_channel: 0,
            wifi_channel: 0,
            event_t: None,
        }
    }
}

impl Drone {
    /// The shown ID: serial number, else session ID, else CAA registration, else UTM UUID, else the address.
    pub fn display_id(&self) -> &str {
        [&self.serial, &self.session_id, &self.caa, &self.utm_uuid]
            .into_iter()
            .find(|s| !s.is_empty())
            .map_or(self.address.as_str(), |s| s.as_str())
    }
    /// How the drone was heard.
    pub fn via(&self) -> Via {
        match (self.via_ble, self.via_wifi) {
            (true, true) => Via::Both,
            (true, false) => Via::Ble,
            _ => Via::Wifi,
        }
    }
    /// The last position.
    pub fn position(&self) -> Option<Position> {
        let l = self.location.as_ref()?;
        Some(Position::new(l.lat?, l.lon?))
    }
    /// The operator's position.
    pub fn operator(&self) -> Option<Position> {
        let s = self.system.as_ref()?;
        Some(Position::new(s.op_lat?, s.op_lon?))
    }
    /// The ground speed, knots.
    pub fn speed_kt(&self) -> Option<f64> {
        self.location.as_ref()?.speed.map(|v| v / KNOT_M_S)
    }
    /// The urgency of the drone's present state.
    pub fn urgency(&self) -> Urgency {
        match &self.location {
            Some(l) if l.status == STATUS_EMERGENCY => Urgency::Emergency,
            _ => Urgency::None,
        }
    }
}

impl Shift for Drone {
    fn shift_times(&mut self, by_s: f64) {
        self.trail.shift_times(by_s);
        self.event_t = None;
    }
}

fn text_opt(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

fn pos(p: Option<Position>) -> Option<(f64, f64)> {
    p.map(|p| (p.lat_deg, p.lon_deg))
}

fn code(v: Option<u8>) -> Option<i64> {
    v.map(i64::from)
}

impl Shaped for Drone {
    type Row = RemoteIdDroneRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let l = self.location.as_ref();
        let s = self.system.as_ref();
        let n = |i: usize| i64::try_from(self.per_type[i]).unwrap_or(i64::MAX);
        out.put(&RemoteIdDroneRow {
            source: self.display_id(),
            address: &self.address,
            serial: text_opt(&self.serial),
            session_id: text_opt(&self.session_id),
            caa_registration: text_opt(&self.caa),
            utm_uuid: text_opt(&self.utm_uuid),
            ua_type: code(self.ua_type),
            status: code(l.map(|l| l.status)),
            position: pos(self.position()),
            alt_geo_m: l.and_then(|l| l.alt_geo),
            alt_baro_m: l.and_then(|l| l.alt_baro),
            height_m: l.and_then(|l| l.height),
            height_type: code(l.map(|l| l.height_type)),
            speed_kt: self.speed_kt(),
            vspeed_m_s: l.and_then(|l| l.vspeed),
            course_deg: l.and_then(|l| l.direction),
            horiz_acc: code(l.map(|l| l.h_acc)),
            vert_acc: code(l.map(|l| l.v_acc)),
            speed_acc: code(l.map(|l| l.speed_acc)),
            timestamp_ds: l.and_then(|l| l.timestamp).map(i64::from),
            operator_position: pos(self.operator()),
            operator_alt_m: s.and_then(|s| s.op_alt),
            location_type: code(s.map(|s| s.loc_type)),
            classification: code(s.map(|s| s.class_type)),
            category: code(s.map(|s| s.category)),
            class: code(s.map(|s| s.class)),
            system_timestamp_s: s.map(|s| i64::from(s.timestamp)),
            operator_id: text_opt(&self.operator_id),
            self_id: text_opt(&self.self_id),
            authentication: code(self.auth_type),
            authentication_pages: code(self.auth_pages),
            basic_id_messages: n(0),
            location_messages: n(1),
            authentication_messages: n(2),
            self_id_messages: n(3),
            system_messages: n(4),
            operator_id_messages: n(5),
            track_points: self.trail.len() as i64,
            via: self.via(),
            level_dbfs: self.level,
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
        });
    }
    fn point(&self) -> Option<Point<'_>> {
        let p = self.position()?;
        let pt = Point::new(p, self.display_id());
        Some(match self.location.as_ref().and_then(|l| l.direction) {
            Some(d) => pt.heading(d as f32),
            None => pt,
        })
    }
}

/// One line of the message log.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LogLine {
    /// Signal time, seconds.
    pub t: f64,
    /// Wall time, when the samples are live.
    pub wall: Option<f64>,
    /// BLE or Wi-Fi.
    pub transport: Transport,
    /// The channel (0 unknown).
    pub channel: u8,
    /// The address.
    pub address: String,
    /// The message type.
    pub msg_type: MsgType,
    /// Level, dBFS.
    pub level: Option<f64>,
    /// The summary line.
    pub summary: String,
}

impl Shift for LogLine {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

impl Shaped for LogLine {
    type Row = RemoteIdMessageRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&RemoteIdMessageRow {
            time_s: self.t,
            time: self.wall.map(Time),
            via: self.transport,
            radio_channel: (self.channel != 0).then_some(i64::from(self.channel)),
            address: &self.address,
            level_dbfs: self.level,
            r#type: self.msg_type,
            message: &self.summary,
        });
    }
}

/// The counters.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Counters {
    /// Messages decoded.
    pub decoded: u64,
    /// BLE good packets.
    pub ble_good: u64,
    /// Of them with Remote ID.
    pub ble_rid: u64,
    /// Wi-Fi good frames.
    pub wifi_good: u64,
    /// Of them with Remote ID.
    pub wifi_rid: u64,
    /// Messages of each type (Basic ID, Location, Authentication, Self-ID, System, Operator ID).
    pub per_type: [u64; 6],
}

/// What the drone table keeps across sessions: the drones, the log and the counters.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The drones by address.
    pub drones: ObjectTable<String, Drone>,
    /// The message log.
    pub log: LogRing<LogLine>,
    /// The counters.
    pub counters: Counters,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            drones: ObjectTable::new(DRONE_CAP),
            log: LogRing::new(LOG_CAP),
            counters: Counters::default(),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.drones.shift_times(by_s);
        self.log.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("drones.remote_id", 1);
    fn clear(&mut self) {
        self.drones.clear();
        self.log.clear();
        self.counters = Counters::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.drones.adopt(saved.drones);
        self.log.adopt(saved.log);
        self.counters = saved.counters;
    }
}

sdrkit_core::settings! {
    /// The settings of [`DroneTable`].
    pub enum DroneTableSetting {
        /// The band.
        Band = BAND,
    }
}


sdrkit_core::commands! {
    /// The commands of [`DroneTable`].
    pub enum DroneTableCommand {
        /// Forgets every drone, the log and the counters.
        Clear = commands::CLEAR,
        /// Forgets one drone.
        Remove = DRONE_REMOVE.command,
    }
}

/// The drone table component (`drones.rid_table`).
pub struct DroneTable {
    published: Published,
    drones_table: TableId,
    status: StatusLine,
    store: Store,
    clock: SignalClock,
    plan: Option<Plan>,
    live: bool,
    host_clock: Clock,
    /// CRC and FCS failures of each receiver: BLE, DSSS, OFDM.
    bad: [u64; 3],
    /// The failures at the last clear.
    bad_base: u64,
    level: [Option<f64>; 3],
    last_rid_t: Option<f64>,
    snr: Option<f64>,
    cfo: Option<f64>,
    band: Option<Band>,
    events: Vec<Event>,
    requests: Vec<HostRequest>,
    lines: Vec<String>,
    new_logged: u32,
    /// Spoof evidence and the last verdict of each broadcast address.
    trust: HashMap<String, (TrustState, Verdict)>,
    key: String,
    configured: bool,
    dirty: bool,
}

impl Default for DroneTable {
    fn default() -> Self {
        let decl = Arc::new(declaration());
        let drones_table = decl
            .table_id(REMOTE_ID_DRONES.key)
            .expect("the declaration holds the drones table");
        DroneTable {
            published: Published::from_decl(decl),
            drones_table,
            status: StatusLine::default(),
            store: Store::default(),
            clock: SignalClock::default(),
            plan: None,
            live: false,
            host_clock: Clock::fixed(0.0),
            bad: [0; 3],
            bad_base: 0,
            level: [None; 3],
            last_rid_t: None,
            snr: None,
            cfo: None,
            band: None,
            events: Vec::new(),
            requests: Vec::new(),
            lines: Vec::new(),
            new_logged: 0,
            trust: HashMap::new(),
            key: String::new(),
            configured: false,
            dirty: true,
        }
    }
}

impl DroneTable {
    /// The table.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Applies a packet with Remote ID at signal time `t`.
    pub fn take(&mut self, t: f64, p: &RidPacket) {
        let wall = wall(self.live, &self.host_clock);
        let stamp = match wall {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        };
        let plan = self.plan;
        let channel = match p.transport {
            Transport::WiFi => plan.map_or(0, |pl| pl.wifi.0),
            Transport::Ble => p.channel,
        };
        let s = &mut self.store;
        match p.transport {
            Transport::Ble => {
                s.counters.ble_rid += 1;
                s.counters.ble_good += 1;
            }
            Transport::WiFi => {
                s.counters.wifi_rid += 1;
                s.counters.wifi_good += 1;
            }
        }
        self.last_rid_t = Some(t);
        self.snr = Some(p.snr_db);
        self.cfo = Some(p.cfo_hz);
        let rx = match p.transport {
            Transport::Ble if p.payload.len() > MSG_LEN => remoteid::Transport::Bluetooth5,
            Transport::Ble => remoteid::Transport::Bluetooth4,
            Transport::WiFi => remoteid::Transport::WifiBeacon,
        };
        let obs = Observation::new(rx, Some(p.counter), &p.payload, (t.max(0.0) * 1e3) as u64);
        if self.trust.len() >= DRONE_CAP && !self.trust.contains_key(&p.address) {
            let drones = &s.drones;
            self.trust.retain(|a, _| drones.contains(a.as_str()));
        }
        let (state, last) = self
            .trust
            .entry(p.address.clone())
            .or_insert_with(|| (TrustState::new(), Verdict::Plausible));
        let a = state.observe(&TrustConfig::default(), &obs);
        if a.verdict != *last && a.verdict <= Verdict::Suspicious {
            let why: Vec<&str> = a.reasons.iter().map(|r| r.name()).collect();
            self.lines.push(format!(
                "Remote ID: drone {} is {} ({})",
                p.address,
                a.verdict,
                why.join(", ")
            ));
        }
        *last = a.verdict;
        let Ok(up) = s.drones.upsert(p.address.as_str(), stamp, || Drone {
            address: p.address.clone(),
            ..Drone::default()
        }) else {
            return;
        };
        if up.new && self.new_logged < 30 {
            self.new_logged += 1;
            self.lines
                .push(format!("Remote ID: new drone {}", p.address));
        }
        let d = up.row;
        match p.transport {
            Transport::Ble => {
                d.via_ble = true;
                d.ble_channel = channel;
            }
            Transport::WiFi => {
                d.via_wifi = true;
                d.wifi_channel = channel;
            }
        }
        d.level = p.level_dbfs;
        d.counter = p.counter;
        for v in &p.messages {
            let k = v.msg.kind();
            d.per_type[k.code() as usize] += 1;
            s.counters.decoded += 1;
            s.counters.per_type[k.code() as usize] += 1;
            match &v.msg {
                Message::BasicId(b) => {
                    d.ua_type = Some(b.ua_type);
                    match b.id_type {
                        1 => d.serial.clone_from(&b.id),
                        2 => d.caa.clone_from(&b.id),
                        3 => d.utm_uuid.clone_from(&b.id),
                        4 => d.session_id.clone_from(&b.id),
                        _ => {}
                    }
                }
                Message::Location(l) => {
                    if let (Some(a), Some(o)) = (l.lat, l.lon) {
                        // A position outside the trail's limits is not kept in the trail. The row still shows it.
                        let _ = d.trail.push(t, Position::new(a, o), ());
                    }
                    d.location = Some(l.clone());
                }
                Message::Authentication(a) => {
                    d.auth_type = Some(a.auth_type);
                    if a.page == 0 {
                        d.auth_pages = a.last_page.map(|l| l + 1);
                    }
                }
                Message::SelfId(x) => d.self_id.clone_from(&x.text),
                Message::System(x) => d.system = Some(x.clone()),
                Message::OperatorId(o) => d.operator_id.clone_from(&o.text),
            }
            s.log.push(LogLine {
                t,
                wall,
                transport: p.transport,
                channel,
                address: p.address.clone(),
                msg_type: k,
                level: p.level_dbfs,
                summary: summary(&v.msg),
            });
        }
        if let Some(at) = d.position() {
            if d.event_t.is_none_or(|e| t - e >= EVENT_EVERY_S) {
                d.event_t = Some(t);
                let freq = match (plan, p.transport) {
                    (Some(pl), _) if pl.centre_mhz == 0.0 => 0.0,
                    (Some(pl), Transport::Ble) => pl.ble.map_or(0.0, |b| pl.centre_mhz * 1e6 + b.1),
                    (Some(pl), Transport::WiFi) => pl.centre_mhz * 1e6 + pl.wifi.1,
                    (None, _) => 0.0,
                };
                let l = d.location.as_ref();
                let e = REMOTE_ID_DRONE.event(
                    t,
                    freq,
                    &RemoteIdDroneEvent {
                        source: d.display_id(),
                        address: &d.address,
                        position: (at.lat_deg, at.lon_deg),
                        alt_geo_m: l.and_then(|l| l.alt_geo),
                        speed_kt: d.speed_kt(),
                        course_deg: l.and_then(|l| l.direction),
                        operator_position: pos(d.operator()),
                        urgency: d.urgency(),
                        family: Family::Aircraft,
                    },
                );
                self.events.push(e);
            }
        }
        self.dirty = true;
    }

    fn take_stats(&mut self, s: &PhyStats) {
        let i = match s.phy {
            Phy::Ble => 0,
            Phy::Dsss => 1,
            Phy::Ofdm => 2,
        };
        self.bad[i] = s.bad;
        self.level[i] = s.level_dbfs;
        self.dirty = true;
    }

    fn clear(&mut self) {
        self.store.clear();
        self.bad_base = self.bad.iter().sum();
        self.new_logged = 0;
        self.trust.clear();
        self.last_rid_t = None;
        self.snr = None;
        self.cfo = None;
        self.dirty = true;
        if self.configured {
            self.publish();
        }
    }

    fn publish(&mut self) {
        let now = self.clock.now();
        let heard = self.last_rid_t.is_some_and(|t| now - t <= HEARD_S);
        let s = &self.store;
        let c = &s.counters;
        let bad = self.bad.iter().sum::<u64>().saturating_sub(self.bad_base);
        let n = s.drones.len();
        let level = self.level[1].or(self.level[0]);
        let p = &mut self.published;
        let lock_state = if heard {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let (snr, cfo) = (self.snr, self.cfo);
        if n == 0 {
            self.status.write(
                p,
                lock_state,
                snr,
                level,
                cfo,
                format_args!("listening for Remote ID"),
            );
        } else {
            let plural = if n == 1 { "" } else { "s" };
            self.status.write(
                p,
                lock_state,
                snr,
                level,
                cfo,
                format_args!("{n} drone{plural}, {} messages", c.decoded),
            );
        }
        for l in self.lines.drain(..) {
            p.results_mut().log(l);
        }
        let pl = self.plan.unwrap_or_else(|| plan(0.0));
        let known = pl.centre_mhz != 0.0;
        let n_type = |i: usize| c.per_type[i] as i64;
        p.put(&RemoteIdFields {
            ble_channel: pl.ble.filter(|_| known).map(|b| i64::from(b.0)),
            ble_offset_hz: pl.ble.map(|b| Hertz(b.1)),
            wifi_channel: known.then_some(i64::from(pl.wifi.0)),
            wifi_offset_hz: Hertz(pl.wifi.1),
            band: Band::at(pl.centre_mhz),
            drones: n as i64,
            messages: c.decoded as i64,
            ble_good: c.ble_good as i64,
            ble_remote_id: c.ble_rid as i64,
            wifi_good: c.wifi_good as i64,
            wifi_remote_id: c.wifi_rid as i64,
            frames_bad: bad as i64,
            basic_id_messages: n_type(0),
            location_messages: n_type(1),
            authentication_messages: n_type(2),
            self_id_messages: n_type(3),
            system_messages: n_type(4),
            operator_id_messages: n_type(5),
            snr_db: snr,
            offset_hz: cfo.map(Hertz),
            level_dbfs: level,
        });
        shaped::rows(&s.drones, &REMOTE_ID_DRONES, now, p);
        shaped::log_rows(&s.log, &REMOTE_ID_MESSAGES, now, p);
        publish_tracks(
            &s.drones,
            |d: &Drone| &d.trail,
            p,
            &mut self.key,
            |p, key, drone, position, time_s| {
                REMOTE_ID_TRACKS.put(p, key, &DroneTrackRow { drone, position, time_s });
            },
            &REMOTE_ID_TRACKS,
        );
        shaped::points(&s.drones, "drone/", p.results_mut());
        self.dirty = false;
    }
}

impl Component for DroneTable {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "drones.rid_table",
            vec![
                Port::typed::<RidItem>("ble"),
                Port::typed::<RidItem>("dsss"),
                Port::typed::<RidItem>("ofdm"),
                Port::iq("clock"),
            ],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(DroneTableSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[3];
        self.clock.restart(f.rate);
        self.plan = Some(plan(f.centre_hz));
        self.configured = true;
        self.publish();
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(3);
        for port in 0..3 {
            for it in io.items::<RidItem>(port) {
                match &it.value {
                    RidItem::Packet(p) => {
                        let t = self.clock.at(it.time);
                        self.take(t, p);
                    }
                    RidItem::Plain(t) => {
                        let c = &mut self.store.counters;
                        match t {
                            Transport::Ble => c.ble_good += 1,
                            Transport::WiFi => c.wifi_good += 1,
                        }
                        self.dirty = true;
                    }
                    RidItem::Stats(s) => self.take_stats(s),
                }
            }
        }
        self.clock.samples += n as u64;
        if io.tick() || self.dirty {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
        if !self.requests.is_empty() {
            let now = self.clock.now();
            io.items_out::<HostRequest>(1)
                .extend(self.requests.drain(..).map(|r| Timed {
                    time: now,
                    value: r,
                }));
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DroneTableSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DroneTableSetting::Band => {
                let b = BAND.read(id, v)?;
                // Only a change requests a retune from the host. The first value set does not.
                if self.band.is_some_and(|old| old != b) {
                    self.requests
                        .push(HostRequest::retune(b.centre_mhz() * 1e6, "Remote ID band"));
                }
                self.band = Some(b);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            DroneTableSetting::Band,
            Value::Choice(sdrkit_core::Choice::index(
                self.band.unwrap_or(BAND.default),
            )),
        )])
    }
    fn commands(&self) -> &Commands {
        DroneTableCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match DroneTableCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DroneTableCommand::Clear => self.clear(),
            DroneTableCommand::Remove => {
                commands::remove(
                    &mut self.store.drones,
                    self.drones_table,
                    &mut self.published,
                    args,
                    &DRONE_ROW,
                )?;
                self.dirty = true;
            }
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.clock.now())?;
        self.dirty = true;
        if self.configured {
            self.publish();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::messages::{decode_message, encode_message};
    use super::*;

    fn packet(i: u16, lon: f64) -> RidPacket {
        let loc = Message::Location(Location {
            status: 2,
            direction: Some(0.0),
            speed: Some(5.0),
            lat: Some(47.0),
            lon: Some(lon),
            alt_geo: Some(100.0),
            h_acc: 10,
            timestamp: Some(6000 + 10 * i),
            ..Location::default()
        });
        let payload = encode_message(&loc, 2).to_vec();
        RidPacket {
            transport: Transport::Ble,
            channel: 38,
            address: "C6:12:34:56:78:9A".into(),
            counter: i as u8,
            messages: vec![decode_message(&payload).unwrap()],
            payload,
            level_dbfs: None,
            cfo_hz: 0.0,
            snr_db: 20.0,
        }
    }

    fn spoof_lines(t: &DroneTable) -> Vec<&String> {
        t.lines
            .iter()
            .filter(|l| l.contains(" is spoofed"))
            .collect()
    }

    #[test]
    fn a_steady_drone_is_not_flagged() {
        let mut t = DroneTable::default();
        for i in 0..20 {
            t.take(
                f64::from(i),
                &packet(i, 8.0 + 5.0 * f64::from(i) / 75_800.0),
            );
        }
        assert!(t.lines.iter().all(|l| !l.contains(" is ")), "{:?}", t.lines);
    }

    #[test]
    fn a_teleporting_drone_is_logged_as_spoofed() {
        // Claims 5 m/s north while jumping 2 km east every second.
        let mut t = DroneTable::default();
        for i in 0..20 {
            t.take(
                f64::from(i),
                &packet(i, 8.0 + 2_000.0 * f64::from(i) / 75_800.0),
            );
        }
        let l = spoof_lines(&t);
        assert_eq!(l.len(), 1, "{:?}", t.lines);
        assert!(l[0].contains("teleport"), "{l:?}");
    }
}
