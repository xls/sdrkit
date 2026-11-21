//! The FM video discriminator: the phase step of consecutive samples, a low-pass filter that passes the video band, and
//! decimation to about 10 Msps. It also outputs the analytic signal (a 31-tap Hilbert transformer) that the colour decoder
//! needs.

use sdrkit_algo_dsp::decim::{FirDecimator, HalfBand};
use sdrkit_core::{Complex32, ConfigError};
use sdrkit_graph::{Component, ComponentInfo, Format, Io, Port};
use std::f64::consts::{PI, TAU};

/// The discriminator.
pub struct FmVideoDisc {
    fs: f64,
    disc: sdrkit_algo_dsp::fm::Discriminator,
    dec: FirDecimator<f32>,
    /// Decimation by 2: a half-band, which meets the same pass and stop edges with half the products.
    half: Option<HalfBand<f32>>,
    hilbert: [f32; 31],
    /// The last 30 video samples, then the new ones.
    line: Vec<f32>,
    /// Video samples seen, up to 30.
    seen: usize,
    q: Vec<f32>,
    raw: Vec<f32>,
    vid: Vec<f32>,
    scale: f32,
}

impl Default for FmVideoDisc {
    fn default() -> Self {
        let mut h = [0f32; 31];
        for (i, t) in h.iter_mut().enumerate() {
            let k = i as i64 - 15;
            if k % 2 != 0 {
                let w = 0.54 - 0.46 * (TAU * i as f64 / 30.0).cos();
                *t = (2.0 / (PI * k as f64) * w) as f32;
            }
        }
        FmVideoDisc {
            fs: 1.0,
            disc: crate::util::phase_steps(),
            dec: FirDecimator::new(&[1.0], 1),
            half: None,
            hilbert: h,
            line: vec![0.0; 30],
            seen: 0,
            q: Vec::new(),
            raw: Vec::new(),
            vid: Vec::new(),
            scale: 1.0,
        }
    }
}

/// The decimation of the FM video discriminator.
pub fn decimation(fs: f64) -> usize {
    ((fs / 10e6).round() as usize).max(1)
}

impl Component for FmVideoDisc {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("atv.fm_video", vec![Port::iq("in")], vec![Port::iq("out")])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        let d = decimation(self.fs);
        let fv = self.fs / d as f64;
        let pass = (4.6e6f64).min(0.46 * fv);
        let stop = (5.4e6f64).min(0.54 * fv);
        self.half = if d == 2 && pass < 0.24 * self.fs {
            Some(HalfBand::design(self.fs, pass, 45.0)?)
        } else {
            None
        };
        let taps = sdrkit_algo_dsp::fir::lowpass(self.fs, pass, stop.min(self.fs / 2.0), 45.0)?;
        self.dec = FirDecimator::new(&taps, d);
        self.scale = -(self.fs / TAU / 16e6) as f32;
        self.disc.reset();
        self.line.clear();
        self.line.resize(30, 0.0);
        self.seen = 0;
        Ok(vec![inputs[0].with_rate(fv)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.raw.clear();
        self.disc.process(x, &mut self.raw);
        for v in &mut self.raw {
            *v *= self.scale;
        }
        self.vid.clear();
        match &mut self.half {
            Some(h) => h.process(&self.raw, &mut self.vid),
            None => self.dec.process(&self.raw, &mut self.vid),
        }
        // The Hilbert transformer over the last 31 samples, a tap at a time across all outputs: each output adds its
        // non-zero terms in the same order as a sample-by-sample sum.
        let n = self.vid.len();
        self.line.extend_from_slice(&self.vid);
        self.q.clear();
        self.q.resize(n, 0.0);
        for j in (0..31).step_by(2) {
            let h = self.hilbert[30 - j];
            for (q, &x) in self.q.iter_mut().zip(&self.line[j..j + n]) {
                *q += h * x;
            }
        }
        let out = io.iq_out(0);
        for (o, &q) in self.q.iter().enumerate() {
            if self.seen < 30 {
                self.seen += 1;
                out.push(Complex32::default());
            } else {
                out.push(Complex32::new(self.line[o + 15], q));
            }
        }
        self.line.drain(..n);
    }
}
