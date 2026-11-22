//! ADS-L (`adsl`): the European position broadcast of light aircraft on 868.2, 868.4 and 869.525 MHz from one capture, with
//! FLARM from the same capture.
//! Standard: EASA ADS-L 4 SRD-860 Issue 2.

pub mod channels;
pub mod generator;
pub mod proto;

use crate::family::{min_rate_message, publish_tracks, wall, SignalClock, StatusLine, REPORT_S};
use crate::flarm::demod::ChannelItem;
use crate::flarm::proto::AIRCRAFT_TYPES as FLARM_TYPES;
use crate::flarm::{DeviceId, FlarmTracker, FlarmTrackerSetting, START_TIME, WALL_CLOCK};
use crate::typed::{
    AdslAircraftEvent, AdslAircraftRow, AdslEmergencyEvent, AdslFields, AircraftTrackRow,
    ADSL_AIRCRAFT, ADSL_AIRCRAFT_SEEN, ADSL_EMERGENCY, ADSL_TRACKS, AIRCRAFT_REMOVE, AIRCRAFT_ROW,
};
use channels::{AdslChannels, AdslItem, AdslPacket, Band, BandState, BANDS};
use proto::{amt_name, category_name, Address, DataHeader, Status, Traffic};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Clock, ConfigError, Event, LockState, Record, Results, ResultsDecl, Setting,
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
use sdrkit_util_objects::{Format as StateFormat, ObjectTable, Seen, Shift, Spacing, Stamp, Trail};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 1e6;
pub(crate) const TRAIL_CAP: usize = 200;
pub(crate) const AIRCRAFT_CAP: usize = 128;
/// A distance above this is not a range of the station, metres.
const MAX_RANGE_M: f64 = 1500e3;
/// The source number of an aircraft heard by the FLARM protocol, beside the address mapping tables 0 to 63.
pub const FLARM_RADIO: u8 = 64;

/// The silence after which an aircraft leaves the table.
pub const EXPIRY_S: Setting<field::F64> =
    Setting::new("expiry_s", "Forget aircraft after", "s", 60.0).range(5.0, 3600.0, 1.0);
/// The position that the aircraft of the test signal circle.
pub const GEN_CENTRE: Setting<field::Position> =
    Setting::new("gen_centre", "Test signal centre", "", (54.70, 25.30))
        .section("Test signal")
        .advanced();
/// The aircraft of the test signal.
pub const GEN_AIRCRAFT: Setting<field::I64> =
    Setting::new("gen_aircraft", "Test signal aircraft", "", 4)
        .range(1.0, 4.0, 1.0)
        .section("Test signal")
        .advanced();
/// The key index of the packets of the test signal: 0 scrambled, 3 not scrambled.
pub const GEN_KEY_INDEX: Setting<field::I64> =
    Setting::new("gen_key_index", "Test signal key index", "", 0)
        .range(0.0, 3.0, 1.0)
        .section("Test signal")
        .advanced();
/// The Traffic packets of the test signal on the M-band and on HDR carry Reed-Solomon parity.
pub const GEN_FEC: Setting<field::Bool> =
    Setting::new("gen_fec", "Test signal with Reed-Solomon", "", false)
        .section("Test signal")
        .advanced();
/// The repeats of "01" in the M-band preamble of the test signal.
pub const GEN_PREAMBLE: Setting<field::I64> =
    Setting::new("gen_preamble", "Test signal M-band preamble", "", 8)
        .range(5.0, 12.0, 1.0)
        .section("Test signal")
        .advanced();
/// Every n-th packet of the test signal has one wrong bit (0: none).
pub const GEN_BAD_BIT: Setting<field::I64> =
    Setting::new("gen_bad_bit", "Wrong bit in every n-th packet", "", 0)
        .range(0.0, 100.0, 1.0)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `adsl` stack.
    pub enum AdslSetting {
        /// M-band 1.
        M1Hz = channels::M1_HZ,
        /// M-band 2.
        M2Hz = channels::M2_HZ,
        /// O-band.
        OHz = channels::O_HZ,
        /// Forget aircraft after.
        ExpiryS = EXPIRY_S,
        /// Decode FLARM from the same capture.
        WithFlarm = channels::WITH_FLARM,
        /// M-band chip detector by tone energy.
        ToneChips = channels::TONE_CHIPS,
        /// FLARM key time from the wall clock.
        WallClock = WALL_CLOCK,
        /// FLARM key time of the first sample.
        StartTime = START_TIME,
        /// Test signal centre.
        GenCentre = GEN_CENTRE,
        /// Test signal aircraft.
        GenAircraft = GEN_AIRCRAFT,
        /// Test signal key index.
        GenKeyIndex = GEN_KEY_INDEX,
        /// Test signal with Reed-Solomon.
        GenFec = GEN_FEC,
        /// Test signal M-band preamble.
        GenPreamble = GEN_PREAMBLE,
        /// Wrong bit in every n-th packet.
        GenBadBit = GEN_BAD_BIT,
    }
}

sdrkit_core::settings! {
    /// The settings of [`AdslTracker`].
    pub enum AdslTrackerSetting {
        /// Forget aircraft after.
        ExpiryS = EXPIRY_S,
        /// FLARM key time from the wall clock.
        WallClock = WALL_CLOCK,
        /// FLARM key time of the first sample.
        StartTime = START_TIME,
    }
}

sdrkit_core::commands! {
    /// The commands of [`AdslTracker`].
    pub enum AdslTrackerCommand {
        /// Forgets the aircraft and the counters, also of the FLARM part.
        Clear = commands::CLEAR,
        /// Forgets one aircraft.
        Remove = AIRCRAFT_REMOVE.command,
    }
}

/// The `adsl` receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among
/// them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("channels", AdslChannels::default())
        .add("tracker", AdslTracker::default())
        .input("channels")
        .input("tracker.clock")
        .connect("channels", "tracker.items")
        .output("tracker.events")
        .clock(REPORT_S)
        .status_from("tracker")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape("adsl"), graph)
        .min_rate(MIN_RATE_HZ, &min_rate_message("ADS-L", MIN_RATE_HZ))
        .route(channels::M1_HZ.key(), &["channels.m1_hz"])?
        .route(channels::M2_HZ.key(), &["channels.m2_hz"])?
        .route(channels::O_HZ.key(), &["channels.o_hz"])?
        .route(channels::WITH_FLARM.key(), &["channels.with_flarm"])?
        .route(channels::TONE_CHIPS.key(), &["channels.tone_chips"])?
        .route(EXPIRY_S.key(), &["tracker.expiry_s"])?
        .route(WALL_CLOCK.key(), &["tracker.wall_clock"])?
        .route(START_TIME.key(), &["tracker.start_time"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("tracker", "remove"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// The key of an aircraft: where its address comes from (an address mapping table, or [`FLARM_RADIO`]) and the address. The same
/// 24 bits under two tables are two aircraft.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AircraftKey {
    /// The address mapping table, or [`FLARM_RADIO`].
    pub source: u8,
    /// The 24-bit address.
    pub address: u32,
}

impl std::fmt::Display for AircraftKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{:06X}", self.source, self.address)
    }
}

/// The name of the source of an address.
pub fn source_name(source: u8) -> &'static str {
    if source == FLARM_RADIO {
        "FLARM (old protocol)"
    } else {
        amt_name(source)
    }
}

/// One aircraft of the table.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Aircraft {
    /// The address as 6 hex digits.
    pub id: String,
    /// Where the address comes from.
    pub source: u8,
    /// The aircraft category, in words.
    pub category: String,
    /// The flight state, in words.
    pub flight_state: String,
    /// The emergency status, 0 to 7.
    pub emergency: u8,
    /// The position.
    pub position: Option<Position>,
    /// The altitude above the WGS-84 ellipsoid, metres.
    pub alt_m: Option<f64>,
    /// The ground speed, m/s.
    pub speed_m_s: Option<f64>,
    /// The ground track, degrees.
    pub track_deg: Option<f64>,
    /// The vertical rate, m/s.
    pub climb_m_s: Option<f64>,
    /// The level of the last packet over the noise, dB.
    pub level_db: f64,
    /// The distance from the station, km.
    pub distance_km: Option<f64>,
    /// The bearing from the station, degrees.
    pub bearing_deg: Option<f64>,
    /// The frequency of the last packet, Hz.
    pub channel_hz: f64,
    /// The channel of the last packet, in words.
    pub channel: String,
    /// Last heard through a ground station or a relay.
    pub relayed: bool,
    /// The navigation integrity, horizontal accuracy, vertical accuracy and velocity accuracy categories.
    pub quality: [u8; 4],
    /// The capabilities of a Status packet, in words.
    pub capabilities: String,
    /// The track of positions.
    pub trail: Trail,
    /// The event of the new aircraft was sent.
    #[serde(skip)]
    announced: bool,
}

impl Aircraft {
    fn new(key: AircraftKey) -> Aircraft {
        Aircraft {
            id: format!("{:06X}", key.address),
            source: key.source,
            category: String::new(),
            flight_state: String::new(),
            emergency: 0,
            position: None,
            alt_m: None,
            speed_m_s: None,
            track_deg: None,
            climb_m_s: None,
            level_db: 0.0,
            distance_km: None,
            bearing_deg: None,
            channel_hz: 0.0,
            channel: String::new(),
            relayed: false,
            quality: [0; 4],
            capabilities: String::new(),
            trail: Trail::new(
                TRAIL_CAP,
                Spacing {
                    min_s: 2.0,
                    min_m: 25.0,
                },
            ),
            announced: false,
        }
    }
    /// The address is a random one: the aircraft asks for privacy. It is shown here and is not for a public map.
    pub fn private(&self) -> bool {
        self.source == 0
    }
    /// Urgent for the emergency states 2 to 6.
    pub fn urgency(&self) -> Urgency {
        if (2..=6).contains(&self.emergency) {
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

/// What `adsl` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("adsl").results_decl()
}

impl Shaped for Aircraft {
    type Row = AdslAircraftRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AdslAircraftRow {
            source: &self.id,
            address_source: source_name(self.source),
            category: &self.category,
            flight_state: &self.flight_state,
            emergency: proto::EMERGENCIES[usize::from(self.emergency & 7)],
            position: self.position.map(|p| (p.lat_deg, p.lon_deg)),
            alt_m: self.alt_m,
            speed_kt: self.speed_m_s.map(|v| v / KNOT_M_S),
            course_deg: self.track_deg,
            climb_m_s: self.climb_m_s,
            level_db: self.level_db,
            distance_km: self.distance_km,
            bearing_deg: self.bearing_deg,
            channel_hz: Hertz(self.channel_hz),
            channel: &self.channel,
            relayed: self.relayed,
            nic: i64::from(self.quality[0]),
            hfom: i64::from(self.quality[1]),
            vfom: i64::from(self.quality[2]),
            velocity_accuracy: i64::from(self.quality[3]),
            capabilities: &self.capabilities,
            private: self.private(),
            urgency: self.urgency(),
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
        });
    }
    fn point(&self) -> Option<Point<'_>> {
        let label = match self.alt_m {
            Some(a) => format!("{} {a:.0} m", self.id),
            None => self.id.clone(),
        };
        let p = Point::new(self.position?, label);
        Some(match self.track_deg {
            Some(t) => p.heading(t as f32),
            None => p,
        })
    }
}

/// The counters of the stack.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Counters {
    /// Packets with good parity.
    pub good: u64,
    /// Good packets on each band.
    pub per_band: [u64; 4],
    /// Traffic, Status, Traffic Uplink, FIS-B Uplink, Remote Identification and other packets.
    pub by_payload: [u64; 6],
    /// Packets with a reserved key index.
    pub encrypted: u64,
    /// Packets with the signature flag.
    pub signed: u64,
    /// Packets that needed a repair.
    pub corrected: u64,
    /// FLARM packets accepted.
    pub flarm: u64,
    /// The farthest range seen, km.
    pub max_range_km: f64,
}

/// What the tracker keeps across sessions: the aircraft with their trails, and the counters.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The aircraft.
    pub aircraft: ObjectTable<AircraftKey, Aircraft>,
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
    const FORMAT: StateFormat = StateFormat::new("drones.adsl", 1);
    fn clear(&mut self) {
        self.aircraft.clear();
        self.counters = Counters::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.aircraft.adopt(saved.aircraft);
        self.counters = saved.counters;
    }
}

/// What the tracker hears: nothing, energy on a channel, aircraft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Heard {
    Nothing,
    Energy,
    Aircraft,
}

/// The tracker component (`adsl.tracker`): unscrambles the packets, reads the payloads and keeps the aircraft table. It holds
/// the FLARM tracker for the FLARM frames of the same capture.
pub struct AdslTracker {
    published: Published,
    aircraft_table: TableId,
    status: StatusLine,
    store: Store,
    dirty: bool,
    flarm: FlarmTracker,
    clock: SignalClock,
    live: bool,
    host_clock: Clock,
    station: Option<Position>,
    expiry_s: f64,
    bands: [BandState; 4],
    signal: Option<f64>,
    windows: [u64; 4],
    window_now: u64,
    new_logged: u32,
    state: Option<Heard>,
    lines: Vec<String>,
    events: Vec<Event>,
    key: String,
    configured: bool,
}

impl Default for AdslTracker {
    fn default() -> Self {
        let decl = Arc::new(declaration());
        let aircraft_table = decl
            .table_id(ADSL_AIRCRAFT.key)
            .expect("the declaration holds the aircraft table");
        AdslTracker {
            published: Published::from_decl(decl),
            aircraft_table,
            status: StatusLine::default(),
            store: Store::default(),
            dirty: true,
            flarm: FlarmTracker::default(),
            clock: SignalClock::default(),
            live: false,
            host_clock: Clock::fixed(0.0),
            station: None,
            expiry_s: EXPIRY_S.default,
            bands: BANDS.map(|band| BandState {
                band,
                ..BandState::default()
            }),
            signal: None,
            windows: [0; 4],
            window_now: 0,
            new_logged: 0,
            state: None,
            lines: Vec::new(),
            events: Vec::new(),
            key: String::new(),
            configured: false,
        }
    }
}

/// What a packet says about one aircraft.
struct Report<'a> {
    key: AircraftKey,
    traffic: &'a Traffic,
    band: Band,
    freq_hz: f64,
    level_db: f64,
    relayed: bool,
    trusted: bool,
}

impl AdslTracker {
    /// The table that the tracker keeps.
    pub fn store(&self) -> &Store {
        &self.store
    }

    fn stamp(&self, t: f64) -> Stamp {
        match wall(self.live, &self.host_clock) {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        }
    }

    /// Takes one packet with good parity, heard at signal time `t`.
    pub fn take_packet(&mut self, t: f64, p: &AdslPacket) {
        let mut data = p.data.clone();
        // The data is 5 bytes or more and a whole number of words.
        if data.len() < 5 || !data.len().is_multiple_of(4) {
            return;
        }
        let reserved_key = matches!(p.head.key_index, 1 | 2);
        if p.head.key_index == 0 {
            proto::scramble(&mut data, true);
        }
        let head = DataHeader::of(&data);
        // A repair of a packet that is not ADS-L of a known version and payload found a pattern in noise.
        if p.repaired
            && (p.head.version > 1 || reserved_key || !(2..=6).contains(&head.payload_type))
        {
            return;
        }
        self.dirty = true;
        let c = &mut self.store.counters;
        c.good += 1;
        c.per_band[p.band.slot()] += 1;
        self.window_now += 1;
        self.signal = Some(match self.signal {
            Some(s) => s + 0.1 * (p.level_db - s),
            None => p.level_db,
        });
        if p.repaired {
            c.corrected += 1;
        }
        if p.head.signed {
            c.signed += 1;
        }
        if reserved_key {
            c.encrypted += 1;
            return;
        }
        let payload = &data[5..];
        let trusted = !p.repaired;
        match head.payload_type {
            2 if payload.len() >= proto::TRAFFIC_PAYLOAD => {
                c.by_payload[0] += 1;
                let traffic = Traffic::of(payload);
                self.report(
                    t,
                    &Report {
                        key: AircraftKey {
                            source: head.address.amt,
                            address: head.address.address,
                        },
                        traffic: &traffic,
                        band: p.band,
                        freq_hz: p.freq_hz,
                        level_db: p.level_db,
                        relayed: head.relay,
                        trusted,
                    },
                );
            }
            3 if payload.len() >= 7 => {
                c.by_payload[1] += 1;
                let key = AircraftKey {
                    source: head.address.amt,
                    address: head.address.address,
                };
                // A Status payload updates only an aircraft that is in the table.
                if let Some(a) = self.store.aircraft.get_mut(&key) {
                    a.capabilities = Status::of(payload).text();
                }
            }
            4 => {
                c.by_payload[2] += 1;
                for (Address { amt, address }, traffic) in proto::uplink_entries(payload) {
                    self.report(
                        t,
                        &Report {
                            key: AircraftKey {
                                source: amt,
                                address,
                            },
                            traffic: &traffic,
                            band: p.band,
                            freq_hz: p.freq_hz,
                            level_db: p.level_db,
                            relayed: true,
                            trusted,
                        },
                    );
                }
            }
            5 => c.by_payload[3] += 1,
            6 => c.by_payload[4] += 1,
            _ => c.by_payload[5] += 1,
        }
    }

    /// Applies a Traffic payload to its aircraft. A report from a repaired packet never makes a new aircraft.
    fn report(&mut self, t: f64, r: &Report<'_>) {
        if !r.trusted && !self.store.aircraft.contains(&r.key) {
            return;
        }
        let stamp = self.stamp(t);
        let station = self.station;
        let Ok(up) = self
            .store
            .aircraft
            .upsert(&r.key, stamp, || Aircraft::new(r.key))
        else {
            return;
        };
        let (new, a) = (up.new, up.row);
        let tr = r.traffic;
        let was_emergency = a.urgency() != Urgency::None;
        a.category = category_name(tr.category).to_string();
        a.flight_state = proto::FLIGHT_STATES[usize::from(tr.flight_state & 3)].to_string();
        a.emergency = tr.emergency;
        a.alt_m = tr.alt_m;
        a.speed_m_s = tr.speed_m_s;
        a.track_deg = tr.track_deg;
        a.climb_m_s = tr.climb_m_s;
        a.level_db = r.level_db;
        a.channel_hz = r.freq_hz;
        a.channel = Band::CHOICES[r.band.slot()].label.to_string();
        a.relayed = r.relayed;
        a.quality = [tr.nic, tr.hfom, tr.vfom, tr.velocity_accuracy];
        let mut range = None;
        if let Some((lat, lon)) = tr.position {
            let at = Position::new(lat, lon);
            a.position = Some(at);
            // A point too near the last one in time or in distance is left out.
            let _ = a.trail.push(t, at, ());
            if let Some(st) = station {
                let d = st.distance_m(&at);
                a.distance_km = Some(d / 1000.0);
                a.bearing_deg = Some(st.initial_bearing_deg(&at));
                range = Some(d).filter(|d| *d < MAX_RANGE_M);
            }
        }
        if let (false, Some(at)) = (a.announced, a.position) {
            a.announced = true;
            self.events.push(ADSL_AIRCRAFT_SEEN.event(
                t,
                a.channel_hz,
                &AdslAircraftEvent {
                    source: &a.id,
                    address_source: source_name(a.source),
                    category: &a.category,
                    position: (at.lat_deg, at.lon_deg),
                    private: a.private(),
                    family: Family::Aircraft,
                },
            ));
        }
        if new && self.new_logged < 40 {
            self.new_logged += 1;
            self.lines.push(format!(
                "ADS-L: new aircraft {} ({}, {})",
                a.id,
                source_name(a.source),
                a.category
            ));
        }
        if a.urgency() != Urgency::None && !was_emergency {
            let e = ADSL_EMERGENCY.event(
                t,
                a.channel_hz,
                &AdslEmergencyEvent {
                    source: &a.id,
                    text: proto::EMERGENCIES[usize::from(a.emergency & 7)],
                    position: a.position.map(|p| (p.lat_deg, p.lon_deg)),
                    private: a.private(),
                    urgency: Urgency::Emergency,
                    family: Family::Alert,
                },
            );
            self.events.push(e);
        }
        if let Some(d) = range {
            let c = &mut self.store.counters;
            c.max_range_km = c.max_range_km.max(d / 1000.0);
        }
    }

    /// Takes what the FLARM tracker accepted into the aircraft table.
    fn take_flarm(&mut self, now: f64) {
        for p in self.flarm.take_accepted(now, self.expiry_s) {
            let v = &p.value.v6;
            let Some(f) = self.flarm.store().aircraft.get(&DeviceId(v.address)) else {
                continue;
            };
            let f = f.clone();
            let key = AircraftKey {
                source: FLARM_RADIO,
                address: v.address,
            };
            let t = self.clock.epoch + p.time;
            let stamp = self.stamp(t);
            let Ok(up) = self
                .store
                .aircraft
                .upsert(&key, stamp, || Aircraft::new(key))
            else {
                continue;
            };
            self.store.counters.flarm += 1;
            self.dirty = true;
            let a = up.row;
            a.category = FLARM_TYPES[usize::from(f.aircraft_type & 15)].to_string();
            a.flight_state = if f.airborne { "airborne" } else { "on ground" }.to_string();
            a.alt_m = Some(f.alt_m);
            a.speed_m_s = Some(f.speed_m_s);
            a.track_deg = Some(f.track);
            a.climb_m_s = Some(f.climb);
            a.level_db = f.level_db;
            a.channel_hz = f.channel.frequency_hz();
            a.channel = format!("{} FLARM", f.channel.frequency_hz() / 1e6);
            a.distance_km = f.distance_km;
            a.bearing_deg = f.bearing;
            if let Some(at) = f.position {
                a.position = Some(at);
                // A point too near the last one in time or in distance is left out.
                let _ = a.trail.push(t, at, ());
            }
        }
    }

    fn heard_now(&self, now: f64) -> Heard {
        if !self.store.aircraft.is_empty() {
            Heard::Aircraft
        } else if self
            .bands
            .iter()
            .any(|b| b.energy_at.is_some_and(|e| now - self.clock.at(e) <= 5.0))
        {
            Heard::Energy
        } else {
            Heard::Nothing
        }
    }

    fn clear(&mut self) {
        self.store.clear();
        self.flarm.forget();
        self.windows = [0; 4];
        self.window_now = 0;
        self.signal = None;
        self.new_logged = 0;
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
                        Heard::Aircraft => "ADS-L: receiving aircraft",
                        Heard::Energy => "ADS-L: energy on a channel, no good packet lately",
                        Heard::Nothing => "ADS-L: no signal",
                    }
                    .into(),
                );
            }
            self.state = Some(state);
        }
        let s = &self.store;
        let c = &s.counters;
        let n = s.aircraft.len();
        let with_position = s
            .aircraft
            .iter()
            .filter(|r| r.record.position.is_some())
            .count();
        let pps = self.windows.iter().sum::<u64>() as f64;
        let floors: Vec<f64> = self.bands.iter().filter_map(|b| b.noise_dbfs).collect();
        let noise = (!floors.is_empty()).then(|| floors.iter().sum::<f64>() / floors.len() as f64);
        let lock_state = match state {
            Heard::Aircraft => LockState::Locked,
            Heard::Energy => LockState::Tracking,
            Heard::Nothing => LockState::Searching,
        };
        let p = &mut self.published;
        let signal = self.signal;
        let st = &mut self.status;
        let line = if n > 0 {
            format!("{n} aircraft, {pps:.1} packets/s")
        } else if state != Heard::Nothing {
            "signal, no packet yet".to_string()
        } else {
            "listening for ADS-L".to_string()
        };
        st.write(p, lock_state, signal, noise, None, format_args!("{line}"));
        for l in self.lines.drain(..) {
            p.results_mut().log(l);
        }
        let as_count = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        let by = |i: usize| as_count(c.by_payload[i]);
        p.put(&AdslFields {
            frames_ok: as_count(c.good),
            frames_bad: as_count(self.bands.iter().map(|b| b.stats.bad).sum()),
            packets_868_2: as_count(c.per_band[0]),
            packets_868_4: as_count(c.per_band[1]),
            packets_ldr: as_count(c.per_band[2]),
            packets_hdr: as_count(c.per_band[3]),
            on_868_2: self.bands[0].on,
            on_868_4: self.bands[1].on,
            on_ldr: self.bands[2].on,
            on_hdr: self.bands[3].on,
            traffic: by(0),
            status: by(1),
            uplink: by(2),
            fis_b: by(3),
            remote_id: by(4),
            other: by(5),
            encrypted: as_count(c.encrypted),
            signed: as_count(c.signed),
            corrected: as_count(c.corrected),
            syncs: as_count(self.bands.iter().map(|b| b.stats.syncs).sum()),
            noise_dbfs: noise,
            snr_db: signal,
            packets_per_s: pps,
            aircraft: n as i64,
            with_position: with_position as i64,
            max_range_km: c.max_range_km,
            flarm_packets: as_count(c.flarm),
        });
        shaped::rows(&s.aircraft, &ADSL_AIRCRAFT, now, p);
        publish_tracks(
            &s.aircraft,
            |a: &Aircraft| &a.trail,
            p,
            &mut self.key,
            |p, key, aircraft, position, time_s| {
                ADSL_TRACKS.put(
                    p,
                    key,
                    &AircraftTrackRow {
                        aircraft,
                        position,
                        time_s,
                    },
                );
            },
            &ADSL_TRACKS,
        );
        let r = p.results_mut();
        shaped::row_points(&s.aircraft, &ADSL_AIRCRAFT, "aircraft/", r);
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

impl Component for AdslTracker {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "adsl.tracker",
            vec![Port::typed::<AdslItem>("items"), Port::iq("clock")],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(AdslTrackerSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.host_clock = ctx.clock.clone();
        self.station = ctx.station.position;
        self.flarm.attach(ctx);
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(inputs[1].rate);
        self.configured = true;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(1);
        let epoch = self.clock.epoch;
        let mut flarm = false;
        for it in io.items::<AdslItem>(0) {
            match &it.value {
                AdslItem::Packet(p) => self.take_packet(epoch + it.time, p),
                AdslItem::Band(b) => self.bands[b.band.slot()] = *b,
                AdslItem::Flarm(ChannelItem::Frame(f)) => {
                    self.flarm.take_frame(epoch + it.time, f);
                    flarm = true;
                }
                AdslItem::Flarm(ChannelItem::Stats(_)) => {}
            }
        }
        self.clock.samples += n as u64;
        let tick = io.tick();
        if flarm || tick {
            self.take_flarm(self.clock.now());
        }
        if tick {
            let now = self.clock.now();
            self.store.aircraft.expire(now, self.expiry_s);
            self.windows.rotate_left(1);
            self.windows[3] = self.window_now;
            self.window_now = 0;
        }
        if tick || self.dirty {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AdslTrackerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AdslTrackerSetting::ExpiryS => self.expiry_s = EXPIRY_S.read(id, v)?,
            AdslTrackerSetting::WallClock => {
                self.flarm.apply(FlarmTrackerSetting::WallClock.id(), v)?;
            }
            AdslTrackerSetting::StartTime => {
                self.flarm.apply(FlarmTrackerSetting::StartTime.id(), v)?;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let f = self.flarm.values();
        let of = |id: SettingId, or: Value| f.value(id).cloned().unwrap_or(or);
        values_of([
            (AdslTrackerSetting::ExpiryS, Value::F64(self.expiry_s)),
            (
                AdslTrackerSetting::WallClock,
                of(FlarmTrackerSetting::WallClock.id(), Value::Bool(true)),
            ),
            (
                AdslTrackerSetting::StartTime,
                of(FlarmTrackerSetting::StartTime.id(), Value::F64(0.0)),
            ),
        ])
    }
    fn commands(&self) -> &Commands {
        AdslTrackerCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match AdslTrackerCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            AdslTrackerCommand::Clear => self.clear(),
            AdslTrackerCommand::Remove => {
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
