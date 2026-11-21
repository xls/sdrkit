//! The inverse DCT and the motion compensation with the vector instructions of 64-bit ARM. Every function gives the
//! same bits as the portable one of the same name.

#![allow(unsafe_code)]

use crate::idct::{self, C1, C2, C3, C4, C5, C6, C7, ROUND_1, ROUND_2};
use crate::mc::{self, Source};
use core::arch::aarch64::*;

/// See [`mc::predict`].
pub(crate) fn predict(
    s: &Source,
    dst: &mut [u8],
    dstride: usize,
    w: usize,
    h: usize,
    average: bool,
) {
    let (dx, dy) = (s.half_x as usize, s.half_y as usize * s.stride);
    let fits = h > 0
        && (w == 16 || w == 8)
        && s.src.len() >= (h - 1) * s.stride + dy + dx + w
        && dst.len() >= (h - 1) * dstride + w;
    if !fits {
        return mc::predict(s, dst, dstride, w, h, average);
    }
    let (src, out) = (s.src.as_ptr(), dst.as_mut_ptr());
    for y in 0..h {
        // SAFETY: `fits` says that the `w` samples at `p`, `p + dx`, `p + dy` and `p + dy + dx` are inside `s.src`
        // and the `w` samples at `o` are inside `dst`, for every row `y` below `h`.
        unsafe {
            let (p, o) = (src.add(y * s.stride), out.add(y * dstride));
            if w == 16 {
                let a = vld1q_u8(p);
                let mut r = match (s.half_x, s.half_y) {
                    (false, false) => a,
                    (true, false) => vrhaddq_u8(a, vld1q_u8(p.add(1))),
                    (false, true) => vrhaddq_u8(a, vld1q_u8(p.add(dy))),
                    (true, true) => {
                        let (b, c, d) = (
                            vld1q_u8(p.add(1)),
                            vld1q_u8(p.add(dy)),
                            vld1q_u8(p.add(dy + 1)),
                        );
                        let low = vaddq_u16(
                            vaddl_u8(vget_low_u8(a), vget_low_u8(b)),
                            vaddl_u8(vget_low_u8(c), vget_low_u8(d)),
                        );
                        let high = vaddq_u16(vaddl_high_u8(a, b), vaddl_high_u8(c, d));
                        vcombine_u8(vrshrn_n_u16::<2>(low), vrshrn_n_u16::<2>(high))
                    }
                };
                if average {
                    r = vrhaddq_u8(vld1q_u8(o), r);
                }
                vst1q_u8(o, r);
            } else {
                let a = vld1_u8(p);
                let mut r = match (s.half_x, s.half_y) {
                    (false, false) => a,
                    (true, false) => vrhadd_u8(a, vld1_u8(p.add(1))),
                    (false, true) => vrhadd_u8(a, vld1_u8(p.add(dy))),
                    (true, true) => {
                        let (b, c, d) = (
                            vld1_u8(p.add(1)),
                            vld1_u8(p.add(dy)),
                            vld1_u8(p.add(dy + 1)),
                        );
                        vrshrn_n_u16::<2>(vaddq_u16(vaddl_u8(a, b), vaddl_u8(c, d)))
                    }
                };
                if average {
                    r = vrhadd_u8(vld1_u8(o), r);
                }
                vst1_u8(o, r);
            }
        }
    }
}

/// One transform of 8 values in each of the 4 lanes: the outputs before the shift.
#[inline(always)]
fn pass(x: [int32x4_t; 8]) -> [int32x4_t; 8] {
    // SAFETY: these instructions are part of every 64-bit ARM processor and touch no memory.
    unsafe {
        let e0 = vmulq_n_s32(vaddq_s32(x[0], x[4]), C4);
        let e1 = vmulq_n_s32(vsubq_s32(x[0], x[4]), C4);
        let e2 = vmlaq_n_s32(vmulq_n_s32(x[2], C2), x[6], C6);
        let e3 = vmlsq_n_s32(vmulq_n_s32(x[2], C6), x[6], C2);
        let (a0, a3) = (vaddq_s32(e0, e2), vsubq_s32(e0, e2));
        let (a1, a2) = (vaddq_s32(e1, e3), vsubq_s32(e1, e3));
        let b0 = vmlaq_n_s32(
            vmlaq_n_s32(vmlaq_n_s32(vmulq_n_s32(x[1], C1), x[3], C3), x[5], C5),
            x[7],
            C7,
        );
        let b1 = vmlsq_n_s32(
            vmlsq_n_s32(vmlsq_n_s32(vmulq_n_s32(x[1], C3), x[3], C7), x[5], C1),
            x[7],
            C5,
        );
        let b2 = vmlaq_n_s32(
            vmlaq_n_s32(vmlsq_n_s32(vmulq_n_s32(x[1], C5), x[3], C1), x[5], C7),
            x[7],
            C3,
        );
        let b3 = vmlsq_n_s32(
            vmlaq_n_s32(vmlsq_n_s32(vmulq_n_s32(x[1], C7), x[3], C5), x[5], C3),
            x[7],
            C1,
        );
        [
            vaddq_s32(a0, b0),
            vaddq_s32(a1, b1),
            vaddq_s32(a2, b2),
            vaddq_s32(a3, b3),
            vsubq_s32(a3, b3),
            vsubq_s32(a2, b2),
            vsubq_s32(a1, b1),
            vsubq_s32(a0, b0),
        ]
    }
}

/// The transpose of 4 vectors of 4 lanes.
#[inline(always)]
fn transpose(m: [int32x4_t; 4]) -> [int32x4_t; 4] {
    // SAFETY: as in `pass`.
    unsafe {
        let (t0, t1) = (vtrn1q_s32(m[0], m[1]), vtrn2q_s32(m[0], m[1]));
        let (t2, t3) = (vtrn1q_s32(m[2], m[3]), vtrn2q_s32(m[2], m[3]));
        let wide = |a, b, second: bool| {
            let (a, b) = (vreinterpretq_s64_s32(a), vreinterpretq_s64_s32(b));
            vreinterpretq_s32_s64(if second {
                vtrn2q_s64(a, b)
            } else {
                vtrn1q_s64(a, b)
            })
        };
        [
            wide(t0, t2, false),
            wide(t1, t3, false),
            wide(t0, t2, true),
            wide(t1, t3, true),
        ]
    }
}

/// The inverse transform of `block`: the 8 rows, limited to -256..=255.
#[inline(always)]
fn inverse(block: &[i16; 64]) -> [int16x8_t; 8] {
    // SAFETY: the loads read the 8 rows of 8 values of `block`; the rest touches no memory.
    unsafe {
        let mut low = [vdupq_n_s32(0); 8];
        let mut high = [vdupq_n_s32(0); 8];
        for v in 0..8 {
            let row = vld1q_s16(block.as_ptr().add(8 * v));
            low[v] = vmovl_s16(vget_low_s16(row));
            high[v] = vmovl_high_s16(row);
        }
        // The columns: lane u of vector v is the value at row v and column u.
        let round = vdupq_n_s32(ROUND_1);
        let low = pass(low).map(|c| vshrq_n_s32::<10>(vaddq_s32(c, round)));
        let high = pass(high).map(|c| vshrq_n_s32::<10>(vaddq_s32(c, round)));
        // The rows: vector u holds the values of column u, for the rows 0 to 3 and for the rows 4 to 7.
        let part = |m: &[int32x4_t; 8], from: usize| {
            transpose([m[from], m[from + 1], m[from + 2], m[from + 3]])
        };
        let join =
            |a: [int32x4_t; 4], b: [int32x4_t; 4]| [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]];
        let round = vdupq_n_s32(ROUND_2);
        let top = pass(join(part(&low, 0), part(&high, 0)))
            .map(|c| vshrq_n_s32::<18>(vaddq_s32(c, round)));
        let bottom = pass(join(part(&low, 4), part(&high, 4)))
            .map(|c| vshrq_n_s32::<18>(vaddq_s32(c, round)));
        let mut rows = [vdupq_n_s16(0); 8];
        let (least, most) = (vdupq_n_s16(-256), vdupq_n_s16(255));
        for (half, m) in [top, bottom].iter().enumerate() {
            let (left, right) = (part(m, 0), part(m, 4));
            for v in 0..4 {
                let row = vcombine_s16(vqmovn_s32(left[v]), vqmovn_s32(right[v]));
                rows[4 * half + v] = vminq_s16(vmaxq_s16(row, least), most);
            }
        }
        rows
    }
}

/// See [`idct::put`].
pub(crate) fn put(block: &[i16; 64], dst: &mut [u8], stride: usize) {
    if dst.len() < 7 * stride + 8 {
        return idct::put(block, dst, stride);
    }
    let rows = inverse(block);
    for (y, row) in rows.into_iter().enumerate() {
        // SAFETY: the 8 samples at `y * stride` are inside `dst` for every `y` below 8, by the check above.
        unsafe { vst1_u8(dst.as_mut_ptr().add(y * stride), vqmovun_s16(row)) }
    }
}

/// See [`idct::add`].
pub(crate) fn add(block: &[i16; 64], dst: &mut [u8], stride: usize) {
    if dst.len() < 7 * stride + 8 {
        return idct::add(block, dst, stride);
    }
    let rows = inverse(block);
    for (y, row) in rows.into_iter().enumerate() {
        // SAFETY: as in `put`.
        unsafe {
            let p = dst.as_mut_ptr().add(y * stride);
            let sum = vaddq_s16(vreinterpretq_s16_u16(vmovl_u8(vld1_u8(p))), row);
            vst1_u8(p, vqmovun_s16(sum));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Random(u64);

    impl Random {
        fn next(&mut self) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) as u32
        }
    }

    #[test]
    fn the_inverse_dct_equals_the_portable_one_for_every_kind_of_block() {
        let mut random = Random(3);
        for round in 0..20_000 {
            // Sparse and dense blocks, small values and the extremes of 16 bits.
            let mut block = [0i16; 64];
            let count = [1, 3, 10, 64][round % 4];
            for _ in 0..count {
                let v = match round % 3 {
                    0 => (random.next() % 512) as i16 - 256,
                    1 => (random.next() % 4096) as i16 - 2048,
                    _ => random.next() as i16,
                };
                block[random.next() as usize % 64] = v;
            }
            let mut a: [u8; 64] = std::array::from_fn(|_| random.next() as u8);
            let mut b = a;
            idct::add(&block, &mut a, 8);
            add(&block, &mut b, 8);
            assert_eq!(a, b, "add, round {round}");
            idct::put(&block, &mut a, 8);
            put(&block, &mut b, 8);
            assert_eq!(a, b, "put, round {round}");
        }
    }

    #[test]
    fn the_prediction_equals_the_portable_one() {
        let mut random = Random(5);
        let src: Vec<u8> = (0..40 * 20).map(|_| random.next() as u8).collect();
        for round in 0..4000 {
            let (w, h) = [(16, 16), (16, 8), (8, 8), (8, 4)][round % 4];
            let s = Source {
                src: &src[random.next() as usize % 60..],
                stride: [20, 40][round / 4 % 2],
                half_x: round / 8 % 2 == 1,
                half_y: round / 16 % 2 == 1,
            };
            let mut a: Vec<u8> = (0..32 * 16).map(|_| random.next() as u8).collect();
            let mut b = a.clone();
            let dstride = [16, 32][round / 32 % 2];
            mc::predict(&s, &mut a, dstride, w, h, round / 64 % 2 == 1);
            predict(&s, &mut b, dstride, w, h, round / 64 % 2 == 1);
            assert_eq!(a, b, "round {round}");
        }
    }
}
