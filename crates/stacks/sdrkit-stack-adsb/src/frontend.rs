//! The front end of the 1090 MHz chain: a half-sample interpolator that doubles rates below 4.5 Msps, and a DC removal that moves
//! only on blocks without pulses. The interpolator states the delay it adds as a [`ChainDelay`] item.

use sdrkit_core::{Complex32, ConfigError, Values};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};

/// The delay that the chain before a demodulator adds to the signal. A demodulator subtracts it from the times it reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChainDelay {
    /// The delay, seconds.
    pub delay_s: f64,
}

impl Item for ChainDelay {
    const KIND: &'static str = "adsb.chain_delay";
}

/// The 12 taps of the half-sample filter: a sinc at offsets +-0.5 .. +-5.5 times a Blackman window, scaled to sum to 1.
pub fn half_sample_taps() -> [f32; 12] {
    let mut t = [0f64; 12];
    for (k, v) in t.iter_mut().enumerate() {
        let u = k as f64 - 5.5;
        let x = std::f64::consts::PI * u;
        let sinc = x.sin() / x;
        let w = 0.42
            + 0.5 * (std::f64::consts::PI * u / 6.0).cos()
            + 0.08 * (2.0 * std::f64::consts::PI * u / 6.0).cos();
        *v = sinc * w;
    }
    let s: f64 = t.iter().sum();
    let mut o = [0f32; 12];
    for (o, v) in o.iter_mut().zip(t) {
        *o = (v / s) as f32;
    }
    o
}

/// Doubles the rate below a threshold (keeping every sample and adding one halfway between each pair); delays by 6 input samples.
#[derive(Clone, Debug)]
pub struct HalfSampleInterpolator {
    threshold_hz: f64,
    rate: f64,
    active: bool,
    taps: [f32; 12],
    hist: [Complex32; 12],
    delay_due: bool,
}

impl Default for HalfSampleInterpolator {
    fn default() -> Self {
        Self::new(4.5e6)
    }
}

impl HalfSampleInterpolator {
    /// Interpolates input below `threshold_hz`.
    pub fn new(threshold_hz: f64) -> Self {
        HalfSampleInterpolator {
            threshold_hz,
            rate: 0.0,
            active: false,
            taps: half_sample_taps(),
            hist: [Complex32::new(0.0, 0.0); 12],
            delay_due: false,
        }
    }
    /// Whether it doubles the rate (known after configure).
    pub fn active(&self) -> bool {
        self.active
    }
    /// The delay that it adds, seconds: 6 input samples when it doubles the rate, else 0.
    pub fn delay_s(&self) -> f64 {
        if self.active {
            6.0 / self.rate
        } else {
            0.0
        }
    }
    /// Interpolates `input`, appending twice as many samples to `out`.
    pub fn run(&mut self, input: &[Complex32], out: &mut Vec<Complex32>) {
        out.reserve(input.len() * 2);
        for &x in input {
            self.hist.copy_within(1..12, 0);
            self.hist[11] = x;
            out.push(self.hist[5]);
            let mut acc = Complex32::new(0.0, 0.0);
            for (h, t) in self.hist.iter().zip(&self.taps) {
                acc += h * *t;
            }
            out.push(acc);
        }
    }
}

impl Component for HalfSampleInterpolator {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "adsb.half_sample_interp",
            vec![Port::iq("in")],
            vec![
                Port::iq("out").rate_from(RateFrom::Configure),
                Port::typed::<ChainDelay>("delay").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let active = f.rate < self.threshold_hz;
        if active != self.active || f.rate != self.rate {
            self.hist = [Complex32::new(0.0, 0.0); 12];
        }
        self.rate = f.rate;
        self.active = active;
        self.delay_due = true;
        let rate = if active { f.rate * 2.0 } else { f.rate };
        Ok(vec![
            f.with_rate(rate),
            Format::new(Kind::typed::<ChainDelay>(), 0.0),
        ])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn process(&mut self, io: &mut Io) {
        if std::mem::take(&mut self.delay_due) {
            let delay_s = self.delay_s();
            io.items_out::<ChainDelay>(1).push(Timed {
                time: 0.0,
                value: ChainDelay { delay_s },
            });
        }
        let input = io.iq(0);
        if self.active {
            let mut out = std::mem::take(io.iq_out(0));
            self.run(input, &mut out);
            *io.iq_out(0) = out;
        } else {
            io.iq_out(0).extend_from_slice(input);
        }
    }
}

/// DC removal for pulse signals: blocks of 64 samples, the estimate moved only on quiet blocks.
#[derive(Clone, Debug)]
pub struct PulseDcRemoval {
    rate: f64,
    floor_cache: Option<f32>,
    floor_age: u32,
    beta: f32,
    dc: Option<Complex32>,
    block: Vec<Complex32>,
    powers: Vec<f32>,
    pos: usize,
}

const DC_BLOCK: usize = 64;
const DC_HISTORY: usize = 64;

impl Default for PulseDcRemoval {
    fn default() -> Self {
        PulseDcRemoval {
            rate: 0.0,
            floor_cache: None,
            floor_age: 0,
            beta: 0.5,
            dc: None,
            block: Vec::with_capacity(DC_BLOCK),
            powers: Vec::with_capacity(DC_HISTORY),
            pos: 0,
        }
    }
}

impl PulseDcRemoval {
    /// For a working rate.
    pub fn new(rate: f64) -> Self {
        let mut d = PulseDcRemoval::default();
        d.set_rate(rate);
        d
    }
    fn set_rate(&mut self, rate: f64) {
        self.beta = (DC_BLOCK as f64 / (rate * 1e-3)).min(0.5) as f32;
        self.dc = None;
        self.block.clear();
        self.powers.clear();
        self.pos = 0;
        self.floor_cache = None;
        self.floor_age = 0;
    }
    /// The current estimate.
    pub fn dc(&self) -> Complex32 {
        self.dc.unwrap_or_default()
    }
    fn floor(&self) -> Option<f32> {
        if self.powers.len() < 16 {
            return None;
        }
        let mut v: [f32; DC_HISTORY] = [0.0; DC_HISTORY];
        let n = self.powers.len();
        v[..n].copy_from_slice(&self.powers);
        let s = &mut v[..n];
        s.sort_unstable_by(|a, b| a.total_cmp(b));
        Some(s[n / 4] / 0.93)
    }
    fn finish_block(&mut self, out: &mut Vec<Complex32>) {
        let n = self.block.len();
        if n == 0 {
            return;
        }
        let mut mean = Complex32::new(0.0, 0.0);
        for s in &self.block {
            mean += s;
        }
        mean /= n as f32;
        match self.dc {
            None => self.dc = Some(mean),
            Some(dc) => {
                let mut pw = 0f32;
                for s in &self.block {
                    pw += (s - dc).norm_sqr();
                }
                pw /= n as f32;
                if self.floor_age == 0 {
                    self.floor_cache = self.floor();
                }
                self.floor_age = (self.floor_age + 1) % 16;
                let quiet = self.floor_cache.is_none_or(|f| pw < 4.0 * f);
                if quiet {
                    let b = self.beta * n as f32 / DC_BLOCK as f32;
                    self.dc = Some(dc + (mean - dc) * b);
                }
            }
        }
        let dc = self.dc.unwrap_or_default();
        let mut pw = 0f32;
        for s in &self.block {
            let v = s - dc;
            pw += v.norm_sqr();
            out.push(v);
        }
        pw /= n as f32;
        if self.powers.len() < DC_HISTORY {
            self.powers.push(pw);
        } else {
            self.powers[self.pos] = pw;
            self.pos = (self.pos + 1) % DC_HISTORY;
        }
        self.block.clear();
    }
    /// Removes DC from `input`, appending to `out` (the output lags the input by up to one block).
    pub fn run(&mut self, input: &[Complex32], out: &mut Vec<Complex32>) {
        for &x in input {
            self.block.push(x);
            if self.block.len() == DC_BLOCK {
                self.finish_block(out);
            }
        }
    }
}

impl Component for PulseDcRemoval {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "adsb.pulse_dc_removal",
            vec![Port::iq("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // At the same rate the stream goes on: the block being gathered is kept, so no sample is lost and times stay right.
        if inputs[0].rate != self.rate {
            self.rate = inputs[0].rate;
            self.set_rate(inputs[0].rate);
        }
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.iq(0);
        let mut out = std::mem::take(io.iq_out(0));
        self.run(input, &mut out);
        *io.iq_out(0) = out;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taps_sum_to_one_and_keep_samples() {
        let t = half_sample_taps();
        assert!((t.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        let mut h = HalfSampleInterpolator::new(4.5e6);
        let x: Vec<Complex32> = (0..100)
            .map(|i| Complex32::new((i as f32 * 0.1).sin(), 0.0))
            .collect();
        let mut out = Vec::new();
        h.run(&x, &mut out);
        assert_eq!(out.len(), 200);
        for i in 6..100 {
            assert_eq!(out[2 * i], x[i - 6]);
            let mid = (0.1 * (i as f32 - 5.5)).sin();
            assert!((out[2 * i + 1].re - mid).abs() < 0.01);
        }
    }

    #[test]
    fn dc_removal_follows_a_constant_offset() {
        let mut d = PulseDcRemoval::new(4e6);
        let x = vec![Complex32::new(0.1, -0.05); 64 * 100];
        let mut out = Vec::new();
        d.run(&x, &mut out);
        assert_eq!(out.len(), x.len());
        assert!(out.last().unwrap().norm() < 1e-3);
    }
}
