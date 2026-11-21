//! Noise figure meter (`noise_figure`), the Y-factor method of the IRE Standards on Methods of Measuring Noise in Linear
//! Twoports (1959): a noise source switched on and off before the receiver steps the power of the middle of the span; the
//! meter finds the plateaus and the steps between them, gives `Y = P_hot / P_cold` as the mean of the last steps, and the
//! receiver's noise figure `NF = ENR - 10 log10(Y - 1)` for each source ENR of a setting. It measures and decodes nothing; it
//! needs a linear receiver with fixed gain and noise only in the band, and cannot know that the source is calibrated. No
//! public document states the plateau rule.

pub mod gen;

use crate::instrument::{publish_list, Front, Log};
use crate::txtest::{tx_stack, Ctx, TxTest};
use crate::typed::{
    NoiseFigureEventRow, NoiseFigureFields, NoiseFigureRow, NoiseSourceState, NOISE_FIGURE_EVENTS,
    NOISE_FIGURE_TABLE,
};
use sdrkit_algo_dsp::plateau::mean_and_scatter;
use sdrkit_algo_dsp::stepfind::StepFinder;
use sdrkit_core::interface::Seconds;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "noise_figure";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 250e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 250e3;
/// The pass edge of the channel: the middle 200 kHz.
pub const PASS_HZ: f64 = 100e3;
/// The stretch dropped at the start, s.
pub const SKIP_S: f64 = 0.1;
/// A power block, s.
pub const BLOCK_S: f64 = 0.025;
/// Blocks of a window.
pub const WINDOW_BLOCKS: usize = 10;
/// Plateaus stored.
pub const PLATEAUS_KEPT: usize = 8;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the steps of the noise power of the middle 200 kHz as a noise source is switched, the \
Y-factor and the noise figure for each source ENR of the setting (IRE 1959 Y-factor method); decodes nothing. It needs fixed \
gain, no AGC and noise only in the band, and cannot know that the source is calibrated. The plateau rule is confirmed by no \
public document.";

/// The ENR values of the sources.
pub const ENR_DB: Setting<field::Text> =
    Setting::new("enr_db", "Source ENR values", "dB", "5, 6.5, 15, 25")
        .help("Values from 0 to 40 dB separated by ','.");
/// A window joins a plateau within this.
pub const AGREE_DB: Setting<field::F64> =
    Setting::new("agree_db", "A window joins a plateau within", "dB", 0.15).range(0.02, 1.0, 0.0);
/// The smallest step.
pub const STEP_DB: Setting<field::F64> =
    Setting::new("step_db", "Smallest step", "dB", 0.3).range(0.1, 3.0, 0.0);
/// The steps averaged.
pub const STEPS_KEPT: Setting<field::I64> =
    Setting::new("steps_kept", "Steps averaged", "", 4).range(1.0, 32.0, 1.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The ENR list.
        EnrDb = ENR_DB,
        /// The plateau rule.
        AgreeDb = AGREE_DB,
        /// The step rule.
        StepDb = STEP_DB,
        /// The steps averaged.
        StepsKept = STEPS_KEPT,
    }
}

sdrkit_core::settings! {
    /// The settings of the `noise_figure` stack.
    pub enum NoiseFigureSetting {
        /// The ENR list.
        EnrDb = ENR_DB,
        /// The plateau rule.
        AgreeDb = AGREE_DB,
        /// The step rule.
        StepDb = STEP_DB,
        /// The steps averaged.
        StepsKept = STEPS_KEPT,
        /// The step of the test signal.
        GenYDb = gen::GEN_Y_DB,
        /// Its period.
        GenPeriodS = gen::GEN_PERIOD_S,
        /// Its gain drift.
        GenDriftDbS = gen::GEN_DRIFT_DB_S,
        /// Its receiver noise.
        GenP0 = gen::GEN_P0,
    }
}

/// The values of a list separated by ',' within 0 to 40.
pub fn parse_enr(text: &str) -> Vec<f64> {
    text.split(',')
        .filter_map(|e| e.trim().parse::<f64>().ok())
        .filter(|v| (0.0..=40.0).contains(v))
        .collect()
}

/// The measurement of `noise_figure`.
#[derive(Default)]
pub struct NoiseFigure {
    front: Option<Front>,
    block: usize,
    acc: f64,
    acc_n: usize,
    blocks: Vec<f64>,
    finder: Option<StepFinder>,
    enr: Vec<f64>,
    keep: usize,
    steps: VecDeque<f64>,
    seen: i64,
    t: f64,
    dt: f64,
    log: Log<NoiseFigureEventRow>,
}

impl NoiseFigure {
    fn window(&mut self, w: f64, c: &mut Ctx) {
        let Some(s) = self.finder.as_mut().and_then(|f| f.push(w)) else {
            return;
        };
        self.seen += 1;
        self.steps.push_back(s.size_db());
        while self.steps.len() > self.keep {
            self.steps.pop_front();
        }
        c.tally.good(self.t);
        self.log.push(NoiseFigureEventRow {
            time_s: Seconds(self.t),
            state: if s.rose() {
                NoiseSourceState::SourceOn
            } else {
                NoiseSourceState::SourceOff
            },
            from_db: s.from_db,
            to_db: s.to_db,
            y_db: s.size_db(),
            y_lin: 10f64.powf(s.size_db() / 10.0),
        });
    }

    fn y(&self) -> Option<(f64, f64)> {
        mean_and_scatter(&self.steps.iter().copied().collect::<Vec<_>>())
    }
}

impl TxTest for NoiseFigure {
    const ID: &'static str = ID;
    const NAME: &'static str = "noise_figure.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let front = Front::new(rate_hz, WORK_HZ, PASS_HZ, SKIP_S);
        self.block = ((BLOCK_S * front.rate()) as usize).max(16);
        self.dt = 1.0 / front.rate();
        self.front = Some(front);
        self.finder = Some(StepFinder::new(
            v.get(&AGREE_DB),
            v.get(&STEP_DB),
            PLATEAUS_KEPT,
        ));
        self.enr = parse_enr(v.get(&ENR_DB));
        self.keep = v.get(&STEPS_KEPT).max(1) as usize;
        self.acc = 0.0;
        self.acc_n = 0;
        self.blocks.clear();
        self.steps.clear();
        self.t = self.t.max(SKIP_S);
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let Some(mut front) = self.front.take() else {
            return;
        };
        for z in front.process(x) {
            self.acc += f64::from(z.norm_sqr());
            self.acc_n += 1;
            self.t += self.dt;
            if self.acc_n < self.block {
                continue;
            }
            self.blocks
                .push(10.0 * (self.acc / self.acc_n as f64).max(1e-30).log10());
            self.acc = 0.0;
            self.acc_n = 0;
            if self.blocks.len() == WINDOW_BLOCKS {
                let w = self.blocks.iter().sum::<f64>() / WINDOW_BLOCKS as f64;
                self.blocks.clear();
                self.window(w, c);
            }
        }
        self.front = Some(front);
    }
    fn clear(&mut self) {
        self.steps.clear();
        self.seen = 0;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&NOISE_FIGURE_EVENTS, p);
        let y = self.y();
        let ylin = y.map(|(m, _)| 10f64.powf(m / 10.0));
        let rows: Vec<NoiseFigureRow> = self
            .enr
            .iter()
            .map(|&e| {
                let nf = ylin
                    .filter(|&l| l > 1.0)
                    .map(|l| e - 10.0 * (l - 1.0).log10());
                NoiseFigureRow {
                    enr_db: e,
                    nf_db: nf,
                    negative: nf.is_some_and(|n| n < 0.0),
                }
            })
            .collect();
        publish_list(&NOISE_FIGURE_TABLE, &rows, p);
        let power = self.finder.as_ref().and_then(StepFinder::level_db);
        p.put(&NoiseFigureFields {
            power_db: power,
            y_db: y.map(|(m, _)| m),
            y_lin: ylin,
            y_spread_db: y.map(|(_, s)| s),
            steps: self.seen,
        });
        match y {
            Some((m, s)) => format!(
                "Y {m:.2} dB (spread {s:.2} dB) over {} steps",
                self.steps.len()
            ),
            None => "no step yet".to_string(),
        }
    }
}

const KEYS: [&str; 4] = ["enr_db", "agree_db", "step_db", "steps_kept"];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("a source step of 10 dB every 10 s", o, 0.0);
    c.seconds = o.seconds.max(45.0);
    c.snr_db = None;
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let y = crate::instrument::number(r, NoiseFigureFields::Y_DB.key);
        vec![CheckLine::new(
            NoiseFigureFields::Y_DB.key,
            (y - 10.0).abs() < 0.12,
            format!("{y:.3} dB, 10 sent"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: NoiseFigure,
    settings: NoiseFigureSetting,
    keys: KEYS,
    name: "Noise figure (Y-factor) meter",
    blurb: "The hot and cold plateaus of the noise power as a source is switched, the Y-factor and the noise figure for given ENR values.",
    tuning: (0.1e6, 6e9, 2e6, 200e3, false),
    rate_text: "The noise figure meter needs at least 250 ksps",
    generator: |cfg| gen::NoiseFigureGenerator::new(cfg.rate_hz, &cfg.settings, cfg.seed),
    verify: verify_cases
}
