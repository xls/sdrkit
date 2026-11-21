//! Signal processing components: oscillators, filters, resamplers, FFTs, windows, loop filters, constellations and estimators.
//!
//! Samples are `f32` ([`Complex32`] for IQ, `f32` for real audio, [`Stereo`] for stereo frames). Phase, time and ratio
//! bookkeeping use `f64` or fixed-point integers where accuracy over hours matters. Frequencies are in hertz with an explicit
//! rate, or in cycles per sample where the name says "norm".
//!
//! Streaming objects keep their state between calls, so a signal passed in pieces of any length, including 1, produces the
//! same output as one piece. Objects that keep the rate work slice to slice (or in place). Objects that change the rate
//! **append** to a caller-owned `Vec` and never clear it. The filter design functions panic on impossible parameters. Objects built
//! from run-time values ([`channel::Channelizer`], [`resample::Resampler`], [`spectrum::SpectrumAnalyzer`]) return [`DspError`]
//! instead.
//!
//! [`benchmarks`] times the main components (for the C API's `sdrkit_benchmark`, run on the device).
//!
//! | Module | Contents |
//! |---|---|
//! | [`pulse`] | RRC, RC, Gaussian frequency pulse, Gaussian, half-sine, rectangle, `erf`/`erfc`, tap design with a stated normalisation, [`pulse::PulseTable`] |
//! | [`interp`] | linear, cubic Lagrange, Catmull-Rom and polyphase interpolators; [`interp::Reader`] reads a stored slice at any fractional position |
//! | [`loopfilter`] | second-order loop gains from noise bandwidth and damping; the `f64` phase accumulator |
//! | [`constellation`] | point sets with their bit labels: PSK, PAM, square QAM (802.11, DVB, 3GPP maps), DVB-T hierarchical, DVB-C, DVB-S2 8PSK/16APSK/32APSK, 8-VSB |
//! | [`cic`] | integer CIC decimators with a register-width check; droop compensation |
//! | [`goertzel`] | Goertzel over a slice, streaming, and a bank of tones |
//! | [`peak`] | parabolic, log-parabolic and Candan peak fits; least-squares lines |
//! | [`carrierfind`] | a carrier finder in a narrow window: peak, noise per bin, presence, refined frequency, C/N0 |
//! | [`allan`], [`kmeans`], [`plateau`] | the Allan deviation of readings; one-dimensional k-means, also with no allocation; plateaus, edge crossings, steps and settling of series |
//! | [`floor`] | noise-floor estimators (bin median with chi-square correction, chunk percentile, minimum follower), never under a set minimum |
//! | [`denoise`] | picture and scan-line noise filters: line Wiener, 3 x 3 Lee with its Laplacian noise measure, click removal |
//! | [`bank`] | polyphase FFT analysis bank and overlap-save channel bank |
//! | [`stats`], [`linalg`], [`ring`] | the nearest-rank quantile of `f64` values, a sliding window sum that does not drift, the plain kurtosis of complex samples, a Cholesky solver, a row-by-row least-squares fit with a ridge, a ring of samples with absolute indices, and a fixed delay line |
//! | [`fft_exact`] | a complex and a real FFT with a fixed order of operations, for bit-exact codecs |
//! | [`dynamics`] | speech level control: a leveller towards an RMS target and a look-ahead limiter whose gain has no steps |
//! | [`lpc`] | autocorrelation, Levinson-Durbin, bandwidth expansion, LPC to LSP and LSP to LPC in `f32` |
//! | [`window`], [`iir`], [`fir`], [`decim`], [`spectrum`] | Hamming, Blackman, flat-top and Kaiser windows in both forms; complex IIR, bilinear de-emphasis and pre-emphasis; design by tap count and centred convolution; complex-tap and per-sample decimators; one-shot Welch |

#![warn(missing_docs)]

pub mod agc;
pub mod allan;
pub mod approx;
pub mod bandscan;
pub mod bandwidth;
pub mod bank;
mod bench;
pub mod blackman_rational;
pub mod blockrank;
pub mod carrierfind;
pub mod carrierline;
pub mod ccdf;
pub mod channel;
pub mod channelgrid;
pub mod cic;
pub mod constellation;
pub mod converter;
pub mod dc;
pub mod decim;
pub mod denoise;
pub mod dispersion;
pub mod dynamics;
mod error;
pub mod fadestats;
pub mod fft;
pub mod fft_exact;
pub mod fir;
pub mod floor;
pub mod fm;
pub mod fold;
pub mod goertzel;
pub mod iir;
pub mod kmeans;
pub mod impulse;
pub mod iqmoments;
pub mod interp;
pub mod levelfit;
pub mod linalg;
pub mod linetrack;
pub mod loopfilter;
pub mod lpc;
pub mod meter;
pub mod nco;
#[cfg(target_arch = "aarch64")]
mod neon;
pub mod notch;
pub mod peak;
pub mod plateau;
pub mod pulse;
pub mod pulsetrain;
pub mod repeat;
pub mod resample;
pub mod ring;
pub mod sample;
pub mod shortframe;
pub mod sinc_table;
pub mod slicer;
pub mod snapshot;
pub mod speckurt;
pub mod spectrum;
pub mod stats;
pub mod stepfind;
pub mod window;

pub use bench::benchmarks;
pub use error::{DspError, Param};
pub use sample::{Append, Frame, FrameWriter, Sample, Stereo};
pub use sdrkit_core::Complex32;

#[cfg(test)]
pub(crate) mod testutil {
    use crate::Complex32;
    use std::f64::consts::TAU;

    /// A complex tone `amp * e^{j 2 pi f n}`, `f` in cycles per sample.
    pub fn ctone(n: usize, f: f64, amp: f64) -> Vec<Complex32> {
        (0..n)
            .map(|i| {
                let p = TAU * ((f * i as f64) % 1.0);
                Complex32::new((amp * p.cos()) as f32, (amp * p.sin()) as f32)
            })
            .collect()
    }

    /// A real tone `amp * cos(2 pi f n)`.
    pub fn rtone(n: usize, f: f64, amp: f64) -> Vec<f32> {
        (0..n)
            .map(|i| (amp * (TAU * ((f * i as f64) % 1.0)).cos()) as f32)
            .collect()
    }

    /// The complex amplitude of the component at `f` (cycles per sample): |(1/N) sum x e^{-j 2 pi f n}|.
    pub fn camp(x: &[Complex32], f: f64) -> f64 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, v) in x.iter().enumerate() {
            let p = -TAU * ((f * i as f64) % 1.0);
            let (s, c) = p.sin_cos();
            re += v.re as f64 * c - v.im as f64 * s;
            im += v.re as f64 * s + v.im as f64 * c;
        }
        (re * re + im * im).sqrt() / x.len() as f64
    }

    /// The amplitude of a real cosine at `f`: 2 |(1/N) sum x e^{-j 2 pi f n}|.
    pub fn ramp(x: &[f32], f: f64) -> f64 {
        let c: Vec<Complex32> = x.iter().map(|&v| Complex32::new(v, 0.0)).collect();
        2.0 * camp(&c, f)
    }

    pub fn db(x: f64) -> f64 {
        20.0 * x.log10()
    }

    /// Repeatable uniform noise in [-1, 1) (xorshift).
    pub struct Rng(pub u64);
    impl Rng {
        pub fn uniform(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        }
        /// Standard normal (Box-Muller).
        pub fn gauss(&mut self) -> f64 {
            let u1 = (self.uniform() + 1.0) / 2.0 + 1e-300;
            let u2 = (self.uniform() + 1.0) / 2.0;
            (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
        }
    }
}
