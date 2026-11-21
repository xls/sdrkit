//! Vector forms of the 16-bit Viterbi butterfly kernel: NEON on `aarch64`, SSE2 and AVX2 on `x86_64`. Each does the arithmetic
//! of the plain backend in `viterbi.rs` (wrapping 16-bit adds; the odd predecessor wins only when strictly better), so the
//! decisions are bit for bit the same. `unsafe` is limited to the intrinsics in this file: every load and store index is below
//! `2 * half <= 256` within fixed-size arrays, or inside the decision slice whose bounds are checked before the call.

#![allow(unsafe_code)]
#![allow(dead_code)]

#[allow(unused_imports)]
use super::viterbi::{store_bits, ButterflyState, MAX_HALF};

#[cfg(target_arch = "aarch64")]
pub(super) fn run_neon16(
    f: &mut ButterflyState,
    llr: &[i8],
    dec: &mut [u64],
    words: usize,
    t0: usize,
    mask: usize,
) {
    if f.half == 32 {
        let steps = llr.len() / f.n;
        // Every decision index the kernel writes is below the slice length.
        if (0..steps).all(|i| ((t0 + i) & mask) < dec.len()) || steps == 0 {
            // SAFETY: NEON is baseline; indices checked above; metric loads and stores are within `f.bufs`.
            unsafe {
                if f.n == 2 {
                    run_neon_k7_n2(f, llr, dec, t0, mask);
                } else {
                    run_neon_k7(f, llr, dec, t0, mask);
                }
            }
            return;
        }
    }
    let n = f.n;
    for (i, l) in llr.chunks_exact(n).enumerate() {
        let t = (t0 + i) & mask;
        let d = &mut dec[t * words..(t + 1) * words];
        // SAFETY: NEON is part of the aarch64 baseline; `step_neon` only indexes inside `f`'s fixed arrays (see the module note).
        unsafe { step_neon(f, l, d) };
    }
}

#[cfg(target_arch = "aarch64")]
#[inline]
#[target_feature(enable = "neon")]
unsafe fn step_neon(f: &mut ButterflyState, l: &[i8], d: &mut [u64]) {
    use core::arch::aarch64::*;
    let half = f.half;
    let (src, dst) = if f.cur == 0 {
        let (a, b) = f.bufs.split_at_mut(1);
        (&a[0], &mut b[0])
    } else {
        let (a, b) = f.bufs.split_at_mut(1);
        (&b[0], &mut a[0])
    };
    let zero = vdupq_n_s16(0);
    let weights: [u16; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
    let bw = vld1q_u16(weights.as_ptr());
    let mut lv = [zero; 8];
    for (k, &v) in l.iter().enumerate() {
        lv[k] = vdupq_n_s16(v as i16);
    }
    let n = l.len();
    let mut bits = [0u8; 2 * MAX_HALF / 8];
    let sp = src.as_ptr();
    let dp = dst.as_mut_ptr();
    // One butterfly group: 8 butterflies starting at j.
    let group = |j: usize, bits: &mut [u8; 2 * MAX_HALF / 8]| -> (int16x8_t, int16x8_t) {
        let e = vld1q_s16(sp.add(j));
        let o = vld1q_s16(sp.add(half + j));
        let mut bm = vmulq_s16(vld1q_s16(f.sgn[0].as_ptr().add(j)), lv[0]);
        for k in 1..n {
            bm = vmlaq_s16(bm, vld1q_s16(f.sgn[k].as_ptr().add(j)), lv[k]);
        }
        let a0 = vaddq_s16(e, bm);
        let a1 = vsubq_s16(o, bm);
        let b0 = vsubq_s16(e, bm);
        let b1 = vaddq_s16(o, bm);
        let m0 = vcgtq_s16(vsubq_s16(a1, a0), zero);
        let m1 = vcgtq_s16(vsubq_s16(b1, b0), zero);
        bits[j / 8] = vaddvq_u16(vandq_u16(m0, bw)) as u8;
        bits[(half + j) / 8] = vaddvq_u16(vandq_u16(m1, bw)) as u8;
        (vbslq_s16(m0, a1, a0), vbslq_s16(m1, b1, b0))
    };
    if half == 8 {
        let (lo, hi) = group(0, &mut bits);
        vst1q_s16(dp, vuzp1q_s16(lo, hi));
        vst1q_s16(dp.add(8), vuzp2q_s16(lo, hi));
    } else {
        let q = half / 2;
        for p in 0..half / 16 {
            let (lo0, hi0) = group(16 * p, &mut bits);
            let (lo1, hi1) = group(16 * p + 8, &mut bits);
            vst1q_s16(dp.add(8 * p), vuzp1q_s16(lo0, lo1));
            vst1q_s16(dp.add(half + 8 * p), vuzp2q_s16(lo0, lo1));
            vst1q_s16(dp.add(q + 8 * p), vuzp1q_s16(hi0, hi1));
            vst1q_s16(dp.add(half + q + 8 * p), vuzp2q_s16(hi0, hi1));
        }
    }
    store_bits(&bits[..2 * half / 8], d);
    f.cur ^= 1;
}

/// The K = 7 kernel with the 64 metrics held in eight registers across steps, and the 64 decisions of a step packed with
/// pairwise adds into one `u64`.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn run_neon_k7(f: &mut ButterflyState, llr: &[i8], dec: &mut [u64], t0: usize, mask: usize) {
    use core::arch::aarch64::*;
    let n = f.n;
    let src = &f.bufs[f.cur];
    let sp = src.as_ptr();
    // ev = states 0, 2, .., 62 in e0..e3; od = states 1, 3, .., 63 in o0..o3.
    let mut e = [
        vld1q_s16(sp),
        vld1q_s16(sp.add(8)),
        vld1q_s16(sp.add(16)),
        vld1q_s16(sp.add(24)),
    ];
    let mut o = [
        vld1q_s16(sp.add(32)),
        vld1q_s16(sp.add(40)),
        vld1q_s16(sp.add(48)),
        vld1q_s16(sp.add(56)),
    ];
    let zero = vdupq_n_s16(0);
    let wts: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];
    let w = vld1q_u8(wts.as_ptr());
    let sg = &f.sgn;
    for (i, l) in llr.chunks_exact(n).enumerate() {
        let mut lv = [zero; 8];
        for (k, &v) in l.iter().enumerate() {
            lv[k] = vdupq_n_s16(v as i16);
        }
        let mut lo = [zero; 4];
        let mut hi = [zero; 4];
        let mut m0 = [vdupq_n_u16(0); 4];
        let mut m1 = [vdupq_n_u16(0); 4];
        for g in 0..4 {
            let mut bm = vmulq_s16(vld1q_s16(sg[0].as_ptr().add(8 * g)), lv[0]);
            for k in 1..n {
                bm = vmlaq_s16(bm, vld1q_s16(sg[k].as_ptr().add(8 * g)), lv[k]);
            }
            let a0 = vaddq_s16(e[g], bm);
            let a1 = vsubq_s16(o[g], bm);
            let b0 = vsubq_s16(e[g], bm);
            let b1 = vaddq_s16(o[g], bm);
            // The driver renormalises after every batch, so the metrics never wrap here and a plain compare and maximum
            // give the scalar kernel's modular decisions.
            m0[g] = vcgtq_s16(a1, a0);
            m1[g] = vcgtq_s16(b1, b0);
            lo[g] = vmaxq_s16(a0, a1);
            hi[g] = vmaxq_s16(b0, b1);
        }
        e = [
            vuzp1q_s16(lo[0], lo[1]),
            vuzp1q_s16(lo[2], lo[3]),
            vuzp1q_s16(hi[0], hi[1]),
            vuzp1q_s16(hi[2], hi[3]),
        ];
        o = [
            vuzp2q_s16(lo[0], lo[1]),
            vuzp2q_s16(lo[2], lo[3]),
            vuzp2q_s16(hi[0], hi[1]),
            vuzp2q_s16(hi[2], hi[3]),
        ];
        // Decisions: states 0..31 from m0, 32..63 from m1; narrow, weight and add pairwise down to 8 bytes.
        let x0 = vandq_u8(vcombine_u8(vmovn_u16(m0[0]), vmovn_u16(m0[1])), w);
        let x1 = vandq_u8(vcombine_u8(vmovn_u16(m0[2]), vmovn_u16(m0[3])), w);
        let x2 = vandq_u8(vcombine_u8(vmovn_u16(m1[0]), vmovn_u16(m1[1])), w);
        let x3 = vandq_u8(vcombine_u8(vmovn_u16(m1[2]), vmovn_u16(m1[3])), w);
        let p = vpaddq_u8(vpaddq_u8(x0, x1), vpaddq_u8(x2, x3));
        let p = vpaddq_u8(p, p);
        let t = (t0 + i) & mask;
        *dec.get_unchecked_mut(t) = vgetq_lane_u64(vreinterpretq_u64_u8(p), 0);
    }
    let dst = &mut f.bufs[f.cur];
    let dp = dst.as_mut_ptr();
    for g in 0..4 {
        vst1q_s16(dp.add(8 * g), e[g]);
        vst1q_s16(dp.add(32 + 8 * g), o[g]);
    }
}

/// [`run_neon_k7`] for two outputs per step, with the signs of the branch labels held in registers.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn run_neon_k7_n2(
    f: &mut ButterflyState,
    llr: &[i8],
    dec: &mut [u64],
    t0: usize,
    mask: usize,
) {
    use core::arch::aarch64::*;
    let sp = f.bufs[f.cur].as_ptr();
    let (mut e0, mut e1, mut e2, mut e3) = (
        vld1q_s16(sp),
        vld1q_s16(sp.add(8)),
        vld1q_s16(sp.add(16)),
        vld1q_s16(sp.add(24)),
    );
    let (mut o0, mut o1, mut o2, mut o3) = (
        vld1q_s16(sp.add(32)),
        vld1q_s16(sp.add(40)),
        vld1q_s16(sp.add(48)),
        vld1q_s16(sp.add(56)),
    );
    let wts: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];
    let w = vld1q_u8(wts.as_ptr());
    let (sa, sb) = (f.sgn[0].as_ptr(), f.sgn[1].as_ptr());
    let (a0, a1, a2, a3) = (
        vld1q_s16(sa),
        vld1q_s16(sa.add(8)),
        vld1q_s16(sa.add(16)),
        vld1q_s16(sa.add(24)),
    );
    let (b0, b1, b2, b3) = (
        vld1q_s16(sb),
        vld1q_s16(sb.add(8)),
        vld1q_s16(sb.add(16)),
        vld1q_s16(sb.add(24)),
    );
    // One group of eight butterflies: the survivors of the low and of the high states and their decisions.
    macro_rules! group {
        ($e:expr, $o:expr, $sa:expr, $sb:expr, $la:expr, $lb:expr) => {{
            let bm = vmlaq_s16(vmulq_s16($sa, $la), $sb, $lb);
            let (p0, p1) = (vaddq_s16($e, bm), vsubq_s16($o, bm));
            let (q0, q1) = (vsubq_s16($e, bm), vaddq_s16($o, bm));
            (
                vmaxq_s16(p0, p1),
                vmaxq_s16(q0, q1),
                vcgtq_s16(p1, p0),
                vcgtq_s16(q1, q0),
            )
        }};
    }
    for (i, l) in llr.as_chunks::<2>().0.iter().enumerate() {
        let (la, lb) = (vdupq_n_s16(l[0] as i16), vdupq_n_s16(l[1] as i16));
        let (lo0, hi0, m00, m10) = group!(e0, o0, a0, b0, la, lb);
        let (lo1, hi1, m01, m11) = group!(e1, o1, a1, b1, la, lb);
        let (lo2, hi2, m02, m12) = group!(e2, o2, a2, b2, la, lb);
        let (lo3, hi3, m03, m13) = group!(e3, o3, a3, b3, la, lb);
        e0 = vuzp1q_s16(lo0, lo1);
        e1 = vuzp1q_s16(lo2, lo3);
        e2 = vuzp1q_s16(hi0, hi1);
        e3 = vuzp1q_s16(hi2, hi3);
        o0 = vuzp2q_s16(lo0, lo1);
        o1 = vuzp2q_s16(lo2, lo3);
        o2 = vuzp2q_s16(hi0, hi1);
        o3 = vuzp2q_s16(hi2, hi3);
        // Decisions: states 0..31 from the low survivors, 32..63 from the high ones, one bit each.
        let x0 = vandq_u8(
            vuzp1q_u8(vreinterpretq_u8_u16(m00), vreinterpretq_u8_u16(m01)),
            w,
        );
        let x1 = vandq_u8(
            vuzp1q_u8(vreinterpretq_u8_u16(m02), vreinterpretq_u8_u16(m03)),
            w,
        );
        let x2 = vandq_u8(
            vuzp1q_u8(vreinterpretq_u8_u16(m10), vreinterpretq_u8_u16(m11)),
            w,
        );
        let x3 = vandq_u8(
            vuzp1q_u8(vreinterpretq_u8_u16(m12), vreinterpretq_u8_u16(m13)),
            w,
        );
        let p = vpaddq_u8(vpaddq_u8(x0, x1), vpaddq_u8(x2, x3));
        let p = vpaddq_u8(p, p);
        let t = (t0 + i) & mask;
        *dec.get_unchecked_mut(t) = vgetq_lane_u64(vreinterpretq_u64_u8(p), 0);
    }
    let dp = f.bufs[f.cur].as_mut_ptr();
    vst1q_s16(dp, e0);
    vst1q_s16(dp.add(8), e1);
    vst1q_s16(dp.add(16), e2);
    vst1q_s16(dp.add(24), e3);
    vst1q_s16(dp.add(32), o0);
    vst1q_s16(dp.add(40), o1);
    vst1q_s16(dp.add(48), o2);
    vst1q_s16(dp.add(56), o3);
}

#[cfg(target_arch = "x86_64")]
pub(super) fn run_sse2_16(
    f: &mut ButterflyState,
    llr: &[i8],
    dec: &mut [u64],
    words: usize,
    t0: usize,
    mask: usize,
) {
    let n = f.n;
    for (i, l) in llr.chunks_exact(n).enumerate() {
        let t = (t0 + i) & mask;
        let d = &mut dec[t * words..(t + 1) * words];
        // SAFETY: SSE2 is part of the x86_64 baseline; indexes stay inside `f`'s fixed arrays.
        unsafe { step_sse2(f, l, d) };
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn step_sse2(f: &mut ButterflyState, l: &[i8], d: &mut [u64]) {
    use core::arch::x86_64::*;
    let half = f.half;
    let (src, dst) = if f.cur == 0 {
        let (a, b) = f.bufs.split_at_mut(1);
        (&a[0], &mut b[0])
    } else {
        let (a, b) = f.bufs.split_at_mut(1);
        (&b[0], &mut a[0])
    };
    let zero = _mm_setzero_si128();
    let mut lv = [zero; 8];
    for (k, &v) in l.iter().enumerate() {
        lv[k] = _mm_set1_epi16(v as i16);
    }
    let n = l.len();
    let mut bits = [0u8; 2 * MAX_HALF / 8];
    let sp = src.as_ptr();
    let dp = dst.as_mut_ptr();
    let ld = |p: *const i16| _mm_loadu_si128(p as *const __m128i);
    let sel = |m: __m128i, a: __m128i, b: __m128i| {
        _mm_or_si128(_mm_and_si128(m, a), _mm_andnot_si128(m, b))
    };
    let even = |x: __m128i, y: __m128i| {
        _mm_packs_epi32(
            _mm_srai_epi32(_mm_slli_epi32(x, 16), 16),
            _mm_srai_epi32(_mm_slli_epi32(y, 16), 16),
        )
    };
    let odd =
        |x: __m128i, y: __m128i| _mm_packs_epi32(_mm_srai_epi32(x, 16), _mm_srai_epi32(y, 16));
    let group = |j: usize, bits: &mut [u8; 2 * MAX_HALF / 8]| -> (__m128i, __m128i) {
        let e = ld(sp.add(j));
        let o = ld(sp.add(half + j));
        let mut bm = _mm_mullo_epi16(ld(f.sgn[0].as_ptr().add(j)), lv[0]);
        for k in 1..n {
            bm = _mm_add_epi16(bm, _mm_mullo_epi16(ld(f.sgn[k].as_ptr().add(j)), lv[k]));
        }
        let a0 = _mm_add_epi16(e, bm);
        let a1 = _mm_sub_epi16(o, bm);
        let b0 = _mm_sub_epi16(e, bm);
        let b1 = _mm_add_epi16(o, bm);
        let m0 = _mm_cmpgt_epi16(_mm_sub_epi16(a1, a0), zero);
        let m1 = _mm_cmpgt_epi16(_mm_sub_epi16(b1, b0), zero);
        bits[j / 8] = _mm_movemask_epi8(_mm_packs_epi16(m0, m0)) as u8;
        bits[(half + j) / 8] = _mm_movemask_epi8(_mm_packs_epi16(m1, m1)) as u8;
        (sel(m0, a1, a0), sel(m1, b1, b0))
    };
    let st = |p: *mut i16, v: __m128i| _mm_storeu_si128(p as *mut __m128i, v);
    if half == 8 {
        let (lo, hi) = group(0, &mut bits);
        st(dp, even(lo, hi));
        st(dp.add(8), odd(lo, hi));
    } else {
        let q = half / 2;
        for p in 0..half / 16 {
            let (lo0, hi0) = group(16 * p, &mut bits);
            let (lo1, hi1) = group(16 * p + 8, &mut bits);
            st(dp.add(8 * p), even(lo0, lo1));
            st(dp.add(half + 8 * p), odd(lo0, lo1));
            st(dp.add(q + 8 * p), even(hi0, hi1));
            st(dp.add(half + q + 8 * p), odd(hi0, hi1));
        }
    }
    store_bits(&bits[..2 * half / 8], d);
    f.cur ^= 1;
}

#[cfg(target_arch = "x86_64")]
pub(super) fn run_avx2_16(
    f: &mut ButterflyState,
    llr: &[i8],
    dec: &mut [u64],
    words: usize,
    t0: usize,
    mask: usize,
) {
    let n = f.n;
    for (i, l) in llr.chunks_exact(n).enumerate() {
        let t = (t0 + i) & mask;
        let d = &mut dec[t * words..(t + 1) * words];
        // SAFETY: the AVX2 kernel is only selected after `is_x86_feature_detected!("avx2")`; indexes stay inside `f`'s arrays.
        unsafe { step_avx2(f, l, d) };
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn step_avx2(f: &mut ButterflyState, l: &[i8], d: &mut [u64]) {
    use core::arch::x86_64::*;
    let half = f.half; // at least 16
    let (src, dst) = if f.cur == 0 {
        let (a, b) = f.bufs.split_at_mut(1);
        (&a[0], &mut b[0])
    } else {
        let (a, b) = f.bufs.split_at_mut(1);
        (&b[0], &mut a[0])
    };
    let zero = _mm256_setzero_si256();
    let mut lv = [zero; 8];
    for (k, &v) in l.iter().enumerate() {
        lv[k] = _mm256_set1_epi16(v as i16);
    }
    let n = l.len();
    let mut bits = [0u8; 2 * MAX_HALF / 8];
    let sp = src.as_ptr();
    let dp = dst.as_mut_ptr();
    let ld = |p: *const i16| _mm256_loadu_si256(p as *const __m256i);
    let st = |p: *mut i16, v: __m256i| _mm256_storeu_si256(p as *mut __m256i, v);
    let even = |x: __m256i, y: __m256i| {
        let p = _mm256_packs_epi32(
            _mm256_srai_epi32(_mm256_slli_epi32(x, 16), 16),
            _mm256_srai_epi32(_mm256_slli_epi32(y, 16), 16),
        );
        _mm256_permute4x64_epi64(p, 0xD8)
    };
    let odd = |x: __m256i, y: __m256i| {
        let p = _mm256_packs_epi32(_mm256_srai_epi32(x, 16), _mm256_srai_epi32(y, 16));
        _mm256_permute4x64_epi64(p, 0xD8)
    };
    let mbits = |m: __m256i| -> u16 {
        let mm = _mm256_movemask_epi8(_mm256_packs_epi16(m, m)) as u32;
        ((mm & 0xFF) | ((mm >> 8) & 0xFF00)) as u16
    };
    let group = |j: usize, bits: &mut [u8; 2 * MAX_HALF / 8]| -> (__m256i, __m256i) {
        let e = ld(sp.add(j));
        let o = ld(sp.add(half + j));
        let mut bm = _mm256_mullo_epi16(ld(f.sgn[0].as_ptr().add(j)), lv[0]);
        for k in 1..n {
            bm = _mm256_add_epi16(bm, _mm256_mullo_epi16(ld(f.sgn[k].as_ptr().add(j)), lv[k]));
        }
        let a0 = _mm256_add_epi16(e, bm);
        let a1 = _mm256_sub_epi16(o, bm);
        let b0 = _mm256_sub_epi16(e, bm);
        let b1 = _mm256_add_epi16(o, bm);
        let m0 = _mm256_cmpgt_epi16(_mm256_sub_epi16(a1, a0), zero);
        let m1 = _mm256_cmpgt_epi16(_mm256_sub_epi16(b1, b0), zero);
        let w0 = mbits(m0).to_le_bytes();
        let w1 = mbits(m1).to_le_bytes();
        bits[j / 8] = w0[0];
        bits[j / 8 + 1] = w0[1];
        bits[(half + j) / 8] = w1[0];
        bits[(half + j) / 8 + 1] = w1[1];
        (
            _mm256_blendv_epi8(a0, a1, m0),
            _mm256_blendv_epi8(b0, b1, m1),
        )
    };
    if half == 16 {
        let (lo, hi) = group(0, &mut bits);
        st(dp, even(lo, hi));
        st(dp.add(16), odd(lo, hi));
    } else {
        let q = half / 2;
        for p in 0..half / 32 {
            let (lo0, hi0) = group(32 * p, &mut bits);
            let (lo1, hi1) = group(32 * p + 16, &mut bits);
            st(dp.add(16 * p), even(lo0, lo1));
            st(dp.add(half + 16 * p), odd(lo0, lo1));
            st(dp.add(q + 16 * p), even(hi0, hi1));
            st(dp.add(half + q + 16 * p), odd(hi0, hi1));
        }
    }
    store_bits(&bits[..2 * half / 8], d);
    f.cur ^= 1;
}
