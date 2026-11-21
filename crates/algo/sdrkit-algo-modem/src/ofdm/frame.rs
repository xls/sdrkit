//! Frame acquisition and channel cleaning for OFDM systems that start each frame with a null symbol and a known symbol (DAB, the
//! PRS: phase reference symbol): the quietest window, the channel impulse response against the known symbol, the FFT window
//! placement for a single-frequency network, and the cleaning of a channel estimate in the delay domain.
//!
//! Standard: ETSI EN 300 401 clause 14 (the transmission frame, the null symbol and the phase reference symbol).

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fft::Fft;

/// The quietest window found by [`quietest_window`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuietWindow {
    /// The index of the window's first sample in the slice searched.
    pub start: usize,
    /// The mean power per sample in the window.
    pub power: f32,
    /// The mean power per sample over the whole span searched.
    pub mean_power: f32,
}

/// The null-symbol search: the window of `window` samples with the least energy, among windows that start every `step` samples
/// within the first `span` samples of `x`. The energy comes from block powers of `block` samples, so the search costs one pass over
/// the samples (`window` and `step` must be multiples of `block`). `scratch` holds the block powers (nothing is allocated once it
/// has grown). A null symbol is found when [`QuietWindow::power`] is well below [`QuietWindow::mean_power`] (DAB: below 0.6 of it).
/// Errors when the sizes are not multiples of `block`, or `x` is shorter than `span + window`.
///
/// ```
/// use sdrkit_algo_modem::ofdm::quietest_window;
/// use sdrkit_algo_modem::Complex32;
/// let mut x = vec![Complex32::new(1.0, 0.0); 4096];
/// x[1536..2048].iter_mut().for_each(|v| *v = Complex32::default());
/// let q = quietest_window(&x, 256, 64, 64, 3584, &mut Vec::new()).unwrap();
/// assert_eq!((q.start, q.power), (1536, 0.0));
/// ```
pub fn quietest_window(
    x: &[Complex32],
    window: usize,
    block: usize,
    step: usize,
    span: usize,
    scratch: &mut Vec<f32>,
) -> Result<QuietWindow, ModemError> {
    if block == 0 {
        return Param::BlockLen.at_least(1.0, 0.0);
    }
    if window == 0 || !window.is_multiple_of(block) {
        return Param::Window.conflict(Param::BlockLen);
    }
    if step == 0 || !step.is_multiple_of(block) {
        return Param::Step.conflict(Param::BlockLen);
    }
    if span == 0 || x.len() < span + window {
        return Err(ModemError::InputTooShort {
            need: span + window,
        });
    }
    let blocks = (span + window).div_ceil(block).min(x.len() / block);
    scratch.clear();
    scratch.extend((0..blocks).map(|b| {
        x[b * block..(b + 1) * block]
            .iter()
            .map(|c| {
                let p = c.norm_sqr();
                if p.is_finite() {
                    p
                } else {
                    0.0
                }
            })
            .sum::<f32>()
    }));
    let wb = window / block;
    let span_blocks = span.div_ceil(block).min(blocks);
    let mean = scratch[..span_blocks]
        .iter()
        .map(|&v| v as f64)
        .sum::<f64>()
        / (span_blocks * block) as f64;
    let mut best = (f64::INFINITY, 0usize);
    let mut start = 0;
    while start < span && start / block + wb <= blocks {
        let b = start / block;
        let e: f64 = scratch[b..b + wb].iter().map(|&v| v as f64).sum();
        if e < best.0 {
            best = (e, start);
        }
        start += step;
    }
    Ok(QuietWindow {
        start: best.1,
        power: (best.0 / window as f64) as f32,
        mean_power: mean as f32,
    })
}

/// The channel impulse response against a known symbol: `IFFT(X[k + shift] conj(P[k]))` over the known cells `P` (DAB's PRS),
/// for a symbol's FFT bins `X` (natural order) and a whole-carrier offset `shift`. Tap `d` is a path that arrives `d` samples after
/// the FFT window's start (a tap above `nfft / 2` arrives `nfft - d` samples before it), so the peak is at the timing of the
/// strongest path, and the ratio of the peak to the median shows whether the symbol is present. Holds its IFFT plan and buffers.
///
/// ```
/// use sdrkit_algo_dsp::fft::Fft;
/// use sdrkit_algo_modem::ofdm::ImpulseResponse;
/// use sdrkit_algo_modem::Complex32;
/// // Known cells on carriers -4..=4 (no DC), and the symbol arrives 3 samples into the window.
/// let known: Vec<(i32, Complex32)> = (-4..=4).filter(|&k| k != 0).map(|k| (k, Complex32::new(if k % 3 == 0 { -1.0 } else { 1.0 }, 0.0))).collect();
/// let mut x = vec![Complex32::default(); 16];
/// for &(k, p) in &known {
///     let ph = -std::f32::consts::TAU * k as f32 * 3.0 / 16.0;
///     x[k.rem_euclid(16) as usize] = p * Complex32::from_polar(1.0, ph);
/// }
/// let mut ir = ImpulseResponse::new(16, &known).unwrap();
/// ir.compute(&x, 0).unwrap();
/// assert_eq!(ir.peak().0, 3);
/// ```
#[derive(Clone, Debug)]
pub struct ImpulseResponse {
    nfft: usize,
    known: Vec<(i32, Complex32)>,
    ifft: Fft,
    h: Vec<Complex32>,
    mags: Vec<f32>,
}

impl ImpulseResponse {
    /// For FFTs of `nfft` points (2 to 65536) and the known cells `(carrier from DC, value)`. Errors for a bad size, no cells, a
    /// cell outside the FFT or a value that is not finite.
    pub fn new(nfft: usize, known: &[(i32, Complex32)]) -> Result<Self, ModemError> {
        Param::FftSize.check_count(nfft, 2, 65_536)?;
        if known.is_empty() {
            return Param::Pilots.at_least(1.0, 0.0);
        }
        let half = nfft as i32 / 2;
        if known
            .iter()
            .any(|&(k, v)| k < -half || k >= half || !(v.re.is_finite() && v.im.is_finite()))
        {
            return Param::Pilots.conflict(Param::FftSize);
        }
        Ok(ImpulseResponse {
            nfft,
            known: known.to_vec(),
            ifft: Fft::inverse(nfft),
            h: vec![Complex32::default(); nfft],
            mags: Vec::with_capacity(nfft),
        })
    }

    /// Computes the response of the symbol whose FFT bins are `bins` (natural order, `nfft` values) at the whole-carrier offset
    /// `shift`. Errors when `bins` has the wrong length.
    pub fn compute(&mut self, bins: &[Complex32], shift: i32) -> Result<&[Complex32], ModemError> {
        let n = self.nfft;
        if bins.len() != n {
            return Err(ModemError::InputTooShort { need: n });
        }
        self.h.fill(Complex32::default());
        for &(k, p) in &self.known {
            let b = k.rem_euclid(n as i32) as usize;
            let x = bins[(k + shift).rem_euclid(n as i32) as usize];
            self.h[b] = x * p.conj();
        }
        self.ifft.process(&mut self.h);
        let s = 1.0 / n as f32;
        self.h.iter_mut().for_each(|v| *v *= s);
        Ok(&self.h)
    }

    /// The last response, `nfft` taps (tap `d` is a path `d` samples after the window start, modulo `nfft`).
    pub fn response(&self) -> &[Complex32] {
        &self.h
    }

    /// The strongest tap of the last response and its power over the median tap power.
    pub fn peak(&mut self) -> (usize, f32) {
        self.mags.clear();
        self.mags.extend(self.h.iter().map(|v| v.norm_sqr()));
        let (mut pi, mut pv) = (0usize, 0f32);
        for (i, &m) in self.mags.iter().enumerate() {
            if m > pv {
                pv = m;
                pi = i;
            }
        }
        let mid = self.mags.len() / 2;
        let (_, med, _) = self.mags.select_nth_unstable_by(mid, |a, b| a.total_cmp(b));
        (pi, pv / med.max(1e-30))
    }
}

/// Where to start the FFT window in a single-frequency network, from the tap powers `power` of an impulse response taken in a
/// window (tap `i` above `n / 2` counts as `i - n`, a path before the window).
///
/// Paths are the taps at or above both `rel` times the peak and `median_factor` times the median tap. Take the paths within `span`
/// samples of the peak (`span` is the guard interval less a margin). When they spread over at most `span - lead / 2` samples, the
/// window starts at the earliest of them. Otherwise the window starts at the path `t` whose stretch from `t - lead` to `t + span`
/// holds the most path energy (the earliest such path on a tie): the window then serves the nearer transmitters when a far one
/// arrives outside the guard. Returns the offset of that start from the window's start, in
/// samples (negative: earlier). `scratch` holds a copy of the powers. `None` for an empty input.
///
/// DAB mode I uses `span` 488 (the guard of 504 less 16), `lead` 16, `rel` 0.1 and `median_factor` 12.
pub fn place_window(
    power: &[f32],
    span: i64,
    lead: i64,
    rel: f32,
    median_factor: f32,
    scratch: &mut Vec<f32>,
) -> Option<i64> {
    let n = power.len() as i64;
    if n == 0 {
        return None;
    }
    let signed = |i: usize| {
        if i as i64 >= n / 2 {
            i as i64 - n
        } else {
            i as i64
        }
    };
    let (mut pi, mut pv) = (0usize, 0f32);
    for (i, &p) in power.iter().enumerate() {
        if p > pv {
            pv = p;
            pi = i;
        }
    }
    scratch.clear();
    scratch.extend_from_slice(power);
    let mid = scratch.len() / 2;
    let med = *scratch.select_nth_unstable_by(mid, |a, b| a.total_cmp(b)).1;
    let thr = (rel * pv).max(median_factor * med);
    let pk = signed(pi);
    // The paths in order of signed delay (earliest first), reusing the scratch: delay in the even slots, power in the odd ones.
    scratch.clear();
    let h = power.len() / 2;
    for i in (h..power.len()).chain(0..h) {
        let p = power[i];
        if p >= thr && p > 0.0 {
            scratch.push(signed(i) as f32);
            scratch.push(p);
        }
    }
    let paths = scratch.len() / 2;
    let at = |s: &Vec<f32>, j: usize| (s[2 * j] as i64, s[2 * j + 1]);
    let (mut early, mut late) = (pk, pk);
    for j in 0..paths {
        let (d, _) = at(scratch, j);
        if (d - pk).abs() <= span {
            early = early.min(d);
            late = late.max(d);
        }
    }
    if late - early <= span - lead.max(0) / 2 {
        return Some(early);
    }
    let mut best = (0f32, pk);
    for j in 0..paths {
        let (t, _) = at(scratch, j);
        let mut e = 0f32;
        for m in 0..paths {
            let (d, p) = at(scratch, m);
            if d >= t - lead && d <= t + span {
                e += p;
            }
        }
        if e > best.0 {
            best = (e, t);
        }
    }
    Some(best.1)
}

/// Cleans a channel estimate in the delay domain: the estimate goes to the delay domain by an IFFT, every tap outside a mask is set
/// to zero, and an FFT brings it back. The mask keeps the taps that stand above `median_factor` times the median tap power of an
/// estimate (the PRS estimate of a frame), with `guard` taps either side. Noise in the estimate spreads over every tap, but the channel
/// occupies a few, so cleaning lowers the noise by about the share of taps the mask keeps. Holds its FFT plans and buffers.
///
/// ```
/// use sdrkit_algo_modem::ofdm::DelayCleaner;
/// use sdrkit_algo_modem::Complex32;
/// // A 256-point FFT with every carrier used, and a flat channel with a little noise.
/// let bins: Vec<usize> = (0..256).collect();
/// let mut s = 1u32;
/// let mut noise = || { s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223); (s >> 8) as f32 / (1u32 << 24) as f32 - 0.5 };
/// let mut h: Vec<Complex32> = (0..bins.len()).map(|_| Complex32::new(1.0 + 0.2 * noise(), 0.2 * noise())).collect();
/// let err = |h: &[Complex32]| h.iter().map(|v| (v - Complex32::new(1.0, 0.0)).norm_sqr()).sum::<f32>();
/// let before = err(&h);
/// let mut c = DelayCleaner::new(256, &bins).unwrap();
/// assert!(c.make_mask(&h, 12.0, 2, 0.25).unwrap());
/// c.clean(&mut h).unwrap();
/// assert!(err(&h) < before / 4.0);
/// ```
#[derive(Clone, Debug)]
pub struct DelayCleaner {
    bins: Vec<usize>,
    fft: Fft,
    ifft: Fft,
    tmp: Vec<Complex32>,
    pw: Vec<f32>,
    mask: Vec<bool>,
}

impl DelayCleaner {
    /// A cleaner for estimates whose value `i` belongs to FFT bin `bins[i]` of an `nfft`-point FFT (2 to 65536). Errors for a bad
    /// size, an empty list or a bin outside the FFT.
    pub fn new(nfft: usize, bins: &[usize]) -> Result<Self, ModemError> {
        Param::FftSize.check_count(nfft, 2, 65_536)?;
        if bins.is_empty() {
            return Param::Carriers.at_least(1.0, 0.0);
        }
        if let Some(&b) = bins.iter().find(|&&b| b >= nfft) {
            return Param::Carriers.range(0.0, (nfft - 1) as f64, b as f64);
        }
        Ok(DelayCleaner {
            bins: bins.to_vec(),
            fft: Fft::forward(nfft),
            ifft: Fft::inverse(nfft),
            tmp: vec![Complex32::default(); nfft],
            pw: Vec::with_capacity(nfft),
            mask: vec![true; nfft],
        })
    }

    fn load_delay(&mut self, h: &[Complex32]) -> Result<(), ModemError> {
        if h.len() != self.bins.len() {
            return Err(ModemError::InputTooShort {
                need: self.bins.len(),
            });
        }
        self.tmp.fill(Complex32::default());
        for (&b, &v) in self.bins.iter().zip(h) {
            self.tmp[b] = if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::default()
            };
        }
        self.ifft.process(&mut self.tmp);
        Ok(())
    }

    /// Builds the mask from the estimate `h` (one value per carrier given to [`DelayCleaner::new`]): taps above `median_factor`
    /// times the median tap power, and `guard` taps either side. Returns whether the channel is clean enough to clean: at least
    /// one tap found, and at most `max_share` of the taps kept. Errors when `h` has the wrong length.
    pub fn make_mask(
        &mut self,
        h: &[Complex32],
        median_factor: f32,
        guard: usize,
        max_share: f32,
    ) -> Result<bool, ModemError> {
        self.load_delay(h)?;
        let n = self.tmp.len();
        self.pw.clear();
        self.pw.extend(self.tmp.iter().map(|v| v.norm_sqr()));
        let mid = n / 2;
        let med = *self.pw.select_nth_unstable_by(mid, |a, b| a.total_cmp(b)).1;
        let thr = median_factor * med.max(1e-30);
        self.mask.fill(false);
        let mut found = 0usize;
        let g = guard.min(n / 2) as i64;
        for i in 0..n {
            if self.tmp[i].norm_sqr() > thr {
                found += 1;
                for d in -g..=g {
                    self.mask[(i as i64 + d).rem_euclid(n as i64) as usize] = true;
                }
            }
        }
        let kept = self.mask.iter().filter(|&&m| m).count();
        Ok(found > 0 && kept as f32 <= max_share * n as f32)
    }

    /// The taps the mask keeps (true) and drops.
    pub fn mask(&self) -> &[bool] {
        &self.mask
    }

    /// Cleans `h` in place with the current mask (all taps are kept until [`make_mask`](Self::make_mask) runs). Errors when `h`
    /// has the wrong length.
    pub fn clean(&mut self, h: &mut [Complex32]) -> Result<(), ModemError> {
        self.load_delay(h)?;
        for (v, &m) in self.tmp.iter_mut().zip(&self.mask) {
            if !m {
                *v = Complex32::default();
            }
        }
        self.fft.process(&mut self.tmp);
        let s = 1.0 / self.tmp.len() as f32;
        for (&b, o) in self.bins.iter().zip(h.iter_mut()) {
            *o = self.tmp[b] * s;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    /// DAB mode I sizes: 2048 points, carriers -768..=768 without DC.
    fn dab_known(seed: u64) -> Vec<(i32, Complex32)> {
        let mut r = Rng::new(seed);
        (-768..=768)
            .filter(|&k| k != 0)
            .map(|k| {
                let q = r.below(4) as f32;
                (
                    k,
                    Complex32::from_polar(1.0, std::f32::consts::FRAC_PI_2 * q + 0.785),
                )
            })
            .collect()
    }

    /// The FFT bins of the known symbol through paths `(delay, gain)`, the window starting `early` samples before the first path.
    fn received(
        known: &[(i32, Complex32)],
        paths: &[(f32, Complex32)],
        early: f32,
        shift: i32,
    ) -> Vec<Complex32> {
        let n = 2048;
        let mut x = vec![Complex32::default(); n];
        for &(k, p) in known {
            let mut h = Complex32::default();
            for &(d, g) in paths {
                h += g * Complex32::from_polar(
                    1.0,
                    -std::f32::consts::TAU * k as f32 * (d + early) / n as f32,
                );
            }
            x[(k + shift).rem_euclid(n as i32) as usize] = p * h;
        }
        x
    }

    #[test]
    fn the_null_symbol_is_the_quietest_window() {
        // A DAB frame at 2.048 Msps: 196 608 samples, the null symbol (2656 samples) at 50 000.
        let mut r = Rng::new(1);
        let mut x: Vec<Complex32> = (0..196_608 + 4096).map(|_| r.complex_normal(1.0)).collect();
        for v in &mut x[50_000..52_656] {
            *v = r.complex_normal(1e-3);
        }
        let mut s = Vec::new();
        let q = quietest_window(&x, 2048, 64, 128, 196_608, &mut s).unwrap();
        // The quietest 2048 samples lie inside the null: the window starts within the null's first 608 samples.
        assert!(
            q.start >= 50_000 && q.start + 2048 <= 52_656 + 64,
            "{}",
            q.start
        );
        assert!(q.power < 0.01 * q.mean_power);
        assert!(quietest_window(&x, 2000, 64, 128, 1000, &mut s).is_err());
        assert!(quietest_window(&x[..100], 2048, 64, 128, 1000, &mut s).is_err());
    }

    #[test]
    fn the_impulse_response_finds_the_timing_and_the_offset() {
        let known = dab_known(2);
        let mut ir = ImpulseResponse::new(2048, &known).unwrap();
        // Two paths (the main one, and an echo at -6 dB 40 samples later), the window 64 samples early, 3 carriers high.
        let x = received(
            &known,
            &[
                (0.0, Complex32::new(1.0, 0.0)),
                (40.0, Complex32::new(0.0, 0.5)),
            ],
            64.0,
            3,
        );
        ir.compute(&x, 3).unwrap();
        let (pk, ratio) = ir.peak();
        assert_eq!(pk, 64);
        assert!(ratio > 1e3, "{ratio}");
        let h = ir.response();
        // The echo's tap holds half the amplitude.
        assert!((h[104].norm() / h[64].norm() - 0.5).abs() < 0.02);
        // At the wrong offset there is no peak.
        ir.compute(&x, 0).unwrap();
        assert!(ir.peak().1 < 30.0);
        // The integer offset from the differential metric, which does not care about the timing.
        let shift =
            crate::ofdm::integer_cfo(&x, &known, 5, crate::ofdm::CfoMetric::Differential).unwrap();
        assert_eq!(shift.shift, 3);
        assert!(ImpulseResponse::new(2048, &[(1024, Complex32::new(1.0, 0.0))]).is_err());
        assert!(ir.compute(&x[..100], 0).is_err());
    }

    #[test]
    fn the_window_goes_before_the_paths_that_fit_the_guard() {
        let n = 2048;
        let mut s = Vec::new();
        let mut p = vec![1e-4f32; n];
        // One path at tap 5: the window starts at it.
        p[5] = 1.0;
        assert_eq!(place_window(&p, 488, 16, 0.1, 12.0, &mut s), Some(5));
        // A strong path at 0 and a weaker one 30 samples before it (tap n - 30): start at the earlier one.
        p[n - 30] = 0.3;
        assert_eq!(place_window(&p, 488, 16, 0.1, 12.0, &mut s), Some(-30));
        // A far transmitter 600 samples after the near ones, weaker than them together: the window serves the near ones.
        let mut q = vec![1e-4f32; n];
        q[0] = 1.0;
        q[20] = 0.8;
        q[600] = 0.9;
        assert_eq!(place_window(&q, 488, 16, 0.1, 12.0, &mut s), Some(0));
        // The far one stronger than the near ones together: the window serves it and the paths within the guard after it.
        q[600] = 2.5;
        q[650] = 0.5;
        assert_eq!(place_window(&q, 488, 16, 0.1, 12.0, &mut s), Some(600));
        // Paths spread wider than the guard around the peak: the stretch with the most energy, the earliest on a tie.
        let mut w = vec![1e-4f32; n];
        w[0] = 1.0;
        w[400] = 0.6;
        w[n - 300] = 0.6;
        assert_eq!(place_window(&w, 488, 16, 0.1, 12.0, &mut s), Some(-300));
        w[400] = 0.7;
        assert_eq!(place_window(&w, 488, 16, 0.1, 12.0, &mut s), Some(0));
        assert_eq!(place_window(&[], 488, 16, 0.1, 12.0, &mut s), None);
    }

    #[test]
    fn delay_cleaning_lowers_the_noise_of_an_estimate() {
        let known = dab_known(3);
        let bins: Vec<usize> = known
            .iter()
            .map(|&(k, _)| k.rem_euclid(2048) as usize)
            .collect();
        // The true channel: two paths within a short delay spread.
        let truth: Vec<Complex32> = known
            .iter()
            .map(|&(k, _)| {
                Complex32::new(1.0, 0.0)
                    + Complex32::new(0.0, 0.4)
                        * Complex32::from_polar(
                            1.0,
                            -std::f32::consts::TAU * k as f32 * 17.0 / 2048.0,
                        )
            })
            .collect();
        let mut r = Rng::new(4);
        let noisy: Vec<Complex32> = truth.iter().map(|&h| h + r.complex_normal(0.05)).collect();
        let mut c = DelayCleaner::new(2048, &bins).unwrap();
        assert!(c.make_mask(&noisy, 12.0, 2, 0.25).unwrap());
        let mut h = noisy.clone();
        c.clean(&mut h).unwrap();
        let err = |a: &[Complex32]| {
            a.iter()
                .zip(&truth)
                .map(|(x, y)| (x - y).norm_sqr() as f64)
                .sum::<f64>()
                / a.len() as f64
        };
        let (before, after) = (err(&noisy), err(&h));
        // The mask keeps about 10 of 2048 taps: the noise falls by far more than 10 dB.
        assert!(after < before / 10.0, "before {before} after {after}");
        // A mask that keeps more than the share allowed says the channel is not clean enough.
        assert!(!c.make_mask(&noisy, 0.5, 2, 0.25).unwrap());
        assert!(c.clean(&mut h[..10]).is_err());
        assert!(DelayCleaner::new(64, &[64]).is_err());
    }
}
