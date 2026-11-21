//! Measures of an FSK signal on an FM channel, for identifying a signal without decoding it: the symbol rate from the spectral
//! line of the squared discriminator difference, the number of levels from a histogram of the discriminator, the occupied
//! width of the channel, and the period of a burst pattern from the autocorrelation of the envelope. [`FmMeasure`] keeps the
//! buffers and the transforms, so a measure allocates nothing once the sizes it meets have been seen.

use crate::Complex32;
use sdrkit_algo_dsp::fft::Fft;

/// The buffers and transforms of the measures.
#[derive(Default)]
pub struct FmMeasure {
    ffts: Vec<Fft>,
    buf: Vec<Complex32>,
    vals: Vec<f64>,
    sorted: Vec<f64>,
}

impl FmMeasure {
    /// No buffers yet.
    pub fn new() -> FmMeasure {
        FmMeasure::default()
    }

    /// The forward transform of `n` points, made once.
    fn fft(&mut self, n: usize) -> usize {
        match self.ffts.iter().position(|f| f.len() == n) {
            Some(i) => i,
            None => {
                self.ffts.push(Fft::forward(n));
                self.ffts.len() - 1
            }
        }
    }

    /// The symbol rate of discriminator values `disc` at `rate` Hz, between `lo_hz` and `hi_hz`: the strongest line of the
    /// spectrum of `(disc[n] - disc[n-1])²` over the samples where `on` holds at both, refined by a parabola. `None` with fewer
    /// than [`MIN_RATE_VALUES`] such values. The caller decides how strong a line must be.
    pub fn symbol_rate(
        &mut self,
        disc: &[f32],
        on: &[bool],
        rate: f64,
        lo_hz: f64,
        hi_hz: f64,
    ) -> Option<SymbolRate> {
        self.vals.clear();
        for n in 1..disc.len().min(on.len()) {
            if on[n] && on[n - 1] {
                let d = f64::from(disc[n] - disc[n - 1]);
                self.vals.push(d * d);
            }
        }
        if self.vals.len() < MIN_RATE_VALUES {
            return None;
        }
        let mut len = 1;
        while len * 2 <= self.vals.len().min(MAX_RATE_VALUES) {
            len *= 2;
        }
        let mean = self.vals[..len].iter().sum::<f64>() / len as f64;
        self.buf.clear();
        self.buf.extend(
            self.vals[..len]
                .iter()
                .map(|&x| Complex32::new((x - mean) as f32, 0.0)),
        );
        let i = self.fft(len);
        self.ffts[i].process(&mut self.buf);
        let bin_hz = rate / len as f64;
        let lo = ((lo_hz / bin_hz).ceil() as usize).max(1);
        let hi = ((hi_hz / bin_hz).floor() as usize).min(len / 2 - 1);
        if lo + 2 >= hi {
            return None;
        }
        let p = |k: usize| f64::from(self.buf[k].norm_sqr());
        let k = (lo..=hi).max_by(|&a, &b| p(a).total_cmp(&p(b)))?;
        let peak = p(k);
        self.sorted.clear();
        self.sorted.extend((lo..=hi).map(p));
        self.sorted.sort_by(f64::total_cmp);
        let median = self.sorted[self.sorted.len() / 2].max(1e-30);
        let (a, b, c) = (p(k - 1), p(k), p(k + 1));
        let den = a - 2.0 * b + c;
        let shift = if den.abs() > 0.0 {
            (0.5 * (a - c) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        Some(SymbolRate {
            hz: (k as f64 + shift) * bin_hz,
            strength_db: 10.0 * (peak / median).log10(),
        })
    }
}

/// A symbol rate found by [`FmMeasure::symbol_rate`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SymbolRate {
    /// The rate, Hz.
    pub hz: f64,
    /// The line over the median of the searched range, dB.
    pub strength_db: f64,
}

/// The least number of values [`FmMeasure::symbol_rate`] works with.
pub const MIN_RATE_VALUES: usize = 4096;
/// The most values [`FmMeasure::symbol_rate`] transforms.
pub const MAX_RATE_VALUES: usize = 1 << 17;

/// The levels found by [`FmMeasure::levels`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levels {
    /// 2 or 4 levels; 0 when the histogram is unclear.
    pub count: u8,
    /// Half the distance between the outermost peaks, in the units of the input.
    pub deviation: f64,
}

impl FmMeasure {
    /// The levels of discriminator values `disc` where `on` holds: a histogram of 121 bins over ±1.05 × the largest deviation
    /// from the mean, smoothed with the weights 1, 2, 3, 2, 1; its peaks are the local maxima above 0.2 × the highest, not
    /// within 4 bins of an edge. 2 or 3 peaks are 2 levels, 4 or more are 4 levels.
    pub fn levels(&mut self, disc: &[f32], on: &[bool]) -> Levels {
        const BINS: usize = 121;
        let none = Levels {
            count: 0,
            deviation: 0.0,
        };
        self.vals.clear();
        self.vals.extend(
            disc.iter()
                .zip(on)
                .filter(|(_, &o)| o)
                .map(|(&d, _)| f64::from(d)),
        );
        let vals = &self.vals;
        if vals.len() < 16 {
            return none;
        }
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        let span = 1.05 * vals.iter().map(|v| (v - mean).abs()).fold(0.0, f64::max);
        if span <= 0.0 {
            return none;
        }
        let mut h = [0f64; BINS];
        for v in vals {
            let k = (((v - mean) / span + 1.0) * 0.5 * (BINS - 1) as f64).round() as usize;
            h[k.min(BINS - 1)] += 1.0;
        }
        let w = [1.0, 2.0, 3.0, 2.0, 1.0];
        let mut s = [0f64; BINS];
        for (i, o) in s.iter_mut().enumerate() {
            for (j, wj) in w.iter().enumerate() {
                let k = i as isize + j as isize - 2;
                if (0..BINS as isize).contains(&k) {
                    *o += wj * h[k as usize];
                }
            }
        }
        let top = s.iter().copied().fold(0.0, f64::max);
        let is_peak = |i: usize| s[i] > 0.2 * top && s[i] >= s[i - 1] && s[i] > s[i + 1];
        let mut count = 0;
        let (mut first, mut last) = (0, 0);
        for i in (4..BINS - 4).filter(|&i| is_peak(i)) {
            if count == 0 {
                first = i;
            }
            last = i;
            count += 1;
        }
        let count = match count {
            2 | 3 => 2,
            n if n >= 4 => 4,
            _ => return none,
        };
        let bin = 2.0 * span / (BINS - 1) as f64;
        Levels {
            count,
            deviation: 0.5 * (last - first) as f64 * bin,
        }
    }

    /// The occupied width of `iq` at `rate`: the power spectrum of the first 2^15 samples with a Hann window, grown from 0 Hz
    /// outwards until 90 % of the power is inside; twice that. `None` with fewer samples or no power.
    pub fn occupied_width(&mut self, iq: &[Complex32], rate: f64) -> Option<f64> {
        const N: usize = 1 << 15;
        if iq.len() < N {
            return None;
        }
        self.buf.clear();
        self.buf.extend(iq[..N].iter().enumerate().map(|(i, &z)| {
            let w = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / N as f64).cos();
            z * w as f32
        }));
        let f = self.fft(N);
        self.ffts[f].process(&mut self.buf);
        let p = |k: usize| f64::from(self.buf[k].norm_sqr());
        let total: f64 = (0..N).map(p).sum();
        if total <= 0.0 {
            return None;
        }
        let mut inside = p(0);
        for k in 1..N / 2 {
            inside += p(k) + p(N - k);
            if inside >= 0.9 * total {
                return Some(2.0 * k as f64 * rate / N as f64);
            }
        }
        Some(rate)
    }

    /// The period of a burst pattern of envelope `env` (|sample|²) at `rate`: the envelope averaged over each millisecond,
    /// and the lag from 5 to 400 ms of its highest autocorrelation over the energy (so the shortest of equal periods wins),
    /// when that is above 0.5. Seconds.
    pub fn burst_period(&mut self, env: &[f32], rate: f64) -> Option<f64> {
        let per_ms = (rate / 1000.0).round().max(1.0) as usize;
        self.vals.clear();
        self.vals.extend(
            env.chunks(per_ms)
                .filter(|c| c.len() == per_ms)
                .map(|c| c.iter().map(|&v| f64::from(v)).sum::<f64>() / per_ms as f64),
        );
        let x = &mut self.vals;
        if x.len() < 20 {
            return None;
        }
        let mean = x.iter().sum::<f64>() / x.len() as f64;
        x.iter_mut().for_each(|v| *v -= mean);
        let e0: f64 = x.iter().map(|v| v * v).sum();
        if e0 <= 0.0 {
            return None;
        }
        let mut best: Option<(usize, f64)> = None;
        for lag in 5..=400.min(x.len() / 2) {
            let r: f64 = x[..x.len() - lag]
                .iter()
                .zip(&x[lag..])
                .map(|(a, b)| a * b)
                .sum::<f64>()
                / e0;
            if best.is_none_or(|(_, b)| r > b) {
                best = Some((lag, r));
            }
        }
        best.filter(|(_, r)| *r > 0.5)
            .map(|(lag, _)| lag as f64 / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// Discriminator values of `m`-level FSK at `baud` with levels ±1/3 and ±1 of `dev` (or ±1 for 2 levels), smoothed, with
    /// noise.
    fn fsk(m: usize, baud: f64, dev: f64, rate: f64, n: usize) -> Vec<f32> {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
        let sps = rate / baud;
        let mut out = Vec::with_capacity(n);
        let mut level = 0.0;
        let mut y = 0.0;
        let a = 1.0 - (-TAU * 0.8 * baud / rate).exp();
        for i in 0..n {
            if (i as f64 % sps) < 1.0 {
                let k = (rng.next_u64() % m as u64) as f64;
                level = if m == 2 {
                    2.0 * k - 1.0
                } else {
                    (2.0 * k - 3.0) / 3.0
                };
            }
            y += a * (level * dev - y);
            // Noise of 0.05 of the deviation, as a received channel has.
            let g = (-2.0 * rng.uniform().max(1e-12).ln()).sqrt() * (TAU * rng.uniform()).cos();
            out.push((y + 0.05 * dev * g) as f32);
        }
        out
    }

    #[test]
    fn rate_and_levels_of_four_level_fsk() {
        let rate = 96_000.0;
        let d = fsk(4, 9600.0, 4800.0, rate, 1 << 17);
        let on = vec![true; d.len()];
        let mut m = FmMeasure::new();
        let r = m.symbol_rate(&d, &on, rate, 2000.0, 25_000.0).unwrap();
        assert!((r.hz - 9600.0).abs() < 100.0, "{r:?}");
        assert!(r.strength_db > 8.0);
        let l = m.levels(&d[1000..], &on[1000..]);
        assert_eq!(l.count, 4, "{l:?}");
        let d2 = fsk(2, 3600.0, 1200.0, rate, 1 << 16);
        assert_eq!(m.levels(&d2[1000..], &on[1000..d2.len()]).count, 2);
    }

    #[test]
    fn width_and_bursts() {
        let rate = 96_000.0;
        let iq: Vec<Complex32> = (0..1 << 16)
            .map(|i| Complex32::from_polar(1.0, (TAU * 3000.0 * i as f64 / rate) as f32))
            .collect();
        let mut m = FmMeasure::new();
        let w = m.occupied_width(&iq, rate).unwrap();
        assert!((w - 6000.0).abs() < 200.0, "{w}");
        // 4 slots of 7.5 ms with 2 on: a period of 30 ms.
        let env: Vec<f32> = (0..rate as usize)
            .map(|i| {
                if (i as f64 / rate * 1000.0) % 30.0 < 15.0 {
                    1.0
                } else {
                    0.0
                }
            })
            .collect();
        let p = m.burst_period(&env, rate).unwrap();
        assert!((p - 0.030).abs() < 0.002, "{p}");
        assert_eq!(m.burst_period(&vec![1.0; rate as usize], rate), None);
    }
}
