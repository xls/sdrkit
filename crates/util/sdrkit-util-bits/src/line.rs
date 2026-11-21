//! Line codes: how bits become levels or chips on the line, and back.
//!
//! Hard bits and chips are `u8` (0 or 1). Encoders append to a caller's `Vec`; decoders append and return an error with the index
//! of the first bad symbol (counted in symbols, so a bad Manchester pair `k` is reported as `at: k`). Stateful codes (NRZI, NRZ-M, the
//! biphase codes) keep their level between calls, so a stream can be fed in pieces.
//!
//! | Code | Rule | Users |
//! |---|---|---|
//! | [`Nrzi`] | 0 is a change of level, 1 none | HDLC links: AIS, AX.25, VDL Mode 4, USB |
//! | [`NrzM`] | 1 is a change of level, 0 none (data = bit XOR previous bit) | CCSDS NRZ-M, RDS, GSM, DBPSK, DVB-T TPS, GLONASS relative code |
//! | [`Manchester`] | each bit is two chips; [`ManchesterConv`] says which pair is 1 | Ethernet (IEEE), ISM sensors, wM-Bus, Z-Wave, FLARM, GLONASS meander |
//! | [`Biphase`] | a change at every bit boundary; a mid-bit change marks 1 ([`BiphaseKind::Mark`]) or 0 ([`BiphaseKind::Space`], FM0) | AES3, EPC Gen2 tags, ISM sensors |
//! | [`DiffManchester`] | a change at every mid-bit; a boundary change marks 0 | IEEE 802.5 |
//! | [`miller_encode`] | delay modulation: a mid-bit change for 1, a boundary change between two 0s | EPC Gen2 tag replies |
//! | [`modified_miller_encode`] | ISO/IEC 14443-2 type A sequences X, Y, Z | NFC-A reader to card |
//! | [`Pie`] | pulse-interval encoding | EPC Gen2 reader to tag |
//! | [`three_of_six`] | 4 bits to 6 chips | EN 13757-4 wM-Bus mode T |
//! | [`four_b_six_b`] | 4 bits to 6 chips, DC balanced | IEEE 802.15.7 VLC |
//! | [`ppm_encode`] | 1 out of 2^k pulse position | ISO/IEC 15693 (1 of 4, 1 of 256) |
//! | [`uart_encode`] | start bit, 8 data bits least significant first, stop bit | UART-framed radiosondes, NFC |
//!
//! ```
//! use sdrkit_util_bits::line::{Manchester, ManchesterConv, Nrzi};
//! let mut chips = Vec::new();
//! Manchester::encode(&[1, 0], ManchesterConv::Ieee, &mut chips);
//! assert_eq!(chips, [0, 1, 1, 0]);
//! let mut nrzi = Nrzi::new(1);
//! let mut line = [1u8, 0, 0, 1];
//! nrzi.encode(&mut line);
//! assert_eq!(line, [1, 0, 1, 1]);
//! ```

#[cfg(test)]
use crate::Soft;
use crate::{Error, Llr, Param, Result};

/// NRZI (NRZ-S): a 0 bit changes the level, a 1 keeps it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Nrzi {
    level: u8,
}

impl Nrzi {
    /// Starts at line level `level` (0 or 1): the level before the first bit.
    pub fn new(level: u8) -> Self {
        Nrzi { level: level & 1 }
    }

    /// Data bits to line levels, in place.
    pub fn encode(&mut self, bits: &mut [u8]) {
        for b in bits {
            if *b & 1 == 0 {
                self.level ^= 1;
            }
            *b = self.level;
        }
    }

    /// Line levels to data bits, in place.
    pub fn decode(&mut self, levels: &mut [u8]) {
        for l in levels {
            let v = *l & 1;
            *l = (v == self.level) as u8;
            self.level = v;
        }
    }

    /// Soft line levels (LLRs of each level) to data LLRs, in place. The first value uses the stored level as certain.
    pub fn decode_soft<T: Llr>(&mut self, levels: &mut [T], prev: &mut T) {
        for l in levels {
            let v = *l;
            // data 1 when the level stays: data = NOT(level XOR previous).
            *l = v.xor_llr(*prev).flip();
            *prev = v;
        }
        self.level = prev.hard();
    }
}

/// NRZ-M (differential): a 1 bit changes the level, a 0 keeps it. Decoding is `data = level XOR previous level`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NrzM {
    level: u8,
}

/// The differential code is NRZ-M under another name.
pub type Differential = NrzM;

impl NrzM {
    /// Starts at line level `level`.
    pub fn new(level: u8) -> Self {
        NrzM { level: level & 1 }
    }

    /// Data bits to line levels, in place.
    pub fn encode(&mut self, bits: &mut [u8]) {
        for b in bits {
            self.level ^= *b & 1;
            *b = self.level;
        }
    }

    /// Line levels to data bits, in place.
    pub fn decode(&mut self, levels: &mut [u8]) {
        for l in levels {
            let v = *l & 1;
            *l = v ^ self.level;
            self.level = v;
        }
    }

    /// Soft levels to data LLRs, in place; `prev` is the LLR of the level before the first one and is updated.
    pub fn decode_soft<T: Llr>(&mut self, levels: &mut [T], prev: &mut T) {
        for l in levels {
            let v = *l;
            *l = v.xor_llr(*prev);
            *prev = v;
        }
        self.level = prev.hard();
    }
}

/// Which chip pair means 1 in Manchester code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManchesterConv {
    /// IEEE 802.3: 1 is chips 0 then 1 (a rising edge), 0 is 1 then 0.
    Ieee,
    /// G. E. Thomas: 1 is chips 1 then 0, 0 is 0 then 1. The GLONASS meander (data XOR 10 at twice the bit rate) is this.
    Thomas,
}

/// Manchester code.
#[derive(Clone, Copy, Debug)]
pub struct Manchester;

impl Manchester {
    fn pair(bit: u8, c: ManchesterConv) -> [u8; 2] {
        let b = bit & 1;
        match c {
            ManchesterConv::Ieee => [b ^ 1, b],
            ManchesterConv::Thomas => [b, b ^ 1],
        }
    }

    /// Appends two chips per bit.
    pub fn encode(bits: &[u8], c: ManchesterConv, out: &mut Vec<u8>) {
        out.reserve(bits.len() * 2);
        for &b in bits {
            out.extend_from_slice(&Self::pair(b, c));
        }
    }

    /// Appends one bit per chip pair. Stops at the first pair without a change (00 or 11) and returns its pair index; the bits
    /// before it are appended. A final odd chip is ignored.
    pub fn decode(chips: &[u8], c: ManchesterConv, out: &mut Vec<u8>) -> Result<()> {
        for (k, p) in chips.as_chunks::<2>().0.iter().enumerate() {
            let (a, b) = (p[0] & 1, p[1] & 1);
            if a == b {
                return Err(Error::Invalid { at: k });
            }
            out.push(match c {
                ManchesterConv::Ieee => b,
                ManchesterConv::Thomas => a,
            });
        }
        Ok(())
    }

    /// Soft chips (LLRs) to soft bits: the two chips are two observations of the bit, so their LLRs add (one of them negated).
    /// Appends one value per pair.
    pub fn decode_soft<T: Llr>(chips: &[T], c: ManchesterConv, out: &mut Vec<T>) {
        for p in chips.as_chunks::<2>().0 {
            out.push(match c {
                // Ieee: bit = second chip = NOT first chip.
                ManchesterConv::Ieee => p[1].add_llr(p[0].flip()),
                ManchesterConv::Thomas => p[0].add_llr(p[1].flip()),
            });
        }
    }
}

/// The two biphase codes with a change of level at every bit boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiphaseKind {
    /// Biphase mark (FM1, BMC): a mid-bit change means 1 (AES3).
    Mark,
    /// Biphase space (FM0): a mid-bit change means 0 (EPC Gen2 tag to reader, ISO/IEC 18000-6). Some sensor decoders call this
    /// "differential Manchester"; that name belongs to [`DiffManchester`].
    Space,
}

/// A biphase coder with its line level. Two chips per bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Biphase {
    kind: BiphaseKind,
    level: u8,
}

impl Biphase {
    /// A coder whose line was at `level` before the first bit.
    pub fn new(kind: BiphaseKind, level: u8) -> Self {
        Biphase {
            kind,
            level: level & 1,
        }
    }

    fn mid_change(&self, bit: u8) -> bool {
        match self.kind {
            BiphaseKind::Mark => bit & 1 == 1,
            BiphaseKind::Space => bit & 1 == 0,
        }
    }

    /// Appends two chips per bit.
    pub fn encode(&mut self, bits: &[u8], out: &mut Vec<u8>) {
        for &b in bits {
            self.level ^= 1;
            let first = self.level;
            if self.mid_change(b) {
                self.level ^= 1;
            }
            out.extend_from_slice(&[first, self.level]);
        }
    }

    /// Appends one bit per chip pair. A pair whose first chip does not change from the previous chip is an error (index of the
    /// pair). The stored level is the last chip read.
    pub fn decode(&mut self, chips: &[u8], out: &mut Vec<u8>) -> Result<()> {
        for (k, p) in chips.as_chunks::<2>().0.iter().enumerate() {
            let (a, b) = (p[0] & 1, p[1] & 1);
            if a == self.level {
                return Err(Error::Invalid { at: k });
            }
            let changed = a != b;
            out.push(match self.kind {
                BiphaseKind::Mark => changed as u8,
                BiphaseKind::Space => (!changed) as u8,
            });
            self.level = b;
        }
        Ok(())
    }
}

/// Differential Manchester (IEEE 802.5): a change at every mid-bit; a 0 also changes at the start of its cell, a 1 does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiffManchester {
    level: u8,
}

impl DiffManchester {
    /// A coder whose line was at `level` before the first bit.
    pub fn new(level: u8) -> Self {
        DiffManchester { level: level & 1 }
    }

    /// Appends two chips per bit.
    pub fn encode(&mut self, bits: &[u8], out: &mut Vec<u8>) {
        for &b in bits {
            if b & 1 == 0 {
                self.level ^= 1;
            }
            let first = self.level;
            self.level ^= 1;
            out.extend_from_slice(&[first, self.level]);
        }
    }

    /// Appends one bit per chip pair; a pair without its mid-bit change is an error.
    pub fn decode(&mut self, chips: &[u8], out: &mut Vec<u8>) -> Result<()> {
        for (k, p) in chips.as_chunks::<2>().0.iter().enumerate() {
            let (a, b) = (p[0] & 1, p[1] & 1);
            if a == b {
                return Err(Error::Invalid { at: k });
            }
            out.push((a == self.level) as u8);
            self.level = b;
        }
        Ok(())
    }
}

/// Miller (delay modulation) code, two chips per bit: a 1 changes level in the middle of its cell, a 0 keeps the level, and the
/// level changes at the boundary between two 0s. `level` is the line level before the first bit and is updated.
pub fn miller_encode(bits: &[u8], level: &mut u8, prev_bit: &mut u8, out: &mut Vec<u8>) {
    for &b in bits {
        let b = b & 1;
        if b == 0 && *prev_bit == 0 {
            *level ^= 1;
        }
        let first = *level;
        if b == 1 {
            *level ^= 1;
        }
        out.extend_from_slice(&[first, *level]);
        *prev_bit = b;
    }
}

/// Decodes Miller chips (two per bit): a mid-cell change is 1, none is 0. A change at the boundary into a 1, or a missing change
/// between two 0s, is an error (index of the bit). `level` is the level before the first chip.
pub fn miller_decode(
    chips: &[u8],
    level: &mut u8,
    prev_bit: &mut u8,
    out: &mut Vec<u8>,
) -> Result<()> {
    for (k, p) in chips.as_chunks::<2>().0.iter().enumerate() {
        let (a, b) = (p[0] & 1, p[1] & 1);
        let bit = (a != b) as u8;
        let boundary_change = a != *level;
        let should = bit == 0 && *prev_bit == 0;
        if boundary_change != should {
            return Err(Error::Invalid { at: k });
        }
        out.push(bit);
        *level = b;
        *prev_bit = bit;
    }
    Ok(())
}

/// The ISO/IEC 14443-2 type A sequences, each two half-bit chips (1 is carrier, 0 is a pause).
const SEQ_X: [u8; 2] = [1, 0];
const SEQ_Y: [u8; 2] = [1, 1];
const SEQ_Z: [u8; 2] = [0, 1];

/// Modified Miller code of ISO/IEC 14443-2 clause 8.1.3 (type A, reader to card), two chips per bit time: appends the start of
/// communication (Z), the bits (1 is X; 0 is Y, except Z after another 0 or as the first bit), and the end of communication (a
/// logic 0 then Y).
pub fn modified_miller_encode(bits: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(&SEQ_Z);
    let mut prev = 0u8; // the start of frame counts as a 0 for the rule
    for &b in bits.iter().chain(std::iter::once(&0u8)) {
        let b = b & 1;
        out.extend_from_slice(if b == 1 {
            &SEQ_X
        } else if prev == 0 {
            &SEQ_Z
        } else {
            &SEQ_Y
        });
        prev = b;
    }
    out.extend_from_slice(&SEQ_Y);
}

/// Decodes a modified Miller frame made by [`modified_miller_encode`]: checks the start (Z), reads X as 1 and Y or Z as 0, stops
/// at the end of communication (a 0 followed by Y), and appends the bits. An error (index of the sequence) for a sequence that
/// is none of X, Y, Z, for Z after a 1, Y after a 0, or a missing end.
pub fn modified_miller_decode(chips: &[u8], out: &mut Vec<u8>) -> Result<()> {
    let mut it = chips.as_chunks::<2>().0.iter().enumerate();
    match it.next() {
        Some((_, p)) if *p == SEQ_Z => {}
        _ => return Err(Error::Invalid { at: 0 }),
    }
    let mut prev = 0u8;
    let start = out.len();
    for (k, p) in it {
        let s = [p[0] & 1, p[1] & 1];
        if s == SEQ_X {
            out.push(1);
            prev = 1;
        } else if s == SEQ_Z {
            if prev != 0 {
                return Err(Error::Invalid { at: k });
            }
            out.push(0);
            prev = 0;
        } else if s == SEQ_Y {
            if prev == 0 {
                // Y after a 0: end of communication; the 0 before it was the end marker.
                if out.len() == start {
                    return Err(Error::Invalid { at: k });
                }
                out.pop();
                return Ok(());
            }
            out.push(0);
            prev = 0;
        } else {
            return Err(Error::Invalid { at: k });
        }
    }
    Err(Error::Invalid {
        at: chips.len() / 2,
    })
}

/// Pulse-interval encoding (EPC UHF Gen2 clause 6.3.1.2.3, reader to tag), in whole chips (1 is carrier, 0 is the low pulse):
/// a 0 is `data0` chips, a 1 is `data1` chips, each ending with `pw` low chips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pie {
    /// Length of a data-0 symbol in chips (Tari).
    pub data0: usize,
    /// Length of a data-1 symbol in chips (1.5 to 2 Tari).
    pub data1: usize,
    /// Low pulse width in chips.
    pub pw: usize,
}

impl Pie {
    /// A coder; an error unless `pw < data0 < data1`.
    pub fn new(data0: usize, data1: usize, pw: usize) -> Result<Self> {
        if pw == 0 || pw >= data0 || data0 >= data1 {
            return Err(Error::BadParameter(Param::PieLengths));
        }
        Ok(Pie { data0, data1, pw })
    }

    /// Appends the chips of `bits`.
    pub fn encode(&self, bits: &[u8], out: &mut Vec<u8>) {
        for &b in bits {
            let n = if b & 1 == 1 { self.data1 } else { self.data0 };
            out.extend(std::iter::repeat_n(1u8, n - self.pw));
            out.extend(std::iter::repeat_n(0u8, self.pw));
        }
    }

    /// Decodes chips: each symbol is a run of ones ended by a run of zeros; its total length is compared with the midpoint of
    /// `data0` and `data1`. A trailing run without its low pulse is left out. Appends the bits.
    pub fn decode(&self, chips: &[u8], out: &mut Vec<u8>) {
        let mid2 = self.data0 + self.data1; // twice the threshold
        let mut i = 0;
        while i < chips.len() {
            let start = i;
            while i < chips.len() && chips[i] & 1 == 1 {
                i += 1;
            }
            let lo = i;
            while i < chips.len() && chips[i] & 1 == 0 {
                i += 1;
            }
            if i == lo {
                break;
            }
            out.push((2 * (i - start) > mid2) as u8);
        }
    }
}

/// The 3-out-of-6 code of EN 13757-4 (wireless M-Bus mode T): each nibble becomes 6 chips with three ones.
pub mod three_of_six {
    /// The code of nibbles 0 to 15 (EN 13757-4 table "3 out of 6" coding).
    pub const TABLE: [u8; 16] = [
        22, 13, 14, 11, 28, 25, 26, 19, 44, 37, 38, 35, 52, 49, 50, 41,
    ];

    const DECODE: [u8; 64] = {
        let mut t = [0xFFu8; 64];
        let mut i = 0;
        while i < 16 {
            t[TABLE[i] as usize] = i as u8;
            i += 1;
        }
        t
    };

    /// The 6-chip code (low 6 bits, first chip most significant) of the low nibble of `n`.
    #[inline]
    pub const fn encode(n: u8) -> u8 {
        TABLE[(n & 0xF) as usize]
    }

    /// The nibble of a 6-chip code, `None` when the code is not in the table. A table lookup, not a search.
    #[inline]
    pub const fn decode(c: u8) -> Option<u8> {
        let v = DECODE[(c & 0x3F) as usize];
        if v == 0xFF {
            None
        } else {
            Some(v)
        }
    }
}

/// The 4B6B code of IEEE 802.15.7 (VLC, OOK and VPPM): each nibble becomes 6 chips with three ones, so the line is DC balanced.
pub mod four_b_six_b {
    /// The codes of nibbles 0 to 15 (IEEE 802.15.7 table "4B6B encoding"), first chip most significant.
    pub const TABLE: [u8; 16] = [
        0b001110, 0b001101, 0b010011, 0b010110, 0b010101, 0b100011, 0b100110, 0b100101, 0b011001,
        0b011010, 0b011100, 0b110001, 0b110010, 0b101001, 0b101010, 0b101100,
    ];

    const DECODE: [u8; 64] = {
        let mut t = [0xFFu8; 64];
        let mut i = 0;
        while i < 16 {
            t[TABLE[i] as usize] = i as u8;
            i += 1;
        }
        t
    };

    /// The 6-chip code of the low nibble of `n`.
    #[inline]
    pub const fn encode(n: u8) -> u8 {
        TABLE[(n & 0xF) as usize]
    }

    /// The nibble of a 6-chip code, or `None`.
    #[inline]
    pub const fn decode(c: u8) -> Option<u8> {
        let v = DECODE[(c & 0x3F) as usize];
        if v == 0xFF {
            None
        } else {
            Some(v)
        }
    }
}

/// Appends 1-out-of-2^k pulse position chips (ISO/IEC 15693-2 clause 8.2: 1 of 4 with `k` = 2, 1 of 256 with `k` = 8): each group
/// of `k` bits (first bit least significant) is a value `v`, sent as `2 * 2^k` chips that are 1 except chip `2 v + 1`, the pause.
/// A final partial group is an error and nothing of it is appended.
pub fn ppm_encode(bits: &[u8], k: u32, out: &mut Vec<u8>) -> Result<()> {
    if !(1..=8).contains(&k) {
        return Err(Error::BadParameter(Param::PpmBits));
    }
    if !bits.len().is_multiple_of(k as usize) {
        return Err(Error::BadLength);
    }
    let slots = 2usize << k;
    for g in bits.chunks_exact(k as usize) {
        let v = crate::pack::to_u64_lsb(g) as usize;
        let at = out.len();
        out.resize(at + slots, 1);
        out[at + 2 * v + 1] = 0;
    }
    Ok(())
}

/// Decodes 1-out-of-2^k pulse position chips: each symbol of `2 * 2^k` chips must hold exactly one pause, at an odd chip. Appends
/// `k` bits per symbol; an error (index of the symbol) otherwise.
pub fn ppm_decode(chips: &[u8], k: u32, out: &mut Vec<u8>) -> Result<()> {
    if !(1..=8).contains(&k) {
        return Err(Error::BadParameter(Param::PpmBits));
    }
    let slots = 2usize << k;
    for (s, sym) in chips.chunks_exact(slots).enumerate() {
        let mut pause = None;
        for (i, &c) in sym.iter().enumerate() {
            if c & 1 == 0 {
                if pause.is_some() || i % 2 == 0 {
                    return Err(Error::Invalid { at: s });
                }
                pause = Some(i);
            }
        }
        let v = pause.ok_or(Error::Invalid { at: s })? / 2;
        crate::pack::push_u64_lsb(out, v as u64, k);
    }
    Ok(())
}

/// Appends UART 8N1 frames: for each byte a start bit (0), the 8 data bits least significant first, and a stop bit (1).
pub fn uart_encode(bytes: &[u8], out: &mut Vec<u8>) {
    for &b in bytes {
        out.push(0);
        for k in 0..8 {
            out.push((b >> k) & 1);
        }
        out.push(1);
    }
}

/// Decodes back-to-back UART 8N1 frames of 10 bits each. Appends the bytes; an error (index of the frame) at the first frame with
/// a bad start or stop bit.
pub fn uart_decode(bits: &[u8], out: &mut Vec<u8>) -> Result<()> {
    for (i, f) in bits.as_chunks::<10>().0.iter().enumerate() {
        if f[0] & 1 != 0 || f[9] & 1 != 1 {
            return Err(Error::Invalid { at: i });
        }
        out.push(crate::pack::to_u64_lsb(&f[1..9]) as u8);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: [u8; 16] = [1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0];

    #[test]
    fn nrzi_and_nrzm_round_trip_and_definitions() {
        let mut l = DATA;
        Nrzi::new(0).encode(&mut l);
        // AX.25: a 0 is a transition.
        let mut level = 0u8;
        for (i, &b) in DATA.iter().enumerate() {
            if b == 0 {
                level ^= 1;
            }
            assert_eq!(l[i], level);
        }
        Nrzi::new(0).decode(&mut l);
        assert_eq!(l, DATA);
        let mut m = DATA;
        NrzM::new(1).encode(&mut m);
        let mut d = NrzM::new(1);
        d.decode(&mut m[..5]);
        d.decode(&mut m[5..]);
        assert_eq!(m, DATA);
    }

    #[test]
    fn differential_soft_decoding_matches_hard() {
        let mut levels = DATA;
        NrzM::new(0).encode(&mut levels);
        let mut soft: Vec<i8> = levels.iter().map(|&b| i8::from_hard(b) / 2).collect();
        let mut prev = 127i8;
        NrzM::new(0).decode_soft(&mut soft, &mut prev);
        let hard: Vec<u8> = soft.iter().map(|v| v.hard()).collect();
        assert_eq!(hard, DATA);
        assert!(soft.iter().all(|v| v.unsigned_abs() == 63));
        let mut levels = DATA;
        Nrzi::new(0).encode(&mut levels);
        let mut soft: Vec<f32> = levels.iter().map(|&b| f32::from_hard(b)).collect();
        let mut prev = 1.0f32;
        Nrzi::new(0).decode_soft(&mut soft, &mut prev);
        let hard: Vec<u8> = soft.iter().map(|v| v.hard()).collect();
        assert_eq!(hard, DATA);
    }

    #[test]
    fn manchester_both_conventions_reject_bad_pairs() {
        for c in [ManchesterConv::Ieee, ManchesterConv::Thomas] {
            let mut chips = Vec::new();
            Manchester::encode(&DATA, c, &mut chips);
            let mut back = Vec::new();
            Manchester::decode(&chips, c, &mut back).unwrap();
            assert_eq!(back, DATA);
            chips[7] ^= 1; // pair 3 becomes 00 or 11
            back.clear();
            assert_eq!(
                Manchester::decode(&chips, c, &mut back),
                Err(Error::Invalid { at: 3 })
            );
            assert_eq!(back, DATA[..3]);
            let soft: Vec<i8> = chips.iter().map(|&b| i8::from_hard(b) / 4).collect();
            let mut s = Vec::new();
            Manchester::decode_soft(&soft, c, &mut s);
            assert_eq!(s[3], 0); // the broken pair carries no information
            assert_eq!(s[0].hard(), DATA[0]);
        }
        let mut chips = Vec::new();
        Manchester::encode(&[1], ManchesterConv::Thomas, &mut chips);
        assert_eq!(chips, [1, 0]);
    }

    #[test]
    fn biphase_codes() {
        for kind in [BiphaseKind::Mark, BiphaseKind::Space] {
            let mut chips = Vec::new();
            Biphase::new(kind, 0).encode(&DATA, &mut chips);
            // A change at every boundary.
            let mut prev = 0;
            for p in chips.chunks(2) {
                assert_ne!(p[0], prev);
                prev = p[1];
            }
            let mut back = Vec::new();
            let mut d = Biphase::new(kind, 0);
            d.decode(&chips[..10], &mut back).unwrap();
            d.decode(&chips[10..], &mut back).unwrap();
            assert_eq!(back, DATA);
            let mut bad = chips.clone();
            bad[6] ^= 1;
            bad[7] ^= 1;
            back.clear();
            // Pair 3 inverted: the change of level at its first chip, or at the first chip of pair 4, is missing.
            assert!(Biphase::new(kind, 0).decode(&bad, &mut back).is_err());
        }
        // FM0 of 0 has a mid-bit change, of 1 none.
        let mut c = Vec::new();
        Biphase::new(BiphaseKind::Space, 0).encode(&[0, 1], &mut c);
        assert_eq!(c, [1, 0, 1, 1]);
    }

    #[test]
    fn diff_manchester_and_miller() {
        let mut chips = Vec::new();
        DiffManchester::new(0).encode(&DATA, &mut chips);
        let mut back = Vec::new();
        DiffManchester::new(0).decode(&chips, &mut back).unwrap();
        assert_eq!(back, DATA);
        let mut chips = Vec::new();
        let (mut lv, mut pb) = (0u8, 1u8);
        miller_encode(&DATA, &mut lv, &mut pb, &mut chips);
        let mut back = Vec::new();
        let (mut lv, mut pb) = (0u8, 1u8);
        miller_decode(&chips, &mut lv, &mut pb, &mut back).unwrap();
        assert_eq!(back, DATA);
        // Delay modulation never has a run longer than 4 half-bits.
        let mut run = 1;
        for w in chips.windows(2) {
            run = if w[0] == w[1] { run + 1 } else { 1 };
            assert!(run <= 4);
        }
    }

    #[test]
    fn modified_miller_follows_iso_14443_2() {
        // Bits 1, 0, 0, 1: Z (start), X, Y, Z, X, then end: 0 (Y after a 1) and Y.
        let mut c = Vec::new();
        modified_miller_encode(&[1, 0, 0, 1], &mut c);
        assert_eq!(c, [0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 1, 1]);
        // A first bit 0 is Z.
        let mut c2 = Vec::new();
        modified_miller_encode(&[0, 1], &mut c2);
        assert_eq!(&c2[..4], &[0, 1, 0, 1]);
        for bits in [&[1u8, 0, 0, 1][..], &[0, 1], &[0, 0, 0], &[1, 1, 1, 0, 1]] {
            let mut c = Vec::new();
            modified_miller_encode(bits, &mut c);
            let mut back = Vec::new();
            modified_miller_decode(&c, &mut back).unwrap();
            assert_eq!(back, bits);
        }
        let mut back = Vec::new();
        assert!(modified_miller_decode(&[1, 1, 1, 0], &mut back).is_err());
        assert!(modified_miller_decode(&[0, 1, 0, 0], &mut back).is_err());
    }

    #[test]
    fn pie_round_trip() {
        let p = Pie::new(4, 7, 2).unwrap();
        let mut c = Vec::new();
        p.encode(&[0, 1, 1, 0], &mut c);
        assert_eq!(
            c,
            [1, 1, 0, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0]
        );
        let mut back = Vec::new();
        p.decode(&c, &mut back);
        assert_eq!(back, [0, 1, 1, 0]);
        assert!(Pie::new(4, 4, 2).is_err());
    }

    #[test]
    fn three_of_six_and_four_b_six_b_tables() {
        for (t, enc, dec) in [
            (
                three_of_six::TABLE,
                three_of_six::encode as fn(u8) -> u8,
                three_of_six::decode as fn(u8) -> Option<u8>,
            ),
            (
                four_b_six_b::TABLE,
                four_b_six_b::encode,
                four_b_six_b::decode,
            ),
        ] {
            for n in 0..16u8 {
                assert_eq!(t[n as usize].count_ones(), 3);
                assert_eq!(dec(enc(n)), Some(n));
            }
            let valid = (0..64u8).filter(|&c| dec(c).is_some()).count();
            assert_eq!(valid, 16);
        }
        // IEEE 802.15.7 table, first rows: 0000 -> 001110, 0001 -> 001101, 0010 -> 010011, 0011 -> 010110.
        assert_eq!(
            &four_b_six_b::TABLE[..4],
            &[0b001110, 0b001101, 0b010011, 0b010110]
        );
        assert_eq!(three_of_six::decode(0), None);
    }

    #[test]
    fn ppm_one_of_four() {
        // ISO 15693: bits 0,0 (value 0) pause in slot 2 (chip 1); bits 1,1 (value 3) pause at chip 7.
        let mut c = Vec::new();
        ppm_encode(&[0, 0, 1, 1], 2, &mut c).unwrap();
        assert_eq!(c, [1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0]);
        let mut back = Vec::new();
        ppm_decode(&c, 2, &mut back).unwrap();
        assert_eq!(back, [0, 0, 1, 1]);
        let mut c = Vec::new();
        ppm_encode(&[1, 0, 1, 0, 0, 0, 0, 1], 8, &mut c).unwrap();
        assert_eq!(c.len(), 512);
        assert_eq!(c.iter().position(|&x| x == 0), Some(2 * 0x85 + 1));
        assert!(ppm_encode(&[1], 2, &mut c).is_err());
        assert!(ppm_decode(&[1; 8], 2, &mut back).is_err());
    }

    #[test]
    fn uart_frames() {
        let mut b = Vec::new();
        uart_encode(&[0x55, 0xA3], &mut b);
        assert_eq!(&b[..10], &[0, 1, 0, 1, 0, 1, 0, 1, 0, 1]);
        let mut out = Vec::new();
        uart_decode(&b, &mut out).unwrap();
        assert_eq!(out, [0x55, 0xA3]);
        b[19] = 0;
        out.clear();
        assert_eq!(uart_decode(&b, &mut out), Err(Error::Invalid { at: 1 }));
    }
}
