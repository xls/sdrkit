//! The NEON form of the layered min-sum decoder (`aarch64`): 16 checks of a layer at a time.
//!
//! It does the arithmetic of the plain decoder in `minsum.rs` (16-bit posterior LLRs, 8-bit check messages limited to ±127,
//! the first smallest magnitude keeps its index), so the hard bits, the result and the iteration count are the same.
//!
//! The `z` checks of a layer are cut into groups of 16 lanes. `z` is rounded up to a multiple of 16; a lane at or above `z`
//! repeats lane `l - z` and is never written to a posterior. The check messages are stored in the order of the sweep:
//! layer, group of 16 lanes, edge. The posteriors of a column are followed by a copy of its first 16, so that 16 lanes
//! rotated by any shift are 16 values in a row.
//!
//! `unsafe` is limited to the calls into the functions that need NEON, which every `aarch64` processor has, and to the loads
//! and stores of the intrinsics. Each load and store is of an array of 16 values.

#![allow(unsafe_code)]

use super::minsum::{LdpcBackend, LdpcKernel, MinSum};
use super::{Edge, LdpcCode, NO_MASK};
use crate::error::{Decoded, FecError, Result};
use crate::llr::Llr8;
use sdrkit_core::backend::Backend;
use std::arch::aarch64::*;

/// Lanes of one vector.
const W: usize = 16;

/// The largest number of edges at one bit that the kernel takes. A posterior is the input value plus one message of at
/// most 127 per edge, so below this bound it stays inside 16 bits and the limit of the plain decoder never acts.
const MAX_BIT_DEGREE: usize = 200;

/// 16 values of all ones, then 16 of zero: the 16 from index `16 - n` select the first `n` lanes.
static LIVE: [i16; 2 * W] = {
    let mut t = [0i16; 2 * W];
    let mut i = 0;
    while i < W {
        t[i] = -1;
        i += 1;
    }
    t
};

/// The bit of a place that marks a group of an edge with a lane that has no edge, with lanes at or above `z`, or whose
/// posteriors touch the first 16 values of the column or its copy.
const SPECIAL: u32 = 1 << 31;

/// The NEON backend.
pub(super) struct Neon;

impl Backend for Neon {
    fn name(&self) -> &'static str {
        "neon"
    }
}

impl LdpcBackend for Neon {
    fn decoder(&self, code: &LdpcCode, rule: MinSum) -> Option<Box<dyn LdpcKernel>> {
        Some(Box::new(NeonMinSum::new(code, rule)?))
    }
}

/// What one group of 16 lanes of a layer keeps between reading and writing its posteriors: the scaled smallest and second
/// smallest magnitude, the edge of the smallest, and the sign product in bit 7.
type Minima = [uint8x16_t; 4];

/// The decoder state for one code.
#[derive(Clone, Debug)]
struct NeonMinSum {
    rule: MinSum,
    /// Groups of 16 lanes per layer.
    groups: usize,
    /// The most edges of a layer.
    maxdeg: usize,
    /// Posterior LLRs: for each column `z` values, then a copy of the first 16 of them.
    l: Vec<i16>,
    /// Check messages: for each layer, for each group, for each edge, 16 lanes.
    r: Vec<i8>,
    /// Bit-to-check messages of the layer in work: for each group, `maxdeg` edges of 16 lanes.
    q: Vec<i8>,
    /// For each layer, for each group, for each edge: where in `l` the 16 posteriors of the group start, with [`SPECIAL`].
    places: Vec<u32>,
    /// The minima of each group of the layer in work.
    minima: Vec<Minima>,
    /// Per layer: no column occurs twice, so the groups of the layer touch separate bits and each group can be finished
    /// before the next is read.
    separate: Vec<bool>,
}

impl LdpcKernel for NeonMinSum {
    fn decode(
        &mut self,
        code: &LdpcCode,
        llr: &[Llr8],
        max_iter: u32,
        out: &mut [u8],
    ) -> Result<Decoded> {
        let (n, z) = (code.n(), code.z());
        let mut i = 0;
        for col in self.l.chunks_exact_mut(z + W) {
            for v in col[..z.min(n - i)].iter_mut() {
                *v = llr[code.pos(i)].max(-127) as i16;
                i += 1;
            }
            col.copy_within(..W, z);
        }
        self.r.fill(0);
        let mut used = 0;
        // SAFETY: NEON is part of the aarch64 baseline, and so is present for the two functions that need it.
        let mut ok = unsafe { self.syndrome_ok(code) };
        while !ok && used < max_iter {
            for li in 0..code.layers() {
                // SAFETY: as above.
                unsafe { self.layer(code, li) };
            }
            used += 1;
            // SAFETY: as above.
            ok = unsafe { self.syndrome_ok(code) };
        }
        let mut corrected = 0u32;
        let mut i = 0;
        for col in self.l.chunks_exact(z + W) {
            for &v in &col[..z.min(n - i)] {
                let p = code.pos(i);
                let b = (v < 0) as u8;
                out[p] = b;
                if llr[p] != 0 && ((llr[p] < 0) as u8) != b {
                    corrected += 1;
                }
                i += 1;
            }
        }
        if ok {
            Ok(Decoded {
                corrected,
                metric: 0,
                iterations: used,
            })
        } else {
            Err(FecError::Uncorrectable)
        }
    }

    fn box_clone(&self) -> Box<dyn LdpcKernel> {
        Box::new(self.clone())
    }

    fn posterior(&self, code: &LdpcCode, out: &mut [i16]) -> bool {
        let (n, z) = (code.n(), code.z());
        let mut i = 0;
        for col in self.l.chunks_exact(z + W) {
            for &v in &col[..z.min(n - i)] {
                out[code.pos(i)] = v;
                i += 1;
            }
        }
        true
    }
}

impl NeonMinSum {
    /// The state for `code`. `None` for a code with `z` below 16, with more than 255 edges in a layer, with more than
    /// [`MAX_BIT_DEGREE`] edges at a bit or with 2^31 posteriors or more.
    fn new(code: &LdpcCode, rule: MinSum) -> Option<Self> {
        let z = code.z();
        let groups = z.div_ceil(W);
        let mut degree = vec![0usize; code.ncols()];
        let mut separate = Vec::with_capacity(code.layers());
        let mut maxdeg = 0;
        for li in 0..code.layers() {
            let edges = code.layer(li);
            maxdeg = maxdeg.max(edges.len());
            for e in edges {
                degree[e.col as usize] += 1;
            }
            let mut cols: Vec<u32> = edges.iter().map(|e| e.col).collect();
            cols.sort_unstable();
            separate.push(cols.windows(2).all(|w| w[0] != w[1]));
        }
        if z < W
            || code.n() > code.ncols() * z
            || maxdeg > 255
            || degree.iter().any(|&d| d > MAX_BIT_DEGREE)
            || code.ncols() * (z + W) >= SPECIAL as usize
        {
            return None;
        }
        let mut places = Vec::with_capacity(code.all_edges().len() * groups);
        for li in 0..code.layers() {
            for lane in (0..z).step_by(W) {
                for edge in code.layer(li) {
                    let (col, from) = place(edge, z, lane);
                    let special = hole_in(edge, lane) || lane + W > z || from + W > z || from < W;
                    places.push((col + from) as u32 | if special { SPECIAL } else { 0 });
                }
            }
        }
        Some(NeonMinSum {
            rule,
            groups,
            maxdeg,
            l: vec![0; code.ncols() * (z + W)],
            r: vec![0; code.all_edges().len() * groups * W],
            q: vec![0; maxdeg * groups * W],
            // SAFETY: NEON is part of the aarch64 baseline.
            minima: vec![[unsafe { vdupq_n_u8(0) }; 4]; groups],
            places,
            separate,
        })
    }

    /// True when the signs of the posteriors satisfy every check.
    #[target_feature(enable = "neon")]
    fn syndrome_ok(&self, code: &LdpcCode) -> bool {
        let z = code.z();
        for li in 0..code.layers() {
            let edges = code.layer(li);
            for lane in (0..z).step_by(W) {
                let mut acc = (vdupq_n_s16(0), vdupq_n_s16(0));
                for e in edges {
                    let v = load_rotated(&self.l, e, z, lane);
                    acc = (veorq_s16(acc.0, v.0), veorq_s16(acc.1, v.1));
                    if hole_in(e, lane) {
                        // The lane without an edge does not count.
                        let absent = self.l[bit_of_hole(e, z)];
                        acc = with_lane_s16(acc, e.mask as usize % W, |v| v ^ absent);
                    }
                }
                if lane + W <= z {
                    if vminvq_s16(vorrq_s16(acc.0, acc.1)) < 0 {
                        return false;
                    }
                } else {
                    let lanes = to_array_s16(acc);
                    if lanes[..z - lane].iter().any(|&v| v < 0) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// One layer: reads the posteriors of its bits, updates the check messages and writes the posteriors back.
    #[target_feature(enable = "neon")]
    fn layer(&mut self, code: &LdpcCode, li: usize) {
        let start = code.layer_start[li] as usize;
        let edges = &code.edges[start..code.layer_start[li + 1] as usize];
        let (z, deg) = (code.z(), edges.len());
        if deg == 0 {
            return;
        }
        let r = &mut self.r[start * self.groups * W..(start + deg) * self.groups * W];
        let places = &self.places[start * self.groups..(start + deg) * self.groups];
        if self.separate[li] {
            let q = &mut self.q[..deg * W];
            let slots = r.chunks_exact_mut(deg * W).zip(places.chunks_exact(deg));
            for (g, (r, places)) in slots.enumerate() {
                let minima = gather(edges, places, &self.l, r, q, g * W, self.rule);
                spread(edges, places, &mut self.l, r, q, z, g * W, minima);
            }
        } else {
            let q = self.q.chunks_exact_mut(self.maxdeg * W);
            let slots = r.chunks_exact(deg * W).zip(places.chunks_exact(deg));
            for (g, (((r, places), q), m)) in slots.zip(q).zip(self.minima.iter_mut()).enumerate() {
                *m = gather(edges, places, &self.l, r, q, g * W, self.rule);
            }
            let q = self.q.chunks_exact(self.maxdeg * W);
            let slots = r.chunks_exact_mut(deg * W).zip(places.chunks_exact(deg));
            for (g, (((r, places), q), m)) in slots.zip(q).zip(self.minima.iter()).enumerate() {
                spread(edges, places, &mut self.l, r, q, z, g * W, *m);
            }
        }
    }
}

/// For the 16 lanes from `lane` of a layer with `edges`, whose posteriors are at `places` in `l`: takes the check message
/// (`r`, 16 per edge) out of each posterior, keeps the result in `q` (16 per edge), and returns the minima of the lanes.
#[inline]
#[target_feature(enable = "neon")]
fn gather(
    edges: &[Edge],
    places: &[u32],
    l: &[i16],
    r: &[i8],
    q: &mut [i8],
    lane: usize,
    rule: MinSum,
) -> Minima {
    let floor = vdupq_n_s8(-127);
    let mut min1 = vdupq_n_u8(127);
    let mut min2 = vdupq_n_u8(127);
    let mut idx = vdupq_n_u8(0);
    let mut sgn = vdupq_n_u8(0);
    let slots = r.as_chunks::<W>().0.iter().zip(q.as_chunks_mut::<W>().0);
    for (((e, edge), &place), (r, q)) in edges.iter().enumerate().zip(places).zip(slots) {
        let (lo, hi) = load_s16(&l[(place & !SPECIAL) as usize..]);
        let r = load_s8(r);
        let dlo = vsubw_s8(lo, vget_low_s8(r));
        let dhi = vsubw_high_s8(hi, r);
        let mut v = vmaxq_s8(vqmovn_high_s16(vqmovn_s16(dlo), dhi), floor);
        if place & SPECIAL != 0 && hole_in(edge, lane) {
            let mut lanes = to_array_s8(v);
            lanes[edge.mask as usize % W] = 127;
            v = load_s8(&lanes);
        }
        store_s8(q, v);
        let a = vreinterpretq_u8_s8(vabsq_s8(v));
        let lt = vcltq_u8(a, min1);
        min2 = vbslq_u8(lt, min1, vminq_u8(min2, a));
        min1 = vminq_u8(min1, a);
        idx = vbslq_u8(lt, vdupq_n_u8(e as u8), idx);
        sgn = veorq_u8(sgn, vreinterpretq_u8_s8(v));
    }
    [scaled(min1, rule), scaled(min2, rule), idx, sgn]
}

/// `v * scale_q4 / 16` rounded, limited to 127, less the offset of the rule.
#[inline]
#[target_feature(enable = "neon")]
fn scaled(v: uint8x16_t, rule: MinSum) -> uint8x16_t {
    let sc = vdup_n_u8(rule.scale_q4);
    let lo = vqrshrn_n_u16::<4>(vmull_u8(vget_low_u8(v), sc));
    let hi = vqrshrn_n_u16::<4>(vmull_u8(vget_high_u8(v), sc));
    let m = vminq_u8(vcombine_u8(lo, hi), vdupq_n_u8(127));
    vqsubq_u8(m, vdupq_n_u8(rule.offset))
}

/// For the 16 lanes from `lane` of a layer with `edges`, whose posteriors are at `places` in `l`: makes the new check
/// message of each edge from `q` and the minima that [`gather`] left, stores it in `r` and adds its change to the posterior.
#[inline]
#[target_feature(enable = "neon")]
#[allow(clippy::too_many_arguments)]
fn spread(
    edges: &[Edge],
    places: &[u32],
    l: &mut [i16],
    r: &mut [i8],
    q: &[i8],
    z: usize,
    lane: usize,
    [min1, min2, idx, sgn]: Minima,
) {
    let slots = r
        .as_chunks_mut::<W>()
        .0
        .iter_mut()
        .zip(q.as_chunks::<W>().0);
    for (((e, edge), &place), (r, q)) in edges.iter().enumerate().zip(places).zip(slots) {
        let q = vreinterpretq_u8_s8(load_s8(q));
        let mag = vbslq_u8(vceqq_u8(idx, vdupq_n_u8(e as u8)), min2, min1);
        let mag = vreinterpretq_s8_u8(mag);
        let neg = vcltzq_s8(vreinterpretq_s8_u8(veorq_u8(sgn, q)));
        let mut rn = vbslq_s8(neg, vnegq_s8(mag), mag);
        let old = load_s8(r);
        if place & SPECIAL == 0 {
            store_s8(r, rn);
            let at = &mut l[place as usize..];
            let (lo, hi) = load_s16(at);
            let lo = vaddq_s16(lo, vsubl_s8(vget_low_s8(rn), vget_low_s8(old)));
            let hi = vaddq_s16(hi, vsubl_high_s8(rn, old));
            store_s16(at, (lo, hi));
            continue;
        }
        let hole = hole_in(edge, lane);
        let mut keep = 0;
        if hole {
            // The lane without an edge: its message stays 0 and its posterior stays as it is.
            keep = l[bit_of_hole(edge, z)];
            let mut lanes = to_array_s8(rn);
            lanes[edge.mask as usize % W] = 0;
            rn = load_s8(&lanes);
        }
        store_s8(r, rn);
        let (lo, hi) = load_rotated(l, edge, z, lane);
        let mut new = (
            vaddq_s16(lo, vsubl_s8(vget_low_s8(rn), vget_low_s8(old))),
            vaddq_s16(hi, vsubl_high_s8(rn, old)),
        );
        if lane + W > z {
            // The lanes at and above `z` keep what they hold.
            let live = load_s16(&LIVE[W - (z - lane)..]);
            new = (
                vbslq_s16(vreinterpretq_u16_s16(live.0), new.0, lo),
                vbslq_s16(vreinterpretq_u16_s16(live.1), new.1, hi),
            );
        }
        store_rotated(l, edge, z, lane, new);
        if hole {
            let bit = bit_of_hole(edge, z);
            l[bit] = keep;
            if bit % (z + W) < W {
                l[bit + z] = keep;
            }
        }
    }
}

/// True when the lane of `edge` that has no edge is one of the 16 from `lane`.
#[inline]
fn hole_in(edge: &Edge, lane: usize) -> bool {
    edge.mask != NO_MASK && edge.mask as usize / W == lane / W
}

/// The place in the posteriors of the bit that the masked lane of `edge` would check.
#[inline]
fn bit_of_hole(edge: &Edge, z: usize) -> usize {
    edge.col as usize * (z + W) + (edge.mask as usize + edge.shift as usize) % z
}

/// The place in the posteriors `l` of the bit that lane `lane` of `edge` checks. The 16 values from there are the bits of
/// the 16 lanes from `lane`; a lane at or above `z` gets the bit of lane `lane - z`.
#[inline]
fn place(edge: &Edge, z: usize, lane: usize) -> (usize, usize) {
    let at = lane + edge.shift as usize;
    let from = if at >= z { at - z } else { at };
    (edge.col as usize * (z + W), from)
}

/// The posteriors of the bits that the 16 lanes from `lane` of `edge` check.
#[inline]
#[target_feature(enable = "neon")]
fn load_rotated(l: &[i16], edge: &Edge, z: usize, lane: usize) -> (int16x8_t, int16x8_t) {
    let (col, from) = place(edge, z, lane);
    load_s16(&l[col + from..col + from + W])
}

/// Stores `v` as the posteriors of the bits that the 16 lanes from `lane` of `edge` check, and keeps the copy of the first
/// 16 values of the column equal to them.
#[inline]
#[target_feature(enable = "neon")]
fn store_rotated(l: &mut [i16], edge: &Edge, z: usize, lane: usize, v: (int16x8_t, int16x8_t)) {
    let (col, from) = place(edge, z, lane);
    store_s16(&mut l[col + from..col + from + W], v);
    if from + W > z || from < W {
        // The store went past the end of the column, into the copy, or into the first 16 values, or both. Lanes below
        // `from` of the first 16 are current in the copy and the others in the column.
        let copy = load_s16(&l[col + z..col + z + W]);
        let first = load_s16(&l[col..col + W]);
        let lower = load_s16(&LIVE[W - from.min(W)..]);
        let v = (
            vbslq_s16(vreinterpretq_u16_s16(lower.0), copy.0, first.0),
            vbslq_s16(vreinterpretq_u16_s16(lower.1), copy.1, first.1),
        );
        store_s16(&mut l[col..col + W], v);
        store_s16(&mut l[col + z..col + z + W], v);
    }
}

/// The first 16 values of `src`.
#[inline]
#[target_feature(enable = "neon")]
fn load_s16(src: &[i16]) -> (int16x8_t, int16x8_t) {
    let src: &[i16; W] = src[..W].try_into().expect("16 values");
    // SAFETY: `src` is 16 values long; the two loads read values 0..8 and 8..16 of it.
    unsafe { (vld1q_s16(src.as_ptr()), vld1q_s16(src.as_ptr().add(8))) }
}

/// Writes `v` to the first 16 values of `dst`.
#[inline]
#[target_feature(enable = "neon")]
fn store_s16(dst: &mut [i16], v: (int16x8_t, int16x8_t)) {
    let dst: &mut [i16; W] = (&mut dst[..W]).try_into().expect("16 values");
    // SAFETY: `dst` is 16 values long; the two stores write values 0..8 and 8..16 of it.
    unsafe {
        vst1q_s16(dst.as_mut_ptr(), v.0);
        vst1q_s16(dst.as_mut_ptr().add(8), v.1);
    }
}

#[inline]
#[target_feature(enable = "neon")]
fn load_s8(src: &[i8; W]) -> int8x16_t {
    // SAFETY: `src` is 16 bytes long.
    unsafe { vld1q_s8(src.as_ptr()) }
}

#[inline]
#[target_feature(enable = "neon")]
fn store_s8(dst: &mut [i8; W], v: int8x16_t) {
    // SAFETY: `dst` is 16 bytes long.
    unsafe { vst1q_s8(dst.as_mut_ptr(), v) }
}

#[inline]
#[target_feature(enable = "neon")]
fn to_array_s8(v: int8x16_t) -> [i8; W] {
    let mut lanes = [0i8; W];
    store_s8(&mut lanes, v);
    lanes
}

#[inline]
#[target_feature(enable = "neon")]
fn to_array_s16(v: (int16x8_t, int16x8_t)) -> [i16; W] {
    let mut lanes = [0i16; W];
    store_s16(&mut lanes, v);
    lanes
}

/// `v` with lane `i` replaced by `f` of it.
#[inline]
#[target_feature(enable = "neon")]
fn with_lane_s16(
    v: (int16x8_t, int16x8_t),
    i: usize,
    f: impl FnOnce(i16) -> i16,
) -> (int16x8_t, int16x8_t) {
    let mut lanes = to_array_s16(v);
    lanes[i] = f(lanes[i]);
    load_s16(&lanes)
}
