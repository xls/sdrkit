//! `fanet`: the channel at the tuned frequency (`fanet.channels`), the frame log and the station table (`fanet.decode`), and the
//! test signal. The receiver is the shared LoRa bank with one detector: SF7, sync word 0xF1.

use super::{count, show_status};
use crate::components::channels::{Input, Planner};
use crate::gen::{lora_burst, SceneGen, AMP};
use crate::proto::fanet::{self, Address, Frame, Ground, Payload, Service, Tracking};
use crate::typed::{
    FanetDistressEvent, FanetFields, FanetFrameRow, FanetMessageEvent, FanetStationEvent,
    FanetStationRow, FanetTrackRow, FANET_DISTRESS, FANET_FRAMES, FANET_MESSAGE, FANET_STATION,
    FANET_STATIONS, FANET_TRACKS,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Clock, ConfigError, Event, GenConfig, LockState, Record, Results, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port, RateFrom,
    RunContext,
};
use sdrkit_proto_lora_phy::bank::{BankChannel, BankPlan};
use sdrkit_proto_lora_phy::components::BankState;
use sdrkit_proto_lora_phy::modulate::TxFrame;
use sdrkit_proto_lora_phy::params::{fmt_khz, LoraParams};
use sdrkit_proto_lora_phy::rx::{DetectorConfig, LoraFrame};
use sdrkit_util_geo::Position;
use sdrkit_util_objects::commands::CLEAR;
use sdrkit_util_objects::publish::{self, columns};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Spacing, Stamp, Tables, Trail,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// The radio is tuned this far above the channel, so that a 500 kHz channel stays clear of the centre of the capture.
pub const TUNE_OFFSET_HZ: f64 = 600e3;
/// The preamble length that the test signal sends, symbols.
pub const PREAMBLE: u16 = 12;
/// Frames kept in the log.
pub const FRAME_CAP: usize = 500;
/// Stations kept.
pub const STATION_CAP: usize = 500;
const TRAIL_CAP: usize = 200;

sdrkit_core::choice_enum! {
    /// The bandwidth of the channel. The keys are the bandwidths in kHz.
    pub enum Bandwidth {
        /// 125 kHz.
        Bw125 = ("125", "125 kHz"),
        /// 250 kHz.
        Bw250 = ("250", "250 kHz"),
        /// 500 kHz.
        Bw500 = ("500", "500 kHz"),
    }
}

impl Bandwidth {
    /// The bandwidth, Hz.
    pub fn hz(self) -> f64 {
        match self {
            Bandwidth::Bw125 => 125e3,
            Bandwidth::Bw250 => 250e3,
            Bandwidth::Bw500 => 500e3,
        }
    }
}

/// The bandwidth of the channel. The default is the bandwidth of the test signal.
pub const BANDWIDTH: Setting<field::Choice<Bandwidth>> = Setting::choice(
    "bandwidth",
    "Bandwidth",
    Bandwidth::Bw250,
)
.section("LoRa")
.help(
    "The bandwidth that the stations of your area use. The default is the one of the test signal.",
);
/// The silence after which a station leaves the table.
pub const EXPIRY_S: Setting<field::F64> =
    Setting::new("expiry_s", "Forget stations after", "s", 600.0)
        .section("Receive")
        .range(30.0, 86400.0, 1.0);
/// The position that the stations of the test signal are around.
pub const GEN_CENTRE: Setting<field::Position> =
    Setting::new("gen_centre", "Test signal centre", "", (54.70, 25.30))
        .section("Test signal")
        .advanced();
/// The stations of the test signal.
pub const GEN_STATIONS: Setting<field::I64> =
    Setting::new("gen_stations", "Test signal stations", "", 5)
        .range(1.0, 20.0, 1.0)
        .section("Test signal")
        .advanced();
/// Every n-th frame of the test signal has a wrong CRC (0: none).
pub const GEN_BAD_CRC: Setting<field::I64> =
    Setting::new("gen_bad_crc", "Wrong CRC in every n-th frame", "", 0)
        .range(0.0, 100.0, 1.0)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of `fanet.channels`.
    pub enum FanetPlanSetting {
        /// Bandwidth.
        Bandwidth = BANDWIDTH,
    }
}

sdrkit_core::settings! {
    /// The settings of `fanet.decode`.
    pub enum FanetDecodeSetting {
        /// Forget stations after.
        ExpiryS = EXPIRY_S,
    }
}

sdrkit_core::settings! {
    /// The settings of `fanet`.
    pub enum FanetSetting {
        /// Bandwidth.
        Bandwidth = BANDWIDTH,
        /// Forget stations after.
        ExpiryS = EXPIRY_S,
        /// Test signal centre.
        GenCentre = GEN_CENTRE,
        /// Test signal stations.
        GenStations = GEN_STATIONS,
        /// Wrong CRC in every n-th frame.
        GenBadCrc = GEN_BAD_CRC,
    }
}

/// The plan of `fanet` as an item (`fanet.plan`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FanetPlan {
    /// The bank.
    pub bank: BankPlan,
    /// The channel in words.
    pub channel: String,
    /// The channel is outside the capture.
    pub outside: bool,
}

impl Item for FanetPlan {
    const KIND: &'static str = "fanet.plan";
}

/// The planner of `fanet`: one channel where the session is tuned, with the bandwidth of the setting.
pub struct FanetPlanner {
    bw: Bandwidth,
}

impl Default for FanetPlanner {
    fn default() -> Self {
        FanetPlanner {
            bw: Bandwidth::Bw250,
        }
    }
}

impl Planner for FanetPlanner {
    type Item = FanetPlan;
    type Setting = FanetPlanSetting;
    const NAME: &'static str = "fanet.channels";

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<bool, SettingError> {
        match FanetPlanSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FanetPlanSetting::Bandwidth => self.bw = BANDWIDTH.read(id, v)?,
        }
        Ok(false)
    }

    fn values(&self) -> Values {
        values_of([(FanetPlanSetting::Bandwidth, Value::Choice(self.bw.index()))])
    }

    /// One channel where the user tuned (the centre of the capture when that is not known).
    fn plan(&self, input: &Input) -> (BankPlan, FanetPlan) {
        let bw = self.bw.hz();
        let dial = input.dial();
        let mut d = DetectorConfig::new(fanet::SF, bw, fanet::SYNC);
        d.preamble = 8;
        let bank = BankPlan {
            channels: vec![BankChannel {
                freq_hz: dial,
                bw,
                detectors: vec![d],
            }],
            fft_from: 0,
        };
        let outside =
            !sdrkit_proto_lora_phy::channel::in_band(input.rate, dial - input.centre_hz, bw);
        let item = FanetPlan {
            bank: bank.clone(),
            channel: format!(
                "{} MHz, {} kHz",
                crate::mesh::plan::fmt_mhz(dial),
                fmt_khz(bw)
            ),
            outside,
        };
        (bank, item)
    }
}

sdrkit_core::choice_enum! {
    /// What a frame is.
    #[derive(Serialize, Deserialize)]
    pub enum FrameKind {
        /// Type 1.
        Tracking = ("tracking", "Tracking"),
        /// Type 7.
        Ground = ("ground", "Ground"),
        /// Type 2.
        Name = ("name", "Name"),
        /// Type 3.
        Message = ("message", "Message"),
        /// Type 4.
        Service = ("service", "Service"),
        /// Type 9.
        Thermal = ("thermal", "Thermal"),
        /// Any other type.
        Other = ("other", "Other"),
    }
}

sdrkit_core::choice_enum! {
    /// What a station is, from the frames it sends.
    #[derive(Serialize, Deserialize)]
    pub enum StationKind {
        /// It sends tracking frames.
        Aircraft = ("aircraft", "Aircraft"),
        /// It sends ground tracking frames.
        Person = ("person", "Person"),
        /// It sends service frames.
        Weather = ("weather", "Weather station"),
        /// Nothing with a position yet.
        Other = ("other", "Other"),
    }
}

/// One row of the frame log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRow {
    /// Signal time, seconds.
    pub time: f64,
    /// What the frame is.
    pub kind: FrameKind,
    /// The sender.
    pub id: String,
    /// The addressee of a unicast frame, else empty.
    pub to: String,
    /// The frame in one line.
    pub decoded: String,
    /// Latitude and longitude, degrees.
    pub position: Option<(f64, f64)>,
    /// The level of the frame, dB.
    pub level_db: f64,
    /// The SNR, dB.
    pub snr_db: f64,
}

impl Shift for FrameRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for FrameRow {
    type Row = FanetFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&FanetFrameRow {
            time_s: self.time,
            kind: self.kind,
            source: &self.id,
            target: &self.to,
            decoded: &self.decoded,
            position: self.position,
            level_db: self.level_db,
            snr_db: self.snr_db,
        });
    }
}

/// One station of the table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationRow {
    /// The id "MM:DDDD".
    pub id: String,
    /// The name, from a name frame.
    pub name: String,
    /// What the station is.
    pub kind: StationKind,
    /// The aircraft type, from a tracking frame.
    pub aircraft: Option<u8>,
    /// The last position.
    pub position: Option<Position>,
    /// The altitude, metres.
    pub alt_m: Option<f64>,
    /// The speed, km/h.
    pub speed_kmh: Option<f64>,
    /// The climb, m/s.
    pub climb_m_s: Option<f64>,
    /// The heading, degrees.
    pub heading_deg: Option<f64>,
    /// The last weather values, in words.
    pub weather: String,
    /// The track of positions.
    pub trail: Trail,
}

impl StationRow {
    fn new(id: String) -> Self {
        StationRow {
            id,
            name: String::new(),
            kind: StationKind::Other,
            aircraft: None,
            position: None,
            alt_m: None,
            speed_kmh: None,
            climb_m_s: None,
            heading_deg: None,
            weather: String::new(),
            trail: Trail::new(TRAIL_CAP, Spacing::seconds(2.0)),
        }
    }
}

impl Shift for StationRow {
    fn shift_times(&mut self, by_s: f64) {
        self.trail.shift_times(by_s);
    }
}

impl Shaped for StationRow {
    type Row = FanetStationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&FanetStationRow {
            source: &self.id,
            name: &self.name,
            kind: self.kind,
            aircraft: self
                .aircraft
                .map(|a| fanet::AIRCRAFT_TYPES[usize::from(a & 7)]),
            position: self.position.map(|p| (p.lat_deg, p.lon_deg)),
            alt_m: self.alt_m,
            speed_kt: self.speed_kmh.map(|v| v / 1.852),
            climb_m_s: self.climb_m_s,
            course_deg: self.heading_deg,
            weather: &self.weather,
            heard: columns::heard(seen),
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// What `fanet.decode` keeps across sessions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heard {
    /// The frame log, oldest first.
    pub frames: LogRing<FrameRow>,
    /// The stations by address.
    pub stations: ObjectTable<Address, StationRow>,
    /// Good frames.
    pub good: u64,
    /// Bad frames.
    pub bad: u64,
}

impl Default for Heard {
    fn default() -> Self {
        Heard {
            frames: LogRing::new(FRAME_CAP),
            stations: ObjectTable::new(STATION_CAP),
            good: 0,
            bad: 0,
        }
    }
}

impl Shift for Heard {
    fn shift_times(&mut self, by_s: f64) {
        self.frames.shift_times(by_s);
        self.stations.shift_times(by_s);
    }
}

impl Tables for Heard {
    const FORMAT: StateFormat = StateFormat::new("lora.fanet", 1);
    fn clear(&mut self) {
        self.frames.clear();
        self.stations.clear();
        self.good = 0;
        self.bad = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.frames.adopt(saved.frames);
        self.stations.adopt(saved.stations);
        self.good = saved.good;
        self.bad = saved.bad;
    }
}

sdrkit_core::commands! {
    /// The commands of `fanet.decode`.
    pub enum FanetCommand {
        /// Forgets the tables and the counters.
        Clear = CLEAR,
    }
}

/// The `fanet.decode` component.
pub struct FanetDecode {
    /// What was heard.
    pub heard: Heard,
    p: Published,
    plan: FanetPlan,
    expiry_s: f64,
    now: f64,
    level: Option<f64>,
    preambles: u64,
    last_signal: f64,
    last_good: f64,
    last: Option<(f64, f64)>,
    live: bool,
    clock: Clock,
    events: Vec<Event>,
    text: String,
    points: Vec<String>,
}

impl Default for FanetDecode {
    fn default() -> Self {
        FanetDecode {
            heard: Heard::default(),
            p: crate::typed::results("fanet"),
            plan: FanetPlan::default(),
            expiry_s: EXPIRY_S.default,
            now: 0.0,
            level: None,
            preambles: 0,
            last_signal: -1e9,
            last_good: -1e9,
            last: None,
            live: false,
            clock: Clock::fixed(0.0),
            events: Vec::new(),
            text: String::new(),
            points: Vec::new(),
        }
    }
}

/// The prefix of the map point ids of a station kind.
fn point_prefix(kind: StationKind) -> &'static str {
    match kind {
        StationKind::Aircraft => "aircraft/",
        StationKind::Person => "people/",
        StationKind::Weather => "weather/",
        StationKind::Other => "stations/",
    }
}

impl FanetDecode {
    fn stamp(&self, t: f64) -> Stamp {
        if self.live {
            Stamp::at(t).with_wall(self.clock.now())
        } else {
            Stamp::at(t)
        }
    }

    /// Takes the payload of one LoRa frame with a good CRC, heard at signal time `t` on `freq_hz`.
    pub fn take(&mut self, t: f64, freq_hz: f64, payload: &[u8], level_db: f64, snr_db: f64) {
        let Ok(f) = fanet::parse(payload) else {
            self.heard.bad += 1;
            self.last_signal = t;
            return;
        };
        self.heard.good += 1;
        self.last_good = t;
        self.last_signal = t;
        let id = f.source.to_string();
        let to = f
            .extension
            .and_then(|e| e.destination)
            .map(|d| d.to_string())
            .unwrap_or_default();
        self.heard.frames.push(FrameRow {
            time: t,
            kind: frame_kind(&f),
            id: id.clone(),
            to: to.clone(),
            decoded: f.line(),
            position: f.position(),
            level_db,
            snr_db,
        });
        let stamp = self.stamp(t);
        let Ok(up) = self
            .heard
            .stations
            .upsert(&f.source, stamp, || StationRow::new(id.clone()))
        else {
            return;
        };
        if up.new {
            self.events.push(FANET_STATION.event(
                t,
                freq_hz,
                &FanetStationEvent {
                    source: &id,
                    family: Family::Aircraft,
                },
            ));
        }
        let s = up.row;
        let at = f
            .position()
            .map(|(lat, lon)| Position::new(lat, lon))
            .filter(|p| p.validate().is_ok());
        if let Some(p) = at {
            s.position = Some(p);
            // A point too near the last one in time is left out.
            let _ = s.trail.push(t, p, ());
        }
        match &f.payload {
            Payload::Tracking(k) => {
                s.kind = StationKind::Aircraft;
                s.aircraft = Some(k.aircraft);
                s.alt_m = Some(k.alt_m);
                s.speed_kmh = Some(k.speed_kmh);
                s.climb_m_s = Some(k.climb_m_s);
                s.heading_deg = Some(k.heading_deg);
            }
            Payload::Ground(g) => {
                if s.kind != StationKind::Aircraft {
                    s.kind = StationKind::Person;
                }
                if g.distress() {
                    self.events.push(FANET_DISTRESS.event(
                        t,
                        freq_hz,
                        &FanetDistressEvent {
                            source: &id,
                            text: fanet::ground_state(g.state),
                            position: at.map(|p| (p.lat_deg, p.lon_deg)),
                            urgency: Urgency::Urgent,
                            family: Family::Alert,
                        },
                    ));
                }
            }
            Payload::Service(_) => {
                if s.kind == StationKind::Other {
                    s.kind = StationKind::Weather;
                }
                let line = f.line();
                s.weather = line
                    .strip_prefix("service, ")
                    .unwrap_or_default()
                    .to_string();
            }
            Payload::Name(n) => s.name.clone_from(n),
            Payload::Message(_, m) => self.events.push(FANET_MESSAGE.event(
                t,
                freq_hz,
                &FanetMessageEvent {
                    source: &id,
                    target: &to,
                    text: m,
                    family: Family::Message,
                },
            )),
            _ => {}
        }
    }

    fn publish(&mut self) {
        let h = &self.heard;
        let state = if self.now - self.last_good < 8.0 {
            LockState::Locked
        } else if self.now - self.last_signal < 2.0 {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        let t = &mut self.text;
        t.clear();
        if h.good > 0 {
            let _ = write!(t, "{} frames", h.good);
        } else {
            t.push_str("listening for FANET");
        }
        let _ = write!(t, ", {}", self.plan.channel);
        if self.plan.outside {
            t.push_str(", outside the capture");
        }
        let p = &mut self.p;
        show_status(
            p,
            state,
            self.last.map(|l| l.0),
            self.last.map(|l| l.1),
            self.level,
            t,
        );
        p.put(&FanetFields {
            frames_ok: count(h.good),
            frames_bad: count(h.bad),
            snr_db: self.last.map(|l| l.0),
            offset_hz: self.last.map(|l| Hertz(l.1)),
            level_dbfs: self.level,
            listening: &self.plan.channel,
            stations_heard: count(h.stations.len()),
        });
        shaped::log_rows(&h.frames, &FANET_FRAMES, self.now, p);
        shaped::rows(&h.stations, &FANET_STATIONS, self.now, p);
        let mut tracks = 0;
        for r in h.stations.iter() {
            for pt in r.record.trail.points() {
                t.clear();
                let _ = write!(t, "{}/{:.3}", r.key, pt.t);
                FANET_TRACKS.put(
                    p,
                    t,
                    &FanetTrackRow {
                        station: &r.record.id,
                        position: (pt.position.lat_deg, pt.position.lon_deg),
                        time_s: pt.t,
                    },
                );
                tracks += 1;
            }
        }
        publish::keep_newest_rows(&FANET_TRACKS, tracks, p);
        // The map points: one for each station with a position, under the prefix of its kind.
        let res = p.results_mut();
        let mut now: Vec<String> = Vec::new();
        for r in h.stations.iter() {
            let s = r.record;
            let Some(at) = s.position else {
                continue;
            };
            let id = format!("{}{}", point_prefix(s.kind), s.id);
            let label = if s.name.is_empty() { &s.id } else { &s.name };
            res.update_point(
                &id,
                at.lat_deg,
                at.lon_deg,
                label,
                s.heading_deg.map(|h| h as f32),
            );
            now.push(id);
        }
        for gone in self.points.iter().filter(|k| !now.contains(k)) {
            res.remove_point(gone);
        }
        self.points = now;
    }
}

fn frame_kind(f: &Frame) -> FrameKind {
    match f.payload {
        Payload::Tracking(_) => FrameKind::Tracking,
        Payload::Ground(_) => FrameKind::Ground,
        Payload::Name(_) => FrameKind::Name,
        Payload::Message(..) => FrameKind::Message,
        Payload::Service(_) => FrameKind::Service,
        Payload::Thermal(_) | Payload::Other(9, _) => FrameKind::Thermal,
        _ => FrameKind::Other,
    }
}

impl Component for FanetDecode {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fanet.decode",
            vec![
                Port::typed::<LoraFrame>("frames"),
                Port::typed::<BankState>("state"),
                Port::typed::<FanetPlan>("plan"),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(FanetDecodeSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.clock = ctx.clock.clone();
    }

    fn process(&mut self, io: &mut Io) {
        if let Some(pl) = io.items::<FanetPlan>(2).last() {
            if pl.value.channel != self.plan.channel {
                let line = format!("FANET: listening on {}", pl.value.channel);
                self.p.results_mut().log(line);
            }
            self.plan = pl.value.clone();
        }
        for s in io.items::<BankState>(1) {
            self.now = s.value.time;
            self.level = Some(s.value.level_db);
            let n: u64 = s.value.detectors.iter().map(|d| d.stats.preambles).sum();
            if n > self.preambles {
                self.last_signal = s.value.time;
            }
            self.preambles = n;
        }
        for f in io.items::<LoraFrame>(0) {
            let fr = &f.value;
            if !(fr.crc_present && fr.crc_ok) {
                self.heard.bad += 1;
                self.last_signal = f.time;
                continue;
            }
            let before = self.heard.good;
            self.take(
                f.time,
                fr.channel_hz + fr.cfo_hz,
                &fr.payload,
                fr.level_db,
                fr.snr_db,
            );
            if self.heard.good > before {
                self.last = Some((fr.snr_db, fr.cfo_hz));
            }
        }
        if io.tick() {
            self.heard.stations.expire(self.now, self.expiry_s);
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FanetDecodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FanetDecodeSetting::ExpiryS => self.expiry_s = EXPIRY_S.read(id, v)?,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([(FanetDecodeSetting::ExpiryS, Value::F64(self.expiry_s))])
    }

    fn commands(&self) -> &Commands {
        FanetCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match FanetCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            FanetCommand::Clear => {
                self.heard.clear();
                self.last = None;
                self.last_good = -1e9;
                self.p.clear_rows(&FANET_FRAMES);
                self.p.clear_rows(&FANET_STATIONS);
                self.p.clear_rows(&FANET_TRACKS);
            }
        }
        self.publish();
        Ok(None)
    }

    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }

    fn forget(&mut self, kept: &Kept) {
        let h = &mut self.heard;
        if !kept.table(FANET_FRAMES.key) {
            h.frames.clear();
            self.p.clear_rows(&FANET_FRAMES);
        }
        if !kept.table(FANET_STATIONS.key) {
            h.stations.clear();
            self.p.clear_rows(&FANET_STATIONS);
            self.p.clear_rows(&FANET_TRACKS);
        }
        if !kept.field("frames_ok") {
            h.good = 0;
        }
        if !kept.field("frames_bad") {
            h.bad = 0;
        }
        self.level = None;
        self.last = None;
        self.last_signal = -1e9;
        self.last_good = -1e9;
        self.publish();
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.heard.save_state(self.now).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let now = self.now;
        self.heard.restore_state(state, now)?;
        self.publish();
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The test signal.

/// The LoRa setting of a FANET sender at bandwidth `bw_hz`: SF7, 4/5, preamble 12, sync 0xF1, CRC on.
pub fn tx_params(bw_hz: f64) -> LoraParams {
    let mut p = LoraParams::new(fanet::SF, bw_hz, 5, PREAMBLE, fanet::SYNC);
    p.crc = true;
    p
}

/// The frames of cycle `cycle` (5 s) of the test scene: (seconds into the cycle, payload). Stations 0 and 1 are paragliders,
/// 2 a glider, 3 a ground tracker, 4 a weather station; more stations are paragliders.
pub fn scene_frames(cycle: u32, centre: (f64, f64), stations: usize) -> Vec<(f64, Vec<u8>)> {
    let mut v = Vec::new();
    let t = f64::from(cycle) * 5.0;
    let addr = |i: usize| Address {
        manufacturer: [0x11, 0x01, 0x20, 0x07, 0xFB][i % 5],
        device: 0x1001 + i as u16,
    };
    let mut at = 0.1;
    for i in 0..stations {
        let a = addr(i);
        let ang = t / 60.0 + i as f64 * 1.3;
        let r = 0.02 + 0.008 * i as f64;
        let (lat, lon) = (centre.0 + r * ang.cos(), centre.1 + 1.7 * r * ang.sin());
        let frame = match i {
            3 => fanet::build_ground(
                a,
                &Ground {
                    lat,
                    lon,
                    state: 1,
                    online: true,
                },
            ),
            4 => {
                if !cycle.is_multiple_of(2) {
                    continue;
                }
                fanet::build_service(
                    a,
                    &Service {
                        gateway: true,
                        position: Some((centre.0 + 0.05, centre.1 - 0.05)),
                        temperature_c: Some(18.5),
                        wind: Some((225.0, 12.0, 19.0)),
                        humidity_pct: Some(56.0),
                        pressure_hpa: Some(1012.5),
                        ..Service::default()
                    },
                )
            }
            _ => fanet::build_tracking(
                a,
                &Tracking {
                    lat,
                    lon,
                    online: true,
                    aircraft: if i == 2 { 4 } else { 1 },
                    alt_m: 900.0 + 150.0 * i as f64 + 20.0 * (t / 30.0).sin(),
                    speed_kmh: if i == 2 { 95.0 } else { 34.0 + 2.0 * i as f64 },
                    climb_m_s: 1.2 - 0.6 * i as f64,
                    heading_deg: (ang.to_degrees() + 90.0).rem_euclid(360.0),
                    turn_deg_s: None,
                    qne_offset_m: None,
                },
            ),
        };
        v.push((at, frame));
        at += 0.2;
    }
    // A name every fourth cycle (20 s) from each station in turn, and one message.
    let names = ["Asta", "Jonas", "Glider LY", "Retrieve", "Hill top"];
    let who = (cycle as usize / 4) % stations.max(1);
    if cycle.is_multiple_of(4) {
        v.push((at, fanet::build_name(addr(who), names[who % names.len()])));
        at += 0.2;
    }
    if cycle % 4 == 1 && stations > 1 {
        v.push((
            at,
            fanet::build_message(addr(0), Some(addr(1)), "landing at the north field"),
        ));
    }
    v
}

/// The test signal of `fanet`: the scene at the bandwidth `bw_hz`, the channel [`TUNE_OFFSET_HZ`] below the centre
/// of the samples. Every `bad_crc`-th frame has a wrong CRC (0: none).
pub fn generator(
    rate: f64,
    bw_hz: f64,
    centre: (f64, f64),
    stations: usize,
    bad_crc: u32,
) -> SceneGen {
    let p = tx_params(bw_hz);
    let mut sent = 0u32;
    SceneGen::new(
        rate,
        5.0,
        SignalInfo::new(bw_hz),
        Box::new(move |cycle, t0, bursts| {
            for (at, payload) in scene_frames(cycle, centre, stations) {
                sent += 1;
                let offset = -TUNE_OFFSET_HZ + 900.0;
                if bad_crc > 0 && sent.is_multiple_of(bad_crc) {
                    if let Ok(mut f) = TxFrame::new(&p, &payload) {
                        spoil(&mut f);
                        bursts.push(f.at(t0 + at).carrier(offset).amplitude(AMP).burst(rate));
                    }
                } else {
                    bursts.extend(lora_burst(&p, &payload, t0 + at, offset, AMP, 0.0, rate));
                }
            }
        }),
    )
}

/// Changes the last eight data symbols of `f`, so that its CRC fails and no repair finds the frame.
pub fn spoil(f: &mut TxFrame) {
    for s in f.symbols.iter_mut().rev().take(8) {
        *s ^= 0x15;
    }
}

/// The generator of `fanet` from a configuration whose settings follow [`FanetSetting`].
pub fn from_config(cfg: &GenConfig) -> SceneGen {
    let s = &cfg.settings;
    generator(
        cfg.rate_hz,
        s.get(&BANDWIDTH).hz(),
        s.get(&GEN_CENTRE),
        usize::try_from(s.get(&GEN_STATIONS)).unwrap_or(5),
        u32::try_from(s.get(&GEN_BAD_CRC)).unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bandwidth_fits_the_capture_of_the_stack() {
        for bw in [125e3, 250e3, 500e3] {
            assert!(sdrkit_proto_lora_phy::channel::in_band(
                crate::FANET.tuning.rate_hz,
                -TUNE_OFFSET_HZ,
                bw
            ));
        }
        assert_eq!(crate::FANET.tuning.tune_offset_hz, TUNE_OFFSET_HZ);
    }
}
