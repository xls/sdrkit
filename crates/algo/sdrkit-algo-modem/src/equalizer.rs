//! Adaptive equalisers and least-squares channel estimation.
//!
//! [`AdaptiveEq`] is one adaptive transversal equaliser for every single-carrier stack that needs one:
//!
//! - **Spacing**: symbol spaced (`sps = 1`) or fractionally spaced (`sps = 2`, T/2, which also corrects the sampling phase).
//! - **Feedback section** (a decision-feedback equaliser, DFE) of any length, fed with decisions or with the known symbols while
//!   training. HF modems (HFDL) need it; DVB-C and FM multipath do not.
//! - **Updates**: LMS (`w += mu e x*`) or normalised LMS (`mu` divided by the power in the taps), with the error from
//!   - a known symbol (**training**, [`AdaptiveEq::step`] with `Some(symbol)`),
//!   - the constant-modulus criterion (**CMA**, blind: `e = y (R2 - |y|^2)`), with `R2 = E|a|^4 / E|a|^2` computed from the
//!     constellation (1.32 for 16-QAM, 1.38 for 64-QAM, 1 for PSK), not one radius for every order,
//!   - the nearest constellation point (**decision directed**, DD-LMS) on any [`Constellation`].
//! - **Mode switch**: blind CMA switches to DD once the decision error is small, and falls back when it grows (hysteresis).
//! - **Carrier**: decisions are taken on `y r`, where `r` is a rotation the caller's carrier loop sets ([`AdaptiveEq::set_rotation`]),
//!   and the error is turned back by `conj(r)`, so the equaliser can sit before a carrier loop.
//! - **Layout**: taps and history are split into real and imaginary arrays and the filter runs in fixed-width chunks with partial
//!   sums, so the inner loops vectorise. The window power for NLMS is a running sum, re-summed exactly every 4096 updates.
//! - A diverged equaliser (non-finite output, or an output power above a limit, 1e6 unless [`AdaptiveEq::set_divergence_limit`]
//!   lowers it) resets itself to the centre tap, outputs that tap's input for the symbol, and counts it ([`AdaptiveEq::resets`]).
//! - **Input normalisation** ([`AdaptiveEq::set_input_normalisation`]): the input divided by the square root of its running mean
//!   power, so the step size does not depend on the signal level (a constant-envelope FM channel, for example).
//!
//! [`EchoCanceller`] removes one long echo of an 8-VSB signal (a path up to 330 symbols before or 480 after the strongest) by a
//! recursion fitted on the field sync, before the usual equalisers; [`earliest_path`] and [`latest_path`] find such paths.
//!
//! [`RealDfe`] is a decision-feedback equaliser for real symbols whose taps come from a least-squares fit on known or decided
//! symbols, with normalised LMS tracking inside a run.
//!
//! [`LsChannel`] estimates a channel's impulse response by least squares from a known training sequence. The pseudo-inverse
//! `(A^H A)^-1 A^H` is computed once per sequence (in `f64`, Gaussian elimination with partial pivoting), so each estimate is one
//! small matrix-vector product. The estimate's error covariance is `sigma^2 (A^H A)^-1`; [`LsChannel::noise_gain`] returns its trace.
//! GSM, AIS and BLE sequence estimators take their channel model from it.
//!
//! ```
//! use sdrkit_algo_dsp::constellation::PointSet;
//! use sdrkit_algo_modem::equalizer::{AdaptiveEq, EqConfig};
//! use sdrkit_algo_modem::{Complex32, Constellation};
//! let c = Constellation::new(PointSet::qpsk()).unwrap();
//! let mut eq = AdaptiveEq::new(EqConfig::new(7, 1), c).unwrap();
//! // Train on a known symbol, then run blind.
//! let a = Complex32::new(0.7071, 0.7071);
//! let y = eq.step(&[a], Some(a)).unwrap();
//! assert!(y.re.is_finite());
//! let mut out = Vec::new();
//! eq.process(&[a, -a, a], &mut out);
//! assert_eq!(out.len(), 3);
//! ```

use crate::{Complex32, Constellation, ModemError, Param};
use num_complex::Complex64;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::linalg::{cholesky_solve, LeastSquares};

/// How the equaliser adapts when it is not given a known symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Blind: the constant-modulus algorithm.
    Cma,
    /// Decision directed: the error to the nearest constellation point.
    Dd,
    /// No adaptation (the taps are kept).
    Frozen,
}

/// The settings of an [`AdaptiveEq`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EqConfig {
    /// Feed-forward taps, in input samples (at least 1, at most 1024).
    pub taps: usize,
    /// The tap that starts at 1 (the others start at 0); below `taps`. The output lags the input by `taps - 1 - centre` samples.
    pub centre: usize,
    /// Input samples per symbol: 1 (symbol spaced) or 2 (fractionally spaced, T/2).
    pub sps: usize,
    /// Feedback taps, in symbols (0 for a linear equaliser, at most 256).
    pub feedback: usize,
    /// Step size, in `(0, 1]` for NLMS; small (about `1 / (taps * power)` or less) for LMS.
    pub mu: f32,
    /// Normalised LMS: `mu` divided by the power in the taps (feed-forward plus feedback).
    pub normalised: bool,
    /// The mode at the start and after [`AdaptiveEq::reset`].
    pub mode: Mode,
    /// When set, CMA switches to DD once the smoothed decision error `E|y r - a|^2` (for unit-energy symbols) falls below this,
    /// and DD falls back to CMA when it rises above four times this.
    pub dd_switch_mse: Option<f32>,
    /// The smoothing factor of the decision-error estimate, in `(0, 1]`.
    pub mse_alpha: f32,
}

impl EqConfig {
    /// `taps` feed-forward taps at `sps` samples per symbol, the centre tap in the middle, no feedback, LMS with `mu = 1e-3`, blind
    /// CMA handing over to DD below a decision error of 0.05, error smoothing 0.002.
    pub fn new(taps: usize, sps: usize) -> Self {
        EqConfig {
            taps,
            centre: taps / 2,
            sps,
            feedback: 0,
            mu: 1e-3,
            normalised: false,
            mode: Mode::Cma,
            dd_switch_mse: Some(0.05),
            mse_alpha: 0.002,
        }
    }
}

/// The constant-modulus radius `E|a|^4 / E|a|^2` of a constellation (all points equally likely).
pub fn cma_radius(c: &Constellation) -> f32 {
    c.cma_radius()
}

/// A complex dot product `sum (wr + j wi)(xr + j xi)` over split arrays, in chunks of 8 with partial sums.
#[inline]
fn cdot(wr: &[f32], wi: &[f32], xr: &[f32], xi: &[f32]) -> (f32, f32) {
    const L: usize = 8;
    let n = wr.len().min(wi.len()).min(xr.len()).min(xi.len());
    let (wr, wi, xr, xi) = (&wr[..n], &wi[..n], &xr[..n], &xi[..n]);
    let mut ar = [0f32; L];
    let mut ai = [0f32; L];
    let ch = n / L * L;
    for (((a, b), c), d) in wr[..ch]
        .as_chunks::<L>()
        .0
        .iter()
        .zip(wi[..ch].as_chunks::<L>().0)
        .zip(xr[..ch].as_chunks::<L>().0)
        .zip(xi[..ch].as_chunks::<L>().0)
    {
        for k in 0..L {
            ar[k] += a[k] * c[k] - b[k] * d[k];
            ai[k] += a[k] * d[k] + b[k] * c[k];
        }
    }
    let (mut re, mut im) = (ar.iter().sum::<f32>(), ai.iter().sum::<f32>());
    for k in ch..n {
        re += wr[k] * xr[k] - wi[k] * xi[k];
        im += wr[k] * xi[k] + wi[k] * xr[k];
    }
    (re, im)
}

/// `w += g e conj(x)` over split arrays.
#[inline]
fn cupdate(wr: &mut [f32], wi: &mut [f32], xr: &[f32], xi: &[f32], er: f32, ei: f32) {
    for (((a, b), &c), &d) in wr.iter_mut().zip(wi.iter_mut()).zip(xr).zip(xi) {
        *a += er * c + ei * d;
        *b += ei * c - er * d;
    }
}

/// A history of the newest `n` complex values, split into real and imaginary arrays stored twice, so the window is always one
/// contiguous slice (oldest first).
#[derive(Clone, Debug)]
struct SplitRing {
    re: Vec<f32>,
    im: Vec<f32>,
    pos: usize,
    n: usize,
}

impl SplitRing {
    fn new(n: usize) -> Self {
        SplitRing {
            re: vec![0.0; 2 * n],
            im: vec![0.0; 2 * n],
            pos: 0,
            n,
        }
    }

    /// Writes `z`; returns the value it replaced (the oldest).
    #[inline]
    fn push(&mut self, z: Complex32) -> Complex32 {
        if self.n == 0 {
            return Complex32::default();
        }
        let p = self.pos;
        let old = Complex32::new(self.re[p], self.im[p]);
        self.re[p] = z.re;
        self.re[p + self.n] = z.re;
        self.im[p] = z.im;
        self.im[p + self.n] = z.im;
        self.pos = if p + 1 == self.n { 0 } else { p + 1 };
        old
    }

    /// The window, oldest first.
    #[inline]
    fn window(&self) -> (&[f32], &[f32]) {
        let p = self.pos;
        (&self.re[p..p + self.n], &self.im[p..p + self.n])
    }

    fn clear(&mut self) {
        self.re.fill(0.0);
        self.im.fill(0.0);
        self.pos = 0;
    }

    fn power(&self) -> f32 {
        let (r, i) = self.window();
        r.iter().zip(i).map(|(a, b)| a * a + b * b).sum()
    }
}

/// An adaptive linear or decision-feedback equaliser (module documentation).
#[derive(Clone, Debug)]
pub struct AdaptiveEq {
    cfg: EqConfig,
    c: Constellation,
    r2: f32,
    wr: Vec<f32>,
    wi: Vec<f32>,
    br: Vec<f32>,
    bi: Vec<f32>,
    x: SplitRing,
    d: SplitRing,
    phase: usize,
    mode: Mode,
    mse: f32,
    rot: Complex32,
    pow_x: f32,
    pow_d: f32,
    since_resum: u32,
    resets: u64,
    last_decision: Complex32,
    norm_alpha: Option<f32>,
    in_power: f32,
    n_power: u32,
    limit: f32,
}

impl AdaptiveEq {
    /// An equaliser for symbols of `constellation` (its decisions and CMA radius). Errors for settings out of range.
    pub fn new(cfg: EqConfig, constellation: Constellation) -> Result<Self, ModemError> {
        Param::Taps.check_count(cfg.taps, 1, 1024)?;
        if cfg.centre >= cfg.taps {
            return Param::Centre.range(0.0, (cfg.taps - 1) as f64, cfg.centre as f64);
        }
        if !(cfg.sps == 1 || cfg.sps == 2) {
            return Param::Sps.not_allowed(cfg.sps as f64);
        }
        if cfg.feedback > 256 {
            return Param::Feedback.range(0.0, 256.0, cfg.feedback as f64);
        }
        Param::StepSize.check_positive(cfg.mu)?;
        if cfg.normalised && cfg.mu > 2.0 {
            return Param::StepSize.range(0.0, 2.0, cfg.mu);
        }
        if !(cfg.mse_alpha > 0.0 && cfg.mse_alpha <= 1.0) {
            return Param::Smoothing.range(0.0, 1.0, cfg.mse_alpha);
        }
        if let Some(t) = cfg.dd_switch_mse {
            if !(t > 0.0 && t.is_finite()) {
                return Param::SwitchLevel.positive(t);
            }
        }
        let r2 = cma_radius(&constellation);
        let mut eq = AdaptiveEq {
            cfg,
            c: constellation,
            r2,
            wr: vec![0.0; cfg.taps],
            wi: vec![0.0; cfg.taps],
            br: vec![0.0; cfg.feedback],
            bi: vec![0.0; cfg.feedback],
            x: SplitRing::new(cfg.taps),
            d: SplitRing::new(cfg.feedback),
            phase: 0,
            mode: cfg.mode,
            mse: 1.0,
            rot: Complex32::new(1.0, 0.0),
            pow_x: 0.0,
            pow_d: 0.0,
            since_resum: 0,
            resets: 0,
            last_decision: Complex32::default(),
            norm_alpha: None,
            in_power: 0.0,
            n_power: 0,
            limit: 1e6,
        };
        eq.reset();
        Ok(eq)
    }

    /// Back to the centre tap, empty history, the configured mode. The reset count is kept.
    pub fn reset(&mut self) {
        self.reset_taps();
        self.x.clear();
        self.d.clear();
        self.phase = 0;
        self.mode = self.cfg.mode;
        self.mse = 1.0;
        self.pow_x = 0.0;
        self.pow_d = 0.0;
        self.since_resum = 0;
        self.in_power = 0.0;
        self.n_power = 0;
    }

    /// Divides the input by the square root of its running mean power before it enters the taps: the mean of the first `1 / alpha`
    /// samples, then a one-pole average with the factor `alpha` (`None`: no normalisation, the default). The equaliser then sees unit
    /// power whatever the signal level, and its outputs come out near unit power. Errors unless `alpha` is in `(0, 1]`.
    pub fn set_input_normalisation(&mut self, alpha: Option<f32>) -> Result<(), ModemError> {
        if let Some(a) = alpha {
            if !(a > 0.0 && a <= 1.0) {
                return Param::Smoothing.range(0.0, 1.0, a);
            }
        }
        self.norm_alpha = alpha;
        Ok(())
    }

    /// The running mean input power the normalisation uses (0 without normalisation).
    pub fn input_power(&self) -> f32 {
        self.in_power
    }

    /// The output power above which the equaliser counts as diverged and resets to the centre tap (default 1e6). With normalised
    /// input, a few times the constellation's peak power (16 for a constant-envelope signal) catches a divergence early. Errors
    /// unless the limit is positive and finite.
    pub fn set_divergence_limit(&mut self, power: f32) -> Result<(), ModemError> {
        if !(power > 0.0 && power.is_finite()) {
            return Param::Power.positive(power);
        }
        self.limit = power;
        Ok(())
    }

    fn reset_taps(&mut self) {
        self.wr.fill(0.0);
        self.wi.fill(0.0);
        self.br.fill(0.0);
        self.bi.fill(0.0);
        self.wr[self.cfg.centre] = 1.0;
    }

    /// The settings.
    pub fn config(&self) -> &EqConfig {
        &self.cfg
    }

    /// The constant-modulus radius in use.
    pub fn cma_radius(&self) -> f32 {
        self.r2
    }

    /// The adaptation mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Sets the adaptation mode.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// Sets the step size (a gear change, for example a larger `mu` while training). Errors when not positive and finite.
    pub fn set_mu(&mut self, mu: f32) -> Result<(), ModemError> {
        Param::StepSize.check_positive(mu)?;
        if self.cfg.normalised && mu > 2.0 {
            return Param::StepSize.range(0.0, 2.0, mu);
        }
        self.cfg.mu = mu;
        Ok(())
    }

    /// Sets the rotation `r` applied before decisions (the carrier loop's phasor; unit magnitude).
    pub fn set_rotation(&mut self, r: Complex32) {
        let n = r.norm();
        if n > 0.0 && n.is_finite() {
            self.rot = r / n;
        }
    }

    /// The smoothed decision error `E|y r - a|^2`.
    pub fn mse(&self) -> f32 {
        self.mse
    }

    /// How many times the equaliser reset itself after diverging.
    pub fn resets(&self) -> u64 {
        self.resets
    }

    /// The decision for the last output (the nearest point to `y r`, or the known symbol while training).
    pub fn last_decision(&self) -> Complex32 {
        self.last_decision
    }

    /// The feed-forward taps, oldest input first, into `out` (at least `taps` long).
    pub fn feed_forward(&self, out: &mut [Complex32]) -> Result<(), ModemError> {
        if out.len() < self.cfg.taps {
            return Err(ModemError::OutputTooShort {
                need: self.cfg.taps,
            });
        }
        for (k, o) in out.iter_mut().enumerate().take(self.cfg.taps) {
            *o = Complex32::new(self.wr[k], self.wi[k]);
        }
        Ok(())
    }

    /// The feedback taps, the most recent decision first, into `out` (at least `feedback` long).
    pub fn feedback(&self, out: &mut [Complex32]) -> Result<(), ModemError> {
        let n = self.cfg.feedback;
        if out.len() < n {
            return Err(ModemError::OutputTooShort { need: n });
        }
        // Stored oldest first.
        for (k, o) in out.iter_mut().enumerate().take(n) {
            *o = Complex32::new(self.br[n - 1 - k], self.bi[n - 1 - k]);
        }
        Ok(())
    }

    /// Sets the feed-forward taps (oldest input first) from `w` (exactly `taps` long).
    pub fn set_feed_forward(&mut self, w: &[Complex32]) -> Result<(), ModemError> {
        if w.len() != self.cfg.taps {
            return Param::Taps.length(self.cfg.taps, w.len());
        }
        if w.iter().any(|z| !(z.re.is_finite() && z.im.is_finite())) {
            return Param::Taps.not_finite();
        }
        for (k, z) in w.iter().enumerate() {
            self.wr[k] = z.re;
            self.wi[k] = z.im;
        }
        Ok(())
    }

    #[inline]
    fn push_sample(&mut self, x: Complex32) {
        let mut x = if x.re.is_finite() && x.im.is_finite() {
            x
        } else {
            Complex32::default()
        };
        if let Some(a) = self.norm_alpha {
            let p = x.norm_sqr();
            if (self.n_power as f32) * a < 1.0 {
                self.n_power += 1;
                self.in_power += (p - self.in_power) / self.n_power as f32;
            } else {
                self.in_power += a * (p - self.in_power);
            }
            x *= 1.0 / self.in_power.max(1e-20).sqrt();
        }
        let old = self.x.push(x);
        self.pow_x += x.norm_sqr() - old.norm_sqr();
    }

    /// One symbol: takes exactly `sps` input samples, returns the equalised symbol `y r` and adapts (towards `known` when given,
    /// otherwise by the current mode). The symbol period starts with `input[0]`, which also aligns [`process`](Self::process).
    pub fn step(
        &mut self,
        input: &[Complex32],
        known: Option<Complex32>,
    ) -> Result<Complex32, ModemError> {
        if input.len() != self.cfg.sps {
            return Err(ModemError::InputTooShort { need: self.cfg.sps });
        }
        for &x in input {
            self.push_sample(x);
        }
        self.phase = 0;
        Ok(self.output(known))
    }

    /// Streaming: takes any number of samples and appends one equalised symbol per `sps` of them to `out`, adapting blind (by the
    /// current mode).
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        for &s in x {
            self.push_sample(s);
            self.phase += 1;
            if self.phase == self.cfg.sps {
                self.phase = 0;
                let y = self.output(None);
                out.push(y);
            }
        }
    }

    /// The output at a symbol instant and the update.
    fn output(&mut self, known: Option<Complex32>) -> Complex32 {
        self.since_resum += 1;
        if self.since_resum >= 4096 {
            self.since_resum = 0;
            self.pow_x = self.x.power();
            self.pow_d = self.d.power();
        }
        let (xr, xi) = self.x.window();
        let (mut yr, mut yi) = cdot(&self.wr, &self.wi, xr, xi);
        if self.cfg.feedback > 0 {
            let (dr, di) = self.d.window();
            let (fr, fi) = cdot(&self.br, &self.bi, dr, di);
            yr += fr;
            yi += fi;
        }
        let y = Complex32::new(yr, yi);
        if !(yr.is_finite() && yi.is_finite()) || y.norm_sqr() > self.limit {
            self.resets += 1;
            self.reset_taps();
            self.mse = 1.0;
            self.mode = self.cfg.mode;
            // The output of the centre tap alone.
            let (xr, xi) = self.x.window();
            let c = self.cfg.centre;
            return Complex32::new(xr[c], xi[c]) * self.rot;
        }
        let yd = y * self.rot;
        let dec = match known {
            Some(a) => a,
            None => self.c.nearest(yd),
        };
        self.last_decision = dec;
        let derr = (yd - dec).norm_sqr();
        self.mse += self.cfg.mse_alpha * (derr - self.mse);
        if known.is_none() {
            if let Some(t) = self.cfg.dd_switch_mse {
                match self.mode {
                    Mode::Cma if self.mse < t => self.mode = Mode::Dd,
                    Mode::Dd if self.mse > 4.0 * t => self.mode = Mode::Cma,
                    _ => {}
                }
            }
        }
        // The error in the equaliser's own frame.
        let e = match (known, self.mode) {
            (Some(a), _) => a * self.rot.conj() - y,
            (None, Mode::Dd) => dec * self.rot.conj() - y,
            (None, Mode::Cma) => y * (self.r2 - y.norm_sqr()),
            (None, Mode::Frozen) => Complex32::default(),
        };
        let mut g = self.cfg.mu;
        if self.cfg.normalised {
            g /= 1e-6 + self.pow_x.max(0.0) + self.pow_d.max(0.0);
        }
        if e != Complex32::default() {
            let (xr, xi) = self.x.window();
            cupdate(&mut self.wr, &mut self.wi, xr, xi, g * e.re, g * e.im);
            if self.cfg.feedback > 0 {
                let (dr, di) = self.d.window();
                cupdate(&mut self.br, &mut self.bi, dr, di, g * e.re, g * e.im);
            }
        }
        if self.cfg.feedback > 0 {
            let fb = dec * self.rot.conj();
            let old = self.d.push(fb);
            self.pow_d += fb.norm_sqr() - old.norm_sqr();
        }
        yd
    }
}

/// Solves `a x = b` for `n` unknowns (`a` row major, `n x n`) by Gaussian elimination with partial pivoting, in place: `b`
/// becomes `x`. Errors when the matrix is singular.
fn solve(a: &mut [Complex64], b: &mut [Complex64], n: usize) -> Result<(), ModemError> {
    let scale = a.iter().map(|v| v.norm()).fold(0.0f64, f64::max);
    for col in 0..n {
        let piv = (col..n)
            .max_by(|&i, &j| a[i * n + col].norm().total_cmp(&a[j * n + col].norm()))
            .unwrap_or(col);
        let mag = a[piv * n + col].norm();
        if mag.is_nan() || mag <= 1e-12 * scale.max(1e-300) {
            return Param::Training.degenerate();
        }
        if piv != col {
            for k in 0..n {
                a.swap(piv * n + k, col * n + k);
            }
            b.swap(piv, col);
        }
        let p = a[col * n + col];
        for r in col + 1..n {
            let f = a[r * n + col] / p;
            if f == Complex64::default() {
                continue;
            }
            for k in col..n {
                let v = a[col * n + k];
                a[r * n + k] -= f * v;
            }
            let v = b[col];
            b[r] -= f * v;
        }
    }
    for col in (0..n).rev() {
        let mut s = b[col];
        for k in col + 1..n {
            s -= a[col * n + k] * b[k];
        }
        b[col] = s / a[col * n + col];
    }
    Ok(())
}

/// A least-squares channel estimator for one training sequence (module documentation).
///
/// The model is `rx[r] = sum_k h[k] t[r + taps - 1 - k] + noise` for `r` in `0..rows()`, `rows = t.len() - taps + 1`: received
/// sample `r` lines up with training symbol `r + taps - 1`, and `h[0]` is the tap on the newest symbol.
///
/// ```
/// use sdrkit_algo_modem::equalizer::LsChannel;
/// use sdrkit_algo_modem::Complex32;
/// let t: Vec<Complex32> = (0..26).map(|k| Complex32::new(if (k * 7 + k / 3) % 3 == 0 { -1.0 } else { 1.0 }, 0.0)).collect();
/// let ls = LsChannel::new(&t, 5).unwrap();
/// let h = [Complex32::new(1.0, 0.0), Complex32::new(0.3, 0.2), Complex32::new(0.0, 0.0), Complex32::new(-0.1, 0.0), Complex32::new(0.05, 0.0)];
/// let rx: Vec<Complex32> = (0..ls.rows()).map(|r| (0..5).map(|k| h[k] * t[r + 4 - k]).sum()).collect();
/// let mut est = [Complex32::default(); 5];
/// let noise = ls.estimate(&rx, &mut est).unwrap();
/// assert!((est[1] - h[1]).norm() < 1e-4 && noise < 1e-8);
/// ```
#[derive(Clone, Debug)]
pub struct LsChannel {
    taps: usize,
    rows: usize,
    train: Vec<Complex32>,
    pinv: Vec<Complex32>,
    gain: f64,
}

impl LsChannel {
    /// The estimator for `training` with `taps` channel taps (1 to 64). Returns an error unless the sequence has more rows than taps and
    /// excites every tap.
    pub fn new(training: &[Complex32], taps: usize) -> Result<Self, ModemError> {
        if taps == 0 || taps > 64 {
            return Param::Taps.range(1.0, 64.0, taps as f64);
        }
        if training.len() < 2 * taps {
            return Param::Training.at_least((2 * taps) as f64, training.len() as f64);
        }
        if training
            .iter()
            .any(|z| !(z.re.is_finite() && z.im.is_finite()))
        {
            return Param::Training.not_finite();
        }
        let rows = training.len() - taps + 1;
        let a = |r: usize, k: usize| -> Complex64 {
            let z = training[r + taps - 1 - k];
            Complex64::new(z.re as f64, z.im as f64)
        };
        // G = A^H A.
        let mut g = vec![Complex64::default(); taps * taps];
        for i in 0..taps {
            for j in 0..taps {
                g[i * taps + j] = (0..rows).map(|r| a(r, i).conj() * a(r, j)).sum();
            }
        }
        // G^-1, column by column.
        let mut inv = vec![Complex64::default(); taps * taps];
        let mut work = vec![Complex64::default(); taps * taps];
        let mut col = vec![Complex64::default(); taps];
        for c in 0..taps {
            work.copy_from_slice(&g);
            col.fill(Complex64::default());
            col[c] = Complex64::new(1.0, 0.0);
            solve(&mut work, &mut col, taps)?;
            for r in 0..taps {
                inv[r * taps + c] = col[r];
            }
        }
        let gain = (0..taps).map(|i| inv[i * taps + i].re).sum();
        // P = G^-1 A^H, taps x rows.
        let mut pinv = vec![Complex32::default(); taps * rows];
        for i in 0..taps {
            for r in 0..rows {
                let v: Complex64 = (0..taps).map(|k| inv[i * taps + k] * a(r, k).conj()).sum();
                pinv[i * rows + r] = Complex32::new(v.re as f32, v.im as f32);
            }
        }
        Ok(LsChannel {
            taps,
            rows,
            train: training.to_vec(),
            pinv,
            gain,
        })
    }

    /// Channel taps.
    pub fn taps(&self) -> usize {
        self.taps
    }

    /// Received samples an estimate reads (`training.len() - taps + 1`).
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// `trace((A^H A)^-1)`: the estimate's total mean squared error is `noise_var * noise_gain()`.
    pub fn noise_gain(&self) -> f64 {
        self.gain
    }

    /// Estimates the taps from `rx` (the first `rows()` samples are read) into `h[0..taps]`; returns the noise variance estimated
    /// from the residual (`|rx - A h|^2 / (rows - taps)`).
    pub fn estimate(&self, rx: &[Complex32], h: &mut [Complex32]) -> Result<f32, ModemError> {
        if rx.len() < self.rows {
            return Err(ModemError::InputTooShort { need: self.rows });
        }
        if h.len() < self.taps {
            return Err(ModemError::OutputTooShort { need: self.taps });
        }
        let rx = &rx[..self.rows];
        if rx.iter().any(|z| !(z.re.is_finite() && z.im.is_finite())) {
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        for (i, hi) in h.iter_mut().enumerate().take(self.taps) {
            let row = &self.pinv[i * self.rows..(i + 1) * self.rows];
            *hi = row.iter().zip(rx).map(|(p, r)| p * r).sum();
        }
        let mut res = 0f64;
        for (r, &y) in rx.iter().enumerate() {
            let mut m = Complex32::default();
            for (k, hk) in h.iter().enumerate().take(self.taps) {
                m += hk * self.train[r + self.taps - 1 - k];
            }
            res += (y - m).norm_sqr() as f64;
        }
        let dof = (self.rows - self.taps).max(1);
        Ok((res / dof as f64) as f32)
    }
}

/// Benchmarks: a 28-tap T/2 CMA equaliser at the DVB-C symbol rate of 6.9 MBd, and an NLMS DFE (25 + 6 taps, T/2) at HFDL's
/// 1800 Bd.
/// Known symbols of the 8-VSB field sync: the 4 segment sync symbols, the PN511, the three PN63 and the 24 symbols that name the VSB type.
pub const VSB_FIELD_SYNC_LEN: usize = 728;

/// Which path an [`EchoCanceller`] removes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EchoSide {
    /// An earlier path (a pre-echo), `D` symbols before the strongest.
    Pre,
    /// A later path (a post-echo), `D` symbols after the strongest.
    Post,
}

/// Why an [`EchoCanceller`] was not made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EchoRefusal {
    /// The delay leaves fewer rows than the fit needs, or is too short for the recursion.
    Delay,
    /// The received symbols do not cover the field sync.
    Input,
    /// A least-squares fit has no solution.
    Fit,
    /// `sum k^2` is at least the largest power: the echo is not the weaker path.
    TooStrong(f64),
    /// `sum k^2` is under the smallest power of a post-echo: not worth cancelling.
    TooWeak(f64),
}

/// The numbers of the echo fits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EchoFitRule {
    /// Taps either side of the strongest path in the fit (6).
    pub main_half: usize,
    /// Taps either side of the strongest path in the fit of the part in quadrature (16): the quadrature pulse is long.
    pub quad_half: usize,
    /// Taps either side of the echo (12): the canceller has `2 echo_half + 1` taps.
    pub echo_half: usize,
    /// The ridge of the first fit, relative to the mean diagonal (1e-6).
    pub ridge_first: f64,
    /// The ridge of the second fit (1e-4).
    pub ridge_second: f64,
    /// Rows a fit needs (200).
    pub min_rows: usize,
    /// A canceller with `sum k^2` at least this is refused (0.81).
    pub max_power: f64,
    /// A post-echo canceller with `sum k^2` under this is refused (0.0025, -26 dB).
    pub min_power: f64,
    /// A post-echo output whose peak exceeds this multiple of the input's is refused (4).
    pub peak_ratio: f32,
}

impl Default for EchoFitRule {
    fn default() -> Self {
        EchoFitRule {
            main_half: 6,
            quad_half: 16,
            echo_half: 12,
            ridge_first: 1e-6,
            ridge_second: 1e-4,
            min_rows: 200,
            max_power: 0.81,
            min_power: 0.0025,
            peak_ratio: 4.0,
        }
    }
}

/// A long echo canceller for 8-VSB: turns the received symbols `r` into an estimate `u` of the strongest path alone, before the
/// usual equalisers, from a fit on the known field sync (`fs[i]`, levels +-5, at `r[f0 + i]`).
///
/// The symbols are complex: the part in phase with the strongest path, which holds the levels, and the part in quadrature
/// at the same instants. A path with another carrier phase is in both parts, and its levels reach the part in phase only
/// through the long quadrature pulse of the vestigial sideband, which a few real taps do not hold. On the complex symbols the
/// echo is the strongest path delayed and turned: real taps `k_j` (the delay, with its fraction of a symbol) times one turn
/// `e^(j p)`. A signal without a part in quadrature (all zero) gives a turn of 0 or 180 degrees.
///
/// Fits ([`LeastSquares`] with a ridge relative to the mean diagonal):
///
/// - first, for each part of the signal, `r[f0 + i] = sum_{|j| <= 6} m_j fs[i + j] + sum_{|j| <= 12} c_j fs[i +- D + j] +
///   bias` (pre-echo `+D`, rows `i = 10 .. 728 - D - 12`; post-echo `-D`, rows `i = D + 12 .. 722`), ridge 1e-6, at least 200
///   rows; the part in quadrature with `|j| <= 16` for `m`, because its pulse is long, on the rows that have those symbols;
/// - `s0(i) = sum m_j fs[i + j]` is the strongest path alone where it is known; second,
///   `r[f0 + i] - s0(i) - bias = sum_{|j| <= 12} k_j s0(i +- D + j)`, ridge 1e-4, over the rows of the first fit where every
///   `s0` term is known (a post-echo from `D + 24`): with complex `k`, whose sum gives the turn `p`, then with real `k` on the
///   left side turned back by `p`. (On a signal of one sideband, complex taps have more than one set that fits: the quadrature
///   pulse times `j` acts as 1. Real taps and one turn have one.)
/// - refused when `sum k^2 >= 0.81` (the recursion needs the echo weaker), and for a post-echo when `sum k^2 < 0.0025`.
///
/// Recursion ([`apply`](Self::apply)): a pre-echo backward, `u[m] = r[m] - e^(j p) sum_j k_j u[m + D + j]` from the end down
/// (run over the field and the 1600 symbols before it); a post-echo forward, `u[m] = r[m] - e^(j p) sum_j k_j u[m - D + j]`
/// from the start (begin
/// 1600 symbols before the field so it settles). Symbols outside `r` count as 0. Both need `D > 12`, so every `u` the sum reads
/// is already made; nearer echoes are the equalisers'. The sums run on the shared FIR (NEON on 64-bit ARM).
#[derive(Clone, Debug)]
pub struct EchoCanceller {
    side: EchoSide,
    delay: usize,
    taps: Vec<f32>,
    /// The cosine and the sine of the turn.
    turn: (f32, f32),
    power: f64,
    peak_ratio: f32,
    /// The taps on the part in phase and on the part in quadrature of `u`.
    fir: [Fir<f32>; 2],
}

/// `s0(i)` for every index of the field sync where all its taps are known (`None` elsewhere): the taps `m` of the part in
/// phase and `mq` of the part in quadrature.
fn main_path(known: &[f32], m: &[f64], mq: &[f64], i: i64) -> Option<Complex64> {
    let part = |m: &[f64]| -> Option<f64> {
        let h = (m.len() / 2) as i64;
        if i - h < 0 || i + h >= known.len() as i64 {
            return None;
        }
        Some(
            m.iter()
                .enumerate()
                .map(|(j, &mj)| mj * known[(i - h + j as i64) as usize] as f64)
                .sum(),
        )
    };
    Some(Complex64::new(part(m)?, part(mq)?))
}

impl EchoCanceller {
    /// Fits a canceller of the path `delay` symbols before (pre) or after (post) the strongest, on the field sync `known` (728
    /// levels) found at `r[f0..]`. `q` is the part in quadrature of `r` (the same length).
    pub fn fit(
        side: EchoSide,
        r: &[f32],
        q: &[f32],
        f0: usize,
        known: &[f32],
        delay: usize,
        rule: &EchoFitRule,
    ) -> Result<Self, EchoRefusal> {
        let (mh, eh) = (rule.main_half as i64, rule.echo_half as i64);
        let n = known.len() as i64;
        let d = delay as i64;
        if d <= eh {
            return Err(EchoRefusal::Delay);
        }
        if known.is_empty() || f0 + known.len() > r.len() || q.len() != r.len() {
            return Err(EchoRefusal::Input);
        }
        // The echo index of row i, and the rows of the first fit.
        let (sign, rows) = match side {
            EchoSide::Pre => (1i64, (4 + mh)..(n - d - eh)),
            EchoSide::Post => (-1i64, (d + eh)..(n - mh)),
        };
        if rows.end - rows.start < rule.min_rows as i64 {
            return Err(EchoRefusal::Delay);
        }
        let ne = (2 * eh + 1) as usize;
        // The two parts of the signal are two fits with real rows. The part in quadrature has more taps of the strongest
        // path, and the rows that have all of them.
        let first = |target: &[f32], half: i64| -> Result<Vec<f64>, EchoRefusal> {
            let nm = (2 * half + 1) as usize;
            let mut ls = LeastSquares::new(nm + ne + 1).map_err(|_| EchoRefusal::Fit)?;
            let mut row = vec![0f64; nm + ne + 1];
            for i in rows.start.max(half)..rows.end.min(n - half) {
                for (j, v) in row[..nm].iter_mut().enumerate() {
                    *v = known[(i - half + j as i64) as usize] as f64;
                }
                for (j, v) in row[nm..nm + ne].iter_mut().enumerate() {
                    *v = known[(i + sign * d - eh + j as i64) as usize] as f64;
                }
                row[nm + ne] = 1.0;
                ls.add_row(&row, target[f0 + i as usize] as f64, 1.0)
                    .map_err(|_| EchoRefusal::Input)?;
            }
            let mut x = vec![0f64; nm + ne + 1];
            ls.solve(rule.ridge_first, &mut x)
                .map_err(|_| EchoRefusal::Fit)?;
            // The taps of the strongest path, then the bias.
            x.copy_within(nm + ne.., nm);
            x.truncate(nm + 1);
            Ok(x)
        };
        let mut m = first(r, mh)?;
        let mut mq = first(q, (rule.quad_half as i64).max(mh))?;
        let bias = Complex64::new(m.pop().unwrap_or(0.0), mq.pop().unwrap_or(0.0));
        // The second fit, on the main path alone: the unknowns are the real parts of the taps, then the imaginary parts, and
        // each symbol gives a row for each of its parts.
        let start = match side {
            EchoSide::Pre => rows.start,
            EchoSide::Post => rows.start.max(d + 2 * eh),
        };
        let mut ls2 = LeastSquares::new(2 * ne).map_err(|_| EchoRefusal::Fit)?;
        let mut row_re = vec![0f64; 2 * ne];
        let mut row_im = vec![0f64; 2 * ne];
        let mut symbols = 0usize;
        for i in start..rows.end {
            let Some(s0) = main_path(known, &m, &mq, i) else {
                continue;
            };
            let mut ok = true;
            for j in 0..ne {
                match main_path(known, &m, &mq, i + sign * d - eh + j as i64) {
                    Some(s) => {
                        (row_re[j], row_re[ne + j]) = (s.re, -s.im);
                        (row_im[j], row_im[ne + j]) = (s.im, s.re);
                    }
                    None => ok = false,
                }
            }
            if ok {
                let at = f0 + i as usize;
                let y = Complex64::new(r[at] as f64, q[at] as f64) - s0 - bias;
                ls2.add_row(&row_re, y.re, 1.0)
                    .map_err(|_| EchoRefusal::Input)?;
                ls2.add_row(&row_im, y.im, 1.0)
                    .map_err(|_| EchoRefusal::Input)?;
                symbols += 1;
            }
        }
        if symbols < rule.min_rows {
            return Err(EchoRefusal::Delay);
        }
        let mut k = vec![0f64; 2 * ne];
        ls2.solve(rule.ridge_second, &mut k)
            .map_err(|_| EchoRefusal::Fit)?;
        // The turn, then real taps on the same symbols turned back.
        let turn = Complex64::new(k[..ne].iter().sum(), k[ne..].iter().sum());
        let turn = if turn.norm() > 0.0 {
            turn / turn.norm()
        } else {
            Complex64::new(1.0, 0.0)
        };
        let mut ls3 = LeastSquares::new(ne).map_err(|_| EchoRefusal::Fit)?;
        for i in start..rows.end {
            let Some(s0) = main_path(known, &m, &mq, i) else {
                continue;
            };
            let mut ok = true;
            for j in 0..ne {
                match main_path(known, &m, &mq, i + sign * d - eh + j as i64) {
                    Some(s) => (row_re[j], row_im[j]) = (s.re, s.im),
                    None => ok = false,
                }
            }
            if ok {
                let at = f0 + i as usize;
                let y = (Complex64::new(r[at] as f64, q[at] as f64) - s0 - bias) * turn.conj();
                ls3.add_row(&row_re[..ne], y.re, 1.0)
                    .map_err(|_| EchoRefusal::Input)?;
                ls3.add_row(&row_im[..ne], y.im, 1.0)
                    .map_err(|_| EchoRefusal::Input)?;
            }
        }
        k.truncate(ne);
        ls3.solve(rule.ridge_second, &mut k)
            .map_err(|_| EchoRefusal::Fit)?;
        let power: f64 = k.iter().map(|v| v * v).sum();
        if power.is_nan() || power >= rule.max_power {
            return Err(EchoRefusal::TooStrong(power));
        }
        if side == EchoSide::Post && power < rule.min_power {
            return Err(EchoRefusal::TooWeak(power));
        }
        let taps: Vec<f32> = k.iter().map(|&v| v as f32).collect();
        // The FIR's newest sample is the nearest to the cursor: u[m + D - 12] backward, u[m - D + 12] forward.
        let fir_taps: Vec<f32> = match side {
            EchoSide::Pre => taps.clone(),
            EchoSide::Post => taps.iter().rev().copied().collect(),
        };
        Ok(EchoCanceller {
            side,
            delay,
            fir: [Fir::new(&fir_taps), Fir::new(&fir_taps)],
            taps,
            turn: (turn.re as f32, turn.im as f32),
            power,
            peak_ratio: rule.peak_ratio,
        })
    }

    /// The side of the path removed.
    pub fn side(&self) -> EchoSide {
        self.side
    }

    /// The echo's delay from the strongest path (symbols).
    pub fn delay(&self) -> usize {
        self.delay
    }

    /// The taps `k_j`, `j = -12 ..= 12` around the delay.
    pub fn taps(&self) -> &[f32] {
        &self.taps
    }

    /// The turn of the echo against the strongest path: its cosine and sine.
    pub fn turn(&self) -> (f32, f32) {
        self.turn
    }

    /// `sum k^2`: the echo's power relative to the strongest path.
    pub fn power(&self) -> f64 {
        self.power
    }

    /// Runs the recursion over `r` with its part in quadrature `q` into `u` and `uq` (all of one length). Returns `false`
    /// when the lengths differ, or (post-echo) when the output is not finite or its peak exceeds 4 times the input's: the
    /// canceller should then be dropped.
    pub fn apply(&mut self, r: &[f32], q: &[f32], u: &mut [f32], uq: &mut [f32]) -> bool {
        if r.len() != u.len() || q.len() != r.len() || uq.len() != r.len() {
            return false;
        }
        let len = r.len() as i64;
        let near = self.delay as i64 - (self.taps.len() / 2) as i64;
        for f in &mut self.fir {
            f.reset();
        }
        let [fir, fir_q] = &mut self.fir;
        let (cos, sin) = self.turn;
        // The echo estimate from the symbol `idx` of `u`, already made: the taps on `u`, turned, in its two parts.
        let mut echo = |u: &[f32], uq: &[f32], idx: i64| -> (f32, f32) {
            let (x, xq) = if (0..len).contains(&idx) {
                (u[idx as usize], uq[idx as usize])
            } else {
                (0.0, 0.0)
            };
            let (e, eq) = (fir.process_sample(x), fir_q.process_sample(xq));
            (cos * e - sin * eq, sin * e + cos * eq)
        };
        match self.side {
            EchoSide::Pre => {
                for m in (0..len).rev() {
                    let (e, eq) = echo(u, uq, m + near);
                    u[m as usize] = r[m as usize] - e;
                    uq[m as usize] = q[m as usize] - eq;
                }
                true
            }
            EchoSide::Post => {
                for m in 0..len {
                    let (e, eq) = echo(u, uq, m - near);
                    u[m as usize] = r[m as usize] - e;
                    uq[m as usize] = q[m as usize] - eq;
                }
                let peak_in = r
                    .iter()
                    .zip(q)
                    .fold(0f32, |a, (v, w)| a.max(v.abs()).max(w.abs()));
                let mut peak = 0f32;
                for v in u.iter().chain(uq.iter()) {
                    if !v.is_finite() {
                        return false;
                    }
                    peak = peak.max(v.abs());
                }
                peak <= self.peak_ratio * peak_in
            }
        }
    }
}

/// The size of the correlation of the received symbols `r + j q` with the PN part of the field sync at lag `l`:
/// `|sum_{i=4..514} (r + j q)[f0 + i + l] fs[i]|` (`None` where `r` does not reach). The size does not depend on the carrier
/// phase of the path at that lag.
fn sync_correlation(r: &[f32], q: &[f32], f0: usize, known: &[f32], l: i64) -> Option<f64> {
    let (a, b) = (f0 as i64 + 4 + l, f0 as i64 + 515 + l);
    if a < 0 || b > r.len() as i64 || q.len() != r.len() || known.len() < 515 {
        return None;
    }
    let part = |x: &[f32]| -> f64 {
        x[a as usize..b as usize]
            .iter()
            .zip(&known[4..515])
            .map(|(&x, &k)| x as f64 * k as f64)
            .sum()
    };
    Some(part(r).hypot(part(q)))
}

/// A path found by [`earliest_path`] or [`latest_path`]: its lag from the strongest path (symbols, negative before it) and its
/// correlation relative to the strongest path's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EchoPath {
    /// The lag (symbols).
    pub lag: i64,
    /// `C(lag) / C0`.
    pub ratio: f64,
}

/// The correlations at `lags` (with one more each side for the peak test) and `C0 = max(C(-1), C(0), C(1))`.
fn correlations(
    r: &[f32],
    q: &[f32],
    f0: usize,
    known: &[f32],
    lags: std::ops::RangeInclusive<i64>,
    out: &mut Vec<f64>,
) -> Option<f64> {
    let c0 = (-1..=1)
        .filter_map(|l| sync_correlation(r, q, f0, known, l))
        .fold(0.0, f64::max);
    if c0 <= 0.0 {
        return None;
    }
    out.clear();
    out.extend(
        (lags.start() - 1..=lags.end() + 1)
            .map(|l| sync_correlation(r, q, f0, known, l).unwrap_or(0.0)),
    );
    Some(c0)
}

/// The earliest path: the most negative lag in `-max_lag ..= -min_lag` (330 and 6 normally; `min_lag` the feed-forward taps
/// after the cursor plus 2 for a second look) whose correlation exceeds `rel x C0` (0.4 normally, 0.2 for the second look) and
/// is a local peak. `q` is the part in quadrature of `r`. `scratch` grows once.
#[allow(clippy::too_many_arguments)]
pub fn earliest_path(
    r: &[f32],
    q: &[f32],
    f0: usize,
    known: &[f32],
    min_lag: usize,
    max_lag: usize,
    rel: f64,
    scratch: &mut Vec<f64>,
) -> Option<EchoPath> {
    let (lo, hi) = (-(max_lag as i64), -(min_lag as i64));
    if lo > hi {
        return None;
    }
    let c0 = correlations(r, q, f0, known, lo..=hi, scratch)?;
    (lo..=hi).find_map(|l| {
        let i = (l - lo + 1) as usize;
        let c = scratch[i];
        (c > rel * c0 && c >= scratch[i - 1] && c >= scratch[i + 1]).then_some(EchoPath {
            lag: l,
            ratio: c / c0,
        })
    })
}

/// The latest path: the strongest local peak with a lag in `min_lag ..= max_lag` (118 and 480) above `rel x C0` (0.2).
/// `q` is the part in quadrature of `r`. `scratch` grows once.
#[allow(clippy::too_many_arguments)]
pub fn latest_path(
    r: &[f32],
    q: &[f32],
    f0: usize,
    known: &[f32],
    min_lag: usize,
    max_lag: usize,
    rel: f64,
    scratch: &mut Vec<f64>,
) -> Option<EchoPath> {
    let (lo, hi) = (min_lag as i64, max_lag as i64);
    if lo > hi {
        return None;
    }
    let c0 = correlations(r, q, f0, known, lo..=hi, scratch)?;
    (lo..=hi)
        .filter_map(|l| {
            let i = (l - lo + 1) as usize;
            let c = scratch[i];
            (c > rel * c0 && c >= scratch[i - 1] && c >= scratch[i + 1]).then_some(EchoPath {
                lag: l,
                ratio: c / c0,
            })
        })
        .max_by(|a, b| a.ratio.total_cmp(&b.ratio))
}

/// How [`RealDfe::run`] follows the channel inside a run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DfeTracking {
    /// Symbols equalised with one set of taps before the taps move (832, a segment of 8-VSB).
    pub segment: usize,
    /// Every this many symbols gives a step, at a different phase each segment (4).
    pub every: usize,
    /// A symbol whose error against its decision is at least this gives no step (3).
    pub max_error: f32,
    /// The step of the normalised LMS and of the bias (0.04).
    pub mu: f32,
    /// The mean square of a symbol (21 for 8 levels +-1 .. +-7).
    pub symbol_power: f32,
}

impl Default for DfeTracking {
    fn default() -> Self {
        DfeTracking {
            segment: 832,
            every: 4,
            max_error: 3.0,
            mu: 0.04,
            symbol_power: 21.0,
        }
    }
}

/// How [`RealDfe::run`] decides the symbols that are not known. A function from the output to the symbol is a decider
/// (a slicer); a decider with memory, such as the survivors of a trellis code, can also correct symbols it decided before,
/// and the feedback then reads the corrected ones.
pub trait DfeDecider {
    /// Called once before the first symbol of a run.
    fn begin(&mut self) {}
    /// Decides symbol `t` from the output `y`: writes `d[t]`, and may write symbols before `t` again.
    fn decide(&mut self, t: usize, y: f32, d: &mut [f32]);
}

impl<F: Fn(f32) -> f32> DfeDecider for F {
    #[inline]
    fn decide(&mut self, t: usize, y: f32, d: &mut [f32]) {
        d[t] = self(y);
    }
}

/// The sum of products of two slices of one length in 8 lanes of `f32` (the feedback sum of [`RealDfe::run`]).
#[inline]
fn dot8(a: &[f32], b: &[f32]) -> f32 {
    let mut lanes = [0f32; 8];
    let (a8, ar) = a.as_chunks::<8>();
    let (b8, br) = b.as_chunks::<8>();
    for (x, y) in a8.iter().zip(b8) {
        for l in 0..8 {
            lanes[l] += x[l] * y[l];
        }
    }
    let rest: f32 = ar.iter().zip(br).map(|(x, y)| x * y).sum();
    ((lanes[0] + lanes[4]) + (lanes[1] + lanes[5]))
        + ((lanes[2] + lanes[6]) + (lanes[3] + lanes[7]))
        + rest
}

/// The sum of products of two slices of one length, in 8 lanes of `f32` per 1024 values and `f64` across them.
fn dot64(a: &[f32], b: &[f32]) -> f64 {
    let mut total = 0f64;
    for (a, b) in a.chunks(1024).zip(b.chunks(1024)) {
        let mut lanes = [0f32; 8];
        let (a8, ar) = a.as_chunks::<8>();
        let (b8, br) = b.as_chunks::<8>();
        for (x, y) in a8.iter().zip(b8) {
            for l in 0..8 {
                lanes[l] += x[l] * y[l];
            }
        }
        let rest: f32 = ar.iter().zip(br).map(|(x, y)| x * y).sum();
        total += f64::from(lanes.iter().sum::<f32>() + rest);
    }
    total
}

/// A decision-feedback equaliser for real symbols (a PAM signal such as 8-VSB at one sample per symbol), with its taps from a
/// least-squares fit:
///
/// `y[t] = sum_{k < F} f[k] x[t - before + k] + sum_{b < B} g[b] d[t - 1 - b] + bias`
///
/// with `x` the received symbols, `d` the symbols decided before `t` (known ones where they are known), `F` feed-forward taps
/// of which `before` lie before the cursor, and `B` feedback taps (0 gives a feed-forward equaliser). The feedback takes off
/// the echoes up to `B` symbols after the strongest path without adding noise; the feed-forward taps take the earlier ones.
///
/// **Fit**: [`begin`](Self::begin), rows of known or decided symbols, [`solve`](Self::solve). [`add_rows`](Self::add_rows)
/// adds each row exactly (a short run of training symbols); [`add_run`](Self::add_run) adds a long run from sums of products
/// by lag, which leaves out the difference at the two ends of the run (for a field of decided symbols).
///
/// **Run**: [`run`](Self::run) equalises a range, deciding each symbol that is not known, and with [`DfeTracking`] moves the taps
/// after each segment by normalised LMS steps. Both filters run on the shared FIR (NEON on 64-bit ARM). Nothing is allocated
/// after [`RealDfe::new`].
#[derive(Clone, Debug)]
pub struct RealDfe {
    before: usize,
    ff: Vec<f32>,
    fb: Vec<f32>,
    bias: f32,
    ata: Vec<f64>,
    atb: Vec<f64>,
    rows: u64,
    row: Vec<f64>,
    lags: Vec<f64>,
    fir_ff: Fir<f32>,
    /// The feedback taps, the last first: tap `i` multiplies `d[t - B + i]`.
    fb_rev: Vec<f32>,
    rev: Vec<f32>,
    grad: Vec<f32>,
    tracking: Option<DfeTracking>,
}

impl RealDfe {
    /// An equaliser of `ff` feed-forward taps (1 to 512), `before` of them before the cursor, and `fb` feedback taps (0 to
    /// 512). The cursor tap starts at 1, every other at 0. Errors for other values.
    pub fn new(ff: usize, before: usize, fb: usize) -> Result<Self, ModemError> {
        Param::Taps.check_count(ff, 1, 512)?;
        Param::Taps.check_count(before, 0, ff - 1)?;
        Param::Feedback.check_count(fb, 0, 512)?;
        let n = ff + fb + 1;
        let mut taps = vec![0f32; ff];
        taps[before] = 1.0;
        let mut e = RealDfe {
            before,
            ff: taps,
            fb: vec![0.0; fb],
            bias: 0.0,
            ata: vec![0.0; n * n],
            atb: vec![0.0; n],
            rows: 0,
            row: vec![0.0; n],
            lags: vec![0.0; 2 * (ff + fb) + 4],
            fir_ff: Fir::new(&vec![0.0; ff]),
            fb_rev: vec![0.0; fb],
            rev: vec![0.0; ff],
            grad: vec![0.0; ff + fb],
            tracking: None,
        };
        e.load();
        Ok(e)
    }

    /// The feed-forward taps: tap `k` multiplies `x[t - before + k]`.
    pub fn feed_forward(&self) -> &[f32] {
        &self.ff
    }

    /// The feedback taps: tap `b` multiplies `d[t - 1 - b]`.
    pub fn feedback(&self) -> &[f32] {
        &self.fb
    }

    /// The bias.
    pub fn bias(&self) -> f32 {
        self.bias
    }

    /// Feed-forward taps before the cursor.
    pub fn before(&self) -> usize {
        self.before
    }

    /// Feed-forward taps after the cursor.
    pub fn after(&self) -> usize {
        self.ff.len() - 1 - self.before
    }

    /// Sets how [`run`](Self::run) follows the channel: `None` (as made) keeps the taps fixed.
    pub fn set_tracking(&mut self, tracking: Option<DfeTracking>) {
        self.tracking = tracking;
    }

    /// Sets the taps (kept from an earlier fit). Errors when a length differs.
    pub fn set_taps(&mut self, ff: &[f32], fb: &[f32], bias: f32) -> Result<(), ModemError> {
        if ff.len() != self.ff.len() {
            return Param::Taps.length(self.ff.len(), ff.len());
        }
        if fb.len() != self.fb.len() {
            return Param::Feedback.length(self.fb.len(), fb.len());
        }
        self.ff.copy_from_slice(ff);
        self.fb.copy_from_slice(fb);
        self.bias = bias;
        self.load();
        Ok(())
    }

    /// Gives the taps to the filters. The feed-forward filter's newest sample is the last one under the taps.
    fn load(&mut self) {
        for (r, t) in self.rev.iter_mut().zip(self.ff.iter().rev()) {
            *r = *t;
        }
        self.fir_ff.set_taps(&self.rev);
        for (r, t) in self.fb_rev.iter_mut().zip(self.fb.iter().rev()) {
            *r = *t;
        }
    }

    /// Starts a fit.
    pub fn begin(&mut self) {
        self.ata.fill(0.0);
        self.atb.fill(0.0);
        self.rows = 0;
    }

    /// Rows in the fit.
    pub fn rows(&self) -> u64 {
        self.rows
    }

    /// The symbols `t` of `range` that have every sample and every earlier symbol the taps read.
    fn usable(&self, len: usize, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
        let lo = range.start.max(self.before).max(self.fb.len());
        let hi = range.end.min(len.saturating_sub(self.after()));
        lo..hi.max(lo)
    }

    /// Adds one exact row per symbol `t` of `range` to the fit: `d[t]` is wanted, and `d` before `t` is the feedback. Symbols
    /// whose taps would read outside `x` or before the start of `d` are left out. `x` and `d` have one length.
    pub fn add_rows(&mut self, x: &[f32], d: &[f32], range: std::ops::Range<usize>) {
        let (f, b) = (self.ff.len(), self.fb.len());
        let n = f + b + 1;
        for t in self.usable(x.len().min(d.len()), range) {
            for (r, v) in self.row[..f].iter_mut().zip(&x[t - self.before..]) {
                *r = f64::from(*v);
            }
            for (k, r) in self.row[f..f + b].iter_mut().enumerate() {
                *r = f64::from(d[t - 1 - k]);
            }
            self.row[f + b] = 1.0;
            let want = f64::from(d[t]);
            for i in 0..n {
                let ri = self.row[i];
                for (a, rj) in self.ata[i * n + i..(i + 1) * n]
                    .iter_mut()
                    .zip(&self.row[i..])
                {
                    *a += ri * rj;
                }
                self.atb[i] += ri * want;
            }
            self.rows += 1;
        }
    }

    /// Adds the symbols of `range` as one long run, from sums of products by lag: the autocorrelation of `x` and of `d` and
    /// their cross-correlation, with both taken as zero outside the run (so the sums are those of real rows and the fit has a
    /// solution). The rows at the two ends of the run are therefore not exact: the run must be long against the taps; a run
    /// under 4 times their number is left out.
    pub fn add_run(&mut self, x: &[f32], d: &[f32], range: std::ops::Range<usize>) {
        let (f, b) = (self.ff.len(), self.fb.len());
        let n = f + b + 1;
        let lo = range.start;
        let hi = range.end.min(x.len().min(d.len()));
        if hi <= lo + 4 * n {
            return;
        }
        let len = hi - lo;
        let (xs, ds) = (&x[lo..hi], &d[lo..hi]);
        // Rxx(l) = sum x[t] x[t + l]; Rdd(l) = sum d[t] d[t - l]; C(l) = sum d[t] x[t + l], l from -before to after + b.
        let c0 = self.before;
        let nc = f + b;
        let (rxx, rest) = self.lags.split_at_mut(f);
        let (rdd, c) = rest.split_at_mut(b + 1);
        for (l, v) in rxx.iter_mut().enumerate() {
            *v = dot64(&xs[..len - l], &xs[l..]);
        }
        for (l, v) in rdd.iter_mut().enumerate() {
            *v = dot64(&ds[l..], &ds[..len - l]);
        }
        for (k, v) in c[..nc].iter_mut().enumerate() {
            *v = if k >= c0 {
                let l = k - c0;
                dot64(&ds[..len - l], &xs[l..])
            } else {
                let l = c0 - k;
                dot64(&ds[l..], &xs[..len - l])
            };
        }
        let sx: f64 = xs.iter().map(|&v| f64::from(v)).sum();
        let sd: f64 = ds.iter().map(|&v| f64::from(v)).sum();
        let ata = &mut self.ata;
        for i in 0..f {
            for j in i..f {
                ata[i * n + j] += rxx[j - i];
            }
            // x[t - before + i] with d[t - 1 - k]: the lag of x from d is i - before + 1 + k.
            for k in 0..b {
                ata[i * n + f + k] += c[i + 1 + k];
            }
            ata[i * n + f + b] += sx;
            self.atb[i] += c[i];
        }
        for k in 0..b {
            for j in k..b {
                ata[(f + k) * n + f + j] += rdd[j - k];
            }
            ata[(f + k) * n + f + b] += sd;
            self.atb[f + k] += rdd[k + 1];
        }
        // The rows run from the first to the last that reads a symbol of the run.
        ata[(f + b) * n + f + b] += (len + f + b) as f64;
        self.atb[f + b] += sd;
        self.rows += len as u64;
    }

    /// Solves the fit with a ridge of `ridge` times the mean diagonal and takes the taps. `false`, with the taps unchanged,
    /// when the fit has fewer rows than unknowns or no solution.
    pub fn solve(&mut self, ridge: f64) -> bool {
        let (f, b) = (self.ff.len(), self.fb.len());
        let n = f + b + 1;
        if self.rows < n as u64 {
            return false;
        }
        let tr: f64 = (0..n).map(|i| self.ata[i * n + i]).sum::<f64>() / n as f64;
        for i in 0..n {
            for j in 0..i {
                self.ata[i * n + j] = self.ata[j * n + i];
            }
            self.ata[i * n + i] += ridge * tr;
        }
        if cholesky_solve(&mut self.ata, &mut self.atb, n).is_err()
            || self.atb.iter().any(|v| !v.is_finite())
        {
            return false;
        }
        for (t, v) in self.ff.iter_mut().zip(&self.atb[..f]) {
            *t = *v as f32;
        }
        for (t, v) in self.fb.iter_mut().zip(&self.atb[f..f + b]) {
            *t = *v as f32;
        }
        self.bias = self.atb[f + b] as f32;
        self.load();
        true
    }

    /// The summed squared error and the number of the symbols of `range` against `d`, with `d` before each symbol as the
    /// feedback (known symbols: no decision is taken).
    pub fn error(&self, x: &[f32], d: &[f32], range: std::ops::Range<usize>) -> (f64, usize) {
        let mut e2 = 0f64;
        let r = self.usable(x.len().min(d.len()), range);
        for t in r.clone() {
            let mut y = self.bias;
            for (w, v) in self.ff.iter().zip(&x[t - self.before..]) {
                y += w * v;
            }
            for (k, w) in self.fb.iter().enumerate() {
                y += w * d[t - 1 - k];
            }
            e2 += f64::from(y - d[t]).powi(2);
        }
        (e2, r.len())
    }

    /// Equalises the symbols of `range`: `y[t]` is the output and `d[t]` the symbol decided, which is `known[t]` where that is
    /// finite and what `decide` ([`DfeDecider`]) writes elsewhere. The feedback of a symbol reads `d` as it is then, with the
    /// corrections of the decider. `d` before the range is the feedback of its first symbols as the caller left it;
    /// samples outside `x` count as 0. With [`set_tracking`](Self::set_tracking), after each segment the taps move by one
    /// normalised LMS step for every
    /// `every`-th symbol whose error against its decision is under `max_error`:
    /// `mu / (F Px + B symbol_power)` times the error times the tap's input, `Px` the mean power of `x`, and the bias by `mu`
    /// times the mean error. `x`, `known`, `y` and `d` have one length; a range past it is cut.
    pub fn run(
        &mut self,
        x: &[f32],
        known: &[f32],
        decide: &mut impl DfeDecider,
        range: std::ops::Range<usize>,
        y: &mut [f32],
        d: &mut [f32],
    ) {
        let track = self.tracking;
        let len = x.len().min(known.len()).min(y.len()).min(d.len());
        let range = range.start.min(len)..range.end.min(len);
        if range.is_empty() {
            return;
        }
        let (f, b) = (self.ff.len(), self.fb.len());
        let after = self.after();
        let segment = track.as_ref().map_or(usize::MAX, |t| t.segment.max(1));
        let px = (dot64(&x[range.clone()], &x[range.clone()]) / range.len() as f64) as f32;
        // The feed-forward filter starts with the samples before the first symbol under its taps.
        self.fir_ff.reset();
        let first = range.start + after + 1;
        for i in first.saturating_sub(f)..first - 1 {
            self.fir_ff.push(x.get(i).copied().unwrap_or(0.0));
        }
        decide.begin();
        let mut seg = 0usize;
        let mut t0 = range.start;
        while t0 < range.end {
            let t1 = range.end.min(t0.saturating_add(segment));
            // Feed-forward over the segment: the output for symbol t comes with sample t + after.
            for (k, o) in y[t0..t1].iter_mut().enumerate() {
                let v = x[..len].get(t0 + k + after).copied().unwrap_or(0.0);
                *o = self.fir_ff.process_sample(v);
            }
            if b == 0 {
                for t in t0..t1 {
                    y[t] += self.bias;
                    if known[t].is_finite() {
                        d[t] = known[t];
                    } else {
                        decide.decide(t, y[t], d);
                    }
                }
            } else {
                for t in t0..t1 {
                    // Symbols before the start of `d` count as 0.
                    let n = t.min(b);
                    y[t] += dot8(&self.fb_rev[b - n..], &d[t - n..t]) + self.bias;
                    if known[t].is_finite() {
                        d[t] = known[t];
                    } else {
                        decide.decide(t, y[t], d);
                    }
                }
            }
            if let Some(tr) = &track {
                self.grad.fill(0.0);
                let every = tr.every.max(1);
                let (mut sum, mut used) = (0f32, 0usize);
                let lo = (t0 + seg % every).max(self.before).max(b);
                for t in (lo..t1).step_by(every) {
                    let e = y[t] - d[t];
                    if e.is_nan() || e.abs() >= tr.max_error || t + after >= len {
                        continue;
                    }
                    for (g, v) in self.grad[..f].iter_mut().zip(&x[t - self.before..]) {
                        *g += e * v;
                    }
                    for (k, g) in self.grad[f..].iter_mut().enumerate() {
                        *g += e * d[t - 1 - k];
                    }
                    sum += e;
                    used += 1;
                }
                if used > 0 {
                    let mu = tr.mu / (f as f32 * px + b as f32 * tr.symbol_power).max(1e-20);
                    for (w, g) in self.ff.iter_mut().zip(&self.grad[..f]) {
                        *w -= mu * g;
                    }
                    for (w, g) in self.fb.iter_mut().zip(&self.grad[f..]) {
                        *w -= mu * g;
                    }
                    self.bias -= tr.mu * sum / used as f32;
                    // The same lengths: the filters keep their history.
                    self.load();
                }
            }
            seg += 1;
            t0 = t1;
        }
    }
}

pub(crate) fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    use sdrkit_core::bench::measure;
    use sdrkit_core::{BenchResult, Benchmark};
    fn signal(n: usize) -> Vec<Complex32> {
        let mut s = 0x1234_5678u32;
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let a = ((s >> 28) as f32 - 7.5) / 4.6;
                let b = (((s >> 24) & 15) as f32 - 7.5) / 4.6;
                Complex32::new(a, b)
            })
            .collect()
    }
    fn run(seconds: f64, cfg: EqConfig, rate: f64) -> BenchResult {
        use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
        let c =
            Constellation::new(PointSet::qam(64, QamMap::DVB).expect("64-QAM")).expect("slicer");
        let mut eq = AdaptiveEq::new(cfg, c).expect("valid settings");
        let x = signal(65_536);
        let mut out = Vec::with_capacity(x.len());
        measure(seconds, x.len(), x.len() as f64 / rate, || {
            out.clear();
            eq.process(&x, &mut out);
        })
    }
    vec![
        Benchmark {
            name: "modem.cma_t2_28tap_6900kbd",
            run: |s| {
                let mut cfg = EqConfig::new(28, 2);
                cfg.mu = 1e-4;
                cfg.dd_switch_mse = None;
                run(s, cfg, 2.0 * 6.9e6)
            },
        },
        Benchmark {
            name: "modem.nlms_dfe_25_6_1800bd",
            run: |s| {
                let mut cfg = EqConfig::new(25, 2);
                cfg.feedback = 6;
                cfg.normalised = true;
                cfg.mu = 0.1;
                cfg.mode = Mode::Dd;
                run(s, cfg, 2.0 * 1800.0)
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
    use sdrkit_algo_sim::rng::Rng;

    fn c64(z: Complex32) -> Complex64 {
        Complex64::new(z.re as f64, z.im as f64)
    }

    /// Symbols through a symbol-spaced channel plus complex noise of variance `n0`.
    fn channel(sym: &[Complex32], h: &[Complex32], n0: f64, rng: &mut Rng) -> Vec<Complex32> {
        (0..sym.len())
            .map(|n| {
                let mut s = Complex32::default();
                for (j, hj) in h.iter().enumerate() {
                    if n >= j {
                        s += hj * sym[n - j];
                    }
                }
                s + rng.complex_normal(n0)
            })
            .collect()
    }

    fn random_symbols(c: &PointSet, n: usize, rng: &mut Rng) -> (Vec<usize>, Vec<Complex32>) {
        let labels: Vec<usize> = (0..n).map(|_| rng.below(c.len() as u64) as usize).collect();
        let s = labels.iter().map(|&l| c.points()[l]).collect();
        (labels, s)
    }

    /// The Wiener solution of an `n`-tap equaliser (oldest first) for unit-energy white symbols through `h` with noise `n0`, and
    /// the target symbol `delay` symbols before the newest input; returns `(w, jmin, trace R)`.
    fn wiener(h: &[Complex32], n0: f64, n: usize, delay: usize) -> (Vec<Complex64>, f64, f64) {
        let hh = |j: i64| -> Complex64 {
            if j >= 0 && (j as usize) < h.len() {
                c64(h[j as usize])
            } else {
                Complex64::default()
            }
        };
        // r(m) = E[x[t]* x[t+m]].
        let r = |m: i64| -> Complex64 {
            let mut s: Complex64 = (0..h.len() as i64).map(|j| hh(j).conj() * hh(j + m)).sum();
            if m == 0 {
                s += n0;
            }
            s
        };
        let mut a = vec![Complex64::default(); n * n];
        for k in 0..n {
            for l in 0..n {
                a[k * n + l] = r(l as i64 - k as i64);
            }
        }
        // p[k] = E[x_k* d], x_k = x[t - (n-1) + k], d = a[t - delay]: conj(h[k - (n-1) + delay]).
        let mut p: Vec<Complex64> = (0..n)
            .map(|k| hh(k as i64 - (n as i64 - 1) + delay as i64).conj())
            .collect();
        let p0 = p.clone();
        let tr = n as f64 * r(0).re;
        solve(&mut a, &mut p, n).unwrap();
        let jmin = 1.0
            - p.iter()
                .zip(&p0)
                .map(|(w, q)| w * q.conj())
                .sum::<Complex64>()
                .re;
        (p, jmin, tr)
    }

    #[test]
    fn lms_and_nlms_converge_to_the_wiener_solution_with_the_predicted_excess_error() {
        let set = PointSet::qpsk();
        let h = [
            Complex32::new(1.0, 0.0),
            Complex32::new(0.35, 0.25),
            Complex32::new(-0.15, 0.05),
        ];
        let n0 = 0.02;
        let (n, delay) = (11usize, 6usize);
        let (wo, jmin, tr) = wiener(&h, n0, n, delay);
        let mut rng = Rng::new(7);
        let (_, sym) = random_symbols(&set, 300_000, &mut rng);
        let x = channel(&sym, &h, n0, &mut rng);
        for normalised in [false, true] {
            let mut cfg = EqConfig::new(n, 1);
            cfg.centre = n - 1 - delay;
            cfg.normalised = normalised;
            cfg.mu = if normalised { 0.1 } else { 0.1 / tr as f32 };
            // Predicted misadjustment of complex LMS: mu tr(R) / 2; of NLMS about mu / 2. Both are 0.05 here.
            let m_pred = 0.05;
            let mut eq = AdaptiveEq::new(cfg, Constellation::new(set.clone()).unwrap()).unwrap();
            let mut wsum = vec![Complex64::default(); n];
            let mut w = vec![Complex32::default(); n];
            let (mut esum, mut count) = (0f64, 0usize);
            for t in 0..x.len() {
                let want = if t >= delay {
                    sym[t - delay]
                } else {
                    Complex32::default()
                };
                let y = eq.step(&x[t..t + 1], Some(want)).unwrap();
                if t > 100_000 {
                    esum += (want - y).norm_sqr() as f64;
                    count += 1;
                    eq.feed_forward(&mut w).unwrap();
                    for (s, v) in wsum.iter_mut().zip(&w) {
                        *s += c64(*v);
                    }
                }
            }
            let dist: f64 = wsum
                .iter()
                .zip(&wo)
                .map(|(s, o)| (s / count as f64 - o).norm_sqr())
                .sum();
            let norm: f64 = wo.iter().map(|o| o.norm_sqr()).sum();
            assert!(
                dist / norm < 1e-3,
                "normalised {normalised}: weight error {}",
                dist / norm
            );
            let mse = esum / count as f64;
            let m = (mse - jmin) / jmin;
            assert!(
                m > 0.4 * m_pred && m < 2.5 * m_pred,
                "normalised {normalised}: misadjustment {m} vs {m_pred} (jmin {jmin})"
            );
        }
    }

    #[test]
    fn cma_opens_16qam_then_dd_reaches_the_awgn_error_rate() {
        let set = PointSet::qam(16, QamMap::DVB).unwrap();
        let c = Constellation::new(set.clone()).unwrap();
        assert!((cma_radius(&c) - 1.32).abs() < 1e-4);
        let c64q = Constellation::new(PointSet::qam(64, QamMap::DVB).unwrap()).unwrap();
        assert!((cma_radius(&c64q) - 1.381).abs() < 1e-3);
        assert!((cma_radius(&Constellation::new(PointSet::qpsk()).unwrap()) - 1.0).abs() < 1e-6);
        // A unit-energy channel, the main tap first and real.
        let raw = [
            Complex32::new(1.0, 0.0),
            Complex32::new(0.3, 0.2),
            Complex32::new(-0.12, 0.0),
        ];
        let e: f32 = raw.iter().map(|z| z.norm_sqr()).sum();
        let h: Vec<Complex32> = raw.iter().map(|z| z / e.sqrt()).collect();
        let ebn0_db = 11.0;
        let ebn0 = sdrkit_algo_dsp::meter::db_to_power(ebn0_db);
        let n0 = 1.0 / (4.0 * ebn0);
        let mut rng = Rng::new(11);
        let (labels, sym) = random_symbols(&set, 400_000, &mut rng);
        let x = channel(&sym, &h, n0, &mut rng);
        let mut cfg = EqConfig::new(15, 1);
        cfg.mu = 5e-4;
        // The channel is minimum phase: its inverse needs taps on older samples, so the main tap is late in the window.
        cfg.centre = 11;
        cfg.dd_switch_mse = Some(0.04);
        let mut eq = AdaptiveEq::new(cfg, c.clone()).unwrap();
        let mut out = Vec::with_capacity(x.len());
        eq.process(&x[..200_000], &mut out);
        assert_eq!(eq.mode(), Mode::Dd, "mse {}", eq.mse());
        eq.set_mu(5e-5).unwrap();
        out.clear();
        eq.process(&x[200_000..], &mut out);
        // Delay and quarter-turn: whichever makes the most decisions right.
        let lag = cfg.taps - 1 - cfg.centre;
        let mut best = (usize::MAX, 0usize);
        for q in 0..4 {
            let r = Complex32::new([1.0, 0.0, -1.0, 0.0][q], [0.0, 1.0, 0.0, -1.0][q]);
            let mut errs = 0usize;
            for (k, y) in out.iter().enumerate().skip(1000) {
                let t = 200_000 + k - lag;
                errs += ((c.decide(y * r) as usize) ^ labels[t]).count_ones() as usize;
            }
            if errs < best.0 {
                best = (errs, q);
            }
        }
        let bits = (out.len() - 1000) * 4;
        let ber = best.0 as f64 / bits as f64;
        // The best a linear equaliser of this length can do: the unbiased output SNR of the Wiener solution, 1 / Jmin - 1.
        let (_, jmin, _) = wiener(&h, n0, cfg.taps, lag);
        let sinr = 1.0 / jmin - 1.0;
        let theory = crate::theory::ber_square_qam(16, sinr / 4.0);
        assert!(
            ber > 0.7 * theory && ber < 1.5 * theory,
            "BER {ber}, MMSE bound {theory} (AWGN {})",
            crate::theory::ber_square_qam(16, ebn0)
        );
    }

    #[test]
    fn fractionally_spaced_cma_and_a_trained_dfe() {
        // T/2: 16-QAM shaped by RRC 0.35 at 2 samples per symbol (sdrkit-algo-sim), a half-symbol echo, the matched filter, then a
        // 28-tap T/2 equaliser in blind mode.
        use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
        use sdrkit_algo_sim::modulate::Linear;
        let set = PointSet::qam(16, QamMap::DVB).unwrap();
        let c = Constellation::new(set.clone()).unwrap();
        let mut rng = Rng::new(3);
        let (labels, sym) = random_symbols(&set, 120_000, &mut rng);
        let mut m = Linear::new(Shape::Rrc { beta: 0.35 }, 16.0, 2.0).unwrap();
        let mut tx = Vec::new();
        m.push(&sym, &mut tx);
        let echo = Complex32::new(0.0, 0.3);
        let rx: Vec<Complex32> = (0..tx.len())
            .map(|n| {
                tx[n]
                    + if n >= 1 {
                        tx[n - 1] * echo
                    } else {
                        Complex32::default()
                    }
                    + rng.complex_normal(1e-3)
            })
            .collect();
        let mf = taps(Shape::Rrc { beta: 0.35 }, 2.0, 16.0, Norm::Energy).unwrap();
        let y: Vec<Complex32> = (0..rx.len())
            .map(|n| {
                mf.iter()
                    .enumerate()
                    .filter(|(k, _)| n >= *k)
                    .map(|(k, &t)| rx[n - k] * t)
                    .sum::<Complex32>()
                    * std::f32::consts::FRAC_1_SQRT_2
            })
            .collect();
        let mut cfg = EqConfig::new(28, 2);
        cfg.mu = 2e-4;
        cfg.centre = 19;
        cfg.dd_switch_mse = Some(0.03);
        let mut eq = AdaptiveEq::new(cfg, c.clone()).unwrap();
        // Two alignments of the symbol instant: one of them is the right one.
        let mut ok = false;
        for skip in 0..2 {
            eq.reset();
            let mut out = Vec::new();
            eq.process(&y[skip..], &mut out);
            let tail = &out[out.len() - 5000..];
            // Decisions match the sent labels at some delay and quarter turn.
            for lag in 0..out.len().min(80) {
                for q in 0..4 {
                    let r = Complex32::new([1.0, 0.0, -1.0, 0.0][q], [0.0, 1.0, 0.0, -1.0][q]);
                    let start = out.len() - 5000;
                    if start < lag {
                        continue;
                    }
                    let wrong = tail
                        .iter()
                        .enumerate()
                        .filter(|(k, z)| c.decide(**z * r) as usize != labels[start + k - lag])
                        .count();
                    ok |= wrong == 0;
                }
            }
        }
        assert!(ok, "the T/2 equaliser did not open the eye");

        // A DFE trained on known symbols through a channel with a strong post-cursor echo.
        let set = PointSet::qpsk();
        let c = Constellation::new(set.clone()).unwrap();
        let (labels, sym) = random_symbols(&set, 40_000, &mut rng);
        let h = [
            Complex32::new(1.0, 0.0),
            Complex32::new(0.0, 0.0),
            Complex32::new(0.7, 0.4),
        ];
        let x = channel(&sym, &h, 0.01, &mut rng);
        // One feed-forward tap: the feedback alone must cancel the echo.
        let mut cfg = EqConfig::new(1, 1);
        cfg.centre = 0;
        cfg.feedback = 4;
        cfg.normalised = true;
        cfg.mu = 0.05;
        cfg.mode = Mode::Dd;
        cfg.dd_switch_mse = None;
        let mut eq = AdaptiveEq::new(cfg, c.clone()).unwrap();
        let mut errs = 0;
        for t in 0..x.len() {
            let known = (t < 2000).then_some(sym[t]);
            let y = eq.step(&x[t..t + 1], known).unwrap();
            if t > 5000 && c.decide(y) as usize != labels[t] {
                errs += 1;
            }
        }
        assert_eq!(errs, 0);
        let mut fb = [Complex32::default(); 4];
        eq.feedback(&mut fb).unwrap();
        // The feedback cancels the echo two symbols back.
        assert!((fb[1] + h[2]).norm() < 0.1, "{fb:?}");
    }

    #[test]
    fn least_squares_error_matches_its_covariance() {
        let mut rng = Rng::new(5);
        let taps = 5;
        let t: Vec<Complex32> = (0..26)
            .map(|_| PointSet::qpsk().points()[rng.below(4) as usize])
            .collect();
        let ls = LsChannel::new(&t, taps).unwrap();
        let h: Vec<Complex32> = (0..taps).map(|_| rng.complex_normal(0.3)).collect();
        let n0 = 0.05;
        let trials = 20_000;
        let (mut mse, mut nsum) = (0f64, 0f64);
        let mut est = vec![Complex32::default(); taps];
        let mut rx = vec![Complex32::default(); ls.rows()];
        for _ in 0..trials {
            for (r, v) in rx.iter_mut().enumerate() {
                *v = (0..taps)
                    .map(|k| h[k] * t[r + taps - 1 - k])
                    .sum::<Complex32>()
                    + rng.complex_normal(n0);
            }
            nsum += ls.estimate(&rx, &mut est).unwrap() as f64;
            mse += est
                .iter()
                .zip(&h)
                .map(|(a, b)| (a - b).norm_sqr() as f64)
                .sum::<f64>();
        }
        let want = n0 * ls.noise_gain();
        let got = mse / trials as f64;
        assert!((got / want - 1.0).abs() < 0.05, "{got} vs {want}");
        assert!((nsum / trials as f64 / n0 - 1.0).abs() < 0.05);
        // Errors, never panics.
        assert!(LsChannel::new(&t[..6], taps).is_err());
        assert!(LsChannel::new(&[Complex32::new(1.0, 0.0); 26], taps).is_err());
        assert!(ls.estimate(&rx[..3], &mut est).is_err());
        assert!(ls.estimate(&rx, &mut est[..2]).is_err());
    }

    #[test]
    fn bad_settings_and_input_are_refused_or_survived() {
        let c = || Constellation::new(PointSet::qpsk()).unwrap();
        for f in [
            |cfg: &mut EqConfig| cfg.taps = 0,
            |cfg: &mut EqConfig| cfg.centre = 99,
            |cfg: &mut EqConfig| cfg.sps = 3,
            |cfg: &mut EqConfig| cfg.mu = f32::NAN,
            |cfg: &mut EqConfig| cfg.feedback = 1000,
            |cfg: &mut EqConfig| cfg.mse_alpha = 0.0,
        ] {
            let mut cfg = EqConfig::new(9, 1);
            f(&mut cfg);
            assert!(AdaptiveEq::new(cfg, c()).is_err());
        }
        let mut cfg = EqConfig::new(9, 2);
        cfg.mu = 10.0;
        let mut eq = AdaptiveEq::new(cfg, c()).unwrap();
        assert!(eq.step(&[Complex32::default()], None).is_err());
        let mut out = Vec::new();
        // Huge input makes the taps blow up; the equaliser resets instead of producing NaN.
        let big: Vec<Complex32> = (0..400)
            .map(|k| Complex32::new(if k % 3 == 0 { 300.0 } else { f32::NAN }, 1.0))
            .collect();
        eq.process(&big, &mut out);
        assert!(out.iter().all(|z| z.re.is_finite() && z.im.is_finite()));
        assert!(eq.resets() > 0);
    }

    #[test]
    fn normalised_input_and_an_early_divergence_reset() {
        // A constant-envelope signal (an FM channel) through an echo of -6 dB 3 samples late, at a level of 300: the blind CMA
        // equaliser with normalised input restores the constant envelope with the step size of a unit-level signal.
        let mut rng = Rng::new(17);
        let mut ph = 0.0f32;
        let s: Vec<Complex32> = (0..60_000)
            .map(|_| {
                ph += 0.6 * rng.normal_f32().clamp(-2.0, 2.0);
                Complex32::from_polar(1.0, ph)
            })
            .collect();
        let x: Vec<Complex32> = (0..s.len())
            .map(|n| {
                (s[n]
                    + if n >= 3 {
                        s[n - 3] * Complex32::new(0.0, 0.5)
                    } else {
                        Complex32::default()
                    })
                    * 300.0
            })
            .collect();
        let c = Constellation::new(sdrkit_algo_dsp::constellation::PointSet::psk(8, 0.0).unwrap())
            .unwrap();
        let mut cfg = EqConfig::new(16, 1);
        cfg.centre = 3;
        cfg.mu = 2e-4;
        cfg.dd_switch_mse = None;
        let ripple = |y: &[Complex32]| {
            let m: f32 = y.iter().map(|v| v.norm()).sum::<f32>() / y.len() as f32;
            (y.iter().map(|v| (v.norm() - m).powi(2)).sum::<f32>() / y.len() as f32).sqrt() / m
        };
        let mut eq = AdaptiveEq::new(cfg, c.clone()).unwrap();
        eq.set_input_normalisation(Some(0.002)).unwrap();
        eq.set_divergence_limit(16.0).unwrap();
        let mut out = Vec::new();
        eq.process(&x, &mut out);
        let before = ripple(&x[1000..]);
        let after = ripple(&out[out.len() - 5000..]);
        assert!(after < before / 2.0, "envelope ripple {before} -> {after}");
        assert!((eq.input_power() - 300.0f32.powi(2) * 1.25).abs() < 0.2 * 300.0f32.powi(2));
        // Unnormalised, the same step size at this level diverges at once and keeps resetting.
        let mut raw = AdaptiveEq::new(cfg, c.clone()).unwrap();
        raw.set_divergence_limit(16.0 * 300.0f32.powi(2)).unwrap();
        let mut out2 = Vec::new();
        raw.process(&x[..2000], &mut out2);
        assert!(raw.resets() > 0);
        // A reset outputs the centre tap's input for that symbol, not zero.
        let mut d = AdaptiveEq::new(cfg, c).unwrap();
        d.set_divergence_limit(1e-6).unwrap();
        let mut o = Vec::new();
        d.process(&[Complex32::new(0.5, 0.0); 20], &mut o);
        assert!(d.resets() > 0);
        assert_eq!(o[19], Complex32::new(0.5, 0.0));
        assert!(d.set_input_normalisation(Some(0.0)).is_err());
        assert!(d.set_divergence_limit(0.0).is_err());
    }

    /// The 728 known levels of an 8-VSB field sync: segment sync, PN511, three PN63, and the 24 symbols that name the VSB type.
    fn field_sync() -> Vec<f32> {
        use sdrkit_util_bits::seq::{ATSC_PN511, ATSC_PN63};
        let mut bits = vec![0u8; 728];
        bits[..4].copy_from_slice(&[1, 0, 0, 1]);
        let mut pn = ATSC_PN511;
        pn.fill_bits(&mut bits[4..515]);
        for k in 0..3 {
            let mut p = ATSC_PN63;
            p.fill_bits(&mut bits[515 + 63 * k..578 + 63 * k]);
        }
        for (i, b) in bits[704..].iter_mut().enumerate() {
            *b = (i % 4 < 2) as u8;
        }
        bits.iter()
            .map(|&b| if b == 1 { 5.0 } else { -5.0 })
            .collect()
    }

    /// 1600 symbols, the field sync, then data symbols to `len`; the data are 8-level.
    fn vsb_job(len: usize, rng: &mut Rng) -> (Vec<f32>, usize) {
        let levels = [-7.0f32, -5.0, -3.0, -1.0, 1.0, 3.0, 5.0, 7.0];
        let mut s: Vec<f32> = (0..len).map(|_| levels[rng.below(8) as usize]).collect();
        let f0 = 1600;
        s[f0..f0 + 728].copy_from_slice(&field_sync());
        (s, f0)
    }

    /// `r = s + a s(n + lag) + noise` (a negative lag: an earlier path).
    fn echo(s: &[f32], lag: i64, a: f32, noise: f64, rng: &mut Rng) -> Vec<f32> {
        (0..s.len() as i64)
            .map(|n| {
                let e = n - lag;
                let x = if (0..s.len() as i64).contains(&e) {
                    s[e as usize]
                } else {
                    0.0
                };
                s[n as usize] + a * x + (noise.sqrt() * rng.normal()) as f32
            })
            .collect()
    }

    /// The echo left in `u`: the projection of `u - s` on `s` delayed by `lag`, in dB of the strongest path, over `range`.
    fn residual_db(u: &[f32], s: &[f32], lag: i64, range: std::ops::Range<usize>) -> f64 {
        let (mut num, mut den) = (0f64, 0f64);
        for n in range {
            let e = n as i64 - lag;
            let x = s[e as usize] as f64;
            num += (u[n] - s[n]) as f64 * x;
            den += x * x;
        }
        20.0 * (num / den).abs().log10()
    }

    #[test]
    fn long_echo_cancellers_remove_the_echo() {
        // Pre-echoes of A/74: -30 us -7 dB, -15 us -5 dB, -10 us -3 dB (10.76 M symbols/s); post-echoes +20 / +40 us at -2 / -4 dB.
        // SNR 30 dB on the 8-level data (mean power 21).
        let mut rng = Rng::new(808);
        let mut scratch = Vec::new();
        let rule = EchoFitRule::default();
        let known = field_sync();
        for &(us, db) in &[
            (-30.0, -7.0),
            (-15.0, -5.0),
            (-10.0, -3.0),
            (20.0, -2.0),
            (40.0, -4.0),
        ] {
            let lag = (us * 10.762f64).round() as i64;
            let a = 10f32.powf(db as f32 / 20.0);
            let (s, f0) = vsb_job(1600 + 832 * 12, &mut rng);
            let r = echo(&s, lag, a, 21.0e-3, &mut rng);
            let q = vec![0f32; r.len()];
            let (side, path) = if lag < 0 {
                // The first look at 0.4 of the main path, then a second look at 0.2 beyond 24 feed-forward taps: a -7 dB path
                // correlates at about 0.45, give or take 0.04 of the data's own correlation, so the first look can miss it.
                let p = earliest_path(&r, &q, f0, &known, 6, 330, 0.4, &mut scratch)
                    .or_else(|| earliest_path(&r, &q, f0, &known, 26, 330, 0.2, &mut scratch));
                (EchoSide::Pre, p.unwrap())
            } else {
                (
                    EchoSide::Post,
                    latest_path(&r, &q, f0, &known, 118, 480, 0.2, &mut scratch).unwrap(),
                )
            };
            assert_eq!(path.lag, lag, "{us}");
            assert!((path.ratio - a as f64).abs() < 0.12, "{us}: {path:?}");
            let mut c =
                EchoCanceller::fit(side, &r, &q, f0, &known, lag.unsigned_abs() as usize, &rule)
                    .unwrap();
            assert!(
                (c.power() - (a * a) as f64).abs() < 0.03,
                "{us}: {}",
                c.power()
            );
            let mut u = vec![0f32; r.len()];
            let mut uq = vec![0f32; r.len()];
            assert!(c.apply(&r, &q, &mut u, &mut uq));
            assert!(
                uq.iter().all(|v| v.abs() < 1e-3),
                "no part in quadrature comes from none"
            );
            // Over the field and the data after it (the recursion settles from the far end for a pre-echo, from the
            // start for a post-echo).
            let range = if lag < 0 {
                1200..r.len() - 1000
            } else {
                2000..r.len()
            };
            let (before, after) = (
                residual_db(&r, &s, lag, range.clone()),
                residual_db(&u, &s, lag, range),
            );
            assert!(
                (before - db).abs() < 0.5 && after < -40.0,
                "{us} us {db} dB: {before:.1} -> {after:.1} dB"
            );
        }
    }

    /// The part in quadrature of real symbols on a vestigial sideband: the symbols through a Hilbert kernel `2 / (pi n)` on
    /// the odd `n`, tapered to nothing at 40 symbols.
    fn quadrature(s: &[f32]) -> Vec<f32> {
        let n = s.len() as i64;
        (0..n)
            .map(|t| {
                let mut v = 0f64;
                for k in (1..40i64).step_by(2) {
                    let w = 2.0 / (std::f64::consts::PI * k as f64)
                        * (0.5 + 0.5 * (std::f64::consts::PI * k as f64 / 40.0).cos());
                    let at = |i: i64| {
                        if (0..n).contains(&i) {
                            s[i as usize] as f64
                        } else {
                            0.0
                        }
                    };
                    v += w * (at(t - k) - at(t + k));
                }
                v as f32
            })
            .collect()
    }

    #[test]
    fn an_echo_of_any_carrier_phase_is_found_and_removed() {
        // The echoes of A/74 beyond the equalisers at 0, 90, 180 and 270 degrees from the strongest path, 30 dB. At a quarter
        // turn the levels of the echo reach the part in phase only through the quadrature pulse.
        let mut rng = Rng::new(31);
        let mut scratch = Vec::new();
        let rule = EchoFitRule::default();
        let known = field_sync();
        for &(us, db) in &[
            (-30.0, -7.0),
            (-15.0, -5.0),
            (-10.0, -3.0),
            (20.0, -2.0),
            (30.0, -3.0),
            (40.0, -4.0),
        ] {
            for phase in [0.0f64, 90.0, 180.0, 270.0] {
                let lag = (us * 10.762f64).round() as i64;
                let a = 10f32.powf(db as f32 / 20.0);
                let (s, f0) = vsb_job(1600 + 832 * 12, &mut rng);
                let sq = quadrature(&s);
                let (sin, cos) = phase.to_radians().sin_cos();
                let (sin, cos) = (sin as f32, cos as f32);
                let at = |x: &[f32], n: i64| {
                    if (0..x.len() as i64).contains(&n) {
                        x[n as usize]
                    } else {
                        0.0
                    }
                };
                // (s + j sq) + a e^(j phase) (s + j sq) delayed, plus noise in both parts.
                let mut r = Vec::with_capacity(s.len());
                let mut q = Vec::with_capacity(s.len());
                for n in 0..s.len() as i64 {
                    let (e, eq) = (at(&s, n - lag), at(&sq, n - lag));
                    r.push(s[n as usize] + a * (cos * e - sin * eq) + 0.145 * rng.normal_f32());
                    q.push(sq[n as usize] + a * (sin * e + cos * eq) + 0.145 * rng.normal_f32());
                }
                let (side, path) = if lag < 0 {
                    let p = earliest_path(&r, &q, f0, &known, 6, 330, 0.4, &mut scratch)
                        .or_else(|| earliest_path(&r, &q, f0, &known, 26, 330, 0.2, &mut scratch));
                    (EchoSide::Pre, p)
                } else {
                    (
                        EchoSide::Post,
                        latest_path(&r, &q, f0, &known, 118, 480, 0.2, &mut scratch),
                    )
                };
                let path = path.unwrap_or_else(|| panic!("{us} us at {phase}: no path"));
                assert_eq!(path.lag, lag, "{us} us at {phase}");
                let mut c = EchoCanceller::fit(
                    side,
                    &r,
                    &q,
                    f0,
                    &known,
                    lag.unsigned_abs() as usize,
                    &rule,
                )
                .unwrap();
                assert!(
                    (c.power() - (a * a) as f64).abs() < 0.05,
                    "{us} us at {phase}: {}",
                    c.power()
                );
                let mut u = vec![0f32; r.len()];
                let mut uq = vec![0f32; r.len()];
                assert!(c.apply(&r, &q, &mut u, &mut uq));
                // What is left beside the strongest path and the noise, against the symbols (mean square 21).
                let range = if lag < 0 {
                    1200..r.len() - 1000
                } else {
                    2000..r.len()
                };
                let left: f64 = range
                    .clone()
                    .map(|n| f64::from(u[n] - s[n]).powi(2))
                    .sum::<f64>()
                    / range.len() as f64;
                let db_left = 10.0 * (left / 21.0).log10();
                assert!(
                    db_left < -24.0,
                    "{us} us {db} dB at {phase}: {db_left:.1} dB left"
                );
            }
        }
    }

    #[test]
    fn echo_recursion_matches_a_direct_sum() {
        // The recursion on the shared FIR (NEON on 64-bit ARM) against the same recursion summed directly in f64.
        let mut rng = Rng::new(5);
        let known = field_sync();
        let (s, f0) = vsb_job(1600 + 832 * 3, &mut rng);
        for (side, lag) in [(EchoSide::Pre, -150i64), (EchoSide::Post, 200)] {
            let r = echo(&s, lag, 0.5, 1e-2, &mut rng);
            let q = vec![0f32; r.len()];
            let d = lag.unsigned_abs() as usize;
            let mut c =
                EchoCanceller::fit(side, &r, &q, f0, &known, d, &EchoFitRule::default()).unwrap();
            let mut u = vec![0f32; r.len()];
            let mut uq = vec![0f32; r.len()];
            assert!(c.apply(&r, &q, &mut u, &mut uq));
            assert!(c.turn().0 > 0.999, "{:?}", c.turn());
            let k: Vec<f64> = c.taps().iter().map(|&v| v as f64).collect();
            let n = r.len() as i64;
            let mut w = vec![0f64; r.len()];
            let order: Vec<i64> = if side == EchoSide::Pre {
                (0..n).rev().collect()
            } else {
                (0..n).collect()
            };
            for m in order {
                let mut acc = 0.0;
                for (j, kj) in k.iter().enumerate() {
                    let i = if side == EchoSide::Pre {
                        m + d as i64 + j as i64 - 12
                    } else {
                        m - d as i64 + j as i64 - 12
                    };
                    if (0..n).contains(&i) {
                        acc += kj * w[i as usize];
                    }
                }
                w[m as usize] = r[m as usize] as f64 - acc;
            }
            let worst = u
                .iter()
                .zip(&w)
                .map(|(a, b)| (*a as f64 - b).abs())
                .fold(0.0, f64::max);
            assert!(worst < 1e-3, "{side:?}: {worst}");
        }
    }

    #[test]
    fn echo_fits_refuse_what_they_cannot_cancel() {
        let mut rng = Rng::new(9);
        let known = field_sync();
        let rule = EchoFitRule::default();
        let (s, f0) = vsb_job(1600 + 832 * 4, &mut rng);
        // A pre-echo stronger than the main path, a post-echo under -26 dB, a delay the recursion cannot take.
        let q = vec![0f32; s.len()];
        let r = echo(&s, -200, 1.2, 1e-3, &mut rng);
        assert!(matches!(
            EchoCanceller::fit(EchoSide::Pre, &r, &q, f0, &known, 200, &rule),
            Err(EchoRefusal::TooStrong(_))
        ));
        let r = echo(&s, 300, 0.03, 1e-3, &mut rng);
        assert!(matches!(
            EchoCanceller::fit(EchoSide::Post, &r, &q, f0, &known, 300, &rule),
            Err(EchoRefusal::TooWeak(_))
        ));
        assert_eq!(
            EchoCanceller::fit(EchoSide::Pre, &r, &q, f0, &known, 10, &rule).unwrap_err(),
            EchoRefusal::Delay
        );
        assert_eq!(
            EchoCanceller::fit(EchoSide::Pre, &r, &q, f0, &known, 600, &rule).unwrap_err(),
            EchoRefusal::Delay
        );
        let cut = f0 + 100;
        assert_eq!(
            EchoCanceller::fit(EchoSide::Post, &r[..cut], &q[..cut], f0, &known, 300, &rule)
                .unwrap_err(),
            EchoRefusal::Input
        );
        assert_eq!(
            EchoCanceller::fit(EchoSide::Post, &r, &q[..cut], f0, &known, 300, &rule).unwrap_err(),
            EchoRefusal::Input
        );
        // No path: nothing is found.
        let mut scratch = Vec::new();
        let r = echo(&s, 0, 0.0, 1e-3, &mut rng);
        assert!(earliest_path(&r, &q, f0, &known, 6, 330, 0.4, &mut scratch).is_none());
        assert!(latest_path(&r, &q, f0, &known, 118, 480, 0.2, &mut scratch).is_none());
    }

    /// The nearest of the 8 levels +-1 .. +-7.
    fn slice8(y: f32) -> f32 {
        ((y + 7.0) * 0.5).round().clamp(0.0, 7.0) * 2.0 - 7.0
    }

    /// 8-level symbols through paths `(lag, gain)` beside the direct one, with noise of power `noise`. The echo gains move
    /// by `drift` of themselves over the run.
    fn dfe_job(
        len: usize,
        paths: &[(i64, f32)],
        noise: f64,
        drift: f32,
        rng: &mut Rng,
    ) -> (Vec<f32>, Vec<f32>) {
        let s: Vec<f32> = (0..len)
            .map(|_| slice8(16.0 * rng.uniform() as f32 - 8.0))
            .collect();
        let x = (0..len as i64)
            .map(|n| {
                let g = 1.0 + drift * n as f32 / len as f32;
                let mut v = s[n as usize] + (noise.sqrt() * rng.normal()) as f32;
                for &(lag, a) in paths {
                    if (0..len as i64).contains(&(n - lag)) {
                        v += g * a * s[(n - lag) as usize];
                    }
                }
                v
            })
            .collect();
        (s, x)
    }

    /// Runs `e` over `range` of `x` with the first `train` symbols of `s` known, and returns the output SNR against `s` in dB
    /// and the wrong decisions.
    fn dfe_run(
        e: &mut RealDfe,
        s: &[f32],
        x: &[f32],
        train: usize,
        range: std::ops::Range<usize>,
    ) -> (f64, usize) {
        let known: Vec<f32> = (0..s.len())
            .map(|t| if t < train { s[t] } else { f32::NAN })
            .collect();
        let mut y = vec![0f32; s.len()];
        let mut d = s.to_vec();
        e.run(x, &known, &mut slice8, range.clone(), &mut y, &mut d);
        let e2: f64 = range.clone().map(|t| f64::from(y[t] - s[t]).powi(2)).sum();
        let wrong = range.clone().filter(|&t| d[t] != s[t]).count();
        (10.0 * (21.0 * range.len() as f64 / e2).log10(), wrong)
    }

    #[test]
    fn decision_feedback_takes_a_strong_post_echo_without_noise_gain() {
        // A post-echo of -0.5 dB 54 symbols late and a pre-echo of -10 dB 5 symbols early, 28 dB of noise under the direct path.
        let mut rng = Rng::new(11);
        let noise = 21.0 / 10f64.powf(2.8);
        let (s, x) = dfe_job(60_000, &[(54, 0.944), (-5, 0.316)], noise, 0.0, &mut rng);
        let train = 2000;
        // Feed-forward alone: 128 taps cannot undo an echo that strong.
        let mut plain = RealDfe::new(128, 103, 0).unwrap();
        plain.begin();
        plain.add_rows(&x, &s, 0..train);
        assert!(plain.solve(1e-6));
        let (snr_plain, _) = dfe_run(&mut plain, &s, &x, train, train..60_000);
        // With feedback, trained on the same symbols.
        let mut dfe = RealDfe::new(65, 24, 128).unwrap();
        assert_eq!((dfe.before(), dfe.after()), (24, 40));
        dfe.begin();
        dfe.add_rows(&x, &s, 0..train);
        assert_eq!(dfe.rows(), train as u64 - 128);
        assert!(dfe.solve(1e-6));
        assert!(
            (dfe.feedback()[53] + 0.944).abs() < 0.1,
            "{}",
            dfe.feedback()[53]
        );
        let (e2, n) = dfe.error(&x, &s, 200..train);
        let fit = 10.0 * (21.0 * n as f64 / e2).log10();
        let (snr, wrong) = dfe_run(&mut dfe, &s, &x, train, train..60_000);
        assert!(snr_plain < 12.0, "{snr_plain}");
        assert!(
            fit > 25.0 && snr > 24.0 && wrong < 60,
            "fit {fit} run {snr} wrong {wrong}"
        );
        // The long run on all decided symbols gives the same taps with less fit noise.
        let before = snr;
        dfe.begin();
        dfe.add_run(&x, &s, 0..60_000);
        assert!(dfe.solve(1e-6));
        let (snr, wrong) = dfe_run(&mut dfe, &s, &x, train, train..60_000);
        assert!(
            snr > before + 0.3 && snr > 26.0 && wrong < 30,
            "{snr} after {before}, {wrong} wrong"
        );
        // Kept taps come back.
        let (ff, fb, bias) = (
            dfe.feed_forward().to_vec(),
            dfe.feedback().to_vec(),
            dfe.bias(),
        );
        let mut other = RealDfe::new(65, 24, 128).unwrap();
        other.set_taps(&ff, &fb, bias).unwrap();
        assert_eq!(dfe_run(&mut other, &s, &x, train, train..60_000).1, wrong);
        assert!(other.set_taps(&ff[1..], &fb, bias).is_err());
        assert!(RealDfe::new(0, 0, 4).is_err() && RealDfe::new(8, 8, 4).is_err());
    }

    /// A decider that takes the nearest level, wrong by one level on every 50th symbol, and puts that symbol right when it
    /// decides the next one.
    struct Late<'a> {
        sent: &'a [f32],
        correct: bool,
    }

    impl DfeDecider for Late<'_> {
        fn decide(&mut self, t: usize, y: f32, d: &mut [f32]) {
            d[t] = slice8(y);
            if t.is_multiple_of(50) {
                d[t] = if self.sent[t] < 7.0 {
                    self.sent[t] + 2.0
                } else {
                    5.0
                };
            }
            if self.correct && t % 50 == 1 {
                d[t - 1] = self.sent[t - 1];
            }
        }
    }

    #[test]
    fn the_feedback_reads_the_symbols_a_decider_corrected() {
        // A post-echo of -1 dB 54 symbols late, 30 dB. A decision wrong by one level comes back through the feedback tap as
        // 1.8 and makes the next decision wrong; a decider that corrects it one symbol later leaves the output clean.
        let mut rng = Rng::new(15);
        let noise = 21.0 / 10f64.powf(3.0);
        let len = 40_000;
        let (s, x) = dfe_job(len, &[(54, 0.891)], noise, 0.0, &mut rng);
        let train = 2000;
        let mut dfe = RealDfe::new(65, 24, 128).unwrap();
        dfe.begin();
        dfe.add_rows(&x, &s, 0..train);
        assert!(dfe.solve(1e-6));
        let known: Vec<f32> = (0..len)
            .map(|t| if t < train { s[t] } else { f32::NAN })
            .collect();
        let mut snr = [0f64; 2];
        for (k, correct) in [false, true].into_iter().enumerate() {
            let mut y = vec![0f32; len];
            let mut d = s.clone();
            let mut decider = Late { sent: &s, correct };
            dfe.run(&x, &known, &mut decider, train..len, &mut y, &mut d);
            let e2: f64 = (train..len).map(|t| f64::from(y[t] - s[t]).powi(2)).sum();
            snr[k] = 10.0 * (21.0 * (len - train) as f64 / e2).log10();
            if correct {
                assert!(
                    (train..len).all(|t| d[t] == s[t]),
                    "every symbol ends right"
                );
            }
        }
        assert!(snr[0] < 20.0 && snr[1] > 27.0, "{snr:?}");
    }

    #[test]
    fn tracking_follows_an_echo_that_moves() {
        // The echoes grow by 40 % over 300 segments; the taps are fitted on the first 3000 symbols.
        let mut rng = Rng::new(12);
        let noise = 21.0 / 10f64.powf(2.6);
        let len = 300 * 832;
        let (s, x) = dfe_job(len, &[(20, 0.5), (-3, 0.2)], noise, 0.4, &mut rng);
        let train = 3000;
        let last = len - 40 * 832..len - 832;
        let mut snr = [0f64; 2];
        for (k, track) in [None, Some(DfeTracking::default())].iter().enumerate() {
            let mut dfe = RealDfe::new(65, 24, 128).unwrap();
            dfe.begin();
            dfe.add_rows(&x, &s, 0..train);
            assert!(dfe.solve(1e-6));
            let known: Vec<f32> = (0..len)
                .map(|t| if t < train { s[t] } else { f32::NAN })
                .collect();
            let mut y = vec![0f32; len];
            let mut d = s.clone();
            dfe.set_tracking(*track);
            dfe.run(&x, &known, &mut slice8, train..len, &mut y, &mut d);
            let e2: f64 = last.clone().map(|t| f64::from(y[t] - s[t]).powi(2)).sum();
            snr[k] = 10.0 * (21.0 * last.len() as f64 / e2).log10();
        }
        assert!(
            snr[0] < 17.0 && snr[1] > 22.0,
            "fixed {} tracking {}",
            snr[0],
            snr[1]
        );
    }
}
