//! Carrier recovery: a symbol-rate phase-locked loop generic over its phase detector, with an optional frequency-locked loop to pull
//! in, gear shifting and a lock test; and [`ToneLoop`], a sample-rate PLL on a pilot or carrier tone.
//!
//! # Symbol-rate carrier loop
//!
//! [`CarrierSync`] turns each symbol back by its oscillator's phase, gets the phase error from the detector, and steers the
//! oscillator through a second-order loop built by `LoopFilter::new(bn_t, zeta)` (`bn_t` per symbol). The oscillator is a
//! `PhaseAcc` in `f64` cycles, its phasor from `approx::sin_cos`: no library trigonometry per symbol.
//!
//! | Detector | Error (radians, about the phase error when small) | Use |
//! |---|---|---|
//! | [`Costas<2>`](Costas) | `Im{y} sgn Re{y} / |y|` | BPSK |
//! | [`Costas<4>`](Costas) | `Im{y d*} / |y||d|`, `d = sgn Re + j sgn Im` | QPSK, OQPSK after timing |
//! | [`Costas<8>`](Costas) | the same with the 8PSK decision by sectors (comparisons, no angle) | 8PSK |
//! | [`DecisionDirected`] | `Im{y d*} / |d|^2`, `d` the nearest point of any [`Constellation`] | QAM, APSK, any map |
//! | [`MlTanh`] | `Im{y} tanh(2 Re{y} / N0)` | BPSK below 0 dB Es/N0 (the maximum-likelihood detector) |
//!
//! [`CarrierSync::push_known`] updates from a known symbol (a pilot or a preamble), whatever the detector: pilot-aided operation
//! is `push_known` on the pilots and [`CarrierSync::push`] (decision-directed) or [`CarrierSync::push_free`] (no update) between
//! them. The optional **frequency-locked loop** compares the `M`-th power of consecutive symbols (removing the modulation) and
//! moves the frequency integrator while the phase loop is unlocked; it pulls in offsets up to `1 / (2M)` of the symbol rate that a
//! narrow phase loop alone would never reach. **Gears** widen the loop until it locks and narrow it after. The frequency can be
//! set from outside (a Doppler hand-over or a feed-forward estimate from `crate::freq`). The lock test averages `cos(M phi)` (or
//! `cos phi` for decision-directed detectors).
//!
//! Detector gains assume unit-power symbols at a useful SNR (where the S-curve slope is 1); at low SNR the decision-directed
//! slope falls and the loop narrows accordingly.
//!
//! ```
//! use sdrkit_algo_modem::carrier::{CarrierConfig, CarrierSync, Costas};
//! use sdrkit_algo_modem::Complex32;
//! // QPSK symbols turned by 0.7 rad and drifting 0.002 cycles per symbol.
//! let mut cs = CarrierSync::new(Costas::<4>, CarrierConfig::new(0.02)).unwrap();
//! let s = std::f32::consts::FRAC_1_SQRT_2;
//! let mut last = Complex32::default();
//! for k in 0..3000 {
//!     let d = Complex32::new(if k % 3 == 0 { s } else { -s }, if k % 5 < 2 { s } else { -s });
//!     let rot = Complex32::from_polar(1.0, 0.7 + std::f32::consts::TAU * 0.002 * k as f32);
//!     last = cs.push(d * rot) - d;
//! }
//! assert!(last.norm() < 0.05 && cs.locked());
//! assert!((cs.freq() - 0.002).abs() < 1e-4);
//! ```

use crate::lock::{rare, Gears, LockDetector};
use crate::{Complex32, Constellation, ModemError, Param};
use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::iir::ComplexOnePole;
use sdrkit_algo_dsp::loopfilter::{LoopFilter, PhaseAcc};
use std::f64::consts::TAU;

/// A phase detector for [`CarrierSync`]: the error of a derotated symbol `y` (unit mean power), in radians for small errors.
pub trait PhaseDetector: Clone {
    /// The order of the phase ambiguity (2 for BPSK, 4 for QPSK and square QAM, 8 for 8PSK, 1 when there is none), which is also
    /// the power the frequency-locked loop raises symbols to.
    fn order(&self) -> u32;
    /// The phase error of `y`, radians.
    fn error(&mut self, y: Complex32) -> f32;
    /// A lock value of `y`: 1 when on a point, lower when turned (about `cos(order phi)`).
    fn lock_value(&self, y: Complex32) -> f32;
    /// The phase error and the lock value of `y`, as [`error`](Self::error) and [`lock_value`](Self::lock_value) return
    /// them. A detector whose two values share work gives its own form.
    #[inline]
    fn error_and_lock(&mut self, y: Complex32) -> (f32, f32) {
        (self.error(y), self.lock_value(y))
    }
}

#[inline]
fn sgn(x: f32) -> f32 {
    if x >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// `Re{y^m} / |y|^m`: about `cos(m phi)`.
#[inline]
fn cos_m(y: Complex32, m: u32) -> f32 {
    let mut w = y;
    let mut k = 1;
    while k < m {
        w = w * w;
        k *= 2;
    }
    let n = w.norm();
    if n > 0.0 {
        w.re / n
    } else {
        0.0
    }
}

/// The Costas detector for M-PSK, `M` = 2, 4 or 8, with sector decisions (module documentation). Other orders are refused by
/// [`CarrierSync::new`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Costas<const M: u32>;

impl<const M: u32> Costas<M> {
    /// The nearest M-PSK point (points at `2 pi k / M`, with `pi / M` offset for M = 4 as QPSK sends them at 45 degrees).
    #[inline]
    fn decide(y: Complex32) -> Complex32 {
        const S: f32 = std::f32::consts::FRAC_1_SQRT_2;
        match M {
            2 => Complex32::new(sgn(y.re), 0.0),
            4 => Complex32::new(sgn(y.re) * S, sgn(y.im) * S),
            _ => {
                // 8PSK on the axes and diagonals: by comparing |re| and |im| with tan(pi/8).
                const T: f32 = 0.414_213_57;
                let (a, b) = (y.re.abs(), y.im.abs());
                if b < a * T {
                    Complex32::new(sgn(y.re), 0.0)
                } else if a < b * T {
                    Complex32::new(0.0, sgn(y.im))
                } else {
                    Complex32::new(sgn(y.re) * S, sgn(y.im) * S)
                }
            }
        }
    }
}

impl<const M: u32> PhaseDetector for Costas<M> {
    fn order(&self) -> u32 {
        M
    }
    #[inline]
    fn error(&mut self, y: Complex32) -> f32 {
        let d = Self::decide(y);
        let n = y.norm();
        if n > 0.0 {
            (y * d.conj()).im / n
        } else {
            0.0
        }
    }
    #[inline]
    fn lock_value(&self, y: Complex32) -> f32 {
        // QPSK points sit at 45 degrees, where y^4 is -1.
        if M == 4 {
            -cos_m(y, 4)
        } else {
            cos_m(y, M)
        }
    }
}

/// The decision-directed detector on any constellation: `Im{y d*} / |d|^2` with `d` the nearest point.
#[derive(Clone, Debug)]
pub struct DecisionDirected {
    c: Constellation,
    order: u32,
}

impl DecisionDirected {
    /// A detector deciding on `c` (unit mean energy, as a timing loop's AGC outputs). The phase ambiguity is found from the
    /// constellation's symmetry (the largest of 1, 2, 4 or 8 quarter-turn-like rotations that map it onto itself).
    pub fn new(c: Constellation) -> Self {
        let order = [8u32, 4, 2]
            .into_iter()
            .find(|&m| {
                let r = Complex32::from_polar(1.0, std::f32::consts::TAU / m as f32);
                c.points()
                    .iter()
                    .all(|&p| c.points().iter().any(|&q| (p * r - q).norm() < 1e-3))
            })
            .unwrap_or(1);
        DecisionDirected { c, order }
    }
}

impl PhaseDetector for DecisionDirected {
    fn order(&self) -> u32 {
        self.order
    }
    #[inline]
    fn error(&mut self, y: Complex32) -> f32 {
        let d = self.c.nearest(y);
        let e = d.norm_sqr();
        if e > 0.0 {
            (y * d.conj()).im / e
        } else {
            0.0
        }
    }
    #[inline]
    fn lock_value(&self, y: Complex32) -> f32 {
        let d = self.c.nearest(y);
        let n = y.norm() * d.norm();
        if n > 0.0 {
            (y * d.conj()).re / n
        } else {
            0.0
        }
    }
}

/// The maximum-likelihood detector for BPSK at low SNR: `Im{y} tanh(2 Re{y} / N0)`. At high SNR `tanh` becomes the sign (the
/// Costas BPSK detector); at low SNR it weighs each symbol by how sure its decision is.
#[derive(Clone, Copy, Debug)]
pub struct MlTanh {
    k: f32,
}

impl MlTanh {
    /// For unit-power symbols with noise variance `n0` (`1 / (Es/N0)`). Errors unless `n0` is positive and finite.
    pub fn new(n0: f32) -> Result<Self, ModemError> {
        if !(n0 > 0.0 && n0.is_finite()) {
            return Param::NoiseVariance.positive(n0);
        }
        Ok(MlTanh { k: 2.0 / n0 })
    }
}

impl PhaseDetector for MlTanh {
    fn order(&self) -> u32 {
        2
    }
    #[inline]
    fn error(&mut self, y: Complex32) -> f32 {
        y.im * (self.k * y.re).tanh()
    }
    #[inline]
    fn lock_value(&self, y: Complex32) -> f32 {
        cos_m(y, 2)
    }
}

/// Settings of a [`CarrierSync`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarrierConfig {
    /// Phase-loop noise bandwidth per update (0 < bn_t < 0.5).
    pub bn_t: f64,
    /// Damping.
    pub zeta: f64,
    /// The largest frequency the loop may pull to, cycles per symbol.
    pub max_freq: f64,
    /// Symbols per detector update: 1 when every symbol updates the loop; the pilot spacing when only pilots do (the symbols
    /// between go through [`CarrierSync::push_free`]). `bn_t` is per update.
    pub update_spacing: f64,
    /// Pull-in and tracking bandwidths, switched by the lock test (overrides `bn_t`).
    pub gears: Option<Gears>,
    /// The frequency-locked loop's noise bandwidth per update, if one assists the pull-in.
    pub fll_bn_t: Option<f64>,
    /// Updates the lock test averages over.
    pub lock_updates: f32,
    /// Lock and unlock thresholds of the averaged lock value.
    pub lock_on: f32,
    /// See `lock_on`.
    pub lock_off: f32,
}

impl CarrierConfig {
    /// Defaults: the given bandwidth, damping 1/sqrt(2), frequency range +-0.1 cycles per symbol, an update every symbol, no gears,
    /// no frequency loop,
    /// lock averaged over 100 updates, on at 0.4, off below 0.2 (about the lock values at 6 dB Es/N0 for QPSK).
    pub fn new(bn_t: f64) -> Self {
        CarrierConfig {
            bn_t,
            zeta: std::f64::consts::FRAC_1_SQRT_2,
            max_freq: 0.1,
            update_spacing: 1.0,
            gears: None,
            fll_bn_t: None,
            lock_updates: 100.0,
            lock_on: 0.4,
            lock_off: 0.2,
        }
    }
}

/// The symbol-rate carrier loop (module documentation).
#[derive(Clone, Debug)]
pub struct CarrierSync<P: PhaseDetector> {
    det: P,
    cfg: CarrierConfig,
    lf: LoopFilter,
    nco: PhaseAcc,
    lock: LockDetector,
    fll_k: f64,
    last_m: Complex32,
    have_last: bool,
    last_error: f32,
    /// The oscillator of the next run, if the run before left it.
    osc: Option<RunOsc>,
}

/// The oscillator of [`CarrierSync::push_run`] as phasors, with the phase and the integrator that they stand for. The
/// error of a run turns the phasor that ran through the run by a small angle, and the next run takes it when the phase
/// and the integrator are still those of the loop: the step from the error of one run to the first symbol of the next
/// holds no sine of a whole phase. After [`OSC_RUNS`] runs the phasor is made from the phase again.
#[derive(Clone, Copy, Debug)]
struct RunOsc {
    phase: f64,
    integ: f64,
    /// `e^(-j 2 pi phase)`.
    rot: Complex32,
    /// `e^(-j 2 pi integ / update_spacing)`.
    step: Complex32,
    /// Runs since `rot` was made from the phase.
    runs: u32,
}

/// Runs that the phasor of a [`RunOsc`] is carried over: the rounding of its products stays under 1e-5.
const OSC_RUNS: u32 = 16;

/// The largest turn, radians, that [`small_turn`] takes.
const SMALL_TURN: f32 = 0.25;

/// `e^(-j x)` for `|x|` up to [`SMALL_TURN`], within 2e-8. The low and the high terms of each series are summed side by
/// side, so the result waits for four operations in a row, not seven.
#[inline]
fn small_turn(x: f32) -> Complex32 {
    let x2 = x * x;
    let x4 = x2 * x2;
    let c = (1.0 - 0.5 * x2) + x4 * (1.0 / 24.0 - x2 * (1.0 / 720.0));
    let s = x * ((1.0 - x2 * (1.0 / 6.0)) + x4 * (1.0 / 120.0));
    Complex32::new(c, -s)
}

impl<P: PhaseDetector> CarrierSync<P> {
    /// A loop with detector `det`. Errors for a bandwidth outside (0, 0.5), a Costas order other than 2, 4 or 8, a negative
    /// frequency range, or a bad lock setting.
    pub fn new(det: P, cfg: CarrierConfig) -> Result<Self, ModemError> {
        if !matches!(det.order(), 1 | 2 | 4 | 8) {
            return Param::Order.not_allowed(det.order() as f64);
        }
        if !(cfg.max_freq >= 0.0 && cfg.max_freq <= 0.5) {
            return Param::MaxFreq.range(0.0, 0.5, cfg.max_freq);
        }
        if !(cfg.update_spacing >= 1.0 && cfg.update_spacing <= 1e6) {
            return Param::UpdateSpacing.range(1.0, 1e6, cfg.update_spacing);
        }
        let bn = cfg.gears.map_or(cfg.bn_t, |g| g.pull_in_bn_t);
        let mut lf = LoopFilter::new(bn, cfg.zeta)?;
        if let Some(g) = cfg.gears {
            LoopFilter::new(g.track_bn_t, cfg.zeta)?;
        }
        lf.set_clamp(cfg.max_freq * cfg.update_spacing);
        let fll_k = match cfg.fll_bn_t {
            // A first-order loop of gain k has a noise bandwidth k / (4 - 2k) per update.
            Some(b) if b > 0.0 && b < 0.25 => 4.0 * b / (1.0 + 2.0 * b),
            Some(b) => return Param::LoopBandwidth.range(0.0, 0.25, b),
            None => 0.0,
        };
        Ok(CarrierSync {
            det,
            cfg,
            lf,
            nco: PhaseAcc::new(0.0),
            lock: LockDetector::new(cfg.lock_updates, cfg.lock_on, cfg.lock_off)?,
            fll_k,
            last_m: Complex32::default(),
            have_last: false,
            last_error: 0.0,
            osc: None,
        })
    }

    /// Back to phase 0, frequency 0, unlocked, pull-in gear.
    pub fn reset(&mut self) {
        self.lf.reset();
        if let Some(g) = self.cfg.gears {
            let _ = self.lf.set_bandwidth(g.pull_in_bn_t, self.cfg.zeta, 1.0);
        }
        self.nco = PhaseAcc::new(0.0);
        self.lock.reset();
        self.have_last = false;
        self.last_error = 0.0;
        self.osc = None;
    }

    /// The loop's frequency estimate, cycles per symbol.
    pub fn freq(&self) -> f64 {
        self.lf.integrator() / self.cfg.update_spacing
    }

    /// Sets the frequency (a Doppler hand-over or a feed-forward estimate), cycles per symbol; clamped to the range.
    pub fn set_freq(&mut self, f: f64) {
        if f.is_finite() {
            self.lf.set_integrator(
                f.clamp(-self.cfg.max_freq, self.cfg.max_freq) * self.cfg.update_spacing,
            );
        }
    }

    /// The oscillator phase, cycles in `[0, 1)`.
    pub fn phase(&self) -> f64 {
        self.nco.phase()
    }

    /// Turns the oscillator by `cycles` (resolving an ambiguity found by a sync word, for example a quarter turn).
    pub fn rotate(&mut self, cycles: f64) {
        self.nco.steer(cycles);
    }

    /// Whether the lock test passes.
    pub fn locked(&self) -> bool {
        self.lock.locked()
    }

    /// The averaged lock value.
    pub fn lock_metric(&self) -> f32 {
        self.lock.metric()
    }

    /// The last phase error, radians.
    pub fn last_error(&self) -> f32 {
        self.last_error
    }

    /// Moves the frequency estimate out of the loop, for a caller whose own oscillator removes the carrier before the loop (a GNSS
    /// correlator that wipes the carrier at the sample rate). Call it after a [`push`](Self::push) and add the result, cycles per
    /// symbol, to the outside oscillator before the next symbol. The loop frequency becomes 0, and the oscillator turns back by the
    /// share it applied for the next symbol, so no symbol is turned twice. The phase, the lock state and the gears stay.
    pub fn take_freq(&mut self) -> f64 {
        let f = self.freq();
        self.lf.set_integrator(0.0);
        self.nco.steer(-f);
        f
    }

    /// The detector.
    pub fn detector(&self) -> &P {
        &self.det
    }

    #[inline]
    fn update(&mut self, y: Complex32, e: f32, lock_value: f32) {
        let e = if e.is_finite() { e } else { 0.0 };
        self.last_error = e;
        let was = self.lock.locked();
        let now = self.lock.update(lock_value);
        if was != now {
            if let Some(g) = self.cfg.gears {
                let _ = self.lf.set_bandwidth(g.bn_t(now), self.cfg.zeta, 1.0);
            }
        }
        // Frequency-locked loop on the M-th power while unlocked.
        if self.fll_k > 0.0 {
            let mut w = y;
            let mut k = 1;
            while k < self.det.order() {
                w = w * w;
                k *= 2;
            }
            if self.have_last && !now {
                let c = w * self.last_m.conj();
                let fe = approx::atan2(c.im, c.re) as f64 / (TAU * self.det.order() as f64);
                let f = (self.lf.integrator() + self.fll_k * fe).clamp(
                    -self.cfg.max_freq * self.cfg.update_spacing,
                    self.cfg.max_freq * self.cfg.update_spacing,
                );
                self.lf.set_integrator(f);
            }
            self.last_m = w;
            self.have_last = true;
        }
        let v = self.lf.update(e as f64 / TAU);
        let i = self.lf.integrator();
        // The proportional part now, and one symbol's share of the frequency.
        self.nco.steer(v - i + i / self.cfg.update_spacing);
    }

    /// Derotates one symbol, updates the loop from the detector, and returns the derotated symbol.
    #[inline]
    pub fn push(&mut self, z: Complex32) -> Complex32 {
        let y = self.nco.derotate(z);
        let (e, l) = self.det.error_and_lock(y);
        self.update(y, e, l);
        y
    }

    /// Derotates a run of symbols in place and updates the loop once, from the sum of their detector errors. The oscillator
    /// runs through the run at the frequency the loop has at its start, so the symbols do not wait for each other. The
    /// integrator and the proportional part are those of one step per symbol; the correction acts from the next run on, so
    /// the run length times the bandwidth should stay under about 0.1. A run of one symbol is [`push`](Self::push). The
    /// frequency-locked loop does not act in a longer run, and takes up again with the next single symbol.
    ///
    /// The phase and the frequency stay in `f64`. The phasor of the oscillator goes from run to run and is turned by the
    /// correction of each run; it is made from the phase again after 16 runs and after any other change of the phase
    /// or the frequency, and differs from the phasor of the phase by less than 1e-5.
    #[inline]
    pub fn push_run(&mut self, z: &mut [Complex32]) {
        if let [one] = z {
            *one = self.push(*one);
            return;
        }
        if z.is_empty() {
            return;
        }
        let spacing = self.cfg.update_spacing;
        let before = self.lf.integrator();
        let n = z.len() as f64;
        // A value per update as a value per symbol; the usual spacing of 1 costs no division.
        let per_symbol = |v: f64| if spacing == 1.0 { v } else { v / spacing };
        // The turn of one symbol, radians.
        let angle = per_symbol(TAU * before);
        let (mut rot, step, runs) = match self.osc {
            Some(o) if o.phase == self.nco.phase() && o.integ == before && o.runs < OSC_RUNS => {
                (o.rot, o.step, o.runs)
            }
            _ => {
                let (s, c) = approx::sin_cos(-angle);
                (self.nco.phasor().conj(), Complex32::new(c, s), 0)
            }
        };
        let was = self.lock.locked();
        // The lock test runs on a copy, so its mean stays in a register from symbol to symbol.
        let mut lock = self.lock.clone();
        let mut last = self.last_error;
        let mut sum = 0.0f32;
        for v in z.iter_mut() {
            let y = *v * rot;
            rot *= step;
            let (e, l) = self.det.error_and_lock(y);
            if e.is_finite() {
                sum += e;
                last = e;
            } else {
                rare();
            }
            lock.update(l);
            *v = y;
        }
        self.lock = lock;
        self.last_error = last;
        let now = self.lock.locked();
        if was != now {
            if let Some(g) = self.cfg.gears {
                let _ = self.lf.set_bandwidth(g.bn_t(now), self.cfg.zeta, 1.0);
            }
        }
        self.have_last = false;
        let v = self.lf.update(sum as f64 / TAU);
        let i = self.lf.integrator();
        // The proportional part now, the frequency of the run, and for the symbols after the first the share of the
        // integrator's change that a step per symbol gives them on average.
        self.nco
            .steer(v - i + per_symbol(n * before + 0.5 * (n + 1.0) * (i - before)));
        // The same step on the phasors: `rot` has run through the run, and the error turns it by a small angle; the
        // step is that of the new frequency. An integrator at its limit or a large angle leaves the next run to the
        // phase.
        let (kp, ki) = self.lf.gains();
        let turn = sum * (kp + per_symbol(0.5 * (n + 1.0) * ki)) as f32;
        let stepped = angle as f32 + sum * per_symbol(ki) as f32;
        let limit = self.cfg.max_freq * spacing;
        self.osc = (i.abs() < limit && turn.abs() <= SMALL_TURN && stepped.abs() <= SMALL_TURN).then(|| RunOsc {
            phase: self.nco.phase(),
            integ: i,
            rot: rot * small_turn(turn),
            step: small_turn(stepped),
            runs: runs + 1,
        });
    }

    /// Derotates one symbol whose sent value `known` is known (a pilot or preamble symbol), updates the loop from it, and returns
    /// the derotated symbol.
    #[inline]
    pub fn push_known(&mut self, z: Complex32, known: Complex32) -> Complex32 {
        let y = self.nco.derotate(z);
        let p = known.norm_sqr();
        let (e, l) = if p > 0.0 {
            let c = y * known.conj();
            let n = c.norm();
            (c.im / p, if n > 0.0 { c.re / n } else { 0.0 })
        } else {
            (0.0, 0.0)
        };
        self.update(y, e, l);
        y
    }

    /// Derotates one symbol without updating the loop (between pilots, or while the input is known to be noise); the oscillator runs
    /// on at the loop's frequency.
    #[inline]
    pub fn push_free(&mut self, z: Complex32) -> Complex32 {
        let y = self.nco.derotate(z);
        self.nco
            .steer(self.lf.integrator() / self.cfg.update_spacing);
        y
    }

    /// [`push`](Self::push) over a slice; `out` must be at least as long as `x`.
    pub fn process(&mut self, x: &[Complex32], out: &mut [Complex32]) -> Result<(), ModemError> {
        if out.len() < x.len() {
            return Err(ModemError::OutputTooShort { need: x.len() });
        }
        for (o, &z) in out.iter_mut().zip(x) {
            *o = self.push(z);
        }
        Ok(())
    }
}

/// A sample-rate phase-locked loop on a tone: the FM stereo pilot, an AM carrier for synchronous detection, C-QUAM, VOR and beacon
/// carriers, a TV vision carrier or colour burst.
///
/// Each input sample is mixed down by the loop's oscillator, smoothed by a one-pole low-pass (which, for a real input, removes the
/// image at twice the tone), and the phase of the result steers the oscillator through a second-order loop of noise bandwidth
/// `bn_hz`. The loop reports the tone's phase as `f64` cycles, its frequency, amplitude and a lock test (the averaged cosine of the
/// phase error, with the amplitude above a floor), and phasors of any harmonic for coherent demodulation (38 kHz from a 19 kHz
/// pilot, 57 kHz for RDS).
///
/// ```
/// use sdrkit_algo_modem::carrier::ToneLoop;
/// // A 19 kHz pilot 2 Hz off nominal at 192 kHz, 10 % of full scale.
/// let rate = 192_000.0;
/// let x: Vec<f32> = (0..96_000).map(|n| 0.1 * (std::f64::consts::TAU * 19_002.0 * n as f64 / rate + 0.4).cos() as f32).collect();
/// let mut pll = ToneLoop::new(rate, 19_000.0, 20.0, 10.0, 2_000.0).unwrap();
/// pll.process_real(&x);
/// assert!(pll.locked() && (pll.freq_hz() - 19_002.0).abs() < 0.5);
/// assert!((pll.amplitude() - 0.1).abs() < 0.01);
/// ```
#[derive(Clone, Debug)]
pub struct ToneLoop {
    rate: f64,
    nominal: f64,
    lf: LoopFilter,
    nco: PhaseAcc,
    lp: ComplexOnePole,
    lock: LockDetector,
    amp: f32,
    min_amp: f32,
    /// Pull-in and tracking noise bandwidths, Hz, switched by the lock test.
    gears: Option<(f64, f64)>,
    bn_hz: f64,
}

impl ToneLoop {
    /// A loop at `rate` samples per second for a tone near `freq_hz`, noise bandwidth `bn_hz`, pulling up to `max_offset_hz` away,
    /// with the low-pass before the detector at `lowpass_hz` (a few times `bn_hz`, well below the tone for a real input). Errors for
    /// bad rates or bandwidths.
    pub fn new(
        rate: f64,
        freq_hz: f64,
        bn_hz: f64,
        max_offset_hz: f64,
        lowpass_hz: f64,
    ) -> Result<Self, ModemError> {
        Param::Rate.check_positive(rate)?;
        if !(freq_hz.is_finite() && freq_hz.abs() < rate / 2.0) {
            return Param::Frequency.range(-rate / 2.0, rate / 2.0, freq_hz);
        }
        if !(max_offset_hz >= 0.0 && max_offset_hz.is_finite()) {
            return Param::OffsetRange.at_least(0.0, max_offset_hz);
        }
        let mut lf = LoopFilter::new(bn_hz / rate, std::f64::consts::FRAC_1_SQRT_2)?;
        lf.set_clamp(max_offset_hz / rate);
        let lp = ComplexOnePole::lowpass(rate, lowpass_hz)?;
        // Lock over about four loop time constants.
        let updates = (rate / bn_hz).max(1.0) as f32;
        Ok(ToneLoop {
            rate,
            nominal: freq_hz / rate,
            lf,
            nco: PhaseAcc::new(freq_hz / rate),
            lp,
            lock: LockDetector::new(updates, 0.8, 0.5)?,
            amp: 0.0,
            min_amp: 1e-6,
            gears: None,
            bn_hz,
        })
    }

    /// Two bandwidths switched by the lock test: `pull_in_hz` while unlocked, `track_hz` once locked (noise bandwidths in Hz, each
    /// positive and below a quarter of the rate). The loop starts again in the pull-in gear. An FM stereo pilot loop of damping
    /// 1/sqrt(2) pulls in with a natural frequency of 80 Hz and tracks with 20 Hz: noise bandwidths of 266.6 Hz and 66.6 Hz
    /// (`Bn = wn (zeta + 1 / (4 zeta)) / 2`). Errors for bandwidths out of range.
    ///
    /// ```
    /// use sdrkit_algo_modem::carrier::ToneLoop;
    /// let mut pll = ToneLoop::new(228_000.0, 19_000.0, 66.6, 10.0, 500.0).unwrap();
    /// pll.set_gears(266.6, 66.6).unwrap();
    /// assert_eq!(pll.bandwidth_hz(), 266.6);
    /// ```
    pub fn set_gears(&mut self, pull_in_hz: f64, track_hz: f64) -> Result<(), ModemError> {
        for b in [pull_in_hz, track_hz] {
            if !(b > 0.0 && b < 0.25 * self.rate) {
                return Param::LoopBandwidth.range(0.0, 0.25 * self.rate, b);
            }
            LoopFilter::new(b / self.rate, std::f64::consts::FRAC_1_SQRT_2)?;
        }
        self.gears = Some((pull_in_hz, track_hz));
        self.lock.reset();
        self.set_bn(pull_in_hz);
        Ok(())
    }

    /// The noise bandwidth in use, Hz.
    pub fn bandwidth_hz(&self) -> f64 {
        self.bn_hz
    }

    fn set_bn(&mut self, bn_hz: f64) {
        // `set_gears` refused bandwidths out of range.
        if self
            .lf
            .set_bandwidth(bn_hz / self.rate, std::f64::consts::FRAC_1_SQRT_2, 1.0)
            .is_ok()
        {
            self.bn_hz = bn_hz;
        }
    }

    /// The smallest amplitude the lock test accepts (default 1e-6).
    pub fn set_min_amplitude(&mut self, a: f32) {
        self.min_amp = a.max(0.0);
    }

    /// Forgets the tone.
    pub fn reset(&mut self) {
        self.lf.reset();
        self.nco = PhaseAcc::new(self.nominal);
        self.lp.reset();
        self.lock.reset();
        self.amp = 0.0;
        if let Some((pull_in, _)) = self.gears {
            self.set_bn(pull_in);
        }
    }

    #[inline]
    fn step(&mut self, x: Complex32, real: bool) -> Complex32 {
        let ph = self.nco.phasor();
        let b = self.lp.process_sample(
            if x.re.is_finite() && x.im.is_finite() {
                x
            } else {
                Complex32::default()
            } * ph.conj(),
        );
        let n = b.norm();
        let (e, c) = if n > 0.0 {
            (b.im / n, b.re / n)
        } else {
            (0.0, 0.0)
        };
        // A real tone A cos has amplitude A/2 after mixing.
        self.amp = if real { 2.0 * n } else { n };
        let was = self.lock.locked();
        let now = self
            .lock
            .update(if self.amp >= self.min_amp { c } else { 0.0 });
        if was != now {
            if let Some((pull_in, track)) = self.gears {
                self.set_bn(if now { track } else { pull_in });
            }
        }
        let v = self.lf.update(e as f64 / TAU);
        self.nco.set_freq(self.nominal + self.lf.integrator());
        self.nco.steer(v - self.lf.integrator());
        self.nco.advance();
        ph
    }

    /// Takes one real sample; returns the loop's phasor `e^(j phase)` at that sample.
    #[inline]
    pub fn push_real(&mut self, x: f32) -> Complex32 {
        self.step(Complex32::new(x, 0.0), true)
    }

    /// Takes one complex sample; returns the loop's phasor at that sample.
    #[inline]
    pub fn push(&mut self, x: Complex32) -> Complex32 {
        self.step(x, false)
    }

    /// Runs over real samples.
    pub fn process_real(&mut self, x: &[f32]) {
        for &v in x {
            self.push_real(v);
        }
    }

    /// Runs over complex samples.
    pub fn process(&mut self, x: &[Complex32]) {
        for &v in x {
            self.push(v);
        }
    }

    /// The phasor of harmonic `k` of the tone at the next sample (`k = 2` regenerates the 38 kHz stereo carrier from a 19 kHz pilot).
    #[inline]
    pub fn harmonic(&self, k: u32) -> Complex32 {
        let (s, c) = approx::sin_cos(TAU * ((self.nco.phase() * k as f64) % 1.0));
        Complex32::new(c, s)
    }

    /// The oscillator phase, cycles.
    pub fn phase(&self) -> f64 {
        self.nco.phase()
    }

    /// The tracked frequency, Hz.
    pub fn freq_hz(&self) -> f64 {
        (self.nominal + self.lf.integrator()) * self.rate
    }

    /// The tone's amplitude (a real `A cos` reads `A`).
    pub fn amplitude(&self) -> f32 {
        self.amp
    }

    /// Whether locked.
    pub fn locked(&self) -> bool {
        self.lock.locked()
    }

    /// The averaged lock value (cosine of the phase error).
    pub fn lock_metric(&self) -> f32 {
        self.lock.metric()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
    use sdrkit_algo_sim::rng::Rng;

    fn symbols(set: &PointSet, n: usize, seed: u64) -> Vec<Complex32> {
        let mut r = Rng::new(seed);
        (0..n)
            .map(|_| set.points()[r.below(set.len() as u64) as usize])
            .collect()
    }

    /// Runs a loop on `d` turned by `phi0 + 2 pi f k` plus noise of variance `n0`; returns the residual phase errors after `skip`.
    fn run<P: PhaseDetector>(
        cs: &mut CarrierSync<P>,
        d: &[Complex32],
        phi0: f32,
        f: f32,
        n0: f64,
        seed: u64,
        skip: usize,
    ) -> Vec<f32> {
        let mut r = Rng::new(seed);
        let mut errs = Vec::new();
        for (k, &s) in d.iter().enumerate() {
            let rot = Complex32::from_polar(
                1.0,
                phi0 + std::f32::consts::TAU * ((f as f64 * k as f64) % 1.0) as f32,
            );
            let y = cs.push(s * rot + r.complex_normal(n0));
            if k >= skip {
                // Phase of the output against the sent symbol, modulo the ambiguity.
                let c = y * s.conj();
                errs.push(c.im.atan2(c.re));
            }
        }
        errs
    }

    #[test]
    fn a_step_per_run_follows_a_step_per_symbol() {
        // QPSK at Es/N0 13 dB, 0.3 rad and 0.002 cycles per symbol off, through a loop of 0.005 per symbol.
        let set = PointSet::qpsk();
        let d = symbols(&set, 8000, 3);
        let mut r = Rng::new(9);
        let x: Vec<Complex32> = d
            .iter()
            .enumerate()
            .map(|(k, &s)| {
                let turn = 0.3 + std::f32::consts::TAU * ((0.002 * k as f64) % 1.0) as f32;
                s * Complex32::from_polar(1.0, turn) + r.complex_normal(0.05)
            })
            .collect();
        let mut one = CarrierSync::new(Costas::<4>, CarrierConfig::new(0.005)).unwrap();
        let by_symbol: Vec<Complex32> = x.iter().map(|&v| one.push(v)).collect();
        for run in [1, 4, 8, 20] {
            let mut cs = CarrierSync::new(Costas::<4>, CarrierConfig::new(0.005)).unwrap();
            let mut by_run = x.clone();
            for part in by_run.chunks_mut(run) {
                cs.push_run(part);
            }
            assert!((cs.freq() - 0.002).abs() < 1e-4, "run {run}: {}", cs.freq());
            assert_eq!(cs.locked(), one.locked(), "run {run}");
            // After the pull-in the symbols differ by the phase noise of two loops: 30 dB or more under the symbols.
            let diff: f32 = by_run[2000..]
                .iter()
                .zip(&by_symbol[2000..])
                .map(|(a, b)| (a - b).norm_sqr())
                .sum::<f32>()
                / 6000.0;
            assert!(diff < 1e-3, "run {run}: {diff}");
            if run == 1 {
                assert_eq!(by_run, by_symbol);
            }
        }
    }

    #[test]
    fn the_oscillator_carried_over_runs_stays_at_the_phase_of_the_loop() {
        // QPSK at Es/N0 13 dB with a carrier of 0.003 cycles per symbol, in runs of 4 and of 8 symbols.
        let set = PointSet::qpsk();
        let d = symbols(&set, 8000, 5);
        let mut r = Rng::new(11);
        for run in [4, 8] {
            let mut cs = CarrierSync::new(Costas::<4>, CarrierConfig::new(0.01)).unwrap();
            let (mut carried, mut worst) = (0, 0.0f32);
            for (k, part) in d.chunks(run).enumerate() {
                let mut z: Vec<Complex32> = part
                    .iter()
                    .enumerate()
                    .map(|(i, &s)| {
                        let turn = std::f32::consts::TAU * ((0.003 * (k * run + i) as f64) % 1.0) as f32;
                        s * Complex32::from_polar(1.0, turn) + r.complex_normal(0.05)
                    })
                    .collect();
                cs.push_run(&mut z);
                if let Some(o) = cs.osc {
                    carried += 1;
                    let (s, c) = approx::sin_cos(-TAU * cs.lf.integrator());
                    worst = worst
                        .max((o.rot - cs.nco.phasor().conj()).norm())
                        .max((o.step - Complex32::new(c, s)).norm());
                }
            }
            assert!(carried > 8000 / run - 50, "run {run}: {carried}");
            assert!(worst < 1e-5, "run {run}: {worst}");
        }
    }

    #[test]
    fn every_detector_locks_its_constellation() {
        let cases: Vec<(&str, PointSet)> = vec![
            ("bpsk", PointSet::bpsk()),
            ("qpsk", PointSet::qpsk()),
            ("8psk", PointSet::psk(8, 0.0).unwrap()),
            ("16qam", PointSet::qam(16, QamMap::DVB).unwrap()),
            ("16apsk", PointSet::dvbs2_16apsk(2.85).unwrap()),
        ];
        for (name, set) in cases {
            let d = symbols(&set, 6000, 1);
            let cfg = CarrierConfig::new(0.01);
            let errs = match name {
                "bpsk" => run(
                    &mut CarrierSync::new(Costas::<2>, cfg).unwrap(),
                    &d,
                    0.3,
                    0.001,
                    0.01,
                    2,
                    3000,
                ),
                "qpsk" => run(
                    &mut CarrierSync::new(Costas::<4>, cfg).unwrap(),
                    &d,
                    0.3,
                    0.001,
                    0.01,
                    2,
                    3000,
                ),
                "8psk" => run(
                    &mut CarrierSync::new(Costas::<8>, cfg).unwrap(),
                    &d,
                    0.2,
                    0.001,
                    0.003,
                    2,
                    3000,
                ),
                _ => {
                    let det = DecisionDirected::new(Constellation::new(set.clone()).unwrap());
                    run(
                        &mut CarrierSync::new(det, cfg).unwrap(),
                        &d,
                        0.15,
                        0.0005,
                        0.002,
                        2,
                        3000,
                    )
                }
            };
            let rms = (errs.iter().map(|e| e * e).sum::<f32>() / errs.len() as f32).sqrt();
            assert!(rms < 0.08, "{name}: rms phase error {rms}");
        }
        assert_eq!(
            DecisionDirected::new(
                Constellation::new(PointSet::qam(64, QamMap::DVB).unwrap()).unwrap()
            )
            .order(),
            4
        );
        assert_eq!(
            DecisionDirected::new(Constellation::new(PointSet::psk(8, 0.1).unwrap()).unwrap())
                .order(),
            8
        );
        assert!(CarrierSync::new(Costas::<3>, CarrierConfig::new(0.01)).is_err());
    }

    /// The phase jitter of a PLL is `2 Bn T var(e)` for a detector error of variance `var(e)` per symbol (Gardner, Phaselock
    /// Techniques, eq. 4.20); for QPSK at high SNR the Costas error variance is the noise on one axis, `N0 / 2`.
    #[test]
    fn phase_jitter_matches_theory() {
        let set = PointSet::qpsk();
        let d = symbols(&set, 60_000, 3);
        let n0 = 0.02; // 17 dB Es/N0
        for bn in [0.005, 0.02] {
            let mut cs = CarrierSync::new(Costas::<4>, CarrierConfig::new(bn)).unwrap();
            let errs = run(&mut cs, &d, 0.0, 0.0, n0, 4, 2000);
            // The output phase error includes the symbol's own noise (variance N0/2 rad^2): subtract it.
            let total = errs.iter().map(|e| (e * e) as f64).sum::<f64>() / errs.len() as f64;
            let jitter = total - n0 / 2.0;
            let want = 2.0 * bn * n0 / 2.0;
            assert!(
                jitter > want / 2.0 && jitter < want * 2.0,
                "bn {bn}: {jitter:.2e} vs {want:.2e}"
            );
        }
    }

    #[test]
    fn fll_pulls_in_what_the_pll_alone_cannot_and_gears_shift() {
        let set = PointSet::qpsk();
        let d = symbols(&set, 8000, 5);
        let f = 0.08; // 8 % of the symbol rate
        let mut plain = CarrierSync::new(Costas::<4>, CarrierConfig::new(0.001)).unwrap();
        run(&mut plain, &d, 0.0, f, 0.01, 6, 0);
        assert!(
            (plain.freq() - f as f64).abs() > 0.005,
            "a narrow loop alone should not reach {f}: {}",
            plain.freq()
        );
        let mut cfg = CarrierConfig::new(0.002);
        cfg.fll_bn_t = Some(0.01);
        cfg.gears = Some(Gears {
            pull_in_bn_t: 0.01,
            track_bn_t: 0.002,
        });
        let mut cs = CarrierSync::new(Costas::<4>, cfg).unwrap();
        let errs = run(&mut cs, &d, 0.0, f, 0.01, 6, 6000);
        assert!((cs.freq() - f as f64).abs() < 2e-4, "{}", cs.freq());
        assert!(
            cs.locked(),
            "lock {} rms {}",
            cs.lock_metric(),
            (errs.iter().map(|e| e * e).sum::<f32>() / errs.len() as f32).sqrt()
        );
        let rms = (errs.iter().map(|e| e * e).sum::<f32>() / errs.len() as f32).sqrt();
        assert!(rms < 0.1, "{rms}");
    }

    #[test]
    fn take_freq_moves_the_frequency_to_an_outside_oscillator() {
        // BPSK at 0.03 cycles per symbol with an FLL. One loop keeps its frequency. The other passes it to an outside oscillator
        // after every symbol. The two loops are the same loop, so their outputs agree.
        let set = PointSet::bpsk();
        let d = symbols(&set, 4000, 11);
        let mut cfg = CarrierConfig::new(0.01);
        cfg.fll_bn_t = Some(0.02);
        cfg.max_freq = 0.25;
        let mut plain = CarrierSync::new(Costas::<2>, cfg).unwrap();
        let mut moved = CarrierSync::new(Costas::<2>, cfg).unwrap();
        let (f, phase0) = (0.03f64, 0.2f64);
        let (mut outside_phase, mut outside_freq) = (0.0f64, 0.0f64);
        let mut worst = 0f32;
        for (k, &s) in d.iter().enumerate() {
            let carrier = phase0 + f * k as f64;
            let z = s * Complex32::from_polar(1.0, (TAU * carrier) as f32);
            let a = plain.push(z);
            let back = Complex32::from_polar(1.0, (-TAU * outside_phase) as f32);
            let b = moved.push(z * back);
            outside_freq += moved.take_freq();
            outside_phase += outside_freq;
            worst = worst.max((a - b).norm());
        }
        assert!(worst < 1e-3, "{worst}");
        assert!(
            (outside_freq - f).abs() < 1e-3 && moved.freq() == 0.0,
            "{outside_freq}"
        );
        assert!(moved.locked() && plain.locked());
    }

    #[test]
    fn ml_tanh_and_pilots() {
        // BPSK at 0 dB Es/N0: the ML detector still locks.
        let set = PointSet::bpsk();
        let d = symbols(&set, 20_000, 7);
        let mut cs =
            CarrierSync::new(MlTanh::new(1.0).unwrap(), CarrierConfig::new(0.002)).unwrap();
        let errs = run(&mut cs, &d, 0.5, 0.0, 1.0, 8, 10_000);
        let mean = errs.iter().map(|e| e.cos()).sum::<f32>() / errs.len() as f32;
        assert!(mean > 0.4, "{mean}");
        // Pilot-aided 32APSK: a known symbol every 16, the rest free-running.
        let set = PointSet::dvbs2_32apsk(2.84, 5.27).unwrap();
        let d = symbols(&set, 16_000, 9);
        let mut pcfg = CarrierConfig::new(0.05);
        pcfg.update_spacing = 16.0;
        let mut cs = CarrierSync::new(Costas::<4>, pcfg).unwrap();
        let mut r = Rng::new(10);
        let mut worst = 0f32;
        for (k, &s) in d.iter().enumerate() {
            let z =
                s * Complex32::from_polar(1.0, 1.0 + 0.0002 * k as f32) + r.complex_normal(1e-5);
            let y = if k % 16 == 0 {
                cs.push_known(z, s)
            } else {
                cs.push_free(z)
            };
            if k > 8000 {
                let c = y * s.conj();
                worst = worst.max(c.im.atan2(c.re).abs());
            }
        }
        assert!(worst < 0.15, "{worst}");
        assert!(MlTanh::new(0.0).is_err());
    }

    #[test]
    fn tone_loop_tracks_real_and_complex_tones_and_regenerates_harmonics() {
        let rate = 48_000.0;
        let f = 1_000.0;
        let x: Vec<Complex32> = (0..48_000)
            .map(|n| Complex32::from_polar(0.5, (TAU * (f + 7.0) * n as f64 / rate + 1.0) as f32))
            .collect();
        let mut p = ToneLoop::new(rate, f, 20.0, 50.0, 200.0).unwrap();
        p.process(&x);
        assert!(
            p.locked() && (p.freq_hz() - 1007.0).abs() < 0.2,
            "{}",
            p.freq_hz()
        );
        assert!((p.amplitude() - 0.5).abs() < 0.02);
        // The next sample's phase: the second harmonic phasor is the square of the fundamental.
        let h1 = p.harmonic(1);
        let h2 = p.harmonic(2);
        assert!((h1 * h1 - h2).norm() < 1e-5);
        let want = (TAU * (f + 7.0) * 48_000.0 / rate + 1.0) / TAU;
        let d = (p.phase() - want).rem_euclid(1.0);
        assert!(d.min(1.0 - d) < 0.01, "{}", p.phase());
        assert!(ToneLoop::new(rate, 30_000.0, 20.0, 50.0, 200.0).is_err());
        // No tone: never locks.
        let mut q = ToneLoop::new(rate, f, 20.0, 50.0, 200.0).unwrap();
        q.set_min_amplitude(0.01);
        let mut r = Rng::new(1);
        let noise: Vec<f32> = (0..48_000).map(|_| 0.001 * r.normal_f32()).collect();
        q.process_real(&noise);
        assert!(!q.locked());
    }

    #[test]
    fn tone_loop_gears_pull_in_wide_and_track_narrow() {
        // The FM stereo pilot at 228 kHz, 9 Hz off, with noise: the 80 Hz / 20 Hz natural frequencies of a damping 1/sqrt(2) loop
        // are noise bandwidths of 266.6 Hz and 66.6 Hz.
        let rate = 228_000.0;
        let mut r = Rng::new(5);
        let x: Vec<f32> = (0..228_000)
            .map(|n| {
                (0.1 * (TAU * 19_009.0 * n as f64 / rate + 2.0).cos()) as f32
                    + 0.05 * r.normal_f32()
            })
            .collect();
        let lock_time = |gears: bool| -> (usize, f64) {
            let mut p = ToneLoop::new(rate, 19_000.0, 66.6, 10.0, 500.0).unwrap();
            if gears {
                p.set_gears(266.6, 66.6).unwrap();
            }
            let mut at = usize::MAX;
            for (n, &v) in x.iter().enumerate() {
                p.push_real(v);
                if at == usize::MAX && p.locked() {
                    at = n;
                }
            }
            assert!(p.locked());
            assert!((p.freq_hz() - 19_009.0).abs() < 0.5, "{}", p.freq_hz());
            (at, p.bandwidth_hz())
        };
        let (slow, bn_slow) = lock_time(false);
        let (fast, bn_fast) = lock_time(true);
        assert!(fast < slow, "with gears {fast}, without {slow}");
        assert_eq!((bn_slow, bn_fast), (66.6, 66.6));
        let mut p = ToneLoop::new(rate, 19_000.0, 66.6, 10.0, 500.0).unwrap();
        p.set_gears(266.6, 66.6).unwrap();
        p.process_real(&x[..50_000]);
        p.reset();
        assert_eq!(p.bandwidth_hz(), 266.6);
        assert!(p.set_gears(0.0, 66.6).is_err());
        assert!(p.set_gears(266.6, 1e6).is_err());
    }
}
