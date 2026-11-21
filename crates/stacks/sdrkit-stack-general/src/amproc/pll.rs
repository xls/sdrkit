//! Carrier acquisition and the carrier loop of the AM processor.

use crate::smoothing::one_pole;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::loopfilter::loop_gains;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

const SEARCH_N: usize = 4096;
const SEARCH_FRAMES: usize = 4;
const ZETA: f64 = 0.707;
const FREQ_LIMIT_HZ: f64 = 3500.0;

/// The carrier search: 4096-point Hann spectra, 4 averaged. The strongest bin within +-3 kHz, when 20 dB over the median there.
pub struct CarrierSearch {
    rate: f64,
    fft: Fft,
    win: Vec<f32>,
    frame: Vec<Complex32>,
    fill: usize,
    acc: Vec<f32>,
    frames: usize,
    sel: Vec<f32>,
    scratch: Vec<f32>,
}

impl CarrierSearch {
    /// At `rate`.
    pub fn new(rate: f64) -> Self {
        CarrierSearch {
            rate,
            fft: Fft::forward(SEARCH_N),
            win: Window::Hann.coefficients(SEARCH_N),
            frame: vec![Complex32::default(); SEARCH_N],
            fill: 0,
            acc: vec![0.0; SEARCH_N],
            frames: 0,
            sel: Vec::with_capacity(1024),
            scratch: Vec::with_capacity(1024),
        }
    }
    /// Starts a new search.
    pub fn restart(&mut self) {
        self.fill = 0;
        self.frames = 0;
        self.acc.iter_mut().for_each(|v| *v = 0.0);
    }
    /// One sample. Returns the carrier frequency (Hz) when a search completes with a carrier.
    #[inline]
    pub fn push(&mut self, x: Complex32) -> Option<Option<f64>> {
        self.frame[self.fill] = x * self.win[self.fill];
        self.fill += 1;
        if self.fill < SEARCH_N {
            return None;
        }
        self.fill = 0;
        self.fft.process(&mut self.frame);
        for (a, v) in self.acc.iter_mut().zip(&self.frame) {
            *a += v.norm_sqr();
        }
        self.frames += 1;
        if self.frames < SEARCH_FRAMES {
            return None;
        }
        self.frames = 0;
        let b = self.rate / SEARCH_N as f64;
        let kmax = (3000.0 / b).floor() as i64;
        self.sel.clear();
        let (mut best, mut best_k) = (0.0f32, 0i64);
        for k in -kmax..=kmax {
            let i = k.rem_euclid(SEARCH_N as i64) as usize;
            let p = self.acc[i];
            self.sel.push(p);
            if p > best {
                best = p;
                best_k = k;
            }
        }
        let med = percentile(&self.sel, 0.5, &mut self.scratch);
        self.acc.iter_mut().for_each(|v| *v = 0.0);
        Some((best > 100.0 * med && best > 0.0).then_some(best_k as f64 * b))
    }
}

/// The carrier loop: a second-order PLL (zeta 0.707) on the derotated signal, the phase error taken from a 1 ms carrier filter.
pub struct CarrierLoop {
    rate: f64,
    /// The loop's phase (rad).
    phase: f64,
    /// The loop's frequency (rad per sample).
    freq: f64,
    cf: Complex32,
    cf_a: f32,
    kp_wide: f64,
    ki_wide: f64,
    kp_narrow: f64,
    ki_narrow: f64,
    since_acq: u64,
    level: f32,
    level_n: u64,
    level_a: f32,
    quality: f32,
    q_a: f32,
    /// Holding (a fade).
    pub holding: bool,
    /// Locked.
    pub locked: bool,
    good_for: u64,
    bad_for: u64,
    unlocked_for: u64,
    /// The carrier was found and the loop runs.
    pub running: bool,
}

impl CarrierLoop {
    /// At `rate`.
    pub fn new(rate: f64) -> Self {
        let gains =
            |bn: f64| loop_gains(bn / rate, ZETA, 1.0).expect("the loop bandwidths are valid");
        let (kp_wide, ki_wide) = gains(40.0);
        let (kp_narrow, ki_narrow) = gains(12.0);
        CarrierLoop {
            rate,
            phase: 0.0,
            freq: 0.0,
            cf: Complex32::default(),
            cf_a: one_pole(0.001, rate),
            kp_wide,
            ki_wide,
            kp_narrow,
            ki_narrow,
            since_acq: 0,
            level: 0.0,
            level_n: 0,
            level_a: one_pole(1.0, rate),
            quality: 0.0,
            q_a: one_pole(0.05, rate),
            holding: false,
            locked: false,
            good_for: 0,
            bad_for: 0,
            unlocked_for: 0,
            running: false,
        }
    }
    /// Starts the loop at a carrier `hz` from the centre, phase 0.
    pub fn acquire(&mut self, hz: f64) {
        self.freq = TAU * hz / self.rate;
        self.phase = 0.0;
        self.cf = Complex32::default();
        self.since_acq = 0;
        self.running = true;
        self.locked = false;
        self.holding = false;
        self.good_for = 0;
        self.bad_for = 0;
        self.unlocked_for = 0;
        self.quality = 0.0;
    }
    /// Stops the loop (a new search follows).
    pub fn drop_lock(&mut self) {
        self.running = false;
        self.locked = false;
        self.holding = false;
        self.phase = 0.0;
        self.freq = 0.0;
        self.cf = Complex32::default();
    }
    /// The carrier offset (Hz).
    pub fn offset_hz(&self) -> f64 {
        self.freq * self.rate / TAU
    }
    /// The carrier level over about a second (magnitude).
    pub fn level(&self) -> f32 {
        self.level
    }
    /// The carrier filter's magnitude now.
    pub fn carrier_mag(&self) -> f32 {
        self.cf.norm()
    }
    /// One sample: returns the derotated sample `r`. When the loop has been unlocked for 1.5 s it returns `true` in the second
    /// place (search again).
    #[inline]
    pub fn push(&mut self, x: Complex32) -> (Complex32, bool) {
        let (s, c) = self.phase.sin_cos();
        let r = x * Complex32::new(c as f32, -s as f32);
        let mut restart = false;
        if self.running {
            self.cf += (r - self.cf) * self.cf_a;
            let err = self.cf.im.atan2(self.cf.re) as f64;
            let mag = self.cf.norm();
            let warm = self.since_acq as f64 > 0.3 * self.rate;
            self.holding = warm && mag < 0.25 * self.level;
            let (kp, ki) = if warm {
                (self.kp_narrow, self.ki_narrow)
            } else {
                (self.kp_wide, self.ki_wide)
            };
            if self.holding {
                self.phase += self.freq;
            } else {
                self.freq += ki * err;
                let lim = TAU * FREQ_LIMIT_HZ / self.rate;
                self.freq = self.freq.clamp(-lim, lim);
                self.phase += self.freq + kp * err;
            }
            if self.phase > TAU || self.phase < -TAU {
                self.phase = self.phase.rem_euclid(TAU);
            }
            self.since_acq += 1;
            self.quality += self.q_a * (err.cos() as f32 - self.quality);
            // Lock decisions.
            if self.quality > 0.7 || self.holding {
                self.good_for += 1;
            } else {
                self.good_for = 0;
            }
            if self.quality < 0.4 && !self.holding {
                self.bad_for += 1;
            } else {
                self.bad_for = 0;
            }
            if !self.locked && self.good_for as f64 >= 0.1 * self.rate {
                self.locked = true;
            }
            if self.locked && self.bad_for as f64 >= 0.4 * self.rate {
                self.locked = false;
            }
            if self.locked {
                self.unlocked_for = 0;
            } else {
                self.unlocked_for += 1;
                if self.unlocked_for as f64 >= 1.5 * self.rate {
                    restart = true;
                }
            }
        }
        // The carrier level over a second: of the carrier filter when locked, else of the input.
        let m = if self.locked {
            self.cf.norm()
        } else {
            x.norm()
        };
        self.level_n += 1;
        if (self.level_n as f64) < self.rate {
            self.level += (m - self.level) / self.level_n as f32;
        } else {
            self.level += self.level_a * (m - self.level);
        }
        (r, restart)
    }
}
