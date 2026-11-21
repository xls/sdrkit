//! ASN.1 unaligned PER (ITU-T X.691, the UNALIGNED variant), as 3GPP RRC messages use it.
//!
//! A message is described once, as a function over a [`PerIo`]: the same description reads (with a [`PerReader`]) and writes
//! (with a [`PerWriter`]), so an encoder and a parser made from it cannot drift apart. Values are passed by `&mut`: a reader
//! stores what it reads, a writer writes what is there. A reader that meets bad or short input sets a failed flag and carries on
//! with zeros, so the description is written without error handling and checked once with [`PerIo::failed`].
//!
//! ```
//! use sdrkit_util_bits::per::{PerIo, PerReader, PerWriter};
//! // LTE MasterInformationBlock (3GPP TS 36.331): dl-Bandwidth ENUMERATED {n6, n15, n25, n50, n75, n100},
//! // phich-Duration ENUMERATED {normal, extended}, phich-Resource ENUMERATED {oneSixth, half, one, two},
//! // systemFrameNumber BIT STRING (SIZE (8)), spare BIT STRING (SIZE (10)).
//! fn mib<P: PerIo>(p: &mut P, bw: &mut usize, dur: &mut usize, res: &mut usize, sfn: &mut u64) {
//!     p.enumerated(bw, 6, false);
//!     p.enumerated(dur, 2, false);
//!     p.enumerated(res, 4, false);
//!     p.bits(sfn, 8);
//!     let mut spare = 0;
//!     p.bits(&mut spare, 10);
//! }
//! let mut out = Vec::new();
//! let mut w = PerWriter::new(&mut out);
//! mib(&mut w, &mut 5, &mut 0, &mut 2, &mut 0);
//! w.finish();
//! assert_eq!(out, [0xA8, 0x00, 0x00]);
//! let (mut bw, mut dur, mut res, mut sfn) = (0, 0, 0, 0);
//! let mut r = PerReader::new(&out);
//! mib(&mut r, &mut bw, &mut dur, &mut res, &mut sfn);
//! assert!(!r.failed());
//! assert_eq!((bw, res), (5, 2));
//! ```

use crate::reader::{BitReader, BitWriter, Msb};

/// The number of bits X.691 uses for a constrained whole number with `range` values (clause 11.5.7.1, unaligned): the fewest that
/// hold `range - 1`; 0 for a single value.
pub const fn bits_for(range: u64) -> u32 {
    if range <= 1 {
        0
    } else {
        64 - (range - 1).leading_zeros()
    }
}

/// Reading or writing unaligned PER. The required methods move bits; the provided ones are the X.691 encodings.
pub trait PerIo {
    /// True for a reader.
    fn reading(&self) -> bool;
    /// `n` bits (at most 64) as an unsigned number.
    fn bits(&mut self, v: &mut u64, n: u32);
    /// Marks the message as broken (a reader only; a writer ignores it).
    fn fail(&mut self);
    /// Whether something failed.
    fn failed(&self) -> bool;
    /// `n` whole octets (not aligned in unaligned PER). A reader replaces the contents of `v`.
    fn octets_n(&mut self, v: &mut Vec<u8>, n: usize);

    /// A BOOLEAN (one bit).
    fn flag(&mut self, b: &mut bool) {
        let mut v = *b as u64;
        self.bits(&mut v, 1);
        *b = v != 0;
    }

    /// A constrained whole number `lo..=hi` (X.691 clause 11.5.7, unaligned): `v - lo` in [`bits_for`]`(hi - lo + 1)` bits. A
    /// writer writes `lo` for a value outside the range; a reader fails on an offset outside it.
    fn int(&mut self, v: &mut i64, lo: i64, hi: i64) {
        if hi < lo {
            self.fail();
            return;
        }
        let range = (hi as i128 - lo as i128 + 1).min(u64::MAX as i128) as u64;
        let n = bits_for(range);
        let mut x = if self.reading() || *v < lo || *v > hi {
            0
        } else {
            (*v as i128 - lo as i128) as u64
        };
        self.bits(&mut x, n);
        if self.reading() {
            if range != u64::MAX && x >= range {
                self.fail();
            }
            *v = (lo as i128 + x as i128) as i64;
        }
    }

    /// A length determinant without an upper bound under 64K (X.691 clause 11.9.3.5 to 11.9.3.7, unaligned): 8 bits below 128,
    /// 16 bits (10 then 14-bit value) below 16384. Fragmented lengths (16K and over) are not supported: a reader fails, a writer
    /// writes nothing and the caller must not use them.
    fn length(&mut self, n: &mut usize) {
        if self.reading() {
            let mut a = 0;
            self.bits(&mut a, 8);
            if a & 0x80 == 0 {
                *n = a as usize;
            } else if a & 0x40 == 0 {
                let mut b = 0;
                self.bits(&mut b, 8);
                *n = (((a & 0x3F) << 8) | b) as usize;
            } else {
                self.fail();
                *n = 0;
            }
        } else if *n < 128 {
            let mut a = *n as u64;
            self.bits(&mut a, 8);
        } else if *n < 16384 {
            let mut a = 0x8000 | *n as u64;
            self.bits(&mut a, 16);
        } else {
            self.fail();
        }
    }

    /// A constrained length `lo..=hi` with `hi` below 64K (X.691 clause 11.9.4.1): no bits when `lo == hi`, otherwise a constrained
    /// whole number.
    fn count(&mut self, n: &mut usize, lo: usize, hi: usize) {
        if lo == hi {
            *n = lo;
            return;
        }
        let mut x = *n as i64;
        self.int(&mut x, lo as i64, hi as i64);
        *n = x as usize;
    }

    /// A semi-constrained whole number with lower bound `lo` (clause 11.7): a length determinant then the octets of `v - lo`.
    fn int_semi(&mut self, v: &mut i64, lo: i64) {
        let mut x = (*v as i128 - lo as i128).max(0) as u64;
        let mut len = if self.reading() {
            0
        } else {
            ((64 - x.leading_zeros()).div_ceil(8)).max(1) as usize
        };
        self.length(&mut len);
        if len > 8 {
            self.fail();
            return;
        }
        self.bits(&mut x, 8 * len as u32);
        if self.reading() {
            *v = (lo as i128 + x as i128) as i64;
        }
    }

    /// An unconstrained whole number (clause 11.8): a length determinant then the fewest two's complement octets.
    fn int_unconstrained(&mut self, v: &mut i64) {
        let mut len = if self.reading() {
            0
        } else {
            let x = *v;
            let sig = if x < 0 {
                64 - (!x).leading_zeros()
            } else {
                64 - x.leading_zeros()
            };
            ((sig + 1).div_ceil(8)).max(1) as usize
        };
        self.length(&mut len);
        if len == 0 || len > 8 {
            self.fail();
            return;
        }
        let mut x = *v as u64;
        self.bits(&mut x, 8 * len as u32);
        if self.reading() {
            *v = crate::pack::sign_extend(x, 8 * len as u32);
        }
    }

    /// A normally small non-negative whole number (clause 11.6): 0 and 6 bits below 64, otherwise 1 and a semi-constrained number.
    fn small(&mut self, v: &mut u64) {
        let mut big = (*v >= 64) as u64;
        self.bits(&mut big, 1);
        if big == 0 {
            self.bits(v, 6);
        } else {
            let mut x = *v as i64;
            self.int_semi(&mut x, 0);
            *v = x as u64;
        }
    }

    /// An ENUMERATED of `count` root values: its index (clause 14). With `extensible`, a leading bit says whether the value is an
    /// extension; extension values are a normally small number after the root count (`*idx >= count`).
    fn enumerated(&mut self, idx: &mut usize, count: usize, extensible: bool) {
        if extensible {
            let mut ext = (*idx >= count) as u64;
            self.bits(&mut ext, 1);
            if ext == 1 {
                let mut e = (*idx - count.min(*idx)) as u64;
                self.small(&mut e);
                *idx = count + e as usize;
                return;
            }
        }
        let mut x = *idx as i64;
        self.int(&mut x, 0, count as i64 - 1);
        *idx = x as usize;
    }

    /// The index of a CHOICE of `count` root alternatives (clause 23), with the same extension rule as [`PerIo::enumerated`].
    fn choice(&mut self, idx: &mut usize, count: usize, extensible: bool) {
        self.enumerated(idx, count, extensible);
    }

    /// An ENUMERATED whose values mean the numbers in `table`: the meaning is stored.
    fn enum_value(&mut self, value: &mut i64, table: &[i64]) {
        let mut idx = table.iter().position(|t| t == value).unwrap_or(0);
        self.enumerated(&mut idx, table.len(), false);
        if self.reading() {
            match table.get(idx) {
                Some(&t) => *value = t,
                None => self.fail(),
            }
        }
    }

    /// The presence bitmap of a SEQUENCE's `n` OPTIONAL or DEFAULT components (clause 19.2), one flag each.
    fn optionals(&mut self, present: &mut [bool]) {
        for p in present {
            self.flag(p);
        }
    }

    /// An OCTET STRING with size `lo..=hi` (clause 17; `hi` below 64K): fixed-size strings have no length.
    fn octets(&mut self, v: &mut Vec<u8>, lo: usize, hi: usize) {
        let mut n = v.len();
        self.count(&mut n, lo, hi);
        self.octets_n(v, n);
    }

    /// An OCTET STRING with no size constraint: a length determinant then the octets.
    fn octets_unconstrained(&mut self, v: &mut Vec<u8>) {
        let mut n = v.len();
        self.length(&mut n);
        self.octets_n(v, n);
    }

    /// A BIT STRING of size `lo..=hi` up to 64 bits, as a number (first bit most significant), and its length.
    fn bit_string(&mut self, v: &mut u64, len: &mut usize, lo: usize, hi: usize) {
        self.count(len, lo, hi);
        if *len > 64 {
            self.fail();
            return;
        }
        self.bits(v, *len as u32);
    }

    /// An open type (clause 11.2): a length determinant then that many octets, for a component this description does not decode.
    fn open_type(&mut self, v: &mut Vec<u8>) {
        self.octets_unconstrained(v);
    }

    /// Skips the extension additions of a SEQUENCE whose extension bit was set (clause 19.7): a normally small length of the
    /// bitmap, the bitmap, and an open type per addition present. A writer writes none.
    fn skip_extensions(&mut self) {
        if !self.reading() {
            return;
        }
        let mut big = 0;
        self.bits(&mut big, 1);
        if big != 0 {
            self.fail();
            return;
        }
        let mut n = 0;
        self.bits(&mut n, 6);
        let n = n as u32 + 1;
        let mut map = 0;
        self.bits(&mut map, n.min(64));
        let mut skip = Vec::new();
        for i in 0..n.min(64) {
            if (map >> (n.min(64) - 1 - i)) & 1 == 1 {
                self.open_type(&mut skip);
            }
        }
    }
}

/// Reads unaligned PER from bytes. Past the end it reads zeros and fails.
#[derive(Clone, Debug)]
pub struct PerReader<'a> {
    r: BitReader<'a, Msb>,
    failed: bool,
}

impl<'a> PerReader<'a> {
    /// A reader at the first bit of `data`.
    pub fn new(data: &'a [u8]) -> Self {
        PerReader {
            r: BitReader::new(data),
            failed: false,
        }
    }

    /// The bit position.
    pub fn pos(&self) -> usize {
        self.r.pos()
    }

    /// Bits left.
    pub fn left(&self) -> usize {
        self.r.left()
    }
}

impl PerIo for PerReader<'_> {
    fn reading(&self) -> bool {
        true
    }
    fn bits(&mut self, v: &mut u64, n: u32) {
        *v = self.r.u(n);
    }
    fn fail(&mut self) {
        self.failed = true;
    }
    fn failed(&self) -> bool {
        self.failed || self.r.overrun()
    }
    fn octets_n(&mut self, v: &mut Vec<u8>, n: usize) {
        v.clear();
        if n > self.r.left() / 8 + 1 {
            self.failed = true;
            return;
        }
        v.resize(n, 0);
        self.r.read_bytes(v);
    }
}

/// Writes unaligned PER into a caller's `Vec<u8>`.
#[derive(Debug)]
pub struct PerWriter<'a> {
    w: BitWriter<'a, Msb>,
    failed: bool,
}

impl<'a> PerWriter<'a> {
    /// A writer appending to `out`.
    pub fn new(out: &'a mut Vec<u8>) -> Self {
        PerWriter {
            w: BitWriter::new(out),
            failed: false,
        }
    }

    /// Bits written so far.
    pub fn bits_written(&self) -> usize {
        self.w.bits_written()
    }

    /// Ends the message and returns the bits written (the last byte is padded with zeros). X.691 clause 11.1 pads a complete
    /// encoding that is empty to one octet; that is the caller's to add.
    pub fn finish(self) -> usize {
        self.w.finish()
    }
}

impl PerIo for PerWriter<'_> {
    fn reading(&self) -> bool {
        false
    }
    fn bits(&mut self, v: &mut u64, n: u32) {
        self.w.put(*v, n);
    }
    fn fail(&mut self) {
        self.failed = true;
    }
    fn failed(&self) -> bool {
        self.failed
    }
    fn octets_n(&mut self, v: &mut Vec<u8>, n: usize) {
        for i in 0..n {
            self.w.put(v.get(i).copied().unwrap_or(0) as u64, 8);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(f: impl FnOnce(&mut PerWriter)) -> Vec<u8> {
        let mut out = Vec::new();
        let mut w = PerWriter::new(&mut out);
        f(&mut w);
        w.finish();
        out
    }

    #[test]
    fn x691_encodings() {
        // Clause 11.5.7: INTEGER (0..7) value 5 is 101.
        assert_eq!(write(|w| w.int(&mut 5, 0, 7)), [0b1010_0000]);
        // INTEGER (-70..-22) value -62: 49 values, 6 bits, offset 8.
        assert_eq!(write(|w| w.int(&mut -62, -70, -22)), [0b0010_0000]);
        // Clause 11.6: normally small 5 is 0 000101; 70 is 1, length 1, 0x46.
        assert_eq!(write(|w| w.small(&mut 5)), [0b0000_1010]);
        assert_eq!(write(|w| w.small(&mut 70)), [0b1000_0000, 0b1010_0011, 0]);
        // Clause 11.9: length 3 is 00000011; length 200 is 10 then 14 bits.
        assert_eq!(write(|w| w.length(&mut 3)), [3]);
        assert_eq!(write(|w| w.length(&mut 200)), [0x80, 0xC8]);
        // Clause 11.8: unconstrained 128 is length 2, 00 80; -129 is length 2, FF 7F; 0 is length 1, 00.
        assert_eq!(write(|w| w.int_unconstrained(&mut 128)), [2, 0x00, 0x80]);
        assert_eq!(write(|w| w.int_unconstrained(&mut -129)), [2, 0xFF, 0x7F]);
        assert_eq!(write(|w| w.int_unconstrained(&mut 0)), [1, 0]);
        assert_eq!(write(|w| w.int_unconstrained(&mut -128)), [1, 0x80]);
        // Clause 11.7: semi-constrained (lower bound 0) 256 is length 2, 01 00.
        assert_eq!(write(|w| w.int_semi(&mut 256, 0)), [2, 1, 0]);
        // Extensible ENUMERATED: root value 1 of 3 is 0 01; the first extension value is 1 0 000000.
        assert_eq!(write(|w| w.enumerated(&mut 1, 3, true)), [0b0010_0000]);
        assert_eq!(write(|w| w.enumerated(&mut 3, 3, true)), [0b1000_0000]);
    }

    #[derive(Default, Debug, PartialEq)]
    struct Msg {
        a: i64,
        flag: bool,
        list: Vec<i64>,
        e: i64,
        inner: Vec<u8>,
        n: u64,
        u: i64,
        ext: usize,
        bs: u64,
        bs_len: usize,
    }

    fn codec<P: PerIo>(m: &mut Msg, p: &mut P) {
        p.int(&mut m.a, -70, -22);
        p.flag(&mut m.flag);
        let mut n = m.list.len();
        p.count(&mut n, 1, 8);
        m.list.resize(n, 0);
        for x in m.list.iter_mut() {
            p.int(x, 0, 503);
        }
        p.enum_value(&mut m.e, &[100, 200, 300, 400, 600, 1000, 1500, 2000]);
        p.octets_unconstrained(&mut m.inner);
        p.small(&mut m.n);
        p.int_unconstrained(&mut m.u);
        p.choice(&mut m.ext, 4, true);
        p.bit_string(&mut m.bs, &mut m.bs_len, 1, 40);
    }

    #[test]
    fn round_trip_and_truncation() {
        let mut m = Msg {
            a: -62,
            flag: true,
            list: vec![1, 302, 503],
            e: 1000,
            inner: vec![1, 2, 3, 0x7E],
            n: 500,
            u: -40_000,
            ext: 6,
            bs: 0x1_2345_6789,
            bs_len: 33,
        };
        let bytes = write(|w| codec(&mut m, w));
        let mut r = PerReader::new(&bytes);
        let mut back = Msg::default();
        codec(&mut back, &mut r);
        assert!(!r.failed());
        assert_eq!(back, m);
        let mut r = PerReader::new(&bytes[..bytes.len() - 3]);
        let mut back = Msg::default();
        codec(&mut back, &mut r);
        assert!(r.failed());
        assert_eq!(bits_for(49), 6);
        assert_eq!(bits_for(1), 0);
        assert_eq!(bits_for(256), 8);
        // An index past the root count fails.
        let mut r = PerReader::new(&[0xE0]);
        let mut i = 0;
        r.int(&mut (i as i64), 0, 4);
        r.enumerated(&mut i, 5, false);
        assert!(r.failed());
    }

    #[test]
    fn extensions_are_skipped() {
        let bytes = write(|w| {
            // Bitmap of 2 additions, the second present, holding 2 octets; then a marker.
            w.bits(&mut 0, 1);
            w.bits(&mut 1, 6);
            w.bits(&mut 0b01, 2);
            w.bits(&mut 2, 8);
            w.bits(&mut 0xFFFF, 16);
            w.bits(&mut 0b101, 3);
        });
        let mut r = PerReader::new(&bytes);
        r.skip_extensions();
        let mut v = 0;
        r.bits(&mut v, 3);
        assert_eq!(v, 0b101);
        assert!(!r.failed());
    }
}
