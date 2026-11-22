//! The sonde table: every sonde heard with its telemetry, track, map points and landing prediction, the carriers table, the
//! events, the lock state and the status line.

use super::channel::CarrierReport;
use super::frame::{SondeFrame, SondeType};
use super::landing::{self, Landing, LandingOptions, TrackPoint};
use crate::family::{lock_state_of, status, wall, write_series};
use crate::typed::{
    SondeCarrierRow, SondeFields, SondeFrameEvent, SondeRow, SondeRowArgs, SONDE_ALT_TIME,
    SONDE_CARRIERS, SONDE_FRAME, SONDE_REMOVE, SONDE_ROW, SONDE_SELECT, SONDE_SONDES,
    SONDE_TEMP_ALT, SONDE_TRACK,
};
use sdrkit_core::interface::{Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Clock, CommandError, CommandId, CommandSet, Commands, ConfigError, Event, Record, Results,
    ResultsDecl, SeriesId, Setting, SettingError, SettingId, SettingSet, StateBlob, StateError,
    Status, TableId, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Port, RateFrom, RunContext,
};
use sdrkit_util_geo::Position;
use sdrkit_util_objects::commands::{self as objcmd};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{rows_by, RowOut};
use sdrkit_util_objects::{Format as StateFormat, ObjectTable, Seen, Shift, Stamp, Tables};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

/// Sondes kept.
pub const MAX_SONDES: usize = 16;
/// Track points per sonde.
pub const MAX_TRACK: usize = 600;
/// Carriers listed.
pub const MAX_CARRIERS: usize = 32;
/// Knots in one metre a second.
const KT_PER_M_S: f64 = 3600.0 / 1852.0;

/// One sonde in the table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sonde {
    /// The family.
    pub kind: SondeType,
    /// The subtype ("RS41-SG", "DFM-17").
    pub subtype: String,
    /// The serial.
    pub serial: String,
    /// The receiving channel.
    pub channel: Option<usize>,
    /// The offset from the centre, Hz.
    pub offset_hz: f64,
    /// The absolute frequency, Hz.
    pub freq_hz: Option<f64>,
    /// The frequency the sonde announces, Hz.
    pub announced_hz: Option<f64>,
    /// The carrier level, dB.
    pub snr_db: f64,
    /// The last frame counter.
    pub frame: Option<i64>,
    /// UTC from the sonde, Unix seconds.
    pub utc: Option<f64>,
    /// The position, degrees.
    pub lat: Option<f64>,
    /// The longitude, degrees.
    pub lon: Option<f64>,
    /// The altitude, m.
    pub alt: Option<f64>,
    /// The highest altitude, m.
    pub max_alt: Option<f64>,
    /// The vertical speed, m/s.
    pub vel_v: Option<f64>,
    /// The horizontal speed, m/s.
    pub vel_h: Option<f64>,
    /// The heading, degrees.
    pub heading: Option<f64>,
    /// The satellites used.
    pub sats: Option<u32>,
    /// The temperature, °C.
    pub temp_c: Option<f64>,
    /// The relative humidity, %.
    pub humidity: Option<f64>,
    /// The pressure, hPa.
    pub pressure_hpa: Option<f64>,
    /// The battery, V.
    pub battery_v: Option<f64>,
    /// The burst or kill timer, s.
    pub kill_timer_s: Option<f64>,
    /// The calibration parts held and needed.
    pub cal: Option<(u32, u32)>,
    /// Good frames.
    pub good: u64,
    /// Bad frames.
    pub bad: u64,
    /// Corrections in the last frame.
    pub corrections: u32,
    /// The track, oldest first.
    pub track: Vec<TrackPoint>,
    track_step: f64,
    /// Temperature and humidity at each track point.
    pub track_ptu: Vec<(Option<f64>, Option<f64>)>,
    /// The landing prediction.
    pub landing: Option<Landing>,
    landing_hist: VecDeque<(f64, Landing)>,
    /// The mean of the predictions over the last minute.
    pub landing_avg: Option<(f64, f64)>,
    last_event: f64,
    falling: bool,
    landed_sent: bool,
    ve: f64,
    vn: f64,
}

impl Sonde {
    fn new(kind: SondeType, serial: String) -> Sonde {
        Sonde {
            kind,
            subtype: String::new(),
            serial,
            channel: None,
            offset_hz: 0.0,
            freq_hz: None,
            announced_hz: None,
            snr_db: 0.0,
            frame: None,
            utc: None,
            lat: None,
            lon: None,
            alt: None,
            max_alt: None,
            vel_v: None,
            vel_h: None,
            heading: None,
            sats: None,
            temp_c: None,
            humidity: None,
            pressure_hpa: None,
            battery_v: None,
            kill_timer_s: None,
            cal: None,
            good: 0,
            bad: 0,
            corrections: 0,
            track: Vec::new(),
            track_step: 1.0,
            track_ptu: Vec::new(),
            landing: None,
            landing_hist: VecDeque::new(),
            landing_avg: None,
            last_event: f64::NEG_INFINITY,
            falling: false,
            landed_sent: false,
            ve: 0.0,
            vn: 0.0,
        }
    }

    fn label(&self) -> &str {
        if self.subtype.is_empty() {
            self.kind.name()
        } else {
            &self.subtype
        }
    }

    fn position(&self) -> Option<(f64, f64)> {
        self.lat.zip(self.lon)
    }
}

impl Shift for Sonde {
    fn shift_times(&mut self, by_s: f64) {
        self.last_event += by_s;
        for h in &mut self.landing_hist {
            h.0 += by_s;
        }
    }
}

/// What the table keeps: the sondes, keyed by "type:serial".
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The sondes.
    pub sondes: ObjectTable<String, Sonde>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            sondes: ObjectTable::new(MAX_SONDES),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.sondes.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("science.sondes", 2);
    fn clear(&mut self) {
        self.sondes.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.sondes.adopt(saved.sondes);
    }
}

/// What `sonde` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("sonde").results_decl()
}

impl Sonde {
    /// Writes the row of the sonde to `out`. `station` is the position of the station, for the distance and the bearing
    /// of the landing point.
    fn put_row(
        &self,
        seen: Option<&Seen>,
        station: Option<&Position>,
        out: RowOut<'_, '_, SondeRow<'static>>,
    ) {
        let s = self;
        let land = s.landing.map(|l| Position::new(l.lat, l.lon));
        let to_land = station.zip(land);
        out.put(&SondeRow {
            r#type: s.kind,
            subtype: (!s.subtype.is_empty()).then_some(s.subtype.as_str()),
            serial: &s.serial,
            channel: s.channel.map(|c| c as i64),
            frequency: s.freq_hz.map(Hertz),
            offset: Hertz(s.offset_hz),
            snr_db: s.snr_db,
            frame: s.frame,
            time: s.utc.map(Time),
            position: s.position(),
            height_m: s.alt,
            max_alt_m: s.max_alt,
            climb_m_s: s.vel_v,
            speed_kt: s.vel_h.map(|v| v * KT_PER_M_S),
            course_deg: s.heading,
            sats: s.sats.map(i64::from),
            temp_c: s.temp_c,
            humidity_pct: s.humidity,
            pressure_hpa: s.pressure_hpa,
            battery_v: s.battery_v,
            kill: s.kill_timer_s.map(Seconds),
            cal_held: s.cal.map(|c| i64::from(c.0)),
            cal_needed: s.cal.map(|c| i64::from(c.1)),
            frames_ok: s.good as i64,
            frames_bad: s.bad as i64,
            ok_pct: 100.0 * s.good as f64 / (s.good + s.bad).max(1) as f64,
            landing: s.landing.map(|l| (l.lat, l.lon)),
            landing_in: s.landing.map(|l| Seconds(l.seconds)),
            landing_distance_m: to_land.map(|(st, l)| st.distance_m(&l)),
            landing_bearing_deg: to_land.map(|(st, l)| st.initial_bearing_deg(&l)),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

sdrkit_core::commands! {
    /// The commands of [`SondeTable`].
    pub enum SondeCommand {
        /// Forgets every sonde.
        Clear = objcmd::CLEAR,
        /// Forgets one sonde.
        Remove = SONDE_REMOVE.command,
        /// Selects the sonde the plots show.
        Select = SONDE_SELECT.command,
    }
}

/// The time over which landing predictions are smoothed.
pub const LANDING_WINDOW: Setting<field::Duration> =
    Setting::new("landing_window_s", "Landing smoothing", "s", 10.0).range(0.0, 60.0, 1.0);
/// Use the winds measured on the ascent.
pub const LANDING_LAYERS: Setting<field::Bool> =
    Setting::new("landing_layers", "Use the ascent winds", "", true);
/// The height of a wind layer.
pub const LANDING_LAYER: Setting<field::F64> =
    Setting::new("landing_layer_m", "Layer", "m", 500.0).range(100.0, 2000.0, 50.0);
/// The weight of the ascent winds.
pub const ASCENT_WEIGHT: Setting<field::F64> =
    Setting::new("landing_ascent_weight", "Ascent wind weight", "", 0.6).range(0.0, 1.0, 0.05);
/// The height of the ground.
pub const GROUND: Setting<field::F64> =
    Setting::new("ground_m", "Ground height", "m", 0.0).range(-500.0, 5000.0, 1.0);

sdrkit_core::settings! {
    /// The settings of [`SondeTable`].
    pub enum TableSetting {
        /// The time over which landing predictions are smoothed.
        LandingWindow = LANDING_WINDOW,
        /// Use the winds measured on the ascent.
        LandingLayers = LANDING_LAYERS,
        /// The height of a wind layer.
        LandingLayer = LANDING_LAYER,
        /// The weight of the ascent winds.
        AscentWeight = ASCENT_WEIGHT,
        /// The height of the ground.
        Ground = GROUND,
    }
}

/// The sonde table component (`sonde.table`: `sonde.frame` + `sonde.report` to events).
pub struct SondeTable {
    rate: f64,
    t: f64,
    store: Store,
    report: CarrierReport,
    report_new: bool,
    good: u64,
    bad: u64,
    last_good: f64,
    published: Published,
    sondes_table: TableId,
    series: [SeriesId; 3],
    shown: Status,
    station: Option<Position>,
    live: bool,
    clock: Clock,
    opt: LandingOptions,
    selected: Option<String>,
    events: Vec<Event>,
    centre: f64,
    key: String,
    text: String,
}

impl Default for SondeTable {
    fn default() -> Self {
        let decl = Arc::new(declaration());
        let sondes_table = decl
            .table_id(SONDE_SONDES.key)
            .expect("the declaration holds the sondes");
        let series = [SONDE_ALT_TIME.key, SONDE_TEMP_ALT.key, SONDE_TRACK.key]
            .map(|k| decl.series_id(k).expect("the declaration holds the series"));
        SondeTable {
            rate: 0.0,
            t: 0.0,
            store: Store::default(),
            report: CarrierReport::default(),
            report_new: false,
            good: 0,
            bad: 0,
            last_good: f64::NEG_INFINITY,
            published: Published::from_decl(decl),
            sondes_table,
            series,
            shown: Status::default(),
            station: None,
            live: false,
            clock: Clock::fixed(0.0),
            opt: LandingOptions::default(),
            selected: None,
            events: Vec::new(),
            centre: 0.0,
            key: String::new(),
            text: String::new(),
        }
    }
}

impl SondeTable {
    /// The sondes held.
    pub fn store(&self) -> &Store {
        &self.store
    }

    fn add(&mut self, f: &SondeFrame) {
        let kind = f.kind.unwrap_or(SondeType::Rs41);
        let chan_key = self
            .store
            .sondes
            .iter()
            .find(|r| r.record.channel == Some(f.channel) && r.record.kind == kind)
            .map(|r| r.key.clone());
        if !f.ok {
            self.bad += 1;
            if let Some(k) = &chan_key {
                if let Some(s) = self.store.sondes.get_mut(k) {
                    s.bad += 1;
                }
            }
            return;
        }
        let serial = match (&f.serial, &chan_key) {
            (Some(s), _) => s.clone(),
            (None, Some(k)) => k.split_once(':').map_or(k.as_str(), |x| x.1).to_string(),
            (None, None) => format!("(channel {})", f.channel),
        };
        self.good += 1;
        self.last_good = f.time;
        let key = format!("{}:{}", kind.name(), serial);
        let mut stamp = Stamp::at(f.time);
        if let Some(w) = wall(self.live, &self.clock) {
            stamp = stamp.with_wall(w);
        }
        let make = || Sonde::new(kind, serial.clone());
        let Ok(up) = self.store.sondes.upsert(&key, stamp, make) else {
            return;
        };
        let new = up.new;
        let s = up.row;
        if new {
            let line = format!(
                "Radiosonde: {} {} heard at {:+.1} kHz",
                if f.subtype.is_empty() {
                    kind.name()
                } else {
                    &f.subtype
                },
                serial,
                f.offset_hz / 1e3
            );
            self.published.results_mut().log(line);
        }
        s.good += 1;
        s.subtype.clone_from(&f.subtype);
        s.channel = Some(f.channel);
        s.offset_hz = f.offset_hz;
        s.snr_db = f.snr_db;
        s.freq_hz = (self.centre != 0.0)
            .then_some(self.centre + f.offset_hz)
            .or(f.announced_hz);
        if f.announced_hz.is_some() {
            s.announced_hz = f.announced_hz;
        }
        s.frame = f.frame.or(s.frame);
        s.utc = f.utc.or(s.utc);
        s.corrections = f.corrections;
        s.sats = f.sats.or(s.sats);
        s.battery_v = f.battery_v.or(s.battery_v);
        s.temp_c = f.temp_c;
        s.humidity = f.humidity;
        s.pressure_hpa = f.pressure_hpa;
        if f.kill_timer_s.is_some() {
            s.kill_timer_s = f.kill_timer_s;
        }
        if f.cal.is_some() {
            s.cal = f.cal;
        }
        if let (Some(lat), Some(lon), Some(alt)) = (f.lat, f.lon, f.alt) {
            s.lat = Some(lat);
            s.lon = Some(lon);
            s.alt = Some(alt);
            s.max_alt = Some(s.max_alt.map_or(alt, |m: f64| m.max(alt)));
            s.vel_v = f.vel_v;
            s.vel_h = f.vel_h;
            s.heading = f.heading;
            let (vh, hd) = (
                f.vel_h.unwrap_or(0.0),
                f.heading.unwrap_or(0.0).to_radians(),
            );
            s.ve = vh * hd.sin();
            s.vn = vh * hd.cos();
            let tp = f.utc.unwrap_or(f.time);
            let last = s.track.last().map(|p| p.t);
            if last.is_none_or(|l| tp - l >= 0.8 * s.track_step) {
                s.track.push(TrackPoint {
                    t: tp,
                    lat,
                    lon,
                    alt,
                });
                s.track_ptu.push((f.temp_c, f.humidity));
                if s.track.len() >= MAX_TRACK {
                    let mut k = 0;
                    s.track.retain(|_| {
                        k += 1;
                        k % 2 == 1
                    });
                    let mut k = 0;
                    s.track_ptu.retain(|_| {
                        k += 1;
                        k % 2 == 1
                    });
                    s.track_step *= 2.0;
                }
            }
            let vv = f.vel_v.unwrap_or(0.0);
            s.falling = vv < -1.0;
            let now = TrackPoint {
                t: tp,
                lat,
                lon,
                alt,
            };
            s.landing = landing::predict(&s.track, now, s.ve, s.vn, vv, &self.opt);
            if let Some(l) = s.landing {
                s.landing_hist.push_back((f.time, l));
                while s.landing_hist.front().is_some_and(|x| f.time - x.0 > 60.0) {
                    s.landing_hist.pop_front();
                }
                let n = s.landing_hist.len() as f64;
                s.landing_avg = Some((
                    s.landing_hist.iter().map(|x| x.1.lat).sum::<f64>() / n,
                    s.landing_hist.iter().map(|x| x.1.lon).sum::<f64>() / n,
                ));
            } else {
                s.landing_hist.clear();
                s.landing_avg = None;
            }
        }
        if f.time - s.last_event >= 5.0 {
            s.last_event = f.time;
            self.text.clear();
            let _ = write!(self.text, "{} {}", s.label(), s.serial);
            if let Some(a) = s.alt {
                let _ = write!(self.text, " at {:.0} m", a);
            }
            let e = SONDE_FRAME.event(
                f.time,
                s.freq_hz.unwrap_or(0.0),
                &SondeFrameEvent {
                    source: &s.serial,
                    text: &self.text,
                    family: Family::Sonde,
                    position: s.position(),
                    height_m: s.alt,
                    speed_kt: s.vel_h.map(|v| v * KT_PER_M_S),
                    course_deg: s.heading,
                    r#type: s.kind,
                    serial: &s.serial,
                    temp_c: s.temp_c,
                    humidity_pct: s.humidity,
                    climb_m_s: s.vel_v,
                    burst_killer: s.kill_timer_s.map(|k| k > 0.0),
                },
            );
            self.events.push(e);
        }
    }

    fn state(&self) -> u8 {
        if self.t - self.last_good <= 5.0 {
            2
        } else if !self.report.carriers.is_empty() {
            1
        } else {
            0
        }
    }

    fn landed(&mut self) {
        let t = self.t;
        let gone: Vec<String> = self
            .store
            .sondes
            .iter()
            .filter(|r| r.record.falling && !r.record.landed_sent && t - r.seen.last_s > 90.0)
            .map(|r| r.key.clone())
            .collect();
        for k in gone {
            if let Some(s) = self.store.sondes.get_mut(&k) {
                s.landed_sent = true;
                let at = match s.landing {
                    Some(l) => format!(", predicted landing {:.5}, {:.5}", l.lat, l.lon),
                    None => String::new(),
                };
                let line = format!(
                    "Radiosonde: {} {} has landed or gone below the horizon{at}",
                    s.label(),
                    s.serial
                );
                self.published.results_mut().log(line);
            }
        }
    }

    /// Writes the carriers of a new search report.
    fn publish_carriers(&mut self) {
        let p = &mut self.published;
        let n = self.report.carriers.len();
        for (i, (c, ch)) in self.report.carriers.iter().enumerate() {
            self.key.clear();
            let _ = write!(self.key, "{i:02}");
            let freq = (self.centre != 0.0).then_some(self.centre + c.offset_hz);
            SONDE_CARRIERS.put(
                p,
                &self.key,
                &SondeCarrierRow {
                    offset: Hertz(c.offset_hz),
                    frequency: freq.map(Hertz),
                    level_db: c.level_db,
                    channel: ch.map(|c| c as i64),
                },
            );
        }
        for i in n..MAX_CARRIERS {
            self.key.clear();
            let _ = write!(self.key, "{i:02}");
            p.remove_row(&SONDE_CARRIERS, &self.key);
        }
    }

    fn publish(&mut self) {
        self.landed();
        if self.report_new {
            self.report_new = false;
            self.publish_carriers();
        }
        let t = self.t;
        let state = self.state();
        let best = self
            .store
            .sondes
            .iter()
            .filter(|x| x.seen.within(t, 30.0))
            .max_by(|a, b| a.record.snr_db.total_cmp(&b.record.snr_db));
        let snr = best
            .map(|s| s.record.snr_db as f32)
            .or_else(|| self.report.carriers.first().map(|c| c.0.level_db as f32));
        let rs41 = self
            .store
            .sondes
            .iter()
            .filter(|x| {
                x.seen.within(t, 30.0)
                    && x.record.kind == SondeType::Rs41
                    && x.record.announced_hz.is_some()
            })
            .max_by(|a, b| a.record.snr_db.total_cmp(&b.record.snr_db));
        let offset = match (rs41, self.centre != 0.0) {
            (Some(s), true) => s
                .record
                .announced_hz
                .map(|a| self.centre + s.record.offset_hz - a),
            _ => None,
        };
        let newest = self.store.sondes.iter_newest_first().next();
        let n = self.store.sondes.len();
        status(
            &mut self.shown,
            lock_state_of(state),
            snr,
            None,
            offset,
            |line| {
                let _ = match (state, newest) {
                    (0, _) => write!(line, "Radiosonde: searching"),
                    (_, None) => write!(line, "Radiosonde: signal"),
                    (_, Some(x)) => {
                        let s = x.record;
                        match s.alt {
                            Some(a) => write!(
                                line,
                                "Radiosonde: {n} heard, {} {} at {:.1} km",
                                s.label(),
                                s.serial,
                                a / 1000.0
                            ),
                            None => {
                                write!(line, "Radiosonde: {n} heard, {} {}", s.label(), s.serial)
                            }
                        }
                    }
                };
            },
        );
        let p = &mut self.published;
        p.results_mut().set_status(&self.shown);
        p.put(&SondeFields {
            sondes: n as i64,
            frames_ok: self.good as i64,
            frames_bad: self.bad as i64,
            searches: self.report.searches as i64,
            channels_used: self.report.channels.len() as i64,
            channels_allowed: self.report.allowed as i64,
            band: Hertz(self.rate),
            narrow: self.rate < 2e6 && self.centre <= 1500e6,
            selected: self.selected.as_deref(),
        });
        let station = self.station;
        rows_by(
            &self.store.sondes,
            &SONDE_SONDES,
            p,
            |_| true,
            |r, out| r.record.put_row(Some(r.seen), station.as_ref(), out),
        );
        let r = p.results_mut();
        let sel = self
            .selected
            .as_ref()
            .and_then(|k| self.store.sondes.iter().find(|x| x.key == k))
            .or(newest);
        let [alt_time, temp_alt, track] = self.series;
        let empty: &[TrackPoint] = &[];
        let (pts, ptu) = sel.map_or((empty, &[][..]), |x| {
            (x.record.track.as_slice(), x.record.track_ptu.as_slice())
        });
        let t0 = pts.first().map_or(0.0, |p| p.t);
        write_series(
            r,
            alt_time,
            true,
            pts.iter().map(|p| [(p.t - t0) as f32, p.alt as f32]),
        );
        write_series(
            r,
            temp_alt,
            false,
            pts.iter()
                .zip(ptu)
                .filter_map(|(p, x)| x.0.map(|t| [t as f32, p.alt as f32])),
        );
        write_series(
            r,
            track,
            true,
            pts.iter().map(|p| [p.lat as f32, p.lon as f32]),
        );
        // Both points of a sonde show its row.
        let table = r.decl().table_id(SONDE_SONDES.def().key);
        for x in self.store.sondes.iter() {
            let s = x.record;
            let row = table.map(|t| (t, x.key.as_str()));
            if let Some((lat, lon)) = s.position() {
                self.text.clear();
                let _ = write!(self.text, "{} {}", s.kind.name(), s.serial);
                r.update_point(x.key, lat, lon, &self.text, s.heading.map(|h| h as f32));
                r.link_point(x.key, row);
            }
            self.key.clear();
            let _ = write!(self.key, "landing:{}", x.key);
            match s.landing {
                Some(l) => {
                    self.text.clear();
                    let _ = write!(self.text, "{} landing (estimate)", s.serial);
                    r.update_point(&self.key, l.lat, l.lon, &self.text, None);
                    r.link_point(&self.key, row);
                }
                None => {
                    r.remove_point(&self.key);
                }
            }
        }
    }

    /// Removes the map points of sondes that left the table.
    fn forget_points(&mut self) {
        self.published.results_mut().clear_points();
    }
}

impl Component for SondeTable {
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "sonde.table",
            vec![
                Port::iq("in"),
                Port::typed::<SondeFrame>("frames"),
                Port::typed::<CarrierReport>("report"),
            ],
            vec![Port::new("events", sdrkit_graph::Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(TableSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.station = ctx.station.position;
        self.live = ctx.live;
        self.clock = ctx.clock.clone();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.centre = inputs[0].centre_hz;
        self.report = CarrierReport::default();
        self.report_new = true;
        self.good = 0;
        self.bad = 0;
        self.publish();
        Ok(vec![Format::new(sdrkit_graph::Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let n = io.input_len(0);
        for tg in io.input(0).tags() {
            if let sdrkit_graph::TagKind::Retuned { centre_hz, .. } = tg.kind {
                self.centre = centre_hz;
                self.report = CarrierReport::default();
                self.report_new = true;
                self.good = 0;
                self.bad = 0;
            }
        }
        for f in io.items::<SondeFrame>(1) {
            self.add(&f.value);
        }
        for r in io.items::<CarrierReport>(2) {
            for l in &r.value.log {
                self.published.results_mut().log(l.clone());
            }
            self.report.clone_from(&r.value);
            self.report_new = true;
            if r.value.centre_hz != 0.0 {
                self.centre = r.value.centre_hz;
            }
        }
        self.t = (first + n as u64) as f64 / self.rate.max(1.0);
        if io.tick() {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TableSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TableSetting::LandingWindow => self.opt.window_s = LANDING_WINDOW.read(id, v)?,
            TableSetting::LandingLayers => self.opt.layers = LANDING_LAYERS.read(id, v)?,
            TableSetting::LandingLayer => self.opt.layer_m = LANDING_LAYER.read(id, v)?,
            TableSetting::AscentWeight => self.opt.ascent_weight = ASCENT_WEIGHT.read(id, v)?,
            TableSetting::Ground => self.opt.ground_m = GROUND.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                TableSetting::LandingWindow,
                Value::Duration(self.opt.window_s),
            ),
            (TableSetting::LandingLayers, Value::Bool(self.opt.layers)),
            (TableSetting::LandingLayer, Value::F64(self.opt.layer_m)),
            (
                TableSetting::AscentWeight,
                Value::F64(self.opt.ascent_weight),
            ),
            (TableSetting::Ground, Value::F64(self.opt.ground_m)),
        ])
    }
    fn commands(&self) -> &Commands {
        SondeCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match SondeCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SondeCommand::Clear => {
                self.store.clear();
                self.selected = None;
                self.forget_points();
            }
            SondeCommand::Remove => {
                objcmd::remove(
                    &mut self.store.sondes,
                    self.sondes_table,
                    &mut self.published,
                    args,
                    &SONDE_ROW,
                )?;
                self.forget_points();
            }
            SondeCommand::Select => {
                let SondeRowArgs { row } = SONDE_SELECT.args(args)?;
                if self.store.sondes.seen(row).is_none() {
                    return Err(CommandError::NoSuchRow {
                        table: self.sondes_table,
                        key: row.to_owned(),
                    });
                }
                self.selected = Some(row.to_owned());
            }
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.t).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.t)?;
        self.forget_points();
        self.publish();
        Ok(())
    }
}
