//! Multipath fading: a tapped delay line `s(t) = sum_k c_k(t) e(t - tau_k)` and the named channel profiles of the standards
//! ([`profiles`]). Each tap gain `c_k(t)` is an independent stationary complex process with the tap's [`Doppler`] spectrum, and
//! the tap powers are scaled to sum to 1. Fractional delays add a latency of 8 samples ([`Tdl::latency`]).
//!
//! Standard: ETSI ES 201 980 equation B.1, 3GPP TS 45.005 clause C.1, ITU-R F.1487 Annex 1.

pub mod profiles;

pub use profiles::{EchoPath, Latitude, Profile, F1487};

use crate::gauss::inverse_normal_cdf;
use crate::rng::Rng;
use crate::{Param, SimError};
use sdrkit_algo_dsp::interp::{Interpolator, Polyphase};
use sdrkit_core::Complex32;
use std::f64::consts::{PI, TAU};

/// Sinusoids per fading gain.
pub const SINUSOIDS: usize = 32;
/// Gain updates per cycle of the fastest sinusoid (at least).
const UPDATES_PER_CYCLE: f64 = 64.0;
/// Most samples between gain updates.
const MAX_UPDATE: usize = 256;
/// Updates between exact re-anchoring of the rotators.
const REANCHOR: u32 = 1024;
/// Taps of the fractional-delay interpolator.
const FRAC_TAPS: usize = 16;

/// The Doppler spectrum of one tap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Doppler {
    /// No fading: a fixed gain of phase 0.
    Static,
    /// No fading: a fixed gain of phase `phase_rad`, turning at `shift_hz` (a steady path with a Doppler shift, or an echo with
    /// its own carrier phase).
    Steady {
        /// The frequency shift, Hz.
        shift_hz: f64,
        /// The phase at the start, radians.
        phase_rad: f64,
    },
    /// The classical (Clarke/Jakes) spectrum `1 / (pi f_d sqrt(1 - (f/f_d)^2))` on `(-f_d, f_d)`: CLASS in TS 45.005 clause C.2,
    /// "Classical" in ES 201 980 equation B.7.
    Jakes {
        /// The maximum Doppler frequency `f_d = v / lambda`, Hz.
        max_hz: f64,
    },
    /// A Gaussian spectrum centred on `shift_hz`, whose spread is two-sided and twice its standard deviation (`D_sp = 2 sigma`, so
    /// it holds 68 % of the power: ES 201 980 equation B.4, ITU-R F.1487 Annex 3 section 1).
    Gaussian {
        /// The centre (Doppler shift), Hz.
        shift_hz: f64,
        /// The spread `2 sigma`, Hz.
        spread_hz: f64,
    },
    /// A classical spectrum plus a line of sight: the Rice spectrum `A0 / (pi f_d sqrt(1 - (f/f_d)^2)) + A1 delta(f - f_los)`
    /// (TS 45.005 clause C.2, ES 201 980 equation B.8), with Rice factor `k = A1 / A0` (power of the direct path over the scattered
    /// power).
    Rice {
        /// The maximum Doppler frequency of the scattered part, Hz.
        max_hz: f64,
        /// The frequency of the direct path, Hz.
        los_hz: f64,
        /// The Rice factor as a power ratio (not dB).
        k: f64,
    },
}

impl Doppler {
    fn check(&self) -> Result<(), SimError> {
        let ok = match *self {
            Doppler::Static => true,
            Doppler::Steady {
                shift_hz,
                phase_rad,
            } => shift_hz.is_finite() && phase_rad.is_finite(),
            Doppler::Jakes { max_hz } => max_hz > 0.0 && max_hz.is_finite(),
            Doppler::Gaussian {
                shift_hz,
                spread_hz,
            } => shift_hz.is_finite() && spread_hz > 0.0 && spread_hz.is_finite(),
            Doppler::Rice { max_hz, los_hz, k } => {
                max_hz > 0.0
                    && max_hz.is_finite()
                    && los_hz.is_finite()
                    && k >= 0.0
                    && k.is_finite()
            }
        };
        if ok {
            Ok(())
        } else {
            Param::Doppler.degenerate()
        }
    }

    /// The highest frequency magnitude this spectrum's sinusoids can reach, Hz (for the Gaussian, the shift plus 5 sigma).
    fn max_freq(&self) -> f64 {
        match *self {
            Doppler::Static => 0.0,
            Doppler::Steady { shift_hz, .. } => shift_hz.abs(),
            Doppler::Jakes { max_hz } => max_hz,
            Doppler::Gaussian {
                shift_hz,
                spread_hz,
            } => shift_hz.abs() + 2.5 * spread_hz,
            Doppler::Rice { max_hz, los_hz, .. } => max_hz.max(los_hz.abs()),
        }
    }
}

/// One tap of a delay line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tap {
    /// The delay, seconds.
    pub delay_s: f64,
    /// The mean power, dB (relative: the line is scaled so the powers sum to 1).
    pub power_db: f64,
    /// How it fades.
    pub doppler: Doppler,
}

impl Tap {
    /// A tap.
    pub const fn new(delay_s: f64, power_db: f64, doppler: Doppler) -> Self {
        Tap {
            delay_s,
            power_db,
            doppler,
        }
    }
}

/// A complex sinusoid kept as a rotator.
#[derive(Clone, Copy, Debug)]
struct Osc {
    /// Cycles per update and the starting phase (cycles), for exact re-anchoring.
    freq: f64,
    phase0: f64,
    z: (f64, f64),
    r: (f64, f64),
}

impl Osc {
    fn new(freq: f64, phase0: f64) -> Self {
        let mut o = Osc {
            freq,
            phase0,
            z: (1.0, 0.0),
            r: (1.0, 0.0),
        };
        let (s, c) = (TAU * freq).sin_cos();
        o.r = (c, s);
        o.anchor(0);
        o
    }

    fn anchor(&mut self, update: u64) {
        let p = (self.phase0 + self.freq * update as f64).rem_euclid(1.0);
        let (s, c) = (TAU * p).sin_cos();
        self.z = (c, s);
    }

    #[inline]
    fn step(&mut self) {
        let (a, b) = self.z;
        let (c, d) = self.r;
        self.z = (a * c - b * d, a * d + b * c);
    }
}

/// The fading gain of one tap: a sum of sinusoids (module documentation), sampled once per update.
///
/// ```
/// use sdrkit_algo_sim::fading::{Doppler, Fader};
/// use sdrkit_algo_sim::rng::Rng;
/// // A Rayleigh gain with a 10 Hz maximum Doppler, sampled at 1 kHz.
/// let mut f = Fader::new(Doppler::Jakes { max_hz: 10.0 }, 1000.0, &mut Rng::new(1)).unwrap();
/// let g = f.next();
/// assert!(g.norm() < 10.0);
/// ```
#[derive(Clone, Debug)]
pub struct Fader {
    oscs: Vec<Osc>,
    /// Scale of the scattered sum (`sqrt(scattered power / N)`) and of the direct path.
    diffuse: f64,
    los: Option<(Osc, f64)>,
    count: u64,
}

impl Fader {
    /// A unit-power gain with spectrum `doppler`, sampled `update_rate_hz` times a second, its randomness drawn from `rng`.
    pub fn new(doppler: Doppler, update_rate_hz: f64, rng: &mut Rng) -> Result<Self, SimError> {
        doppler.check()?;
        if !(update_rate_hz > 0.0 && update_rate_hz.is_finite()) {
            return Param::UpdateRate.positive(update_rate_hz);
        }
        let n = SINUSOIDS;
        // The scattered part: its quantile function and power; the direct part: its frequency and power.
        type Quantile = Option<Box<dyn Fn(f64) -> f64>>;
        let (quantile, scattered, direct): (Quantile, f64, Option<(f64, f64, f64)>) = match doppler
        {
            Doppler::Static => (None, 0.0, Some((0.0, 1.0, 0.0))),
            Doppler::Steady {
                shift_hz,
                phase_rad,
            } => (None, 0.0, Some((shift_hz, 1.0, phase_rad / TAU))),
            Doppler::Jakes { max_hz } => {
                (Some(Box::new(move |p| -max_hz * (PI * p).cos())), 1.0, None)
            }
            Doppler::Gaussian {
                shift_hz,
                spread_hz,
            } => {
                let sigma = spread_hz / 2.0;
                (
                    Some(Box::new(move |p| shift_hz + sigma * inverse_normal_cdf(p))),
                    1.0,
                    None,
                )
            }
            Doppler::Rice { max_hz, los_hz, k } => (
                Some(Box::new(move |p| -max_hz * (PI * p).cos())),
                1.0 / (1.0 + k),
                Some((los_hz, k / (1.0 + k), f64::NAN)),
            ),
        };
        let mut oscs = Vec::new();
        if let Some(q) = quantile {
            for i in 0..n {
                let p = (i as f64 + rng.uniform()) / n as f64;
                oscs.push(Osc::new(q(p) / update_rate_hz, rng.uniform()));
            }
        }
        let diffuse = (scattered / n as f64).sqrt();
        // A steady path keeps phase 0 so a static channel is exactly its tap amplitudes; a line of sight inside a fading tap gets a
        // random phase.
        let los = direct.map(|(f, p, phase)| {
            let phase = if phase.is_nan() { rng.uniform() } else { phase };
            (Osc::new(f / update_rate_hz, phase), p.sqrt())
        });
        Ok(Fader {
            oscs,
            diffuse,
            los,
            count: 0,
        })
    }

    /// Whether the gain changes with time.
    pub fn is_static(&self) -> bool {
        self.oscs.is_empty() && self.los.as_ref().is_none_or(|(o, _)| o.freq == 0.0)
    }

    /// The next gain.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Complex32 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        let anchor = self.count.is_multiple_of(REANCHOR as u64) && self.count > 0;
        for o in &mut self.oscs {
            if anchor {
                o.anchor(self.count);
            }
            re += o.z.0;
            im += o.z.1;
            o.step();
        }
        re *= self.diffuse;
        im *= self.diffuse;
        if let Some((o, a)) = &mut self.los {
            if anchor {
                o.anchor(self.count);
            }
            re += o.z.0 * *a;
            im += o.z.1 * *a;
            o.step();
        }
        self.count += 1;
        Complex32::new(re as f32, im as f32)
    }
}

/// One tap of a running delay line.
#[derive(Clone, Debug)]
struct Line {
    /// Whole samples of delay (latency included) and the fraction, for the interpolator.
    whole: usize,
    mu: f32,
    frac: bool,
    amp: f32,
    fader: Fader,
    /// The gain now, the gain at the next update, and the step per sample between them.
    g: Complex32,
    target: Complex32,
    dg: Complex32,
}

/// A tapped delay line with fading taps (module documentation). Process in place, in pieces of any size.
///
/// ```
/// use sdrkit_algo_sim::fading::{Profile, Tdl};
/// use sdrkit_core::Complex32;
/// // DRM channel 4 ("CCIR poor") at 48 kHz.
/// let mut ch = Tdl::from_profile(&Profile::Drm(4), 48_000.0, 7).unwrap();
/// let mut x = vec![Complex32::new(1.0, 0.0); 4800];
/// ch.process(&mut x);
/// ```
#[derive(Clone, Debug)]
pub struct Tdl {
    lines: Vec<Line>,
    interp: Polyphase<FRAC_TAPS, 128>,
    /// History ring (power of two), written twice so a window is one slice.
    hist: Vec<Complex32>,
    mask: usize,
    w: usize,
    latency: usize,
    /// Samples between gain updates, and the position in the current interval.
    every: usize,
    pos: usize,
    is_static: bool,
}

impl Tdl {
    /// A delay line of `taps` for samples at `rate_hz`, seeded. Errors for no taps, a negative or non-finite delay or power, a
    /// delay above 10 s, or a bad Doppler spectrum.
    pub fn new(taps: &[Tap], rate_hz: f64, seed: u64) -> Result<Self, SimError> {
        if taps.is_empty() {
            return Param::Taps.at_least(1.0, 0.0);
        }
        if !(rate_hz > 0.0 && rate_hz.is_finite()) {
            return Param::Rate.positive(rate_hz);
        }
        let mut total = 0.0;
        let mut fmax = 0.0f64;
        for t in taps {
            Param::Tap.check_in(t.delay_s, 0.0, 10.0)?;
            Param::Level.check_finite(t.power_db)?;
            t.doppler.check()?;
            total += 10f64.powf(t.power_db / 10.0);
            fmax = fmax.max(t.doppler.max_freq());
        }
        let every = if fmax > 0.0 {
            ((rate_hz / (UPDATES_PER_CYCLE * fmax)).floor() as usize).clamp(1, MAX_UPDATE)
        } else {
            MAX_UPDATE
        };
        let update_rate = rate_hz / every as f64;
        let delays: Vec<f64> = taps.iter().map(|t| t.delay_s * rate_hz).collect();
        let any_frac = delays.iter().any(|d| (d - d.round()).abs() > 1e-9);
        let latency = if any_frac { FRAC_TAPS / 2 } else { 0 };
        let mut lines = Vec::with_capacity(taps.len());
        for (k, (t, &d)) in taps.iter().zip(&delays).enumerate() {
            let mut rng = Rng::stream(seed, 0x7A90 + k as u64);
            let fader = Fader::new(t.doppler, update_rate, &mut rng)?;
            let d = if (d - d.round()).abs() <= 1e-9 {
                d.round()
            } else {
                d
            };
            let whole = d.floor() as usize + latency;
            let mu = (d - d.floor()) as f32;
            lines.push(Line {
                whole,
                mu,
                frac: mu != 0.0,
                amp: (10f64.powf(t.power_db / 10.0) / total).sqrt() as f32,
                fader,
                g: Complex32::default(),
                target: Complex32::default(),
                dg: Complex32::default(),
            });
        }
        let max_delay = lines.iter().map(|l| l.whole).max().unwrap_or(0) + FRAC_TAPS;
        let size = (max_delay + 1).next_power_of_two();
        let is_static = lines.iter().all(|l| l.fader.is_static());
        let mut tdl = Tdl {
            lines,
            interp: Polyphase::design(0.9, 70.0)?,
            hist: vec![Complex32::default(); 2 * size],
            mask: size - 1,
            w: 0,
            latency,
            every,
            pos: 0,
            is_static,
        };
        tdl.start_interval();
        Ok(tdl)
    }

    /// The delay line of a named profile ([`Profile::taps`]).
    pub fn from_profile(profile: &Profile, rate_hz: f64, seed: u64) -> Result<Self, SimError> {
        Self::new(&profile.taps()?, rate_hz, seed)
    }

    /// The fixed latency added to every tap, samples (8 when a delay is fractional, else 0).
    pub fn latency(&self) -> usize {
        self.latency
    }

    /// The linear amplitude of each tap after scaling (their squares sum to 1).
    pub fn tap_amplitudes(&self) -> impl Iterator<Item = f32> + '_ {
        self.lines.iter().map(|l| l.amp)
    }

    fn start_interval(&mut self) {
        let inv = 1.0 / self.every as f32;
        for l in &mut self.lines {
            l.g = l.fader.next() * l.amp;
            l.target = l.fader.next() * l.amp;
            l.dg = (l.target - l.g) * inv;
        }
        self.pos = 0;
    }

    /// Lands every gain exactly on its update value and aims at the next.
    fn next_interval(&mut self) {
        let inv = 1.0 / self.every as f32;
        for l in &mut self.lines {
            l.g = l.target;
            l.target = l.fader.next() * l.amp;
            l.dg = (l.target - l.g) * inv;
        }
        self.pos = 0;
    }

    /// Runs `buf` through the channel in place.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        let size = self.mask + 1;
        let half = FRAC_TAPS / 2;
        for x in buf.iter_mut() {
            self.hist[self.w] = *x;
            self.hist[self.w + size] = *x;
            let mut acc = Complex32::default();
            for l in &mut self.lines {
                let v = if l.frac {
                    // The point `whole + mu` samples back lies a fraction `1 - mu` of the way from x[n - whole - 1] to
                    // x[n - whole]; the interpolator's window puts those at positions 7 and 8, so it starts `whole + 8` back.
                    let start = (self.w + size - (l.whole + half)) & self.mask;
                    self.interp
                        .at(&self.hist[start..start + FRAC_TAPS], 1.0 - l.mu)
                } else {
                    self.hist[(self.w + size - l.whole) & self.mask]
                };
                acc += v * l.g;
                l.g += l.dg;
            }
            *x = acc;
            self.w = (self.w + 1) & self.mask;
            if !self.is_static {
                self.pos += 1;
                if self.pos == self.every {
                    self.next_interval();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
