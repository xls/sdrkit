//! An envelope slicer with a peak tracker, a floor tracker and a gate: the level changes of a two-level envelope (the load
//! modulation of a tag or a phone on a carrier, the ASK of a reader, a keyed carrier), with the carrier level and the swing.
//!
//! For each envelope value `e` at the working rate `R`:
//!
//! 1. the smoothed envelope `y += a (e - y)`, and the carrier level `m += 0.0005 (e - m)` (2000 samples);
//! 2. the peak `H` follows a rise of `y` by the attack factor and decays towards `y` with `1 / (peak decay x R)`; the floor `L`
//!    mirrors it with its own decay; the span is `S = H - L` and the middle `M = (H + L) / 2`;
//! 3. the noise of the envelope: the variance `N` of the change `e - e_prev` from sample to sample, updated only while that
//!    change is under `S / 4` (so the edges of the signal are left out); `sigma = sqrt(N / 2)`. From complex samples
//!    ([`TrackingSlicer::push_iq`]) `N` is instead the variance of the part of each sample at right angles to the carrier,
//!    which amplitude modulation does not move: its estimate holds for noise alone too, whose span is only a few deviations.
//!    `N` is the plain mean of its first values, then an exponential mean of weight 0.001; the gate stays shut for the first
//!    32;
//! 4. the gate ([`Gate`]): while it is shut no change is reported, and the state is forced or held ([`TrackConfig::shut_level`]);
//! 5. the state is high over `M + h S` and low under `M - h S` (hysteresis share `h`);
//! 6. the time of a change is the sample of the decision, or the last crossing of `M` in the same direction, interpolated
//!    between the two samples around it, when it lies close enough ([`EdgeTime`]);
//! 7. at each change the swing `W += 0.01 (S - W)` (the first change sets it); the modulation depth is `W / m`.
//!
//! During a start-up time `H = L = y` and `m = y`, so the filters before the slicer can fill. Times are fractional sample
//! indices of the working rate, counted from the first value.
//!
//! ```
//! use sdrkit_algo_modem::envtrack::{EdgeTime, Gate, TrackConfig, TrackingSlicer};
//! let cfg = TrackConfig {
//!     smoothing: 0.5,
//!     attack: 1.0,
//!     peak_decay_s: 0.06,
//!     floor_decay_s: 0.06,
//!     gate: Gate::Sigma(5.0),
//!     shut_level: Some(0),
//!     hysteresis: 0.22,
//!     edge: EdgeTime::Interpolated { within: 6.0 },
//!     start_s: 0.0,
//! };
//! let mut s = TrackingSlicer::new(cfg, 16_000.0);
//! // A carrier of level 1 with a 5 % dip of 40 samples every 80.
//! let env: Vec<f32> = (0..1600).map(|i| if (i / 40) % 2 == 1 { 0.95 } else { 1.0 }).collect();
//! let mut out = Vec::new();
//! s.process(&env, &mut out);
//! assert!(out.len() >= 30);
//! assert!((s.depth() - 0.05).abs() < 0.005);
//! ```

use crate::Complex32;

/// The carrier level follows the envelope with this weight per sample (a time constant of 2000 samples).
const CARRIER_WEIGHT: f64 = 0.0005;
/// The noise of the envelope follows with this weight per sample.
const NOISE_WEIGHT: f64 = 0.001;
/// The mean direction of the phase step follows with this weight per sample.
const STEP_WEIGHT: f64 = 0.01;
/// The noise changes that the gate needs before it opens.
const NOISE_LEARN: u64 = 32;

/// When the slicer decides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gate {
    /// Open while the span is at least `k` standard deviations of the envelope noise (plus `1e-6` of the carrier).
    Sigma(f64),
    /// Open while the span is over this share of the peak and the peak is over `1e-6`.
    Share(f64),
}

/// How the time of a change is found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EdgeTime {
    /// The sample of the decision.
    Sample,
    /// The last crossing of the middle in the same direction, when it lies at most `within` samples before the decision; else
    /// the sample of the decision.
    Interpolated {
        /// The longest distance of the crossing, samples.
        within: f64,
    },
}

/// The rules of a [`TrackingSlicer`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackConfig {
    /// The smoothing weight `a` of the envelope.
    pub smoothing: f64,
    /// The share of a rise of the peak (a fall of the floor) taken at once; 1 takes it whole.
    pub attack: f64,
    /// The decay time of the peak, s.
    pub peak_decay_s: f64,
    /// The decay time of the floor, s.
    pub floor_decay_s: f64,
    /// The gate.
    pub gate: Gate,
    /// The state while the gate is shut: `Some(level)` sets it without a report, `None` keeps it.
    pub shut_level: Option<u8>,
    /// The hysteresis share `h` of the span.
    pub hysteresis: f64,
    /// The time of a change.
    pub edge: EdgeTime,
    /// The start-up time, s.
    pub start_s: f64,
}

/// A level change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Change {
    /// Its time: a fractional sample index of the working rate.
    pub at: f64,
    /// The level before the change (0 low, 1 high).
    pub before: u8,
}

/// The slicer (module docs).
#[derive(Clone, Debug)]
pub struct TrackingSlicer {
    cfg: TrackConfig,
    rate: f64,
    seeded: bool,
    peak_w: f64,
    floor_w: f64,
    start: u64,
    pos: u64,
    y: f64,
    m: f64,
    hi: f64,
    lo: f64,
    noise: f64,
    noise_n: u64,
    z_prev: (f64, f64),
    mean_step: (f64, f64),
    e_prev: f64,
    d_prev: f64,
    state: u8,
    open: bool,
    swing: Option<f64>,
    up: Option<f64>,
    down: Option<f64>,
}

impl TrackingSlicer {
    /// A slicer with the rules `cfg` at the working rate `rate_hz`.
    pub fn new(cfg: TrackConfig, rate_hz: f64) -> TrackingSlicer {
        let r = rate_hz.max(1.0);
        TrackingSlicer {
            cfg,
            rate: r,
            seeded: false,
            peak_w: (1.0 / (cfg.peak_decay_s * r)).min(1.0),
            floor_w: (1.0 / (cfg.floor_decay_s * r)).min(1.0),
            start: (cfg.start_s * r).round() as u64,
            pos: 0,
            y: 0.0,
            m: 0.0,
            hi: 0.0,
            lo: 0.0,
            noise: 0.0,
            noise_n: 0,
            z_prev: (0.0, 0.0),
            mean_step: (0.0, 0.0),
            e_prev: 0.0,
            d_prev: 0.0,
            state: 0,
            open: false,
            swing: None,
            up: None,
            down: None,
        }
    }

    /// The rules.
    pub fn config(&self) -> &TrackConfig {
        &self.cfg
    }

    /// Forgets the trackers, the noise, the swing and the state; the sample count goes on, and a new start-up begins.
    pub fn reset(&mut self) {
        let pos = self.pos;
        *self = TrackingSlicer::new(self.cfg, self.rate);
        self.start += pos;
        self.pos = pos;
    }

    /// The working rate, Hz.
    pub fn rate_hz(&self) -> f64 {
        self.rate
    }

    /// Sets the gate from the next value on.
    pub fn set_gate(&mut self, g: Gate) {
        self.cfg.gate = g;
    }

    /// The index of the next value.
    pub fn position(&self) -> u64 {
        self.pos
    }

    /// The carrier level `m`.
    pub fn carrier(&self) -> f64 {
        self.m
    }

    /// The swing `W`; 0 before the first change.
    pub fn swing(&self) -> f64 {
        self.swing.unwrap_or(0.0)
    }

    /// The modulation depth `W / m`; 0 before the first change.
    pub fn depth(&self) -> f64 {
        self.swing() / self.m.max(1e-30)
    }

    /// The peak `H`.
    pub fn peak(&self) -> f64 {
        self.hi
    }

    /// The floor `L`.
    pub fn floor(&self) -> f64 {
        self.lo
    }

    /// The standard deviation of the envelope noise.
    pub fn sigma(&self) -> f64 {
        (self.noise / 2.0).max(0.0).sqrt()
    }

    /// The gate was open at the last value.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The state (0 low, 1 high).
    pub fn state(&self) -> u8 {
        self.state
    }

    /// Takes one envelope value and returns the change it decides. The noise comes from the change of the envelope.
    pub fn push(&mut self, e: f64) -> Option<Change> {
        self.step(e, None)
    }

    /// Takes one complex sample and returns the change its envelope decides. The noise comes from the part of the sample at
    /// right angles to the carrier: the step of the phase from the last sample, less its mean (the carrier offset), times the
    /// envelope. Amplitude modulation does not move it, so edges need not be left out, and noise alone gives its whole
    /// deviation.
    pub fn push_iq(&mut self, z: Complex32) -> Option<Change> {
        let (re, im) = (f64::from(z.re), f64::from(z.im));
        let e = re.hypot(im);
        let (pr, pi) = self.z_prev;
        self.z_prev = (re, im);
        // u = z conj(z_prev), as a unit vector.
        let (ur, ui) = (re * pr + im * pi, im * pr - re * pi);
        let un = ur.hypot(ui);
        let t = (un > 0.0).then(|| {
            let (ur, ui) = (ur / un, ui / un);
            let (mr, mi) = &mut self.mean_step;
            *mr += STEP_WEIGHT * (ur - *mr);
            *mi += STEP_WEIGHT * (ui - *mi);
            let mn = mr.hypot(*mi).max(1e-12);
            // The imaginary part of u conj(mean) / |mean|: the sine of the step less its mean.
            e * (ui * *mr - ur * *mi) / mn
        });
        self.step(e, t)
    }

    fn step(&mut self, e: f64, tangential: Option<f64>) -> Option<Change> {
        let i = self.pos;
        self.pos += 1;
        let c = &self.cfg;
        if !self.seeded {
            self.seeded = true;
            self.y = e;
            self.m = e;
            self.hi = e;
            self.lo = e;
            self.e_prev = e;
        }
        self.y += c.smoothing * (e - self.y);
        self.m += CARRIER_WEIGHT * (e - self.m);
        let dd = e - self.e_prev;
        self.e_prev = e;
        if i < self.start {
            self.hi = self.y;
            self.lo = self.y;
            self.m = self.y;
            self.d_prev = 0.0;
            self.open = false;
            return None;
        }
        let y = self.y;
        if y > self.hi {
            self.hi += c.attack * (y - self.hi);
        } else {
            self.hi -= self.peak_w * (self.hi - y);
        }
        if y < self.lo {
            self.lo += c.attack * (y - self.lo);
        } else {
            self.lo += self.floor_w * (y - self.lo);
        }
        let span = self.hi - self.lo;
        let mid = 0.5 * (self.hi + self.lo);
        if let Some(t) = tangential {
            self.noise_n += 1;
            let w = NOISE_WEIGHT.max(1.0 / self.noise_n as f64);
            self.noise += w * (t * t - self.noise);
        } else if dd.abs() < 0.25 * span {
            // A change under a quarter of the span is noise; a larger one is an edge.
            self.noise_n += 1;
            let w = NOISE_WEIGHT.max(1.0 / self.noise_n as f64);
            self.noise += w * (dd * dd - self.noise);
        }
        // Crossings of the middle, placed between the two samples around them.
        let d = y - mid;
        let dp = self.d_prev;
        if dp <= 0.0 && d > 0.0 && i > self.start {
            self.up = Some(i as f64 - 1.0 + dp / (dp - d));
        } else if dp >= 0.0 && d < 0.0 && i > self.start {
            self.down = Some(i as f64 - 1.0 + dp / (dp - d));
        }
        self.d_prev = d;
        self.open = match c.gate {
            Gate::Sigma(k) => {
                self.noise_n >= NOISE_LEARN && span >= k * self.sigma() + 1e-6 * self.m
            }
            Gate::Share(s) => span > s * self.hi && self.hi > 1e-6,
        };
        if !self.open {
            if let Some(l) = c.shut_level {
                self.state = l;
            }
            return None;
        }
        let h = c.hysteresis * span;
        let new = if y > mid + h {
            1
        } else if y < mid - h {
            0
        } else {
            self.state
        };
        if new == self.state {
            return None;
        }
        let decided = i as f64;
        let at = match c.edge {
            EdgeTime::Sample => decided,
            EdgeTime::Interpolated { within } => {
                let cross = if new == 1 { self.up } else { self.down };
                match cross {
                    Some(t) if decided - t <= within && t <= decided => t,
                    _ => decided,
                }
            }
        };
        let before = self.state;
        self.state = new;
        self.swing = Some(match self.swing {
            None => span,
            Some(w) => w + 0.01 * (span - w),
        });
        Some(Change { at, before })
    }

    /// Takes envelope values and appends the changes they decide to `out`.
    pub fn process(&mut self, env: &[f32], out: &mut Vec<Change>) {
        for &e in env {
            if let Some(c) = self.push(f64::from(e)) {
                out.push(c);
            }
        }
    }

    /// Takes complex samples, slices their envelope `|x|` ([`TrackingSlicer::push_iq`]) and appends the changes to `out`.
    pub fn process_iq(&mut self, x: &[Complex32], out: &mut Vec<Change>) {
        for &z in x {
            if let Some(c) = self.push_iq(z) {
                out.push(c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(gate: Gate, edge: EdgeTime) -> TrackConfig {
        TrackConfig {
            smoothing: 0.5,
            attack: 1.0,
            peak_decay_s: 0.06,
            floor_decay_s: 0.06,
            gate,
            shut_level: Some(0),
            hysteresis: 0.22,
            edge,
            start_s: 0.02,
        }
    }

    fn gauss(s: &mut u64) -> f64 {
        let mut u = || {
            *s ^= *s << 13;
            *s ^= *s >> 7;
            *s ^= *s << 17;
            (*s >> 11) as f64 / (1u64 << 53) as f64
        };
        let (a, b) = (u().max(1e-300), u());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }

    #[test]
    fn a_square_dip_gives_its_edges_and_its_depth() {
        let rate = 16_000.0;
        let mut s = TrackingSlicer::new(
            cfg(Gate::Sigma(5.0), EdgeTime::Interpolated { within: 6.0 }),
            rate,
        );
        // A 4 % dip of 40 samples every 80, after the start-up.
        let env: Vec<f32> = (0..8000)
            .map(|i| {
                if i >= 400 && (i / 40) % 2 == 1 {
                    0.96
                } else {
                    1.0
                }
            })
            .collect();
        let mut out = Vec::new();
        s.process(&env, &mut out);
        assert!(out.len() > 180, "{}", out.len());
        // Each change lies within a sample of the edge of the dip (the smoothing delays the crossing by about a sample).
        for c in &out[2..] {
            let r = c.at.rem_euclid(40.0);
            assert!(r < 1.5 || r > 39.5, "{c:?}");
        }
        assert!((s.depth() - 0.04).abs() < 0.002, "{}", s.depth());
        assert!((s.carrier() - 0.98).abs() < 0.01);
    }

    #[test]
    fn a_carrier_with_noise_and_noise_alone_open_no_sigma_gate() {
        let rate = 16_000.0;
        for seed in 1..=6u64 {
            for carrier in [1.0f64, 0.0] {
                let mut s = TrackingSlicer::new(cfg(Gate::Sigma(5.0), EdgeTime::Sample), rate);
                let mut st = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
                let mut n = 0;
                for k in 0..160_000 {
                    let (a, b) = (gauss(&mut st), gauss(&mut st));
                    // A carrier 1 kHz off the centre.
                    let ph = std::f64::consts::TAU * 1000.0 * k as f64 / rate;
                    let z = Complex32::new(
                        (carrier * ph.cos() + 0.01 * a) as f32,
                        (carrier * ph.sin() + 0.01 * b) as f32,
                    );
                    n += usize::from(s.push_iq(z).is_some());
                }
                assert_eq!(n, 0, "seed {seed}, carrier {carrier}");
            }
        }
    }

    #[test]
    fn the_share_gate_holds_the_state_and_decides_a_keyed_carrier() {
        let mut c = cfg(Gate::Share(0.25), EdgeTime::Sample);
        c.shut_level = None;
        c.attack = 0.06;
        c.smoothing = 0.12;
        c.hysteresis = 0.12;
        let mut s = TrackingSlicer::new(c, 160_000.0);
        let mut out = Vec::new();
        // A steady carrier: no change.
        s.process(&vec![1.0f32; 32_000], &mut out);
        assert!(out.is_empty());
        // Pulses of 56 samples to zero every 240 samples.
        let env: Vec<f32> = (0..32_000)
            .map(|i| if i % 240 < 56 { 0.0 } else { 1.0 })
            .collect();
        s.process(&env, &mut out);
        assert!(out.len() > 200, "{}", out.len());
    }
}
