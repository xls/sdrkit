//! Short linear block codes (`n` up to 128, at most 20 check bits) decoded by one syndrome-table lookup, with a soft Chase
//! search; the least-reliable-bits search for codes checked by a CRC; product codes (DMR BPTC); and first-order Reed-Muller
//! (biorthogonal) codes decoded with the fast Hadamard transform.
//!
//! # Word layout
//!
//! A word is a `u128` holding `n` bits, **the first bit sent in the most significant place** (bit `n − 1`), as in
//! `sdrkit_util_bits::crc::cyclic::CyclicCode`. Codes are systematic: `k` information bits, then `r = n − k` check bits.
//!
//! # Decoding
//!
//! The table maps every syndrome to its lowest-weight error pattern, filled weight by weight up to the code's unique-decoding
//! radius (the largest weight whose patterns all have distinct syndromes). A syndrome outside the table is a detected
//! uncorrectable word. Golay (23,12) fills the whole table (a perfect code); Golay (24,12) corrects 3 and detects 4.
//!
//! ```
//! use sdrkit_algo_fec::small::SmallCode;
//! let g = SmallCode::golay_24_12();
//! let cw = g.encode(0xABC);
//! assert_eq!(g.decode(cw ^ 0x80_0101), Some((cw, 3)));
//! assert_eq!(g.info(cw), 0xABC);
//! ```

use crate::error::{Decoded, FecError, Param, Result};

/// A syndrome with no entry (no position byte is ever 0xFD: positions are below 128, unused bytes are 0xFF).
const EMPTY: u32 = 0xFDFD_FDFD;

/// A short systematic linear code with its syndrome table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmallCode {
    n: u8,
    k: u8,
    /// Check bits of each information unit vector (information bit 0 is the first sent); bit `r − 1` is the first check bit.
    rows: Vec<u32>,
    /// XORed onto every code word (MPT 1327 inverts one check bit).
    offset: u128,
    radius: u8,
    /// Per syndrome: up to 4 bit indices (0 = first sent) in bytes, unused bytes 0xFF; `EMPTY` when not decodable.
    table: Vec<u32>,
}

impl SmallCode {
    /// The code with `n` bits, `k` information bits and check rows `rows` (one per information bit, `n − k` bits each, the
    /// first check bit most significant), as printed in a systematic generator `G = [I | P]`. The table is filled to the unique
    /// decoding radius (at most 4). An error when `n > 128`, `n − k > 20` or `rows.len() != k`.
    pub fn from_parity_rows(n: usize, k: usize, rows: &[u32]) -> Result<Self> {
        let mut c = Self::raw(n, k, rows)?;
        c.fill_by_weight();
        Ok(c)
    }

    fn raw(n: usize, k: usize, rows: &[u32]) -> Result<Self> {
        Param::N.check(n, 1, 128)?;
        Param::K.check(k, 1, n as i64 - 1)?;
        if n - k > 20 {
            return Param::K.range(n as i64 - 20, n as i64 - 1, k);
        }
        if rows.len() != k {
            return Param::Rows.range(k as i64, k as i64, rows.len());
        }
        let r = n - k;
        if rows.iter().any(|&v| v >> r != 0) {
            return Param::Rows.conflict(Param::N);
        }
        Ok(SmallCode {
            n: n as u8,
            k: k as u8,
            rows: rows.to_vec(),
            offset: 0,
            radius: 0,
            table: Vec::new(),
        })
    }

    /// The systematic cyclic code (or shortened cyclic code) of length `n` with generator `g` (degree `n − k`, bit `i` the
    /// coefficient of `x^i`).
    pub fn cyclic(n: usize, g: u64) -> Result<Self> {
        let r = 63 - g.leading_zeros() as usize;
        if g == 0 {
            return Param::Generator.degenerate();
        }
        Param::N.check(n, 1, 128)?;
        Param::Generator.check(r, 1, (n as i64 - 1).min(20))?;
        let k = n - r;
        // Information bit j is the coefficient of x^(n-1-j); its check bits are x^(n-1-j) mod g.
        let rows: Vec<u32> = (0..k).map(|j| xpow_mod(n - 1 - j, g, r)).collect();
        Self::from_parity_rows(n, k, &rows)
    }

    /// The code extended by an even overall parity bit sent last (`n + 1`, same `k`).
    pub fn extended(&self) -> Result<Self> {
        let rows: Vec<u32> = self
            .rows
            .iter()
            .map(|&v| (v << 1) | ((1 + v.count_ones()) & 1))
            .collect();
        let mut c = Self::raw(self.n as usize + 1, self.k as usize, &rows)?;
        c.offset = self.offset << 1 | (self.offset.count_ones() & 1) as u128;
        c.fill_by_weight();
        Ok(c)
    }

    /// The code shortened by `s` leading information bits (`n − s`, `k − s`).
    pub fn shortened(&self, s: usize) -> Result<Self> {
        if s >= self.k as usize {
            return Param::Shortening.range(0, self.k as i64 - 1, s);
        }
        let mut c = Self::raw(self.n as usize - s, self.k as usize - s, &self.rows[s..])?;
        c.offset = self.offset & low(c.n as u32);
        c.fill_by_weight();
        Ok(c)
    }

    /// The same code with `offset` XORed onto every code word (a fixed inversion of some bits, as MPT 1327 inverts its last
    /// check bit, or an RDS offset word on the check bits).
    pub fn with_offset(mut self, offset: u128) -> Self {
        self.offset = offset & low(self.n as u32);
        self
    }

    /// Refills the table with every burst (a run whose first and last bits are wrong) of length up to `max_len`, for burst
    /// trapping (RDS corrects bursts of 5 or less: IEC 62106 / NRSC-4-A clause 2.3). Bursts are stored as their first and last
    /// position plus the pattern; the radius becomes 0 for weight purposes.
    pub fn with_burst_table(mut self, max_len: usize) -> Result<Self> {
        if max_len == 0 || max_len > 8 {
            return Param::BurstLen.range(1, 8, max_len);
        }
        let r = self.r();
        self.table = vec![EMPTY; 1 << r];
        self.table[0] = pack(&[]);
        self.radius = 0;
        // A burst entry is (start, pattern) with the pattern in the low byte; marked by 0xFE in the top byte.
        for len in 1..=max_len {
            for start in 0..=(self.n as usize - len) {
                let inner = if len > 2 { 1u32 << (len - 2) } else { 1 };
                for mid in 0..inner {
                    let pat: u32 = if len == 1 {
                        1
                    } else {
                        (1 << (len - 1)) | (mid << 1) | 1
                    };
                    let mut s = 0u32;
                    for b in 0..len {
                        if pat >> (len - 1 - b) & 1 != 0 {
                            s ^= self.col(start + b);
                        }
                    }
                    if self.table[s as usize] == EMPTY {
                        self.table[s as usize] = 0xFE00_0000 | ((start as u32) << 8) | pat;
                    }
                }
            }
        }
        Ok(self)
    }

    /// Bits per word.
    pub fn n(&self) -> usize {
        self.n as usize
    }

    /// Information bits.
    pub fn k(&self) -> usize {
        self.k as usize
    }

    /// Check bits, `n − k`.
    pub fn r(&self) -> usize {
        (self.n - self.k) as usize
    }

    /// Errors always corrected (the table's radius).
    pub fn radius(&self) -> usize {
        self.radius as usize
    }

    /// The check rows (as given to [`SmallCode::from_parity_rows`]).
    pub fn rows(&self) -> &[u32] {
        &self.rows
    }

    /// The syndrome of an error in bit `i` (0 = first sent).
    fn col(&self, i: usize) -> u32 {
        let k = self.k as usize;
        if i < k {
            self.rows[i]
        } else {
            1 << (self.n as usize - 1 - i)
        }
    }

    fn check_of(&self, info: u128) -> u32 {
        let k = self.k as usize;
        let mut c = 0u32;
        for j in 0..k {
            if info >> (k - 1 - j) & 1 != 0 {
                c ^= self.rows[j];
            }
        }
        c
    }

    /// The code word of `info` (its low `k` bits, the first sent most significant).
    pub fn encode(&self, info: u128) -> u128 {
        let info = info & low(self.k as u32);
        ((info << self.r()) | self.check_of(info) as u128) ^ self.offset
    }

    /// The information bits of a word.
    pub fn info(&self, word: u128) -> u128 {
        ((word ^ self.offset) >> self.r()) & low(self.k as u32)
    }

    /// The syndrome of a word (0 for a code word).
    pub fn syndrome(&self, word: u128) -> u32 {
        let w = (word ^ self.offset) & low(self.n as u32);
        self.check_of(w >> self.r()) ^ (w as u32 & low(self.r() as u32) as u32)
    }

    fn fill_by_weight(&mut self) {
        let r = self.r();
        let n = self.n as usize;
        self.table = vec![EMPTY; 1 << r];
        self.table[0] = pack(&[]);
        let mut filled = 1usize;
        self.radius = 0;
        let mut idx = [0usize; 4];
        for w in 1..=4usize {
            if filled == self.table.len() || w > n {
                break;
            }
            // Positions of this weight's entries, to undo on a collision.
            let mut collided = false;
            let mut added: Vec<u32> = Vec::new();
            for (i, v) in idx.iter_mut().enumerate().take(w) {
                *v = i;
            }
            loop {
                let mut s = 0u32;
                for &p in &idx[..w] {
                    s ^= self.col(p);
                }
                if self.table[s as usize] == EMPTY {
                    let pos: Vec<u8> = idx[..w].iter().map(|&p| p as u8).collect();
                    self.table[s as usize] = pack(&pos);
                    added.push(s);
                } else {
                    collided = true;
                    break;
                }
                // Next combination.
                let mut i = w;
                loop {
                    if i == 0 {
                        break;
                    }
                    i -= 1;
                    if idx[i] < n - w + i {
                        idx[i] += 1;
                        for j in i + 1..w {
                            idx[j] = idx[j - 1] + 1;
                        }
                        break;
                    }
                    if i == 0 {
                        i = usize::MAX;
                        break;
                    }
                }
                if i == usize::MAX {
                    break;
                }
            }
            if collided {
                for s in added {
                    self.table[s as usize] = EMPTY;
                }
                break;
            }
            filled += added.len();
            self.radius = w as u8;
        }
    }

    /// Decodes a word: `Some((code word, bits corrected))`, or `None` when the error is beyond the table (detected).
    pub fn decode(&self, word: u128) -> Option<(u128, u8)> {
        let word = word & low(self.n as u32);
        let s = self.syndrome(word);
        let e = *self.table.get(s as usize)?;
        if e == EMPTY {
            return None;
        }
        let n = self.n as usize;
        if e >> 24 == 0xFE {
            // A burst: start and pattern.
            let start = (e >> 8 & 0xFFFF) as usize;
            let pat = e & 0xFF;
            let len = 32 - pat.leading_zeros() as usize;
            let mut m = 0u128;
            for b in 0..len {
                if pat >> (len - 1 - b) & 1 != 0 {
                    m |= 1u128 << (n - 1 - (start + b));
                }
            }
            return Some((word ^ m, m.count_ones() as u8));
        }
        let mut m = 0u128;
        let mut c = 0u8;
        for b in e.to_le_bytes() {
            if b != 0xFF {
                m |= 1u128 << (n - 1 - b as usize);
                c += 1;
            }
        }
        Some((word ^ m, c))
    }

    /// Soft decoding with a Chase search: tries flipping the `chase.weakest` least reliable bits (up to `chase.max_flips` of
    /// them at a time), decodes each trial with the table, and keeps the code word nearest the received bits in soft distance
    /// (the sum of `rel` over the bits it changes). `rel[i]` is the reliability (|LLR|) of bit `i`, 0 the first sent. Returns
    /// the code word, the number of bits it changes, and its soft distance; `None` when no trial decodes.
    pub fn decode_soft(&self, hard: u128, rel: &[f32], chase: Chase) -> Option<(u128, u8, f32)> {
        let n = self.n as usize;
        if rel.len() < n {
            return None;
        }
        let hard = hard & low(n as u32);
        let mut weak = [(f32::INFINITY, 0u8); MAX_CHASE];
        let w = least_reliable(&rel[..n], chase.weakest as usize, &mut weak);
        let mut best: Option<(u128, u8, f32)> = None;
        for mask in 0u32..(1 << w) {
            if mask.count_ones() > chase.max_flips as u32 {
                continue;
            }
            let mut trial = hard;
            for (b, &(_, p)) in weak[..w].iter().enumerate() {
                if mask >> b & 1 != 0 {
                    trial ^= 1u128 << (n - 1 - p as usize);
                }
            }
            if let Some((cw, _)) = self.decode(trial) {
                let diff = cw ^ hard;
                let mut cost = 0f32;
                let mut d = diff;
                while d != 0 {
                    let bit = 127 - d.leading_zeros() as usize;
                    cost += rel[n - 1 - bit].abs();
                    d &= !(1u128 << bit);
                }
                if best.is_none_or(|(_, _, c)| cost < c) {
                    best = Some((cw, diff.count_ones() as u8, cost));
                }
            }
        }
        best
    }

    /// Decodes bits held one per byte (`n` of them, the first sent first), in place.
    pub fn decode_bits(&self, bits: &mut [u8]) -> Result<Decoded> {
        if bits.len() != self.n as usize {
            return Err(FecError::InputLength {
                expected: self.n as usize,
                got: bits.len(),
            });
        }
        let w = bits.iter().fold(0u128, |a, &b| (a << 1) | (b & 1) as u128);
        let (cw, c) = self.decode(w).ok_or(FecError::Uncorrectable)?;
        let n = bits.len();
        for (i, b) in bits.iter_mut().enumerate() {
            *b = (cw >> (n - 1 - i)) as u8 & 1;
        }
        Ok(Decoded::corrected(c as u32))
    }
}

fn pack(pos: &[u8]) -> u32 {
    let mut b = [0xFFu8; 4];
    b[..pos.len()].copy_from_slice(pos);
    u32::from_le_bytes(b)
}

const fn low(n: u32) -> u128 {
    if n >= 128 {
        u128::MAX
    } else {
        (1u128 << n) - 1
    }
}

/// `x^e mod g` (degree `r`).
fn xpow_mod(e: usize, g: u64, r: usize) -> u32 {
    let mut v = 1u64;
    for _ in 0..e {
        v <<= 1;
        if v >> r & 1 != 0 {
            v ^= g;
        }
    }
    v as u32
}

/// The verdict of the GLONASS string check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GlonassCheck {
    /// No error, or one error in a check bit: the data is as received.
    Good,
    /// One error in a data bit, corrected in place.
    Corrected,
    /// More errors: the string must be erased.
    Erased,
}

/// The GLONASS navigation string code: a Hamming code on string bits 85 to 9 (77 data bits) with check bits β1 to β7 in
/// string bits 1 to 7, and β8 in string bit 8 making the parity of the whole string even. Bit 85 is sent first.
/// Standard: GLONASS ICD Edition 5.1 clause 4.7 (table 4.13 and the check rules a to c).
///
/// ```
/// use sdrkit_algo_fec::small::{GlonassCheck, GlonassHamming};
/// let g = GlonassHamming::new();
/// let mut s = [0u8; 85];
/// s[3] = 1; // string bit 82
/// g.encode_bits(&mut s)?;
/// s[40] ^= 1; // one data error
/// assert_eq!(g.check_bits(&mut s)?, GlonassCheck::Corrected);
/// assert_eq!(s[3], 1);
/// # Ok::<(), sdrkit_algo_fec::FecError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlonassHamming {
    code: SmallCode,
}

impl Default for GlonassHamming {
    fn default() -> Self {
        Self::new()
    }
}

impl GlonassHamming {
    /// The code with its syndrome table. This is a configuration call: it allocates.
    pub fn new() -> Self {
        let rows: Vec<u32> = (0..77)
            .map(|j| {
                let h = Self::index(85 - j);
                (((h.count_ones() & 1) ^ 1) << 7) | h
            })
            .collect();
        GlonassHamming {
            code: SmallCode::from_parity_rows(85, 77, &rows).expect("valid GLONASS code"),
        }
    }

    /// The Hamming index of string data bit `k` (9 to 85): the (k − 9)-th whole number from 3 up that is not a power of two.
    /// Check bit βi (i from 1 to 7) covers the data bits whose index has bit i − 1 set.
    fn index(k: usize) -> u32 {
        let mut left = k - 9;
        let mut h = 3u32;
        loop {
            if !h.is_power_of_two() {
                if left == 0 {
                    return h;
                }
                left -= 1;
            }
            h += 1;
        }
    }

    /// The code as a [`SmallCode`] (85 bits, 77 information bits, check bits β8 to β1 last).
    pub fn code(&self) -> &SmallCode {
        &self.code
    }

    /// Writes the check bits β8 to β1 into the last 8 of the 85 string bits (one bit per byte, string bit 85 first), from the
    /// first 77. Returns an error when `bits` does not hold 85 values.
    pub fn encode_bits(&self, bits: &mut [u8]) -> Result<()> {
        if bits.len() != 85 {
            return Err(FecError::InputLength {
                expected: 85,
                got: bits.len(),
            });
        }
        let info = bits[..77]
            .iter()
            .fold(0u128, |a, &b| (a << 1) | (b & 1) as u128);
        let check = self.code.encode(info) as u32;
        for (i, b) in bits[77..].iter_mut().enumerate() {
            *b = (check >> (7 - i)) as u8 & 1;
        }
        Ok(())
    }

    /// Checks a received string word (85 bits, string bit 85 most significant) by the rules a to c of the standard, and
    /// corrects a single data error in place.
    pub fn check(&self, word: &mut u128) -> GlonassCheck {
        let s = self.code.syndrome(*word);
        let c = s & 0x7F;
        // C-sigma: the parity of all 85 received bits.
        let c_sigma = (s >> 7) ^ (c.count_ones() & 1);
        match (c.count_ones(), c_sigma) {
            (0, 0) | (1, 1) => GlonassCheck::Good,
            (w, 1) if w >= 2 => match self.code.decode(*word) {
                Some((cw, 1)) => {
                    *word = cw;
                    GlonassCheck::Corrected
                }
                _ => GlonassCheck::Erased,
            },
            _ => GlonassCheck::Erased,
        }
    }

    /// [`GlonassHamming::check`] on 85 bits held one per byte, string bit 85 first. Returns an error when `bits` does not
    /// hold 85 values.
    pub fn check_bits(&self, bits: &mut [u8]) -> Result<GlonassCheck> {
        if bits.len() != 85 {
            return Err(FecError::InputLength {
                expected: 85,
                got: bits.len(),
            });
        }
        let mut w = bits.iter().fold(0u128, |a, &b| (a << 1) | (b & 1) as u128);
        let r = self.check(&mut w);
        if r == GlonassCheck::Corrected {
            for (i, b) in bits.iter_mut().enumerate() {
                *b = (w >> (84 - i)) as u8 & 1;
            }
        }
        Ok(r)
    }
}

/// Most bits a Chase search flips over.
pub const MAX_CHASE: usize = 16;

/// The parameters of a least-reliable-bits search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chase {
    /// How many of the least reliable bits to consider (at most 16).
    pub weakest: u8,
    /// Most of them flipped at once.
    pub max_flips: u8,
}

impl Default for Chase {
    /// The 6 weakest bits, up to 3 flips (Chase-II as used for POCSAG and FLEX).
    fn default() -> Self {
        Chase {
            weakest: 6,
            max_flips: 3,
        }
    }
}

/// The `count` least reliable positions of `rel` (by |value|) into `out`, most unreliable first; returns how many.
fn least_reliable(rel: &[f32], count: usize, out: &mut [(f32, u8); MAX_CHASE]) -> usize {
    let count = count.min(MAX_CHASE).min(rel.len());
    let mut len = 0usize;
    for (i, &v) in rel.iter().enumerate() {
        let a = if v.is_nan() { 0.0 } else { v.abs() };
        if len < count {
            out[len] = (a, i as u8);
            len += 1;
        } else if count > 0 && a < out[len - 1].0 {
            out[len - 1] = (a, i as u8);
        } else {
            continue;
        }
        // Keep sorted (insertion).
        let mut j = len - 1;
        while j > 0 && out[j].0 < out[j - 1].0 {
            out.swap(j, j - 1);
            j -= 1;
        }
    }
    len
}

/// The least-reliable-bits search for words protected by a check (a CRC, a parity, a syndrome): tries flipping up to
/// `chase.max_flips` of the `chase.weakest` least reliable bits of `bits` (one bit per byte, `rel[i]` the reliability of bit `i`)
/// and keeps the passing pattern with the smallest total reliability. On success `bits` holds the corrected word and the number
/// of flipped bits is returned; otherwise `bits` is left as it was. Positions are indexed up to 2^16.
///
/// ```
/// use sdrkit_algo_fec::small::{chase_search, Chase};
/// // A word checked by even parity in each half; two weak wrong bits.
/// let mut bits = [1u8, 0, 1, 1, 0, 1, 1, 1];
/// let rel = [9.0, 9.0, 0.5, 9.0, 9.0, 0.4, 9.0, 9.0];
/// let check = |b: &[u8]| b[..4].iter().sum::<u8>() % 2 == 0 && b[4..].iter().sum::<u8>() % 2 == 0;
/// assert_eq!(chase_search(&mut bits, &rel, Chase { weakest: 4, max_flips: 2 }, check), Some(2));
/// assert_eq!(bits, [1, 0, 0, 1, 0, 0, 1, 1]);
/// ```
pub fn chase_search(
    bits: &mut [u8],
    rel: &[f32],
    chase: Chase,
    mut check: impl FnMut(&[u8]) -> bool,
) -> Option<u32> {
    if rel.len() < bits.len() || bits.len() > 1 << 16 {
        return None;
    }
    let mut weak = [(f32::INFINITY, 0u16); MAX_CHASE];
    let count = (chase.weakest as usize).min(MAX_CHASE).min(bits.len());
    let mut len = 0usize;
    for (i, &v) in rel[..bits.len()].iter().enumerate() {
        let a = if v.is_nan() { 0.0 } else { v.abs() };
        if len < count {
            weak[len] = (a, i as u16);
            len += 1;
        } else if count > 0 && a < weak[len - 1].0 {
            weak[len - 1] = (a, i as u16);
        } else {
            continue;
        }
        let mut j = len - 1;
        while j > 0 && weak[j].0 < weak[j - 1].0 {
            weak.swap(j, j - 1);
            j -= 1;
        }
    }
    let mut best: Option<(u32, f32)> = None;
    for mask in 0u32..(1 << len) {
        if mask.count_ones() > chase.max_flips as u32 {
            continue;
        }
        let mut cost = 0f32;
        for (b, &(r, p)) in weak[..len].iter().enumerate() {
            if mask >> b & 1 != 0 {
                bits[p as usize] ^= 1;
                cost += r;
            }
        }
        let ok = check(bits);
        for (b, &(_, p)) in weak[..len].iter().enumerate() {
            if mask >> b & 1 != 0 {
                bits[p as usize] ^= 1;
            }
        }
        if ok && best.is_none_or(|(_, c)| cost < c) {
            best = Some((mask, cost));
        }
    }
    let (mask, _) = best?;
    for (b, &(_, p)) in weak[..len].iter().enumerate() {
        if mask >> b & 1 != 0 {
            bits[p as usize] ^= 1;
        }
    }
    Some(mask.count_ones())
}

/// A product code: a matrix whose rows are words of `row` and whose columns are words of `col` (DMR BPTC(196,96): Hamming (15,11)
/// rows and (13,9) columns, ETSI TS 102 361-1 annex B.1.1), decoded by alternating row and column passes.
#[derive(Clone, Debug)]
pub struct ProductCode {
    row: SmallCode,
    col: SmallCode,
}

impl ProductCode {
    /// The product of the row code and the column code.
    pub fn new(row: SmallCode, col: SmallCode) -> Self {
        ProductCode { row, col }
    }

    /// Rows (`col.n`) and columns (`row.n`) of the full matrix.
    pub fn shape(&self) -> (usize, usize) {
        (self.col.n(), self.row.n())
    }

    /// Encodes the `col.k × row.k` information bits (row by row) into the `col.n × row.n` matrix (row by row): row checks first,
    /// then column checks over every column.
    pub fn encode(&self, info: &[u8], out: &mut [u8]) -> Result<()> {
        let (kr, kc, nr, nc) = (self.row.k(), self.col.k(), self.row.n(), self.col.n());
        if info.len() != kr * kc {
            return Err(FecError::InputLength {
                expected: kr * kc,
                got: info.len(),
            });
        }
        if out.len() != nr * nc {
            return Err(FecError::OutputTooShort { need: nr * nc });
        }
        for i in 0..kc {
            let v = info[i * kr..(i + 1) * kr]
                .iter()
                .fold(0u128, |a, &b| (a << 1) | (b & 1) as u128);
            let cw = self.row.encode(v);
            for j in 0..nr {
                out[i * nr + j] = (cw >> (nr - 1 - j)) as u8 & 1;
            }
        }
        for j in 0..nr {
            let v = (0..kc).fold(0u128, |a, i| (a << 1) | out[i * nr + j] as u128);
            let cw = self.col.encode(v);
            for i in 0..nc {
                out[i * nr + j] = (cw >> (nc - 1 - i)) as u8 & 1;
            }
        }
        Ok(())
    }

    /// Decodes the matrix in place with up to `passes` row-then-column passes (stopping when a pass changes nothing).
    /// `corrected` counts the bits changed; `Err(Uncorrectable)` (the matrix keeps the passes' changes) when a row or column is
    /// still not a code word at the end.
    pub fn decode(&self, m: &mut [u8], passes: usize) -> Result<Decoded> {
        let (nr, nc) = (self.row.n(), self.col.n());
        if m.len() != nr * nc {
            return Err(FecError::InputLength {
                expected: nr * nc,
                got: m.len(),
            });
        }
        let mut total = 0u32;
        let mut used = 0;
        for _ in 0..passes.max(1) {
            used += 1;
            let mut changed = 0;
            for i in 0..nc {
                let v = (0..nr).fold(0u128, |a, j| (a << 1) | (m[i * nr + j] & 1) as u128);
                if let Some((cw, c)) = self.row.decode(v) {
                    if c > 0 {
                        for j in 0..nr {
                            m[i * nr + j] = (cw >> (nr - 1 - j)) as u8 & 1;
                        }
                        changed += c as u32;
                    }
                }
            }
            for j in 0..nr {
                let v = (0..nc).fold(0u128, |a, i| (a << 1) | (m[i * nr + j] & 1) as u128);
                if let Some((cw, c)) = self.col.decode(v) {
                    if c > 0 {
                        for i in 0..nc {
                            m[i * nr + j] = (cw >> (nc - 1 - i)) as u8 & 1;
                        }
                        changed += c as u32;
                    }
                }
            }
            total += changed;
            if changed == 0 {
                break;
            }
        }
        let rows_ok = (0..nc).all(|i| {
            let v = (0..nr).fold(0u128, |a, j| (a << 1) | (m[i * nr + j] & 1) as u128);
            self.row.syndrome(v) == 0
        });
        let cols_ok = (0..nr).all(|j| {
            let v = (0..nc).fold(0u128, |a, i| (a << 1) | (m[i * nr + j] & 1) as u128);
            self.col.syndrome(v) == 0
        });
        if !(rows_ok && cols_ok) {
            return Err(FecError::Uncorrectable);
        }
        Ok(Decoded {
            corrected: total,
            metric: 0,
            iterations: used,
        })
    }
}

/// The first-order Reed-Muller code word of `msg` (`m + 1` bits) of length `2^m`: bit `i` is the parity of `msg & i` over the
/// low `m` bits, XOR bit `m` of `msg` (a biorthogonal code: (32,6) for `m = 5`, (64,7) for `m = 6`). Bit `i` of the result is
/// code bit `i`.
pub fn rm1_encode(msg: u32, m: u32) -> u64 {
    let mut w = 0u64;
    let top = (msg >> m) & 1;
    for i in 0..(1u32 << m) {
        let b = ((msg & i & ((1 << m) - 1)).count_ones() & 1) ^ top;
        w |= (b as u64) << i;
    }
    w
}

/// Maximum-likelihood decoding of a first-order Reed-Muller (biorthogonal) code with the fast Hadamard transform. `soft` holds
/// `2^m` LLRs (positive means 0, code bit `i` at index `i`, `m` up to 16) and is transformed in place. Returns the message (as
/// for [`rm1_encode`]) and its correlation (larger is more certain).
pub fn fht_decode(soft: &mut [f32]) -> Result<(u32, f32)> {
    let len = soft.len();
    if len < 2 || !len.is_power_of_two() || len > 1 << 16 {
        return Err(FecError::InputLength {
            expected: 64,
            got: len,
        });
    }
    crate::llr::check_finite(soft)?;
    let mut h = 1;
    while h < len {
        for i in (0..len).step_by(2 * h) {
            for j in i..i + h {
                let (a, b) = (soft[j], soft[j + h]);
                soft[j] = a + b;
                soft[j + h] = a - b;
            }
        }
        h *= 2;
    }
    let (mut bi, mut bv) = (0usize, 0f32);
    for (i, &v) in soft.iter().enumerate() {
        if v.abs() > bv.abs() {
            bi = i;
            bv = v;
        }
    }
    let m = len.trailing_zeros();
    let top = (bv < 0.0) as u32;
    Ok(((top << m) | bi as u32, bv.abs()))
}

impl SmallCode {
    /// Golay (23,12), `g(x) = x^11 + x^10 + x^6 + x^5 + x^4 + x^2 + 1` (0xC75; 6165 octal in ETSI TS 102 361-1 equation B.4):
    /// perfect, corrects 3. DCS, FreeDV, MAC.
    pub fn golay_23_12() -> Self {
        Self::cyclic(23, 0xC75).expect("valid Golay")
    }

    /// Extended Golay (24,12): (23,12) plus even parity (M17 specification appendix D; TS 102 361-1 clause B.3.1). Corrects 3,
    /// detects 4. ALE, P25, dPMR, M17, YSF.
    pub fn golay_24_12() -> Self {
        Self::golay_23_12().extended().expect("valid Golay")
    }

    /// DMR Golay (20,8) (TS 102 361-1 clause B.3.1, table B.11): the extended code shortened by 4. Slot type.
    pub fn golay_20_8() -> Self {
        Self::golay_24_12().shortened(4).expect("valid Golay")
    }

    /// Golay (18,6): the extended code shortened by 6 (P25 HDU) (TIA-102 is not free).
    pub fn golay_18_6() -> Self {
        Self::golay_24_12().shortened(6).expect("valid Golay")
    }

    /// DMR quadratic residue code QR(16,7,6) (TS 102 361-1 clause B.3.2, table B.12): the cyclic (17,9) code with
    /// `G(x) = x^8 + x^5 + x^4 + x^3 + 1`, shortened by 2, plus even parity. EMB.
    pub fn qr_16_7() -> Self {
        Self::cyclic(17, 0x139)
            .and_then(|c| c.shortened(2))
            .and_then(|c| c.extended())
            .expect("valid QR")
    }

    /// Hamming (7,4), `G(x) = x^3 + x + 1` (TS 102 361-1 clause B.3.5, table B.17).
    pub fn hamming_7_4() -> Self {
        Self::cyclic(7, 0xB).expect("valid Hamming")
    }

    /// Extended Hamming (8,4): corrects 1, detects 2 (LoRa 4/8, DFM, P25 phase 2 DUID).
    pub fn hamming_8_4() -> Self {
        Self::hamming_7_4().extended().expect("valid Hamming")
    }

    /// Hamming (15,11), `G(x) = x^4 + x + 1` (TS 102 361-1 clause B.3.4, table B.15). Also BeiDou BCH(15,11) (BDS-SIS-ICD-B1I-3.0
    /// clause 5.1.3, `g(X) = 1 + X + X^4`).
    pub fn hamming_15_11() -> Self {
        Self::cyclic(15, 0x13).expect("valid Hamming")
    }

    /// Hamming (13,9): (15,11) shortened by 2 (TS 102 361-1 table B.14).
    pub fn hamming_13_9() -> Self {
        Self::hamming_15_11().shortened(2).expect("valid Hamming")
    }

    /// Hamming (16,11,4): (15,11) plus even parity (TS 102 361-1 table B.16).
    pub fn hamming_16_11() -> Self {
        Self::hamming_15_11().extended().expect("valid Hamming")
    }

    /// Hamming (17,12,3), `g(x) = x^5 + x^2 + 1` (TS 102 361-1 clause B.3.3, table B.13). CACH short LC.
    pub fn hamming_17_12() -> Self {
        Self::cyclic(17, 0x25).expect("valid Hamming")
    }

    /// Hamming (10,6): (15,11) shortened by 5 (P25).
    pub fn hamming_10_6() -> Self {
        Self::hamming_15_11().shortened(5).expect("valid Hamming")
    }

    /// Hamming (12,8): (15,11) shortened by 3, the code of ETSI TS 102 490 clause 7.3 table 7.1 (TS 102 658 clause 7.2
    /// table 7.2): X7 sent first, C3 to C0 last.
    pub fn hamming_12_8() -> Self {
        Self::hamming_15_11().shortened(3).expect("valid Hamming")
    }

    /// POCSAG / FLEX / GOES DCS BCH(31,21) plus even parity (ITU-R M.584-2 annex 1 clause 1.4: `x^10 + x^9 + x^8 + x^6 + x^5 + x^3
    /// + 1`). 32 bits, corrects 2.
    pub fn pocsag_32_21() -> Self {
        Self::cyclic(31, 0x769)
            .and_then(|c| c.extended())
            .expect("valid BCH")
    }

    /// BCH(63,51) `0x1539` (Cospas-Sarsat C/S T.001 annex B2: `1 + x^3 + x^4 + x^5 + x^8 + x^10 + x^12`), corrects 2. Shortened
    /// by 25 it is Sarsat BCH-2 (38,26); by 23 EDACS (40,28); unshortened, the Datawell HXV code.
    pub fn bch_63_51() -> Self {
        Self::cyclic(63, 0x1539).expect("valid BCH")
    }

    /// BCH(127,113) `0x4377` (EN 300 744 clause 4.6.2.11: `x^14 + x^9 + x^8 + x^6 + x^5 + x^4 + x^2 + x + 1`) shortened to `n`
    /// bits: (67,53) for DVB-T TPS, (71,57) and (94,80) for MAC.
    pub fn bch_127_113(n: usize) -> Result<Self> {
        if !(15..=127).contains(&n) {
            return Param::N.range(15, 127, n);
        }
        Self::cyclic(127, 0x4377)?.shortened(127 - n)
    }

    /// MPT 1327 codeword (MPT 1327 clause 3.2.3): the (63,48) cyclic code `X^15 + X^14 + X^13 + X^11 + X^4 + X^2 + 1` with its
    /// last check bit inverted, then even parity: 64 bits.
    pub fn mpt1327_64_48() -> Self {
        Self::cyclic(63, 0xE815)
            .map(|c| c.with_offset(1))
            .and_then(|c| c.extended())
            .expect("valid MPT 1327 code")
    }

    /// RDS / RBDS block code (NRSC-4-A clause 2.3, IEC 62106): the (26,16) shortened cyclic code with `g(x) = x^10 + x^8 + x^7 +
    /// x^5 + x^4 + x^3 + 1`, decoding bursts up to 5 bits. XOR the block's offset word onto the check bits before decoding (or
    /// use [`SmallCode::with_offset`]).
    pub fn rds_26_16() -> Self {
        Self::cyclic(26, 0x5B9)
            .and_then(|c| c.with_burst_table(5))
            .expect("valid RDS code")
    }

    /// TETRA shortened (30,14) Reed-Muller code (EN 300 392-2 clause 8.2.3.2, equation 8.13): AACH.
    pub fn tetra_rm_30_14() -> Self {
        const P: [u32; 14] = [
            0b1001101101100000,
            0b0010110111100000,
            0b1111110000100000,
            0b1110000000111100,
            0b1001100000111010,
            0b0101010000110110,
            0b0010110000101110,
            0b1111111111011111,
            0b1000001100111001,
            0b0100001010110101,
            0b0010000110101101,
            0b0001001001110011,
            0b0000100101101011,
            0b0000010011100111,
        ];
        Self::from_parity_rows(30, 14, &P).expect("valid RM code")
    }

    /// TETRA (16,5) Reed-Muller code for QAM (EN 300 392-2 clause 8.2.3.5, equation 8.33).
    pub fn tetra_rm_16_5() -> Self {
        const P: [u32; 5] = [
            0b01110111100,
            0b00001111111,
            0b11100001111,
            0b10111010101,
            0b11011100110,
        ];
        Self::from_parity_rows(16, 5, &P).expect("valid RM code")
    }

    /// ERMES code word (ETS 300 133-4): the shortened cyclic (30,18) code with `g(x) = x^12 + x^11 + x^9 + x^7 + x^6 + x^3 +
    /// x^2 + 1`. Corrects 2.
    pub fn ermes_30_18() -> Self {
        Self::cyclic(30, 0x1ACD).expect("valid ERMES code")
    }

    /// The (63,45) cyclic code of end-of-train telemetry, `g(x) = x^18 + x^17 + x^16 + x^15 + x^12 + x^11 + x^9 + x^3 + x^2 +
    /// x + 1`, with the fixed key `101011011101110000` XORed onto the check bits. The data bits go into the word last sent
    /// first. No public document confirms the polynomial or the key.
    pub fn eot_63_45() -> Self {
        Self::cyclic(63, 0x79A0F)
            .map(|c| c.with_offset(0x2B770))
            .expect("valid end-of-train code")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hamming_fills_radius_one() {
        let h = SmallCode::hamming_7_4();
        assert_eq!(h.radius(), 1);
        for info in 0..16u128 {
            let cw = h.encode(info);
            for b in 0..7 {
                assert_eq!(h.decode(cw ^ (1 << b)), Some((cw, 1)));
            }
        }
        let e = SmallCode::hamming_8_4();
        assert_eq!(e.radius(), 1);
        assert_eq!(e.decode(e.encode(5) ^ 0b11), None);
    }

    #[test]
    fn fht_round_trip() {
        for m in [5u32, 6] {
            for msg in [0u32, 1, 37, (1 << (m + 1)) - 1] {
                let w = rm1_encode(msg, m);
                let mut soft: Vec<f32> = (0..1 << m)
                    .map(|i| if w >> i & 1 == 0 { 1.0 } else { -1.0 })
                    .collect();
                soft[3] = -soft[3];
                assert_eq!(fht_decode(&mut soft).unwrap().0, msg);
            }
        }
        assert!(fht_decode(&mut [0.0; 3]).is_err());
    }

    #[test]
    fn ermes_code_words() {
        let c = SmallCode::ermes_30_18();
        assert_eq!(c.radius(), 2);
        assert_eq!(c.encode(1), 0b000000000000000001101011001101);
        assert_eq!(c.encode(0x3FFFF), 0b111111111111111111110001001011);
        let w = c.encode(0x12345);
        assert_eq!(w, 0x1234_57F6);
        // Bits 3 and 20 (counted from the least significant) repaired; a third error is refused.
        let two = w ^ (1 << 3) ^ (1 << 20);
        assert_eq!(c.decode(two), Some((w, 2)));
        assert_eq!(c.info(c.decode(two).unwrap().0), 0x12345);
        assert_eq!(c.decode(two ^ (1 << 28)), None);
    }

    #[test]
    fn end_of_train_code_word() {
        let c = SmallCode::eot_63_45();
        assert!(c.radius() >= 2, "{}", c.radius());
        // The data bits d0 to d44 of a frame, sent d0 first; the code word holds them d44 first.
        let d = "001100010011100000011000010110100100110100101";
        let info = d
            .bytes()
            .rev()
            .fold(0u128, |w, b| (w << 1) | u128::from(b - b'0'));
        let word = c.encode(info);
        assert_eq!(word & 0x3FFFF, 0b100001000111110101);
        assert_eq!(c.decode(word ^ (1 << 7) ^ (1 << 50)), Some((word, 2)));
    }
}
