//! The frequency interpolator of a scattered-pilot grid that passes only the delay window of the channel
//! ([`WindowedGridInterp`]): the window from [`delay_window`](super::delay_window), the grid as
//! [`PilotTimeInterp`](super::PilotTimeInterp) gives it.

use super::{rotator, GridInterp};
use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fir::Fir;

/// The most taps either side of a grid value in the window filter.
pub const MAX_WINDOW_HALF: usize = 48;

/// The width of a Hann-windowed sinc's transition times its length, in cycles per grid value.
const TRANSITION: f64 = 3.1;

/// A [`GridInterp`] that can be narrowed to a delay window.
///
/// A grid of every `step`-th carrier holds paths within `+-N / (2 step)` samples, and noise from all of that range. When the
/// paths of the channel lie in a window `centre +- half` samples, the grid values are first filtered along frequency so that
/// only that window passes: the window's centre delay is taken off (value `j` turned by `e^(j 2 pi centre j step / N)`), a
/// low-pass over the grid values runs, and the delay is put back. The low-pass is a sinc windowed by `cos^2`, unit gain at
/// the centre delay, half amplitude at `half` and flat inside `half - margin`; its length follows from the margin
/// (`3.1 N / (2 margin step)` grid values, at most `2 x` [`MAX_WINDOW_HALF`] `+ 1`). Within its half length of an end of the
/// grid the filter is the symmetric one that fits, with the same flat part and a wider transition, so the ends are noisier and
/// not bent; the end values themselves pass as they are. [`GridInterp::fill`] then fills the carriers between.
///
/// The noise of the filtered grid values is [`noise_gain`](Self::noise_gain) times that of the grid. Without a window it is a
/// [`GridInterp`]. The filter runs on the shared FIR. Nothing is allocated after the buffers have grown to the grid.
#[derive(Clone, Debug)]
pub struct WindowedGridInterp {
    inner: GridInterp,
    window: Option<(f64, f64)>,
    /// Cycles per grid value of the window's centre delay.
    slope: f64,
    /// Taps either side in use.
    half: usize,
    fir: Fir<Complex32>,
    /// The centre and one side of the filter of each half length `h < half`, at `h (h + 1) / 2`.
    edge: Vec<f32>,
    full: Vec<f32>,
    noise_gain: f64,
    flat: Vec<Complex32>,
    smooth: Vec<Complex32>,
}

/// The centre tap and one side (`h + 1` values) of a sinc with its half-amplitude point at `fc` cycles per grid value,
/// windowed by `cos^2` over `2 h + 1` taps, unit gain at 0. `fc` of 0.5 or more gives the single tap 1.
fn design(h: usize, fc: f64, out: &mut [f32]) {
    out[..=h].fill(0.0);
    if fc >= 0.5 || h == 0 {
        out[0] = 1.0;
        return;
    }
    let mut w = [0f64; MAX_WINDOW_HALF + 1];
    let mut sum = 0.0;
    for (j, v) in w[..=h].iter_mut().enumerate() {
        let x = std::f64::consts::TAU * fc * j as f64;
        let sinc = if j == 0 { 1.0 } else { x.sin() / x };
        let win = (std::f64::consts::PI * j as f64 / (2 * h + 2) as f64)
            .cos()
            .powi(2);
        *v = sinc * win;
        sum += if j == 0 { *v } else { 2.0 * *v };
    }
    for (o, v) in out[..=h].iter_mut().zip(&w[..=h]) {
        *o = (v / sum) as f32;
    }
}

impl WindowedGridInterp {
    /// An interpolator for a grid of every `step`-th carrier with `half` taps either side between grid values (as
    /// [`GridInterp::new`]), with no window.
    pub fn new(step: usize, half: usize) -> Result<Self, ModemError> {
        let mut full = vec![0f32; 2 * MAX_WINDOW_HALF + 1];
        full[MAX_WINDOW_HALF] = 1.0;
        Ok(WindowedGridInterp {
            inner: GridInterp::new(step, half)?,
            window: None,
            slope: 0.0,
            half: 0,
            fir: Fir::new(&full),
            edge: vec![0.0; MAX_WINDOW_HALF * (MAX_WINDOW_HALF + 1) / 2],
            full,
            noise_gain: 1.0,
            flat: Vec::new(),
            smooth: Vec::new(),
        })
    }

    /// The interpolator without the window (for the full-window estimate that the delay window is measured on).
    pub fn full(&self) -> &GridInterp {
        &self.inner
    }

    /// The window in use: `(centre, half width)` in samples.
    pub fn window(&self) -> Option<(f64, f64)> {
        self.window
    }

    /// The noise power of a filtered grid value away from the ends over that of a grid value (1 without a window).
    pub fn noise_gain(&self) -> f64 {
        self.noise_gain
    }

    /// Sets the delay window for an `nfft`-point FFT: paths within `centre +- half` samples pass, `margin` samples inside
    /// `half` is flat. `None` passes everything the grid holds, and so does a window as wide as the grid's range
    /// (`half >= nfft / (2 step)`). Errors for an `nfft` of 0 or values that are not finite, a `half` that is not positive
    /// or a `margin` outside `(0, half]`.
    pub fn set_window(
        &mut self,
        nfft: usize,
        window: Option<(f64, f64)>,
        margin: f64,
    ) -> Result<(), ModemError> {
        let Some((centre, half)) = window else {
            self.window = None;
            self.noise_gain = 1.0;
            return Ok(());
        };
        Param::FftSize.check_count(nfft, 1, 1 << 24)?;
        Param::Centre.check_finite(centre)?;
        Param::Span.check_in(half, f64::MIN_POSITIVE, f64::MAX)?;
        Param::Span.check_in(margin, f64::MIN_POSITIVE, half)?;
        if self.window == Some((centre, half)) {
            return Ok(());
        }
        let per_sample = self.inner.step() as f64 / nfft as f64;
        if half * per_sample >= 0.5 {
            self.window = None;
            self.noise_gain = 1.0;
            return Ok(());
        }
        let pass = (half - margin) * per_sample;
        let taps = (TRANSITION / (2.0 * margin * per_sample)).ceil() as usize;
        let h_full = (taps / 2).clamp(1, MAX_WINDOW_HALF);
        let cut = |h: usize| pass + 0.5 * TRANSITION / (2 * h + 1) as f64;
        for h in 0..h_full {
            let at = h * (h + 1) / 2;
            design(h, cut(h), &mut self.edge[at..at + h + 1]);
        }
        let mut side = [0f32; MAX_WINDOW_HALF + 1];
        design(h_full, cut(h_full), &mut side);
        self.full.fill(0.0);
        let mut gain = 0f64;
        for (j, &v) in side[..=h_full].iter().enumerate() {
            self.full[MAX_WINDOW_HALF + j] = v;
            self.full[MAX_WINDOW_HALF - j] = v;
            gain += if j == 0 { 1.0 } else { 2.0 } * f64::from(v) * f64::from(v);
        }
        self.fir.set_taps(&self.full);
        self.half = h_full;
        self.slope = centre * per_sample;
        self.noise_gain = gain;
        self.window = Some((centre, half));
        Ok(())
    }

    /// Fills `out` from the grid values `grid` as [`GridInterp::fill`] does, after the window filter when a window is set.
    /// Returns the mean phase step taken out, in cycles per carrier. Errors as [`GridInterp::fill`].
    pub fn fill(&mut self, grid: &[Complex32], out: &mut [Complex32]) -> Result<f64, ModemError> {
        let m = grid.len();
        if self.window.is_none() || m < 2 {
            return self.inner.fill(grid, out);
        }
        if grid.iter().any(|g| !(g.re.is_finite() && g.im.is_finite())) {
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        // The grid with the window's centre delay taken off.
        let mut down = rotator(0.0, self.slope);
        self.flat.clear();
        self.flat
            .extend(grid.iter().map(|g| g * down.next_sample()));
        self.smooth.clear();
        self.smooth.resize(m, Complex32::default());
        let flat = &self.flat;
        let h_full = self.half;
        // Away from the ends: the FIR's output at `i` is the filtered value at `i - MAX_WINDOW_HALF`.
        if m > 2 * h_full {
            self.fir.reset();
            let lag = MAX_WINDOW_HALF;
            let mut done = 0;
            let mut chunk = [Complex32::default(); 64];
            for (i, part) in flat.chunks(64).enumerate() {
                let y = &mut chunk[..part.len()];
                self.fir.process(part, y);
                for (k, v) in y.iter().enumerate() {
                    let at = i * 64 + k;
                    if at >= lag + h_full && at - lag < m - h_full {
                        self.smooth[at - lag] = *v;
                        done = at - lag + 1;
                    }
                }
            }
            // The values the FIR has not reached when the grid ends.
            for c in done.max(h_full)..m - h_full {
                let mut acc = Complex32::default();
                for (j, &w) in self.full[MAX_WINDOW_HALF - h_full..=MAX_WINDOW_HALF + h_full]
                    .iter()
                    .enumerate()
                {
                    acc += flat[c + j - h_full] * w;
                }
                self.smooth[c] = acc;
            }
        }
        // Near the ends: the symmetric filter that fits.
        for c in (0..m).filter(|&c| c.min(m - 1 - c) < h_full) {
            let h = c.min(m - 1 - c);
            let w = &self.edge[h * (h + 1) / 2..][..h + 1];
            let mut acc = flat[c] * w[0];
            for j in 1..=h {
                acc += (flat[c - j] + flat[c + j]) * w[j];
            }
            self.smooth[c] = acc;
        }
        let mut up = rotator(0.0, -self.slope);
        for v in self.smooth.iter_mut() {
            *v *= up.next_sample();
        }
        self.inner.fill(&self.smooth, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: usize = 8192;

    /// The channel of paths `(delay in samples, gain)` at carrier `k`.
    fn channel(paths: &[(f64, f32)], k: usize) -> Complex32 {
        paths
            .iter()
            .map(|&(d, g)| {
                Complex32::from_polar(g, -std::f32::consts::TAU * d as f32 * k as f32 / N as f32)
            })
            .sum()
    }

    fn noise(seed: &mut u64) -> Complex32 {
        let mut u = || {
            *seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((*seed >> 40) as f32 / (1u64 << 24) as f32) - 0.5
        };
        // The sum of 12 uniform values is close to a unit normal one.
        let mut g = || (0..12).map(|_| u()).sum::<f32>();
        Complex32::new(g(), g()) * std::f32::consts::FRAC_1_SQRT_2
    }

    /// Mean square error of the estimate over the carriers `range` for a grid of 1872 values with noise of power `var`.
    fn mse(
        gi: &mut WindowedGridInterp,
        paths: &[(f64, f32)],
        var: f32,
        range: std::ops::Range<usize>,
    ) -> f64 {
        let m = 1872;
        let mut seed = 7u64;
        let grid: Vec<Complex32> = (0..m)
            .map(|j| channel(paths, 3 * j) + noise(&mut seed) * var.sqrt())
            .collect();
        let mut out = vec![Complex32::default(); 3 * m - 2];
        gi.fill(&grid, &mut out).unwrap();
        range
            .clone()
            .map(|k| (out[k] - channel(paths, k)).norm_sqr() as f64)
            .sum::<f64>()
            / range.len() as f64
    }

    #[test]
    fn the_window_keeps_the_paths_and_drops_the_noise_outside() {
        // Paths over 41 samples, as TU6 at 8.127 Msps; the window of `delay_window` for them.
        let paths = [(30.0, 0.8f32), (38.0, 0.5), (50.0, 0.3), (71.0, 0.2)];
        let (centre, half) = (50.5, 0.7 * 41.0 + 48.0);
        let mut gi = WindowedGridInterp::new(3, 4).unwrap();
        let var = 0.01f32;
        let middle = 600..5000;
        let plain = mse(&mut gi, &paths, var, middle.clone());
        gi.set_window(N, Some((centre, half)), 48.0).unwrap();
        assert_eq!(gi.window(), Some((centre, half)));
        let windowed = mse(&mut gi, &paths, var, middle.clone());
        // The grid passes 2 x 76.7 of 2 x 1365 samples of delay: about 11 dB less noise away from the ends.
        assert!(
            gi.noise_gain() < 0.09 && gi.noise_gain() > 0.03,
            "{}",
            gi.noise_gain()
        );
        assert!(windowed < 0.12 * plain, "{windowed} against {plain}");
        // Without noise the paths come back: the filter is flat over them, also at the ends of the grid.
        let clean = mse(&mut gi, &paths, 0.0, 0..5614);
        assert!(clean < 2e-5, "{clean}");
        // The ends are no worse than without the window.
        let ends_plain = {
            gi.set_window(N, None, 48.0).unwrap();
            mse(&mut gi, &paths, var, 0..60)
        };
        gi.set_window(N, Some((centre, half)), 48.0).unwrap();
        let ends = mse(&mut gi, &paths, var, 0..60);
        assert!(ends < 1.1 * ends_plain, "{ends} against {ends_plain}");
    }

    #[test]
    fn a_wide_window_and_short_grids() {
        let mut gi = WindowedGridInterp::new(3, 4).unwrap();
        // As wide as the grid's range: no window.
        gi.set_window(N, Some((0.0, 1400.0)), 48.0).unwrap();
        assert_eq!(gi.window(), None);
        assert!(gi.set_window(N, Some((0.0, 100.0)), 0.0).is_err());
        assert!(gi.set_window(N, Some((f64::NAN, 100.0)), 48.0).is_err());
        assert!(gi.set_window(0, Some((0.0, 100.0)), 48.0).is_err());
        gi.set_window(N, Some((0.0, 100.0)), 48.0).unwrap();
        // Grids shorter than the filter: every value has the filter that fits; a constant channel stays constant.
        for m in [2usize, 3, 10, 97, 150] {
            let grid = vec![Complex32::new(0.5, -0.25); m];
            let mut out = vec![Complex32::default(); 3 * m - 2];
            gi.fill(&grid, &mut out).unwrap();
            for v in &out {
                assert!((v - grid[0]).norm() < 1e-4, "{m}: {v}");
            }
        }
        let mut out = [Complex32::default(); 1];
        assert!(gi.fill(&[Complex32::new(1.0, 0.0)], &mut out).is_err());
    }
}
