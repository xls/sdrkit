//! The decoding core of the DVB-C2 stack: the equalised cells of the data slices in, the descrambled BBFRAMEs of the chosen
//! PLPs out. Per slice it de-interleaves the cells in frequency and time, finds the FEC blocks of the chosen PLPs, demaps and
//! decodes them (LDPC, BCH, BB descrambling), and puts the BBFRAMEs of a bundled PLP back in ISCR order. It measures the MER
//! of the chosen slice. It runs in the stage `fec` of the stack, behind [`C2Cells`] from the front.
//!
//! Standard: ETSI EN 302 769 V1.3.1.

use super::bicm::{point_set, BlockResult, PlpDecoder};
use super::packets::{note_coding, SliceRx, SliceSetup, SliceStats, Thresholds};
use super::signal::{freq_addresses, D_Y};
use super::tables::{ModCod, Qam};
use sdrkit_algo_fec::twistblock::TwistedBlock;
use sdrkit_algo_modem::Constellation;
use sdrkit_core::Complex32;
use sdrkit_graph::Item;
use sdrkit_proto_ipcast::adapt::Issy;
use sdrkit_proto_ipcast::bbheader::BbHeader;
use std::sync::Arc;

/// One data slice of a data path, as the front passes it on.
#[derive(Clone, Debug)]
pub struct SliceSpec {
    /// DSLICE_ID.
    pub id: u8,
    /// The data cells of each symbol class: their columns in the time interleaver.
    pub payload: [Vec<u32>; D_Y],
    /// The time interleaver: rows (the depth) by columns (the carriers of the slice).
    pub ti: TwistedBlock,
    /// The depth of the time interleaver, symbols.
    pub depth: usize,
    /// The columns of the time interleaver.
    pub cols: usize,
    /// What the packet finder of the slice needs.
    pub setup: SliceSetup,
    /// PLP_START of the first frame (type 1).
    pub plp_start: Option<usize>,
}

/// A data path: the slices to decode and the PLPs they pass on.
#[derive(Clone, Debug)]
pub struct PathSetup {
    /// The slices; the first carries the chosen PLP.
    pub slices: Vec<SliceSpec>,
    /// The modulation of the chosen slice when the L1 names it (type 1).
    pub qam: Option<Qam>,
    /// Each PLP passed on: PLP_ID, PLP_PAYLOAD_TYPE, whether it is the common PLP.
    pub roles: Vec<(u8, u8, bool)>,
    /// The slices that carry the chosen PLP.
    pub bundled: usize,
    /// The chosen PLP.
    pub plp: u8,
    /// The common PLP decoded with it.
    pub common: Option<u8>,
    /// The header and alignment thresholds.
    pub packets: Thresholds,
    /// The most LDPC iterations.
    pub iterations: u32,
    /// BBFRAMEs of a bundled PLP held to put them in ISCR order.
    pub bundle_hold: usize,
}

/// What the front of the DVB-C2 receiver passes to the decoding stage, in signal order.
#[derive(Clone, Debug)]
pub enum C2Cells {
    /// Start again from nothing: a new receiver.
    Reset,
    /// A new data path.
    Path(Arc<PathSetup>),
    /// The start of the data symbols of a C2 frame, with the PLP_START of each slice when the L1 gave it again.
    Frame(Option<Vec<Option<usize>>>),
    /// The equalised cells and their weights of data symbol `l` of the frame, per slice of the path.
    Symbol {
        /// The data symbol of the frame.
        l: usize,
        /// The cells of each slice.
        cells: Vec<Vec<Complex32>>,
        /// Their weights.
        weights: Vec<Vec<f32>>,
    },
    /// The front lost the signal: frames held for the ISCR order are dropped.
    Lost,
    /// The most LDPC iterations.
    Iterations(u32),
}

impl Item for C2Cells {
    const KIND: &'static str = "dvbc2.cells";
}

/// One BBFRAME of the PLP.
pub struct BbFrame<'a> {
    /// The descrambled BBFRAME.
    pub bytes: &'a [u8],
    /// The BCH decoder accepted it.
    pub ok: bool,
    /// The signal time of its last cell, seconds.
    pub time: f64,
    /// The PLP it belongs to.
    pub plp: u8,
    /// PLP_PAYLOAD_TYPE of the PLP: 0 GFPS, 1 GCS, 2 GSE, 3 TS.
    pub payload: u8,
    /// It belongs to the common PLP of the chosen PLP's group.
    pub common: bool,
}

/// The measurements and counters of the decoding stage.
#[derive(Clone, Debug, Default)]
pub struct DecodeStatus {
    /// MER of the decoded slice cells against the nearest points, dB.
    pub mer_db: Option<f64>,
    /// The coding that the last FECFrame header of each PLP named, in every slice of the path.
    pub codings: Vec<(u8, ModCod)>,
    /// FEC blocks decoded.
    pub blocks_ok: u64,
    /// FEC blocks that failed.
    pub blocks_bad: u64,
    /// Times a slice lost its packet alignment or header lock.
    pub sync_losses: u64,
    /// The bit error ratio before the LDPC decoder over the good blocks.
    pub pre_ber: Option<f64>,
    /// The mean LDPC iterations of the good blocks.
    pub iterations: Option<f64>,
    /// The slice counters.
    pub slice: SliceStats,
}

/// A BBFRAME held for the ISCR order: (order key, bytes, BCH result, time, PLP, payload type).
type Held = (u32, Vec<u8>, bool, f64, u8, u8);

/// The ISCR of a BBFRAME whose header carries ISSY in high efficiency mode, `None` otherwise.
fn iscr_of(bytes: &[u8]) -> Option<u32> {
    let h = BbHeader::parse(bytes).ok()?;
    match Issy::parse(&h.issy?).ok()? {
        Issy::IscrLong(v) => Some(v),
        Issy::IscrShort(v) => Some(u32::from(v)),
        _ => None,
    }
}

/// Puts the BBFRAMEs of a PLP bundled over several slices back in the order of their ISCR (22 bits, modulo): it holds up to
/// `hold` frames and lets the earliest go when more arrive.
#[derive(Default)]
struct Reorder {
    held: Vec<Held>,
    /// The ISCR of the last frame let go.
    last: Option<u32>,
}

impl Reorder {
    const MASK: u32 = 0x3F_FFFF;

    /// The order key of `iscr`: its distance from the last frame let go (or from the first held), centred so that a frame a
    /// little earlier sorts first.
    fn key(&self, iscr: u32) -> u32 {
        let r = self.last.or(self.held.first().map(|h| h.0)).unwrap_or(iscr);
        iscr.wrapping_sub(r).wrapping_add(1 << 21) & Self::MASK
    }

    /// The earliest held frame, taken out.
    fn pop(&mut self) -> Option<Held> {
        let i = (0..self.held.len()).min_by_key(|&i| self.key(self.held[i].0))?;
        let h = self.held.swap_remove(i);
        self.last = Some(h.0);
        Some(h)
    }

    fn clear(&mut self) {
        self.held.clear();
        self.last = None;
    }
}

/// One slice of the data path: its de-interleavers and packet finder.
struct SliceDec {
    payload: [Vec<u32>; D_Y],
    addr: [[Vec<u32>; D_Y]; 2],
    depth: usize,
    cols: usize,
    ti: TwistedBlock,
    mask: Vec<bool>,
    rows_z: Vec<Complex32>,
    rows_w: Vec<f32>,
    rows_in: usize,
    ti_z: Vec<Complex32>,
    ti_w: Vec<f32>,
    slice: SliceRx,
    plp_start: Option<usize>,
}

impl SliceDec {
    fn new(s: &SliceSpec, th: Thresholds) -> SliceDec {
        let addr = std::array::from_fn(|odd| {
            std::array::from_fn(|d| {
                let mut v = Vec::new();
                freq_addresses(odd == 1, s.payload[d].len(), &mut v);
                v
            })
        });
        SliceDec {
            payload: s.payload.clone(),
            addr,
            depth: s.depth,
            cols: s.cols,
            ti: s.ti.clone(),
            mask: vec![false; s.depth * s.cols],
            rows_z: Vec::new(),
            rows_w: Vec::new(),
            rows_in: 0,
            ti_z: Vec::new(),
            ti_w: Vec::new(),
            slice: SliceRx::new(s.setup, th),
            plp_start: s.plp_start,
        }
    }
}

/// A decoded block: (BBFRAME, result, coding, PLP).
type Decoded = (Vec<u8>, BlockResult, ModCod, u8);

/// The decoding core.
#[derive(Default)]
pub struct C2Decoder {
    path: Option<Arc<PathSetup>>,
    slices: Vec<SliceDec>,
    cons: Option<Constellation>,
    mer_err: f64,
    mer_sig: f64,
    decoders: Vec<PlpDecoder>,
    iterations: u32,
    decoded: Vec<Decoded>,
    /// BBFRAME buffers for reuse.
    spare: Vec<Vec<u8>>,
    reorder: Reorder,
    iter_sum: u64,
    bit_err: u64,
    bits: u64,
    /// The measurements and counters.
    pub status: DecodeStatus,
}

impl C2Decoder {
    /// Takes one item of the front at signal time `time`; `out` receives the BBFRAMEs decoded.
    pub fn push(&mut self, item: &C2Cells, time: f64, out: &mut impl FnMut(BbFrame<'_>)) {
        match item {
            C2Cells::Reset => *self = C2Decoder::default(),
            C2Cells::Path(p) => self.set_path(p),
            C2Cells::Frame(starts) => {
                for (i, sp) in self.slices.iter_mut().enumerate() {
                    if let Some(s) = starts {
                        sp.plp_start = s.get(i).copied().flatten();
                    }
                    sp.rows_in = 0;
                    sp.rows_z.clear();
                    sp.rows_w.clear();
                }
            }
            C2Cells::Symbol { l, cells, weights } => self.symbol(*l, cells, weights, time, out),
            C2Cells::Lost => self.reorder.clear(),
            C2Cells::Iterations(n) => self.set_iterations(*n),
        }
    }

    /// Sends out the frames held for the ISCR order.
    pub fn finish(&mut self, out: &mut impl FnMut(BbFrame<'_>)) {
        while let Some(h) = self.reorder.pop() {
            self.emit(h, out);
        }
    }

    fn set_iterations(&mut self, n: u32) {
        if n != self.iterations {
            self.iterations = n;
            for d in &mut self.decoders {
                d.set_iterations(n);
            }
        }
    }

    fn set_path(&mut self, p: &Arc<PathSetup>) {
        self.set_iterations(p.iterations);
        self.slices = p.slices.iter().map(|s| SliceDec::new(s, p.packets)).collect();
        self.cons = p.qam.and_then(|q| Constellation::new(point_set(q)).ok());
        self.mer_err = 0.0;
        self.mer_sig = 0.0;
        self.path = Some(p.clone());
    }

    /// Data symbol `l` of the frame: the MER of the chosen slice, then per slice the frequency de-interleaving into the time
    /// interleaver row and, once the rows of a block are in, its blocks.
    fn symbol(
        &mut self,
        l: usize,
        cells: &[Vec<Complex32>],
        weights: &[Vec<f32>],
        time: f64,
        out: &mut impl FnMut(BbFrame<'_>),
    ) {
        let d = l % D_Y;
        let mut slices = std::mem::take(&mut self.slices);
        for (idx, sp) in slices.iter_mut().enumerate() {
            let (Some(a), Some(aw)) = (cells.get(idx), weights.get(idx)) else { continue };
            // MER on a part of the cells of the chosen slice.
            if idx == 0 {
                if let Some(c) = &self.cons {
                    for z in a.iter().step_by(7) {
                        let near = c.nearest(*z);
                        self.mer_err += f64::from((z - near).norm_sqr());
                        self.mer_sig += f64::from(near.norm_sqr());
                    }
                    if self.mer_sig > 0.0 && self.mer_err > 0.0 {
                        self.status.mer_db = Some(10.0 * (self.mer_sig / self.mer_err).log10());
                        self.mer_sig *= 0.98;
                        self.mer_err *= 0.98;
                    }
                }
            }
            // Frequency de-interleaving into the time interleaver row.
            let addr = &sp.addr[l % 2][d];
            let base = sp.rows_z.len();
            sp.rows_z.resize(base + a.len(), Complex32::default());
            sp.rows_w.resize(base + a.len(), 0.0);
            for (q, &h) in addr.iter().enumerate() {
                sp.rows_z[base + h as usize] = a[q];
                sp.rows_w[base + h as usize] = aw[q];
            }
            sp.rows_in += 1;
            if sp.rows_in == sp.depth {
                let first = l + 1 - sp.depth;
                if let Some(m) = self.ti_block(sp, idx, first, time, out) {
                    if idx == 0 && self.cons.as_ref().map(|c| c.bits()) != Some(m.qam.bits()) {
                        self.cons = Constellation::new(point_set(m.qam)).ok();
                    }
                }
            }
        }
        self.slices = slices;
    }

    /// De-interleaves one time interleaving block of slice `idx` whose first row is data symbol `first`, decodes its FEC
    /// blocks and sends out their BBFRAMEs. Returns the coding the last FECFrame header named.
    fn ti_block(
        &mut self,
        p: &mut SliceDec,
        idx: usize,
        first: usize,
        time: f64,
        out: &mut impl FnMut(BbFrame<'_>),
    ) -> Option<ModCod> {
        p.mask.fill(false);
        for r in 0..p.depth {
            for &c in &p.payload[(first + r) % D_Y] {
                p.mask[r * p.cols + c as usize] = true;
            }
        }
        let n = p.rows_z.len();
        let mut z = std::mem::take(&mut p.ti_z);
        let mut wv = std::mem::take(&mut p.ti_w);
        z.resize(n, Complex32::default());
        wv.resize(n, 0.0);
        // Depth 1 is the identity.
        let ok = if p.depth == 1 {
            z.copy_from_slice(&p.rows_z);
            wv.copy_from_slice(&p.rows_w);
            true
        } else {
            p.ti.deinterleave(Some(&p.mask), &p.rows_z, &mut z).is_ok()
                && p.ti.deinterleave(Some(&p.mask), &p.rows_w, &mut wv).is_ok()
        };
        p.rows_z.clear();
        p.rows_w.clear();
        p.rows_in = 0;
        if !ok {
            p.ti_z = z;
            p.ti_w = wv;
            return None;
        }
        let start = if first == 0 { p.plp_start.take() } else { None };
        let losses = p.slice.stats.losses;
        let (decoders, spare, decoded, iters) =
            (&mut self.decoders, &mut self.spare, &mut self.decoded, self.iterations);
        p.slice.push_frame(&z, &wv, start, &mut |b| {
            let i = match decoders.iter().position(|d| d.modcod() == b.modcod) {
                Some(i) => i,
                None => match PlpDecoder::new(b.modcod, iters) {
                    Some(d) => {
                        decoders.push(d);
                        decoders.len() - 1
                    }
                    None => return,
                },
            };
            let mut bytes = spare.pop().unwrap_or_default();
            bytes.clear();
            bytes.resize(b.modcod.fec().k_bch / 8, 0);
            match decoders[i].decode(b.z, b.w, &mut bytes) {
                Some(r) => decoded.push((bytes, r, b.modcod, b.plp)),
                None => spare.push(bytes),
            }
        });
        p.ti_z = z;
        p.ti_w = wv;
        if p.slice.stats.losses != losses {
            self.status.sync_losses += p.slice.stats.losses - losses;
        }
        let mut decoded = std::mem::take(&mut self.decoded);
        for b in decoded.drain(..) {
            self.result(p, b, time, out);
        }
        self.decoded = decoded;
        if idx == 0 {
            self.status.slice = p.slice.stats;
        }
        for &(id, m) in &p.slice.codings {
            note_coding(&mut self.status.codings, id, m);
        }
        p.slice.last.map(|l| l.1)
    }

    /// Counts the result of one block of slice `p`, tells the slice, and sends out or holds its BBFRAME.
    fn result(&mut self, p: &mut SliceDec, (bytes, r, m, plp): Decoded, time: f64, out: &mut impl FnMut(BbFrame<'_>)) {
        if r.bch_ok {
            self.status.blocks_ok += 1;
            self.iter_sum += u64::from(r.iterations);
            self.bit_err += u64::from(r.raw_errors);
            self.bits += m.n() as u64;
            self.status.pre_ber = Some(self.bit_err as f64 / self.bits as f64);
            self.status.iterations = Some(self.iter_sum as f64 / self.status.blocks_ok as f64);
        } else {
            self.status.blocks_bad += 1;
        }
        let before = p.slice.stats.losses;
        p.slice.block_result(r.bch_ok);
        if p.slice.stats.losses != before {
            self.status.sync_losses += 1;
        }
        let (payload, bundled, hold) = match self.path.as_ref() {
            Some(path) => (
                path.roles.iter().find(|x| x.0 == plp).map_or(3, |x| x.1),
                path.bundled > 1 && path.plp == plp,
                path.bundle_hold,
            ),
            None => (3, false, 0),
        };
        match (bundled && r.bch_ok).then(|| iscr_of(&bytes)).flatten() {
            Some(iscr) => {
                let key = iscr & Reorder::MASK;
                self.reorder.held.push((key, bytes, true, time, plp, payload));
                while self.reorder.held.len() > hold {
                    if let Some(h) = self.reorder.pop() {
                        self.emit(h, out);
                    }
                }
            }
            None => self.emit((0, bytes, r.bch_ok, time, plp, payload), out),
        }
    }

    /// Sends out one frame and keeps its buffer for reuse.
    fn emit(&mut self, (_, bytes, ok, time, plp, payload): Held, out: &mut impl FnMut(BbFrame<'_>)) {
        out(BbFrame {
            bytes: &bytes,
            ok,
            time,
            plp,
            payload,
            common: self.path.as_ref().is_some_and(|p| p.common == Some(plp)),
        });
        self.spare.push(bytes);
    }
}
