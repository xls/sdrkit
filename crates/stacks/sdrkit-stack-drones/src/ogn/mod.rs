//! OGN tracker (`ogn`): the position packets of Open Glider Network trackers on the channels of one capture, with the
//! aircraft table and the map points. No public document gives the protocol.

pub mod channels;
pub mod generator;
pub mod proto;

use crate::family::{
    min_rate_message, publish_tracks, wall, SignalClock, StatusLine, REPORT_S,
};
use channels::{ChannelState, OgnChannels, OgnItem, OgnPacket, CHANNELS, CHANNEL_STEP_HZ};
use proto::{Content, Header, PositionReport, PACKET};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use crate::typed::{
    AircraftTrackRow, OgnAircraftRow, OgnEmergencyEvent, OgnFields, OgnPacketRow, OgnTrackerEvent,
    AIRCRAFT_REMOVE, AIRCRAFT_ROW, OGN_AIRCRAFT, OGN_EMERGENCY, OGN_PACKETS, OGN_TRACKER,
    OGN_TRACKS,
};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Clock, ConfigError, Event, LockState, Record, Results, ResultsDecl, Setting,
    SettingError, SettingId, SettingSet, StackConfig, StateBlob, StateError, TableId, Value,
    Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Kind, Port, RateFrom, RunContext, StackGraph,
};
use sdrkit_util_geo::{Position, KNOT_M_S};
use sdrkit_util_objects::commands::{self, Tables};
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Spacing, Stamp, Trail,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::sync::Arc;

/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 1e6;
/// Packets kept in the log.
pub const PACKET_CAP: usize = 500;
pub(crate) const TRAIL_CAP: usize = 200;
pub(crate) const AIRCRAFT_CAP: usize = 256;
/// A packet with the bytes of the last accepted one that ends within this time of it is the same packet, seconds.
pub const SAME_PACKET_S: f64 = 4e-3;
/// The least time between two emergency events of one tracker, seconds.
pub const EMERGENCY_HOLD_S: f64 = 60.0;
/// What the stack says about its sources.
pub const NOT_CONFIRMED: &str =
    "experimental: not confirmed on a real signal. Sync word, code table, whitening and field \
                                 layout are not confirmed by a public document.";

/// The silence after which a tracker leaves the table.
pub const EXPIRY_S: Setting<field::F64> =
    Setting::new("expiry_s", "Forget trackers after", "s", 600.0).range(30.0, 86400.0, 1.0);
/// The position that the trackers of the test signal circle.
pub const GEN_CENTRE: Setting<field::Position> =
    Setting::new("gen_centre", "Test signal centre", "", (54.70, 25.30))
        .section("Test signal")
        .advanced();
/// The trackers of the test signal.
pub const GEN_TRACKERS: Setting<field::I64> =
    Setting::new("gen_trackers", "Test signal trackers", "", 4)
        .range(1.0, 4.0, 1.0)
        .section("Test signal")
        .advanced();
/// The wrong bits in every third packet of the test signal.
pub const GEN_BAD_BITS: Setting<field::I64> =
    Setting::new("gen_bad_bits", "Wrong bits in every third packet", "", 0)
        .range(0.0, 4.0, 1.0)
        .section("Test signal")
        .advanced();
/// One tracker of the test signal sets the emergency flag.
pub const GEN_EMERGENCY: Setting<field::Bool> =
    Setting::new("gen_emergency", "Test signal with an emergency", "", false)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `ogn` stack.
    pub enum OgnSetting {
        /// Channels.
        Channels = CHANNELS,
        /// Channel spacing.
        ChannelStepHz = CHANNEL_STEP_HZ,
        /// Forget trackers after.
        ExpiryS = EXPIRY_S,
        /// Test signal centre.
        GenCentre = GEN_CENTRE,
        /// Test signal trackers.
        GenTrackers = GEN_TRACKERS,
        /// Wrong bits in every third packet.
        GenBadBits = GEN_BAD_BITS,
        /// Test signal with an emergency.
        GenEmergency = GEN_EMERGENCY,
    }
}

sdrkit_core::settings! {
    /// The settings of [`OgnTracker`].
    pub enum OgnTrackerSetting {
        /// Forget trackers after.
        ExpiryS = EXPIRY_S,
    }
}


sdrkit_core::commands! {
    /// The commands of [`OgnTracker`].
    pub enum OgnTrackerCommand {
        /// Forgets the tables and the counters.
        Clear = commands::CLEAR,
        /// Forgets one tracker.
        Remove = AIRCRAFT_REMOVE.command,
    }
}

/// The `ogn` receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among
/// them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("channels", OgnChannels::default())
        .add("tracker", OgnTracker::default())
        .input("channels")
        .input("tracker.clock")
        .connect("channels", "tracker.items")
        .output("tracker.events")
        .clock(REPORT_S)
        .status_from("tracker")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape("ogn"), graph)
        .min_rate(MIN_RATE_HZ, &min_rate_message("OGN tracker", MIN_RATE_HZ))
        .route(CHANNELS.key(), &["channels.channels"])?
        .route(CHANNEL_STEP_HZ.key(), &["channels.channel_step_hz"])?
        .route(EXPIRY_S.key(), &["tracker.expiry_s"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("tracker", "remove"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// The key of a tracker: its address type and its address. The same 24 bits under two types are two trackers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TrackerKey {
    /// The address type, 0 to 3.
    pub address_type: u8,
    /// The 24-bit address.
    pub address: u32,
}

impl std::fmt::Display for TrackerKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{:06X}", self.address_type, self.address)
    }
}

sdrkit_core::choice_enum! {
    /// What a packet is.
    #[derive(Serialize, Deserialize)]
    pub enum PacketKind {
        /// A position.
        Position = ("position", "Position"),
        /// A status or information packet.
        Status = ("status", "Status"),
        /// A position that is encrypted with a key of the owner.
        Encrypted = ("encrypted", "Encrypted"),
    }
}

/// One tracker of the table.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Aircraft {
    /// The address as 6 hex digits.
    pub id: String,
    /// The address type, 0 to 3.
    pub address_type: u8,
    /// The last position report.
    pub report: Option<ReportRow>,
    /// The count of relays of the last packet.
    pub relay: u8,
    /// The last packet had the emergency flag.
    pub emergency: bool,
    /// The level of the last packet over the noise, dB.
    pub level_db: f64,
    /// The frequency of the last packet, Hz.
    pub channel_hz: f64,
    /// The track of positions.
    pub trail: Trail,
    /// The event of the new tracker was sent.
    #[serde(skip)]
    announced: bool,
    /// The signal time of the last emergency event.
    #[serde(skip)]
    alarm_at: Option<f64>,
}

/// The fields of a position report, as the table keeps them.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReportRow {
    /// The position.
    pub position: Position,
    /// The altitude, metres.
    pub alt_m: f64,
    /// The speed, m/s.
    pub speed_m_s: f64,
    /// The heading, degrees.
    pub heading_deg: f64,
    /// The climb rate, m/s.
    pub climb_m_s: f64,
    /// The turn rate, degrees a second.
    pub turn_deg_s: f64,
    /// The aircraft type, 0 to 15.
    pub aircraft_type: u8,
    /// The sender does not want to be shown by other trackers.
    pub stealth: bool,
    /// The fix quality, 0 to 3.
    pub fix_quality: u8,
    /// The second of the minute.
    pub second: u8,
}

impl Aircraft {
    fn new(key: TrackerKey) -> Aircraft {
        Aircraft {
            id: format!("{:06X}", key.address),
            address_type: key.address_type,
            report: None,
            relay: 0,
            emergency: false,
            level_db: 0.0,
            channel_hz: 0.0,
            trail: Trail::new(
                TRAIL_CAP,
                Spacing {
                    min_s: 2.0,
                    min_m: 25.0,
                },
            ),
            announced: false,
            alarm_at: None,
        }
    }
    /// The sender asks other trackers not to show it, or its address is a random one. It is shown here and is not for a
    /// public map.
    pub fn private(&self) -> bool {
        self.address_type == 0 || self.report.is_some_and(|r| r.stealth)
    }
    /// Urgent while the emergency flag is set.
    pub fn urgency(&self) -> Urgency {
        if self.emergency {
            Urgency::Emergency
        } else {
            Urgency::None
        }
    }
}

impl Shift for Aircraft {
    fn shift_times(&mut self, by_s: f64) {
        self.trail.shift_times(by_s);
    }
}

/// What `ogn` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("ogn").results_decl()
}

impl Shaped for Aircraft {
    type Row = OgnAircraftRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let r = self.report.as_ref();
        out.put(&OgnAircraftRow {
            source: &self.id,
            address_type: proto::ADDRESS_TYPES[usize::from(self.address_type & 3)],
            aircraft_type: r.map(|r| proto::AIRCRAFT_TYPES[usize::from(r.aircraft_type & 15)]),
            position: r.map(|r| (r.position.lat_deg, r.position.lon_deg)),
            alt_m: r.map(|r| r.alt_m),
            speed_kt: r.map(|r| r.speed_m_s / KNOT_M_S),
            course_deg: r.map(|r| r.heading_deg),
            climb_m_s: r.map(|r| r.climb_m_s),
            turn_deg_s: r.map(|r| r.turn_deg_s),
            relay: i64::from(self.relay),
            stealth: r.map(|r| r.stealth),
            fix_quality: r.map(|r| i64::from(r.fix_quality)),
            second: r.map(|r| i64::from(r.second)),
            level_db: self.level_db,
            channel_hz: Hertz(self.channel_hz),
            private: self.private(),
            urgency: self.urgency(),
            heard: columns::heard(seen),
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
    fn point(&self) -> Option<Point<'_>> {
        let r = self.report.as_ref()?;
        Some(
            Point::new(r.position, format!("{} {:.0} m", self.id, r.alt_m))
                .heading(r.heading_deg as f32),
        )
    }
}

/// One row of the packet log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PacketRow {
    /// Signal time at the end of the packet, seconds.
    pub time: f64,
    /// What the packet is.
    pub kind: PacketKind,
    /// The address and the name of its type.
    pub id: String,
    /// The packet in one line.
    pub decoded: String,
    /// Latitude and longitude, degrees.
    pub position: Option<(f64, f64)>,
    /// The channel, Hz.
    pub channel_hz: f64,
    /// The repair changed a bit.
    pub repaired: bool,
    /// The 26 bytes after the repair.
    pub bytes: Vec<u8>,
}

impl Shift for PacketRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for PacketRow {
    type Row = OgnPacketRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let mut hex = String::with_capacity(3 * self.bytes.len());
        for (i, b) in self.bytes.iter().enumerate() {
            let _ = write!(hex, "{}{b:02X}", if i > 0 { " " } else { "" });
        }
        out.put(&OgnPacketRow {
            time_s: self.time,
            kind: self.kind,
            id: &self.id,
            decoded: &self.decoded,
            position: self.position,
            channel_hz: Hertz(self.channel_hz),
            repaired: self.repaired,
            hex: &hex,
        });
    }
}

/// The counters of the stack.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Counters {
    /// Good packets.
    pub good: u64,
    /// Packets that needed a repair.
    pub corrected: u64,
    /// Packets with an encrypted position.
    pub encrypted: u64,
}

/// What the tracker keeps across sessions: the packet log, the trackers with their trails, and the counters.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The packet log, oldest first.
    pub packets: LogRing<PacketRow>,
    /// The trackers.
    pub aircraft: ObjectTable<TrackerKey, Aircraft>,
    /// The counters.
    pub counters: Counters,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            packets: LogRing::new(PACKET_CAP),
            aircraft: ObjectTable::new(AIRCRAFT_CAP),
            counters: Counters::default(),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.packets.shift_times(by_s);
        self.aircraft.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("drones.ogn", 1);
    fn clear(&mut self) {
        self.packets.clear();
        self.aircraft.clear();
        self.counters = Counters::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.packets.adopt(saved.packets);
        self.aircraft.adopt(saved.aircraft);
        self.counters = saved.counters;
    }
}

/// A frequency in MHz with the digits that it needs.
fn mhz(hz: f64) -> String {
    let s = format!("{:.4}", hz / 1e6);
    let s = s.trim_end_matches('0');
    if s.ends_with('.') {
        format!("{s}0")
    } else {
        s.to_string()
    }
}

/// The line of a position packet.
pub fn position_line(head: &Header, r: &PositionReport, channel_hz: f64) -> String {
    format!(
        "{}, {:.0} m, {:.1} m/s, heading {:.0}, climb {:.1} m/s, relay {}, {} MHz",
        r.aircraft(),
        r.alt_m,
        r.speed_m_s,
        r.heading_deg,
        r.climb_m_s,
        head.relay,
        mhz(channel_hz)
    )
}

/// The line of a packet heard on `channel_hz`.
pub fn packet_line(head: &Header, content: &Content, channel_hz: f64) -> String {
    match content {
        Content::Position(r) => position_line(head, r, channel_hz),
        Content::Status => format!("{} MHz, status or information packet", mhz(channel_hz)),
        Content::Encrypted => format!("{} MHz, position encrypted, not read", mhz(channel_hz)),
    }
}

/// The tracker component (`ogn.tracker`): drops the second copy of a packet, reads the header and the fields, and keeps the
/// packet log and the aircraft table.
pub struct OgnTracker {
    published: Published,
    aircraft_table: TableId,
    status: StatusLine,
    store: Store,
    dirty: bool,
    clock: SignalClock,
    live: bool,
    host_clock: Clock,
    expiry_s: f64,
    channels: Vec<ChannelState>,
    syncs_seen: u64,
    bad_seen: u64,
    signal: Option<f64>,
    last: Option<(f64, [u8; PACKET], f64)>,
    last_good: f64,
    last_signal: f64,
    events: Vec<Event>,
    key: String,
    configured: bool,
}

impl Default for OgnTracker {
    fn default() -> Self {
        let decl = Arc::new(declaration());
        let aircraft_table = decl
            .table_id(OGN_AIRCRAFT.key)
            .expect("the declaration holds the aircraft table");
        OgnTracker {
            published: Published::from_decl(decl),
            aircraft_table,
            status: StatusLine::default(),
            store: Store::default(),
            dirty: true,
            clock: SignalClock::default(),
            live: false,
            host_clock: Clock::fixed(0.0),
            expiry_s: EXPIRY_S.default,
            channels: Vec::new(),
            syncs_seen: 0,
            bad_seen: 0,
            signal: None,
            last: None,
            last_good: -1e9,
            last_signal: -1e9,
            events: Vec::new(),
            key: String::new(),
            configured: false,
        }
    }
}

impl OgnTracker {
    /// The tables that the tracker keeps.
    pub fn store(&self) -> &Store {
        &self.store
    }

    fn stamp(&self, t: f64) -> Stamp {
        match wall(self.live, &self.host_clock) {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        }
    }

    /// Takes one packet with good checks that ended at signal time `t`. Returns false for the second copy of the last packet
    /// and for a packet whose header parity is wrong.
    pub fn take_packet(&mut self, t: f64, p: &OgnPacket) -> bool {
        if let Some((at, bytes, power)) = self.last {
            if bytes == p.bytes && (t - at).abs() < SAME_PACKET_S {
                // A strong packet is also read on the channel beside its own: the copy with more power names the channel.
                if p.power_dbfs > power {
                    self.last = Some((at, bytes, p.power_dbfs));
                    self.move_last(p);
                }
                return false;
            }
        }
        let Some((head, content)) = proto::parse(&p.bytes) else {
            return false;
        };
        self.last = Some((t, p.bytes, p.power_dbfs));
        self.last_good = t;
        self.last_signal = t;
        self.dirty = true;
        let c = &mut self.store.counters;
        c.good += 1;
        c.corrected += u64::from(p.repaired > 0);
        self.signal = Some(match self.signal {
            Some(s) => s + 0.1 * (p.level_db - s),
            None => p.level_db,
        });
        let decoded = packet_line(&head, &content, p.freq_hz);
        let (kind, position) = match &content {
            Content::Position(r) => (PacketKind::Position, Some((r.lat_deg, r.lon_deg))),
            Content::Status => (PacketKind::Status, None),
            Content::Encrypted => {
                c.encrypted += 1;
                (PacketKind::Encrypted, None)
            }
        };
        self.store.packets.push(PacketRow {
            time: t,
            kind,
            id: head.id(),
            decoded,
            position,
            channel_hz: p.freq_hz,
            repaired: p.repaired > 0,
            bytes: p.bytes.to_vec(),
        });
        let key = TrackerKey {
            address_type: head.address_type,
            address: head.address,
        };
        let stamp = self.stamp(t);
        let Ok(up) = self
            .store
            .aircraft
            .upsert(&key, stamp, || Aircraft::new(key))
        else {
            return true;
        };
        let a = up.row;
        a.relay = head.relay;
        a.emergency = head.emergency;
        a.level_db = p.level_db;
        a.channel_hz = p.freq_hz;
        if let Content::Position(r) = &content {
            let at = Position::new(r.lat_deg, r.lon_deg);
            if at.validate().is_ok() {
                a.report = Some(ReportRow {
                    position: at,
                    alt_m: r.alt_m,
                    speed_m_s: r.speed_m_s,
                    heading_deg: r.heading_deg,
                    climb_m_s: r.climb_m_s,
                    turn_deg_s: r.turn_deg_s,
                    aircraft_type: r.aircraft_type,
                    stealth: r.stealth,
                    fix_quality: r.fix_quality,
                    second: r.second,
                });
                // A point too near the last one in time or in distance is left out.
                let _ = a.trail.push(t, at, ());
            }
        }
        let at = a.report.map(|r| (r.position.lat_deg, r.position.lon_deg));
        if let (false, Some(r)) = (a.announced, a.report) {
            a.announced = true;
            self.events.push(
                OGN_TRACKER.event(
                    t, p.freq_hz,
                    &OgnTrackerEvent {
                        source: &a.id,
                        address_type: proto::ADDRESS_TYPES[usize::from(a.address_type & 3)],
                        aircraft_type: proto::AIRCRAFT_TYPES[usize::from(r.aircraft_type & 15)],
                        position: (r.position.lat_deg, r.position.lon_deg),
                        private: a.private(),
                        family: Family::Aircraft,
                    },
                ),
            );
        }
        if head.emergency && a.alarm_at.is_none_or(|l| t - l >= EMERGENCY_HOLD_S) {
            a.alarm_at = Some(t);
            let e = OGN_EMERGENCY.event(
                t,
                p.freq_hz,
                &OgnEmergencyEvent {
                    source: &a.id,
                    text: "emergency flag set",
                    position: at,
                    private: a.private(),
                    urgency: Urgency::Emergency,
                    family: Family::Alert,
                },
            );
            self.events.push(e);
        }
        true
    }

    /// Puts the last accepted packet on the channel of its copy `p`.
    fn move_last(&mut self, p: &OgnPacket) {
        let Some((head, content)) = proto::parse(&p.bytes) else {
            return;
        };
        if let Some(row) = self.store.packets.newest_mut() {
            row.channel_hz = p.freq_hz;
            row.decoded = packet_line(&head, &content, p.freq_hz);
        }
        let key = TrackerKey {
            address_type: head.address_type,
            address: head.address,
        };
        if let Some(a) = self.store.aircraft.get_mut(&key) {
            a.channel_hz = p.freq_hz;
            a.level_db = p.level_db;
        }
        self.dirty = true;
    }

    fn clear(&mut self) {
        self.store.clear();
        self.signal = None;
        self.last = None;
        self.last_good = -1e9;
        if self.configured {
            self.publish();
        }
    }

    /// The channels in words: each with "on" or "off" for a short plan, the count and the range for a long one.
    fn channel_text(&self) -> String {
        let on = self.channels.iter().filter(|c| c.on).count();
        let mut t = String::new();
        if self.channels.len() <= 4 {
            for (i, c) in self.channels.iter().enumerate() {
                let _ = write!(
                    t,
                    "{}{} MHz {}",
                    if i > 0 { ", " } else { "" },
                    mhz(c.freq_hz),
                    if c.on { "on" } else { "off" }
                );
            }
        } else if let (Some(a), Some(b)) = (self.channels.first(), self.channels.last()) {
            let _ = write!(
                t,
                "{} channels, {} to {} MHz, {on} on, {} outside the capture",
                self.channels.len(),
                mhz(a.freq_hz),
                mhz(b.freq_hz),
                self.channels.len() - on
            );
        }
        t
    }

    fn publish(&mut self) {
        let now = self.clock.now();
        let s = &self.store;
        let c = &s.counters;
        let n = s.aircraft.len();
        let lock_state = if now - self.last_good < 8.0 {
            LockState::Locked
        } else if now - self.last_signal < 2.0 {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        let on: Vec<&ChannelState> = self.channels.iter().filter(|c| c.on).collect();
        let floors: Vec<f64> = on.iter().filter_map(|c| c.noise_dbfs).collect();
        let noise = (!floors.is_empty()).then(|| floors.iter().sum::<f64>() / floors.len() as f64);
        let headline = if n > 0 {
            format!("{n} trackers, {} packets", c.good)
        } else {
            match on.as_slice() {
                [] => "no channel inside the capture".to_string(),
                [a] => format!("listening on {} MHz", mhz(a.freq_hz)),
                [a, b] => format!("listening on {} and {} MHz", mhz(a.freq_hz), mhz(b.freq_hz)),
                [a, .., b] => format!(
                    "listening on {} channels, {} to {} MHz",
                    on.len(),
                    mhz(a.freq_hz),
                    mhz(b.freq_hz)
                ),
            }
        };
        let text = self.channel_text();
        let on_n = on.len();
        let syncs: u64 = self.channels.iter().map(|c| c.stats.syncs).sum();
        let bad: u64 = self.channels.iter().map(|c| c.stats.bad).sum();
        let p = &mut self.published;
        self.status.write(
            p,
            lock_state,
            self.signal,
            noise,
            None,
            format_args!(
                "{headline}; experimental: sync word, code table, whitening and field layout not confirmed by a public \
                 document"
            ),
        );
        let as_count = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        p.put(&OgnFields {
            frames_ok: as_count(c.good),
            frames_bad: as_count(bad),
            syncs: as_count(syncs),
            corrected: as_count(c.corrected),
            encrypted: as_count(c.encrypted),
            trackers: n as i64,
            channels_on: on_n as i64,
            channels: &text,
            noise_dbfs: noise,
            snr_db: self.signal,
            note: NOT_CONFIRMED,
        });
        shaped::log_rows(&s.packets, &OGN_PACKETS, now, p);
        shaped::rows(&s.aircraft, &OGN_AIRCRAFT, now, p);
        publish_tracks(
            &s.aircraft,
            |a: &Aircraft| &a.trail,
            p,
            &mut self.key,
            |p, key, aircraft, position, time_s| {
                OGN_TRACKS.put(p, key, &AircraftTrackRow { aircraft, position, time_s });
            },
            &OGN_TRACKS,
        );
        shaped::points(&s.aircraft, "aircraft/", p.results_mut());
        self.dirty = false;
    }
}

impl Component for OgnTracker {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ogn.tracker",
            vec![Port::typed::<OgnItem>("items"), Port::iq("clock")],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(OgnTrackerSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(inputs[1].rate);
        self.configured = true;
        self.syncs_seen = 0;
        self.bad_seen = 0;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(1);
        let epoch = self.clock.epoch;
        for it in io.items::<OgnItem>(0) {
            match &it.value {
                OgnItem::Packet(p) => {
                    self.take_packet(epoch + it.time, p);
                }
                OgnItem::Channels(c) => {
                    let (syncs, bad) = c
                        .iter()
                        .fold((0, 0), |(s, b), c| (s + c.stats.syncs, b + c.stats.bad));
                    if syncs > self.syncs_seen || bad > self.bad_seen {
                        self.last_signal = self.clock.now();
                    }
                    self.syncs_seen = syncs;
                    self.bad_seen = bad;
                    self.channels.clone_from(c);
                    self.dirty = true;
                }
            }
        }
        self.clock.samples += n as u64;
        let tick = io.tick();
        if tick {
            let now = self.clock.now();
            self.store.aircraft.expire(now, self.expiry_s);
        }
        if tick || self.dirty {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match OgnTrackerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            OgnTrackerSetting::ExpiryS => self.expiry_s = EXPIRY_S.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(OgnTrackerSetting::ExpiryS, Value::F64(self.expiry_s))])
    }
    fn commands(&self) -> &Commands {
        OgnTrackerCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match OgnTrackerCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            OgnTrackerCommand::Clear => {
                self.published.clear_rows(&OGN_PACKETS);
                self.clear();
            }
            OgnTrackerCommand::Remove => {
                commands::remove(
                    &mut self.store.aircraft,
                    self.aircraft_table,
                    &mut self.published,
                    args,
                    &AIRCRAFT_ROW,
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
        if self.configured {
            self.publish();
        }
        Ok(())
    }
}
