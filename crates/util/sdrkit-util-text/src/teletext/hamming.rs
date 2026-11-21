//! The teletext protection codes (ETS 300 706 clause 8): Hamming 8/4 (8.2), Hamming 24/18 (8.3) and odd parity (8.1).
//!
//! Bytes are in transmission order with the first bit sent in bit 0 (as they come off the line, or after reversing the bits of an
//! EN 300 472 data unit).

/// The Hamming 8/4 code words of the nibbles 0 to 15, built from the encoding equations of 8.2.
pub const HAMMING_8_4: [u8; 16] = {
    let mut t = [0u8; 16];
    let mut n = 0;
    while n < 16 {
        let (d1, d2, d3, d4) = (n & 1, (n >> 1) & 1, (n >> 2) & 1, (n >> 3) & 1);
        let p1 = 1 ^ d1 ^ d3 ^ d4;
        let p2 = 1 ^ d1 ^ d2 ^ d4;
        let p3 = 1 ^ d1 ^ d2 ^ d3;
        let p4 = 1 ^ p1 ^ d1 ^ p2 ^ d2 ^ p3 ^ d3 ^ d4;
        // Bit order 1..8: P1 D1 P2 D2 P3 D3 P4 D4.
        t[n as usize] = p1 | d1 << 1 | p2 << 2 | d2 << 3 | p3 << 4 | d3 << 5 | p4 << 6 | d4 << 7;
        n += 1;
    }
    t
};

/// Decode table: the nibble in bits 0 to 3, bit 4 set when a bit was corrected, 0xFF for a double error.
const DECODE_8_4: [u8; 256] = {
    let mut t = [0xFFu8; 256];
    let mut n = 0;
    while n < 16 {
        let c = HAMMING_8_4[n];
        t[c as usize] = n as u8;
        let mut b = 0;
        while b < 8 {
            t[(c ^ (1 << b)) as usize] = n as u8 | 0x10;
            b += 1;
        }
        n += 1;
    }
    t
};

/// The result of a Hamming decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded<T> {
    /// The data.
    pub data: T,
    /// A single bit error was corrected.
    pub corrected: bool,
}

/// Decodes a Hamming 8/4 byte; `None` for a double error.
pub fn decode_8_4(b: u8) -> Option<Decoded<u8>> {
    match DECODE_8_4[b as usize] {
        0xFF => None,
        v => Some(Decoded {
            data: v & 15,
            corrected: v & 0x10 != 0,
        }),
    }
}

/// The Hamming 8/4 byte of a nibble (the low four bits are used).
pub fn encode_8_4(n: u8) -> u8 {
    HAMMING_8_4[(n & 15) as usize]
}

/// Positions (1 to 24) of the data bits D1 to D18 in a triplet.
const DATA_POS: [u8; 18] = [
    3, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 17, 18, 19, 20, 21, 22, 23,
];

/// Encodes 18 data bits (D1 in bit 0) as a Hamming 24/18 triplet, first byte first.
pub fn encode_24_18(data: u32) -> [u8; 3] {
    let mut w = 0u32; // bit i - 1 holds position i
    for (k, &p) in DATA_POS.iter().enumerate() {
        w |= ((data >> k) & 1) << (p - 1);
    }
    // P1 to P5 at positions 1, 2, 4, 8, 16: odd parity over the positions whose index has that bit set (positions 1 to 23).
    for k in 0..5 {
        let mask = check_mask(k);
        let p = 1u32 << k;
        let par = 1 ^ ((w & mask).count_ones() & 1);
        w |= par << (p - 1);
    }
    // P6 at position 24: odd parity over all 24 bits.
    w |= (1 ^ (w.count_ones() & 1)) << 23;
    [w as u8, (w >> 8) as u8, (w >> 16) as u8]
}

/// The bits of positions 1 to 23 whose index has bit `k` set.
fn check_mask(k: u32) -> u32 {
    (1..24u32)
        .filter(|i| (i >> k) & 1 == 1)
        .fold(0, |m, i| m | 1 << (i - 1))
}

/// Decodes a Hamming 24/18 triplet; `None` for a double error (or a single error the syndrome places outside the word).
pub fn decode_24_18(b: [u8; 3]) -> Option<Decoded<u32>> {
    let mut w = b[0] as u32 | (b[1] as u32) << 8 | (b[2] as u32) << 16;
    let mut syndrome = 0u32;
    for k in 0..5 {
        // Tests A to E: odd parity over the bits of each check, its own parity bit included.
        if (w & check_mask(k)).count_ones() & 1 == 0 {
            syndrome |= 1 << k;
        }
    }
    let overall_ok = w.count_ones() & 1 == 1; // test F
    let corrected = match (syndrome, overall_ok) {
        (0, true) => false,
        (0, false) => true, // P6 itself
        (s, false) if s <= 23 => {
            w ^= 1 << (s - 1);
            true
        }
        _ => return None,
    };
    let mut data = 0u32;
    for (k, &p) in DATA_POS.iter().enumerate() {
        data |= ((w >> (p - 1)) & 1) << k;
    }
    Some(Decoded { data, corrected })
}

/// The 7-bit character of an odd-parity byte (`None` on a parity error).
pub fn parity(b: u8) -> Option<u8> {
    (b.count_ones() & 1 == 1).then_some(b & 0x7F)
}

/// A 7-bit character with its odd parity bit.
pub fn with_parity(c: u8) -> u8 {
    crate::ia5::add_parity(c, crate::ia5::Parity::Odd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hamming_8_4_known_words() {
        // The code words in transmission order (bit 0 first), as tabulated for ETS 300 706 8.2.
        assert_eq!(
            HAMMING_8_4,
            [
                0x15, 0x02, 0x49, 0x5E, 0x64, 0x73, 0x38, 0x2F, 0xD0, 0xC7, 0x8C, 0x9B, 0xA1, 0xB6,
                0xFD, 0xEA
            ]
        );
        for (n, &c) in HAMMING_8_4.iter().enumerate() {
            assert_eq!(
                decode_8_4(c),
                Some(Decoded {
                    data: n as u8,
                    corrected: false
                })
            );
            for b in 0..8 {
                assert_eq!(
                    decode_8_4(c ^ (1 << b)),
                    Some(Decoded {
                        data: n as u8,
                        corrected: true
                    })
                );
                for b2 in b + 1..8 {
                    assert_eq!(decode_8_4(c ^ (1 << b) ^ (1 << b2)), None);
                }
            }
        }
        // Every byte is a code word, one bit from one, or two bits from several.
        assert_eq!(
            (0..=255u8).filter(|&b| decode_8_4(b).is_some()).count(),
            16 * 9
        );
    }

    #[test]
    fn hamming_24_18_known_words() {
        // Data 0: P1 to P5 are 1 (odd parity of nothing), P6 makes the total odd: 0.
        assert_eq!(encode_24_18(0), [0x8B, 0x80, 0x00]);
        // Any word has odd overall parity.
        let w = encode_24_18(0x3FFFF);
        let all = w[0] as u32 | (w[1] as u32) << 8 | (w[2] as u32) << 16;
        assert_eq!(all.count_ones() % 2, 1);
        for &d in &[0u32, 1, 0x2AAAA, 0x15555, 0x3FFFF, 0x12345] {
            let w = encode_24_18(d);
            assert_eq!(
                decode_24_18(w),
                Some(Decoded {
                    data: d,
                    corrected: false
                })
            );
            for bit in 0..24 {
                let mut x = w;
                x[bit / 8] ^= 1 << (bit % 8);
                assert_eq!(
                    decode_24_18(x),
                    Some(Decoded {
                        data: d,
                        corrected: true
                    }),
                    "bit {bit}"
                );
                for bit2 in bit + 1..24 {
                    let mut y = x;
                    y[bit2 / 8] ^= 1 << (bit2 % 8);
                    assert_eq!(decode_24_18(y), None, "bits {bit} {bit2}");
                }
            }
        }
    }

    #[test]
    fn hamming_24_18_matches_the_8_3_equations() {
        // P1 = 1 + D1 + D2 + D4 + D5 + D7 + D9 + D11 + D12 + D14 + D16 + D18 (mod 2), and so on for P2 to P5.
        let eq: [&[u32]; 5] = [
            &[1, 2, 4, 5, 7, 9, 11, 12, 14, 16, 18],
            &[1, 3, 4, 6, 7, 10, 11, 13, 14, 17, 18],
            &[2, 3, 4, 8, 9, 10, 11, 15, 16, 17, 18],
            &[5, 6, 7, 8, 9, 10, 11],
            &[12, 13, 14, 15, 16, 17, 18],
        ];
        for d in [0u32, 0x1, 0x155, 0x2BEEF & 0x3FFFF, 0x3FFFF, 0x20000] {
            let w = encode_24_18(d);
            let all = w[0] as u32 | (w[1] as u32) << 8 | (w[2] as u32) << 16;
            for (k, e) in eq.iter().enumerate() {
                let p = 1 ^ e.iter().map(|&i| (d >> (i - 1)) & 1).fold(0, |a, b| a ^ b);
                assert_eq!((all >> ((1 << k) - 1)) & 1, p, "P{} for {d:#x}", k + 1);
            }
        }
    }

    #[test]
    fn odd_parity() {
        assert_eq!(parity(b'A'), None);
        assert_eq!(parity(0x80 | b'A'), Some(b'A'));
        assert_eq!(parity(with_parity(b'A')), Some(b'A'));
        assert_eq!(with_parity(b'A'), 0xC1);
        assert_eq!(with_parity(b'C'), 0x43);
    }
}
