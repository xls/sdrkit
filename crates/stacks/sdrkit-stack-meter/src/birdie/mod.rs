//! Persistent carrier finder (`birdie`): the narrow carriers that stay in the capture, present in at least 90 % of the
//! snapshots of the last minute, with their frequency, level, drift, scatter and whether each is bare. A snapshot is the
//! mean of about 0.5 s of 16384-point spectra. It measures and decodes nothing, and does not name the source of a carrier
//! (a spur of the radio, a power supply, a beacon).

pub mod gen;

use crate::instrument::{publish_list, InputLevel, Log, Tally};
use crate::typed::{
    BirdieCarrierRow, BirdieChange, BirdieClass, BirdieEvent, BirdieEventRow, BirdieFields,
    WatchFields, BIRDIE_CARRIERS, BIRDIE_CARRIER_EVENTS, BIRDIE_EVENT,
};
use gen::BirdieGenerator;
use sdrkit_algo_dsp::peak::line_fit;
use sdrkit_algo_dsp::snapshot::{snapshot_peaks, PeakTracker, SnapshotPeak, Snapshots};
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, Receiver, Record,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, Status, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kind, Outbox, Port, RateFrom, StackGraph,
};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "birdie";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 500e3;
/// The points of the FFT.
pub const FFT_N: usize = 16384;
/// The time of a snapshot asked for, s.
pub const SNAPSHOT_S: f64 = 0.5;
/// Signed bins `-DC..=DC` hold no peak.
pub const DC_BINS: i64 = 3;
/// The most peaks of a snapshot.
pub const MAX_PEAKS: usize = 200;
/// A peak joins a track within this many bins.
pub const GATE_BINS: f64 = 2.5;
/// The least history of a track, snapshots.
pub const HISTORY: usize = 240;
/// A track not seen for more than this many snapshots is dropped.
pub const DROP_AFTER: usize = 60;
/// The first evaluation, and the least history of an evaluated track, snapshots.
pub const MIN_SNAPSHOTS: usize = 20;
/// Snapshots between evaluations.
pub const EVALUATE_EVERY: u64 = 4;
/// A listed carrier updates a report within this many bins.
pub const SAME_REPORT_BINS: f64 = 3.0;
/// Evaluations without a match after which a report is gone.
pub const GONE_AFTER: u32 = 3;
/// Rows kept in the log.
pub const ROWS_KEPT: usize = 500;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the narrow carriers present in most snapshots of the presence window; decodes \
nothing and does not name a source. A peak stands 10 dB over the median noise of a snapshot; a carrier is bare when 5 bins \
around it hold 75 % of the power of 17. No public document confirms these rules.";

/// The level of a peak over the noise.
pub const THRESHOLD_DB: Setting<field::F64> =
    Setting::new("threshold_db", "Peak over the noise", "dB", 10.0).range(6.0, 30.0, 0.0);
/// The share of the window in which a carrier must be seen.
pub const PRESENCE_PCT: Setting<field::F64> =
    Setting::new("presence_pct", "Present in at least", "%", 90.0).range(50.0, 100.0, 0.0);
/// The presence window.
pub const WINDOW_S: Setting<field::F64> =
    Setting::new("window_s", "Presence window", "s", 60.0).range(10.0, 600.0, 0.0);
/// The narrowness from which a carrier is bare.
pub const BARE_SHARE: Setting<field::F64> = Setting::new(
    "bare_share",
    "Bare carrier when the narrowness is at least",
    "",
    0.75,
)
.range(0.5, 0.95, 0.0);

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum DecoderSetting {
        /// The peak threshold.
        ThresholdDb = THRESHOLD_DB,
        /// The presence share.
        PresencePct = PRESENCE_PCT,
        /// The presence window.
        WindowS = WINDOW_S,
        /// The bare share.
        BareShare = BARE_SHARE,
    }
}

sdrkit_core::settings! {
    /// The settings of the `birdie` stack.
    pub enum BirdieSetting {
        /// The peak threshold.
        ThresholdDb = THRESHOLD_DB,
        /// The presence share.
        PresencePct = PRESENCE_PCT,
        /// The presence window.
        WindowS = WINDOW_S,
        /// The bare share.
        BareShare = BARE_SHARE,
        /// The carriers of the test signal.
        GenTones = gen::GEN_TONES,
        /// The modulated carrier offset.
        GenFmHz = gen::GEN_FM_HZ,
        /// The modulated carrier amplitude.
        GenFmAmp = gen::GEN_FM_AMP,
        /// The modulated carrier deviation.
        GenFmDevHz = gen::GEN_FM_DEV_HZ,
    }
}

/// The `reset` command: the tracks and the list start again; the log stays.
pub const RESET: Command =
    Command::new("reset", "Reset").help("Drops the tracks and the list; the log stays.");
static COMMANDS: [&Command; 1] = [&RESET];

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// A carrier of the present list.
#[derive(Clone, Copy, Debug)]
struct Listed {
    bin: f64,
    level: f64,
    level_sd: f64,
    class: BirdieClass,
    drift_hz_min: f64,
    scatter_hz: f64,
    present: usize,
    of: usize,
}

/// A carrier that has been reported.
#[derive(Clone, Copy, Debug)]
struct Report {
    bin: f64,
    gone: bool,
    unmatched: u32,
    last: Listed,
}

/// The `birdie` decoder.
pub struct BirdieDecoder {
    threshold_db: f64,
    presence_pct: f64,
    window_s: f64,
    bare_share: f64,
    rate: f64,
    dial_hz: f64,
    spec: Option<Snapshots>,
    tracker: PeakTracker,
    peaks: Vec<SnapshotPeak>,
    listed: Vec<Listed>,
    reports: Vec<Report>,
    samples: u64,
    tally: Tally,
    level: InputLevel,
    log: Log<BirdieEventRow>,
    events: Vec<Event>,
    status: Status,
}

impl Default for BirdieDecoder {
    fn default() -> Self {
        BirdieDecoder {
            threshold_db: THRESHOLD_DB.default,
            presence_pct: PRESENCE_PCT.default,
            window_s: WINDOW_S.default,
            bare_share: BARE_SHARE.default,
            rate: 0.0,
            dial_hz: 0.0,
            spec: None,
            tracker: PeakTracker::new(GATE_BINS, HISTORY, DROP_AFTER, 2 * MAX_PEAKS),
            peaks: Vec::new(),
            listed: Vec::new(),
            reports: Vec::new(),
            samples: 0,
            tally: Tally::default(),
            level: InputLevel::default(),
            log: Log::default(),
            events: Vec::new(),
            status: Status::default(),
        }
    }
}

impl BirdieDecoder {
    fn bin_hz(&self) -> f64 {
        self.rate / FFT_N as f64
    }

    fn snapshot_s(&self) -> f64 {
        self.spec
            .as_ref()
            .map_or(SNAPSHOT_S, |s| s.samples() as f64 / self.rate)
    }

    fn window(&self) -> usize {
        ((self.window_s / self.snapshot_s()).round() as usize).max(2)
    }

    fn build(&mut self) {
        let blocks = ((SNAPSHOT_S * self.rate / FFT_N as f64).round() as usize).max(1);
        self.spec = Snapshots::new(FFT_N, blocks).ok();
        self.reset();
    }

    fn reset(&mut self) {
        let depth = HISTORY.max(self.window());
        self.tracker = PeakTracker::new(GATE_BINS, depth, DROP_AFTER, 2 * MAX_PEAKS);
        self.listed.clear();
        if let Some(s) = self.spec.as_mut() {
            s.reset();
        }
    }

    fn clear(&mut self) {
        self.reset();
        self.reports.clear();
        self.tally = Tally::default();
        self.log.clear();
        self.events.clear();
    }

    fn snapshot(&mut self, power: &[f32], noise: f32, t: f64) {
        self.peaks.clear();
        snapshot_peaks(
            power,
            noise,
            self.threshold_db,
            DC_BINS,
            MAX_PEAKS,
            &mut self.peaks,
        );
        self.tracker.update(&self.peaks);
        if !self.peaks.is_empty() {
            self.tally.energy(t);
        }
        let k = self.tracker.snapshots();
        if (k as usize) < MIN_SNAPSHOTS || !(k - MIN_SNAPSHOTS as u64).is_multiple_of(EVALUATE_EVERY) {
            return;
        }
        self.evaluate(t);
    }

    fn evaluate(&mut self, t: f64) {
        let window = self.window();
        let snap = self.snapshot_s();
        let bin = self.bin_hz();
        let mut list = Vec::new();
        for tr in self.tracker.tracks() {
            if tr.history.len() < MIN_SNAPSHOTS || !tr.seen_within(2) {
                continue;
            }
            let n = tr.history.len().min(window);
            let recent = tr.history.iter().skip(tr.history.len() - n);
            let seen: Vec<(f64, f64, f64)> = recent
                .enumerate()
                .filter_map(|(i, s)| s.map(|s| (i as f64 * snap, s.bin * bin, s.level_db)))
                .collect();
            if (seen.len() as f64) < self.presence_pct / 100.0 * n as f64 {
                continue;
            }
            let m = seen.len() as f64;
            let mean = seen.iter().map(|s| s.2).sum::<f64>() / m;
            let sd = (seen.iter().map(|s| (s.2 - mean).powi(2)).sum::<f64>() / m).sqrt();
            let x: Vec<f64> = seen.iter().map(|s| s.0).collect();
            let y: Vec<f64> = seen.iter().map(|s| s.1).collect();
            let (drift, scatter) = match line_fit(&x, &y, None) {
                Ok(l) => {
                    let r = x
                        .iter()
                        .zip(&y)
                        .map(|(&a, &b)| (b - l.at(a)).powi(2))
                        .sum::<f64>()
                        / m;
                    (60.0 * l.slope, r.sqrt())
                }
                Err(_) => (0.0, 0.0),
            };
            list.push(Listed {
                bin: tr.bin,
                level: mean,
                level_sd: sd,
                class: if tr.narrowness >= self.bare_share {
                    BirdieClass::Bare
                } else {
                    BirdieClass::Modulated
                },
                drift_hz_min: drift,
                scatter_hz: scatter,
                present: seen.len(),
                of: n,
            });
        }
        list.sort_by(|a, b| b.level.total_cmp(&a.level));
        let mut matched = vec![false; self.reports.len()];
        let mut rows = Vec::new();
        for l in &list {
            match self
                .reports
                .iter()
                .position(|r| (r.bin - l.bin).abs() <= SAME_REPORT_BINS)
            {
                Some(k) => {
                    matched[k] = true;
                    let r = &mut self.reports[k];
                    if r.gone {
                        rows.push((BirdieChange::Returned, *l));
                    }
                    r.bin = l.bin;
                    r.gone = false;
                    r.unmatched = 0;
                    r.last = *l;
                }
                None => {
                    self.reports.push(Report {
                        bin: l.bin,
                        gone: false,
                        unmatched: 0,
                        last: *l,
                    });
                    matched.push(true);
                    rows.push((BirdieChange::New, *l));
                }
            }
        }
        for (r, &m) in self.reports.iter_mut().zip(&matched) {
            if m || r.gone {
                continue;
            }
            r.unmatched += 1;
            if r.unmatched >= GONE_AFTER {
                r.gone = true;
                rows.push((BirdieChange::Gone, r.last));
            }
        }
        for (kind, l) in rows {
            self.row(t, kind, &l);
        }
        if !list.is_empty() {
            self.tally.good(t);
        }
        self.listed = list;
    }

    fn row(&mut self, t: f64, kind: BirdieChange, l: &Listed) {
        let row = BirdieEventRow {
            time_s: Seconds(t),
            kind,
            frequency: Hertz(self.dial_hz + l.bin * self.bin_hz()),
            level_db: l.level,
            class: l.class,
            drift_hz_min: l.drift_hz_min,
            scatter_hz: l.scatter_hz,
        };
        self.events.push(BIRDIE_EVENT.event(
            t,
            row.frequency.0,
            &BirdieEvent {
                time_s: row.time_s,
                kind: row.kind,
                frequency: row.frequency,
                level_db: row.level_db,
                class: row.class,
                drift_hz_min: row.drift_hz_min,
                scatter_hz: row.scatter_hz,
            },
        ));
        self.log.push(row);
    }
}

impl Decoder for BirdieDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "birdie.finder"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::ThresholdDb => self.threshold_db = THRESHOLD_DB.read(id, value)?,
            DecoderSetting::PresencePct => self.presence_pct = PRESENCE_PCT.read(id, value)?,
            DecoderSetting::WindowS => {
                self.window_s = WINDOW_S.read(id, value)?;
                self.reset();
            }
            DecoderSetting::BareShare => self.bare_share = BARE_SHARE.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::ThresholdDb, Value::F64(self.threshold_db)),
            (DecoderSetting::PresencePct, Value::F64(self.presence_pct)),
            (DecoderSetting::WindowS, Value::F64(self.window_s)),
            (DecoderSetting::BareShare, Value::F64(self.bare_share)),
        ])
    }
    fn commands(&self) -> &'static [&'static Command] {
        &COMMANDS
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        if id.index() == 0 {
            self.reset();
            Ok(None)
        } else {
            Err(CommandError::NoSuchId { id })
        }
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if !input.rate.is_finite() || input.rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.dial_hz = input.tuned_hz;
        self.samples = 0;
        self.build();
        self.clear();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.level.push(x);
        let Some(mut spec) = self.spec.take() else {
            return;
        };
        let per = spec.samples() as f64;
        spec.push(x, |s| {
            self.snapshot(s.power, s.noise, (s.index + 1) as f64 * per / self.rate)
        });
        self.spec = Some(spec);
        self.samples += x.len() as u64;
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
    fn report(&mut self, p: &mut Published) {
        self.log.publish(&BIRDIE_CARRIER_EVENTS, p);
        let bin = self.bin_hz();
        let rows: Vec<BirdieCarrierRow> = self
            .listed
            .iter()
            .map(|l| BirdieCarrierRow {
                frequency: Hertz(self.dial_hz + l.bin * bin),
                level_db: l.level,
                level_sd_db: l.level_sd,
                class: l.class,
                drift_hz_min: l.drift_hz_min,
                scatter_hz: l.scatter_hz,
                present: l.present as i64,
                of: l.of as i64,
            })
            .collect();
        publish_list(&BIRDIE_CARRIERS, &rows, p);
        let now = self.samples as f64 / self.rate.max(1.0);
        let snapshots = self.tracker.snapshots();
        let headline = match self.listed.first() {
            _ if self.samples == 0 => "waiting for samples".to_string(),
            _ if (snapshots as usize) < MIN_SNAPSHOTS => "collecting spectra".to_string(),
            None => "no persistent narrow carriers".to_string(),
            Some(top) => format!(
                "{} persistent carriers; strongest {:.1} Hz at {:.1} dB over the noise",
                self.listed.len(),
                self.dial_hz + top.bin * bin,
                top.level
            ),
        };
        let level = self.level.read();
        p.put(&BirdieFields {
            carriers: self.listed.len() as i64,
            snapshots: snapshots as i64,
            snapshot_s: Seconds(self.snapshot_s()),
            bin_hz: bin,
        });
        let st = &mut self.status;
        st.lock_state = self.tally.lock(now);
        st.level_dbfs = level.map(|l| l as f32);
        st.offset_hz = None;
        st.snr_db = self.listed.first().map(|l| l.level as f32);
        st.summary = headline;
        p.put(&WatchFields {
            text: &st.summary,
            state: self.tally.state(now),
            good: i64::try_from(self.tally.good).unwrap_or(i64::MAX),
            bad: i64::try_from(self.tally.bad).unwrap_or(i64::MAX),
            level_dbfs: level,
            note: NOTE_TEXT,
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        0.25
    }
    fn clear(&mut self) {
        BirdieDecoder::clear(self);
    }
}

/// The receiver: one decoder on the input.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("finder", DecoderComponent::new(BirdieDecoder::default()))
        .input("finder")
        .output("finder.events")
        .status_from("finder")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(ID), graph).min_rate(
        MIN_RATE_HZ,
        "The persistent carrier finder needs a sample rate of at least 500 ksps",
    );
    for key in [
        THRESHOLD_DB.key(),
        PRESENCE_PCT.key(),
        WINDOW_S.key(),
        BARE_SHARE.key(),
    ] {
        g = g.route(key, &[&format!("finder.{key}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("finder", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("birdie.finder", || {
        Box::new(DecoderComponent::new(BirdieDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(BirdieGenerator::new(
        cfg.rate_hz,
        &cfg.settings,
        cfg.seed,
    )))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, three persistent carriers", o, 0.0);
    c.seconds = o.seconds.max(20.0);
    // The SNR is over the whole rate against a carrier of power 1. The weakest listed carrier is 56.5 dB under it: at 40 dB
    // it stands about 24 dB over the noise of a bin, at 20 dB only about 4 dB, under the 10 dB of a peak.
    c.snr_db = (!o.clean).then_some(40.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = crate::instrument::number(r, BirdieFields::CARRIERS.key);
        vec![CheckLine::new(
            BirdieFields::CARRIERS.key,
            n >= 3.0,
            format!("{n}, at least 3 expected"),
        )]
    });
    vec![c]
}

/// The engine's checks of the test signal.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The persistent carrier finder.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Persistent carrier finder",
    group: "Measurement",
    blurb: "Narrow carriers that stay in the capture: frequency, level, drift, scatter, and whether each is bare or \
            modulated.",
    tuning: Tuning {
        min_hz: 0.1e6,
        max_hz: 6e9,
        ..Tuning::anywhere(0.0, 2e6, MIN_RATE_HZ, 2e6)
    },
    submodes: &[],
    settings: BirdieSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::bench::benchmarks),
};
