//! The decoder synthesis of one 10 ms frame: the zero-order phase model, the background noise post filter, sinusoidal
//! synthesis by inverse FFT, overlap-add, and the output gain and limiter.
//!
//! Standard: D. Rowe, "Codec 2 Algorithm Description" (2025), sections 3.3 and 3.6.

use crate::analysis::Model;
use crate::consts::{
    atan2f, cosf, log10f, pow10d, sinf, trunc_i, FFT_SIZE, MAX_AMP, N_SAMP, TWO_PI,
};
use sdrkit_algo_dsp::fft_exact::ExactRealFft;
use sdrkit_algo_dsp::Complex32;

/// The overlap-add buffer length (two internal frames).
const SYN_LEN: usize = 2 * N_SAMP;

/// The decoder synthesis state.
pub(crate) struct Synth {
    ex_phase: f32,
    bg_est: f32,
    out: [f32; SYN_LEN],
    pn: [f32; SYN_LEN],
    rng: u32,
    gain: f32,
    phase: [f32; MAX_AMP + 1],
    ifft: ExactRealFft,
    bins: Box<[Complex32]>,
    s: Box<[f32]>,
}

impl Synth {
    /// A new state. `gain` is the output gain of the mode.
    pub fn new(gain: f32) -> Self {
        let mut pn = [0.0f32; SYN_LEN];
        let mut v = 0.0f32;
        for p in &mut pn[..N_SAMP] {
            *p = v;
            v = (v as f64 + 1.0 / N_SAMP as f64) as f32;
        }
        let mut v = 1.0f32;
        for p in &mut pn[N_SAMP..] {
            *p = v;
            v = (v as f64 - 1.0 / N_SAMP as f64) as f32;
        }
        Synth {
            ex_phase: 0.0,
            bg_est: 0.0,
            out: [0.0; SYN_LEN],
            pn,
            rng: 1,
            gain,
            phase: [0.0; MAX_AMP + 1],
            ifft: ExactRealFft::new(FFT_SIZE, true).expect("512 is a valid FFT size"),
            bins: vec![Complex32::new(0.0, 0.0); FFT_SIZE / 2 + 1].into_boxed_slice(),
            s: vec![0.0; FFT_SIZE].into_boxed_slice(),
        }
    }

    /// The next value of the phase randomiser, 0 to 32767.
    fn rand(&mut self) -> u32 {
        self.rng = self.rng.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (self.rng / 65_536) % 32_768
    }

    /// Synthesises one frame from `model` (with `model.a` and `model.wo` set) and the phase vector `h`. Writes 80 samples.
    pub fn frame(&mut self, model: &Model, h: &[Complex32; MAX_AMP + 1], pcm: &mut [i16]) {
        let l = model.l.min(MAX_AMP);
        let wo = model.wo;

        // Excitation phase and harmonic phases.
        self.ex_phase += wo * N_SAMP as f32;
        let wrap = ((self.ex_phase as f64 / TWO_PI + 0.5) as f32).floor();
        self.ex_phase = (self.ex_phase as f64 - TWO_PI * wrap as f64) as f32;
        for m in 1..=l {
            let (c, s) = if model.voiced {
                let arg = self.ex_phase * m as f32;
                (cosf(arg), sinf(arg))
            } else {
                let phi = (TWO_PI * self.rand() as f64 / 32767.0) as f32;
                (cosf(phi), sinf(phi))
            };
            let hm = h[m];
            let yr = hm.re * c - hm.im * s;
            let yi = hm.im * c + hm.re * s;
            self.phase[m] = atan2f(yi, (yr as f64 + 1e-12) as f32);
        }

        // Background noise estimate, and random phases for weak harmonics of voiced frames.
        let mut e = 1e-12f64 as f32;
        for a in &model.a[1..=l] {
            e += a * a;
        }
        let e = (10.0 * log10f(e / l.max(1) as f32) as f64) as f32;
        if e < 40.0 && !model.voiced {
            self.bg_est = (self.bg_est as f64 * (1.0 - 0.1) + e as f64 * 0.1) as f32;
        }
        let thresh = pow10d((self.bg_est as f64 + 6.0) / 20.0);
        if model.voiced {
            for m in 1..=l {
                if model.a[m] < thresh {
                    self.phase[m] = ((TWO_PI / 32767.0) * self.rand() as f64) as f32;
                }
            }
        }

        // Sinusoidal synthesis.
        self.out.copy_within(N_SAMP.., 0);
        self.out[N_SAMP - 1] = 0.0;
        self.bins.fill(Complex32::new(0.0, 0.0));
        for m in 1..=l {
            let mw = (m as f32 * wo) * FFT_SIZE as f32;
            let b = trunc_i(mw as f64 / TWO_PI + 0.5).clamp(0, FFT_SIZE as i32 / 2 - 1) as usize;
            let ph = self.phase[m];
            self.bins[b] = Complex32::new(model.a[m] * cosf(ph), model.a[m] * sinf(ph));
        }
        let _ = self.ifft.inverse(&self.bins, &mut self.s);
        let tail = FFT_SIZE - N_SAMP + 1;
        for i in 0..N_SAMP - 1 {
            self.out[i] += self.s[tail + i] * self.pn[i];
        }
        for i in N_SAMP - 1..SYN_LEN {
            self.out[i] = self.s[i - (N_SAMP - 1)] * self.pn[i];
        }

        // Gain, ear protection and output.
        let mut peak = 0.0f32;
        for v in &mut self.out[..N_SAMP] {
            *v *= self.gain;
            if *v > peak {
                peak = *v;
            }
        }
        let over = (peak as f64 / 30000.0) as f32;
        if over > 1.0 {
            let g = (1.0 / (over * over) as f64) as f32;
            for v in &mut self.out[..N_SAMP] {
                *v *= g;
            }
        }
        for (o, &v) in pcm.iter_mut().zip(&self.out[..N_SAMP]) {
            *o = if v > 32767.0 {
                32767
            } else if v < -32767.0 {
                -32767
            } else {
                v as i16
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_randomiser_follows_the_c_library_generator() {
        // The first outputs of the ANSI C example generator with seed 1 (ISO C 7.20.2.2).
        let mut s = Synth::new(1.0);
        let got: Vec<u32> = (0..3).map(|_| s.rand()).collect();
        assert_eq!(got, [16838, 5758, 10113]);
    }

    #[test]
    fn the_synthesis_window_is_a_triangle() {
        let s = Synth::new(1.0);
        assert_eq!(s.pn[0], 0.0);
        assert_eq!(s.pn[N_SAMP], 1.0);
        assert!((s.pn[40] - 0.5).abs() < 1e-6);
        assert!((s.pn[120] - 0.5).abs() < 1e-6);
    }
}
