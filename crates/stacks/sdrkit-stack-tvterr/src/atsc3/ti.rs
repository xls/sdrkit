//! The time interleavers of an ATSC 3.0 PLP: what L1-Detail says of them, the sequences of the cell interleaver, and the
//! de-interleaver of the cells of one PLP from frame to frame.
//!
//! Standard: ATSC A/322:2023-03 clauses 7.1 and 9.3.

use super::l1::Plp;
use sdrkit_algo_fec::addrgen::AddressRule;
use sdrkit_algo_fec::interleave::DelayLanes;
use sdrkit_core::Complex32;

/// The rows of the convolutional time interleaver for depth code `depth` (`L1D_plp_CTI_depth`, table 9.26): 512, 724, 887
/// or 1024, and with `extended` interleaving 1254 for code 2 and 1448 for code 3. `None` for a reserved code.
pub fn cti_rows(depth: u8, extended: bool) -> Option<usize> {
    match (depth, extended) {
        (0, _) => Some(512),
        (1, _) => Some(724),
        (2, false) => Some(887),
        (3, false) => Some(1024),
        (2, true) => Some(1254),
        (3, true) => Some(1448),
        _ => None,
    }
}

/// The time interleaving of one PLP in one frame, as L1-Detail gives it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TiFrame {
    /// No time interleaver.
    #[default]
    None,
    /// The convolutional time interleaver.
    Cti {
        /// Its rows.
        rows: usize,
        /// The row that the first cell of the PLP in this subframe leaves (`L1D_plp_CTI_start_row`).
        start_row: usize,
        /// The cell of the PLP, counted from its first cell in this subframe, that is the first cell of a FEC block
        /// (`L1D_plp_CTI_fec_block_start`).
        fec_block_start: usize,
    },
    /// The hybrid time interleaver across subframes: each TI block is spread over `units` subframes.
    HtiAcross {
        /// The subframes that one TI block is spread over, 2 to 16.
        units: usize,
        /// The FEC blocks of the TI block that began in this subframe, in the one before, and so on.
        blocks: [u16; 16],
        /// The most FEC blocks of a TI block.
        blocks_max: usize,
        /// The cells of each FEC block are interleaved.
        cell_interleaver: bool,
    },
    /// The hybrid time interleaver within one subframe.
    Hti {
        /// The TI blocks of the subframe.
        ti_blocks: usize,
        /// The FEC blocks of the subframe.
        blocks: usize,
        /// The most FEC blocks that a subframe carries.
        blocks_max: usize,
        /// The cells of each FEC block are interleaved.
        cell_interleaver: bool,
    },
}

impl TiFrame {
    /// The subframes from the one whose cells before time interleaving the de-interleaver gives in this subframe to this
    /// one: `units - 1` for the hybrid time interleaver across subframes, else 0.
    pub fn subframes_late(&self) -> usize {
        match *self {
            TiFrame::HtiAcross { units, .. } => units.saturating_sub(1),
            _ => 0,
        }
    }

    /// The time interleaving that L1-Detail signals for `p`. `None` for a reserved value.
    pub fn of(p: &Plp) -> Option<TiFrame> {
        match p.ti_mode {
            0 => Some(TiFrame::None),
            1 => {
                let (depth, row) = p.cti?;
                let rows = cti_rows(depth, p.ti_extended == Some(true))?;
                (usize::from(row) < rows).then_some(TiFrame::Cti {
                    rows,
                    start_row: usize::from(row),
                    fec_block_start: p.cti_fec_block_start? as usize,
                })
            }
            2 => {
                let h = p.hti?;
                let blocks_max = usize::from(h.fec_blocks_max) + 1;
                let units = usize::from(h.ti_blocks) + 1;
                // One TI block in one subframe is the same with and without `inter_subframe`.
                if !h.inter_subframe || units == 1 {
                    let t = TiFrame::Hti {
                        ti_blocks: units,
                        blocks: usize::from(h.fec_blocks[0]) + 1,
                        blocks_max,
                        cell_interleaver: h.cell_interleaver,
                    };
                    return (h.fec_blocks[0] <= h.fec_blocks_max).then_some(t);
                }
                let mut blocks = [0u16; 16];
                for (count, &field) in blocks.iter_mut().zip(&h.fec_blocks[..units.min(16)]) {
                    if field > h.fec_blocks_max {
                        return None;
                    }
                    *count = field + 1;
                }
                Some(TiFrame::HtiAcross {
                    units,
                    blocks,
                    blocks_max,
                    cell_interleaver: h.cell_interleaver,
                })
            }
            _ => None,
        }
    }
}

/// The bits of the address of a cell in a FEC block of `n_cells` cells.
fn address_bits(n_cells: usize) -> u32 {
    usize::BITS - n_cells.saturating_sub(1).leading_zeros()
}

/// The basic sequence `L_0(q)` of the cell interleaver for FEC blocks of `n_cells` cells (clause 7.1.5.2): `n_cells` addresses
/// below `n_cells`, each once. `None` for a block size outside 1025 to 32768 cells.
pub fn cell_sequence(n_cells: usize) -> Option<Vec<u32>> {
    const STRAIGHT: [u32; 14] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
    let bits = address_bits(n_cells);
    let taps: &[u32] = match bits {
        11 => &[0, 3],
        12 => &[0, 2],
        13 => &[0, 1, 4, 6],
        14 => &[0, 1, 4, 5, 9, 11],
        15 => &[0, 1, 2, 12],
        _ => return None,
    };
    let rule = AddressRule {
        taps,
        perm: &STRAIGHT[..bits as usize - 1],
    };
    let mut out = Vec::with_capacity(n_cells);
    rule.addresses(n_cells, &mut out).ok()?;
    Some(out)
}

/// The shift `P(r)` of the cell interleaver for each of the first `count` FEC blocks of a TI block, into `out`: the values
/// of a counter with its bits reversed, without those of `n_cells` or more.
pub fn cell_shifts(n_cells: usize, count: usize, out: &mut Vec<u32>) {
    out.clear();
    let bits = address_bits(n_cells).max(1);
    let mut k = 0u32;
    while out.len() < count && n_cells > 0 {
        let v = k.reverse_bits() >> (32 - bits);
        k += 1;
        if (v as usize) < n_cells {
            out.push(v);
        }
    }
}

/// The FEC blocks of TI block `s` of `ti_blocks` when the subframe carries `blocks`: the last `blocks mod ti_blocks` TI blocks
/// hold one block more than the others.
pub fn ti_block_size(blocks: usize, ti_blocks: usize, s: usize) -> usize {
    blocks / ti_blocks + usize::from(s >= ti_blocks - blocks % ti_blocks)
}

/// Reads one TI block of `n` FEC blocks of `block` cells back from the twisted block interleaver of `columns` columns and
/// from the cell interleaver (clauses 7.1.5.2 and 7.1.5.4). `src` holds the cells as sent, without those of the virtual FEC
/// blocks in the first columns; `dst` receives the FEC blocks in order. The sender read row `R` of column
/// `(R + pass) mod columns` in each pass, and with `sequence` cell `q` of a block is the cell `L_r(q)` of its input.
fn untwist(
    src: (&[Complex32], &[f32]),
    dst: (&mut [Complex32], &mut [f32]),
    (block, columns, n): (usize, usize, usize),
    sequence: Option<&[u32]>,
    shifts: &[u32],
) {
    let spare = columns - n;
    let mut p = 0;
    for pass in 0..columns {
        let mut c = pass;
        for row in 0..block {
            if c >= spare {
                let b = c - spare;
                let q = match sequence {
                    Some(l) => {
                        let a = l[row] as usize + shifts[b] as usize;
                        if a >= block {
                            a - block
                        } else {
                            a
                        }
                    }
                    None => row,
                };
                dst.0[b * block + q] = src.0[p];
                dst.1[b * block + q] = src.1[p];
                p += 1;
            }
            c += 1;
            if c == columns {
                c = 0;
            }
        }
    }
}

/// The place, among the cells that leave the convolutional de-interleaver of `rows` rows in a subframe, of the cell that
/// `L1D_plp_CTI_fec_block_start` = `fec_block_start` names when the subframe starts at row `start_row` (clause 9.3.9.1):
/// that cell left row `r = (start_row + fec_block_start) mod rows` of the sender, and row `r` here delays it by
/// `(rows - 1 - r) rows` more cells. It may lie after the cells of the subframe.
pub fn cti_boundary(rows: usize, start_row: usize, fec_block_start: usize) -> usize {
    let r = (start_row + fec_block_start) % rows;
    fec_block_start + (rows - 1 - r) * rows
}

/// A cell that no input has filled: its weight erases it.
const ERASED: (Complex32, f32) = (Complex32::new(0.0, 0.0), 0.0);

/// What [`TimeDeinterleaver::frame`] gives: the cells of the PLP in the order of its FEC blocks.
#[derive(Debug)]
pub struct Deinterleaved<'a> {
    /// The cells.
    pub cells: &'a [Complex32],
    /// Their weights.
    pub weights: &'a [f32],
    /// The first cell of the first FEC block that begins in `cells`.
    pub block_start: Option<usize>,
    /// The cells do not follow those of the frame before: a FEC block that has begun is dropped.
    pub restart: bool,
    /// The cells of the frame left out before `cells`: those that the convolutional de-interleaver gives before it is
    /// full, up to the first FEC block after them.
    pub skipped: usize,
}

/// The time de-interleaver of one PLP. Its delay lines and buffers are made when a frame first needs them and kept.
#[derive(Default)]
pub struct TimeDeinterleaver {
    /// The rows and the delay lines of the convolutional de-interleaver.
    lanes: Option<(usize, DelayLanes<(Complex32, f32)>)>,
    /// The delay lines hold the cells of the frame before.
    running: bool,
    /// Cells taken since the delay lines were emptied.
    count: u64,
    cells: Vec<Complex32>,
    weights: Vec<f32>,
    /// The block size and the basic sequence of the cell interleaver for it.
    sequence: (usize, Vec<u32>),
    shifts: Vec<u32>,
    /// The TI blocks that arrive over several subframes: the cells as sent, their weights and the FEC blocks of each.
    ring: Vec<(Vec<Complex32>, Vec<f32>, usize)>,
    /// The subframes of a TI block, the cells of a FEC block and the most FEC blocks that `ring` is made for.
    layout: (usize, usize, usize),
    /// Subframes taken since `ring` started.
    age: usize,
    /// The subframe before gave a whole TI block.
    whole: bool,
}

impl TimeDeinterleaver {
    /// Forgets the cells of the frames before.
    pub fn restart(&mut self) {
        self.running = false;
        self.age = 0;
    }

    /// De-interleaves the cells of the PLP in one frame. `block` is the cells of a FEC block, `block_start` the first cell
    /// of the first block of a PLP without time interleaving, `follows` whether the frame begins where the last one ended.
    /// `None` when the cells are not what `ti` says.
    pub fn frame<'a>(
        &'a mut self,
        ti: &TiFrame,
        follows: bool,
        block: usize,
        cells: &'a [Complex32],
        weights: &'a [f32],
        block_start: Option<usize>,
    ) -> Option<Deinterleaved<'a>> {
        if block == 0 || cells.len() != weights.len() {
            return None;
        }
        if !matches!(ti, TiFrame::Cti { .. }) {
            self.running = false;
        }
        if !matches!(ti, TiFrame::HtiAcross { .. }) {
            self.age = 0;
        }
        match *ti {
            TiFrame::HtiAcross {
                units,
                blocks,
                blocks_max,
                cell_interleaver,
            } => self.across(
                (units, blocks, blocks_max),
                (cell_interleaver, follows),
                block,
                cells,
                weights,
            ),
            TiFrame::None => {
                Some(Deinterleaved {
                    cells,
                    weights,
                    block_start,
                    restart: !follows,
                    skipped: 0,
                })
            }
            TiFrame::Cti {
                rows,
                start_row,
                fec_block_start,
            } => self.convolutional(
                (rows, start_row, fec_block_start),
                follows,
                block,
                cells,
                weights,
            ),
            TiFrame::Hti {
                ti_blocks,
                blocks,
                blocks_max,
                cell_interleaver,
            } => {
                self.hybrid(
                    (ti_blocks, blocks, blocks_max),
                    (cell_interleaver, follows),
                    block,
                    cells,
                    weights,
                )
            }
        }
    }

    /// Clause 7.1.4: row `r` of the sender delays by `r` visits, so row `r` here delays by `rows - 1 - r`, and every cell
    /// leaves `rows (rows - 1)` cells after it entered the sender.
    fn convolutional<'a>(
        &'a mut self,
        (rows, start_row, fec_block_start): (usize, usize, usize),
        follows: bool,
        block: usize,
        cells: &[Complex32],
        weights: &[f32],
    ) -> Option<Deinterleaved<'a>> {
        if rows == 0 || start_row >= rows {
            return None;
        }
        if self.lanes.as_ref().is_none_or(|l| l.0 != rows) {
            self.lanes = Some((rows, DelayLanes::forney(rows as u32, 1, true, ERASED)));
            self.running = false;
        }
        let (_, lanes) = self.lanes.as_mut()?;
        if !follows || !self.running || lanes.lane() != start_row {
            lanes.reset(ERASED);
            for _ in 0..start_row {
                lanes.push(ERASED);
            }
            self.count = 0;
            self.running = true;
        }
        self.cells.clear();
        self.weights.clear();
        for (&z, &w) in cells.iter().zip(weights) {
            let (z, w) = lanes.push((z, w));
            self.cells.push(z);
            self.weights.push(w);
        }
        // The cells before `warm` left lines that no cell of the signal had filled.
        let depth = (rows * (rows - 1)) as u64;
        let warm = depth.saturating_sub(self.count) as usize;
        self.count += cells.len() as u64;
        let boundary = cti_boundary(rows, start_row, fec_block_start);
        let first = warm + (boundary + block - warm % block) % block;
        let n = self.cells.len();
        let from = if warm > 0 { first.min(n) } else { 0 };
        Some(Deinterleaved {
            cells: &self.cells[from..],
            weights: &self.weights[from..],
            block_start: (first < n).then_some(first - from),
            restart: warm > 0,
            skipped: from,
        })
    }

    /// Makes the basic sequence of the cell interleaver for `block` cells, when the PLP uses it.
    fn prepare(&mut self, cell_interleaver: bool, block: usize) -> Option<()> {
        if cell_interleaver && self.sequence.0 != block {
            self.sequence = (block, cell_sequence(block)?);
        }
        Some(())
    }

    /// Clause 7.1.5: each TI block of the subframe is read back from the twisted block interleaver and each FEC block from
    /// the cell interleaver.
    fn hybrid<'a>(
        &'a mut self,
        (ti_blocks, blocks, blocks_max): (usize, usize, usize),
        (cell_interleaver, follows): (bool, bool),
        block: usize,
        cells: &[Complex32],
        weights: &[f32],
    ) -> Option<Deinterleaved<'a>> {
        if ti_blocks == 0 || blocks > blocks_max || cells.len() != blocks * block {
            return None;
        }
        self.prepare(cell_interleaver, block)?;
        let most = blocks_max * block;
        self.cells.reserve(most.saturating_sub(self.cells.len()));
        self.weights
            .reserve(most.saturating_sub(self.weights.len()));
        self.cells.clear();
        self.cells.resize(cells.len(), Complex32::default());
        self.weights.clear();
        self.weights.resize(cells.len(), 0.0);
        let columns = blocks_max.div_ceil(ti_blocks);
        let mut base = 0;
        for s in 0..ti_blocks {
            let n = ti_block_size(blocks, ti_blocks, s);
            if n > columns {
                return None;
            }
            cell_shifts(block, if cell_interleaver { n } else { 0 }, &mut self.shifts);
            let r = base..base + n * block;
            untwist(
                (&cells[r.clone()], &weights[r.clone()]),
                (&mut self.cells[r.clone()], &mut self.weights[r]),
                (block, columns, n),
                cell_interleaver.then_some(&self.sequence.1[..]),
                &self.shifts,
            );
            base += n * block;
        }
        Some(Deinterleaved {
            cells: &self.cells,
            weights: &self.weights,
            block_start: Some(0),
            restart: !follows,
            skipped: 0,
        })
    }

    /// Clauses 7.1.5.5 and 7.1.5.6: the output of the twisted block interleaver for one TI block is cut into as many groups
    /// as a FEC block has cells, group `g` is sent `g mod units` subframes late, and a subframe carries its groups in the
    /// order of `g`. The TI block that began `units - 1` subframes ago is whole after this subframe and is read back.
    fn across<'a>(
        &'a mut self,
        (units, counts, blocks_max): (usize, [u16; 16], usize),
        (cell_interleaver, follows): (bool, bool),
        block: usize,
        cells: &[Complex32],
        weights: &[f32],
    ) -> Option<Deinterleaved<'a>> {
        if !(2..=16).contains(&units) {
            return None;
        }
        self.prepare(cell_interleaver, block)?;
        if self.layout != (units, block, blocks_max) || self.ring.len() != units {
            let size = block * blocks_max;
            self.ring = (0..units)
                .map(|_| (vec![Complex32::default(); size], vec![0.0; size], 0))
                .collect();
            self.layout = (units, block, blocks_max);
            self.cells = Vec::with_capacity(size);
            self.weights = Vec::with_capacity(size);
            self.age = 0;
        }
        let count = |j: usize| usize::from(counts[j]);
        // The TI block of each earlier subframe has the FEC blocks that its own subframe gave.
        let known = |age: usize| (1..units.min(age + 1)).all(|j| self.ring[(age - j) % units].2 == count(j));
        if !follows || !known(self.age) {
            self.age = 0;
        }
        let (small, large) = (block / units, block % units);
        let total: usize = (0..units)
            .map(|j| (small + usize::from(j < large)) * count(j))
            .sum();
        if total != cells.len() || (0..units).any(|j| count(j) > blocks_max) {
            self.age = 0;
            return None;
        }
        let age = self.age;
        self.ring[age % units].2 = count(0);
        let mut p = 0;
        for g in 0..block {
            let j = g % units;
            let n = count(j);
            if age >= j {
                let to = &mut self.ring[(age - j) % units];
                to.0[g * n..(g + 1) * n].copy_from_slice(&cells[p..p + n]);
                to.1[g * n..(g + 1) * n].copy_from_slice(&weights[p..p + n]);
            }
            p += n;
        }
        self.age += 1;
        if age + 1 < units {
            self.whole = false;
            return Some(Deinterleaved {
                cells: &[],
                weights: &[],
                block_start: None,
                restart: true,
                skipped: 0,
            });
        }
        let from = &self.ring[(age + 1) % units];
        let n = from.2;
        self.cells.clear();
        self.cells.resize(n * block, Complex32::default());
        self.weights.clear();
        self.weights.resize(n * block, 0.0);
        cell_shifts(block, if cell_interleaver { n } else { 0 }, &mut self.shifts);
        untwist(
            (&from.0[..n * block], &from.1[..n * block]),
            (&mut self.cells, &mut self.weights),
            (block, blocks_max, n),
            cell_interleaver.then_some(&self.sequence.1[..]),
            &self.shifts,
        );
        let restart = !self.whole;
        self.whole = true;
        Some(Deinterleaved {
            cells: &self.cells,
            weights: &self.weights,
            block_start: Some(0),
            restart,
            skipped: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::gen::{convolutional_interleaver, twisted_block, DelayLine};
    use super::super::l1::Hti;
    use super::*;

    fn cell(i: usize) -> Complex32 {
        Complex32::new(i as f32, 1.0)
    }

    /// Clause 7.1.4 and table 9.26: the rows of each depth code without and with extended interleaving, and the memory of
    /// the deepest lines.
    #[test]
    fn the_depths_are_those_of_the_table() {
        let rows = |extended| -> Vec<_> { (0..8).map(|d| cti_rows(d, extended)).collect() };
        let reserved = [None; 4];
        assert_eq!(rows(false)[..4], [Some(512), Some(724), Some(887), Some(1024)]);
        assert_eq!(rows(true)[..4], [Some(512), Some(724), Some(1254), Some(1448)]);
        assert_eq!((&rows(false)[4..], &rows(true)[4..]), (&reserved[..], &reserved[..]));
        assert_eq!(1024 * 1023 / 2, 523_776);
        assert_eq!(1448 * 1447 / 2, 1_047_628);
    }

    #[test]
    fn l1_detail_gives_the_time_interleaving_of_a_plp() {
        let cti = Plp {
            ti_mode: 1,
            cti_fec_block_start: Some(70_000),
            cti: Some((1, 700)),
            ..Plp::default()
        };
        assert_eq!(
            TiFrame::of(&cti),
            Some(TiFrame::Cti {
                rows: 724,
                start_row: 700,
                fec_block_start: 70_000
            })
        );
        let refuse = |f: &dyn Fn(&mut Plp)| {
            let mut p = cti;
            f(&mut p);
            TiFrame::of(&p)
        };
        assert_eq!(refuse(&|p| p.cti = Some((1, 724))), None);
        assert_eq!(refuse(&|p| p.cti = Some((4, 0))), None);
        assert_eq!(refuse(&|p| p.cti = Some((3, 1024))), None);
        assert_eq!(
            refuse(&|p| {
                p.cti = Some((5, 0));
                p.ti_extended = Some(true);
            }),
            None
        );
        assert_eq!(refuse(&|p| p.ti_mode = 3), None);
        assert!(refuse(&|p| {
            p.cti = Some((3, 1447));
            p.ti_extended = Some(true);
        })
        .is_some());
        let hti = Plp {
            ti_mode: 2,
            hti: Some(Hti {
                ti_blocks: 2,
                fec_blocks_max: 9,
                fec_blocks: [7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                cell_interleaver: true,
                ..Hti::default()
            }),
            ..Plp::default()
        };
        assert_eq!(
            TiFrame::of(&hti),
            Some(TiFrame::Hti {
                ti_blocks: 3,
                blocks: 8,
                blocks_max: 10,
                cell_interleaver: true
            })
        );
        // Across subframes: the counts of the TI blocks that began in this subframe and in the two before it.
        let mut across = hti;
        let h = across.hti.as_mut().unwrap();
        h.inter_subframe = true;
        h.fec_blocks[..3].copy_from_slice(&[7, 9, 0]);
        let mut blocks = [0u16; 16];
        blocks[..3].copy_from_slice(&[8, 10, 1]);
        assert_eq!(
            TiFrame::of(&across),
            Some(TiFrame::HtiAcross {
                units: 3,
                blocks,
                blocks_max: 10,
                cell_interleaver: true
            })
        );
        across.hti.as_mut().unwrap().fec_blocks[1] = 10;
        assert_eq!(TiFrame::of(&across), None);
        // One subframe per TI block is the interleaver within a subframe.
        let h = across.hti.as_mut().unwrap();
        (h.ti_blocks, h.fec_blocks[1]) = (0, 0);
        assert_eq!(
            TiFrame::of(&across),
            Some(TiFrame::Hti {
                ti_blocks: 1,
                blocks: 8,
                blocks_max: 10,
                cell_interleaver: true
            })
        );
        assert_eq!(TiFrame::of(&Plp::default()), Some(TiFrame::None));
    }

    /// Clause 7.1.4: on send, row 0 passes its cell at once and row `r` holds a cell for `r` turns of the rows.
    #[test]
    fn the_sender_delays_row_r_by_r_turns() {
        let rows = 512;
        let mut tx = convolutional_interleaver(rows);
        let n = 3 * rows * rows;
        let out: Vec<Complex32> = (0..n).map(|i| tx.push(cell(i + 1))).collect();
        for i in (0..n - rows * rows).step_by(997) {
            assert_eq!(out[i + i % rows * rows], cell(i + 1), "cell {i}");
        }
        assert_eq!(out[0], cell(1));
        assert_eq!(out[1], Complex32::default());
        assert_eq!(out[rows + 1], cell(2));
        assert_eq!(tx.lane(), 0);
    }

    /// Clause 7.1.5.4 with the worked example of A/327: FEC blocks `a0 .. d0` and `e0 .. h0` of 4 cells where a TI block
    /// holds 3 at most are sent as b0 g0 a0 f0 d0 e0 c0 h0.
    #[test]
    fn the_twisted_block_interleaver_reads_as_the_clause_says() {
        let x: Vec<Complex32> = (0..8).map(cell).collect();
        let (a, b, c, d, e, f, g, h) = (x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7]);
        assert_eq!(twisted_block(&x, 4, 3, false), [b, g, a, f, d, e, c, h]);
        // A full memory of one column is read as it was written, and one of two columns is not made odd.
        assert_eq!(twisted_block(&x[..4], 4, 1, false), [a, b, c, d]);
        assert_eq!(twisted_block(&x, 4, 2, false), [a, f, c, h, e, b, g, d]);
        let mut rx = TimeDeinterleaver::default();
        let ti = TiFrame::Hti {
            ti_blocks: 1,
            blocks: 2,
            blocks_max: 3,
            cell_interleaver: false,
        };
        let sent = [b, g, a, f, d, e, c, h];
        let got = rx.frame(&ti, true, 4, &sent, &[1.0; 8], None).unwrap();
        assert_eq!(got.cells, &x[..]);
    }

    /// Clause 7.1.5.2: the sender reads a FEC block in the order of the sequence, so cell `q` sent is cell `L_r(q)` of the
    /// block, and the shift of the second block of a TI block is the second value of the counter.
    #[test]
    fn the_cell_interleaver_reads_in_the_order_of_the_sequence() {
        let block = 1350;
        let x: Vec<Complex32> = (0..2 * block).map(cell).collect();
        let l = cell_sequence(block).unwrap();
        // Two columns and two blocks: pass 0 reads the even rows of the first block and the odd rows of the second.
        let out = twisted_block(&x, block, 2, true);
        assert_eq!(out[0], cell(l[0] as usize));
        assert_eq!(out[2], cell(l[2] as usize));
        assert_eq!(out[1], cell(block + (l[1] as usize + 1024) % block));
    }

    /// The cells of a sender at every depth come back in order `rows (rows - 1)` cells late, in frames of any size, from any
    /// start row, and the block start follows the signalled cell.
    #[test]
    fn the_convolutional_de_interleaver_gives_the_cells_back_at_every_depth() {
        for (depth, extended, frame, block) in [
            (0u8, false, 300_000usize, 32_400usize),
            (1, true, 250_001, 8100),
            (2, false, 400_000, 16_200),
            (3, false, 482_596, 32_400),
            (2, true, 500_000, 32_400),
            (3, true, 700_000, 8100),
        ] {
            let rows = cti_rows(depth, extended).unwrap();
            let delay = rows * (rows - 1);
            let mut tx = convolutional_interleaver(rows);
            // The sender ran for 12345 cells before the first frame that the receiver takes.
            let before = 12_345;
            for i in 0..before {
                tx.push(cell(i));
            }
            let mut rx = TimeDeinterleaver::default();
            let frames = delay / frame + 3;
            let (mut sent, mut seen, mut whole) = (before, 0usize, 0usize);
            for f in 0..frames {
                let start_row = tx.lane();
                // The first FEC block that begins in the frame, where its first cell leaves the sender.
                let left = (block - sent % block) % block;
                let start = left + (start_row + left) % rows * rows;
                let x: Vec<Complex32> = (0..frame).map(|i| tx.push(cell(sent + i))).collect();
                let w = vec![1.0f32; frame];
                let ti = TiFrame::Cti {
                    rows,
                    start_row,
                    fec_block_start: start,
                };
                let d = rx.frame(&ti, f > 0, block, &x, &w, None).unwrap();
                assert_eq!(d.restart, seen < delay, "depth {depth} frame {f}");
                let offset = frame - d.cells.len();
                for (i, (z, w)) in d.cells.iter().zip(d.weights).enumerate() {
                    let at = seen + offset + i;
                    assert_eq!((*z, *w), (cell(before + at - delay), 1.0), "depth {depth}");
                }
                if let Some(s) = d.block_start {
                    assert_eq!((before + seen + offset + s - delay) % block, 0);
                    assert!(s < block);
                    whole += (d.cells.len() - s) / block;
                }
                seen += frame;
                sent += frame;
            }
            assert!(whole > 0, "depth {depth}");
        }
    }

    /// A frame that does not follow, or whose start row is not where the last frame left off, starts the lines again.
    #[test]
    fn a_gap_starts_the_convolutional_de_interleaver_again() {
        let rows = 512;
        let ti = |start_row| TiFrame::Cti {
            rows,
            start_row,
            fec_block_start: 0,
        };
        let x = vec![cell(1); 300_000];
        let w = vec![1.0f32; 300_000];
        let mut rx = TimeDeinterleaver::default();
        let restarts = |rx: &mut TimeDeinterleaver, row, follows| {
            rx.frame(&ti(row), follows, 1350, &x, &w, None)
                .unwrap()
                .restart
        };
        assert!(restarts(&mut rx, 0, false));
        let next = 300_000 % rows;
        assert!(!restarts(&mut rx, next, true));
        let next = 600_000 % rows;
        assert!(restarts(&mut rx, next + 1, true));
        // One frame holds more cells than the lines: the frame after a new start is whole again.
        assert!(!restarts(&mut rx, (next + 1 + 300_000) % rows, true));
        assert!(restarts(&mut rx, (next + 1 + 600_000) % rows, false));
        assert!(rx.frame(&ti(rows), true, 1350, &x, &w, None).is_none());
    }

    /// Clause 7.1.5.2: the basic sequence holds each address once and starts 0, 2^(bits - 1), 1; the shifts are the counter
    /// with its bits reversed.
    #[test]
    fn the_cell_interleaver_sequences_are_permutations() {
        for n in [
            1350, 1620, 2025, 2700, 4050, 5400, 6480, 8100, 10_800, 16_200, 32_400,
        ] {
            let l = cell_sequence(n).unwrap();
            let mut seen = vec![false; n];
            l.iter().for_each(|&a| seen[a as usize] = true);
            assert!(l.len() == n && seen.iter().all(|&s| s), "{n}");
            assert_eq!(l[0], 0);
        }
        assert_eq!(cell_sequence(8100).unwrap()[..3], [0, 4096, 1]);
        assert!(cell_sequence(1000).is_none());
        assert!(cell_sequence(40_000).is_none());
        let mut p = Vec::new();
        cell_shifts(1350, 6, &mut p);
        // 11 bits: 0, 1024, 512, (1536 is left out), 256, 1280, 768.
        assert_eq!(p, [0, 1024, 512, 256, 1280, 768]);
        cell_shifts(10_800, 8, &mut p);
        assert_eq!(p, [0, 8192, 4096, 2048, 10_240, 6144, 1024, 9216]);
    }

    /// Clause 7.1.5.4: the TI blocks of a subframe.
    #[test]
    fn ti_blocks_follow_the_clause() {
        let sizes = |blocks, n| -> Vec<usize> {
            (0..n).map(|s| ti_block_size(blocks, n, s)).collect()
        };
        assert_eq!(sizes(8, 3), [2, 3, 3]);
        assert_eq!(sizes(9, 3), [3, 3, 3]);
        assert_eq!(sizes(5, 1), [5]);
    }

    /// The cells of the sender's twisted block and cell interleavers come back in order, with and without virtual FEC
    /// blocks, in one and in several TI blocks.
    #[test]
    fn the_hybrid_de_interleaver_gives_the_cells_back() {
        let block = 1350;
        for (ti_blocks, blocks, blocks_max, ci) in [
            (1usize, 5usize, 5usize, false),
            (1, 4, 4, true),
            (1, 3, 8, true),
            (3, 8, 10, true),
            (2, 1, 3, false),
            (4, 16, 16, true),
        ] {
            let mut x = Vec::new();
            let mut at = 0;
            for s in 0..ti_blocks {
                let n = ti_block_size(blocks, ti_blocks, s);
                let cells: Vec<Complex32> = (at..at + n * block).map(cell).collect();
                x.extend(twisted_block(
                    &cells,
                    block,
                    blocks_max.div_ceil(ti_blocks),
                    ci,
                ));
                at += n * block;
            }
            assert_eq!(x.len(), blocks * block);
            let w = vec![2.0f32; x.len()];
            let ti = TiFrame::Hti {
                ti_blocks,
                blocks,
                blocks_max,
                cell_interleaver: ci,
            };
            let mut rx = TimeDeinterleaver::default();
            let d = rx.frame(&ti, true, block, &x, &w, None).unwrap();
            assert_eq!((d.block_start, d.restart), (Some(0), false));
            assert!(
                d.cells.iter().enumerate().all(|(i, z)| *z == cell(i)),
                "{ti_blocks} {blocks} {blocks_max} {ci}"
            );
            assert!(d.weights.iter().all(|&w| w == 2.0));
            // The cells of a subframe with another count of blocks are refused.
            assert!(rx
                .frame(&ti, true, block, &x[block..], &w[block..], None)
                .is_none());
        }
    }

    /// TI blocks spread over 2, 3 and 16 subframes with a count of FEC blocks that changes from subframe to subframe: each
    /// TI block comes back whole once its last subframe is in, and a lost subframe starts the wait again.
    #[test]
    fn the_hybrid_de_interleaver_across_subframes_gives_the_cells_back() {
        let block = 1350;
        for (units, blocks_max, ci) in [(2usize, 4usize, false), (3, 5, true), (16, 2, true), (7, 3, false)] {
            let start: Vec<Complex32> = (0..DelayLine::start_cells(units, block))
                .map(|i| cell(1_000_000 + i))
                .collect();
            assert_eq!(
                start.len(),
                (units * (units - 1) * (block / units)
                    + (block % units) * (block % units).saturating_sub(1))
                    / 2
            );
            let mut tx = DelayLine::new(units, block, blocks_max, &start);
            let mut rx = TimeDeinterleaver::default();
            let mut history = vec![1usize; units];
            let mut sent: Vec<(usize, usize)> = Vec::new();
            let (mut made, mut whole) = (0, 0);
            for m in 0..3 * units + 4 {
                let n = 1 + (m * 7 + units) % blocks_max;
                let fec: Vec<Complex32> = (made..made + n * block).map(cell).collect();
                sent.push((made, n));
                made += n * block;
                let x = tx.subframe(&twisted_block(&fec, block, blocks_max, ci), n);
                history.insert(0, n);
                history.truncate(units);
                let mut counts = [0u16; 16];
                for (c, &h) in counts.iter_mut().zip(&history) {
                    *c = h as u16;
                }
                let ti = TiFrame::HtiAcross {
                    units,
                    blocks: counts,
                    blocks_max,
                    cell_interleaver: ci,
                };
                // Subframe `2 units` is lost: the receiver starts again with the next one.
                let lost = 2 * units;
                if m == lost {
                    continue;
                }
                let w = vec![1.0f32; x.len()];
                let d = rx.frame(&ti, m > 0 && m != lost + 1, block, &x, &w, None).unwrap();
                let since = if m > lost { m - lost - 1 } else { m };
                if since + 1 < units {
                    assert!(d.cells.is_empty() && d.restart, "{units} subframe {m}");
                    continue;
                }
                let (first, n) = sent[m + 1 - units];
                assert_eq!(d.cells.len(), n * block);
                assert_eq!(d.restart, since + 1 == units);
                assert!(
                    d.cells.iter().enumerate().all(|(i, z)| *z == cell(first + i)),
                    "{units} subframe {m}"
                );
                assert!(d.weights.iter().all(|&w| w == 1.0));
                whole += 1;
            }
            // Cells that are not what the counts say are refused.
            let ti = TiFrame::HtiAcross {
                units,
                blocks: [1; 16],
                blocks_max,
                cell_interleaver: ci,
            };
            let x = vec![cell(0); block + 1];
            assert!(rx.frame(&ti, true, block, &x, &vec![1.0; block + 1], None).is_none());
            assert_eq!(whole, units + 5, "{units}");
        }
    }
}
