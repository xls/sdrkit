//! Speech payloads: the packing of channel bits as sent, the speech frame builder, and the coded test speech of the test
//! signals. The `codewords` packing of the half-rate coder is [`sdrkit_codec_mbe::codewords`].

use sdrkit_codec_mbe::{Coder, Encoder, FRAME};
use sdrkit_core::media::Codec;
use sdrkit_graph::media::{SpeechChannel, SpeechPacket};
use sdrkit_graph::PacketPool;
use std::sync::OnceLock;

/// The two pitches of the test speech, Hz: a sung vowel, one note of each pitch in turn.
pub const TEST_SPEECH_F0: [f64; 2] = [120.0, 160.0];
/// The 20 ms frames of one note of the test speech, the pause after it included.
pub const TEST_SPEECH_STEP: usize = 20;
/// The 20 ms frames of silence at the end of each note.
const TEST_SPEECH_PAUSE: usize = 3;
/// The rise and the fall of a note, samples.
const TEST_SPEECH_RAMP: usize = 80;

/// One period of the test speech at 8 kHz, twice over: two notes of a vowel with formants near 600 Hz and 1300 Hz, at the
/// two pitches of [`TEST_SPEECH_F0`], each with soft edges and a short pause. The pause lets the pitch search of the
/// encoder start again at each note, so that it does not keep a multiple of the period across the step.
fn test_vowel() -> Vec<i16> {
    let note = TEST_SPEECH_STEP * FRAME;
    let sound = (TEST_SPEECH_STEP - TEST_SPEECH_PAUSE) * FRAME;
    let envelope = |f: f64| {
        let g = |c: f64, w: f64| (-((f - c) / w).powi(2)).exp();
        g(600.0, 250.0) + 0.5 * g(1300.0, 300.0) + 0.07 * (-f / 1000.0).exp()
    };
    (0..2 * 2 * note)
        .map(|i| {
            let (k, n) = (i / note, i % note);
            if n >= sound {
                return 0;
            }
            let edge = n.min(sound - 1 - n);
            let gain = if edge < TEST_SPEECH_RAMP {
                0.5 - 0.5 * (std::f64::consts::PI * edge as f64 / TEST_SPEECH_RAMP as f64).cos()
            } else {
                1.0
            };
            let f0 = TEST_SPEECH_F0[k % 2];
            let t = n as f64 / 8000.0;
            let v: f64 = (1..)
                .map(|h| f0 * h as f64)
                .take_while(|&f| f < 3800.0)
                .map(|f| envelope(f) * (std::f64::consts::TAU * f * t).sin())
                .sum();
            (3000.0 * gain * v).round().clamp(-32767.0, 32767.0) as i16
        })
        .collect()
}

/// One period of the test speech through the encoder of `coder`: the channel bits of each frame as sent, or its speech
/// bits before the error correction when `speech` is set. Two periods are coded and the second is kept, so that the
/// state of the coder at the end of the period matches its start and the period repeats without a seam.
fn coded_test_speech<const N: usize>(coder: Coder, speech: bool) -> Vec<[u8; N]> {
    let pcm = test_vowel();
    let mut enc = Encoder::new(coder);
    let frames: Vec<[u8; N]> = pcm
        .as_chunks::<FRAME>()
        .0
        .iter()
        .map(|x| {
            let mut bits = [0u8; N];
            if speech {
                enc.encode_speech(x, &mut bits);
            } else {
                enc.encode(x, &mut bits);
            }
            bits
        })
        .collect();
    frames[frames.len() / 2..].to_vec()
}

/// The test speech as frames of the half-rate coder (72 channel bits each, as sent), coded once and kept. The voice frames
/// of the test signals carry them in turn.
pub fn test_speech_half() -> &'static [[u8; 72]] {
    static FRAMES: OnceLock<Vec<[u8; 72]>> = OnceLock::new();
    FRAMES.get_or_init(|| coded_test_speech(Coder::HalfRate, false))
}

/// The test speech as frames of the full-rate coder (144 channel bits each, as sent), coded once and kept.
pub fn test_speech_full() -> &'static [[u8; 144]] {
    static FRAMES: OnceLock<Vec<[u8; 144]>> = OnceLock::new();
    FRAMES.get_or_init(|| coded_test_speech(Coder::FullRate, false))
}

/// The test speech as the 49 speech bits of each half-rate frame (u0 to u3, most significant bit first), with no error
/// correction, coded once and kept.
pub fn test_speech_half_bits() -> &'static [[u8; 49]] {
    static FRAMES: OnceLock<Vec<[u8; 49]>> = OnceLock::new();
    FRAMES.get_or_init(|| coded_test_speech(Coder::HalfRate, true))
}

/// The packaging of the 72 channel bits of a D-STAR AMBE frame as they are sent (9 bytes, the first bit most significant).
/// The codec does the voice de-interleave.
pub const DSTAR_PACKAGING: sdrkit_graph::SpeechPackaging = sdrkit_graph::SpeechPackaging::Dstar72;

/// The packaging of the 144 channel bits of a P25 IMBE frame as they are sent (18 bytes, the first bit most significant).
/// The codec does the de-interleave. Standard: TIA-102.BABA.
pub const IMBE_PACKAGING: sdrkit_graph::SpeechPackaging = sdrkit_graph::SpeechPackaging::P25_144;

/// The first `8 N` channel bits as sent, packed most significant bit first (the payload of the `dstar72` and `p25_144`
/// packagings). Bits past the end of `bits` are zero.
pub fn channel_bits<const N: usize>(bits: &[u8]) -> [u8; N] {
    let mut out = [0u8; N];
    for (i, &b) in bits.iter().take(8 * N).enumerate() {
        if b & 1 != 0 {
            out[i / 8] |= 0x80 >> (i % 8);
        }
    }
    out
}

/// Where a speech frame comes from and what it is.
#[derive(Clone, Copy, Debug)]
pub struct FrameSpec {
    /// The speech codec.
    pub codec: Codec,
    /// The bit layout of the payload.
    pub packaging: sdrkit_graph::SpeechPackaging,
    /// The codec mode, for a codec with several modes.
    pub codec_mode: Option<sdrkit_graph::Codec2Mode>,
    /// The frame duration, milliseconds.
    pub duration_ms: u16,
    /// The stack id.
    pub stack: &'static str,
    /// The carrier frequency, Hz.
    pub carrier_hz: f64,
    /// The time slot.
    pub slot: u8,
}

/// A received speech frame of `spec` with the payload `data`, from a buffer of `pool`. `time` is the signal time of its first
/// symbol. A `damaged` frame failed its own check. `bit_errors` are the errors the stack found, when known.
pub fn frame(
    pool: &mut PacketPool,
    spec: FrameSpec,
    data: &[u8],
    time: f64,
    damaged: bool,
    bit_errors: Option<u16>,
) -> SpeechPacket {
    let packet = pool
        .make(data.len(), time, |b| b.copy_from_slice(data))
        .stamped(Some(pts(time)), None, true);
    let mut f = SpeechPacket::frame(
        spec.codec.fourcc(),
        spec.packaging,
        spec.duration_ms,
        packet,
    )
    .on(SpeechChannel {
        stack: spec.stack,
        carrier_hz: spec.carrier_hz,
        slot: spec.slot,
    });
    f.codec_mode = spec.codec_mode;
    f.damaged = damaged;
    f.bit_errors = bit_errors;
    f
}

/// An erasure of `spec`: the position of a frame with no frame, at signal time `time`.
pub fn erasure(spec: FrameSpec, time: f64) -> SpeechPacket {
    SpeechPacket::erasure(
        spec.codec.fourcc(),
        spec.packaging,
        spec.duration_ms,
        pts(time),
        time,
    )
    .on(SpeechChannel {
        stack: spec.stack,
        carrier_hz: spec.carrier_hz,
        slot: spec.slot,
    })
}

/// The time stamp of signal time `time`, in 8 kHz samples.
fn pts(time: f64) -> i64 {
    (time * 8000.0).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imbe_round_trips() {
        let bits: [u8; 144] = std::array::from_fn(|i| ((i * 7 + 3) % 5 < 2) as u8);
        assert_eq!(crate::bits::bytes_to_bits(&channel_bits::<18>(&bits)), bits);
    }

    #[test]
    fn the_test_speech_decodes_to_its_notes_through_both_coders_and_across_the_repeat() {
        fn check<const N: usize>(coder: Coder, frames: &[[u8; N]]) {
            assert_eq!(frames.len(), 2 * TEST_SPEECH_STEP);
            let mut dec = sdrkit_codec_mbe::Decoder::new(coder);
            let mut out = [0i16; FRAME];
            for k in 0..3 * frames.len() {
                dec.decode(&frames[k % frames.len()], &mut out);
                let (note, n) = (k / TEST_SPEECH_STEP % 2, k % TEST_SPEECH_STEP);
                if k >= TEST_SPEECH_STEP && (3..TEST_SPEECH_STEP - TEST_SPEECH_PAUSE - 1).contains(&n) {
                    let f0 = dec.params().f0_hz();
                    let want = TEST_SPEECH_F0[note];
                    assert!((f0 - want).abs() < 0.03 * want, "{coder:?} frame {k}: {f0} Hz for {want} Hz");
                }
            }
        }
        check(Coder::HalfRate, test_speech_half());
        check(Coder::FullRate, test_speech_full());
    }
}
