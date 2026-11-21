//! Spectral kurtosis (Nita et al. 2007, PASP 119, 805; Nita and Gary 2010, MNRAS 406, L60). Per bin over `M` frames of bin
//! power `p`: `S1 = sum p`, `S2 = sum p^2`, `SK = (M + 1) / (M - 1) (M S2 / S1^2 - 1)`. Gaussian noise gives 1 with a standard
//! deviation of about `sqrt(4 / M)`, and `sqrt(4 / (M B))` for the mean of `B` estimates. A steady carrier that dominates its
//! bin drives SK towards 0; a pulse much stronger than the noise present in a share `q` of the frames gives about `1 / q - 1`.
//! [`SkEstimator`] keeps the sums of a spectrum of `n` bins and averages `B` estimates and the mean bin powers.

/// The spectral kurtosis of `m` frames with sums `s1` and `s2`; 1 (no information) for fewer than 2 frames or no power.
pub fn spectral_kurtosis(s1: f64, s2: f64, m: usize) -> f64 {
    if m < 2 || s1 <= 0.0 {
        return 1.0;
    }
    let m = m as f64;
    (m + 1.0) / (m - 1.0) * (m * s2 / (s1 * s1) - 1.0)
}

/// The expected standard deviation of the mean of `blocks` estimates of `frames` frames each for Gaussian noise.
pub fn sk_sigma(frames: usize, blocks: usize) -> f64 {
    (4.0 / (frames.max(1) * blocks.max(1)) as f64).sqrt()
}

/// The side of 1 on which a flagged estimate lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkKind {
    /// Over `1 + k sigma`: power that comes and goes.
    Pulsed,
    /// Under `1 - k sigma`: a steady carrier.
    Steady,
}

/// The flag of an estimate `sk` at `sigmas` standard deviations `sigma`; `None` within the limits.
pub fn sk_flag(sk: f64, sigma: f64, sigmas: f64) -> Option<SkKind> {
    if sk > 1.0 + sigmas * sigma {
        Some(SkKind::Pulsed)
    } else if sk < 1.0 - sigmas * sigma {
        Some(SkKind::Steady)
    } else {
        None
    }
}

/// The per-bin estimator of `B` estimates of `M` frames (module docs).
#[derive(Clone, Debug)]
pub struct SkEstimator {
    frames: usize,
    blocks: usize,
    s1: Vec<f64>,
    s2: Vec<f64>,
    sum_sk: Vec<f64>,
    sum_p: Vec<f64>,
    frame: usize,
    block: usize,
    sk: Vec<f64>,
    mean: Vec<f64>,
}

impl SkEstimator {
    /// An estimator of `n` bins, `frames` frames per estimate (at least 2) and `blocks` estimates per evaluation (at least 1).
    pub fn new(n: usize, frames: usize, blocks: usize) -> Self {
        SkEstimator {
            frames: frames.max(2),
            blocks: blocks.max(1),
            s1: vec![0.0; n],
            s2: vec![0.0; n],
            sum_sk: vec![0.0; n],
            sum_p: vec![0.0; n],
            frame: 0,
            block: 0,
            sk: vec![1.0; n],
            mean: vec![0.0; n],
        }
    }

    /// The expected standard deviation of an evaluation for Gaussian noise.
    pub fn sigma(&self) -> f64 {
        sk_sigma(self.frames, self.blocks)
    }

    /// Frames per evaluation.
    pub fn frames_per_evaluation(&self) -> usize {
        self.frames * self.blocks
    }

    /// Forgets the sums.
    pub fn reset(&mut self) {
        for v in [
            &mut self.s1,
            &mut self.s2,
            &mut self.sum_sk,
            &mut self.sum_p,
        ] {
            v.fill(0.0);
        }
        self.frame = 0;
        self.block = 0;
    }

    /// Takes the bin powers of one frame (`n` long). True when this frame completed an evaluation: [`SkEstimator::sk`]
    /// and [`SkEstimator::mean_power`] then hold it until the next.
    pub fn push(&mut self, power: &[f32]) -> bool {
        for ((a, b), &p) in self.s1.iter_mut().zip(self.s2.iter_mut()).zip(power) {
            let p = f64::from(p);
            *a += p;
            *b += p * p;
        }
        self.frame += 1;
        if self.frame < self.frames {
            return false;
        }
        self.frame = 0;
        let m = self.frames;
        for i in 0..self.s1.len() {
            self.sum_sk[i] += spectral_kurtosis(self.s1[i], self.s2[i], m);
            self.sum_p[i] += self.s1[i] / m as f64;
        }
        self.s1.fill(0.0);
        self.s2.fill(0.0);
        self.block += 1;
        if self.block < self.blocks {
            return false;
        }
        self.block = 0;
        let b = self.blocks as f64;
        for i in 0..self.sk.len() {
            self.sk[i] = self.sum_sk[i] / b;
            self.mean[i] = self.sum_p[i] / b;
        }
        self.sum_sk.fill(0.0);
        self.sum_p.fill(0.0);
        true
    }

    /// The mean SK of each bin at the last evaluation.
    pub fn sk(&self) -> &[f64] {
        &self.sk
    }

    /// The mean bin power of each bin at the last evaluation.
    pub fn mean_power(&self) -> &[f64] {
        &self.mean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Gauss(u64);
    impl Gauss {
        fn uniform(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        }
        /// A complex Gaussian value of unit power.
        fn complex(&mut self) -> (f64, f64) {
            let r = (-self.uniform().ln()).sqrt();
            let a = std::f64::consts::TAU * self.uniform();
            (r * a.cos(), r * a.sin())
        }
    }

    fn run(n: usize, tone_amp: f64, seed: u64) -> Vec<f64> {
        let mut g = Gauss(seed);
        let mut e = SkEstimator::new(n, 128, 8);
        let mut p = vec![0f32; n];
        for _ in 0..e.frames_per_evaluation() {
            for (i, q) in p.iter_mut().enumerate() {
                let (a, b) = g.complex();
                let a = a + if i == 0 { tone_amp } else { 0.0 };
                *q = (a * a + b * b) as f32;
            }
            if e.push(&p) {
                return e.sk().to_vec();
            }
        }
        panic!("no evaluation");
    }

    #[test]
    fn gaussian_noise_reads_one_and_a_steady_tone_reads_near_zero() {
        for seed in [11, 12, 13] {
            let sk = run(256, 1000f64.sqrt(), 0x2545_f491_4f6c_dd1d ^ seed);
            let mean = sk[1..].iter().sum::<f64>() / (sk.len() - 1) as f64;
            assert!((mean - 1.0).abs() < 0.02, "seed {seed}: {mean}");
            assert!(sk[0] < 0.1, "seed {seed}: tone {}", sk[0]);
        }
        assert!((sk_sigma(128, 8) - 0.0625).abs() < 1e-12);
        assert_eq!(sk_flag(1.3, 0.0625, 4.5), Some(SkKind::Pulsed));
        assert_eq!(sk_flag(0.7, 0.0625, 4.5), Some(SkKind::Steady));
        assert_eq!(sk_flag(1.2, 0.0625, 4.5), None);
    }
}
