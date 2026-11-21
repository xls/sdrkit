//! The Airband test signal: AM channels around the dial, each with its level, transmitter offset, depth, audio (voice-like
//! noise with a syllable rhythm, a 1 kHz tone, or carrier only) and keying. The test layouts. The signal is clean: the engine
//! adds the noise and the radio's tuning error.
//! Standard: ICAO Annex 10 Volume III Part II clause 2.2 (double sideband AM voice).

use super::names::STEP_833;
use super::TUNE_OFFSET_HZ;
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use std::f64::consts::TAU;

/// What a channel carries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Audio {
    /// Voice-like.
    Voice,
    /// A 1 kHz tone.
    Tone,
    /// Carrier only.
    Carrier,
}

/// When a channel is keyed.
#[derive(Clone, Debug, PartialEq)]
pub enum Keying {
    /// Always.
    Continuous,
    /// Periodic: period, on time, phase (start of the first), s.
    Periodic(f64, f64, f64),
    /// Bursts: (start, length).
    Bursts(Vec<(f64, f64)>),
}

/// One channel of the scene.
#[derive(Clone, Debug)]
pub struct TxChannel {
    /// Frequency, Hz from the dial.
    pub offset_hz: f64,
    /// Level, dB.
    pub level_db: f64,
    /// Transmitter offset, Hz.
    pub tx_offset_hz: f64,
    /// Peak modulation depth (above 1 clips the envelope at zero).
    pub depth: f64,
    /// Audio.
    pub audio: Audio,
    /// Keying.
    pub keying: Keying,
}

/// The scene.
#[derive(Clone, Debug)]
pub struct GenParams {
    /// Output rate.
    pub rate_hz: f64,
    /// Channels.
    pub channels: Vec<TxChannel>,
    /// The dial is this far below the centre of the samples.
    pub tune_offset_hz: f64,
    /// Seed.
    pub seed: u64,
}

/// The test layout (`modeOpt` 0) or layout 1.
pub fn layout(n: u32) -> Vec<TxChannel> {
    if n == 1 {
        return vec![TxChannel {
            offset_hz: 0.0,
            level_db: 0.0,
            tx_offset_hz: 0.0,
            depth: 0.8,
            audio: Audio::Tone,
            keying: Keying::Periodic(5.0, 2.0, 0.0),
        }];
    }
    vec![
        TxChannel {
            offset_hz: 0.0,
            level_db: 0.0,
            tx_offset_hz: 120.0,
            depth: 0.8,
            audio: Audio::Voice,
            keying: Keying::Periodic(6.0, 2.5, 0.5),
        },
        TxChannel {
            offset_hz: STEP_833,
            level_db: -6.0,
            tx_offset_hz: -250.0,
            depth: 0.8,
            audio: Audio::Voice,
            keying: Keying::Periodic(5.0, 1.8, 2.0),
        },
        TxChannel {
            offset_hz: 250e3,
            level_db: 3.0,
            tx_offset_hz: 60.0,
            depth: 0.8,
            audio: Audio::Voice,
            keying: Keying::Periodic(7.0, 2.0, 4.0),
        },
        TxChannel {
            offset_hz: -300e3,
            level_db: -3.0,
            tx_offset_hz: 0.0,
            depth: 0.6,
            audio: Audio::Voice,
            keying: Keying::Continuous,
        },
        TxChannel {
            offset_hz: 191_666.67,
            level_db: 0.0,
            tx_offset_hz: 300.0,
            depth: 0.8,
            audio: Audio::Tone,
            keying: Keying::Periodic(4.0, 1.0, 1.0),
        },
    ]
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            rate_hz: 1e6,
            channels: layout(0),
            tune_offset_hz: TUNE_OFFSET_HZ,
            seed: 1,
        }
    }
}

/// When a channel is keyed at `t`: the carrier envelope (with 2 ms ramps).
pub fn keyed(k: &Keying, t: f64) -> f64 {
    let ramp = |x: f64| (x / 0.002).clamp(0.0, 1.0);
    match k {
        Keying::Continuous => 1.0,
        Keying::Periodic(p, on, ph) => {
            if t < *ph {
                return 0.0;
            }
            let u = (t - ph) % p;
            if u < *on {
                ramp(u).min(ramp(on - u))
            } else {
                0.0
            }
        }
        Keying::Bursts(v) => v
            .iter()
            .map(|(s, l)| {
                if t >= *s && t < s + l {
                    ramp(t - s).min(ramp(s + l - t))
                } else {
                    0.0
                }
            })
            .fold(0.0, f64::max),
    }
}

struct State {
    phase: f64,
    hp: Biquad,
    lp: Biquad,
    lp2: Biquad,
    rng: Rng,
    last: f64,
    cnt: u32,
}

/// The Airband generator.
pub struct AirbandGenerator {
    p: GenParams,
    n: u64,
    st: Vec<State>,
}

/// The carrier amplitude of a channel at 0 dB.
pub const AMPLITUDE: f64 = 0.05;

impl AirbandGenerator {
    /// A generator for the scene.
    pub fn new(p: GenParams) -> Self {
        let st = (0..p.channels.len())
            .map(|i| State {
                phase: 0.0,
                hp: Biquad::new(BiquadCoeffs::highpass(16e3, 300.0, 0.707)),
                lp: Biquad::new(BiquadCoeffs::lowpass(16e3, 2800.0, 0.707)),
                lp2: Biquad::new(BiquadCoeffs::lowpass(16e3, 2800.0, 0.707)),
                rng: Rng::new(p.seed * 31 + i as u64),
                last: 0.0,
                cnt: 0,
            })
            .collect();
        AirbandGenerator { n: 0, st, p }
    }

    /// A generator from the stack's `gen_*` settings and the rate.
    pub fn from_config(cfg: &GenConfig) -> Self {
        AirbandGenerator::new(GenParams {
            rate_hz: cfg.rate_hz,
            seed: cfg.seed,
            ..Default::default()
        })
    }
}

impl Generator for AirbandGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.rate_hz
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of a channel at 0 dB is its carrier-to-noise in 6.8 kHz.
        let mut s = SignalInfo::new(6.8e3);
        s.power = AMPLITUDE * AMPLITUDE;
        Some(s)
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let fs = self.p.rate_hz;
        out.fill(Complex32::default());
        let per16 = (fs / 16e3).max(1.0) as u32;
        for (c, s) in self.p.channels.iter().zip(self.st.iter_mut()) {
            let f = c.offset_hz - self.p.tune_offset_hz + c.tx_offset_hz;
            let amp = AMPLITUDE * 10f64.powf(c.level_db / 20.0);
            for (i, v) in out.iter_mut().enumerate() {
                let t = (self.n + i as u64) as f64 / fs;
                let key = keyed(&c.keying, t);
                s.phase = (s.phase + TAU * f / fs) % TAU;
                if key <= 0.0 {
                    continue;
                }
                // The audio at 16 kHz, held between its samples.
                if s.cnt == 0 {
                    s.last = match c.audio {
                        Audio::Tone => (TAU * 1000.0 * t).sin(),
                        Audio::Carrier => 0.0,
                        Audio::Voice => {
                            let w = s.rng.normal_f32();
                            let y = s
                                .lp2
                                .process_sample(s.lp.process_sample(s.hp.process_sample(w)))
                                as f64;
                            (y * 2.2 * (0.35 + 0.65 * (TAU * 2.7 * t).sin().abs())).clamp(-1.0, 1.0)
                        }
                    };
                }
                s.cnt = (s.cnt + 1) % per16;
                let env = (amp * key * (1.0 + c.depth * s.last)).max(0.0);
                *v += Complex32::new((env * s.phase.cos()) as f32, (env * s.phase.sin()) as f32);
            }
        }
        self.n += out.len() as u64;
        out.len()
    }
}
