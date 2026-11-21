//! FLARM (`flarm`): the old ("V6") protocol on 868.2 and 868.4 MHz from one capture centred on 868.3 MHz.

pub mod demod;
pub mod generator;
pub mod proto;

use crate::family::{
    min_rate_message, publish_tracks, wall, SignalClock, StatusLine, REPORT_S,
};
use demod::{Channel, ChannelItem, ChannelStats, FlarmChannelComponent, FlarmFrame};
use proto::{AIRCRAFT_TYPES, PAYLOAD, V6};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use crate::typed::{
    AircraftTrackRow, FlarmAircraftEvent, FlarmAircraftRow, FlarmFields, AIRCRAFT_REMOVE,
    AIRCRAFT_ROW, FLARM_AIRCRAFT, FLARM_AIRCRAFT_SEEN, FLARM_TRACKS,
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
    Item, Kind, Port, RateFrom, RunContext, StackGraph, Timed,
};
use sdrkit_util_geo::position::{closest_approach, Track};
use sdrkit_util_geo::{Position, KNOT_M_S};
use sdrkit_util_objects::commands::{self, Tables};
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, ObjectTable, Seen, Shift, Spacing, Stamp, Trail};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::RandomState;
use std::collections::{HashSet, VecDeque};
use std::hash::BuildHasher;
use std::sync::Arc;

/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 1e6;
pub(crate) const TRAIL_CAP: usize = 200;
pub(crate) const AIRCRAFT_CAP: usize = 128;
const PENDING_CAP: usize = 256;
const MAX_RANGE_M: f64 = 600e3;
/// The least time between two trail points, seconds.
const TRAIL_EVERY_S: f64 = 2.0;
/// The least distance between two trail points, metres.
const TRAIL_SPACING_M: f64 = 25.0;
/// The collision rule: the closest approach to the station is nearer than this, metres.
pub const COLLISION_DISTANCE_M: f64 = 500.0;
/// The collision rule: the closest approach comes within this time, seconds.
pub const COLLISION_TIME_S: f64 = 30.0;
/// The collision rule: the altitude differs from the station's height by at most this, metres.
pub const COLLISION_HEIGHT_M: f64 = 150.0;

/// Take the key time from the wall clock.
pub const WALL_CLOCK: Setting<field::Bool> =
    Setting::new("wall_clock", "Key time from this computer's clock", "", true).help(
        "On for a radio: the packets' key time is the computer's clock (keep it right to a few seconds). Off, or for a \
         recording or the test signal: the start time plus signal time.",
    );
/// The key time of the first sample when the wall clock is not used.
pub const START_TIME: Setting<field::F64> =
    Setting::new("start_time", "Start time (unix seconds)", "s", 0.0)
        .range(0.0, 4.0e9, 1.0)
        .help("The key time of the first sample when the computer's clock is not used. 0: the start of playback.");
/// The silence after which an aircraft leaves the table.
pub const EXPIRY_S: Setting<field::F64> =
    Setting::new("expiry_s", "Forget aircraft after", "s", 60.0)
        .range(5.0, 3600.0, 1.0)
        .help("Silence after which an aircraft leaves the table.");
/// The position that the aircraft of the test signal circle.
pub const GEN_CENTRE: Setting<field::Position> =
    Setting::new("gen_centre", "Test signal centre", "", (54.70, 25.30))
        .help("The position the simulated aircraft circle.")
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `flarm` stack.
    pub enum FlarmSetting {
        /// Take the key time from the wall clock.
        WallClock = WALL_CLOCK,
        /// The key time of the first sample.
        StartTime = START_TIME,
        /// The silence after which an aircraft leaves the table.
        ExpiryS = EXPIRY_S,
        /// The centre of the test signal.
        GenCentre = GEN_CENTRE,
    }
}

sdrkit_core::settings! {
    /// The settings of [`FlarmTracker`].
    pub enum FlarmTrackerSetting {
        /// Take the key time from the wall clock.
        WallClock = WALL_CLOCK,
        /// The key time of the first sample.
        StartTime = START_TIME,
        /// The silence after which an aircraft leaves the table.
        ExpiryS = EXPIRY_S,
    }
}


sdrkit_core::commands! {
    /// The commands of [`FlarmTracker`].
    pub enum FlarmTrackerCommand {
        /// Forgets the aircraft and the counters.
        Clear = commands::CLEAR,
        /// Forgets one aircraft.
        Remove = AIRCRAFT_REMOVE.command,
    }
}

/// The FLARM receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among
/// them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("ch0", FlarmChannelComponent::new(Channel::Low))
        .add("ch1", FlarmChannelComponent::new(Channel::High))
        .add("tracker", FlarmTracker::default())
        .input("ch0")
        .input("ch1")
        .input("tracker.clock")
        .connect("ch0", "tracker.ch0")
        .connect("ch1", "tracker.ch1")
        .output("tracker.events")
        .clock(REPORT_S)
        .status_from("tracker")
        .reconfigure_on_retune()
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape("flarm"), graph)
        .min_rate(MIN_RATE_HZ, &min_rate_message("FLARM", MIN_RATE_HZ))
        .route(WALL_CLOCK.key(), &["tracker.wall_clock"])?
        .route(START_TIME.key(), &["tracker.start_time"])?
        .route(EXPIRY_S.key(), &["tracker.expiry_s"])?
        .command("clear", sdrkit_graph::CommandTarget::of("tracker", "clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("tracker", "remove"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// A decoded V6 packet that passed the privacy gate and the agreement rule.
#[derive(Clone, Debug, PartialEq)]
pub struct FlarmPosition {
    /// The decrypted fields.
    pub v6: V6,
    /// The channel.
    pub channel: Channel,
    /// Level over the noise, dB.
    pub level_db: f64,
}

impl Item for FlarmPosition {
    const KIND: &'static str = "flarm.position";
}

/// A device address: the 24-bit address of a FLARM device. Its text is 6 hex digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeviceId(pub u32);

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:06X}", self.0)
    }
}

/// One aircraft of the table.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Aircraft {
    /// The 24-bit address.
    pub address: u32,
    /// The address as 6 hex digits.
    pub id: String,
    /// Address type (0 to 3).
    pub addr_type: u8,
    /// Aircraft type (0 to 15).
    pub aircraft_type: u8,
    /// Altitude, metres.
    pub alt_m: f64,
    /// Airborne flag.
    pub airborne: bool,
    /// Channel of the last packet.
    pub channel: Channel,
    /// Level of the last packet over the noise, dB.
    pub level_db: f64,
    /// Speed, m/s.
    pub speed_m_s: f64,
    /// Track, degrees.
    pub track: f64,
    /// Climb, m/s.
    pub climb: f64,
    /// Completed position.
    pub position: Option<Position>,
    /// Distance from the station, km.
    pub distance_km: Option<f64>,
    /// Bearing from the station, degrees.
    pub bearing: Option<f64>,
    /// On a collision course with the station ([`Aircraft::urgency`]).
    pub collision: bool,
    /// The track of positions.
    pub trail: Trail,
    #[serde(skip)]
    trail_t: Option<f64>,
    #[serde(skip)]
    event_t: Option<f64>,
    /// The last packet's clear position bits (for the agreement of later packets).
    pub lat19: u32,
    /// The last packet's clear position bits.
    pub lon20: u32,
}

impl Aircraft {
    fn new(address: u32) -> Aircraft {
        Aircraft {
            address,
            id: DeviceId(address).to_string(),
            addr_type: 0,
            aircraft_type: 0,
            alt_m: 0.0,
            airborne: false,
            channel: Channel::Low,
            level_db: 0.0,
            speed_m_s: 0.0,
            track: 0.0,
            climb: 0.0,
            position: None,
            distance_km: None,
            bearing: None,
            collision: false,
            trail: Trail::new(TRAIL_CAP, Spacing::metres(TRAIL_SPACING_M)),
            trail_t: None,
            event_t: None,
            lat19: 0,
            lon20: 0,
        }
    }
    /// Urgent when the aircraft is on a collision course with the station, else none. Collision course: held to its present
    /// speed and track, it passes nearer than [`COLLISION_DISTANCE_M`] to the station within [`COLLISION_TIME_S`], at an
    /// altitude within [`COLLISION_HEIGHT_M`] of the station's height.
    pub fn urgency(&self) -> Urgency {
        if self.collision {
            Urgency::Urgent
        } else {
            Urgency::None
        }
    }
}

/// Whether an aircraft at `at`, altitude `alt_m`, speed `speed_m_s` and track `track_deg` is on a collision course with the
/// station at `station` (the rule of [`Aircraft::urgency`]).
pub fn collision_course(
    station: &Position,
    at: Position,
    alt_m: f64,
    speed_m_s: f64,
    track_deg: f64,
) -> bool {
    if (alt_m - station.alt_m).abs() > COLLISION_HEIGHT_M {
        return false;
    }
    let own = Track {
        position: *station,
        speed_m_s: 0.0,
        course_deg: 0.0,
    };
    let other = Track {
        position: at,
        speed_m_s,
        course_deg: track_deg,
    };
    let a = closest_approach(&own, &other);
    a.distance_m < COLLISION_DISTANCE_M && a.time_s <= COLLISION_TIME_S
}

impl Shift for Aircraft {
    fn shift_times(&mut self, by_s: f64) {
        self.trail.shift_times(by_s);
        self.trail_t = None;
        self.event_t = None;
    }
}

sdrkit_core::choice_enum! {
    /// Where the key time of a packet comes from.
    pub enum KeyTime {
        /// The computer's clock.
        Clock = ("clock", "the computer's clock (keep it right to a few seconds)"),
        /// The start time plus signal time.
        StartPlusSignal = ("start_plus_signal", "the start time plus signal time"),
    }
}

/// What `flarm` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("flarm").results_decl()
}

impl Shaped for Aircraft {
    type Row = FlarmAircraftRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&FlarmAircraftRow {
            source: &self.id,
            address_type: i64::from(self.addr_type),
            r#type: i64::from(self.aircraft_type),
            position: self.position.map(|p| (p.lat_deg, p.lon_deg)),
            alt_m: self.alt_m,
            speed_kt: self.speed_m_s / KNOT_M_S,
            course_deg: self.track,
            climb_m_s: self.climb,
            distance_km: self.distance_km,
            bearing_deg: self.bearing,
            channel_hz: Hertz(self.channel.frequency_hz()),
            level_db: self.level_db,
            airborne: self.airborne,
            urgency: self.urgency(),
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
        });
    }
    fn point(&self) -> Option<Point<'_>> {
        let p = Point::new(self.position?, format!("{} {:.0} m", self.id, self.alt_m));
        Some(p.heading(self.track as f32))
    }
}

/// The counters of the stack.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Counters {
    /// Old-protocol packets accepted.
    pub v6_decoded: u64,
    /// Bad packets (no key giving a good parity).
    pub bad: u64,
    /// Good frames on each channel.
    pub per_channel: [u64; 2],
    /// Newer-protocol packets.
    pub newer_packets: u64,
    /// Newer-protocol devices (distinct).
    pub newer_devices: u64,
    /// Packets of other types.
    pub other_type: u64,
    /// Privacy-flagged aircraft (distinct).
    pub privacy_aircraft: u64,
    /// Privacy-flagged packets.
    pub privacy_packets: u64,
    /// The farthest range seen, km.
    pub farthest_km: f64,
}

/// What the tracker keeps across sessions: the aircraft and the counters. Nothing privacy-flagged is ever kept.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The aircraft by device address.
    pub aircraft: ObjectTable<DeviceId, Aircraft>,
    /// The counters.
    pub counters: Counters,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            aircraft: ObjectTable::new(AIRCRAFT_CAP),
            counters: Counters::default(),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.aircraft.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("drones.flarm", 2);
    fn clear(&mut self) {
        self.aircraft.clear();
        self.counters = Counters::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.aircraft.adopt(saved.aircraft);
        self.counters = saved.counters;
    }
}

#[derive(Clone, Debug)]
struct PendingFirst {
    hash: u64,
    t: f64,
    cands: Vec<V6>,
    channel: Channel,
    level_db: f64,
}

/// What the tracker hears: nothing, energy on a channel, aircraft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Heard {
    Nothing,
    Energy,
    Aircraft,
}

/// The tracker component (`drones.flarm_tracker`): decrypts V6 packets, applies the privacy gate and the agreement rule, and
/// keeps the aircraft table.
pub struct FlarmTracker {
    published: Published,
    aircraft_table: TableId,
    status: StatusLine,
    store: Store,
    clock: SignalClock,
    live: bool,
    host_clock: Clock,
    station: Option<Position>,
    wall_clock: bool,
    start_time: f64,
    playback_start: f64,
    expiry_s: f64,
    salt: RandomState,
    pending: VecDeque<PendingFirst>,
    flagged: HashSet<u64>,
    newer: HashSet<u64>,
    newer_t: Option<f64>,
    new_logged: u32,
    windows: [u64; 4],
    window_now: u64,
    signal: Option<f64>,
    chan: [ChannelStats; 2],
    state: Option<Heard>,
    lines: Vec<String>,
    events: Vec<Event>,
    positions: Vec<Timed<FlarmPosition>>,
    key: String,
    configured: bool,
    dirty: bool,
}

impl Default for FlarmTracker {
    fn default() -> Self {
        let decl = Arc::new(declaration());
        let aircraft_table = decl
            .table_id(FLARM_AIRCRAFT.key)
            .expect("the declaration holds the aircraft table");
        FlarmTracker {
            published: Published::from_decl(decl),
            aircraft_table,
            status: StatusLine::default(),
            store: Store::default(),
            clock: SignalClock::default(),
            live: false,
            host_clock: Clock::fixed(0.0),
            station: None,
            wall_clock: WALL_CLOCK.default,
            start_time: START_TIME.default,
            playback_start: 0.0,
            expiry_s: EXPIRY_S.default,
            salt: RandomState::new(),
            pending: VecDeque::new(),
            flagged: HashSet::new(),
            newer: HashSet::new(),
            newer_t: None,
            new_logged: 0,
            windows: [0; 4],
            window_now: 0,
            signal: None,
            chan: [
                ChannelStats::default(),
                ChannelStats {
                    channel: Channel::High,
                    ..Default::default()
                },
            ],
            state: None,
            lines: Vec::new(),
            events: Vec::new(),
            positions: Vec::new(),
            key: String::new(),
            configured: false,
            dirty: true,
        }
    }
}

impl FlarmTracker {
    /// The table the tracker keeps.
    pub fn store(&self) -> &Store {
        &self.store
    }

    fn hash(&self, address: u32) -> u64 {
        self.salt.hash_one(address)
    }

    /// The key time (unix seconds) of a packet at signal time `t`.
    pub fn key_time(&self, t: f64) -> f64 {
        match (self.wall_clock && self.live)
            .then(|| wall(true, &self.host_clock))
            .flatten()
        {
            Some(w) => w,
            None => {
                let start = if self.start_time > 0.0 {
                    self.start_time
                } else {
                    self.playback_start
                };
                start + t
            }
        }
    }

    /// Takes one good frame at signal time `t`.
    pub fn take_frame(&mut self, t: f64, f: &FlarmFrame) {
        self.store.counters.per_channel[f.channel.slot()] += 1;
        let mut payload = [0u8; PAYLOAD];
        payload.copy_from_slice(&f.bytes[..PAYLOAD]);
        let (address, htype, _) = proto::header(&payload);
        let h = self.hash(address);
        match htype {
            0 => {}
            2 => {
                self.store.counters.newer_packets += 1;
                self.newer.insert(h);
                self.store.counters.newer_devices = self.newer.len() as u64;
                self.newer_t = Some(t);
                self.dirty = true;
                return;
            }
            _ => {
                self.store.counters.other_type += 1;
                return;
            }
        }
        let kt = self.key_time(t).floor().max(0.0) as u32;
        let step_pos = kt % 64;
        let mut times = [kt, kt, kt];
        let mut nt = 1;
        if step_pos >= 52 {
            times[nt] = kt.wrapping_add(64);
            nt += 1;
        }
        if step_pos < 12 {
            times[nt] = kt.wrapping_sub(64);
            nt += 1;
        }
        let cands: Vec<V6> = times[..nt]
            .iter()
            .filter_map(|&k| proto::decode(&payload, k))
            .collect();
        if cands.is_empty() {
            self.store.counters.bad += 1;
            return;
        }
        self.dirty = true;
        if self.flagged.contains(&h) && cands.iter().any(|c| c.flagged()) {
            self.store.counters.privacy_packets += 1;
            return;
        }
        // A known aircraft: the unflagged candidate agreeing with its last packet applies at once.
        if let Some(a) = self.store.aircraft.get(&DeviceId(address)) {
            let (al, ao, at) = (a.lat19, a.lon20, a.aircraft_type);
            if let Some(c) = cands.iter().find(|c| {
                !c.flagged()
                    && c.aircraft_type == at
                    && proto::mod_distance(c.lat19, al, 19) <= 4000
                    && proto::mod_distance(c.lon20, ao, 20) <= 4000
            }) {
                let c = c.clone();
                self.accept(t, c, f.channel, f.level_db);
                return;
            }
        }
        // The agreement rule.
        self.pending.retain(|p| t - p.t <= 60.0);
        if let Some(i) = self.pending.iter().position(|p| p.hash == h) {
            let first = &self.pending[i];
            let mut pair = None;
            'outer: for a in &first.cands {
                for b in &cands {
                    if a.flagged() == b.flagged()
                        && a.aircraft_type == b.aircraft_type
                        && proto::mod_distance(a.lat19, b.lat19, 19) <= 4000
                        && proto::mod_distance(a.lon20, b.lon20, 20) <= 4000
                    {
                        pair = Some((a.clone(), b.clone()));
                        break 'outer;
                    }
                }
            }
            if let Some((mut a, b)) = pair {
                let Some(p) = self.pending.remove(i) else {
                    return;
                };
                if b.flagged() {
                    self.flagged.insert(h);
                    self.store.counters.privacy_aircraft = self.flagged.len() as u64;
                    self.store.counters.privacy_packets += 2;
                } else {
                    a.address = b.address;
                    self.accept(p.t, a, p.channel, p.level_db);
                    self.accept(t, b, f.channel, f.level_db);
                }
                return;
            }
            let p = &mut self.pending[i];
            p.t = t;
            p.cands = strip(cands);
            p.channel = f.channel;
            p.level_db = f.level_db;
            return;
        }
        if self.pending.len() >= PENDING_CAP {
            self.pending.pop_front();
        }
        self.pending.push_back(PendingFirst {
            hash: h,
            t,
            cands: strip(cands),
            channel: f.channel,
            level_db: f.level_db,
        });
    }

    fn accept(&mut self, t: f64, v: V6, channel: Channel, level_db: f64) {
        self.store.counters.v6_decoded += 1;
        self.window_now += 1;
        self.signal = Some(match self.signal {
            Some(s) => 0.9 * s + 0.1 * level_db,
            None => level_db,
        });
        let stamp = match wall(self.live, &self.host_clock) {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        };
        let station = self.station;
        let Ok(up) = self
            .store
            .aircraft
            .upsert(&DeviceId(v.address), stamp, || Aircraft::new(v.address))
        else {
            return;
        };
        if up.new && self.new_logged < 40 {
            self.new_logged += 1;
            self.lines.push(format!(
                "FLARM: new aircraft, device {:06X} ({})",
                v.address,
                AIRCRAFT_TYPES[v.aircraft_type as usize & 15]
            ));
        }
        let a = up.row;
        let (speed, track) = v.velocity();
        a.addr_type = v.addr_type;
        a.aircraft_type = v.aircraft_type;
        a.alt_m = f64::from(v.alt_m);
        a.airborne = v.airborne;
        a.channel = channel;
        a.level_db = level_db;
        a.speed_m_s = speed;
        a.track = track;
        a.climb = v.climb();
        a.lat19 = v.lat19;
        a.lon20 = v.lon20;
        let mut farthest = None;
        if let Some(st) = station {
            let (lat, lon) = v.position(st.lat_deg, st.lon_deg);
            let at = Position::new(lat, lon);
            let d = st.distance_m(&at);
            if d < MAX_RANGE_M {
                if a.trail_t.is_none_or(|p| t - p >= TRAIL_EVERY_S)
                    && a.trail.push(t, at, ()).unwrap_or(false)
                {
                    a.trail_t = Some(t);
                }
                a.position = Some(at);
                a.distance_km = Some(d / 1000.0);
                a.bearing = Some(st.initial_bearing_deg(&at));
                a.collision = collision_course(&st, at, a.alt_m, speed, track);
                farthest = Some(d / 1000.0);
            }
        }
        if let Some(at) = a.position {
            if a.event_t.is_none_or(|e| t - e >= 5.0) {
                a.event_t = Some(t);
                self.events.push(
                    FLARM_AIRCRAFT_SEEN.event(
                        t,
                        channel.frequency_hz(),
                        &FlarmAircraftEvent {
                            source: &a.id,
                            r#type: i64::from(a.aircraft_type),
                            position: (at.lat_deg, at.lon_deg),
                            alt_m: a.alt_m,
                            speed_kt: a.speed_m_s / KNOT_M_S,
                            course_deg: a.track,
                            climb_m_s: a.climb,
                            urgency: a.urgency(),
                            family: Family::Aircraft,
                        },
                    ),
                );
            }
        }
        if let Some(d) = farthest {
            let c = &mut self.store.counters;
            c.farthest_km = c.farthest_km.max(d);
        }
        self.positions.push(Timed {
            time: t - self.clock.epoch,
            value: FlarmPosition {
                v6: v,
                channel,
                level_db,
            },
        });
    }

    /// For a component that holds the tracker inside itself: the positions accepted since the last call. The events and
    /// the log lines of the tracker are dropped, and aircraft not heard for `expiry_s` leave its table.
    pub(crate) fn take_accepted(&mut self, now: f64, expiry_s: f64) -> Vec<Timed<FlarmPosition>> {
        self.events.clear();
        self.lines.clear();
        self.store.aircraft.expire(now, expiry_s);
        std::mem::take(&mut self.positions)
    }

    /// For a component that holds the tracker inside itself: forgets the aircraft and the counters.
    pub(crate) fn forget(&mut self) {
        self.clear();
    }

    fn heard_now(&self, now: f64) -> Heard {
        if !self.store.aircraft.is_empty() {
            Heard::Aircraft
        } else if self
            .chan
            .iter()
            .any(|c| c.energy_at.is_some_and(|e| now - self.clock.at(e) <= 5.0))
        {
            Heard::Energy
        } else {
            Heard::Nothing
        }
    }

    fn clear(&mut self) {
        self.store.clear();
        self.pending.clear();
        self.flagged.clear();
        self.newer.clear();
        self.newer_t = None;
        self.windows = [0; 4];
        self.window_now = 0;
        self.signal = None;
        self.new_logged = 0;
        for c in &mut self.chan {
            c.syncs = 0;
            c.bad = 0;
            c.good = 0;
        }
        self.dirty = true;
        if self.configured {
            self.publish();
        }
    }

    fn publish(&mut self) {
        let now = self.clock.now();
        let state = self.heard_now(now);
        if self.state != Some(state) {
            if self.state.is_some() || state != Heard::Nothing {
                self.lines.push(
                    match state {
                        Heard::Aircraft => "FLARM: receiving aircraft",
                        Heard::Energy => "FLARM: energy on a channel, no good packet lately",
                        Heard::Nothing => "FLARM: no signal",
                    }
                    .into(),
                );
            }
            self.state = Some(state);
        }
        let s = &self.store;
        let c = &s.counters;
        let syncs = self.chan[0].syncs + self.chan[1].syncs;
        let bad = c.bad + self.chan[0].bad + self.chan[1].bad;
        let with_position = s
            .aircraft
            .iter()
            .filter(|r| r.record.position.is_some())
            .count();
        let pps: u64 = self.windows.iter().sum::<u64>();
        let noise = match (self.chan[0].noise_dbfs, self.chan[1].noise_dbfs) {
            (Some(a), Some(b)) => Some((a + b) / 2.0),
            (a, b) => a.or(b),
        };
        let p = &mut self.published;
        let lock_state = match state {
            Heard::Aircraft => LockState::Locked,
            Heard::Energy => LockState::Tracking,
            Heard::Nothing => LockState::Searching,
        };
        let signal = self.signal;
        let st = &mut self.status;
        let n = s.aircraft.len();
        if n > 0 && c.privacy_aircraft > 0 {
            st.write(
                p,
                lock_state,
                signal,
                noise,
                None,
                format_args!(
                    "{n} aircraft, {pps} packets/s, {} privacy-flagged",
                    c.privacy_aircraft
                ),
            );
        } else if n > 0 {
            st.write(
                p,
                lock_state,
                signal,
                noise,
                None,
                format_args!("{n} aircraft, {pps} packets/s"),
            );
        } else if c.newer_packets > 0 {
            st.write(
                p,
                lock_state,
                signal,
                noise,
                None,
                format_args!(
                    "FLARM (new protocol): {} packets, no positions",
                    c.newer_packets
                ),
            );
        } else if state != Heard::Nothing {
            st.write(
                p,
                lock_state,
                signal,
                noise,
                None,
                format_args!("signal, no packet yet"),
            );
        } else {
            st.write(
                p,
                lock_state,
                signal,
                noise,
                None,
                format_args!("listening on 868.2 and 868.4 MHz"),
            );
        }
        for l in self.lines.drain(..) {
            p.results_mut().log(l);
        }
        let key_time = if self.wall_clock && self.live {
            KeyTime::Clock
        } else {
            KeyTime::StartPlusSignal
        };
        p.put(&FlarmFields {
            v6_decoded: c.v6_decoded as i64,
            frames_bad: bad as i64,
            syncs: syncs as i64,
            frames_868_2: c.per_channel[0] as i64,
            frames_868_4: c.per_channel[1] as i64,
            newer_packets: c.newer_packets as i64,
            newer_devices: c.newer_devices as i64,
            other_type: c.other_type as i64,
            privacy_aircraft: c.privacy_aircraft as i64,
            privacy_packets: c.privacy_packets as i64,
            aircraft: n as i64,
            with_position: with_position as i64,
            farthest_km: c.farthest_km,
            packets_per_s: pps as f64,
            key_time,
            needs_station: self.station.is_none(),
            snr_db: signal,
            noise_dbfs: noise,
        });
        shaped::rows(&s.aircraft, &FLARM_AIRCRAFT, now, p);
        publish_tracks(
            &s.aircraft,
            |a: &Aircraft| &a.trail,
            p,
            &mut self.key,
            |p, key, aircraft, position, time_s| {
                FLARM_TRACKS.put(p, key, &AircraftTrackRow { aircraft, position, time_s });
            },
            &FLARM_TRACKS,
        );
        let r = p.results_mut();
        shaped::points(&s.aircraft, "aircraft/", r);
        match self.station {
            Some(st) => {
                r.update_point("station", st.lat_deg, st.lon_deg, "station", None);
            }
            None => {
                r.remove_point("station");
            }
        }
        self.dirty = false;
    }
}

fn strip(mut c: Vec<V6>) -> Vec<V6> {
    // Only the fields are held until a second packet agrees: not the address.
    for v in &mut c {
        v.address = 0;
    }
    c
}

impl Component for FlarmTracker {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "drones.flarm_tracker",
            vec![
                Port::typed::<ChannelItem>("ch0"),
                Port::typed::<ChannelItem>("ch1"),
                Port::iq("clock"),
            ],
            vec![
                Port::typed::<FlarmPosition>("positions").rate_from(RateFrom::Fixed(0.0)),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FlarmTrackerSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
        self.playback_start = self.host_clock.now();
        self.station = ctx.station.position;
        self.dirty = true;
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[2];
        if !self.configured {
            self.lines.push(
                "FLARM: listening on 868.2 and 868.4 MHz (old protocol decoded, newer protocol counted)".into(),
            );
        }
        self.clock.restart(f.rate);
        self.configured = true;
        self.dirty = true;
        Ok(vec![
            Format::new(Kind::typed::<FlarmPosition>(), 0.0),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(2);
        let epoch = self.clock.epoch;
        for port in 0..2 {
            for it in io.items::<ChannelItem>(port) {
                match &it.value {
                    ChannelItem::Frame(f) => self.take_frame(epoch + it.time, f),
                    ChannelItem::Stats(s) => self.chan[s.channel.slot()].clone_from(s),
                }
            }
        }
        self.clock.samples += n as u64;
        let tick = io.tick();
        if tick {
            let now = self.clock.now();
            if self.store.aircraft.expire(now, self.expiry_s) > 0 {
                self.dirty = true;
            }
            self.windows.rotate_left(1);
            self.windows[3] = self.window_now;
            self.window_now = 0;
        }
        if tick || self.dirty {
            self.publish();
        }
        if !self.positions.is_empty() {
            io.items_out::<FlarmPosition>(0).append(&mut self.positions);
        }
        if !self.events.is_empty() {
            io.messages_out(1).append(&mut self.events);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FlarmTrackerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FlarmTrackerSetting::WallClock => self.wall_clock = WALL_CLOCK.read(id, v)?,
            FlarmTrackerSetting::StartTime => self.start_time = START_TIME.read(id, v)?,
            FlarmTrackerSetting::ExpiryS => self.expiry_s = EXPIRY_S.read(id, v)?,
        }
        self.dirty = true;
        if self.configured {
            self.publish();
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (FlarmTrackerSetting::WallClock, Value::Bool(self.wall_clock)),
            (FlarmTrackerSetting::StartTime, Value::F64(self.start_time)),
            (FlarmTrackerSetting::ExpiryS, Value::F64(self.expiry_s)),
        ])
    }
    fn commands(&self) -> &Commands {
        FlarmTrackerCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match FlarmTrackerCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            FlarmTrackerCommand::Clear => self.clear(),
            FlarmTrackerCommand::Remove => {
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
        self.dirty = true;
        if self.configured {
            self.publish();
        }
        Ok(())
    }
}
