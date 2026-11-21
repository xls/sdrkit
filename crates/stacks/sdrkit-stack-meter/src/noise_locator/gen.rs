//! The impulse noise test signal: Gaussian impulses of random phase at a steady rate (which can drift), with timing
//! jitter, a spread of levels and a share of them missing, and optionally random impulses at a Poisson rate. A level is
//! the peak over the mean envelope of complex Gaussian noise of `gen_noise_sigma` per quadrature (1.2533 sigma); the
//! noise itself comes from the standard impairments.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::collections::VecDeque;
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

macro_rules! number {
    ($(#[$m:meta])* $name:ident = ($key:literal, $label:literal, $unit:literal, $default:expr, $min:expr, $max:expr)) => {
        $(#[$m])*
        pub const $name: Setting<field::F64> =
            Setting::new($key, $label, $unit, $default).range($min, $max, 0.0).section(SECTION);
    };
}

number!(
    /// The steady impulse rate; 0 for none.
    GEN_RATE_HZ = ("gen_rate_hz", "Impulse rate", "Hz", 100.0, 0.0, 10_000.0)
);
number!(
    /// The standard deviation of the timing of each impulse.
    GEN_JITTER_US = ("gen_jitter_us", "Timing jitter", "us", 60.0, 0.0, 10_000.0)
);
number!(
    /// The full width at half height of an impulse.
    GEN_WIDTH_US = ("gen_width_us", "Impulse width", "us", 70.0, 1.0, 100_000.0)
);
number!(
    /// The peak of an impulse over the mean noise envelope.
    GEN_PEAK_DB = ("gen_peak_db", "Impulse peak", "dB", 20.0, -40.0, 80.0)
);
number!(
    /// The spread of the peaks: uniform over this many dB around the peak.
    GEN_SPREAD_DB = ("gen_spread_db", "Peak spread", "dB", 3.0, 0.0, 40.0)
);
number!(
    /// The share of the steady impulses left out.
    GEN_DROP = ("gen_drop", "Missing share", "", 0.1, 0.0, 1.0)
);
number!(
    /// The rate of random impulses; 0 for none.
    GEN_RANDOM_RATE = ("gen_random_rate", "Random impulse rate", "Hz", 0.0, 0.0, 10_000.0)
);
number!(
    /// The peak of the random impulses over the mean noise envelope.
    GEN_RANDOM_DB = ("gen_random_db", "Random impulse peak", "dB", 12.0, -40.0, 80.0)
);
number!(
    /// The change of the steady rate per second.
    GEN_RATE_DRIFT_HZ_S = ("gen_rate_drift_hz_s", "Rate drift", "Hz/s", 0.0, -100.0, 100.0)
);
number!(
    /// The noise per quadrature that the levels refer to.
    GEN_NOISE_SIGMA = ("gen_noise_sigma", "Reference noise per quadrature", "", 1.0, 1e-6, 1e6)
);

/// One impulse: its centre (s), amplitude and phase.
#[derive(Clone, Copy, Debug)]
struct Impulse {
    t: f64,
    amp: f64,
    phase: f64,
}

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct ImpulseGenerator {
    rate: f64,
    steady: f64,
    drift: f64,
    jitter: f64,
    sigma_t: f64,
    peak: f64,
    spread: f64,
    drop: f64,
    random_rate: f64,
    random_peak: f64,
    reference: f64,
    next_steady: f64,
    next_random: f64,
    queue: VecDeque<Impulse>,
    rng: Rng,
    n: u64,
}

impl ImpulseGenerator {
    /// The signal of the settings `v` at `rate_hz`, its random parts from `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> ImpulseGenerator {
        let mut rng = Rng::new(seed ^ 0x696d_7075_6c73);
        let random_rate = v.get(&GEN_RANDOM_RATE);
        let next_random = if random_rate > 0.0 {
            -rng.uniform_open().ln() / random_rate
        } else {
            f64::INFINITY
        };
        ImpulseGenerator {
            rate: rate_hz,
            steady: v.get(&GEN_RATE_HZ),
            drift: v.get(&GEN_RATE_DRIFT_HZ_S),
            jitter: v.get(&GEN_JITTER_US) * 1e-6,
            sigma_t: v.get(&GEN_WIDTH_US) * 1e-6 / (8.0 * 2f64.ln()).sqrt(),
            peak: v.get(&GEN_PEAK_DB),
            spread: v.get(&GEN_SPREAD_DB),
            drop: v.get(&GEN_DROP),
            random_rate,
            random_peak: v.get(&GEN_RANDOM_DB),
            reference: 1.2533 * v.get(&GEN_NOISE_SIGMA),
            next_steady: 0.05,
            next_random,
            queue: VecDeque::new(),
            rng,
            n: 0,
        }
    }

    fn amplitude(&mut self, db: f64) -> f64 {
        let d = db + self.spread * (self.rng.uniform() - 0.5);
        self.reference * 10f64.powf(d / 20.0)
    }

    /// Queues the impulses whose centres lie before `until`.
    fn plan(&mut self, until: f64) {
        while self.steady > 0.0 && self.next_steady < until {
            let t = self.next_steady;
            let f = (self.steady + self.drift * t).max(1e-3);
            self.next_steady += 1.0 / f;
            if self.rng.uniform() < self.drop {
                continue;
            }
            let amp = self.amplitude(self.peak);
            let at = t + self.jitter * self.rng.normal();
            self.queue.push_back(Impulse {
                t: at,
                amp,
                phase: TAU * self.rng.uniform(),
            });
        }
        while self.next_random < until {
            let t = self.next_random;
            self.next_random += -self.rng.uniform_open().ln() / self.random_rate;
            let amp = self.amplitude(self.random_peak);
            self.queue.push_back(Impulse {
                t,
                amp,
                phase: TAU * self.rng.uniform(),
            });
        }
    }
}

impl Generator for ImpulseGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::default());
        let reach = 5.0 * self.sigma_t;
        let t0 = self.n as f64 / self.rate;
        let t1 = (self.n + out.len() as u64) as f64 / self.rate;
        self.plan(t1 + reach);
        for p in &self.queue {
            let a = (((p.t - reach - t0) * self.rate).ceil().max(0.0)) as usize;
            let b = ((((p.t + reach - t0) * self.rate).floor().max(-1.0) + 1.0) as usize)
                .min(out.len());
            let rot = Complex32::from_polar(1.0, p.phase as f32);
            for (i, z) in out.iter_mut().enumerate().take(b).skip(a) {
                let u = (t0 + i as f64 / self.rate - p.t) / self.sigma_t;
                *z += rot * (p.amp * (-0.5 * u * u).exp()) as f32;
            }
        }
        self.queue.retain(|p| p.t + reach >= t1);
        self.n += out.len() as u64;
        out.len()
    }
    /// The impulses have no reference power; the noise is given as a fixed floor.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.rate).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
