//! The MSC cell interleaver of DRM: the block permutation of each multiplex frame, and the long form that also delays cell `i`
//! by `i mod D` multiplex frames.
//!
//! Standard: ETSI ES 201 980 V4.3.1 clause 7.6.

use sdrkit_core::Complex32;
use sdrkit_util_bits::interleave::Permutation;
use std::collections::VecDeque;

/// The permutation of clause 7.6 over `n` cells (multiplier 5), as a transmit interleaver: `apply` produces output `i` from
/// input `Π(i)`.
pub fn cell_permutation(n: usize) -> Permutation {
    Permutation::drm(n, 5)
        .map(Permutation::inverse)
        .unwrap_or_else(|_| Permutation::identity(n))
}

/// The transmit side: `ẑ(n, i) = z(n - Γ(i), Π(i))` with `Γ(i) = i mod D` (`D` = 1 for the short interleaver).
#[derive(Clone, Debug)]
pub struct CellInterleaver {
    perm: Permutation,
    depth: usize,
    /// The permuted multiplex frames, newest first.
    history: VecDeque<Vec<Complex32>>,
    spare: Vec<Complex32>,
}

impl CellInterleaver {
    /// An interleaver of `n` cells per multiplex frame and depth `depth` (1 short; 5, or 6 in mode E, long).
    pub fn new(n: usize, depth: usize) -> CellInterleaver {
        CellInterleaver {
            perm: cell_permutation(n),
            depth: depth.max(1),
            history: VecDeque::with_capacity(depth.max(1) + 1),
            spare: vec![Complex32::default(); n],
        }
    }

    /// Interleaves one multiplex frame `z` into `out`. A cell whose frame was never given (the pre-roll) takes `fill(i)`.
    pub fn push(
        &mut self,
        z: &[Complex32],
        out: &mut Vec<Complex32>,
        mut fill: impl FnMut(usize) -> Complex32,
    ) {
        let n = self.perm.len();
        let mut x = std::mem::take(&mut self.spare);
        x.resize(n, Complex32::default());
        let _ = self
            .perm
            .apply(&z[..n.min(z.len())], &mut x[..n.min(z.len())]);
        self.history.push_front(x);
        if self.history.len() > self.depth {
            self.spare = self.history.pop_back().unwrap_or_default();
        } else {
            self.spare = vec![Complex32::default(); n];
        }
        for i in 0..n {
            let v = self
                .history
                .get(i % self.depth)
                .map_or_else(|| fill(i), |h| h[i]);
            out.push(v);
        }
    }
}

/// The receive side: collects the cells of each multiplex frame `n` from the frames `n` to `n + D - 1` that carry them.
#[derive(Clone, Debug)]
pub struct CellDeinterleaver {
    perm: Permutation,
    depth: usize,
    /// Frames being filled: (frame number, cells, weights, cells received).
    open: VecDeque<(u64, Vec<Complex32>, Vec<f32>, usize)>,
}

impl CellDeinterleaver {
    /// A deinterleaver of `n` cells per multiplex frame and depth `depth`.
    pub fn new(n: usize, depth: usize) -> CellDeinterleaver {
        CellDeinterleaver {
            perm: cell_permutation(n),
            depth: depth.max(1),
            open: VecDeque::with_capacity(depth.max(1) + 1),
        }
    }

    /// Cells per multiplex frame.
    pub fn cells(&self) -> usize {
        self.perm.len()
    }

    /// Forgets every frame being filled (after a gap).
    pub fn clear(&mut self) {
        self.open.clear();
    }

    /// Takes the received cells `zx` (with weights `wx`) of multiplex frame number `n`. When this completes frame `n - D + 1`,
    /// returns its cells and weights in their order before interleaving; `None` while it waits, and for a frame of which some
    /// cells came before the receiver started.
    pub fn push(
        &mut self,
        n: u64,
        zx: &[Complex32],
        wx: &[f32],
    ) -> Option<(Vec<Complex32>, Vec<f32>)> {
        let len = self.perm.len();
        if zx.len() < len || wx.len() < len {
            return None;
        }
        let d = self.depth as u64;
        if !self.open.iter().any(|e| e.0 == n) {
            self.open
                .push_back((n, vec![Complex32::default(); len], vec![0.0; len], 0));
        }
        for i in 0..len {
            let m = n.wrapping_sub((i as u64) % d);
            if let Some(e) = self.open.iter_mut().find(|e| e.0 == m) {
                // Output cell i of frame n carries input cell Π(i) of frame n - (i mod D).
                let p = self.perm.src(i).unwrap_or(i);
                e.1[p] = zx[i];
                e.2[p] = wx[i];
                e.3 += 1;
            }
        }
        let done = n.wrapping_sub(d - 1);
        let pos = self.open.iter().position(|e| e.0 == done);
        let out = pos.and_then(|p| self.open.remove(p));
        while self.open.len() > self.depth {
            self.open.pop_front();
        }
        match out {
            Some((_, z, w, got)) if got == len => Some((z, w)),
            _ => None,
        }
    }
}
