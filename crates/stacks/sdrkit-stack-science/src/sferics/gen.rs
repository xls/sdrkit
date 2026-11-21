//! The test signal of the sferics stack: flashes of lightning strokes at random gaps, and the impulses of a mains train.

use super::{GEN_MAINS_HZ, GEN_SCENE, GEN_STRIKE_RATE};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, Hz, SignalInfo};
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The decay time of a stroke, s.
pub const STROKE_TAU_S: f64 = 40e-6;
/// The rotation of a stroke, cycles per sample.
pub const STROKE_TURN: f64 = 0.15;
/// The decay time of a mains impulse, s.
pub const MAINS_TAU_S: f64 = 30e-6;
/// The rotation of a mains impulse, cycles per sample.
pub const MAINS_TURN: f64 = 0.2;
/// The time of the first mains impulse, s.
pub const MAINS_FIRST_S: f64 = 3.7e-3;
/// A stroke or impulse lasts this many decay times.
pub const LENGTH_TAUS: f64 = 7.0;
/// The strokes of a flash are 40 to 80 ms apart, s.
pub const STROKE_GAP_S: (f64, f64) = (0.04, 0.08);
/// The strokes of a flash are 0 to 8 dB under the set SNR.
pub const STROKE_SPREAD_DB: f64 = 8.0;
/// The mean flash gap is this over the strike rate, s.
pub const FLASH_GAP_FACTOR: f64 = 1.7;
/// The first flash falls in this span, s: after the floor is valid, early in a short run.
pub const FIRST_FLASH_S: (f64, f64) = (0.7, 1.7);

sdrkit_core::choice_enum! {
    /// What the test signal holds.
    pub enum Scene {
        /// Lightning only.
        Storm = ("storm", "Storm"),
        /// A mains train only.
        Mains = ("mains", "Mains train only"),
        /// Lightning and a mains train.
        StormMains = ("storm_mains", "Storm and mains train"),
        /// Nothing.
        Quiet = ("quiet", "Quiet"),
    }
}

/// Adds to `x` one decaying impulse: amplitude `amp`, starting at sample `start` of the stream whose first sample in `x` is
/// `first`, rising over 2 samples, decaying with `tau` samples for [`LENGTH_TAUS`] of them, rotating `turn` cycles a
/// sample from the phase `phase` (rad). The parts of the impulse outside `x` are left out, so an impulse can be added piece
/// by piece.
pub fn add_impulse(
    x: &mut [Complex32],
    first: u64,
    start: u64,
    amp: f64,
    tau: f64,
    turn: f64,
    phase: f64,
) {
    let len = ((LENGTH_TAUS * tau).ceil() as u64).max(3);
    let lo = start.max(first);
    let hi = (start + len).min(first + x.len() as u64);
    for n in lo..hi {
        let k = (n - start) as f64;
        let a = amp * ((k + 1.0) / 2.0).min(1.0) * (-k / tau).exp();
        let p = phase + TAU * (turn * k).fract();
        x[(n - first) as usize] += Complex32::from_polar(a as f32, p as f32);
    }
}

/// Adds a lightning stroke of amplitude `amp` at sample `start` at `rate` (see [`add_impulse`]).
pub fn add_stroke(x: &mut [Complex32], first: u64, rate: f64, start: u64, amp: f64, phase: f64) {
    add_impulse(
        x,
        first,
        start,
        amp,
        STROKE_TAU_S * rate,
        STROKE_TURN,
        phase,
    );
}

/// Adds the impulses of a mains train of amplitude `amp` and `per_s` impulses a second, from [`MAINS_FIRST_S`] on.
pub fn add_mains(x: &mut [Complex32], first: u64, rate: f64, per_s: f64, amp: f64) {
    let end = first + x.len() as u64;
    let tau = MAINS_TAU_S * rate;
    let len = ((LENGTH_TAUS * tau).ceil() as u64).max(3);
    let t0 = (first.saturating_sub(len) as f64 / rate - MAINS_FIRST_S) * per_s;
    let mut m = t0.ceil().max(0.0) as u64;
    loop {
        let start = ((MAINS_FIRST_S + m as f64 / per_s) * rate).round() as u64;
        if start >= end {
            break;
        }
        add_impulse(x, first, start, amp, tau, MAINS_TURN, 0.0);
        m += 1;
    }
}

/// A stroke waiting to be made.
#[derive(Clone, Copy, Debug)]
struct Stroke {
    start: u64,
    amp: f64,
    phase: f64,
}

/// The generator of the test signal. The strongest stroke has a peak amplitude of 1, and the signal power declared is 1 over
/// the whole rate, so the SNR of the impairments is the peak SNR of the strongest stroke; each stroke is 0 to 8 dB weaker.
/// The mains impulses have a peak amplitude of 1.
pub struct SfericsGenerator {
    rate: f64,
    rng: Rng,
    scene: Scene,
    strike_rate: f64,
    mains_per_s: f64,
    n: u64,
    next_flash: f64,
    strokes: VecDeque<Stroke>,
    /// The start of every stroke made or planned so far, s.
    placed: Vec<f64>,
}

impl SfericsGenerator {
    /// A generator at `rate` of `scene`, `strike_rate` strokes a second on average and a train of `mains_hz` mains (two
    /// impulses a period), from the random seed `seed`.
    pub fn new(rate: f64, scene: Scene, strike_rate: f64, mains_hz: f64, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let next_flash = rng.range(FIRST_FLASH_S.0, FIRST_FLASH_S.1);
        SfericsGenerator {
            rate,
            rng,
            scene,
            strike_rate,
            mains_per_s: 2.0 * mains_hz,
            n: 0,
            next_flash,
            strokes: VecDeque::new(),
            placed: Vec::new(),
        }
    }

    /// From the stack's settings.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        SfericsGenerator::new(
            cfg.rate_hz,
            s.get(&GEN_SCENE),
            s.get(&GEN_STRIKE_RATE),
            s.get(&GEN_MAINS_HZ),
            cfg.seed,
        )
    }

    /// The starts of the strokes planned so far, s.
    pub fn placed(&self) -> &[f64] {
        &self.placed
    }

    fn storm(&self) -> bool {
        matches!(self.scene, Scene::Storm | Scene::StormMains) && self.strike_rate > 0.0
    }

    /// Plans the flashes that begin before sample `end`.
    fn plan(&mut self, end: u64) {
        if !self.storm() {
            return;
        }
        while self.next_flash * self.rate < end as f64 {
            let strokes = 1 + self.rng.below(3);
            let mut t = self.next_flash;
            for _ in 0..strokes {
                let db = -STROKE_SPREAD_DB * self.rng.uniform();
                self.strokes.push_back(Stroke {
                    start: (t * self.rate).round() as u64,
                    amp: 10f64.powf(db / 20.0),
                    phase: TAU * self.rng.uniform(),
                });
                self.placed.push(t);
                t += self.rng.range(STROKE_GAP_S.0, STROKE_GAP_S.1);
            }
            self.next_flash += FLASH_GAP_FACTOR / self.strike_rate * self.rng.exponential();
        }
    }
}

impl Generator for SfericsGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::default());
        let first = self.n;
        let end = first + out.len() as u64;
        self.plan(end);
        let tau = STROKE_TAU_S * self.rate;
        let len = ((LENGTH_TAUS * tau).ceil() as u64).max(3);
        for s in self.strokes.iter().filter(|s| s.start < end) {
            add_stroke(out, first, self.rate, s.start, s.amp, s.phase);
        }
        self.strokes.retain(|s| s.start + len > end);
        if matches!(self.scene, Scene::Mains | Scene::StormMains) {
            add_mains(out, first, self.rate, self.mains_per_s, 1.0);
        }
        self.n = end;
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of the test signal is the peak SNR of the strongest stroke over the noise of the whole rate.
        Some(SignalInfo::new(self.rate))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

/// The generator.
///
/// # Errors
///
/// None: every setting gives a signal.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(SfericsGenerator::from_config(cfg)))
}
