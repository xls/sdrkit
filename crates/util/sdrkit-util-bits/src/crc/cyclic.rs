//! Remainders of bit strings, and the check words of cyclic codes that are not whole bytes.
//!
//! A cyclic check word is the remainder of the message (followed by zero bits) divided by a generator polynomial. This module holds
//! that division ([`remainder`], [`remainder_word`]), the weak-signal CRCs as [`BitCrc`] entries, and systematic cyclic codes
//! as [`CyclicCode`] entries (POCSAG, MPT 1327, EDACS, RDS, Golay, BCH words). Only the check bits are computed here; correcting
//! errors (syndrome tables, Chase search) belongs to `sdrkit-algo-fec`.
//!
//! Generators are written with their top term: x^10 + x^9 + x^8 + x^6 + x^5 + x^3 + 1 is `0x769`, degree 10.
//!
//! ```
//! use sdrkit_util_bits::crc::cyclic::{self, POCSAG};
//! // The POCSAG idle codeword (ITU-R M.584-2) is a valid codeword.
//! assert!(POCSAG.check_with_parity(0x7A89_C197));
//! assert_eq!(cyclic::remainder(&[1, 0, 1], 3, 0b1011, 3), 0b100);
//! ```

use crate::Error;
use crate::Param;

/// The remainder of the bit string `bits` (one bit per byte, first bit the highest power) followed by `zeros` zero bits, divided by
/// `gen` of degree `deg` (top term included in `gen`). `deg` is at most 127; a larger degree returns 0.
pub fn remainder(bits: &[u8], zeros: usize, gen: u128, deg: u32) -> u128 {
    if deg == 0 || deg > 127 {
        return 0;
    }
    let top = 1u128 << deg;
    let mut r = 0u128;
    let mut step = |b: u8| {
        r = (r << 1) | (b & 1) as u128;
        if r & top != 0 {
            r ^= gen;
        }
    };
    for &b in bits {
        step(b);
    }
    for _ in 0..zeros {
        step(0);
    }
    r
}

/// The remainder of the `len` low bits of `word` (bit `len - 1` the highest power) divided by `gen` of degree `deg`. The fast
/// path for words up to 128 bits: one conditional XOR per bit above the degree, no loop over a slice.
pub fn remainder_word(word: u128, len: u32, gen: u128, deg: u32) -> u128 {
    if deg == 0 || deg > 127 || len > 128 {
        return 0;
    }
    let mut w = if len == 128 {
        word
    } else {
        word & ((1u128 << len) - 1)
    };
    let mut bit = len;
    while bit > deg {
        bit -= 1;
        if w & (1u128 << bit) != 0 {
            w ^= gen << (bit - deg);
        }
    }
    w
}

/// A CRC defined on a bit string of a fixed or varying length: the remainder of the message followed by `zeros` zero bits,
/// divided by `gen`, XORed with `xorout`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BitCrc {
    /// A name for the CRC.
    pub name: &'static str,
    /// Generator, top term included.
    pub gen: u128,
    /// Width of the CRC (the generator's degree).
    pub width: u32,
    /// Zero bits that follow the message in the division.
    pub zeros: usize,
    /// Value XORed onto the remainder.
    pub xorout: u64,
    /// The message length the standard uses, in bits (0 when it varies).
    pub msg_bits: usize,
}

impl BitCrc {
    /// The CRC of `bits`.
    pub fn compute(&self, bits: &[u8]) -> u64 {
        remainder(bits, self.zeros, self.gen, self.width) as u64 ^ self.xorout
    }

    /// Whether the `width` bits after the first `msg_bits` of `frame` are the CRC of those bits (most significant first). For a
    /// CRC whose message length varies, `msg_bits` is `frame.len() - width`.
    pub fn check(&self, frame: &[u8]) -> bool {
        let w = self.width as usize;
        let m = if self.msg_bits == 0 {
            match frame.len().checked_sub(w) {
                Some(m) => m,
                None => return false,
            }
        } else {
            self.msg_bits
        };
        match frame.get(m..m + w) {
            Some(tail) => crate::pack::to_u64(tail) == self.compute(&frame[..m]),
            None => false,
        }
    }
}

/// CRC-14 of FT8, FT4 and FT2 (QEX 2020 "The FT4 and FT8 Communication Protocols"): generator 0x6757 (x^14 + x^13 + x^10 + x^9 +
/// x^8 + x^6 + x^4 + x^2 + x + 1), over the 77 message bits followed by 19 zero bits (5 pad bits and the 14 CRC positions).
pub const FT8_CRC14: BitCrc = BitCrc {
    name: "FT8 CRC-14",
    gen: 0x6757,
    width: 14,
    zeros: 19,
    xorout: 0,
    msg_bits: 77,
};

/// CRC-13 of MSK144: generator 0x35D7, over the 77 message bits padded with zeros to 96 bits.
pub const MSK144_CRC13: BitCrc = BitCrc {
    name: "MSK144 CRC-13",
    gen: 0x35D7,
    width: 13,
    zeros: 19,
    xorout: 0,
    msg_bits: 77,
};

/// CRC-24 of FST4 and FST4W: generator 0x100065B (the BLE polynomial, not reflected), over the information bits and 24 zero bits.
pub const FST4_CRC24: BitCrc = BitCrc {
    name: "FST4 CRC-24",
    gen: 0x100_065B,
    width: 24,
    zeros: 24,
    xorout: 0,
    msg_bits: 0,
};

/// CRC-12 of JS8: generator 0x1C06 over the 75 frame and flag bits padded with zeros to 88 bits, XORed with 42.
pub const JS8_CRC12: BitCrc = BitCrc {
    name: "JS8 CRC-12",
    gen: 0x1C06,
    width: 12,
    zeros: 13,
    xorout: 42,
    msg_bits: 75,
};

/// CRC-12 of Q65: the CRC-12/DECT generator 0x180F over the 78 message bits and 12 zero bits.
pub const Q65_CRC12: BitCrc = BitCrc {
    name: "Q65 CRC-12",
    gen: 0x180F,
    width: 12,
    zeros: 12,
    xorout: 0,
    msg_bits: 78,
};

/// CRC-12 of the HD Radio SIS PDU on the PIDS logical channel: generator 0x180B (x^12 + x^11 + x^3 + x + 1), no zero bits.
/// [`BitCrc::compute`] takes PDU bits 67 down to 0 (bit 67 first). Use [`nrsc5_pids_crc`] and [`nrsc5_pids_check`] with bits
/// in PDU order. No byte-wise catalogue CRC produces this value, because the division has no zero bits after the message.
/// Standard: NRSC-5 document 1020s (SIS transport) clause 4.10.
pub const NRSC5_PIDS_CRC12: BitCrc = BitCrc {
    name: "NRSC-5 PIDS CRC-12",
    gen: 0x180B,
    width: 12,
    zeros: 0,
    xorout: 0,
    msg_bits: 68,
};

/// The CRC of a SIS PDU from its first 68 bits, `pdu[i]` being PDU bit `i` (one bit per byte). Bits 68 to 79 count as zeros, the
/// 80 bits are divided by x^12 + x^11 + x^3 + x + 1 with bit 79 the highest power, and the 12-bit remainder is the result.
/// PDU bit 68 holds its most significant bit and bit 79 its least significant bit. Bits after the first 68 are ignored. An
/// error when `pdu` has fewer than 68 bits. Never allocates.
/// Standard: NRSC-5 document 1020s (SIS transport) clause 4.10.
///
/// ```
/// use sdrkit_util_bits::crc::cyclic::nrsc5_pids_crc;
/// // Only PDU bit 12 set: the remainder of x^12 is x^11 + x^3 + x + 1.
/// let mut pdu = [0u8; 68];
/// pdu[12] = 1;
/// assert_eq!(nrsc5_pids_crc(&pdu)?, 0x80B);
/// # Ok::<(), sdrkit_util_bits::Error>(())
/// ```
pub fn nrsc5_pids_crc(pdu: &[u8]) -> Result<u16, Error> {
    let msg = pdu
        .get(..68)
        .ok_or(Error::BadParameter(Param::PidsLength))?;
    let g = NRSC5_PIDS_CRC12.gen;
    let mut r = 0u128;
    for &b in msg.iter().rev() {
        r = (r << 1) | (b & 1) as u128;
        if r & 0x1000 != 0 {
            r ^= g;
        }
    }
    Ok(r as u16)
}

/// Whether an 80-bit SIS PDU (`pdu[i]` is PDU bit `i`) holds the right CRC in bits 68 to 79 (bit 68 most significant). False
/// when `pdu` is not 80 bits long. Never allocates.
/// Standard: NRSC-5 document 1020s (SIS transport) clause 4.10.
pub fn nrsc5_pids_check(pdu: &[u8]) -> bool {
    if pdu.len() != 80 {
        return false;
    }
    let got = pdu[68..]
        .iter()
        .fold(0u16, |v, &b| (v << 1) | (b & 1) as u16);
    matches!(nrsc5_pids_crc(pdu), Ok(c) if c == got)
}

/// The ARDOP frame check: a 16-bit register started at all ones, each data bit (most significant first) shifted in at the bottom,
/// and 0x8810 XORed in when a one leaves the top; no zero bits follow. That is the remainder of sixteen ones followed by the
/// message, divided by x^16 + x^15 + x^11 + x^4. The generator is even, so this is not a catalogue CRC, and it is not the
/// x^16 + x^12 + x^5 + 1 the ARDOP documentation names. Not a published standard: kept here so that the one decoder that needs it
/// shares the division.
pub fn ardop_crc16(data: &[u8]) -> u16 {
    let mut r: u32 = 0xFFFF;
    for &b in data {
        for i in (0..8).rev() {
            r = (r << 1) | ((b >> i) & 1) as u32;
            if r & 0x1_0000 != 0 {
                r ^= 0x1_8810;
            }
        }
    }
    r as u16
}

/// A systematic cyclic code (or a shortened one): `k` information bits followed by `n - k` check bits, the check bits being the
/// remainder of the information bits times x^(n-k) divided by `gen`. Codewords are held in a `u128`, the first bit sent most
/// significant. Some standards change the check bits after the division: `invert` is XORed onto them (MPT 1327 inverts the last
/// one), and [`CyclicCode::with_parity`] adds an even parity bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CyclicCode {
    /// A name for the code.
    pub name: &'static str,
    /// Codeword length without any added parity bit.
    pub n: u32,
    /// Information bits.
    pub k: u32,
    /// Generator, top term included; its degree is `n - k`.
    pub gen: u128,
    /// XORed onto the check bits after the division.
    pub invert: u128,
}

impl CyclicCode {
    /// The `n - k` check bits of the `k` low bits of `info`.
    pub fn check_bits(&self, info: u128) -> u128 {
        let r = self.n - self.k;
        let info = info & low(self.k);
        remainder_word(info << r, self.n, self.gen, r) ^ self.invert
    }

    /// The `n`-bit codeword of `info`.
    pub fn encode(&self, info: u128) -> u128 {
        ((info & low(self.k)) << (self.n - self.k)) | self.check_bits(info)
    }

    /// The information bits of a codeword.
    pub fn info(&self, cw: u128) -> u128 {
        (cw >> (self.n - self.k)) & low(self.k)
    }

    /// The syndrome of an `n`-bit word: 0 for a codeword. Nonzero values depend only on the error pattern.
    pub fn syndrome(&self, cw: u128) -> u128 {
        let r = self.n - self.k;
        remainder_word((cw ^ self.invert) & low(self.n), self.n, self.gen, r)
    }

    /// Whether an `n`-bit word is a codeword.
    pub fn check(&self, cw: u128) -> bool {
        self.syndrome(cw) == 0
    }

    /// The `n + 1`-bit codeword with an even parity bit after the code bits.
    pub fn with_parity(&self, info: u128) -> u128 {
        let cw = self.encode(info);
        (cw << 1) | (cw.count_ones() & 1) as u128
    }

    /// Whether an `n + 1`-bit word is a codeword followed by its even parity bit.
    pub fn check_with_parity(&self, word: u128) -> bool {
        word.count_ones() & 1 == 0 && self.check(word >> 1)
    }
}

const fn low(n: u32) -> u128 {
    if n >= 128 {
        u128::MAX
    } else {
        (1u128 << n) - 1
    }
}

/// POCSAG and FLEX code words: BCH(31,21), generator x^10 + x^9 + x^8 + x^6 + x^5 + x^3 + 1, plus an even parity bit (ITU-R
/// M.584-2 clause 3.2).
pub const POCSAG: CyclicCode = CyclicCode {
    name: "POCSAG BCH(31,21)",
    n: 31,
    k: 21,
    gen: 0x769,
    invert: 0,
};

/// MPT 1327 code words: the (63,48) cyclic code with generator x^15 + x^14 + x^13 + x^11 + x^4 + x^2 + 1, the last check bit
/// inverted, plus an even parity bit over all 64 bits (MPT 1327 clause 3.4).
pub const MPT1327: CyclicCode = CyclicCode {
    name: "MPT 1327 (63,48)",
    n: 63,
    k: 48,
    gen: 0xE815,
    invert: 1,
};

/// EDACS words: BCH(40,28), generator x^12 + x^10 + x^8 + x^5 + x^4 + x^3 + 1 (0x1539), a shortened BCH(63,51).
pub const EDACS: CyclicCode = CyclicCode {
    name: "EDACS BCH(40,28)",
    n: 40,
    k: 28,
    gen: 0x1539,
    invert: 0,
};

/// Datawell HXV buoy words: BCH(63,51) with the EDACS generator 0x1539.
pub const HXV: CyclicCode = CyclicCode {
    name: "HXV BCH(63,51)",
    n: 63,
    k: 51,
    gen: 0x1539,
    invert: 0,
};

/// The binary Golay code (23,12), generator x^11 + x^10 + x^6 + x^5 + x^4 + x^2 + 1 (0xC75): DCS (TIA-603) and, with a parity bit
/// ([`CyclicCode::with_parity`]), the extended (24,12) code of MIL-STD-188-141 ALE.
pub const GOLAY_23_12: CyclicCode = CyclicCode {
    name: "Golay (23,12)",
    n: 23,
    k: 12,
    gen: 0xC75,
    invert: 0,
};

/// BeiDou D1/D2 navigation message words: BCH(15,11), generator x^4 + x + 1 (BDS-SIS-ICD-B1I clause 5.1.3).
pub const BEIDOU_BCH_15_11: CyclicCode = CyclicCode {
    name: "BeiDou BCH(15,11)",
    n: 15,
    k: 11,
    gen: 0x13,
    invert: 0,
};

/// DVB-T TPS: BCH(67,53), shortened from BCH(127,113), generator x^14 + x^9 + x^8 + x^6 + x^5 + x^4 + x^2 + x + 1 (ETSI EN 300 744
/// clause 4.6.2).
pub const DVBT_TPS: CyclicCode = CyclicCode {
    name: "DVB-T TPS BCH(67,53)",
    n: 67,
    k: 53,
    gen: 0x4377,
    invert: 0,
};

/// RDS: the (26,16) shortened cyclic code with generator x^10 + x^8 + x^7 + x^5 + x^4 + x^3 + 1 (IEC 62106 annex B). The checkword
/// sent is the check bits XOR an offset word ([`RDS_OFFSETS`]).
pub const RDS: CyclicCode = CyclicCode {
    name: "RDS (26,16)",
    n: 26,
    k: 16,
    gen: 0x5B9,
    invert: 0,
};

/// The RDS offset words A, B, C, C' and D (IEC 62106 annex A).
pub const RDS_OFFSETS: [u16; 5] = [0x0FC, 0x198, 0x168, 0x350, 0x1B4];

/// The RDS parity-check matrix H of IEC 62106 annex B, one row per bit of the 26-bit block, first sent first.
const RDS_H: [u16; 26] = [
    0x200, 0x100, 0x080, 0x040, 0x020, 0x010, 0x008, 0x004, 0x002, 0x001, 0x2DC, 0x16E, 0x0B7,
    0x287, 0x39F, 0x313, 0x355, 0x376, 0x1BB, 0x201, 0x3DC, 0x1EE, 0x0F7, 0x2A7, 0x38F, 0x31B,
];

/// The RDS syndrome of a 26-bit block as IEC 62106 annex B defines it (the block times H). A block with offset word X and no
/// errors has the syndrome the standard lists for X: A 0x3D8, B 0x3D4, C 0x25C, C' 0x3CC, D 0x258.
pub fn rds_syndrome(block: u32) -> u16 {
    let mut s = 0;
    for (i, h) in RDS_H.iter().enumerate() {
        if (block >> (25 - i)) & 1 != 0 {
            s ^= h;
        }
    }
    s
}

/// The 26-bit RDS block of 16 information bits with offset word `offset`.
pub fn rds_block(info: u16, offset: u16) -> u32 {
    (RDS.encode(info as u128) as u32) ^ (offset as u32 & 0x3FF)
}

/// The parity bits D25..D30 of a GPS LNAV word (IS-GPS-200 table 20-XIV), from its 24 source data bits `d` (bit 23 is d1, the
/// first sent) and the last two parity bits of the previous word, D29* and D30*. The word sent is the data XOR D30* followed by
/// these six bits.
pub fn gps_lnav_parity(d: u32, d29s: u8, d30s: u8) -> u8 {
    // Data bits (1-based) of each parity equation, and whether it adds D29* (false) or D30* (true).
    const EQ: [(u32, bool); 6] = [
        (
            mask_of(&[1, 2, 3, 5, 6, 10, 11, 12, 13, 14, 17, 18, 20, 23]),
            false,
        ),
        (
            mask_of(&[2, 3, 4, 6, 7, 11, 12, 13, 14, 15, 18, 19, 21, 24]),
            true,
        ),
        (
            mask_of(&[1, 3, 4, 5, 7, 8, 12, 13, 14, 15, 16, 19, 20, 22]),
            false,
        ),
        (
            mask_of(&[2, 4, 5, 6, 8, 9, 13, 14, 15, 16, 17, 20, 21, 23]),
            true,
        ),
        (
            mask_of(&[1, 3, 5, 6, 7, 9, 10, 14, 15, 16, 17, 18, 21, 22, 24]),
            true,
        ),
        (
            mask_of(&[3, 5, 6, 8, 9, 10, 11, 13, 15, 19, 22, 23, 24]),
            false,
        ),
    ];
    let mut p = 0u8;
    for (m, uses30) in EQ {
        let s = if uses30 { d30s } else { d29s };
        p = (p << 1) | ((((d & m).count_ones() as u8) ^ s) & 1);
    }
    p
}

const fn mask_of(bits: &[u32]) -> u32 {
    let mut m = 0u32;
    let mut i = 0;
    while i < bits.len() {
        m |= 1 << (24 - bits[i]);
        i += 1;
    }
    m
}

/// The 30-bit GPS LNAV word of 24 source data bits, given D29* and D30* of the previous word (IS-GPS-200 clause 20.3.5.2).
pub fn gps_lnav_word(d: u32, d29s: u8, d30s: u8) -> u32 {
    let d = d & 0xFF_FFFF;
    let sent = if d30s & 1 != 0 { d ^ 0xFF_FFFF } else { d };
    (sent << 6) | gps_lnav_parity(d, d29s, d30s) as u32
}

/// Checks a received 30-bit LNAV word; on success returns its 24 source data bits (the D30* inversion undone).
pub fn gps_lnav_check(word: u32, d29s: u8, d30s: u8) -> Result<u32, Error> {
    let sent = (word >> 6) & 0xFF_FFFF;
    let d = if d30s & 1 != 0 {
        sent ^ 0xFF_FFFF
    } else {
        sent
    };
    if gps_lnav_parity(d, d29s, d30s) as u32 == word & 0x3F {
        Ok(d)
    } else {
        Err(Error::Invalid { at: 0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NRSC-5 1020s clause 4.10 step by step: zeros in bits 79 to 68, modulo-two division of bits 79 to 0 with bit 79 first,
    /// the remainder into bits 68 to 79 with bit 68 most significant.
    fn pids_by_the_clause(msg: &[u8]) -> Vec<u8> {
        let mut pdu = msg[..68].to_vec();
        pdu.resize(80, 0);
        let dividend: Vec<u8> = (0..80).rev().map(|i| pdu[i]).collect();
        let r = remainder(&dividend, 0, 0x180B, 12);
        for k in 0..12 {
            pdu[68 + k] = ((r >> (11 - k)) & 1) as u8;
        }
        pdu
    }

    #[test]
    fn nrsc5_pids_crc_follows_clause_4_10() {
        // The CRC of a single bit at PDU bit i is x^i mod g: x^0 to x^11 are themselves, x^12 is x^11 + x^3 + x + 1 (0x80B) and
        // x^13 is x^12 + x^4 + x^2 + x, so 0x80B + 0x016 = 0x81D.
        let one = |i: usize| {
            let mut p = [0u8; 68];
            p[i] = 1;
            nrsc5_pids_crc(&p).unwrap()
        };
        assert_eq!(one(0), 0x001);
        assert_eq!(one(11), 0x800);
        assert_eq!(one(12), 0x80B);
        assert_eq!(one(13), 0x81D);
        // The BitCrc form takes the bits from 67 down to 0.
        let mut seed = 0x1234_5678u32;
        for _ in 0..50 {
            let msg: Vec<u8> = (0..68)
                .map(|_| {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (seed >> 31) as u8
                })
                .collect();
            let pdu = pids_by_the_clause(&msg);
            let crc = nrsc5_pids_crc(&msg).unwrap();
            let rev: Vec<u8> = msg.iter().rev().copied().collect();
            assert_eq!(NRSC5_PIDS_CRC12.compute(&rev), crc as u64);
            assert!(nrsc5_pids_check(&pdu));
            let mut bad = pdu.clone();
            bad[7] ^= 1;
            assert!(!nrsc5_pids_check(&bad));
            let mut bad = pdu.clone();
            bad[79] ^= 1;
            assert!(!nrsc5_pids_check(&bad));
        }
        // It is not CRC-12/DECT, and short or long inputs are refused.
        assert_ne!(one(12), 0x80F);
        assert!(nrsc5_pids_crc(&[0; 67]).is_err());
        assert!(!nrsc5_pids_check(&[0; 79]));
        assert!(nrsc5_pids_check(&[0; 80]));
    }
    use crate::pack::push_u64;

    #[test]
    fn remainder_forms_agree() {
        let w: u128 = 0x1_2345_6789;
        let mut bits = Vec::new();
        push_u64(&mut bits, w as u64, 33);
        for (gen, deg) in [(0x769u128, 10), (0x6757, 14), (0x100_065B, 24), (0x13, 4)] {
            assert_eq!(
                remainder(&bits, 0, gen, deg),
                remainder_word(w, 33, gen, deg)
            );
            assert_eq!(
                remainder(&bits, deg as usize, gen, deg),
                remainder_word(w << deg, 33 + deg, gen, deg)
            );
        }
        assert_eq!(remainder(&bits, 0, 0x13, 200), 0);
    }

    #[test]
    fn bit_crcs_with_zero_padding_equal_catalogue_crcs() {
        // A remainder of message plus `width` zero bits is the plain (init 0) CRC, so FST4 and Q65 match catalogue entries.
        let msg = b"123456789";
        let mut bits = Vec::new();
        crate::pack::unpack(msg, crate::BitOrder::Msb, &mut bits);
        assert_eq!(Q65_CRC12.compute(&bits), 0xF5B); // CRC-12/DECT check value
        let ble_normal = crate::crc::CrcSpec::new("x", 24, 0x65B, 0, false, false, 0, 0);
        assert_eq!(FST4_CRC24.compute(&bits), ble_normal.compute(msg));
        // FT8: the 77 bits and 5 zeros, then a plain CRC-14 with generator 0x2757.
        let mut bits = Vec::new();
        crate::pack::unpack(b"123456789AB", crate::BitOrder::Msb, &mut bits);
        let ft = crate::crc::CrcSpec::new("x", 14, 0x2757, 0, false, false, 0, 0);
        let mut m = bits[..77].to_vec();
        m.extend_from_slice(&[0; 5]);
        assert_eq!(FT8_CRC14.compute(&bits[..77]), ft.compute_bits(&m));
        let mut frame = bits[..77].to_vec();
        push_u64(&mut frame, FT8_CRC14.compute(&bits[..77]), 14);
        assert!(FT8_CRC14.check(&frame));
        frame[5] ^= 1;
        assert!(!FT8_CRC14.check(&frame));
        assert_eq!(JS8_CRC12.compute(&[0; 75]), 42);
    }

    #[test]
    fn ardop_check_is_the_remainder_after_sixteen_ones() {
        let mut bits = vec![1u8; 16];
        crate::pack::unpack(b"123456789", crate::BitOrder::Msb, &mut bits);
        assert_eq!(
            remainder(&bits, 0, 0x1_8810, 16),
            ardop_crc16(b"123456789") as u128
        );
        assert_eq!(ardop_crc16(b"123456789"), 0x0E59);
    }

    #[test]
    fn pocsag_sync_and_idle_are_codewords() {
        // ITU-R M.584-2: synchronisation codeword 0x7CD215D8 and idle codeword 0x7A89C197.
        assert!(POCSAG.check_with_parity(0x7CD2_15D8));
        assert!(POCSAG.check_with_parity(0x7A89_C197));
        assert!(!POCSAG.check_with_parity(0x7A89_C196));
        assert_eq!(POCSAG.with_parity(0x7A89_C197 >> 11), 0x7A89_C197);
    }

    #[test]
    fn rds_offset_syndromes_match_iec_62106() {
        let want = [0x3D8, 0x3D4, 0x25C, 0x3CC, 0x258];
        for (o, s) in RDS_OFFSETS.iter().zip(want) {
            assert_eq!(rds_syndrome(rds_block(0x1234, *o)), s);
            assert_eq!(rds_syndrome(rds_block(0xBEEF, *o)), s);
        }
        assert!(RDS.check(RDS.encode(0xABCD)));
    }

    #[test]
    fn generators_divide_their_cycle() {
        // A generator of a cyclic code of natural length N divides x^N + 1.
        for (gen, deg, n) in [
            (0x769u128, 10, 31),
            (0x1539, 12, 63),
            (0xC75, 11, 23),
            (0x13, 4, 15),
            (0x4377, 14, 127),
            (0x5B9, 10, 341),
        ] {
            let mut bits = vec![0u8; n + 1];
            bits[0] = 1;
            bits[n] = 1;
            assert_eq!(remainder(&bits, 0, gen, deg), 0, "{gen:#x}");
        }
    }

    #[test]
    fn golay_has_minimum_distance_7() {
        let min = (1..4096u128)
            .map(|i| GOLAY_23_12.encode(i).count_ones())
            .min()
            .unwrap();
        assert_eq!(min, 7);
        let min24 = (1..4096u128)
            .map(|i| GOLAY_23_12.with_parity(i).count_ones())
            .min()
            .unwrap();
        assert_eq!(min24, 8);
    }

    #[test]
    fn mpt1327_last_check_bit_is_inverted() {
        let cw = MPT1327.with_parity(0);
        // The all-zero message provides check bits 000...01 and a parity bit 1.
        assert_eq!(cw, 0b11);
        assert!(MPT1327.check_with_parity(MPT1327.with_parity(0x8000_1234_5678)));
        assert!(!MPT1327.check_with_parity(MPT1327.with_parity(0x8000_1234_5678) ^ 4));
    }

    #[test]
    fn gps_lnav_words_round_trip() {
        for (d, a, b) in [
            (0x8B0000u32, 0u8, 0u8),
            (0x123456, 1, 0),
            (0xABCDEF, 0, 1),
            (0xFFFFFF, 1, 1),
        ] {
            let w = gps_lnav_word(d, a, b);
            assert_eq!(gps_lnav_check(w, a, b), Ok(d));
            assert!(gps_lnav_check(w ^ (1 << 10), a, b).is_err());
        }
        // The (32,26) code detects every single error in the 30 bits.
        let w = gps_lnav_word(0x5A5A5A, 0, 1);
        for i in 0..30 {
            assert!(gps_lnav_check(w ^ (1 << i), 0, 1).is_err());
        }
    }
}
