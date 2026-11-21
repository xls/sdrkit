//! The Bauer (10,5) code of five-unit teleprinter characters: the five elements, then five check units that repeat the elements
//! when the character has an odd number of 1 elements and are their inverse when it has an even number. The 32 code words have a
//! minimum Hamming distance of 4, so one error is corrected and two are detected.
//!
//! A word holds unit 1 (sent first) in bit 0: the elements in bits 0 to 4 (element 1 in bit 0, mark as 1, as an ITA2 code), the
//! check units in bits 5 to 9.
//!
//! ```
//! use sdrkit_algo_fec::bauer;
//! assert_eq!(bauer::encode(3), 0b11100_00011); // A, even: the inverse
//! assert_eq!(bauer::decode(bauer::encode(21) ^ 0x40), (21, 1));
//! ```

/// The code word of five-unit character `code` (its low five bits).
pub const fn encode(code: u8) -> u16 {
    let c = (code & 31) as u16;
    let check = if c.count_ones() % 2 == 1 { c } else { !c & 31 };
    c | (check << 5)
}

/// Whether the check units of `word` match its elements.
pub fn consistent(word: u16) -> bool {
    encode((word & 31) as u8) == word & 0x3FF
}

/// The nearest code word of `word` (its low ten bits): the character and the Hamming distance to its code word. Of two code
/// words at the same distance, the one of the lower character is taken.
pub fn decode(word: u16) -> (u8, u32) {
    let w = word & 0x3FF;
    let mut best = (0u8, u32::MAX);
    for c in 0..32u8 {
        let d = (encode(c) ^ w).count_ones();
        if d < best.1 {
            best = (c, d);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The word as units 1 to 10.
    fn units(w: u16) -> [u8; 10] {
        std::array::from_fn(|i| ((w >> i) & 1) as u8)
    }

    #[test]
    fn worked_words() {
        assert_eq!(units(encode(3)), [1, 1, 0, 0, 0, 0, 0, 1, 1, 1]);
        assert_eq!(units(encode(1)), [1, 0, 0, 0, 0, 1, 0, 0, 0, 0]);
        assert_eq!(units(encode(4)), [0, 0, 1, 0, 0, 0, 0, 1, 0, 0]);
        assert_eq!(units(encode(21)), [1, 0, 1, 0, 1, 1, 0, 1, 0, 1]);
    }

    #[test]
    fn distance_four_and_single_errors_corrected() {
        let mut min = u32::MAX;
        for a in 0..32u8 {
            for b in (a + 1)..32 {
                min = min.min((encode(a) ^ encode(b)).count_ones());
            }
            assert!(consistent(encode(a)));
            for i in 0..10 {
                assert_eq!(decode(encode(a) ^ (1 << i)), (a, 1));
            }
        }
        assert_eq!(min, 4);
    }
}
