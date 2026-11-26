//! Pulse radar detection (`radar_detect`, submodes `marine` and `weather`). It finds the pulses of a rotating radar at the radio
//! rate over a running noise level, measures their repetition frequency (one, or two for a staggered train) and their width,
//! groups the peak pulse levels in 2 ms buckets into beam crossings, and takes the rotation period and the beamwidth from the
//! crossings. Marine: a row when the period is first found, changes by more than 5 % or 30 s after the last row. Weather: a row
//! for every crossing (a sweep); a crossing more than 2.5 dB stronger than the one before starts a new volume.
//!
//! The pulses, the train and the crossings are the shared pulse-train analysis of `sdrkit-algo-dsp` (`pulsetrain`). One antenna
//! gives the rotation and the beam crossing, not the bearing. Radars that send no pulses (FMCW) are not seen.
//!
//! Document: ITU-R M.1313 (maritime radionavigation radars) and ITU-R M.1849 (ground-based meteorological radars) give the
//! typical values of these radars; neither was read.

pub mod gen;

use crate::family::REPORT_S;
use gen::{kind_of, Kind};
use sdrkit_algo_dsp::pulsetrain::{
    BeamScan, Crossing, Prf, Pulse, PulseConfig, PulseDetector, PulseTrain, ScanConfig, TrainConfig,
};
use crate::typed::{
    RadarDetectFields, RadarDetectRow, RadarDetectRowEvent, RADAR_DETECT_ROWS, RADAR_ROW,
};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::Published;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, LockState,
    Receiver, ResultsDecl, Schema, SettingSet, StackConfig, StackDescriptor, Status, SubMode,
    Tuning, Values,
};
use sdrkit_graph::{
    no_values, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kind as PortKind, Outbox, Port, RateFrom, StackGraph,
};
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "radar_detect";
/// The submode of marine radars.
pub const MARINE: &str = "marine";
/// The submode of weather radars.
pub const WEATHER: &str = "weather";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 1e6;

/// The samples whose mean `|x|²` is the first noise.
pub const NOISE_INIT: usize = 2000;
/// The time constant of the noise mean after them, s.
pub const NOISE_TAU_S: f64 = 0.2;
/// Samples below this many times the noise, outside pulses, enter the noise mean.
pub const NOISE_GATE: f64 = 4.0;
/// A pulse starts above this many times the noise (13 dB).
pub const PULSE_START: f64 = 20.0;
/// A pulse ends below this many times the noise.
pub const PULSE_END: f64 = 5.0;
/// A pulse also ends after this long, s.
pub const PULSE_MAX_S: f64 = 200e-6;
/// The repetition intervals kept: longer than this, shorter than that, s.
pub const PRI_S: (f64, f64) = (50e-6, 20e-3);
/// The newest intervals kept.
pub const PRI_KEPT: usize = 400;
/// The newest widths kept.
pub const WIDTHS_KEPT: usize = 200;
/// The bins of the interval histogram, s.
pub const PRI_BIN_S: f64 = 5e-6;
/// A second repetition frequency holds more than this share of the fullest bin ...
pub const DUAL_SHARE: f64 = 1.0 / 3.0;
/// ... and lies more than this many bins away.
pub const DUAL_BINS: usize = 3;
/// The buckets of the pulse levels, s.
pub const BUCKET_S: f64 = 2e-3;
/// The buckets whose largest level sets the level of a crossing (7 s).
pub const WINDOW_BUCKETS: usize = 3500;
/// No crossing while that level is below this, dB.
pub const CROSSING_DB: f64 = 13.0;
/// A crossing holds the buckets at that level less this or more, dB.
pub const WITHIN_DB: f64 = 6.0;
/// A crossing closes after more than this many buckets below.
pub const CLOSE_AFTER: usize = 6;
/// The newest crossings kept.
pub const CROSSINGS_KEPT: usize = 80;
/// The least crossings for a rotation.
pub const MIN_CROSSINGS: usize = 3;
/// The shortest and the longest rotation, s.
pub const ROTATION_S: (f64, f64) = (0.5, 60.0);
/// An interval agrees with the median within this share of it; at least half the intervals must agree.
pub const AGREE: f64 = 0.2;
/// Marine: a new row when the rotation changes by more than this share ...
pub const ROW_CHANGE: f64 = 0.05;
/// ... or this long after the last row, s.
pub const ROW_EVERY_S: f64 = 30.0;
/// Weather: a crossing this much stronger than the one before starts a new volume, dB.
pub const VOLUME_STEP_DB: f64 = 2.5;
/// The most rows kept in the table.
pub const ROWS_KEPT: usize = 500;

/// What no public document confirms.
pub const NOTE_TEXT: &str = "Not confirmed by a public document: every rule of the detector (the first noise over 2000 \
samples and its 0.2 s mean below 4 times the noise; pulses from 13 dB over the noise to 5 times it or 200 µs; intervals of \
50 µs to 20 ms in 5 µs bins with a second frequency above a third of the fullest bin and more than 3 bins away; 2 ms buckets, \
crossings within 6 dB of the largest level of 7 s and at least 13 dB, closed after 6 buckets; the rotation from 3 crossings \
with half the intervals within 20 % of the median, 0.5 to 60 s; marine rows at a 5 % change or every 30 s; a new weather \
volume at 2.5 dB). The typical radar values of ITU-R M.1313 and M.1849 were not read. One antenna gives no bearing; FMCW \
radars are not seen.";

sdrkit_core::settings! {
    /// The settings of the `radar_detect` stack: those of its test signal.
    pub enum RadarSetting {
        /// The rotation of the test signal.
        GenRpm = gen::GEN_RPM,
        /// The repetition frequency of the test signal.
        GenPrf = gen::GEN_PRF_HZ,
        /// A staggered train in the test signal.
        GenDual = gen::GEN_DUAL_PRF,
    }
}

/// The text of a repetition: `1000 Hz`, or `dual 1200 / 900 Hz, ratio 1.33` for a staggered train; empty without one.
pub fn prf_text(prf: Option<&Prf>) -> String {
    match prf {
        None => String::new(),
        Some(p) => match p.dual() {
            Some((hi, lo)) => format!("dual {hi:.0} / {lo:.0} Hz, ratio {:.2}", hi / lo),
            None => format!("{:.0} Hz", p.hz),
        },
    }
}

/// One row: a rotation (marine) or a sweep (weather).
#[derive(Clone, Debug, PartialEq)]
pub struct RadarRow {
    /// The time of the crossing, s of signal.
    pub time_s: f64,
    /// A sweep (weather) or a rotation (marine).
    pub sweep: bool,
    /// The rotation period, s.
    pub rotation_s: Option<f64>,
    /// The repetition text ([`prf_text`]).
    pub prf: String,
    /// The repetition frequency of the fullest bin, Hz.
    pub prf_hz: Option<f64>,
    /// The median pulse width, µs.
    pub width_us: Option<f64>,
    /// The beamwidth, degrees.
    pub beamwidth_deg: Option<f64>,
    /// The peak of the crossing over the noise, dB.
    pub peak_db: f64,
    /// Weather: the volume of the sweep (1 for the first seen) and the sweep in it (from 1).
    pub volume_sweep: Option<(u64, u64)>,
}

impl RadarRow {
    /// Rotations per minute.
    pub fn rpm(&self) -> Option<f64> {
        self.rotation_s.map(|r| 60.0 / r)
    }
}

/// The detector at one input rate: pulses, train, crossings, rotation and rows. Every decision follows absolute sample
/// indices, so pieces of any size give the same rows.
pub struct RadarDetector {
    rate: f64,
    kind: Kind,
    pulses: PulseDetector,
    train: PulseTrain,
    scan: BeamScan,
    found: Vec<Pulse>,
    crossed: Vec<Crossing>,
    pulse_count: u64,
    crossing_count: u64,
    rotation: Option<f64>,
    beamwidth: Option<f64>,
    last_crossing: Option<Crossing>,
    last_row: Option<(f64, f64)>,
    rows: Vec<RadarRow>,
    volumes: u64,
    sweeps: u64,
    volume_start: Option<f64>,
    last_volume_s: Option<f64>,
}

impl RadarDetector {
    /// A detector for `kind` at `rate`.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] below [`MIN_RATE_HZ`].
    pub fn new(rate: f64, kind: Kind) -> Result<Self, ConfigError> {
        let low = ConfigError::RateTooLow {
            need: MIN_RATE_HZ,
            got: rate,
        };
        if !rate.is_finite() || rate < MIN_RATE_HZ * 0.999_999 {
            return Err(low);
        }
        let pulses = PulseDetector::new(PulseConfig {
            init: NOISE_INIT,
            tau: rate * NOISE_TAU_S,
            gate: NOISE_GATE,
            start: PULSE_START,
            end: PULSE_END,
            max_len: ((rate * PULSE_MAX_S).round() as usize).max(1),
            min_noise: 1e-20,
        })
        .map_err(|_| low.clone())?;
        let train = PulseTrain::new(TrainConfig {
            min_interval_s: PRI_S.0,
            max_interval_s: PRI_S.1,
            intervals: PRI_KEPT,
            widths: WIDTHS_KEPT,
            bin_s: PRI_BIN_S,
            second_share: DUAL_SHARE,
            second_bins: DUAL_BINS,
        })
        .map_err(|_| low.clone())?;
        let scan = BeamScan::new(ScanConfig {
            bucket_s: BUCKET_S,
            window: WINDOW_BUCKETS,
            min_level_db: CROSSING_DB,
            within_db: WITHIN_DB,
            close_after: CLOSE_AFTER,
            crossings: CROSSINGS_KEPT,
            min_crossings: MIN_CROSSINGS,
            period_s: ROTATION_S,
            agree: AGREE,
            agree_share: 0.5,
        })
        .map_err(|_| low)?;
        Ok(RadarDetector {
            rate,
            kind,
            pulses,
            train,
            scan,
            found: Vec::new(),
            crossed: Vec::new(),
            pulse_count: 0,
            crossing_count: 0,
            rotation: None,
            beamwidth: None,
            last_crossing: None,
            last_row: None,
            rows: Vec::new(),
            volumes: 0,
            sweeps: 0,
            volume_start: None,
            last_volume_s: None,
        })
    }

    /// Feeds `x`, whose first sample has the absolute index `first`.
    pub fn process(&mut self, x: &[Complex32], first: u64) {
        let mut found = std::mem::take(&mut self.found);
        found.clear();
        self.pulses.process(x, first, &mut found);
        for p in &found {
            self.advance_to(p.start);
            let t = p.start as f64 / self.rate;
            self.train.push(t, p.len as f64 / self.rate);
            self.scan.add(t, p.peak_db());
            self.pulse_count += 1;
        }
        self.found = found;
        let upto = self
            .pulses
            .active_start()
            .unwrap_or(first + x.len() as u64);
        self.advance_to(upto);
    }

    /// Closes the buckets that end before sample `n` and handles the crossings that close.
    fn advance_to(&mut self, n: u64) {
        let mut crossed = std::mem::take(&mut self.crossed);
        crossed.clear();
        self.scan.advance(n as f64 / self.rate, &mut crossed);
        for c in &crossed {
            self.crossing(c);
        }
        self.crossed = crossed;
    }

    /// A crossing: the rotation and the beamwidth, then the row rules of the submode.
    fn crossing(&mut self, c: &Crossing) {
        self.crossing_count += 1;
        self.rotation = self.scan.period();
        self.beamwidth = self.rotation.map(|m| c.width_s * 360.0 / m);
        let previous = self.last_crossing.replace(*c);
        match self.kind {
            Kind::Marine => {
                let Some(m) = self.rotation else {
                    return;
                };
                let due = match self.last_row {
                    None => true,
                    Some((t, r)) => (m - r).abs() > ROW_CHANGE * r || c.time_s - t >= ROW_EVERY_S,
                };
                if due {
                    self.last_row = Some((c.time_s, m));
                    let row = self.row(c, None);
                    self.rows.push(row);
                }
            }
            Kind::Weather => {
                let restart = previous.is_some_and(|p| c.peak_db > p.peak_db + VOLUME_STEP_DB);
                if restart {
                    self.volumes += 1;
                    self.last_volume_s = self.volume_start.map(|s| c.time_s - s);
                    self.volume_start = Some(c.time_s);
                    self.sweeps = 1;
                } else {
                    if self.volume_start.is_none() {
                        self.volume_start = Some(c.time_s);
                    }
                    self.sweeps += 1;
                }
                let row = self.row(c, Some((self.volumes + 1, self.sweeps)));
                self.rows.push(row);
            }
        }
    }

    fn row(&mut self, c: &Crossing, volume_sweep: Option<(u64, u64)>) -> RadarRow {
        let prf = self.train.prf();
        RadarRow {
            time_s: c.time_s,
            sweep: self.kind == Kind::Weather,
            rotation_s: self.rotation,
            prf: prf_text(prf.as_ref()),
            prf_hz: prf.map(|p| p.hz),
            width_us: self.train.median_width().map(|w| w * 1e6),
            beamwidth_deg: self.beamwidth,
            peak_db: c.peak_db,
            volume_sweep,
        }
    }

    /// The rows made since the last call, in order.
    pub fn take_rows(&mut self) -> Vec<RadarRow> {
        std::mem::take(&mut self.rows)
    }

    /// The rotation period, s.
    pub fn rotation(&self) -> Option<f64> {
        self.rotation
    }

    /// The beamwidth from the last crossing, degrees.
    pub fn beamwidth(&self) -> Option<f64> {
        self.beamwidth
    }

    /// The repetition of the newest intervals.
    pub fn prf(&self) -> Option<Prf> {
        self.train.prf()
    }

    /// The median width of the newest pulses, s.
    pub fn median_width(&mut self) -> Option<f64> {
        self.train.median_width()
    }

    /// Pulses found.
    pub fn pulses(&self) -> u64 {
        self.pulse_count
    }

    /// Crossings found.
    pub fn crossings(&self) -> u64 {
        self.crossing_count
    }

    /// The noise power, once the first samples are in.
    pub fn noise(&self) -> Option<f64> {
        self.pulses.noise()
    }

    /// Weather: volume starts seen after the first crossing, sweeps in this volume and the length of the last whole volume, s.
    pub fn volumes(&self) -> (u64, u64, Option<f64>) {
        (self.volumes, self.sweeps, self.last_volume_s)
    }

    /// Forgets the train, the crossings, the rotation, the rows and the counters; the noise and the buckets go on.
    pub fn clear(&mut self) {
        self.train.clear();
        self.scan.clear();
        self.pulse_count = 0;
        self.crossing_count = 0;
        self.rotation = None;
        self.beamwidth = None;
        self.last_crossing = None;
        self.last_row = None;
        self.rows.clear();
        self.volumes = 0;
        self.sweeps = 0;
        self.volume_start = None;
        self.last_volume_s = None;
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The summary of a row.
fn row_text(r: &RadarRow) -> String {
    let rot = r
        .rotation_s
        .map_or("rotation not known".to_string(), |s| {
            format!("rotation {s:.2} s ({:.1} rpm)", 60.0 / s)
        });
    let mut t = format!("{rot}, PRF {}, peak {:.1} dB", r.prf, r.peak_db);
    if let Some((v, s)) = r.volume_sweep {
        t.push_str(&format!(", volume {v} sweep {s}"));
    }
    t
}

/// The event of a row.
fn row_event(r: &RadarRow) -> Event {
    let (volume, sweep) = volume_sweep(r);
    RADAR_ROW.event(
        r.time_s,
        0.0,
        &RadarDetectRowEvent {
            text: &row_text(r),
            r#type: if r.sweep { "sweep" } else { "rotation" },
            rotation_s: r.rotation_s,
            rpm: r.rotation_s.map(|v| 60.0 / v),
            prf: &r.prf,
            pulse_width_us: r.width_us,
            beamwidth_deg: r.beamwidth_deg,
            peak_db: r.peak_db,
            volume,
            sweep,
        },
    )
}

/// The volume and the sweep of a weather row.
fn volume_sweep(r: &RadarRow) -> (Option<i64>, Option<i64>) {
    r.volume_sweep.map_or((None, None), |(v, s)| {
        (
            Some(i64::try_from(v).unwrap_or(i64::MAX)),
            Some(i64::try_from(s).unwrap_or(i64::MAX)),
        )
    })
}

/// The `radar_detect` decoder: the [`RadarDetector`] with its rows, events and headline.
pub struct RadarDecoder {
    kind: Kind,
    det: Option<RadarDetector>,
    rows: VecDeque<(u64, RadarRow)>,
    serial: u64,
    fresh: usize,
    rows_total: u64,
    cleared: bool,
    events: Vec<Event>,
    started: bool,
    status: Status,
}

impl RadarDecoder {
    /// A decoder for `kind`.
    pub fn new(kind: Kind) -> Self {
        RadarDecoder {
            kind,
            det: None,
            rows: VecDeque::new(),
            serial: 0,
            fresh: 0,
            rows_total: 0,
            cleared: false,
            events: Vec::new(),
            started: false,
            status: Status::default(),
        }
    }

    fn take(&mut self, out: &mut Outbox) {
        let Some(det) = self.det.as_mut() else {
            return;
        };
        for r in det.take_rows() {
            self.events.push(row_event(&r));
            self.rows.push_back((self.serial, r));
            self.serial += 1;
            self.rows_total += 1;
            self.fresh = (self.fresh + 1).min(ROWS_KEPT);
            while self.rows.len() > ROWS_KEPT {
                self.rows.pop_front();
            }
        }
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }

    fn publish_rows(&mut self, p: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&RADAR_DETECT_ROWS);
        }
        let start = self.rows.len().saturating_sub(self.fresh);
        for (serial, r) in self.rows.iter().skip(start) {
            let (volume, sweep) = volume_sweep(r);
            RADAR_DETECT_ROWS.put(
                p,
                &format!("{serial:06}"),
                &RadarDetectRow {
                    signal_time_s: r.time_s,
                    r#type: if r.sweep { "sweep" } else { "rotation" },
                    rotation_s: r.rotation_s,
                    rpm: r.rpm(),
                    prf: &r.prf,
                    pulse_width_us: r.width_us,
                    beamwidth_deg: r.beamwidth_deg,
                    peak_db: r.peak_db,
                    volume,
                    sweep,
                },
            );
        }
        self.fresh = 0;
    }
}

impl Default for RadarDecoder {
    fn default() -> Self {
        RadarDecoder::new(Kind::Marine)
    }
}

fn count(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

impl Decoder for RadarDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "science.radar"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", PortKind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        Arc::new(Schema::of(&[]))
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.det = Some(RadarDetector::new(input.rate, self.kind)?);
        Ok(vec![Format::new(PortKind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        if let Some(d) = self.det.as_mut() {
            d.process(x, first);
        }
        self.take(out);
    }
    fn report(&mut self, p: &mut Published) {
        self.publish_rows(p);
        let weather = self.kind == Kind::Weather;
        let Some(d) = self.det.as_mut() else {
            p.put(&RadarDetectFields {
                text: &self.status.summary,
                rotation_s: None,
                rpm: None,
                prf_hz: None,
                prf: None,
                pulse_width_us: None,
                beamwidth_deg: None,
                crossings: 0,
                pulses: 0,
                rows_total: count(self.rows_total),
                noise_dbfs: None,
                sweeps: None,
                volumes: None,
                last_volume_s: None,
                note: NOTE_TEXT,
            });
            return;
        };
        let prf = d.prf();
        let text = prf_text(prf.as_ref());
        let rotation = d.rotation();
        let width = d.median_width().map(|w| w * 1e6);
        let (volumes, sweeps, last) = d.volumes();
        let st = &mut self.status;
        st.lock_state = match (self.started, d.pulses() > 0, rotation.is_some()) {
            (false, ..) => LockState::Off,
            (_, false, _) => LockState::Searching,
            (_, true, false) => LockState::Tracking,
            (_, true, true) => LockState::Locked,
        };
        st.summary.clear();
        st.summary.push_str(&match (self.started, rotation) {
            (false, _) => "Pulse radar: waiting for samples".to_string(),
            (true, None) => format!(
                "Pulse radar: {} pulses, {} crossings, no rotation yet{}",
                d.pulses(),
                d.crossings(),
                if text.is_empty() {
                    String::new()
                } else {
                    format!(", PRF {text}")
                }
            ),
            (true, Some(r)) => {
                let mut s = format!("Rotation {r:.2} s ({:.1} rpm), PRF {text}", 60.0 / r);
                if weather {
                    s.push_str(&format!(", volumes {volumes}, sweep {sweeps}"));
                }
                s
            }
        });
        p.put(&RadarDetectFields {
            text: &st.summary,
            rotation_s: rotation,
            rpm: rotation.map(|r| 60.0 / r),
            prf_hz: prf.map(|x| Hertz(x.hz)),
            prf: prf.is_some().then_some(text.as_str()),
            pulse_width_us: width,
            beamwidth_deg: d.beamwidth(),
            crossings: count(d.crossings()),
            pulses: count(d.pulses()),
            rows_total: count(self.rows_total),
            noise_dbfs: d.noise().map(|n| 10.0 * n.log10()),
            sweeps: weather.then_some(count(sweeps)),
            volumes: weather.then_some(count(volumes)),
            last_volume_s: last.filter(|_| weather),
            note: NOTE_TEXT,
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        if let Some(d) = self.det.as_mut() {
            d.clear();
        }
        self.rows.clear();
        self.fresh = 0;
        self.rows_total = 0;
        self.cleared = true;
    }
}

/// The receiver: one decoder on the input, for the submode of `cfg` (marine unless `weather`).
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let kind = kind_of(cfg.submode.as_deref());
    let graph = GraphBuilder::new()
        .add("dec", DecoderComponent::new(RadarDecoder::new(kind)))
        .input("dec")
        .output("dec.events")
        .status_from("dec")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE_HZ,
            "The pulse radar detector needs a sample rate of at least 1 Msps",
        )
        .command("clear", CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("science.radar", || {
        Box::new(DecoderComponent::new(RadarDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::RadarGenerator::from_config(cfg)?))
}

/// The marine case: a rotation of 1.25 s within 2 %.
fn marine_check(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let got = crate::family::field(r, RadarDetectFields::ROTATION_S.key);
    vec![CheckLine::new(
        RadarDetectFields::ROTATION_S.key,
        (got - 1.25).abs() <= 0.025,
        format!("{got:.3} s, 1.25 s expected"),
    )]
}

/// The weather case: two crossings or more and a staggered train.
fn weather_check(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let n = crate::family::field(r, RadarDetectFields::CROSSINGS.key);
    let text = r
        .decl()
        .field_id(RadarDetectFields::PRF.key)
        .and_then(|id| r.fields().value(id))
        .map(|v| format!("{v:?}"))
        .unwrap_or_default();
    vec![
        CheckLine::new(RadarDetectFields::CROSSINGS.key, n >= 2.0, format!("{n}, at least 2 expected")),
        CheckLine::new(
            RadarDetectFields::PRF.key,
            text.contains("dual"),
            format!("{text}, a dual PRF expected"),
        ),
    ]
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    // Fast rotations keep the captures at the stack's own rate short. Marine: three crossings in 3 s. Weather: the sweeps of
    // 38 and 34 dB in 2.6 s (a weaker sweep crosses only when the stronger ones are 7 s old).
    let mut m = VerifyCase::new("test signal, marine at 48 rpm", o, 0.0)
        .with(&gen::GEN_RPM, 48.0);
    m.submode = Some(MARINE.into());
    m.seconds = 3.0;
    m.snr_db = None;
    m.expect_audio = Some(false);
    m.check = Some(marine_check);
    let mut w = VerifyCase::new("test signal, weather at 60 rpm, staggered PRF", o, 0.0)
        .with(&gen::GEN_RPM, 60.0)
        .with(&gen::GEN_DUAL_PRF, gen::DualPrf::Dual);
    w.submode = Some(WEATHER.into());
    w.seconds = 2.6;
    w.snr_db = None;
    w.expect_audio = Some(false);
    w.check = Some(weather_check);
    vec![m, w]
}

/// The engine's checks of the test signal.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// Pulse radar detection.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Pulse radar (detect)",
    group: "Science, time and utility",
    blurb: "Rotating pulse radars in S, C and X band: the rotation period, the pulse repetition frequency (single or dual), \
            the pulse width and the beamwidth from the beam passing; for weather radars, the sweeps and volume restarts.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 2700e6,
        max_hz: 9600e6,
        default_hz: 3050e6,
        rate_hz: 10e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 1e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[
        SubMode {
            id: MARINE,
            name: "Marine radar",
            default_hz: 3050e6,
            ..SubMode::DEFAULT
        },
        SubMode {
            id: WEATHER,
            name: "Weather radar",
            default_hz: 5625e6,
            ..SubMode::DEFAULT
        },
    ],
    settings: RadarSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};
