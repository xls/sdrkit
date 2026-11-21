//! MPEG-4 AAC: the decoder of AAC-LC, HE-AAC (SBR) and HE-AAC v2 (SBR and parametric stereo) with frames of 1024 or 960
//! samples, as a codec plug-in, and an AAC-LC encoder of 960-sample frames for test signals as DAB+ and DRM carry them. The
//! packaging (AudioSpecificConfig, LATM, ADTS) is in `sdrkit_proto_bcdata::aac`.
//!
//! Standard: ISO/IEC 14496-3 subparts 4 and 8.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod decoder;
mod encoder;
mod error;
mod filterbank;
mod huffman;
mod ics;
pub mod mdct;
mod ps;
mod sbr;
mod stereo;
mod tables;

pub use encoder::{Encoder, FRAME};
pub use error::{Error, Result};
pub use mdct::Mdct;

use sdrkit_core::codec::{AudioCodec, CodecSet};
use sdrkit_core::media::{Codec, MediaFormat};
use std::sync::Arc;

/// Registers the decoder of AAC-LC, HE-AAC and HE-AAC v2 in `set`: bare access units with an AudioSpecificConfig in the
/// media format (frames of 1024 or 960 samples), or ADTS.
pub fn register(set: &mut CodecSet) {
    for codec in [Codec::Aac, Codec::HeAac, Codec::HeAacV2] {
        set.register(
            codec.fourcc(),
            None,
            Arc::new(|f: &MediaFormat| {
                decoder::Decoder::new(f).map(|d| Box::new(d) as Box<dyn AudioCodec>)
            }),
        );
    }
}
