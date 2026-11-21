//! The max-log-MAP pass of one constituent, with all states of a trellis step in one vector of 16-bit lanes.
//!
//! Metrics are `i16`. Branch metrics add with wrap-around, state metrics with saturation. After every step the metrics have
//! the metric of state 0 subtracted and are held at or above [`NEG`], so they stay inside 16 bits. The sums of the extrinsic
//! output are formed in 32 bits. [`Plain`] and the NEON forms do the same arithmetic lane by lane: their results are equal bit
//! for bit.

use super::Rsc;

/// The floor of a state metric: an unreachable state.
pub(super) const NEG: i16 = -16_000;

/// The trellis of a constituent as lane tables. `next` and `next_par` are indexed by the source state, `pred*` by the
/// destination state. Signs are +1 for a 0 bit, -1 for a 1 bit, 0 for a parity the constituent does not have.
#[derive(Clone, Debug)]
pub(super) struct Trellis {
    /// Number of states (4, 8 or 16).
    pub ns: usize,
    /// `next[u][s]`: the state after input `u` in state `s`.
    next: [[u8; 16]; 2],
    /// `next_par[u][j][s]`: the sign of parity `j` on that branch.
    next_par: [[[i16; 16]; 2]; 2],
    /// `pred[b][d]`: the source state of branch `b` into state `d`.
    pred: [[u8; 16]; 2],
    /// `pred_sys[b][d]`: the sign of the input on that branch.
    pred_sys: [[i16; 16]; 2],
    /// `pred_par[b][j][d]`: the sign of parity `j` on that branch.
    pred_par: [[[i16; 16]; 2]; 2],
}

impl Trellis {
    /// The tables of `r`.
    pub fn new(r: &Rsc) -> Self {
        let ns = r.states();
        let mut t = Trellis {
            ns,
            next: [[0; 16]; 2],
            next_par: [[[0; 16]; 2]; 2],
            pred: [[0; 16]; 2],
            pred_sys: [[0; 16]; 2],
            pred_par: [[[0; 16]; 2]; 2],
        };
        let sign = |bit: u8| if bit & 1 == 0 { 1i16 } else { -1 };
        let mut seen = [0usize; 16];
        for s in 0..ns {
            for u in 0..2 {
                let (p, d) = r.step(s, u as u8);
                t.next[u][s] = d as u8;
                // Every state has two branches in: the register shift fixes all but one bit of the source.
                let b = seen[d].min(1);
                seen[d] += 1;
                t.pred[b][d] = s as u8;
                t.pred_sys[b][d] = sign(u as u8);
                for j in 0..r.parities {
                    t.next_par[u][j][s] = sign(p >> j);
                    t.pred_par[b][j][d] = sign(p >> j);
                }
            }
        }
        t
    }
}

/// One vector with a lane per trellis state.
pub(super) trait Lanes: Copy {
    /// Number of lanes.
    const N: usize;
    /// A lane permutation in the form [`Lanes::permute`] takes.
    type Perm: Copy;
    /// The permutation that puts lane `idx[i]` into lane `i`.
    fn perm(idx: &[u8; 16]) -> Self::Perm;
    /// `v` in every lane.
    fn splat(v: i16) -> Self;
    /// The first `N` values of `p`.
    fn load(p: &[i16]) -> Self;
    /// Writes the lanes to the first `N` values of `p`.
    fn store(self, p: &mut [i16]);
    /// The lanes in the order of `p`.
    fn permute(self, p: Self::Perm) -> Self;
    /// Saturating sum.
    fn qadd(self, o: Self) -> Self;
    /// Saturating difference.
    fn qsub(self, o: Self) -> Self;
    /// The larger of each lane.
    fn max(self, o: Self) -> Self;
    /// `self + v * sign`, wrapping.
    fn mla(self, v: i16, sign: Self) -> Self;
    /// Lane 0 in every lane.
    fn spread_first(self) -> Self;
    /// The largest `a + g + b` over the lanes, summed in 32 bits.
    fn max_sum(a: Self, g: Self, b: Self) -> i32;
}

/// The lanes as an array; the arithmetic every other form must match.
#[derive(Clone, Copy)]
pub(super) struct Plain<const N: usize>([i16; N]);

impl<const N: usize> Lanes for Plain<N> {
    const N: usize = N;
    type Perm = [u8; N];

    #[inline(always)]
    fn perm(idx: &[u8; 16]) -> [u8; N] {
        core::array::from_fn(|i| idx[i] & (N - 1) as u8)
    }
    #[inline(always)]
    fn splat(v: i16) -> Self {
        Plain([v; N])
    }
    #[inline(always)]
    fn load(p: &[i16]) -> Self {
        let p = &p[..N];
        Plain(core::array::from_fn(|i| p[i]))
    }
    #[inline(always)]
    fn store(self, p: &mut [i16]) {
        p[..N].copy_from_slice(&self.0);
    }
    #[inline(always)]
    fn permute(self, p: [u8; N]) -> Self {
        Plain(core::array::from_fn(|i| self.0[p[i] as usize & (N - 1)]))
    }
    #[inline(always)]
    fn qadd(self, o: Self) -> Self {
        Plain(core::array::from_fn(|i| self.0[i].saturating_add(o.0[i])))
    }
    #[inline(always)]
    fn qsub(self, o: Self) -> Self {
        Plain(core::array::from_fn(|i| self.0[i].saturating_sub(o.0[i])))
    }
    #[inline(always)]
    fn max(self, o: Self) -> Self {
        Plain(core::array::from_fn(|i| self.0[i].max(o.0[i])))
    }
    #[inline(always)]
    fn mla(self, v: i16, sign: Self) -> Self {
        Plain(core::array::from_fn(|i| {
            self.0[i].wrapping_add(v.wrapping_mul(sign.0[i]))
        }))
    }
    #[inline(always)]
    fn spread_first(self) -> Self {
        Plain([self.0[0]; N])
    }
    #[inline(always)]
    fn max_sum(a: Self, g: Self, b: Self) -> i32 {
        let mut m = i32::MIN;
        for i in 0..N {
            m = m.max(a.0[i] as i32 + g.0[i] as i32 + b.0[i] as i32);
        }
        m
    }
}

/// NEON lanes for 8 and 16 states. `unsafe` is limited to the intrinsics: NEON is part of the aarch64 baseline, and every
/// load and store is on a slice cut to the vector length first.
#[cfg(target_arch = "aarch64")]
#[allow(unsafe_code)]
pub(super) mod neon {
    use super::Lanes;
    use core::arch::aarch64::*;

    /// Byte indices of a table lookup that moves 16-bit lane `idx[first + i]` into lane `i`.
    fn bytes(idx: &[u8; 16], first: usize) -> uint8x16_t {
        let mut b = [0u8; 16];
        for i in 0..8 {
            b[2 * i] = 2 * idx[first + i];
            b[2 * i + 1] = 2 * idx[first + i] + 1;
        }
        // SAFETY: `b` holds 16 bytes.
        unsafe { vld1q_u8(b.as_ptr()) }
    }

    /// Eight lanes.
    #[derive(Clone, Copy)]
    pub struct Neon8(int16x8_t);

    impl Lanes for Neon8 {
        const N: usize = 8;
        type Perm = uint8x16_t;

        #[inline(always)]
        fn perm(idx: &[u8; 16]) -> uint8x16_t {
            let mut low = *idx;
            for v in low.iter_mut() {
                *v &= 7;
            }
            bytes(&low, 0)
        }
        #[inline(always)]
        fn splat(v: i16) -> Self {
            // SAFETY: see the module note.
            Neon8(unsafe { vdupq_n_s16(v) })
        }
        #[inline(always)]
        fn load(p: &[i16]) -> Self {
            let p = &p[..8];
            // SAFETY: `p` holds 8 values.
            Neon8(unsafe { vld1q_s16(p.as_ptr()) })
        }
        #[inline(always)]
        fn store(self, p: &mut [i16]) {
            let p = &mut p[..8];
            // SAFETY: `p` holds 8 values.
            unsafe { vst1q_s16(p.as_mut_ptr(), self.0) }
        }
        #[inline(always)]
        fn permute(self, p: uint8x16_t) -> Self {
            // SAFETY: see the module note.
            Neon8(unsafe { vreinterpretq_s16_u8(vqtbl1q_u8(vreinterpretq_u8_s16(self.0), p)) })
        }
        #[inline(always)]
        fn qadd(self, o: Self) -> Self {
            // SAFETY: see the module note.
            Neon8(unsafe { vqaddq_s16(self.0, o.0) })
        }
        #[inline(always)]
        fn qsub(self, o: Self) -> Self {
            // SAFETY: see the module note.
            Neon8(unsafe { vqsubq_s16(self.0, o.0) })
        }
        #[inline(always)]
        fn max(self, o: Self) -> Self {
            // SAFETY: see the module note.
            Neon8(unsafe { vmaxq_s16(self.0, o.0) })
        }
        #[inline(always)]
        fn mla(self, v: i16, sign: Self) -> Self {
            // SAFETY: see the module note.
            Neon8(unsafe { vmlaq_n_s16(self.0, sign.0, v) })
        }
        #[inline(always)]
        fn spread_first(self) -> Self {
            // SAFETY: see the module note.
            Neon8(unsafe { vdupq_laneq_s16::<0>(self.0) })
        }
        #[inline(always)]
        fn max_sum(a: Self, g: Self, b: Self) -> i32 {
            // SAFETY: see the module note.
            unsafe {
                let lo = vaddw_s16(
                    vaddl_s16(vget_low_s16(a.0), vget_low_s16(g.0)),
                    vget_low_s16(b.0),
                );
                let hi = vaddw_high_s16(vaddl_high_s16(a.0, g.0), b.0);
                vmaxvq_s32(vmaxq_s32(lo, hi))
            }
        }
    }

    /// Sixteen lanes as two vectors.
    #[derive(Clone, Copy)]
    pub struct Neon16(Neon8, Neon8);

    impl Lanes for Neon16 {
        const N: usize = 16;
        type Perm = (uint8x16_t, uint8x16_t);

        #[inline(always)]
        fn perm(idx: &[u8; 16]) -> Self::Perm {
            let mut low = *idx;
            for v in low.iter_mut() {
                *v &= 15;
            }
            (bytes(&low, 0), bytes(&low, 8))
        }
        #[inline(always)]
        fn splat(v: i16) -> Self {
            Neon16(Neon8::splat(v), Neon8::splat(v))
        }
        #[inline(always)]
        fn load(p: &[i16]) -> Self {
            let p = &p[..16];
            Neon16(Neon8::load(&p[..8]), Neon8::load(&p[8..]))
        }
        #[inline(always)]
        fn store(self, p: &mut [i16]) {
            let (a, b) = p[..16].split_at_mut(8);
            self.0.store(a);
            self.1.store(b);
        }
        #[inline(always)]
        fn permute(self, p: Self::Perm) -> Self {
            // SAFETY: see the module note.
            unsafe {
                let t = uint8x16x2_t(
                    vreinterpretq_u8_s16(self.0 .0),
                    vreinterpretq_u8_s16(self.1 .0),
                );
                Neon16(
                    Neon8(vreinterpretq_s16_u8(vqtbl2q_u8(t, p.0))),
                    Neon8(vreinterpretq_s16_u8(vqtbl2q_u8(t, p.1))),
                )
            }
        }
        #[inline(always)]
        fn qadd(self, o: Self) -> Self {
            Neon16(self.0.qadd(o.0), self.1.qadd(o.1))
        }
        #[inline(always)]
        fn qsub(self, o: Self) -> Self {
            Neon16(self.0.qsub(o.0), self.1.qsub(o.1))
        }
        #[inline(always)]
        fn max(self, o: Self) -> Self {
            Neon16(self.0.max(o.0), self.1.max(o.1))
        }
        #[inline(always)]
        fn mla(self, v: i16, sign: Self) -> Self {
            Neon16(self.0.mla(v, sign.0), self.1.mla(v, sign.1))
        }
        #[inline(always)]
        fn spread_first(self) -> Self {
            let v = self.0.spread_first();
            Neon16(v, v)
        }
        #[inline(always)]
        fn max_sum(a: Self, g: Self, b: Self) -> i32 {
            Neon8::max_sum(a.0, g.0, b.0).max(Neon8::max_sum(a.1, g.1, b.1))
        }
    }
}

/// The inputs of one constituent, each at least as long as the pass: systematic, first parity, second parity (`None` for a
/// constituent with one parity), a-priori.
#[derive(Clone, Copy)]
pub(super) struct Branch<'a> {
    pub sys: &'a [i16],
    pub p0: &'a [i16],
    pub p1: Option<&'a [i16]>,
    pub la: &'a [i16],
}

/// One max-log-MAP pass over `len` steps from state 0; the end state is 0 when `known`, else free. Writes the extrinsic of the
/// first `n` steps, scaled by `scale`, into `le`. `alpha` holds `(len + 1) * tr.ns` values. With `plain` the array lanes run on
/// every target; else the vector form of the target runs when there is one.
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    tr: &Trellis,
    plain: bool,
    alpha: &mut [i16],
    b: Branch,
    le: &mut [i16],
    n: usize,
    len: usize,
    known: bool,
    scale: (i32, i32),
) {
    #[cfg(target_arch = "aarch64")]
    if !plain {
        match tr.ns {
            8 => return siso::<neon::Neon8>(tr, alpha, b, le, n, len, known, scale),
            16 => return siso::<neon::Neon16>(tr, alpha, b, le, n, len, known, scale),
            _ => {}
        }
    }
    let _ = plain;
    match tr.ns {
        4 => siso::<Plain<4>>(tr, alpha, b, le, n, len, known, scale),
        8 => siso::<Plain<8>>(tr, alpha, b, le, n, len, known, scale),
        _ => siso::<Plain<16>>(tr, alpha, b, le, n, len, known, scale),
    }
}

/// The pass of [`run`] on lanes `V`.
#[allow(clippy::too_many_arguments)]
fn siso<V: Lanes>(
    tr: &Trellis,
    alpha: &mut [i16],
    b: Branch,
    le: &mut [i16],
    n: usize,
    len: usize,
    known: bool,
    scale: (i32, i32),
) {
    let w = V::N;
    let n = n.min(len);
    let sys = &b.sys[..len];
    let la = &b.la[..len];
    let p0 = &b.p0[..len];
    let p1 = b.p1.map(|p| &p[..len]);
    let le = &mut le[..n];
    let alpha = &mut alpha[..(len + 1) * w];
    let neg = V::splat(NEG);
    let zero = V::splat(0);
    // The channel values of step `t`: systematic with a-priori, the two parities.
    let inputs = |t: usize| -> (i16, i16, i16) {
        (sys[t].wrapping_add(la[t]), p0[t], p1.map_or(0, |p| p[t]))
    };

    // Forward: the metric of each state from its two branches in.
    let pred = [V::perm(&tr.pred[0]), V::perm(&tr.pred[1])];
    let pred_sys = [V::load(&tr.pred_sys[0]), V::load(&tr.pred_sys[1])];
    let pred_par = [
        [V::load(&tr.pred_par[0][0]), V::load(&tr.pred_par[0][1])],
        [V::load(&tr.pred_par[1][0]), V::load(&tr.pred_par[1][1])],
    ];
    let mut first = [NEG; 16];
    first[0] = 0;
    let mut a = V::load(&first);
    let mut rows = alpha.chunks_exact_mut(w);
    if let Some(row) = rows.next() {
        a.store(row);
    }
    for (t, row) in rows.enumerate() {
        let (su, v0, v1) = inputs(t);
        let g = |k: usize| {
            zero.mla(su, pred_sys[k])
                .mla(v0, pred_par[k][0])
                .mla(v1, pred_par[k][1])
        };
        let m = a
            .permute(pred[0])
            .qadd(g(0))
            .max(a.permute(pred[1]).qadd(g(1)))
            .max(neg);
        a = m.qsub(m.spread_first()).max(neg);
        a.store(row);
    }

    // Backward: the metric of each state from its two branches out, and the extrinsic of the data steps.
    let next = [V::perm(&tr.next[0]), V::perm(&tr.next[1])];
    let next_par = [
        [V::load(&tr.next_par[0][0]), V::load(&tr.next_par[0][1])],
        [V::load(&tr.next_par[1][0]), V::load(&tr.next_par[1][1])],
    ];
    let mut last = [if known { NEG } else { 0 }; 16];
    last[0] = 0;
    let mut bt = V::load(&last);
    for t in (0..len).rev() {
        let (su, v0, v1) = inputs(t);
        let g0 = V::splat(su).mla(v0, next_par[0][0]).mla(v1, next_par[0][1]);
        let g1 = V::splat(su.wrapping_neg())
            .mla(v0, next_par[1][0])
            .mla(v1, next_par[1][1]);
        let b0 = bt.permute(next[0]);
        let b1 = bt.permute(next[1]);
        if t < n {
            let a = V::load(&alpha[t * w..(t + 1) * w]);
            let m0 = V::max_sum(a, g0, b0);
            let m1 = V::max_sum(a, g1, b1);
            // (m0 - m1) is twice the posterior; the extrinsic removes the systematic and a-priori parts.
            let e = ((m0 - m1) / 2 - sys[t] as i32 - la[t] as i32) * scale.0 / scale.1;
            le[t] = e.clamp(-2000, 2000) as i16;
        }
        let m = b0.qadd(g0).max(b1.qadd(g1)).max(neg);
        bt = m.qsub(m.spread_first()).max(neg);
    }
}
