//! The T2-frame as cells: where L1-pre and L1-post sit in the P2 symbols, the one-dimensional cell addresses of the PLPs, the
//! slices and sub-slices of each PLP, and the dummy cells.
//!
//! Standard: ETSI EN 302 755 V1.4.1 clauses 8.3.2, 8.3.5, 8.3.6 and 8.3.8.

use crate::error::Rule;
use crate::l1::{PlpConfig, PlpDynamic, L1_PRE_CELLS};
use crate::pilots::PilotMaps;
use sdrkit_core::Complex32;
use sdrkit_util_bits::seq::DVB_DISPERSAL;

/// The cell geometry of one T2-frame: the active cells of each symbol and where the PLP addresses start.
#[derive(Clone, Debug, Default)]
pub struct FrameCells {
    /// Active cells of each symbol (C_P2, C_data, or C_FC).
    pub active: Vec<usize>,
    /// The cells of each P2 symbol taken by L1 ((D_L1pre + D_L1post) / N_P2).
    pub l1_per_p2: usize,
    /// N_P2.
    pub n_p2: usize,
    /// D_L1post, the L1-post cells.
    pub l1_post: usize,
    /// The first PLP address of each symbol.
    starts: Vec<usize>,
    /// D_PLP, the cells left for PLPs, auxiliary streams and dummy cells.
    pub d_plp: usize,
}

impl FrameCells {
    /// The geometry of a frame with these pilot maps and `l1_post` L1-post cells. Errors when L1 does not fit the P2 symbols
    /// or does not divide by N_P2.
    pub fn new(maps: &PilotMaps, l1_post: usize) -> Result<FrameCells, Rule> {
        let shape = maps.shape();
        let n_p2 = shape.n_p2();
        let l_f = shape.l_f();
        let d_l1 = L1_PRE_CELLS + l1_post;
        if !d_l1.is_multiple_of(n_p2) || !l1_post.is_multiple_of(n_p2) {
            return Err(Rule::L1NotDivisible);
        }
        let l1_per_p2 = d_l1 / n_p2;
        let active: Vec<usize> = (0..l_f).map(|l| maps.active_cells(l)).collect();
        if l1_per_p2 > active[0] {
            return Err(Rule::L1DoesNotFit);
        }
        let mut starts = Vec::with_capacity(l_f + 1);
        let mut a = 0usize;
        for (l, &n) in active.iter().enumerate() {
            starts.push(a);
            a += if l < n_p2 { n - l1_per_p2 } else { n };
        }
        starts.push(a);
        Ok(FrameCells {
            active,
            l1_per_p2,
            n_p2,
            l1_post,
            starts,
            d_plp: a,
        })
    }

    /// The symbol and data cell index of PLP address `a` (clause 8.3.6.2), or `None` past D_PLP.
    pub fn locate(&self, a: usize) -> Option<(usize, usize)> {
        if a >= self.d_plp {
            return None;
        }
        let l = self.starts.partition_point(|&s| s <= a) - 1;
        let off = if l < self.n_p2 { self.l1_per_p2 } else { 0 };
        Some((l, a - self.starts[l] + off))
    }

    /// The first PLP address of symbol `l` and the data cell index where it starts in that symbol.
    pub fn symbol_start(&self, l: usize) -> (usize, usize) {
        let off = if l < self.n_p2 { self.l1_per_p2 } else { 0 };
        (self.starts[l], off)
    }

    /// The symbol and cell of L1-pre cell `i` (row-wise zig-zag, clause 8.3.5).
    pub fn l1_pre_cell(&self, i: usize) -> (usize, usize) {
        (i % self.n_p2, i / self.n_p2)
    }

    /// The symbol and cell of L1-post cell `i` (after the L1-pre cells, clause 8.3.5).
    pub fn l1_post_cell(&self, i: usize) -> (usize, usize) {
        (i % self.n_p2, L1_PRE_CELLS / self.n_p2 + i / self.n_p2)
    }
}

/// Writes into `out` the cell address ranges of one PLP in one T2-frame: one range for common and type 1 PLPs, N_subslices
/// for type 2 (clauses 8.3.6.3.2 and 8.3.6.3.3).
pub fn plp_ranges(
    cfg: &PlpConfig,
    dynamic: &PlpDynamic,
    cells_per_block: usize,
    sub_slices: usize,
    sub_slice_interval: usize,
    out: &mut Vec<(usize, usize)>,
) {
    out.clear();
    let total = dynamic.num_blocks as usize * cells_per_block / cfg.p_i();
    if total == 0 {
        return;
    }
    let start = dynamic.plp_start as usize;
    if cfg.plp_type == 2 && sub_slices > 1 {
        let each = total / sub_slices;
        out.extend((0..sub_slices).map(|j| (start + j * sub_slice_interval, each)));
    } else {
        out.push((start, total));
    }
}

/// Whether a PLP is in T2-frame `frame_idx` (clause 8.2): (FRAME_IDX - FIRST_FRAME_IDX) mod FRAME_INTERVAL = 0.
pub fn plp_in_frame(cfg: &PlpConfig, frame_idx: u8) -> bool {
    let iv = cfg.frame_interval.max(1);
    frame_idx >= cfg.first_frame_idx && (frame_idx - cfg.first_frame_idx).is_multiple_of(iv)
}

/// Writes `n` dummy cells (clause 8.3.8): the BB scrambling sequence from its start, Re = 2 (1/2 - b), Im = 0.
pub fn dummy_cells(out: &mut [Complex32]) {
    let mut r = DVB_DISPERSAL;
    for c in out.iter_mut() {
        *c = Complex32::new(1.0 - 2.0 * r.bit() as f32, 0.0);
    }
}
