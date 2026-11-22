//! The `codewords` payload of a half-rate frame: its 72 channel bits de-interleaved into the code words c0..c3, each in a
//! 24-bit row. Stacks pack their frames this way; a decoder plug-in unpacks them for [`crate::Decoder::decode`].

use crate::fec::{codes, HALF_WORDS};

/// The 72 channel bits of a half-rate frame as sent, one bit per byte, de-interleaved and packed as the `codewords`
/// payload: four 24-bit rows (12 bytes), code word c_k from bit 24 k, its first bit most significant. A slice shorter
/// than 72 bits gives zeros.
pub fn codewords(bits: &[u8]) -> [u8; 12] {
    let mut out = [0u8; 12];
    let mut words = [0u8; 72];
    if bits.len() < 72 || codes().half_layout.split(&bits[..72], &mut words).is_err() {
        return out;
    }
    let mut start = 0;
    for (r, &len) in HALF_WORDS.iter().enumerate() {
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

/// The inverse of [`codewords`]: the 72 channel bits as sent, one bit per byte, of the `codewords` payload `cw`.
pub fn codeword_channel_bits(cw: &[u8; 12]) -> [u8; 72] {
    let mut words = [0u8; 72];
    let mut start = 0;
    for (r, &len) in HALF_WORDS.iter().enumerate() {
        for j in 0..len {
            let bit = 24 * r + j;
            words[start + j] = (cw[bit / 8] >> (7 - bit % 8)) & 1;
        }
        start += len;
    }
    let mut bits = [0u8; 72];
    codes()
        .half_layout
        .join(&words, &mut bits)
        .expect("the layout has 72 bits");
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codewords_pack_the_half_rate_layout() {
        let bits: [u8; 72] = std::array::from_fn(|i| ((i * 5 + 1) % 3 == 0) as u8);
        let cw = codewords(&bits);
        let mut words = [0u8; 72];
        codes().half_layout.split(&bits, &mut words).unwrap();
        // c0 is the first 24 bits; c1 starts at bit 24 of the rows and word bit 24.
        let row_bit = |b: usize| (cw[b / 8] >> (7 - b % 8)) & 1;
        assert!((0..24).all(|j| row_bit(j) == words[j]));
        assert!((0..23).all(|j| row_bit(24 + j) == words[24 + j]));
        assert_eq!(row_bit(47), 0);
        assert_eq!(codewords(&bits[..71]), [0; 12]);
        assert_eq!(codeword_channel_bits(&cw), bits);
    }

    #[test]
    fn the_half_rate_interleave_is_the_shared_layout() {
        let shared = sdrkit_algo_fec::voice::VoiceLayout::ambe_half_rate();
        let chan: [u16; 72] = std::array::from_fn(|i| i as u16);
        let (mut a, mut b) = ([0u16; 72], [0u16; 72]);
        codes().half_layout.split(&chan, &mut a).unwrap();
        shared.split(&chan, &mut b).unwrap();
        assert_eq!(a, b);
    }
}
