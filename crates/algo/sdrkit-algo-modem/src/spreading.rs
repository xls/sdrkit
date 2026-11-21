//! Spreading codes of the 3G CDMA air interfaces: Walsh functions and OVSF channelisation codes, the WCDMA downlink scrambling
//! codes and synchronisation codes, and the cdma2000 short pilot PN sequences.
//!
//! Chips are `±1` values (`i8`). A binary 0 maps to `+1` and a binary 1 to `-1`, as every standard here writes it.
//!
//! | Item | What it is | Standard |
//! |---|---|---|
//! | [`hadamard`], [`walsh`] | row `n` of the Sylvester Hadamard matrix: chip `k` is `(-1)^popcount(n AND k)` | 3GPP2 C.S0002-C 2.1.3.1.8.2 |
//! | [`ovsf`] | code `k` of spreading factor `SF` in the OVSF tree: the Hadamard row `bitreverse(k)` | 3GPP TS 25.213 4.3.1 |
//! | [`wcdma::DlScrambling`] | the degree-18 Gold codes of the WCDMA downlink, 38 400 chips a frame | TS 25.213 5.2.2 |
//! | [`wcdma::psc`], [`wcdma::ssc`], [`wcdma::SSC_GROUPS`] | the primary and the 16 secondary synchronisation codes, and the comma-free allocation of the SSCs to the 64 code groups | TS 25.213 5.2.3.1, table 4 |
//! | [`cdma2000::short_pn`] | the I and Q pilot PN sequences of 32 768 chips with the inserted zero | C.S0002-C 3.1.3.1.16 |
//!
//! ```
//! use sdrkit_algo_modem::spreading::{ovsf, walsh};
//! assert_eq!(walsh(1, 4), vec![1, -1, 1, -1]);
//! // C(4, 1) is the second code of the tree: + + - -.
//! assert_eq!(ovsf(4, 1), vec![1, 1, -1, -1]);
//! ```

/// Chip `k` of row `n` of the Sylvester Hadamard matrix: `(-1)^popcount(n AND k)`.
#[inline]
pub fn hadamard(n: usize, k: usize) -> i8 {
    if (n & k).count_ones().is_multiple_of(2) {
        1
    } else {
        -1
    }
}

/// The Walsh function `n` of length `len` (a power of two): row `n` of the Hadamard matrix of order `len`.
pub fn walsh(n: usize, len: usize) -> Vec<i8> {
    (0..len).map(|k| hadamard(n, k)).collect()
}

/// The OVSF code `C(sf, k)`: the tree `C(2n, 2k) = <C(n,k), C(n,k)>`, `C(2n, 2k+1) = <C(n,k), -C(n,k)>`, which is the Hadamard
/// row whose index is `k` with its `log2(sf)` bits reversed. `sf` is a power of two and `k < sf`.
pub fn ovsf(sf: usize, k: usize) -> Vec<i8> {
    let bits = sf.trailing_zeros();
    let row = if bits == 0 {
        0
    } else {
        k.reverse_bits() >> (usize::BITS - bits)
    };
    walsh(row, sf)
}

/// The WCDMA FDD downlink codes (3GPP TS 25.213).
pub mod wcdma {
    use std::sync::OnceLock;

    /// Chips of a radio frame.
    pub const FRAME_CHIPS: usize = 38_400;
    /// Chips of a slot.
    pub const SLOT_CHIPS: usize = 2560;
    /// The length of the synchronisation codes, chips.
    pub const SYNC_CHIPS: usize = 256;
    /// The period of the Gold code generators, `2^18 - 1`.
    const GOLD_PERIOD: usize = (1 << 18) - 1;
    /// The offset of the Q branch of a scrambling code.
    const Q_SHIFT: usize = 131_072;

    /// The sequence `a` of clause 5.2.3.1.
    const A: [i8; 16] = [1, 1, 1, 1, 1, 1, -1, -1, 1, -1, 1, -1, 1, -1, -1, 1];
    /// The signs of the 16 blocks of the PSC.
    const PSC_SIGNS: [i8; 16] = [1, 1, 1, -1, -1, 1, -1, -1, 1, 1, 1, -1, 1, -1, 1, 1];
    /// The signs of the 16 blocks of `z`, the base of the SSCs.
    const Z_SIGNS: [i8; 16] = [1, 1, 1, -1, 1, 1, -1, -1, 1, -1, 1, -1, -1, -1, -1, -1];

    /// The sequence `a` of 16 chips. The PSC is `a` repeated with the block signs of clause 5.2.3.1, so a correlation with
    /// the PSC can run as a correlation with `a` followed by a sum of 16 blocks.
    pub fn psc_base() -> [i8; 16] {
        A
    }

    /// The block signs of the PSC: chip `16 b + i` of the PSC is `psc_block_signs()[b] * a[i]`.
    pub fn psc_block_signs() -> [i8; 16] {
        PSC_SIGNS
    }

    /// The real part of the primary synchronisation code; the code sent is `(1 + j)` times it.
    pub fn psc() -> [i8; SYNC_CHIPS] {
        let mut c = [0i8; SYNC_CHIPS];
        for (b, &s) in PSC_SIGNS.iter().enumerate() {
            for (i, &a) in A.iter().enumerate() {
                c[16 * b + i] = s * a;
            }
        }
        c
    }

    /// The real part of the secondary synchronisation code `k` (1 to 16); the code sent is `(1 + j)` times it.
    ///
    /// # Panics
    ///
    /// If `k` is not 1 to 16.
    pub fn ssc(k: usize) -> [i8; SYNC_CHIPS] {
        assert!((1..=16).contains(&k), "SSC numbers are 1 to 16");
        let mut b = A;
        for v in &mut b[8..] {
            *v = -*v;
        }
        let m = 16 * (k - 1);
        let mut c = [0i8; SYNC_CHIPS];
        for (blk, &s) in Z_SIGNS.iter().enumerate() {
            for (i, &bv) in b.iter().enumerate() {
                let n = 16 * blk + i;
                c[n] = super::hadamard(m, n) * s * bv;
            }
        }
        c
    }

    /// The SSC number (1 to 16) sent in slot `s` (0 to 14) of a frame by a cell of scrambling code group `g` (0 to 63):
    /// TS 25.213 table 4. The 64 sequences are comma-free, so a cyclic shift tells the group and the frame start together.
    pub const SSC_GROUPS: [[u8; 15]; 64] = [
        [1, 1, 2, 8, 9, 10, 15, 8, 10, 16, 2, 7, 15, 7, 16],
        [1, 1, 5, 16, 7, 3, 14, 16, 3, 10, 5, 12, 14, 12, 10],
        [1, 2, 1, 15, 5, 5, 12, 16, 6, 11, 2, 16, 11, 15, 12],
        [1, 2, 3, 1, 8, 6, 5, 2, 5, 8, 4, 4, 6, 3, 7],
        [1, 2, 16, 6, 6, 11, 15, 5, 12, 1, 15, 12, 16, 11, 2],
        [1, 3, 4, 7, 4, 1, 5, 5, 3, 6, 2, 8, 7, 6, 8],
        [1, 4, 11, 3, 4, 10, 9, 2, 11, 2, 10, 12, 12, 9, 3],
        [1, 5, 6, 6, 14, 9, 10, 2, 13, 9, 2, 5, 14, 1, 13],
        [1, 6, 10, 10, 4, 11, 7, 13, 16, 11, 13, 6, 4, 1, 16],
        [1, 6, 13, 2, 14, 2, 6, 5, 5, 13, 10, 9, 1, 14, 10],
        [1, 7, 8, 5, 7, 2, 4, 3, 8, 3, 2, 6, 6, 4, 5],
        [1, 7, 10, 9, 16, 7, 9, 15, 1, 8, 16, 8, 15, 2, 2],
        [1, 8, 12, 9, 9, 4, 13, 16, 5, 1, 13, 5, 12, 4, 8],
        [1, 8, 14, 10, 14, 1, 15, 15, 8, 5, 11, 4, 10, 5, 4],
        [1, 9, 2, 15, 15, 16, 10, 7, 8, 1, 10, 8, 2, 16, 9],
        [1, 9, 15, 6, 16, 2, 13, 14, 10, 11, 7, 4, 5, 12, 3],
        [1, 10, 9, 11, 15, 7, 6, 4, 16, 5, 2, 12, 13, 3, 14],
        [1, 11, 14, 4, 13, 2, 9, 10, 12, 16, 8, 5, 3, 15, 6],
        [1, 12, 12, 13, 14, 7, 2, 8, 14, 2, 1, 13, 11, 8, 11],
        [1, 12, 15, 5, 4, 14, 3, 16, 7, 8, 6, 2, 10, 11, 13],
        [1, 15, 4, 3, 7, 6, 10, 13, 12, 5, 14, 16, 8, 2, 11],
        [1, 16, 3, 12, 11, 9, 13, 5, 8, 2, 14, 7, 4, 10, 15],
        [2, 2, 5, 10, 16, 11, 3, 10, 11, 8, 5, 13, 3, 13, 8],
        [2, 2, 12, 3, 15, 5, 8, 3, 5, 14, 12, 9, 8, 9, 14],
        [2, 3, 6, 16, 12, 16, 3, 13, 13, 6, 7, 9, 2, 12, 7],
        [2, 3, 8, 2, 9, 15, 14, 3, 14, 9, 5, 5, 15, 8, 12],
        [2, 4, 7, 9, 5, 4, 9, 11, 2, 14, 5, 14, 11, 16, 16],
        [2, 4, 13, 12, 12, 7, 15, 10, 5, 2, 15, 5, 13, 7, 4],
        [2, 5, 9, 9, 3, 12, 8, 14, 15, 12, 14, 5, 3, 2, 15],
        [2, 5, 11, 7, 2, 11, 9, 4, 16, 7, 16, 9, 14, 14, 4],
        [2, 6, 2, 13, 3, 3, 12, 9, 7, 16, 6, 9, 16, 13, 12],
        [2, 6, 9, 7, 7, 16, 13, 3, 12, 2, 13, 12, 9, 16, 6],
        [2, 7, 12, 15, 2, 12, 4, 10, 13, 15, 13, 4, 5, 5, 10],
        [2, 7, 14, 16, 5, 9, 2, 9, 16, 11, 11, 5, 7, 4, 14],
        [2, 8, 5, 12, 5, 2, 14, 14, 8, 15, 3, 9, 12, 15, 9],
        [2, 9, 13, 4, 2, 13, 8, 11, 6, 4, 6, 8, 15, 15, 11],
        [2, 10, 3, 2, 13, 16, 8, 10, 8, 13, 11, 11, 16, 3, 5],
        [2, 11, 15, 3, 11, 6, 14, 10, 15, 10, 6, 7, 7, 14, 3],
        [2, 16, 4, 5, 16, 14, 7, 11, 4, 11, 14, 9, 9, 7, 5],
        [3, 3, 4, 6, 11, 12, 13, 6, 12, 14, 4, 5, 13, 5, 14],
        [3, 3, 6, 5, 16, 9, 15, 5, 9, 10, 6, 4, 15, 4, 10],
        [3, 4, 5, 14, 4, 6, 12, 13, 5, 13, 6, 11, 11, 12, 14],
        [3, 4, 9, 16, 10, 4, 16, 15, 3, 5, 10, 5, 15, 6, 6],
        [3, 4, 16, 10, 5, 10, 4, 9, 9, 16, 15, 6, 3, 5, 15],
        [3, 5, 12, 11, 14, 5, 11, 13, 3, 6, 14, 6, 13, 4, 4],
        [3, 6, 4, 10, 6, 5, 9, 15, 4, 15, 5, 16, 16, 9, 10],
        [3, 7, 8, 8, 16, 11, 12, 4, 15, 11, 4, 7, 16, 3, 15],
        [3, 7, 16, 11, 4, 15, 3, 15, 11, 12, 12, 4, 7, 8, 16],
        [3, 8, 7, 15, 4, 8, 15, 12, 3, 16, 4, 16, 12, 11, 11],
        [3, 8, 15, 4, 16, 4, 8, 7, 7, 15, 12, 11, 3, 16, 12],
        [3, 10, 10, 15, 16, 5, 4, 6, 16, 4, 3, 15, 9, 6, 9],
        [3, 13, 11, 5, 4, 12, 4, 11, 6, 6, 5, 3, 14, 13, 12],
        [3, 14, 7, 9, 14, 10, 13, 8, 7, 8, 10, 4, 4, 13, 9],
        [5, 5, 8, 14, 16, 13, 6, 14, 13, 7, 8, 15, 6, 15, 7],
        [5, 6, 11, 7, 10, 8, 5, 8, 7, 12, 12, 10, 6, 9, 11],
        [5, 6, 13, 8, 13, 5, 7, 7, 6, 16, 14, 15, 8, 16, 15],
        [5, 7, 9, 10, 7, 11, 6, 12, 9, 12, 11, 8, 8, 6, 10],
        [5, 9, 6, 8, 10, 9, 8, 12, 5, 11, 10, 11, 12, 7, 7],
        [5, 10, 10, 12, 8, 11, 9, 7, 8, 9, 5, 12, 6, 7, 6],
        [5, 10, 12, 6, 5, 12, 8, 9, 7, 6, 7, 8, 11, 11, 9],
        [5, 13, 15, 15, 14, 8, 6, 7, 16, 8, 7, 13, 14, 5, 16],
        [9, 10, 13, 10, 11, 15, 15, 9, 16, 12, 14, 13, 16, 14, 11],
        [9, 11, 12, 15, 12, 9, 13, 13, 11, 14, 10, 16, 15, 14, 16],
        [9, 12, 10, 15, 13, 14, 9, 14, 15, 11, 11, 13, 12, 16, 10],
    ];

    /// The x and y sequences of the Gold codes over one period (bits).
    fn gold_xy() -> &'static (Vec<u8>, Vec<u8>) {
        static XY: OnceLock<(Vec<u8>, Vec<u8>)> = OnceLock::new();
        XY.get_or_init(|| {
            let mut x = vec![0u8; GOLD_PERIOD + 18];
            let mut y = vec![1u8; GOLD_PERIOD + 18];
            x[0] = 1;
            for i in 0..GOLD_PERIOD {
                x[i + 18] = x[i + 7] ^ x[i];
                y[i + 18] = y[i + 10] ^ y[i + 7] ^ y[i + 5] ^ y[i];
            }
            x.truncate(GOLD_PERIOD);
            y.truncate(GOLD_PERIOD);
            (x, y)
        })
    }

    /// The downlink scrambling codes (clause 5.2.2): `S(i) = Z_n(i) + j Z_n(i + 131 072)` for `i = 0 .. 38 399`, with
    /// `z_n(i) = x(i + n) + y(i)` modulo 2. The generators are built once and shared.
    #[derive(Clone, Copy, Debug)]
    pub struct DlScrambling;

    impl DlScrambling {
        /// Code number `n` (0 to 262 142) as `(I, Q)` chips of one frame. A primary scrambling code `i` (0 to 511) is
        /// `n = 16 i`.
        pub fn code(n: usize) -> Vec<(i8, i8)> {
            let mut out = Vec::with_capacity(FRAME_CHIPS);
            Self::code_into(n, &mut out);
            out
        }

        /// As [`DlScrambling::code`], into `out` (cleared first).
        pub fn code_into(n: usize, out: &mut Vec<(i8, i8)>) {
            let (x, y) = gold_xy();
            let n = n % GOLD_PERIOD;
            out.clear();
            for i in 0..FRAME_CHIPS {
                let zi = x[(i + n) % GOLD_PERIOD] ^ y[i];
                let iq = i + Q_SHIFT;
                let zq = x[(iq + n) % GOLD_PERIOD] ^ y[iq];
                out.push((1 - 2 * zi as i8, 1 - 2 * zq as i8));
            }
        }

        /// The primary scrambling code `i` (0 to 511).
        pub fn primary(i: usize) -> Vec<(i8, i8)> {
            Self::code(16 * i)
        }
    }
}

/// The cdma2000 and IS-95 short pilot PN sequences (3GPP2 C.S0002-C 3.1.3.1.16).
pub mod cdma2000 {
    use std::sync::OnceLock;

    /// Chips of a pilot PN period.
    pub const PN_PERIOD: usize = 32_768;

    /// The recurrence delays of `P_I(x) = x^15 + x^13 + x^9 + x^8 + x^7 + x^5 + 1`.
    const I_TAPS: [usize; 6] = [15, 10, 8, 7, 6, 2];
    /// The recurrence delays of `P_Q(x) = x^15 + x^12 + x^11 + x^10 + x^6 + x^5 + x^4 + x^3 + 1`.
    const Q_TAPS: [usize; 8] = [15, 12, 11, 10, 9, 5, 4, 3];

    /// One period of the m-sequence of `taps` with a zero inserted after its run of 14 zeros, starting at the `1` that follows
    /// the run of 15 zeros (the zero-offset sequence).
    fn extended(taps: &[usize]) -> Vec<u8> {
        let m = PN_PERIOD - 1;
        let mut s = vec![0u8; m + 15];
        s[14] = 1;
        for n in 15..m + 15 {
            s[n] = taps.iter().fold(0, |a, &d| a ^ s[n - d]);
        }
        let seq = &s[15..];
        // The run of 14 zeros ends just before position `end`; the inserted zero makes it 15.
        let mut run = 0;
        let mut end = 0;
        for k in 0..2 * m {
            if seq[k % m] == 0 {
                run += 1;
                if run == 14 {
                    end = (k + 1) % m;
                    break;
                }
            } else {
                run = 0;
            }
        }
        let mut out = Vec::with_capacity(PN_PERIOD);
        for k in 0..m {
            out.push(seq[(end + k) % m]);
        }
        out.push(0);
        out
    }

    /// The I (`q = false`) or Q (`q = true`) pilot PN sequence of 32 768 bits, chip 0 first. Built once and shared.
    pub fn short_pn(q: bool) -> &'static [u8] {
        static I: OnceLock<Vec<u8>> = OnceLock::new();
        static Q: OnceLock<Vec<u8>> = OnceLock::new();
        if q {
            Q.get_or_init(|| extended(&Q_TAPS))
        } else {
            I.get_or_init(|| extended(&I_TAPS))
        }
    }

    /// The complex pilot PN chip `n` (taken modulo the period): `PN_I + j PN_Q` with bit 0 as `+1`.
    #[inline]
    pub fn chip(n: usize) -> (i8, i8) {
        let k = n % PN_PERIOD;
        (
            1 - 2 * short_pn(false)[k] as i8,
            1 - 2 * short_pn(true)[k] as i8,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chips as hex, bit 1 for chip -1, first chip in the most significant place.
    fn hex(c: impl Iterator<Item = i8>) -> String {
        let bits: Vec<u8> = c.map(|v| u8::from(v < 0)).collect();
        bits.chunks(4)
            .map(|q| format!("{:X}", q.iter().fold(0, |a, &b| a << 1 | b)))
            .collect()
    }

    #[test]
    fn wcdma_codes_match_the_standard() {
        assert_eq!(
            hex(wcdma::psc().into_iter()),
            "035603560356FCA9FCA90356FCA9FCA9035603560356FCA90356FCA903560356"
        );
        assert_eq!(
            hex(wcdma::ssc(1).into_iter()),
            "03A903A903A9FC5603A903A9FC56FC5603A9FC5603A9FC56FC56FC56FC56FC56"
        );
        assert_eq!(
            hex(wcdma::ssc(16).into_iter()),
            "03A9FC56FC56FC56FC5603A9FC5603A9FC56FC5603A903A9FC5603A903A9FC56"
        );
        for (i, want_i, want_q) in [
            (0, "7FFFE03D", "05575E1F"),
            (1, "DFFBC8B9", "105DFA09"),
            (511, "C7689CF7", "179A8317"),
        ] {
            let c = wcdma::DlScrambling::primary(i);
            assert_eq!(hex(c[..32].iter().map(|v| v.0)), want_i, "{i}");
            // The Q branch of the first chips is Z_n(131 072 ..).
            assert_eq!(hex(c[..32].iter().map(|v| v.1)), want_q, "{i}");
        }
    }

    #[test]
    fn pilot_pn_matches_the_standard() {
        for (q, head, tail) in [
            (false, "A93A37990784695A", "B3D08000"),
            (true, "9EBAD38A738D8E98", "8F9C8000"),
        ] {
            let s = cdma2000::short_pn(q);
            assert_eq!(s.len(), 32_768);
            let chips = |r: std::ops::Range<usize>| hex(s[r].iter().map(|&b| 1 - 2 * b as i8));
            assert_eq!(chips(0..64), head);
            assert_eq!(chips(32_736..32_768), tail);
            assert_eq!(s.iter().filter(|&&b| b == 1).count(), 16_384);
        }
    }

    #[test]
    fn ovsf_is_the_tree() {
        assert_eq!(ovsf(256, 0), vec![1; 256]);
        let c1 = ovsf(256, 1);
        assert!(c1[..128].iter().all(|&v| v == 1) && c1[128..].iter().all(|&v| v == -1));
        assert_eq!(ovsf(16, 2), [vec![1; 4], vec![-1; 4], vec![1; 4], vec![-1; 4]].concat());
    }
}
