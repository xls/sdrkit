//! Vector kernels for Reed-Solomon syndromes and the Chien search over GF(2^m), `m ≤ 8`: NEON on `aarch64`, SSSE3 (detected at
//! run time) on `x86_64`. Both multiply 16 symbols by one constant with two 16-entry nibble table lookups.
//!
//! `unsafe` is limited to the intrinsics. Every load and store is of a 16-byte array held on the stack or a slice whose length
//! is checked first.

#![allow(unsafe_code)]

use super::RsBackend;
use crate::gf::Gf;
use sdrkit_core::backend::Backend;

/// Largest locator degree the vector Chien search handles (beyond, the scalar one runs).
const MAX_DEG: usize = 64;

/// The vector backend: NEON on `aarch64`, SSSE3 on `x86_64`, for codes with `m ≤ 8`.
pub(super) struct Vector;

impl Backend for Vector {
    fn name(&self) -> &'static str {
        if cfg!(target_arch = "aarch64") {
            "neon"
        } else {
            "ssse3"
        }
    }
    fn available(&self) -> bool {
        available()
    }
}

impl RsBackend for Vector {
    fn prepare(&self, gf: &Gf, roots: &[u16]) -> Option<Vec<u8>> {
        if gf.m() > 8 {
            return None;
        }
        Some(
            roots
                .iter()
                .flat_map(|&r| table(gf, gf.pow(r, 16)))
                .collect(),
        )
    }

    fn syndromes(&self, tables: &[u8], gf: &Gf, word: &[u16], roots: &[u16], synd: &mut [u16]) {
        syndromes(gf, word, roots, tables.as_chunks::<32>().0, synd);
    }

    fn chien(
        &self,
        _tables: &[u8],
        gf: &Gf,
        lambda: &[u16],
        prim: usize,
        n: usize,
        locs: &mut [u16],
    ) -> usize {
        chien(gf, lambda, prim, n, locs)
    }
}

/// Whether a vector kernel runs on this machine.
fn available() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        true
    }
    #[cfg(target_arch = "x86_64")]
    {
        std::is_x86_feature_detected!("ssse3")
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        false
    }
}

/// Nibble table for `y` (as `rs::nibble_table`).
fn table(gf: &Gf, y: u16) -> [u8; 32] {
    let mut t = [0u8; 32];
    for x in 0..16u16 {
        t[x as usize] = gf.mul(y, x) as u8;
        t[16 + x as usize] = gf.mul(y, x << 4) as u8;
    }
    t
}

/// 16 lanes multiplied by the constant whose nibble table is `t`.
#[inline]
fn mul16(t: &[u8; 32], v: [u8; 16]) -> [u8; 16] {
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is baseline on aarch64; loads and stores are of fixed-size arrays.
        unsafe {
            use core::arch::aarch64::*;
            let lo = vld1q_u8(t.as_ptr());
            let hi = vld1q_u8(t.as_ptr().add(16));
            let x = vld1q_u8(v.as_ptr());
            let r = veorq_u8(
                vqtbl1q_u8(lo, vandq_u8(x, vdupq_n_u8(15))),
                vqtbl1q_u8(hi, vshrq_n_u8(x, 4)),
            );
            let mut o = [0u8; 16];
            vst1q_u8(o.as_mut_ptr(), r);
            o
        }
    }
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: only reached when `available()` found SSSE3; loads and stores are of fixed-size arrays.
        unsafe { mul16_ssse3(t, v) }
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        let mut o = [0u8; 16];
        for i in 0..16 {
            o[i] = t[(v[i] & 15) as usize] ^ t[16 + (v[i] >> 4) as usize];
        }
        o
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn mul16_ssse3(t: &[u8; 32], v: [u8; 16]) -> [u8; 16] {
    use core::arch::x86_64::*;
    let lo = _mm_loadu_si128(t.as_ptr() as *const __m128i);
    let hi = _mm_loadu_si128(t.as_ptr().add(16) as *const __m128i);
    let x = _mm_loadu_si128(v.as_ptr() as *const __m128i);
    let m = _mm_set1_epi8(15);
    let l = _mm_shuffle_epi8(lo, _mm_and_si128(x, m));
    let h = _mm_shuffle_epi8(hi, _mm_and_si128(_mm_srli_epi16(x, 4), m));
    let mut o = [0u8; 16];
    _mm_storeu_si128(o.as_mut_ptr() as *mut __m128i, _mm_xor_si128(l, h));
    o
}

#[inline]
fn xor16(a: [u8; 16], b: &[u8]) -> [u8; 16] {
    let mut o = a;
    for (x, y) in o.iter_mut().zip(b) {
        *x ^= y;
    }
    o
}

/// Syndromes of `word` (coefficients by power, at most 255 symbols of `m ≤ 8` bits) for each root, using the per-root nibble
/// tables of `root^16`.
fn syndromes(gf: &Gf, word: &[u16], roots: &[u16], nib16: &[[u8; 32]], synd: &mut [u16]) {
    let mut padded = [0u8; 256];
    for (p, &d) in padded.iter_mut().zip(word) {
        *p = d as u8;
    }
    let chunks = word.len().div_ceil(16);
    for ((s, &r), t) in synd.iter_mut().zip(roots).zip(nib16) {
        let mut acc = [0u8; 16];
        for q in (0..chunks).rev() {
            acc = xor16(mul16(t, acc), &padded[16 * q..16 * q + 16]);
        }
        // S = Σ_l r^l acc[l], by Horner over l.
        let lr = gf.log(r).unwrap_or(0);
        let mut v = 0u16;
        for l in (0..16).rev() {
            v = gf.mul_exp(v, lr) ^ acc[l] as u16;
        }
        *s = v;
    }
}

/// Chien search: the powers `p < n` with `Λ(γ^−p) = 0` into `locs`; returns how many (more than `locs.len()` means too many).
fn chien(gf: &Gf, lambda: &[u16], prim: usize, n: usize, locs: &mut [u16]) -> usize {
    let deg = lambda.len() - 1;
    if deg > MAX_DEG {
        return super::chien_scalar(gf, lambda, prim, n, locs);
    }
    let nn = gf.order();
    let mut terms = [[0u8; 16]; MAX_DEG + 1];
    let mut tabs = [[0u8; 32]; MAX_DEG + 1];
    for (k, &l) in lambda.iter().enumerate() {
        for (lane, t) in terms[k].iter_mut().enumerate() {
            // λ_k γ^(-k lane)
            let e = (nn - (prim * k * lane) % nn) % nn;
            *t = gf.mul_exp(l, e) as u8;
        }
        let step = (nn - (prim * k * 16) % nn) % nn;
        tabs[k] = table(gf, gf.exp(step));
    }
    let mut found = 0;
    for b in 0..n.div_ceil(16) {
        let mut sum = [0u8; 16];
        for k in 0..=deg {
            sum = xor16(sum, &terms[k]);
            terms[k] = mul16(&tabs[k], terms[k]);
        }
        for (lane, &v) in sum.iter().enumerate() {
            let p = 16 * b + lane;
            if v == 0 && p < n {
                if found == locs.len() {
                    return found + 1;
                }
                locs[found] = p as u16;
                found += 1;
            }
        }
    }
    found
}
