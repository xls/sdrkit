//! Squelch: deciding whether a channel holds a signal, measured on short blocks with one shared gate.
//!
//! | Squelch | Measures | Threshold |
//! |---|---|---|
//! | [`PowerSquelch`] | the block's mean power, dBFS | fixed levels |
//! | [`SnrSquelch`] | the block's power over a noise floor tracked while closed (`sdrkit_algo_dsp::floor`) | dB above the floor: follows the band's noise without a setting |
//! | [`QuietingSquelch`] | for FM: the voice-band power over the power above the voice band in the discriminator output (a carrier quiets the noise) | dB of the ratio |
//!
//! Every squelch passes its measurement through a [`Gate`]: it opens after `attack` blocks above `open_db`, stays open while the
//! measure is above `close_db` (below `open_db`, the hysteresis), and closes after `hang` blocks below it. A block of digital
//! silence (all zeros: a gap in a file) never opens it.
//!
//! ```
//! use sdrkit_algo_modem::squelch::{Gate, PowerSquelch};
//! use sdrkit_algo_modem::Complex32;
//! let mut sq = PowerSquelch::new(480, Gate::new(-40.0, -45.0, 1, 5).unwrap()).unwrap();
//! assert!(!sq.process(&[Complex32::new(0.001, 0.0); 4800])); // -60 dBFS
//! assert!(sq.process(&[Complex32::new(0.1, 0.0); 4800])); // -20 dBFS
//! ```

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::floor::FloorTracker;
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_algo_dsp::meter::power_to_db;

/// Open and close decisions with hysteresis, attack and hang counts (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Gate {
    open_db: f32,
    close_db: f32,
    attack: u32,
    hang: u32,
    above: u32,
    below: u32,
    open: bool,
}

impl Gate {
    /// Opens after `attack` blocks (at least 1) above `open_db`, closes after `hang` blocks below `close_db`. Errors unless
    /// `close_db <= open_db` and both are finite.
    pub fn new(open_db: f32, close_db: f32, attack: u32, hang: u32) -> Result<Self, ModemError> {
        Param::Threshold.check_finite(open_db as f64)?;
        Param::Threshold.check_finite(close_db as f64)?;
        if close_db > open_db {
            return Param::CloseLevel.conflict(Param::OpenLevel);
        }
        Ok(Gate {
            open_db,
            close_db,
            attack: attack.max(1),
            hang,
            above: 0,
            below: 0,
            open: false,
        })
    }

    /// Changes the thresholds (a user setting), keeping the state.
    pub fn set_thresholds(&mut self, open_db: f32, close_db: f32) -> Result<(), ModemError> {
        Param::Threshold.check_finite(open_db as f64)?;
        Param::Threshold.check_finite(close_db as f64)?;
        if close_db > open_db {
            return Param::CloseLevel.conflict(Param::OpenLevel);
        }
        self.open_db = open_db;
        self.close_db = close_db;
        Ok(())
    }

    /// Takes one block's measure (`None` for a block that cannot open the gate, such as silence); returns whether open.
    pub fn update(&mut self, measure_db: Option<f32>) -> bool {
        let m = measure_db
            .filter(|v| !v.is_nan())
            .unwrap_or(f32::NEG_INFINITY);
        if self.open {
            if m >= self.close_db {
                self.below = 0;
            } else {
                self.below += 1;
                if self.below > self.hang {
                    self.open = false;
                    self.above = 0;
                }
            }
        } else if m >= self.open_db {
            self.above += 1;
            if self.above >= self.attack {
                self.open = true;
                self.below = 0;
            }
        } else {
            self.above = 0;
        }
        self.open
    }

    /// Whether open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Closed, counts cleared.
    pub fn reset(&mut self) {
        self.open = false;
        self.above = 0;
        self.below = 0;
    }
}

/// Mean power of blocks of `n` complex or real samples, collected across calls.
#[derive(Clone, Debug)]
struct Blocks {
    n: usize,
    acc: f64,
    k: usize,
}

impl Blocks {
    fn new(n: usize) -> Result<Self, ModemError> {
        if n == 0 {
            return Param::BlockLen.at_least(1.0, 0.0);
        }
        Ok(Blocks { n, acc: 0.0, k: 0 })
    }
    /// Adds one power value; returns the block's mean when it completes.
    #[inline]
    fn push(&mut self, p: f32) -> Option<f64> {
        if p.is_finite() {
            self.acc += p as f64;
        }
        self.k += 1;
        if self.k == self.n {
            let m = self.acc / self.n as f64;
            self.acc = 0.0;
            self.k = 0;
            Some(m)
        } else {
            None
        }
    }
    fn reset(&mut self) {
        self.acc = 0.0;
        self.k = 0;
    }
}

fn db(p: f64) -> Option<f32> {
    (p > 1e-30).then(|| power_to_db(p) as f32)
}

/// Squelch on block power in dBFS (a mean power of 1 is 0 dBFS).
#[derive(Clone, Debug)]
pub struct PowerSquelch {
    blocks: Blocks,
    gate: Gate,
    level_db: Option<f32>,
}

impl PowerSquelch {
    /// Blocks of `block` samples through `gate` (thresholds in dBFS).
    pub fn new(block: usize, gate: Gate) -> Result<Self, ModemError> {
        Ok(PowerSquelch {
            blocks: Blocks::new(block)?,
            gate,
            level_db: None,
        })
    }

    /// Takes samples; returns whether open after the last complete block.
    pub fn process(&mut self, x: &[Complex32]) -> bool {
        for z in x {
            if let Some(m) = self.blocks.push(z.norm_sqr()) {
                self.level_db = db(m);
                self.gate.update(self.level_db);
            }
        }
        self.gate.is_open()
    }

    /// The last block's level, dBFS (`None` for silence).
    pub fn level_db(&self) -> Option<f32> {
        self.level_db
    }

    /// The gate (to change thresholds).
    pub fn gate_mut(&mut self) -> &mut Gate {
        &mut self.gate
    }

    /// Whether open.
    pub fn is_open(&self) -> bool {
        self.gate.is_open()
    }

    /// Closed, partial block dropped.
    pub fn reset(&mut self) {
        self.blocks.reset();
        self.gate.reset();
        self.level_db = None;
    }
}

/// Squelch on block power over a tracked noise floor: the floor follows the band's noise while the squelch is closed (and is held
/// while open, so a transmission does not raise it), and the gate's thresholds are dB above that floor. If the squelch stays open
/// longer than `max_hold` blocks the floor follows again, so a rise of the noise itself (a new antenna, interference) closes it
/// in the end instead of holding it open for ever.
#[derive(Clone, Debug)]
pub struct SnrSquelch {
    blocks: Blocks,
    gate: Gate,
    floor: FloorTracker,
    snr_db: Option<f32>,
    held: u32,
    max_hold: u32,
}

impl SnrSquelch {
    /// Blocks of `block` samples, the floor from `floor` (fed one block power at a time; a `MinFollower` or `ChunkPercentile` suits),
    /// thresholds of `gate` in dB above the floor.
    pub fn new(block: usize, floor: FloorTracker, gate: Gate) -> Result<Self, ModemError> {
        Ok(SnrSquelch {
            blocks: Blocks::new(block)?,
            gate,
            floor,
            snr_db: None,
            held: 0,
            max_hold: 500,
        })
    }

    /// Blocks the floor is held while open (default 500) before it follows again.
    pub fn set_max_hold(&mut self, blocks: u32) {
        self.max_hold = blocks;
    }

    /// Takes samples; returns whether open after the last complete block.
    pub fn process(&mut self, x: &[Complex32]) -> bool {
        for z in x {
            if let Some(m) = self.blocks.push(z.norm_sqr()) {
                if m <= 1e-30 {
                    self.snr_db = None;
                    self.gate.update(None);
                    continue;
                }
                self.held = if self.gate.is_open() {
                    self.held.saturating_add(1)
                } else {
                    0
                };
                if self.held == 0 || self.held > self.max_hold {
                    self.floor.update(&[m as f32]);
                }
                self.snr_db = match self.floor.floor() {
                    Some(f) if f > 0.0 => Some((10.0 * (m / f as f64).log10()) as f32),
                    _ => None,
                };
                self.gate.update(self.snr_db);
            }
        }
        self.gate.is_open()
    }

    /// The last block's power over the floor, dB.
    pub fn snr_db(&self) -> Option<f32> {
        self.snr_db
    }

    /// The tracked floor (mean power).
    pub fn floor(&self) -> Option<f32> {
        self.floor.floor()
    }

    /// The gate (to change thresholds).
    pub fn gate_mut(&mut self) -> &mut Gate {
        &mut self.gate
    }

    /// Whether open.
    pub fn is_open(&self) -> bool {
        self.gate.is_open()
    }
}

/// The FM noise-quieting squelch: on the discriminator output, the power in the voice band (300 to 3000 Hz) over the power above it
/// (from `noise_hz` to Nyquist). With no carrier the discriminator outputs noise rising with frequency, so the ratio is low; a carrier
/// quiets that noise and the ratio climbs, whatever the signal level.
#[derive(Clone, Debug)]
pub struct QuietingSquelch {
    voice: [Biquad; 2],
    noise: [Biquad; 2],
    n: usize,
    k: usize,
    pv: f64,
    pn: f64,
    gate: Gate,
    ratio_db: Option<f32>,
}

impl QuietingSquelch {
    /// For discriminator output at `rate` (at least 8 kHz), blocks of `block` samples, the noise band from `noise_hz` (above 3 kHz,
    /// below Nyquist), thresholds of `gate` in dB of voice-band over noise-band power.
    pub fn new(rate: f64, block: usize, noise_hz: f64, gate: Gate) -> Result<Self, ModemError> {
        if !(rate >= 8000.0 && rate.is_finite()) {
            return Param::Rate.at_least(8000.0, rate);
        }
        Param::NoiseBand.check_open(noise_hz, 3000.0, 0.45 * rate)?;
        if block == 0 {
            return Param::BlockLen.at_least(1.0, 0.0);
        }
        let q = std::f64::consts::FRAC_1_SQRT_2;
        Ok(QuietingSquelch {
            voice: [
                Biquad::new(BiquadCoeffs::highpass(rate, 300.0, q)),
                Biquad::new(BiquadCoeffs::lowpass(rate, 3000.0, q)),
            ],
            noise: [
                Biquad::new(BiquadCoeffs::highpass(rate, noise_hz, q)),
                Biquad::new(BiquadCoeffs::highpass(rate, noise_hz, q)),
            ],
            n: block,
            k: 0,
            pv: 0.0,
            pn: 0.0,
            gate,
            ratio_db: None,
        })
    }

    /// Takes discriminator output; returns whether open after the last complete block.
    pub fn process(&mut self, x: &[f32]) -> bool {
        for &v in x {
            let v = if v.is_finite() { v } else { 0.0 };
            let a0 = self.voice[0].process_sample(v);
            let a = self.voice[1].process_sample(a0);
            let b0 = self.noise[0].process_sample(v);
            let b = self.noise[1].process_sample(b0);
            self.pv += (a * a) as f64;
            self.pn += (b * b) as f64;
            self.k += 1;
            if self.k == self.n {
                self.ratio_db = if self.pn > 1e-30 {
                    Some((10.0 * (self.pv / self.pn).log10()) as f32)
                } else if self.pv > 1e-30 {
                    Some(100.0)
                } else {
                    None
                };
                self.gate.update(self.ratio_db);
                self.k = 0;
                self.pv = 0.0;
                self.pn = 0.0;
            }
        }
        self.gate.is_open()
    }

    /// The last block's voice-over-noise ratio, dB.
    pub fn ratio_db(&self) -> Option<f32> {
        self.ratio_db
    }

    /// The gate (to change thresholds).
    pub fn gate_mut(&mut self) -> &mut Gate {
        &mut self.gate
    }

    /// Whether open.
    pub fn is_open(&self) -> bool {
        self.gate.is_open()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::floor::MinFollower;
    use sdrkit_algo_dsp::fm::Discriminator;
    use sdrkit_algo_sim::rng::Rng;
    use std::f64::consts::TAU;

    #[test]
    fn gate_attack_hysteresis_hang() {
        let mut g = Gate::new(10.0, 6.0, 2, 3).unwrap();
        assert!(!g.update(Some(12.0)));
        assert!(g.update(Some(12.0)));
        assert!(g.update(Some(7.0))); // between: stays
        for _ in 0..3 {
            assert!(g.update(Some(0.0))); // hang
        }
        assert!(!g.update(Some(0.0)));
        assert!(!g.update(None));
        assert!(Gate::new(1.0, 2.0, 1, 1).is_err());
    }

    #[test]
    fn snr_squelch_follows_the_floor() {
        let mut rng = Rng::new(1);
        let floor = FloorTracker::MinFollower(MinFollower::new(0.5, 0.5).unwrap());
        let mut sq = SnrSquelch::new(256, floor, Gate::new(6.0, 3.0, 2, 4).unwrap()).unwrap();
        sq.set_max_hold(50);
        // Noise at two different levels: after a settling pass, it stays closed.
        for level in [1e-4, 1e-2] {
            let x: Vec<Complex32> = (0..256 * 200).map(|_| rng.complex_normal(level)).collect();
            sq.process(&x);
            let mut opened = false;
            for c in x.chunks(256) {
                opened |= sq.process(c);
            }
            assert!(!opened, "noise at {level}");
        }
        // A carrier 10 dB over the noise opens it; silence afterwards closes it.
        let x: Vec<Complex32> = (0..256 * 20)
            .map(|_| Complex32::new(0.316, 0.0) + rng.complex_normal(1e-2))
            .collect();
        assert!(sq.process(&x));
        assert!(sq.snr_db().unwrap() > 8.0);
        assert!(!sq.process(&vec![Complex32::default(); 256 * 10]));
    }

    #[test]
    fn quieting_squelch_opens_on_a_carrier_at_any_level() {
        let rate = 24_000.0;
        let mut rng = Rng::new(2);
        let mut disc = Discriminator::new(rate, 5_000.0).unwrap();
        let mut run = |sq: &mut QuietingSquelch, amp: f32, n0: f64, tone: bool| -> bool {
            let x: Vec<Complex32> = (0..24_000)
                .map(|k| {
                    let ph = if tone {
                        2.0 * (TAU * 1000.0 * k as f64 / rate).sin()
                    } else {
                        0.0
                    };
                    Complex32::from_polar(amp, ph as f32) + rng.complex_normal(n0)
                })
                .collect();
            let mut a = Vec::new();
            disc.process(&x, &mut a);
            sq.process(&a)
        };
        let mut sq =
            QuietingSquelch::new(rate, 240, 6_000.0, Gate::new(6.0, 3.0, 2, 10).unwrap()).unwrap();
        assert!(
            !run(&mut sq, 0.0, 1e-2, false),
            "noise only: {:?}",
            sq.ratio_db()
        );
        // A modulated carrier 20 dB over the noise, at two levels.
        assert!(run(&mut sq, 1.0, 1e-2, true), "{:?}", sq.ratio_db());
        assert!(run(&mut sq, 0.1, 1e-4, true), "{:?}", sq.ratio_db());
        assert!(!run(&mut sq, 0.0, 1e-4, false));
        assert!(
            QuietingSquelch::new(rate, 240, 2_000.0, Gate::new(6.0, 3.0, 2, 10).unwrap()).is_err()
        );
    }
}
