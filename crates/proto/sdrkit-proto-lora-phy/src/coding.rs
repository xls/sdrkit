//! The LoRa coding chain in both directions: the header and its checksum, whitening, the payload CRC, the Hamming codes, the
//! diagonal interleaver and the Gray mapping.
//!
//! Standard: J. Tapparel et al., "An Open-Source LoRa Physical Layer Prototype on GNU Radio", IEEE SPAWC 2020

use crate::error::{LoraError, Param, Result};
use crate::params::{data_symbols, Layout, LoraParams};
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_util_bits::crc::catalog::CRC16_XMODEM;
use sdrkit_util_bits::gray::{gray, ungray};
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::seq::LORA_WHITENING;

/// XORs `bytes` with the LoRa whitening sequence from its first byte (`sdrkit_util_bits::seq::LORA_WHITENING`). The operation is its
/// own inverse.
pub fn whiten(bytes: &mut [u8]) {
    LORA_WHITENING.apply(bytes, 0);
}

/// The 5-bit header checksum of the header nibbles `n0`, `n1` and `n2`.
pub fn header_checksum(n0: u8, n1: u8, n2: u8) -> u8 {
    let b = |n: u8, k: u8| (n >> k) & 1;
    let c4 = b(n0, 3) ^ b(n0, 2) ^ b(n0, 1) ^ b(n0, 0);
    let c3 = b(n0, 3) ^ b(n1, 3) ^ b(n1, 2) ^ b(n1, 1) ^ b(n2, 0);
    let c2 = b(n0, 2) ^ b(n1, 3) ^ b(n1, 0) ^ b(n2, 3) ^ b(n2, 1);
    let c1 = b(n0, 1) ^ b(n1, 2) ^ b(n1, 0) ^ b(n2, 2) ^ b(n2, 1) ^ b(n2, 0);
    let c0 = b(n0, 0) ^ b(n1, 1) ^ b(n2, 3) ^ b(n2, 2) ^ b(n2, 1) ^ b(n2, 0);
    (c4 << 4) | (c3 << 3) | (c2 << 2) | (c1 << 1) | c0
}

/// The LoRa payload CRC: CRC-16/XMODEM over all bytes but the last two, XORed with the last byte (low 8 bits) and the byte
/// before it (high 8 bits). For a payload of `"123456789"` and two zero bytes, the result is the XMODEM check value 0x31C3.
pub fn payload_crc(payload: &[u8]) -> u16 {
    let n = payload.len();
    let mut c = CRC16_XMODEM.checksum(&payload[..n.saturating_sub(2)]);
    if n >= 1 {
        c ^= payload[n - 1] as u16;
    }
    if n >= 2 {
        c ^= (payload[n - 2] as u16) << 8;
    }
    c
}

/// The Hamming codeword of `nibble` for a codeword length `c` (5 to 8). The data bits come first, nibble bit 0 most
/// significant. The parity bits follow: `p0 = d0+d1+d2`, `p1 = d1+d2+d3`, `p2 = d0+d1+d3`, `p3 = d0+d2+d3`. At `c = 5` the one
/// check bit is the parity of the four data bits.
pub const fn hamming_encode(nibble: u8, c: u8) -> u8 {
    let d0 = nibble & 1;
    let d1 = (nibble >> 1) & 1;
    let d2 = (nibble >> 2) & 1;
    let d3 = (nibble >> 3) & 1;
    let p0 = d0 ^ d1 ^ d2;
    let p1 = d1 ^ d2 ^ d3;
    let p2 = d0 ^ d1 ^ d3;
    let p3 = d0 ^ d2 ^ d3;
    let data = (d0 << 3) | (d1 << 2) | (d2 << 1) | d3;
    match c {
        5 => (data << 1) | (d0 ^ d1 ^ d2 ^ d3),
        6 => (data << 2) | (p0 << 1) | p1,
        7 => (data << 3) | (p0 << 2) | (p1 << 1) | p2,
        _ => (data << 4) | (p0 << 3) | (p1 << 2) | (p2 << 1) | p3,
    }
}

/// The nibble that the codeword `cw` of length `c` carries (its data bits).
pub const fn codeword_nibble(cw: u8, c: u8) -> u8 {
    reverse4(cw >> (c - 4))
}

const fn reverse4(x: u8) -> u8 {
    ((x >> 3) & 1) | (((x >> 2) & 1) << 1) | (((x >> 1) & 1) << 2) | ((x & 1) << 3)
}

const fn codebook(c: u8) -> [u8; 16] {
    let mut t = [0u8; 16];
    let mut n = 0;
    while n < 16 {
        t[n] = hamming_encode(n as u8, c);
        n += 1;
    }
    t
}

/// The 16 codewords of each length 5 to 8 (index `c - 5`, then the nibble).
pub const CODEBOOKS: [[u8; 16]; 4] = [codebook(5), codebook(6), codebook(7), codebook(8)];

/// The parity rows of the LoRa Hamming code of length `c`, in the layout of `sdrkit_algo_fec::small::SmallCode::from_parity_rows`
/// (one row per data bit, the first sent first).
pub const fn hamming_rows(c: u8) -> [u32; 4] {
    match c {
        5 => [1, 1, 1, 1],
        6 => [0b10, 0b11, 0b11, 0b01],
        7 => [0b101, 0b111, 0b110, 0b011],
        _ => [0b1011, 0b1110, 0b1101, 0b0111],
    }
}

/// The layout of one interleaver block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    /// The codeword length. This is also the number of symbols in the block.
    pub c: u8,
    /// The rows: the number of codewords, and the bits per symbol.
    pub rows: u8,
    /// Reduced rate: a symbol carries `SF - 2` bits, in steps of 4 bins.
    pub reduced: bool,
}

/// The first interleaver block of a frame at `sf`.
pub fn first_block(sf: u8) -> Block {
    if sf < 7 {
        Block {
            c: 8,
            rows: sf,
            reduced: false,
        }
    } else {
        Block {
            c: 8,
            rows: sf - 2,
            reduced: true,
        }
    }
}

/// The later interleaver blocks of a frame at `sf` with coding rate `cr` (5 to 8).
pub fn later_block(sf: u8, cr: u8, ldro: bool) -> Block {
    Block {
        c: cr,
        rows: if ldro { sf - 2 } else { sf },
        reduced: ldro,
    }
}

/// A decoded explicit header.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Header {
    /// The payload length, bytes.
    pub len: u8,
    /// The coding rate, 5 to 8.
    pub cr: u8,
    /// The payload CRC is present.
    pub crc: bool,
}

/// The header from the first five nibbles. If the checksum is wrong, the coding rate is not 5 to 8, or the length is 0, this
/// returns `None`.
pub fn parse_header(nib: &[u8]) -> Option<Header> {
    if nib.len() < 5 {
        return None;
    }
    let (n0, n1, n2) = (nib[0], nib[1], nib[2]);
    if nib[3] > 1 {
        return None;
    }
    let c = (nib[3] << 4) | nib[4];
    if header_checksum(n0, n1, n2) != c {
        return None;
    }
    let cr = (n2 >> 1) + 4;
    let len = (n0 << 4) | n1;
    if !(5..=8).contains(&cr) || len == 0 {
        return None;
    }
    Some(Header {
        len,
        cr,
        crc: n2 & 1 == 1,
    })
}

/// The soft output of the demodulator for one data symbol: its strongest bins (the strongest first) with their log-likelihoods, and
/// the log-likelihood of every other bin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SymbolSoft {
    /// The strongest bins. The first one is the hard decision.
    pub bins: [u16; 4],
    /// The log-likelihoods of `bins`.
    pub ll: [f32; 4],
    /// How many of `bins` are valid, 1 to 4.
    pub count: u8,
    /// The log-likelihood of a bin that is not in the list.
    pub floor: f32,
}

impl SymbolSoft {
    /// A hard symbol: one bin with no soft values.
    pub fn hard(bin: u16) -> Self {
        SymbolSoft {
            bins: [bin, 0, 0, 0],
            ll: [1.0, 0.0, 0.0, 0.0],
            count: 1,
            floor: 0.0,
        }
    }

    /// The hard decision.
    pub fn bin(&self) -> u16 {
        self.bins[0]
    }

    /// The certainty: the log-likelihood gap between the two strongest bins. A hard symbol has an infinite gap.
    pub fn gap(&self) -> f32 {
        if self.count >= 2 {
            self.ll[0] - self.ll[1]
        } else {
            f32::INFINITY
        }
    }

    /// The same symbol with every bin moved down by `shift`, modulo `n`.
    pub fn shifted(&self, shift: i32, n: u32) -> Self {
        let mut s = *self;
        for b in s.bins.iter_mut().take(s.count as usize) {
            *b = ((*b as i64 - shift as i64).rem_euclid(n as i64)) as u16;
        }
        s
    }

    /// The same symbol with the runner-up bin as the decision. The two bins change places but their log-likelihoods do not, so
    /// the runner-up gets the strongest log-likelihood. Soft decoding then sees a different symbol. (Swapping the
    /// log-likelihoods as well would leave the soft metrics unchanged.)
    pub fn runner_up(&self) -> Self {
        let mut s = *self;
        if s.count >= 2 {
            s.bins.swap(0, 1);
        }
        s
    }
}

/// The interleaved word that a bin carries.
fn demap(bin: u16, sf: u8, reduced: bool) -> u32 {
    let n = 1u32 << sf;
    let x = (bin as u32 + n - 1) % n;
    if reduced {
        gray(((x + 2) / 4) % (n >> 2))
    } else {
        gray(x)
    }
}

/// The result of decoding one interleaver block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockOut {
    /// The nibbles of its rows.
    pub nibbles: [u8; 12],
    /// The number of rows decoded.
    pub rows: u8,
    /// The codewords whose decision differs from the hard bits.
    pub corrected: u32,
    /// The codewords whose hard bits are not a codeword and that the decoder did not correct.
    pub uncorrectable: u32,
    /// The hard bits of every codeword are a valid codeword.
    pub clean: bool,
}

/// A decoded payload. Use one value again and again: the decoder reuses its vectors, so it allocates nothing once they have
/// grown. `clone_from` reuses them too.
#[derive(Debug, Default, PartialEq)]
pub struct Payload {
    /// The payload bytes, de-whitened.
    pub bytes: Vec<u8>,
    /// The coding rate used.
    pub cr: u8,
    /// The CRC is present.
    pub crc_present: bool,
    /// The CRC is right. This is false when there is no CRC.
    pub crc_ok: bool,
    /// The codewords corrected.
    pub corrected: u32,
    /// The codewords that the decoder did not correct.
    pub uncorrectable: u32,
    /// The hard bits of every interleaver block are valid codewords.
    pub clean: bool,
    /// For each later block: its hard bits are valid codewords.
    pub block_clean: Vec<bool>,
}

impl Clone for Payload {
    fn clone(&self) -> Self {
        let mut p = Payload::default();
        p.clone_from(self);
        p
    }

    fn clone_from(&mut self, src: &Self) {
        self.bytes.clone_from(&src.bytes);
        self.block_clean.clone_from(&src.block_clean);
        self.cr = src.cr;
        self.crc_present = src.crc_present;
        self.crc_ok = src.crc_ok;
        self.corrected = src.corrected;
        self.uncorrectable = src.uncorrectable;
        self.clean = src.clean;
    }
}

/// The size of the largest interleaver block, in bits (12 rows of 8 bits).
const MAX_BLOCK_BITS: usize = 96;

/// The encoder and decoder of one spreading factor. It holds the interleavers and the syndrome tables, built once in `new`.
///
/// Each nibble becomes a Hamming codeword of 5 to 8 bits. A diagonal interleaver puts the codewords of a block into `c`
/// symbols, and the inverse Gray code produces the symbol values. The first block uses `c = 8` and the reduced rate. With soft
/// values, each codeword is decoded by maximum correlation over its 16 words. Without them, through the syndrome tables.
#[derive(Clone, Debug)]
pub struct Codec {
    sf: u8,
    /// Interleavers by rows (`SF`, then `SF - 2`) and codeword length (`c - 5`).
    perms: [[Permutation; 4]; 2],
    /// The syndrome decoders of the codes of length 5 to 8.
    codes: [SmallCode; 4],
}

impl Codec {
    /// The codec of spreading factor `sf`. If `sf` is not 5 to 12, this returns an error.
    pub fn new(sf: u8) -> Result<Self> {
        if !(5..=12).contains(&sf) {
            return Err(LoraError::BadParameter(Param::SpreadingFactor));
        }
        let perm =
            |rows: u8, c: u8| Permutation::lora(rows as usize, c as usize).map_err(LoraError::Bits);
        let code = |c: u8| {
            SmallCode::from_parity_rows(c as usize, 4, &hamming_rows(c)).map_err(LoraError::Fec)
        };
        Ok(Codec {
            sf,
            perms: [
                [perm(sf, 5)?, perm(sf, 6)?, perm(sf, 7)?, perm(sf, 8)?],
                [
                    perm(sf - 2, 5)?,
                    perm(sf - 2, 6)?,
                    perm(sf - 2, 7)?,
                    perm(sf - 2, 8)?,
                ],
            ],
            codes: [code(5)?, code(6)?, code(7)?, code(8)?],
        })
    }

    /// The spreading factor.
    pub fn sf(&self) -> u8 {
        self.sf
    }

    fn perm(&self, blk: Block) -> &Permutation {
        let r = usize::from(blk.rows != self.sf);
        &self.perms[r][(blk.c - 5) as usize]
    }

    fn check(&self, layout: &Layout, cr: u8) -> Result<()> {
        if layout.sf != self.sf {
            return Err(LoraError::BadParameter(Param::LayoutSpreadingFactor));
        }
        if !(5..=8).contains(&cr) {
            return Err(LoraError::BadParameter(Param::CodingRate));
        }
        Ok(())
    }

    /// Encodes `payload` into data symbol values with coding rate `cr` (5 to 8) and the CRC flag `crc`, and appends them to
    /// `out`. Returns the number of symbols. If the payload is longer than 255 bytes, or a parameter is out of range, this
    /// returns an error.
    pub fn encode(
        &self,
        payload: &[u8],
        layout: &Layout,
        cr: u8,
        crc: bool,
        out: &mut Vec<u16>,
    ) -> Result<usize> {
        self.check(layout, cr)?;
        if payload.len() > 255 {
            return Err(LoraError::PayloadTooLong { len: payload.len() });
        }
        let sf = self.sf;
        let len = payload.len();
        let hdr = {
            let l = len as u8;
            let n2 = ((cr - 4) << 1) | crc as u8;
            let c = header_checksum(l >> 4, l & 0xF, n2);
            [l >> 4, l & 0xF, n2, c >> 4, c & 0xF]
        };
        let hlen = if layout.implicit { 0 } else { 5 };
        let crc_val = if crc { payload_crc(payload) } else { 0 };
        let white = LORA_WHITENING.bytes();
        let nibble = |k: usize| -> u8 {
            if k < hlen {
                return hdr[k];
            }
            let k = k - hlen;
            if k < 2 * len {
                let b = payload[k / 2] ^ white[(k / 2) % 255];
                return if k.is_multiple_of(2) { b & 0xF } else { b >> 4 };
            }
            let k = k - 2 * len;
            if crc && k < 4 {
                return ((crc_val >> (4 * k)) & 0xF) as u8;
            }
            0
        };
        let total = data_symbols(sf, cr, len, crc, layout.implicit, layout.ldro);
        let n = 1u32 << sf;
        let start = out.len();
        let mut pos = 0usize;
        let mut first = true;
        let mut cw_bits = [0u8; MAX_BLOCK_BITS];
        let mut sym_bits = [0u8; MAX_BLOCK_BITS];
        while out.len() - start < total {
            let blk = if first {
                first_block(sf)
            } else {
                later_block(sf, cr, layout.ldro)
            };
            first = false;
            let rows = blk.rows as usize;
            let c = blk.c as usize;
            for r in 0..rows {
                let cw = hamming_encode(nibble(pos + r), blk.c);
                for i in 0..c {
                    cw_bits[r * c + i] = (cw >> (c - 1 - i)) & 1;
                }
            }
            pos += rows;
            let m = rows * c;
            self.perm(blk)
                .apply(&cw_bits[..m], &mut sym_bits[..m])
                .map_err(LoraError::Bits)?;
            for i in 0..c {
                let mut v = 0u32;
                for j in 0..rows {
                    v = (v << 1) | sym_bits[i * rows + j] as u32;
                }
                let g = ungray(v);
                let s = if blk.reduced { 4 * g + 1 } else { g + 1 } % n;
                out.push(s as u16);
            }
        }
        Ok(total)
    }

    /// Decodes one interleaver block from its `blk.c` symbols. A missing symbol counts as a hard 1. With `soft`, the codewords
    /// are decoded by maximum correlation with the soft values. Without it, the syndrome tables decode the hard bits: one error
    /// is corrected at 4/7 and 4/8, and errors are only detected at 4/5 and 4/6.
    pub fn decode_block(&self, syms: &[SymbolSoft], blk: Block, soft: bool) -> BlockOut {
        let rows = blk.rows as usize;
        let c = blk.c as usize;
        let m = rows * c;
        // Symbol bit metrics, symbol i bit j (j = 0 the most significant) at i * rows + j. Positive means 0.
        let mut sym_llr = [0f32; MAX_BLOCK_BITS];
        let mut sym_hard = [0u8; MAX_BLOCK_BITS];
        for i in 0..c {
            let s = syms.get(i).copied().unwrap_or(SymbolSoft::hard(1));
            let hard = demap(s.bin(), self.sf, blk.reduced);
            let mut vs = [0u32; 4];
            let count = (s.count as usize).clamp(1, 4);
            for (k, v) in vs.iter_mut().enumerate().take(count) {
                *v = demap(s.bins[k], self.sf, blk.reduced);
            }
            for j in 0..rows {
                let bit = rows - 1 - j;
                sym_hard[i * rows + j] = ((hard >> bit) & 1) as u8;
                let l = if soft {
                    // Max-log over the listed bins. A bit value no listed bin carries takes the floor.
                    let (mut b1, mut b0) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
                    for (v, &ll) in vs.iter().zip(&s.ll).take(count) {
                        if (v >> bit) & 1 == 1 {
                            b1 = b1.max(ll);
                        } else {
                            b0 = b0.max(ll);
                        }
                    }
                    let b1 = if b1.is_finite() { b1 } else { s.floor };
                    let b0 = if b0.is_finite() { b0 } else { s.floor };
                    let d = b0 - b1;
                    if d.is_finite() {
                        d.clamp(-10_000.0, 10_000.0)
                    } else {
                        0.0
                    }
                } else if (hard >> bit) & 1 == 1 {
                    -1.0
                } else {
                    1.0
                };
                sym_llr[i * rows + j] = l;
            }
        }
        // Codeword r bit i (i = 0 the most significant) at r * c + i.
        let mut cw_llr = [0f32; MAX_BLOCK_BITS];
        let mut cw_hard = [0u8; MAX_BLOCK_BITS];
        let perm = self.perm(blk);
        if perm.invert(&sym_llr[..m], &mut cw_llr[..m]).is_err()
            || perm.invert(&sym_hard[..m], &mut cw_hard[..m]).is_err()
        {
            return BlockOut::default();
        }
        let book = &CODEBOOKS[c - 5];
        let code = &self.codes[c - 5];
        let mut out = BlockOut {
            rows: blk.rows,
            clean: true,
            ..Default::default()
        };
        for r in 0..rows {
            let mut word = 0u8;
            for i in 0..c {
                word = (word << 1) | cw_hard[r * c + i];
            }
            let valid = code.syndrome(word as u128) == 0;
            out.clean &= valid;
            let nibble = if soft {
                let mut best = 0usize;
                let mut best_score = f32::NEG_INFINITY;
                for (k, &cw) in book.iter().enumerate() {
                    let mut score = 0f32;
                    for i in 0..c {
                        let l = cw_llr[r * c + i];
                        score += if (cw >> (c - 1 - i)) & 1 == 0 { l } else { -l };
                    }
                    if score > best_score {
                        best_score = score;
                        best = k;
                    }
                }
                if book[best] != word {
                    out.corrected += 1;
                }
                best as u8
            } else {
                match code.decode(word as u128) {
                    Some((cw, fixed)) => {
                        if fixed > 0 {
                            out.corrected += 1;
                        }
                        codeword_nibble(cw as u8, blk.c)
                    }
                    None => {
                        out.uncorrectable += 1;
                        codeword_nibble(word, blk.c)
                    }
                }
            };
            out.nibbles[r] = nibble;
        }
        out
    }

    /// Decodes the header from the first eight data symbols. If there are fewer symbols, or the header is not valid, this
    /// returns `None`.
    pub fn decode_header(&self, syms: &[SymbolSoft], soft: bool) -> Option<Header> {
        if syms.len() < 8 {
            return None;
        }
        let b = self.decode_block(&syms[..8], first_block(self.sf), soft);
        parse_header(&b.nibbles[..b.rows as usize])
    }

    /// Decodes the data symbols of a whole frame into `out`, with the length, the coding rate and the CRC flag of `h` (from the
    /// header, or agreed in advance for an implicit header). `out` is overwritten. Its vectors are reused.
    pub fn decode_payload(
        &self,
        syms: &[SymbolSoft],
        ldro: bool,
        implicit: bool,
        h: Header,
        soft: bool,
        out: &mut Payload,
    ) {
        out.bytes.clear();
        out.block_clean.clear();
        out.cr = h.cr;
        out.crc_present = h.crc;
        out.crc_ok = false;
        out.corrected = 0;
        out.uncorrectable = 0;
        out.clean = true;
        let len = h.len as usize;
        out.bytes.resize(len, 0);
        let skip = if implicit { 0 } else { 5 };
        let cr = h.cr.clamp(5, 8);
        let mut rx_crc = 0u16;
        let mut k = 0usize;
        let mut at = 0usize;
        let mut first = true;
        while at < syms.len() {
            let blk = if first {
                first_block(self.sf)
            } else {
                later_block(self.sf, cr, ldro)
            };
            let end = (at + blk.c as usize).min(syms.len());
            let b = self.decode_block(&syms[at..end], blk, soft);
            for &v in &b.nibbles[..b.rows as usize] {
                if k >= skip {
                    let q = k - skip;
                    if q < 2 * len {
                        out.bytes[q / 2] |= if q.is_multiple_of(2) { v } else { v << 4 };
                    } else if q - 2 * len < 4 {
                        rx_crc |= (v as u16) << (4 * (q - 2 * len));
                    }
                }
                k += 1;
            }
            out.corrected += b.corrected;
            out.uncorrectable += b.uncorrectable;
            out.clean &= b.clean;
            if !first {
                out.block_clean.push(b.clean);
            }
            first = false;
            at = end;
        }
        whiten(&mut out.bytes);
        if h.crc {
            out.crc_ok = rx_crc == payload_crc(&out.bytes);
        }
    }

    /// True if the hard bits of the later interleaver block that starts at symbol `at` are all valid codewords. The search for
    /// lost samples uses this.
    pub fn block_clean(&self, syms: &[SymbolSoft], at: usize, cr: u8, ldro: bool) -> bool {
        let blk = later_block(self.sf, cr.clamp(5, 8), ldro);
        let end = (at + blk.c as usize).min(syms.len());
        if at >= end {
            return true;
        }
        self.decode_block(&syms[at..end], blk, false).clean
    }
}

/// Encodes `payload` with the setting `p` into a new vector of data symbol values. If the setting is not valid, or the payload
/// is longer than 255 bytes, this returns an error.
pub fn encode(payload: &[u8], p: &LoraParams) -> Result<Vec<u16>> {
    p.check()?;
    let codec = Codec::new(p.sf)?;
    let mut out = Vec::new();
    codec.encode(payload, &p.layout(), p.cr, p.crc, &mut out)?;
    Ok(out)
}
