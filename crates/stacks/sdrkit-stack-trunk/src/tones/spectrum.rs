//! `tones.spectrum`: the audio spectrum (112 points from 0 to 3.5 kHz) and the strongest pure tone, every 32 ms over the last
//! 64 ms of the 8 kHz audio.

use crate::typed_port;
use num_complex::Complex32;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::{ConfigError, Values};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Item, Kind, Port, Timed};

/// The points of the spectrum.
pub const POINTS: usize = 112;
/// The highest frequency of the spectrum, Hz.
pub const TOP_HZ: f32 = 3500.0;
const N: usize = 512;
const HOP: u64 = 256;
const RATE: f64 = 8000.0;
/// The mean square below which no strongest tone is named (20 Hz of deviation in units of 5 kHz, squared).
const QUIET: f32 = (20.0 / 5000.0) * (20.0 / 5000.0);

/// What `tones.spectrum` reports every 32 ms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spectrum {
    /// dB relative to a full-scale sine, floored at -100 dB and smoothed: point `i` is at `i × 3500 / 112` Hz.
    pub db: [f32; POINTS],
    /// The strongest pure tone (Hz and dB), when one stands out.
    pub strongest: Option<(f64, f32)>,
}

impl Item for Spectrum {
    const KIND: &'static str = "tones.spectrum";
}

/// `tones.spectrum` (module documentation).
pub struct SpectrumComponent {
    ring: SampleRing<f32>,
    win: Vec<f32>,
    fft: Fft,
    z: Vec<Complex32>,
    pw: Vec<f64>,
    db: [f32; POINTS],
    started: bool,
    done: Vec<Timed<Spectrum>>,
}

impl Default for SpectrumComponent {
    fn default() -> Self {
        let mut win = vec![0.0; N];
        Window::Hann.fill_symmetric(&mut win);
        SpectrumComponent {
            ring: SampleRing::new(N).expect("a ring of 512 samples"),
            win,
            fft: Fft::forward(N),
            z: vec![Complex32::new(0.0, 0.0); N],
            pw: vec![0.0; N / 2 + 1],
            db: [-100.0; POINTS],
            started: false,
            done: Vec::with_capacity(8),
        }
    }
}

impl SpectrumComponent {
    /// The analysis of the last 512 samples.
    fn analyse(&mut self) -> Spectrum {
        let start = self.ring.end() - N as u64;
        let mut ms = 0.0f32;
        for k in 0..N {
            let v = self.ring.get(start + k as u64).unwrap_or(0.0);
            ms += v * v;
            self.z[k] = Complex32::new(v * self.win[k], 0.0);
        }
        ms /= N as f32;
        self.fft.process(&mut self.z);
        // A full-scale sine through the Hann window peaks at N / 4.
        let refp = (N as f64 / 4.0).powi(2);
        for (p, z) in self.pw.iter_mut().zip(&self.z) {
            *p = z.norm_sqr() as f64;
        }
        for i in 0..POINTS {
            let p = self.pw[2 * i].max(self.pw[2 * i + 1]);
            let db = (10.0 * (p / refp).max(1e-10).log10()).max(-100.0) as f32;
            self.db[i] = if self.started {
                0.5 * self.db[i] + 0.5 * db
            } else {
                db
            };
        }
        self.started = true;
        Spectrum {
            db: self.db,
            strongest: (ms >= QUIET).then(|| self.strongest(refp)).flatten(),
        }
    }
    /// The peak bin from 312 to 3500 Hz when its three bins hold at least 60 % of the power above bin 8, with its frequency
    /// from a parabola through the log powers.
    fn strongest(&self, refp: f64) -> Option<(f64, f32)> {
        let pw = &self.pw;
        let above: f64 = pw[9..N / 2].iter().sum();
        let k = (20..=224).max_by(|&a, &b| pw[a].total_cmp(&pw[b]))?;
        let three = pw[k - 1] + pw[k] + pw[k + 1];
        if above <= 0.0 || three < 0.6 * above {
            return None;
        }
        let peak = log_parabolic(pw[k - 1], pw[k], pw[k + 1]);
        let f = (k as f64 + peak.offset) * RATE / N as f64;
        Some((f, (10.0 * (pw[k] / refp).max(1e-10).log10()) as f32))
    }
}

impl Component for SpectrumComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "tones.spectrum",
            vec![Port::real("in").rate(sdrkit_graph::RateRule::Exact(RATE))],
            vec![typed_port::<Spectrum>("out")],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = SpectrumComponent::default();
        Ok(vec![Format::new(Kind::typed::<Spectrum>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let x = io.real(0);
        for (k, &v) in x.iter().enumerate() {
            self.ring.push(v);
            let n = self.ring.end();
            if n >= N as u64 && n.is_multiple_of(HOP) {
                let value = self.analyse();
                self.done.push(Timed {
                    time: (first + k as u64 + 1) as f64 / RATE,
                    value,
                });
            }
        }
        io.items_out::<Spectrum>(0).append(&mut self.done);
    }
}
