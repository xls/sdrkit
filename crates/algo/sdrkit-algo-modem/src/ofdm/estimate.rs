//! Channel estimation from pilots, common phase and timing tracking, and the zero-forcing equaliser with channel state information.

use super::framer::rotator;
use super::{OfdmLayout, Pilot};
use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::approx;

/// A pilot-based channel estimator. It takes the used cells of each symbol in order and returns, possibly a few symbols later, the
/// cells of one symbol with the channel `H` of every used carrier and the noise variance per cell.
///
/// Symbols are numbered by the caller (the number picks the pilot pattern phase); a gap in the numbering restarts the estimator.
pub trait ChannelEstimator {
    /// Symbols between a symbol going in and its estimate coming out.
    fn delay(&self) -> usize;
    /// Takes the used cells (`layout.used()` values) of symbol `symbol`. Returns whether an estimate is ready (for symbol
    /// [`output_symbol`](Self::output_symbol)).
    fn push(&mut self, symbol: u64, cells: &[Complex32]) -> Result<bool, ModemError>;
    /// The number of the symbol the current estimate belongs to.
    fn output_symbol(&self) -> u64;
    /// The cells of that symbol.
    fn cells(&self) -> &[Complex32];
    /// The channel of every used carrier for that symbol.
    fn h(&self) -> &[Complex32];
    /// The noise variance per cell (complex, `E|n|^2`), from the pilots.
    fn noise(&self) -> f32;
    /// Forgets the history.
    fn reset(&mut self);
}

/// The noise variance per cell from the least-squares values `ls[j] = y_j / P_j` of neighbouring pilots of one symbol: the residual
/// of a `[1/4, 1/2, 1/4]` smoother has `0.375` of the LS noise variance when the channel is close to linear over three pilots; each
/// residual is scaled back by its pilot power. `None` with fewer than three pilots.
fn pilot_noise(ls: &[Complex32], pilots: &[Pilot]) -> Option<f32> {
    if ls.len() < 3 {
        return None;
    }
    let mut acc = 0.0f64;
    for j in 1..ls.len() - 1 {
        let s = ls[j - 1] * 0.25 + ls[j] * 0.5 + ls[j + 1] * 0.25;
        acc += ((ls[j] - s).norm_sqr() * pilots[j].value.norm_sqr()) as f64;
    }
    Some((acc / (0.375 * (ls.len() - 2) as f64)) as f32)
}

/// Linear interpolation over the used carriers from estimates at sorted indices `at` (values `v`), holding the end values beyond
/// the first and last.
fn interpolate(at: &[u32], v: &[Complex32], h: &mut [Complex32]) {
    if at.is_empty() {
        h.fill(Complex32::new(1.0, 0.0));
        return;
    }
    let n = h.len();
    let first = at[0] as usize;
    h[..first.min(n)].fill(v[0]);
    for j in 0..at.len() - 1 {
        let (a, b) = (at[j] as usize, at[j + 1] as usize);
        let d = (b - a) as f32;
        for (k, hh) in h[a..b].iter_mut().enumerate() {
            let t = k as f32 / d;
            *hh = v[j] * (1.0 - t) + v[j + 1] * t;
        }
    }
    let last = *at.last().unwrap_or(&0) as usize;
    if last < n {
        h[last..].fill(v[at.len() - 1]);
    }
}

/// Least squares at the pilots of each symbol, linear interpolation over frequency, optionally smoothed with `[1/4, 1/2, 1/4]`
/// over neighbouring pilots (for dense pilots: LTE, NR, 802.11 when the channel is smooth). No delay. Suits layouts with pilots in
/// every symbol.
///
/// Mean square error at a pilot: `sigma^2 / |P|^2` raw, `0.375 sigma^2 / |P|^2` smoothed (flat channel); between pilots at fraction
/// `t` of the way, `((1 - t)^2 + t^2)` times the pilot value.
#[derive(Clone, Debug)]
pub struct PilotLs {
    layout: OfdmLayout,
    smooth: bool,
    ls: Vec<Complex32>,
    sm: Vec<Complex32>,
    at: Vec<u32>,
    cells: Vec<Complex32>,
    h: Vec<Complex32>,
    noise: f32,
    symbol: u64,
}

impl PilotLs {
    /// An estimator for `layout`, smoothing over pilots when `smooth`.
    pub fn new(layout: &OfdmLayout, smooth: bool) -> Self {
        let maxp = (0..layout.period() as u64)
            .map(|l| layout.pilots(l).len())
            .max()
            .unwrap_or(0);
        PilotLs {
            layout: layout.clone(),
            smooth,
            ls: vec![Complex32::default(); maxp],
            sm: vec![Complex32::default(); maxp],
            at: vec![0; maxp],
            cells: vec![Complex32::default(); layout.used()],
            h: vec![Complex32::new(1.0, 0.0); layout.used()],
            noise: 0.0,
            symbol: 0,
        }
    }
}

impl ChannelEstimator for PilotLs {
    fn delay(&self) -> usize {
        0
    }

    fn push(&mut self, symbol: u64, cells: &[Complex32]) -> Result<bool, ModemError> {
        let used = self.layout.used();
        if cells.len() < used {
            return Err(ModemError::InputTooShort { need: used });
        }
        let pilots = self.layout.pilots(symbol);
        let np = pilots.len();
        for (j, p) in pilots.iter().enumerate() {
            self.ls[j] = cells[p.index as usize] / p.value;
            self.at[j] = p.index;
        }
        if let Some(n) = pilot_noise(&self.ls[..np], pilots) {
            self.noise = n;
        }
        if self.smooth && np >= 3 {
            self.sm[0] = self.ls[0];
            self.sm[np - 1] = self.ls[np - 1];
            for j in 1..np - 1 {
                self.sm[j] = self.ls[j - 1] * 0.25 + self.ls[j] * 0.5 + self.ls[j + 1] * 0.25;
            }
            interpolate(&self.at[..np], &self.sm[..np], &mut self.h);
        } else {
            interpolate(&self.at[..np], &self.ls[..np], &mut self.h);
        }
        self.cells.copy_from_slice(&cells[..used]);
        self.symbol = symbol;
        Ok(true)
    }

    fn output_symbol(&self) -> u64 {
        self.symbol
    }

    fn cells(&self) -> &[Complex32] {
        &self.cells
    }

    fn h(&self) -> &[Complex32] {
        &self.h
    }

    fn noise(&self) -> f32 {
        self.noise
    }

    fn reset(&mut self) {
        self.h.fill(Complex32::new(1.0, 0.0));
        self.noise = 0.0;
    }
}

/// Scattered pilots interpolated in time, then in frequency: the DVB-T and ISDB-T method (ETSI EN 300 744 clause 4.5.3 places
/// a pilot on every third carrier once every four symbols). For each carrier that carries a pilot somewhere in the pattern, the
/// estimate of symbol `m` is the straight line between its nearest pilots at or before and at or after `m`; then every carrier is
/// interpolated linearly between those carriers. The output is `period - 1` symbols late, so the pilots after `m` have arrived.
///
/// Mean square error (flat channel, LS noise `s2 = sigma^2 / |P|^2`): on a pilot carrier at time fraction `u` between its pilots
/// `((1 - u)^2 + u^2) s2`; between pilot carriers at frequency fraction `t`, `(1 - t)^2 e_a + t^2 e_b` of their errors.
#[derive(Clone, Debug)]
pub struct TimeFreq {
    layout: OfdmLayout,
    period: usize,
    depth: usize,
    /// The used indices that carry pilots somewhere in the pattern.
    grid: Vec<u32>,
    /// Per grid carrier and pattern phase: (symbols back to a pilot, symbols forward to one).
    reach: Vec<(u8, u8)>,
    /// LS values per ring slot and used index (only pilot positions are written).
    ls: Vec<Complex32>,
    ring: Vec<Complex32>,
    gv: Vec<Complex32>,
    tmp: Vec<Complex32>,
    out_cells: Vec<Complex32>,
    h: Vec<Complex32>,
    noise: f32,
    first: u64,
    next: u64,
    started: bool,
    out_symbol: u64,
}

impl TimeFreq {
    /// An estimator for `layout`. Errors when some pilot carrier has no pilot within `period - 1` symbols either side (the pattern
    /// is not a regular scattered grid) or the layout has no pilots.
    pub fn new(layout: &OfdmLayout) -> Result<Self, ModemError> {
        let period = layout.period();
        let used = layout.used();
        let mut has = vec![vec![false; used]; period];
        for (l, row) in has.iter_mut().enumerate() {
            for p in layout.pilots(l as u64) {
                row[p.index as usize] = true;
            }
        }
        let grid: Vec<u32> = (0..used as u32)
            .filter(|&i| (0..period).any(|l| has[l][i as usize]))
            .collect();
        if grid.is_empty() {
            return Param::Pilots.at_least(1.0, 0.0);
        }
        let mut reach = Vec::with_capacity(grid.len() * period);
        for &g in &grid {
            for ph in 0..period {
                let back = (0..period).find(|&b| has[(ph + period - b) % period][g as usize]);
                let fwd = (0..period).find(|&f| has[(ph + f) % period][g as usize]);
                match (back, fwd) {
                    (Some(b), Some(f)) => reach.push((b as u8, f as u8)),
                    _ => return Param::Pilots.degenerate(),
                }
            }
        }
        let depth = 2 * period - 1;
        Ok(TimeFreq {
            layout: layout.clone(),
            period,
            depth,
            reach,
            ls: vec![Complex32::default(); depth * used],
            ring: vec![Complex32::default(); depth * used],
            gv: vec![Complex32::default(); grid.len()],
            tmp: vec![Complex32::default(); used],
            grid,
            out_cells: vec![Complex32::default(); used],
            h: vec![Complex32::new(1.0, 0.0); used],
            noise: 0.0,
            first: 0,
            next: 0,
            started: false,
            out_symbol: 0,
        })
    }
}

impl ChannelEstimator for TimeFreq {
    fn delay(&self) -> usize {
        self.period - 1
    }

    fn push(&mut self, symbol: u64, cells: &[Complex32]) -> Result<bool, ModemError> {
        let used = self.layout.used();
        if cells.len() < used {
            return Err(ModemError::InputTooShort { need: used });
        }
        if !self.started || symbol != self.next {
            self.started = true;
            self.first = symbol;
        }
        self.next = symbol + 1;
        let slot = (symbol % self.depth as u64) as usize;
        self.ring[slot * used..(slot + 1) * used].copy_from_slice(&cells[..used]);
        let pilots = self.layout.pilots(symbol);
        for (j, p) in pilots.iter().enumerate() {
            let v = cells[p.index as usize] / p.value;
            self.ls[slot * used + p.index as usize] = v;
            self.tmp[j] = v;
        }
        if let Some(n) = pilot_noise(&self.tmp[..pilots.len()], pilots) {
            self.noise = n;
        }
        let d = (self.period - 1) as u64;
        if symbol < self.first + d {
            return Ok(false);
        }
        let m = symbol - d;
        let ph = (m % self.period as u64) as usize;
        for (gi, &g) in self.grid.iter().enumerate() {
            let (b, f) = self.reach[gi * self.period + ph];
            let (b, f) = (b as u64, f as u64);
            let val = |s: u64| self.ls[(s % self.depth as u64) as usize * used + g as usize];
            self.gv[gi] = if b == 0 {
                val(m)
            } else if m < self.first + b {
                // Before the first pilot here: hold the next one.
                val(m + f)
            } else {
                let (a, c) = (val(m - b), val(m + f));
                let u = b as f32 / (b + f) as f32;
                a * (1.0 - u) + c * u
            };
        }
        interpolate(&self.grid, &self.gv, &mut self.h);
        let ms = (m % self.depth as u64) as usize;
        self.out_cells
            .copy_from_slice(&self.ring[ms * used..(ms + 1) * used]);
        self.out_symbol = m;
        Ok(true)
    }

    fn output_symbol(&self) -> u64 {
        self.out_symbol
    }

    fn cells(&self) -> &[Complex32] {
        &self.out_cells
    }

    fn h(&self) -> &[Complex32] {
        &self.h
    }

    fn noise(&self) -> f32 {
        self.noise
    }

    fn reset(&mut self) {
        self.started = false;
        self.h.fill(Complex32::new(1.0, 0.0));
        self.noise = 0.0;
    }
}

/// The largest grid step [`GridInterp`] takes.
pub const MAX_GRID_STEP: usize = 64;

/// Interpolates a channel known on a regular grid of carriers (every `step`-th carrier, as scattered pilots give it after time
/// interpolation) to every carrier between, by a windowed sinc after the channel's mean delay is taken out.
///
/// A path `d` samples late turns the channel by `2 pi d / N` per carrier, so between grid carriers it turns `step` times as far: a
/// straight line between them cannot follow it. The interpolator first measures the mean phase step between neighbouring grid
/// values, `arg sum g[j + 1] conj(g[j])` (the mean delay), takes it off, interpolates what is left, and puts it back. The sinc has
/// `2 half` taps over the grid, windowed by `cos^2(pi x / (2 half + 1))` and normalised to unit gain. The grid ends repeat their
/// end values. Paths within `+-N / (2 step)` samples of the mean delay come back. Nothing is allocated after [`GridInterp::new`].
///
/// ```
/// use sdrkit_algo_modem::ofdm::GridInterp;
/// use sdrkit_algo_modem::Complex32;
/// // A path 700 samples late in an 8192-point FFT, on a grid of every third carrier: 1.6 rad between grid carriers.
/// let h = |k: usize| Complex32::from_polar(1.0, -std::f32::consts::TAU * 700.0 * k as f32 / 8192.0);
/// let grid: Vec<Complex32> = (0..100).map(|j| h(3 * j)).collect();
/// let gi = GridInterp::new(3, 4).unwrap();
/// let mut out = vec![Complex32::default(); 297];
/// gi.fill(&grid, &mut out).unwrap();
/// assert!((out[100] - h(100)).norm() < 1e-3);
/// ```
#[derive(Clone, Debug)]
pub struct GridInterp {
    step: usize,
    half: usize,
    /// `step - 1` rows of `2 half` weights: row `r - 1` is for the carrier `r` past a grid carrier, over grid offsets
    /// `-(half - 1) ..= half`.
    weights: Vec<f32>,
}

impl GridInterp {
    /// An interpolator for a grid of every `step`-th carrier (2 to [`MAX_GRID_STEP`]) with `half` taps either side (1 to 16).
    /// Errors for other values.
    pub fn new(step: usize, half: usize) -> Result<Self, ModemError> {
        Param::GridStep.check_count(step, 2, MAX_GRID_STEP)?;
        Param::Taps.check_count(half, 1, 16)?;
        let taps = 2 * half;
        let mut weights = vec![0f32; (step - 1) * taps];
        let span = (2 * half + 1) as f64;
        for r in 1..step {
            let frac = r as f64 / step as f64;
            let row = &mut weights[(r - 1) * taps..r * taps];
            let mut sum = 0.0;
            let mut w = [0f64; 32];
            for (k, v) in w[..taps].iter_mut().enumerate() {
                let x = k as f64 - (half as f64 - 1.0) - frac;
                let px = std::f64::consts::PI * x;
                let sinc = if x.abs() < 1e-12 { 1.0 } else { px.sin() / px };
                let win = (px / span).cos().powi(2);
                *v = sinc * win;
                sum += *v;
            }
            for (o, v) in row.iter_mut().zip(&w[..taps]) {
                *o = (v / sum) as f32;
            }
        }
        Ok(GridInterp {
            step,
            half,
            weights,
        })
    }

    /// The grid step.
    pub fn step(&self) -> usize {
        self.step
    }

    /// Fills `out` (carrier `c` at `out[c]`, grid value `j` at carrier `j step`) from the grid values `grid`. `out` can be shorter
    /// than the grid covers, or run up to `step - 1` carriers past the last grid value (those are interpolated with the end value
    /// repeated). Returns the mean phase step taken out, in cycles per carrier (a mean delay of `-N` times it, in samples, for an
    /// `N`-point FFT). Errors with fewer than two grid values, an `out` longer than `grid.len() * step`, or a value that is not
    /// finite.
    pub fn fill(&self, grid: &[Complex32], out: &mut [Complex32]) -> Result<f64, ModemError> {
        let m = grid.len();
        if m < 2 {
            return Err(ModemError::InputTooShort { need: 2 });
        }
        if out.len() > m * self.step {
            return Err(ModemError::InputTooShort {
                need: out.len().div_ceil(self.step),
            });
        }
        if grid.iter().any(|g| !(g.re.is_finite() && g.im.is_finite())) {
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        let mut acc = Complex32::default();
        for w in grid.windows(2) {
            acc += w[1] * w[0].conj();
        }
        let slope = if acc.norm_sqr() > 0.0 {
            approx::atan2(acc.im, acc.re) as f64 / std::f64::consts::TAU / self.step as f64
        } else {
            0.0
        };
        // The grid with the mean delay taken off: value k turned by e^(-j 2 pi slope k step). The values are read in increasing
        // order of k (the ends repeat), so one rotator produces every phasor.
        let mut down = rotator(0.0, -slope * self.step as f64);
        let (mut next_k, mut ph) = (0usize, Complex32::new(1.0, 0.0));
        let mut flat = |k: isize| -> Complex32 {
            let k = k.clamp(0, m as isize - 1) as usize;
            while next_k <= k {
                ph = down.next_sample();
                next_k += 1;
            }
            grid[k] * ph
        };
        let taps = 2 * self.half;
        let lead = self.half as isize - 1;
        let mut ring = [Complex32::default(); 32];
        for (k, v) in ring[..taps].iter_mut().enumerate() {
            *v = flat(k as isize - lead);
        }
        let mut have = 0isize;
        let mut back = rotator(0.0, slope);
        for (c, o) in out.iter_mut().enumerate() {
            let j = (c / self.step) as isize;
            let r = c % self.step;
            let turn = back.next_sample();
            if r == 0 {
                // `out` holds at most `m step` carriers, so a grid carrier is always in the grid.
                *o = grid[j as usize];
                continue;
            }
            while have < j {
                // Slide the taps one grid value on.
                ring.copy_within(1..taps, 0);
                have += 1;
                ring[taps - 1] = flat(have + taps as isize - 1 - lead);
            }
            let w = &self.weights[(r - 1) * taps..r * taps];
            let mut h = Complex32::default();
            for (v, &wk) in ring[..taps].iter().zip(w) {
                h += *v * wk;
            }
            *o = h * turn;
        }
        Ok(slope)
    }
}

/// The channel from repeated known training symbols (802.11 long training, LTE/NR-style preambles): `h = mean(y) / known` on every
/// carrier whose known value is non-zero (others get 0). With two or more repeats it also returns the noise variance per cell from
/// their spread (unbiased: divided by `repeats - 1`), the estimate whose error is `noise / (repeats |known|^2)`.
pub fn training_ls(
    repeats: &[&[Complex32]],
    known: &[Complex32],
    h: &mut [Complex32],
) -> Result<Option<f32>, ModemError> {
    let n = known.len();
    if repeats.is_empty() {
        return Param::Training.at_least(1.0, 0.0);
    }
    if h.len() < n {
        return Err(ModemError::OutputTooShort { need: n });
    }
    if repeats.iter().any(|r| r.len() < n) {
        return Err(ModemError::InputTooShort { need: n });
    }
    let r = repeats.len() as f32;
    let (mut acc, mut cnt) = (0.0f64, 0usize);
    for k in 0..n {
        if known[k].norm_sqr() == 0.0 {
            h[k] = Complex32::default();
            continue;
        }
        let mean = repeats.iter().fold(Complex32::default(), |a, x| a + x[k]) / r;
        h[k] = mean / known[k];
        if repeats.len() > 1 {
            acc += repeats
                .iter()
                .map(|x| (x[k] - mean).norm_sqr() as f64)
                .sum::<f64>();
            cnt += 1;
        }
    }
    Ok((repeats.len() > 1 && cnt > 0).then(|| (acc / (cnt as f64 * (r as f64 - 1.0))) as f32))
}

/// Zero-forcing equalisation with channel state information: `z = y / h` and the weight `w = |h|^2 / noise`, the inverse of the noise
/// variance of `z`, ready for `Constellation::demap_block_weighted`. A cell whose `|h|^2` is below `1e-12` of the mean (a deep fade)
/// gets `z = 0` and `w = 0`, an erasure. Weights are capped at `1e9`. Errors on unequal lengths or a noise that is not positive.
pub fn equalise(
    cells: &[Complex32],
    h: &[Complex32],
    noise: f32,
    z: &mut [Complex32],
    w: &mut [f32],
) -> Result<(), ModemError> {
    let n = cells.len();
    if h.len() < n {
        return Err(ModemError::InputTooShort { need: n });
    }
    if z.len() < n || w.len() < n {
        return Err(ModemError::OutputTooShort { need: n });
    }
    if !(noise > 0.0 && noise.is_finite()) {
        return Param::NoiseVariance.positive(noise as f64);
    }
    let mean = h[..n].iter().map(|v| v.norm_sqr()).sum::<f32>() / n.max(1) as f32;
    let floor = 1e-12 * mean;
    for i in 0..n {
        let p = h[i].norm_sqr();
        if p > floor && p.is_finite() {
            z[i] = cells[i] * h[i].conj() / p;
            w[i] = (p / noise).min(1e9);
        } else {
            z[i] = Complex32::default();
            w[i] = 0.0;
        }
    }
    Ok(())
}

/// The common phase and phase slope of a symbol against an expected channel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhaseFit {
    /// Phase at carrier 0 (DC), radians.
    pub common: f32,
    /// Phase change per carrier, radians.
    pub slope: f32,
}

impl PhaseFit {
    /// The timing offset in samples the slope stands for (`slope = -2 pi tau / nfft`): positive when the symbol arrived late.
    pub fn timing(&self, nfft: usize) -> f64 {
        -(self.slope as f64) * nfft as f64 / std::f64::consts::TAU
    }

    /// Removes this phase from used cells whose first carrier is `first` (multiplies by `e^(-j (common + slope k))`).
    pub fn remove(&self, first: i32, cells: &mut [Complex32]) {
        let t = std::f64::consts::TAU;
        rotator(
            -(self.common as f64 + self.slope as f64 * first as f64) / t,
            -(self.slope as f64) / t,
        )
        .mix_in_place(cells);
    }
}

/// Measures the common phase error and the phase slope of one symbol over its pilots by weighted least squares, against the
/// channel `h` expected for it (the DVB-T and ISDB-T tracking method). The phase of every `y_j conj(H_j P_j)` is taken after a first
/// slope from the phase steps between neighbouring pilots and the circular mean are removed (so nothing wraps while the slope stays
/// below pi per pilot spacing), weighted by its magnitude, and fitted with a line over the carrier number. The common phase follows
/// the residual carrier offset and phase noise; the slope follows the sampling clock (see [`PhaseFit::timing`], and [`ClockDrift`]
/// for its rate). Errors on short inputs or fewer than two pilots.
pub fn phase_fit(
    layout: &OfdmLayout,
    symbol: u64,
    cells: &[Complex32],
    h: &[Complex32],
) -> Result<PhaseFit, ModemError> {
    let used = layout.used();
    if cells.len() < used || h.len() < used {
        return Err(ModemError::InputTooShort { need: used });
    }
    let pilots = layout.pilots(symbol);
    let n = pilots.len();
    if n < 2 {
        return Param::Pilots.at_least(2.0, n as f64);
    }
    let z = |j: usize| {
        let i = pilots[j].index as usize;
        cells[i] * (h[i] * pilots[j].value).conj()
    };
    let (mut num, mut den0) = (0f64, 0f64);
    for j in 1..n {
        let d = z(j) * z(j - 1).conj();
        let dk = (pilots[j].index - pilots[j - 1].index) as f64;
        let w = d.norm() as f64;
        num += w * approx::atan2(d.im, d.re) as f64 / dk;
        den0 += w;
    }
    let slope0 = if den0 > 0.0 { num / den0 } else { 0.0 };
    let mut sum = Complex32::default();
    for (j, p) in pilots.iter().enumerate() {
        let k = layout.carrier(p.index as usize) as f64;
        let (sn, cs) = approx::sin_cos(-slope0 * k);
        sum += z(j) * Complex32::new(cs, sn);
    }
    let c0 = approx::atan2(sum.im, sum.re);
    let (mut sw, mut swk, mut swa, mut swkk, mut swka) = (0f64, 0f64, 0f64, 0f64, 0f64);
    for (j, p) in pilots.iter().enumerate() {
        let k = layout.carrier(p.index as usize) as f64;
        let (sn, cs) = approx::sin_cos(-(c0 as f64) - slope0 * k);
        let zz = z(j) * Complex32::new(cs, sn);
        let (w, a) = (zz.norm() as f64, approx::atan2(zz.im, zz.re) as f64);
        sw += w;
        swk += w * k;
        swa += w * a;
        swkk += w * k * k;
        swka += w * k * a;
    }
    let den = sw * swkk - swk * swk;
    if sw <= 0.0 || den.is_nan() || den.abs() < 1e-12 * sw * sw.max(1.0) {
        return Ok(PhaseFit {
            common: c0,
            slope: slope0 as f32,
        });
    }
    let slope = (sw * swka - swk * swa) / den;
    let a = (swa - slope * swk) / sw;
    let pi = std::f64::consts::PI;
    let common = (c0 as f64 + a + pi).rem_euclid(2.0 * pi) - pi;
    Ok(PhaseFit {
        common: common as f32,
        slope: (slope0 + slope) as f32,
    })
}

/// The sampling clock error from the timing offset of successive symbols: a least-squares line through `(sample index, timing)`
/// pairs with exponential forgetting, its slope in parts per million.
#[derive(Clone, Debug)]
pub struct ClockDrift {
    lambda: f64,
    s: [f64; 5],
    origin: Option<f64>,
}

impl ClockDrift {
    /// A fit that forgets with factor `lambda` per update (`0 < lambda <= 1`; 1 keeps everything).
    pub fn new(lambda: f64) -> Result<Self, ModemError> {
        if !(lambda > 0.0 && lambda <= 1.0) {
            return Param::Smoothing.range(0.0, 1.0, lambda);
        }
        Ok(ClockDrift {
            lambda,
            s: [0.0; 5],
            origin: None,
        })
    }

    /// Adds the timing (samples) measured on the symbol that starts at absolute sample `at`.
    pub fn push(&mut self, at: f64, timing: f64) {
        if !(at.is_finite() && timing.is_finite()) {
            return;
        }
        let o = *self.origin.get_or_insert(at);
        let t = at - o;
        let l = self.lambda;
        let s = &mut self.s;
        s[0] = l * s[0] + 1.0;
        s[1] = l * s[1] + t;
        s[2] = l * s[2] + t * t;
        s[3] = l * s[3] + timing;
        s[4] = l * s[4] + t * timing;
    }

    /// The clock error in ppm (positive: the received symbols stretch, the transmitter's clock is slower than assumed), once two
    /// distinct times are in.
    pub fn ppm(&self) -> Option<f64> {
        let s = &self.s;
        let den = s[0] * s[2] - s[1] * s[1];
        (den > 1e-9 * s[2].max(1.0)).then(|| (s[0] * s[4] - s[1] * s[3]) / den * 1e6)
    }

    /// Forgets everything.
    pub fn reset(&mut self) {
        self.s = [0.0; 5];
        self.origin = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ofdm::{FftFramer, OfdmLayout, PilotPattern};
    use sdrkit_algo_sim::rng::Rng;

    fn noisy(
        rng: &mut Rng,
        layout: &OfdmLayout,
        symbol: u64,
        h: impl Fn(usize) -> Complex32,
        n0: f64,
        out: &mut [Complex32],
    ) {
        for (i, o) in out.iter_mut().enumerate() {
            *o = h(i) * Complex32::new(0.7, -0.7) + rng.complex_normal(n0);
        }
        for p in layout.pilots(symbol) {
            let i = p.index as usize;
            out[i] = h(i) * p.value + rng.complex_normal(n0);
        }
    }

    #[test]
    fn pilot_ls_mse_matches_theory() {
        // Pilots of power 1 on every 4th carrier, a flat channel.
        let cells: Vec<(i32, Complex32)> = (-40..=40)
            .step_by(4)
            .map(|c| (c, Complex32::new(1.0, 0.0)))
            .collect();
        let layout = OfdmLayout::new(
            128,
            &[16],
            -40,
            40,
            &[],
            &[],
            &PilotPattern::continual(&cells),
        )
        .unwrap();
        let n0 = 0.05f64;
        let hval = Complex32::new(0.6, 0.8);
        let mut rng = Rng::new(1);
        let mut y = vec![Complex32::default(); layout.used()];
        for smooth in [false, true] {
            let mut est = PilotLs::new(&layout, smooth);
            let mut mse = vec![0.0f64; layout.used()];
            let mut noise = 0.0;
            let trials = 3000;
            for s in 0..trials {
                noisy(&mut rng, &layout, s, |_| hval, n0, &mut y);
                assert!(est.push(s, &y).unwrap());
                for (m, h) in mse.iter_mut().zip(est.h()) {
                    *m += (h - hval).norm_sqr() as f64 / trials as f64;
                }
                noise += est.noise() as f64 / trials as f64;
            }
            assert!((noise / n0 - 1.0).abs() < 0.05, "noise {noise}");
            let pil = if smooth { 0.375 } else { 1.0 };
            // Interior carriers: a pilot at index 4j, cells at fraction t between.
            for (i, m) in mse.iter().enumerate().take(72).skip(8) {
                let t = (i % 4) as f64 / 4.0;
                let th = if smooth {
                    // Two neighbouring smoothed pilots share noise: var((1-t) s_a + t s_b) with cov(s_a, s_b) = 0.25 s2.
                    n0 * ((1.0 - t).powi(2) * pil + t * t * pil + 2.0 * t * (1.0 - t) * 0.25)
                } else {
                    n0 * ((1.0 - t).powi(2) + t * t)
                };
                assert!(
                    (m / th - 1.0).abs() < 0.12,
                    "smooth {smooth} index {i}: {m} vs {th}"
                );
            }
        }
    }

    #[test]
    fn time_frequency_estimate_follows_theory_on_dvbt() {
        let layout = OfdmLayout::dvbt_2k(4).unwrap();
        let mut est = TimeFreq::new(&layout).unwrap();
        assert_eq!(est.delay(), 3);
        let n0 = 0.02f64;
        let used = layout.used();
        // A two-path channel that changes nothing over time: H_k = 1 + 0.5 e^(-j 2 pi k 5 / 2048).
        let hf = |i: usize| {
            let ph = -std::f64::consts::TAU * 5.0 * i as f64 / 2048.0;
            Complex32::new(1.0 + 0.5 * ph.cos() as f32, 0.5 * ph.sin() as f32)
        };
        let mut rng = Rng::new(5);
        let mut y = vec![Complex32::default(); used];
        let mut mse = 0.0f64;
        let mut count = 0;
        let mut theory = 0.0f64;
        // The pilot boost: |P|^2 = 16/9.
        let s2 = n0 / (16.0 / 9.0);
        for s in 0..400u64 {
            noisy(&mut rng, &layout, s, hf, n0, &mut y);
            if est.push(s, &y).unwrap() && s > 10 {
                let m = est.output_symbol();
                assert_eq!(m, s - 3);
                for i in 30..used - 30 {
                    mse += (est.h()[i] - hf(i)).norm_sqr() as f64;
                    count += 1;
                    // Theory: grid carriers at multiples of 3 from k = 0; time fraction from the pattern phase.
                    let k = i; // DVB-T k = used index
                    let e_grid = |g: usize| {
                        // The pilot at carrier g comes in symbols where 3 (l mod 4) = g mod 12, so every 4 symbols.
                        let phase = (g % 12) / 3;
                        let back = (m as usize + 4 - phase) % 4;
                        let u = back as f64 / 4.0;
                        let cont = crate::ofdm::DVBT_2K_CONTINUAL.contains(&(g as u16));
                        if cont {
                            s2
                        } else {
                            s2 * ((1.0 - u).powi(2) + u * u)
                        }
                    };
                    let (ga, t) = (k / 3 * 3, (k % 3) as f64 / 3.0);
                    theory += (1.0 - t).powi(2) * e_grid(ga) + t * t * e_grid(ga + 3);
                }
            }
        }
        let (mse, theory) = (mse / count as f64, theory / count as f64);
        // The channel's curvature between grid carriers adds a little bias.
        assert!(
            (mse / theory - 1.0).abs() < 0.15,
            "mse {mse:e} theory {theory:e}"
        );
        assert!(
            (est.noise() as f64 / n0 - 1.0).abs() < 0.3,
            "{}",
            est.noise()
        );
    }

    #[test]
    fn training_equaliser_and_phase_tracking() {
        let layout = OfdmLayout::ieee80211a();
        let known: Vec<Complex32> = crate::ofdm::IEEE80211A_LTF
            .iter()
            .map(|&v| Complex32::new(v as f32, 0.0))
            .collect();
        let mut rng = Rng::new(9);
        let n0 = 0.01f64;
        let hc =
            |i: usize| Complex32::from_polar(1.0 + 0.3 * (i as f32 * 0.2).cos(), i as f32 * 0.05);
        let mk = |rng: &mut Rng| -> Vec<Complex32> {
            (0..53)
                .map(|i| known[i] * hc(i) + rng.complex_normal(n0))
                .collect()
        };
        let a = mk(&mut rng);
        let mut h = vec![Complex32::default(); 53];
        let mut nsum = 0.0;
        for _ in 0..200 {
            let (a, b) = (mk(&mut rng), mk(&mut rng));
            nsum += training_ls(&[&a, &b], &known, &mut h).unwrap().unwrap() as f64 / 200.0;
        }
        assert!((nsum / n0 - 1.0).abs() < 0.05, "{nsum}");
        assert_eq!(training_ls(&[&a], &known, &mut h).unwrap(), None);
        assert_eq!(h[26], Complex32::default());
        // Equalise a data symbol with a common phase of 0.3 rad and a 1.5-sample timing slope, then track it.
        let mut y: Vec<Complex32> = (0..53).map(|i| hc(i) * Complex32::new(0.7, 0.7)).collect();
        for p in layout.pilots(1) {
            y[p.index as usize] = hc(p.index as usize) * p.value;
        }
        let slope = -std::f32::consts::TAU * 1.5 / 64.0;
        for (i, v) in y.iter_mut().enumerate() {
            *v *= Complex32::from_polar(1.0, 0.3 + slope * layout.carrier(i) as f32);
        }
        let hh: Vec<Complex32> = (0..53).map(hc).collect();
        let fit = phase_fit(&layout, 1, &y, &hh).unwrap();
        assert!(
            (fit.common - 0.3).abs() < 1e-3 && (fit.timing(64) - 1.5).abs() < 1e-3,
            "{fit:?}"
        );
        fit.remove(layout.first(), &mut y);
        let mut z = vec![Complex32::default(); 53];
        let mut w = vec![0f32; 53];
        equalise(&y, &hh, 0.01, &mut z, &mut w).unwrap();
        for &i in layout.data(1) {
            assert!((z[i as usize] - Complex32::new(0.7, 0.7)).norm() < 1e-3);
            assert!((w[i as usize] - hh[i as usize].norm_sqr() / 0.01).abs() < 1e-2);
        }
        // A null channel cell is erased.
        let mut h0 = hh.clone();
        h0[3] = Complex32::default();
        equalise(&y, &h0, 0.01, &mut z, &mut w).unwrap();
        assert_eq!((z[3], w[3]), (Complex32::default(), 0.0));
        assert!(equalise(&y, &hh, 0.0, &mut z, &mut w).is_err());
    }

    #[test]
    fn clock_drift_from_the_timing_slope() {
        // 40 ppm: the timing grows 40e-6 samples per sample.
        let mut d = ClockDrift::new(1.0).unwrap();
        assert!(d.ppm().is_none());
        for k in 0..100 {
            let at = 5000.0 + k as f64 * 2560.0;
            d.push(at, 0.2 + 40e-6 * (at - 5000.0));
        }
        assert!((d.ppm().unwrap() - 40.0).abs() < 1e-6);
        assert!(ClockDrift::new(0.0).is_err());
        let _ = FftFramer::new(&OfdmLayout::ieee80211a(), 4).unwrap();
    }

    #[test]
    fn the_grid_interpolator_follows_echoes_a_line_cannot() {
        // ISDB-T mode 3 (8192 points) with a 0 dB echo 700 samples late, on a grid of every third carrier: between grid carriers
        // the second path turns 1.6 rad, too far for a straight line.
        let n = 8192.0;
        let h = |k: usize| {
            Complex32::new(1.0, 0.0)
                + Complex32::from_polar(1.0, -std::f32::consts::TAU * 700.0 * k as f32 / n as f32)
        };
        let m = 600;
        let grid: Vec<Complex32> = (0..m).map(|j| h(3 * j)).collect();
        let gi = GridInterp::new(3, 4).unwrap();
        let mut out = vec![Complex32::default(); 3 * (m - 1) + 1];
        let slope = gi.fill(&grid, &mut out).unwrap();
        // The mean delay is half way between the paths: 350 samples.
        assert!((slope * n + 350.0).abs() < 2.0, "{}", slope * n);
        let err = |c: usize, v: Complex32| (v - h(c)).norm();
        // Away from the grid's ends (where the sinc runs short) the error is small. A straight line is far off.
        let (mut worst, mut worst_line) = (0f32, 0f32);
        for (c, &v) in out.iter().enumerate().take(out.len() - 30).skip(30) {
            worst = worst.max(err(c, v));
            let (j, r) = (c / 3, c % 3);
            let line = grid[j] + (grid[(j + 1).min(m - 1)] - grid[j]) * (r as f32 / 3.0);
            worst_line = worst_line.max(err(c, line));
        }
        assert!(worst < 0.01, "{worst}");
        assert!(
            worst_line > 0.2 && worst_line > 20.0 * worst,
            "{worst_line} {worst}"
        );
        // Grid carriers come back as they were. The end runs up to step - 1 past the last grid value.
        assert_eq!(out[30], grid[10]);
        let mut longer = vec![Complex32::default(); 3 * m];
        gi.fill(&grid, &mut longer).unwrap();
        assert!(longer[3 * m - 1].norm().is_finite());
        // A single path comes back exactly at any delay within the grid's reach.
        let one: Vec<Complex32> = (0..m)
            .map(|j| Complex32::from_polar(1.0, -0.537 * (3 * j) as f32))
            .collect();
        gi.fill(&one, &mut out).unwrap();
        for c in [1usize, 2, 500, 1000] {
            assert!(
                (out[c] - Complex32::from_polar(1.0, -0.537 * c as f32)).norm() < 2e-3,
                "{c}"
            );
        }
        assert!(gi.fill(&grid[..1], &mut out).is_err());
        assert!(gi.fill(&grid[..10], &mut out).is_err());
        assert!(GridInterp::new(1, 4).is_err());
        assert!(GridInterp::new(3, 17).is_err());
    }
}
