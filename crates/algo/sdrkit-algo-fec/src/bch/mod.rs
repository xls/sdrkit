//! Binary BCH codes of any length, `t` and field (narrow sense: roots `α^1 .. α^(2t)`), shortened as needed, with algebraic
//! decoding: syndromes from remainders by the minimal polynomials (a byte at a time), Berlekamp-Massey, and a Chien search over
//! the sent positions.
//!
//! The DVB-S2 / DVB-T2 outer codes (t = 8, 10, 12 over GF(2^16), t = 12 over GF(2^14)) encode a byte at a time, never bit by bit.
//!
//! Every code a standard names is in [`codes`] (DVB-S2 and S2X, Cospas-Sarsat, POCSAG, P25, DVB-T TPS, BeiDou ...), with a
//! known-answer test from its standard; [`Bch::from_code`] builds its decoder.
//!
//! ```
//! use sdrkit_algo_fec::bch::{codes, Bch};
//! // DVB-S2 normal frame, rate 1/2: N_bch = 32 400, t = 12 (EN 302 307-1 table 5a).
//! let mut bch = Bch::from_code(&codes::DVB_S2_NORMAL_1_2).unwrap();
//! assert_eq!(bch.k(), 32_208);
//! let mut word = vec![0u8; 32_400];
//! for (i, b) in word[..32_208].iter_mut().enumerate() {
//!     *b = (i % 3 == 0) as u8;
//! }
//! bch.encode(&mut word).unwrap();
//! let sent = word.clone();
//! for i in [5, 900, 17_000, 32_399] {
//!     word[i] ^= 1;
//! }
//! assert_eq!(bch.decode(&mut word).unwrap().corrected, 4);
//! assert_eq!(word, sent);
//! ```

use crate::cyclic::{poly_divmod, poly_mul, CyclicEncoder};
use crate::error::{Decoded, FecError, Param, Result};
use crate::gf::Gf;

pub mod codes;

pub use codes::BchCode;

/// A minimal polynomial with its byte-wise remainder table and the odd roots that share it.
#[derive(Clone, Debug)]
struct MinPoly {
    poly: u32,
    deg: u32,
    /// `table[v] = v(x) x^deg mod poly` (when `deg >= 8`).
    table: Vec<u32>,
    /// The odd `j <= 2t − 1` whose root `α^j` it has.
    js: Vec<usize>,
}

/// A narrow-sense binary BCH code and its decoder.
#[derive(Clone, Debug)]
pub struct Bch {
    n: usize,
    t: usize,
    gf: Gf,
    enc: CyclicEncoder,
    mins: Vec<MinPoly>,
    synd: Vec<u16>,
    c: Vec<u16>,
    b: Vec<u16>,
    tmp: Vec<u16>,
    locs: Vec<u32>,
    logs: Vec<i64>,
    /// The word in bytes, for the syndromes.
    packed: Vec<u8>,
}

impl Bch {
    /// The narrow-sense code over GF(2^m) with primitive polynomial `poly`, correcting `t` errors, sent `n` bits long (at most
    /// `2^m − 1`; shorter is a shortened code). `k = n − deg g`.
    pub fn new(m: u8, poly: u32, n: usize, t: usize) -> Result<Self> {
        let gf = Gf::new(m, poly)?;
        let nn = gf.order();
        if t == 0 {
            return Param::T.range(1, i64::MAX, 0);
        }
        Param::N.check(n, 1, nn as i64)?;
        if 2 * t >= nn {
            return Param::T.conflict(Param::M);
        }
        // Distinct minimal polynomials of α^j, j odd up to 2t - 1.
        let mut mins: Vec<MinPoly> = Vec::new();
        for j in (1..2 * t).step_by(2) {
            let p = gf.minimal_poly(j);
            if let Some(mp) = mins.iter_mut().find(|mp| mp.poly as u64 == p) {
                mp.js.push(j);
                continue;
            }
            let deg = 63 - p.leading_zeros();
            let mut mp = MinPoly {
                poly: p as u32,
                deg,
                table: Vec::new(),
                js: vec![j],
            };
            if deg >= 8 {
                mp.table = (0..256u32).map(|v| rem_bits(v as u64, 8, &mp)).collect();
            }
            mins.push(mp);
        }
        let mut g: Vec<u64> = vec![1];
        for mp in &mins {
            g = poly_mul(&g, &[mp.poly as u64]);
        }
        let enc = CyclicEncoder::new(&g)?;
        if enc.degree() >= n {
            return Param::N.range(enc.degree() as i64 + 1, i64::MAX, n);
        }
        Ok(Bch {
            n,
            t,
            enc,
            mins,
            synd: vec![0; 2 * t + 1],
            c: vec![0; 2 * t + 2],
            b: vec![0; 2 * t + 2],
            tmp: vec![0; 2 * t + 2],
            locs: vec![0; t],
            logs: vec![0; 2 * t + 2],
            packed: Vec::with_capacity(n / 8),
            gf,
        })
    }

    /// The decoder of a named code ([`codes`]). An error when the field, `n` and `t` do not give the code's `k`.
    pub fn from_code(code: &BchCode) -> Result<Self> {
        let b = Bch::new(code.m, code.poly, code.n, code.t)?;
        if b.k() != code.k {
            return Param::K.conflict(Param::T);
        }
        Ok(b)
    }

    /// The narrow-sense code whose generator is `g` (little-endian words), sent `n` bits long, correcting `t` errors: finds the
    /// field (`m` up to 14) whose primitive polynomial divides `g` and returns exactly `g`. An error when no field does.
    pub fn from_generator(n: usize, g: &[u64], t: usize) -> Result<Self> {
        let mut m = 2u8;
        while (1usize << m) - 1 < n {
            m += 1;
        }
        while m <= 14 {
            for p in (1u32 << m) + 1..(1u32 << (m + 1)) {
                if p & 1 == 0 {
                    continue;
                }
                let (_, r) = poly_divmod(g, &[p as u64]).ok_or(FecError::Degenerate {
                    param: Param::Generator,
                })?;
                if r.iter().any(|&w| w != 0) {
                    continue;
                }
                if let Ok(b) = Bch::new(m, p, n, t) {
                    let mine = b.generator();
                    let len = mine.len().max(g.len());
                    if (0..len).all(|i| {
                        mine.get(i).copied().unwrap_or(0) == g.get(i).copied().unwrap_or(0)
                    }) {
                        return Ok(b);
                    }
                }
            }
            m += 1;
        }
        Param::Generator.degenerate()
    }

    /// Bits per code word.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Information bits.
    pub fn k(&self) -> usize {
        self.n - self.enc.degree()
    }

    /// Errors corrected.
    pub fn t(&self) -> usize {
        self.t
    }

    /// The field.
    pub fn field(&self) -> &Gf {
        &self.gf
    }

    /// The generator (little-endian words).
    pub fn generator(&self) -> &[u64] {
        self.enc.generator()
    }

    /// Encodes in place: `word` holds `n` bits, the first `k` the information; the parity is written after them.
    pub fn encode(&self, word: &mut [u8]) -> Result<()> {
        if word.len() != self.n {
            return Err(FecError::InputLength {
                expected: self.n,
                got: word.len(),
            });
        }
        self.enc.encode(word)
    }

    /// Whether `word` is a code word.
    pub fn is_codeword(&self, word: &[u8]) -> bool {
        word.len() == self.n && self.enc.is_codeword(word)
    }

    /// Computes the syndromes `S_1 .. S_2t` of `word`; returns whether all are zero.
    fn syndromes(&mut self, word: &[u8]) -> bool {
        let nn = self.gf.order();
        let mut zero = true;
        // The bits go into bytes once, for every minimal polynomial.
        let lead = word.len() % 8;
        self.packed.clear();
        self.packed.extend(
            word[lead..]
                .as_chunks::<8>()
                .0
                .iter()
                .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | (b & 1))),
        );
        for mp in &self.mins {
            let r = rem_word(&word[..lead], &self.packed, mp);
            for &j in &mp.js {
                // S_j = r(α^j).
                let mut s = 0u16;
                for i in 0..mp.deg as usize {
                    if r >> i & 1 != 0 {
                        s ^= self.gf.exp((i * j) % nn);
                    }
                }
                self.synd[j] = s;
                zero &= s == 0;
            }
        }
        for j in (2..=2 * self.t).step_by(2) {
            self.synd[j] = self.gf.mul(self.synd[j / 2], self.synd[j / 2]);
        }
        zero
    }

    /// Corrects up to `t` errors in `word` (`n` bits, one per byte) in place. `corrected` is the number of bits flipped.
    /// `Err(Uncorrectable)` leaves the word as received.
    pub fn decode(&mut self, word: &mut [u8]) -> Result<Decoded> {
        if word.len() != self.n {
            return Err(FecError::InputLength {
                expected: self.n,
                got: word.len(),
            });
        }
        if self.syndromes(word) {
            return Ok(Decoded::corrected(0));
        }
        let gf = &self.gf;
        let t2 = 2 * self.t;
        // Berlekamp-Massey: C(x) the connection polynomial.
        self.c.fill(0);
        self.b.fill(0);
        self.c[0] = 1;
        self.b[0] = 1;
        let (mut l, mut shift, mut bd) = (0usize, 1usize, 1u16);
        for r in 0..t2 {
            let mut d = self.synd[r + 1];
            for i in 1..=l {
                d ^= gf.mul(self.c[i], self.synd[r + 1 - i]);
            }
            if d == 0 {
                shift += 1;
                continue;
            }
            let coef = gf.div(d, bd).unwrap_or(0);
            if 2 * l <= r {
                self.tmp.copy_from_slice(&self.c);
                for i in 0..self.c.len() - shift {
                    self.c[i + shift] ^= gf.mul(coef, self.b[i]);
                }
                l = r + 1 - l;
                self.b.copy_from_slice(&self.tmp);
                bd = d;
                shift = 1;
            } else {
                for i in 0..self.c.len() - shift {
                    self.c[i + shift] ^= gf.mul(coef, self.b[i]);
                }
                shift += 1;
            }
        }
        let deg = (0..self.c.len())
            .rev()
            .find(|&i| self.c[i] != 0)
            .unwrap_or(0);
        if deg == 0 || deg > self.t || deg != l {
            return Err(FecError::Uncorrectable);
        }
        // Chien search with the terms kept as logarithms: term_k = c_k α^(-k p).
        let nn = gf.order() as i64;
        for k in 1..=deg {
            self.logs[k] = gf.log(self.c[k]).map(|v| v as i64).unwrap_or(-1);
        }
        let mut found = 0;
        for p in 0..self.n {
            let mut s = 1u16;
            for k in 1..=deg {
                let lg = self.logs[k];
                if lg >= 0 {
                    s ^= gf.exp(lg as usize);
                    let nl = lg - k as i64;
                    self.logs[k] = if nl < 0 { nl + nn } else { nl };
                }
            }
            if s == 0 {
                if found == deg {
                    return Err(FecError::Uncorrectable);
                }
                self.locs[found] = p as u32;
                found += 1;
                if found == deg {
                    break;
                }
            }
        }
        if found != deg {
            return Err(FecError::Uncorrectable);
        }
        for &p in &self.locs[..found] {
            word[self.n - 1 - p as usize] ^= 1;
        }
        if !self.enc.is_codeword(word) {
            for &p in &self.locs[..found] {
                word[self.n - 1 - p as usize] ^= 1;
            }
            return Err(FecError::Uncorrectable);
        }
        Ok(Decoded::corrected(found as u32))
    }
}

/// `bits(x) x^deg mod poly` for the low `count` bits of `v` (the highest first), bit by bit.
fn rem_bits(v: u64, count: u32, mp: &MinPoly) -> u32 {
    let mut r = 0u64;
    let top = 1u64 << mp.deg;
    for b in (0..count).rev() {
        let fb = ((r >> (mp.deg - 1)) & 1) ^ ((v >> b) & 1);
        r = (r << 1) & (top - 1);
        if fb == 1 {
            r ^= mp.poly as u64 & (top - 1);
        }
    }
    r as u32
}

/// `word(x) mod poly` for a word given as its first bits `lead` (one per byte, fewer than 8) and the rest in `bytes` (eight
/// bits each), the first bit the highest power: a byte at a time when the degree allows.
fn rem_word(lead: &[u8], bytes: &[u8], mp: &MinPoly) -> u32 {
    let d = mp.deg;
    let full = mp.poly as u64;
    let mask = (1u64 << d) - 1;
    let mut r = 0u64;
    let bitwise = |r: &mut u64, b: u8| {
        *r = (*r << 1) | (b & 1) as u64;
        if *r >> d & 1 != 0 {
            *r ^= full;
        }
    };
    for &b in lead {
        bitwise(&mut r, b);
    }
    if d < 8 {
        for &byte in bytes {
            for k in (0..8).rev() {
                bitwise(&mut r, byte >> k);
            }
        }
        return r as u32;
    }
    for &byte in bytes {
        let top = (r >> (d - 8)) as usize;
        r = ((r << 8) & mask) ^ mp.table[top] as u64 ^ byte as u64;
    }
    r as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_code_round_trip_and_limits() {
        // BCH(15,7) t = 2 over x^4 + x + 1: g = 0x1D1.
        let mut b = Bch::new(4, 0x13, 15, 2).unwrap();
        assert_eq!(b.generator(), &[0x1D1]);
        let mut w = [1u8, 0, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0];
        b.encode(&mut w).unwrap();
        let sent = w;
        w[0] ^= 1;
        w[14] ^= 1;
        b.decode(&mut w).unwrap();
        assert_eq!(w, sent);
        assert!(Bch::new(4, 0x13, 16, 2).is_err());
        assert!(Bch::new(4, 0x13, 15, 0).is_err());
        assert!(b.decode(&mut [0u8; 14]).is_err());
        let found = Bch::from_generator(15, &[0x1D1], 2).unwrap();
        assert_eq!(found.field().poly(), 0x13);
    }
}
