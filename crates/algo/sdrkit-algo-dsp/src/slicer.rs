//! An envelope slicer with a noise gate: impulses over the noise of the envelope `|x|` of a complex stream.
//!
//! The slicer first learns the mean and the variance of the envelope over a fixed number of samples (each sample with weight
//! `1 / n`). The threshold is then `mean + k sigma`. A sample over the threshold starts or continues an impulse, whose peak and
//! peak index are kept; the first sample under it ends the impulse. Only samples under the threshold update the noise, with
//! the weight `max(1 / tau, 1 / n)`, so impulses do not raise the threshold and the noise follows slow changes.
//!
//! For complex Gaussian noise the envelope is Rayleigh: mean `1.2533 s` and standard deviation `0.6551 s` for `s` per
//! quadrature, so `k = 6.5` puts the threshold at `5.51 s`, 11.8 dB over the mean power, which noise crosses with the
//! probability `exp(-15.2) = 2.5e-7` per sample.

use num_complex::Complex32;

/// The rules of an [`EnvelopeSlicer`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlicerConfig {
    /// Samples that learn the noise before the first decision.
    pub learn: u64,
    /// The threshold over the mean, in standard deviations of the envelope.
    pub sigmas: f64,
    /// The time constant of the noise once learned, samples.
    pub tau: f64,
}

/// One impulse of an [`EnvelopeSlicer`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slice {
    /// The absolute index of its largest sample.
    pub peak_index: u64,
    /// Its largest envelope value.
    pub peak: f64,
    /// The mean noise envelope when it ended.
    pub noise_mean: f64,
}

impl Slice {
    /// The peak over the mean noise envelope, dB (`20 log10`).
    pub fn level_db(&self) -> f64 {
        20.0 * (self.peak / self.noise_mean.max(1e-30)).log10()
    }
}

/// The slicer (module docs).
#[derive(Clone, Debug)]
pub struct EnvelopeSlicer {
    cfg: SlicerConfig,
    n: u64,
    mean: f64,
    var: f64,
    active: Option<(u64, f64)>,
    pos: u64,
}

impl EnvelopeSlicer {
    /// A slicer with the rules `cfg`, before learning.
    pub fn new(cfg: SlicerConfig) -> Self {
        EnvelopeSlicer {
            cfg,
            n: 0,
            mean: 0.0,
            var: 0.0,
            active: None,
            pos: 0,
        }
    }

    /// The rules.
    pub fn config(&self) -> &SlicerConfig {
        &self.cfg
    }

    /// Sets the threshold in standard deviations from the next sample on.
    pub fn set_sigmas(&mut self, k: f64) {
        self.cfg.sigmas = k;
    }

    /// The noise is learned and impulses are decided.
    pub fn is_ready(&self) -> bool {
        self.n >= self.cfg.learn
    }

    /// The mean noise envelope.
    pub fn noise_mean(&self) -> f64 {
        self.mean
    }

    /// The threshold, envelope units.
    pub fn threshold(&self) -> f64 {
        self.mean + self.cfg.sigmas * self.var.max(0.0).sqrt()
    }

    /// The absolute index of the next sample.
    pub fn position(&self) -> u64 {
        self.pos
    }

    /// Forgets the noise and any impulse in progress; the next sample is index `pos`.
    pub fn reset(&mut self, pos: u64) {
        *self = EnvelopeSlicer::new(self.cfg);
        self.pos = pos;
    }

    fn learn(&mut self, e: f64, w: f64) {
        let d = e - self.mean;
        self.mean += w * d;
        self.var = (1.0 - w) * (self.var + w * d * d);
    }

    /// Takes the samples `x` that follow the last ones and appends the impulses that end in them to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Slice>) {
        for z in x {
            let e = f64::from(z.norm());
            let i = self.pos;
            self.pos += 1;
            if self.n < self.cfg.learn {
                self.n += 1;
                self.learn(e, 1.0 / self.n as f64);
                continue;
            }
            if e > self.threshold() {
                match &mut self.active {
                    Some((at, peak)) if e > *peak => {
                        *at = i;
                        *peak = e;
                    }
                    Some(_) => {}
                    None => self.active = Some((i, e)),
                }
                continue;
            }
            if let Some((at, peak)) = self.active.take() {
                out.push(Slice {
                    peak_index: at,
                    peak,
                    noise_mean: self.mean,
                });
            }
            self.n += 1;
            let w = (1.0 / self.cfg.tau.max(1.0)).max(1.0 / self.n as f64);
            self.learn(e, w);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(n: usize, sigma: f64, seed: u64) -> Vec<Complex32> {
        let mut s = seed;
        let mut u = || {
            s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((s >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        (0..n)
            .map(|_| {
                let r = sigma * (-2.0 * u().ln()).sqrt();
                let a = std::f64::consts::TAU * u();
                Complex32::new((r * a.cos()) as f32, (r * a.sin()) as f32)
            })
            .collect()
    }

    #[test]
    fn noise_learns_its_rayleigh_mean_and_gives_few_false_impulses() {
        let cfg = SlicerConfig {
            learn: 14_400,
            sigmas: 6.5,
            tau: 24_000.0,
        };
        let mut total = 0;
        for seed in 1..=4 {
            let mut s = EnvelopeSlicer::new(cfg);
            let mut out = Vec::new();
            s.process(&noise(48_000 * 20, 1.0, seed), &mut out);
            assert!(
                (s.noise_mean() / 1.2533 - 1.0).abs() < 0.02,
                "{}",
                s.noise_mean()
            );
            total += out.len();
        }
        // 3.8 million samples at 2.5e-7 each: about one.
        assert!(total <= 6, "{total}");
    }

    #[test]
    fn an_impulse_is_found_at_its_peak_and_does_not_move_the_noise() {
        let cfg = SlicerConfig {
            learn: 1000,
            sigmas: 6.5,
            tau: 24_000.0,
        };
        let mut s = EnvelopeSlicer::new(cfg);
        let mut x = noise(10_000, 1.0, 7);
        for (k, a) in [(5000usize, 10.0f32), (5001, 20.0), (5002, 12.0)] {
            x[k] = Complex32::new(a, 0.0);
        }
        let mut out = Vec::new();
        s.process(&x[..6000], &mut out);
        let before = s.noise_mean();
        s.process(&x[6000..], &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].peak_index, 5001);
        assert!((out[0].level_db() - 20.0 * (20.0f64 / 1.2533).log10()).abs() < 0.5);
        assert!((s.noise_mean() - before).abs() < 0.05);
    }
}
