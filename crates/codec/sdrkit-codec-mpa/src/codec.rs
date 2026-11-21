//! The Layer II decoder as a codec plug-in.

use crate::layer2::Decoder;
use sdrkit_core::codec::{AudioCodec, CodecError, CodecFactory, CodecSet, Pcm};
use sdrkit_core::media::{Codec, MediaFormat, Packaging};
use std::sync::Arc;

/// Registers the Layer II decoder in `set` for bare MPEG audio frames (`Codec::Mp2`, `Packaging::Raw`), for any layout.
pub fn register(set: &mut CodecSet) {
    let factory: CodecFactory = Arc::new(|f: &MediaFormat| {
        (f.codec == Codec::Mp2 && f.packaging == Packaging::Raw)
            .then(|| Box::new(Plugin::default()) as Box<dyn AudioCodec>)
    });
    set.register(Codec::Mp2.fourcc(), None, factory);
}

/// One decoder state. The output is interleaved stereo; a mono frame has the same sample on both channels.
#[derive(Default)]
struct Plugin {
    dec: Decoder,
    out: Vec<f32>,
    rate: u32,
}

impl AudioCodec for Plugin {
    fn decode(&mut self, frame: &[u8], erasure: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
        pcm.channels = 2;
        if erasure || frame.is_empty() {
            if self.rate > 0 {
                pcm.rate_hz = self.rate;
                pcm.samples.resize(pcm.samples.len() + 2 * 1152, 0);
            }
            return Ok(());
        }
        self.out.clear();
        let h = self
            .dec
            .decode(frame, &mut self.out)
            .map_err(|_| CodecError::Frame)?;
        self.rate = h.sample_rate;
        pcm.rate_hz = h.sample_rate;
        pcm.samples.extend(
            self.out
                .iter()
                .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0) as i16),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Encoder;

    #[test]
    fn a_registered_decoder_decodes_a_frame_and_conceals_a_lost_one() {
        let mut set = CodecSet::default();
        register(&mut set);
        let entry = set.find(Codec::Mp2.fourcc(), "").unwrap();
        assert!((entry.factory)(&MediaFormat::new(Codec::Mp2, Packaging::Adts)).is_none());
        let mut dec = (entry.factory)(&MediaFormat::new(Codec::Mp2, Packaging::Raw)).unwrap();
        let tone: Vec<f32> = (0..1152).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        let mut frame = Vec::new();
        Encoder::new(128, 48_000, true)
            .unwrap()
            .encode(&tone, &tone, 0, &mut frame)
            .unwrap();
        let mut pcm = Pcm::default();
        dec.decode(&frame, false, &mut pcm).unwrap();
        assert_eq!(
            (pcm.rate_hz, pcm.channels, pcm.samples.len()),
            (48_000, 2, 2 * 1152)
        );
        dec.decode(&[], true, &mut pcm).unwrap();
        assert_eq!(pcm.samples.len(), 4 * 1152);
        assert_eq!(dec.decode(&[0; 4], false, &mut pcm), Err(CodecError::Frame));
    }
}
