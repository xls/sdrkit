//! MPEG-1 and MPEG-2 audio: the frame header of Layers I, II and III, and a Layer II decoder and encoder. DAB sends Layer II,
//! and transport streams carry all three layers.
//!
//! Standard: ISO/IEC 11172-3, ISO/IEC 13818-3.
//!
//! ```
//! use sdrkit_codec_mpa::{Decoder, Encoder};
//!
//! let mut enc = Encoder::new(128, 48_000, true).unwrap();
//! let tone: Vec<f32> = (0..1152).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
//! let mut frame = Vec::new();
//! enc.encode(&tone, &tone, 0, &mut frame).unwrap();
//! assert_eq!(frame.len(), 384);
//!
//! let mut pcm = Vec::new();
//! let h = Decoder::new().decode(&frame, &mut pcm).unwrap();
//! assert_eq!((h.layer, h.sample_rate, pcm.len()), (2, 48_000, 2 * 1152));
//! ```

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod codec;
mod error;
pub mod header;
pub mod layer2;
pub mod window;

pub use codec::register;
pub use error::{Error, Result};
pub use header::Header;
pub use layer2::{Decoder, Encoder, ScaleFactor};
