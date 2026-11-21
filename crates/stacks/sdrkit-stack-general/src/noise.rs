//! The noise-floor estimator: the noise beside the channel (for the carrier squelch and the SNR) and inside the passband (for USB
//! and LSB), from medians of averaged spectra.

use crate::types::{NoiseEstimate, RestartCommand};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::floor::BinMedian;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::publish::field;
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, Record, Setting,
    SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed,
};

const N: usize = 512;
const FRAMES: usize = 8;

/// The estimator as a plain object: feed the decimated channel, read estimates.
pub struct NoiseFloor {
    rate: f64,
    lo: f64,
    hi: f64,
    fft: Fft,
    win: Vec<f32>,
    win_energy: f32,
    frame: Vec<Complex32>,
    fill: usize,
    acc: Vec<f32>,
    frames: usize,
    beside: Option<f32>,
    inside: Option<f32>,
    sel: Vec<f32>,
    median: BinMedian,
}

impl NoiseFloor {
    /// For a channel at `rate` with filter edges `lo..hi` (Hz relative to the channel at 0 Hz).
    pub fn new(rate: f64, lo: f64, hi: f64) -> Self {
        let win = Window::Hann.coefficients(N);
        // The median of averages of 8 exponential bins, divided by its median-to-mean ratio (about 0.959).
        let mut median = BinMedian::new(FRAMES as u32).expect("8 frames is a valid count");
        median.reserve(N);
        let win_energy = win.iter().map(|w| w * w).sum();
        NoiseFloor {
            rate,
            lo,
            hi,
            fft: Fft::forward(N),
            win,
            win_energy,
            frame: vec![Complex32::default(); N],
            fill: 0,
            acc: vec![0.0; N],
            frames: 0,
            beside: None,
            inside: None,
            sel: Vec::with_capacity(N),
            median,
        }
    }
    /// New edges: the estimate restarts.
    pub fn set_edges(&mut self, lo: f64, hi: f64) {
        self.lo = lo;
        self.hi = hi;
        self.restart();
    }
    /// Starts again (a channel move, a reset).
    pub fn restart(&mut self) {
        self.fill = 0;
        self.frames = 0;
        self.acc.iter_mut().for_each(|v| *v = 0.0);
        self.beside = None;
        self.inside = None;
    }
    fn bin_hz(&self, k: usize) -> f64 {
        let b = self.rate / N as f64;
        if k < N / 2 {
            k as f64 * b
        } else {
            (k as f64 - N as f64) * b
        }
    }
    fn evaluate(&mut self) -> NoiseEstimate {
        let b = self.rate / N as f64;
        let width = self.hi - self.lo;
        let guard = (2.0 * b).max(0.15 * width);
        let norm = 1.0 / (FRAMES as f32 * self.win_energy * self.rate as f32);
        let edge = 0.4 * self.rate;
        // Beside the channel.
        self.sel.clear();
        for k in 0..N {
            let f = self.bin_hz(k);
            if f.abs() <= edge && (f < self.lo - guard || f > self.hi + guard) {
                self.sel.push(self.acc[k]);
            }
        }
        let impossible = self.sel.len() < 16;
        let beside_now = if impossible {
            None
        } else {
            self.median
                .estimate(self.sel.iter().copied())
                .map(|m| m * norm * width as f32)
        };
        // Inside the passband.
        self.sel.clear();
        for k in 0..N {
            let f = self.bin_hz(k);
            if f.abs() <= edge && f > self.lo && f < self.hi {
                self.sel.push(self.acc[k]);
            }
        }
        let inside_now = if self.sel.len() >= 6 {
            self.median
                .estimate(self.sel.iter().copied())
                .map(|m| m * norm * width as f32)
        } else {
            None
        };
        let smooth = |old: Option<f32>, new: Option<f32>| match (old, new) {
            (Some(o), Some(n)) => Some(0.7 * o + 0.3 * n),
            (_, n) => n,
        };
        self.beside = smooth(self.beside, beside_now);
        self.inside = smooth(self.inside, inside_now);
        NoiseEstimate {
            beside: self.beside,
            inside: self.inside,
            impossible,
        }
    }
    /// Feeds samples. `emit(offset, estimate)` is called with the position (samples into `x`, after the frame's last sample) of each
    /// new estimate.
    pub fn process(&mut self, x: &[Complex32], mut emit: impl FnMut(usize, NoiseEstimate)) {
        let mut i = 0;
        while i < x.len() {
            let take = (N - self.fill).min(x.len() - i);
            for (j, &v) in x[i..i + take].iter().enumerate() {
                self.frame[self.fill + j] = v * self.win[self.fill + j];
            }
            self.fill += take;
            i += take;
            if self.fill == N {
                self.fill = 0;
                self.fft.process(&mut self.frame);
                for (a, v) in self.acc.iter_mut().zip(&self.frame) {
                    *a += v.norm_sqr();
                }
                self.frames += 1;
                if self.frames == FRAMES {
                    let e = self.evaluate();
                    self.frames = 0;
                    self.acc.iter_mut().for_each(|v| *v = 0.0);
                    emit(i, e);
                }
            }
        }
    }
}

const LO: Setting<field::Hz> =
    Setting::new("lo", "Low edge", "Hz", -6250.0).range(-150e3, 150e3, 0.0);
const HI: Setting<field::Hz> =
    Setting::new("hi", "High edge", "Hz", 6250.0).range(-150e3, 150e3, 0.0);

sdrkit_core::settings! {
    /// The settings of [`NoiseComponent`]: the edges of the channel filter. A change restarts the estimate.
    pub enum NoiseSetting {
        /// The low edge relative to the channel (Hz).
        Lo = LO,
        /// The high edge relative to the channel (Hz).
        Hi = HI,
    }
}

/// The noise-floor estimator component (`general.noise`): `Iq` at the decimated rate in, [`NoiseEstimate`]s out. Its command
/// `restart` starts the estimate again.
pub struct NoiseComponent {
    lo: f64,
    hi: f64,
    nf: Option<NoiseFloor>,
    rate: f64,
    /// Samples since the last configure: times count from there, so they stay aligned with the squelch across a change of rate.
    n: u64,
}

impl Default for NoiseComponent {
    fn default() -> Self {
        NoiseComponent {
            lo: -6250.0,
            hi: 6250.0,
            nf: None,
            rate: 0.0,
            n: 0,
        }
    }
}

impl Component for NoiseComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.noise",
            vec![Port::iq("in")],
            vec![Port::typed::<NoiseEstimate>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(NoiseSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.nf = Some(NoiseFloor::new(self.rate, self.lo, self.hi));
        self.n = 0;
        Ok(vec![Format::new(Kind::typed::<NoiseEstimate>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let first = self.n;
        let x = io.iq(0);
        self.n += x.len() as u64;
        let rate = self.rate;
        let Some(nf) = self.nf.as_mut() else { return };
        let out = io.items_out::<NoiseEstimate>(0);
        nf.process(x, |at, e| {
            out.push(Timed {
                time: (first + at as u64) as f64 / rate,
                value: e,
            })
        });
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match NoiseSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            NoiseSetting::Lo => self.lo = LO.read(id, v)?,
            NoiseSetting::Hi => self.hi = HI.read(id, v)?,
        }
        if let Some(nf) = &mut self.nf {
            nf.set_edges(self.lo, self.hi);
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (NoiseSetting::Lo, Value::Hz(self.lo)),
            (NoiseSetting::Hi, Value::Hz(self.hi)),
        ])
    }
    fn commands(&self) -> &Commands {
        RestartCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match RestartCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RestartCommand::Restart => {
                if let Some(nf) = &mut self.nf {
                    nf.restart();
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_noise_reads_its_power_in_the_channel() {
        let mut rng = 12345u64;
        let mut g = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng >> 11) as f64 / (1u64 << 53) as f64
        };
        let sigma = 0.01f64;
        let x: Vec<Complex32> = (0..125_000)
            .map(|_| {
                let (u1, u2) = (g().max(1e-300), g());
                let r = (-2.0 * u1.ln()).sqrt() * sigma;
                let a = std::f64::consts::TAU * u2;
                Complex32::new((r * a.cos()) as f32, (r * a.sin()) as f32)
            })
            .collect();
        let mut nf = NoiseFloor::new(125e3, -6250.0, 6250.0);
        let mut last = None;
        nf.process(&x, |_, e| last = Some(e));
        let e = last.unwrap();
        let want = 2.0 * sigma * sigma * 12_500.0 / 125e3;
        let got = e.beside.unwrap() as f64;
        assert!((10.0 * (got / want).log10()).abs() < 0.5, "{got} vs {want}");
        assert!(!e.impossible);
    }
}
