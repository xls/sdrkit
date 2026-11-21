//! Codec 2 speech encoder and decoder for the modes 3200, 1300, 1200 and 700C, at 8 kHz. The trained codebook tables and the
//! pitch filter taps are not part of this crate: the host loads them at run time into [`Codebooks`]. The arithmetic has a
//! fixed order and a fixed precision, so the output is bit-exact on every platform.
//!
//! Standard: D. Rowe, "Codec 2 Algorithm Description" (2025).
//!
//! ```no_run
//! use sdrkit_codec_codec2::{Codebooks, Decoder, Encoder, Mode};
//! let books = Codebooks::load_dir(std::path::Path::new("codec2-tables")).unwrap();
//! let mode = Mode::M1300;
//! let mut enc = Encoder::new(mode, &books).unwrap();
//! let mut dec = Decoder::new(mode, &books).unwrap();
//! let speech = vec![0i16; mode.samples_per_frame()];
//! let mut frame = vec![0u8; mode.bytes_per_frame()];
//! let mut out = vec![0i16; mode.samples_per_frame()];
//! enc.encode(&speech, &mut frame).unwrap();
//! dec.decode(&frame, &mut out).unwrap();
//! ```
#![warn(missing_docs)]
// The DSP loops index several arrays by the same harmonic or bin number, as the formulas do.
#![allow(clippy::needless_range_loop)]

mod analysis;
mod codebooks;
mod codec;
mod consts;
mod error;
mod lpc;
mod newamp1;
mod synth;

pub use codebooks::{Codebooks, Table, TABLE_COUNT};
pub use codec::{Decoder, Encoder, Mode};
pub use error::Error;
