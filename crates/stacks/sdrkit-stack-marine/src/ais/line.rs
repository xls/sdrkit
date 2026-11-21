//! The AIS line around an HDLC frame: the training sequence before the start flag, and the ramp bits after the end flag.
//!
//! Standard: ITU-R M.1371-5 Annex 2, 3.2.2

use sdrkit_proto_link::hdlc::{Line, Receiver, Transmitter};
use sdrkit_util_bits::pack::{pack, BitOrder, Pad};

/// Training bits before the start flag (alternating levels).
pub const TRAINING_BITS: usize = 24;
/// The shortest frame kept, bytes without the FCS.
pub const MIN_FRAME_BYTES: usize = 5;
/// The longest frame kept, bytes without the FCS (five slots and more).
pub const MAX_FRAME_BYTES: usize = 150;

/// Makes the line levels of bursts. It keeps its buffers, so it allocates nothing once they have grown.
#[derive(Clone, Debug)]
pub struct LineCoder {
    tx: Transmitter,
    bytes: Vec<u8>,
}

impl Default for LineCoder {
    fn default() -> Self {
        LineCoder {
            tx: Transmitter::new(Line::Nrzi),
            bytes: Vec::with_capacity(MAX_FRAME_BYTES),
        }
    }
}

impl LineCoder {
    /// Replaces `out` with the line levels (0 or 1) of one burst. `payload` holds the message bits in message-table order, one per
    /// byte. A payload that is not whole octets is padded with zeros. The levels are the 24 training bits, then the start flag,
    /// the payload and its FCS (CRC-16/IBM-SDLC, octets least significant bit first, zero-bit insertion) and the end flag, all
    /// NRZI coded, then `ramp` bits that hold the last level.
    pub fn levels(&mut self, payload: &[u8], ramp: usize, out: &mut Vec<u8>) {
        out.clear();
        out.extend((0..TRAINING_BITS).map(|i| (i & 1) as u8));
        // Message bits packed most significant bit first go out least significant bit first, as M.1371 sends each octet.
        self.bytes.clear();
        pack(payload, BitOrder::Msb, Pad::Zero, &mut self.bytes);
        let start = out.len();
        self.tx.encode(&[&self.bytes], 1, out);
        // The level after the training sequence is 1, so the start flag's first 0 makes the first level 0.
        if out.get(start) == Some(&1) {
            out[start..].iter_mut().for_each(|l| *l ^= 1);
        }
        let last = out.last().copied().unwrap_or(0);
        out.extend(std::iter::repeat_n(last, ramp));
    }
}

/// The line levels of one burst (see [`LineCoder::levels`]).
pub fn burst_levels(payload: &[u8], ramp: usize) -> Vec<u8> {
    let mut out = Vec::new();
    LineCoder::default().levels(payload, ramp, &mut out);
    out
}

/// The HDLC receiver of AIS line levels: NRZI decoding, flags, zero-bit removal and the 16-bit FCS, keeping frames of
/// [`MIN_FRAME_BYTES`] to [`MAX_FRAME_BYTES`].
pub fn receiver() -> Receiver {
    Receiver::new(Line::Nrzi, MIN_FRAME_BYTES, MAX_FRAME_BYTES)
        .expect("the AIS frame lengths are in order")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
    use sdrkit_util_bits::pack::unpack;

    fn frames(levels: &[u8]) -> (Vec<Vec<u8>>, u64) {
        let mut rx = receiver();
        let mut got = Vec::new();
        rx.push_bits(levels, |f| {
            let mut bits = Vec::new();
            unpack(f, BitOrder::Msb, &mut bits);
            got.push(bits);
        });
        (got, rx.stats().bad_fcs)
    }

    #[test]
    fn the_fcs_is_crc16_ibm_sdlc() {
        // The catalogue check value of "123456789" (ISO/IEC 13239 FCS-16).
        assert_eq!(CRC16_IBM_SDLC.checksum(b"123456789"), 0x906E);
    }

    #[test]
    fn a_frame_is_found_after_any_lead_in_and_a_damaged_one_is_counted() {
        let payload: Vec<u8> = (0..168).map(|i| ((i * 5 + 1) % 3 == 0) as u8).collect();
        let levels = burst_levels(&payload, 8);
        assert_eq!(&levels[..TRAINING_BITS], &[0, 1].repeat(12)[..]);
        for lead in 0..12 {
            let mut lv: Vec<u8> = (0..lead).map(|i| (i % 3 == 0) as u8).collect();
            lv.extend_from_slice(&levels);
            assert_eq!(frames(&lv).0, vec![payload.clone()], "lead {lead}");
        }
        let mut lv = levels.clone();
        lv[100] ^= 1;
        let (good, bad) = frames(&lv);
        assert!(good.is_empty());
        assert_eq!(bad, 1);
    }

    #[test]
    fn a_short_payload_is_padded_to_whole_octets() {
        let payload = [1u8, 0, 1, 1, 0, 0, 1, 0, 1, 1].repeat(5);
        let (good, _) = frames(&burst_levels(&payload, 0));
        let mut want = payload.clone();
        want.resize(56, 0);
        assert_eq!(good, vec![want]);
    }
}
