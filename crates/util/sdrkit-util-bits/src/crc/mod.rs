//! Cyclic redundancy checks.
//!
//! - [`CrcSpec`] is a CRC in the usual catalogue form (width, polynomial, initial value, input and output reflection, final XOR) with
//!   its check value, the CRC of the ASCII bytes `"123456789"`.
//! - [`Crc`] is a ready engine: its 256-entry table is built at compile time by a `const fn`, so a `static` costs nothing at run
//!   time and needs no lock. One table step handles a byte. Reflected and normal CRCs share the engine: a reflected spec gets a
//!   reflected table, so no byte is ever reversed at run time.
//! - [`catalog`] names every CRC the sdrkit standards use.
//! - [`cyclic`] holds remainders of bit strings that are not whole bytes (the weak-signal CRCs, BCH and Golay parity words, RDS
//!   offset words, MPT 1327, POCSAG and others).
//!
//! ```
//! use sdrkit_util_bits::crc::{catalog, Trailer};
//! assert_eq!(catalog::CRC32_ISO_HDLC.checksum(b"123456789"), 0xCBF4_3926);
//! let mut frame = b"hello".to_vec();
//! catalog::CRC16_IBM_SDLC.append(&mut frame, Trailer::Little).unwrap();
//! assert!(catalog::CRC16_IBM_SDLC.verify(&frame, Trailer::Little));
//! ```
//!
//! # Bit input
//!
//! [`Crc::bits`] takes one bit per byte, in the order the CRC consumes them. For a normal (non-reflected) CRC that is each byte's
//! most significant bit first; for a reflected CRC (HDLC, BLE) it is each byte's least significant bit first, which is the order
//! those links send. So `crc.bits(&unpack(bytes, order))` equals `crc.checksum(bytes)` when `order` is the CRC's own order, and the
//! bit form also works for messages that are not whole bytes.

pub mod catalog;
pub mod cyclic;

use crate::pack::{gather8, BitOrder};
use crate::Param;
use crate::{Error, Result};

/// A CRC in catalogue form.
///
/// The polynomial is in normal form without its top term (x^16 + x^12 + x^5 + 1 is `0x1021`). `init` and `xorout` are as the
/// catalogue provides them: `init` is the register's start value in normal (unreflected) form, and `xorout` is applied to the final
/// value after any output reflection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrcSpec {
    /// The catalogue name, for example `"CRC-16/IBM-SDLC"`.
    pub name: &'static str,
    /// Width in bits, 1 to 64.
    pub width: u8,
    /// Generator polynomial, normal form, top term left out.
    pub poly: u64,
    /// Initial register value (normal form).
    pub init: u64,
    /// Each input byte is taken least significant bit first.
    pub refin: bool,
    /// The final register is bit-reversed before `xorout`.
    pub refout: bool,
    /// Value XORed onto the result.
    pub xorout: u64,
    /// The CRC of the ASCII bytes `"123456789"`.
    pub check: u64,
    /// The register (before `xorout`, reflected when `refout`) after a message followed by its own correct CRC, sent in the CRC's
    /// natural order. It is the same for every message: a receiver can check a frame by comparing with it.
    pub residue: u64,
}

const fn reflect(x: u64, n: u32) -> u64 {
    crate::pack::reverse_bits(x, n)
}

const fn wmask(width: u32) -> u64 {
    crate::pack::mask(width)
}

impl CrcSpec {
    /// A spec from its catalogue parameters; the residue is computed.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        name: &'static str,
        width: u8,
        poly: u64,
        init: u64,
        refin: bool,
        refout: bool,
        xorout: u64,
        check: u64,
    ) -> CrcSpec {
        let mut s = CrcSpec {
            name,
            width,
            poly,
            init,
            refin,
            refout,
            xorout,
            check,
            residue: 0,
        };
        s.residue = s.compute_residue();
        s
    }

    /// The `width` low bits set.
    pub const fn mask(&self) -> u64 {
        wmask(self.width as u32)
    }

    const fn compute_residue(&self) -> u64 {
        let w = self.width as u32;
        if w == 0 || w > 64 {
            return 0;
        }
        let m = wmask(w);
        let top = 1u64 << (w - 1);
        let mut reg = if self.refout {
            reflect(self.xorout, w)
        } else {
            self.xorout & m
        };
        let mut i = 0;
        while i < w {
            let t = reg & top != 0;
            reg = (reg << 1) & m;
            if t {
                reg ^= self.poly & m;
            }
            i += 1;
        }
        if self.refout {
            reflect(reg, w)
        } else {
            reg
        }
    }

    /// Whether the parameters can be used (width 1 to 64, polynomial, init and xorout within the width, odd polynomial).
    pub const fn is_valid(&self) -> bool {
        let w = self.width as u32;
        w >= 1
            && w <= 64
            && self.poly & !wmask(w) == 0
            && self.init & !wmask(w) == 0
            && self.xorout & !wmask(w) == 0
    }

    #[inline]
    fn slow_bit(&self, reg: u64, bit: u8) -> u64 {
        let w = self.width as u32;
        let t = ((reg >> (w - 1)) as u8 ^ bit) & 1;
        let r = (reg << 1) & self.mask();
        if t != 0 {
            r ^ self.poly
        } else {
            r
        }
    }

    fn slow_finish(&self, reg: u64) -> u64 {
        let r = if self.refout {
            reflect(reg, self.width as u32)
        } else {
            reg
        };
        r ^ self.xorout
    }

    /// The CRC of `data`, one bit at a time. Slow; for checking, for specs built at run time, and for tools that try every
    /// catalogue CRC. Returns 0 for an invalid spec.
    pub fn compute(&self, data: &[u8]) -> u64 {
        if !self.is_valid() {
            return 0;
        }
        let mut reg = self.init;
        for &b in data {
            for k in 0..8 {
                let bit = if self.refin { b >> k } else { b >> (7 - k) };
                reg = self.slow_bit(reg, bit);
            }
        }
        self.slow_finish(reg)
    }

    /// The CRC of a bit string (one bit per byte, in the order the CRC consumes them), one bit at a time. Returns 0 for an invalid
    /// spec.
    pub fn compute_bits(&self, bits: &[u8]) -> u64 {
        if !self.is_valid() {
            return 0;
        }
        let reg = bits.iter().fold(self.init, |r, &b| self.slow_bit(r, b));
        self.slow_finish(reg)
    }
}

/// Where a frame carries its CRC, for [`Crc::verify`] and [`Crc::append`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trailer {
    /// The last `width / 8` bytes, most significant byte first (non-reflected CRCs: MPEG, CCSDS, DAB, Mode S).
    Big,
    /// The last `width / 8` bytes, least significant byte first (reflected CRCs: HDLC FCS, 802.11, BLE, PNG uses `Big`).
    Little,
}

/// A CRC engine with its byte table. Build it with [`Crc::new`] in a `static` (the table is made at compile time), or with
/// [`Crc::try_new`] at run time.
///
/// `W` is the register type: `u8`, `u16`, `u32` or `u64`, at least as wide as the CRC.
#[derive(Clone, Debug)]
pub struct Crc<W> {
    spec: CrcSpec,
    table: [W; 256],
}

/// A CRC being computed piece by piece. Feed it with [`Digest::update`] and [`Digest::update_bits`] in any mix, then read
/// [`Digest::finalize`].
#[derive(Clone, Debug)]
pub struct Digest<'a, W> {
    crc: &'a Crc<W>,
    reg: W,
}

macro_rules! engine {
    ($w:ty) => {
        impl Crc<$w> {
            const BITS: u32 = <$w>::BITS;

            /// The engine for `spec`, with its table built at compile time when used in a `static` or `const`.
            ///
            /// # Panics
            ///
            /// When the spec is invalid or wider than the register type. In a `static` this is a compile error; for specs made at
            /// run time use [`Crc::try_new`].
            pub const fn new(spec: CrcSpec) -> Self {
                match Self::try_new(spec) {
                    Ok(c) => c,
                    Err(_) => panic!("invalid CRC spec, or wider than the register type"),
                }
            }

            /// The engine for `spec`, or an error when the spec is invalid or wider than the register type.
            pub const fn try_new(spec: CrcSpec) -> Result<Self> {
                let w = spec.width as u32;
                if !spec.is_valid() || w > Self::BITS {
                    return Err(Error::BadParameter(Param::CrcWidth));
                }
                let mut table = [0 as $w; 256];
                let mut i = 0;
                if spec.refin {
                    let rpoly = reflect(spec.poly, w) as $w;
                    while i < 256 {
                        let mut c = i as $w;
                        let mut k = 0;
                        while k < 8 {
                            c = if c & 1 != 0 { (c >> 1) ^ rpoly } else { c >> 1 };
                            k += 1;
                        }
                        table[i] = c;
                        i += 1;
                    }
                } else {
                    let shift = Self::BITS - w;
                    let apoly = (spec.poly as $w) << shift;
                    let top: $w = 1 << (Self::BITS - 1);
                    while i < 256 {
                        let mut c = (i as $w) << (Self::BITS - 8);
                        let mut k = 0;
                        while k < 8 {
                            c = if c & top != 0 {
                                (c << 1) ^ apoly
                            } else {
                                c << 1
                            };
                            k += 1;
                        }
                        table[i] = c;
                        i += 1;
                    }
                }
                Ok(Crc { spec, table })
            }

            /// The spec.
            pub const fn spec(&self) -> &CrcSpec {
                &self.spec
            }

            #[inline]
            fn start(&self) -> $w {
                let w = self.spec.width as u32;
                if self.spec.refin {
                    reflect(self.spec.init, w) as $w
                } else {
                    (self.spec.init as $w) << (Self::BITS - w)
                }
            }

            #[inline]
            fn step_bytes(&self, mut reg: $w, data: &[u8]) -> $w {
                if self.spec.refin {
                    for &b in data {
                        let hi = if Self::BITS > 8 {
                            reg >> 8.min(Self::BITS - 1)
                        } else {
                            0
                        };
                        reg = hi ^ self.table[((reg as u8) ^ b) as usize];
                    }
                } else {
                    for &b in data {
                        let lo = if Self::BITS > 8 {
                            reg << 8.min(Self::BITS - 1)
                        } else {
                            0
                        };
                        reg = lo ^ self.table[(((reg >> (Self::BITS - 8)) as u8) ^ b) as usize];
                    }
                }
                reg
            }

            #[inline]
            fn step_bit(&self, reg: $w, bit: u8) -> $w {
                let w = self.spec.width as u32;
                if self.spec.refin {
                    let t = (reg as u8 ^ bit) & 1;
                    let r = reg >> 1;
                    if t != 0 {
                        r ^ reflect(self.spec.poly, w) as $w
                    } else {
                        r
                    }
                } else {
                    let t = ((reg >> (Self::BITS - 1)) as u8 ^ bit) & 1;
                    let r = reg << 1;
                    if t != 0 {
                        r ^ ((self.spec.poly as $w) << (Self::BITS - w))
                    } else {
                        r
                    }
                }
            }

            fn step_bits(&self, mut reg: $w, bits: &[u8]) -> $w {
                let order = if self.spec.refin {
                    BitOrder::Lsb
                } else {
                    BitOrder::Msb
                };
                let (chunks, rest) = bits.as_chunks::<8>();
                for c in chunks {
                    let mut a = [0u8; 8];
                    a.copy_from_slice(c);
                    reg = self.step_bytes(reg, &[gather8(a, order)]);
                }
                for &b in rest {
                    reg = self.step_bit(reg, b);
                }
                reg
            }

            /// The register in output form (aligned, reflected as `refout` says), before `xorout`.
            #[inline]
            fn register(&self, reg: $w) -> $w {
                let w = self.spec.width as u32;
                let normal = if self.spec.refin {
                    reg
                } else {
                    reg >> (Self::BITS - w)
                };
                if self.spec.refin != self.spec.refout {
                    reflect(normal as u64, w) as $w
                } else {
                    normal
                }
            }

            #[inline]
            fn finish(&self, reg: $w) -> $w {
                self.register(reg) ^ self.spec.xorout as $w
            }

            /// The CRC of `data`.
            #[inline]
            pub fn checksum(&self, data: &[u8]) -> $w {
                self.finish(self.step_bytes(self.start(), data))
            }

            /// The CRC of a bit string, one bit per byte (only the lowest bit counts), in the order the CRC consumes them (see the
            /// [module documentation](self)). Runs of 8 bits go through the table.
            pub fn bits(&self, bits: &[u8]) -> $w {
                self.finish(self.step_bits(self.start(), bits))
            }

            /// A digest for computing the CRC piece by piece.
            pub fn digest(&self) -> Digest<'_, $w> {
                Digest {
                    crc: self,
                    reg: self.start(),
                }
            }

            fn trailer_len(&self) -> Option<usize> {
                (self.spec.width % 8 == 0).then_some(self.spec.width as usize / 8)
            }

            /// Whether the last `width / 8` bytes of `frame` are the CRC of the bytes before them. False for widths that are not
            /// whole bytes and for frames too short to hold a CRC.
            pub fn verify(&self, frame: &[u8], at: Trailer) -> bool {
                self.verify_masked(frame, at, 0)
            }

            /// As [`Crc::verify`], for a CRC sent XORed with `mask` (the RNTI mask of LTE and NR, the data-type masks of DMR).
            pub fn verify_masked(&self, frame: &[u8], at: Trailer, mask: $w) -> bool {
                let Some(n) = self.trailer_len() else {
                    return false;
                };
                if frame.len() < n {
                    return false;
                }
                let (data, tail) = frame.split_at(frame.len() - n);
                let mut v: u64 = 0;
                for i in 0..n {
                    let b = match at {
                        Trailer::Big => tail[i],
                        Trailer::Little => tail[n - 1 - i],
                    };
                    v = (v << 8) | b as u64;
                }
                (self.checksum(data) ^ mask) as u64 == v
            }

            /// Appends the CRC of `frame` to it. An error (and nothing appended) for widths that are not whole bytes.
            pub fn append(&self, frame: &mut Vec<u8>, at: Trailer) -> Result<()> {
                let n = self
                    .trailer_len()
                    .ok_or(Error::BadParameter(Param::CrcWholeBytes))?;
                let c = self.checksum(frame) as u64;
                for i in 0..n {
                    let shift = match at {
                        Trailer::Big => 8 * (n - 1 - i),
                        Trailer::Little => 8 * i,
                    };
                    frame.push((c >> shift) as u8);
                }
                Ok(())
            }

            /// The CRC as `width` hard bits in the order they follow the message on a link: most significant first for a normal
            /// CRC, least significant first for a reflected one (HDLC, BLE).
            fn crc_bits(&self, c: $w, out: &mut Vec<u8>) {
                let w = self.spec.width as u32;
                if self.spec.refout {
                    crate::pack::push_u64_lsb(out, c as u64, w);
                } else {
                    crate::pack::push_u64(out, c as u64, w);
                }
            }

            /// Appends the CRC of the bit string `bits` to it, in link order (see [`Crc::verify_bits`]).
            pub fn append_bits(&self, bits: &mut Vec<u8>) {
                let c = self.bits(bits);
                self.crc_bits(c, bits);
            }

            /// Whether the last `width` bits of `bits` are the CRC of the bits before them, sent most significant bit first for a
            /// normal CRC and least significant bit first for a reflected one.
            pub fn verify_bits(&self, bits: &[u8]) -> bool {
                self.verify_bits_masked(bits, 0)
            }

            /// As [`Crc::verify_bits`], with the CRC sent XORed with `mask`.
            pub fn verify_bits_masked(&self, bits: &[u8], mask: $w) -> bool {
                let w = self.spec.width as usize;
                if bits.len() < w {
                    return false;
                }
                let (data, tail) = bits.split_at(bits.len() - w);
                let got = if self.spec.refout {
                    crate::pack::to_u64_lsb(tail)
                } else {
                    crate::pack::to_u64(tail)
                };
                (self.bits(data) ^ mask) as u64 == got
            }
        }

        impl Digest<'_, $w> {
            /// Adds bytes.
            #[inline]
            pub fn update(&mut self, data: &[u8]) {
                self.reg = self.crc.step_bytes(self.reg, data);
            }

            /// Adds hard bits (one per byte, in the CRC's own order).
            pub fn update_bits(&mut self, bits: &[u8]) {
                self.reg = self.crc.step_bits(self.reg, bits);
            }

            /// The CRC of everything added so far. The digest can go on after it.
            #[inline]
            pub fn finalize(&self) -> $w {
                self.crc.finish(self.reg)
            }

            /// The register as the catalogue's `residue` is given (before `xorout`). After a message and its correct CRC it
            /// equals [`CrcSpec::residue`].
            #[inline]
            pub fn register(&self) -> $w {
                self.crc.register(self.reg)
            }

            /// Starts again.
            pub fn reset(&mut self) {
                self.reg = self.crc.start();
            }
        }
    };
}

engine!(u8);
engine!(u16);
engine!(u32);
engine!(u64);

/// CRC-32 with slicing-by-8: eight 256-entry tables (8 KiB) process 8 bytes per step. For bulk data such as the 802.11 FCS, PNG
/// and MPEG sections; the result is identical to [`Crc::checksum`].
///
/// ```
/// use sdrkit_util_bits::crc::{catalog, Sliced32};
/// static FAST: Sliced32 = Sliced32::new(catalog::specs::CRC32_ISO_HDLC);
/// assert_eq!(FAST.checksum(b"123456789"), 0xCBF4_3926);
/// ```
#[derive(Clone, Debug)]
pub struct Sliced32 {
    crc: Crc<u32>,
    tables: [[u32; 256]; 8],
}

impl Sliced32 {
    /// The engine for a 32-bit spec (its table built at compile time in a `static`).
    ///
    /// # Panics
    ///
    /// When the spec is not a valid 32-bit CRC; in a `static` this is a compile error.
    pub const fn new(spec: CrcSpec) -> Self {
        assert!(
            spec.width == 32 && spec.is_valid(),
            "Sliced32 needs a 32-bit CRC"
        );
        let crc = Crc::<u32>::new(spec);
        let mut tables = [[0u32; 256]; 8];
        tables[0] = crc.table;
        let mut k = 1;
        while k < 8 {
            let mut i = 0;
            while i < 256 {
                let prev = tables[k - 1][i];
                tables[k][i] = if spec.refin {
                    (prev >> 8) ^ tables[0][(prev & 0xFF) as usize]
                } else {
                    (prev << 8) ^ tables[0][(prev >> 24) as usize]
                };
                i += 1;
            }
            k += 1;
        }
        Sliced32 { crc, tables }
    }

    /// The byte-at-a-time engine of the same spec.
    pub const fn engine(&self) -> &Crc<u32> {
        &self.crc
    }

    /// The CRC of `data`.
    pub fn checksum(&self, data: &[u8]) -> u32 {
        let t = &self.tables;
        let mut reg = self.crc.start();
        let (chunks, rest) = data.as_chunks::<8>();
        if self.crc.spec.refin {
            for c in chunks {
                let a = reg ^ u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                reg = t[7][(a & 0xFF) as usize]
                    ^ t[6][((a >> 8) & 0xFF) as usize]
                    ^ t[5][((a >> 16) & 0xFF) as usize]
                    ^ t[4][(a >> 24) as usize]
                    ^ t[3][c[4] as usize]
                    ^ t[2][c[5] as usize]
                    ^ t[1][c[6] as usize]
                    ^ t[0][c[7] as usize];
            }
        } else {
            for c in chunks {
                let a = reg ^ u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
                reg = t[7][(a >> 24) as usize]
                    ^ t[6][((a >> 16) & 0xFF) as usize]
                    ^ t[5][((a >> 8) & 0xFF) as usize]
                    ^ t[4][(a & 0xFF) as usize]
                    ^ t[3][c[4] as usize]
                    ^ t[2][c[5] as usize]
                    ^ t[1][c[6] as usize]
                    ^ t[0][c[7] as usize];
            }
        }
        reg = self.crc.step_bytes(reg, rest);
        self.crc.finish(reg)
    }
}

#[cfg(test)]
mod tests {
    use super::catalog::{self, specs};
    use super::*;
    use crate::pack::unpack;

    #[test]
    fn engine_matches_the_slow_definition_on_random_data() {
        let mut data = Vec::new();
        let mut s = 1u32;
        for _ in 0..257 {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
            data.push((s >> 16) as u8);
        }
        for spec in catalog::ALL {
            for len in [0usize, 1, 7, 8, 9, 100, 257] {
                let want = spec.compute(&data[..len]);
                assert_eq!(
                    catalog::checksum_any(spec, &data[..len]),
                    Some(want),
                    "{} len {len}",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn bit_input_equals_byte_input_and_handles_tails() {
        let data = b"The quick brown fox";
        for spec in catalog::ALL {
            let order = if spec.refin {
                BitOrder::Lsb
            } else {
                BitOrder::Msb
            };
            let mut bits = Vec::new();
            unpack(data, order, &mut bits);
            assert_eq!(
                spec.compute_bits(&bits),
                spec.compute(data),
                "{}",
                spec.name
            );
            // A tail of 3 bits goes through the bit path.
            bits.truncate(bits.len() - 3);
            let c = Crc::<u64>::try_new(*spec).unwrap();
            assert_eq!(c.bits(&bits), spec.compute_bits(&bits), "{}", spec.name);
        }
    }

    #[test]
    fn digest_in_pieces_equals_one_call() {
        let c = &catalog::CRC32_MPEG2;
        let mut d = c.digest();
        d.update(b"1234");
        let mut bits = Vec::new();
        unpack(b"56", BitOrder::Msb, &mut bits);
        d.update_bits(&bits);
        d.update(b"789");
        assert_eq!(d.finalize(), 0x0376_E6E7);
        d.reset();
        d.update(b"123456789");
        assert_eq!(d.finalize(), 0x0376_E6E7);
    }

    #[test]
    fn residues_after_a_message_and_its_crc() {
        for spec in catalog::ALL {
            if spec.refin != spec.refout {
                continue; // no natural order for the CRC on the link
            }
            let c = Crc::<u64>::try_new(*spec).unwrap();
            let mut bits = Vec::new();
            crate::pack::push_u64(&mut bits, 0x1234_5678_9ABC, 45);
            c.append_bits(&mut bits);
            assert!(c.verify_bits(&bits), "{}", spec.name);
            let mut d = c.digest();
            d.update_bits(&bits);
            assert_eq!(d.register(), spec.residue, "{}", spec.name);
            bits[3] ^= 1;
            assert!(!c.verify_bits(&bits), "{}", spec.name);
        }
    }

    #[test]
    fn published_residues() {
        // Residues as the CRC RevEng catalogue provides them.
        assert_eq!(specs::CRC16_IBM_SDLC.residue, 0xF0B8);
        assert_eq!(specs::CRC32_ISO_HDLC.residue, 0xDEBB_20E3);
        assert_eq!(specs::CRC32_BZIP2.residue, 0xC704_DD7B);
        assert_eq!(specs::CRC32_CKSUM.residue, 0xC704_DD7B);
        assert_eq!(specs::CRC16_GENIBUS.residue, 0x1D0F);
        assert_eq!(specs::CRC16_GSM.residue, 0x1D0F);
        assert_eq!(specs::CRC16_DNP.residue, 0x66C5);
        assert_eq!(specs::CRC16_EN_13757.residue, 0xA366);
        assert_eq!(specs::CRC16_DECT_R.residue, 0x0589);
        assert_eq!(specs::CRC10_GSM.residue, 0x0C6);
        assert_eq!(specs::CRC40_GSM.residue, 0xC4_FF80_71FF);
        assert_eq!(specs::CRC4_INTERLAKEN.residue, 0x2);
        assert_eq!(specs::CRC32_ISCSI.residue, 0xB798_B438);
        assert_eq!(specs::CRC32_MPEG2.residue, 0);
    }

    #[test]
    fn trailers_and_masks() {
        let c = &catalog::CRC16_GSM;
        let mut f = b"DMR!".to_vec();
        c.append(&mut f, Trailer::Big).unwrap();
        assert!(c.verify(&f, Trailer::Big));
        assert!(!c.verify(&f, Trailer::Little));
        let n = f.len();
        f[n - 2] ^= 0x69;
        f[n - 1] ^= 0x69;
        assert!(c.verify_masked(&f, Trailer::Big, 0x6969));
        assert!(!c.verify(&[0x12], Trailer::Big));
        let mut f = vec![1u8, 2];
        assert!(catalog::CRC12_DECT.append(&mut f, Trailer::Big).is_err());
        assert_eq!(f, [1, 2]);
        assert!(!catalog::CRC12_DECT.verify(&[1, 2, 3], Trailer::Big));
    }

    #[test]
    fn bad_specs_are_refused() {
        let wide = CrcSpec::new("x", 17, 0x1_0001, 0, false, false, 0, 0);
        assert!(Crc::<u16>::try_new(wide).is_err());
        assert!(Crc::<u32>::try_new(wide).is_ok());
        let bad = CrcSpec::new("x", 8, 0x107, 0, false, false, 0, 0);
        assert!(Crc::<u8>::try_new(bad).is_err());
        assert_eq!(bad.compute(b"1"), 0);
        let zero = CrcSpec::new("x", 0, 0, 0, false, false, 0, 0);
        assert!(Crc::<u8>::try_new(zero).is_err());
    }

    #[test]
    fn sliced_equals_bytewise() {
        static A: Sliced32 = Sliced32::new(specs::CRC32_ISO_HDLC);
        static B: Sliced32 = Sliced32::new(specs::CRC32_MPEG2);
        let data: Vec<u8> = (0..1000u32).map(|i| (i * 7 + i / 3) as u8).collect();
        for len in [0, 1, 7, 8, 9, 15, 16, 17, 999, 1000] {
            assert_eq!(
                A.checksum(&data[..len]),
                catalog::CRC32_ISO_HDLC.checksum(&data[..len])
            );
            assert_eq!(
                B.checksum(&data[..len]),
                catalog::CRC32_MPEG2.checksum(&data[..len])
            );
        }
        assert_eq!(B.engine().spec().name, "CRC-32/MPEG-2");
    }
}
