//! Bit readers and writers.
//!
//! - [`BitReader`] reads fields from packed bytes, MSB first ([`Msb`], the default) or LSB first ([`Lsb`]). Each read is one
//!   unaligned load and a shift, not a loop over bits. **Past the end it returns zero bits and sets [`BitReader::overrun`]**, so a
//!   parser reads every field and checks once at the end.
//! - [`BitWriter`] writes fields into a caller-owned `Vec<u8>`, packed. The argument order is always `(value, bits)`.
//! - [`HardBits`] reads the same fields from one-bit-per-byte input (demodulated bits: AIS, GNSS, POCSAG), with the same past-the-end
//!   rule.
//!
//! ```
//! use sdrkit_util_bits::{BitReader, BitWriter};
//! let mut buf = Vec::new();
//! let mut w = BitWriter::new(&mut buf);
//! w.put(0b101, 3);
//! w.put_signed(-2, 5);
//! w.put(0xBEEF, 16);
//! assert_eq!(w.finish(), 24);
//! let mut r = BitReader::new(&buf);
//! assert_eq!(r.u(3), 0b101);
//! assert_eq!(r.i(5), -2);
//! assert_eq!(r.u(16), 0xBEEF);
//! assert_eq!(r.u(4), 0); // past the end: zeros ...
//! assert!(r.overrun()); // ... and the flag
//!
//! // LSB first: the first bit read is bit 0 of the first byte, and fields are little-endian.
//! let mut r = BitReader::new_lsb(&[0x34, 0x12]);
//! assert_eq!(r.u(12), 0x234);
//! ```

use crate::pack::{mask, sign_extend, sign_magnitude, to_sign_magnitude, BitOrder};
use std::marker::PhantomData;

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Msb {}
    impl Sealed for super::Lsb {}
}

/// A bit order chosen at compile time: [`Msb`] or [`Lsb`].
pub trait Order: sealed::Sealed + Copy + Default + std::fmt::Debug + Send + Sync + 'static {
    /// The same order as a value.
    const ORDER: BitOrder;
}

/// Most significant bit first: the first bit of a byte is bit 7, and a field's first bit is its most significant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Msb;

/// Least significant bit first: the first bit of a byte is bit 0, and a field's first bit is its least significant (HFDL, VDL
/// Mode 4, UART-framed data).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lsb;

impl Order for Msb {
    const ORDER: BitOrder = BitOrder::Msb;
}
impl Order for Lsb {
    const ORDER: BitOrder = BitOrder::Lsb;
}

/// Reads fields from packed bytes. See the [module documentation](self).
#[derive(Clone, Debug)]
pub struct BitReader<'a, O: Order = Msb> {
    data: &'a [u8],
    pos: usize,
    overrun: bool,
    _order: PhantomData<O>,
}

impl<'a> BitReader<'a, Msb> {
    /// An MSB-first reader at the first bit of `data`.
    pub fn new(data: &'a [u8]) -> Self {
        Self::starting_at(data, 0)
    }

    /// An MSB-first reader at bit `bit` of `data`.
    pub fn at(data: &'a [u8], bit: usize) -> Self {
        Self::starting_at(data, bit)
    }
}

impl<'a> BitReader<'a, Lsb> {
    /// An LSB-first reader at the first bit of `data`.
    pub fn new_lsb(data: &'a [u8]) -> Self {
        Self::starting_at(data, 0)
    }

    /// An LSB-first reader at bit `bit` of `data`.
    pub fn at_lsb(data: &'a [u8], bit: usize) -> Self {
        Self::starting_at(data, bit)
    }
}

impl<'a, O: Order> BitReader<'a, O> {
    /// A reader in either order at bit `bit` of `data` (counted from the first bit of the first byte), for code generic over
    /// the order.
    pub fn starting_at(data: &'a [u8], bit: usize) -> Self {
        BitReader {
            data,
            pos: bit,
            overrun: bit > data.len() * 8,
            _order: PhantomData,
        }
    }

    /// The bit position.
    #[inline]
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Bits left before the end.
    #[inline]
    pub fn left(&self) -> usize {
        (self.data.len() * 8).saturating_sub(self.pos)
    }

    /// Whether a read went past the end (or requested more than 64 bits). Once set it stays set.
    #[inline]
    pub fn overrun(&self) -> bool {
        self.overrun
    }

    /// Whether the position is at a byte boundary.
    #[inline]
    pub fn is_aligned(&self) -> bool {
        self.pos.is_multiple_of(8)
    }

    /// The 72 bits from byte `byte` on, as the low bits of a `u128` in reading order (zeros past the end).
    #[inline]
    fn window(&self, byte: usize) -> u128 {
        let mut buf = [0u8; 16];
        match self.data.get(byte..byte + 9) {
            Some(s) => buf[..9].copy_from_slice(s),
            None => {
                if byte < self.data.len() {
                    let s = &self.data[byte..];
                    buf[..s.len()].copy_from_slice(s);
                }
            }
        }
        match O::ORDER {
            BitOrder::Msb => u128::from_be_bytes(buf) >> 56,
            BitOrder::Lsb => u128::from_le_bytes(buf),
        }
    }

    /// The next `n` bits (at most 64) without moving.
    #[inline]
    pub fn peek(&self, n: u32) -> u64 {
        if n == 0 || n > 64 {
            return 0;
        }
        let w = self.window(self.pos / 8);
        let off = (self.pos % 8) as u32;
        let v = match O::ORDER {
            BitOrder::Msb => (w >> (72 - off - n)) as u64,
            BitOrder::Lsb => (w >> off) as u64,
        };
        v & mask(n)
    }

    /// The next `n` bits (at most 64) as an unsigned number. More than 64 bits reads nothing and sets the overrun flag.
    #[inline]
    pub fn u(&mut self, n: u32) -> u64 {
        if n > 64 {
            self.overrun = true;
            return 0;
        }
        let v = self.peek(n);
        self.advance(n as usize);
        v
    }

    /// The next `n` bits as a two's complement number.
    #[inline]
    pub fn i(&mut self, n: u32) -> i64 {
        sign_extend(self.u(n), n)
    }

    /// The next `n` bits as a sign-and-magnitude number (the first bit is the sign).
    #[inline]
    pub fn sm(&mut self, n: u32) -> i64 {
        sign_magnitude(self.u(n), n)
    }

    /// The next bit.
    #[inline]
    pub fn bit(&mut self) -> bool {
        self.u(1) != 0
    }

    #[inline]
    fn advance(&mut self, n: usize) {
        self.pos = self.pos.saturating_add(n);
        if self.pos > self.data.len() * 8 {
            self.overrun = true;
        }
    }

    /// Skips `n` bits.
    pub fn skip(&mut self, n: usize) {
        self.advance(n);
    }

    /// Moves to the next byte boundary (nothing when already there).
    pub fn align(&mut self) {
        let r = self.pos % 8;
        if r != 0 {
            self.advance(8 - r);
        }
    }

    /// The next `n` whole bytes, without copying. At a byte boundary only; when not aligned, or when fewer than `n` bytes are left,
    /// it returns an empty slice, sets the overrun flag and does not move.
    pub fn bytes(&mut self, n: usize) -> &'a [u8] {
        let start = self.pos / 8;
        match self.data.get(start..start.saturating_add(n)) {
            Some(s) if self.is_aligned() => {
                self.pos += n * 8;
                s
            }
            _ => {
                self.overrun = true;
                &[]
            }
        }
    }

    /// Fills `out` with the next bytes, at any bit position (each byte read in this reader's order).
    pub fn read_bytes(&mut self, out: &mut [u8]) {
        if self.is_aligned() {
            let start = self.pos / 8;
            let avail = self.data.len().saturating_sub(start).min(out.len());
            out[..avail].copy_from_slice(&self.data[start..start + avail]);
            out[avail..].fill(0);
            self.advance(out.len() * 8);
        } else {
            for b in out {
                *b = self.u(8) as u8;
            }
        }
    }

    /// The rest of the data from the current byte on (the whole current byte, even when part of it was read).
    pub fn rest(&self) -> &'a [u8] {
        self.data.get(self.pos / 8..).unwrap_or(&[])
    }

    /// Counts zero bits up to and including the next one bit, and returns the count of zeros (a unary code, as Rice and Golomb
    /// codes use). Stops at the end with the overrun flag set.
    pub fn unary(&mut self) -> u32 {
        let mut zeros = 0u32;
        loop {
            if self.left() == 0 {
                self.overrun = true;
                return zeros;
            }
            let w = self.left().min(32) as u32;
            let v = self.peek(w);
            // Leading zeros of the field in reading order.
            let z = match O::ORDER {
                BitOrder::Msb => (v << (64 - w)).leading_zeros().min(w),
                BitOrder::Lsb => v.trailing_zeros().min(w),
            };
            if z < w {
                self.advance(z as usize + 1);
                return zeros + z;
            }
            zeros += w;
            self.advance(w as usize);
        }
    }

    /// An unsigned Exp-Golomb code `ue(v)` (ITU-T H.264 clause 9.1). A code longer than 32 bits sets the overrun flag and returns 0.
    pub fn ue(&mut self) -> u32 {
        let zeros = self.unary();
        if zeros > 31 {
            self.overrun = true;
            return 0;
        }
        (((1u64 << zeros) - 1) + self.u(zeros)) as u32
    }

    /// A signed Exp-Golomb code `se(v)` (ITU-T H.264 clause 9.1.1): 1, −1, 2, −2 ... for codes 1, 2, 3, 4 ...
    pub fn se(&mut self) -> i32 {
        let k = self.ue() as i64;
        if k & 1 == 1 {
            ((k + 1) / 2) as i32
        } else {
            (-(k / 2)) as i32
        }
    }
}

/// Writes fields into a caller-owned `Vec<u8>`, packed, after whatever it already holds. See the [module documentation](self).
///
/// The final partial byte is in the `Vec` at all times (padded with zero bits), so there is nothing to flush.
#[derive(Debug)]
pub struct BitWriter<'a, O: Order = Msb> {
    out: &'a mut Vec<u8>,
    /// Bits used in the last byte (0 when byte-aligned).
    fill: u32,
    written: usize,
    _order: PhantomData<O>,
}

impl<'a> BitWriter<'a, Msb> {
    /// An MSB-first writer that appends to `out`.
    pub fn new(out: &'a mut Vec<u8>) -> Self {
        Self::appending(out)
    }
}

impl<'a> BitWriter<'a, Lsb> {
    /// An LSB-first writer that appends to `out`.
    pub fn new_lsb(out: &'a mut Vec<u8>) -> Self {
        Self::appending(out)
    }
}

impl<'a, O: Order> BitWriter<'a, O> {
    /// A writer in either order that appends to `out`, for code generic over the order.
    pub fn appending(out: &'a mut Vec<u8>) -> Self {
        BitWriter {
            out,
            fill: 0,
            written: 0,
            _order: PhantomData,
        }
    }

    /// Writes the `n` low bits of `v`. For `n` above 64 the extra bits are zeros (leading for MSB first, trailing for LSB first).
    #[inline]
    pub fn put(&mut self, v: u64, n: u32) {
        if n > 64 {
            match O::ORDER {
                BitOrder::Msb => {
                    self.put(0, n - 64);
                    self.put(v, 64);
                }
                BitOrder::Lsb => {
                    self.put(v, 64);
                    self.put(0, n - 64);
                }
            }
            return;
        }
        let mut v = v & mask(n);
        let mut n = n;
        self.written += n as usize;
        while n > 0 {
            if self.fill == 0 {
                self.out.push(0);
            }
            let space = 8 - self.fill;
            let take = space.min(n);
            let last = self.out.len() - 1;
            match O::ORDER {
                BitOrder::Msb => {
                    let chunk = ((v >> (n - take)) & mask(take)) as u8;
                    self.out[last] |= chunk << (space - take);
                }
                BitOrder::Lsb => {
                    let chunk = (v & mask(take)) as u8;
                    self.out[last] |= chunk << self.fill;
                    v = if take >= 64 { 0 } else { v >> take };
                }
            }
            n -= take;
            self.fill = (self.fill + take) % 8;
        }
    }

    /// Writes `v` as an `n`-bit two's complement number.
    #[inline]
    pub fn put_signed(&mut self, v: i64, n: u32) {
        self.put(v as u64, n);
    }

    /// Writes `v` as an `n`-bit sign-and-magnitude number (the first bit is the sign). A magnitude too large for `n − 1` bits
    /// saturates (see [`to_sign_magnitude`]).
    pub fn put_sm(&mut self, v: i64, n: u32) {
        if n == 0 {
            return;
        }
        let u = to_sign_magnitude(v, n);
        match O::ORDER {
            BitOrder::Msb => self.put(u, n),
            BitOrder::Lsb => {
                self.put(u >> (n - 1), 1);
                self.put(u & mask(n - 1), n - 1);
            }
        }
    }

    /// Writes one bit (its lowest bit).
    #[inline]
    pub fn bit(&mut self, b: u8) {
        self.put((b & 1) as u64, 1);
    }

    /// Writes whole bytes (each in this writer's order); a plain copy when aligned.
    pub fn bytes(&mut self, b: &[u8]) {
        if self.fill == 0 {
            self.out.extend_from_slice(b);
            self.written += b.len() * 8;
        } else {
            for &x in b {
                self.put(x as u64, 8);
            }
        }
    }

    /// Pads with zero bits to the next byte boundary.
    pub fn align(&mut self) {
        if self.fill != 0 {
            self.written += (8 - self.fill) as usize;
            self.fill = 0;
        }
    }

    /// Whether the writer is at a byte boundary.
    #[inline]
    pub fn is_aligned(&self) -> bool {
        self.fill == 0
    }

    /// Bits written by this writer so far.
    #[inline]
    pub fn bits_written(&self) -> usize {
        self.written
    }

    /// Writes `v` as an unsigned Exp-Golomb code `ue(v)` (H.264 clause 9.1). Values above `u32::MAX - 1` are written as
    /// `u32::MAX - 1`.
    pub fn ue(&mut self, v: u32) {
        let x = v.min(u32::MAX - 1) as u64 + 1;
        let len = 64 - x.leading_zeros();
        self.put(0, len - 1);
        self.put(x, len);
    }

    /// Writes `v` as a signed Exp-Golomb code `se(v)`.
    pub fn se(&mut self, v: i32) {
        let k = if v > 0 {
            2 * v as i64 - 1
        } else {
            -2 * v as i64
        };
        self.ue(k.min(u32::MAX as i64 - 1) as u32);
    }

    /// Ends writing and returns the number of bits written (the last byte is already padded with zeros).
    pub fn finish(self) -> usize {
        self.written
    }
}

/// Reads fields from hard bits held one per byte (only the lowest bit of each counts). Fields are most significant bit first,
/// except [`HardBits::u_lsb`]. Past the end it returns zeros and sets [`HardBits::overrun`].
///
/// ```
/// use sdrkit_util_bits::HardBits;
/// let bits = [1, 0, 1, 1, 0, 1];
/// let mut r = HardBits::new(&bits);
/// assert_eq!(r.u(3), 0b101);
/// assert_eq!(r.sm(3), -1);
/// assert!(!r.overrun());
/// ```
#[derive(Clone, Debug)]
pub struct HardBits<'a> {
    bits: &'a [u8],
    pos: usize,
    overrun: bool,
}

impl<'a> HardBits<'a> {
    /// A reader at the first bit.
    pub fn new(bits: &'a [u8]) -> Self {
        Self::at(bits, 0)
    }

    /// A reader at bit `pos`.
    pub fn at(bits: &'a [u8], pos: usize) -> Self {
        HardBits {
            bits,
            pos,
            overrun: pos > bits.len(),
        }
    }

    /// The position.
    #[inline]
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Bits left.
    #[inline]
    pub fn left(&self) -> usize {
        self.bits.len().saturating_sub(self.pos)
    }

    /// Whether a read went past the end (or requested more than 64 bits).
    #[inline]
    pub fn overrun(&self) -> bool {
        self.overrun
    }

    fn take(&mut self, n: u32) -> &'a [u8] {
        let end = self.pos.saturating_add(n as usize);
        if end > self.bits.len() || n > 64 {
            self.overrun = true;
        }
        let s = self
            .bits
            .get(self.pos.min(self.bits.len())..end.min(self.bits.len()))
            .unwrap_or(&[]);
        if n <= 64 {
            self.pos = end;
        }
        s
    }

    /// The next `n` bits (at most 64), most significant first.
    #[inline]
    pub fn u(&mut self, n: u32) -> u64 {
        let short = (n as usize).saturating_sub(self.left());
        let s = self.take(n);
        if n > 64 {
            return 0;
        }
        // Missing bits at the end read as zeros.
        let v = crate::pack::to_u64(s);
        if short >= 64 {
            0
        } else {
            v << short
        }
    }

    /// The next `n` bits (at most 64), least significant first.
    #[inline]
    pub fn u_lsb(&mut self, n: u32) -> u64 {
        let s = self.take(n);
        if n > 64 {
            return 0;
        }
        crate::pack::to_u64_lsb(s)
    }

    /// The next `n` bits as a two's complement number.
    #[inline]
    pub fn i(&mut self, n: u32) -> i64 {
        sign_extend(self.u(n), n)
    }

    /// The next `n` bits as a sign-and-magnitude number.
    #[inline]
    pub fn sm(&mut self, n: u32) -> i64 {
        sign_magnitude(self.u(n), n)
    }

    /// The next bit.
    #[inline]
    pub fn bit(&mut self) -> bool {
        self.u(1) != 0
    }

    /// Skips `n` bits.
    pub fn skip(&mut self, n: usize) {
        self.pos = self.pos.saturating_add(n);
        if self.pos > self.bits.len() {
            self.overrun = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *seed
    }

    fn round_trip<O: Order>() {
        let mut seed = 7u64;
        let mut fields = Vec::new();
        let mut buf = Vec::new();
        let mut w = BitWriter::<O>::appending(&mut buf);
        for k in 0..2000 {
            let n = (k % 64) as u32 + 1;
            let v = lcg(&mut seed) & mask(n);
            fields.push((v, n));
            w.put(v, n);
        }
        let total = w.finish();
        assert_eq!(total, fields.iter().map(|f| f.1 as usize).sum::<usize>());
        let mut r = BitReader::<O>::starting_at(&buf, 0);
        for &(v, n) in &fields {
            assert_eq!(r.u(n), v, "width {n}");
        }
        assert!(!r.overrun());
        assert!(r.left() < 8);
    }

    #[test]
    fn every_width_round_trips_both_orders() {
        round_trip::<Msb>();
        round_trip::<Lsb>();
    }

    #[test]
    fn known_layouts() {
        let data = [0b1011_0010, 0b0111_0000];
        let mut r = BitReader::new(&data);
        assert_eq!(r.u(4), 0b1011);
        assert_eq!(r.u(6), 0b00_1001);
        let mut r = BitReader::new_lsb(&data);
        assert_eq!(r.u(4), 0b0010);
        assert_eq!(r.u(6), 0b00_1011);
        let mut b = Vec::new();
        let mut w = BitWriter::new_lsb(&mut b);
        w.put(0x234, 12);
        w.align();
        assert_eq!(w.finish(), 16);
        assert_eq!(b, [0x34, 0x02]);
    }

    #[test]
    fn past_the_end_reads_zeros_and_flags() {
        let mut r = BitReader::new(&[0xFF]);
        assert_eq!(r.u(12), 0xFF0);
        assert!(r.overrun());
        let mut r = BitReader::new(&[0xFF]);
        assert_eq!(r.u(65), 0);
        assert!(r.overrun());
        let mut r = BitReader::new(&[1, 2, 3]);
        r.skip(4);
        assert!(r.bytes(1).is_empty());
        assert!(r.overrun());
        let mut r = BitReader::new(&[1, 2, 3]);
        assert_eq!(r.bytes(2), &[1, 2]);
        assert!(r.bytes(2).is_empty());
        let mut h = HardBits::new(&[1, 1]);
        assert_eq!(h.u(4), 0b1100);
        assert!(h.overrun());
    }

    #[test]
    fn signed_and_sign_magnitude() {
        let mut b = Vec::new();
        let mut w = BitWriter::new(&mut b);
        w.put_signed(-5, 7);
        w.put_sm(-5, 7);
        w.put_sm(5, 7);
        w.finish();
        let mut r = BitReader::new(&b);
        assert_eq!(r.i(7), -5);
        assert_eq!(r.sm(7), -5);
        assert_eq!(r.sm(7), 5);
        let mut b = Vec::new();
        let mut w = BitWriter::new_lsb(&mut b);
        w.put_sm(-5, 7);
        w.finish();
        let mut r = BitReader::new_lsb(&b);
        assert_eq!(r.u(1), 1);
        assert_eq!(r.u(6), 5);
    }

    #[test]
    fn exp_golomb_matches_h264_table_9_2() {
        // H.264 Table 9-2: codeNum 0 = 1, 1 = 010, 2 = 011, 3 = 00100, 4 = 00101 ... 7 = 0001000.
        let mut r = BitReader::new(&[0b1010_0110, 0b0100_0010, 0b1000_0100, 0]);
        assert_eq!([r.ue(), r.ue(), r.ue(), r.ue(), r.ue()], [0, 1, 2, 3, 4]);
        assert_eq!(r.u(1), 0);
        assert_eq!(r.ue(), 7);
        let mut b = Vec::new();
        let mut w = BitWriter::new(&mut b);
        for v in [0, 1, -1, 2, -2, 1000, -1000] {
            w.se(v);
        }
        w.finish();
        let mut r = BitReader::new(&b);
        for v in [0, 1, -1, 2, -2, 1000, -1000] {
            assert_eq!(r.se(), v);
        }
        assert!(!r.overrun());
        // A run of zeros to the end is an overrun, not a hang.
        let mut r = BitReader::new(&[0, 0, 0, 0, 0]);
        assert_eq!(r.ue(), 0);
        assert!(r.overrun());
    }

    #[test]
    fn unary_and_unaligned_bytes() {
        let mut r = BitReader::new(&[0b0001_0000, 0, 0, 0, 0, 0b0100_0000]);
        assert_eq!(r.unary(), 3);
        assert_eq!(r.unary(), 37);
        let mut r = BitReader::new_lsb(&[0b0000_1000]);
        assert_eq!(r.unary(), 3);
        let mut r = BitReader::new(&[0x0A, 0xBC, 0xD0]);
        r.skip(4);
        let mut o = [0u8; 2];
        r.read_bytes(&mut o);
        assert_eq!(o, [0xAB, 0xCD]);
        let mut b = vec![0xEE];
        let mut w = BitWriter::new(&mut b);
        w.put(0xF, 4);
        w.bytes(&[0x12]);
        w.finish();
        assert_eq!(b, [0xEE, 0xF1, 0x20]);
    }

    #[test]
    fn hard_bits_fields() {
        let bits = [1u8, 0, 1, 1, 0, 0, 0, 1];
        let mut h = HardBits::new(&bits);
        assert_eq!(h.u_lsb(4), 0b1101);
        assert_eq!(h.i(4), 1);
        let mut h = HardBits::at(&bits, 4);
        assert_eq!(h.sm(4), 1);
        assert_eq!(h.left(), 0);
    }
}
