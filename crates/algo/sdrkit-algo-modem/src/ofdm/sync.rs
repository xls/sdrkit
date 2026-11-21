//! Time and frequency acquisition: the lag correlator (cyclic prefix, repeated preamble halves, short training symbols) and the
//! integer carrier offset search over known cells.

use crate::{Complex32, ModemError, Param};
use num_complex::Complex64;
use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::nco::Nco;

/// A peak found by [`LagCorrelator`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LagPeak {
    /// The absolute index (samples since the correlator started) of the **newest** sample in the window at the peak: for a cyclic
    /// prefix correlator with `lag = nfft` and `window = cp`, the last sample of the symbol, so the symbol's useful part started
    /// `nfft - 1` samples earlier.
    pub end: u64,
    /// The normalised metric at the peak, 0 to 1 ([`LagCorrelator`]).
    pub metric: f32,
    /// The fractional frequency offset in cycles per sample, from the phase of the correlation: `arg(P) / (2 pi lag)`. Unambiguous
    /// within `+-1 / (2 lag)`.
    pub freq: f64,
}

/// A streaming lag correlator: `P(n) = sum over the last `window` samples of x[i] conj(x[i - lag])`, with the energies of both
/// windows, giving the normalised metric `|P| / sqrt(E0 E1)` (1 for an exact repetition, near 0 for noise) and the frequency
/// offset from `arg P` (J.-J. van de Beek, M. Sandell and P. O. Borjesson, IEEE Trans. Signal Process. 45(7), 1997; T. M. Schmidl and
/// D. C. Cox, IEEE Trans. Commun. 45(12), 1997, with `lag = window = N/2`).
///
/// The sums run in `f64`, are updated in O(1) per sample, and are re-summed exactly every `4 window` samples so they cannot drift.
/// Peaks are reported with hysteresis: the metric must rise above `on` to open a peak, the largest value is kept, and the peak is
/// reported when the metric falls below `off`.
///
/// ```
/// use sdrkit_algo_modem::ofdm::LagCorrelator;
/// use sdrkit_algo_modem::Complex32;
/// // A signal that repeats every 64 samples (a 16-sample prefix of a 64-point symbol), in silence.
/// let sym: Vec<Complex32> = (0..64).map(|i| Complex32::new(((i * 37) % 11) as f32 - 5.0, ((i * 13) % 7) as f32 - 3.0)).collect();
/// let mut x = vec![Complex32::default(); 100];
/// x.extend_from_slice(&sym[48..]);
/// x.extend_from_slice(&sym);
/// x.extend(vec![Complex32::default(); 100]);
/// let mut c = LagCorrelator::new(64, 16, 0.8, 0.5).unwrap();
/// let mut peaks = Vec::new();
/// c.process(&x, |p| peaks.push(p));
/// assert_eq!(peaks.len(), 1);
/// assert_eq!(peaks[0].end, 100 + 16 + 64 - 1);
/// ```
#[derive(Clone, Debug)]
pub struct LagCorrelator {
    lag: usize,
    window: usize,
    ring: Vec<Complex32>,
    pos: usize,
    seen: u64,
    p: (f64, f64),
    e0: f64,
    e1: f64,
    since_resum: usize,
    on: f32,
    off: f32,
    open: bool,
    best: LagPeak,
    shift: Option<Shift>,
}

/// The rotation of a [`LagCorrelator`] whose second copy is shifted in frequency.
#[derive(Clone, Debug)]
struct Shift {
    /// Cycles per sample of the newer copy against the older.
    freq: f64,
    /// `e^(-j 2 pi freq n)` for the next sample.
    nco: Nco,
    /// The phasor applied to the newest product.
    last: Complex32,
    /// `e^(j 2 pi freq window)`: the phasor of the product leaving the window, relative to the newest.
    leave: Complex32,
    /// `e^(j 2 pi freq)` in `f64`, for the exact re-sum.
    step: Complex64,
}

impl LagCorrelator {
    /// A correlator of `lag` samples over `window` samples, opening a peak above `on` and closing it below `off` (`0 <= off <= on
    /// <= 1`).
    pub fn new(lag: usize, window: usize, on: f32, off: f32) -> Result<Self, ModemError> {
        if lag == 0 {
            return Param::Lag.at_least(1.0, 0.0);
        }
        if window == 0 {
            return Param::Window.at_least(1.0, 0.0);
        }
        Param::OpenLevel.check_in(on as f64, 0.0, 1.0)?;
        Param::CloseLevel.check_in(off as f64, 0.0, on as f64)?;
        Ok(LagCorrelator {
            lag,
            window,
            ring: vec![Complex32::default(); lag + window],
            pos: 0,
            seen: 0,
            p: (0.0, 0.0),
            e0: 0.0,
            e1: 0.0,
            since_resum: 0,
            on,
            off,
            open: false,
            best: LagPeak {
                end: 0,
                metric: 0.0,
                freq: 0.0,
            },
            shift: None,
        })
    }

    /// A correlator whose newer copy is the older one shifted up by `shift` cycles per sample: each product `x[i] conj(x[i - lag])`
    /// is turned back by `e^(-j 2 pi shift i)` before it is summed. The DVB-T2 P1 symbol has this structure (its C and B parts are
    /// the A part shifted by one carrier spacing of the 1K FFT, ETSI EN 302 755 clause 7.2.2). The metric is as for
    /// [`LagCorrelator::new`]. The phase of the correlation is that of the plain correlator plus a constant, so
    /// [`freq_of`](Self::freq_of) still returns the carrier offset modulo `1 / lag`. Errors as [`LagCorrelator::new`] does, and for
    /// a shift that is not finite.
    pub fn with_shift(
        lag: usize,
        window: usize,
        shift: f64,
        on: f32,
        off: f32,
    ) -> Result<Self, ModemError> {
        if !shift.is_finite() {
            return Param::Shift.not_finite();
        }
        let mut c = Self::new(lag, window, on, off)?;
        let leave = (std::f64::consts::TAU * (shift * window as f64).rem_euclid(1.0)).sin_cos();
        let step = (std::f64::consts::TAU * shift.rem_euclid(1.0)).sin_cos();
        c.shift = Some(Shift {
            freq: shift,
            nco: Nco::new(1.0, -shift),
            last: Complex32::new(1.0, 0.0),
            leave: Complex32::new(leave.1 as f32, leave.0 as f32),
            step: Complex64::new(step.1, step.0),
        });
        Ok(c)
    }

    /// The frequency shift of the newer copy, cycles per sample (0 for a plain correlator).
    pub fn shift(&self) -> f64 {
        self.shift.as_ref().map_or(0.0, |s| s.freq)
    }

    /// The energies of the two windows after the last sample: `(newest window, the window lag samples earlier)`.
    pub fn energies(&self) -> (f64, f64) {
        (self.e0.max(0.0), self.e1.max(0.0))
    }

    /// The correlation `P` after the last sample.
    pub fn correlation(&self) -> Complex32 {
        Complex32::new(self.p.0 as f32, self.p.1 as f32)
    }

    /// The lag.
    pub fn lag(&self) -> usize {
        self.lag
    }

    /// The window.
    pub fn window(&self) -> usize {
        self.window
    }

    /// Forgets everything.
    pub fn reset(&mut self) {
        self.ring.fill(Complex32::default());
        self.pos = 0;
        self.seen = 0;
        self.p = (0.0, 0.0);
        self.e0 = 0.0;
        self.e1 = 0.0;
        self.since_resum = 0;
        self.open = false;
        if let Some(sh) = &mut self.shift {
            sh.nco = Nco::new(1.0, -sh.freq);
            sh.last = Complex32::new(1.0, 0.0);
        }
    }

    /// Samples taken so far.
    pub fn samples(&self) -> u64 {
        self.seen
    }

    #[inline]
    fn at(&self, back: usize) -> Complex32 {
        // back = 0 is the newest sample.
        let n = self.ring.len();
        self.ring[(self.pos + n - 1 - back) % n]
    }

    fn resum(&mut self) {
        let (mut pr, mut pi, mut e0, mut e1) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        // With a shift, the product `back` samples before the newest is turned by `last e^(j 2 pi shift back)`.
        let (mut r, step) = match &self.shift {
            Some(sh) => (
                Complex64::new(sh.last.re as f64, sh.last.im as f64),
                sh.step,
            ),
            None => (Complex64::new(1.0, 0.0), Complex64::new(1.0, 0.0)),
        };
        for i in 0..self.window {
            let a = self.at(i);
            let b = self.at(i + self.lag);
            let (xr, xi) = (
                (a.re * b.re + a.im * b.im) as f64,
                (a.im * b.re - a.re * b.im) as f64,
            );
            pr += xr * r.re - xi * r.im;
            pi += xr * r.im + xi * r.re;
            r *= step;
            e0 += a.norm_sqr() as f64;
            e1 += b.norm_sqr() as f64;
        }
        self.p = (pr, pi);
        self.e0 = e0;
        self.e1 = e1;
        self.since_resum = 0;
    }

    /// Takes one sample; returns the metric and the correlation `P` after it.
    #[inline]
    pub fn push(&mut self, x: Complex32) -> (f32, Complex32) {
        let x = if x.re.is_finite() && x.im.is_finite() {
            x
        } else {
            Complex32::default()
        };
        // The sample leaving the newest window and its partner leaving the delayed one.
        let out_a = self.at(self.window - 1);
        let out_b = self.at(self.window - 1 + self.lag);
        let n = self.ring.len();
        self.ring[self.pos] = x;
        self.pos = (self.pos + 1) % n;
        self.seen += 1;
        let in_b = self.at(self.lag);
        let mut add = x * in_b.conj();
        let mut sub = out_a * out_b.conj();
        if let Some(sh) = &mut self.shift {
            let r = sh.nco.next_sample();
            sh.last = r;
            add *= r;
            sub *= r * sh.leave;
        }
        self.p.0 += (add.re - sub.re) as f64;
        self.p.1 += (add.im - sub.im) as f64;
        self.e0 += (x.norm_sqr() - out_a.norm_sqr()) as f64;
        self.e1 += (in_b.norm_sqr() - out_b.norm_sqr()) as f64;
        self.since_resum += 1;
        if self.since_resum >= 4 * self.window {
            self.resum();
        }
        let d = (self.e0.max(0.0) * self.e1.max(0.0)).sqrt();
        let m = if d > 1e-30 && self.seen >= n as u64 {
            ((self.p.0 * self.p.0 + self.p.1 * self.p.1).sqrt() / d).min(1.0) as f32
        } else {
            0.0
        };
        (m, Complex32::new(self.p.0 as f32, self.p.1 as f32))
    }

    /// Returns the frequency (cycles per sample) of a correlation `p` at this lag.
    #[inline]
    pub fn freq_of(&self, p: Complex32) -> f64 {
        approx::atan2(p.im, p.re) as f64 / (std::f64::consts::TAU * self.lag as f64)
    }

    /// Runs over `x`, calling `on_peak` for every peak closed (module documentation of [`LagCorrelator`]).
    pub fn process(&mut self, x: &[Complex32], mut on_peak: impl FnMut(LagPeak)) {
        for &v in x {
            let (m, p) = self.push(v);
            if !self.open {
                if m >= self.on {
                    self.open = true;
                    self.best = LagPeak {
                        end: self.seen - 1,
                        metric: m,
                        freq: self.freq_of(p),
                    };
                }
            } else {
                if m > self.best.metric {
                    self.best = LagPeak {
                        end: self.seen - 1,
                        metric: m,
                        freq: self.freq_of(p),
                    };
                }
                if m < self.off {
                    self.open = false;
                    on_peak(self.best);
                }
            }
        }
    }

    /// Writes the metric of every sample of `x` into `out` (`out.len() >= x.len()`), for a burst receiver that looks at the whole
    /// curve.
    pub fn metric_into(&mut self, x: &[Complex32], out: &mut [f32]) -> Result<(), ModemError> {
        if out.len() < x.len() {
            return Err(ModemError::OutputTooShort { need: x.len() });
        }
        for (o, &v) in out.iter_mut().zip(x) {
            *o = self.push(v).0;
        }
        Ok(())
    }
}

/// How [`integer_cfo`] scores a shift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CfoMetric {
    /// `|sum Y[k + s] conj(P_k)|`: needs the channel phase to be nearly the same on every known cell (flat channel, good timing).
    Coherent,
    /// `|sum Y[k_j + s] conj(Y[k_{j+1} + s]) conj(P_j) P_{j+1}|` over neighbouring known cells: removes the channel and a timing
    /// slope, as long as the channel changes little between neighbours.
    Differential,
    /// `sum |Y[k + s]|^2`: needs only the positions (boosted continual pilots of unknown phase).
    Power,
}

/// The integer carrier offset found by [`integer_cfo`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntegerCfo {
    /// The offset in carriers: the signal is `shift` bins above where the layout puts it.
    pub shift: i32,
    /// The best score over the second best (above 1; larger is surer).
    pub ratio: f32,
}

/// Finds the integer carrier offset of an FFT output `bins` (natural order, `nfft` values) from known cells `(carrier, value)`
/// (sorted by carrier for [`CfoMetric::Differential`]), trying shifts `-max_shift ..= max_shift`. Errors when `bins` is empty or no
/// cell is given.
pub fn integer_cfo(
    bins: &[Complex32],
    known: &[(i32, Complex32)],
    max_shift: i32,
    metric: CfoMetric,
) -> Result<IntegerCfo, ModemError> {
    let n = bins.len() as i32;
    if n == 0 {
        return Param::FftSize.at_least(1.0, 0.0);
    }
    if known.is_empty() {
        return Param::Pilots.at_least(1.0, 0.0);
    }
    if max_shift < 0 {
        return Param::Shift.at_least(0.0, max_shift as f64);
    }
    let at = |c: i32| bins[c.rem_euclid(n) as usize];
    let (mut best, mut second, mut shift) = (-1.0f32, -1.0f32, 0);
    for s in -max_shift..=max_shift {
        let score = match metric {
            CfoMetric::Coherent => known
                .iter()
                .fold(Complex32::default(), |a, &(c, p)| a + at(c + s) * p.conj())
                .norm(),
            CfoMetric::Power => known.iter().map(|&(c, _)| at(c + s).norm_sqr()).sum(),
            CfoMetric::Differential => known
                .windows(2)
                .fold(Complex32::default(), |a, w| {
                    a + at(w[0].0 + s) * at(w[1].0 + s).conj() * w[0].1.conj() * w[1].1
                })
                .norm(),
        };
        if score > best {
            second = best;
            best = score;
            shift = s;
        } else if score > second {
            second = score;
        }
    }
    Ok(IntegerCfo {
        shift,
        ratio: if second > 0.0 {
            best / second
        } else {
            f32::INFINITY
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    /// CP-OFDM-like symbols of white Gaussian samples (period `n + cp`), at a frequency offset, in noise.
    fn cp_signal(
        rng: &mut Rng,
        n: usize,
        cp: usize,
        symbols: usize,
        f: f64,
        noise: f64,
    ) -> Vec<Complex32> {
        let mut x = Vec::new();
        for _ in 0..symbols {
            let s: Vec<Complex32> = (0..n).map(|_| rng.complex_normal(1.0)).collect();
            x.extend_from_slice(&s[n - cp..]);
            x.extend_from_slice(&s);
        }
        for (i, v) in x.iter_mut().enumerate() {
            let ph = std::f64::consts::TAU * f * i as f64;
            *v = *v * Complex32::new(ph.cos() as f32, ph.sin() as f32) + rng.complex_normal(noise);
        }
        x
    }

    #[test]
    fn cyclic_prefix_peaks_and_frequency_error_follow_theory() {
        let (n, cp) = (256usize, 32usize);
        let snr = 10.0f64; // linear
        let f = 0.3 / n as f64;
        let mut rng = Rng::new(7);
        let mut errs = Vec::new();
        let mut c = LagCorrelator::new(n, cp, 0.7, 0.3).unwrap();
        for _ in 0..20 {
            let x = cp_signal(&mut rng, n, cp, 40, f, 1.0 / snr);
            c.reset();
            let mut peaks = Vec::new();
            c.process(&x, |p| peaks.push(p));
            assert!(peaks.len() >= 38, "{}", peaks.len());
            for p in &peaks {
                // The peak is at the end of a symbol (within a few samples: the metric is flat near its top).
                let k = (p.end + 1) as i64 % (n + cp) as i64;
                assert!(k <= 8 || k >= (n + cp) as i64 - 8, "end {}", p.end);
                errs.push(p.freq - f);
            }
        }
        // Theory: arg of a sum of W products r r* with |s|^2 = 1: variance (2/SNR + 1/SNR^2) / (2 W) rad^2.
        let var_th = (2.0 / snr + 1.0 / (snr * snr))
            / (2.0 * cp as f64)
            / (std::f64::consts::TAU * n as f64).powi(2);
        let mean = errs.iter().sum::<f64>() / errs.len() as f64;
        let var = errs.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / errs.len() as f64;
        // The peak position is chosen by the metric, which favours lucky noise a little: allow 35 %.
        assert!(
            (var / var_th - 1.0).abs() < 0.35,
            "var {var:e} theory {var_th:e}"
        );
        assert!(
            mean.abs() < 3.0 * (var_th / errs.len() as f64).sqrt() + 1e-6,
            "bias {mean:e}"
        );
    }

    #[test]
    fn running_sums_do_not_drift() {
        let mut rng = Rng::new(3);
        let x = cp_signal(&mut rng, 64, 16, 20_000, 0.001, 0.1);
        let mut c = LagCorrelator::new(64, 16, 0.9, 0.5).unwrap();
        let mut last = (0.0, Complex32::default());
        for &v in &x {
            let r = c.push(v);
            last = (r.0, r.1);
        }
        let p_run = last.1;
        c.resum();
        let p_exact = Complex32::new(c.p.0 as f32, c.p.1 as f32);
        assert!((p_run - p_exact).norm() < 1e-3 * (1.0 + p_exact.norm()));
        assert!(LagCorrelator::new(0, 4, 0.5, 0.2).is_err());
        assert!(LagCorrelator::new(4, 4, 0.5, 0.7).is_err());
    }

    #[test]
    fn integer_offsets_are_found_by_every_metric() {
        let n = 64usize;
        // Irregular positions, as continual pilots are placed, so no shift lines most of them up again.
        let known: Vec<(i32, Complex32)> = [-25, -19, -14, -6, -1, 3, 9, 12, 20, 26]
            .iter()
            .enumerate()
            .map(|(j, &c)| (c, Complex32::new(if j % 3 == 0 { 1.0 } else { -1.0 }, 0.0)))
            .collect();
        let mut rng = Rng::new(11);
        for shift in [-3, 0, 2, 5] {
            let mut bins = vec![Complex32::default(); n];
            for v in bins.iter_mut() {
                *v = rng.complex_normal(0.05);
            }
            // A channel with a timing slope: H_k = e^(j 0.2 k).
            for &(c, p) in &known {
                let ph = 0.2 * c as f64 + 1.0;
                bins[(c + shift).rem_euclid(n as i32) as usize] +=
                    p * Complex32::new(ph.cos() as f32, ph.sin() as f32);
            }
            for m in [CfoMetric::Differential, CfoMetric::Power] {
                let r = integer_cfo(&bins, &known, 6, m).unwrap();
                assert_eq!(r.shift, shift, "{m:?}");
                assert!(r.ratio > 1.5);
            }
        }
        // The coherent metric needs a flat channel.
        let mut bins = vec![Complex32::default(); n];
        for &(c, p) in &known {
            bins[(c + 1).rem_euclid(64) as usize] = p * Complex32::new(0.0, 1.0);
        }
        assert_eq!(
            integer_cfo(&bins, &known, 4, CfoMetric::Coherent)
                .unwrap()
                .shift,
            1
        );
        assert!(integer_cfo(&[], &known, 4, CfoMetric::Coherent).is_err());
    }

    #[test]
    fn window_energies_and_a_shifted_second_copy() {
        // A 256-sample block repeated, the second copy shifted up by one carrier of a 256-point FFT (the P1 structure).
        let lag = 256;
        let f = 1.0 / lag as f64;
        let a: Vec<Complex32> = (0..lag)
            .map(|i| Complex32::new(((i * 37) % 11) as f32 - 5.0, ((i * 13) % 7) as f32 - 3.0))
            .collect();
        let mut x = vec![Complex32::default(); 300];
        x.extend_from_slice(&a);
        let start = x.len();
        for (i, v) in a.iter().enumerate() {
            let ph = std::f64::consts::TAU * f * (start + i) as f64;
            x.push(v * Complex32::new(ph.cos() as f32, ph.sin() as f32));
        }
        x.extend(vec![Complex32::default(); 300]);
        // The plain correlator sees much less (the shift turns the products a whole turn over the window). The shifted one sees
        // the copy whole.
        let mut plain = LagCorrelator::new(lag, lag, 0.5, 0.3).unwrap();
        let mut shifted = LagCorrelator::with_shift(lag, lag, f, 0.5, 0.3).unwrap();
        let (mut mp, mut ms) = (0f32, 0f32);
        let mut at = 0;
        for (i, &v) in x.iter().enumerate() {
            mp = mp.max(plain.push(v).0);
            let m = shifted.push(v).0;
            if m > ms {
                ms = m;
                at = i;
            }
        }
        assert!(mp < 0.6, "{mp}");
        assert!(ms > 0.999, "{ms}");
        assert_eq!(at, start + lag - 1);
        assert_eq!(shifted.shift(), f);
        // Once both windows hold the copies, their energies are those of the block.
        let mut c = LagCorrelator::with_shift(lag, lag, f, 0.5, 0.3).unwrap();
        for &v in &x[..start + lag] {
            c.push(v);
        }
        let e: f64 = a.iter().map(|v| v.norm_sqr() as f64).sum();
        let (e0, e1) = c.energies();
        assert!((e0 - e).abs() < 1e-6 * e && (e1 - e).abs() < 1e-6 * e);
        assert!((c.correlation().norm() as f64 - e).abs() < 1e-3 * e);
        // The exact re-sum (every 4 windows) returns the same values as the running sums over a long run of noise.
        let mut r = sdrkit_algo_sim::rng::Rng::new(5);
        let noise: Vec<Complex32> = (0..20_000).map(|_| r.complex_normal(1.0)).collect();
        let mut c = LagCorrelator::with_shift(64, 48, 0.013, 0.5, 0.3).unwrap();
        for &v in &noise {
            c.push(v);
        }
        let p = c.correlation();
        // The product `back` samples before the newest is turned by e^(-j 2 pi f (n - back)).
        let n = noise.len() - 1;
        let mut want = num_complex::Complex64::default();
        for back in 0..48 {
            let i = n - back;
            let prod = noise[i] * noise[i - 64].conj();
            let ph = -std::f64::consts::TAU * ((0.013 * i as f64) % 1.0);
            want += num_complex::Complex64::new(prod.re as f64, prod.im as f64)
                * num_complex::Complex64::from_polar(1.0, ph);
        }
        assert!(
            (p.re as f64 - want.re).abs() < 1e-3 && (p.im as f64 - want.im).abs() < 1e-3,
            "{p} {want}"
        );
        assert!(LagCorrelator::with_shift(64, 16, f64::NAN, 0.5, 0.3).is_err());
    }
}
