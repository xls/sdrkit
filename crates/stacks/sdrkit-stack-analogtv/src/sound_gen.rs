//! The sound side of the generators: the programme (tone with gaps, melody, continuous tone), and the second sound (the A2
//! carrier with its pilot, and NICAM-728 symbols with root-raised-cosine pulses).

use crate::nicam::{build_frame, Mode, FRAME_BITS, SYMBOL_RATE};
use sdrkit_algo_dsp::iir::PreEmphasis;
use sdrkit_algo_dsp::pulse::{PulseTable, Shape};
use sdrkit_core::Complex32;
use std::collections::VecDeque;
use std::f64::consts::TAU;

sdrkit_core::choice_enum! {
    /// The sound programme of the test signal.
    pub enum Programme {
        /// 1 kHz with a 0.2 s gap every 1.5 s.
        ToneGaps = ("gaps", "1 kHz tone with gaps"),
        /// The 16-note melody.
        Melody = ("melody", "Melody"),
        /// An unmodulated carrier.
        Unmodulated = ("carrier", "Unmodulated carrier"),
        /// No sound carrier at all.
        NoCarrier = ("none", "No sound carrier"),
        /// A continuous 1 kHz tone.
        Continuous = ("tone", "Continuous 1 kHz"),
    }
}

impl Programme {
    /// The programme at time `t` (full scale 1; tones at 0.5).
    pub fn value(self, t: f64) -> f64 {
        match self {
            Programme::ToneGaps => {
                let ph = t % 1.5;
                if ph >= 1.3 {
                    0.0
                } else {
                    0.5 * (TAU * 1000.0 * t).sin()
                }
            }
            Programme::Continuous => 0.5 * (TAU * 1000.0 * t).sin(),
            Programme::Melody => {
                const NOTES: [f64; 16] = [
                    261.63, 261.63, 392.00, 392.00, 440.00, 440.00, 392.00, 0.0, 349.23, 349.23,
                    329.63, 329.63, 293.66, 293.66, 261.63, 0.0,
                ];
                let d = 0.35;
                let k = ((t / d).floor() as i64).rem_euclid(16) as usize;
                let f = NOTES[k];
                if f == 0.0 {
                    return 0.0;
                }
                let tn = t - (t / d).floor() * d;
                let fade = d / 20.0;
                let env = (tn / fade).min(1.0).min((d - tn) / fade).max(0.0);
                0.5 * env * (TAU * f * t).sin()
            }
            Programme::Unmodulated | Programme::NoCarrier => 0.0,
        }
    }
}

/// The interval of the programme in output samples: a generator reads it once every so many samples and interpolates
/// linearly between the readings.
pub const PROGRAMME_STEP: u64 = 32;

/// How many programme readings a generator at output sample `k` makes in its next `n` samples (one at each multiple of
/// [`PROGRAMME_STEP`]).
pub fn programme_due(k: u64, n: usize) -> usize {
    let s = PROGRAMME_STEP;
    ((k + n as u64).div_ceil(s) - k.div_ceil(s)) as usize
}

/// Host audio waiting for a generator: mono samples at the programme rate (the output rate over [`PROGRAMME_STEP`]), read
/// one at each reading of the programme. Silence where none wait.
#[derive(Debug, Default)]
pub struct HostAudio {
    waiting: VecDeque<f32>,
}

impl HostAudio {
    /// Adds samples.
    pub fn push(&mut self, x: &[f32]) {
        self.waiting.extend(x.iter().copied());
    }

    /// The next sample, limited to full scale; 0 where none waits.
    pub fn take(&mut self) -> f64 {
        f64::from(self.waiting.pop_front().unwrap_or(0.0)).clamp(-1.0, 1.0)
    }
}

/// The FM pre-emphasis of the generator for `tau_s` (0: none) at `rate`: `(1 + s tau)` up to a corner far above the audio band
/// (`sdrkit_algo_dsp::iir::PreEmphasis::bilinear`).
pub fn pre_emphasis(tau_s: f64, rate: f64) -> Option<sdrkit_algo_dsp::iir::PreEmphasis> {
    if tau_s <= 0.0 {
        return None;
    }
    let upper = (0.4 * rate).min(150e3);
    sdrkit_algo_dsp::iir::PreEmphasis::bilinear(tau_s, upper, rate).ok()
}

sdrkit_core::choice_enum! {
    /// The second sound of the generator.
    pub enum SecondSound {
        /// None.
        None = ("none", "None"),
        /// A2 stereo (pilot modulated with 117.5 Hz).
        A2Stereo = ("a2_stereo", "A2 stereo"),
        /// A2 two languages (274.1 Hz).
        A2Dual = ("a2_dual", "A2 two languages"),
        /// A2 mono (an unmodulated pilot).
        A2Mono = ("a2_mono", "A2 mono (pilot)"),
        /// NICAM stereo.
        NicamStereo = ("nicam_stereo", "NICAM stereo"),
        /// NICAM two mono channels.
        NicamDual = ("nicam_dual", "NICAM two languages"),
    }
}

impl SecondSound {
    /// A2 of any kind.
    pub fn is_a2(self) -> bool {
        matches!(
            self,
            SecondSound::A2Stereo | SecondSound::A2Dual | SecondSound::A2Mono
        )
    }
    /// NICAM of any kind.
    pub fn is_nicam(self) -> bool {
        matches!(self, SecondSound::NicamStereo | SecondSound::NicamDual)
    }
}

/// The A2 second carrier spacing above the first, Hz (15.5 line rates).
pub const A2_SPACING: f64 = 242_187.5;
/// The A2 pilot, Hz (3.5 line rates).
pub const A2_PILOT: f64 = 54_687.5;
/// The A2 stereo identification, Hz.
pub const A2_STEREO_ID: f64 = 117.5;
/// The A2 dual identification, Hz.
pub const A2_DUAL_ID: f64 = 274.1;

/// The NICAM-728 transmitter: frames of tones (or two languages), DQPSK, root-raised-cosine pulses evaluated at any sample time.
pub struct NicamTx {
    mode: Mode,
    frame: u64,
    symbols: VecDeque<Complex32>,
    /// Index of `symbols[0]`.
    first: i64,
    phase: Complex32,
    pre: [PreEmphasis; 2],
    t32: u64,
    pulse: PulseTable,
    peak: f32,
    span: i64,
    tones: [f64; 2],
}

impl NicamTx {
    /// Stereo or two languages, roll-off `alpha`.
    pub fn new(dual: bool, alpha: f64) -> Self {
        let span = 4i64;
        let shape = Shape::Rrc { beta: alpha };
        let pulse =
            PulseTable::new(shape, (span + 1) as f64, 512).expect("the roll-off is in [0, 1]");
        NicamTx {
            mode: if dual { Mode::DualMono } else { Mode::Stereo },
            frame: 0,
            symbols: VecDeque::new(),
            first: 0,
            phase: Complex32::new(1.0, 0.0),
            pre: [crate::nicam::j17_pre(), crate::nicam::j17_pre()],
            t32: 0,
            peak: (1.0 / shape.value(0.0)) as f32,
            pulse,
            span,
            tones: [1000.0, 1500.0],
        }
    }

    fn next_frame(&mut self) {
        let mut x = [0i32; 64];
        match self.mode {
            Mode::Stereo => {
                for k in 0..32 {
                    let t = self.t32 as f64 / 32_000.0;
                    for ch in 0..2 {
                        let v = self.pre[ch].process_f64(0.25 * (TAU * self.tones[ch] * t).sin());
                        x[2 * k + ch] = (v * 8192.0).round().clamp(-8192.0, 8191.0) as i32;
                    }
                    self.t32 += 1;
                }
            }
            _ => {
                // Two languages: M1 in odd frames (frame numbers 1, 3, ...), M2 in even frames; 64 samples (2 ms) of one channel.
                let ch = if self.frame % 2 == 1 { 0 } else { 1 };
                let t0 = (self.frame / 2) * 64;
                for (k, xv) in x.iter_mut().enumerate() {
                    let t = (t0 + k as u64) as f64 / 32_000.0;
                    let v = self.pre[ch].process_f64(0.25 * (TAU * self.tones[ch] * t).sin());
                    *xv = (v * 8192.0).round().clamp(-8192.0, 8191.0) as i32;
                }
            }
        }
        let mut bits = [0u8; FRAME_BITS];
        build_frame(self.frame, self.mode, &x, &mut bits);
        self.frame += 1;
        for pair in bits.as_chunks::<2>().0 {
            let step = match (pair[0], pair[1]) {
                (0, 0) => Complex32::new(1.0, 0.0),
                (0, 1) => Complex32::new(0.0, -1.0),
                (1, 0) => Complex32::new(0.0, 1.0),
                _ => Complex32::new(-1.0, 0.0),
            };
            self.phase *= step;
            self.symbols.push_back(self.phase);
        }
    }

    /// The baseband at time `t` seconds (a pulse train of unit-peak symbols).
    pub fn at(&mut self, t: f64) -> Complex32 {
        let ts = t * SYMBOL_RATE;
        let k0 = ts.floor() as i64;
        while self.first + (self.symbols.len() as i64) <= k0 + self.span + 1 {
            self.next_frame();
        }
        // Drop old symbols.
        while self.first < k0 - self.span - 2 && !self.symbols.is_empty() {
            self.symbols.pop_front();
            self.first += 1;
        }
        let mut s = Complex32::default();
        for k in (k0 - self.span)..=(k0 + self.span + 1) {
            if k < self.first {
                continue;
            }
            let p = self.pulse.value(ts - k as f64) * self.peak;
            s += self.symbols[(k - self.first) as usize] * p;
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programmes_have_their_shape() {
        let rms = |p: Programme, a: f64, b: f64| {
            let n = 4800;
            ((0..n)
                .map(|i| p.value(a + (b - a) * i as f64 / n as f64).powi(2))
                .sum::<f64>()
                / n as f64)
                .sqrt()
        };
        assert!((rms(Programme::ToneGaps, 0.1, 1.0) - 0.3536).abs() < 0.01);
        assert!(rms(Programme::ToneGaps, 1.31, 1.49) < 1e-9);
        assert!(rms(Programme::Unmodulated, 0.0, 1.0) == 0.0);
        assert!(rms(Programme::Melody, 0.0, 0.3) > 0.2);
    }
}
