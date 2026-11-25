//! DJI drones (`dji_droneid`): shape detection of wideband bursts (tracks and classes, a guess from the shape) and DroneID
//! decoding (the clear broadcast of OcuSync 2 drones). Nothing is decrypted.

pub mod burst;
pub mod droneid;
pub mod generator;

use crate::family::{min_rate_message, publish_tracks, wall, SignalClock, StatusLine, REPORT_S};
use crate::typed::{
    DjiDroneidBurstRow, DjiDroneidDroneRow, DjiDroneidEvent, DjiDroneidFields, DjiDroneidSignalRow,
    DroneTrackRow, DJI_BURSTS, DJI_DRONEID, DJI_DRONES, DJI_SIGNALS, DJI_TRACKS, DRONE_REMOVE,
    DRONE_ROW, LEVEL_SERIES,
};
use burst::{BurstAnalyser, BurstSignal, DetectorStats};
use droneid::{BurstReceiver, DroneIdPacket};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::results::Series;
use sdrkit_core::{
    Clock, ConfigError, Event, LockState, Record, Results, ResultsDecl, SeriesId, Setting,
    SettingError, SettingId, SettingSet, StackConfig, StateBlob, StateError, TableId, Value,
    Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Kind, Port, RateFrom, RunContext, StackGraph, Timed,
};
use sdrkit_util_geo::{Position, KNOT_M_S};
use sdrkit_util_objects::commands::{self, Tables};
use sdrkit_util_objects::publish::keep_newest_rows;
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Spacing, Stamp, Trail,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 16e6;
/// Frequencies reported for DroneID, MHz.
pub const DRONEID_FREQS: [f64; 8] = [
    2399.5, 2414.5, 2429.5, 2444.5, 2459.5, 5756.5, 5776.5, 5796.5,
];
pub(crate) const TRACK_CAP: usize = 24;
const OBS: usize = 64;
const LEVELS: usize = 240;
pub(crate) const BURST_LOG: usize = 300;
pub(crate) const DRONE_CAP: usize = 50;
pub(crate) const TRAIL_CAP: usize = 120;

sdrkit_core::choice_enum! {
    /// The signals of the test signal.
    pub enum GenScenario {
        /// Every shape and DroneID.
        Everything = ("0", "everything"),
        /// DroneID only.
        DroneIdOnly = ("1", "DroneID only"),
        /// OcuSync-like bursts only.
        OcuSyncOnly = ("2", "OcuSync-like only"),
        /// Lightbridge-like bursts only.
        LightbridgeOnly = ("3", "Lightbridge-like only"),
        /// Wi-Fi-like bursts only.
        WifiOnly = ("4", "Wi-Fi-like only"),
        /// DroneID with colliding bursts.
        Collisions = ("5", "DroneID with collisions"),
    }
}

/// Try DroneID decoding.
pub const DECODE_DRONEID: Setting<field::Bool> =
    Setting::new("decode_droneid", "Decode DroneID", "", true).help(
        "Try every 0.5 to 1.5 ms burst that is 7 to 11 MHz wide as a DroneID burst: Zadoff-Chu synchronisation, turbo \
         decoding, CRC-24.",
    );
/// Stamp rows with the wall clock.
pub const WALL_CLOCK: Setting<field::Bool> =
    Setting::new("wall_clock", "Stamp with this computer's clock", "", true).help(
        "Stamp rows and bursts of live samples with the computer's clock as well as signal time.",
    );
/// The signals of the test signal.
pub const GEN_SCENARIO: Setting<field::Choice<GenScenario>> =
    Setting::choice("gen_scenario", "Test signal", GenScenario::Everything)
        .section("Test signal")
        .advanced();
/// The DroneID period of the test signal.
pub const GEN_PERIOD_S: Setting<field::F64> =
    Setting::new("gen_period_s", "DroneID period", "s", 0.64)
        .range(0.01, 10.0, 0.01)
        .section("Test signal")
        .advanced();
/// 8-symbol DroneID bursts in the test signal.
pub const GEN_EIGHT_SYMBOLS: Setting<field::Bool> =
    Setting::new("gen_eight_symbols", "8-symbol DroneID", "", false)
        .section("Test signal")
        .advanced();
/// 20 MHz OcuSync-like bursts in the test signal.
pub const GEN_WIDE: Setting<field::Bool> =
    Setting::new("gen_wide", "20 MHz OcuSync-like", "", false)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `dji_droneid` stack.
    pub enum DjiSetting {
        /// Try DroneID decoding.
        DecodeDroneId = DECODE_DRONEID,
        /// Stamp rows with the wall clock.
        WallClock = WALL_CLOCK,
        /// The signals of the test signal.
        GenScenario = GEN_SCENARIO,
        /// The DroneID period of the test signal.
        GenPeriodS = GEN_PERIOD_S,
        /// 8-symbol DroneID bursts.
        GenEightSymbols = GEN_EIGHT_SYMBOLS,
        /// 20 MHz OcuSync-like bursts.
        GenWide = GEN_WIDE,
    }
}

sdrkit_core::settings! {
    /// The settings of [`DjiTracker`].
    pub enum DjiTrackerSetting {
        /// Try DroneID decoding.
        DecodeDroneId = DECODE_DRONEID,
        /// Stamp rows with the wall clock.
        WallClock = WALL_CLOCK,
    }
}

sdrkit_core::commands! {
    /// The commands of [`DjiTracker`].
    pub enum DjiTrackerCommand {
        /// Forgets the drones, the tracks, the burst log and the counters.
        Clear = commands::CLEAR,
        /// Forgets one drone.
        Remove = DRONE_REMOVE.command,
    }
}

/// The DJI receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among
/// them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("bursts", BurstComponent::default())
        .add("tracker", DjiTracker::default())
        .input("bursts")
        .input("tracker.clock")
        .connect("bursts", "tracker.signals")
        .output("tracker.events")
        .clock(REPORT_S)
        .status_from("tracker")
        .reconfigure_on_retune()
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape("dji_droneid"), graph)
        .min_rate(MIN_RATE_HZ, &min_rate_message("DJI drones", MIN_RATE_HZ))
        .route(DECODE_DRONEID.key(), &["tracker.decode_droneid"])?
        .route(WALL_CLOCK.key(), &["tracker.wall_clock"])?
        .command("clear", sdrkit_graph::CommandTarget::of("tracker", "clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("tracker", "remove"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// The output of the burst component.
#[derive(Clone, Debug)]
pub enum BurstItem {
    /// A measured signal.
    Signal(BurstSignal),
    /// The detector's counters, at each report.
    Stats(DetectorStats),
}

impl Item for BurstItem {
    const KIND: &'static str = "burst.signal";
}

/// The burst component (`drones.dji_bursts`): the wideband burst detector and the measurement of each burst.
#[derive(Default)]
pub struct BurstComponent {
    a: Option<BurstAnalyser>,
    tmp: Vec<BurstSignal>,
}

impl Component for BurstComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "drones.dji_bursts",
            vec![Port::iq("in")],
            vec![Port::typed::<BurstItem>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A new rate or centre: the detector starts again (floor, idle spectrum). The tracker keeps the counters.
        self.a = Some(BurstAnalyser::new(inputs[0].rate));
        Ok(vec![Format::new(Kind::typed::<BurstItem>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let Some(a) = self.a.as_mut() else { return };
        self.tmp.clear();
        a.feed(x, &mut self.tmp);
        let st = io.tick().then(|| a.stats());
        let o = io.items_out::<BurstItem>(0);
        for s in self.tmp.drain(..) {
            o.push(Timed {
                time: s.start_s,
                value: BurstItem::Signal(s),
            });
        }
        if let Some(s) = st {
            o.push(Timed {
                time: 0.0,
                value: BurstItem::Stats(s),
            });
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
}

sdrkit_core::choice_enum! {
    /// The classes of a signal track, in rank order.
    #[derive(PartialOrd, Ord, Serialize, Deserialize)]
    pub enum Class {
        /// Not OFDM-like.
        OtherWideband = ("other_wideband", "other wideband (not OFDM-like)"),
        /// OFDM-like, nothing more to tell.
        UnknownOfdm = ("unknown_ofdm", "unknown wideband OFDM"),
        /// OFDM bursts every 0.3 to 25 ms.
        OcuSyncLike = ("ocusync_like", "DJI OcuSync-like"),
        /// OFDM bursts every 1 ms.
        LightbridgeLike = ("lightbridge_like", "DJI Lightbridge-like"),
        /// The DroneID ZC symbols were seen.
        DroneId = ("droneid", "DJI DroneID"),
    }
}

impl Class {
    /// The name shown.
    pub fn name(self) -> &'static str {
        let all: &'static [sdrkit_core::ChoiceDecl] = <Class as sdrkit_core::Choice>::CHOICES;
        &all[usize::from(sdrkit_core::Choice::index(self))].label
    }
}

sdrkit_core::choice_enum! {
    /// Why a track has its class.
    pub enum Reason {
        /// A DroneID packet passed its CRC.
        Decoded = ("decoded", "DroneID packet decoded (the CRC-24 passed): an identification, not a guess"),
        /// The DroneID symbols were found, no packet passed its CRC yet.
        ZcOnly = ("zc_only", "the two DroneID Zadoff-Chu symbols were found; the packet did not pass its CRC yet"),
        /// OFDM bursts every millisecond.
        EveryMs = ("every_ms", "OFDM bursts every 1 ms, 10 or 20 MHz wide"),
        /// Periodic OFDM bursts in the OcuSync range.
        Periodic = ("periodic", "periodic OFDM bursts, 10 or 20 MHz wide, 15 kHz-type spacing"),
        /// A period outside the OcuSync and Lightbridge range.
        OtherPeriod = ("other_period", "OFDM bursts with a period outside the OcuSync / Lightbridge range"),
        /// OFDM-like but not regular.
        Irregular = ("irregular", "wideband OFDM-like bursts, not regular enough to tell more"),
        /// Fewer than three bursts.
        TooFew = ("too_few", "not enough bursts yet"),
        /// Not 15 or 30 kHz OFDM.
        NotOfdm = ("not_ofdm", "wideband bursts that do not look like 15 / 30 kHz OFDM (Wi-Fi and the like)"),
    }
}

sdrkit_core::choice_enum! {
    /// The bands a signal track can be in.
    pub enum RadioBand {
        /// 2400 to 2483.5 MHz.
        B2g4 = ("2g4", "2.4 GHz"),
        /// 5725 to 5850 MHz.
        B5g8 = ("5g8", "5.8 GHz"),
        /// 5150 to 5350 MHz.
        B5g2 = ("5g2", "5.2 GHz"),
    }
}

sdrkit_core::choice_enum! {
    /// The result of a DroneID attempt on a burst.
    #[derive(Serialize, Deserialize)]
    pub enum Attempt {
        /// The burst was not tried.
        NotTried = ("not_tried", "not tried"),
        /// No DroneID symbols.
        NoDroneId = ("no_droneid", "no DroneID"),
        /// The symbols were found, the CRC failed.
        CrcFailed = ("crc_failed", "CRC failed"),
        /// A packet was decoded.
        Decoded = ("decoded", "decoded"),
    }
}

/// What `dji_droneid` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("dji_droneid").results_decl()
}

/// One signal track.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    /// Number (from 1).
    pub id: u32,
    /// Offset from the centre, Hz.
    pub offset_hz: f64,
    /// RF frequency, Hz (0 when the centre is unknown).
    pub freq_hz: f64,
    /// Width, Hz.
    pub width_hz: f64,
    /// The last burst was clipped.
    pub clipped: bool,
    /// The last burst's level, dBFS.
    pub level_dbfs: f64,
    /// The last burst's SNR, dB.
    pub snr_db: f64,
    /// Bursts seen.
    pub bursts: u64,
    /// Last heard, signal time.
    pub last_t: f64,
    /// Observations: (start, duration, OFDM-likeness).
    pub obs: VecDeque<(f64, f64, f64)>,
    /// Level history: (time, strongest level in the report period).
    pub levels: VecDeque<(f64, f64)>,
    /// Period, s (median gap).
    pub period_s: Option<f64>,
    /// Regularity (0..1).
    pub regularity: f64,
    /// Duty cycle (0..1).
    pub duty: f64,
    /// Median duration, s.
    pub burst_s: f64,
    /// Median OFDM-likeness.
    pub ofdm: f64,
    /// The class.
    pub class: Class,
    /// Its confidence.
    pub confidence: f64,
    /// Why it has its class.
    pub reason: Reason,
    /// Within 3 MHz of a frequency reported for DroneID, for a 10 MHz OFDM track that is not DroneID.
    pub near_droneid: bool,
    /// The DroneID ZC symbols were seen.
    pub zc_seen: bool,
    /// DroneID packets decoded on this track.
    pub decoded: u64,
    /// DroneID bursts with ZC whose CRC failed.
    pub failed: u64,
    /// The band.
    pub band: Option<RadioBand>,
    /// Scratch for the medians.
    scratch: Vec<f64>,
    /// Scratch of the shared quantile.
    sorted: Vec<f64>,
}

/// The band of a frequency in MHz.
pub fn band_of(mhz: f64) -> Option<RadioBand> {
    if (2400.0..=2483.5).contains(&mhz) {
        Some(RadioBand::B2g4)
    } else if (5725.0..=5850.0).contains(&mhz) {
        Some(RadioBand::B5g8)
    } else if (5150.0..=5350.0).contains(&mhz) {
        Some(RadioBand::B5g2)
    } else {
        None
    }
}

/// The median of `v` (the upper one of an even count), 0 for none. `scratch` is a buffer of the shared quantile.
fn median(v: &[f64], scratch: &mut Vec<f64>) -> f64 {
    sdrkit_algo_dsp::stats::quantile_f64(v, 0.5, scratch).unwrap_or(0.0)
}

impl Track {
    fn new(id: u32, t: f64, s: &BurstSignal) -> Track {
        Track {
            id,
            offset_hz: s.offset_hz,
            freq_hz: 0.0,
            width_hz: s.width_hz,
            clipped: s.clipped,
            level_dbfs: s.level_dbfs,
            snr_db: s.snr_db,
            bursts: 0,
            last_t: t,
            obs: VecDeque::with_capacity(OBS + 1),
            levels: VecDeque::new(),
            period_s: None,
            regularity: 0.0,
            duty: 0.0,
            burst_s: 0.0,
            ofdm: 0.0,
            class: Class::OtherWideband,
            confidence: 0.0,
            reason: Reason::TooFew,
            near_droneid: false,
            zc_seen: false,
            decoded: 0,
            failed: 0,
            band: None,
            scratch: Vec::with_capacity(OBS),
            sorted: Vec::with_capacity(OBS),
        }
    }

    fn update_class(&mut self, centre_hz: f64) {
        let n = self.obs.len();
        let mut v = std::mem::take(&mut self.scratch);
        v.clear();
        v.extend(self.obs.iter().map(|o| o.1));
        self.burst_s = median(&v, &mut self.sorted);
        v.clear();
        v.extend(self.obs.iter().map(|o| o.2));
        self.ofdm = median(&v, &mut self.sorted);
        if n >= 3 {
            v.clear();
            v.extend(
                self.obs
                    .iter()
                    .zip(self.obs.iter().skip(1))
                    .map(|(a, b)| b.0 - a.0),
            );
            let p = median(&v, &mut self.sorted);
            self.period_s = (p > 0.0).then_some(p);
            if p > 0.0 {
                let ok = v
                    .iter()
                    .filter(|&&g| {
                        let m = (g / p).round();
                        (1.0..=8.0).contains(&m) && (g - m * p).abs() <= 0.06 * p
                    })
                    .count();
                self.regularity = ok as f64 / v.len() as f64;
            }
            let span = self.obs[n - 1].0 + self.obs[n - 1].1 - self.obs[0].0;
            let sum: f64 = self.obs.iter().map(|o| o.1).sum();
            self.duty = if span > 0.0 {
                (sum / span).min(1.0)
            } else {
                0.0
            };
        }
        self.scratch = v;
        let w = self.width_hz / 1e6;
        let thr = if w > 12.0 { 0.6 } else { 0.35 };
        let ofdm_like = self.ofdm >= thr;
        let f_o = ((self.ofdm - 0.2) / 0.4).clamp(0.0, 1.0);
        let ten_twenty = (8.0..=11.8).contains(&w) || (16.0..=21.5).contains(&w);
        let clipped_wide = self.clipped && w >= 14.0;
        let f_b = if ten_twenty {
            1.0
        } else if clipped_wide {
            0.6
        } else {
            0.0
        };
        let f_n = (self.bursts as f64 / 10.0).min(1.0);
        let conf = (0.15 + 0.35 * f_o + 0.25 * self.regularity + 0.15 * f_b + 0.10 * f_n).min(0.95);
        let regular_period = self.period_s.filter(|_| {
            n >= 3 && ofdm_like && (ten_twenty || clipped_wide) && self.regularity >= 0.6
        });
        let (class, confidence, reason) = if self.zc_seen {
            if self.decoded > 0 {
                (Class::DroneId, 0.99, Reason::Decoded)
            } else {
                (Class::DroneId, 0.9, Reason::ZcOnly)
            }
        } else if let Some(p) = regular_period {
            if (p - 1e-3).abs() <= 0.06e-3 {
                (Class::LightbridgeLike, conf, Reason::EveryMs)
            } else if (0.3e-3..=25e-3).contains(&p) {
                (Class::OcuSyncLike, conf, Reason::Periodic)
            } else {
                (Class::UnknownOfdm, conf.min(0.4), Reason::OtherPeriod)
            }
        } else if n >= 2 && ofdm_like && w >= 5.0 {
            (
                Class::UnknownOfdm,
                (0.1 + 0.3 * f_o * f_n + 0.1 * self.regularity).min(0.4),
                Reason::Irregular,
            )
        } else if n < 3 {
            (Class::OtherWideband, 0.0, Reason::TooFew)
        } else {
            (Class::OtherWideband, 0.0, Reason::NotOfdm)
        };
        self.class = class;
        self.confidence = confidence;
        self.reason = reason;
        let f = self.freq_hz / 1e6;
        self.near_droneid = (8.0..=11.8).contains(&w)
            && class >= Class::UnknownOfdm
            && class != Class::DroneId
            && centre_hz != 0.0
            && DRONEID_FREQS.iter().any(|d| (d - f).abs() <= 3.0);
    }

    /// The row of the track in the signal table, with the capture centre `centre` (0 when unknown).
    fn row(&self, centre: f64) -> DjiDroneidSignalRow {
        DjiDroneidSignalRow {
            track: i64::from(self.id),
            class: self.class,
            confidence_pct: self.confidence * 100.0,
            freq_hz: (centre != 0.0).then_some(Hertz(self.freq_hz)),
            offset_hz: Hertz(self.offset_hz),
            width_hz: Hertz(self.width_hz),
            clipped: self.clipped,
            burst: Seconds(self.burst_s),
            period: self.period_s.map(Seconds),
            duty_pct: self.duty * 100.0,
            regularity_pct: self.regularity * 100.0,
            ofdm_pct: self.ofdm * 100.0,
            level_dbfs: self.level_dbfs,
            snr_db: self.snr_db,
            bursts: i64::try_from(self.bursts).unwrap_or(i64::MAX),
            zc_seen: self.zc_seen,
            decoded: i64::try_from(self.decoded).unwrap_or(i64::MAX),
            failed: i64::try_from(self.failed).unwrap_or(i64::MAX),
            band: self.band,
            reason: self.reason,
            near_droneid: self.near_droneid,
            last_s: self.last_t,
        }
    }
}

/// One DroneID drone.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Drone {
    /// Serial number.
    pub serial: String,
    /// The newest packet.
    pub last: DroneIdPacket,
    /// SNR, dB.
    pub snr_db: f64,
    /// RF frequency, Hz (0 when the centre is unknown).
    pub freq_hz: f64,
    /// The track it came on.
    pub track: u32,
    /// The track of positions.
    pub trail: Trail,
}

impl Shift for Drone {
    fn shift_times(&mut self, by_s: f64) {
        self.trail.shift_times(by_s);
    }
}

fn opt_pos(lat: f64, lon: f64) -> Option<Position> {
    (lat.abs() > 1e-4 && lon.abs() > 1e-4 && lat.abs() <= 90.0 && lon.abs() <= 180.0)
        .then(|| Position::new(lat, lon))
}

fn pair(p: Option<Position>) -> Option<(f64, f64)> {
    p.map(|p| (p.lat_deg, p.lon_deg))
}

impl Drone {
    /// The drone's position, when the packet has a fix.
    pub fn position(&self) -> Option<Position> {
        self.last
            .has_fix()
            .then(|| Position::new(self.last.lat, self.last.lon))
    }
    /// The home position, when it is set.
    pub fn home(&self) -> Option<Position> {
        opt_pos(self.last.home_lat, self.last.home_lon)
    }
    /// The pilot's position, when it is set.
    pub fn pilot(&self) -> Option<Position> {
        opt_pos(self.last.pilot_lat, self.last.pilot_lon)
    }
    /// The ground speed in knots and the course in degrees, from the velocity north and east (cm/s).
    pub fn speed_course(&self) -> (f64, f64) {
        let (n, e) = (
            f64::from(self.last.v_north) / 100.0,
            f64::from(self.last.v_east) / 100.0,
        );
        let course = e.atan2(n).to_degrees().rem_euclid(360.0);
        (n.hypot(e) / KNOT_M_S, course)
    }
}

impl Shaped for Drone {
    type Row = DjiDroneidDroneRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let p = &self.last;
        let (speed, course) = self.speed_course();
        out.put(&DjiDroneidDroneRow {
            source: &self.serial,
            position: pair(self.position()),
            alt_m: p.alt_m(),
            height_m: p.height_m(),
            alt_raw: i64::from(p.alt_raw),
            height_raw: i64::from(p.height_raw),
            speed_kt: speed,
            course_deg: course,
            home: pair(self.home()),
            pilot: pair(self.pilot()),
            seq: i64::from(p.seq),
            state_info: i64::from(p.state),
            version: i64::from(p.version),
            inner_crc_ok: p.inner_crc_ok,
            v_north: i64::from(p.v_north),
            v_east: i64::from(p.v_east),
            v_up: i64::from(p.v_up),
            yaw: i64::from(p.yaw),
            gps_time: Time(p.gps_ms as f64 / 1e3),
            device_type: i64::from(p.device_type),
            uuid: (!p.uuid.is_empty()).then_some(p.uuid.as_str()),
            freq_hz: (self.freq_hz != 0.0).then_some(Hertz(self.freq_hz)),
            snr_db: self.snr_db,
            track: i64::from(self.track),
            trail_points: self.trail.len() as i64,
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
        });
    }
    fn point(&self) -> Option<Point<'_>> {
        Some(Point::new(self.position()?, self.serial.as_str()))
    }
}

/// One line of the burst log.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BurstLine {
    /// Signal time, seconds.
    pub t: f64,
    /// Wall time, when the samples are live and the wall clock is on.
    pub wall: Option<f64>,
    /// Track.
    pub track: u32,
    /// Offset, Hz.
    pub offset_hz: f64,
    /// Width, Hz.
    pub width_hz: f64,
    /// Length, s.
    pub length_s: f64,
    /// SNR, dB.
    pub snr_db: f64,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// OFDM-likeness.
    pub ofdm: f64,
    /// The result of the DroneID attempt.
    pub droneid: Attempt,
}

impl Shift for BurstLine {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

impl Shaped for BurstLine {
    type Row = DjiDroneidBurstRow;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&DjiDroneidBurstRow {
            time_s: self.t,
            time: self.wall.map(Time),
            track: i64::from(self.track),
            offset_hz: Hertz(self.offset_hz),
            width_hz: Hertz(self.width_hz),
            burst: Seconds(self.length_s),
            level_dbfs: self.level_dbfs,
            snr_db: self.snr_db,
            ofdm_pct: self.ofdm * 100.0,
            droneid: self.droneid,
        });
    }
}

/// The counters.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Counters {
    /// Bursts measured.
    pub bursts_measured: u64,
    /// Of them OFDM-like.
    pub bursts_ofdm_like: u64,
    /// DroneID packets decoded.
    pub droneid_decoded: u64,
    /// DroneID bursts with ZC whose CRC failed.
    pub droneid_crc_failed: u64,
    /// DroneID attempts.
    pub droneid_tried: u64,
}

/// What the tracker keeps across sessions: the drones, the burst log and the counters.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The drones by serial number.
    pub drones: ObjectTable<String, Drone>,
    /// The burst log.
    pub log: LogRing<BurstLine>,
    /// The counters.
    pub counters: Counters,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            drones: ObjectTable::new(DRONE_CAP),
            log: LogRing::new(BURST_LOG),
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
    const FORMAT: StateFormat = StateFormat::new("drones.dji_droneid", 2);
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

/// What the tracker shows: nothing, wideband bursts, a DJI-like shape, DroneID packets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Heard {
    Nothing,
    Bursts,
    DjiShape,
    DroneId,
}

/// The tracker component (`drones.dji_tracker`): tracks and classes, the DroneID receiver, the drone table and the results.
pub struct DjiTracker {
    published: Published,
    drones_table: TableId,
    level_series: SeriesId,
    status: StatusLine,
    store: Store,
    clock: SignalClock,
    centre: f64,
    signal_offset: f64,
    live: bool,
    host_clock: Clock,
    decode: bool,
    wall_clock: bool,
    next_track: u32,
    tracks: Vec<Track>,
    det: DetectorStats,
    rx: BurstReceiver,
    last_droneid_t: Option<f64>,
    cfo_centre: Option<f64>,
    burst_cfo: Option<f64>,
    last_level: Option<f64>,
    last_snr: Option<f64>,
    level_hist: VecDeque<(f64, f64)>,
    state: Option<Heard>,
    lines: Vec<String>,
    events: Vec<Event>,
    key: String,
    configured: bool,
    dirty: bool,
}

impl Default for DjiTracker {
    fn default() -> Self {
        let decl = Arc::new(declaration());
        let drones_table = decl
            .table_id(DJI_DRONES.key)
            .expect("the declaration holds the drones table");
        let level_series = decl
            .series_id(LEVEL_SERIES)
            .expect("the declaration holds the level series");
        DjiTracker {
            published: Published::from_decl(decl),
            drones_table,
            level_series,
            status: StatusLine::default(),
            store: Store::default(),
            clock: SignalClock::default(),
            centre: 0.0,
            signal_offset: 0.0,
            live: false,
            host_clock: Clock::fixed(0.0),
            decode: DECODE_DRONEID.default,
            wall_clock: WALL_CLOCK.default,
            next_track: 1,
            tracks: Vec::with_capacity(TRACK_CAP),
            det: DetectorStats::default(),
            rx: BurstReceiver::default(),
            last_droneid_t: None,
            cfo_centre: None,
            burst_cfo: None,
            last_level: None,
            last_snr: None,
            level_hist: VecDeque::with_capacity(LEVELS + 1),
            state: None,
            lines: Vec::new(),
            events: Vec::new(),
            key: String::new(),
            configured: false,
            dirty: true,
        }
    }
}

impl DjiTracker {
    /// The table the tracker keeps.
    pub fn store(&self) -> &Store {
        &self.store
    }
    /// The signal tracks.
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    fn wall(&self) -> Option<f64> {
        wall(self.live && self.wall_clock, &self.host_clock)
    }

    /// Takes one measured signal at signal time `t` (its start).
    pub fn take(&mut self, t: f64, s: &BurstSignal) {
        self.store.counters.bursts_measured += 1;
        let thr = if s.width_hz > 12e6 { 0.6 } else { 0.35 };
        if s.ofdm >= thr {
            self.store.counters.bursts_ofdm_like += 1;
        }
        self.last_level = Some(s.level_dbfs);
        self.last_snr = Some(s.snr_db);
        // Association: within 1.5 MHz and a width ratio of 0.7 to 1.43, nearest offset first.
        let idx = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, tr)| {
                let r = s.width_hz / tr.width_hz.max(1.0);
                (tr.offset_hz - s.offset_hz).abs() <= 1.5e6 && (0.7..=1.43).contains(&r)
            })
            .min_by(|a, b| {
                (a.1.offset_hz - s.offset_hz)
                    .abs()
                    .total_cmp(&(b.1.offset_hz - s.offset_hz).abs())
            })
            .map(|(i, _)| i);
        let i = match idx {
            Some(i) => i,
            None => {
                if self.tracks.len() >= TRACK_CAP {
                    let old = (0..self.tracks.len())
                        .min_by(|&a, &b| self.tracks[a].last_t.total_cmp(&self.tracks[b].last_t))
                        .unwrap_or(0);
                    self.tracks.remove(old);
                }
                let id = self.next_track;
                self.next_track += 1;
                self.tracks.push(Track::new(id, t, s));
                self.tracks.len() - 1
            }
        };
        let centre = self.centre;
        let sig_off = self.signal_offset;
        {
            let tr = &mut self.tracks[i];
            if tr.bursts > 0 {
                tr.offset_hz += 0.2 * (s.offset_hz - tr.offset_hz);
                tr.width_hz += 0.2 * (s.width_hz - tr.width_hz);
            }
            tr.bursts += 1;
            tr.clipped = s.clipped;
            tr.level_dbfs = s.level_dbfs;
            tr.snr_db = s.snr_db;
            tr.last_t = t;
            tr.freq_hz = if centre != 0.0 {
                centre + tr.offset_hz
            } else {
                0.0
            };
            tr.band = band_of(tr.freq_hz / 1e6);
            tr.obs.push_back((t, s.duration_s, s.ofdm));
            while tr.obs.len() > OBS {
                tr.obs.pop_front();
            }
            let slot = (t / REPORT_S).floor() * REPORT_S;
            match tr.levels.back_mut() {
                Some(l) if l.0 == slot => l.1 = l.1.max(s.level_dbfs),
                _ => {
                    tr.levels.push_back((slot, s.level_dbfs));
                    while tr.levels.len() > LEVELS {
                        tr.levels.pop_front();
                    }
                }
            }
            tr.update_class(centre);
        }
        // DroneID attempt.
        let tr = &self.tracks[i];
        let w = s.width_hz;
        let allows = tr.zc_seen || tr.period_s.is_none() || tr.period_s.is_some_and(|p| p > 50e-3);
        let mut outcome = Attempt::NotTried;
        if self.decode
            && (6.5e6..=11.8e6).contains(&w)
            && (520e-6..=1500e-6).contains(&s.duration_s)
            && allows
            && (s.rate - droneid::FS).abs() < 1.0
        {
            self.store.counters.droneid_tried += 1;
            let d = self.rx.decode(&s.samples);
            outcome = if !d.zc_found {
                Attempt::NoDroneId
            } else if d.packet.is_some() {
                Attempt::Decoded
            } else {
                Attempt::CrcFailed
            };
            let tr = &mut self.tracks[i];
            if d.zc_found {
                tr.zc_seen = true;
                if outcome == Attempt::Decoded {
                    tr.decoded += 1;
                } else {
                    tr.failed += 1;
                    self.store.counters.droneid_crc_failed += 1;
                }
            }
            if let Some(p) = d.packet {
                let tid = tr.id;
                tr.update_class(centre);
                // The burst's offset from the capture centre, and its own carrier offset from the nearest 0.5 MHz channel.
                let from_centre = s.offset_hz + d.cfo_hz + sig_off;
                let abs = if centre != 0.0 {
                    centre + from_centre
                } else {
                    from_centre
                };
                let grid = (abs / 0.5e6).round() * 0.5e6;
                self.cfo_centre = Some(from_centre);
                self.burst_cfo = Some(abs - grid);
                let freq = if centre != 0.0 { grid } else { 0.0 };
                self.take_packet(t, p, d.snr_db, freq, tid);
            } else if d.zc_found {
                tr.update_class(centre);
            }
        }
        let wall = self.wall();
        self.store.log.push(BurstLine {
            t,
            wall,
            track: self.tracks[i].id,
            offset_hz: s.offset_hz,
            width_hz: s.width_hz,
            length_s: s.duration_s,
            snr_db: s.snr_db,
            level_dbfs: s.level_dbfs,
            ofdm: s.ofdm,
            droneid: outcome,
        });
        self.dirty = true;
    }

    fn take_packet(&mut self, t: f64, p: DroneIdPacket, snr: f64, freq: f64, track: u32) {
        self.store.counters.droneid_decoded += 1;
        self.last_droneid_t = Some(t);
        let serial = p.serial.clone();
        self.lines.push(format!(
            "DJI DroneID: serial {}",
            if serial.is_empty() {
                "(empty)"
            } else {
                &serial
            }
        ));
        let stamp = match self.wall() {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        };
        let Ok(up) = self.store.drones.upsert(serial.as_str(), stamp, || Drone {
            serial: serial.clone(),
            last: p.clone(),
            snr_db: snr,
            freq_hz: freq,
            track,
            trail: Trail::new(TRAIL_CAP, Spacing::metres(0.01)),
        }) else {
            return;
        };
        let d = up.row;
        d.snr_db = snr;
        d.freq_hz = freq;
        d.track = track;
        d.last = p;
        if let Some(at) = d.position() {
            // A position outside the trail's limits is not kept in the trail. The row still shows it.
            let _ = d.trail.push(t, at, ());
        }
        let (speed, course) = d.speed_course();
        let e = DJI_DRONEID.event(
            t,
            freq,
            &DjiDroneidEvent {
                source: &serial,
                position: pair(d.position()),
                alt_m: d.last.alt_m(),
                height_m: d.last.height_m(),
                alt_raw: i64::from(d.last.alt_raw),
                height_raw: i64::from(d.last.height_raw),
                speed_kt: speed,
                course_deg: course,
                home: pair(d.home()),
                pilot: pair(d.pilot()),
                family: Family::Aircraft,
            },
        );
        self.events.push(e);
    }

    fn heard_now(&self, now: f64) -> Heard {
        if self.last_droneid_t.is_some_and(|t| now - t <= 30.0) {
            Heard::DroneId
        } else if self
            .tracks
            .iter()
            .any(|t| t.class >= Class::OcuSyncLike && now - t.last_t <= 10.0)
        {
            Heard::DjiShape
        } else if self.tracks.iter().any(|t| now - t.last_t <= 5.0) {
            Heard::Bursts
        } else {
            Heard::Nothing
        }
    }

    fn clear(&mut self) {
        self.store.clear();
        self.tracks.clear();
        self.next_track = 1;
        self.last_droneid_t = None;
        self.cfo_centre = None;
        self.burst_cfo = None;
        self.level_hist.clear();
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
                        Heard::DroneId => "DJI: DroneID packets decoded",
                        Heard::DjiShape => "DJI: a DJI-like signal (by its shape)",
                        Heard::Bursts => "DJI: wideband bursts",
                        Heard::Nothing => "DJI: nothing heard",
                    }
                    .into(),
                );
            }
            self.state = Some(state);
        }
        // The newest track is written last, so it is the newest row.
        self.tracks
            .sort_unstable_by(|a, b| a.last_t.total_cmp(&b.last_t));
        let s = &self.store;
        let c = &s.counters;
        let newest = s.drones.iter_newest_first().next().map(|r| r.record);
        let best = self
            .tracks
            .iter()
            .filter(|t| t.class >= Class::UnknownOfdm)
            .max_by(|a, b| a.confidence.total_cmp(&b.confidence));
        let p = &mut self.published;
        let lock_state = match state {
            Heard::DroneId => LockState::Locked,
            Heard::DjiShape | Heard::Bursts => LockState::Tracking,
            Heard::Nothing => LockState::Searching,
        };
        let (snr, floor, cfo) = (self.last_snr, self.det.floor_dbfs, self.cfo_centre);
        let st = &mut self.status;
        match (state, newest, best) {
            (Heard::DroneId, d, _) => {
                let serial = d
                    .map(|d| d.serial.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("no serial");
                let packets = d
                    .and_then(|d| s.drones.seen(d.serial.as_str()))
                    .map_or(0, |x| x.count);
                st.write(
                    p,
                    lock_state,
                    snr,
                    floor,
                    cfo,
                    format_args!("DroneID: {serial} ({packets} packets)"),
                );
            }
            (_, _, Some(t)) => st.write(
                p,
                lock_state,
                snr,
                floor,
                cfo,
                format_args!(
                    "{}, {:.1} MHz wide, {:.0}%",
                    t.class.name(),
                    t.width_hz / 1e6,
                    t.confidence * 100.0
                ),
            ),
            (Heard::Bursts | Heard::DjiShape, _, None) => st.write(
                p,
                lock_state,
                snr,
                floor,
                cfo,
                format_args!(
                    "{} wideband bursts, nothing DJI-like yet",
                    c.bursts_measured
                ),
            ),
            (Heard::Nothing, _, None) => {
                st.write(p, lock_state, snr, floor, cfo, format_args!("listening"))
            }
        }
        for l in self.lines.drain(..) {
            p.results_mut().log(l);
        }
        self.key.clear();
        if let Some(t) = best {
            let _ = write!(self.key, "{}", t.id);
        }
        p.put(&DjiDroneidFields {
            bursts_measured: c.bursts_measured as i64,
            bursts_ofdm_like: c.bursts_ofdm_like as i64,
            droneid_decoded: c.droneid_decoded as i64,
            droneid_crc_failed: c.droneid_crc_failed as i64,
            droneid_tried: c.droneid_tried as i64,
            centre_hz: (self.centre != 0.0).then_some(Hertz(self.centre)),
            burst_cfo_hz: self.burst_cfo.map(Hertz),
            drones: s.drones.len() as i64,
            signal: best.map(|_| self.key.as_str()),
            noise_dbfs: floor,
            level_dbfs: self.last_level,
            snr_db: snr,
            offset_hz: cfo.map(Hertz),
        });
        for t in &self.tracks {
            self.key.clear();
            let _ = write!(self.key, "{}", t.id);
            DJI_SIGNALS.put(p, &self.key, &t.row(self.centre));
        }
        keep_newest_rows(&DJI_SIGNALS, self.tracks.len(), p);
        shaped::rows(&s.drones, &DJI_DRONES, now, p);
        shaped::log_rows(&s.log, &DJI_BURSTS, now, p);
        publish_tracks(
            &s.drones,
            |d: &Drone| &d.trail,
            p,
            &mut self.key,
            |p, key, drone, position, time_s| {
                DJI_TRACKS.put(
                    p,
                    key,
                    &DroneTrackRow {
                        drone,
                        position,
                        time_s,
                    },
                );
            },
            &DJI_TRACKS,
        );
        let hist = &self.level_hist;
        let r = p.results_mut();
        r.update_series(self.level_series, |series: &mut Series| {
            let same = series.line
                && series.points.len() == hist.len()
                && series
                    .points
                    .iter()
                    .zip(hist)
                    .all(|(a, &(t, l))| *a == [t as f32, l as f32]);
            if same {
                return false;
            }
            series.line = true;
            series.points.clear();
            series
                .points
                .extend(hist.iter().map(|&(t, l)| [t as f32, l as f32]));
            true
        });
        shaped::row_points(&s.drones, &DJI_DRONES, "drone/", r);
        self.dirty = false;
    }
}

impl Component for DjiTracker {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "drones.dji_tracker",
            vec![Port::typed::<BurstItem>("signals"), Port::iq("clock")],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(DjiTrackerSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.host_clock = ctx.clock.clone();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[1];
        self.clock.restart(f.rate);
        self.centre = f.centre_hz;
        self.signal_offset = f.signal_offset_hz();
        self.configured = true;
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(1);
        let epoch = self.clock.epoch;
        for it in io.items::<BurstItem>(0) {
            match &it.value {
                BurstItem::Signal(s) => self.take(epoch + s.start_s, s),
                BurstItem::Stats(s) => self.det.clone_from(s),
            }
        }
        self.clock.samples += n as u64;
        let tick = io.tick();
        if tick {
            let now = self.clock.now();
            if let Some(l) = self.det.level_dbfs {
                self.level_hist.push_back((now, l));
                while self.level_hist.len() > LEVELS {
                    self.level_hist.pop_front();
                }
            }
        }
        if tick || self.dirty {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DjiTrackerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DjiTrackerSetting::DecodeDroneId => self.decode = DECODE_DRONEID.read(id, v)?,
            DjiTrackerSetting::WallClock => self.wall_clock = WALL_CLOCK.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (DjiTrackerSetting::DecodeDroneId, Value::Bool(self.decode)),
            (DjiTrackerSetting::WallClock, Value::Bool(self.wall_clock)),
        ])
    }
    fn commands(&self) -> &Commands {
        DjiTrackerCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match DjiTrackerCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DjiTrackerCommand::Clear => self.clear(),
            DjiTrackerCommand::Remove => {
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
