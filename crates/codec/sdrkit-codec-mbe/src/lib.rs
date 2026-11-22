//! Decoder and encoder for the two multi-band excitation voice coders of digital voice radio, at 8 kHz, 16-bit, 160
//! samples (20 ms) a frame:
//!
//! - the full-rate coder: 88 speech bits, 144 channel bits with [23,12] Golay and [15,11] Hamming codes;
//! - the half-rate coder: 49 speech bits, 72 channel bits with [24,12] and [23,12] Golay codes, and tone frames.
//!
//! Both share one speech model ([`Params`]), one analysis and one synthesis; each has its own quantisers and bit layout.
//! Channel bits are one bit per byte in the order sent.
//!
//! Standards: TIA-102.BABA (full rate) and TIA-102.BABA-1 (half rate).
//!
//! Readings where the documents are unclear, each kept in one named item:
//! sub-multiple rounding to the nearest half sample (`analysis::submultiple`); the sub-multiple test of Equations 18
//! to 20 takes E at the best 1/8-sample period near P / n with 4-point lag interpolation, so that a period between half
//! samples is not lost to two or three periods (`analysis::Analyser::refined_submultiple`); higher-order coefficients for k >= 2 of
//! every block (`full::dequantise_frame`); the full-rate repeat and mute rules of the text, not of the flow chart
//! ([`full_rate_repeats`], [`FULL_MUTE_RATE`]); tau_M starts at 20480 ([`TAU_M_START`]); the random phase of harmonic
//! l uses noise sample l of the frame's window (`synth::Synth::voiced`); the half-rate voicing element is
//! floor(16 l omega_0 / 2 pi) (`half::voicing_element`); half-rate coefficients C_{i,k} = H_{i,k-2} for
//! 3 <= k <= min(J, 6) (`half::HOC_LAST_K`); the half-rate pitch column is cycles per sample (`half::w0_of`); nearest pitch
//! by absolute difference (`half::b0_of`); the tone test on the six top bits of u0 first (`half::is_tone`); the tone
//! amplitude LSB in u3 bit 4 (`half::TONE_AD_LSB_U3_BIT`); tones are synthesised through Annex J (`half::tone_params`),
//! both tones of a pair at the level of Equation 68, without enhancement and without the random phase of Equation 141;
//! no half-rate smoothing ([`HALF_RATE_SMOOTHING`]); the half-rate encoder measures the amplitudes at the Annex A
//! fundamental and L; the encoder sends only voice frames; the first bit of a dibit is its high bit.
//!
//! ```
//! use sdrkit_codec_mbe::{Coder, Decoder, Encoder};
//! let mut enc = Encoder::new(Coder::HalfRate);
//! let mut dec = Decoder::new(Coder::HalfRate);
//! let pcm = [0i16; 160];
//! let mut frame = [0u8; 72];
//! assert_eq!(enc.encode(&pcm, &mut frame), 72);
//! let mut out = [0i16; 160];
//! let status = dec.decode(&frame, &mut out);
//! assert_eq!(status.errors, 0);
//! ```
#![warn(missing_docs)]
// The DSP loops index several arrays by the same harmonic or bin number, as the formulas do.
#![allow(clippy::needless_range_loop)]

mod analysis;
mod decoder;
mod encoder;
mod enhance;
mod fec;
mod full;
mod half;
mod model;
mod synth;
mod tables;
#[cfg(test)]
mod tests;

pub use decoder::{full_rate_repeats, Decoder, FULL_MUTE_RATE, HALF_RATE_SMOOTHING};
pub use encoder::Encoder;
pub use enhance::TAU_M_START;
pub use model::{Params, FRAME, MAX_L};

/// One of the two coders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Coder {
    /// The full-rate coder: 88 speech bits, 144 channel bits.
    FullRate,
    /// The half-rate coder: 49 speech bits, 72 channel bits.
    HalfRate,
}

impl Coder {
    /// Both coders.
    pub const ALL: [Coder; 2] = [Coder::FullRate, Coder::HalfRate];

    /// Channel bits per frame.
    pub const fn frame_bits(self) -> usize {
        match self {
            Coder::FullRate => 144,
            Coder::HalfRate => 72,
        }
    }

    /// Speech bits per frame, after error correction.
    pub const fn speech_bits(self) -> usize {
        match self {
            Coder::FullRate => 88,
            Coder::HalfRate => 49,
        }
    }
}

/// What the decoder made of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKind {
    /// A voice frame.
    Voice,
    /// A half-rate silence frame.
    Silence,
    /// A half-rate tone frame with its index and amplitude.
    Tone {
        /// Tone index.
        id: u8,
        /// Amplitude, 0..=127, 0.711 dB a step.
        amplitude: u8,
    },
    /// The frame was invalid and the previous one was synthesised again.
    Repeated,
    /// The output is comfort noise.
    Muted,
}

/// The result of decoding one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameStatus {
    /// What the frame was.
    pub kind: FrameKind,
    /// Bit errors that the FEC corrected (or detected) in the frame.
    pub errors: u32,
}
