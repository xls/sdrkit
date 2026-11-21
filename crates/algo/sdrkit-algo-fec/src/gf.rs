//! Arithmetic in the finite field GF(2^m), `m` from 2 to 16, with any primitive polynomial.
//!
//! Elements are `u16` bit patterns in the polynomial basis (bit `j` is the coefficient of `α^j`). Multiplication and division go
//! through log and antilog tables built at construction.
//!
//! ```
//! use sdrkit_algo_fec::gf::Gf;
//! // GF(256) with p(x) = x^8 + x^4 + x^3 + x^2 + 1 (DVB, ATSC, DAB).
//! let f = Gf::new(8, 0x11D).unwrap();
//! assert_eq!(f.exp(8), 0x1D);
//! assert_eq!(f.mul(f.exp(200), f.exp(100)), f.exp(45));
//! assert_eq!(f.mul(0x53, f.inv(0x53).unwrap()), 1);
//! ```

use crate::error::{Param, Result};

/// A field GF(2^m).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gf {
    m: u8,
    poly: u32,
    /// `exp[i] = α^i` for `i` in `0 .. 2 (2^m − 1)` (doubled so a sum of two logs needs no reduction).
    exp: Vec<u16>,
    /// `log[x]` for `x` in `1 .. 2^m`; `log[0]` is unused.
    log: Vec<u16>,
}

impl Gf {
    /// The field with primitive polynomial `poly` (its top bit is `x^m`). An error when `m` is not 2 to 16, `poly` does not have
    /// degree `m`, or `α` does not generate every nonzero element (the polynomial is not primitive).
    pub fn new(m: u8, poly: u32) -> Result<Self> {
        if !(2..=16).contains(&m) {
            return Param::M.range(2, 16, m);
        }
        if poly >> m != 1 {
            return Param::Polynomial.conflict(Param::M);
        }
        let q = 1usize << m;
        let nn = q - 1;
        let mut exp = vec![0u16; 2 * nn + 1];
        let mut log = vec![0u16; q];
        let mut x = 1u32;
        for i in 0..nn {
            if i > 0 && x == 1 {
                return Param::Polynomial.degenerate();
            }
            exp[i] = x as u16;
            log[x as usize] = i as u16;
            x <<= 1;
            if x & (1 << m) != 0 {
                x ^= poly;
            }
        }
        if x != 1 {
            return Param::Polynomial.degenerate();
        }
        for i in nn..exp.len() {
            exp[i] = exp[i - nn];
        }
        Ok(Gf { m, poly, exp, log })
    }

    /// `m`.
    pub fn m(&self) -> u8 {
        self.m
    }

    /// The field polynomial.
    pub fn poly(&self) -> u32 {
        self.poly
    }

    /// `2^m − 1`, the order of the multiplicative group.
    pub fn order(&self) -> usize {
        (1usize << self.m) - 1
    }

    /// `α^i` (any `i`, reduced modulo `2^m − 1`).
    #[inline]
    pub fn exp(&self, i: usize) -> u16 {
        self.exp[i % self.order()]
    }

    /// The discrete logarithm of `x` (`None` for 0).
    #[inline]
    pub fn log(&self, x: u16) -> Option<usize> {
        if x == 0 || x as usize > self.order() {
            None
        } else {
            Some(self.log[x as usize] as usize)
        }
    }

    #[inline]
    fn mask(&self, a: u16) -> usize {
        a as usize & self.order()
    }

    /// `a × b` (bits of `a` and `b` above `m` are ignored).
    #[inline]
    pub fn mul(&self, a: u16, b: u16) -> u16 {
        let (a, b) = (self.mask(a), self.mask(b));
        if a == 0 || b == 0 {
            0
        } else {
            self.exp[self.log[a] as usize + self.log[b] as usize]
        }
    }

    /// `a × α^e` (a fast form for a known power).
    #[inline]
    pub fn mul_exp(&self, a: u16, e: usize) -> u16 {
        let a = self.mask(a);
        if a == 0 {
            0
        } else {
            self.exp[self.log[a] as usize + e % self.order()]
        }
    }

    /// `a / b`; `None` when `b` is 0.
    #[inline]
    pub fn div(&self, a: u16, b: u16) -> Option<u16> {
        let (a, b) = (self.mask(a), self.mask(b));
        if b == 0 {
            return None;
        }
        if a == 0 {
            return Some(0);
        }
        let nn = self.order();
        Some(self.exp[self.log[a] as usize + nn - self.log[b] as usize])
    }

    /// `1 / a`; `None` for 0.
    #[inline]
    pub fn inv(&self, a: u16) -> Option<u16> {
        self.div(1, a)
    }

    /// `a^e`.
    pub fn pow(&self, a: u16, e: usize) -> u16 {
        let a = self.mask(a);
        if e == 0 {
            return 1;
        }
        if a == 0 {
            return 0;
        }
        let l = self.log[a] as usize;
        self.exp[(l * (e % self.order())) % self.order()]
    }

    /// The minimal polynomial over GF(2) of `α^i`, as a bit mask (bit `j` = coefficient of `x^j`).
    pub fn minimal_poly(&self, i: usize) -> u64 {
        let nn = self.order();
        // The conjugates α^(i 2^k).
        let mut conj = Vec::with_capacity(self.m as usize);
        let mut e = i % nn;
        loop {
            conj.push(e);
            e = (e * 2) % nn;
            if e == i % nn {
                break;
            }
        }
        // Product of (x + α^c) over the conjugates, coefficients in the field (they end up 0 or 1).
        let mut p: Vec<u16> = vec![1];
        for &c in &conj {
            let r = self.exp(c);
            let mut q = vec![0u16; p.len() + 1];
            for (k, &a) in p.iter().enumerate() {
                q[k + 1] ^= a;
                q[k] ^= self.mul(a, r);
            }
            p = q;
        }
        p.iter()
            .enumerate()
            .fold(0u64, |acc, (k, &a)| acc | ((a as u64 & 1) << k))
    }
}

/// Multiplies two polynomials over GF(2) given as bit masks (`u128`, bit `j` = coefficient of `x^j`). `None` on overflow.
pub fn gf2_poly_mul(a: u128, b: u128) -> Option<u128> {
    let mut r = 0u128;
    let deg_a = 127 - a.leading_zeros() as i32;
    let deg_b = 127 - b.leading_zeros() as i32;
    if a != 0 && b != 0 && deg_a + deg_b > 127 {
        return None;
    }
    for j in 0..128 {
        if b >> j & 1 != 0 {
            r ^= a << j;
        }
    }
    Some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_build_and_refuse_non_primitive_polynomials() {
        for (m, p) in [
            (2, 0x7),
            (3, 0xB),
            (4, 0x13),
            (6, 0x43),
            (7, 0x89),
            (8, 0x11D),
            (8, 0x187),
            (14, 0x402B),
            (16, 0x1002D),
        ] {
            let f = Gf::new(m, p).unwrap();
            assert_eq!(f.exp(f.order()), 1);
            for x in 1..=f.order().min(4000) as u16 {
                assert_eq!(f.mul(x, f.inv(x).unwrap()), 1);
            }
        }
        // x^8 + x^4 + x^3 + x + 1 (the AES polynomial) is irreducible but not primitive.
        assert!(Gf::new(8, 0x11B).is_err());
        assert!(Gf::new(8, 0x1D).is_err());
        assert!(Gf::new(17, 0x20009).is_err());
    }

    #[test]
    fn minimal_polynomials() {
        let f = Gf::new(4, 0x13).unwrap();
        assert_eq!(f.minimal_poly(1), 0x13);
        // x^4 + x^3 + x^2 + x + 1 for α^3 in GF(16).
        assert_eq!(f.minimal_poly(3), 0x1F);
        assert_eq!(f.minimal_poly(5), 0x7);
        assert_eq!(
            gf2_poly_mul(0x13, 0x1F),
            Some(0x13 ^ (0x13 << 1) ^ (0x13 << 2) ^ (0x13 << 3) ^ (0x13 << 4))
        );
        assert!(gf2_poly_mul(1 << 100, 1 << 30).is_none());
        assert_eq!(f.div(5, 0), None);
        assert_eq!(f.log(0), None);
        assert_eq!(f.pow(f.exp(3), 5), f.exp(15));
    }
}
