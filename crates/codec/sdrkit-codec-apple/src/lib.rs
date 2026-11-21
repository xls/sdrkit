//! Audio decoders on Apple's AudioToolbox, registered as codec plug-ins: AAC-LC, HE-AAC v1 and v2 and xHE-AAC (bare access
//! units with an AudioSpecificConfig, or ADTS), MPEG-1 and MPEG-2 Layers I, II and III, AC-3 and E-AC-3 (one access unit
//! a packet). The crate is empty on other platforms.
//!
//! Standard: ISO/IEC 14496-3 clauses 1.6.2.1 (AudioSpecificConfig) and 1.A.2.2 (ADTS), ATSC A/52 clause 5.4 (AC-3 frame
//! header), ETSI TS 102 366 annex E (E-AC-3 frame header).

#![cfg(any(target_os = "macos", target_os = "ios"))]
#![warn(missing_docs)]

mod converter;
mod setup;

use converter::Converter;
use sdrkit_core::codec::{AudioCodec, CodecError, CodecFactory, CodecSet, Pcm};
use sdrkit_core::media::{ChannelLayout, Codec, MediaFormat, Packaging};
use setup::Stream;
use std::sync::Arc;

/// The codecs this crate decodes.
pub const CODECS: [Codec; 9] = [
    Codec::Aac,
    Codec::HeAac,
    Codec::HeAacV2,
    Codec::XheAac,
    Codec::Mp1,
    Codec::Mp2,
    Codec::Mp3,
    Codec::Ac3,
    Codec::EAc3,
];

/// Registers a decoder for every codec of [`CODECS`] in `set`, for any layout.
pub fn register(set: &mut CodecSet) {
    let factory: CodecFactory =
        Arc::new(|f: &MediaFormat| Decoder::new(f).map(|d| Box::new(d) as Box<dyn AudioCodec>));
    for c in CODECS {
        set.register(c.fourcc(), None, factory.clone());
    }
}

/// One AudioToolbox decoder for one stream.
struct Decoder {
    format: MediaFormat,
    /// The converter and the stream it was made for.
    converter: Option<(Converter, Stream)>,
    /// Frames of silence for a lost packet, from the last good one.
    last_frames: usize,
}

impl Decoder {
    /// A decoder for `format`, or `None` if the packaging or the configuration is not one this crate decodes. A stream that
    /// carries its configuration in band starts its converter at the first packet.
    fn new(format: &MediaFormat) -> Option<Decoder> {
        let in_band = match (format.codec, format.packaging) {
            (Codec::Aac | Codec::HeAac | Codec::HeAacV2 | Codec::XheAac, Packaging::Raw) => false,
            (Codec::Aac | Codec::HeAac | Codec::HeAacV2, Packaging::Adts) => true,
            (Codec::Mp1 | Codec::Mp2 | Codec::Mp3 | Codec::Ac3 | Codec::EAc3, Packaging::Raw) => {
                true
            }
            _ => return None,
        };
        let mut d = Decoder {
            format: format.clone(),
            converter: None,
            last_frames: 0,
        };
        if !in_band {
            d.start(setup::from_config(format)?).ok()?;
        }
        Some(d)
    }

    /// Makes the converter for `stream`.
    fn start(&mut self, stream: Stream) -> Result<(), CodecError> {
        self.converter = None;
        let input = stream.input;
        let c = Converter::new(input, input.rate, &stream.cookie())
            .map_err(|_| CodecError::Unsupported)?;
        self.converter = Some((c, stream));
        Ok(())
    }

    /// The bytes of `frame` to decode. For a stream that carries its configuration in band, makes the converter at the first
    /// packet and again when the configuration changes.
    fn prepare<'a>(&mut self, frame: &'a [u8]) -> Result<&'a [u8], CodecError> {
        if self.format.packaging == Packaging::Raw && setup::is_aac(self.format.codec) {
            return Ok(frame);
        }
        let (stream, payload) = setup::from_frame(&self.format, frame)?;
        if self.converter.as_ref().map(|c| &c.1) != Some(&stream) {
            self.start(stream)?;
        }
        Ok(payload)
    }
}

impl Decoder {
    /// Whether the codec is AC-3 or E-AC-3.
    fn is_ac3(&self) -> bool {
        matches!(self.format.codec, Codec::Ac3 | Codec::EAc3)
    }

    /// Whether an AC-3 or E-AC-3 stream of 5.1 is mixed down to stereo: the format asks for 2 channels.
    fn mixes_down(&self) -> bool {
        self.is_ac3() && self.format.audio.as_ref().is_some_and(|a| a.channels == 2)
    }
}

impl AudioCodec for Decoder {
    fn decode(&mut self, frame: &[u8], erasure: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
        if erasure || frame.is_empty() {
            if let Some((_, s)) = &self.converter {
                pcm.rate_hz = s.input.rate as u32;
                pcm.channels = s.input.channels as u8;
                if self.mixes_down() && pcm.channels == 6 {
                    pcm.channels = 2;
                }
                pcm.layout = ChannelLayout::of_count(u16::from(pcm.channels));
                let n = self.last_frames * usize::from(pcm.channels);
                pcm.samples.resize(pcm.samples.len() + n, 0);
            }
            return Ok(());
        }
        let payload = self.prepare(frame)?;
        let Some((c, s)) = self.converter.as_mut() else {
            return Err(CodecError::Unsupported);
        };
        let at = pcm.samples.len();
        c.decode(payload, &mut pcm.samples)
            .map_err(|_| CodecError::Frame)?;
        pcm.rate_hz = s.input.rate as u32;
        pcm.channels = s.input.channels as u8;
        // AudioToolbox gives AC-3 and E-AC-3 3/2 with LFE as L C R Ls Rs LFE.
        pcm.layout = match pcm.channels {
            6 if self.is_ac3() => ChannelLayout::Surround51,
            n => ChannelLayout::of_count(u16::from(n)),
        };
        if self.mixes_down() {
            // E-AC-3 states its mix levels in optional mixing metadata, which is not read: it takes the -3 dB of
            // ATSC A/52 7.8.2.
            let levels = match self.format.codec {
                Codec::Ac3 => setup::ac3_mix_levels(frame),
                _ => Some((
                    std::f32::consts::FRAC_1_SQRT_2,
                    std::f32::consts::FRAC_1_SQRT_2,
                )),
            };
            if let Some((centre, surround)) = levels {
                pcm.downmix_51(at, centre, surround);
            }
        }
        let frames = (pcm.samples.len() - at) / usize::from(pcm.channels.max(1));
        if frames > 0 {
            self.last_frames = frames;
        }
        Ok(())
    }
}
