//! Occupied channels in an averaged power spectrum: runs of bins that stand over the local noise, of any modulation, with
//! their width and level. A carrier alone is no channel; a carrier with two sidebands of modulation is one.
//!
//! The input is the sum (or the mean) of the power spectra of `frames` frames, lowest frequency first, bin `bins / 2` at
//! the middle of the block, bins `bin_hz` apart.
//!
//! | Step | What it does |
//! |---|---|
//! | Noise | the mean noise per bin of each window of about `floor_hz` from its lower quartile ([`BinMedian`]); between the middles of the windows the noise of a bin is on the line through the two nearest, also past the outer middles, so a slope of the passband is followed |
//! | Hot | a bin is hot when it is `rise_db` over its noise, and at least `min_sigma` deviations of the averaged noise over it |
//! | Runs | hot bins with gaps of at most `gap_hz` between them are one run ([`bridged_runs`]) |
//! | Sides | the mean power of the cold bins beside each end of a run, over the width of one gap; a run at an end of the block has no such bins on one side and is left out; so is a run whose sides differ by `rise_db` or more, which is the edge of a wide signal or of a filter and not a channel |
//! | Parts | a run is split where its power, smoothed over one gap, has a dip `split_db` under the crests on both sides: each part is judged alone, and the power under it is the noise or the line between its dips where that is more, so a weak channel in the skirts of strong neighbours is measured against those skirts |
//! | Extent | the channel of a part is the part from its first to its last bin that is hot and whose power over the noise is within `skirt_db` of that of the strongest bin of the run |
//! | Width | a channel from `min_width_hz` to `max_width_hz` wide is kept when, outside `lobe_bins` on each side of its strongest bin, at least `min_fill` of its bins are hot and within `skirt_db` of the strongest; a carrier lights only the main lobe of the window around it |
//! | AM | a part that is no channel by these rules is one when its strongest bin is a carrier with a pair of sidebands. The sidebands are measured in blocks of one gap outward from the main lobe, by the mean of a block over the noise, which finds sidebands under the noise of one bin. The two sides of the first block are within a factor of 2 and the carrier is 10 times over them. Blocks count while both sides hold 0.5 of the first and half that rise, and two or more must: the skirt of keying falls as the square of the distance and does not. At least `min_fill` of the bins stand half the mean of the sidebands over the noise: lines of a steady tone do not. The first block stands `min_sigma` deviations of the mean of a block over the level one block past the end of the sidebands, so an error of the noise estimate is no sideband. The width is that of the blocks that count; sidebands that hold out to `max_width_hz` are a wider signal and no channel |
//!
//! The skirt that keying spreads beside a strong carrier stands over the noise across some kHz but falls more than
//! `skirt_db` under the peak a few bins from it, so the extent follows the shape of a signal and not its level. Power
//! more than `skirt_db` under a carrier is listed only as the pair of sidebands of that carrier.
//!
//! A ripple of the passband under `rise_db` from trough to crest gives nothing. Signals over more than about two thirds of
//! a window raise its noise and are missed.
//!
//! ```
//! use sdrkit_algo_modem::occupancy::{OccupancyConfig, OccupancyFinder};
//! let mut f = OccupancyFinder::new(2048, 76.3, OccupancyConfig::default()).unwrap();
//! let mut p = vec![1.0f32; 2048];
//! p[500..600].fill(8.0);
//! let mut found = Vec::new();
//! f.find(&p, 100, &mut found);
//! assert_eq!((found.len(), found[0].lo, found[0].hi), (1, 500, 599));
//! ```

use crate::{ModemError, Param};
use sdrkit_algo_dsp::bandwidth::bridged_runs;
use sdrkit_algo_dsp::floor::BinMedian;

/// The quantile of the bins of a window that gives its noise: a channel of a third of the window moves it little.
const FLOOR_QUANTILE: f64 = 0.25;
/// The noise on the line past the outer window middles stays within this factor of the noise of the outer windows.
const MOST_SLOPE: f32 = 2.0;
/// The most frames for which the quantile of averaged noise is computed; with more it is within 1 % of the mean.
const MOST_FRAMES: u32 = 4096;

/// A carrier stands at least this many times over the mean of its sidebands in one bin: all the power of the sidebands
/// of AM is at most half that of the carrier, over 8 bins or more.
const CARRIER_OVER: f32 = 10.0;
/// The two sidebands of a carrier differ by at most this factor in the block next to it.
const SIDES_ALIKE: f32 = 2.0;
/// A sideband block counts while both sides stand at this share of the first block or more, and at half the rise of
/// a block that counts as the first. The skirt of keying falls
/// as the square of the distance, to 0.15 in the second block; the sidebands of speech hold about 0.6.
const SIDEBAND_HOLD: f32 = 0.5;

/// The settings of an [`OccupancyFinder`].
#[derive(Clone, Debug, PartialEq)]
pub struct OccupancyConfig {
    /// The width of a noise window, Hz: several times the widest channel.
    pub floor_hz: f64,
    /// A bin is hot this far over its noise, dB.
    pub rise_db: f32,
    /// A bin is hot at least this many standard deviations of the averaged noise over it.
    pub min_sigma: f32,
    /// The widest gap of cold bins inside a run, Hz.
    pub gap_hz: f64,
    /// The narrowest channel, Hz.
    pub min_width_hz: f64,
    /// The widest channel, Hz.
    pub max_width_hz: f64,
    /// The least share of hot bins in a channel, outside the main lobe around its strongest bin.
    pub min_fill: f64,
    /// A channel ends where its power over the noise falls this far under that of its strongest bin, dB.
    pub skirt_db: f32,
    /// A run is split at a dip of its smoothed power this far under the crests on both sides of it, dB.
    pub split_db: f32,
    /// The bins on each side of a bin that the window lights with it (the main lobe).
    pub lobe_bins: usize,
    /// The fewest frames averaged.
    pub min_frames: u32,
    /// The most channels given.
    pub most: usize,
}

impl Default for OccupancyConfig {
    /// Channels of 2 to 30 kHz that stand 3 dB over the noise, for frames with a Blackman-Harris window.
    fn default() -> OccupancyConfig {
        OccupancyConfig {
            floor_hz: 100e3,
            rise_db: 3.0,
            min_sigma: 5.0,
            gap_hz: 1000.0,
            min_width_hz: 2000.0,
            max_width_hz: 30e3,
            min_fill: 0.5,
            skirt_db: 30.0,
            split_db: 6.0,
            lobe_bins: 4,
            min_frames: 8,
            most: 64,
        }
    }
}

/// An occupied channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Occupied {
    /// The centre of its power over the noise, from the middle of the block (bin `bins / 2`), Hz.
    pub offset_hz: f64,
    /// The width from its first to its last bin, Hz.
    pub width_hz: f64,
    /// Its power over the noise, against the noise in its width, dB.
    pub snr_db: f32,
    /// Its strongest bin over the noise of one bin, dB.
    pub peak_db: f32,
    /// The first bin: hot and within `skirt_db` of the strongest.
    pub lo: usize,
    /// The last bin.
    pub hi: usize,
}

/// Finds the occupied channels of averaged spectra of one size (module docs).
#[derive(Clone, Debug)]
pub struct OccupancyFinder {
    cfg: OccupancyConfig,
    bins: usize,
    bin_hz: f64,
    floor: Vec<f32>,
    smooth: Vec<f32>,
    quantile: BinMedian,
    frames: u32,
}

/// The estimator of the mean noise from the quantile of bins averaged over `frames` frames, with room for `len` bins.
fn quantile_of(frames: u32, len: usize) -> Result<BinMedian, ModemError> {
    let mut q = BinMedian::with_quantile(frames.clamp(1, MOST_FRAMES), FLOOR_QUANTILE)
        .map_err(ModemError::Dsp)?;
    q.reserve(len);
    Ok(q)
}

impl OccupancyFinder {
    /// A finder for spectra of `bins` bins `bin_hz` apart. Errors for fewer than 64 bins, a bin width, window width or
    /// gap that is not positive, a rise that is not positive, or a width range that is empty.
    pub fn new(
        bins: usize,
        bin_hz: f64,
        cfg: OccupancyConfig,
    ) -> Result<OccupancyFinder, ModemError> {
        if bins < 64 {
            return Param::Bins.range(64.0, f64::MAX, bins as f64);
        }
        Param::BinWidth.check_positive(bin_hz)?;
        Param::Span.check_positive(cfg.floor_hz)?;
        Param::Threshold.check_positive(cfg.rise_db)?;
        Param::Threshold.check_positive(cfg.skirt_db)?;
        Param::Threshold.check_positive(cfg.split_db)?;
        Param::Width.check_positive(cfg.min_width_hz)?;
        if !(cfg.gap_hz.is_finite() && cfg.gap_hz >= 0.0) {
            return Param::Width.not_allowed(cfg.gap_hz);
        }
        if cfg.max_width_hz.is_nan() || cfg.max_width_hz < cfg.min_width_hz {
            return Err(ModemError::Conflict {
                param: Param::Width,
                with: Param::Width,
            });
        }
        let windows = ((bins as f64 * bin_hz / cfg.floor_hz).round() as usize).clamp(1, bins / 32);
        Ok(OccupancyFinder {
            quantile: quantile_of(1, bins.div_ceil(windows))?,
            frames: 1,
            floor: vec![0.0; windows],
            smooth: vec![0.0; bins],
            bins,
            bin_hz,
            cfg,
        })
    }

    /// The settings.
    pub fn config(&self) -> &OccupancyConfig {
        &self.cfg
    }

    /// The first bin of noise window `i`.
    fn edge(&self, i: usize) -> usize {
        i * self.bins / self.floor.len()
    }

    /// The noise of bin `b` from the noise of the windows.
    fn noise(&self, b: usize) -> f32 {
        let w = self.floor.len();
        if w == 1 {
            return self.floor[0];
        }
        let at = (b as f32 + 0.5) * w as f32 / self.bins as f32 - 0.5;
        let i = (at.max(0.0) as usize).min(w - 2);
        let (a, c) = (self.floor[i], self.floor[i + 1]);
        let line = a + (at - i as f32) * (c - a);
        line.clamp(a.min(c) / MOST_SLOPE, a.max(c) * MOST_SLOPE)
    }

    /// The occupied channels of `power`, the sum or the mean of the power of `frames` frames, the strongest first, into
    /// `out` (cleared first). Nothing for a spectrum of another size or of fewer than `min_frames` frames. Allocates
    /// only when `frames` differs from the call before, or `out` grows.
    pub fn find(&mut self, power: &[f32], frames: u32, out: &mut Vec<Occupied>) {
        out.clear();
        if power.len() != self.bins || frames < self.cfg.min_frames.max(1) {
            return;
        }
        if frames.min(MOST_FRAMES) != self.frames {
            let Ok(q) = quantile_of(frames, self.bins.div_ceil(self.floor.len())) else {
                return;
            };
            self.quantile = q;
            self.frames = frames.min(MOST_FRAMES);
        }
        for i in 0..self.floor.len() {
            let chunk = &power[self.edge(i)..self.edge(i + 1)];
            self.floor[i] = self
                .quantile
                .estimate(chunk.iter().copied())
                .unwrap_or(self.quantile.min());
        }
        let rise = 10f32.powf(self.cfg.rise_db / 10.0);
        let rise = rise.max(1.0 + self.cfg.min_sigma / (frames as f32).sqrt());
        let gap = (self.cfg.gap_hz / self.bin_hz).round() as usize;
        let block = gap.max(1);
        let look = Look {
            power,
            rise,
            skirt: 10f32.powf(-self.cfg.skirt_db / 10.0),
            block,
            block_rise: self.cfg.min_sigma / (frames as f32 * block as f32).sqrt(),
        };
        let mut smooth = std::mem::take(&mut self.smooth);
        let this = &*self;
        let hot = |b: usize| power[b] > rise * this.noise(b);
        let depth = 10f32.powf(this.cfg.split_db / 10.0);
        let side = gap + 1;
        for (run_lo, run_hi) in bridged_runs((0..self.bins).map(hot), gap) {
            if run_lo < side || run_hi + side >= self.bins {
                continue;
            }
            let below: f32 = power[run_lo - side..run_lo].iter().sum();
            let above: f32 = power[run_hi + 1..=run_hi + side].iter().sum();
            if below.max(above) >= rise * below.min(above) {
                continue;
            }
            smooth_into(&mut smooth, power, run_lo, run_hi, gap / 2);
            let mut lo = run_lo;
            let mut base_lo = None;
            let mut each = |part: Part| {
                let found = this
                    .channel(&look, &part)
                    .or_else(|| this.carrier_channel(&look, &part));
                out.extend(found);
            };
            // The strongest smoothed bin of the part so far, and the weakest since the power fell `depth` under it.
            let mut crest = smooth[run_lo];
            let mut dip: Option<(usize, f32)> = None;
            for (b, &v) in smooth.iter().enumerate().take(run_hi + 1).skip(run_lo) {
                match dip {
                    None => {
                        crest = crest.max(v);
                        if v * depth <= crest {
                            dip = Some((b, v));
                        }
                    }
                    Some((_, least)) if v < least => dip = Some((b, v)),
                    Some((at, least)) if v >= least * depth => {
                        let base_hi = Some(least);
                        each(Part {
                            lo,
                            hi: at,
                            base_lo,
                            base_hi,
                        });
                        (lo, base_lo, crest, dip) = (at + 1, base_hi, v, None);
                    }
                    Some(_) => {}
                }
            }
            each(Part {
                lo,
                hi: run_hi,
                base_lo,
                base_hi: None,
            });
        }
        self.smooth = smooth;
        out.sort_by(|a, b| b.snr_db.total_cmp(&a.snr_db));
        out.truncate(self.cfg.most);
    }

    /// The power under a channel at bin `b` of `part`: the noise, or the line between the dips at the ends of the part
    /// where that is more. Past an end it is the level of that end.
    fn base(&self, part: &Part, b: usize) -> f32 {
        let noise = self.noise(b);
        if part.base_lo.is_none() && part.base_hi.is_none() {
            return noise;
        }
        let a = part.base_lo.unwrap_or_else(|| self.noise(part.lo));
        let c = part.base_hi.unwrap_or_else(|| self.noise(part.hi));
        let span = (part.hi - part.lo).max(1) as f32;
        let at = (b.saturating_sub(part.lo) as f32 / span).min(1.0);
        noise.max(a + at * (c - a))
    }

    /// The strongest bin of `part` and its power against the base.
    fn top(&self, look: &Look<'_>, part: &Part) -> (usize, f32) {
        let (mut top, mut top_over) = (part.lo, 0.0f32);
        for b in part.lo..=part.hi {
            let over = look.power[b] / self.base(part, b);
            if over > top_over {
                (top, top_over) = (b, over);
            }
        }
        (top, top_over)
    }

    /// The channel of the bins `lo..=hi` around a strongest bin that stands `top_over` times over the base.
    fn measured(
        &self,
        look: &Look<'_>,
        part: &Part,
        (lo, hi): (usize, usize),
        top_over: f32,
    ) -> Occupied {
        let (mut signal, mut noise, mut moment) = (0.0f64, 0.0f64, 0.0f64);
        for b in lo..=hi {
            let n = self.base(part, b);
            let over = f64::from(look.power[b] - n).max(0.0);
            signal += over;
            noise += f64::from(n);
            moment += over * (b - lo) as f64;
        }
        let mid = (self.bins / 2) as f64;
        Occupied {
            offset_hz: (lo as f64 + moment / signal.max(f64::MIN_POSITIVE) - mid) * self.bin_hz,
            width_hz: (hi - lo + 1) as f64 * self.bin_hz,
            snr_db: (10.0 * (signal / noise).max(1e-30).log10()) as f32,
            peak_db: 10.0 * (top_over - 1.0).max(1e-30).log10(),
            lo,
            hi,
        }
    }

    /// The channel of `part` by the rules of extent, width and fill (module docs), if it is one.
    fn channel(&self, look: &Look<'_>, part: &Part) -> Option<Occupied> {
        let (top, top_over) = self.top(look, part);
        let level = (top_over - 1.0) * look.skirt;
        let lit = |b: usize| {
            let base = self.base(part, b);
            look.power[b] > look.rise * base && look.power[b] / base - 1.0 >= level
        };
        let lo = (part.lo..=part.hi).find(|&b| lit(b))?;
        let hi = (part.lo..=part.hi).rev().find(|&b| lit(b))?;
        let width = (hi - lo + 1) as f64 * self.bin_hz;
        if !(self.cfg.min_width_hz..=self.cfg.max_width_hz).contains(&width) {
            return None;
        }
        let lobe = top.saturating_sub(self.cfg.lobe_bins)..=top + self.cfg.lobe_bins;
        let beside = (lo..=hi).filter(|b| !lobe.contains(b));
        let (cells, filled) = beside.fold((0u32, 0u32), |(cells, filled), b| {
            (cells + 1, filled + u32::from(lit(b)))
        });
        if cells == 0 || f64::from(filled) < self.cfg.min_fill * f64::from(cells) {
            return None;
        }
        Some(self.measured(look, part, (lo, hi), top_over))
    }

    /// The mean power over the base, less 1, of sideband block `k` below (`upper` false) or above the bin `top`. `None`
    /// past an end of the spectrum.
    fn sideband(
        &self,
        look: &Look<'_>,
        part: &Part,
        top: usize,
        k: usize,
        upper: bool,
    ) -> Option<f32> {
        let near = self.cfg.lobe_bins + 1 + k * look.block;
        let far = near + look.block - 1;
        let (lo, hi) = if upper {
            (top + near, top + far)
        } else {
            (top.checked_sub(far)?, top - near)
        };
        if hi >= self.bins {
            return None;
        }
        let (mut signal, mut base) = (0.0f32, 0.0f32);
        for b in lo..=hi {
            signal += look.power[b];
            base += self.base(part, b);
        }
        Some(signal / base - 1.0)
    }

    /// The channel of a carrier with two sidebands of modulation around the strongest bin of `part` (module docs, AM),
    /// if there is one.
    fn carrier_channel(&self, look: &Look<'_>, part: &Part) -> Option<Occupied> {
        let (top, top_over) = self.top(look, part);
        let sides = |k: usize| {
            Some((
                self.sideband(look, part, top, k, false)?,
                self.sideband(look, part, top, k, true)?,
            ))
        };
        let (below, above) = sides(0)?;
        let (less, more) = (below.min(above), below.max(above));
        if less <= 0.0 || more > SIDES_ALIKE * less || top_over - 1.0 < CARRIER_OVER * more {
            return None;
        }
        let first = 0.5 * (below + above);
        let hold = (SIDEBAND_HOLD * first).max(0.5 * look.block_rise);
        let most = (self.cfg.max_width_hz / self.bin_hz) as usize;
        let reach = |blocks: usize| self.cfg.lobe_bins + blocks * look.block;
        let mut blocks = 1;
        let mut sum = first;
        while let Some((b, a)) = sides(blocks).filter(|(b, a)| b.min(*a) >= hold) {
            // Modulation has an end: sidebands that hold to the widest channel are a wider signal under the carrier.
            if 2 * reach(blocks + 1) >= most {
                return None;
            }
            sum += 0.5 * (b + a);
            blocks += 1;
        }
        // The level past the sidebands, one block beyond the block that ended them: an error of the noise estimate
        // stands there too, sidebands do not.
        let (b, a) = sides(blocks + 1)?;
        if first - 0.5 * (b + a) < look.block_rise {
            return None;
        }
        let (lo, hi) = (top - reach(blocks), top + reach(blocks));
        let width = (hi - lo + 1) as f64 * self.bin_hz;
        if blocks < 2 || width < self.cfg.min_width_hz {
            return None;
        }
        // A bin of a filled sideband stands at least half the mean of the sidebands over the base.
        let half = 1.0 + 0.5 * sum / blocks as f32;
        let lobe = top - self.cfg.lobe_bins..=top + self.cfg.lobe_bins;
        let beside = (lo..=hi).filter(|b| !lobe.contains(b));
        let (cells, filled) = beside.fold((0u32, 0u32), |(cells, filled), b| {
            let lit = look.power[b] >= half * self.base(part, b);
            (cells + 1, filled + u32::from(lit))
        });
        if f64::from(filled) < self.cfg.min_fill * f64::from(cells) {
            return None;
        }
        Some(self.measured(look, part, (lo, hi), top_over))
    }
}

/// The spectrum of one call of [`OccupancyFinder::find`] with the levels derived from the settings.
struct Look<'a> {
    power: &'a [f32],
    /// A bin is hot this many times over its base.
    rise: f32,
    /// The skirt level as a ratio.
    skirt: f32,
    /// The bins of one sideband block: one gap.
    block: usize,
    /// The first sideband block stands this far over the level past the sidebands, as a share of the base.
    block_rise: f32,
}

/// A part of a run between two dips, with the smoothed power of the dip at each end that is one.
struct Part {
    lo: usize,
    hi: usize,
    base_lo: Option<f32>,
    base_hi: Option<f32>,
}

/// Writes into `smooth[lo..=hi]` the mean of `power` over `half` bins on each side of each bin.
fn smooth_into(smooth: &mut [f32], power: &[f32], lo: usize, hi: usize, half: usize) {
    let last = power.len() - 1;
    let mut sum: f64 = power[lo.saturating_sub(half)..=(lo + half).min(last)]
        .iter()
        .map(|&p| f64::from(p))
        .sum();
    for b in lo..=hi {
        let (from, to) = (b.saturating_sub(half), (b + half).min(last));
        smooth[b] = (sum / (to - from + 1) as f64) as f32;
        if b >= half {
            sum -= f64::from(power[b - half]);
        }
        if b + half < last {
            sum += f64::from(power[b + half + 1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    const BINS: usize = 16_384;
    const BIN_HZ: f64 = 76.3;
    const FRAMES: u32 = 152;

    /// The mean of [`FRAMES`] frames of noise whose mean power in bin `b` is `shape(b)`.
    fn spectrum(seed: u64, shape: impl Fn(usize) -> f64) -> Vec<f32> {
        spectrum_of(seed, FRAMES, shape)
    }

    /// As [`spectrum`], the mean of `frames` frames.
    fn spectrum_of(seed: u64, frames: u32, shape: impl Fn(usize) -> f64) -> Vec<f32> {
        let mut rng = Rng::new(seed);
        (0..BINS)
            .map(|b| {
                let sum: f64 = (0..frames).map(|_| rng.exponential()).sum();
                (shape(b) * sum / f64::from(frames)) as f32
            })
            .collect()
    }

    fn find(p: &[f32]) -> Vec<Occupied> {
        let mut f = OccupancyFinder::new(BINS, BIN_HZ, OccupancyConfig::default()).unwrap();
        let mut out = Vec::new();
        f.find(p, FRAMES, &mut out);
        out
    }

    /// A passband that falls by 6 dB over the block, with a ripple of 2 dB from trough to crest every 400 kHz.
    fn passband(b: usize) -> f64 {
        let x = b as f64 / BINS as f64;
        let ripple = (std::f64::consts::TAU * x * BINS as f64 * BIN_HZ / 400e3).sin();
        10f64.powf((-6.0 * x + ripple) / 10.0)
    }

    /// 1 inside a channel of `width_hz` around bin `at`, else 0.
    fn channel(b: usize, at: usize, width_hz: f64) -> f64 {
        let half = (width_hz / BIN_HZ / 2.0) as usize;
        f64::from(u8::from(b.abs_diff(at) <= half))
    }

    #[test]
    fn noise_alone_and_a_sloped_rippled_passband_give_nothing() {
        for seed in 0..8 {
            assert_eq!(find(&spectrum(seed, |_| 1.0)), []);
            assert_eq!(find(&spectrum(100 + seed, passband)), []);
        }
    }

    #[test]
    fn a_channel_on_a_sloped_passband_is_given_with_its_width_and_level() {
        for (at, width_hz, over) in [(3000, 12.5e3, 4.0), (9000, 6e3, 3.0), (14_000, 25e3, 10.0)] {
            let p = spectrum(at as u64, |b| {
                passband(b) * (1.0 + over * channel(b, at, width_hz))
            });
            let c = find(&p);
            assert_eq!(c.len(), 1, "{c:?}");
            let off = (at as f64 - (BINS / 2) as f64) * BIN_HZ;
            assert!((c[0].offset_hz - off).abs() < 0.1 * width_hz, "{c:?}");
            assert!((c[0].width_hz / width_hz - 1.0).abs() < 0.1, "{c:?}");
            let level = 10.0 * over.log10();
            assert!((f64::from(c[0].snr_db) - level).abs() < 1.0, "{c:?}");
        }
    }

    /// The main lobe of a Blackman-Harris window around bin `at`: 3.5 dB down one bin away, 56 dB down four bins away.
    fn lobe(b: usize, at: usize) -> f64 {
        let d = b.abs_diff(at) as f64;
        if d <= 4.0 {
            10f64.powf(-0.35 * d * d)
        } else {
            0.0
        }
    }

    /// A skirt beside the main lobe around bin `at` that is `edge` of the peak four bins away and falls as the square of
    /// the distance, as the keying of a carrier spreads it.
    fn skirt(b: usize, at: usize, edge: f64) -> f64 {
        let d = b.abs_diff(at) as f64;
        if d < 4.0 {
            0.0
        } else {
            edge * (4.0 / d).powi(2)
        }
    }

    #[test]
    fn a_carrier_alone_is_no_channel_and_a_carrier_inside_a_channel_keeps_it() {
        let carrier = |b: usize, at: usize| 1e5 * lobe(b, at);
        assert_eq!(find(&spectrum(1, |b| 1.0 + carrier(b, 5000))), []);
        // Three carriers 1.5 kHz apart are one run, with under half of the bins beside the strongest lit.
        let three = |b| 1.0 + carrier(b, 5000) + carrier(b, 5020) + carrier(b, 5040);
        assert_eq!(find(&spectrum(2, three)), []);
        // A carrier 22 dB over the power per bin of its channel.
        let voice = |b| 1.0 + 1e3 * lobe(b, 5000) + 6.0 * channel(b, 5010, 8e3);
        let c = find(&spectrum(3, voice));
        assert_eq!(c.len(), 1, "{c:?}");
        assert!((c[0].width_hz / 8e3 - 1.0).abs() < 0.1, "{c:?}");
        assert!(c[0].peak_db > 29.0, "{c:?}");
        // With the carrier 42 dB over it, a band on both sides of the carrier is its pair of sidebands: the width is
        // that of the part that both sides fill.
        let low = |b| 1.0 + carrier(b, 5000) + 6.0 * channel(b, 5010, 8e3);
        let c = find(&spectrum(4, low));
        assert_eq!(c.len(), 1, "{c:?}");
        assert!((5e3..=8e3).contains(&c[0].width_hz), "{c:?}");
        assert!(c[0].peak_db > 49.0, "{c:?}");
        // A band on one side of the carrier only is under the skirt level of the carrier and no sideband pair.
        let beside = |b| 1.0 + carrier(b, 5000) + 6.0 * channel(b, 5060, 8e3);
        assert_eq!(find(&spectrum(4, beside)), []);
    }

    #[test]
    fn a_carrier_on_a_sloped_rippled_passband_or_on_a_wide_signal_has_no_sidebands() {
        // Bins from a trough over a steep rise to a crest of the ripple, in looks of 2 and of 6 s.
        for frames in [FRAMES, 3 * FRAMES] {
            for (seed, at) in (3000..8300).step_by(330).enumerate() {
                let p = spectrum_of(seed as u64, frames, |b| passband(b) * (1.0 + 1e4 * lobe(b, at)));
                let mut f = OccupancyFinder::new(BINS, BIN_HZ, OccupancyConfig::default()).unwrap();
                let mut out = Vec::new();
                f.find(&p, frames, &mut out);
                assert_eq!(out, [], "carrier at bin {at}, {frames} frames");
            }
        }
        // A carrier 30 dB over a signal of 45 kHz around it, and beside the middle of it.
        for at in [5000, 5100] {
            let wide = |b| 1.0 + 1e4 * lobe(b, at) + 10.0 * channel(b, 5000, 45e3);
            assert_eq!(find(&spectrum(at as u64, wide)), [], "carrier at bin {at}");
        }
    }

    #[test]
    fn the_skirt_of_a_strong_keyed_or_pulsed_carrier_is_no_channel() {
        // Skirts 32 dB (keyed) and 24 dB (pulses of 25 ms) under the peak four bins away; a steady carrier has none.
        for edge in [0.0, 6e-4, 4e-3] {
            for level_db in [40.0, 60.0, 80.0] {
                let level = 10f64.powf(level_db / 10.0);
                let p = spectrum(level_db as u64, |b| {
                    1.0 + level * (lobe(b, 5000) + skirt(b, 5000, edge))
                });
                assert_eq!(find(&p), [], "{edge} at {level_db} dB");
            }
        }
    }

    #[test]
    fn a_strong_channel_is_given_with_its_width_beside_a_weak_one() {
        // 12.5 kHz at 60 dB over the noise per bin with a skirt that starts 30 dB under it beside its edges, and 6 kHz
        // at 7 dB 25 kHz away.
        let half = (12.5e3 / BIN_HZ / 2.0) as usize;
        let p = spectrum(5, |b| {
            let d = b.abs_diff(5000);
            let strong = if d <= half {
                1.0
            } else {
                1e-3 * (4.0 / (d - half + 3) as f64).powi(2)
            };
            1.0 + 1e6 * strong + 5.0 * channel(b, 5000 + 328, 6e3)
        });
        let c = find(&p);
        assert_eq!(c.len(), 2, "{c:?}");
        assert!((c[0].width_hz / 12.5e3 - 1.0).abs() < 0.1, "{c:?}");
        assert!((c[1].width_hz / 6e3 - 1.0).abs() < 0.1, "{c:?}");
    }

    #[test]
    fn a_run_too_wide_too_narrow_or_at_the_end_of_the_block_is_left_out() {
        assert_eq!(
            find(&spectrum(1, |b| 1.0 + 5.0 * channel(b, 5000, 45e3))),
            []
        );
        assert_eq!(
            find(&spectrum(2, |b| 1.0 + 5.0 * channel(b, 5000, 1e3))),
            []
        );
        assert_eq!(find(&spectrum(3, |b| 1.0 + 5.0 * channel(b, 40, 10e3))), []);
        assert_eq!(
            find(&spectrum(4, |b| 1.0 + 5.0 * channel(b, BINS - 40, 10e3))),
            []
        );
    }

    #[test]
    fn the_edge_of_a_filter_or_of_a_wide_signal_is_no_channel() {
        // The noise falls by 30 dB at a place from 2 to 30 kHz past the edge of a noise window, and rises again.
        let w = BINS / 13;
        for past in [30usize, 100, 200, 390] {
            let fall = |b: usize| if b < 6 * w + past { 1.0 } else { 1e-3 };
            assert_eq!(find(&spectrum(past as u64, fall)), [], "{past} bins past");
            let rise = |b: usize| if b < 6 * w - past { 1e-3 } else { 1.0 };
            assert_eq!(find(&spectrum(past as u64, rise)), [], "{past} bins before");
            // A signal 20 dB over the noise and three windows wide.
            let wide = |b: usize| {
                if (4 * w - past..7 * w + past).contains(&b) {
                    100.0
                } else {
                    1.0
                }
            };
            assert_eq!(find(&spectrum(past as u64, wide)), [], "{past} bins wider");
        }
    }

    #[test]
    fn too_few_frames_or_a_spectrum_of_another_size_give_nothing() {
        let p = spectrum(1, |b| 1.0 + 5.0 * channel(b, 5000, 10e3));
        let mut f = OccupancyFinder::new(BINS, BIN_HZ, OccupancyConfig::default()).unwrap();
        let mut out = Vec::new();
        f.find(&p, 7, &mut out);
        assert_eq!(out, []);
        f.find(&p[1..], FRAMES, &mut out);
        assert_eq!(out, []);
        f.find(&p, FRAMES, &mut out);
        assert_eq!(out.len(), 1);
    }

    /// Signals made as samples: the rate, the frame size and the look of a survey block.
    mod made {
        use super::super::*;
        use sdrkit_algo_dsp::fft::WindowedFft;
        use sdrkit_algo_dsp::window::Window;
        use sdrkit_algo_dsp::Complex32;
        use sdrkit_algo_sim::modulate::{Am, Fm};
        pub use sdrkit_algo_sim::rng::Rng;
        use sdrkit_algo_sim::voice::Voice;
        use std::f64::consts::TAU;

        pub const RATE: f64 = 156_250.0;
        pub const N: usize = 2048;
        pub const LOOK: usize = 152 * N;
        pub const RASTER_HZ: f64 = 12.5e3;

        fn tone(at_hz: f64, i: usize) -> Complex32 {
            let p = TAU * (at_hz * i as f64 / RATE).fract() + 0.3;
            Complex32::new(p.cos() as f32, p.sin() as f32)
        }

        fn voice(rng: &mut Rng) -> Vec<f32> {
            let phase = rng.uniform();
            let mut v = Voice::new(RATE, 0.4, Rng::new(rng.next_u64()))
                .unwrap()
                .with_phase(phase);
            let mut audio = vec![0f32; LOOK];
            v.fill(&mut audio);
            audio
        }

        fn shifted(mut x: Vec<Complex32>, at_hz: f64, gain_db: f64) -> Vec<Complex32> {
            let g = 10f64.powf(gain_db / 20.0) as f32;
            for (i, y) in x.iter_mut().enumerate() {
                *y *= tone(at_hz, i) * g;
            }
            x
        }

        /// Narrow FM voice of 2.5 kHz peak deviation at `at_hz`, `gain_db` against a carrier of power 1.
        pub fn fm(at_hz: f64, gain_db: f64, rng: &mut Rng) -> Vec<Complex32> {
            let mut out = vec![Complex32::default(); LOOK];
            let audio = voice(rng);
            Fm::new(RATE, 2.5e3).unwrap().process(&audio, &mut out).unwrap();
            shifted(out, at_hz, gain_db)
        }

        /// AM voice of depth `depth` at `at_hz`.
        pub fn am(at_hz: f64, depth: f64, rng: &mut Rng) -> Vec<Complex32> {
            let mut out = vec![Complex32::default(); LOOK];
            let audio = voice(rng);
            Am::new(depth).unwrap().process(&audio, &mut out).unwrap();
            shifted(out, at_hz, 0.0)
        }

        /// A carrier at `at_hz` keyed in random marks and spaces of 60 or 180 ms with edges of 5 ms.
        pub fn keyed(at_hz: f64, rng: &mut Rng) -> Vec<Complex32> {
            let unit = (0.06 * RATE) as usize;
            let edge = 0.005 * RATE;
            let mut out = Vec::with_capacity(LOOK);
            let mut on = false;
            while out.len() < LOOK {
                on = !on;
                let len = unit * if rng.bit() == 1 { 3 } else { 1 };
                for k in 0..len {
                    let ramp = ((k as f64).min((len - k) as f64) / edge).min(1.0);
                    let e = if on { 0.5 - 0.5 * (std::f64::consts::PI * ramp).cos() } else { 0.0 };
                    out.push(tone(at_hz, out.len()) * e as f32);
                }
            }
            out.truncate(LOOK);
            out
        }

        /// A steady carrier at each of `at_hz`.
        pub fn carriers(at_hz: &[f64]) -> Vec<Complex32> {
            (0..LOOK).map(|i| at_hz.iter().map(|&f| tone(f, i)).sum()).collect()
        }

        /// The sum of signals.
        pub fn sum(parts: &[Vec<Complex32>]) -> Vec<Complex32> {
            (0..LOOK).map(|i| parts.iter().map(|p| p[i]).sum()).collect()
        }

        /// The mean power spectrum of `x` with noise that a carrier of power 1 stands `snr_db` over in one bin.
        pub fn spectrum(mut x: Vec<Complex32>, snr_db: f64, rng: &mut Rng) -> Vec<f32> {
            let power = N as f64 / 10f64.powf(snr_db / 10.0);
            for v in x.iter_mut() {
                *v += rng.complex_normal(power);
            }
            let mut fft = WindowedFft::new(N, N, Window::BlackmanHarris4).unwrap();
            let mut total = vec![0f32; N];
            for chunk in x.as_chunks::<N>().0 {
                for (k, &p) in fft.run_power(chunk).iter().enumerate() {
                    total[(k + N / 2) % N] += p;
                }
            }
            total
        }

        /// The channels of `x` as [`spectrum`] gives it.
        pub fn find(x: Vec<Complex32>, snr_db: f64, rng: &mut Rng) -> Vec<Occupied> {
            let p = spectrum(x, snr_db, rng);
            let mut f = OccupancyFinder::new(N, RATE / N as f64, OccupancyConfig::default()).unwrap();
            let mut out = Vec::new();
            f.find(&p, (LOOK / N) as u32, &mut out);
            out
        }

        /// The level of a carrier over the noise of one bin that stands `snr_db` over the noise of one raster, dB.
        pub fn in_channel(snr_db: f64) -> f64 {
            snr_db + 10.0 * (RASTER_HZ / (RATE / N as f64)).log10()
        }
    }

    /// The channel within half a raster of `at_hz`, if there is exactly one.
    fn near(found: &[Occupied], at_hz: f64) -> Option<Occupied> {
        let mut near = found
            .iter()
            .filter(|c| (c.offset_hz - at_hz).abs() <= made::RASTER_HZ / 2.0);
        near.next().copied().filter(|_| near.next().is_none())
    }

    #[test]
    fn a_weak_channel_between_two_strong_ones_a_raster_away_is_given() {
        use made::*;
        // Narrow FM voice 20 dB under two neighbours at 30 to 60 dB in their channels: from 40 dB up the skirts of the
        // neighbours stand over the noise in the gaps and the three are one run.
        for seed in 0..6 {
            let mut rng = Rng::new(seed);
            let at = rng.range(-40e3, 40e3);
            for strong in [30.0, 40.0, 50.0, 60.0] {
                let x = sum(&[
                    fm(at - RASTER_HZ, 0.0, &mut rng),
                    fm(at, -20.0, &mut rng),
                    fm(at + RASTER_HZ, 0.0, &mut rng),
                ]);
                let c = find(x, in_channel(strong), &mut rng);
                assert_eq!(c.len(), 3, "seed {seed} at {strong} dB: {c:?}");
                for (f, level) in [(at - RASTER_HZ, strong), (at, strong - 20.0), (at + RASTER_HZ, strong)] {
                    let one = near(&c, f).unwrap_or_else(|| panic!("seed {seed} at {strong} dB: {c:?}"));
                    assert!((one.offset_hz - f).abs() < 500.0, "seed {seed}: {c:?}");
                    assert!((3e3..=8e3).contains(&one.width_hz), "seed {seed}: {c:?}");
                    // Against the noise or, in the skirts of the neighbours, against those.
                    assert!(f64::from(one.snr_db) > level.min(18.0) - 5.0, "seed {seed}: {c:?}");
                }
            }
        }
    }

    #[test]
    fn am_voice_is_given_with_the_width_of_its_sidebands() {
        use made::*;
        // The carrier stands 10 to 50 dB over the noise of one raster. At a depth of 0.5 and 10 dB the sidebands are
        // 8 dB under the noise in each bin: given in most looks, and in every one at full depth.
        let mut weak = 0;
        for seed in 0..10 {
            let mut rng = Rng::new(100 + seed);
            let at = rng.range(-40e3, 40e3);
            for (depth, snr) in [(1.0, 10.0), (0.5, 20.0), (1.0, 20.0), (0.5, 30.0), (1.0, 30.0), (0.5, 50.0)] {
                let c = find(am(at, depth, &mut rng), in_channel(snr), &mut rng);
                assert_eq!(c.len(), 1, "seed {seed} depth {depth} at {snr} dB: {c:?}");
                assert!((c[0].offset_hz - at).abs() < 300.0, "seed {seed}: {c:?}");
                assert!((2e3..=8e3).contains(&c[0].width_hz), "seed {seed} depth {depth} at {snr} dB: {c:?}");
            }
            let c = find(am(at, 0.5, &mut rng), in_channel(10.0), &mut rng);
            assert!(c.len() <= 1, "seed {seed}: {c:?}");
            weak += c.len();
        }
        assert!(weak >= 6, "{weak} of 10");
    }

    #[test]
    fn a_keyed_carrier_a_steady_carrier_and_steady_tones_are_no_channel() {
        use made::*;
        for seed in 0..6 {
            let mut rng = Rng::new(200 + seed);
            let at = rng.range(-40e3, 40e3);
            let tones: Vec<f64> = (-2..=2).map(|k| at + 1e3 * f64::from(k)).collect();
            for snr in [20.0, 30.0, 40.0, 60.0, 80.0] {
                let name = format!("seed {seed} at {snr} dB");
                assert_eq!(find(keyed(at, &mut rng), snr, &mut rng), [], "keyed, {name}");
                assert_eq!(find(carriers(&[at]), snr, &mut rng), [], "carrier, {name}");
                assert_eq!(find(carriers(&tones), snr, &mut rng), [], "tones, {name}");
                // A carrier with AM of a steady tone of 1 kHz: lines, not filled sidebands.
                let lines = carriers(&[at - 1e3, at, at, at, at, at + 1e3]);
                assert_eq!(find(lines, snr + 12.0, &mut rng), [], "am of a tone, {name}");
            }
        }
    }
}
