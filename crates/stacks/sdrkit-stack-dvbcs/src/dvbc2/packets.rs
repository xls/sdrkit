//! Data Slice Packets: the cell stream of a data slice cut into the FEC blocks of its PLPs. Type 1 slices carry one PLP whose
//! blocks follow one another from PLP_START; type 2 slices carry packets with a Reed-Muller header (PLP, coding and one or two
//! XFECFrames). The transmit side builds the stream; the receive side finds the blocks of one PLP in it.
//!
//! Standard: ETSI EN 302 769 V1.3.1 clause 7.

use super::header::{self, FecFrameHeader, HeaderDecoder};
use super::tables::{xfecframe_len, CodeRate, ModCod, Qam, STUFFING_CELLS};
use sdrkit_core::Complex32;

/// The coding a FECFrame header names, `None` for a stuffing packet or a code with no coding.
pub fn header_modcod(h: &FecFrameHeader) -> Option<ModCod> {
    Some(ModCod {
        long: h.long,
        qam: Qam::from_code(h.plp_mod)?,
        rate: CodeRate::from_code(h.plp_cod)?,
    })
}

/// The cells of a packet with header `h` behind a header of `head` cells; `None` for a code with no length.
pub fn packet_len(h: &FecFrameHeader, head: usize) -> Option<usize> {
    if h.plp_mod == 0 {
        return h.long.then_some(head + STUFFING_CELLS);
    }
    Some(head + xfecframe_len(h.long, h.plp_mod)? * (1 + usize::from(h.two)))
}

/// What the receive side of a slice needs to know: its type and header form, the PLPs whose blocks go out (a data PLP and the
/// common PLP of its group), and the coding of a type 1 slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SliceSetup {
    /// A type 2 slice.
    pub type2: bool,
    /// The robust (QPSK) header form.
    pub robust: bool,
    /// The PLPs whose blocks go out.
    pub plps: [Option<u8>; 2],
    /// The coding of a type 1 slice.
    pub modcod: Option<ModCod>,
    /// The header scrambling word w^RM.
    pub w_rm: u32,
}

/// The thresholds of the header search and of the alignment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    /// The candidate measure that starts a header decode.
    pub candidate: f32,
    /// Code bits of 32 that must agree with the decoded header.
    pub agree: u32,
    /// Failed headers in a row that lose the lock.
    pub header_misses: u32,
    /// Failed blocks in a row that drop the alignment of a type 1 slice.
    pub block_misses: u32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            candidate: 0.45,
            agree: 22,
            header_misses: 2,
            block_misses: 8,
        }
    }
}

/// One FEC block found in the slice stream.
pub struct Block<'a> {
    /// Its PLP.
    pub plp: u8,
    /// Its coding.
    pub modcod: ModCod,
    /// Its cells.
    pub z: &'a [Complex32],
    /// Their weights.
    pub w: &'a [f32],
}

/// Counters of the receive side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SliceStats {
    /// Headers decoded.
    pub headers: u64,
    /// Expected headers that failed.
    pub header_misses: u64,
    /// Times the alignment or the header lock was lost.
    pub losses: u64,
    /// Blocks of the chosen PLPs sent out.
    pub blocks: u64,
    /// Packets of other PLPs or stuffing skipped.
    pub skipped: u64,
}

/// The receive side of one data slice. It keeps the cells not yet used and allocates nothing once its buffers have grown.
pub struct SliceRx {
    setup: SliceSetup,
    th: Thresholds,
    dec: HeaderDecoder,
    z: Vec<Complex32>,
    w: Vec<f32>,
    /// Cells already used at the front of the buffers.
    used: usize,
    /// The position of the next block (type 1) or header (type 2) when aligned.
    next: Option<usize>,
    misses: u32,
    last_len: usize,
    failed_blocks: u32,
    /// The coding the last header named.
    pub last: Option<(u8, ModCod)>,
    /// The coding the last header of each PLP named, for the PLP table.
    pub codings: Vec<(u8, ModCod)>,
    /// Counters.
    pub stats: SliceStats,
}

impl SliceRx {
    /// The receive side for a slice.
    pub fn new(setup: SliceSetup, th: Thresholds) -> SliceRx {
        SliceRx {
            setup,
            th,
            dec: HeaderDecoder::new(setup.w_rm),
            z: Vec::new(),
            w: Vec::new(),
            used: 0,
            next: None,
            misses: 0,
            last_len: STUFFING_CELLS,
            failed_blocks: 0,
            last: None,
            codings: Vec::new(),
            stats: SliceStats::default(),
        }
    }

    /// The setup.
    pub fn setup(&self) -> SliceSetup {
        self.setup
    }

    /// Forgets the cells and the alignment.
    pub fn reset(&mut self) {
        self.z.clear();
        self.w.clear();
        self.used = 0;
        self.next = None;
        self.misses = 0;
        self.failed_blocks = 0;
    }

    /// Cells buffered and not yet used.
    pub fn pending(&self) -> usize {
        self.z.len() - self.used
    }

    /// The result of a block that went out: a type 1 slice drops its alignment after a run of failures.
    pub fn block_result(&mut self, ok: bool) {
        if ok {
            self.failed_blocks = 0;
            return;
        }
        self.failed_blocks += 1;
        if !self.setup.type2 && self.failed_blocks >= self.th.block_misses {
            self.failed_blocks = 0;
            if self.next.take().is_some() {
                self.stats.losses += 1;
            }
        }
    }

    /// Adds the slice cells of one frame (in stream order). `plp_start` is the PLP_START of the frame in a type 1 slice: it
    /// aligns the stream when it is not aligned. Then sends out every whole block of the chosen PLP.
    pub fn push_frame(
        &mut self,
        z: &[Complex32],
        w: &[f32],
        plp_start: Option<usize>,
        out: &mut impl FnMut(Block<'_>),
    ) {
        if self.used > 0 && self.used * 2 >= self.z.len() {
            self.z.drain(..self.used);
            self.w.drain(..self.used);
            self.next = self.next.map(|n| n.saturating_sub(self.used));
            self.used = 0;
        }
        let frame_at = self.z.len();
        self.z.extend_from_slice(z);
        self.w.extend_from_slice(w);
        if !self.setup.type2 && self.next.is_none() {
            if let Some(s) = plp_start {
                self.next = Some(frame_at + s);
                self.used = self.used.max(frame_at.min(self.z.len()));
            }
        }
        if self.setup.type2 {
            self.run_type2(out);
        } else {
            self.run_type1(out);
        }
    }

    fn run_type1(&mut self, out: &mut impl FnMut(Block<'_>)) {
        let Some(m) = self.setup.modcod else {
            self.used = self.z.len();
            return;
        };
        let len = m.cells();
        let Some(mut at) = self.next else {
            self.used = self.z.len();
            return;
        };
        let plp = self.setup.plps[0].unwrap_or(0);
        while at + len <= self.z.len() {
            self.stats.blocks += 1;
            out(Block {
                plp,
                modcod: m,
                z: &self.z[at..at + len],
                w: &self.w[at..at + len],
            });
            at += len;
        }
        self.next = Some(at);
        self.used = at.min(self.z.len());
    }

    /// Decodes the header at `at`: its fields and packet length when it is good enough.
    fn header_at(&mut self, at: usize) -> Option<(FecFrameHeader, usize)> {
        let head = header::header_cells(self.setup.robust);
        let d = self.dec.decode(&self.z[at..at + head], &self.w[at..at + head], self.setup.robust)?;
        if d.agree < self.th.agree {
            return None;
        }
        let h = FecFrameHeader::from_bits(d.bits);
        if h.plp_mod != 0 && !header_modcod(&h).is_some_and(|m| m.valid()) {
            return None;
        }
        let len = packet_len(&h, head)?;
        Some((h, len))
    }

    /// The length of the packet whose header may start at `at` (`at` plus a header lies inside the buffer): the candidate
    /// measure passes and the header decodes.
    fn header_start(&mut self, at: usize) -> Option<usize> {
        let head = header::header_cells(self.setup.robust);
        let c = self.dec.candidate(&self.z[at..at + head], &self.w[at..at + head], self.setup.robust);
        if c < self.th.candidate {
            return None;
        }
        self.header_at(at).map(|h| h.1)
    }

    fn run_type2(&mut self, out: &mut impl FnMut(Block<'_>)) {
        let head = header::header_cells(self.setup.robust);
        loop {
            let Some(at) = self.next else {
                // Search: the first position whose candidate measure passes, whose header decodes, and whose packet is
                // followed by another such header. Cells of high order QAM pass the first two tests now and then; a
                // header taken from them would cost the packets up to the next loss of the lock.
                let mut p = self.used;
                let mut found = None;
                // The first position that cannot be judged before more cells come.
                let mut open = None;
                while p + head <= self.z.len() {
                    if let Some(len) = self.header_start(p) {
                        if p + len + head > self.z.len() {
                            open.get_or_insert(p);
                        } else if self.header_start(p + len).is_some() {
                            found = Some(p);
                            break;
                        }
                    }
                    p += 1;
                }
                match found {
                    Some(p) => {
                        self.next = Some(p);
                        self.misses = 0;
                        continue;
                    }
                    None => {
                        self.used = open.unwrap_or(p);
                        return;
                    }
                }
            };
            if at + head > self.z.len() {
                self.used = at.min(self.z.len());
                return;
            }
            let Some((h, len)) = self.header_at(at) else {
                self.stats.header_misses += 1;
                self.misses += 1;
                if self.misses >= self.th.header_misses {
                    self.stats.losses += 1;
                    self.next = None;
                    self.used = at + 1;
                    continue;
                }
                // Keep the place with the length of the packet before.
                self.next = Some(at + self.last_len);
                continue;
            };
            if at + len > self.z.len() {
                self.used = at;
                return;
            }
            self.misses = 0;
            self.last_len = len;
            self.stats.headers += 1;
            let modcod = header_modcod(&h);
            if let Some(m) = modcod {
                self.last = Some((h.plp_id, m));
                note_coding(&mut self.codings, h.plp_id, m);
            }
            match modcod {
                Some(m) if self.setup.plps.contains(&Some(h.plp_id)) => {
                    let x = m.cells();
                    for f in 0..1 + usize::from(h.two) {
                        let s = at + head + f * x;
                        self.stats.blocks += 1;
                        out(Block {
                            plp: h.plp_id,
                            modcod: m,
                            z: &self.z[s..s + x],
                            w: &self.w[s..s + x],
                        });
                    }
                }
                _ => self.stats.skipped += 1,
            }
            self.next = Some(at + len);
            self.used = at + len;
        }
    }
}

/// Records `m` as the coding of PLP `plp` in `list`. Returns whether the list changed.
pub fn note_coding(list: &mut Vec<(u8, ModCod)>, plp: u8, m: ModCod) -> bool {
    match list.iter_mut().find(|c| c.0 == plp) {
        Some(c) if c.1 == m => false,
        Some(c) => {
            c.1 = m;
            true
        }
        None => {
            list.push((plp, m));
            true
        }
    }
}

/// The transmit side of one data slice: the cell stream of its packets from the encoded FEC blocks of its PLPs.
pub struct SliceTx {
    type2: bool,
    robust: bool,
    w_rm: u32,
    cons: sdrkit_algo_modem::Constellation,
    queue: std::collections::VecDeque<Complex32>,
    head: Vec<Complex32>,
    /// The state of the generator of the stuffing cells.
    fill: u32,
    /// Cells sent so far.
    pub sent: u64,
}

impl SliceTx {
    /// The transmit side of a slice of type 1, or type 2 with the robust or high efficiency header scrambled with `w_rm`.
    pub fn new(type2: bool, robust: bool, w_rm: u32) -> SliceTx {
        SliceTx {
            type2,
            robust,
            w_rm,
            cons: header::constellation(robust),
            queue: std::collections::VecDeque::new(),
            head: Vec::new(),
            fill: 1,
            sent: 0,
        }
    }

    /// Cells queued.
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// Queues one packet: the FEC block `cells` of PLP `plp` with coding `m` (with its header in a type 2 slice).
    pub fn push_block(&mut self, plp: u8, m: &ModCod, cells: &[Complex32]) {
        if self.type2 {
            let h = FecFrameHeader {
                plp_id: plp,
                long: m.long,
                plp_mod: m.qam.code(),
                plp_cod: m.rate.code(),
                two: false,
            };
            header::cells(h.bits(), self.robust, &self.cons, self.w_rm, &mut self.head);
            self.queue.extend(self.head.iter().copied());
        }
        self.queue.extend(cells.iter().copied());
    }

    /// Queues one stuffing packet of a type 2 slice: a header with PLP_MOD 000 for 64K frames and [`STUFFING_CELLS`] cells
    /// of no meaning (points of the header constellation). It does nothing in a type 1 slice.
    pub fn push_stuffing(&mut self) {
        if !self.type2 {
            return;
        }
        let h = FecFrameHeader {
            plp_id: 0,
            long: true,
            plp_mod: 0,
            plp_cod: 0,
            two: false,
        };
        header::cells(h.bits(), self.robust, &self.cons, self.w_rm, &mut self.head);
        self.queue.extend(self.head.iter().copied());
        let points = self.cons.points();
        for _ in 0..STUFFING_CELLS {
            self.fill = self.fill.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            self.queue.push_back(points[(self.fill >> 24) as usize % points.len()]);
        }
    }

    /// Takes `n` cells of the stream into `out`; `None` when fewer are queued.
    pub fn take(&mut self, n: usize, out: &mut Vec<Complex32>) -> Option<()> {
        if self.queue.len() < n {
            return None;
        }
        out.clear();
        out.extend(self.queue.drain(..n));
        self.sent += n as u64;
        Some(())
    }
}
