//! Carrier frequency accuracy meter (`freq_accuracy`): the offset of a known carrier from the tuned frequency, in Hz and
//! ppm, its C/N0, the mean and scatter of the last readings, the drift, the Allan deviation (the non-overlapping estimator
//! of IEEE Std 1139 and ITU-R TF.538), steps, and the loss and return of the carrier. It measures and decodes nothing. The
//! reading is the sum of the radio's frequency error and the transmitter's. The windows, thresholds and rules are confirmed
//! by no public document.

pub mod gen;

use crate::instrument::Log;
use crate::txtest::{Ctx, TxTest};
use crate::typed::{
    FreqAccuracyAllanRow, FreqAccuracyEvent, FreqAccuracyEventRow, FreqAccuracyFields,
    FreqAccuracyKind, FREQ_ACCURACY_ALLAN, FREQ_ACCURACY_EVENT, FREQ_ACCURACY_EVENTS,
};
use gen::FreqAccuracyGenerator;
use sdrkit_algo_dsp::allan::allan_octaves;
use sdrkit_algo_dsp::carrierfind::{block_len, CarrierFinder, FinderConfig};
use sdrkit_algo_dsp::peak::line_fit;
use sdrkit_algo_dsp::plateau::{mean_and_scatter, StepRule};
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver, Schema, Setting,
    SettingSet, StackConfig, StackDescriptor, Tuning, Values,
};
use sdrkit_graph::{GraphError, StackGraph};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "freq_accuracy";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 16e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 16e3;
/// The pass edge of the channel filter, Hz.
pub const PASS_HZ: f64 = 3000.0;
/// The stretch dropped at the start while the filter fills, s.
pub const SKIP_S: f64 = 0.2;
/// The half width of the band whose bins give the noise, Hz.
pub const BAND_HZ: f64 = 3000.0;
/// Bins next to the peak that do not count for the noise.
pub const EXCLUDE_BINS: usize = 8;
/// The readings kept; at this count the oldest [`DROP_READINGS`] go.
pub const MAX_READINGS: usize = 4000;
/// The readings dropped at [`MAX_READINGS`].
pub const DROP_READINGS: usize = 1000;
/// The readings of the mean and the scatter.
pub const MEAN_OF: usize = 10;
/// The readings of the drift line.
pub const DRIFT_OF: usize = 120;
/// The least readings for a drift.
pub const DRIFT_MIN: usize = 8;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the offset of a known carrier from the tuned frequency, its C/N0, drift, Allan \
deviation, steps, loss and return; decodes nothing. The reading is the sum of the radio's frequency error and the \
transmitter's: a carrier known to 1e-7 limits what a ppm reading means. The ppm values need the tuned frequency. The \
windows and thresholds are confirmed by no public document.";

/// The search half width.
pub const SEARCH_HZ: Setting<field::F64> =
    Setting::new("search_hz", "Search half width", "Hz", 2800.0).range(100.0, 2800.0, 0.0);
/// The carrier over the noise per bin.
pub const PRESENT_DB: Setting<field::F64> =
    Setting::new("present_db", "Carrier over the noise per bin", "dB", 16.0).range(6.0, 40.0, 0.0);
/// The step threshold in scatter widths.
pub const STEP_SIGMAS: Setting<field::F64> =
    Setting::new("step_sigmas", "Step threshold in scatter widths", "", 8.0).range(3.0, 20.0, 0.0);
/// The smallest step.
pub const STEP_MIN_HZ: Setting<field::F64> =
    Setting::new("step_min_hz", "Smallest step", "Hz", 0.5).range(0.01, 100.0, 0.0);
/// The interval of the frequency rows.
pub const ROW_EVERY_S: Setting<field::F64> =
    Setting::new("row_every_s", "Frequency row every", "s", 10.0).range(1.0, 600.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The search half width.
        SearchHz = SEARCH_HZ,
        /// The presence threshold.
        PresentDb = PRESENT_DB,
        /// The step threshold.
        StepSigmas = STEP_SIGMAS,
        /// The smallest step.
        StepMinHz = STEP_MIN_HZ,
        /// The row interval.
        RowEveryS = ROW_EVERY_S,
    }
}

sdrkit_core::settings! {
    /// The settings of the `freq_accuracy` stack.
    pub enum FreqAccuracySetting {
        /// The search half width.
        SearchHz = SEARCH_HZ,
        /// The presence threshold.
        PresentDb = PRESENT_DB,
        /// The step threshold.
        StepSigmas = STEP_SIGMAS,
        /// The smallest step.
        StepMinHz = STEP_MIN_HZ,
        /// The row interval.
        RowEveryS = ROW_EVERY_S,
        /// The steps of the test signal.
        GenSteps = gen::GEN_STEPS,
        /// The gaps of the test signal.
        GenGaps = gen::GEN_GAPS,
        /// The frequency noise of the test signal.
        GenFmNoiseHz = gen::GEN_FM_NOISE_HZ,
        /// Its hold.
        GenFmHoldS = gen::GEN_FM_HOLD_S,
    }
}

/// The measurement of `freq_accuracy`.
#[derive(Default)]
pub struct FreqAccuracy {
    front: Option<crate::instrument::Front>,
    finder: Option<CarrierFinder>,
    rate: f64,
    buf: Vec<Complex32>,
    t: f64,
    step: StepRule,
    row_every: f64,
    present: Option<bool>,
    was_present: bool,
    absent_run: u32,
    lost: bool,
    cn0: Option<f64>,
    readings: Vec<(f64, f64)>,
    last_row: Option<f64>,
    log: Log<FreqAccuracyEventRow>,
}

impl FreqAccuracy {
    fn row(
        &mut self,
        c: &mut Ctx,
        kind: FreqAccuracyKind,
        f: f64,
        step: Option<f64>,
        cn0: Option<f64>,
    ) {
        let row = FreqAccuracyEventRow {
            time_s: Seconds(self.t),
            kind,
            offset_hz: Hertz(f),
            step_hz: step.map(Hertz),
            cn0_dbhz: cn0,
        };
        c.events.push(FREQ_ACCURACY_EVENT.event(
            self.t,
            c.dial(),
            &FreqAccuracyEvent {
                time_s: row.time_s,
                kind,
                offset_hz: row.offset_hz,
                step_hz: row.step_hz,
                cn0_dbhz: cn0,
            },
        ));
        self.log.push(row);
    }

    fn window(&mut self, c: &mut Ctx) {
        let Some(look) = self.finder.as_mut().and_then(|f| f.look(&self.buf)) else {
            return;
        };
        if !look.present {
            self.present = Some(false);
            self.cn0 = None;
            self.absent_run += 1;
            if self.was_present && self.absent_run >= 2 {
                self.was_present = false;
                self.lost = true;
                let f = self.readings.last().map_or(0.0, |r| r.1);
                self.row(c, FreqAccuracyKind::Lost, f, None, None);
            }
            if self.lost {
                c.tally.energy(self.t);
            }
            return;
        }
        let f = look.offset_hz;
        let cn0 = look.cn0_dbhz;
        let present_before = self.present == Some(true);
        self.present = Some(true);
        self.absent_run = 0;
        self.cn0 = Some(cn0);
        if present_before {
            let hist: Vec<f64> = self.readings.iter().map(|r| r.1).collect();
            if let Some(old) = self.step.test(&hist, f) {
                self.row(c, FreqAccuracyKind::Step, f, Some(f - old), Some(cn0));
            }
        }
        if self.lost && !self.readings.is_empty() {
            self.row(c, FreqAccuracyKind::Returned, f, None, Some(cn0));
        }
        self.lost = false;
        self.was_present = true;
        self.readings.push((self.t, f));
        if self.readings.len() >= MAX_READINGS {
            self.readings.drain(..DROP_READINGS);
        }
        c.tally.good(self.t);
        if self.last_row.is_none_or(|t| self.t - t >= self.row_every) {
            self.last_row = Some(self.t);
            self.row(c, FreqAccuracyKind::Frequency, f, None, Some(cn0));
        }
    }

    fn window_s(&self) -> Option<f64> {
        self.finder.as_ref().map(|f| f.len() as f64 / self.rate)
    }
}

impl TxTest for FreqAccuracy {
    const ID: &'static str = ID;
    const NAME: &'static str = "freq_accuracy.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let front = crate::instrument::Front::new(rate_hz, WORK_HZ, PASS_HZ, SKIP_S);
        self.rate = front.rate();
        let n = block_len(self.rate, 2.0);
        let ratio = 10f64.powf(v.get(&PRESENT_DB) / 10.0);
        let cfg = FinderConfig::new(v.get(&SEARCH_HZ), BAND_HZ, EXCLUDE_BINS, ratio);
        self.finder = CarrierFinder::new(n, self.rate, cfg).ok();
        self.front = Some(front);
        self.buf.clear();
        self.t = self.t.max(SKIP_S);
        self.step = StepRule {
            median_of: 5,
            scatter_of: MEAN_OF,
            default_scatter: 0.1,
            sigmas: v.get(&STEP_SIGMAS),
            min_step: v.get(&STEP_MIN_HZ),
        };
        self.row_every = v.get(&ROW_EVERY_S);
        self.present = None;
        self.absent_run = 0;
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let (Some(mut front), Some(n)) = (
            self.front.take(),
            self.finder.as_ref().map(CarrierFinder::len),
        ) else {
            return;
        };
        for &z in front.process(x) {
            self.buf.push(z);
            if self.buf.len() == n {
                self.t += n as f64 / self.rate;
                self.window(c);
                self.buf.clear();
            }
        }
        self.front = Some(front);
    }
    fn clear(&mut self) {
        self.readings.clear();
        self.last_row = None;
        self.was_present = false;
        self.lost = false;
        self.present = None;
        self.cn0 = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, c: &Ctx) -> String {
        self.log.publish(&FREQ_ACCURACY_EVENTS, p);
        let on = self.present == Some(true);
        let m = self.readings.len();
        let last: Vec<f64> = self.readings[m.saturating_sub(MEAN_OF)..]
            .iter()
            .map(|r| r.1)
            .collect();
        let ms = if on { mean_and_scatter(&last) } else { None };
        let drift = (on && m >= DRIFT_MIN)
            .then(|| {
                let r = &self.readings[m.saturating_sub(DRIFT_OF)..];
                let (x, y): (Vec<f64>, Vec<f64>) = r.iter().copied().unzip();
                line_fit(&x, &y, None).ok().map(|l| l.slope)
            })
            .flatten();
        let tuned = c.tuned_hz;
        let ys: Vec<f64> = self.readings.iter().map(|r| r.1).collect();
        let tw = self.window_s().unwrap_or(0.0);
        let rows: Vec<FreqAccuracyAllanRow> = allan_octaves(&ys, 16)
            .iter()
            .map(|a| FreqAccuracyAllanRow {
                tau_s: Seconds(a.factor as f64 * tw),
                adev: tuned.map(|f| a.sigma / f),
                adev_hz: Hertz(a.sigma),
                groups: a.groups as i64,
            })
            .collect();
        crate::instrument::publish_list(&FREQ_ACCURACY_ALLAN, &rows, p);
        p.put(&FreqAccuracyFields {
            offset_hz: ms.map(|v| Hertz(v.0)),
            scatter_hz: ms.map(|v| Hertz(v.1)),
            error_ppm: ms.and_then(|v| tuned.map(|f| v.0 / f * 1e6)),
            cn0_dbhz: if on { self.cn0 } else { None },
            drift_hz_s: drift,
            drift_ppm_h: drift.and_then(|d| tuned.map(|f| d * 3600.0 / f * 1e6)),
            readings: m as i64,
            window_s: self.window_s().map(Seconds),
            carrier: self.present,
        });
        match (ms, self.cn0) {
            (Some((o, s)), Some(cn0)) => {
                format!("carrier at {o:+.2} Hz (scatter {s:.2} Hz), {cn0:.1} dB-Hz, {m} readings")
            }
            _ if self.lost => "carrier lost".to_string(),
            _ => "searching for a carrier".to_string(),
        }
    }
}

const KEYS: [&str; 5] = [
    "search_hz",
    "present_db",
    "step_sigmas",
    "step_min_hz",
    "row_every_s",
];

/// The receiver.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    crate::txtest::receiver::<FreqAccuracy>(
        cfg,
        &KEYS,
        "The frequency accuracy meter needs at least 16 ksps",
    )
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    crate::txtest::register::<FreqAccuracy>(r);
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(FreqAccuracyGenerator::new(
        cfg.rate_hz,
        &cfg.settings,
        cfg.seed,
    )))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("a carrier 37 Hz off the tuned frequency", o, 37.0);
    c.seconds = o.seconds.max(12.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = crate::instrument::number(r, FreqAccuracyFields::OFFSET_HZ.key);
        vec![CheckLine::new(
            FreqAccuracyFields::OFFSET_HZ.key,
            (f - 37.0).abs() < 0.5,
            format!("{f:.2} Hz, 37 sent"),
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

/// The carrier frequency accuracy meter.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Carrier frequency accuracy meter",
    group: "Measurement",
    blurb: "The offset of a known carrier in Hz and ppm, its C/N0, drift, Allan deviation, steps, loss and return.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 0.1e6,
        max_hz: 6e9,
        carrier_at_centre: true,
        ..Tuning::anywhere(0.0, 48e3, MIN_RATE_HZ, 6e3)
    },
    submodes: &[],
    settings: FreqAccuracySetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::bench::benchmarks),
};
