//! A budgeted code search: code phase and carrier frequency by FFT circular correlation over a segment of whole code periods. The
//! search runs a slice of work per call, so a caller can spread one search over time. Non-coherent sums move each term by the code
//! drift that the carrier offset causes.
//!
//! Standard: NIST/SEMATECH e-Handbook of Statistical Methods 1.3.6.7.4 (chi-square critical values), for the threshold.

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::peak;

/// The FFT length of one work unit. An FFT of `n` points costs `n / UNIT_POINTS` units.
pub const UNIT_POINTS: usize = 4096;

/// The most frequency offsets inside one FFT bin that a [`SearchGrid`] holds.
pub const MAX_OFFSETS: usize = 8;

/// A stage that costs more than this many units runs as soon as the budget reaches it.
const MIN_STEP_UNITS: f64 = 8.0;

/// Mixes `x` down by `freq` cycles per sample, starting at oscillator phase `phase` cycles, into `out`, and transforms `out` in
/// place. `out` must be as long as `x`.
pub(super) fn mix_transform(
    fwd: &mut Fft,
    x: &[Complex32],
    phase: f64,
    freq: f64,
    out: &mut [Complex32],
) {
    let mut nco = Nco::new(1.0, -freq);
    nco.set_phase_cycles(-phase);
    nco.mix(x, out);
    fwd.process(out);
}

/// Writes (or adds, if `add`) `spec[(k + shift) mod n] * code[k]` to `out[k]`. This moves the spectrum down by `shift` bins and
/// multiplies it by the conjugate code spectrum. All slices have the length `n`, and `shift < n`.
pub(super) fn shift_product(
    spec: &[Complex32],
    shift: usize,
    code: &[Complex32],
    out: &mut [Complex32],
    add: bool,
) {
    let n = spec.len();
    let (low, high) = spec.split_at(shift);
    let (o1, o2) = out.split_at_mut(n - shift);
    let (c1, c2) = code.split_at(n - shift);
    if add {
        o1.iter_mut()
            .zip(high)
            .zip(c1)
            .for_each(|((o, x), c)| *o += x * c);
        o2.iter_mut()
            .zip(low)
            .zip(c2)
            .for_each(|((o, x), c)| *o += x * c);
    } else {
        o1.iter_mut()
            .zip(high)
            .zip(c1)
            .for_each(|((o, x), c)| *o = x * c);
        o2.iter_mut()
            .zip(low)
            .zip(c2)
            .for_each(|((o, x), c)| *o = x * c);
    }
}

/// Adds `|y[(i + d) mod n]|^2 * scale` to `acc[i]`. Both slices have the length `n`, and `d < n`.
pub(super) fn add_power(y: &[Complex32], d: usize, acc: &mut [f32], scale: f32) {
    let n = y.len();
    let (y1, y2) = y.split_at(d);
    let (a1, a2) = acc.split_at_mut(n - d);
    a1.iter_mut()
        .zip(y2)
        .for_each(|(a, v)| *a += v.norm_sqr() * scale);
    a2.iter_mut()
        .zip(y1)
        .for_each(|(a, v)| *a += v.norm_sqr() * scale);
}

/// The detection threshold on a peak-over-mean ratio for a sum of `terms` non-coherent terms in noise.
///
/// In noise, each term is a chi-square variable with 2 degrees of freedom. The sum `S` of `m = terms` such terms of unit mean is
/// Gamma(m, 1). The function returns `x` with `P(S / m > x) = p`: the inverse of the regularised upper incomplete gamma function,
/// divided by `m`. So `2 m x` is the upper critical value of the chi-square distribution with `2m` degrees of freedom at tail `p`
/// (NIST/SEMATECH e-Handbook 1.3.6.7.4). For `m = 1`, `x = -ln p`.
///
/// Errors if `terms` is 0 or above 100 000, or if `p` is not inside (0, 1).
///
/// ```
/// use sdrkit_algo_modem::dsss::gamma_threshold;
/// let x = gamma_threshold(1, 1e-3).unwrap();
/// assert!((x - 1e3f64.ln()).abs() < 1e-9);
/// ```
pub fn gamma_threshold(terms: usize, p: f64) -> Result<f64, ModemError> {
    Param::Terms.check_count(terms, 1, 100_000)?;
    Param::Probability.check_open(p, 0.0, 1.0)?;
    let ln_p = p.ln();
    // ln Q(m, y) with Q(m, y) = e^-y sum_{k<m} y^k / k!, summed in the log domain so that large m does not underflow.
    let ln_q = |y: f64| -> f64 {
        if y <= 0.0 {
            return 0.0;
        }
        let ly = y.ln();
        let (mut lt, mut top) = (-y, -y);
        for k in 1..terms {
            lt += ly - (k as f64).ln();
            top = top.max(lt);
        }
        let mut lt = -y;
        let mut sum = (lt - top).exp();
        for k in 1..terms {
            lt += ly - (k as f64).ln();
            sum += (lt - top).exp();
        }
        top + sum.ln()
    };
    let m = terms as f64;
    let (mut lo, mut hi) = (0.0, m + 10.0);
    while ln_q(hi) > ln_p {
        lo = hi;
        hi *= 2.0;
    }
    // Q falls as y grows: bisect to the precision of f64.
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if ln_q(mid) > ln_p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(0.5 * (lo + hi) / m)
}

/// How a search adds its code periods.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Integration {
    /// Code periods in the segment.
    pub periods: usize,
    /// Consecutive periods added coherently into one term. The terms add non-coherently. `periods` must be a multiple of it.
    pub coherent: usize,
}

impl Integration {
    /// The number of terms that add non-coherently.
    pub fn terms(&self) -> usize {
        self.periods / self.coherent.max(1)
    }

    fn check(&self) -> Result<(), ModemError> {
        Param::Periods.check_count(self.periods, 1, 1024)?;
        if self.coherent == 0 || !self.periods.is_multiple_of(self.coherent) {
            return Param::Coherent.conflict(Param::Periods);
        }
        Ok(())
    }
}

/// The carrier frequencies a search tries: whole FFT bins (a bin is `1 / period` cycles per sample) from `first_bin` on, each
/// with the same offsets inside the bin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchGrid {
    period: usize,
    first_bin: i64,
    bins: usize,
    offsets: [f64; MAX_OFFSETS],
    count: usize,
}

impl SearchGrid {
    /// Every frequency within `centre ± half_width` cycles per sample, for code periods of `period` samples, each bin split into
    /// `sub_bins` equal steps.
    ///
    /// Errors if `period` is below 8, if `sub_bins` is not 1 to 8, or if the window is not finite or reaches half the sample
    /// rate.
    pub fn window(
        period: usize,
        centre: f64,
        half_width: f64,
        sub_bins: usize,
    ) -> Result<Self, ModemError> {
        if period < 8 {
            return Param::Period.at_least(8.0, period as f64);
        }
        Param::SubBins.check_count(sub_bins, 1, MAX_OFFSETS)?;
        Param::Centre.check_finite(centre)?;
        if !(half_width >= 0.0 && centre.abs() + half_width < 0.5) {
            return Param::Width.range(0.0, 0.5 - centre.abs(), half_width);
        }
        let n = period as f64;
        let cb = (centre * n).round() as i64;
        let hb = (half_width * n).ceil() as i64;
        let mut offsets = [0.0; MAX_OFFSETS];
        for (s, o) in offsets.iter_mut().enumerate().take(sub_bins) {
            *o = s as f64 / (sub_bins as f64 * n);
        }
        Ok(SearchGrid {
            period,
            first_bin: cb - hb,
            bins: (2 * hb + 1) as usize,
            offsets,
            count: sub_bins,
        })
    }

    /// `count` frequencies `spacing` cycles per sample apart, centred on `freq`, all from the bin that holds `freq`. This is a
    /// small search to make sure of an earlier hit.
    ///
    /// Errors if `period` is below 8, if `count` is not 1 to 8, or if a value is not finite or reaches half the sample rate.
    pub fn around(
        period: usize,
        freq: f64,
        spacing: f64,
        count: usize,
    ) -> Result<Self, ModemError> {
        if period < 8 {
            return Param::Period.at_least(8.0, period as f64);
        }
        Param::Frequencies.check_count(count, 1, MAX_OFFSETS)?;
        Param::Spacing.check_finite(spacing)?;
        if !(freq.is_finite() && freq.abs() < 0.5) {
            return Param::Frequency.range(-0.5, 0.5, freq);
        }
        let n = period as f64;
        let b = (freq * n).floor() as i64;
        let frac = freq - b as f64 / n;
        let mid = (count as f64 - 1.0) / 2.0;
        let mut offsets = [0.0; MAX_OFFSETS];
        for (i, o) in offsets.iter_mut().enumerate().take(count) {
            *o = frac + (i as f64 - mid) * spacing;
        }
        Ok(SearchGrid {
            period,
            first_bin: b,
            bins: 1,
            offsets,
            count,
        })
    }

    /// The number of frequencies in the grid.
    pub fn points(&self) -> usize {
        self.bins * self.count
    }

    /// The frequency of grid point `j`, cycles per sample. Point `j` is offset `j % offsets` of bin `j / offsets`.
    pub fn freq(&self, j: usize) -> f64 {
        let b = self.first_bin + (j / self.count) as i64;
        b as f64 / self.period as f64 + self.offsets[j % self.count]
    }

    /// The step between neighbouring offsets, or one bin when there is one offset, cycles per sample.
    fn step(&self) -> f64 {
        if self.count > 1 {
            self.offsets[1] - self.offsets[0]
        } else {
            1.0 / self.period as f64
        }
    }
}

/// The settings of one search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchConfig {
    /// How the periods add.
    pub integration: Integration,
    /// The relative change of the code rate per cycle per sample of carrier offset. For a code sent on a carrier, this is the
    /// sample rate over the carrier frequency. Use 0 for no drift compensation.
    pub code_doppler: f64,
    /// A known relative error of the code rate, added to the drift that the carrier offset causes.
    pub code_rate_offset: f64,
    /// The false-alarm probability of the whole search.
    pub p_fa: f64,
    /// Samples per independent code cell (about the samples per chip). The threshold counts `points * period / samples_per_cell`
    /// independent cells.
    pub samples_per_cell: f64,
}

impl SearchConfig {
    /// `integration` with no drift compensation, a false-alarm probability of 1e-3, and one sample per cell.
    pub fn new(integration: Integration) -> Self {
        SearchConfig {
            integration,
            code_doppler: 0.0,
            code_rate_offset: 0.0,
            p_fa: 1e-3,
            samples_per_cell: 1.0,
        }
    }

    fn check(&self) -> Result<(), ModemError> {
        self.integration.check()?;
        Param::CodeDoppler.check_finite(self.code_doppler)?;
        Param::CodeRateOffset.check_finite(self.code_rate_offset)?;
        Param::Probability.check_open(self.p_fa, 0.0, 1.0)?;
        if !(self.samples_per_cell >= 1.0 && self.samples_per_cell.is_finite()) {
            return Param::SamplesPerCell.at_least(1.0, self.samples_per_cell);
        }
        Ok(())
    }
}

/// What a finished search found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchResult {
    /// Where the first code period starts, in samples from the start of the segment (fraction from a parabola through the peak).
    pub code_phase: f64,
    /// The carrier frequency, cycles per sample. If the peak passed the threshold, a parabola through the powers at the
    /// neighbouring frequencies refines it.
    pub freq: f64,
    /// The peak power over the mean power of all cells searched.
    pub ratio: f64,
    /// The ratio that the peak had to pass, from [`gamma_threshold`].
    pub threshold: f64,
    /// Whether `ratio > threshold`.
    pub detected: bool,
    /// The sample index of the peak, `0 .. period`.
    pub peak_index: usize,
}

impl SearchResult {
    /// The signal-to-noise ratio of one coherent term, from the ratio: `ratio - 1`, at least 1e-3.
    pub fn snr(&self) -> f64 {
        (self.ratio - 1.0).max(1e-3)
    }
}

/// One code period sampled at the search rate, with its conjugate spectrum. Make it with [`CodeSearch::replica`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CodeReplica {
    samples: Vec<f32>,
    spectrum: Vec<Complex32>,
}

impl CodeReplica {
    /// The samples of one period.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Samples per period.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether the replica has no samples.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    /// Mix the segment by offset `s` and transform its periods.
    Spectra(usize),
    /// Correlate bin `b` of offset `s`.
    Bin(usize, usize),
    Refine,
    Done,
}

/// The state of one search: its segment, grid, settings, buffers and the best grid point so far. Start it with
/// [`CodeSearch::start`]. A job keeps its buffers from search to search, so a reused job allocates nothing.
#[derive(Clone, Debug)]
pub struct SearchJob {
    grid: SearchGrid,
    cfg: SearchConfig,
    n: usize,
    stage: Stage,
    seg: Vec<Complex32>,
    spectra: Vec<Complex32>,
    tmp: Vec<Complex32>,
    acc: Vec<f32>,
    best_acc: Vec<f32>,
    best: (f32, usize, usize),
    total: f64,
    cells: usize,
    spent: f64,
    result: Option<SearchResult>,
}

impl Default for SearchJob {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchJob {
    /// An empty job with no buffers. It is done until [`CodeSearch::start`] fills it.
    pub fn new() -> Self {
        SearchJob {
            grid: SearchGrid {
                period: 8,
                first_bin: 0,
                bins: 1,
                offsets: [0.0; MAX_OFFSETS],
                count: 1,
            },
            cfg: SearchConfig::new(Integration {
                periods: 1,
                coherent: 1,
            }),
            n: 0,
            stage: Stage::Done,
            seg: Vec::new(),
            spectra: Vec::new(),
            tmp: Vec::new(),
            acc: Vec::new(),
            best_acc: Vec::new(),
            best: (0.0, 0, 0),
            total: 0.0,
            cells: 0,
            spent: 0.0,
            result: None,
        }
    }

    /// The units the whole search costs, without the final refinement.
    pub fn cost(&self) -> f64 {
        let u = self.n as f64 / UNIT_POINTS as f64;
        let i = self.cfg.integration;
        self.grid.count as f64 * (i.periods as f64 + i.terms() as f64 * self.grid.bins as f64) * u
    }

    /// The units spent so far.
    pub fn spent(&self) -> f64 {
        self.spent
    }

    /// The part of the search done, 0 to 1.
    pub fn progress(&self) -> f64 {
        (self.spent / self.cost().max(1e-9)).min(1.0)
    }

    /// The settings of the search.
    pub fn config(&self) -> &SearchConfig {
        &self.cfg
    }

    /// The grid of the search.
    pub fn grid(&self) -> &SearchGrid {
        &self.grid
    }

    /// The summed powers over all code phases at the best grid point so far (`period` values).
    pub fn best_powers(&self) -> &[f32] {
        &self.best_acc
    }

    /// Whether the search is finished.
    pub fn is_done(&self) -> bool {
        self.stage == Stage::Done
    }
}

/// The search engine for one code period length: the FFT plans and a work counter, shared by every code and every job of that
/// length.
///
/// One work unit is an FFT of [`UNIT_POINTS`] points. A search transforms each period once per offset, then per bin one inverse
/// FFT per term. The cost is `offsets * (periods + terms * bins) * period / UNIT_POINTS` units, and a final refinement costs
/// `periods / 2` FFTs.
///
/// ```
/// use sdrkit_algo_modem::dsss::{CodeSearch, Integration, SearchConfig, SearchGrid, SearchJob};
/// use sdrkit_algo_modem::Complex32;
/// // Barker-11 at 8 samples per chip, delayed by 40 samples, 0.3 bins of carrier offset, 4 periods.
/// let n = 88;
/// let samples: Vec<f32> = (0..n).map(|i| sdrkit_algo_modem::dsss::BARKER_11[i / 8] as f32).collect();
/// let mut cs = CodeSearch::new(n).unwrap();
/// let replica = cs.replica(&samples).unwrap();
/// let f = 0.3 / n as f64;
/// let x: Vec<Complex32> = (0..4 * n)
///     .map(|i| Complex32::from_polar(samples[(i + n - 40) % n], (std::f64::consts::TAU * f * i as f64) as f32))
///     .collect();
/// let cfg = SearchConfig::new(Integration { periods: 4, coherent: 1 });
/// let grid = SearchGrid::window(n, 0.0, 2.0 / n as f64, 4).unwrap();
/// let mut job = SearchJob::new();
/// cs.start(&mut job, &x, &grid, &cfg).unwrap();
/// let mut budget = f64::INFINITY;
/// let r = cs.step(&mut job, &replica, &mut budget).unwrap().unwrap();
/// assert!(r.detected && r.peak_index == 40);
/// assert!((r.freq - f).abs() < 0.1 / n as f64);
/// ```
#[derive(Clone, Debug)]
pub struct CodeSearch {
    n: usize,
    fwd: Fft,
    inv: Fft,
    work: f64,
}

impl CodeSearch {
    /// An engine for code periods of `period` samples. Errors if `period` is below 8.
    pub fn new(period: usize) -> Result<Self, ModemError> {
        if period < 8 {
            return Param::Period.at_least(8.0, period as f64);
        }
        Ok(CodeSearch {
            n: period,
            fwd: Fft::forward(period),
            inv: Fft::inverse(period),
            work: 0.0,
        })
    }

    /// Samples per code period.
    pub fn period(&self) -> usize {
        self.n
    }

    /// The units spent by all jobs.
    pub fn work(&self) -> f64 {
        self.work
    }

    /// A replica from one code period sampled at the search rate (`period` finite values). Errors on a wrong length or a value
    /// that is not finite.
    pub fn replica(&mut self, samples: &[f32]) -> Result<CodeReplica, ModemError> {
        if samples.len() != self.n {
            return Err(ModemError::InputTooShort { need: self.n });
        }
        if samples.iter().any(|v| !v.is_finite()) {
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        let mut spectrum: Vec<Complex32> =
            samples.iter().map(|&v| Complex32::new(v, 0.0)).collect();
        self.fwd.process(&mut spectrum);
        spectrum.iter_mut().for_each(|v| *v = v.conj());
        Ok(CodeReplica {
            samples: samples.to_vec(),
            spectrum,
        })
    }

    /// Starts `job` on the segment `x` (its first `periods * period` samples are copied) over `grid` with `cfg`. The job keeps
    /// its buffers, so a reused job allocates nothing when the sizes do not grow.
    ///
    /// Errors if the grid has another period, if a setting is out of range, or if `x` is too short.
    pub fn start(
        &self,
        job: &mut SearchJob,
        x: &[Complex32],
        grid: &SearchGrid,
        cfg: &SearchConfig,
    ) -> Result<(), ModemError> {
        cfg.check()?;
        if grid.period != self.n {
            return Param::Period.length(self.n, grid.period);
        }
        let n = self.n;
        let len = cfg.integration.periods * n;
        if x.len() < len {
            return Err(ModemError::InputTooShort { need: len });
        }
        job.grid = *grid;
        job.cfg = *cfg;
        job.n = n;
        job.stage = Stage::Spectra(0);
        job.seg.clear();
        job.seg.extend_from_slice(&x[..len]);
        job.spectra.resize(len, Complex32::default());
        job.tmp.resize(n, Complex32::default());
        job.acc.resize(n, 0.0);
        job.best_acc.resize(n, 0.0);
        job.best_acc.fill(0.0);
        job.best = (-1.0, 0, 0);
        job.total = 0.0;
        job.cells = 0;
        job.spent = 0.0;
        job.result = None;
        Ok(())
    }

    /// The code drift over one period, samples, for carrier frequency `freq`.
    fn drift(&self, cfg: &SearchConfig, freq: f64) -> f64 {
        -(freq * cfg.code_doppler + cfg.code_rate_offset) * self.n as f64
    }

    /// Spends up to `budget` units on `job` and takes the units spent off `budget`. A stage runs when the budget covers its
    /// cost, or `MIN_STEP_UNITS` (8) units for a stage that costs more. Returns the result once the search finishes, and
    /// `None` before. `replica` must be the same for every step of a job.
    ///
    /// Errors if the replica has another period.
    pub fn step(
        &mut self,
        job: &mut SearchJob,
        replica: &CodeReplica,
        budget: &mut f64,
    ) -> Result<Option<SearchResult>, ModemError> {
        let n = self.n;
        if job.stage != Stage::Done && (replica.len() != n || job.n != n) {
            return Param::Period.length(n, replica.len().max(job.n));
        }
        let unit = n as f64 / UNIT_POINTS as f64;
        let integ = job.cfg.integration;
        let k = integ.periods;
        loop {
            match job.stage {
                Stage::Done => return Ok(job.result.take()),
                Stage::Spectra(sub) => {
                    let cost = k as f64 * unit;
                    if *budget < cost.min(MIN_STEP_UNITS) {
                        return Ok(None);
                    }
                    // Whole bins are spectrum rotations. Mix by the offset only, with the phase continuous over the segment.
                    let off = job.grid.offsets[sub];
                    for p in 0..k {
                        let phase = (off * (p * n) as f64).rem_euclid(1.0);
                        mix_transform(
                            &mut self.fwd,
                            &job.seg[p * n..(p + 1) * n],
                            phase,
                            off,
                            &mut job.spectra[p * n..(p + 1) * n],
                        );
                    }
                    self.spend(job, budget, cost);
                    job.stage = Stage::Bin(sub, 0);
                }
                Stage::Bin(sub, bi) => {
                    let terms = integ.terms();
                    let cost = terms as f64 * unit;
                    if *budget < cost.min(MIN_STEP_UNITS) {
                        return Ok(None);
                    }
                    let b = job.grid.first_bin + bi as i64;
                    let freq = b as f64 / n as f64 + job.grid.offsets[sub];
                    let shift = b.rem_euclid(n as i64) as usize;
                    let drift = self.drift(&job.cfg, freq);
                    job.acc.fill(0.0);
                    for t in 0..terms {
                        // A coherent group adds in the spectrum (the inverse FFT is linear): one transform per term.
                        let p0 = t * integ.coherent;
                        for p in p0..p0 + integ.coherent {
                            shift_product(
                                &job.spectra[p * n..(p + 1) * n],
                                shift,
                                &replica.spectrum,
                                &mut job.tmp,
                                p > p0,
                            );
                        }
                        self.inv.process(&mut job.tmp);
                        let d = ((drift * p0 as f64).round() as i64).rem_euclid(n as i64) as usize;
                        add_power(&job.tmp, d, &mut job.acc, 1.0);
                    }
                    let mut best = (0f32, 0usize);
                    let mut sum = 0f64;
                    for (i, &v) in job.acc.iter().enumerate() {
                        sum += v as f64;
                        if v > best.0 {
                            best = (v, i);
                        }
                    }
                    job.total += sum;
                    job.cells += n;
                    if best.0 > job.best.0 {
                        job.best = (best.0, bi * job.grid.count + sub, best.1);
                        job.best_acc.copy_from_slice(&job.acc);
                    }
                    self.spend(job, budget, cost);
                    job.stage = if bi + 1 < job.grid.bins {
                        Stage::Bin(sub, bi + 1)
                    } else if sub + 1 < job.grid.count {
                        Stage::Spectra(sub + 1)
                    } else {
                        Stage::Refine
                    };
                }
                Stage::Refine => {
                    let cost = 0.5 * k as f64 * unit;
                    if *budget < cost.min(MIN_STEP_UNITS) {
                        return Ok(None);
                    }
                    self.spend(job, budget, cost);
                    job.result = Some(self.finish(job, replica)?);
                    job.stage = Stage::Done;
                }
            }
        }
    }

    fn spend(&mut self, job: &mut SearchJob, budget: &mut f64, cost: f64) {
        *budget -= cost;
        job.spent += cost;
        self.work += cost;
    }

    /// The summed power at carrier frequency `freq` and code phase `tau`, from the segment (for frequencies between grid points).
    fn power_at(&self, job: &mut SearchJob, replica: &CodeReplica, freq: f64, tau: usize) -> f64 {
        let n = self.n;
        let integ = job.cfg.integration;
        let drift = self.drift(&job.cfg, freq);
        let rep = &replica.samples;
        let mut nco = Nco::new(1.0, -freq);
        let mut total = 0.0;
        let mut group = Complex32::default();
        for p in 0..integ.periods {
            nco.mix(&job.seg[p * n..(p + 1) * n], &mut job.tmp);
            let d = (drift * p as f64).round() as i64;
            let start = (tau as i64 + d).rem_euclid(n as i64) as usize;
            // The replica delayed by `start`: sample i meets replica sample (i - start) mod n.
            let (head, tail) = job.tmp.split_at(start);
            let mut acc = (0f64, 0f64);
            for (y, &c) in tail.iter().zip(&rep[..n - start]) {
                acc.0 += (y.re * c) as f64;
                acc.1 += (y.im * c) as f64;
            }
            for (y, &c) in head.iter().zip(&rep[n - start..]) {
                acc.0 += (y.re * c) as f64;
                acc.1 += (y.im * c) as f64;
            }
            group += Complex32::new(acc.0 as f32, acc.1 as f32);
            if (p + 1) % integ.coherent == 0 {
                total += group.norm_sqr() as f64;
                group = Complex32::default();
            }
        }
        total
    }

    fn finish(
        &self,
        job: &mut SearchJob,
        replica: &CodeReplica,
    ) -> Result<SearchResult, ModemError> {
        let n = self.n;
        let (peak, j, idx) = job.best;
        let mean = job.total / job.cells.max(1) as f64;
        let ratio = peak.max(0.0) as f64 / mean.max(1e-30);
        let cells = job.grid.points() as f64 * n as f64 / job.cfg.samples_per_cell;
        let threshold = gamma_threshold(
            job.cfg.integration.terms(),
            (job.cfg.p_fa / cells.max(1.0)).max(f64::MIN_POSITIVE),
        )?;
        let detected = ratio > threshold;
        // Code phase: the correlation peak is a triangle in amplitude, so fit the parabola to the magnitudes.
        let a = job.best_acc[(idx + n - 1) % n].max(0.0) as f64;
        let b = job.best_acc[idx].max(0.0) as f64;
        let c = job.best_acc[(idx + 1) % n].max(0.0) as f64;
        let dcode = peak::parabolic(a.sqrt(), b.sqrt(), c.sqrt()).offset;
        let f0 = job.grid.freq(j);
        let mut freq = f0;
        if detected {
            let step = job.grid.step();
            let ym = self.power_at(job, replica, f0 - step, idx);
            let y0 = self.power_at(job, replica, f0, idx);
            let yp = self.power_at(job, replica, f0 + step, idx);
            freq = f0 + peak::parabolic(ym, y0, yp).offset * step;
        }
        Ok(SearchResult {
            code_phase: (idx as f64 + dcode).rem_euclid(n as f64),
            freq,
            ratio,
            threshold,
            detected,
            peak_index: idx,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// Upper critical values of the chi-square distribution, from NIST/SEMATECH e-Handbook of Statistical Methods, section
    /// 1.3.6.7.4: (degrees of freedom, upper tail probability, critical value).
    const NIST_CHI_SQUARE: [(usize, f64, f64); 8] = [
        (2, 0.05, 5.991),
        (4, 0.05, 9.488),
        (10, 0.01, 23.209),
        (16, 0.01, 32.000),
        (20, 0.001, 45.315),
        (40, 0.05, 55.758),
        (64, 0.001, 104.716),
        (100, 0.01, 135.807),
    ];

    #[test]
    fn gamma_threshold_matches_published_chi_square_values() {
        for p in [0.5, 0.05, 1e-3, 1e-9] {
            let x = gamma_threshold(1, p).unwrap();
            assert!((x + p.ln()).abs() < 1e-9, "m = 1, p = {p}: {x}");
        }
        for (nu, p, crit) in NIST_CHI_SQUARE {
            let m = nu / 2;
            let x = gamma_threshold(m, p).unwrap();
            assert!(
                (2.0 * m as f64 * x - crit).abs() < 6e-4,
                "nu = {nu}, p = {p}: {} against {crit}",
                2.0 * m as f64 * x
            );
        }
        // A large sum does not underflow: the threshold goes to 1 as the terms grow.
        let x = gamma_threshold(20_000, 1e-6).unwrap();
        assert!(x > 1.0 && x < 1.05, "{x}");
        assert!(gamma_threshold(0, 0.1).is_err());
        assert!(gamma_threshold(4, 0.0).is_err());
        assert!(gamma_threshold(4, 1.0).is_err());
        assert!(gamma_threshold(4, f64::NAN).is_err());
    }

    fn gauss(seed: &mut u64) -> f32 {
        let mut u = || {
            *seed ^= *seed << 13;
            *seed ^= *seed >> 7;
            *seed ^= *seed << 17;
            (*seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let (u1, u2) = (u() + 1e-12, u());
        ((-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()) as f32
    }

    /// A 1023-chip maximal-length code from the polynomial x^10 + x^3 + 1, as +1/-1.
    fn mls_1023() -> Vec<f32> {
        let mut r = 1u16;
        (0..1023)
            .map(|_| {
                let out = r & 1;
                let fb = (r ^ (r >> 3)) & 1;
                r = (r >> 1) | (fb << 9);
                if out == 0 {
                    1.0
                } else {
                    -1.0
                }
            })
            .collect()
    }

    /// A segment of `k` periods: the code at 4 samples per chip delayed by `tau`, at carrier `f` cycles per sample, with the code
    /// rate changed by the relative amount `f * ratio`, at `cn0` dB-Hz for a 4.092 MHz rate, in unit noise per component.
    fn segment(code: &[f32], k: usize, tau: f64, f: f64, ratio: f64, cn0: f64) -> Vec<Complex32> {
        let fs = 4.092e6;
        let n = code.len() * 4;
        let amp = (10f64.powf(cn0 / 10.0) * 2.0 / fs).sqrt() as f32;
        let mut seed = 99u64;
        (0..k * n)
            .map(|i| {
                let chip = (i as f64 - tau) * (1.0 + f * ratio) / 4.0;
                let c = code[(chip.floor() as i64).rem_euclid(code.len() as i64) as usize];
                let ph = TAU * f * i as f64;
                Complex32::new(gauss(&mut seed), gauss(&mut seed))
                    + Complex32::from_polar(amp * c, ph as f32)
            })
            .collect()
    }

    fn run(
        cs: &mut CodeSearch,
        job: &mut SearchJob,
        rep: &CodeReplica,
        budget: f64,
    ) -> SearchResult {
        loop {
            let mut b = budget;
            if let Some(r) = cs.step(job, rep, &mut b).unwrap() {
                return r;
            }
        }
    }

    #[test]
    fn finds_a_code_in_noise_with_coherent_pairs() {
        let code = mls_1023();
        let n = 4092;
        let samples: Vec<f32> = (0..n).map(|i| code[i / 4]).collect();
        let mut cs = CodeSearch::new(n).unwrap();
        let rep = cs.replica(&samples).unwrap();
        let (f, tau) = (2345.0 / 4.092e6, 1234.0);
        let x = segment(&code, 16, tau, f, 0.0, 40.0);
        let mut cfg = SearchConfig::new(Integration {
            periods: 16,
            coherent: 2,
        });
        cfg.samples_per_cell = 4.0;
        let grid = SearchGrid::window(n, 0.0, 10e3 / 4.092e6, 4).unwrap();
        let mut job = SearchJob::new();
        cs.start(&mut job, &x, &grid, &cfg).unwrap();
        let r = run(&mut cs, &mut job, &rep, 50.0);
        assert!(r.detected, "{r:?}");
        assert!(((r.freq - f) * 4.092e6).abs() < 100.0, "{r:?}");
        assert!((r.code_phase - tau).abs() < 1.0, "{r:?}");
        assert!((job.spent() - job.cost()).abs() < 0.5 * 16.0 * n as f64 / 4096.0 + 1e-9);
        // The confirmation grid around the hit finds it again.
        let grid = SearchGrid::around(n, r.freq, 0.1 / n as f64, 3).unwrap();
        cs.start(&mut job, &x, &grid, &cfg).unwrap();
        let c = run(&mut cs, &mut job, &rep, 1e9);
        assert!(c.detected && (c.code_phase - tau).abs() < 1.0, "{c:?}");
        // Another code is not found.
        let other: Vec<f32> = (0..n).map(|i| code[(i / 4 * 7) % 1023]).collect();
        let rep2 = cs.replica(&other).unwrap();
        let grid = SearchGrid::window(n, 0.0, 10e3 / 4.092e6, 4).unwrap();
        cs.start(&mut job, &x, &grid, &cfg).unwrap();
        assert!(!run(&mut cs, &mut job, &rep2, 1e9).detected);
    }

    #[test]
    fn drift_compensation_keeps_a_long_sum_together() {
        // A code rate 2e-4 fast (a very large ratio, to make the drift 0.8 samples per period): 64 periods drift 52 samples.
        let code = mls_1023();
        let n = 4092;
        let samples: Vec<f32> = (0..n).map(|i| code[i / 4]).collect();
        let mut cs = CodeSearch::new(n).unwrap();
        let rep = cs.replica(&samples).unwrap();
        let f = 1000.0 / 4.092e6;
        let ratio = 2e-4 / f;
        let x = segment(&code, 64, 500.0, f, ratio, 40.0);
        let grid = SearchGrid::around(n, f, 0.05 / n as f64, 1).unwrap();
        let mut job = SearchJob::new();
        let mut cfg = SearchConfig::new(Integration {
            periods: 64,
            coherent: 1,
        });
        cs.start(&mut job, &x, &grid, &cfg).unwrap();
        let plain = run(&mut cs, &mut job, &rep, 1e9);
        cfg.code_doppler = ratio;
        cs.start(&mut job, &x, &grid, &cfg).unwrap();
        let comp = run(&mut cs, &mut job, &rep, 1e9);
        assert!(
            comp.detected && (comp.code_phase - 500.0).abs() < 1.5,
            "{comp:?}"
        );
        assert!(
            comp.ratio > 2.5 * plain.ratio,
            "{} against {}",
            comp.ratio,
            plain.ratio
        );
    }

    #[test]
    fn budget_slices_and_errors() {
        let n = 64;
        let samples: Vec<f32> = (0..n)
            .map(|i| if (i * 5) % 7 < 3 { 1.0 } else { -1.0 })
            .collect();
        let mut cs = CodeSearch::new(n).unwrap();
        let rep = cs.replica(&samples).unwrap();
        let x: Vec<Complex32> = (0..4 * n)
            .map(|i| Complex32::new(samples[i % n], 0.0))
            .collect();
        let cfg = SearchConfig::new(Integration {
            periods: 4,
            coherent: 2,
        });
        let grid = SearchGrid::window(n, 0.0, 2.0 / n as f64, 2).unwrap();
        let mut job = SearchJob::new();
        cs.start(&mut job, &x, &grid, &cfg).unwrap();
        // A budget below the smallest stage does nothing.
        let mut b = 0.0;
        assert_eq!(cs.step(&mut job, &rep, &mut b).unwrap(), None);
        assert_eq!(job.spent(), 0.0);
        // One stage at a time.
        let mut steps = 0;
        let r = loop {
            let mut b = 4.0 * n as f64 / 4096.0;
            steps += 1;
            if let Some(r) = cs.step(&mut job, &rep, &mut b).unwrap() {
                break r;
            }
        };
        assert!(
            steps > 5 && r.detected && r.peak_index == 0,
            "{steps} {r:?}"
        );
        assert!((r.freq).abs() < 0.05 / n as f64, "{r:?}");
        assert!(job.is_done());
        // Errors, not panics.
        let other = CodeSearch::new(128).unwrap();
        assert!(other.start(&mut job, &x, &grid, &cfg).is_err());
        assert!(cs.start(&mut job, &x[..n], &grid, &cfg).is_err());
        let bad = SearchConfig::new(Integration {
            periods: 3,
            coherent: 2,
        });
        assert!(cs.start(&mut job, &x, &grid, &bad).is_err());
        assert!(cs.replica(&samples[..10]).is_err());
        assert!(CodeSearch::new(4).is_err());
        assert!(SearchGrid::window(n, 0.0, 0.6, 2).is_err());
        assert!(SearchGrid::around(n, 0.0, 0.01, 9).is_err());
        cs.start(&mut job, &x, &grid, &cfg).unwrap();
        let rep_short = CodeSearch::new(32)
            .unwrap()
            .replica(&samples[..32])
            .unwrap();
        let mut b = 1e9;
        assert!(cs.step(&mut job, &rep_short, &mut b).is_err());
    }
}
