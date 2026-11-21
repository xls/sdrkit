//! The codecs of the custom stacks: voice codec 4, the V3 and V1 speech codecs, the wavelet picture codec and the synthetic video
//! test units.
//!
//! # Modules
//!
//! | Module | What it holds |
//! |---|---|
//! | [`voice4`] | the encoder and decoder of voice codec 4 (3.2 to 16 kbit/s, 20 and 40 ms frames, 8 and 16 kHz, SID, DTX, concealment) |
//! | [`voice3`] | the V3 (12 to 32 kbit/s) and V1 (16 kbit/s) speech encoders and decoders |
//! | [`picture`] | the wavelet picture encoder and decoder (CDF 9/7 wavelet, bitplanes, range coder) |
//! | [`video_units`] | the synthetic video test units that stand in for the video and audio codecs |
//!
//! PCM is signed 16-bit mono. Bad input returns an [`Error`] or a lost frame.

#![warn(missing_docs)]
#![forbid(unsafe_code)]
// The codec arithmetic follows the spec index by index, and negated comparisons catch not-a-number values on purpose.
#![allow(
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::neg_cmp_op_on_partial_ord
)]

mod error;
pub mod picture;
pub mod video_units;
pub mod voice4;
pub mod voice3;

pub use error::{Error, Param};
