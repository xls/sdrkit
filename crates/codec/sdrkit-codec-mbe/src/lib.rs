//! Decoder and encoder for the multi-band excitation voice coders of digital voice radio, at 8 kHz, 16-bit, 160
//! samples (20 ms) a frame:
//!
//! - the full-rate coder: 88 speech bits, 144 channel bits with \[23,12\] Golay and \[15,11\] Hamming codes;
//! - the half-rate coder: 49 speech bits, 72 channel bits with \[24,12\] and \[23,12\] Golay codes, and tone frames;
//! - the D-STAR coder: 48 speech bits, 72 channel bits with two Golay words, the quantisers of the half-rate coder with
//!   its own tables.
//!
//! All share one speech model ([`Params`]), one analysis and one synthesis; each has its own quantisers and bit layout.
//! Channel bits are one bit per byte in the order sent.
//! [`codewords`] packs a half-rate frame as the `codewords` payload of speech frames, [`codeword_channel_bits`] unpacks it.
//!
//! Standards: TIA-102.BABA (full rate), TIA-102.BABA-1 (half rate), JARL D-STAR (the frame of the D-STAR coder and its
//! silence pattern; no public document gives the tables of that coder).
//!
//! Readings where the documents are unclear, each kept in one named item:
//! sub-multiple rounding to the nearest half sample (`analysis::submultiple`); higher-order coefficients for k >= 2 of
//! every block (`full::dequantise_frame`); the full-rate repeat and mute rules of the text, not of the flow chart
//! ([`full_rate_repeats`], [`FULL_MUTE_RATE`]), with the fourth-frame mute of the flow chart for lost frames only
//! ([`FULL_LOST_MUTE_RUN`]); tau_M starts at 20480 ([`TAU_M_START`]); the random phase of harmonic
//! l uses noise sample l of the frame's window (`synth::Synth::voiced`); the half-rate voicing element is
//! floor(16 l omega_0 / 2 pi) (`half::voicing_element`); half-rate coefficients C_{i,k} = H_{i,k-2} for
//! 3 <= k <= min(J, 6) (`half::HOC_LAST_K`); the half-rate pitch column is cycles per sample (`half::w0_of`); nearest pitch
//! by absolute difference (`half::Books::b0_of`); the tone test on the six top bits of u0 first (`half::is_tone`); the tone
//! amplitude LSB in u3 bit 4 (`half::TONE_AD_LSB_U3_BIT`); tones are synthesised through Annex J (`half::tone_params`),
//! both tones of a pair at the level of Equation 68, without enhancement and without the random phase of Equation 141;
//! no half-rate smoothing ([`HALF_RATE_SMOOTHING`]); the half-rate encoder measures the amplitudes at the Annex A
//! fundamental and L and the voicing at the analysed fundamental; the encoder sends only voice frames; the first bit of
//! a dibit is its high bit.
//!
//! The D-STAR coder, where no document settles the point, each in one named item: the pitch formula (`dstar::w0_of`);
//! b0 = 120 to 125 are voice frames of 56 harmonics ([`DSTAR_B0_VOICE_MAX`]); the state starts at L(-1) = 30 and
//! Lambda(-1) = 0 (`dstar::BOOKS`); a frame repeats above three corrected data-bit errors ([`DSTAR_REPEAT_ERRORS`]) or
//! when word A has four errors ([`DSTAR_WORD_A_DETECTS`]), and the fourth repeat in a row is muted; bit 0 of word B is
//! sent as the even parity of the 23 bits before it and is not read (`dstar::decode_channel`); the tone field map
//! (`dstar::tone_from`); tone frames and muted frames are silence; enhancement and synthesis are those of the other
//! coders.
//!
//! Where the encoder's pitch estimation departs from the printed text, each in one named item: the error function
//! E(P) sums the autocorrelation over every lag, not to lag 150 (`analysis::Lags::error`); above P = 80 it divides by
//! the real window overlap (`analysis::fit_error`); the sub-multiple test of Equations 18 to 20 takes E at P / n not
//! rounded, with a cubic between lags (`analysis::Analyser::submultiple_error`); sub-multiples count down to the
//! period 19.875, with the divisor 6 (`analysis::submultiple`).
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
mod codewords;
mod decoder;
mod dstar;
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

pub use codewords::{codeword_channel_bits, codewords};
pub use decoder::{full_rate_repeats, Decoder, FULL_LOST_MUTE_RUN, FULL_MUTE_RATE, HALF_RATE_SMOOTHING};
pub use dstar::{
    silence_bits as dstar_silence_bits, B0_VOICE_MAX as DSTAR_B0_VOICE_MAX, REPEAT_ERRORS as DSTAR_REPEAT_ERRORS,
    SILENCE as DSTAR_SILENCE, WORD_A_DETECTS as DSTAR_WORD_A_DETECTS,
};
pub use encoder::Encoder;
pub use enhance::TAU_M_START;
pub use model::{Params, FRAME, MAX_L};

/// One of the coders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Coder {
    /// The full-rate coder: 88 speech bits, 144 channel bits.
    FullRate,
    /// The half-rate coder: 49 speech bits, 72 channel bits.
    HalfRate,
    /// The D-STAR coder: 48 speech bits, 72 channel bits.
    Dstar,
}

impl Coder {
    /// Every coder.
    pub const ALL: [Coder; 3] = [Coder::FullRate, Coder::HalfRate, Coder::Dstar];

    /// Channel bits per frame.
    pub const fn frame_bits(self) -> usize {
        match self {
            Coder::FullRate => 144,
            Coder::HalfRate | Coder::Dstar => 72,
        }
    }

    /// Speech bits per frame, after error correction.
    pub const fn speech_bits(self) -> usize {
        match self {
            Coder::FullRate => 88,
            Coder::HalfRate => 49,
            Coder::Dstar => 48,
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
    /// A tone frame with its index and amplitude. The half-rate decoder gives the tone; the D-STAR decoder gives
    /// silence.
    Tone {
        /// Tone index.
        id: u8,
        /// Amplitude. Half rate: 0..=127, 0.711 dB a step. D-STAR: the 8-bit level of the frame, of no known scale.
        amplitude: u8,
    },
    /// The frame was invalid and the previous one was synthesised again.
    Repeated,
    /// The output is comfort noise, or silence for the D-STAR coder.
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
