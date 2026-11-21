//! The EDACS outbound frame: 48 sync bits, then messages A and B, each 28 data bits and 12 BCH check bits, sent three times
//! with the middle copy inverted, and the vote of the copies. The BCH (40,28) code is the shared one of `sdrkit-algo-fec`.

use sdrkit_algo_fec::small::SmallCode;
use std::sync::OnceLock;

/// The 48 sync bits.
pub const SYNC: u64 = 0x5555_5712_5555;

/// The EDACS message code: the BCH (63,51) code with generator `0x1539` shortened to (40,28).
pub fn code() -> &'static SmallCode {
    static C: OnceLock<SmallCode> = OnceLock::new();
    C.get_or_init(|| {
        SmallCode::bch_63_51()
            .shortened(23)
            .expect("BCH (63,51) shortens to (40,28)")
    })
}

/// The 12 check bits of 28 data bits: (data × x^12) mod g.
pub fn check(data: u32) -> u32 {
    (code().encode((data & 0xFFF_FFFF) as u128) & 0xFFF) as u32
}

/// The 40-bit message (data then check).
pub fn encode(data: u32) -> u64 {
    code().encode((data & 0xFFF_FFFF) as u128) as u64
}

/// Whether a 40-bit message passes its check.
pub fn valid(m: u64) -> bool {
    code().syndrome(m as u128) == 0
}

/// The 288 bits of a frame carrying `a` and `b` (28-bit data words).
pub fn frame_bits(a: u32, b: u32) -> Vec<bool> {
    let mut bits = Vec::with_capacity(288);
    for i in (0..48).rev() {
        bits.push((SYNC >> i) & 1 == 1);
    }
    for m in [encode(a), encode(b)] {
        for copy in 0..3 {
            for i in (0..40).rev() {
                let bit = (m >> i) & 1 == 1;
                bits.push(if copy == 1 { !bit } else { bit });
            }
        }
    }
    bits
}

/// Votes three received copies (the middle one inverted): (the 40-bit message, disagreements).
pub fn vote(c1: u64, c2: u64, c3: u64) -> (u64, u32) {
    let c2 = !c2 & ((1 << 40) - 1);
    let v = (c1 & c2) | (c1 & c3) | (c2 & c3);
    let all_agree = !(c1 ^ c2) & !(c1 ^ c3) & ((1 << 40) - 1);
    (v, 40 - all_agree.count_ones())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d1_oracle() {
        for (d, c) in [
            (0x0000001, 0x539),
            (0x0000002, 0xA72),
            (0x0000004, 0x1DD),
            (0x0000008, 0x3BA),
            (0x0800000, 0xC5B),
            (0x8000000, 0x7D7),
            (0xF77F2D9, 0xB26),
            (0xD369FFC, 0xDED),
            (0x7CE9242, 0x393),
        ] {
            assert_eq!(check(d), c, "{d:07X}");
        }
        let m = encode(0xABCDEF1);
        assert!(valid(m));
        for i in 0..40 {
            assert!(!valid(m ^ (1 << i)));
        }
    }

    fn word(bits: &[bool]) -> u64 {
        bits.iter().fold(0, |a, &b| (a << 1) | b as u64)
    }

    #[test]
    fn d2_d3_frame_and_vote() {
        let f = frame_bits(0xABCDEF1, 0x1234567);
        assert_eq!(f.len(), 288);
        assert_eq!(word(&f[..48]), 0x5555_5712_5555);
        let (c1, c2, c3) = (word(&f[48..88]), word(&f[88..128]), word(&f[128..168]));
        let (v, d) = vote(c1, c2, c3);
        assert_eq!((v >> 12, d), (0xABCDEF1, 0));
        let (b, _) = vote(word(&f[168..208]), word(&f[208..248]), word(&f[248..288]));
        assert_eq!(b >> 12, 0x1234567);
        let (v, d) = vote(c1 ^ 1, c2 ^ 2, c3 ^ 4);
        assert_eq!((v >> 12, d), (0xABCDEF1, 3));
        let (v, _) = vote(c1 ^ (1 << 20), c2 ^ (1 << 20), c3);
        assert_ne!(v >> 12, 0xABCDEF1);
    }
}
