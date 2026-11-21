//! The FEC of the DVB-T2 receive chain: the cells of one Interleaving Frame of a PLP to its BBFRAMEs. Time and cell
//! de-interleaving, de-mapping, LDPC and BCH decoding, descrambling.
//!
//! Standard: ETSI EN 302 755 V1.4.1 clause 6.

use crate::bicm::{BlockResult, Coding, Decoder};
use crate::interleave::{time_deinterleave, CellInterleaver};
use crate::rx::BbFrameOut;
use sdrkit_core::Complex32;
use sdrkit_graph::Pool;
use sdrkit_proto_ipcast::bbheader::bb_descramble;
use sdrkit_proto_ipcast::components::PlpPair;
use sdrkit_util_bits::pack::{pack_into, Pad};
use sdrkit_util_bits::BitOrder;
use std::sync::Arc;

/// The equalised cells of one Interleaving Frame of a PLP, in the order of their addresses (the item `dvbt2.cells`).
#[derive(Clone, Debug, Default)]
pub struct IfCells {
    /// The PLP.
    pub plp: u8,
    /// Its coding. `None` only in an empty item.
    pub coding: Option<Coding>,
    /// The TI-blocks of the Interleaving Frame. 0 without time interleaving.
    pub n_ti: usize,
    /// The FEC blocks of the Interleaving Frame.
    pub blocks: usize,
    /// Signal time of the T2-frame that ended it, s.
    pub time: f64,
    /// The data PLP and the common PLP that the receive chain decodes.
    pub pair: PlpPair,
    /// The cells, at least `blocks` times the cells of a FEC block.
    pub z: Arc<[Complex32]>,
    /// The weight of each cell.
    pub w: Arc<[f32]>,
}

impl sdrkit_graph::Item for IfCells {
    const KIND: &'static str = "dvbt2.cells";
}

/// What the decoder counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FecCounts {
    /// FEC blocks that the BCH decoder accepted.
    pub ok: u64,
    /// FEC blocks that failed.
    pub bad: u64,
    /// Bits the BCH decoder corrected.
    pub corrected: u64,
    /// The LDPC iterations of the blocks since [`FecCounts::start_mean`], and those blocks.
    pub iterations: (u64, u64),
}

impl FecCounts {
    /// Starts the mean of the LDPC iterations again.
    pub fn start_mean(&mut self) {
        self.iterations = (0, 0);
    }

    /// The mean LDPC iterations of the blocks since [`FecCounts::start_mean`]. `None` without a block.
    pub fn mean_iterations(&self) -> Option<f32> {
        (self.iterations.1 > 0).then(|| self.iterations.0 as f32 / self.iterations.1 as f32)
    }
}

/// True if the receive chain can decode `coding`.
pub fn supported(coding: Coding) -> bool {
    Decoder::new(coding, 1).is_ok() && CellInterleaver::new(coding.cells()).is_ok()
}

/// A run of FEC blocks for one thread of the worker pool: a decoder, the de-interleaved cells and weights of the
/// blocks, and the bits and the result of each block. `None` for a block that the decoder refused.
struct Batch {
    dec: Decoder,
    z: Vec<Complex32>,
    w: Vec<f32>,
    bits: Vec<u8>,
    res: Vec<Option<BlockResult>>,
}

impl Batch {
    /// Decodes the blocks of `nc` cells into `k` bits each.
    fn decode(&mut self, nc: usize, k: usize) {
        let n = self.z.len() / nc;
        self.bits.resize(n * k, 0);
        self.res.clear();
        for ((z, w), bits) in self
            .z
            .chunks_exact(nc)
            .zip(self.w.chunks_exact(nc))
            .zip(self.bits.chunks_exact_mut(k))
        {
            self.res.push(self.dec.decode(z, w, bits).ok());
        }
    }
}

/// The decoder state of one PLP.
struct PlpFec {
    id: u8,
    coding: Coding,
    dec: Decoder,
    ci: CellInterleaver,
    bits: Vec<u8>,
    bytes: Vec<u8>,
    tz: Vec<Complex32>,
    tw: Vec<f32>,
    bz: Vec<Complex32>,
    bw: Vec<f32>,
    /// The FEC blocks of the Interleaving Frame in work: the block and its number within its TI-block.
    order: Vec<(usize, usize)>,
    /// The parts of the blocks that the worker pool decodes, one decoder each.
    batches: Vec<Batch>,
}

impl PlpFec {
    fn new(id: u8, coding: Coding, max_iter: u32) -> Option<PlpFec> {
        let dec = Decoder::new(coding, max_iter).ok()?;
        let ci = CellInterleaver::new(coding.cells()).ok()?;
        let k = coding.k_bch();
        Some(PlpFec {
            id,
            coding,
            dec,
            ci,
            bits: vec![0; k],
            bytes: vec![0; k / 8],
            tz: Vec::new(),
            tw: Vec::new(),
            bz: vec![Complex32::default(); coding.cells()],
            bw: vec![0.0; coding.cells()],
            order: Vec::new(),
            batches: Vec::new(),
        })
    }
}

/// The FEC decoders of the PLPs of one multiplex.
pub struct T2Fec {
    max_iter: u32,
    plps: Vec<PlpFec>,
    /// What the decoders counted since the start.
    pub counts: FecCounts,
}

impl T2Fec {
    /// Decoders with at most `max_iter` LDPC iterations.
    pub fn new(max_iter: u32) -> T2Fec {
        T2Fec {
            max_iter,
            plps: Vec::new(),
            counts: FecCounts::default(),
        }
    }

    /// Decodes the FEC blocks of `cells` and gives each BBFRAME to `out`, descrambled, in the order of the blocks.
    /// With `pool`, its threads share the blocks: the output does not depend on it.
    pub fn decode(
        &mut self,
        cells: &IfCells,
        pool: Option<&Pool>,
        out: &mut impl FnMut(BbFrameOut<'_>),
    ) {
        let Some(coding) = cells.coding else { return };
        let nc = coding.cells();
        let total = cells.blocks * nc;
        if total == 0 || cells.z.len() < total || cells.w.len() < total {
            return;
        }
        let at = match self
            .plps
            .iter()
            .position(|p| p.id == cells.plp && p.coding == coding)
        {
            Some(i) => i,
            None => {
                let Some(p) = PlpFec::new(cells.plp, coding, self.max_iter) else {
                    return;
                };
                self.plps.retain(|q| q.id != cells.plp);
                self.plps.push(p);
                self.plps.len() - 1
            }
        };
        let p = &mut self.plps[at];
        let counts = &mut self.counts;
        let (n_ti, blocks, time) = (cells.n_ti, cells.blocks, cells.time);
        p.tz.resize(total, Complex32::default());
        p.tw.resize(total, 0.0);
        if n_ti == 0 {
            p.tz.copy_from_slice(&cells.z[..total]);
            p.tw.copy_from_slice(&cells.w[..total]);
        } else {
            let mut at = 0;
            for s in 0..n_ti {
                let base = blocks / n_ti;
                let extra = blocks % n_ti;
                let nfec = if s < n_ti - extra { base } else { base + 1 };
                let len = nfec * nc;
                time_deinterleave(nc, nfec, &cells.z[at..at + len], &mut p.tz[at..at + len]);
                time_deinterleave(nc, nfec, &cells.w[at..at + len], &mut p.tw[at..at + len]);
                at += len;
            }
        }
        // FEC blocks: r counts within each TI-block (0 for every block without time interleaving).
        let mut r = 0usize;
        let mut s = 0usize;
        let mut in_block = blocks.checked_div(n_ti).unwrap_or(usize::MAX);
        p.order.clear();
        for b in 0..blocks {
            if n_ti > 0 && r == in_block {
                s += 1;
                r = 0;
                let base = blocks / n_ti;
                let extra = blocks % n_ti;
                in_block = if s < n_ti - extra { base } else { base + 1 };
            }
            if n_ti > 0 && in_block == 0 {
                continue;
            }
            p.order.push((b, if n_ti == 0 { 0 } else { r }));
            r += 1;
        }
        let k = p.bits.len();
        let id = p.id;
        let mut emit = |res: BlockResult, bits: &[u8], bytes: &mut Vec<u8>| {
            counts.iterations.0 += u64::from(res.iterations);
            counts.iterations.1 += 1;
            if res.bch_ok {
                counts.ok += 1;
                counts.corrected += u64::from(res.bch_corrected);
            } else {
                counts.bad += 1;
            }
            // `bytes` holds K_bch / 8 bytes for the K_bch bits, so the packing cannot fail.
            let _ = pack_into(bits, BitOrder::Msb, Pad::Zero, bytes);
            bb_descramble(bytes);
            out(BbFrameOut {
                plp: id,
                bytes,
                ok: res.bch_ok,
                time,
            });
        };
        let helpers = pool.map_or(0, |pool| {
            pool.workers().min(p.order.len().saturating_sub(1))
        });
        if let (Some(pool), true) = (pool, helpers > 0) {
            let per = p.order.len().div_ceil(helpers + 1);
            let parts = p.order.len().div_ceil(per);
            while p.batches.len() < parts {
                let Ok(dec) = Decoder::new(p.coding, self.max_iter) else {
                    break;
                };
                p.batches.push(Batch {
                    dec,
                    z: Vec::new(),
                    w: Vec::new(),
                    bits: Vec::new(),
                    res: Vec::new(),
                });
            }
            if p.batches.len() >= parts {
                let mut work: Vec<Batch> = p.batches.drain(..parts).collect();
                for (batch, blocks) in work.iter_mut().zip(p.order.chunks(per)) {
                    batch.z.resize(blocks.len() * nc, Complex32::default());
                    batch.w.resize(blocks.len() * nc, 0.0);
                    for (i, &(b, rr)) in blocks.iter().enumerate() {
                        let at = i * nc..(i + 1) * nc;
                        p.ci.deinterleave(
                            rr,
                            &p.tz[b * nc..(b + 1) * nc],
                            &mut batch.z[at.clone()],
                        );
                        p.ci.deinterleave(rr, &p.tw[b * nc..(b + 1) * nc], &mut batch.w[at]);
                    }
                }
                let done = pool.map(work, move |_, mut batch| {
                    batch.decode(nc, k);
                    batch
                });
                for batch in &done {
                    for (res, bits) in batch.res.iter().zip(batch.bits.chunks_exact(k)) {
                        if let Some(res) = res {
                            emit(*res, bits, &mut p.bytes);
                        }
                    }
                }
                p.batches.extend(done);
                p.order.clear();
            }
        }
        for &(b, rr) in &p.order {
            p.ci.deinterleave(rr, &p.tz[b * nc..(b + 1) * nc], &mut p.bz);
            p.ci.deinterleave(rr, &p.tw[b * nc..(b + 1) * nc], &mut p.bw);
            if let Ok(res) = p.dec.decode(&p.bz, &p.bw, &mut p.bits) {
                emit(res, &p.bits, &mut p.bytes);
            }
        }
    }
}
