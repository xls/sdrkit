//! The slot form of the OFDM structure detector, for numerologies whose symbols in a slot have prefixes of different lengths
//! (LTE uplink and sidelink SC-FDMA, NB-IoT uplink): the prefix correlation folded over the slot, the slot start from the
//! sum at the ends of all symbols of the slot, and the power of each symbol of each frame period (the subframe), which tells
//! busy periods from empty ones and shows an empty last symbol (the guard of a sidelink subframe).
//!
//! The running sum `S(t)` over the last `w` samples of `x(t) conj(x(t - N))` (`w` the shortest prefix) and the power of the same
//! samples are folded over the slot. With a prefix the end of each symbol gives a peak at its last sample. At every report the
//! slot start `o` maximises the sum of `|S|` over the ends of the symbols of the slot; the peak is that sum over the number of
//! symbols, the floor the mean of the bins farther than `w` from every end. The carrier offset is the angle of the folded `S`
//! summed at the ends, less half a turn for a half-carrier shift ([`Numerology::with_half_shift`]).
//!
//! The power is summed in blocks of [`SLOT_BLOCK`] samples. The blocks folded over the frame period (`frame_s`, a whole number of
//! slots) give the frame phase: the frame starts at the slot start whose last symbol holds the least folded power, when that
//! is at least `phase_db` below the other candidates. Each whole frame of the report then gives its power, the power of its
//! last symbol and the mean power of its other symbols except the first.
//!
//! ```
//! use sdrkit_algo_modem::ofdm::slots::SlotDetector;
//! use sdrkit_algo_modem::ofdm::Numerology;
//! // LTE sidelink at 15.36 Msps: 1024-point FFT, prefixes 80 and 6 x 72, a 1 ms subframe of two slots.
//! let n = Numerology::new(15.36e6, 1024, &[80, 72, 72, 72, 72, 72, 72], 1e-3).unwrap().with_half_shift();
//! let d = SlotDetector::new(n).unwrap();
//! assert_eq!(d.ends(), &[1104, 2200, 3296, 4392, 5488, 6584, 7680]);
//! ```

use super::{FrameShape, Numerology, StructureConfig};
use crate::{Complex32, ModemError, Param};
use std::f64::consts::TAU;

/// Samples of a power block. Every prefix and symbol length of the LTE numerologies at 15.36 and 30.72 Msps is a multiple of it.
pub const SLOT_BLOCK: usize = 8;
/// Bins of the frame envelope shape ([`FrameShape`]): the folded blocks summed into this many equal parts.
const SHAPE_BINS: usize = 240;

/// The settings of the frame part of a report.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameConfig {
    /// The two lowest last-symbol powers of the frame candidates must differ by this much for the phase to be known, dB.
    pub phase_db: f64,
    /// A frame is busy when its power exceeds the noise floor by this much, dB.
    pub busy_db: f64,
    /// The quantile of the frame powers (and of the last-symbol powers) that is the noise floor.
    pub floor_quantile: f64,
}

impl Default for FrameConfig {
    fn default() -> Self {
        FrameConfig {
            phase_db: 1.0,
            busy_db: 3.0,
            floor_quantile: 0.2,
        }
    }
}

/// One report of the detector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotReport {
    /// Stream time of the end of the report, s.
    pub time_s: f64,
    /// This report found the numerology.
    pub hit: bool,
    /// A hit in this report or within the hold time.
    pub detected: bool,
    /// No hit, but the peak stands out somewhat.
    pub energy: bool,
    /// The peak over the floor.
    pub ratio: f64,
    /// The prefix correlation, 0 to 1.
    pub correlation: f64,
    /// The slot start: the stream sample index modulo the slot length.
    pub slot_timing: usize,
    /// The carrier offset, Hz (valid on a hit).
    pub cfo_hz: f64,
    /// The mean power per sample.
    pub mean_power: f64,
    /// The frame start modulo the frame length, when the folded power tells it.
    pub frame_timing: Option<usize>,
    /// Whole frames of this report (counted on a hit only).
    pub frames: usize,
    /// Of them, the busy ones.
    pub busy: usize,
    /// In the busy frames, the power of the last symbol over the mean power of symbols 1 to `n - 2`, dB.
    pub last_symbol_db: Option<f64>,
    /// The shape of the folded power over the frame period.
    pub frame: FrameShape,
}

/// The detector (module documentation).
#[derive(Clone, Debug)]
pub struct SlotDetector {
    num: Numerology,
    cfg: StructureConfig,
    fcfg: FrameConfig,
    period: usize,
    window: usize,
    frame_len: usize,
    /// The end (exclusive) of each symbol of the slot, from the slot start.
    ends: Vec<usize>,
    hist: Vec<Complex32>,
    hpos: usize,
    prods: Vec<(Complex32, f32)>,
    ppos: usize,
    s: (f64, f64),
    p: f64,
    n: u64,
    fold_mag: Vec<f64>,
    fold_s: Vec<(f64, f64)>,
    fold_p: Vec<f64>,
    in_report: usize,
    per_report: usize,
    last_hit: Option<f64>,
    block: f64,
    block_n: usize,
    /// The power blocks of this report and the block index of the first.
    blocks: Vec<f32>,
    blocks_first: u64,
    /// The power blocks folded over the frame period.
    env: Vec<f64>,
    env_frames: f64,
}

impl SlotDetector {
    /// A detector of `num` with the default settings.
    ///
    /// # Errors
    ///
    /// As [`SlotDetector::with_config`].
    pub fn new(num: Numerology) -> Result<Self, ModemError> {
        Self::with_config(num, StructureConfig::default(), FrameConfig::default())
    }

    /// A detector of `num` with the decision settings `cfg` and the frame settings `fcfg`.
    ///
    /// # Errors
    ///
    /// [`ModemError`] when the slot is over 2^20 samples, the report period is under one slot, the frame period is not a
    /// whole number of slots, or a symbol or the slot is not a whole number of [`SLOT_BLOCK`]s.
    pub fn with_config(
        num: Numerology,
        cfg: StructureConfig,
        fcfg: FrameConfig,
    ) -> Result<Self, ModemError> {
        let period = num.period();
        Param::Period.check_count(period, 1, 1 << 20)?;
        let per_report = (cfg.report_s * num.rate_hz).round() as usize;
        Param::Window.check_count(per_report, period, usize::MAX)?;
        let frame_len = (num.frame_s * num.rate_hz).round() as usize;
        if frame_len < period || !frame_len.is_multiple_of(period) {
            return Param::Period.conflict(Param::Window);
        }
        let mut ends = Vec::with_capacity(num.prefixes.len());
        let mut e = 0;
        for &cp in &num.prefixes {
            e += cp + num.fft_len;
            if !e.is_multiple_of(SLOT_BLOCK) {
                return Param::Guard.conflict(Param::Period);
            }
            ends.push(e);
        }
        let window = num.window();
        Ok(SlotDetector {
            hist: vec![Complex32::default(); num.fft_len + window],
            hpos: 0,
            prods: vec![(Complex32::default(), 0.0); window],
            ppos: 0,
            s: (0.0, 0.0),
            p: 0.0,
            n: 0,
            fold_mag: vec![0.0; period],
            fold_s: vec![(0.0, 0.0); period],
            fold_p: vec![0.0; period],
            in_report: 0,
            per_report,
            last_hit: None,
            block: 0.0,
            block_n: 0,
            blocks: Vec::with_capacity(per_report / SLOT_BLOCK + 1),
            blocks_first: 0,
            env: vec![0.0; frame_len / SLOT_BLOCK],
            env_frames: 0.0,
            ends,
            num,
            cfg,
            fcfg,
            period,
            window,
            frame_len,
        })
    }

    /// The numerology.
    pub fn numerology(&self) -> &Numerology {
        &self.num
    }

    /// The end of each symbol of the slot, samples from the slot start.
    pub fn ends(&self) -> &[usize] {
        &self.ends
    }

    /// Forgets the stream.
    pub fn reset(&mut self) {
        if let Ok(d) = Self::with_config(self.num.clone(), self.cfg, self.fcfg) {
            *self = d;
        }
    }

    /// Takes samples at the numerology's rate; calls `on_report` at the end of each report period.
    pub fn process(&mut self, x: &[Complex32], mut on_report: impl FnMut(&SlotReport)) {
        let n_fft = self.num.fft_len;
        let hlen = self.hist.len();
        for &v in x {
            let v = if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::default()
            };
            let old = self.hist[(self.hpos + hlen - n_fft) % hlen];
            self.hist[self.hpos] = v;
            self.hpos = (self.hpos + 1) % hlen;
            let prod = v * old.conj();
            let pw = 0.5 * (v.norm_sqr() + old.norm_sqr());
            let (gone, gone_p) = self.prods[self.ppos];
            self.prods[self.ppos] = (prod, pw);
            self.ppos = (self.ppos + 1) % self.window;
            self.s.0 += f64::from(prod.re) - f64::from(gone.re);
            self.s.1 += f64::from(prod.im) - f64::from(gone.im);
            self.p += f64::from(pw) - f64::from(gone_p);
            let bin = (self.n % self.period as u64) as usize;
            self.fold_mag[bin] += self.s.0.hypot(self.s.1);
            self.fold_s[bin].0 += self.s.0;
            self.fold_s[bin].1 += self.s.1;
            self.fold_p[bin] += self.p.max(0.0);
            self.block += f64::from(v.norm_sqr());
            self.block_n += 1;
            self.n += 1;
            if self.block_n == SLOT_BLOCK {
                self.end_block();
            }
            self.in_report += 1;
            if self.in_report == self.per_report {
                let r = self.report();
                on_report(&r);
            }
        }
    }

    fn end_block(&mut self) {
        let b = self.n / SLOT_BLOCK as u64 - 1;
        if self.blocks.is_empty() {
            self.blocks_first = b;
        }
        let p = (self.block / SLOT_BLOCK as f64) as f32;
        self.blocks.push(p);
        let nb = self.env.len();
        self.env[(b % nb as u64) as usize] += f64::from(p);
        if (b + 1).is_multiple_of(nb as u64) {
            self.env_frames += 1.0;
            // Eight frames held: the older half is forgotten.
            if self.env_frames >= 8.0 {
                self.env.iter_mut().for_each(|v| *v *= 0.5);
                self.env_frames *= 0.5;
            }
        }
        self.block = 0.0;
        self.block_n = 0;
    }

    /// The fold bins that the slot start `o` scores: the last sample of every symbol but the last of the slot (a frame may
    /// leave its last symbol empty, a guard), and the first sample of the flat top of each symbol whose prefix is longer than
    /// the window. A flat top is where `|S|` stays at its peak; a slot start one symbol off puts that point on the slope of a
    /// symbol with the shorter prefix.
    fn timing_points(&self, o: usize) -> impl Iterator<Item = usize> + '_ {
        let p = self.period;
        let used = self.ends.len().saturating_sub(1).max(1);
        let ends = self.ends[..used].iter().map(move |&e| (o + e - 1) % p);
        let tops = self
            .ends
            .iter()
            .zip(&self.num.prefixes)
            .filter(|(_, &cp)| cp > self.window)
            .map(move |(&e, &cp)| (o + p + e - 1 - (cp - self.window)) % p);
        ends.chain(tops)
    }

    /// The mean power of the blocks wholly inside stream samples `start .. end`, from the folded frame envelope when
    /// `folded`, else from the blocks of this report. `None` when no block is inside.
    fn power(&self, start: u64, end: u64, folded: bool) -> Option<f64> {
        let b = SLOT_BLOCK as u64;
        let (b0, b1) = (start.div_ceil(b), end / b);
        if b1 <= b0 {
            return None;
        }
        let mut sum = 0.0;
        for k in b0..b1 {
            sum += if folded {
                self.env[(k % self.env.len() as u64) as usize]
            } else {
                let i = k.checked_sub(self.blocks_first)? as usize;
                f64::from(*self.blocks.get(i)?)
            };
        }
        Some(sum / (b1 - b0) as f64)
    }

    /// The frame start among the slot starts `o + k P`, from the folded power of the last symbol of each candidate frame.
    fn frame_phase(&self, o: usize) -> Option<usize> {
        let f = self.frame_len as u64;
        let k = self.ends.len();
        let last = (self.ends[k - 1] - if k > 1 { self.ends[k - 2] } else { 0 }) as u64;
        let mut c: Vec<(usize, f64)> = (0..self.frame_len / self.period)
            .filter_map(|k| {
                let start = (o + k * self.period) as u64 % f;
                // A frame that starts at `start` ends at `start + f`; its last symbol is the last `last` samples. Folded, so
                // any stream index of the same phase serves.
                let end = start + 2 * f;
                self.power(end - last, end, true).map(|p| (start as usize, p))
            })
            .collect();
        if c.len() < 2 {
            return None;
        }
        c.sort_by(|a, b| a.1.total_cmp(&b.1));
        let ratio_db = 10.0 * (c[1].1 / c[0].1.max(1e-30)).log10();
        (ratio_db >= self.fcfg.phase_db).then_some(c[0].0)
    }

    /// The shape of the folded power over the frame period.
    fn frame_shape(&self) -> FrameShape {
        if self.env_frames < 2.0 {
            return FrameShape::Unknown;
        }
        let per = self.env.len().div_ceil(SHAPE_BINS).max(1);
        let bins: Vec<f64> = self.env.chunks(per).map(|c| c.iter().sum::<f64>() / c.len() as f64).collect();
        let mean = bins.iter().sum::<f64>() / bins.len() as f64;
        let max = bins.iter().copied().fold(0.0, f64::max);
        let min = bins.iter().copied().fold(f64::INFINITY, f64::min);
        let over_half = bins.iter().filter(|&&b| b > 0.5 * mean).count();
        if mean > 0.0 && max > 1.6 * mean && (over_half as f64) < 0.8 * bins.len() as f64 {
            let mid = 0.5 * (max + min);
            let on = bins.iter().filter(|&&b| b > mid).count();
            FrameShape::Bursts {
                duty: on as f64 / bins.len() as f64,
            }
        } else {
            FrameShape::Continuous
        }
    }

    /// The frames of this report: (frames, busy, last-symbol power over the mean of the inner symbols in busy frames, dB).
    fn frames(&self, frame_start: usize) -> (usize, usize, Option<f64>) {
        let f = self.frame_len as u64;
        let b = SLOT_BLOCK as u64;
        let first = self.blocks_first * b;
        let end = first + self.blocks.len() as u64 * b;
        let mut start = first - first % f + frame_start as u64;
        if start < first {
            start += f;
        }
        // Each frame: (power, last-symbol power, mean power of the symbols 1 .. n - 2).
        let mut rows = Vec::new();
        let slots = self.frame_len / self.period;
        while start + f <= end {
            let mut syms = Vec::with_capacity(slots * self.ends.len());
            for k in 0..slots {
                let s0 = start + (k * self.period) as u64;
                let mut a = s0;
                for &e in &self.ends {
                    let z = s0 + e as u64;
                    if let Some(p) = self.power(a, z, false) {
                        syms.push(p);
                    }
                    a = z;
                }
            }
            if let (Some(all), true) = (self.power(start, start + f, false), syms.len() >= 3) {
                let inner = &syms[1..syms.len() - 1];
                let mid = inner.iter().sum::<f64>() / inner.len() as f64;
                rows.push((all, syms[syms.len() - 1], mid));
            }
            start += f;
        }
        if rows.is_empty() {
            return (0, 0, None);
        }
        let quantile = |mut v: Vec<f64>| {
            v.sort_by(f64::total_cmp);
            v[((v.len() - 1) as f64 * self.fcfg.floor_quantile).round() as usize]
        };
        let floor = quantile(rows.iter().map(|r| r.0).collect()).min(quantile(rows.iter().map(|r| r.1).collect()));
        let over = 10f64.powf(self.fcfg.busy_db / 10.0);
        let busy: Vec<&(f64, f64, f64)> = rows.iter().filter(|r| r.0 > over * floor).collect();
        let last_db = (!busy.is_empty()).then(|| {
            let l: f64 = busy.iter().map(|r| r.1).sum();
            let m: f64 = busy.iter().map(|r| r.2).sum();
            10.0 * (l.max(1e-30) / m.max(1e-30)).log10()
        });
        (rows.len(), busy.len(), last_db)
    }

    fn report(&mut self) -> SlotReport {
        let time_s = self.n as f64 / self.num.rate_hz;
        let p = self.period;
        let k = self.ends.len();
        let used = k.saturating_sub(1).max(1);
        let (mut best, mut best_o) = (f64::NEG_INFINITY, 0);
        for o in 0..p {
            let v: f64 = self.timing_points(o).map(|t| self.fold_mag[t]).sum();
            if v > best {
                best = v;
                best_o = o;
            }
        }
        let peak = self.ends[..used]
            .iter()
            .map(|&e| self.fold_mag[(best_o + e - 1) % p])
            .sum::<f64>()
            / used as f64;
        let near = |i: usize| {
            self.ends.iter().any(|&e| {
                let at = (best_o + e - 1) % p;
                let d = i.abs_diff(at);
                d.min(p - d) <= self.window
            })
        };
        let (mut far, mut far_n) = (0.0, 0usize);
        for (i, &v) in self.fold_mag.iter().enumerate() {
            if !near(i) {
                far += v;
                far_n += 1;
            }
        }
        let far = far / far_n.max(1) as f64;
        let ratio = if far > 0.0 { peak / far } else { 0.0 };
        let total_p: f64 = self.fold_p.iter().sum();
        let correlation = if total_p > 0.0 {
            (peak * p as f64 / total_p).min(1.0)
        } else {
            0.0
        };
        let mean_power = total_p / (self.in_report.max(1) * self.window) as f64;
        let mean_bin = total_p / p as f64;
        let even = self
            .fold_p
            .iter()
            .all(|&v| v >= self.cfg.min_bin_share * mean_bin);
        let hit = ratio > self.cfg.min_ratio
            && correlation > self.cfg.min_correlation
            && mean_power > self.cfg.min_power
            && even;
        let mut cfo_hz = 0.0;
        let (mut frame_timing, mut frames, mut busy, mut last_symbol_db) = (None, 0, 0, None);
        if hit {
            self.last_hit = Some(time_s);
            let (mut re, mut im) = (0.0, 0.0);
            for &e in &self.ends[..used] {
                let s = self.fold_s[(best_o + e - 1) % p];
                re += s.0;
                im += s.1;
            }
            let mut turn = im.atan2(re) / TAU;
            if self.num.half_shift {
                turn -= 0.5;
            }
            turn -= turn.round();
            cfo_hz = turn * self.num.spacing_hz();
            frame_timing = self.frame_phase(best_o);
            (frames, busy, last_symbol_db) = self.frames(frame_timing.unwrap_or(best_o));
        }
        let detected = self.last_hit.is_some_and(|t| time_s - t <= self.cfg.hold_s);
        let frame = self.frame_shape();
        self.fold_mag.iter_mut().for_each(|v| *v = 0.0);
        self.fold_s.iter_mut().for_each(|v| *v = (0.0, 0.0));
        self.fold_p.iter_mut().for_each(|v| *v = 0.0);
        self.blocks.clear();
        self.in_report = 0;
        SlotReport {
            time_s,
            hit,
            detected,
            energy: !hit && ratio > self.cfg.energy_ratio && mean_power > self.cfg.min_power,
            ratio,
            correlation,
            slot_timing: best_o,
            cfo_hz,
            mean_power,
            frame_timing,
            frames,
            busy,
            last_symbol_db,
            frame,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::fft::Fft;

    /// SC-FDMA-shaped frames of `num` (random carriers, half-carrier shift, the last symbol of every frame empty when
    /// `guard`), at `cfo` Hz, starting `lead` samples into a frame.
    fn sidelink(num: &Numerology, frames: usize, cfo: f64, guard: bool, lead: usize) -> Vec<Complex32> {
        let n = num.fft_len;
        let mut fft = Fft::inverse(n);
        let mut s = 5u64;
        let slots = (num.frame_s * num.rate_hz).round() as usize / num.period();
        let mut x = Vec::new();
        for _ in 0..frames {
            for k in 0..slots {
                for (l, &cp) in num.prefixes.iter().enumerate() {
                    let mut sym = vec![Complex32::default(); n];
                    for c in (1..300).chain(n - 300..n) {
                        s ^= s << 13;
                        s ^= s >> 7;
                        s ^= s << 17;
                        sym[c] = Complex32::new(if s & 1 == 0 { 1.0 } else { -1.0 }, if s & 2 == 0 { 1.0 } else { -1.0 });
                    }
                    fft.process(&mut sym);
                    let start = x.len();
                    x.extend_from_slice(&sym[n - cp..]);
                    x.extend_from_slice(&sym);
                    for (i, v) in x[start..].iter_mut().enumerate() {
                        *v *= Complex32::from_polar(0.03, (TAU * 0.5 * (i as f64 - cp as f64) / n as f64) as f32);
                        if guard && k == slots - 1 && l == num.prefixes.len() - 1 {
                            *v = Complex32::default();
                        }
                    }
                }
            }
        }
        x.rotate_left(lead);
        for (i, v) in x.iter_mut().enumerate() {
            *v *= Complex32::from_polar(1.0, (TAU * cfo * i as f64 / num.rate_hz) as f32);
        }
        x
    }

    #[test]
    fn finds_the_slot_the_frame_and_the_guard() {
        let num = Numerology::new(15.36e6, 1024, &[80, 72, 72, 72, 72, 72, 72], 1e-3)
            .unwrap()
            .with_half_shift();
        let lead = 5000;
        let x = sidelink(&num, 60, 1500.0, true, lead);
        let mut d = SlotDetector::new(num.clone()).unwrap();
        let mut last = None;
        d.process(&x, |r| last = Some(*r));
        let r = last.unwrap();
        assert!(r.hit, "{r:?}");
        // The frame starts at sample 15360 - lead of the stream.
        let start = 15_360 - lead;
        assert!((r.slot_timing as i64 - (start % 7680) as i64).abs() <= 2, "{r:?}");
        assert_eq!(r.frame_timing.map(|t| (t as i64 - start as i64).abs() <= 2), Some(true), "{r:?}");
        assert!((r.cfo_hz - 1500.0).abs() < 100.0, "{r:?}");
        assert!(r.frames >= 40 && r.busy == r.frames, "{r:?}");
        assert!(r.last_symbol_db.unwrap() < -20.0, "{r:?}");
        // Without the guard the last symbol is as strong as the others.
        let x = sidelink(&num, 60, 0.0, false, lead);
        let mut d = SlotDetector::new(num).unwrap();
        let mut last = None;
        d.process(&x, |r| last = Some(*r));
        let r = last.unwrap();
        assert!(r.hit && r.frame_timing.is_none(), "{r:?}");
    }
}
