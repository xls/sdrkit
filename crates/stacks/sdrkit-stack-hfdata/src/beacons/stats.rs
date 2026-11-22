//! Beacon statistics, histories, bands and the results of the HF beacon monitor.

use super::list::{band_row, Beacon, Pattern, BAND_ROWS};
use super::prop::{Observation, PropModel, Source};
use super::rx::{Judged, Plan, PlanItem, CLOCK_OFFSET};
use crate::typed::{
    HfbDetectionEvent, HfbeaconsBandRow, HfbeaconsBeaconRow, HfbeaconsFields,
    HfbeaconsPropagationRow, HFBEACONS_BANDS, HFBEACONS_BEACONS, HFBEACONS_PROPAGATION,
    HFB_DETECTION,
};
use crate::util::Origin;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    ConfigError, Event, LockState, Record, Results, ResultsDecl, SettingError, SettingId,
    SettingSet, StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RunContext,
};
use sdrkit_util_geo::Position;
use sdrkit_util_objects::commands::{self as objcmd, Tables};
use sdrkit_util_objects::{Format as StateFormat, Shift};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

/// One history point: signal time, best SNR heard (None: listened, not heard).
pub type Point = (f64, Option<f64>);

/// What is kept about one beacon (by name and frequency).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BeaconStat {
    pub last_heard: Option<f64>,
    pub snr_db: Option<f64>,
    pub confidence: u8,
    pub conf_time: f64,
    pub steps: u8,
    pub step_snrs: Vec<f64>,
    pub note: String,
    pub history: VecDeque<Point>,
    pub logged: bool,
    #[serde(skip)]
    minute: Option<(i64, Option<f64>)>,
}

/// One row of the beacon table.
#[derive(Clone, Debug, PartialEq)]
pub struct BeaconRow {
    /// The beacon's name.
    pub name: String,
    /// Its frequency, kHz.
    pub khz: f64,
    /// The keying.
    pub kind: Pattern,
    /// Frequency, place or identity not firmly given by the source.
    pub uncertain: bool,
    /// Inside the capture.
    pub in_capture: bool,
    /// Heard now with its identity confirmed.
    pub heard: bool,
    /// 0 nothing, 1 energy (keying not confirmed), 2 identity confirmed.
    pub confidence: u8,
    /// Signal time last heard.
    pub last_t: Option<f64>,
    /// UTC last heard, when the clock is known.
    pub last_utc: Option<f64>,
    /// SNR in 100 Hz now, dB.
    pub snr_db: Option<f64>,
    /// Best SNR in the last hour, dB.
    pub best_1h: Option<f64>,
    /// Best SNR in the last 24 hours, dB.
    pub best_24h: Option<f64>,
    /// NCDXF power steps heard.
    pub steps: u8,
    /// Their SNRs, dB.
    pub step_snrs: Vec<f64>,
    /// Great-circle distance from the station, km.
    pub distance_km: Option<f64>,
    /// Bearing from the station, degrees.
    pub bearing_deg: Option<f64>,
    /// The newest 48 history points (None: listened, not heard).
    pub sparkline: Vec<Option<f64>>,
    /// History points in 24 hours, and how many of them heard.
    pub listens: usize,
    /// History points heard.
    pub heard_count: usize,
    /// The note.
    pub note: String,
    /// The band row.
    pub band: String,
    /// The position, when the list has one.
    pub position: Option<(f64, f64)>,
}

/// One band row.
#[derive(Clone, Debug, PartialEq)]
pub struct BandRow {
    /// The band name.
    pub band: String,
    /// Beacons of the band in the list.
    pub in_list: usize,
    /// In the capture.
    pub in_capture: usize,
    /// Heard now.
    pub heard_now: usize,
    /// The farthest heard, km.
    pub farthest_km: Option<f64>,
}

const NO_STATION: &str = "no station position: distances and bearings need it";

/// What the HF beacon stack publishes.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(HfbeaconsFields::SHAPE.fields)
        .with_table(&HFBEACONS_BEACONS)
        .with_table(&HFBEACONS_BANDS)
        .with_table(&HFBEACONS_PROPAGATION)
        .points("Beacons heard")
        .event(&HFB_DETECTION)
}

fn hold(p: Pattern) -> f64 {
    match p {
        Pattern::Ncdxf => 200.0,
        Pattern::Letter => 60.0,
        Pattern::Dashes => 90.0,
        Pattern::Carrier => 20.0,
    }
}

fn key(b: &Beacon) -> String {
    format!("{}|{:.3}", b.name, b.khz)
}

fn best_of(v: impl Iterator<Item = f64>) -> Option<f64> {
    v.fold(None, |a: Option<f64>, x| Some(a.map_or(x, |y| y.max(x))))
}

impl Shift for BeaconStat {
    fn shift_times(&mut self, by_s: f64) {
        if let Some(t) = self.last_heard.as_mut() {
            *t += by_s;
        }
        self.conf_time += by_s;
        for p in self.history.iter_mut() {
            p.0 += by_s;
        }
    }
}

/// What the stack keeps between runs: per beacon (by name and frequency) its last hearing and its history, and the propagation
/// model as text ("hfprop 1").
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BeaconLists {
    /// The statistics, by "name|kHz".
    pub stats: BTreeMap<String, BeaconStat>,
    /// The propagation model as text. It is filled when the state is saved.
    pub propagation: String,
}

impl Shift for BeaconLists {
    fn shift_times(&mut self, by_s: f64) {
        for st in self.stats.values_mut() {
            st.shift_times(by_s);
        }
    }
}

impl Tables for BeaconLists {
    const FORMAT: StateFormat = StateFormat::new("hfdata.hfbeacons_lists", 1);
    fn clear(&mut self) {
        self.stats.clear();
        self.propagation.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.stats = saved.stats;
        self.propagation = saved.propagation;
    }
}

sdrkit_core::settings! {
    /// The settings of [`BeaconStats`].
    pub enum StatsSetting {
        /// Clock offset.
        ClockOffset = CLOCK_OFFSET,
    }
}

sdrkit_core::commands! {
    /// The commands of [`BeaconStats`].
    pub enum StatsCommand {
        /// Forget the statistics, the histories and the propagation model.
        Clear = objcmd::CLEAR,
    }
}

/// `hfdata.hfb_stats`, the statistics: `hfb.detection`, the input and the plan in, `hfb.detection` events out.
pub struct BeaconStats {
    p: Published,
    points_shown: Vec<String>,
    origin: Origin,
    live: bool,
    plan: Arc<Plan>,
    lists: BeaconLists,
    now: f64,
    frames: u64,
    runs: u64,
    clock: bool,
    observer: Option<Position>,
    /// The propagation model of this receiver's observations.
    pub prop: PropModel,
    input_db: f64,
    pw: (f64, u64),
    shown_rows: usize,
    clock_offset: f64,
    /// Signal time at the last restart: the chain counts its time from each restart (a retune), the statistics run on.
    base: f64,
    configured: bool,
}

impl Default for BeaconStats {
    fn default() -> Self {
        BeaconStats {
            p: Published::from_decl(Arc::new(declaration())),
            points_shown: Vec::new(),
            origin: Origin::default(),
            live: false,
            plan: Arc::default(),
            lists: BeaconLists::default(),
            now: 0.0,
            frames: 0,
            runs: 0,
            clock: false,
            observer: None,
            prop: PropModel::default(),
            input_db: -120.0,
            pw: (0.0, 0),
            shown_rows: 0,
            clock_offset: 0.0,
            base: 0.0,
            configured: false,
        }
    }
}

impl BeaconStats {
    fn stat(&mut self, i: usize) -> &mut BeaconStat {
        let k = key(&self.plan.list[i]);
        self.lists.stats.entry(k).or_default()
    }

    /// What the stack keeps.
    pub fn lists(&self) -> &BeaconLists {
        &self.lists
    }

    fn push_point(st: &mut BeaconStat, p: Point) {
        if st.history.len() >= 1440 {
            st.history.pop_front();
        }
        st.history.push_back(p);
        while st.history.front().is_some_and(|f| p.0 - f.0 > 86_400.0) {
            st.history.pop_front();
        }
    }

    /// UTC of signal time `t` (stream time when the clock is unknown).
    fn utc(&self, t: f64) -> f64 {
        self.origin.at(t - self.base) - self.clock_offset
    }

    fn take(&mut self, d: &super::rx::Detection, out: &mut Vec<Event>) {
        if d.beacon >= self.plan.list.len() {
            return;
        }
        let b = self.plan.list[d.beacon].clone();
        let utc = self.utc(d.time);
        let obs = self.observer;
        let st = self.stat(d.beacon);
        if d.slot {
            Self::push_point(st, (d.time, d.heard.then_some(d.snr_db)));
        }
        if !d.heard {
            if d.confidence >= st.confidence
                || st.last_heard.is_none_or(|t| d.time - t > hold(b.pattern))
            {
                st.confidence = d.confidence;
                st.conf_time = d.time;
                st.note = d.note.clone();
            }
            return;
        }
        st.last_heard = Some(d.time);
        if d.snr_db.is_finite() {
            st.snr_db = Some(d.snr_db);
        }
        if d.confidence >= st.confidence || d.time - st.conf_time > hold(b.pattern) {
            st.confidence = d.confidence;
            st.conf_time = d.time;
        }
        st.steps = d.steps;
        st.step_snrs = d.step_snrs.clone();
        st.note = d.note.clone();
        if b.pattern != Pattern::Ncdxf {
            let m = (d.time / 60.0).floor() as i64;
            let best = match st.minute {
                Some((mm, v)) if mm == m => Some(v.map_or(d.snr_db, |x| x.max(d.snr_db))),
                _ => Some(d.snr_db),
            };
            st.minute = Some((m, best));
        }
        let mut new_log = None;
        if d.confidence == 2 && !st.logged {
            st.logged = true;
            new_log = Some(format!(
                "HF beacons: {} {:.1} kHz heard, {:.0} dB in 100 Hz",
                b.name, b.khz, d.snr_db
            ));
        }
        if let Some(l) = new_log {
            self.p.results_mut().log(l);
        }
        if d.confidence == 2 {
            let dist = match (obs, b.lat, b.lon) {
                (Some(o), Some(la), Some(lo)) => Some(o.distance_m(&Position::new(la, lo)) / 1e3),
                _ => None,
            };
            let source = match b.pattern {
                Pattern::Ncdxf => Source::Ncdxf,
                Pattern::Letter => Source::Letters,
                Pattern::Dashes => Source::Dashes,
                Pattern::Carrier => Source::TimeSignal,
            };
            if b.pattern != Pattern::Carrier {
                self.prop.add(&Observation {
                    utc_s: utc,
                    mhz: b.khz / 1e3,
                    source,
                    id: b.name.clone(),
                    snr_db: d.snr_db,
                    km: dist,
                });
            }
            out.push(HFB_DETECTION.event(
                d.time,
                b.khz * 1e3,
                &HfbDetectionEvent {
                    source: &b.name,
                    snr_db: d.snr_db,
                    confidence: i64::from(d.confidence),
                    text: &d.note,
                    position: b.lat.zip(b.lon),
                    family: Family::Other,
                },
            ));
        }
    }

    /// One minute passed: letters, dashes and carriers get their history point.
    fn minute_points(&mut self) {
        let m = (self.now / 60.0).floor() as i64;
        for i in 0..self.plan.list.len() {
            let b = &self.plan.list[i];
            if b.pattern == Pattern::Ncdxf || !self.plan.in_capture.get(i).copied().unwrap_or(false)
            {
                continue;
            }
            let st = self.stat(i);
            let best = match st.minute {
                Some((mm, v)) if mm == m - 1 => v,
                _ => None,
            };
            Self::push_point(st, (((m - 1) as f64 + 0.5) * 60.0, best));
        }
    }

    /// The rows of the beacon table, in list order.
    pub fn rows(&self) -> Vec<BeaconRow> {
        let mut v = Vec::new();
        let empty = BeaconStat::default();
        for (i, b) in self.plan.list.iter().enumerate() {
            let st = self.lists.stats.get(&key(b)).unwrap_or(&empty);
            let heard = st
                .last_heard
                .is_some_and(|t| self.now - t <= hold(b.pattern))
                && st.confidence >= 1;
            let best = |w: f64| {
                best_of(
                    st.history
                        .iter()
                        .filter(|p| self.now - p.0 <= w)
                        .filter_map(|p| p.1),
                )
                .or(if w >= 3600.0 && heard {
                    st.snr_db
                } else {
                    None
                })
            };
            let position = b.lat.zip(b.lon);
            let (dist, bear) = match (self.observer, position) {
                (Some(from), Some((la, lo))) => {
                    let to = Position::new(la, lo);
                    (
                        Some(from.distance_m(&to) / 1e3),
                        Some(from.initial_bearing_deg(&to)),
                    )
                }
                _ => (None, None),
            };
            let inc = self.plan.in_capture.get(i).copied().unwrap_or(false);
            let conf = if heard || self.now - st.conf_time <= 20.0 {
                st.confidence
            } else {
                0
            };
            v.push(BeaconRow {
                name: b.name.clone(),
                khz: b.khz,
                kind: b.pattern,
                uncertain: b.uncertain,
                in_capture: inc,
                heard: heard && st.confidence == 2,
                confidence: conf,
                last_t: st.last_heard,
                last_utc: st
                    .last_heard
                    .filter(|_| self.origin.valid)
                    .map(|t| self.utc(t)),
                snr_db: if heard { st.snr_db } else { None },
                best_1h: best(3600.0),
                best_24h: best(86_400.0),
                steps: st.steps,
                step_snrs: st.step_snrs.clone(),
                distance_km: dist,
                bearing_deg: bear,
                sparkline: st
                    .history
                    .iter()
                    .rev()
                    .take(48)
                    .rev()
                    .map(|p| p.1)
                    .collect(),
                listens: st.history.len(),
                heard_count: st.history.iter().filter(|p| p.1.is_some()).count(),
                note: if inc {
                    st.note.clone()
                } else {
                    "outside the capture".into()
                },
                band: band_row(b.khz).into(),
                position,
            });
        }
        v
    }

    fn publish(&mut self) {
        let rows = self.rows();
        let in_span = rows.iter().filter(|r| r.in_capture).count();
        let heard_now = rows.iter().filter(|r| r.heard).count();
        let names: Vec<&str> = BAND_ROWS
            .iter()
            .map(|b| b.0)
            .chain(["other below 10 MHz", "other above 10 MHz"])
            .collect();
        let bands: Vec<BandRow> = names
            .into_iter()
            .map(|n| {
                let rs: Vec<&BeaconRow> = rows.iter().filter(|r| r.band == n).collect();
                BandRow {
                    band: n.into(),
                    in_list: rs.len(),
                    in_capture: rs.iter().filter(|r| r.in_capture).count(),
                    heard_now: rs.iter().filter(|r| r.heard).count(),
                    farthest_km: best_of(
                        rs.iter().filter(|r| r.heard).filter_map(|r| r.distance_km),
                    ),
                }
            })
            .collect();
        let utc_now = self.utc(self.now);
        let prop = self.prop.now(utc_now);
        let best_band = self.prop.best(utc_now);
        let lo = self.plan.centre_hz - 0.4 * self.plan.rate;
        let hi = self.plan.centre_hz + 0.4 * self.plan.rate;
        let mut summary = format!(
            "{:.3} - {:.3} MHz, {in_span} beacons in the span, {heard_now} heard now, {:.0} dBFS",
            lo / 1e6,
            hi / 1e6,
            self.input_db
        );
        if self.observer.is_none() {
            summary.push_str(&format!(", {NO_STATION}"));
        }
        let p = &mut self.p;
        p.put(&HfbeaconsFields {
            span_low: Hertz(lo),
            span_high: Hertz(hi),
            in_span: in_span as i64,
            heard_now: heard_now as i64,
            channels: self.plan.channels.len() as i64,
            frames: self.frames as i64,
            runs_started: self.runs as i64,
            clock: self.origin.source(),
            clock_valid: self.clock,
            list_count: self.plan.list.len() as i64,
            station_known: self.observer.is_some(),
            best_band: best_band.as_ref().map(|b| b.band.as_str()),
            best_band_stations: best_band.as_ref().map(|b| i64::from(b.stations)),
            level_dbfs: self.input_db,
        });
        // The table lists the newest row first: write the list backwards, so it reads in list order.
        for (i, x) in rows.iter().enumerate().rev() {
            let steps: Vec<String> = x.step_snrs.iter().map(|v| format!("{v:.1}")).collect();
            let trend: Vec<String> = x
                .sparkline
                .iter()
                .map(|v| v.map_or("-".into(), |d| format!("{d:.0}")))
                .collect();
            HFBEACONS_BEACONS.put(
                p,
                &format!("{i:04}"),
                &HfbeaconsBeaconRow {
                    name: &x.name,
                    frequency: Hertz(x.khz * 1e3),
                    kind: x.kind,
                    uncertain: x.uncertain,
                    in_capture: x.in_capture,
                    heard: x.heard,
                    confidence: i64::from(x.confidence),
                    last: x.last_t.map(Seconds),
                    last_heard: x.last_utc.map(f64::floor).map(Time),
                    snr_db: x.snr_db,
                    best_1h_db: x.best_1h,
                    best_24h_db: x.best_24h,
                    steps: i64::from(x.steps),
                    step_snrs: &steps.join(", "),
                    distance_km: x.distance_km,
                    bearing_deg: x.bearing_deg,
                    trend: &trend.join(" "),
                    listens: x.listens as i64,
                    heard_count: x.heard_count as i64,
                    note: &x.note,
                    band: &x.band,
                    position: x.position,
                },
            );
        }
        for i in rows.len()..self.shown_rows {
            p.remove_row(&HFBEACONS_BEACONS, &format!("{i:04}"));
        }
        self.shown_rows = rows.len();
        for (i, b) in bands.iter().enumerate().rev() {
            HFBEACONS_BANDS.put(
                p,
                &format!("{i:02}"),
                &HfbeaconsBandRow {
                    band: &b.band,
                    in_list: b.in_list as i64,
                    in_capture: b.in_capture as i64,
                    heard_now: b.heard_now as i64,
                    farthest_km: b.farthest_km,
                },
            );
        }
        for (i, b) in prop.iter().enumerate().rev() {
            HFBEACONS_PROPAGATION.put(
                p,
                &format!("{i:02}"),
                &HfbeaconsPropagationRow {
                    band: &b.band,
                    state: b.state,
                    stations: i64::from(b.stations),
                    best_snr_db: b.best_snr,
                    farthest_km: b.farthest_km,
                    score: b.score,
                },
            );
        }
        let r = p.results_mut();
        r.set_status(&Status {
            lock_state: if self.frames > 0 {
                LockState::Tracking
            } else {
                LockState::Searching
            },
            snr_db: None,
            level_dbfs: Some(self.input_db as f32),
            offset_hz: None,
            summary,
        });
        // One point for each beacon heard, at the row of the band where it was heard last. The row key is the place
        // of the row in the list.
        let table = r.decl().table_id(HFBEACONS_BEACONS.def().key);
        let mut newest: Vec<(&str, usize)> = Vec::new();
        for (i, x) in rows.iter().enumerate() {
            if !x.heard || x.position.is_none() {
                continue;
            }
            match newest.iter_mut().find(|(n, _)| *n == x.name) {
                Some(n) if rows[n.1].last_t < x.last_t => n.1 = i,
                Some(_) => {}
                None => newest.push((&x.name, i)),
            }
        }
        let mut shown = Vec::new();
        for &(name, i) in &newest {
            let x = &rows[i];
            if let Some((la, lo)) = x.position {
                let label = format!("{name} {:+.0} dB", x.snr_db.unwrap_or(0.0));
                r.update_point(name, la, lo, &label, None);
                let key = format!("{i:04}");
                r.link_point(name, table.map(|t| (t, key.as_str())));
                shown.push(name.to_owned());
            }
        }
        for n in &self.points_shown {
            if !shown.contains(n) {
                r.remove_point(n);
            }
        }
        self.points_shown = shown;
    }
}

impl Component for BeaconStats {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.hfb_stats",
            vec![
                Port::typed::<Judged>("detections"),
                Port::iq("input"),
                Port::typed::<PlanItem>("plan"),
            ],
            vec![Port::new("events", Kind::Messages)],
        )
        .with_settings(StatsSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.origin = Origin::new(self.live);
        if self.configured {
            self.base = self.now;
        }
        self.configured = true;
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        if self.observer != ctx.station.position {
            self.observer = ctx.station.position;
            self.publish();
        }
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(p) = io.items::<PlanItem>(2).last() {
            self.plan = Arc::clone(&p.value.0);
            self.publish();
        }
        // Input power: the mean over each report, smoothed 0.9 / 0.1 from report to report.
        for v in io.iq(1) {
            self.pw.0 += v.norm_sqr() as f64;
            self.pw.1 += 1;
        }
        if io.tick() && self.pw.1 > 0 {
            let db = 10.0 * (self.pw.0 / self.pw.1 as f64).max(1e-15).log10();
            self.input_db = 0.9 * self.input_db + 0.1 * db;
            self.pw = (0.0, 0);
        }
        let mut evs: Vec<Event> = Vec::new();
        for j in io.items::<Judged>(0) {
            match &j.value {
                Judged::Detection(d) => {
                    let mut d = d.clone();
                    d.time += self.base;
                    self.take(&d, &mut evs);
                }
                Judged::Tick {
                    time,
                    hops,
                    runs,
                    clock,
                } => {
                    let before = (self.now / 60.0).floor();
                    self.now = self.base + *time;
                    self.frames = *hops;
                    self.runs = *runs;
                    self.clock = *clock;
                    if (self.now / 60.0).floor() > before {
                        self.minute_points();
                    }
                }
            }
        }
        io.messages_out(0).extend(evs);
        if io.tick() {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match StatsSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            StatsSetting::ClockOffset => self.clock_offset = CLOCK_OFFSET.read(id, v)?,
        }
        self.publish();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(StatsSetting::ClockOffset, Value::F64(self.clock_offset))])
    }
    fn commands(&self) -> &Commands {
        StatsCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match StatsCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            StatsCommand::Clear => {
                self.lists.clear();
                self.prop = PropModel::default();
            }
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        let mut l = self.lists.clone();
        l.propagation = self.prop.save(self.utc(self.now));
        l.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.lists.restore_state(state, self.now)?;
        let text = std::mem::take(&mut self.lists.propagation);
        let mut prop = PropModel::default();
        if prop.load(&text, self.utc(self.now)) {
            self.prop = prop;
        }
        self.publish();
        Ok(())
    }
}
