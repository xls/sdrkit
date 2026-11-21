//! Carrier measurement: an averaged Hann power spectrum, the width and centre of a carrier from its area and from its edges, and
//! a raised cosine fit.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32;

/// An averaged power spectrum of `n` points (Hann window), segments taken every `hop` samples and summed in `f64`, which the
/// carrier survey needs.
pub struct Psd {
    n: usize,
    hop: usize,
    fft: Fft,
    win: Vec<f32>,
    acc: Vec<f64>,
    /// Segments averaged.
    pub count: usize,
    seg: Vec<Complex32>,
    fill: usize,
    skip: usize,
    work: Vec<Complex32>,
}

impl std::fmt::Debug for Psd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Psd")
            .field("n", &self.n)
            .field("count", &self.count)
            .finish()
    }
}

impl Psd {
    /// `n` points, a segment every `hop` samples (`hop >= n`).
    pub fn new(n: usize, hop: usize) -> Psd {
        let win: Vec<f32> = (0..n)
            .map(|i| (0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos()) as f32)
            .collect();
        Psd {
            n,
            hop: hop.max(n),
            fft: Fft::forward(n),
            win,
            acc: vec![0.0; n],
            count: 0,
            seg: vec![Complex32::new(0.0, 0.0); n],
            fill: 0,
            skip: 0,
            work: vec![Complex32::new(0.0, 0.0); n],
        }
    }
    /// Size.
    pub fn len(&self) -> usize {
        self.n
    }
    /// Never empty.
    pub fn is_empty(&self) -> bool {
        false
    }
    /// Starts the average again.
    pub fn clear(&mut self) {
        self.acc.iter_mut().for_each(|a| *a = 0.0);
        self.count = 0;
        self.fill = 0;
        self.skip = 0;
    }
    /// Adds samples.
    pub fn push(&mut self, x: &[Complex32]) {
        let mut i = 0;
        while i < x.len() {
            if self.skip > 0 {
                let k = self.skip.min(x.len() - i);
                self.skip -= k;
                i += k;
                continue;
            }
            let k = (self.n - self.fill).min(x.len() - i);
            self.seg[self.fill..self.fill + k].copy_from_slice(&x[i..i + k]);
            self.fill += k;
            i += k;
            if self.fill == self.n {
                self.fill = 0;
                self.skip = self.hop - self.n;
                for (w, (s, h)) in self.work.iter_mut().zip(self.seg.iter().zip(&self.win)) {
                    let v = *s * *h;
                    *w = if v.re.is_finite() && v.im.is_finite() {
                        v
                    } else {
                        Complex32::new(0.0, 0.0)
                    };
                }
                self.fft.process(&mut self.work);
                for (a, w) in self.acc.iter_mut().zip(&self.work) {
                    *a += w.norm_sqr() as f64;
                }
                self.count += 1;
            }
        }
    }
    /// The average, bin 0 at -fs/2 (shifted), in linear power per bin.
    pub fn spectrum(&self, out: &mut Vec<f64>) {
        out.clear();
        let c = self.count.max(1) as f64;
        let h = self.n / 2;
        out.extend(self.acc[h..].iter().chain(&self.acc[..h]).map(|a| a / c));
    }
}

/// The `q` quantile (0 to 1) of `s` (`sdrkit_algo_dsp::stats::quantile_f64`), 0 when `s` is empty.
pub fn quantile(s: &[f64], q: f64) -> f64 {
    sdrkit_algo_dsp::stats::quantile_f64(s, q, &mut Vec::new()).unwrap_or(0.0)
}

/// A carrier measured from a spectrum.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CarrierShape {
    /// Centre offset, Hz.
    pub centre_hz: f64,
    /// Width at its plateau (= symbol rate of an RRC carrier), Hz.
    pub width_hz: f64,
    /// Plateau and floor, linear per bin.
    pub plateau: f64,
    /// Noise floor.
    pub floor: f64,
    /// Lower and upper -3 dB edges, Hz.
    pub lo_hz: f64,
    /// Upper edge.
    pub hi_hz: f64,
    /// Roll-off when measured (0 when not).
    pub rolloff: f64,
    /// Misfit of the raised cosine fit (0.1 good, 1 not a carrier).
    pub misfit: f64,
    /// Plateau over floor, dB.
    pub snr_db: f64,
}

/// Width and centre from the area of a smoothed spectrum and its -10 dB edges. `rate` is the
/// sample rate; `alpha` the roll-off assumed for the edge rule. `None` when the spectrum spans less than 6 dB.
pub fn area_carrier(s: &[f64], rate: f64, alpha: f64) -> Option<CarrierShape> {
    let n = s.len();
    let bin = rate / n as f64;
    let max = s.iter().cloned().fold(f64::MIN, f64::max);
    let min = s.iter().cloned().fold(f64::MAX, f64::min);
    if max.is_nan() || max <= 0.0 || max < 4.0 * min.max(1e-30) {
        return None;
    }
    let floor = quantile(s, 0.08);
    let mut top: Vec<f64> = s.iter().copied().filter(|&v| v > max * 0.1).collect();
    top.sort_by(|a, b| a.total_cmp(b));
    let q = top.len() / 4;
    let mid = &top[q..top.len() - q];
    let plateau = mid.iter().sum::<f64>() / mid.len().max(1) as f64;
    let (mut area, mut wsum, mut csum) = (0.0f64, 0.0f64, 0.0f64);
    for (i, &v) in s.iter().enumerate() {
        if v > floor * 3.98 {
            let ex = v - floor;
            area += ex;
            wsum += ex;
            csum += ex * (i as f64 - (n / 2) as f64);
        }
    }
    let height = (plateau - floor).max(1e-30);
    let mut width = area / height * bin;
    let mut centre = csum / wsum.max(1e-30) * bin;
    // Edges at plateau - 10 dB, walking out from the centre.
    let thr = floor + (plateau - floor) * 0.1;
    let c = ((centre / bin) as isize + (n / 2) as isize).clamp(0, n as isize - 1) as usize;
    let walk = |dir: isize| -> Option<f64> {
        let mut i = c as isize;
        let mut below = 0;
        let mut first: Option<isize> = None;
        while i > 0 && i < n as isize - 1 {
            let v = s[i as usize];
            if v < thr {
                if below == 0 {
                    first = Some(i);
                }
                below += 1;
                if below >= 24 {
                    let f = first?;
                    let prev = (f - dir) as usize;
                    let (a, b) = (s[prev], s[f as usize]);
                    let frac = if (a - b).abs() > 0.0 {
                        (a - thr) / (a - b)
                    } else {
                        0.5
                    };
                    return Some((prev as f64 + dir as f64 * frac - (n / 2) as f64) * bin);
                }
            } else {
                below = 0;
            }
            i += dir;
        }
        None
    };
    let (lo, hi) = (walk(-1), walk(1));
    if let (Some(lo), Some(hi)) = (lo, hi) {
        let factor = 1.0 - alpha + 2.0 * alpha * (-0.8f64).acos() / std::f64::consts::PI;
        let w = (hi - lo) / factor;
        if w > 0.3e6 && w < 0.92 * width {
            width = w;
            centre = 0.5 * (hi + lo);
        }
    }
    Some(CarrierShape {
        centre_hz: centre,
        width_hz: width,
        plateau,
        floor,
        lo_hz: centre - width / 2.0,
        hi_hz: centre + width / 2.0,
        rolloff: 0.0,
        misfit: 0.0,
        snr_db: 10.0 * (height / floor.max(1e-30)).log10(),
    })
}

/// A raised cosine spectrum of unit plateau: 1 inside (1 - a) / 2, falling to 0 at (1 + a) / 2 (in units of the rate).
pub fn raised_cosine(f: f64, a: f64) -> f64 {
    let f = f.abs();
    let lo = (1.0 - a) / 2.0;
    let hi = (1.0 + a) / 2.0;
    if f <= lo {
        1.0
    } else if f >= hi {
        0.0
    } else {
        0.5 * (1.0 + (std::f64::consts::PI / a * (f - lo)).cos())
    }
}

/// The depth of the raised cosine fit under the plateau (20 dB, the level at which the roll-off is measured). Under it a bin
/// counts with the scatter of this level: without noise the skirts fall far under the model's floor, and their ratio to the
/// model says nothing about the shape.
pub const FIT_DEPTH: f64 = 0.01;

/// Measures the strongest carrier of a spectrum (`s` shifted, linear) for the satellite survey: its
/// half-power edges, plateau, floor, a raised cosine fit (roll-off when the plateau is 15 dB over the floor) and the misfit. `segs`
/// is the number of segments averaged (for the scatter). `floor` is the noise floor.
pub fn measure_carrier(
    s: &[f64],
    rate: f64,
    floor: f64,
    segs: usize,
    around: Option<usize>,
) -> Option<CarrierShape> {
    let n = s.len();
    let bin = rate / n as f64;
    // Narrow lines (a residual carrier, sync patterns) taken out by a 9-bin median, then the strongest region of a wide average.
    let med: Vec<f64> = (0..n)
        .map(|i| {
            let mut v = [0f64; 9];
            for (k, x) in v.iter_mut().enumerate() {
                *x = s[(i + n + k - 4) % n];
            }
            v.sort_by(|a, b| a.total_cmp(b));
            v[4]
        })
        .collect();
    let s = &med[..];
    let mut w = Vec::new();
    sdrkit_algo_dsp::stats::centred_mean(s, (n / 256).max(2), &mut w);
    let peak = match around {
        Some(c) => c.min(n - 1),
        None => (0..n).max_by(|&a, &b| w[a].total_cmp(&w[b]))?,
    };
    if w[peak] <= floor * 1.4 {
        return None;
    }
    // Plateau: the mean of bins near the peak within 3 dB of it.
    let pk = w[peak];
    let half = floor + (pk - floor) / 2.0;
    let edge = |dir: isize| -> usize {
        let mut i = peak as isize;
        loop {
            let j = i + dir;
            if j <= 0 || j >= n as isize - 1 {
                return j.clamp(0, n as isize - 1) as usize;
            }
            // First bin below half that stays below for 8 more.
            if w[j as usize] < half {
                let mut stays = true;
                for k in 1..=8 {
                    let q = j + dir * k;
                    if q > 0 && q < n as isize && w[q as usize] >= half {
                        stays = false;
                        break;
                    }
                }
                if stays {
                    return j as usize;
                }
            }
            i = j;
        }
    };
    let (l, h) = (edge(-1), edge(1));
    if h <= l + 2 {
        return None;
    }
    let inner = (h - l) / 4;
    let plat: f64 = w[l + inner..=h - inner].iter().sum::<f64>() / (h - l - 2 * inner + 1) as f64;
    // Interpolated half-power crossings.
    let halfp = floor + (plat - floor) / 2.0;
    let cross = |a: usize, b: usize| -> f64 {
        let (va, vb) = (w[a], w[b]);
        let t = if (va - vb).abs() > 0.0 {
            (va - halfp) / (va - vb)
        } else {
            0.5
        };
        a as f64 + (b as f64 - a as f64) * t.clamp(0.0, 1.0)
    };
    let lo = cross(l + 1, l);
    let hi = cross(h - 1, h);
    let rate_bins = hi - lo;
    let centre_bins = 0.5 * (hi + lo);
    let height = (plat - floor).max(1e-30);
    let snr_db = 10.0 * (height / floor.max(1e-30)).log10();
    // Roll-off from the width at -20 dB.
    let mut rolloff = 0.0;
    if snr_db > 15.0 {
        let t20 = floor + height * 0.01;
        let mut a = l as isize;
        while a > 0 && s[a as usize] > t20 {
            a -= 1;
        }
        let mut b = h;
        while b < n - 1 && s[b] > t20 {
            b += 1;
        }
        let w20 = (b as f64 - a as f64) / rate_bins;
        rolloff = ((w20 - 1.0) / (2.0 * 0.436)).clamp(0.0, 0.5);
    }
    // Misfit: weighted rms residual against the raised cosine, over the scatter of the average.
    let a = if rolloff > 0.0 { rolloff } else { 0.35 };
    let (mut num, mut den) = (0.0f64, 0.0f64);
    let span = (rate_bins * 0.75) as isize;
    for d in -span..=span {
        let i = centre_bins as isize + d;
        if i < 0 || i >= n as isize {
            continue;
        }
        let model = floor + height * raised_cosine(d as f64 / rate_bins, a);
        let scatter = model.max(FIT_DEPTH * plat) / (segs.max(1) as f64).sqrt();
        let r = (s[i as usize] - model) / scatter.max(1e-30);
        num += r * r;
        den += 1.0;
    }
    let misfit = (num / den.max(1.0)).sqrt() / 3.0;
    Some(CarrierShape {
        centre_hz: (centre_bins - (n / 2) as f64) * bin,
        width_hz: rate_bins * bin,
        plateau: plat,
        floor,
        lo_hz: (lo - (n / 2) as f64) * bin,
        hi_hz: (hi - (n / 2) as f64) * bin,
        rolloff,
        misfit,
        snr_db,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A raised cosine with no noise under it fits, and a triangle of the same width does not: its skirts stand far over
    /// the model, also with the scatter of [`FIT_DEPTH`].
    #[test]
    fn the_fit_takes_a_carrier_without_noise_and_refuses_another_shape() {
        let (n, rate, width) = (4096usize, 4e6, 1000.0);
        let spectrum = |shape: &dyn Fn(f64) -> f64| -> Vec<f64> {
            (0..n)
                .map(|i| shape((i as f64 - (n / 2) as f64) / width) + 1e-9)
                .collect()
        };
        let carrier = spectrum(&|x| raised_cosine(x, 0.25));
        let c = measure_carrier(&carrier, rate, 1e-5, 48, None).unwrap();
        assert!(c.misfit < 0.5, "{c:?}");
        assert!(
            (c.width_hz - width * rate / n as f64).abs() < 0.02 * c.width_hz,
            "{c:?}"
        );
        assert!((c.rolloff - 0.25).abs() < 0.03, "{c:?}");
        let triangle = spectrum(&|x| (1.0 - x.abs()).max(0.0));
        let t = measure_carrier(&triangle, rate, 1e-5, 48, None).unwrap();
        assert!(t.misfit > 2.5, "{t:?}");
    }

    #[test]
    fn raised_cosine_edges() {
        assert_eq!(raised_cosine(0.0, 0.35), 1.0);
        assert!((raised_cosine(0.5, 0.35) - 0.5).abs() < 1e-9);
        assert_eq!(raised_cosine(0.7, 0.35), 0.0);
    }
}
