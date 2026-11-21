//! Systematic encoding of long binary cyclic codes (generator degree up to 256) a byte at a time, and one-step majority-logic
//! decoding of difference-set cyclic codes.
//!
//! The encoder divides by the generator with a 256-entry table, eight bits per step, so a DVB-S2 BCH word (64 800 bits) takes
//! about 8 000 table steps instead of 64 800 shift-and-XOR steps over a 192-bit register.

use crate::error::{Decoded, FecError, Param, Result};

/// Most generator degree [`CyclicEncoder`] accepts.
pub const MAX_DEGREE: usize = 256;
const W: usize = MAX_DEGREE / 64;

/// A big polynomial over GF(2) as little-endian `u64` words (bit `i` of the whole = coefficient of `x^i`).
pub(crate) type Poly = Vec<u64>;

pub(crate) fn poly_deg(p: &[u64]) -> Option<usize> {
    for (w, &v) in p.iter().enumerate().rev() {
        if v != 0 {
            return Some(64 * w + 63 - v.leading_zeros() as usize);
        }
    }
    None
}

pub(crate) fn poly_bit(p: &[u64], i: usize) -> bool {
    p.get(i / 64).is_some_and(|w| w >> (i % 64) & 1 != 0)
}

pub(crate) fn poly_mul(a: &[u64], b: &[u64]) -> Poly {
    let (Some(da), Some(db)) = (poly_deg(a), poly_deg(b)) else {
        return vec![0];
    };
    let mut r = vec![0u64; (da + db) / 64 + 1];
    for i in 0..=db {
        if poly_bit(b, i) {
            for j in 0..=da {
                if poly_bit(a, j) {
                    r[(i + j) / 64] ^= 1 << ((i + j) % 64);
                }
            }
        }
    }
    r
}

/// `a / b` and `a mod b`.
pub(crate) fn poly_divmod(a: &[u64], b: &[u64]) -> Option<(Poly, Poly)> {
    let db = poly_deg(b)?;
    let mut r = a.to_vec();
    let da = poly_deg(&r).unwrap_or(0);
    let mut q = vec![0u64; da / 64 + 1];
    while let Some(d) = poly_deg(&r) {
        if d < db {
            break;
        }
        let s = d - db;
        q[s / 64] ^= 1 << (s % 64);
        for j in 0..=db {
            if poly_bit(b, j) {
                let k = j + s;
                r[k / 64] ^= 1 << (k % 64);
            }
        }
    }
    Some((q, r))
}

/// A systematic encoder for a cyclic code (or a shortened one) given by its generator.
///
/// Bits are `u8`, one per byte, the first bit sent first: `k` information bits, then `n − k` check bits, the check bits being the
/// remainder of `info(x) x^(n−k)` divided by `g(x)` (the first bit sent is the highest power).
///
/// ```
/// use sdrkit_algo_fec::cyclic::CyclicEncoder;
/// // POCSAG BCH(31,21), g(x) = x^10 + x^9 + x^8 + x^6 + x^5 + x^3 + 1 (ITU-R M.584-2 annex 1, clause 1.4).
/// let enc = CyclicEncoder::new(&[0x769]).unwrap();
/// // The synchronisation codeword 7CD215D8 (table 1): 21 information bits, 10 check bits, even parity.
/// let word = 0x7CD2_15D8u32 >> 1;
/// let bits: Vec<u8> = (0..31).rev().map(|i| (word >> i) as u8 & 1).collect();
/// let mut check = [0u8; 10];
/// enc.check_bits(&bits[..21], &mut check).unwrap();
/// assert_eq!(&check[..], &bits[21..]);
/// ```
#[derive(Clone, Debug)]
pub struct CyclicEncoder {
    deg: usize,
    gen: Poly,
    /// `table[v]` = `(v(x) x^deg) mod g(x)` for the byte `v` (bit 7 the highest power), when `deg >= 8`.
    table: Vec<[u64; W]>,
}

impl CyclicEncoder {
    /// An encoder for the generator `g` (little-endian words, bit `i` the coefficient of `x^i`, degree 1 to 256).
    pub fn new(g: &[u64]) -> Result<Self> {
        let deg = poly_deg(g).ok_or(FecError::Degenerate {
            param: Param::Generator,
        })?;
        if deg == 0 || deg > MAX_DEGREE {
            return Param::Generator.range(1, MAX_DEGREE as i64, deg);
        }
        if !poly_bit(g, 0) {
            return Param::Generator.degenerate();
        }
        let mut gen = g.to_vec();
        gen.resize(deg / 64 + 1, 0);
        let mut e = CyclicEncoder {
            deg,
            gen,
            table: Vec::new(),
        };
        if deg >= 8 {
            e.table = (0..256u32)
                .map(|v| {
                    let mut r = [0u64; W];
                    for b in (0..8).rev() {
                        e.shift_in(&mut r, (v >> b) as u8 & 1);
                    }
                    r
                })
                .collect();
        }
        Ok(e)
    }

    /// The generator degree, `n − k`.
    pub fn degree(&self) -> usize {
        self.deg
    }

    /// The generator.
    pub fn generator(&self) -> &[u64] {
        &self.gen
    }

    /// One bit into the division register (bit `deg − 1` the top).
    #[inline]
    fn shift_in(&self, r: &mut [u64; W], bit: u8) {
        let top = ((r[(self.deg - 1) / 64] >> ((self.deg - 1) % 64)) & 1) as u8 ^ (bit & 1);
        shl(r, 1);
        mask(r, self.deg);
        if top == 1 {
            for (i, v) in r.iter_mut().enumerate() {
                // g without its x^deg term.
                let g = self.gen.get(i).copied().unwrap_or(0);
                *v ^= g;
            }
            mask(r, self.deg);
        }
    }

    /// The remainder of `bits(x) x^deg` (first bit the highest power) into `r`.
    fn remainder(&self, bits: &[u8], r: &mut [u64; W]) {
        *r = [0; W];
        if self.deg < 8 {
            for &b in bits {
                self.shift_in(r, b);
            }
            return;
        }
        let lead = bits.len() % 8;
        for &b in &bits[..lead] {
            self.shift_in(r, b);
        }
        for c in bits[lead..].as_chunks::<8>().0 {
            let byte = c.iter().fold(0u8, |a, &b| (a << 1) | (b & 1));
            let top = top8(r, self.deg) ^ byte;
            shl(r, 8);
            mask(r, self.deg);
            let t = &self.table[top as usize];
            for i in 0..W {
                r[i] ^= t[i];
            }
        }
    }

    /// Writes the `n − k` check bits of the information bits `info` into `check`.
    pub fn check_bits(&self, info: &[u8], check: &mut [u8]) -> Result<()> {
        if check.len() != self.deg {
            return Err(FecError::OutputTooShort { need: self.deg });
        }
        let mut r = [0u64; W];
        self.remainder(info, &mut r);
        for (i, c) in check.iter_mut().enumerate() {
            let p = self.deg - 1 - i;
            *c = (r[p / 64] >> (p % 64)) as u8 & 1;
        }
        Ok(())
    }

    /// Encodes in place: `word[..len − deg]` holds the information bits; the check bits are written after them.
    pub fn encode(&self, word: &mut [u8]) -> Result<()> {
        if word.len() <= self.deg {
            return Err(FecError::InputLength {
                expected: self.deg + 1,
                got: word.len(),
            });
        }
        let k = word.len() - self.deg;
        let (info, check) = word.split_at_mut(k);
        self.check_bits(info, check)
    }

    /// Whether `word` (information then check bits) is a code word.
    pub fn is_codeword(&self, word: &[u8]) -> bool {
        if word.len() <= self.deg {
            return false;
        }
        // The remainder of word(x) = info(x) x^deg + check(x): divide info, then add the check bits.
        let k = word.len() - self.deg;
        let mut r = [0u64; W];
        self.remainder(&word[..k], &mut r);
        (0..self.deg).all(|i| {
            let p = self.deg - 1 - i;
            ((r[p / 64] >> (p % 64)) as u8 & 1) == word[k + i] & 1
        })
    }
}

#[inline]
fn top8(r: &[u64; W], deg: usize) -> u8 {
    let mut v = 0u8;
    for i in 0..8 {
        let p = deg - 1 - i;
        v = (v << 1) | ((r[p / 64] >> (p % 64)) as u8 & 1);
    }
    v
}

#[inline]
fn shl(r: &mut [u64; W], s: u32) {
    for i in (0..W).rev() {
        let lo = if i > 0 { r[i - 1] >> (64 - s) } else { 0 };
        r[i] = (r[i] << s) | lo;
    }
}

#[inline]
fn mask(r: &mut [u64; W], deg: usize) {
    for (i, v) in r.iter_mut().enumerate() {
        let lo = 64 * i;
        if lo >= deg {
            *v = 0;
        } else if deg - lo < 64 {
            *v &= (1u64 << (deg - lo)) - 1;
        }
    }
}

/// One-step majority-logic decoding of a cyclic code whose parity checks are orthogonal on every position: a perfect
/// difference-set cyclic code such as the (273,191) code of ISDB-T TMCC (ARIB STD-B31 clause 3.15.6.9), shortened to (184,102).
/// With `J` orthogonal checks per bit it corrects any `⌊J / 2⌋` errors (8 for the (273,191) code).
///
/// The checks are the cyclic shifts of a set `S` of positions: check `j` is the parity of the bits at `(j + d) mod N`, `d` in `S`.
#[derive(Clone, Debug)]
pub struct MajorityLogic {
    full: usize,
    n: usize,
    set: Vec<u16>,
    enc: CyclicEncoder,
    word: Vec<u8>,
    checks: Vec<u8>,
    tmp: Vec<u8>,
}

impl MajorityLogic {
    /// The decoder for the cyclic code of length `full` with generator `g`, sent shortened to `n` bits, with checks from the
    /// position set `set`. An error unless every shift of `set` is a parity check of the code (a multiple of `g`) and the
    /// differences of `set` are distinct (the checks are orthogonal).
    pub fn new(full: usize, g: &[u64], n: usize, set: &[u16]) -> Result<Self> {
        let enc = CyclicEncoder::new(g)?;
        Param::N.check(full, 1, 4096)?;
        if n > full || n <= enc.degree() {
            return Param::N.range(enc.degree() as i64 + 1, full as i64, n);
        }
        if set.is_empty() {
            return Param::DifferenceSet.degenerate();
        }
        if let Some(&d) = set.iter().find(|&&d| d as usize >= full) {
            return Param::DifferenceSet.range(0, full as i64 - 1, d);
        }
        let mut seen = vec![false; full];
        for &a in set {
            for &b in set {
                if a != b {
                    let d = (a as usize + full - b as usize) % full;
                    if seen[d] {
                        return Param::DifferenceSet.degenerate();
                    }
                    seen[d] = true;
                }
            }
        }
        // Every shift of g (a code word) must satisfy every check: the cyclic correlation of g with the set is even.
        for j in 0..full {
            let par = set
                .iter()
                .filter(|&&d| poly_bit(g, (j + d as usize) % full))
                .count();
            if par % 2 != 0 {
                return Param::DifferenceSet.conflict(Param::Generator);
            }
        }
        Ok(MajorityLogic {
            full,
            n,
            set: set.to_vec(),
            enc,
            word: vec![0; full],
            checks: vec![0; full],
            tmp: vec![0; n],
        })
    }

    /// The ISDB-T TMCC code (ARIB STD-B31 clause 3.15.6.9): the (273,191) difference-set cyclic code with the published
    /// generator of degree 82, shortened to (184,102). Its 17 orthogonal checks come from the Singer difference set of the
    /// projective plane over GF(16) (the points of a line: the `α^i` of GF(2^12) whose trace to GF(16) is 0), oriented and
    /// matched to the generator.
    pub fn isdb_t_tmcc() -> Result<Self> {
        let mut g = [0u64; 2];
        for e in [
            82u32, 77, 76, 71, 67, 66, 56, 52, 48, 40, 36, 34, 24, 22, 18, 10, 4, 0,
        ] {
            g[(e / 64) as usize] |= 1 << (e % 64);
        }
        let f = crate::gf::Gf::new(12, 0x1053)?;
        let set: Vec<u16> = (0..273usize)
            .filter(|&i| {
                let x = f.exp(i);
                x ^ f.pow(x, 16) ^ f.pow(x, 256) == 0
            })
            .map(|i| i as u16)
            .collect();
        if set.len() != 17 {
            return Param::DifferenceSet.not_allowed(set.len());
        }
        // The code uses one image of the set under a multiplier t prime to 273 (another primitive element); find it.
        let mut cand = [0u16; 17];
        for t in 1..273usize {
            if gcd(t, 273) != 1 {
                continue;
            }
            for (c, &d) in cand.iter_mut().zip(&set) {
                *c = ((d as usize * t) % 273) as u16;
            }
            if let Ok(ml) = Self::new(273, &g, 184, &cand) {
                return Ok(ml);
            }
        }
        Param::DifferenceSet.unsupported()
    }

    /// Checks per bit (`J`); the decoder corrects `J / 2` errors.
    pub fn checks_per_bit(&self) -> usize {
        self.set.len()
    }

    /// The encoder (the same code).
    pub fn encoder(&self) -> &CyclicEncoder {
        &self.enc
    }

    /// Corrects `bits` (`n` bits, information first) in place; `corrected` in the result is the number of bits flipped.
    /// `Err(Uncorrectable)` (and no change) when the result is not a code word.
    pub fn decode(&mut self, bits: &mut [u8]) -> Result<Decoded> {
        if bits.len() != self.n {
            return Err(FecError::InputLength {
                expected: self.n,
                got: bits.len(),
            });
        }
        let full = self.full;
        let n = self.n;
        // Word by power: the first sent bit is x^(n-1); the shortened (zero) bits are the powers n .. full - 1.
        self.word.fill(0);
        for (i, &b) in bits.iter().enumerate() {
            self.word[n - 1 - i] = b & 1;
        }
        for j in 0..full {
            let mut s = 0u8;
            for &d in &self.set {
                s ^= self.word[(j + d as usize) % full];
            }
            self.checks[j] = s;
        }
        // The checks containing position p are j = p - d.
        let jn = self.set.len();
        let mut flips = 0u32;
        for p in 0..n {
            let fails = self
                .set
                .iter()
                .filter(|&&d| self.checks[(p + full - d as usize) % full] == 1)
                .count();
            if 2 * fails > jn {
                self.word[p] ^= 1;
                flips += 1;
            }
        }
        for (i, t) in self.tmp.iter_mut().enumerate() {
            *t = self.word[n - 1 - i];
        }
        if !self.enc.is_codeword(&self.tmp) {
            return Err(FecError::Uncorrectable);
        }
        bits.copy_from_slice(&self.tmp);
        Ok(Decoded::corrected(flips))
    }
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytewise_and_bitwise_division_agree() {
        // A degree-20 generator and a degree-5 one.
        for g in [0x1_A4C9u64 | (1 << 20), 0b100101] {
            let enc = CyclicEncoder::new(&[g]).unwrap();
            let info: Vec<u8> = (0..77).map(|i| ((i * 13 + 5) % 7 % 2) as u8).collect();
            let deg = enc.degree();
            let mut word = info.clone();
            word.resize(77 + deg, 0);
            enc.encode(&mut word).unwrap();
            assert!(enc.is_codeword(&word));
            // Bit-serial check: the whole word divides by g.
            let mut r = 0u64;
            for &b in &word {
                r = (r << 1) | b as u64;
                if r >> deg & 1 == 1 {
                    r ^= g;
                }
            }
            assert_eq!(r, 0);
            word[3] ^= 1;
            assert!(!enc.is_codeword(&word));
        }
        assert!(CyclicEncoder::new(&[0]).is_err());
        assert!(CyclicEncoder::new(&[0b110]).is_err());
    }
}
