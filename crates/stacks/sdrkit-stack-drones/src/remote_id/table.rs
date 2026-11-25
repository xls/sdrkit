//! The drone table of Remote ID: drones by broadcast address, the message log, the counters, the published results, the events,
//! the commands, the saved state and the band request to the host.

use super::messages::{summary, Authentication, Location, Message, MsgType, System, Transport};
use super::phy::{Phy, PhyStats};
use super::trust::{Heard, Trust, TrustReasons, TrustVerdict};
use super::{plan, Band, Plan, RidItem, RidPacket, BAND, SHARED_RADIO};
use crate::family::{publish_tracks, wall, SignalClock, StatusLine};
use crate::typed::{
    DroneTrackRow, RemoteIdDroneEvent, RemoteIdDroneRow, RemoteIdFields, RemoteIdMessageRow,
    RemoteIdVerdictEvent, DRONE_REMOVE, DRONE_ROW, REMOTE_ID_DRONE, REMOTE_ID_DRONES,
    REMOTE_ID_MESSAGES, REMOTE_ID_TRACKS, REMOTE_ID_VERDICT,
};
use remoteid::TrustConfig;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Clock, ConfigError, Event, HostRequest, LockState, Record, Results, ResultsDecl, SettingError,
    SettingId, SettingSet, StateBlob, StateError, TableId, Value, Values,
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
    /// Authentication data length in bytes, from a page 0 that passed its checks.
    #[serde(default)]
    pub auth_length: Option<u8>,
    /// The pages heard of the present authentication, one bit for each page.
    #[serde(default)]
    pub auth_have: u16,
    /// The type and the timestamp of the page 0 of the present authentication.
    #[serde(default)]
    pub auth_stamp: Option<(u8, u32)>,
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
    /// The trust check of the broadcasts heard since the stack started. Not saved: a verdict without its evidence would
    /// change on the next packet.
    #[serde(skip)]
    trust: Option<Trust>,
    /// The carrier offset of the packets of each carrier (BLE, Wi-Fi), for the shared-radio check.
    #[serde(skip)]
    cfo: [Offset; 2],
    /// Signal time of the last shared-radio evidence.
    #[serde(skip)]
    shared_t: Option<f64>,
}

/// A mean of the carrier offsets of one drone's packets on one carrier: a running mean over the last few packets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Offset {
    mean_hz: f64,
    n: u32,
    /// Signal time of the last packet.
    t: f64,
}

/// Packets before a mean of carrier offsets takes part in the shared-radio check.
const OFFSET_MIN: u32 = 4;
/// The weight of a new packet in the mean.
const OFFSET_ALPHA: f64 = 0.125;
/// Two means at most this far apart are one radio, Hz: above 3 standard deviations of the mean at 10 dB SNR (one BLE packet
/// measures its offset to 1.3 kHz at 10 dB, 0.36 kHz at 20 dB).
const SHARED_HZ: f64 = 1000.0;
/// Both drones heard within this time, seconds.
const SHARED_WITHIN_S: f64 = 2.0;
/// The least time between two pieces of shared-radio evidence for one drone, seconds.
const SHARED_EVERY_S: f64 = 0.25;

impl Offset {
    fn push(&mut self, hz: f64, t: f64) {
        self.mean_hz = if self.n == 0 {
            hz
        } else {
            self.mean_hz + OFFSET_ALPHA * (hz - self.mean_hz)
        };
        self.n = self.n.saturating_add(1);
        self.t = t;
    }
    /// True when `self` and `o` are both settled, heard within [`SHARED_WITHIN_S`] of `t`, and within [`SHARED_HZ`].
    fn shared(&self, o: &Offset, t: f64) -> bool {
        self.n >= OFFSET_MIN
            && o.n >= OFFSET_MIN
            && t - self.t <= SHARED_WITHIN_S
            && t - o.t <= SHARED_WITHIN_S
            && (self.mean_hz - o.mean_hz).abs() <= SHARED_HZ
    }
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
            auth_length: None,
            auth_have: 0,
            auth_stamp: None,
            self_id: String::new(),
            system: None,
            operator_id: String::new(),
            ble_channel: 0,
            wifi_channel: 0,
            event_t: None,
            trust: None,
            cfo: [Offset::default(); 2],
            shared_t: None,
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
    /// Whether every page of the present authentication was heard, once its page 0 was.
    pub fn auth_complete(&self) -> Option<bool> {
        let n = self.auth_pages?;
        Some(self.auth_have == ((1u32 << n) - 1) as u16)
    }
    /// Takes one authentication page. Returns false for a page that fails its checks: a page 0 whose length needs more
    /// pages than it states (17 bytes on page 0, 23 on each other page), or a page after the last one.
    fn take_auth(&mut self, a: &Authentication) -> bool {
        self.auth_type = Some(a.auth_type);
        if a.page == 0 {
            let last = a.last_page.unwrap_or(0);
            let length = a.length.unwrap_or(0);
            if usize::from(length) > 17 + 23 * usize::from(last) {
                return false;
            }
            let stamp = (a.auth_type, a.timestamp.unwrap_or(0));
            if self.auth_stamp != Some(stamp) {
                // A new authentication: the pages heard belong to the one before.
                self.auth_stamp = Some(stamp);
                self.auth_have = 0;
            }
            self.auth_pages = Some(last + 1);
            self.auth_length = Some(length);
            self.auth_have |= 1;
            return true;
        }
        if self.auth_pages.is_some_and(|n| a.page >= n) {
            return false;
        }
        self.auth_have |= 1 << a.page;
        true
    }
    /// The trust verdict, once a packet was judged since the stack started.
    pub fn verdict(&self) -> Option<TrustVerdict> {
        self.trust.as_ref().map(Trust::verdict)
    }
    /// The active reasons of the verdict, empty when there is none.
    pub fn reasons(&self) -> TrustReasons {
        self.trust.as_ref().map_or(TrustReasons::empty(), Trust::reasons)
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
            verdict: self.verdict(),
            reasons: self.reasons(),
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
            authentication_length: code(self.auth_length),
            authentication_complete: self.auth_complete(),
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
    /// Messages refused: members of a pack that are packs or of a reserved type, authentication pages that fail their checks.
    #[serde(default)]
    pub refused: u64,
    /// Messages with a value outside its range.
    #[serde(default)]
    pub out_of_range: u64,
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
        /// Flag broadcast addresses that share one radio.
        SharedRadio = SHARED_RADIO,
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
    /// The samples are live: the host clock is a UTC clock for the trust check.
    utc: bool,
    trust_cfg: TrustConfig,
    /// The clock of the trust check: signal time in ms, never going back.
    trust_ms: u64,
    shared_radio: bool,
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
            utc: false,
            trust_cfg: TrustConfig::default(),
            trust_ms: 0,
            shared_radio: false,
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
        s.counters.refused += u64::from(p.refused);
        s.counters.out_of_range += u64::from(p.out_of_range);
        self.last_rid_t = Some(t);
        self.snr = Some(p.snr_db);
        self.cfo = Some(p.cfo_hz);
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
                    if !d.take_auth(a) {
                        s.counters.refused += 1;
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
        let freq = carrier_hz(plan, p.transport);
        self.trust_ms = self.trust_ms.max((t.max(0.0) * 1e3) as u64);
        let heard = Heard {
            link: p.link,
            counter: p.counter,
            payload: &p.payload,
            now_ms: self.trust_ms,
            unix_ms: self.utc.then(|| (self.host_clock.now() * 1e3) as u64),
            beacon: p.beacon,
        };
        let ci = match p.transport {
            Transport::Ble => 0,
            Transport::WiFi => 1,
        };
        d.cfo[ci].push(p.cfo_hz, t);
        let offset = d.cfo[ci];
        let trust = d.trust.get_or_insert_with(Trust::new);
        if trust.observe(&self.trust_cfg, &heard) {
            self.events.push(verdict_event(d, t, freq));
            let v = d.verdict().unwrap_or(TrustVerdict::Plausible);
            if v.flagged() {
                self.lines.push(format!(
                    "Remote ID: drone {} is {} ({})",
                    d.address,
                    v.name(),
                    d.reasons()
                ));
            }
        }
        if let Some(at) = d.position() {
            if d.event_t.is_none_or(|e| t - e >= EVENT_EVERY_S) {
                d.event_t = Some(t);
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
        if self.shared_radio {
            self.check_shared(&p.address, ci, offset, t, freq);
        }
        self.dirty = true;
    }

    /// The shared-radio check after a packet of `address` on carrier `ci` (0 BLE, 1 Wi-Fi) whose mean offset is now `mine`:
    /// when another drone heard on the same carrier keeps the same offset, both get shared-radio evidence in the trust check,
    /// at most every [`SHARED_EVERY_S`] for each drone. Allocates nothing unless a verdict changes.
    fn check_shared(&mut self, address: &str, ci: usize, mine: Offset, t: f64, freq: f64) {
        let drones = &mut self.store.drones;
        let others = drones
            .iter()
            .any(|r| r.key.as_str() != address && r.record.cfo[ci].shared(&mine, t));
        if !others {
            return;
        }
        for (key, d) in drones.records_mut() {
            let me = key.as_str() == address;
            if !(me || d.cfo[ci].shared(&mine, t))
                || d.shared_t.is_some_and(|x| t - x < SHARED_EVERY_S)
            {
                continue;
            }
            d.shared_t = Some(t);
            let Some(trust) = d.trust.as_mut() else {
                continue;
            };
            if trust.note(
                &self.trust_cfg,
                remoteid::Reason::SharedRadio,
                self.trust_ms,
            ) {
                self.events.push(verdict_event(d, t, freq));
                let v = d.verdict().unwrap_or(TrustVerdict::Plausible);
                if v.flagged() {
                    self.lines.push(format!(
                        "Remote ID: drone {} is {} ({})",
                        d.address,
                        v.name(),
                        d.reasons()
                    ));
                }
            }
        }
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
        let flagged = s
            .drones
            .iter()
            .filter(|r| r.record.verdict().is_some_and(TrustVerdict::flagged))
            .count();
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
        } else if flagged > 0 {
            let plural = if n == 1 { "" } else { "s" };
            self.status.write(
                p,
                lock_state,
                snr,
                level,
                cfo,
                format_args!(
                    "{n} drone{plural}, {flagged} flagged, {} messages",
                    c.decoded
                ),
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
            flagged: flagged as i64,
            messages: c.decoded as i64,
            messages_refused: c.refused as i64,
            fields_out_of_range: c.out_of_range as i64,
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
                REMOTE_ID_TRACKS.put(
                    p,
                    key,
                    &DroneTrackRow {
                        drone,
                        position,
                        time_s,
                    },
                );
            },
            &REMOTE_ID_TRACKS,
        );
        shaped::row_points(&s.drones, &REMOTE_ID_DRONES, "drone/", p.results_mut());
        self.dirty = false;
    }
}

/// The carrier of a packet of `transport` under `plan`, Hz (0 when unknown).
fn carrier_hz(plan: Option<Plan>, transport: Transport) -> f64 {
    match (plan, transport) {
        (Some(pl), _) if pl.centre_mhz == 0.0 => 0.0,
        (Some(pl), Transport::Ble) => pl.ble.map_or(0.0, |b| pl.centre_mhz * 1e6 + b.1),
        (Some(pl), Transport::WiFi) => pl.centre_mhz * 1e6 + pl.wifi.1,
        (None, _) => 0.0,
    }
}

/// The event of a change of the trust verdict of `d` at signal time `t` on the carrier `freq`.
fn verdict_event(d: &Drone, t: f64, freq: f64) -> Event {
    let v = d.verdict().unwrap_or(TrustVerdict::Plausible);
    REMOTE_ID_VERDICT.event(
        t,
        freq,
        &RemoteIdVerdictEvent {
            source: d.display_id(),
            address: &d.address,
            verdict: v,
            reasons: d.reasons(),
            position: pos(d.position()),
            urgency: v.urgency(),
            family: Family::Aircraft,
        },
    )
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
        self.live = ctx.wall_clock();
        self.utc = ctx.live;
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
            DroneTableSetting::SharedRadio => self.shared_radio = SHARED_RADIO.read(id, v)?,
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
        values_of([
            (
                DroneTableSetting::Band,
                Value::Choice(sdrkit_core::Choice::index(
                    self.band.unwrap_or(BAND.default),
                )),
            ),
            (
                DroneTableSetting::SharedRadio,
                Value::Bool(self.shared_radio),
            ),
        ])
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
    use super::super::messages::{decode_message, encode_message, Link, Payload};
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
        let payload = encode_message(&loc, 2);
        RidPacket {
            transport: Transport::Ble,
            channel: 38,
            address: "C6:12:34:56:78:9A".into(),
            counter: i as u8,
            messages: vec![decode_message(&payload).unwrap()],
            link: Link::Ble4,
            payload: Payload::new(&payload),
            beacon: None,
            refused: 0,
            out_of_range: 0,
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

    const ADDR: &str = "C6:12:34:56:78:9A";

    /// Packet `i` of a drone that jumps 2 km east at each packet, one a second.
    fn teleport(i: u16) -> RidPacket {
        packet(i, 8.0 + 2_000.0 * f64::from(i) / 75_800.0)
    }

    fn verdict_events(t: &DroneTable) -> Vec<&Event> {
        t.events
            .iter()
            .filter(|e| e.is(&REMOTE_ID_VERDICT))
            .collect()
    }

    fn spoofed(t: &mut DroneTable, from: u16, n: u16) {
        for i in from..from + n {
            t.take(f64::from(i), &teleport(i));
        }
    }

    #[test]
    fn a_spoofed_drone_shows_its_verdict_in_the_row_the_event_and_the_count() {
        let mut t = DroneTable::default();
        spoofed(&mut t, 0, 20);
        let d = t.store().drones.get(ADDR).unwrap();
        assert_eq!(d.verdict(), Some(TrustVerdict::Spoofed));
        assert!(d.reasons().contains(TrustReasons::TELEPORT), "{}", d.reasons());
        let e = verdict_events(&t);
        assert_eq!(e.len(), 1, "{:?}", t.events);
        assert_eq!(
            e[0].at(&RemoteIdVerdictEvent::VERDICT),
            Some(TrustVerdict::Spoofed)
        );
        assert_eq!(e[0].at(&RemoteIdVerdictEvent::ADDRESS), Some(ADDR));
        assert_eq!(
            e[0].get(&sdrkit_core::publish::keys::URGENCY),
            Some(Urgency::Notice)
        );
        t.publish();
        let f = t.published.results().fields();
        assert_eq!(
            (
                f.at(&RemoteIdFields::DRONES),
                f.at(&RemoteIdFields::FLAGGED)
            ),
            (Some(1), Some(1))
        );
        assert!(t.published.results().status().summary.contains("1 flagged"));
    }

    #[test]
    fn a_steady_drone_has_a_plausible_verdict_and_no_event() {
        let mut t = DroneTable::default();
        for i in 0..20 {
            t.take(
                f64::from(i),
                &packet(i, 8.0 + 5.0 * f64::from(i) / 75_800.0),
            );
        }
        let d = t.store().drones.get(ADDR).unwrap();
        // The packets carry no Basic ID: one weak reason, which leaves the verdict plausible.
        assert_eq!(
            (d.verdict(), d.reasons()),
            (Some(TrustVerdict::Plausible), TrustReasons::NO_BASIC_ID)
        );
        assert!(verdict_events(&t).is_empty());
        t.publish();
        let f = t.published.results().fields();
        assert_eq!(f.at(&RemoteIdFields::FLAGGED), Some(0));
    }

    #[test]
    fn the_evidence_goes_with_its_drone() {
        // Remove, clear and the eviction of the oldest drone each drop the trust state with the row.
        let mut t = DroneTable::default();
        spoofed(&mut t, 0, 20);
        assert!(t.store.drones.remove(ADDR).is_some());
        spoofed(&mut t, 20, 2);
        assert_eq!(
            t.store().drones.get(ADDR).unwrap().verdict(),
            Some(TrustVerdict::Plausible)
        );
        spoofed(&mut t, 22, 20);
        t.clear();
        spoofed(&mut t, 42, 2);
        assert_eq!(
            t.store().drones.get(ADDR).unwrap().verdict(),
            Some(TrustVerdict::Plausible)
        );
        spoofed(&mut t, 44, 20);
        for k in 0..DRONE_CAP {
            let mut p = packet(0, 8.0);
            p.address = format!("C6:00:00:00:{:02X}:{:02X}", k / 256, k % 256);
            t.take(100.0, &p);
        }
        assert!(t.store().drones.get(ADDR).is_none());
        assert_eq!(t.store().drones.len(), DRONE_CAP);
    }

    #[test]
    fn the_utc_clock_is_used_only_for_live_samples() {
        // The Location timestamps (10 minutes past the hour) are far from the clock (30 minutes past).
        let clock = 1_795_000_000.0 - 1_795_000_000.0 % 3600.0 + 1800.0;
        for (utc, flagged) in [(false, false), (true, true)] {
            let mut t = DroneTable {
                utc,
                host_clock: Clock::fixed(clock),
                ..DroneTable::default()
            };
            for i in 0..20 {
                t.take(f64::from(i), &packet(i, 8.0));
            }
            let d = t.store().drones.get(ADDR).unwrap();
            assert_eq!(d.verdict().unwrap().flagged(), flagged, "{}", d.reasons());
            assert_eq!(
                d.reasons().contains(TrustReasons::LOCATION_CLOCK_DRIFT),
                flagged
            );
        }
    }

    #[test]
    fn each_link_is_its_own_transport() {
        use super::super::phy::ble_link;
        use super::super::trust::transport;
        use sdrkit_proto_wlan_phy::ble::PduType;
        assert_eq!(ble_link(PduType::AdvNonconnInd), Link::Ble4);
        assert_eq!(ble_link(PduType::AdvInd), Link::Ble4);
        assert_eq!(ble_link(PduType::AdvExtInd), Link::Ble5);
        assert_eq!(transport(Link::Ble4), remoteid::Transport::Bluetooth4);
        assert_eq!(transport(Link::Ble5), remoteid::Transport::Bluetooth5);
        assert_eq!(transport(Link::WifiBeacon), remoteid::Transport::WifiBeacon);
    }

    #[test]
    fn a_restored_drone_has_no_verdict_until_heard_again() {
        let mut t = DroneTable::default();
        spoofed(&mut t, 0, 20);
        let blob = t.save_state().unwrap();
        let mut u = DroneTable::default();
        u.restore_state(&blob).unwrap();
        let d = u.store().drones.get(ADDR).unwrap();
        assert_eq!((d.verdict(), d.reasons()), (None, TrustReasons::empty()));
        u.take(0.0, &packet(0, 8.0));
        assert_eq!(
            u.store().drones.get(ADDR).unwrap().verdict(),
            Some(TrustVerdict::Plausible)
        );
        assert!(verdict_events(&u).is_empty());
    }

    #[test]
    fn the_verdict_stays_over_a_change_of_frequency() {
        let mut t = DroneTable::default();
        let f = Format::iq(20e6, 2432e6, 2432e6);
        t.configure(&[f.clone(), f.clone(), f.clone(), f]).unwrap();
        t.clock.samples = 20 * 20_000_000;
        spoofed(&mut t, 0, 20);
        let before = t.trust_ms;
        let g = Format::iq(20e6, 2412e6, 2412e6);
        t.configure(&[g.clone(), g.clone(), g.clone(), g]).unwrap();
        // Item times restart at 0 after a configure; the signal clock does not.
        let at = t.clock.at(0.5);
        t.take(at, &teleport(20));
        let d = t.store().drones.get(ADDR).unwrap();
        assert_eq!(d.verdict(), Some(TrustVerdict::Spoofed));
        assert!(t.trust_ms >= before);
    }

    fn cfo_packet(addr: &str, i: u16, cfo_hz: f64) -> RidPacket {
        let mut p = packet(i, 8.0);
        p.address = addr.into();
        p.cfo_hz = cfo_hz;
        p
    }

    /// Three drones heard for 2 s, one packet each every 50 ms, at these carrier offsets with a spread of 300 Hz.
    fn three_drones(shared: bool, offsets: [f64; 3]) -> DroneTable {
        let mut t = DroneTable {
            shared_radio: shared,
            ..DroneTable::default()
        };
        let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
        for i in 0..40u16 {
            for (k, o) in offsets.iter().enumerate() {
                let tt = f64::from(i) * 0.05 + k as f64 * 0.01;
                t.take(
                    tt,
                    &cfo_packet(&format!("C6:00:00:00:00:0{k}"), i, o + 300.0 * rng.normal()),
                );
            }
        }
        t
    }

    fn reasons_of(t: &DroneTable) -> Vec<TrustReasons> {
        t.store()
            .drones
            .iter()
            .map(|r| r.record.reasons())
            .collect()
    }

    #[test]
    fn drones_that_keep_one_carrier_offset_share_a_radio() {
        let one = three_drones(true, [12e3; 3]);
        assert!(
            reasons_of(&one)
                .iter()
                .all(|r| r.contains(TrustReasons::SHARED_RADIO)),
            "{:?}",
            reasons_of(&one)
        );
        let real = three_drones(true, [-40e3, 10e3, 55e3]);
        assert!(
            reasons_of(&real)
                .iter()
                .all(|r| !r.contains(TrustReasons::SHARED_RADIO)),
            "{:?}",
            reasons_of(&real)
        );
        let off = three_drones(false, [12e3; 3]);
        assert!(reasons_of(&off)
            .iter()
            .all(|r| !r.contains(TrustReasons::SHARED_RADIO)));
    }

    #[test]
    fn authentication_pages_are_checked_and_counted() {
        let mut t = DroneTable::default();
        let auth = |page: u8, last: Option<u8>, length: Option<u8>| {
            let m = Message::Authentication(Authentication {
                auth_type: 1,
                page,
                last_page: last,
                length,
                timestamp: Some(245_000_000),
                data: vec![0; 17],
            });
            let b = encode_message(&m, 2);
            let mut p = packet(0, 8.0);
            p.messages = vec![decode_message(&b).unwrap()];
            p.payload = Payload::new(&b);
            p
        };
        t.take(0.0, &auth(0, Some(2), Some(60)));
        t.take(0.1, &auth(1, None, None));
        let d = t.store().drones.get(ADDR).unwrap();
        assert_eq!(
            (d.auth_pages, d.auth_length, d.auth_complete()),
            (Some(3), Some(60), Some(false))
        );
        t.take(0.2, &auth(2, None, None));
        assert_eq!(
            t.store().drones.get(ADDR).unwrap().auth_complete(),
            Some(true)
        );
        // A page after the last one, and a page 0 whose length needs more pages than it states.
        t.take(0.3, &auth(3, None, None));
        t.take(0.4, &auth(0, Some(1), Some(41)));
        assert_eq!(t.store().counters.refused, 2);
        assert_eq!(t.store().drones.get(ADDR).unwrap().auth_length, Some(60));
    }

    #[test]
    fn the_spoofer_of_the_test_signal_teleports() {
        use super::super::generator::{spoofer_message, SPOOFER_EVERY_S};
        let mut t = DroneTable::default();
        let mut times = Vec::new();
        for j in 0..10u64 {
            let b = encode_message(&spoofer_message(j), 2);
            let mut p = packet(0, 8.0);
            p.counter = j as u8;
            p.messages = vec![decode_message(&b).unwrap()];
            p.payload = Payload::new(&b);
            let at = 0.002 + SPOOFER_EVERY_S * j as f64;
            t.take(at, &p);
            let d = t.store().drones.get(ADDR).unwrap();
            times.push((j, d.verdict(), d.reasons()));
        }
        assert_eq!(times[7].1, Some(TrustVerdict::Spoofed), "{times:?}");
    }
}
