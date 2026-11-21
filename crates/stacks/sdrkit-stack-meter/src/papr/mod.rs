//! Crest factor meter (`papr`): the distribution of the instantaneous power of the whole span against its mean (the
//! complementary cumulative distribution, CCDF), the peak and the crest factor, and a reading of the shape against complex
//! Gaussian noise, whose level at share `t` is `10 log10(-ln t)`. It measures the span with the radio's own noise and limits
//! and decodes nothing. No public document states the block rule or the reading limits.

pub mod gen;

use crate::instrument::Log;
use crate::txtest::{tx_stack, Ctx, TxTest};
use crate::typed::{PaprEventRow, PaprFields, PaprReading, PAPR_EVENTS};
use sdrkit_algo_dsp::ccdf::{ccdf, Ccdf};
use sdrkit_core::interface::Seconds;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Choice, Complex32, Schema, Setting, SettingSet, Values};
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "papr";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 100e3;
/// The samples of a block.
pub const BLOCK: usize = 1 << 14;
/// The shares of the CCDF: 10, 1, 0.1 and 0.01 %.
pub const SHARES: [f64; 4] = [0.1, 0.01, 0.001, 0.0001];
/// The 0.01 % level needs this many samples.
pub const MIN_FOR_001: usize = 1_000_000;
/// Rows after the first come at most this often, s.
pub const ROW_EVERY_S: f64 = 10.0;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the distribution of the instantaneous power of the whole span against its mean, the \
peak and the crest factor, and reads the shape against complex Gaussian noise; decodes nothing. The readings include the \
radio's own noise and limits. The block rule and the reading limits are confirmed by no public document.";

/// The samples in the statistics.
pub const KEEP_SAMPLES: Setting<field::I64> =
    Setting::new("keep_samples", "Samples in the statistics", "", 1 << 20).range(
        65_536.0,
        16_777_216.0,
        1.0,
    );
/// Blocks between evaluations.
pub const EVERY_BLOCKS: Setting<field::I64> =
    Setting::new("every_blocks", "Evaluate every (blocks of 16384)", "", 16).range(1.0, 256.0, 1.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The kept samples.
        KeepSamples = KEEP_SAMPLES,
        /// The evaluation period.
        EveryBlocks = EVERY_BLOCKS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `papr` stack.
    pub enum PaprSetting {
        /// The kept samples.
        KeepSamples = KEEP_SAMPLES,
        /// The evaluation period.
        EveryBlocks = EVERY_BLOCKS,
        /// The test signal.
        GenKind = gen::GEN_KIND,
        /// Its power.
        GenPower = gen::GEN_POWER,
        /// The limit of clipped noise.
        GenClip = gen::GEN_CLIP,
        /// The period of the impulses.
        GenImpulseEvery = gen::GEN_IMPULSE_EVERY,
        /// The size of an impulse.
        GenImpulseAmp = gen::GEN_IMPULSE_AMP,
        /// The QPSK symbol rate.
        GenSymbolRate = gen::GEN_SYMBOL_RATE,
        /// The QPSK roll-off.
        GenRolloff = gen::GEN_ROLLOFF,
    }
}

/// The reading of the shape of a CCDF (levels at 10, 1, 0.1 and 0.01 %).
pub fn reading(c: &Ccdf) -> PaprReading {
    let (l1, l01) = (c.levels_db[1], c.levels_db[2]);
    if c.crest_db < 2.0 {
        PaprReading::ConstantEnvelope
    } else if (5.5..=7.5).contains(&l1) && l01 > 7.3 {
        PaprReading::GaussianLike
    } else if l01 < 7.0 {
        PaprReading::LessPeaky
    } else {
        PaprReading::Peakier
    }
}

/// The measurement of `papr`.
#[derive(Default)]
pub struct Papr {
    rate: f64,
    keep_blocks: usize,
    every: u64,
    fill: Vec<f64>,
    blocks: VecDeque<Vec<f64>>,
    count: u64,
    t: f64,
    all: Vec<f64>,
    scratch: Vec<f64>,
    last: Option<(Ccdf, usize)>,
    last_row: Option<f64>,
    log: Log<PaprEventRow>,
}

impl Papr {
    fn evaluate(&mut self, c: &mut Ctx) {
        self.all.clear();
        for b in &self.blocks {
            self.all.extend_from_slice(b);
        }
        let n = self.all.len();
        let Some(r) = ccdf(&self.all, &SHARES, &mut self.scratch) else {
            self.last = None;
            return;
        };
        c.tally.good(self.t);
        if self.last_row.is_none_or(|t| self.t - t >= ROW_EVERY_S) {
            self.last_row = Some(self.t);
            self.log.push(PaprEventRow {
                time_s: Seconds(self.t),
                ccdf_10_db: r.levels_db[0],
                ccdf_1_db: r.levels_db[1],
                ccdf_01_db: r.levels_db[2],
                ccdf_001_db: (n >= MIN_FOR_001).then_some(r.levels_db[3]),
                crest_db: r.crest_db,
                mean_dbfs: 10.0 * r.mean.log10(),
                samples: n as i64,
            });
        }
        self.last = Some((r, n));
    }
}

impl TxTest for Papr {
    const ID: &'static str = ID;
    const NAME: &'static str = "papr.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        self.rate = rate_hz;
        self.keep_blocks = (v.get(&KEEP_SAMPLES).max(1) as usize)
            .div_ceil(BLOCK)
            .max(1);
        self.every = v.get(&EVERY_BLOCKS).max(1) as u64;
        self.fill.clear();
        self.blocks.clear();
        self.count = 0;
        self.last = None;
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        for z in x {
            self.fill.push(f64::from(z.norm_sqr()));
            if self.fill.len() < BLOCK {
                continue;
            }
            let b = if self.blocks.len() >= self.keep_blocks {
                let mut old = self.blocks.pop_front().unwrap_or_default();
                old.clear();
                std::mem::swap(&mut old, &mut self.fill);
                old
            } else {
                std::mem::replace(&mut self.fill, Vec::with_capacity(BLOCK))
            };
            self.blocks.push_back(b);
            self.t += BLOCK as f64 / self.rate;
            if self.count.is_multiple_of(self.every) {
                self.evaluate(c);
            }
            self.count += 1;
        }
    }
    fn clear(&mut self) {
        self.last = None;
        self.last_row = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&PAPR_EVENTS, p);
        let r = self.last.as_ref();
        let lv = |k: usize| r.map(|(c, _)| c.levels_db[k]);
        p.put(&PaprFields {
            mean_dbfs: r.map(|(c, _)| 10.0 * c.mean.log10()),
            peak_dbfs: r.map(|(c, _)| 10.0 * c.peak.max(1e-30).log10()),
            crest_db: r.map(|(c, _)| c.crest_db),
            ccdf_10_db: lv(0),
            ccdf_1_db: lv(1),
            ccdf_01_db: lv(2),
            ccdf_001_db: r
                .filter(|(_, n)| *n >= MIN_FOR_001)
                .map(|(c, _)| c.levels_db[3]),
            samples: r.map(|(_, n)| *n as i64),
            reading: r.map(|(c, _)| reading(c)),
        });
        match r {
            Some((c, _)) => format!(
                "crest {:.2} dB; {:.2} dB at 1 %, {:.2} dB at 0.1 %: {}",
                c.crest_db,
                c.levels_db[1],
                c.levels_db[2],
                reading(c).label()
            ),
            None => "no power".to_string(),
        }
    }
}

const KEYS: [&str; 2] = ["keep_samples", "every_blocks"];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("complex Gaussian noise", o, 0.0);
    c.seconds = o.seconds.max(3.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let l = crate::instrument::number(r, PaprFields::CCDF_1_DB.key);
        vec![CheckLine::new(
            PaprFields::CCDF_1_DB.key,
            (l - 6.63).abs() < 0.3,
            format!("{l:.2} dB, 6.63 for Gaussian noise"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: Papr,
    settings: PaprSetting,
    keys: KEYS,
    name: "Crest factor meter",
    blurb: "Mean power, peak, crest factor and the power CCDF at 10, 1, 0.1 and 0.01 % of the span, read against Gaussian noise.",
    tuning: (0.1e6, 6e9, 1e6, 500e3, false),
    rate_text: "The crest factor meter needs at least 100 ksps",
    generator: |cfg| gen::PaprGenerator::new(cfg.rate_hz, &cfg.settings, cfg.seed),
    verify: verify_cases
}
