//! The error-control codes, the bit modulation and the interleaving of both frames.

use crate::tables::{full, half, GOLAY24_EXTRA, GOLAY_PARITY, HAMMING_PARITY};
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_algo_fec::voice::VoiceLayout;
use std::sync::OnceLock;

/// Code word lengths c0..c7 of the full-rate frame.
pub const FULL_WORDS: [usize; 8] = [23, 23, 23, 23, 15, 15, 15, 7];
/// Information lengths u0..u7 of the full-rate frame.
pub const FULL_INFO: [usize; 8] = [12, 12, 12, 12, 11, 11, 11, 7];
/// Code word lengths c0..c3 of the half-rate frame.
pub const HALF_WORDS: [usize; 4] = [24, 23, 11, 14];
/// Information lengths u0..u3 of the half-rate frame.
pub const HALF_INFO: [usize; 4] = [12, 12, 11, 14];

/// The codes and layouts, built once.
pub struct Codes {
    /// [23,12] Golay code of g_G.
    pub golay23: SmallCode,
    /// [24,12] extended Golay code.
    pub golay24: SmallCode,
    /// [15,11] Hamming code of g_H.
    pub hamming: SmallCode,
    /// Full-rate interleaving (144 channel bits into c0..c7).
    pub full_layout: VoiceLayout,
    /// Half-rate interleaving (72 channel bits into c0..c3).
    pub half_layout: VoiceLayout,
}

/// The shared codes and layouts.
pub fn codes() -> &'static Codes {
    static CODES: OnceLock<Codes> = OnceLock::new();
    CODES.get_or_init(|| {
        let g24: [u32; 12] = std::array::from_fn(|i| GOLAY_PARITY[i] << 1 | GOLAY24_EXTRA[i]);
        Codes {
            golay23: SmallCode::from_parity_rows(23, 12, &GOLAY_PARITY).expect("printed Golay rows"),
            golay24: SmallCode::from_parity_rows(24, 12, &g24).expect("printed Golay rows"),
            hamming: SmallCode::from_parity_rows(15, 11, &HAMMING_PARITY).expect("printed Hamming rows"),
            full_layout: layout(&full::INTERLEAVE, &FULL_WORDS),
            half_layout: layout(&half::INTERLEAVE, &HALF_WORDS),
        }
    })
}

/// The layout of a dibit interleaving table: channel bit `2 s` is the first (high) bit of symbol `s`, `2 s + 1` the second.
fn layout(table: &[[(u8, u8); 2]], lens: &[usize]) -> VoiceLayout {
    let mut start = [0usize; 8];
    for k in 1..lens.len() {
        start[k] = start[k - 1] + lens[k - 1];
    }
    let mut dest = [0u16; 144];
    for (s, pair) in table.iter().enumerate() {
        for (h, &(w, b)) in pair.iter().enumerate() {
            let w = w as usize;
            dest[2 * s + h] = (start[w] + lens[w] - 1 - b as usize) as u16;
        }
    }
    VoiceLayout::new(&dest[..2 * table.len()], lens).expect("printed interleaving is a permutation")
}

/// The modulation bits of Equations 84 to 93: `out[n - 1]` is the top bit of `p_r(n)` for `p_r(0) = 16 u0` and
/// `p_r(n) = (173 p_r(n - 1) + 13849) mod 65536`.
pub fn modulation(u0: u32, out: &mut [u8]) {
    let mut p = 16 * (u0 & 0xFFF);
    for o in out.iter_mut() {
        p = (173 * p + 13849) % 65536;
        *o = (p >> 15) as u8;
    }
}

/// The bits `bits` (one per byte, first most significant) as a number.
pub fn word_of(bits: &[u8]) -> u32 {
    bits.iter().fold(0u32, |v, &b| (v << 1) | (b & 1) as u32)
}

/// Writes the low `out.len()` bits of `v` into `out`, most significant first.
pub fn put_word(v: u32, out: &mut [u8]) {
    let n = out.len();
    for (i, o) in out.iter_mut().enumerate() {
        *o = (v >> (n - 1 - i) & 1) as u8;
    }
}

/// Decodes a word with `code`: the information bits and the number of bits corrected. A word beyond the code's radius
/// keeps its received information bits and counts as `radius + 1` errors.
pub fn decode_word(code: &SmallCode, word: u32) -> (u32, u8) {
    match code.decode(word as u128) {
        Some((cw, e)) => (code.info(cw) as u32, e),
        None => (code.info(word as u128) as u32, code.radius() as u8 + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printed_golay_rows_are_the_cyclic_code() {
        let c = codes();
        assert_eq!(c.golay23.rows(), SmallCode::golay_23_12().rows());
        assert_eq!(c.golay24.rows(), SmallCode::golay_24_12().rows());
    }

    fn weight_min(code: &SmallCode, k: usize) -> u32 {
        (1..1u32 << k)
            .map(|i| code.encode(i as u128).count_ones())
            .min()
            .unwrap()
    }

    #[test]
    fn distances() {
        let c = codes();
        assert_eq!(weight_min(&c.golay23, 12), 7);
        assert_eq!(weight_min(&c.golay24, 12), 8);
        assert_eq!(weight_min(&c.hamming, 11), 3);
    }

    #[test]
    fn golay_corrects_three() {
        let c = codes();
        let mut x = 12345u32;
        for i in 0..4096u32 {
            let cw = c.golay23.encode(i as u128) as u32;
            for _ in 0..3 {
                x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let mut e = 0u32;
                let n = (x >> 28) % 4;
                let mut y = x;
                while e.count_ones() < n {
                    y = y.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    e |= 1 << ((y >> 16) % 23);
                }
                assert_eq!(decode_word(&c.golay23, cw ^ e), (i, n as u8));
            }
        }
    }

    #[test]
    fn golay24_detects_four() {
        let c = codes();
        let cw = c.golay24.encode(0xA5C) as u32;
        assert_eq!(decode_word(&c.golay24, cw ^ 0b111), (0xA5C, 3));
        assert_eq!(decode_word(&c.golay24, cw ^ 0x80_0007).1, 4);
    }

    #[test]
    fn hamming_corrects_one() {
        let c = codes();
        for i in 0..2048u32 {
            let cw = c.hamming.encode(i as u128) as u32;
            assert_eq!(decode_word(&c.hamming, cw), (i, 0));
            for b in 0..15 {
                assert_eq!(decode_word(&c.hamming, cw ^ 1 << b), (i, 1));
            }
        }
    }

    #[test]
    fn modulation_start() {
        let mut m = [0u8; 3];
        modulation(0, &mut m);
        assert_eq!(m, [0, 1, 0]);
    }

    #[test]
    fn interleaving_is_a_permutation_and_spreads_words() {
        let c = codes();
        assert_eq!(c.full_layout.len(), 144);
        assert_eq!(c.half_layout.len(), 72);
        let chan: Vec<u8> = (0..144).map(|i| (i % 3 == 0) as u8).collect();
        let mut w = [0u8; 144];
        let mut back = [0u8; 144];
        c.full_layout.split(&chan, &mut w).unwrap();
        c.full_layout.join(&w, &mut back).unwrap();
        assert_eq!(&back[..], &chan[..]);
        // Bits of one full-rate code word are at least 3 symbols apart.
        for (j, _) in FULL_WORDS.iter().enumerate() {
            let mut syms: Vec<usize> = (0..72)
                .filter(|&s| full::INTERLEAVE[s].iter().any(|&(w, _)| w as usize == j))
                .collect();
            syms.sort();
            for p in syms.windows(2) {
                assert!(p[1] - p[0] >= 3, "word {j}");
            }
            let in_one = (0..72).any(|s| full::INTERLEAVE[s][0].0 == full::INTERLEAVE[s][1].0 && full::INTERLEAVE[s][0].0 as usize == j);
            assert!(!in_one);
        }
    }
}
