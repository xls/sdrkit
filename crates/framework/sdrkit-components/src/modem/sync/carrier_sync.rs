//! `modem.carrier_sync`: carrier phase and frequency recovery on symbols.

use super::points::{ConstellationName, CONSTELLATION};
use super::{is_retune, points, split_at_tags};
use sdrkit_algo_modem::carrier::{
    CarrierConfig, CarrierSync, Costas, DecisionDirected, MlTanh, PhaseDetector,
};
use sdrkit_algo_modem::lock::Gears;
use sdrkit_core::publish::field;
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{Complex32, ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Every, Format, Io, Port, TagCarry, TagKind};

sdrkit_core::choice_enum! {
    /// The phase detector of [`CarrierSyncComponent`].
    pub enum CarrierDetector {
        /// Costas.
        Costas = ("costas", "Costas"),
        /// Decision directed.
        Dd = ("dd", "Decision directed"),
        /// Maximum likelihood (BPSK).
        MlTanh = ("ml_tanh", "Maximum likelihood (BPSK)"),
        /// Known pilots.
        Pilot = ("pilot", "Known pilots"),
    }
}

/// One loop behind a per-call interface.
trait CarrierCore: Send {
    /// Derotates `x` into `out`, appending `(index in out, locked)` at each change of the lock test to `locks`.
    fn run(&mut self, x: &[Complex32], out: &mut Vec<Complex32>, locks: &mut Vec<(usize, bool)>);
    fn reset(&mut self);
    fn freq(&self) -> f64;
    fn locked(&self) -> bool;
}

impl<P: PhaseDetector + Send> CarrierCore for CarrierSync<P> {
    fn run(&mut self, x: &[Complex32], out: &mut Vec<Complex32>, locks: &mut Vec<(usize, bool)>) {
        for &z in x {
            let was = self.locked();
            out.push(self.push(z));
            if self.locked() != was {
                locks.push((out.len() - 1, !was));
            }
        }
    }
    fn reset(&mut self) {
        CarrierSync::reset(self)
    }
    fn freq(&self) -> f64 {
        CarrierSync::freq(self)
    }
    fn locked(&self) -> bool {
        CarrierSync::locked(self)
    }
}

/// Pilot-aided operation: every `spacing`-th symbol (counted from the start or the last retune, the first one a pilot) is the
/// known `pilot`; the symbols between run free at the loop's frequency.
struct Pilots {
    sync: CarrierSync<DecisionDirected>,
    spacing: u64,
    pilot: Complex32,
    k: u64,
}

impl CarrierCore for Pilots {
    fn run(&mut self, x: &[Complex32], out: &mut Vec<Complex32>, locks: &mut Vec<(usize, bool)>) {
        for &z in x {
            let was = self.sync.locked();
            let y = if self.k.is_multiple_of(self.spacing) {
                self.sync.push_known(z, self.pilot)
            } else {
                self.sync.push_free(z)
            };
            self.k += 1;
            out.push(y);
            if self.sync.locked() != was {
                locks.push((out.len() - 1, !was));
            }
        }
    }
    fn reset(&mut self) {
        self.sync.reset();
        self.k = 0;
    }
    fn freq(&self) -> f64 {
        self.sync.freq()
    }
    fn locked(&self) -> bool {
        self.sync.locked()
    }
}

fn boxed<P: PhaseDetector + Send + 'static>(
    det: P,
    cfg: CarrierConfig,
) -> Result<Box<dyn CarrierCore>, ConfigError> {
    Ok(Box::new(CarrierSync::new(det, cfg)?))
}

const DETECTOR: Setting<field::Choice<CarrierDetector>> =
    Setting::choice("detector", "Phase detector", CarrierDetector::Costas);
const ORDER: Setting<field::I64> = Setting::new("order", "Costas order", "", 4)
    .range(2.0, 8.0, 0.0)
    .help("2, 4 or 8.");
const BANDWIDTH: Setting<field::F64> =
    Setting::new("bandwidth", "Loop bandwidth", "", 0.02).range(1e-5, 0.25, 0.0);
const DAMPING: Setting<field::F64> =
    Setting::new("damping", "Damping", "", std::f64::consts::FRAC_1_SQRT_2).range(0.1, 4.0, 0.0);
const FLL: Setting<field::F64> =
    Setting::new("fll", "Frequency loop bandwidth", "", 0.0).range(0.0, 0.2, 0.0);
const GEARS: Setting<field::F64> =
    Setting::new("gears", "Tracking bandwidth", "", 0.0).range(0.0, 0.25, 0.0);
const MAX_OFFSET_HZ: Setting<field::Hz> =
    Setting::new("max_offset_hz", "Frequency range", "Hz", 0.0)
        .range(0.0, 1e9, 0.0)
        .help("The largest offset the loop pulls to; 0 for a tenth of the symbol rate.");
const NOISE_VAR: Setting<field::F64> =
    Setting::new("noise_var", "Noise variance", "", 0.5).range(1e-4, 100.0, 0.0);
const PILOT_SPACING: Setting<field::I64> =
    Setting::new("pilot_spacing", "Pilot spacing", "symbols", 16).range(1.0, 1e6, 1.0);
const PILOT_PHASE: Setting<field::F64> =
    Setting::new("pilot_phase", "Pilot phase", "deg", 45.0).range(-360.0, 360.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`CarrierSyncComponent`].
    pub enum CarrierSyncComponentSetting {
        /// Phase detector.
        Detector = DETECTOR,
        /// Costas order.
        Order = ORDER,
        /// Constellation.
        Constellation = CONSTELLATION,
        /// Loop bandwidth.
        Bandwidth = BANDWIDTH,
        /// Damping.
        Damping = DAMPING,
        /// Frequency loop bandwidth.
        Fll = FLL,
        /// Tracking bandwidth.
        Gears = GEARS,
        /// Frequency range.
        MaxOffsetHz = MAX_OFFSET_HZ,
        /// Noise variance.
        NoiseVar = NOISE_VAR,
        /// Pilot spacing.
        PilotSpacing = PILOT_SPACING,
        /// Pilot phase.
        PilotPhase = PILOT_PHASE,
    }
}

/// Carrier recovery (`modem.carrier_sync`): `Symbols` in, derotated `Symbols` out at the same rate.
///
/// Wraps `sdrkit_algo_modem::carrier::CarrierSync`: each symbol is turned back by the loop's oscillator, the detector produces the phase
/// error, and a second-order loop of noise bandwidth `bandwidth` (per update) and `damping` steers the oscillator. The symbols
/// should be at unit mean power (as `modem.symbol_sync` outputs them).
///
/// Settings (each rebuilds the loop):
///
/// - `detector`: `costas` (M-PSK with `order` 2, 4 or 8), `dd` (decision directed on `constellation`), `ml_tanh` (BPSK at low SNR,
///   weighted by `noise_var`, the complex noise variance of unit-power symbols), `pilot` (a known symbol at `pilot_phase` degrees,
///   unit magnitude, every `pilot_spacing` symbols from the start or the last retune; the loop runs free between pilots).
/// - `bandwidth`, `damping`; `max_offset_hz` (the frequency range, at most half the symbol rate; 0 for a tenth of it).
/// - `fll`: the noise bandwidth per update of the M-th power frequency-locked loop that assists the pull-in (0 for none).
/// - `gears`: the tracking bandwidth after lock (0 for none; `bandwidth` is then the pull-in bandwidth).
///
/// The loop leaves the ambiguity of the constellation's symmetry (a quarter turn for QPSK): resolve it with a known preamble or
/// with differential coding (`modem.slicer`'s `differential`). Tags: a `Retuned` tag resets the loop at its symbol; `Lock {
/// locked }` marks each change of the lock test. Results: `offset_hz` (the loop's frequency), `locked`.
pub struct CarrierSyncComponent {
    detector: CarrierDetector,
    order: u32,
    constellation: ConstellationName,
    bandwidth: f64,
    damping: f64,
    fll: f64,
    gears: f64,
    max_offset_hz: f64,
    noise_var: f64,
    pilot_spacing: u64,
    pilot_phase: f64,
    rate: f64,
    core: Option<Box<dyn CarrierCore>>,
    locks: Vec<(usize, bool)>,
    carry: TagCarry,
    every: Every,
    results: Published,
}

impl Default for CarrierSyncComponent {
    /// Costas for QPSK, bandwidth 0.02, damping 0.707, no frequency loop, no gears, a range of a tenth of the symbol rate.
    fn default() -> Self {
        CarrierSyncComponent {
            detector: CarrierDetector::Costas,
            order: ORDER.default as _,
            constellation: ConstellationName::Qpsk,
            bandwidth: BANDWIDTH.default,
            damping: std::f64::consts::FRAC_1_SQRT_2,
            fll: FLL.default,
            gears: GEARS.default,
            max_offset_hz: MAX_OFFSET_HZ.default,
            noise_var: NOISE_VAR.default,
            pilot_spacing: PILOT_SPACING.default as _,
            pilot_phase: PILOT_PHASE.default,
            rate: 0.0,
            core: None,
            locks: Vec::new(),
            carry: TagCarry::same_rate(),
            every: Every::default(),
            results: Published::new(&[&keys::OFFSET_HZ, &keys::LOCKED]),
        }
    }
}

impl CarrierSyncComponent {
    /// A Costas loop for M-PSK (`order` 2, 4 or 8) of noise bandwidth `bandwidth` per symbol.
    pub fn costas(order: u32, bandwidth: f64) -> Self {
        CarrierSyncComponent {
            order,
            bandwidth,
            ..Self::default()
        }
    }
    /// The loop's frequency, Hz (0 before configure).
    pub fn offset_hz(&self) -> f64 {
        self.core.as_ref().map_or(0.0, |c| c.freq() * self.rate)
    }
    /// Whether the lock test passes.
    pub fn locked(&self) -> bool {
        self.core.as_ref().is_some_and(|c| c.locked())
    }
    fn build(&self) -> Result<Box<dyn CarrierCore>, ConfigError> {
        let mut cfg = CarrierConfig::new(self.bandwidth);
        cfg.zeta = self.damping;
        // 0 keeps the crate's range, a tenth of the symbol rate; a setting may open it to half.
        if self.max_offset_hz > 0.0 {
            cfg.max_freq = (self.max_offset_hz / self.rate).min(0.5);
        }
        if self.fll > 0.0 {
            cfg.fll_bn_t = Some(self.fll);
        }
        if self.gears > 0.0 {
            cfg.gears = Some(Gears {
                pull_in_bn_t: self.bandwidth,
                track_bn_t: self.gears,
            });
        }
        match self.detector {
            CarrierDetector::Costas => match self.order {
                2 => boxed(Costas::<2>, cfg),
                4 => boxed(Costas::<4>, cfg),
                8 => boxed(Costas::<8>, cfg),
                _ => Err(SettingError::NotPermitted {
                    id: CarrierSyncComponentSetting::Order.id(),
                }
                .into()),
            },
            CarrierDetector::Dd => boxed(
                DecisionDirected::new(points::constellation(self.constellation)?),
                cfg,
            ),
            CarrierDetector::MlTanh => boxed(
                MlTanh::new(self.noise_var as f32).map_err(ConfigError::from)?,
                cfg,
            ),
            CarrierDetector::Pilot => {
                cfg.update_spacing = self.pilot_spacing as f64;
                let det = DecisionDirected::new(points::constellation(self.constellation)?);
                let sync = CarrierSync::new(det, cfg).map_err(ConfigError::from)?;
                let ph = self.pilot_phase.to_radians() as f32;
                Ok(Box::new(Pilots {
                    sync,
                    spacing: self.pilot_spacing,
                    pilot: Complex32::from_polar(1.0, ph),
                    k: 0,
                }))
            }
        }
    }
    fn publish(&mut self) {
        let (f, l) = (self.offset_hz(), self.locked());
        self.results.set(&keys::OFFSET_HZ, super::r2(f));
        self.results.set(&keys::LOCKED, l);
    }
}

impl Component for CarrierSyncComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.carrier_sync",
            vec![Port::symbols("in")],
            vec![Port::symbols("out")],
        )
        .manual_tags()
        .with_settings(CarrierSyncComponentSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        super::need_rate(f.rate)?;
        self.rate = f.rate;
        self.core = Some(self.build()?);
        self.carry = TagCarry::same_rate();
        self.locks = Vec::with_capacity(16);
        self.every = Every::new(f.rate, 0.1);
        self.publish();
        Ok(vec![f.clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.samples::<Complex32>(0);
        let input = io.input(0);
        if self.core.is_none() {
            return;
        }
        let o0 = io.output(0).first();
        let out = io.samples_out::<Complex32>(0);
        let start = out.len();
        self.locks.clear();
        let mut st = (&mut *self, out);
        split_at_tags(
            &mut st,
            input,
            x.len(),
            |(me, out), a, b| {
                if let Some(c) = &mut me.core {
                    c.run(&x[a..b], out, &mut me.locks);
                }
            },
            |(me, out), t| {
                if is_retune(t) {
                    if let Some(c) = &mut me.core {
                        if c.locked() {
                            me.locks.push((out.len(), false));
                        }
                        c.reset();
                    }
                }
                me.carry
                    .queue(o0 + (out.len() - start) as u64, t.kind.clone());
            },
        );
        for &(at, locked) in &self.locks {
            self.carry
                .queue(o0 + (at - start) as u64, TagKind::Lock { locked });
        }
        self.carry.flush(io.output(0));
        if self.every.tick(x.len()) {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match CarrierSyncComponentSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CarrierSyncComponentSetting::Detector => self.detector = DETECTOR.read(id, v)?,
            CarrierSyncComponentSetting::Order => {
                let o = ORDER.read(id, v)?;
                if !matches!(o, 2 | 4 | 8) {
                    return Err(SettingError::NotPermitted { id });
                }
                self.order = o as u32;
            }
            CarrierSyncComponentSetting::Constellation => {
                self.constellation = CONSTELLATION.read(id, v)?
            }
            CarrierSyncComponentSetting::Bandwidth => self.bandwidth = BANDWIDTH.read(id, v)?,
            CarrierSyncComponentSetting::Damping => self.damping = DAMPING.read(id, v)?,
            CarrierSyncComponentSetting::Fll => self.fll = FLL.read(id, v)?,
            CarrierSyncComponentSetting::Gears => self.gears = GEARS.read(id, v)?,
            CarrierSyncComponentSetting::MaxOffsetHz => {
                self.max_offset_hz = MAX_OFFSET_HZ.read(id, v)?
            }
            CarrierSyncComponentSetting::NoiseVar => self.noise_var = NOISE_VAR.read(id, v)?,
            CarrierSyncComponentSetting::PilotSpacing => {
                self.pilot_spacing = PILOT_SPACING.read(id, v)? as u64
            }
            CarrierSyncComponentSetting::PilotPhase => {
                self.pilot_phase = PILOT_PHASE.read(id, v)?
            }
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([
            (
                CarrierSyncComponentSetting::Detector,
                Value::Choice(self.detector.index()),
            ),
            (
                CarrierSyncComponentSetting::Order,
                Value::I64(i64::from(self.order)),
            ),
            (
                CarrierSyncComponentSetting::Constellation,
                Value::Choice(self.constellation.index()),
            ),
            (
                CarrierSyncComponentSetting::Bandwidth,
                Value::F64(self.bandwidth),
            ),
            (
                CarrierSyncComponentSetting::Damping,
                Value::F64(self.damping),
            ),
            (CarrierSyncComponentSetting::Fll, Value::F64(self.fll)),
            (CarrierSyncComponentSetting::Gears, Value::F64(self.gears)),
            (
                CarrierSyncComponentSetting::MaxOffsetHz,
                Value::Hz(self.max_offset_hz),
            ),
            (
                CarrierSyncComponentSetting::NoiseVar,
                Value::F64(self.noise_var),
            ),
            (
                CarrierSyncComponentSetting::PilotSpacing,
                Value::I64(self.pilot_spacing as i64),
            ),
            (
                CarrierSyncComponentSetting::PilotPhase,
                Value::F64(self.pilot_phase),
            ),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}
