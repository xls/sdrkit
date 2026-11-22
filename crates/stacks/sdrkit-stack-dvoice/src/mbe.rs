//! The decoders of the two multi-band excitation speech coders as codec plug-ins of the host's codec set: the half-rate
//! coder for frames of the `codewords` packaging (DMR, NXDN, dPMR, P25 Phase 2) and the full-rate coder for frames of the
//! `p25_144` packaging (P25 Phase 1). Each frame gives 160 samples at 8 kHz, mono. An erasure gives the comfort noise of
//! the decoder, so the sound keeps its timing.

use crate::voice::amb2_channel_bits;
use sdrkit_codec_mbe::{Coder, Decoder, FRAME};
use sdrkit_core::codec::{AudioCodec, CodecError, CodecFactory, CodecSet, Pcm};
use sdrkit_core::media::{Codec, MediaFormat};
use sdrkit_graph::SpeechPackaging;
use std::sync::Arc;

/// The rate of the decoded speech, Hz.
const RATE_HZ: u32 = 8000;

/// Registers the two decoders in `set`: [`Codec::Ambe2`] for the `codewords` packaging and [`Codec::Imbe`] for the
/// `p25_144` packaging.
pub fn register(set: &mut CodecSet) {
    set.register(
        Codec::Ambe2.fourcc(),
        Some(SpeechPackaging::Codewords.key()),
        factory(Coder::HalfRate),
    );
    set.register(
        Codec::Imbe.fourcc(),
        Some(SpeechPackaging::P25_144.key()),
        factory(Coder::FullRate),
    );
}

/// The codec of the frames of `coder`.
fn codec_of(coder: Coder) -> Codec {
    match coder {
        Coder::HalfRate => Codec::Ambe2,
        Coder::FullRate => Codec::Imbe,
    }
}

/// A factory of decoders of `coder` for streams of its codec.
fn factory(coder: Coder) -> CodecFactory {
    Arc::new(move |f: &MediaFormat| {
        (f.codec == codec_of(coder))
            .then(|| Box::new(SpeechDecoder::new(coder)) as Box<dyn AudioCodec>)
    })
}

/// One decoder state of a stream.
pub struct SpeechDecoder {
    dec: Decoder,
    bits: [u8; 144],
    out: [i16; FRAME],
}

impl SpeechDecoder {
    /// A decoder of `coder` in its initial state.
    pub fn new(coder: Coder) -> SpeechDecoder {
        SpeechDecoder {
            dec: Decoder::new(coder),
            bits: [0; 144],
            out: [0; FRAME],
        }
    }
}

impl AudioCodec for SpeechDecoder {
    /// Takes a half-rate frame as 12 bytes of code words ([`crate::voice::amb2_codewords`]) or a full-rate frame as the 18
    /// bytes of its channel bits as sent. A frame of another length is [`CodecError::Frame`].
    fn decode(&mut self, frame: &[u8], erasure: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
        let n = if erasure || frame.is_empty() {
            0
        } else {
            match self.dec.coder() {
                Coder::HalfRate => {
                    let cw: &[u8; 12] = frame.try_into().map_err(|_| CodecError::Frame)?;
                    self.bits[..72].copy_from_slice(&amb2_channel_bits(cw));
                    72
                }
                Coder::FullRate => {
                    if frame.len() != 18 {
                        return Err(CodecError::Frame);
                    }
                    for (i, b) in self.bits.iter_mut().enumerate() {
                        *b = (frame[i / 8] >> (7 - i % 8)) & 1;
                    }
                    144
                }
            }
        };
        // A frame of no bits is muted: the comfort noise of an erasure.
        self.dec.decode(&self.bits[..n], &mut self.out);
        pcm.rate_hz = RATE_HZ;
        pcm.channels = 1;
        pcm.samples.extend_from_slice(&self.out);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{amb2_codewords, channel_bits};
    use sdrkit_algo_dsp::goertzel::goertzel_power;
    use sdrkit_algo_fec::voice::{VocoderTestFrame, NXDN_EHR_SILENCE, NXDN_EHR_TONE};
    use sdrkit_codec_mbe::Encoder;

    /// The samples of `frames` frames of the test frame `t`, through the plug-in in the `codewords` packaging.
    fn decode_test_frame(t: &VocoderTestFrame, frames: usize) -> Vec<f32> {
        let bits: Vec<u8> = (0..72).map(|i| t.coded_bit(i)).collect();
        let cw = amb2_codewords(&bits);
        let mut d = SpeechDecoder::new(Coder::HalfRate);
        let mut pcm = Pcm::default();
        for _ in 0..frames {
            d.decode(&cw, false, &mut pcm).unwrap();
        }
        assert_eq!((pcm.rate_hz, pcm.channels), (8000, 1));
        pcm.samples.iter().map(|&s| f32::from(s)).collect()
    }

    /// The frequency, in 10 Hz steps from 200 to 3800 Hz, with the most power in `x`.
    fn peak_hz(x: &[f32]) -> f64 {
        (20..380)
            .map(|k| f64::from(k) * 10.0)
            .max_by(|&a, &b| {
                goertzel_power(x, a / 8000.0).total_cmp(&goertzel_power(x, b / 8000.0))
            })
            .unwrap()
    }

    #[test]
    fn the_nxdn_test_frames_decode_to_a_1031_hz_tone_and_to_silence() {
        let tone = decode_test_frame(&NXDN_EHR_TONE, 25);
        let f = peak_hz(&tone[800..]);
        assert!((f - 1031.0).abs() <= 20.0, "peak at {f} Hz");
        let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        let quiet = decode_test_frame(&NXDN_EHR_SILENCE, 25);
        assert!(rms(&quiet[800..]) < 0.05 * rms(&tone[800..]));
    }

    #[test]
    fn both_packagings_take_the_encoder_frames_and_an_erasure_keeps_the_timing() {
        let pcm_in: [i16; FRAME] =
            std::array::from_fn(|i| (8000.0 * (i as f64 * 0.3).sin()) as i16);
        for coder in Coder::ALL {
            let mut set = CodecSet::default();
            register(&mut set);
            let codec = codec_of(coder);
            let key = match coder {
                Coder::HalfRate => "codewords",
                Coder::FullRate => "p25_144",
            };
            let entry = set.find(codec.fourcc(), key).unwrap();
            let mut d =
                (entry.factory)(&MediaFormat::new(codec, sdrkit_core::media::Packaging::Raw))
                    .unwrap();
            let mut enc = Encoder::new(coder);
            let mut bits = [0u8; 144];
            let n = enc.encode(&pcm_in, &mut bits);
            let payload: Vec<u8> = match coder {
                Coder::HalfRate => amb2_codewords(&bits[..n]).to_vec(),
                Coder::FullRate => channel_bits::<18>(&bits[..n]).to_vec(),
            };
            let mut pcm = Pcm::default();
            d.decode(&payload, false, &mut pcm).unwrap();
            d.decode(&[], true, &mut pcm).unwrap();
            assert_eq!(pcm.samples.len(), 2 * FRAME);
            assert_eq!(
                d.decode(&payload[1..], false, &mut pcm),
                Err(CodecError::Frame)
            );
        }
    }
}
