//! Levels: AGC, the power meter and the squelch.

use super::Every;
use crate::component::values_of;
use crate::component::{Applied, Component, ComponentInfo, Io, Port};
use crate::graph::GraphError;
use crate::stream::{Format, Kind, TagKind};
use sdrkit_algo_dsp::agc::Agc;
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_algo_dsp::meter::PowerMeter;
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::{field, keys};
use sdrkit_core::Values;
use sdrkit_core::{
    Complex32, ConfigError, Published, Results, Setting, SettingError, SettingId, SettingSet, Value,
};

const TARGET: Setting<field::F64> = Setting::new("target", "Target", "", 0.5).range(0.01, 1.0, 0.0);
const ATTACK: Setting<field::F64> =
    Setting::new("attack", "Attack", "s", 0.005).range(1e-5, 10.0, 0.0);
const DECAY: Setting<field::F64> = Setting::new("decay", "Decay", "s", 0.5).range(1e-4, 60.0, 0.0);
const MAX_GAIN: Setting<field::F64> =
    Setting::new("max_gain", "Maximum gain", "dB", 60.0).range(0.0, 120.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`AgcComponent`].
    pub enum AgcSetting {
        /// The output level.
        Target = TARGET,
        /// The attack time.
        Attack = ATTACK,
        /// The decay time.
        Decay = DECAY,
        /// The largest gain.
        MaxGain = MAX_GAIN,
    }
}

const METER_TAU: Setting<field::F64> =
    Setting::new("tau", "Averaging", "s", 0.1).range(1e-4, 60.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`PowerMeterSink`].
    pub enum PowerMeterSetting {
        /// The averaging time.
        Tau = METER_TAU,
    }
}

/// The result set of [`PowerMeterSink`]. No field has a value before the first interval ends.
#[derive(Clone, Copy, Debug, PartialEq, Fields)]
pub struct PowerMeterFields {
    /// The mean level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The peak level of the interval.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak_dbfs: Option<f64>,
}

sdrkit_core::choice_enum! {
    /// What a squelch measures.
    pub enum SquelchMode {
        /// The power of the signal.
        Level = ("level", "Level"),
        /// The noise above the voice band.
        Noise = ("noise", "Noise"),
    }
}

const MODE: Setting<field::Choice<SquelchMode>> =
    Setting::choice("mode", "Squelch", SquelchMode::Level);
const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold", "Threshold", "dB", -50.0).range(-200.0, 50.0, 0.0);
const HYSTERESIS: Setting<field::F64> =
    Setting::new("hysteresis", "Hysteresis", "dB", 3.0).range(0.0, 30.0, 0.0);
const SQUELCH_TAU: Setting<field::F64> =
    Setting::new("tau", "Averaging", "s", 0.02).range(1e-4, 10.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`Squelch`].
    pub enum SquelchSetting {
        /// What it measures.
        Mode = MODE,
        /// Where it opens.
        Threshold = THRESHOLD,
        /// The gap between opening and closing.
        Hysteresis = HYSTERESIS,
        /// The averaging time.
        Tau = SQUELCH_TAU,
    }
}

/// The result set of [`Squelch`]. No field has a value before the first interval ends.
#[derive(Clone, Copy, Debug, PartialEq, Fields)]
pub struct SquelchFields {
    /// The squelch passes the signal.
    #[field(label = "Open")]
    pub open: Option<bool>,
    /// The level that the squelch compares with its threshold.
    #[field(label = "Level", unit = "dB")]
    pub level_db: Option<f64>,
}

/// Automatic gain control (`dsp.agc`) on `Iq` or `Real` (sdrkit-algo-dsp `Agc`): the envelope follows `|x|` with a fast attack and a slow
/// decay, and the gain is `target / envelope`, capped at `max_gain`.
///
/// Settings (live): `target` (0.01 to 1), `attack` and `decay` (seconds), `max_gain` (dB).
pub struct AgcComponent {
    target: f64,
    attack: f64,
    decay: f64,
    max_gain_db: f64,
    agc: Option<Agc>,
}

impl Default for AgcComponent {
    fn default() -> Self {
        AgcComponent {
            target: TARGET.default,
            attack: ATTACK.default,
            decay: DECAY.default,
            max_gain_db: MAX_GAIN.default,
            agc: None,
        }
    }
}

impl AgcComponent {
    fn max_gain(&self) -> f32 {
        10f64.powf(self.max_gain_db / 20.0) as f32
    }
}

impl Component for AgcComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.agc",
            vec![Port::one_of("in", &[Kind::Iq, Kind::Real])],
            vec![Port::same_as("out", 0).same_format_as(0)],
        )
        .in_place(0, 0)
        .with_settings(AgcSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([
            (AgcSetting::Target, Value::F64(self.target)),
            (AgcSetting::Attack, Value::F64(self.attack)),
            (AgcSetting::Decay, Value::F64(self.decay)),
            (AgcSetting::MaxGain, Value::F64(self.max_gain_db)),
        ])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let rate = inputs[0].rate;
        if rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        self.agc = Some(Agc::new(
            rate,
            self.target as f32,
            self.attack,
            self.decay,
            self.max_gain(),
        ));
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(agc) = &mut self.agc else { return };
        if io.input_format(0).kind == Kind::Iq {
            agc.process(io.pass_through::<Complex32>(0, 0));
        } else {
            agc.process_real(io.pass_through::<f32>(0, 0));
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AgcSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AgcSetting::Target => self.target = TARGET.read(id, v)?,
            AgcSetting::Attack => self.attack = ATTACK.read(id, v)?,
            AgcSetting::Decay => self.decay = DECAY.read(id, v)?,
            AgcSetting::MaxGain => self.max_gain_db = MAX_GAIN.read(id, v)?,
        }
        let mg = self.max_gain();
        if let Some(a) = &mut self.agc {
            a.set_target(self.target as f32);
            a.set_times(self.attack, self.decay);
            a.set_max_gain(mg);
        }
        Ok(Applied::Live)
    }
}

/// Power meter (`dsp.power_meter`), a sink for `Iq` or `Real` (sdrkit-algo-dsp `PowerMeter`): an exponential power average.
///
/// Settings: `tau` (seconds). Results, every tenth of a second of signal, at its exact sample: `status.level_dbfs`, fields
/// `level_dbfs` and `peak_dbfs` (the highest in that tenth of a second).
pub struct PowerMeterSink {
    tau: f64,
    meter: Option<PowerMeter>,
    every: Every,
    results: Published,
}

fn meter_results() -> Published {
    Published::new(PowerMeterFields::SHAPE.fields)
}

fn squelch_results() -> Published {
    Published::new(SquelchFields::SHAPE.fields)
}

impl Default for PowerMeterSink {
    fn default() -> Self {
        PowerMeterSink {
            tau: METER_TAU.default,
            meter: None,
            every: Every::default(),
            results: meter_results(),
        }
    }
}

impl PowerMeterSink {
    /// Measures `n` samples, which `update` feeds to the meter from index `a` to `b`, and publishes at the end of each
    /// interval at its sample.
    fn measure(&mut self, n: usize, mut update: impl FnMut(&mut PowerMeter, usize, usize)) {
        let Some(m) = &mut self.meter else { return };
        let mut done = 0;
        while done < n {
            let k = self.every.left().min(n - done);
            update(m, done, done + k);
            done += k;
            if self.every.tick(k) {
                let level = m.dbfs();
                let peak = m.take_peak_dbfs();
                self.results.results_mut().update_status(|s| {
                    let c = s.level_dbfs != Some(level as f32);
                    s.level_dbfs = Some(level as f32);
                    c
                });
                self.results.put(&PowerMeterFields {
                    level_dbfs: Some(level),
                    peak_dbfs: Some(peak),
                });
            }
        }
    }
    /// The level now, in dBFS.
    pub fn dbfs(&self) -> Option<f64> {
        self.meter.as_ref().map(|m| m.dbfs())
    }
}

impl Component for PowerMeterSink {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "dsp.power_meter",
            vec![Port::one_of("in", &[Kind::Iq, Kind::Real])],
        )
        .with_settings(PowerMeterSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(PowerMeterSetting::Tau, Value::F64(self.tau))])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let rate = inputs[0].rate;
        if rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        self.meter = Some(PowerMeter::new(rate, self.tau));
        self.every = Every::new(rate, 0.1);
        self.results = Published::from_decl(std::sync::Arc::clone(self.results.results().decl()));
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        if io.input_format(0).kind == Kind::Iq {
            let x = io.iq(0);
            self.measure(x.len(), |m, a, b| m.update(&x[a..b]));
        } else {
            let x = io.real(0);
            self.measure(x.len(), |m, a, b| m.update_real(&x[a..b]));
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PowerMeterSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PowerMeterSetting::Tau => self.tau = METER_TAU.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

/// Squelch (`dsp.squelch`) on `Iq` or `Real`: passes the signal while it is open and outputs silence (zeros) while it is closed, with
/// hysteresis, sample by sample.
///
/// - `level`: open while the averaged power is at least `threshold` dBFS.
/// - `noise` (`Real` only, for a discriminator's output): open while the power above the voice band (a high-pass at 0.35 of the
///   rate, at most 5 kHz) is at most `threshold` dB. Noise rises as the signal fades, so this closes on a weak carrier.
///
/// A `Lock` tag marks each change: locked when it opens. Settings (live): `mode`, `threshold` (dB), `hysteresis`
/// (dB), `tau` (seconds). Results: `open`, `level_db`.
pub struct Squelch {
    mode: SquelchMode,
    threshold: f64,
    hysteresis: f64,
    tau: f64,
    rate: f64,
    alpha: f64,
    power: f64,
    open: bool,
    hp: Option<Biquad>,
    every: Every,
    results: Published,
}

impl Default for Squelch {
    fn default() -> Self {
        Squelch {
            mode: MODE.default,
            threshold: THRESHOLD.default,
            hysteresis: HYSTERESIS.default,
            tau: SQUELCH_TAU.default,
            rate: 0.0,
            alpha: 0.0,
            power: 0.0,
            open: false,
            hp: None,
            every: Every::default(),
            results: squelch_results(),
        }
    }
}

impl Squelch {
    fn of(mode: SquelchMode, threshold: f64) -> Result<Self, SettingError> {
        SquelchSetting::Threshold.check(&Value::F64(threshold))?;
        Ok(Squelch {
            mode,
            threshold,
            ..Self::default()
        })
    }
    /// A level squelch opening at `threshold_dbfs`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `threshold` setting refuses `threshold_dbfs`.
    pub fn level(threshold_dbfs: f64) -> Result<Self, SettingError> {
        Self::of(SquelchMode::Level, threshold_dbfs)
    }
    /// A noise squelch opening while the noise is at most `threshold_db`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `threshold` setting refuses `threshold_db`.
    pub fn noise(threshold_db: f64) -> Result<Self, SettingError> {
        Self::of(SquelchMode::Noise, threshold_db)
    }
    /// Whether it is open now.
    pub fn is_open(&self) -> bool {
        self.open
    }
    fn decide(&mut self) -> bool {
        let db = 10.0 * self.power.max(1e-30).log10();
        let (thr, h) = (self.threshold, self.hysteresis);
        let next = if self.mode == SquelchMode::Noise {
            if self.open {
                db <= thr + h
            } else {
                db <= thr
            }
        } else if self.open {
            db >= thr - h
        } else {
            db >= thr
        };
        let changed = next != self.open;
        self.open = next;
        changed
    }
}

impl Component for Squelch {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.squelch",
            vec![Port::one_of("in", &[Kind::Iq, Kind::Real])],
            vec![Port::same_as("out", 0).same_format_as(0)],
        )
        .in_place(0, 0)
        .with_settings(SquelchSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([
            (
                SquelchSetting::Mode,
                Value::Choice(sdrkit_core::Choice::index(self.mode)),
            ),
            (SquelchSetting::Threshold, Value::F64(self.threshold)),
            (SquelchSetting::Hysteresis, Value::F64(self.hysteresis)),
            (SquelchSetting::Tau, Value::F64(self.tau)),
        ])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        if self.mode == SquelchMode::Noise && f.kind != Kind::Real {
            return Err(GraphError::input_kind(0, f.kind));
        }
        self.rate = f.rate;
        self.alpha = 1.0 - (-1.0 / (self.tau * f.rate)).exp();
        self.power = 0.0;
        self.open = false;
        self.hp = (f.kind == Kind::Real).then(|| {
            Biquad::new(BiquadCoeffs::highpass(
                f.rate,
                (0.35 * f.rate).min(5000.0),
                0.707,
            ))
        });
        self.every = Every::new(f.rate, 0.1);
        self.results = squelch_results();
        Ok(vec![f.clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let iq = io.input_format(0).kind == Kind::Iq;
        let noise = self.mode == SquelchMode::Noise;
        let mut changes: [(usize, bool); 8] = [(0, false); 8];
        let mut nchanges = 0;
        let n;
        if iq {
            let buf = io.pass_through::<Complex32>(0, 0);
            n = buf.len();
            for (i, x) in buf.iter_mut().enumerate() {
                self.power += self.alpha * (x.norm_sqr() as f64 - self.power);
                if self.decide() && nchanges < changes.len() {
                    changes[nchanges] = (i, self.open);
                    nchanges += 1;
                }
                if !self.open {
                    *x = Complex32::default();
                }
            }
        } else {
            let buf = io.pass_through::<f32>(0, 0);
            n = buf.len();
            for (i, x) in buf.iter_mut().enumerate() {
                let p = if noise {
                    let y = self.hp.as_mut().map_or(*x, |h| h.process_sample(*x));
                    2.0 * (y as f64) * (y as f64)
                } else {
                    2.0 * (*x as f64) * (*x as f64)
                };
                self.power += self.alpha * (p - self.power);
                if self.decide() && nchanges < changes.len() {
                    changes[nchanges] = (i, self.open);
                    nchanges += 1;
                }
                if !self.open {
                    *x = 0.0;
                }
            }
        }
        let out = io.output(0);
        let start = out.len() - n;
        for &(i, open) in &changes[..nchanges] {
            out.tag_here(start + i, TagKind::Lock { locked: open });
        }
        if self.every.tick(n) {
            self.results.put(&SquelchFields {
                open: Some(self.open),
                level_db: Some(10.0 * self.power.max(1e-30).log10()),
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SquelchSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SquelchSetting::Mode => {
                let m = MODE.read(id, v)?;
                if m == SquelchMode::Noise && self.rate > 0.0 && self.hp.is_none() {
                    return Err(SettingError::Unsupported { id });
                }
                self.mode = m;
            }
            SquelchSetting::Threshold => self.threshold = THRESHOLD.read(id, v)?,
            SquelchSetting::Hysteresis => self.hysteresis = HYSTERESIS.read(id, v)?,
            SquelchSetting::Tau => {
                self.tau = SQUELCH_TAU.read(id, v)?;
                if self.rate > 0.0 {
                    self.alpha = 1.0 - (-1.0 / (self.tau * self.rate)).exp();
                }
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

    #[test]
    fn a_level_squelch_opens_on_a_carrier_and_tags_the_change() {
        let mut s = Squelch::level(-30.0).unwrap();
        let mut h = Harness::new(&mut s, vec![Format::iq(10e3, 0.0, 0.0)]);
        let mut x = vec![Complex32::new(1e-3, 0.0); 2000];
        x.extend(vec![Complex32::new(0.5, 0.0); 2000]);
        h.run(&mut s, |ins| {
            if let Data::Iq(v) = &mut ins[0].data {
                v.extend_from_slice(&x)
            }
        });
        let Data::Iq(y) = &h.outs[0].data else {
            panic!()
        };
        assert_eq!(y[1000], Complex32::default());
        assert_eq!(y[3999], Complex32::new(0.5, 0.0));
        assert!(s.is_open());
        let t = &h.outs[0].tags()[0];
        assert!(t.at >= 2000 && t.at < 2100, "opened at {}", t.at);
    }

    #[test]
    fn the_agc_brings_a_weak_signal_to_its_target() {
        let mut a = AgcComponent::default();
        let mut h = Harness::new(&mut a, vec![Format::new(Kind::Real, 48e3)]);
        let x: Vec<f32> = (0..48_000).map(|i| 0.01 * (i as f32 * 0.1).sin()).collect();
        h.run(&mut a, |ins| {
            if let Data::Real(v) = &mut ins[0].data {
                v.extend_from_slice(&x)
            }
        });
        let Data::Real(y) = &h.outs[0].data else {
            panic!()
        };
        let peak = y[40_000..].iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.3 && peak < 0.8, "peak {peak}");
    }

    #[test]
    fn the_power_meter_reads_a_tone() {
        let mut m = PowerMeterSink::default();
        let mut h = Harness::new(&mut m, vec![Format::iq(10e3, 0.0, 0.0)]);
        h.run(&mut m, |ins| {
            if let Data::Iq(v) = &mut ins[0].data {
                v.extend((0..20_000).map(|i| Complex32::from_polar(0.1, i as f32 * 0.3)))
            }
        });
        assert!((m.dbfs().unwrap() + 20.0).abs() < 0.1);
        let level = m
            .results()
            .unwrap()
            .fields()
            .get(&keys::LEVEL_DBFS)
            .unwrap();
        assert!((level + 20.0).abs() < 0.1);
    }
}
