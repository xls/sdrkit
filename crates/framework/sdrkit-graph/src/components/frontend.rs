//! The front-end clean-up: DC removal, IQ imbalance correction and the noise blanker.

use super::Every;
use crate::component::{no_values, values_of};
use crate::component::{Applied, Component, ComponentInfo, Io, Port};
use crate::stream::Format;
use sdrkit_algo_dsp::dc::{DcEstimator, IqImbalance};
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::field;
use sdrkit_core::Values;
use sdrkit_core::{
    Complex32, ConfigError, Published, Results, Setting, SettingError, SettingId, SettingSet, Value,
};

/// The result set of [`DcRemoval`].
#[derive(Clone, Copy, Debug, PartialEq, Fields)]
pub struct DcRemovalFields {
    /// The level that the component removes. No value before the first measurement.
    #[field(label = "DC level", unit = "dBFS")]
    pub dc_dbfs: Option<f64>,
}

/// The result set of [`IqCorrection`]. No field has a value before the first measurement.
#[derive(Clone, Copy, Debug, PartialEq, Fields)]
pub struct IqCorrectionFields {
    /// The gain error between I and Q.
    #[field(label = "Gain error", unit = "dB")]
    pub gain_error_db: Option<f64>,
    /// The phase error between I and Q.
    #[field(label = "Phase error", unit = "°")]
    pub phase_error_deg: Option<f64>,
}

/// The result set of [`NoiseBlanker`]. No field has a value in the first second.
#[derive(Clone, Copy, Debug, PartialEq, Fields)]
pub struct NoiseBlankerFields {
    /// The share of the last second that was blanked.
    #[field(label = "Blanked", unit = "%")]
    pub blanked_pct: Option<f64>,
    /// The blanks since the start.
    #[field(label = "Blank events")]
    pub blank_events: Option<i64>,
}

const THRESHOLD: Setting<field::F64> = Setting::new("threshold", "Threshold", "x", 10.0)
    .range(2.0, 40.0, 0.0)
    .help("Blank above this many times the mean level.");
const WIDTH: Setting<field::Duration> = Setting::new("width", "Width", "s", 100e-6)
    .range(2e-6, 2e-3, 0.0)
    .help("How long each blank lasts.");

sdrkit_core::settings! {
    /// The settings of [`NoiseBlanker`].
    pub enum NoiseBlankerSetting {
        /// The level that starts a blank, in times the mean magnitude.
        Threshold = THRESHOLD,
        /// How long a blank lasts.
        Width = WIDTH,
    }
}

/// DC removal (`dsp.dc_removal`): a running mean of I and of Q is subtracted from each sample. The notch is a
/// fraction of a hertz wide once it has settled. No settings; it starts fresh when configured.
///
/// Results: `dc_dbfs`, the level removed.
#[derive(Debug)]
pub struct DcRemoval {
    mean: DcEstimator,
    every: Every,
    results: Published,
}

impl Default for DcRemoval {
    fn default() -> Self {
        DcRemoval {
            mean: DcEstimator::default(),
            every: Every::default(),
            results: Published::new(DcRemovalFields::SHAPE.fields),
        }
    }
}

impl Component for DcRemoval {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.dc_removal",
            vec![Port::iq("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
        .in_place(0, 0)
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = DcRemoval {
            mean: DcEstimator::new(),
            every: Every::new(inputs[0].rate, 0.25),
            ..DcRemoval::default()
        };
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let buf = io.pass_through::<Complex32>(0, 0);
        self.mean.remove(buf);
        if self.every.tick(buf.len()) {
            let m = self.mean.mean();
            self.results.put(&DcRemovalFields {
                dc_dbfs: Some(sdrkit_algo_dsp::meter::power_to_db(m.norm_sqr() as f64)),
            });
        }
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

/// IQ imbalance correction (`dsp.iq_correction`), blind (sdrkit-algo-dsp [`IqImbalance`], on the signal minus
/// its running mean [`DcEstimator`], which is added back, so DC and a carrier on the centre pass untouched).
///
/// Results: `gain_error_db`, `phase_error_deg`.
#[derive(Debug)]
pub struct IqCorrection {
    mean: DcEstimator,
    iq: IqImbalance,
    every: Every,
    results: Published,
}

impl Default for IqCorrection {
    fn default() -> Self {
        IqCorrection {
            mean: DcEstimator::default(),
            iq: IqImbalance::default(),
            every: Every::default(),
            results: Published::new(IqCorrectionFields::SHAPE.fields),
        }
    }
}

impl IqCorrection {
    /// Samples per statistics batch.
    pub const CHUNK: usize = IqImbalance::BATCH;

    /// The gain error in dB and the phase error in degrees, as measured so far.
    pub fn imbalance(&self) -> (f64, f64) {
        self.iq.imbalance()
    }
}

impl Component for IqCorrection {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.iq_correction",
            vec![Port::iq("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
        .in_place(0, 0)
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = IqCorrection {
            every: Every::new(inputs[0].rate, 0.25),
            ..Default::default()
        };
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let buf = io.pass_through::<Complex32>(0, 0);
        let n = buf.len();
        self.iq.process(&mut self.mean, buf, true);
        if self.every.tick(n) {
            let (g, p) = self.imbalance();
            self.results.put(&IqCorrectionFields {
                gain_error_db: Some(g),
                phase_error_deg: Some(p),
            });
        }
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

/// Noise blanker (`dsp.noise_blanker`): an impulse blanker.
///
/// It follows the mean magnitude with a one-pole average of time constant `max(64 samples, 10 ms)`, started by the first sample. A
/// sample above `threshold` times the mean starts (or stretches) a blank of `width` seconds (at least one sample): the samples are
/// set to 0 and the average does not learn from them. A click inside a blank stretches it and is not a new event.
///
/// Settings: `threshold` (2 to 40, default 10), `width` (seconds, 2 us to 2 ms, default 100 us). Results: `blanked_pct` (of the last
/// second), `blank_events`.
#[derive(Debug)]
pub struct NoiseBlanker {
    threshold: f64,
    width_s: f64,
    rate: f64,
    alpha: f32,
    avg: f32,
    started: bool,
    width: u64,
    left: u64,
    events: u64,
    blanked_window: u64,
    every: Every,
    results: Published,
}

impl Default for NoiseBlanker {
    fn default() -> Self {
        NoiseBlanker {
            threshold: THRESHOLD.default,
            width_s: WIDTH.default,
            rate: 0.0,
            alpha: 0.0,
            avg: 0.0,
            started: false,
            width: 1,
            left: 0,
            events: 0,
            blanked_window: 0,
            every: Every::default(),
            results: Published::new(NoiseBlankerFields::SHAPE.fields),
        }
    }
}

impl NoiseBlanker {
    /// A blanker at `threshold` times the mean magnitude, blanking `width_s` seconds.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `threshold` or `width` setting refuses its value.
    pub fn new(threshold: f64, width_s: f64) -> Result<Self, SettingError> {
        NoiseBlankerSetting::Threshold.check(&Value::F64(threshold))?;
        NoiseBlankerSetting::Width.check(&Value::Duration(width_s))?;
        Ok(NoiseBlanker {
            threshold,
            width_s,
            ..Self::default()
        })
    }
    /// Blank events so far.
    pub fn events(&self) -> u64 {
        self.events
    }
    fn set_width(&mut self) {
        self.width = ((self.width_s * self.rate).round() as u64).max(1);
    }
}

impl Component for NoiseBlanker {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.noise_blanker",
            vec![Port::iq("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
        .in_place(0, 0)
        .with_settings(NoiseBlankerSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([
            (NoiseBlankerSetting::Threshold, Value::F64(self.threshold)),
            (NoiseBlankerSetting::Width, Value::Duration(self.width_s)),
        ])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let rate = inputs[0].rate;
        if rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        *self = NoiseBlanker {
            rate,
            every: Every::new(rate, 1.0),
            threshold: self.threshold,
            width_s: self.width_s,
            ..NoiseBlanker::default()
        };
        self.alpha = (1.0 / (0.01 * rate).max(64.0)) as f32;
        self.set_width();
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let buf = io.pass_through::<Complex32>(0, 0);
        let thr = self.threshold as f32;
        for x in buf.iter_mut() {
            let mag = x.norm();
            if !self.started {
                self.avg = mag;
                self.started = true;
            }
            if mag > thr * self.avg {
                if self.left == 0 {
                    self.events += 1;
                }
                self.left = self.width;
            }
            if self.left > 0 {
                self.left -= 1;
                self.blanked_window += 1;
                *x = Complex32::default();
            } else {
                self.avg += self.alpha * (mag - self.avg);
            }
            if self.every.tick(1) {
                let pct = 100.0 * self.blanked_window as f64 / self.rate;
                self.blanked_window = 0;
                self.results.put(&NoiseBlankerFields {
                    blanked_pct: Some(pct),
                    blank_events: Some(self.events as i64),
                });
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match NoiseBlankerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            NoiseBlankerSetting::Threshold => self.threshold = THRESHOLD.read(id, v)?,
            NoiseBlankerSetting::Width => {
                self.width_s = WIDTH.read(id, v)?;
                self.set_width();
            }
        }
        Ok(Applied::Live)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::testing::Harness;
    use crate::stream::Data;

    fn run(c: &mut dyn Component, x: &[Complex32], piece: usize) -> Vec<Complex32> {
        let mut h = Harness::new(c, vec![Format::iq(1e6, 0.0, 0.0)]);
        let mut out = Vec::new();
        for chunk in x.chunks(piece) {
            h.run(c, |ins| {
                if let Data::Iq(v) = &mut ins[0].data {
                    v.extend_from_slice(chunk)
                }
            });
            if let Data::Iq(v) = &h.outs[0].data {
                out.extend_from_slice(v)
            }
        }
        out
    }

    fn tone(n: usize, f: f64, amp: f32, dc: Complex32) -> Vec<Complex32> {
        (0..n)
            .map(|i| {
                let p = std::f64::consts::TAU * f * i as f64;
                Complex32::new(amp * p.cos() as f32, amp * p.sin() as f32) + dc
            })
            .collect()
    }

    #[test]
    fn dc_removal_takes_the_offset_away_and_keeps_a_tone() {
        let x = tone(400_000, 0.01, 0.5, Complex32::new(0.2, -0.1));
        let y = run(&mut DcRemoval::default(), &x, 10_000);
        let tail = &y[300_000..];
        let mean: Complex32 = tail.iter().copied().sum::<Complex32>() / tail.len() as f32;
        assert!(mean.norm() < 2e-3, "dc left {}", mean.norm());
        let p = tail.iter().map(|v| v.norm_sqr()).sum::<f32>() / tail.len() as f32;
        assert!((p - 0.25).abs() < 0.01, "tone power {p}");
    }

    #[test]
    fn iq_correction_removes_an_image() {
        // A tone with 1.1 gain error and 5 degrees of skew on Q, plus noise-like second tone.
        let n = 1 << 20;
        let (ph, g) = (5f32.to_radians(), 1.1f32);
        let x: Vec<Complex32> = (0..n)
            .map(|i| {
                let t = i as f64;
                let a = std::f64::consts::TAU * 0.0123 * t;
                let b = std::f64::consts::TAU * -0.2711 * t;
                let (i_, q) = (
                    (a.cos() + 0.5 * b.cos()) as f32,
                    (a.sin() + 0.5 * b.sin()) as f32,
                );
                Complex32::new(i_, g * (q * ph.cos() + i_ * ph.sin()))
            })
            .collect();
        let mut c = IqCorrection::default();
        let y = run(&mut c, &x, 7777);
        let tail = &y[n - 65536..];
        let amp = |f: f64| {
            let s: Complex32 = tail
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let p = -std::f64::consts::TAU * f * i as f64;
                    *v * Complex32::new(p.cos() as f32, p.sin() as f32)
                })
                .sum();
            s.norm() / tail.len() as f32
        };
        let image = 20.0 * (amp(-0.0123) / amp(0.0123)).log10();
        assert!(image < -40.0, "image at {image} dB");
        let (ge, pe) = c.imbalance();
        assert!(
            ge.abs() > 0.3 && pe.abs() > 2.0,
            "measured {ge} dB {pe} deg"
        );
    }

    #[test]
    fn the_blanker_blanks_clicks_and_counts_events() {
        let mut x = tone(100_000, 0.01, 0.1, Complex32::default());
        for k in [20_000usize, 50_000, 50_010] {
            x[k] = Complex32::new(5.0, 0.0);
        }
        let mut nb = NoiseBlanker::new(10.0, 50e-6).unwrap();
        let y = run(&mut nb, &x, 999);
        assert_eq!(y[20_000], Complex32::default());
        assert_eq!(y[20_049], Complex32::default());
        assert_ne!(y[20_051], Complex32::default());
        assert_eq!(nb.events(), 2, "the second click stretched the blank");
        assert_eq!(y[50_059], Complex32::default());
    }
}
