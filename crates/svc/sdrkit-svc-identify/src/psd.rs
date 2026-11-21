//! Spectra and small numeric helpers that the measurements share: a Welch power spectral density, line spectra, quantiles and FIR
//! filtering.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_core::Complex32;

/// A power spectral density, averaged over frames (Welch, Hann window, half overlap), with bin 0 at −rate/2 (shifted).
/// It keeps the peak of each bin over the frames and selects frames by a burst mask, which the shared `Welch` does not.
#[derive(Clone, Debug)]
pub struct Psd {
    /// Power per hertz in each bin, lowest frequency first.
    pub density: Vec<f32>,
    /// The largest density any one frame had in each bin (a bursty signal shows here at its full width).
    pub peak: Vec<f32>,
    /// The sample rate.
    pub rate: f64,
    /// The FFT size.
    pub nfft: usize,
    /// Frames averaged.
    pub frames: usize,
}

/// The largest power of two not above `n` (1 for 0).
pub fn pow2_floor(n: usize) -> usize {
    if n <= 1 {
        1
    } else {
        1usize << (usize::BITS - 1 - n.leading_zeros())
    }
}

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let x = std::f64::consts::PI * (i as f64 + 0.5) / n as f64;
            (x.sin() * x.sin()) as f32
        })
        .collect()
}

impl Psd {
    /// The Welch estimate of `x` with frames of `nfft` points (rounded down to a power of two and to the length of `x`), at most
    /// `max_frames` frames spread evenly over `x`.
    pub fn welch(x: &[Complex32], rate: f64, nfft: usize, max_frames: usize) -> Psd {
        Self::welch_where(x, rate, nfft, max_frames, None)
    }

    /// [`Psd::welch`] over the frames at least half inside `keep` (the bursts of a bursty signal), or over all frames when none is.
    pub fn welch_where(
        x: &[Complex32],
        rate: f64,
        nfft: usize,
        max_frames: usize,
        keep: Option<&[bool]>,
    ) -> Psd {
        let nfft = pow2_floor(nfft.min(x.len()).max(16));
        let w = hann(nfft);
        let wsum2: f64 = w.iter().map(|v| (*v as f64) * (*v as f64)).sum();
        let mut fft = Fft::forward(nfft);
        let mut acc = vec![0f64; nfft];
        let mut peak = vec![0f64; nfft];
        let mut buf = vec![Complex32::default(); nfft];
        let starts: Vec<usize> = if x.len() < nfft {
            Vec::new()
        } else {
            let all: Vec<usize> = (0..=(x.len() - nfft) / (nfft / 2))
                .map(|f| f * (nfft / 2))
                .collect();
            let kept: Vec<usize> = match keep {
                Some(k) => all
                    .iter()
                    .copied()
                    .filter(|s| k[*s..*s + nfft].iter().filter(|o| **o).count() * 2 >= nfft)
                    .collect(),
                None => Vec::new(),
            };
            let pool = if kept.is_empty() { all } else { kept };
            let take = pool.len().min(max_frames.max(1));
            (0..take)
                .map(|f| {
                    pool[if take <= 1 {
                        0
                    } else {
                        f * (pool.len() - 1) / (take - 1)
                    }]
                })
                .collect()
        };
        let mut used = 0;
        for start in starts {
            for i in 0..nfft {
                buf[i] = x[start + i] * w[i];
            }
            fft.process(&mut buf);
            for ((a, p), b) in acc.iter_mut().zip(peak.iter_mut()).zip(&buf) {
                let v = b.norm_sqr() as f64;
                *a += v;
                *p = p.max(v);
            }
            used += 1;
        }
        if used == 0 && !x.is_empty() {
            // Fewer samples than a frame: zero-padded.
            for (i, b) in buf.iter_mut().enumerate() {
                *b = x.get(i).copied().unwrap_or_default() * w[i];
            }
            fft.process(&mut buf);
            for ((a, p), b) in acc.iter_mut().zip(peak.iter_mut()).zip(&buf) {
                *a += b.norm_sqr() as f64;
                *p = *a;
            }
            used = 1;
        }
        let scale = 1.0 / (wsum2 * rate);
        let mut density: Vec<f32> = acc
            .iter()
            .map(|a| (a * scale / used.max(1) as f64) as f32)
            .collect();
        let mut peak: Vec<f32> = peak.iter().map(|a| (a * scale) as f32).collect();
        density.rotate_left(nfft / 2);
        peak.rotate_left(nfft / 2);
        Psd {
            density,
            peak,
            rate,
            nfft,
            frames: used,
        }
    }
    /// The width of one bin (Hz).
    pub fn bin_hz(&self) -> f64 {
        self.rate / self.nfft as f64
    }
    /// The frequency (Hz, from the centre) of a (fractional) bin.
    pub fn freq(&self, bin: f64) -> f64 {
        (bin - (self.nfft / 2) as f64) * self.bin_hz()
    }
    /// The (fractional) bin of a frequency.
    pub fn bin_of(&self, hz: f64) -> f64 {
        hz / self.bin_hz() + (self.nfft / 2) as f64
    }
    /// The noise floor: a low quantile of the density, which signals do not move even when one fills most of the capture.
    pub fn floor(&self) -> f64 {
        // Ignore 1 % at each edge (the radio's roll-off).
        let e = self.nfft / 100;
        quantile(&self.density[e..self.nfft - e], 0.05)
    }
    /// The floor of the peak hold.
    pub fn peak_floor(&self) -> f64 {
        let e = self.nfft / 100;
        quantile(&self.peak[e..self.nfft - e], 0.2)
    }
}

/// The `q` quantile (0 to 1) of `v` (0 for an empty slice).
pub fn quantile(v: &[f32], q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s: Vec<f32> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if s.is_empty() {
        return 0.0;
    }
    let k = ((s.len() - 1) as f64 * q.clamp(0.0, 1.0)).round() as usize;
    let (_, m, _) = s.select_nth_unstable_by(k, |a, b| a.total_cmp(b));
    *m as f64
}

/// The median of `v`.
pub fn median(v: &[f32]) -> f64 {
    quantile(v, 0.5)
}

/// The median of `v` (f64).
pub fn median64(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        0.5 * (s[n / 2 - 1] + s[n / 2])
    }
}

/// The power spectrum of one Hann-windowed FFT of `x` (zero-padded or cut to `n`, a power of two), not shifted (bin k is k·rate/n,
/// the upper half negative).
pub fn line_spectrum(x: &[Complex32], n: usize) -> Vec<f32> {
    let w = hann(n);
    let mut buf: Vec<Complex32> = (0..n)
        .map(|i| x.get(i).copied().unwrap_or_default() * w[i])
        .collect();
    Fft::forward(n).process(&mut buf);
    buf.iter().map(|c| c.norm_sqr()).collect()
}

/// The power spectrum of a real signal (mean removed), one Hann-windowed FFT of `n` points; bins 0 to n/2.
pub fn real_line_spectrum(x: &[f32], n: usize) -> Vec<f32> {
    let take = x.len().min(n);
    let mean = if take == 0 {
        0.0
    } else {
        x[..take].iter().map(|v| *v as f64).sum::<f64>() / take as f64
    };
    let c: Vec<Complex32> = x[..take]
        .iter()
        .map(|v| Complex32::new((*v as f64 - mean) as f32, 0.0))
        .collect();
    let mut s = line_spectrum(&c, n);
    s.truncate(n / 2 + 1);
    s
}

/// A spectral line: where and how far above its neighbourhood.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    /// The fractional bin.
    pub bin: f64,
    /// Its power over the median of the bins around it, in dB.
    pub strength_db: f64,
}

/// The strongest line of `spec` between bins `lo` and `hi` (inclusive, clamped), with its strength over the median of `±half`
/// bins around it.
pub fn strongest_line(spec: &[f32], lo: usize, hi: usize, half: usize) -> Option<Line> {
    if spec.len() < 3 {
        return None;
    }
    let lo = lo.max(1);
    let hi = hi.min(spec.len() - 2);
    if lo > hi {
        return None;
    }
    let (mut k, mut best) = (lo, f32::MIN);
    for (i, v) in spec.iter().enumerate().take(hi + 1).skip(lo) {
        if *v > best {
            best = *v;
            k = i;
        }
    }
    let a = k.saturating_sub(half);
    let b = (k + half + 1).min(spec.len());
    let m = median(&spec[a..b]).max(1e-30);
    let frac =
        sdrkit_algo_dsp::peak::parabolic(spec[k - 1] as f64, spec[k] as f64, spec[k + 1] as f64)
            .offset;
    Some(Line {
        bin: k as f64 + frac,
        strength_db: power_to_db(best as f64 / m),
    })
}

/// Up to `count` lines of `spec` between bins `lo` and `hi`, strongest first, each at least `apart` (a fraction of its bin) from
/// a stronger one, with their strengths over the median of `±half` bins around them.
pub fn top_lines(
    spec: &[f32],
    lo: usize,
    hi: usize,
    half: usize,
    count: usize,
    apart: f64,
) -> Vec<Line> {
    if spec.len() < 3 {
        return Vec::new();
    }
    let lo = lo.max(1);
    let hi = hi.min(spec.len() - 2);
    if lo > hi {
        return Vec::new();
    }
    let mut peaks: Vec<usize> = (lo..=hi)
        .filter(|&i| spec[i] >= spec[i - 1] && spec[i] >= spec[i + 1])
        .collect();
    peaks.sort_by(|a, b| spec[*b].total_cmp(&spec[*a]));
    let mut out: Vec<Line> = Vec::new();
    for k in peaks {
        if out.len() >= count {
            break;
        }
        if out
            .iter()
            .any(|l| (l.bin - k as f64).abs() < apart * l.bin.max(1.0))
        {
            continue;
        }
        let a = k.saturating_sub(half);
        let b = (k + half + 1).min(spec.len());
        let m = median(&spec[a..b]).max(1e-30);
        let frac = sdrkit_algo_dsp::peak::parabolic(
            spec[k - 1] as f64,
            spec[k] as f64,
            spec[k + 1] as f64,
        )
        .offset;
        out.push(Line {
            bin: k as f64 + frac,
            strength_db: power_to_db(spec[k] as f64 / m),
        });
    }
    out
}

/// Filters `x` with real taps (direct convolution, the output aligned with the input: the group delay is taken out).
pub fn fir_filter(x: &[Complex32], taps: &[f32]) -> Vec<Complex32> {
    let n = taps.len();
    if n == 0 {
        return x.to_vec();
    }
    let d = (n - 1) / 2;
    let mut out = Vec::with_capacity(x.len());
    for i in 0..x.len() {
        let mut acc = Complex32::default();
        // y[i] = sum_k h[k] x[i + d - k]
        let base = i + d;
        let k0 = base.saturating_sub(x.len() - 1);
        let k1 = (base + 1).min(n);
        for k in k0..k1 {
            acc += x[base - k] * taps[k];
        }
        out.push(acc);
    }
    out
}

/// A moving average of `len` samples, centred. An even `len` averages `len / 2` samples before and `len / 2 - 1` after
/// each sample, so it is not the symmetric `sdrkit_algo_dsp::stats::centred_mean`.
pub fn moving_average(x: &[f32], len: usize) -> Vec<f32> {
    let len = len.max(1);
    if len == 1 || x.is_empty() {
        return x.to_vec();
    }
    let mut out = vec![0f32; x.len()];
    let mut acc = 0f64;
    let h = len / 2;
    let mut prefix = Vec::with_capacity(x.len() + 1);
    prefix.push(0f64);
    for v in x {
        acc += *v as f64;
        prefix.push(acc);
    }
    for (i, o) in out.iter_mut().enumerate() {
        let a = i.saturating_sub(h);
        let b = (i + len - h).min(x.len());
        *o = ((prefix[b] - prefix[a]) / (b - a) as f64) as f32;
    }
    out
}

/// Root-raised-cosine taps at `sps` samples a symbol, roll-off `beta`, `span` symbols long, unit DC gain.
pub fn rrc(sps: f64, beta: f64, span: usize) -> Vec<f32> {
    let n = ((span as f64 * sps).round() as usize) | 1;
    let mid = (n / 2) as f64;
    let mut h: Vec<f64> = (0..n)
        .map(|i| {
            let t = (i as f64 - mid) / sps;
            if t.abs() < 1e-9 {
                1.0 - beta + 4.0 * beta / std::f64::consts::PI
            } else if beta > 0.0 && (t.abs() - 1.0 / (4.0 * beta)).abs() < 1e-9 {
                let pi = std::f64::consts::PI;
                beta / 2f64.sqrt()
                    * ((1.0 + 2.0 / pi) * (pi / (4.0 * beta)).sin()
                        + (1.0 - 2.0 / pi) * (pi / (4.0 * beta)).cos())
            } else {
                let pi = std::f64::consts::PI;
                ((pi * t * (1.0 - beta)).sin() + 4.0 * beta * t * (pi * t * (1.0 + beta)).cos())
                    / (pi * t * (1.0 - (4.0 * beta * t).powi(2)))
            }
        })
        .collect();
    let s: f64 = h.iter().sum();
    for v in &mut h {
        *v /= s;
    }
    h.into_iter().map(|v| v as f32).collect()
}

/// Linear interpolation of `x` at a fractional index (clamped).
pub fn interp(x: &[f32], t: f64) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    let t = t.clamp(0.0, (x.len() - 1) as f64);
    let i = t.floor() as usize;
    let f = (t - i as f64) as f32;
    if i + 1 < x.len() {
        x[i] * (1.0 - f) + x[i + 1] * f
    } else {
        x[i]
    }
}

/// Linear interpolation of complex `x` at a fractional index (clamped).
pub fn interp_c(x: &[Complex32], t: f64) -> Complex32 {
    if x.is_empty() {
        return Complex32::default();
    }
    let t = t.clamp(0.0, (x.len() - 1) as f64);
    let i = t.floor() as usize;
    let f = (t - i as f64) as f32;
    if i + 1 < x.len() {
        x[i] * (1.0 - f) + x[i + 1] * f
    } else {
        x[i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, f: f64, rate: f64, a: f32) -> Vec<Complex32> {
        (0..n)
            .map(|i| {
                let p = std::f64::consts::TAU * f * i as f64 / rate;
                Complex32::new(p.cos() as f32 * a, p.sin() as f32 * a)
            })
            .collect()
    }

    #[test]
    fn welch_finds_a_tone_and_keeps_power() {
        let x = tone(65536, 10_000.0, 100_000.0, 1.0);
        let p = Psd::welch(&x, 100_000.0, 1024, 1000);
        let (k, _) = p
            .density
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap();
        assert!((p.freq(k as f64) - 10_000.0).abs() < 2.0 * p.bin_hz());
        let total: f64 = p.density.iter().map(|d| *d as f64 * p.bin_hz()).sum();
        assert!((total - 1.0).abs() < 0.05, "{total}");
    }

    #[test]
    fn helpers() {
        assert_eq!(pow2_floor(1000), 512);
        assert_eq!(pow2_floor(1024), 1024);
        assert!((median(&[3.0, 1.0, 2.0]) - 2.0).abs() < 1e-9);
        let h = rrc(4.0, 0.35, 8);
        assert!((h.iter().map(|v| *v as f64).sum::<f64>() - 1.0).abs() < 1e-5);
        let y = fir_filter(&tone(100, 0.0, 1.0, 1.0), &[0.25, 0.5, 0.25]);
        assert!((y[50].re - 1.0).abs() < 1e-6);
        let m = moving_average(&[1.0, 1.0, 4.0, 1.0, 1.0], 3);
        assert!((m[2] - 2.0).abs() < 1e-6);
    }
}
