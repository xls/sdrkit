//! Automatic gain control.

use crate::Complex32;

/// The most that an output sample passes the target, as a ratio (3 dB), unless [`Agc::with_overshoot`] sets another.
const OVERSHOOT: f32 = std::f32::consts::SQRT_2;

/// An envelope AGC. The envelope follows `|x|` with a fast attack (when the signal rises) and a slow decay (when it falls); the gain is
/// `target / envelope`, capped at `max_gain`. The output envelope is then at `target`.
///
/// The gain of a sample is also never more than `OVERSHOOT * target / |x|`: no output sample passes the target by more than 3 dB, also
/// at the start of a burst after silence, while the envelope still rises. This limit has no memory and does not change the envelope, so
/// on a steady signal the envelope alone sets the gain.
#[derive(Clone, Debug)]
pub struct Agc {
    rate: f64,
    target: f32,
    max_gain: f32,
    attack: f32,
    decay: f32,
    level: f32,
    overshoot: f32,
}

impl Agc {
    /// An AGC for `rate`, aiming the output envelope at `target` (for example 0.5), with attack and decay time constants in seconds and
    /// a gain cap.
    pub fn new(rate: f64, target: f32, attack_s: f64, decay_s: f64, max_gain: f32) -> Self {
        assert!(rate > 0.0 && target > 0.0 && max_gain > 0.0);
        let mut a = Agc {
            rate,
            target,
            max_gain,
            attack: 0.0,
            decay: 0.0,
            level: 0.0,
            overshoot: OVERSHOOT,
        };
        a.set_times(attack_s, decay_s);
        a
    }

    /// Sets the most that an output sample passes the target, as a ratio of at least 1. `f32::INFINITY` is no limit: the start of a
    /// burst after silence is then louder than the target by up to the attack time in samples.
    pub fn with_overshoot(mut self, ratio: f32) -> Self {
        assert!(ratio >= 1.0);
        self.overshoot = ratio;
        self
    }

    fn coeff(rate: f64, t: f64) -> f32 {
        assert!(t > 0.0, "time constants must be positive");
        (1.0 - (-1.0 / (t * rate)).exp()) as f32
    }

    /// Changes the attack and decay time constants.
    pub fn set_times(&mut self, attack_s: f64, decay_s: f64) {
        self.attack = Self::coeff(self.rate, attack_s);
        self.decay = Self::coeff(self.rate, decay_s);
    }

    /// Changes the target.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Changes the gain cap.
    pub fn set_max_gain(&mut self, max_gain: f32) {
        self.max_gain = max_gain;
    }

    /// The current gain of the envelope (a sample over the 3 dB limit had less).
    pub fn gain(&self) -> f32 {
        if self.level * self.max_gain <= self.target {
            self.max_gain
        } else {
            self.target / self.level
        }
    }

    /// The current gain in dB.
    pub fn gain_db(&self) -> f32 {
        20.0 * self.gain().log10()
    }

    /// Forgets the envelope (the gain goes back to the cap).
    pub fn reset(&mut self) {
        self.level = 0.0;
    }

    /// Takes the magnitude `m` of a sample into the envelope and returns the gain for that sample.
    #[inline]
    fn step(&mut self, m: f32) -> f32 {
        let a = if m > self.level {
            self.attack
        } else {
            self.decay
        };
        self.level += a * (m - self.level);
        let g = self.gain();
        let top = self.overshoot * self.target;
        if m * g > top {
            top / m
        } else {
            g
        }
    }

    /// Applies the AGC to complex samples in place.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        for v in buf {
            let g = self.step(v.norm());
            *v *= g;
        }
    }

    /// Applies the AGC to real samples in place (the envelope of a real signal is `|x|`; for a steady tone it follows the peaks).
    pub fn process_real(&mut self, buf: &mut [f32]) {
        for v in buf {
            let g = self.step(v.abs());
            *v *= g;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn settles_on_steps_up_and_down() {
        let rate = 48_000.0;
        let mut agc = Agc::new(rate, 0.5, 0.002, 0.05, 1e4);
        let level_db = |agc: &mut Agc, amp: f64, secs: f64| {
            let mut x = ctone((rate * secs) as usize, 0.01, amp);
            agc.process(&mut x);
            let tail = &x[x.len() - 480..];
            db(tail.iter().map(|v| v.norm() as f64).sum::<f64>() / tail.len() as f64 / 0.5)
        };
        // Weak signal: settles within 1 dB in 20 ms.
        assert!(level_db(&mut agc, 0.01, 0.02).abs() < 1.0);
        // 26 dB up: the fast attack gets there in 15 ms.
        assert!(level_db(&mut agc, 0.2, 0.015).abs() < 1.0);
        // 26 dB down: the slow decay needs a few time constants (0.3 s).
        assert!(level_db(&mut agc, 0.01, 0.3).abs() < 1.0);
        assert!((agc.gain_db() - 34.0).abs() < 0.5);
        // The cap holds on silence.
        let mut z = vec![Complex32::default(); 48_000];
        agc.process(&mut z);
        assert_eq!(agc.gain(), 1e4);
    }

    /// The settings of the consumers: (rate, target, attack s, decay s, largest gain).
    const USES: [(f64, f32, f64, f64, f32); 5] = [
        (48_000.0, 0.5, 0.005, 0.5, 1e3),
        (8_000.0, 0.7, 0.05, 2.0, 7e6),
        (8_000.0, 0.5, 0.012_44, 1.25, 1e6),
        (8_000.0, 0.3, 8.3e-3, 1.67, 1e6),
        (48_000.0, 0.5, 0.002, 0.05, 1e4),
    ];

    /// The largest output magnitude over the target, dB, in the first 50 ms of `burst` after `lead`, on the complex and on the real path.
    fn overshoot_db(u: (f64, f32, f64, f64, f32), lead: &[Complex32], amp: f64) -> f64 {
        let n = (0.05 * u.0) as usize;
        let mut agc = Agc::new(u.0, u.1, u.2, u.3, u.4);
        let mut x = lead.to_vec();
        agc.process(&mut x);
        let mut b = ctone(n, 0.11, amp);
        agc.process(&mut b);
        let pc = b.iter().map(|v| v.norm()).fold(0.0f32, f32::max);
        let mut agc = Agc::new(u.0, u.1, u.2, u.3, u.4);
        let mut x: Vec<f32> = lead.iter().map(|v| v.re).collect();
        agc.process_real(&mut x);
        let mut b = rtone(n, 0.11, amp);
        agc.process_real(&mut b);
        let pr = b.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        db((pc.max(pr) / u.1) as f64)
    }

    /// The bound on the output: 3 dB over the target on every sample, so below full scale for a target of 0.7.
    const BOUND_DB: f64 = 3.02;

    #[test]
    fn a_burst_after_silence_stays_within_3_db_of_the_target() {
        for u in USES {
            let lead = vec![Complex32::default(); u.0 as usize];
            let o = overshoot_db(u, &lead, 0.1);
            assert!(o <= BOUND_DB, "{u:?}: {o:.1} dB");
            assert!(u.1 as f64 * 10f64.powf(o / 20.0) < 1.0, "{u:?}: over full scale");
        }
    }

    #[test]
    fn a_burst_after_weak_noise_stays_within_3_db_of_the_target() {
        for u in USES {
            let mut rng = Rng(7);
            let s = 10f64.powf(-90.0 / 20.0) / 2f64.sqrt();
            let lead: Vec<Complex32> = (0..u.0 as usize)
                .map(|_| Complex32::new((s * rng.gauss()) as f32, (s * rng.gauss()) as f32))
                .collect();
            let o = overshoot_db(u, &lead, 0.1);
            assert!(o <= BOUND_DB, "{u:?}: {o:.1} dB");
        }
    }

    #[test]
    fn a_step_up_of_30_db_stays_within_3_db_of_the_target() {
        for u in USES {
            let lead = ctone(u.0 as usize, 0.11, 0.01);
            let o = overshoot_db(u, &lead, 10f64.powf(-0.5));
            assert!(o <= BOUND_DB, "{u:?}: {o:.1} dB");
        }
    }

    /// The share of the samples of the last second of `out` that the 3 dB limit held (such a sample is at the limit).
    fn held(u: (f64, f32, f64, f64, f32), out: impl DoubleEndedIterator<Item = f32>) -> f64 {
        let n = u.0 as usize;
        out.rev().take(n).filter(|&m| m >= 0.9999 * OVERSHOOT * u.1).count() as f64 / n as f64
    }

    #[test]
    fn the_limit_leaves_a_steady_signal_alone() {
        for u in USES {
            let n = (4.0 * u.0) as usize;
            let mut agc = Agc::new(u.0, u.1, u.2, u.3, u.4);
            let mut x = ctone(n, 0.11, 0.1);
            agc.process(&mut x);
            assert_eq!(held(u, x.iter().map(|v| v.norm())), 0.0, "{u:?}");
            let mut agc = Agc::new(u.0, u.1, u.2, u.3, u.4);
            let mut x = rtone(n, 0.11, 0.1);
            agc.process_real(&mut x);
            assert_eq!(held(u, x.iter().map(|v| v.abs())), 0.0, "{u:?}");
            // Complex Gaussian noise: the limit holds the highest peaks only.
            let mut rng = Rng(3);
            let mut x: Vec<Complex32> = (0..n)
                .map(|_| Complex32::new((0.05 * rng.gauss()) as f32, (0.05 * rng.gauss()) as f32))
                .collect();
            agc.reset();
            agc.process(&mut x);
            let h = held(u, x.iter().map(|v| v.norm()));
            eprintln!("{u:?}: noise held {:.3} %", 100.0 * h);
            assert!(h < 0.01, "{u:?}: {h}");
        }
    }

    #[test]
    fn a_click_is_held_and_moves_the_gain_as_the_envelope_does() {
        let u = USES[1];
        let mut agc = Agc::new(u.0, u.1, u.2, u.3, u.4);
        let mut x = ctone(2 * u.0 as usize, 0.11, 0.05);
        agc.process(&mut x);
        let before = agc.gain_db();
        let mut click = [Complex32::new(0.5, 0.0)];
        agc.process(&mut click);
        assert!(click[0].norm() <= u.1 * 1.4143);
        // One sample at 10 times the level moves the envelope by 9 attack steps: 0.2 dB.
        assert!((agc.gain_db() - before).abs() < 0.25, "{} {before}", agc.gain_db());
    }

    #[test]
    fn without_a_limit_the_burst_is_as_loud_as_the_attack_allows() {
        let u = USES[1];
        let mut agc = Agc::new(u.0, u.1, u.2, u.3, u.4).with_overshoot(f32::INFINITY);
        let mut b = ctone(8, 0.11, 0.1);
        agc.process(&mut b);
        // The first sample is at target / attack factor: 400 times the target.
        assert!((b[0].norm() / u.1 / 400.0 - 1.0).abs() < 0.01, "{}", b[0].norm());
    }

    /// Prints the cost of a sample: `cargo test --release -p sdrkit-algo-dsp agc::tests::cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn cost() {
        let mut x = ctone(1 << 16, 0.11, 0.1);
        let mut r = rtone(1 << 16, 0.11, 0.1);
        let mut agc = Agc::new(8_000.0, 0.7, 0.05, 2.0, 7e6);
        let t = std::time::Instant::now();
        for _ in 0..200 {
            agc.process(std::hint::black_box(&mut x));
            let g = 0.1 / x[0].norm();
            x.iter_mut().for_each(|v| *v *= g);
        }
        let c = t.elapsed().as_secs_f64();
        let t = std::time::Instant::now();
        for _ in 0..200 {
            agc.process_real(std::hint::black_box(&mut r));
        }
        let k = 1e9 / (200.0 * 65536.0);
        eprintln!("complex {:.2} ns, real {:.2} ns a sample", c * k, t.elapsed().as_secs_f64() * k);
    }
}
