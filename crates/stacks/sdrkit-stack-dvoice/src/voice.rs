//! Speech payloads: the de-interleaving of the channel bits of each vocoder into the code-word order that the codecs take,
//! and the speech frame builder.

use sdrkit_algo_fec::voice::{VoiceLayout, AMBE_HALF_RATE_WORDS};
use sdrkit_core::media::Codec;
use sdrkit_graph::media::{SpeechChannel, SpeechPacket};
use sdrkit_graph::PacketPool;
use std::sync::LazyLock;

/// The AMBE+2 half-rate layout shared by DMR, NXDN, dPMR and P25 Phase 2.
static AMB2: LazyLock<VoiceLayout> = LazyLock::new(VoiceLayout::ambe_half_rate);

/// The 72 channel bits of an AMBE+2 frame, de-interleaved and packed as four 24-bit rows (12 bytes, `codewords`): code word
/// Ck starts at bit 24 k, its first bit most significant. A slice shorter than 72 bits returns zeros.
pub fn amb2_codewords(bits: &[u8]) -> [u8; 12] {
    let mut out = [0u8; 12];
    let mut words = [0u8; 72];
    if bits.len() < 72 || AMB2.split(&bits[..72], &mut words).is_err() {
        return out;
    }
    let mut start = 0;
    for (r, &len) in AMBE_HALF_RATE_WORDS.iter().enumerate() {
        for j in 0..len {
            if words[start + j] & 1 != 0 {
                let bit = 24 * r + j;
                out[bit / 8] |= 0x80 >> (bit % 8);
            }
        }
        start += len;
    }
    out
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
    fn amb2_codewords_pack_the_shared_layout() {
        let bits: [u8; 72] = std::array::from_fn(|i| ((i * 5 + 1) % 3 == 0) as u8);
        let cw = amb2_codewords(&bits);
        let mut words = [0u8; 72];
        AMB2.split(&bits, &mut words).unwrap();
        // C0 is the first 24 bits, C1 starts at bit 24 of the rows and word bit 24.
        let row_bit = |b: usize| (cw[b / 8] >> (7 - b % 8)) & 1;
        assert!((0..24).all(|j| row_bit(j) == words[j]));
        assert!((0..23).all(|j| row_bit(24 + j) == words[24 + j]));
        assert_eq!(row_bit(47), 0);
        assert_eq!(amb2_codewords(&bits[..71]), [0; 12]);
    }

    #[test]
    fn imbe_round_trips() {
        let bits: [u8; 144] = std::array::from_fn(|i| ((i * 7 + 3) % 5 < 2) as u8);
        assert_eq!(crate::bits::bytes_to_bits(&channel_bits::<18>(&bits)), bits);
    }
}
