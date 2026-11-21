//! MPEG-2 video: a decoder of main profile at main level. It takes the access units of an elementary stream and gives
//! frames of planar 4:2:0 samples in display order, each with its size, aspect ratio, frame rate, time stamp and field
//! order.
//!
//! It decodes I, P and B pictures as frame pictures and as field pictures, with frame, field, 16x8 and dual-prime
//! prediction, frame and field DCT, both scans and both coefficient tables. A slice with an error ends at the error,
//! and decoding goes on at the next slice; the macroblocks that are missing come from the picture before.
//! MPEG-1 video and the 4:2:2 and 4:4:4 formats are not decoded.
//!
//! Standard: ISO/IEC 13818-2 (ITU-T H.262). The inverse DCT meets IEEE Std 1180-1990.
//!
//! ```
//! use sdrkit_codec_mpeg2video::Decoder;
//!
//! let mut dec = Decoder::new();
//! let mut frames = 0;
//! // A sequence end code alone: no picture.
//! dec.decode(&[0, 0, 1, 0xb7], Some(0), &mut |_frame| frames += 1);
//! assert_eq!(frames, 0);
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]

mod decoder;
mod header;
pub mod idct;
mod kernel;
mod mc;
#[cfg(target_arch = "aarch64")]
mod neon;
mod slice;
pub mod tables;

pub use decoder::{Decoder, Frame, PictureKind, Stats};
