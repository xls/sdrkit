//! Measuring the settle time of a radio after a retune, from the power of short blocks of IQ samples.

use sdrkit_algo_dsp::Complex32;

/// What the meter requests next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettleStep {
    /// Retune here and keep feeding samples.
    Retune(f64),
    /// The settle time is measured, s.
    Done(f64),
}

/// Measures the settle time.
#[derive(Clone, Debug)]
pub struct SettleMeter {
    ends: [f64; 2],
    rate_hz: f64,
    block: usize,
    blocks: usize,
    trials: usize,
    tolerance_db: f32,
    trial: usize,
    acc: f64,
    acc_n: usize,
    powers: Vec<f32>,
    results: Vec<f64>,
    scratch: Vec<f32>,
}

impl SettleMeter {
    /// A meter retuning between `lo_hz` and `hi_hz` at `rate_hz`, `trials` times, watching `window_s` after each retune.
    pub fn new(lo_hz: f64, hi_hz: f64, rate_hz: f64, trials: usize, window_s: f64) -> Self {
        let block = ((rate_hz * 1e-4).round() as usize).clamp(256, 4096);
        let blocks = ((window_s * rate_hz) as usize / block).max(8);
        SettleMeter {
            ends: [lo_hz, hi_hz],
            rate_hz,
            block,
            blocks,
            trials: trials.max(1),
            tolerance_db: 1.0,
            trial: 0,
            acc: 0.0,
            acc_n: 0,
            powers: Vec::with_capacity(blocks),
            results: Vec::with_capacity(trials.max(1)),
            scratch: Vec::with_capacity(blocks),
        }
    }

    /// Where to tune first.
    pub fn first(&self) -> f64 {
        self.ends[0]
    }

    #[cfg(test)]
    /// The settle time of each trial so far, s.
    pub fn trials(&self) -> &[f64] {
        &self.results
    }

    /// Feeds samples received after the last retune. Returns the next step once the window is full.
    pub fn push(&mut self, iq: &[Complex32]) -> Option<SettleStep> {
        for x in iq {
            if self.powers.len() >= self.blocks {
                break;
            }
            self.acc += x.norm_sqr() as f64;
            self.acc_n += 1;
            if self.acc_n == self.block {
                let p = (self.acc / self.block as f64).max(1e-20);
                self.powers.push(10.0 * p.log10() as f32);
                self.acc = 0.0;
                self.acc_n = 0;
            }
        }
        if self.powers.len() < self.blocks {
            return None;
        }
        let q = self.blocks * 3 / 4;
        let fin = crate::median(&self.powers[q..], &mut self.scratch);
        // The band is wide enough for the block-to-block spread of the final level (5 sigma from its median absolute deviation).
        let dev: Vec<f32> = self.powers[q..].iter().map(|p| (p - fin).abs()).collect();
        let sigma = 1.4826 * crate::median(&dev, &mut self.scratch);
        let tol = self.tolerance_db.max(5.0 * sigma);
        let last_out = self
            .powers
            .iter()
            .rposition(|&p| (p - fin).abs() > tol)
            .map_or(0, |i| i + 1);
        self.results
            .push(last_out as f64 * self.block as f64 / self.rate_hz);
        self.powers.clear();
        self.acc = 0.0;
        self.acc_n = 0;
        self.trial += 1;
        if self.trial >= self.trials * 2 {
            let worst = self.results.iter().copied().fold(0.0, f64::max);
            let window = self.blocks as f64 * self.block as f64 / self.rate_hz;
            return Some(SettleStep::Done((worst * 1.2).clamp(0.0005, window / 2.0)));
        }
        Some(SettleStep::Retune(self.ends[self.trial % 2]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{FakeRadio, FakeSignal};

    #[test]
    fn measures_the_fake_radios_settle_time() {
        let rate = 2.4e6;
        let mut radio = FakeRadio::new(rate, 100e6, 5);
        radio.settle_s = 0.005;
        radio.add(FakeSignal::carrier(100.2e6, -30.0));
        let mut m = SettleMeter::new(100e6, 500e6, rate, 3, 0.05);
        radio.tune(m.first(), rate);
        let mut buf = vec![Complex32::default(); 4096];
        let mut retunes = 0;
        let settle = loop {
            radio.read(&mut buf);
            match m.push(&buf) {
                None => {}
                Some(SettleStep::Retune(hz)) => {
                    retunes += 1;
                    radio.tune(hz, rate);
                }
                Some(SettleStep::Done(s)) => break s,
            }
        };
        assert_eq!(retunes, 5);
        assert!(settle > 0.004 && settle < 0.0075, "{settle}");
        assert_eq!(m.trials().len(), 6);
    }

    #[test]
    fn a_radio_that_settles_at_once() {
        let rate = 1e6;
        let mut radio = FakeRadio::new(rate, 100e6, 6);
        let mut m = SettleMeter::new(100e6, 200e6, rate, 1, 0.02);
        let mut buf = vec![Complex32::default(); 1000];
        let s = loop {
            radio.read(&mut buf);
            if let Some(SettleStep::Done(s)) = m.push(&buf) {
                break s;
            }
        };
        assert!(s <= 0.001, "{s}");
    }
}
