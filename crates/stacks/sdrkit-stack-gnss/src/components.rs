//! The graph components of the satellite navigation stacks. Each one wraps one stage with typed ports:
//!
//! | Kind | Inputs | Outputs | Stage |
//! |---|---|---|---|
//! | `gnss.receiver`, `gnss.sbas_receiver` | `Iq` | hits, frames, ephemerides, measurements, fixes, SBAS messages, channels, events | the whole receiver chain |
//! | `gnss.input_conditioning` | `Iq` | `Iq` | DC removal, level and clipping |
//! | `gnss.excision` | `Iq` | `Iq` | narrowband interference excision |
//! | `gnss.acquisition` | `Iq` at a band's rate and carrier | hits | code phase search over a Doppler grid |
//! | `gnss.nav_store` | frames | ephemerides | navigation data |
//! | `gnss.pvt` | frames, measurements | fixes | position, velocity and time |
//! | `gnss.interference` | `Iq`, measurements, fixes | results | interference and spoofing monitor |
//! | `gnss.nmea` | fixes | NMEA sentences | NMEA 0183 RMC and GGA |
//! | `gnss.rinex` | measurements, ephemerides | RINEX text | RINEX 3.04 observation and navigation text |
//! | `gnss.sbas_content` | SBAS messages, channels | results | the SBAS content decoder |

use crate::acq::{AcqHit, Integration, Search, Searcher};
use crate::codes::{self, SignalKind};
use crate::core::{ChannelRow, CoreConfig, Fix, GnssCore, MeasEpoch, NavFrame, SbasMessage};
use crate::fixsolve::FixSolver;
use crate::frontend::{Conditioner, Excision};
use crate::monitor::{Monitor, RxView};
use crate::navstore::{Ephemeris, NavStore};
use crate::outputs::{nmea, rinex_nav, RinexObs};
use crate::results::{self as res, EphemerisState, OrbitSource};
use crate::sbas::{self, SbasContent};
use crate::settings::{
    ReceiverSetting, ReceiverValues, SbasReceiverSetting, ACQUISITION_EFFORT, CLOCK_COMPARE,
    ELEVATION_MASK, FREQUENCY_HINT, FREQUENCY_HINT_VALID, GALILEO_CODES, KNOWN_WEEK,
    SEARCH_HALF_WIDTH, SYS_BEIDOU, SYS_GALILEO, SYS_GLONASS, SYS_GPS, SYS_QZSS, SYS_SBAS,
};
use crate::system::*;
use crate::track::ChannelState;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::publish::{field, keys, Published, TableDef};
use sdrkit_core::settings::{Setting, SettingSet};
use sdrkit_core::{
    Choice, Complex32, ConfigError, LockState, Record, Results, SeriesId, SettingError, SettingId,
    TableId, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port,
    RateFrom, RateRule, Registry, RunContext, TagKind, Timed,
};
use crate::typed::{
    GnssFields, GnssFixSystemRow, GnssSatelliteRow, GnssSystemRow, InterferenceEventRow,
    InterferenceFields, InterferenceLineRow, NmeaEvent, FIX_SYSTEMS, INTERFERENCE_EVENTS,
    INTERFERENCE_LINES, MONITOR_SERIES, NMEA, RECEIVER_SERIES, SATELLITES, SBAS_FAST_CORRECTIONS,
    SBAS_IONOSPHERE, SBAS_MESSAGES, SBAS_SATELLITES, SBAS_SATELLITE_TYPES, SBAS_SELECT, SBAS_TYPES,
    SYSTEMS,
};
use crate::typed::{
    SbasFastCorrectionRow, SbasFields, SbasIonosphereRow, SbasMessageRow, SbasSatelliteRow,
    SbasSatelliteTypeRow, SbasSelectArgs, SbasTypeRow,
};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_util_objects::publish::Point;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{ObjectTable, Seen, Stamp};
use std::fmt::Write as _;
use std::sync::Arc;

/// An ephemeris with its satellite.
#[derive(Clone, Debug, PartialEq)]
pub struct EphemerisItem {
    /// The satellite.
    pub sat: SatId,
    /// Its ephemeris.
    pub eph: Ephemeris,
}

/// The channel table of one report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelTable(
    /// One row for each channel.
    pub Vec<ChannelRow>,
);

/// One NMEA 0183 sentence. It leaves the graph as a `text.nmea` event.
#[derive(Clone, Debug, PartialEq)]
pub struct NmeaText(
    /// The sentence, from `$` to the checksum.
    pub String,
);

/// RINEX text lines: which file and the lines.
#[derive(Clone, Debug, PartialEq)]
pub struct RinexText {
    /// The lines belong to the navigation file (else to the observation file).
    pub navigation: bool,
    /// The lines.
    pub lines: Vec<String>,
}

impl Item for AcqHit {
    const KIND: &'static str = "gnss.acq_hit";
}
impl Item for NavFrame {
    const KIND: &'static str = "gnss.nav_frame";
}
impl Item for EphemerisItem {
    const KIND: &'static str = "gnss.ephemeris";
}
impl Item for MeasEpoch {
    const KIND: &'static str = "gnss.measurement";
}
impl Item for Fix {
    const KIND: &'static str = "gnss.fix";
}
impl Item for SbasMessage {
    const KIND: &'static str = "sbas.message";
}
impl Item for ChannelTable {
    const KIND: &'static str = "gnss.channels";
}
impl Item for NmeaText {
    const KIND: &'static str = "text.nmea";
    fn event_def() -> Option<&'static sdrkit_core::EventDef> {
        Some(NMEA.def())
    }
    fn write_fields(&self, fields: &mut Record) {
        NmeaEvent {
            family: keys::Family::Receiver,
            text: &self.0,
        }
        .write(fields);
    }
}
impl Item for RinexText {
    const KIND: &'static str = "text.rinex";
}

fn typed_fmt<T: Item>() -> Format {
    Format::new(Kind::typed::<T>(), 0.0)
}

/// A typed output port with no sample rate.
fn typed_out<T: Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

/// A product of the core as a timed item.
fn timed<T>((time, value): (f64, T)) -> Timed<T> {
    Timed { time, value }
}

/// Writes `v` into `slot` and says whether it changed.
fn upd<T: PartialEq>(slot: &mut T, v: T) -> bool {
    let c = *slot != v;
    if c {
        *slot = v;
    }
    c
}

/// The ids of the series `keys` in `p`.
fn series_ids<const N: usize>(
    p: &Published,
    keys: &[(&str, &str, &str, &str); N],
) -> [SeriesId; N] {
    keys.map(|(k, ..)| {
        p.results()
            .decl()
            .series_id(k)
            .expect("the declaration holds the series")
    })
}

/// The id of table `t` in `p`.
fn table_id(p: &Published, t: &TableDef) -> TableId {
    p.results()
        .decl()
        .table_id(t.key)
        .expect("the declaration holds the table")
}

// The receiver.

/// Starts the receiver again: drops every channel and search, keeps the navigation data.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Drops every channel and search. The navigation data stays.");

const SATELLITE_ROW: sdrkit_core::settings::Argument<field::Text> =
    sdrkit_util_objects::commands::row_of("satellites");
const REMOVE: Command = sdrkit_util_objects::commands::remove_command(&[&SATELLITE_ROW]);

sdrkit_core::commands! {
    /// The commands of the receiver components.
    pub enum ReceiverCommand {
        /// Reset.
        Reset = RESET,
        /// Forgets one satellite of the satellite table. It comes back when it is heard again.
        Remove = REMOVE,
    }
}

/// One row of the satellite table: what the channels, the navigation data, the sky and the orbits say of one satellite.
#[derive(Clone, Debug, Default)]
struct SatRow {
    /// The report that wrote the row last.
    report: u64,
    name: String,
    system: Option<System>,
    prn: Option<i64>,
    glo_channel: Option<i64>,
    tracked: bool,
    state: Option<ChannelState>,
    cn0: Option<f64>,
    doppler: Option<f64>,
    code_chips: Option<f64>,
    az: Option<f64>,
    el: Option<f64>,
    locked_for: Option<f64>,
    frames_good: Option<i64>,
    frames: Option<i64>,
    residual: Option<f64>,
    healthy: Option<bool>,
    used: bool,
    ephemeris: Option<EphemerisState>,
    parts: Option<(u32, u32)>,
    seconds_left: Option<f64>,
    iod: Option<i64>,
    data_age: Option<f64>,
    week: Option<i64>,
    tow: Option<f64>,
    af0: Option<f64>,
    sub_point: Option<(f64, f64)>,
    height_km: Option<f64>,
    orbit: Option<OrbitSource>,
    public: bool,
}

impl Shaped for SatRow {
    type Row = GnssSatelliteRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&GnssSatelliteRow {
            system: self.system,
            prn: self.prn,
            glo_channel: self.glo_channel,
            tracked: self.tracked,
            state: self.state,
            cn0_dbhz: self.cn0,
            doppler: self.doppler.map(Hertz),
            code_chips: self.code_chips,
            azimuth_deg: self.az,
            elevation_deg: self.el,
            locked_for: self.locked_for.map(Seconds),
            frames_good: self.frames_good,
            frames: self.frames,
            residual_m: self.residual,
            healthy: self.healthy,
            used: self.used,
            ephemeris: self.ephemeris,
            parts: self.parts.map(|p| i64::from(p.0)),
            parts_of: self.parts.map(|p| i64::from(p.1)),
            seconds_left: self.seconds_left.map(Seconds),
            iod: self.iod,
            data_age: self.data_age.map(Seconds),
            week: self.week,
            tow_s: self.tow,
            af0_s: self.af0,
            sub_point: self.sub_point,
            height_km: self.height_km,
            orbit: self.orbit,
        });
    }

    fn point(&self) -> Option<Point<'_>> {
        let (lat, lon) = self.sub_point.filter(|_| self.public)?;
        Some(Point::new(
            sdrkit_util_geo::Position::new(lat, lon),
            self.name.as_str(),
        ))
    }
}

/// The row of satellite `name` for report `report`: emptied if an earlier report wrote it.
fn sat_row<'a>(
    sats: &'a mut ObjectTable<String, SatRow>,
    name: &str,
    now: f64,
    report: u64,
) -> Option<&'a mut SatRow> {
    let up = sats
        .upsert(name, Stamp::at(now), || SatRow {
            name: name.to_owned(),
            ..SatRow::default()
        })
        .ok()?;
    if up.row.report != report {
        let name = std::mem::take(&mut up.row.name);
        *up.row = SatRow {
            report,
            name,
            ..SatRow::default()
        };
    }
    Some(up.row)
}

/// The whole receiver chain as one component (`gnss.receiver`, or `gnss.sbas_receiver` for the `sbas` stack): bands, searches,
/// channels, message layers, measurements and the fix.
pub struct GnssCoreComponent {
    /// The receiver core.
    pub core: GnssCore,
    values: ReceiverValues,
    sbas: bool,
    results: Published,
    /// The progress lines of the last report, joined with new lines.
    progress: String,
    series: [SeriesId; 5],
    sats: ObjectTable<String, SatRow>,
    reports: u64,
    seen_log: usize,
    live: bool,
    /// The Galileo table of the present path is read (it is read at configure).
    galileo_read: bool,
}

/// The results store of the receiver.
fn receiver_results() -> Published {
    Published::from_decl(Arc::new(res::receiver_declaration()))
}

/// Reads the Galileo code table file `path` of setting `id`, or returns none for an empty path.
///
/// # Errors
///
/// [`ConfigError::Setting`] with [`SettingError::Malformed`] if the file does not read or holds no code.
fn galileo_table(path: &str, id: SettingId) -> Result<Option<codes::E1bTable>, ConfigError> {
    if path.is_empty() {
        return Ok(None);
    }
    codes::E1bTable::load(path)
        .map(Some)
        .map_err(|_| ConfigError::Setting(SettingError::Malformed { id, at: None }))
}

impl GnssCoreComponent {
    /// A receiver at its defaults. With `sbas`, the receiver of the `sbas` stack (SBAS alone).
    pub fn new(sbas: bool) -> Self {
        let values = ReceiverValues::defaults(sbas);
        let cfg = values.core_config(sbas, None);
        Self::make(GnssCore::new(cfg), values, sbas)
    }

    /// A receiver with this configuration, for tests that set what the settings cannot (a synthetic Galileo table).
    pub fn with_config(cfg: CoreConfig) -> Self {
        let sbas = cfg.sbas_mode;
        Self::make(GnssCore::new(cfg), ReceiverValues::defaults(sbas), sbas)
    }

    fn make(core: GnssCore, values: ReceiverValues, sbas: bool) -> Self {
        let results = receiver_results();
        GnssCoreComponent {
            core,
            values,
            sbas,
            progress: String::new(),
            series: series_ids(&results, &RECEIVER_SERIES),
            results,
            sats: ObjectTable::new(SATELLITES.cap),
            reports: 0,
            seen_log: 0,
            live: false,
            galileo_read: true,
        }
    }

    fn apply_receiver(
        &mut self,
        s: ReceiverSetting,
        id: SettingId,
        v: &Value,
    ) -> Result<(), SettingError> {
        let mut replan = false;
        let r = &mut self.values;
        let system = |r: &mut ReceiverValues, sys: System, on: bool| {
            if on {
                r.systems |= sys.bit();
            } else {
                r.systems &= !sys.bit();
            }
        };
        match s {
            ReceiverSetting::SysGps => system(r, System::Gps, SYS_GPS.read(id, v)?),
            ReceiverSetting::SysQzss => system(r, System::Qzss, SYS_QZSS.read(id, v)?),
            ReceiverSetting::SysSbas => system(r, System::Sbas, SYS_SBAS.read(id, v)?),
            ReceiverSetting::SysGalileo => system(r, System::Galileo, SYS_GALILEO.read(id, v)?),
            ReceiverSetting::SysBeidou => system(r, System::Beidou, SYS_BEIDOU.read(id, v)?),
            ReceiverSetting::SysGlonass => system(r, System::Glonass, SYS_GLONASS.read(id, v)?),
            ReceiverSetting::ElevationMask => r.elevation_mask = ELEVATION_MASK.read(id, v)?,
            ReceiverSetting::SearchHalfWidth => {
                r.search_half_width = SEARCH_HALF_WIDTH.read(id, v)?;
                replan = true;
            }
            ReceiverSetting::FrequencyHint => r.frequency_hint = FREQUENCY_HINT.read(id, v)?,
            ReceiverSetting::FrequencyHintValid => {
                r.frequency_hint_valid = FREQUENCY_HINT_VALID.read(id, v)?;
            }
            ReceiverSetting::AcquisitionEffort => r.effort = ACQUISITION_EFFORT.read(id, v)?,
            ReceiverSetting::KnownWeek => r.known_week = KNOWN_WEEK.read(id, v)?,
            ReceiverSetting::ClockCompare => r.clock_compare = CLOCK_COMPARE.read(id, v)?,
            ReceiverSetting::GalileoCodes => {
                let path = GALILEO_CODES.read(id, v)?;
                if path != r.galileo_codes {
                    r.galileo_codes = path.to_owned();
                    self.galileo_read = false;
                }
            }
        }
        let keep = self.core.cfg.galileo.take();
        let mut cfg = self.values.core_config(self.sbas, keep);
        if self.sbas {
            cfg.week_ref = self.core.cfg.week_ref;
        }
        // These come from the station, not from the settings.
        cfg.approx_position = self.core.cfg.approx_position;
        cfg.leap_table = self.core.cfg.leap_table;
        replan |= cfg.systems != self.core.cfg.systems;
        self.core.nav.week_ref = cfg.week_ref;
        self.core.cfg = cfg;
        if replan && self.core.ready() {
            self.core.plan();
        }
        Ok(())
    }

    fn publish(&mut self) {
        let now = self.core.signal_s();
        let d = self.core.last_report();
        let p = &mut self.results;
        let (good, badn) = self.core.frame_counts();
        let tracked = d.channels.iter().filter(|c| c.state_level >= 1).count();
        let best = d
            .channels
            .iter()
            .filter(|c| c.state_level >= 1)
            .map(|c| c.cn0)
            .fold(0.0, f64::max);
        p.results_mut().update_status(|st| {
            let lock_state = match d.state {
                2 => LockState::Locked,
                1 => LockState::Tracking,
                _ => LockState::Searching,
            };
            let mut c = upd(&mut st.lock_state, lock_state);
            c |= upd(&mut st.snr_db, (best > 0.0).then_some(best as f32));
            c |= upd(&mut st.level_dbfs, Some(d.front_end.level_dbfs as f32));
            c |= upd(&mut st.offset_hz, d.frequency_error_hz);
            if st.summary != d.status_line {
                st.summary.clone_from(&d.status_line);
                c = true;
            }
            c
        });
        if !lines_equal(&self.progress, &d.progress) {
            self.progress = d.progress.join("\n");
        }
        let s = &d.search;
        let running = s.running;
        let measured = d.frequency_error_hz.filter(|_| d.fix.is_some());
        let f = d.fix.as_ref();
        let leap = d.utc.map(|u| u.dtls).or_else(|| f.and_then(|f| f.leap));
        let moving = f.filter(|f| f.has_velocity);
        p.put(&GnssFields {
            centre_hz: Hertz(d.centre_hz),
            input_rate: Hertz(d.input_rate),
            search_system: s.system.filter(|_| running),
            search_prn: running.then_some(i64::from(s.prn)),
            search_progress_pct: running.then_some(s.progress * 100.0),
            search_round: running.then_some(i64::from(s.rounds) + 1),
            search_centre: running.then_some(Hertz(s.centre_hz)),
            search_half_width: running.then_some(Hertz(s.half_hz)),
            search_integration_ms: running.then_some(s.ms),
            frequency_error: measured.map(Hertz),
            frequency_error_ppm: measured.map(|f| f / L1_HZ * 1e6),
            frequency_source: match (measured, running) {
                (Some(_), _) => res::FrequencySource::Measured,
                (None, true) => res::FrequencySource::SearchCentre,
                (None, false) => res::FrequencySource::Unknown,
            },
            first_lock: s.first_lock_s.map(Seconds),
            frames_ok: good as i64,
            frames_bad: badn as i64,
            sbas_messages: d.sbas_total as i64,
            glonass_tau_gps_s: f.and_then(|f| f.glo_tau_gps_ns).map(|v| v * 1e-9),
            glonass_tau_c_s: f.and_then(|f| f.glo_tau_c_ns).map(|v| v * 1e-9),
            bdt_gps_s: f.and_then(|f| f.bdt_offset_ns).map(|v| v * 1e-9),
            almanac_satellites: d.almanac_count as i64,
            iono_broadcast: d.iono_valid,
            leap_seconds: leap.map(i64::from),
            leap_seconds_broadcast: d.utc.is_some(),
            level_dbfs: d.front_end.level_dbfs,
            clipped_pct: d.front_end.clip_percent,
            dc_i: d.front_end.dc_i,
            dc_q: d.front_end.dc_q,
            signal: Seconds(d.signal_s),
            progress: &self.progress,
            position: f.map(|f| (f.lat, f.lon)),
            height_m: f.map(|f| f.height),
            error_h_m: f.map(|f| f.herr),
            error_v_m: f.map(|f| f.verr),
            hdop: f.map(|f| f.hdop),
            vdop: f.map(|f| f.vdop),
            pdop: f.map(|f| f.pdop),
            satellites_used: f.map(|f| f.satellites as i64),
            satellites_tracked: tracked as i64,
            utc: f.and_then(|f| f.utc).map(Time),
            gps_week: f.map(|f| f.week),
            gps_tow_s: f.map(|f| f.tow),
            speed_kt: moving.map(|f| f.speed * 3600.0 / 1852.0),
            course_deg: moving.and_then(|f| f.course),
            clock_bias_m: f.map(|f| f.clock_bias_m),
            first_fix: f.map(|f| Seconds(f.first_fix_s)),
        });
        for s in System::ALL {
            let wanted = d.wanted_mask & s.bit() != 0;
            let aux = s.bit() & (System::Qzss.bit() | System::Sbas.bit()) != 0;
            let active = wanted
                && (d.active_mask & s.bit() != 0
                    || (aux && d.active_mask & System::Gps.bit() != 0));
            SYSTEMS.put(
                p,
                s.key(),
                &GnssSystemRow {
                    system: s,
                    wanted,
                    active,
                    searched: d.searched_mask & s.bit() != 0,
                },
            );
        }
        for sys in System::ALL {
            let in_fix =
                f.and_then(|f| f.systems.iter().find(|x| x.system == Some(sys) && x.in_fix));
            match in_fix {
                Some(x) => FIX_SYSTEMS.put(
                    p,
                    sys.key(),
                    &GnssFixSystemRow {
                        system: sys,
                        satellites: x.satellites as i64,
                        clock_m: x.clock_m,
                    },
                ),
                None => {
                    p.remove_row(&FIX_SYSTEMS, sys.key());
                }
            }
        }
        // The satellite table: one row a satellite, from the four lists of the report.
        self.reports += 1;
        let report = self.reports;
        let sats = &mut self.sats;
        for c in &d.channels {
            let Some(r) = sat_row(sats, &c.name, now, report) else {
                continue;
            };
            r.system = c.system;
            r.prn = c.sat.map(|s| i64::from(s.prn));
            r.glo_channel = c.glo_k.map(i64::from);
            r.tracked = true;
            r.state = c.state;
            r.cn0 = Some(c.cn0);
            r.doppler = Some(c.doppler);
            r.code_chips = Some(c.code_chips);
            r.az = c.az;
            r.el = c.el;
            r.locked_for = Some(c.lock_s);
            r.frames_good = Some(c.good as i64);
            r.frames = Some((c.good + c.bad) as i64);
            r.residual = c.residual;
            r.healthy = c.health;
            r.used = c.in_fix;
        }
        for n in &d.navigation {
            let Some(r) = sat_row(sats, &n.name, now, report) else {
                continue;
            };
            r.ephemeris = n.ephemeris;
            r.parts = n.parts;
            r.seconds_left = n.seconds_left;
            r.iod = n.iod;
            r.data_age = n.age_s;
            r.week = n.week;
            r.tow = n.tow;
            r.af0 = n.af0_us.map(|v| v * 1e-6);
        }
        for e in &d.sky {
            let Some(r) = sat_row(sats, &e.name, now, report) else {
                continue;
            };
            r.system = e.system;
            r.prn.get_or_insert(i64::from(e.prn));
            r.az = Some(e.az);
            r.el = Some(e.el);
            r.used |= e.used;
            r.healthy = Some(e.healthy);
        }
        for e in &d.satellites {
            let Some(r) = sat_row(sats, &e.name, now, report) else {
                continue;
            };
            r.system = e.system;
            r.sub_point = Some((e.lat, e.lon));
            r.height_km = Some(e.height_km);
            r.healthy = Some(e.healthy);
            r.orbit = Some(if e.from_ephemeris {
                OrbitSource::Ephemeris
            } else {
                OrbitSource::Almanac
            });
            if e.from_ephemeris {
                r.data_age.get_or_insert(e.age_s);
            }
            r.public = e.public;
        }
        sats.retain(|r| r.record.report == report);
        shaped::rows(sats, &SATELLITES, now, p);
        let r = p.results_mut();
        shaped::points(sats, "sat/", r);
        match f {
            Some(f) => {
                r.update_point(
                    "fix",
                    f.lat,
                    f.lon,
                    &f.type_text,
                    f.course.map(|c| c as f32),
                );
            }
            None => {
                r.remove_point("fix");
            }
        }
        let [acq, scatter, level, best_cn0, clock] = self.series;
        if let Some(a) = &d.acq_plot {
            r.update_series(acq, |s| {
                s.fill(
                    true,
                    a.points.iter().enumerate().map(|(i, v)| [i as f32, *v]),
                )
            });
        }
        r.update_series(scatter, |s| s.fill(false, d.scatter.iter().copied()));
        for (id, h) in [
            (level, &d.level_history),
            (best_cn0, &d.cn0_history),
            (clock, &d.clock_history),
        ] {
            r.update_series(id, |s| {
                s.fill(true, h.iter().enumerate().map(|(i, v)| [i as f32, *v]))
            });
        }
        // Log lines new since the last report.
        let log = &self.core.log;
        let total = log.len();
        for l in log.iter().skip(self.seen_log.min(total)) {
            r.log(l.clone());
        }
        self.seen_log = total;
    }
}

/// Whether `text` is `lines` joined with new lines.
fn lines_equal(text: &str, lines: &[String]) -> bool {
    let mut parts = text.split('\n');
    if lines.is_empty() {
        return text.is_empty();
    }
    lines.iter().all(|l| parts.next() == Some(l.as_str())) && parts.next().is_none()
}

impl Component for GnssCoreComponent {
    fn info(&self) -> ComponentInfo {
        let (name, schema) = if self.sbas {
            ("gnss.sbas_receiver", SbasReceiverSetting::schema())
        } else {
            ("gnss.receiver", ReceiverSetting::schema())
        };
        ComponentInfo::processor(
            name,
            vec![Port::iq("in").rate(RateRule::AtLeast(GnssCore::MIN_RATE))],
            vec![
                typed_out::<AcqHit>("hits"),
                typed_out::<NavFrame>("frames"),
                typed_out::<EphemerisItem>("ephemeris"),
                typed_out::<MeasEpoch>("measurement"),
                typed_out::<Fix>("fix"),
                typed_out::<SbasMessage>("sbas"),
                typed_out::<ChannelTable>("channels"),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(schema)
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let centre = if f.centre_hz > 0.0 {
            f.centre_hz
        } else {
            L1_HZ
        };
        let read = !self.galileo_read;
        if read {
            self.core.cfg.galileo = galileo_table(
                &self.values.galileo_codes,
                ReceiverSetting::GalileoCodes.id(),
            )?;
            self.galileo_read = true;
        }
        self.core.configure(f.rate, centre, self.live)?;
        if read {
            self.core.plan();
        }
        Ok(vec![
            typed_fmt::<AcqHit>(),
            typed_fmt::<NavFrame>(),
            typed_fmt::<EphemerisItem>(),
            typed_fmt::<MeasEpoch>(),
            typed_fmt::<Fix>(),
            typed_fmt::<SbasMessage>(),
            typed_fmt::<ChannelTable>(),
            Format::new(Kind::Messages, 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        if io
            .input(0)
            .tags()
            .iter()
            .any(|t| matches!(t.kind, TagKind::Discontinuity { .. }))
        {
            self.core.discontinuity();
        }
        self.core.feed(io.iq(0));
        if self.live && self.core.cfg.clock_compare {
            if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                self.core.clock_sample(d.as_secs_f64());
            }
        }
        let c = &mut self.core;
        io.items_out::<AcqHit>(0)
            .extend(c.out_hits.drain(..).map(timed));
        io.items_out::<NavFrame>(1)
            .extend(c.out_frames.drain(..).map(timed));
        io.items_out::<EphemerisItem>(2)
            .extend(c.out_eph.drain(..).map(|(time, (sat, eph))| Timed {
                time,
                value: EphemerisItem { sat, eph },
            }));
        io.items_out::<MeasEpoch>(3)
            .extend(c.out_meas.drain(..).map(timed));
        io.items_out::<Fix>(4)
            .extend(c.out_fix.drain(..).map(timed));
        io.items_out::<SbasMessage>(5)
            .extend(c.out_sbas.drain(..).map(timed));
        io.items_out::<ChannelTable>(6)
            .extend(c.out_channels.drain(..).map(|(time, v)| Timed {
                time,
                value: ChannelTable(v),
            }));
        io.messages_out(7).append(&mut c.events);
        if self.core.report_due {
            self.core.report_due = false;
            self.publish();
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let s = if self.sbas {
            SbasReceiverSetting::from_id(id)
                .ok_or(SettingError::NoSuchId { id })?
                .receiver()
        } else {
            ReceiverSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?
        };
        self.apply_receiver(s, id, v)?;
        Ok(if self.galileo_read {
            Applied::Live
        } else {
            Applied::Reformat
        })
    }

    fn values(&self) -> Values {
        let r = &self.values;
        let sys = |s: System| Value::Bool(r.systems & s.bit() != 0);
        if self.sbas {
            return values_of([
                (
                    SbasReceiverSetting::SearchHalfWidth,
                    Value::Hz(r.search_half_width),
                ),
                (
                    SbasReceiverSetting::FrequencyHint,
                    Value::Hz(r.frequency_hint),
                ),
                (
                    SbasReceiverSetting::FrequencyHintValid,
                    Value::Bool(r.frequency_hint_valid),
                ),
                (SbasReceiverSetting::AcquisitionEffort, Value::F64(r.effort)),
            ]);
        }
        values_of([
            (ReceiverSetting::SysGps, sys(System::Gps)),
            (ReceiverSetting::SysQzss, sys(System::Qzss)),
            (ReceiverSetting::SysSbas, sys(System::Sbas)),
            (ReceiverSetting::SysGalileo, sys(System::Galileo)),
            (ReceiverSetting::SysBeidou, sys(System::Beidou)),
            (ReceiverSetting::SysGlonass, sys(System::Glonass)),
            (ReceiverSetting::ElevationMask, Value::F64(r.elevation_mask)),
            (
                ReceiverSetting::SearchHalfWidth,
                Value::Hz(r.search_half_width),
            ),
            (ReceiverSetting::FrequencyHint, Value::Hz(r.frequency_hint)),
            (
                ReceiverSetting::FrequencyHintValid,
                Value::Bool(r.frequency_hint_valid),
            ),
            (ReceiverSetting::AcquisitionEffort, Value::F64(r.effort)),
            (ReceiverSetting::KnownWeek, Value::I64(r.known_week)),
            (ReceiverSetting::ClockCompare, Value::Bool(r.clock_compare)),
            (
                ReceiverSetting::GalileoCodes,
                Value::Text(r.galileo_codes.clone()),
            ),
        ])
    }

    fn commands(&self) -> &Commands {
        ReceiverCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match ReceiverCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ReceiverCommand::Reset => self.core.reset(),
            ReceiverCommand::Remove => {
                let row = args.arg(&SATELLITE_ROW)?;
                if !sdrkit_util_objects::rows::remove_row(&mut self.sats, row) {
                    return Err(self.results.no_such_row(&SATELLITES, row));
                }
                self.results.remove_row(&SATELLITES, row);
            }
        }
        Ok(None)
    }

    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        // The station: its leap-second table, and its position as the search's approximate position.
        let st = &ctx.station;
        self.core.set_leap_table(st.leap_seconds());
        self.core.cfg.approx_position = st.position.map(|p| (p.lat_deg, p.lon_deg));
    }

    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

// Stages that work on their own.

/// Input conditioning (`gnss.input_conditioning`) in pieces of 16 384 samples, so any cut produces the same output.
pub struct ConditioningComponent {
    cond: Conditioner,
    pending: Vec<Complex32>,
    rate: f64,
}

impl Default for ConditioningComponent {
    fn default() -> Self {
        ConditioningComponent {
            cond: Conditioner::default(),
            pending: Vec::with_capacity(crate::core::PIECE),
            rate: 0.0,
        }
    }
}

impl Component for ConditioningComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "gnss.input_conditioning",
            vec![Port::iq("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = ConditioningComponent {
            rate: inputs[0].rate,
            ..Default::default()
        };
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let mut i = 0;
        while i < x.len() {
            let take = (crate::core::PIECE - self.pending.len()).min(x.len() - i);
            self.pending.extend_from_slice(&x[i..i + take]);
            i += take;
            if self.pending.len() == crate::core::PIECE {
                self.cond.process(&mut self.pending, self.rate);
                io.iq_out(0).extend_from_slice(&self.pending);
                self.pending.clear();
            }
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// Narrowband interference excision (`gnss.excision`).
#[derive(Default)]
pub struct ExcisionComponent {
    e: Excision,
}

impl Component for ExcisionComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "gnss.excision",
            vec![Port::iq("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.e = Excision::default();
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let mut out = std::mem::take(io.iq_out(0));
        self.e.process(x, &mut out);
        *io.iq_out(0) = out;
    }
    fn values(&self) -> Values {
        no_values()
    }
}

const ACQ_SYSTEM: Setting<field::Choice<System>> = Setting::choice("system", "System", System::Gps);
const ACQ_CENTRE: Setting<field::Hz> = Setting::new("centre_hz", "Window centre", "Hz", 0.0)
    .range(-200e3, 200e3, 0.0)
    .help("The centre of the Doppler window, as an error at 1575.42 MHz.");
const ACQ_HALF_WIDTH: Setting<field::Hz> =
    Setting::new("half_width_hz", "Window half width", "Hz", 10e3).range(500.0, 200e3, 0.0);
const ACQ_PERIODS: Setting<field::I64> =
    Setting::new("periods", "Code periods", "", 16).range(2.0, 64.0, 2.0);
const ACQ_EFFORT: Setting<field::F64> =
    Setting::new("effort", "Effort", "FFT/ms", 10.0).range(1.0, 100.0, 1.0);
const ACQ_GALILEO: Setting<field::Text> =
    Setting::new("galileo_codes", "Galileo code table", "", "")
        .help("A text file with the E1-B memory codes of the Galileo OS SIS ICD, Annex C.");

sdrkit_core::settings! {
    /// The settings of [`AcquisitionComponent`].
    pub enum AcquisitionSetting {
        /// System.
        System = ACQ_SYSTEM,
        /// Window centre.
        Centre = ACQ_CENTRE,
        /// Window half width.
        HalfWidth = ACQ_HALF_WIDTH,
        /// Code periods.
        Periods = ACQ_PERIODS,
        /// Effort.
        Effort = ACQ_EFFORT,
        /// Galileo code table.
        GalileoCodes = ACQ_GALILEO,
    }
}

/// A search on one band (`gnss.acquisition`): rounds over a system's PRNs on fresh segments, within a work budget, with a hit
/// out for each satellite found. The input is the band at the system's carrier and working rate.
pub struct AcquisitionComponent {
    system: System,
    centre: f64,
    half: f64,
    periods: usize,
    effort: f64,
    searcher: Option<Searcher>,
    prns: Vec<u16>,
    pos: usize,
    job: Option<Search>,
    buf: Vec<Complex32>,
    buf_start: u64,
    budget: f64,
    rate: f64,
    galileo_codes: String,
    galileo: Option<codes::E1bTable>,
}

impl Default for AcquisitionComponent {
    fn default() -> Self {
        AcquisitionComponent {
            system: ACQ_SYSTEM.default,
            centre: ACQ_CENTRE.default,
            half: ACQ_HALF_WIDTH.default,
            periods: ACQ_PERIODS.default as usize,
            effort: ACQ_EFFORT.default,
            searcher: None,
            prns: Vec::new(),
            pos: 0,
            job: None,
            buf: Vec::new(),
            buf_start: 0,
            budget: 0.0,
            rate: 0.0,
            galileo_codes: String::new(),
            galileo: None,
        }
    }
}

impl AcquisitionComponent {
    fn kind(&self) -> SignalKind {
        SignalKind::of(self.system)
    }
    fn chips(&self, prn: u16) -> Option<Vec<u8>> {
        match self.system {
            System::Galileo => self
                .galileo
                .as_ref()
                .and_then(|t| t.code(prn))
                .map(|c| c.to_vec()),
            System::Beidou => codes::bds_b1i(prn),
            System::Glonass => Some(codes::glonass_code()),
            _ => codes::l1_ca(prn),
        }
    }
}

impl Component for AcquisitionComponent {
    fn info(&self) -> ComponentInfo {
        let kind = self.kind();
        ComponentInfo::processor(
            "gnss.acquisition",
            vec![Port::iq("in")
                .rate(RateRule::Exact(kind.band_rate()))
                .centre(self.system.carrier_hz())],
            vec![typed_out::<AcqHit>("hits")],
        )
        .with_settings(AcquisitionSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        let kind = self.kind();
        if (self.rate - kind.band_rate()).abs() > 1.0 {
            return Err(ConfigError::WrongRate {
                need: kind.band_rate(),
                got: self.rate,
            });
        }
        self.galileo = galileo_table(&self.galileo_codes, AcquisitionSetting::GalileoCodes.id())?;
        self.searcher = Some(Searcher::new(kind));
        self.prns = match self.system {
            System::Gps => (1..=32).collect(),
            System::Qzss => (193..=202).collect(),
            System::Sbas => (120..=158).collect(),
            System::Galileo => {
                let n = self.galileo.as_ref().map_or(0, |t| t.len().min(36) as u16);
                (1..=n).collect()
            }
            System::Beidou => (1..=codes::BDS_MAX_PRN).collect(),
            System::Glonass => vec![0],
        };
        self.pos = 0;
        self.job = None;
        self.buf.clear();
        self.buf_start = 0;
        Ok(vec![typed_fmt::<AcqHit>()])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let first = io.input(0).first();
        if self.buf.is_empty() {
            self.buf_start = first;
        }
        self.buf.extend_from_slice(x);
        let kind = self.kind();
        let n = kind.period_samples();
        let need = self.periods * n;
        if self.buf.len() > 4 * need {
            let drop = self.buf.len() - 2 * need;
            self.buf.drain(..drop);
            self.buf_start += drop as u64;
        }
        self.budget = (self.budget + self.effort * x.len() as f64 / self.rate * 1e3).min(600.0);
        let carrier = self.system.carrier_hz();
        loop {
            if self.prns.is_empty() || self.budget < 8.0 {
                return;
            }
            if self.job.is_none() {
                if self.buf.len() < need {
                    return;
                }
                let prn = self.prns[self.pos % self.prns.len()];
                let Some(chips) = self.chips(prn) else {
                    self.pos += 1;
                    continue;
                };
                let Some(s) = self.searcher.as_mut() else {
                    return;
                };
                s.add_code(prn, &chips);
                let start = self.buf.len() - need;
                let integ = Integration {
                    periods: self.periods,
                    coherent: if kind == SignalKind::L1Ca { 2 } else { 1 },
                };
                let subs = if kind == SignalKind::E1b { 2 } else { 4 };
                self.job = Some(s.start(
                    prn,
                    &self.buf[start..],
                    self.buf_start + start as u64,
                    self.centre * carrier / L1_HZ,
                    self.half,
                    subs,
                    integ,
                    carrier,
                    0.0,
                    1e-3,
                    None,
                ));
            }
            let (Some(s), Some(mut job)) = (self.searcher.as_mut(), self.job.take()) else {
                return;
            };
            match s.step(&mut job, &mut self.budget) {
                None => {
                    self.job = Some(job);
                    return;
                }
                Some((hit, _)) => {
                    self.pos += 1;
                    if hit.found {
                        let time = hit.seg_start as f64 / self.rate;
                        io.items_out::<AcqHit>(0).push(Timed { time, value: hit });
                    }
                }
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AcquisitionSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AcquisitionSetting::System => {
                self.system = ACQ_SYSTEM.read(id, v)?;
                return Ok(Applied::Reformat);
            }
            AcquisitionSetting::Centre => self.centre = ACQ_CENTRE.read(id, v)?,
            AcquisitionSetting::HalfWidth => self.half = ACQ_HALF_WIDTH.read(id, v)?,
            AcquisitionSetting::Periods => {
                let n = ACQ_PERIODS.read(id, v)?;
                if n % 2 != 0 {
                    return Err(SettingError::NotPermitted { id });
                }
                self.periods = n as usize;
            }
            AcquisitionSetting::Effort => self.effort = ACQ_EFFORT.read(id, v)?,
            AcquisitionSetting::GalileoCodes => {
                self.galileo_codes = ACQ_GALILEO.read(id, v)?.to_owned();
                return Ok(Applied::Reformat);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                AcquisitionSetting::System,
                Value::Choice(self.system.index()),
            ),
            (AcquisitionSetting::Centre, Value::Hz(self.centre)),
            (AcquisitionSetting::HalfWidth, Value::Hz(self.half)),
            (AcquisitionSetting::Periods, Value::I64(self.periods as i64)),
            (AcquisitionSetting::Effort, Value::F64(self.effort)),
            (
                AcquisitionSetting::GalileoCodes,
                Value::Text(self.galileo_codes.clone()),
            ),
        ])
    }
}

/// The known GPS week of the stand-alone navigation stages.
const STAGE_WEEK: Setting<field::I64> = Setting::new("known_week", "Known week", "", 2400)
    .range(0.0, 9999.0, 1.0)
    .help("The GPS week used to resolve broadcast weeks.");

sdrkit_core::settings! {
    /// The settings of [`NavStoreComponent`].
    pub enum NavStoreSetting {
        /// Known week.
        KnownWeek = STAGE_WEEK,
    }
}

/// The navigation data store (`gnss.nav_store`): frames in, ephemerides out.
pub struct NavStoreComponent {
    nav: NavStore,
}

impl Default for NavStoreComponent {
    fn default() -> Self {
        NavStoreComponent {
            nav: NavStore::new(STAGE_WEEK.default),
        }
    }
}

impl Component for NavStoreComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "gnss.nav_store",
            vec![Port::typed::<NavFrame>("frames")],
            vec![typed_out::<EphemerisItem>("ephemeris")],
        )
        .with_settings(NavStoreSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![typed_fmt::<EphemerisItem>()])
    }
    fn process(&mut self, io: &mut Io) {
        for f in io.items::<NavFrame>(0) {
            let k = f.value.sat.glo_k().unwrap_or(0);
            for n in self
                .nav
                .add(f.value.sat, k, &f.value.unit, f.value.time, f.time)
            {
                if let crate::navstore::NavNews::Ephemeris(sat, eph) = n {
                    io.items_out::<EphemerisItem>(0).push(Timed {
                        time: f.time,
                        value: EphemerisItem { sat, eph },
                    });
                }
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match NavStoreSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            NavStoreSetting::KnownWeek => self.nav.week_ref = STAGE_WEEK.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(NavStoreSetting::KnownWeek, Value::I64(self.nav.week_ref))])
    }
}

const PVT_MASK: Setting<field::F64> = Setting::new("elevation_mask", "Elevation mask", "°", 5.0)
    .range(0.0, 30.0, 1.0)
    .help("Satellites lower than this are left out of the position.");

sdrkit_core::settings! {
    /// The settings of [`PvtComponent`].
    pub enum PvtSetting {
        /// Elevation mask.
        ElevationMask = PVT_MASK,
        /// Known week.
        KnownWeek = STAGE_WEEK,
    }
}

/// The position solver (`gnss.pvt`): frames (for the navigation data) and measurements in, fixes out.
pub struct PvtComponent {
    nav: NavStore,
    solver: FixSolver,
}

impl Default for PvtComponent {
    fn default() -> Self {
        PvtComponent {
            nav: NavStore::new(STAGE_WEEK.default),
            solver: FixSolver::new(STAGE_WEEK.default, PVT_MASK.default),
        }
    }
}

impl Component for PvtComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "gnss.pvt",
            vec![
                Port::typed::<NavFrame>("frames"),
                Port::typed::<MeasEpoch>("measurement"),
            ],
            vec![typed_out::<Fix>("fix")],
        )
        .with_settings(PvtSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![typed_fmt::<Fix>()])
    }
    fn process(&mut self, io: &mut Io) {
        for f in io.items::<NavFrame>(0) {
            let k = f.value.sat.glo_k().unwrap_or(0);
            self.nav
                .add(f.value.sat, k, &f.value.unit, f.value.time, f.time);
        }
        for m in io.items::<MeasEpoch>(1) {
            if let Some(fix) = self.solver.solve(&self.nav, &m.value, m.value.rx_time) {
                io.items_out::<Fix>(0).push(Timed {
                    time: m.time,
                    value: fix,
                });
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PvtSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PvtSetting::ElevationMask => self.solver.mask_deg = PVT_MASK.read(id, v)?,
            PvtSetting::KnownWeek => {
                let w = STAGE_WEEK.read(id, v)?;
                self.nav.week_ref = w;
                self.solver.week_ref = w;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (PvtSetting::ElevationMask, Value::F64(self.solver.mask_deg)),
            (PvtSetting::KnownWeek, Value::I64(self.nav.week_ref)),
        ])
    }
}

/// The interference and spoofing monitor (`gnss.interference`): the input, and the receiver's measurements and fixes, in. Its
/// verdict in its results.
pub struct MonitorComponent {
    mon: Option<Monitor>,
    pending: Vec<Complex32>,
    dc: (f64, f64),
    dc_started: bool,
    rate: f64,
    view: RxView,
    results: Published,
    series: [SeriesId; 2],
    events_written: u64,
    key: String,
    unix0: f64,
}

impl Default for MonitorComponent {
    fn default() -> Self {
        let results = Published::from_decl(Arc::new(res::monitor_declaration()));
        MonitorComponent {
            mon: None,
            pending: Vec::with_capacity(crate::core::PIECE),
            dc: (0.0, 0.0),
            dc_started: false,
            rate: 0.0,
            view: RxView::default(),
            series: series_ids(&results, &MONITOR_SERIES),
            results,
            events_written: 0,
            key: String::new(),
            unix0: 0.0,
        }
    }
}

impl MonitorComponent {
    fn publish(&mut self) {
        let Some(m) = &self.mon else { return };
        let rep = &m.report;
        let p = &mut self.results;
        p.put(&InterferenceFields {
            interference: rep.status,
            swept: rep.kinds.swept,
            pulsed: rep.kinds.pulsed,
            narrowband: rep.kinds.narrowband,
            wideband: rep.kinds.wideband,
            interference_reason: &rep.reason,
            noise_floor_dbfs: rep.noise_dbfs_hz,
            band_power_dbfs: rep.power_dbfs,
            flatness: rep.flatness,
            spoofing: rep.spoofing,
            spoofing_reason: rep.spoofing.then_some(rep.spoof_reason.as_str()),
        });
        let key = &mut self.key;
        for (i, l) in rep
            .lines
            .iter()
            .enumerate()
            .take(INTERFERENCE_LINES.cap)
        {
            key.clear();
            let _ = write!(key, "{i:02}");
            INTERFERENCE_LINES.put(
                p,
                key,
                &InterferenceLineRow {
                    offset: Hertz(l.offset_hz),
                    level_db: l.level_db,
                    known: l.known,
                },
            );
        }
        for i in rep.lines.len()..INTERFERENCE_LINES.cap {
            key.clear();
            let _ = write!(key, "{i:02}");
            if !p.remove_row(&INTERFERENCE_LINES, key) {
                break;
            }
        }
        for (seq, e) in m.events().filter(|(seq, _)| *seq >= self.events_written) {
            key.clear();
            let _ = write!(key, "{seq:06}");
            INTERFERENCE_EVENTS.put(
                p,
                key,
                &InterferenceEventRow {
                    at: Seconds(e.signal_s),
                    level: e.level,
                    text: &e.text,
                },
            );
        }
        self.events_written = m.events_added();
        let r = p.results_mut();
        let [spectrum, history] = self.series;
        // The band spectrum at 256 points (the strongest of each four bins) and the newest 300 history points.
        r.update_series(spectrum, |s| {
            s.fill(
                true,
                rep.spectrum
                    .chunks(4)
                    .enumerate()
                    .map(|(i, c)| [i as f32 * 4.0, c.iter().copied().fold(f32::MIN, f32::max)]),
            )
        });
        let skip = m.history().count().saturating_sub(300);
        r.update_series(history, |s| {
            s.fill(
                true,
                m.history()
                    .skip(skip)
                    .map(|h| [h.signal_s as f32, h.noise_dbfs_hz as f32]),
            )
        });
    }
}

impl Component for MonitorComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "gnss.interference",
            vec![
                Port::iq("in"),
                Port::typed::<MeasEpoch>("measurement"),
                Port::typed::<Fix>("fix"),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.mon = Some(Monitor::new(self.rate));
        self.pending.clear();
        self.dc_started = false;
        self.unix0 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(m) = io.items::<MeasEpoch>(1).last() {
            self.view.cn0.clear();
            self.view.cn0.extend(m.value.sats.iter().map(|s| s.cn0));
        }
        for f in io.items::<Fix>(2) {
            let fx = &f.value;
            self.view.fix = Some((fx.signal_s, fx.ecef, fx.herr, fx.speed, fx.clock_bias_m));
        }
        let x = io.iq(0);
        let first = io.input(0).first();
        let mut i = 0;
        let mut published = false;
        while i < x.len() {
            let take = (crate::core::PIECE - self.pending.len()).min(x.len() - i);
            self.pending.extend_from_slice(&x[i..i + take]);
            i += take;
            if self.pending.len() < crate::core::PIECE {
                continue;
            }
            let n = self.pending.len() as f64;
            let (si, sq) = self
                .pending
                .iter()
                .fold((0f64, 0f64), |a, s| (a.0 + s.re as f64, a.1 + s.im as f64));
            let mean = (si / n, sq / n);
            if !self.dc_started {
                self.dc = mean;
                self.dc_started = true;
            }
            let a = 1.0 - (-n / self.rate / 0.3).exp();
            let Some(mon) = self.mon.as_mut() else {
                return;
            };
            if mon.piece(&self.pending, self.dc) {
                let secs = (first + i as u64) as f64 / self.rate;
                mon.second(&self.view, self.unix0 + secs);
                self.view.fix = None;
                published = true;
            }
            self.dc.0 += a * (mean.0 - self.dc.0);
            self.dc.1 += a * (mean.1 - self.dc.1);
            self.pending.clear();
        }
        if published {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

/// The NMEA 0183 writer (`gnss.nmea`): RMC and GGA sentences of each fix.
#[derive(Default)]
pub struct NmeaComponent;

impl Component for NmeaComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "gnss.nmea",
            vec![Port::typed::<Fix>("fix")],
            vec![typed_out::<NmeaText>("out")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![typed_fmt::<NmeaText>()])
    }
    fn process(&mut self, io: &mut Io) {
        for f in io.items::<Fix>(0) {
            for s in nmea(&f.value) {
                io.items_out::<NmeaText>(0).push(Timed {
                    time: f.time,
                    value: NmeaText(s),
                });
            }
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// The RINEX 3.04 writer (`gnss.rinex`): observation and navigation text.
pub struct RinexComponent {
    obs: RinexObs,
    header_written: bool,
    week: i64,
}

impl Default for RinexComponent {
    fn default() -> Self {
        RinexComponent {
            obs: RinexObs::new("sdrkit", "SDRKIT", ""),
            header_written: false,
            week: 2400,
        }
    }
}

impl Component for RinexComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "gnss.rinex",
            vec![
                Port::typed::<MeasEpoch>("measurement"),
                Port::typed::<EphemerisItem>("ephemeris"),
            ],
            vec![typed_out::<RinexText>("out")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![typed_fmt::<RinexText>()])
    }
    fn process(&mut self, io: &mut Io) {
        for e in io.items::<EphemerisItem>(1) {
            if let (Ephemeris::Kepler(k), System::Gps) = (&e.value.eph, e.value.sat.system) {
                self.week = k.week;
                io.items_out::<RinexText>(0).push(Timed {
                    time: e.time,
                    value: RinexText {
                        navigation: true,
                        lines: rinex_nav(k),
                    },
                });
            }
        }
        for m in io.items::<MeasEpoch>(0) {
            let week = m.value.gps_time.map(|g| g.0).unwrap_or(self.week);
            let lines = self.obs.epoch(&m.value, week);
            if lines.is_empty() {
                continue;
            }
            if !self.header_written {
                self.header_written = true;
                io.items_out::<RinexText>(0).push(Timed {
                    time: m.time,
                    value: RinexText {
                        navigation: false,
                        lines: self.obs.header(),
                    },
                });
            }
            io.items_out::<RinexText>(0).push(Timed {
                time: m.time,
                value: RinexText {
                    navigation: false,
                    lines,
                },
            });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// Forgets every message decoded so far.
pub const CLEAR: Command =
    Command::new("clear", "Clear").help("Forgets every message decoded so far.");

sdrkit_core::commands! {
    /// The commands of [`SbasContentComponent`].
    pub enum SbasContentCommand {
        /// Select.
        Select = SBAS_SELECT.command,
        /// Clear.
        Clear = CLEAR,
    }
}

/// The SBAS content decoder (`gnss.sbas_content`): the messages and the receiver's channels in, the decoded content in its
/// results.
pub struct SbasContentComponent {
    /// The decoded content.
    pub content: SbasContent,
    channels: Vec<ChannelRow>,
    selected: u16,
    results: Published,
    last_report: f64,
    /// The PRNs (bit `prn - 120`) with a row in the satellite table, and the message types each satellite has a row for.
    rows: u64,
    types_written: Vec<(u16, u64)>,
    messages_written: u64,
    shown: u16,
    key: String,
    summary: String,
}

impl Default for SbasContentComponent {
    fn default() -> Self {
        SbasContentComponent {
            content: SbasContent::default(),
            channels: Vec::new(),
            selected: 0,
            results: Published::from_decl(Arc::new(res::sbas_content_declaration())),
            last_report: -1.0,
            rows: 0,
            types_written: Vec::new(),
            messages_written: 0,
            shown: 0,
            key: String::new(),
            summary: String::new(),
        }
    }
}

/// The first SBAS PRN.
const SBAS_FIRST_PRN: u16 = 120;

impl SbasContentComponent {
    fn clear(&mut self) {
        self.forget_all_but(&Kept::default());
    }

    /// Forgets the satellites with their corrections and the count of missed messages. The message log with the
    /// count of messages, and the counts by type, go too, each unless `kept` holds its table. The count of messages
    /// numbers the rows of the log, so it stays and goes with the log.
    fn forget_all_but(&mut self, kept: &Kept) {
        let c = &mut self.content;
        c.sats.clear();
        c.bad = 0;
        if !kept.table(SBAS_MESSAGES.key) {
            c.log.clear();
            c.total = 0;
            self.results.clear_rows(&SBAS_MESSAGES);
        }
        self.messages_written = c.total.saturating_sub(c.log.len() as u64);
        if !kept.table(SBAS_TYPES.key) {
            c.per_type.clear();
            self.results.clear_rows(&SBAS_TYPES);
        }
        self.results.clear_rows(&SBAS_SATELLITE_TYPES);
        self.results.clear_rows(&SBAS_FAST_CORRECTIONS);
        self.results.clear_rows(&SBAS_IONOSPHERE);
        self.types_written.clear();
        self.shown = 0;
    }

    fn publish(&mut self) {
        let p = &mut self.results;
        let c = &self.content;
        let key = &mut self.key;
        let sbas_ch = || {
            self.channels
                .iter()
                .filter(|ch| ch.sat.is_some_and(|s| s.system == System::Sbas))
        };
        let tracked = sbas_ch().filter(|ch| ch.state_level >= 1).count();
        let best = sbas_ch().map(|ch| ch.cn0).fold(0.0, f64::max);
        let with_mask = c.sats.values().filter(|s| s.mask.is_some()).count();
        let state = if c.total > 0 {
            2
        } else if tracked > 0 {
            1
        } else {
            0
        };
        let summary = &mut self.summary;
        summary.clear();
        let _ = match state {
            2 => write!(
                summary,
                "SBAS: {tracked} satellites tracked, {} messages, {with_mask} with a PRN mask, best {best:.0} dB-Hz",
                c.total
            ),
            1 => write!(summary, "tracking {tracked} satellites, best {best:.0} dB-Hz, no message yet"),
            _ => write!(summary, "searching (PRN 120 to 158)"),
        };
        p.results_mut().update_status(|st| {
            let mut ch = upd(
                &mut st.lock_state,
                [LockState::Searching, LockState::Tracking, LockState::Locked][state],
            );
            ch |= upd(&mut st.snr_db, (best > 0.0).then_some(best as f32));
            if st.summary != *summary {
                st.summary.clone_from(summary);
                ch = true;
            }
            ch
        });
        let valid = c
            .sats
            .values()
            .any(|s| s.mask.is_some() && s.fast.iter().flatten().any(|f| f.prc.is_some()));
        // The satellites that a channel tracks.
        let mut rows = 0u64;
        for ch in sbas_ch() {
            let Some(prn) = ch.sat.map(|s| s.prn) else {
                continue;
            };
            let Some(bit) = prn.checked_sub(SBAS_FIRST_PRN).filter(|b| *b < 64) else {
                continue;
            };
            rows |= 1 << bit;
            let s = c.sats.get(&prn);
            key.clear();
            let _ = write!(key, "{prn}");
            SBAS_SATELLITES.put(
                p,
                key,
                &SbasSatelliteRow {
                    prn: i64::from(prn),
                    service: sbas_service(prn),
                    state: ch.state,
                    cn0_dbhz: ch.cn0,
                    doppler: Hertz(ch.doppler),
                    messages: s.map_or(0, |s| s.messages as i64),
                    messages_missed: s.map_or(0, |s| s.bad as i64),
                },
            );
        }
        let gone = self.rows & !rows;
        for bit in (0..64).filter(|b| gone >> b & 1 == 1) {
            key.clear();
            let _ = write!(key, "{}", SBAS_FIRST_PRN + bit);
            p.remove_row(&SBAS_SATELLITES, key);
        }
        self.rows = rows;
        // The message types each satellite sent: one row for each new type.
        for (prn, s) in &c.sats {
            let at = match self.types_written.iter().position(|(p, _)| p == prn) {
                Some(at) => at,
                None => {
                    self.types_written.push((*prn, 0));
                    self.types_written.len() - 1
                }
            };
            let new = s.types_seen & !self.types_written[at].1;
            for ty in (0..64u8).filter(|t| new >> t & 1 == 1) {
                key.clear();
                let _ = write!(key, "{prn}/{ty}");
                SBAS_SATELLITE_TYPES.put(
                    p,
                    key,
                    &SbasSatelliteTypeRow {
                        prn: i64::from(*prn),
                        message_type: i64::from(ty),
                    },
                );
            }
            self.types_written[at].1 = s.types_seen;
        }
        for (ty, n) in &c.per_type {
            key.clear();
            let _ = write!(key, "{ty}");
            SBAS_TYPES.put(
                p,
                key,
                &SbasTypeRow {
                    message_type: i64::from(*ty),
                    name: sbas::type_label(*ty),
                    count: *n as i64,
                },
            );
        }
        // The message log: one row for each new message.
        let first = c.total - c.log.len() as u64;
        for (seq, m) in (first..)
            .zip(c.log.iter())
            .filter(|(seq, _)| *seq >= self.messages_written)
        {
            key.clear();
            let _ = write!(key, "{seq:06}");
            SBAS_MESSAGES.put(
                p,
                key,
                &SbasMessageRow {
                    at: Seconds(m.time_s),
                    prn: i64::from(m.prn),
                    message_type: i64::from(m.msg_type),
                    text: &m.text,
                },
            );
        }
        self.messages_written = c.total;
        // The corrections of the selected satellite.
        let sel = if self.selected != 0 && c.sats.contains_key(&self.selected) {
            Some(self.selected)
        } else {
            c.sats.keys().next().copied()
        };
        if sel.unwrap_or(0) != self.shown {
            p.clear_rows(&SBAS_FAST_CORRECTIONS);
            p.clear_rows(&SBAS_IONOSPHERE);
            self.shown = sel.unwrap_or(0);
        }
        let s = sel.and_then(|n| c.sats.get(&n));
        let geo = s.and_then(|s| s.geo.as_ref());
        let tm = s.and_then(|s| s.time.as_ref());
        p.put(&SbasFields {
            data_valid: valid,
            sbas_messages: c.total as i64,
            messages_missed: c.bad as i64,
            help: c.sats.is_empty().then_some(if tracked == 0 {
                "Searching PRN 120 to 158. A satellite in view is found within a minute or two. No GPS lock is needed."
            } else {
                "A message arrives every second once a satellite is locked."
            }),
            selected: sel.map(i64::from),
            geo_iodn: geo.map(|g| i64::from(g.iodn)),
            geo_t0_s: geo.map(|g| g.t0),
            geo_ura: geo.map(|g| i64::from(g.ura)),
            geo_radius_m: geo
                .map(|g| (g.pos[0].powi(2) + g.pos[1].powi(2) + g.pos[2].powi(2)).sqrt()),
            geo_longitude_deg: geo.map(|g| g.pos[1].atan2(g.pos[0]).to_degrees()),
            sbas_week: tm.map(|t| i64::from(t.week)),
            sbas_tow_s: tm.map(|t| i64::from(t.tow)),
            sbas_leap_s: tm.map(|t| i64::from(t.dtls)),
            sbas_leap_next_s: tm.map(|t| i64::from(t.dtlsf)),
            utc_a0_s: tm.map(|t| t.a0),
            utc_a1: tm.map(|t| t.a1),
            utc_id: tm.map(|t| i64::from(t.utc_id)),
        });
        let Some(s) = s else {
            return;
        };
        for (slot, f) in s.fast.iter().enumerate() {
            key.clear();
            let _ = write!(key, "{slot:02}");
            let Some(f) = f else {
                p.remove_row(&SBAS_FAST_CORRECTIONS, key);
                continue;
            };
            let sat = s
                .mask
                .as_ref()
                .and_then(|m| m.slots.get(slot.wrapping_sub(1)))
                .and_then(|b| sbas::mask_bit_sat(*b));
            let ai = s.ai.as_ref().and_then(|(_, a)| a.get(slot.wrapping_sub(1)));
            SBAS_FAST_CORRECTIONS.put(
                p,
                key,
                &SbasFastCorrectionRow {
                    slot: slot as i64,
                    system: sat.map(|s| s.0),
                    prn: sat.map(|s| i64::from(s.1)),
                    correction_m: f.prc,
                    udrei: i64::from(f.udrei),
                    udre_sigma_m: sbas::udre_variance(f.udrei).map(f64::sqrt),
                    iodf: i64::from(f.iodf),
                    iodp: i64::from(f.iodp),
                    iodp_current: s.mask.as_ref().is_some_and(|m| m.iodp == f.iodp),
                    degradation: ai.map(|i| sbas::degradation_factor(*i)),
                },
            );
        }
        for b in s.iono_summary() {
            key.clear();
            let _ = write!(key, "{}", b.band);
            SBAS_IONOSPHERE.put(
                p,
                key,
                &SbasIonosphereRow {
                    band: i64::from(b.band),
                    iodi: i64::from(b.iodi),
                    points: b.points as i64,
                    blocks_seen: b.blocks_seen as i64,
                    blocks_needed: b.blocks_needed as i64,
                    values: b.values as i64,
                    not_monitored: b.not_monitored as i64,
                    min_m: b.min_m,
                    mean_m: b.mean_m,
                    max_m: b.max_m,
                    worst_givei: i64::from(b.worst_givei),
                },
            );
        }
    }

    fn select(&mut self, args: &Values) -> Result<(), CommandError> {
        let row = SBAS_SELECT.args::<SbasSelectArgs>(args)?.row;
        match row
            .parse::<u16>()
            .ok()
            .filter(|prn| self.content.sats.contains_key(prn))
        {
            Some(prn) => {
                self.selected = prn;
                self.publish();
                Ok(())
            }
            None => Err(CommandError::NoSuchRow {
                table: table_id(&self.results, SBAS_SATELLITES.def()),
                key: row.to_owned(),
            }),
        }
    }
}

impl Component for SbasContentComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "gnss.sbas_content",
            vec![
                Port::typed::<SbasMessage>("messages"),
                Port::typed::<ChannelTable>("channels"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let mut changed = false;
        for m in io.items::<SbasMessage>(0) {
            let (prn, ty) = (m.value.prn, m.value.msg_type);
            let had = self
                .content
                .sats
                .get(&prn)
                .is_some_and(|s| s.types_seen >> ty & 1 == 1);
            let line = self.content.add(prn, &m.value.bits, m.value.signal_s);
            if !had {
                self.results
                    .results_mut()
                    .log(format!("PRN {prn} first type {ty}: {line}"));
            }
            changed = true;
        }
        if let Some(ch) = io.items::<ChannelTable>(1).last() {
            self.channels.clone_from(&ch.value.0);
            if ch.time - self.last_report >= 0.25 {
                self.last_report = ch.time;
                changed = true;
            }
        }
        if changed {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        SbasContentCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match SbasContentCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SbasContentCommand::Select => self.select(args)?,
            SbasContentCommand::Clear => {
                self.clear();
                self.publish();
            }
        }
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        self.forget_all_but(kept);
        self.publish();
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

/// Registers every component kind of this crate.
pub fn register(r: &mut Registry) {
    r.add("gnss.receiver", || Box::new(GnssCoreComponent::new(false)));
    r.add("gnss.sbas_receiver", || {
        Box::new(GnssCoreComponent::new(true))
    });
    r.add("gnss.input_conditioning", || {
        Box::new(ConditioningComponent::default())
    });
    r.add("gnss.excision", || Box::new(ExcisionComponent::default()));
    r.add("gnss.acquisition", || {
        Box::new(AcquisitionComponent::default())
    });
    r.add("gnss.nav_store", || Box::new(NavStoreComponent::default()));
    r.add("gnss.pvt", || Box::new(PvtComponent::default()));
    r.add(
        "gnss.interference",
        || Box::new(MonitorComponent::default()),
    );
    r.add("gnss.nmea", || Box::new(NmeaComponent));
    r.add("gnss.rinex", || Box::new(RinexComponent::default()));
    r.add("gnss.sbas_content", || {
        Box::new(SbasContentComponent::default())
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_registers_with_its_settings_and_commands() {
        let mut r = Registry::new();
        register(&mut r);
        for kind in [
            "gnss.receiver",
            "gnss.sbas_receiver",
            "gnss.acquisition",
            "gnss.sbas_content",
        ] {
            let c = r.make(kind).unwrap();
            assert_eq!(c.info().name, kind);
        }
        let rx = r.make("gnss.receiver").unwrap();
        assert!(rx.commands().id("reset").is_ok());
        let sbas = r.make("gnss.sbas_content").unwrap();
        assert!(sbas.commands().id("select").is_ok() && sbas.commands().id("clear").is_ok());
    }

    #[test]
    fn the_receiver_reports_the_settings_it_runs_with() {
        let mut rx = GnssCoreComponent::new(false);
        let id = ReceiverSetting::SysGlonass.id();
        rx.apply(id, &Value::Bool(false)).unwrap();
        assert_eq!(rx.values().value(id), Some(&Value::Bool(false)));
        assert_eq!(rx.core.cfg.systems & System::Glonass.bit(), 0);
        let id = ReceiverSetting::GalileoCodes.id();
        let applied = rx.apply(id, &Value::Text("no/such/file".into())).unwrap();
        assert!(matches!(applied, Applied::Reformat));
        assert!(matches!(
            rx.configure(&[Format::new(Kind::Iq, 4e6)]),
            Err(ConfigError::Setting(SettingError::Malformed { .. }))
        ));
    }
}
