//! The one constellation type: hard decisions and soft demapping over any [`PointSet`] (BPSK, QPSK, 8PSK, square and cross QAM,
//! APSK, PAM and VSB, with any bit map).
//!
//! The points and labels come from `sdrkit_algo_dsp::constellation`, the same definitions the modulators in `sdrkit-algo-sim` use. A
//! [`Constellation`] adds decision tables, chosen once at construction. A thread keeps the tables of the point sets it used last,
//! and constellations of the same points share them: making a constellation again costs one comparison of the points.
//!
//! | Structure | Decision | Soft demapping |
//! |---|---|---|
//! | **Separable** (square QAM, PAM, VSB, hierarchical QAM: every label bit depends on one axis only) | per axis, O(1) on a uniform grid | per axis, exact or max-log: `levels x bits` per axis instead of `points x bits` |
//! | **Grid** (every pair of an I level and a Q level once, bits not per axis: DVB-C 16-, 64- and 256-QAM) | per axis, then the label of the pair from a table | generic |
//! | **PSK** (equally spaced on a circle) | by sector, one `approx::atan2`, no distances | generic |
//! | anything else (APSK, cross QAM, odd shapes; up to 256 points) | nearest point | generic: every distance once, then per bit the minimum (max-log) or the log-sum-exp (exact) |
//!
//! LLRs are true LLRs, `L = ln P(b=0 | z) / P(b=1 | z)`, **positive means 0**, for complex Gaussian noise of total variance
//! `noise_var = E|n|^2`; [`Demap::Exact`] sums over every point, [`Demap::MaxLog`] keeps the nearest point of each bit value. For
//! an equalised cell `z = r / H` pass the weight `|H|^2 / sigma^2` to [`Constellation::demap_weighted`] (channel state information).
//! `out[0]` is the label's most significant bit, the one sent first.
//!
//! ```
//! use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
//! use sdrkit_algo_modem::{Complex32, Constellation, Demap};
//! let c = Constellation::new(PointSet::qam(16, QamMap::THREE_GPP).unwrap()).unwrap();
//! let z = c.points()[0b0110] + Complex32::new(0.05, -0.02);
//! assert_eq!(c.decide(z), 0b0110);
//! let mut llr = [0f32; 4];
//! c.demap(z, 0.01, Demap::MaxLog, &mut llr).unwrap();
//! let bits: Vec<u8> = llr.iter().map(|&l| (l < 0.0) as u8).collect();
//! assert_eq!(bits, [0, 1, 1, 0]);
//! ```

use crate::{Complex32, ModemError, Param, PointSet};
use sdrkit_algo_dsp::approx;
use std::cell::RefCell;
use std::sync::Arc;

/// How [`Constellation::demap`] computes an LLR.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Demap {
    /// The exact LLR: the log of the sum over all points of each bit value (log-sum-exp).
    Exact,
    /// Max-log: only the nearest point of each bit value counts. Exact for BPSK and QPSK, slightly over-confident elsewhere at low
    /// SNR, and cheaper.
    #[default]
    MaxLog,
}

/// The most points the generic paths handle (they keep distances in a stack array).
pub const MAX_GENERIC_POINTS: usize = 256;
/// The most levels per axis the separable path handles.
const MAX_LEVELS: usize = 64;

/// One axis of a separable constellation.
#[derive(Clone, Debug)]
struct Axis {
    /// The levels, in increasing order.
    levels: Vec<f32>,
    /// The label bits this level contributes.
    part: Vec<u16>,
    /// Label bit positions (0 = most significant) that live on this axis.
    bits: Vec<u8>,
    /// `(first level, 1 / step)` when the levels are equally spaced.
    grid: Option<(f32, f32)>,
    /// Per axis bit `j` (in the order of `bits`), per level `i`: `[j][0][i]` is 0 where the bit is 0 and infinite where it is 1,
    /// `[j][1][i]` the reverse (branch-free minima), and `sel[j][i]` is 1.0 where the bit is 1 (sums for the exact method).
    pen: Vec<f32>,
    sel: Vec<f32>,
    /// The tables of the direct max-log form, when the axis has them.
    halves: Option<Halves>,
}

/// The cells that the direct max-log form takes at once.
const LANES: usize = 16;

/// An axis of 2, 4, 8 or 16 levels whose bits each divide the levels in two equal sets (BPSK, QPSK and the square QAM sets up
/// to 256-QAM): the max-log LLR of a bit is the difference of the least squared distances to the two sets.
#[derive(Clone, Debug)]
struct Halves {
    /// The levels of the axis, `2 per` of them.
    levels: [f32; 16],
    /// Per axis bit: the indices of the levels where the bit is 0, then of those where it is 1, `per` of each.
    sides: [[[u8; 8]; 2]; 4],
    /// The levels in each set: half the levels of the axis.
    per: usize,
}

impl Halves {
    /// The tables for `levels` with the labels `part`, or `None` when the axis is not of this form.
    fn of(levels: &[f32], part: &[u16], bits: &[u8], label_bits: usize) -> Option<Halves> {
        let n = levels.len();
        if !matches!(n, 2 | 4 | 8 | 16) || 1 << bits.len() != n {
            return None;
        }
        let mut h = Halves {
            levels: [0.0; 16],
            sides: [[[0; 8]; 2]; 4],
            per: n / 2,
        };
        h.levels[..n].copy_from_slice(levels);
        for (sides, &pos) in h.sides.iter_mut().zip(bits) {
            let shift = label_bits - 1 - pos as usize;
            for (bit, side) in sides.iter_mut().enumerate() {
                let of_side: Vec<u8> = part
                    .iter()
                    .enumerate()
                    .filter(|&(_, &l)| ((l >> shift) & 1) as usize == bit)
                    .map(|(i, _)| i as u8)
                    .collect();
                if of_side.len() != n / 2 {
                    return None;
                }
                side[..n / 2].copy_from_slice(&of_side);
            }
        }
        Some(h)
    }

    /// The max-log LLRs of the bits at `positions` for the coordinates `x` of [`LANES`] cells with the weights `w`: the values
    /// that [`llrs`] writes for each cell. `out` holds `bits` values per cell. The squared distance to each level is computed
    /// once per cell and each bit takes the least of its two sets from them: the values of [`Halves::maxlog_plain`], which
    /// computes the squares again for every bit.
    #[inline]
    fn maxlog(
        &self,
        positions: &[u8],
        x: &[f32; LANES],
        w: &[f32; LANES],
        bits: usize,
        out: &mut [f32],
    ) {
        let mut sq = [[0f32; LANES]; 16];
        for (s, &l) in sq[..2 * self.per].iter_mut().zip(&self.levels) {
            for c in 0..LANES {
                let e = x[c] - l;
                s[c] = e * e;
            }
        }
        for (&pos, [zero, one]) in positions.iter().zip(&self.sides) {
            let mut d0 = sq[usize::from(zero[0])];
            let mut d1 = sq[usize::from(one[0])];
            for (&a, &b) in zero[1..self.per].iter().zip(&one[1..self.per]) {
                let (sa, sb) = (&sq[usize::from(a)], &sq[usize::from(b)]);
                for c in 0..LANES {
                    d0[c] = d0[c].min(sa[c]);
                    d1[c] = d1[c].min(sb[c]);
                }
            }
            for c in 0..LANES {
                out[c * bits + pos as usize] = (d1[c] - d0[c]) * w[c];
            }
        }
    }

    /// The plain form of [`Halves::maxlog`]: for each bit the squared distances to the levels of both sets.
    #[cfg(test)]
    fn maxlog_plain(
        &self,
        positions: &[u8],
        x: &[f32; LANES],
        w: &[f32; LANES],
        bits: usize,
        out: &mut [f32],
    ) {
        for (&pos, [zero, one]) in positions.iter().zip(&self.sides) {
            let (zero, one) = (zero.map(|i| self.levels[usize::from(i)]), one.map(|i| self.levels[usize::from(i)]));
            let mut d0 = [0f32; LANES];
            let mut d1 = [0f32; LANES];
            for c in 0..LANES {
                let e0 = x[c] - zero[0];
                let e1 = x[c] - one[0];
                d0[c] = e0 * e0;
                d1[c] = e1 * e1;
            }
            for (&l0, &l1) in zero[1..self.per].iter().zip(&one[1..self.per]) {
                for c in 0..LANES {
                    let e0 = x[c] - l0;
                    let e1 = x[c] - l1;
                    d0[c] = d0[c].min(e0 * e0);
                    d1[c] = d1[c].min(e1 * e1);
                }
            }
            for c in 0..LANES {
                out[c * bits + pos as usize] = (d1[c] - d0[c]) * w[c];
            }
        }
    }
}

impl Axis {
    /// The tables of the direct max-log form; an axis without bits needs none and has empty ones.
    #[inline]
    fn direct(&self) -> Option<Option<&Halves>> {
        match &self.halves {
            Some(h) => Some(Some(h)),
            None if self.bits.is_empty() => Some(None),
            None => None,
        }
    }

    #[inline]
    fn nearest(&self, x: f32) -> usize {
        let n = self.levels.len();
        if let Some((l0, inv)) = self.grid {
            let k = ((x - l0) * inv).round();
            // NaN and out-of-range values clamp to an end.
            if k >= (n - 1) as f32 {
                n - 1
            } else if k > 0.0 {
                k as usize
            } else {
                0
            }
        } else {
            let mut best = 0;
            let mut bd = f32::INFINITY;
            for (i, &l) in self.levels.iter().enumerate() {
                let d = (x - l).abs();
                if d < bd {
                    bd = d;
                    best = i;
                }
            }
            best
        }
    }
}

#[derive(Clone, Debug)]
enum Fast {
    Separable {
        i: Box<Axis>,
        q: Box<Axis>,
    },
    /// `labels[a * (levels of q) + b]` is the label of the point at level `a` of `i` and level `b` of `q`.
    Grid {
        i: Box<Axis>,
        q: Box<Axis>,
        labels: Vec<u16>,
    },
    Psk {
        m: usize,
        phase0: f32,
        by_sector: Vec<u16>,
    },
    Generic,
}

/// A constellation with its decision tables and soft demapper (module documentation).
#[derive(Clone, Debug)]
pub struct Constellation {
    set: PointSet,
    t: Arc<Tables>,
}

/// The decision tables of the points of a set. They depend on the points and their order only.
#[derive(Debug)]
struct Tables {
    points: Vec<Complex32>,
    bits: usize,
    fast: Fast,
    /// The generic path's penalty and selection tables over all points (see `Axis`).
    pen: Vec<f32>,
    sel: Vec<f32>,
    /// `E|a|^4 / E|a|^2` over the points.
    cma_radius: f32,
}

/// How many point sets a thread keeps tables for.
const KEPT: usize = 8;

thread_local! {
    /// The tables this thread made last, newest first.
    static RECENT: RefCell<Vec<Arc<Tables>>> = const { RefCell::new(Vec::new()) };
}

/// Whether `a` and `b` hold the same values in the same order, bit for bit.
#[inline]
fn same_points(a: &[Complex32], b: &[Complex32]) -> bool {
    let differ = |x: &Complex32, y: &Complex32| {
        (x.re.to_bits() ^ y.re.to_bits()) | (x.im.to_bits() ^ y.im.to_bits())
    };
    a.len() == b.len()
        && a.first()
            .zip(b.first())
            .is_none_or(|(x, y)| differ(x, y) == 0)
        && a.iter().zip(b).fold(0, |acc, (x, y)| acc | differ(x, y)) == 0
}

/// The tables of `set`: those the thread keeps for the same points, or new ones, which the thread then keeps in place of the
/// oldest.
fn tables_of(set: &PointSet) -> Result<Arc<Tables>, ModemError> {
    let bits = set.bits() as usize;
    let kept = RECENT.try_with(|r| {
        r.borrow()
            .iter()
            .find(|t| t.bits == bits && same_points(&t.points, set.points()))
            .cloned()
    });
    if let Ok(Some(t)) = kept {
        return Ok(t);
    }
    let t = Arc::new(Tables::new(set)?);
    // A thread that is ending keeps nothing.
    let _ = RECENT.try_with(|r| {
        let mut r = r.borrow_mut();
        r.truncate(KEPT - 1);
        r.insert(0, Arc::clone(&t));
    });
    Ok(t)
}

/// Sorted distinct values of `v` (merged within `tol`), and each value's index.
fn levels_of(v: &[f32], tol: f32) -> (Vec<f32>, Vec<usize>) {
    let mut s: Vec<f32> = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let mut lv: Vec<f32> = Vec::new();
    for x in s {
        if lv.last().is_none_or(|&l| (x - l).abs() > tol) {
            lv.push(x);
        }
    }
    let idx = v
        .iter()
        .map(|&x| lv.iter().position(|&l| (x - l).abs() <= tol).unwrap_or(0))
        .collect();
    (lv, idx)
}

fn axis_of(levels: Vec<f32>, idx: &[usize], labels_bits: usize) -> Option<Axis> {
    let n = levels.len();
    if n > MAX_LEVELS {
        return None;
    }
    // For each label bit, is it a function of this axis's level alone?
    let mut part = vec![0u16; n];
    let mut bits = Vec::new();
    for pos in 0..labels_bits {
        let shift = labels_bits - 1 - pos;
        let mut val: Vec<Option<u16>> = vec![None; n];
        let mut ok = true;
        for (label, &li) in idx.iter().enumerate() {
            let b = ((label >> shift) & 1) as u16;
            match val[li] {
                None => val[li] = Some(b),
                Some(v) if v == b => {}
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        // A bit constant over the whole axis carries nothing on it; it belongs to the other axis.
        let varies = val.iter().flatten().any(|&b| b == 1) && val.iter().flatten().any(|&b| b == 0);
        if ok && varies {
            bits.push(pos as u8);
            for (li, v) in val.iter().enumerate() {
                part[li] |= v.unwrap_or(0) << shift;
            }
        }
    }
    let grid = if n >= 2 {
        let step = (levels[n - 1] - levels[0]) / (n - 1) as f32;
        let uniform = levels.iter().enumerate().all(|(k, &l)| {
            (l - (levels[0] + k as f32 * step)).abs() <= 1e-4 * step.abs().max(1e-6)
        });
        (uniform && step > 0.0).then(|| (levels[0], 1.0 / step))
    } else {
        Some((levels[0], 0.0))
    };
    let (pen, sel) = penalties(&part, &bits, labels_bits);
    let halves = Halves::of(&levels, &part, &bits, labels_bits);
    Some(Axis {
        halves,
        levels,
        part,
        bits,
        grid,
        pen,
        sel,
    })
}

/// The penalty and selection tables of `labels` (one per level or point) for the label bits at `positions`.
fn penalties(labels: &[u16], positions: &[u8], label_bits: usize) -> (Vec<f32>, Vec<f32>) {
    let n = labels.len();
    let mut pen = vec![0f32; positions.len() * 2 * n];
    let mut sel = vec![0f32; positions.len() * n];
    for (j, &pos) in positions.iter().enumerate() {
        let shift = label_bits - 1 - pos as usize;
        for (i, &l) in labels.iter().enumerate() {
            let one = (l >> shift) & 1 == 1;
            pen[(2 * j) * n + i] = if one { f32::INFINITY } else { 0.0 };
            pen[(2 * j + 1) * n + i] = if one { 0.0 } else { f32::INFINITY };
            sel[j * n + i] = if one { 1.0 } else { 0.0 };
        }
    }
    (pen, sel)
}

/// `e^x` for `x <= 0` within 2e-7 relative (0 below about -87, like the `f32` library function): `2^(x log2 e)` split into a power of two and a degree-6 polynomial on
/// the fraction. No library call, so the exact demapper's per-point likelihoods vectorise.
#[inline]
fn exp_neg(x: f32) -> f32 {
    let t = x * std::f32::consts::LOG2_E;
    if t.is_nan() || t < -126.0 {
        return 0.0;
    }
    let k = t.floor();
    let f = t - k;
    // 2^f on [0, 1): Taylor in f ln 2 to the 6th power (error below 2e-7).
    let y = f * std::f32::consts::LN_2;
    let p = 1.0
        + y * (1.0
            + y * (0.5
                + y * (1.0 / 6.0 + y * (1.0 / 24.0 + y * (1.0 / 120.0 + y * (1.0 / 720.0))))));
    p * f32::from_bits(((k as i32 + 127) as u32) << 23)
}

impl Tables {
    /// Decision tables for `set`. Errors when the set is neither separable nor PSK and has more than [`MAX_GENERIC_POINTS`] points.
    fn new(set: &PointSet) -> Result<Self, ModemError> {
        let points = set.points().to_vec();
        let bits = set.bits() as usize;
        let m = points.len();
        // A set of two levels on each axis that is also PSK (QPSK with a bit map across the axes) takes the PSK path.
        let fast = match Self::try_separable(&points, bits) {
            Some(f @ Fast::Separable { .. }) => f,
            grid => Self::try_psk(&points).or(grid).unwrap_or(Fast::Generic),
        };
        if matches!(fast, Fast::Generic) && m > MAX_GENERIC_POINTS {
            return Param::Points.range(2.0, MAX_GENERIC_POINTS as f64, m as f64);
        }
        let (pen, sel) = if matches!(fast, Fast::Separable { .. }) {
            (Vec::new(), Vec::new())
        } else {
            let labels: Vec<u16> = (0..m as u16).collect();
            let positions: Vec<u8> = (0..bits as u8).collect();
            penalties(&labels, &positions, bits)
        };
        let (mut p2, mut p4) = (0f64, 0f64);
        for p in &points {
            let e = p.norm_sqr() as f64;
            p2 += e;
            p4 += e * e;
        }
        let cma_radius = if p2 > 0.0 { (p4 / p2) as f32 } else { 1.0 };
        Ok(Tables {
            points,
            bits,
            fast,
            pen,
            sel,
            cma_radius,
        })
    }

    fn try_separable(p: &[Complex32], bits: usize) -> Option<Fast> {
        let scale = p.iter().map(|z| z.norm()).fold(0.0f32, f32::max);
        let tol = 1e-4 * scale.max(1e-6);
        let re: Vec<f32> = p.iter().map(|z| z.re).collect();
        let im: Vec<f32> = p.iter().map(|z| z.im).collect();
        let (lr, ir) = levels_of(&re, tol);
        let (lq, iq) = levels_of(&im, tol);
        // A product grid: every (I, Q) pair once.
        if lr.len() * lq.len() != p.len() {
            return None;
        }
        let mut seen = vec![false; p.len()];
        for (a, b) in ir.iter().zip(&iq) {
            let k = a * lq.len() + b;
            if seen[k] {
                return None;
            }
            seen[k] = true;
        }
        let i = axis_of(lr, &ir, bits)?;
        let q = axis_of(lq, &iq, bits)?;
        if i.bits.len() + q.bits.len() != bits {
            // Some bit depends on both axes. The decision is still per axis; the soft demapper is the generic one, with its
            // limit on the number of points.
            if p.len() > MAX_GENERIC_POINTS {
                return None;
            }
            let mut labels = vec![0u16; p.len()];
            for (label, (a, b)) in ir.iter().zip(&iq).enumerate() {
                labels[a * q.levels.len() + b] = label as u16;
            }
            return Some(Fast::Grid {
                i: Box::new(i),
                q: Box::new(q),
                labels,
            });
        }
        Some(Fast::Separable {
            i: Box::new(i),
            q: Box::new(q),
        })
    }

    fn try_psk(p: &[Complex32]) -> Option<Fast> {
        let m = p.len();
        if m < 2 {
            return None;
        }
        if p.iter().any(|z| (z.norm() - 1.0).abs() > 1e-4) {
            return None;
        }
        let step = std::f64::consts::TAU / m as f64;
        let phase0 = (p[0].im as f64).atan2(p[0].re as f64);
        let mut by_sector = vec![u16::MAX; m];
        for (label, z) in p.iter().enumerate() {
            let a = (z.im as f64).atan2(z.re as f64) - phase0;
            let k = (a / step).round();
            if (a - k * step).abs() > 1e-3 {
                return None;
            }
            let s = (k as i64).rem_euclid(m as i64) as usize;
            if by_sector[s] != u16::MAX {
                return None;
            }
            by_sector[s] = label as u16;
        }
        Some(Fast::Psk {
            m,
            phase0: phase0 as f32,
            by_sector,
        })
    }
}

impl Constellation {
    /// Decision tables for `set`. Errors when the set is neither separable nor PSK and has more than [`MAX_GENERIC_POINTS`] points.
    pub fn new(set: PointSet) -> Result<Self, ModemError> {
        let t = tables_of(&set)?;
        Ok(Constellation { set, t })
    }

    /// The constant-modulus radius `E|a|^4 / E|a|^2` (all points equally likely; 1 when every point is 0).
    #[inline]
    pub(crate) fn cma_radius(&self) -> f32 {
        self.t.cma_radius
    }

    /// The point set.
    pub fn set(&self) -> &PointSet {
        &self.set
    }

    /// The points, indexed by label.
    pub fn points(&self) -> &[Complex32] {
        &self.t.points
    }

    /// Number of points.
    pub fn len(&self) -> usize {
        self.t.points.len()
    }

    /// Never true.
    pub fn is_empty(&self) -> bool {
        self.t.points.is_empty()
    }

    /// Bits per symbol.
    pub fn bits(&self) -> usize {
        self.t.bits
    }

    /// Whether every label bit depends on one axis only (the fast per-axis paths are in use).
    pub fn is_separable(&self) -> bool {
        matches!(self.t.fast, Fast::Separable { .. })
    }

    /// Returns the label of the point nearest `z`. A non-finite `z` returns some valid label.
    #[inline]
    pub fn decide(&self, z: Complex32) -> u16 {
        match &self.t.fast {
            Fast::Separable { i, q } => i.part[i.nearest(z.re)] | q.part[q.nearest(z.im)],
            Fast::Grid { i, q, labels } => {
                labels[i.nearest(z.re) * q.levels.len() + q.nearest(z.im)]
            }
            Fast::Psk {
                m,
                phase0,
                by_sector,
            } => {
                let a = approx::atan2(z.im, z.re) - phase0;
                let k = (a * (*m as f32) * (1.0 / std::f32::consts::TAU)).round();
                let s = if k.is_finite() {
                    (k as i64).rem_euclid(*m as i64) as usize
                } else {
                    0
                };
                by_sector[s]
            }
            Fast::Generic => {
                let mut best = 0;
                let mut bd = f32::INFINITY;
                for (l, p) in self.t.points.iter().enumerate() {
                    let d = (z - p).norm_sqr();
                    if d < bd {
                        bd = d;
                        best = l;
                    }
                }
                best as u16
            }
        }
    }

    /// The point nearest `z`.
    #[inline]
    pub fn nearest(&self, z: Complex32) -> Complex32 {
        self.t.points[self.decide(z) as usize]
    }

    /// The hard bits of the point nearest `z` into `out[0..bits]` (most significant first).
    pub fn decide_bits(&self, z: Complex32, out: &mut [u8]) -> Result<(), ModemError> {
        if out.len() < self.t.bits {
            return Err(ModemError::OutputTooShort { need: self.t.bits });
        }
        let l = self.decide(z);
        for (k, o) in out[..self.t.bits].iter_mut().enumerate() {
            *o = ((l >> (self.t.bits - 1 - k)) & 1) as u8;
        }
        Ok(())
    }

    /// The mean point and the mean energy for the bit LLRs `llr` (`bits` values, most significant first, positive means 0):
    /// the sums of the points and of their energies, each weighted by the product of the probabilities of its label bits.
    /// Strong LLRs give the point of their label and its energy, LLRs of 0 the centre of the constellation and its mean energy.
    /// These are the soft symbol and its second moment that re-estimate a channel from the soft output of a decoder: the gain
    /// is `sum z conj(mean) / sum energy`.
    pub fn soft_point(&self, llr: &[f32]) -> Result<(Complex32, f32), ModemError> {
        let bits = self.t.bits;
        if llr.len() < bits {
            return Err(ModemError::InputTooShort { need: bits });
        }
        // The probability of a 0 and of a 1 for each bit.
        let mut p = [[0f32; 2]; 16];
        for (q, &l) in p.iter_mut().zip(&llr[..bits]) {
            let p0 = 1.0 / (1.0 + (-l.clamp(-60.0, 60.0)).exp());
            *q = [p0, 1.0 - p0];
        }
        let (mut sum, mut energy) = (Complex32::new(0.0, 0.0), 0f32);
        for (label, &pt) in self.t.points.iter().enumerate() {
            let w: f32 = (0..bits).map(|k| p[k][(label >> (bits - 1 - k)) & 1]).product();
            sum += pt * w;
            energy += pt.norm_sqr() * w;
        }
        Ok((sum, energy))
    }

    /// The LLRs of the label bits of `z` into `out[0..bits]`, for complex noise of total variance `noise_var`.
    pub fn demap(
        &self,
        z: Complex32,
        noise_var: f32,
        method: Demap,
        out: &mut [f32],
    ) -> Result<(), ModemError> {
        if !(noise_var > 0.0 && noise_var.is_finite()) {
            return Param::NoiseVariance.positive(noise_var as f64);
        }
        self.demap_weighted(z, 1.0 / noise_var, method, out)
    }

    /// The LLRs of `z` with weight `w = 1 / noise_var` (for an equalised cell `z = r / H`, `w = |H|^2 / sigma^2`). A weight of 0
    /// (a cell lost in a null) produces LLRs of 0, erasures.
    pub fn demap_weighted(
        &self,
        z: Complex32,
        weight: f32,
        method: Demap,
        out: &mut [f32],
    ) -> Result<(), ModemError> {
        if out.len() < self.t.bits {
            return Err(ModemError::OutputTooShort { need: self.t.bits });
        }
        if !(z.re.is_finite() && z.im.is_finite() && weight.is_finite() && weight >= 0.0) {
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        let out = &mut out[..self.t.bits];
        match &self.t.fast {
            Fast::Separable { i, q } => {
                self.axis_llr(i, z.re, weight, method, out);
                self.axis_llr(q, z.im, weight, method, out);
            }
            _ => self.generic_llr(z, weight, method, out),
        }
        Ok(())
    }

    /// Demaps several symbols: `out[k * bits ..]` holds the LLRs of `z[k]`. `out` must hold `z.len() * bits` values.
    pub fn demap_block(
        &self,
        z: &[Complex32],
        noise_var: f32,
        method: Demap,
        out: &mut [f32],
    ) -> Result<(), ModemError> {
        if !(noise_var > 0.0 && noise_var.is_finite()) {
            return Param::NoiseVariance.positive(noise_var as f64);
        }
        let need = z.len() * self.t.bits;
        if out.len() < need {
            return Err(ModemError::OutputTooShort { need });
        }
        let w = 1.0 / noise_var;
        for (zz, o) in z.iter().zip(out.chunks_exact_mut(self.t.bits)) {
            self.demap_weighted(*zz, w, method, o)?;
        }
        Ok(())
    }

    /// Demaps equalised cells, each with its own weight `|H|^2 / sigma^2` (the CSI of an OFDM equaliser).
    pub fn demap_block_weighted(
        &self,
        z: &[Complex32],
        weights: &[f32],
        method: Demap,
        out: &mut [f32],
    ) -> Result<(), ModemError> {
        if weights.len() < z.len() {
            return Err(ModemError::InputTooShort { need: z.len() });
        }
        let need = z.len() * self.t.bits;
        if out.len() < need {
            return Err(ModemError::OutputTooShort { need });
        }
        if method == Demap::MaxLog {
            if let Fast::Separable { i, q } = &self.t.fast {
                let valid = |(zz, &w): (&Complex32, &f32)| {
                    zz.re.is_finite() && zz.im.is_finite() && w.is_finite() && w >= 0.0
                };
                if let (Some(hi), Some(hq), true) =
                    (i.direct(), q.direct(), z.iter().zip(weights).all(valid))
                {
                    let m = self.t.bits;
                    // The cells LANES at a time; the last ones in a block of their own that zeros fill up.
                    let mut tail = [0f32; LANES * 8];
                    let cells = z.chunks(LANES).zip(weights[..z.len()].chunks(LANES));
                    for (k, (zs, ws)) in cells.enumerate() {
                        let (mut re, mut im, mut w) = ([0f32; LANES], [0f32; LANES], [0f32; LANES]);
                        for (c, (zz, &ww)) in zs.iter().zip(ws).enumerate() {
                            (re[c], im[c], w[c]) = (zz.re, zz.im, ww);
                        }
                        let whole = zs.len() == LANES;
                        let o = if whole {
                            &mut out[k * LANES * m..(k + 1) * LANES * m]
                        } else {
                            &mut tail[..LANES * m]
                        };
                        if let Some(h) = hi {
                            h.maxlog(&i.bits, &re, &w, m, o);
                        }
                        if let Some(h) = hq {
                            h.maxlog(&q.bits, &im, &w, m, o);
                        }
                        if !whole {
                            let n = zs.len() * m;
                            out[k * LANES * m..k * LANES * m + n].copy_from_slice(&tail[..n]);
                        }
                    }
                    return Ok(());
                }
            }
        }
        for ((zz, &w), o) in z.iter().zip(weights).zip(out.chunks_exact_mut(self.t.bits)) {
            self.demap_weighted(*zz, w, method, o)?;
        }
        Ok(())
    }

    #[inline]
    fn axis_llr(&self, a: &Axis, x: f32, w: f32, method: Demap, out: &mut [f32]) {
        let n = a.levels.len();
        let mut d = [0f32; MAX_LEVELS];
        let mut ex = [0f32; MAX_LEVELS];
        for (dk, &l) in d.iter_mut().zip(&a.levels) {
            let e = x - l;
            *dk = e * e;
        }
        llrs(
            &d[..n],
            &mut ex[..n],
            (&a.pen, &a.sel),
            &a.bits,
            w,
            method,
            out,
        );
    }

    fn generic_llr(&self, z: Complex32, w: f32, method: Demap, out: &mut [f32]) {
        // Scratch on the stack, as small as the set allows (zeroing 2 KB per symbol would cost more than the work).
        if self.t.points.len() <= 64 {
            self.generic_llr_n::<64>(z, w, method, out)
        } else {
            self.generic_llr_n::<MAX_GENERIC_POINTS>(z, w, method, out)
        }
    }

    #[inline]
    fn generic_llr_n<const N: usize>(&self, z: Complex32, w: f32, method: Demap, out: &mut [f32]) {
        let m = self.t.points.len();
        let mut d = [0f32; N];
        let mut ex = [0f32; N];
        for (dk, p) in d.iter_mut().zip(&self.t.points) {
            *dk = (z - p).norm_sqr();
        }
        const POS: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let tables = (self.t.pen.as_slice(), self.t.sel.as_slice());
        llrs(
            &d[..m],
            &mut ex[..m],
            tables,
            &POS[..self.t.bits],
            w,
            method,
            out,
        );
    }
}

/// The LLRs of the bits at `positions` from squared distances `d` to each level or point, with the penalty and selection tables
/// of those bits (branch-free loops that vectorise). `ex` is scratch of the same length as `d`.
#[inline]
fn llrs(
    d: &[f32],
    ex: &mut [f32],
    (pen, sel): (&[f32], &[f32]),
    positions: &[u8],
    w: f32,
    method: Demap,
    out: &mut [f32],
) {
    let n = d.len();
    let dmin = d.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    // The likelihood of each level or point once (exact method only), not once per bit.
    if method == Demap::Exact {
        for (e, &dk) in ex.iter_mut().zip(d) {
            *e = exp_neg(-(dk - dmin) * w);
        }
    }
    for (j, &pos) in positions.iter().enumerate() {
        let p0 = &pen[2 * j * n..(2 * j + 1) * n];
        let p1 = &pen[(2 * j + 1) * n..(2 * j + 2) * n];
        let (mut d0, mut d1) = (f32::INFINITY, f32::INFINITY);
        for ((&dk, &a), &b) in d.iter().zip(p0).zip(p1) {
            d0 = d0.min(dk + a);
            d1 = d1.min(dk + b);
        }
        let maxlog = (d1 - d0) * w;
        out[pos as usize] = match method {
            Demap::MaxLog => maxlog,
            Demap::Exact => {
                // Both sums directly (the smaller one carries the information; a difference would lose it).
                let (mut s0, mut s1) = (0f32, 0f32);
                for (e, s) in ex.iter().zip(&sel[j * n..(j + 1) * n]) {
                    s1 += e * s;
                    s0 += e * (1.0 - s);
                }
                exact_or(s0, s1, maxlog)
            }
        };
    }
}

/// `ln s0 - ln s1`, or the max-log value when a sum underflowed (the bit is then as sure as max-log says).
#[inline]
fn exact_or(s0: f32, s1: f32, maxlog: f32) -> f32 {
    if s0 > 0.0 && s1 > 0.0 {
        // The ratio in f64: in f32 it could overflow (s0 near 256, s1 near 1e-38).
        ((s0 as f64) / (s1 as f64)).ln() as f32
    } else {
        maxlog
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::constellation::{Dvbs2Rate, QamMap};

    fn all_sets() -> Vec<(&'static str, PointSet)> {
        let (g1, g2) = Dvbs2Rate::R3_4.gammas_32apsk().unwrap();
        vec![
            ("bpsk", PointSet::bpsk()),
            ("qpsk", PointSet::qpsk()),
            ("8psk", PointSet::psk(8, 0.0).unwrap()),
            ("dvbs2 8psk", PointSet::dvbs2_8psk()),
            ("16qam 3gpp", PointSet::qam(16, QamMap::THREE_GPP).unwrap()),
            ("64qam dvb", PointSet::qam(64, QamMap::DVB).unwrap()),
            (
                "256qam 802.11",
                PointSet::qam(256, QamMap::IEEE_802_11).unwrap(),
            ),
            ("64qam alpha 4", PointSet::qam_hierarchical(64, 4).unwrap()),
            ("dvbc 16", PointSet::dvb_c(16).unwrap()),
            ("dvbc 64", PointSet::dvb_c(64).unwrap()),
            ("dvbc 256", PointSet::dvb_c(256).unwrap()),
            ("dvbc 32", PointSet::dvb_c(32).unwrap()),
            ("dvbc 128", PointSet::dvb_c(128).unwrap()),
            ("16apsk", PointSet::dvbs2_16apsk(2.85).unwrap()),
            ("32apsk", PointSet::dvbs2_32apsk(g1, g2).unwrap()),
            ("pam4", PointSet::pam(4).unwrap()),
            ("vsb8", PointSet::vsb8()),
        ]
    }

    #[test]
    fn the_direct_max_log_kernel_gives_the_values_of_its_plain_form() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f32 / (1u64 << 23) as f32 - 1.0
        };
        let sets = all_sets()
            .into_iter()
            .map(|(n, s)| (n.to_owned(), s))
            .chain(sdrkit_algo_dsp::constellation::NAMES.iter().filter_map(|n| {
                sdrkit_algo_dsp::constellation::by_name(n).map(|s| (n.to_string(), s))
            }));
        let mut tried = 0;
        for (name, set) in sets {
            let c = Constellation::new(set).unwrap();
            let Fast::Separable { i, q } = &c.t.fast else { continue };
            let m = c.t.bits;
            for axis in [i, q] {
                let Some(h) = &axis.halves else { continue };
                tried += 1;
                let edges = [0.0, -0.0, 1e30, -1e30, 1e-30, f32::MAX, f32::MIN_POSITIVE];
                for round in 0..200 {
                    let mut x = [0f32; LANES];
                    let mut w = [0f32; LANES];
                    for k in 0..LANES {
                        x[k] = match (round + k) % 9 {
                            // A level, half way between two levels and the far ends.
                            0 => h.levels[k % (2 * h.per)],
                            1 => 0.5 * (h.levels[k % (2 * h.per)] + h.levels[(k + 1) % (2 * h.per)]),
                            2 => edges[(round + k) % edges.len()],
                            _ => 2.5 * next(),
                        };
                        w[k] = match k % 5 {
                            0 => 0.0,
                            1 => 1e20,
                            _ => 3.0 * (1.0 + next()),
                        };
                    }
                    let (mut a, mut b) = (vec![0f32; LANES * m], vec![0f32; LANES * m]);
                    h.maxlog(&axis.bits, &x, &w, m, &mut a);
                    h.maxlog_plain(&axis.bits, &x, &w, m, &mut b);
                    let bits = |v: &[f32]| v.iter().map(|f| f.to_bits()).collect::<Vec<u32>>();
                    assert_eq!(bits(&a), bits(&b), "{name} round {round}");
                }
            }
        }
        assert!(tried >= 10, "{tried} axes with the direct form");
    }

    #[test]
    fn block_max_log_is_the_cell_max_log() {
        for (name, set) in all_sets() {
            let c = Constellation::new(set).unwrap();
            let m = c.bits();
            let mut seed = 0x2545_F491_4F6C_DD1Du64;
            let mut next = || {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (seed >> 40) as f32 / (1u64 << 23) as f32 - 1.0
            };
            let z: Vec<Complex32> = (0..500)
                .map(|_| Complex32::new(1.7 * next(), 1.7 * next()))
                .collect();
            let w: Vec<f32> = (0..500).map(|k| (k % 7) as f32 * (1.0 + next())).collect();
            let mut block = vec![0f32; 500 * m];
            c.demap_block_weighted(&z, &w, Demap::MaxLog, &mut block)
                .unwrap();
            let mut cell = vec![0f32; m];
            for (k, (zz, &ww)) in z.iter().zip(&w).enumerate() {
                c.demap_weighted(*zz, ww, Demap::MaxLog, &mut cell).unwrap();
                let got: Vec<u32> = block[k * m..(k + 1) * m]
                    .iter()
                    .map(|v| v.to_bits())
                    .collect();
                let want: Vec<u32> = cell.iter().map(|v| v.to_bits()).collect();
                assert_eq!(got, want, "{name} cell {k}");
            }
        }
        let direct = |s: PointSet| match &Constellation::new(s).unwrap().t.fast {
            Fast::Separable { i, q } => {
                i.halves.is_some() && (q.halves.is_some() || q.bits.is_empty())
            }
            _ => false,
        };
        assert!(direct(PointSet::bpsk()));
        assert!(direct(PointSet::qpsk()));
        assert!(direct(PointSet::qam(16, QamMap::THREE_GPP).unwrap()));
        assert!(direct(PointSet::qam(64, QamMap::DVB).unwrap()));
        assert!(direct(PointSet::qam(256, QamMap::IEEE_802_11).unwrap()));
    }

    #[test]
    fn soft_point_from_bit_llrs() {
        for (name, set) in all_sets() {
            let c = Constellation::new(set).unwrap();
            let m = c.bits();
            let centre = c.points().iter().sum::<Complex32>() / c.len() as f32;
            let mean_energy = c.points().iter().map(|p| p.norm_sqr()).sum::<f32>() / c.len() as f32;
            let (got, e) = c.soft_point(&vec![0.0; m]).unwrap();
            assert!((got - centre).norm() < 1e-4 && (e - mean_energy).abs() < 1e-4, "{name}: centre {got} {e}");
            for (label, &p) in c.points().iter().enumerate() {
                let llr: Vec<f32> = (0..m).map(|k| if (label >> (m - 1 - k)) & 1 == 0 { 40.0 } else { -40.0 }).collect();
                let (got, e) = c.soft_point(&llr).unwrap();
                assert!((got - p).norm() < 1e-4 && (e - p.norm_sqr()).abs() < 1e-4, "{name} label {label}: {got} against {p}");
            }
            assert!(c.soft_point(&vec![0.0; m - 1]).is_err());
        }
        // One QPSK bit known, the other unknown: the mean of two points, of energy 1.
        let c = Constellation::new(PointSet::qpsk()).unwrap();
        let want = (c.points()[0] + c.points()[1]) / 2.0;
        let (got, e) = c.soft_point(&[40.0, 0.0]).unwrap();
        assert!((got - want).norm() < 1e-4 && (e - 1.0).abs() < 1e-4);
    }

    #[test]
    fn paths_are_chosen_by_structure() {
        let sep = |s: PointSet| Constellation::new(s).unwrap().is_separable();
        assert!(sep(PointSet::qam(256, QamMap::DVB).unwrap()));
        assert!(sep(PointSet::qam_hierarchical(16, 2).unwrap()));
        assert!(sep(PointSet::vsb8()));
        assert!(sep(PointSet::qpsk()));
        assert!(!sep(PointSet::psk(8, 0.0).unwrap()));
        assert!(!sep(PointSet::dvb_c(32).unwrap()));
        // The square DVB-C orders are grids with bits across the axes; the cross orders are neither.
        let grid = |m| {
            let c = Constellation::new(PointSet::dvb_c(m).unwrap()).unwrap();
            matches!(c.t.fast, Fast::Grid { .. })
        };
        assert!(grid(16) && grid(64) && grid(256));
        assert!(!grid(32) && !grid(128));
        // 4096-QAM has too many points for the generic path but is separable.
        assert!(sep(PointSet::qam(4096, QamMap::DVB).unwrap()));
    }

    #[test]
    fn every_point_decides_to_its_label_and_demaps_to_its_bits() {
        for (name, set) in all_sets() {
            let c = Constellation::new(set).unwrap();
            let mut llr = vec![0f32; c.bits()];
            let mut hard = vec![0u8; c.bits()];
            for (l, &p) in c.points().iter().enumerate() {
                // A small offset towards the origin must not change the decision.
                let z = p * 0.98 + Complex32::new(0.003, -0.002);
                assert_eq!(c.decide(z) as usize, l, "{name} label {l}");
                c.decide_bits(z, &mut hard).unwrap();
                for method in [Demap::MaxLog, Demap::Exact] {
                    c.demap(z, 0.01, method, &mut llr).unwrap();
                    for b in 0..c.bits() {
                        let bit = (l >> (c.bits() - 1 - b)) & 1;
                        assert_eq!(hard[b] as usize, bit);
                        assert_eq!(
                            llr[b] < 0.0,
                            bit == 1,
                            "{name} {method:?} label {l} bit {b}: {}",
                            llr[b]
                        );
                    }
                }
            }
        }
    }

    /// The exact LLR by brute force in f64.
    fn brute(c: &Constellation, z: Complex32, n0: f64) -> Vec<f64> {
        (0..c.bits())
            .map(|b| {
                let (mut s0, mut s1) = (0f64, 0f64);
                for (l, p) in c.points().iter().enumerate() {
                    let d = ((z.re - p.re) as f64).powi(2) + ((z.im - p.im) as f64).powi(2);
                    let e = (-d / n0).exp();
                    if (l >> (c.bits() - 1 - b)) & 1 == 0 {
                        s0 += e
                    } else {
                        s1 += e
                    }
                }
                s0.ln() - s1.ln()
            })
            .collect()
    }

    #[test]
    fn exact_llrs_match_the_definition_and_max_log_is_its_bound() {
        let zs = [
            Complex32::new(0.31, -0.77),
            Complex32::new(-0.05, 0.12),
            Complex32::new(1.4, 0.9),
        ];
        for (name, set) in all_sets() {
            let c = Constellation::new(set).unwrap();
            let mut ex = vec![0f32; c.bits()];
            let mut ml = vec![0f32; c.bits()];
            for &z in &zs {
                for n0 in [0.05f32, 0.5, 2.0] {
                    c.demap(z, n0, Demap::Exact, &mut ex).unwrap();
                    c.demap(z, n0, Demap::MaxLog, &mut ml).unwrap();
                    let want = brute(&c, z, n0 as f64);
                    for b in 0..c.bits() {
                        assert!(
                            (ex[b] as f64 - want[b]).abs() < 1e-3 * (1.0 + want[b].abs()),
                            "{name} n0 {n0} bit {b}: {} vs {}",
                            ex[b],
                            want[b]
                        );
                        // Max-log has the same sign where the LLR is clear.
                        if want[b].abs() > 1.0 {
                            assert_eq!(ml[b] > 0.0, want[b] > 0.0, "{name}");
                        }
                    }
                }
            }
        }
        // BPSK: LLR = 4 Re z / N0 exactly, both methods.
        let c = Constellation::new(PointSet::bpsk()).unwrap();
        let mut l = [0f32];
        for m in [Demap::Exact, Demap::MaxLog] {
            c.demap(Complex32::new(0.3, 0.7), 0.5, m, &mut l).unwrap();
            assert!((l[0] - 2.4).abs() < 1e-5, "{m:?} {}", l[0]);
        }
    }

    #[test]
    fn weights_errors_and_blocks() {
        let c = Constellation::new(PointSet::qpsk()).unwrap();
        let mut l = [0f32; 2];
        assert!(c
            .demap(Complex32::new(1.0, 0.0), 0.0, Demap::MaxLog, &mut l)
            .is_err());
        assert!(c
            .demap(Complex32::new(f32::NAN, 0.0), 1.0, Demap::MaxLog, &mut l)
            .is_err());
        assert!(c
            .demap(Complex32::new(1.0, 0.0), 1.0, Demap::MaxLog, &mut l[..1])
            .is_err());
        c.demap_weighted(Complex32::new(0.7, -0.7), 0.0, Demap::Exact, &mut l)
            .unwrap();
        assert_eq!(l, [0.0, 0.0]);
        let z = [Complex32::new(0.7, 0.7), Complex32::new(-0.7, 0.7)];
        let mut out = [0f32; 4];
        c.demap_block(&z, 0.1, Demap::MaxLog, &mut out).unwrap();
        assert!(out[0] > 0.0 && out[1] > 0.0 && out[2] < 0.0 && out[3] > 0.0);
        let mut out2 = [0f32; 4];
        c.demap_block_weighted(&z, &[10.0, 10.0], Demap::MaxLog, &mut out2)
            .unwrap();
        assert_eq!(out, out2);
        assert!(c
            .demap_block(&z, 0.1, Demap::MaxLog, &mut out[..3])
            .is_err());
        assert!(c.decide(Complex32::new(f32::NAN, f32::NAN)) < 4);
    }

    /// Repeatable values in -1 to 1.
    fn noise(seed: &mut u32) -> f32 {
        *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (*seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }

    #[test]
    fn shared_tables_decide_as_tables_made_for_one_constellation() {
        for (name, set) in all_sets() {
            let c = Constellation::new(set.clone()).unwrap();
            let again = Constellation::new(set.clone()).unwrap();
            assert!(Arc::ptr_eq(&c.t, &again.t), "{name}: tables are shared");
            let own = Constellation {
                t: Arc::new(Tables::new(&set).unwrap()),
                set,
            };
            assert_eq!(c.cma_radius(), own.cma_radius(), "{name}");
            let scale = c.points().iter().map(|p| p.norm()).fold(0.0, f32::max);
            let mut seed = 7;
            let check = |z: Complex32| {
                let l = c.decide(z);
                assert_eq!(l, own.decide(z), "{name} at {z}");
                assert_eq!(l, again.decide(z), "{name} at {z}");
                // The decision is the nearest point, where one point is clearly the nearest.
                let mut d: Vec<(f32, usize)> = c
                    .points()
                    .iter()
                    .enumerate()
                    .map(|(k, p)| ((z - p).norm(), k))
                    .collect();
                d.sort_by(|a, b| a.0.total_cmp(&b.0));
                let margin = if matches!(c.t.fast, Fast::Psk { .. }) {
                    0.02
                } else {
                    1e-4
                };
                if d[1].0 - d[0].0 > margin * scale {
                    assert_eq!(l as usize, d[0].1, "{name} at {z}");
                }
            };
            for l in 0..c.len() {
                let p = c.points()[l];
                check(p);
                for _ in 0..200 {
                    let n = Complex32::new(noise(&mut seed), noise(&mut seed));
                    check(p + n * (0.3 * scale));
                }
            }
        }
    }

    #[test]
    fn point_sets_of_one_size_keep_their_own_tables() {
        // The same 16 points with two bit maps, and 8PSK at many rotations (more than a thread keeps): each constellation
        // decides its own points to their labels, also when it is made again after the others.
        let mut sets = vec![
            PointSet::qam(16, QamMap::THREE_GPP).unwrap(),
            PointSet::qam(16, QamMap::DVB).unwrap(),
        ];
        sets.extend((0..2 * KEPT).map(|k| PointSet::psk(8, 0.05 * k as f64).unwrap()));
        let made: Vec<Constellation> = sets
            .iter()
            .map(|s| Constellation::new(s.clone()).unwrap())
            .collect();
        let again: Vec<Constellation> = sets
            .iter()
            .map(|s| Constellation::new(s.clone()).unwrap())
            .collect();
        for (k, set) in sets.iter().enumerate() {
            for c in [&made[k], &again[k]] {
                assert_eq!(c.points(), set.points(), "set {k}");
                for (l, &p) in set.points().iter().enumerate() {
                    assert_eq!(c.decide(p * 0.99) as usize, l, "set {k} label {l}");
                }
            }
            for other in &made[..k] {
                assert!(!Arc::ptr_eq(&made[k].t, &other.t), "set {k}");
            }
        }
    }
}
