//! SDR input health meter (`input_health`): of the raw samples that reach the stack, block by block, the RMS level and the
//! peak against a full scale of 1.0 per component, the crest factor, the share of samples near full scale, the DC offset,
//! the IQ gain and phase mismatch read blind from second-order statistics and the image rejection they imply, and a level
//! verdict. It measures and decodes nothing. The blind IQ reading assumes signals on both sides of the centre: a strong real
//! signal reads a mismatch that is not the radio's. It does not know the radio's true full scale. The verdict limits are
//! rules of thumb that no public document confirms.

pub mod gen;

use crate::instrument::Log;
use crate::txtest::{tx_stack, Ctx, TxTest};
use crate::typed::{
    InputEventKind, InputHealthEventRow, InputHealthFields, InputVerdict, INPUT_HEALTH_EVENTS,
};
use sdrkit_algo_dsp::iqmoments::{IqMoments, IqReading};
use sdrkit_core::interface::Seconds;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Choice, Complex32, Schema, Setting, SettingSet, Values};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "input_health";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 100e3;
/// Clipping rows come at most this often, s.
pub const CLIP_ROW_EVERY_S: f64 = 10.0;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the raw samples before any filter: level, peak, crest factor, samples near full \
scale, DC offset, and the IQ mismatch read blind from second-order statistics, with the image rejection it implies; decodes \
nothing. The IQ reading needs signals on both sides of the centre. The verdict limits are rules of thumb confirmed by no \
public document.";

/// The length of a block.
pub const BLOCK_S: Setting<field::F64> =
    Setting::new("block_s", "Reading every", "s", 1.0).range(0.1, 10.0, 0.0);
/// The clip level.
pub const CLIP_LEVEL: Setting<field::F64> =
    Setting::new("clip_level", "Clip at (of full scale)", "", 0.985).range(0.9, 1.0, 0.0);
/// The overload limit.
pub const OVERLOAD_PPM: Setting<field::F64> =
    Setting::new("overload_ppm", "Overload above", "ppm", 100.0).range(1.0, 10_000.0, 0.0);
/// The high limit.
pub const HIGH_DBFS: Setting<field::F64> =
    Setting::new("high_dbfs", "High above", "dBFS", -10.0).range(-80.0, 0.0, 0.0);
/// The low limit.
pub const LOW_DBFS: Setting<field::F64> =
    Setting::new("low_dbfs", "Low under", "dBFS", -50.0).range(-80.0, 0.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The block.
        BlockS = BLOCK_S,
        /// The clip level.
        ClipLevel = CLIP_LEVEL,
        /// The overload limit.
        OverloadPpm = OVERLOAD_PPM,
        /// The high limit.
        HighDbfs = HIGH_DBFS,
        /// The low limit.
        LowDbfs = LOW_DBFS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `input_health` stack.
    pub enum InputHealthSetting {
        /// The block.
        BlockS = BLOCK_S,
        /// The clip level.
        ClipLevel = CLIP_LEVEL,
        /// The overload limit.
        OverloadPpm = OVERLOAD_PPM,
        /// The high limit.
        HighDbfs = HIGH_DBFS,
        /// The low limit.
        LowDbfs = LOW_DBFS,
        /// The noise power of the test signal.
        GenPower = gen::GEN_POWER,
        /// Its tones.
        GenTones = gen::GEN_TONES,
        /// Its clip level.
        GenClip = gen::GEN_CLIP,
        /// Its gain after the clip.
        GenScale = gen::GEN_SCALE,
    }
}

/// One block's reading.
#[derive(Clone, Copy, Debug)]
struct Block {
    iq: IqReading,
    peak: f64,
    clip_ppm: f64,
    verdict: InputVerdict,
}

impl Block {
    fn rms_dbfs(&self) -> f64 {
        10.0 * self.iq.power.max(1e-30).log10()
    }
    fn peak_dbfs(&self) -> f64 {
        20.0 * self.peak.max(1e-15).log10()
    }
}

/// The measurement of `input_health`.
#[derive(Default)]
pub struct InputHealth {
    rate: f64,
    block: u64,
    clip: f32,
    overload: f64,
    high: f64,
    low: f64,
    sums: IqMoments,
    peak: f32,
    clips: u64,
    t: f64,
    last: Option<Block>,
    verdict: Option<InputVerdict>,
    last_clip_row: Option<f64>,
    log: Log<InputHealthEventRow>,
}

impl InputHealth {
    fn finish(&mut self, c: &mut Ctx) {
        let Some(iq) = self.sums.reading() else {
            return;
        };
        let clip_ppm = 1e6 * self.clips as f64 / iq.n as f64;
        let rms = 10.0 * iq.power.max(1e-30).log10();
        let verdict = if clip_ppm > self.overload {
            InputVerdict::Overload
        } else if rms > self.high {
            InputVerdict::High
        } else if rms < self.low {
            InputVerdict::Low
        } else {
            InputVerdict::Good
        };
        let b = Block {
            iq,
            peak: f64::from(self.peak),
            clip_ppm,
            verdict,
        };
        let row = |kind| InputHealthEventRow {
            time_s: Seconds(self.t),
            kind,
            verdict,
            rms_dbfs: b.rms_dbfs(),
            peak_dbfs: b.peak_dbfs(),
            clipping_ppm: clip_ppm,
        };
        if self.verdict != Some(verdict) {
            self.verdict = Some(verdict);
            self.log.push(row(InputEventKind::Verdict));
        }
        if clip_ppm > self.overload
            && self
                .last_clip_row
                .is_none_or(|t| self.t - t >= CLIP_ROW_EVERY_S)
        {
            self.last_clip_row = Some(self.t);
            self.log.push(row(InputEventKind::Clipping));
        }
        if verdict == InputVerdict::Overload {
            c.tally.bad(self.t);
        } else {
            c.tally.good(self.t);
        }
        self.last = Some(b);
        self.sums = IqMoments::default();
        self.peak = 0.0;
        self.clips = 0;
    }
}

impl TxTest for InputHealth {
    const ID: &'static str = ID;
    const NAME: &'static str = "input_health.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        self.rate = rate_hz;
        self.block = ((v.get(&BLOCK_S) * rate_hz).round() as u64).max(1);
        self.clip = v.get(&CLIP_LEVEL) as f32;
        self.overload = v.get(&OVERLOAD_PPM);
        self.high = v.get(&HIGH_DBFS);
        self.low = v.get(&LOW_DBFS);
        self.sums = IqMoments::default();
        self.peak = 0.0;
        self.clips = 0;
        self.last = None;
        self.verdict = None;
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let mut rest = x;
        while !rest.is_empty() {
            let take = ((self.block - self.sums.count()) as usize).min(rest.len());
            let (a, b) = rest.split_at(take);
            self.sums.push(a);
            for z in a {
                let m = z.re.abs().max(z.im.abs());
                self.peak = self.peak.max(m);
                self.clips += u64::from(m >= self.clip);
            }
            self.t += take as f64 / self.rate;
            rest = b;
            if self.sums.count() >= self.block {
                self.finish(c);
            }
        }
    }
    fn clear(&mut self) {
        self.last = None;
        self.verdict = None;
        self.last_clip_row = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&INPUT_HEALTH_EVENTS, p);
        let b = self.last;
        let iq = b.map(|b| b.iq);
        let dc2 = iq.map(|r| r.dc_i * r.dc_i + r.dc_q * r.dc_q);
        p.put(&InputHealthFields {
            rms_dbfs: b.map(|b| b.rms_dbfs()),
            peak_dbfs: b.map(|b| b.peak_dbfs()),
            crest_db: b.map(|b| b.peak_dbfs() - b.rms_dbfs()),
            clipping_ppm: b.map(|b| b.clip_ppm),
            dc_i_pct: iq.map(|r| 100.0 * r.dc_i),
            dc_q_pct: iq.map(|r| 100.0 * r.dc_q),
            dc_pct: dc2.map(|d| 100.0 * d.sqrt()),
            dc_under_rms_db: iq
                .zip(dc2)
                .filter(|(_, d)| *d > 0.0)
                .map(|(r, d)| 10.0 * (d / r.power.max(1e-30)).log10()),
            iq_gain_db: iq.and_then(|r| r.gain_mismatch_db()),
            iq_phase_deg: iq.and_then(|r| r.phase_error_rad()).map(f64::to_degrees),
            image_rejection_db: iq.and_then(|r| r.image_rejection_db()),
            verdict: b.map(|b| b.verdict),
        });
        match b {
            Some(b) => format!(
                "{}: level {:.1} dBFS, peak {:.1} dBFS, {:.0} ppm near full scale",
                b.verdict.label(),
                b.rms_dbfs(),
                b.peak_dbfs(),
                b.clip_ppm
            ),
            None => "first block".to_string(),
        }
    }
}

const KEYS: [&str; 5] = [
    "block_s",
    "clip_level",
    "overload_ppm",
    "high_dbfs",
    "low_dbfs",
];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("noise and two tones at -25.3 dBFS", o, 0.0);
    c.seconds = o.seconds.max(3.0);
    c.snr_db = None;
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let l = crate::instrument::number(r, InputHealthFields::RMS_DBFS.key);
        vec![CheckLine::new(
            InputHealthFields::RMS_DBFS.key,
            (l + 25.3).abs() < 1.5,
            format!("{l:.2} dBFS, -25.3 sent"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: InputHealth,
    settings: InputHealthSetting,
    keys: KEYS,
    name: "SDR input health meter",
    blurb: "Level, peak, crest factor, clipping, DC offset and blind IQ mismatch of the raw samples, with a level verdict.",
    tuning: (0.1e6, 6e9, 2e6, 2e6, false),
    rate_text: "The input health meter needs at least 100 ksps",
    generator: |cfg| gen::InputHealthGenerator::new(cfg.rate_hz, &cfg.settings, cfg.seed),
    verify: verify_cases
}
