//! Incoherent dedispersion and period folding of filter-bank rows, with a width-matched S/N.
//!
//! The input is a set of rows of power, `channels` values a row (row `r`, channel `c` at `r * channels + c`), one row every
//! `row_s` seconds, with the centre frequency of each channel. [`FoldSearch::run`] searches it over trial dispersions and
//! trial periods:
//!
//! 1. Each channel is normalised: in groups of `group_rows` rows (from row 0; the last group may be short), `v / mean - 1`;
//!    then the whole channel is divided by its rms.
//! 2. For each trial dispersion `D` (in the units of the law), channel `c` is delayed by
//!    `shift_c = round(D (g(f_c) - g(f_ref)) / row_s)` rows, where `g` is a law of [`crate::dispersion`] and `f_ref` the channel
//!    of least delay. A negative `D` is skipped, and so is a `D` whose largest shift plus `margin_rows` reaches the row count.
//!    The dedispersed series is `s[r] = sum over c of z[r + shift_c][c]` for `r` below the rows minus the largest shift.
//! 3. For each trial period `P` the series is folded: row `r` goes to bin `min(B - 1, floor(frac(r row_s / P) B))`, and the
//!    profile is the mean of each bin.
//! 4. The profile is scored by [`matched_snr`].
//!
//! The dispersion curve and the period curve keep the best S/N of each trial. The best of all gives the profile in units of
//! the off-pulse deviation, mean removed, turned so that the peak sits at bin `B / 2` ([`centred`]).
//!
//! ```
//! use sdrkit_algo_dsp::dispersion::cold_plasma;
//! use sdrkit_algo_dsp::fold::{linear_trials, FoldSearch};
//! // 4 channels, 1 ms rows, 4000 rows of power near 1 with a pulse of 3 in every channel every 50 rows (no dispersion).
//! let mut cells: Vec<f32> = (0..4000 * 4).map(|i| 1.0 + 0.1 * ((i * 7919 % 13) as f32 / 13.0 - 0.5)).collect();
//! for r in (7..4000).step_by(50) {
//!     cells[r * 4..r * 4 + 4].fill(3.0);
//! }
//! let freqs = [400e6, 401e6, 402e6, 403e6];
//! let s = FoldSearch::new(4, 0.001, &freqs, cold_plasma, 8, 100, 50).unwrap();
//! let found = s.run(&cells, &[0.0], &linear_trials(0.05, 1e-4, 3));
//! let best = found.best.unwrap();
//! assert!(best.pulse.snr > 8.0, "{best:?}");
//! assert_eq!((best.dispersion, best.period), (0, 1));
//! ```

use crate::{Complex32, DspError, Param};

/// The pulse widths tried by [`matched_snr`], bins. Only widths up to a third of the profile are used.
pub const WIDTHS: [usize; 8] = [1, 2, 3, 4, 6, 8, 12, 16];
/// The share of the profile, lowest values first, that gives the off-pulse mean and deviation.
pub const OFF_PULSE_SHARE: f64 = 0.75;
/// The variance of the lowest [`OFF_PULSE_SHARE`] of a Gaussian sample, as a share of the whole variance (about one half).
pub const OFF_PULSE_VARIANCE: f64 = 0.5;

/// The best pulse of a profile.
#[derive(Clone, Debug, PartialEq)]
pub struct Pulse {
    /// The sum of the profile minus the off-pulse mean over the pulse, over `sd sqrt(width)`.
    pub snr: f64,
    /// The first bin of the pulse.
    pub start: usize,
    /// The width, bins.
    pub width: usize,
    /// The middle of the pulse, `start + width / 2`, in bins from 0 to the bin count.
    pub peak: f64,
    /// The width as a share of the profile.
    pub duty: f64,
    /// The off-pulse mean.
    pub mean: f64,
    /// The off-pulse deviation.
    pub sd: f64,
}

/// The width-matched S/N of `profile`: the lowest [`OFF_PULSE_SHARE`] of its values give the mean `m` and the deviation
/// `sd = sqrt(variance / OFF_PULSE_VARIANCE)`; for each width of [`WIDTHS`] up to a third of the bins and each start bin
/// (the profile is circular), `S/N = sum(profile - m) / (sd sqrt(w))`. Returns the best, the first one on a tie. `None` for
/// fewer than 3 bins or a deviation of 0.
pub fn matched_snr(profile: &[f64]) -> Option<Pulse> {
    let b = profile.len();
    if b < 3 {
        return None;
    }
    let mut sorted = profile.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n_off = ((b as f64 * OFF_PULSE_SHARE) as usize).max(2);
    let off = &sorted[..n_off];
    let mean = off.iter().sum::<f64>() / n_off as f64;
    let var = off.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n_off as f64;
    let sd = (var / OFF_PULSE_VARIANCE).sqrt();
    if !(sd > 0.0 && sd.is_finite()) {
        return None;
    }
    let mut best: Option<Pulse> = None;
    for &w in WIDTHS.iter().filter(|&&w| w * 3 <= b) {
        let norm = sd * (w as f64).sqrt();
        // The sum over the window, moved one bin at a time round the circle.
        let mut sum: f64 = (0..w).map(|k| profile[k] - mean).sum();
        for start in 0..b {
            if start > 0 {
                sum += profile[(start + w - 1) % b] - profile[start - 1];
            }
            let snr = sum / norm;
            if best.as_ref().is_none_or(|p| snr > p.snr) {
                best = Some(Pulse {
                    snr,
                    start,
                    width: w,
                    peak: (start as f64 + w as f64 / 2.0) % b as f64,
                    duty: w as f64 / b as f64,
                    mean,
                    sd,
                });
            }
        }
    }
    best
}

/// `profile` in units of the pulse's deviation, its mean removed, turned so that bin `floor(pulse.peak)` sits at bin
/// `len / 2`.
pub fn centred(profile: &[f64], pulse: &Pulse) -> Vec<f64> {
    let b = profile.len();
    let mut out = vec![0.0; b];
    if b == 0 {
        return out;
    }
    let k = (pulse.peak.floor() as usize) % b;
    for (j, v) in profile.iter().enumerate() {
        out[(j + b / 2 + b - k) % b] = (v - pulse.mean) / pulse.sd;
    }
    out
}

/// `n` trials from `centre - half_span` to `centre + half_span` in equal steps: `centre + half_span (2 i / (n - 1) - 1)`.
/// One trial is `centre`; none for `n` = 0.
pub fn linear_trials(centre: f64, half_span: f64, n: usize) -> Vec<f64> {
    match n {
        0 => Vec::new(),
        1 => vec![centre],
        _ => (0..n)
            .map(|i| centre + half_span * (2.0 * i as f64 / (n - 1) as f64 - 1.0))
            .collect(),
    }
}

/// Folds `series` (one value every `row_s` seconds from row 0) at period `period_s` into `profile` (its length is the bin
/// count): row `r` goes to bin `min(B - 1, floor(frac(r row_s / P) B))`; each bin is the mean of its rows (0 when it has
/// none).
pub fn fold(series: &[f32], row_s: f64, period_s: f64, profile: &mut [f64]) {
    let b = profile.len();
    profile.fill(0.0);
    if b == 0 || period_s.is_nan() || period_s <= 0.0 {
        return;
    }
    let mut counts = vec![0u32; b];
    let step = row_s / period_s;
    for (r, &v) in series.iter().enumerate() {
        let x = r as f64 * step;
        let bin = (((x - x.floor()) * b as f64) as usize).min(b - 1);
        profile[bin] += f64::from(v);
        counts[bin] += 1;
    }
    for (p, &n) in profile.iter_mut().zip(&counts) {
        if n > 0 {
            *p /= f64::from(n);
        }
    }
}

/// Folds a complex `series` at a whole `period` of values into `profile` (cleared, then `period` long): value `i` is
/// added to bin `i % period`, over the whole periods of the series only (the values after the last whole period are left
/// out). Returns the bin of the largest squared magnitude, the first of equals. With a period of 0 or a series shorter
/// than one period the profile is empty and the result is 0.
///
/// Values that point the same way add in amplitude and noise adds in power, so a bin that repeats every period stands
/// out of the profile by the count of periods. No allocation when `profile` has the capacity of one period.
pub fn fold_complex(series: &[Complex32], period: usize, profile: &mut Vec<Complex32>) -> usize {
    profile.clear();
    if period == 0 || series.len() < period {
        return 0;
    }
    let mut periods = series.chunks_exact(period);
    if let Some(first) = periods.next() {
        profile.extend_from_slice(first);
    }
    for row in periods {
        for (p, v) in profile.iter_mut().zip(row) {
            *p += *v;
        }
    }
    let mut best = (0usize, 0f32);
    for (bin, p) in profile.iter().enumerate() {
        let level = p.norm_sqr();
        if level > best.1 {
            best = (bin, level);
        }
    }
    best.0
}

/// The best trial of a search.
#[derive(Clone, Debug, PartialEq)]
pub struct Best {
    /// The index of the trial dispersion.
    pub dispersion: usize,
    /// The index of the trial period.
    pub period: usize,
    /// The pulse of its profile.
    pub pulse: Pulse,
    /// Its profile by [`centred`].
    pub profile: Vec<f64>,
}

/// What a search found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Folded {
    /// The rows searched.
    pub rows: usize,
    /// The best S/N of each trial dispersion over the periods; `None` for a trial that was skipped.
    pub dispersion_curve: Vec<Option<f64>>,
    /// The best S/N of each trial period over the dispersions; `None` when every dispersion was skipped.
    pub period_curve: Vec<Option<f64>>,
    /// The best trial; `None` when no trial gave a score.
    pub best: Option<Best>,
}

/// A search over trial dispersions and periods of filter-bank rows (module docs).
#[derive(Clone, Debug)]
pub struct FoldSearch {
    channels: usize,
    row_s: f64,
    /// `g(f_c) - g(f_ref)` of each channel.
    delay: Vec<f64>,
    group_rows: usize,
    margin_rows: usize,
    bins: usize,
}

impl FoldSearch {
    /// A search of rows of `channels` values every `row_s` seconds, channel `c` centred at `freqs_hz[c]`, delayed by the law
    /// `law`, normalised in groups of `group_rows` rows, with at least `margin_rows` rows left after the largest shift, folded
    /// into `bins` bins.
    ///
    /// # Errors
    ///
    /// [`DspError`] when `channels` is 0, `freqs_hz` has another length or a value that is not above 0, `row_s` is not above 0,
    /// `group_rows` is 0 or `bins` is below 3.
    pub fn new(
        channels: usize,
        row_s: f64,
        freqs_hz: &[f64],
        law: fn(f64) -> f64,
        group_rows: usize,
        margin_rows: usize,
        bins: usize,
    ) -> Result<Self, DspError> {
        if channels == 0 {
            return Param::Channels.positive(0.0);
        }
        if freqs_hz.len() != channels {
            return Param::Frequency.length(channels, freqs_hz.len());
        }
        if let Some(&f) = freqs_hz.iter().find(|f| !(**f > 0.0 && f.is_finite())) {
            return Param::Frequency.positive(f);
        }
        if !(row_s > 0.0 && row_s.is_finite()) {
            return Param::Row.positive(row_s);
        }
        if group_rows == 0 {
            return Param::BlockLen.positive(0.0);
        }
        if bins < 3 {
            return Param::Bins.at_least(3.0, bins as f64);
        }
        let g: Vec<f64> = freqs_hz.iter().map(|&f| law(f)).collect();
        let g_ref = g.iter().copied().fold(f64::INFINITY, f64::min);
        Ok(FoldSearch {
            channels,
            row_s,
            delay: g.iter().map(|v| v - g_ref).collect(),
            group_rows,
            margin_rows,
            bins,
        })
    }

    /// The row shift of each channel at dispersion `d`.
    pub fn shifts(&self, d: f64) -> Vec<usize> {
        self.delay
            .iter()
            .map(|g| (d * g / self.row_s).round().max(0.0) as usize)
            .collect()
    }

    /// The rows of `cells` normalised by channel (module docs, step 1), channel by channel: channel `c` at
    /// `c * rows .. (c + 1) * rows`.
    pub fn normalised(&self, cells: &[f32]) -> Vec<f32> {
        let ch = self.channels;
        let rows = cells.len() / ch;
        let mut z = vec![0f32; rows * ch];
        for c in 0..ch {
            let col = &mut z[c * rows..(c + 1) * rows];
            for (r, v) in col.iter_mut().enumerate() {
                *v = cells[r * ch + c];
            }
            for group in col.chunks_mut(self.group_rows) {
                let mean = group.iter().map(|&v| f64::from(v)).sum::<f64>() / group.len() as f64;
                if mean != 0.0 && mean.is_finite() {
                    for v in group.iter_mut() {
                        *v = (f64::from(*v) / mean - 1.0) as f32;
                    }
                } else {
                    group.fill(0.0);
                }
            }
            let rms = (col.iter().map(|&v| f64::from(v) * f64::from(v)).sum::<f64>()
                / rows.max(1) as f64)
                .sqrt();
            if rms > 0.0 && rms.is_finite() {
                let k = (1.0 / rms) as f32;
                col.iter_mut().for_each(|v| *v *= k);
            }
        }
        z
    }

    /// The dedispersed series of the normalised channels `z` ([`FoldSearch::normalised`]) at the shifts `shifts`. Empty when
    /// the largest shift plus the margin reaches the row count.
    pub fn dedispersed(&self, z: &[f32], shifts: &[usize]) -> Vec<f32> {
        let rows = z.len() / self.channels;
        let most = shifts.iter().copied().max().unwrap_or(0);
        if most + self.margin_rows >= rows {
            return Vec::new();
        }
        let n = rows - most;
        let mut s = vec![0f32; n];
        for (c, &k) in shifts.iter().enumerate() {
            let col = &z[c * rows + k..c * rows + k + n];
            for (a, &v) in s.iter_mut().zip(col) {
                *a += v;
            }
        }
        s
    }

    /// Searches the rows `cells` (row-major, any whole number of rows) over the trial dispersions `dispersions` (in the units
    /// of the law) and the trial periods `periods_s`.
    pub fn run(&self, cells: &[f32], dispersions: &[f64], periods_s: &[f64]) -> Folded {
        let rows = cells.len() / self.channels;
        let z = self.normalised(&cells[..rows * self.channels]);
        let mut out = Folded {
            rows,
            dispersion_curve: vec![None; dispersions.len()],
            period_curve: vec![None; periods_s.len()],
            best: None,
        };
        let mut profile = vec![0.0; self.bins];
        let mut best_profile = vec![0.0; self.bins];
        for (i, &d) in dispersions.iter().enumerate() {
            if !(d >= 0.0 && d.is_finite()) {
                continue;
            }
            let series = self.dedispersed(&z, &self.shifts(d));
            if series.is_empty() {
                continue;
            }
            for (j, &p) in periods_s.iter().enumerate() {
                if !(p > 0.0 && p.is_finite()) {
                    continue;
                }
                fold(&series, self.row_s, p, &mut profile);
                let Some(pulse) = matched_snr(&profile) else {
                    continue;
                };
                let snr = pulse.snr;
                if out.dispersion_curve[i].is_none_or(|v| snr > v) {
                    out.dispersion_curve[i] = Some(snr);
                }
                if out.period_curve[j].is_none_or(|v| snr > v) {
                    out.period_curve[j] = Some(snr);
                }
                if out.best.as_ref().is_none_or(|b| snr > b.pulse.snr) {
                    best_profile.copy_from_slice(&profile);
                    out.best = Some(Best {
                        dispersion: i,
                        period: j,
                        pulse,
                        profile: Vec::new(),
                    });
                }
            }
        }
        if let Some(b) = out.best.as_mut() {
            b.profile = centred(&best_profile, &b.pulse);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispersion::cold_plasma;

    /// A deterministic noise of mean 0 and deviation about 0.29 (uniform).
    fn noise(seed: &mut u64) -> f32 {
        *seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((*seed >> 40) as f32 / (1u64 << 24) as f32) - 0.5
    }

    #[test]
    fn a_single_peak_is_found_with_its_width() {
        let mut p = vec![0.0; 64];
        let mut seed = 3;
        for v in p.iter_mut() {
            *v = f64::from(noise(&mut seed));
        }
        for v in &mut p[30..34] {
            *v += 5.0;
        }
        let pulse = matched_snr(&p).unwrap();
        assert_eq!((pulse.start, pulse.width), (30, 4));
        assert_eq!(pulse.peak, 32.0);
        assert!((pulse.duty - 4.0 / 64.0).abs() < 1e-12);
        assert!(pulse.snr > 20.0, "{pulse:?}");
        let c = centred(&p, &pulse);
        assert!(c[32] > 10.0 && c[31] > 10.0, "{c:?}");
    }

    #[test]
    fn the_profile_wraps_round_the_circle() {
        let mut p = vec![0.0; 32];
        let mut seed = 9;
        for v in p.iter_mut() {
            *v = f64::from(noise(&mut seed));
        }
        p[31] += 4.0;
        p[0] += 4.0;
        let pulse = matched_snr(&p).unwrap();
        assert_eq!((pulse.start, pulse.width), (31, 2));
        assert_eq!(pulse.peak, 0.0);
    }

    #[test]
    fn a_flat_profile_has_no_score() {
        assert!(matched_snr(&[1.0; 16]).is_none());
        assert!(matched_snr(&[1.0, 2.0]).is_none());
    }

    #[test]
    fn trials_are_spread_evenly() {
        assert_eq!(linear_trials(10.0, 2.0, 5), vec![8.0, 9.0, 10.0, 11.0, 12.0]);
        assert_eq!(linear_trials(10.0, 2.0, 1), vec![10.0]);
        assert!(linear_trials(1.0, 1.0, 0).is_empty());
    }

    #[test]
    fn folding_takes_the_mean_of_each_bin() {
        let series: Vec<f32> = (0..96).map(|r| if r % 8 == 0 { 2.0 } else { 0.0 }).collect();
        let mut p = vec![0.0; 8];
        fold(&series, 0.125, 1.0, &mut p);
        assert!((p[0] - 2.0).abs() < 1e-9, "{p:?}");
        assert!(p[1..].iter().all(|v| v.abs() < 1e-9), "{p:?}");
    }

    #[test]
    fn shifts_follow_the_law_from_the_channel_of_least_delay() {
        let freqs = [400e6, 402e6];
        let s = FoldSearch::new(2, 1e-3, &freqs, cold_plasma, 8, 10, 16).unwrap();
        let d = 4.148808e15 * 50.0;
        let want = (d * (1.0 / (400e6f64 * 400e6) - 1.0 / (402e6f64 * 402e6)) / 1e-3).round() as usize;
        assert_eq!(s.shifts(d), vec![want, 0]);
        assert!(want > 0);
    }

    #[test]
    fn normalising_removes_a_slow_gain_and_scales_to_unit_rms() {
        let s = FoldSearch::new(2, 1e-3, &[1e6, 2e6], cold_plasma, 10, 0, 16).unwrap();
        let mut seed = 1;
        let cells: Vec<f32> = (0..200)
            .flat_map(|r| {
                let g = 1.0 + r as f32 / 100.0;
                [g * (1.0 + 0.1 * noise(&mut seed)), 5.0 * g * (1.0 + 0.1 * noise(&mut seed))]
            })
            .collect();
        let z = s.normalised(&cells);
        for c in 0..2 {
            let col = &z[c * 200..(c + 1) * 200];
            let mean = col.iter().sum::<f32>() / 200.0;
            let rms = (col.iter().map(|v| v * v).sum::<f32>() / 200.0).sqrt();
            assert!(mean.abs() < 0.05, "{mean}");
            assert!((rms - 1.0).abs() < 1e-3, "{rms}");
        }
    }

    #[test]
    fn a_dispersed_pulse_train_is_found_at_its_dispersion_and_period() {
        let ch = 16;
        let row_s = 1e-3;
        let freqs: Vec<f64> = (0..ch).map(|c| 400e6 + c as f64 * 0.5e6).collect();
        let s = FoldSearch::new(ch, row_s, &freqs, cold_plasma, 100, 100, 32).unwrap();
        let dm_unit = 4.148808e15;
        let true_d = dm_unit * 300.0;
        let shifts = s.shifts(true_d);
        let rows = 20_000;
        let period = 0.2;
        let mut seed = 5;
        let mut cells: Vec<f32> = (0..rows * ch).map(|_| 1.0 + 0.3 * noise(&mut seed)).collect();
        for k in 0.. {
            let r0 = (k as f64 * period / row_s) as usize;
            if r0 >= rows {
                break;
            }
            for (c, &sh) in shifts.iter().enumerate() {
                if r0 + sh < rows {
                    cells[(r0 + sh) * ch + c] += 0.5;
                }
            }
        }
        let ds = linear_trials(true_d, dm_unit * 200.0, 5);
        let ps = linear_trials(period, period * 1e-3, 3);
        let f = s.run(&cells, &ds, &ps);
        let b = f.best.clone().unwrap();
        assert_eq!((b.dispersion, b.period), (2, 1), "{f:?}");
        assert!(b.pulse.snr > 10.0);
        assert_eq!(b.profile.len(), 32);
        let curve = f.dispersion_curve;
        assert!(curve[2].unwrap() > curve[0].unwrap() && curve[2].unwrap() > curve[4].unwrap());
    }

    #[test]
    fn too_long_a_shift_and_a_negative_dispersion_are_skipped() {
        let s = FoldSearch::new(2, 1e-3, &[100e6, 200e6], cold_plasma, 8, 100, 16).unwrap();
        let cells = vec![1.0f32; 2 * 300];
        let f = s.run(&cells, &[-1.0, 4.148808e15 * 1000.0], &[0.05]);
        assert_eq!(f.dispersion_curve, vec![None, None]);
        assert!(f.best.is_none());
    }

    #[test]
    fn bad_parameters_are_refused() {
        assert!(FoldSearch::new(0, 1e-3, &[], cold_plasma, 8, 0, 16).is_err());
        assert!(FoldSearch::new(2, 1e-3, &[1e6], cold_plasma, 8, 0, 16).is_err());
        assert!(FoldSearch::new(1, 1e-3, &[0.0], cold_plasma, 8, 0, 16).is_err());
        assert!(FoldSearch::new(1, 0.0, &[1e6], cold_plasma, 8, 0, 16).is_err());
        assert!(FoldSearch::new(1, 1e-3, &[1e6], cold_plasma, 0, 0, 16).is_err());
        assert!(FoldSearch::new(1, 1e-3, &[1e6], cold_plasma, 8, 0, 2).is_err());
    }
    #[test]
    fn a_complex_fold_adds_whole_periods_and_finds_the_largest_bin() {
        // 4 whole periods of 5 and a tail of 3. Bin 2 holds 1 + 2j in each period; bin 4 cancels; the tail is left out.
        let mut series = vec![Complex32::new(0.0, 0.0); 23];
        for k in 0..4 {
            series[k * 5 + 2] = Complex32::new(1.0, 2.0);
            series[k * 5 + 4] = Complex32::new(if k % 2 == 0 { 3.0 } else { -3.0 }, 0.0);
        }
        series[20] = Complex32::new(100.0, 0.0);
        let mut profile = Vec::new();
        assert_eq!(fold_complex(&series, 5, &mut profile), 2);
        assert_eq!(profile.len(), 5);
        assert_eq!(profile[2], Complex32::new(4.0, 8.0));
        assert_eq!(profile[4], Complex32::new(0.0, 0.0));
        assert_eq!(profile[0], Complex32::new(0.0, 0.0));
    }

    #[test]
    fn a_complex_fold_takes_the_first_of_equal_bins() {
        let series = [
            Complex32::new(0.0, 0.0),
            Complex32::new(0.0, 2.0),
            Complex32::new(-2.0, 0.0),
            Complex32::new(0.0, 0.0),
            Complex32::new(0.0, 2.0),
            Complex32::new(-2.0, 0.0),
        ];
        let mut profile = Vec::new();
        assert_eq!(fold_complex(&series, 3, &mut profile), 1);
    }

    #[test]
    fn a_complex_fold_without_a_whole_period_is_empty() {
        let series = [Complex32::new(1.0, 0.0); 4];
        let mut profile = vec![Complex32::new(9.0, 9.0); 2];
        assert_eq!(fold_complex(&series, 0, &mut profile), 0);
        assert!(profile.is_empty());
        profile.push(Complex32::new(9.0, 9.0));
        assert_eq!(fold_complex(&series, 5, &mut profile), 0);
        assert!(profile.is_empty());
        assert_eq!(fold_complex(&[], 5, &mut profile), 0);
    }

    #[test]
    fn a_complex_fold_keeps_the_buffer_of_the_profile() {
        let series: Vec<Complex32> = (0..64).map(|i| Complex32::new(i as f32, -(i as f32))).collect();
        let mut profile = Vec::with_capacity(16);
        let (at, cap) = (profile.as_ptr(), profile.capacity());
        for period in [16, 7, 1, 16] {
            fold_complex(&series, period, &mut profile);
            assert_eq!(profile.len(), period);
            assert_eq!((profile.as_ptr(), profile.capacity()), (at, cap));
        }
    }

}
