//! The LE Coded detector of the 1 Msym/s lane (Bluetooth Core Specification v5.4 Vol 6 Part B clauses 2.2 and 3.3). It matches
//! the first 128 symbols of the coded access address on hard symbols, fits the timing, the offset and the deviation over the
//! preamble and the coded access address, demaps the pattern mapper's symbols to soft bits and decodes FEC block 1 (the CI) and
//! FEC block 2 (the PDU and CRC) with a Viterbi decoder of the K = 4 code.

use super::phy::{coded_block1, Coding, Link, Phy, CODED_PREAMBLE, CODED_PREAMBLE_SYMBOLS};
use super::rx::{link_crc_ok, plausible, well_formed, BlePacket, Front, ReceiverStats};
use super::{whiten, MAX_PAYLOAD};
use crate::Result;
use sdrkit_algo_fec::conv::{ConvCode, Ends, Viterbi};
use sdrkit_util_bits::sync::SyncWord;
use sdrkit_util_bits::{pack, BitOrder, Pad};
use std::collections::VecDeque;

/// Symbols of history kept before the oldest candidate.
pub(super) const KEEP_BACK: usize = 256;
/// The first symbol of the sync window: the first symbol of the coded access address.
const SYNC_AT: usize = CODED_PREAMBLE_SYMBOLS;
/// Symbols in the sync window.
const SYNC_LEN: usize = 128;
/// Symbol errors allowed in the sync window.
const SYNC_ERRORS: u32 = 20;
/// The known symbols: the preamble and the coded access address.
const KNOWN_LEN: usize = CODED_PREAMBLE_SYMBOLS + 256;
/// Symbol errors allowed over the known symbols once the timing is fitted.
const KNOWN_ERRORS: usize = 60;
/// The first symbol of FEC block 2.
const BLOCK2_AT: usize = CODED_PREAMBLE_SYMBOLS + 296;
/// Trellis steps decoded to read the header (16 bits, then the rest as look-ahead; the shortest packet has 43 steps).
const HEADER_STEPS: usize = 40;
/// The most trellis steps of FEC block 2: the longest PDU, the CRC and TERM2.
const MAX_STEPS: usize = 8 * (2 + MAX_PAYLOAD + 3) + 3;
/// The soft value of a clean coded bit.
const LLR_FULL: f32 = 32.0;

/// The LE Coded convolutional code (clause 3.3.1): G0 = 1 + x + x^2 + x^3 sent first, G1 = 1 + x^2 + x^3, written with the
/// newest bit in the most significant place.
pub fn code() -> ConvCode {
    ConvCode::new(4, &[0o17, 0o13])
}

#[derive(Clone, Debug)]
struct LinkCode {
    /// The known symbols as +1 (symbol 1, the higher frequency) and -1.
    known: Vec<f32>,
    /// One per sample phase.
    sync: Vec<SyncWord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Block1,
    Header,
    Body,
}

#[derive(Clone, Copy, Debug)]
struct Cand {
    /// The sample of the last symbol of the sync window.
    pos: u64,
    link: usize,
    power: f64,
    exact: bool,
    stage: Stage,
    /// The fitted sample of symbol 0 (the first preamble symbol).
    p0: u64,
    dc: f32,
    dev: f32,
    snr_db: f64,
    coding: Coding,
    /// (trellis steps of FEC block 2, channel), in increasing order; `next` is the next to try.
    targets: [(usize, u8); 3],
    count: usize,
    next: usize,
}

/// The coded detector (module documentation).
#[derive(Clone, Debug)]
pub(super) struct CodedLane {
    sps: usize,
    links: Vec<LinkCode>,
    holdoff: u64,
    good_end: u64,
    pending: VecDeque<Cand>,
    viterbi: Viterbi,
    llr: Vec<i8>,
    bits: Vec<u8>,
    bytes: Vec<u8>,
    trial: Vec<u8>,
}

impl CodedLane {
    pub(super) fn new(sps: usize) -> Result<CodedLane> {
        Ok(CodedLane {
            sps,
            links: Vec::new(),
            holdoff: 0,
            good_end: 0,
            pending: VecDeque::with_capacity(super::rx::MAX_PENDING),
            viterbi: Viterbi::new(code(), MAX_STEPS)?,
            llr: Vec::with_capacity(2 * MAX_STEPS),
            bits: vec![0; MAX_STEPS],
            bytes: Vec::with_capacity(MAX_STEPS / 8 + 1),
            trial: Vec::with_capacity(MAX_STEPS / 8 + 1),
        })
    }

    pub(super) fn add_link(&mut self, link: &Link) -> Result<()> {
        let mut sym = Vec::with_capacity(KNOWN_LEN + 40);
        for _ in 0..10 {
            sym.extend_from_slice(&CODED_PREAMBLE);
        }
        coded_block1(link.access_address, Coding::S8, &mut sym);
        sym.truncate(KNOWN_LEN);
        let pat = sym[SYNC_AT..SYNC_AT + SYNC_LEN]
            .iter()
            .fold(0u128, |a, &b| a << 1 | b as u128);
        let s = SyncWord::new(pat, SYNC_LEN as u32, SYNC_ERRORS)?;
        self.links.push(LinkCode {
            known: sym.iter().map(|&b| 2.0 * b as f32 - 1.0).collect(),
            sync: vec![s; self.sps],
        });
        Ok(())
    }

    pub(super) fn remove_link(&mut self, i: usize) {
        self.links.remove(i);
        self.pending.retain(|c| c.link != i);
        for c in &mut self.pending {
            if c.link > i {
                c.link -= 1;
            }
        }
    }

    pub(super) fn reset(&mut self, holdoff: u64) {
        for l in &mut self.links {
            for s in &mut l.sync {
                s.reset();
            }
        }
        self.pending.clear();
        self.holdoff = holdoff;
    }

    /// The oldest sample a waiting candidate needs.
    pub(super) fn oldest(&self, now: u64) -> u64 {
        self.pending.iter().map(|c| c.p0.min(c.pos)).min().unwrap_or(now)
    }

    /// Takes the hard decision `bit` of the symbol that ends at the front's current sample (phase `k`).
    #[inline]
    pub(super) fn push_symbol(&mut self, front: &Front, bit: u8, k: usize, warm: u64) {
        let n = front.n;
        let sps = self.sps as u64;
        for li in 0..self.links.len() {
            let Some(h) = self.links[li].sync[k].push(bit) else {
                continue;
            };
            if n < self.holdoff || n < warm {
                continue;
            }
            self.holdoff = n + sps;
            if self.pending.len() == super::rx::MAX_PENDING {
                self.pending.pop_front();
            }
            self.pending.push_back(Cand {
                pos: n,
                link: li,
                power: front.power(),
                exact: h.errors() == 0,
                stage: Stage::Block1,
                p0: n.saturating_sub(sps * (SYNC_AT + SYNC_LEN - 1) as u64),
                dc: 0.0,
                dev: 0.0,
                snr_db: 0.0,
                coding: Coding::S8,
                targets: [(0, 0); 3],
                count: 0,
                next: 0,
            });
        }
    }

    /// The last symbol a candidate needs at its stage.
    fn needs(c: &Cand) -> usize {
        match c.stage {
            Stage::Block1 => BLOCK2_AT + 2,
            Stage::Header => BLOCK2_AT + 2 * c.coding.pattern() * HEADER_STEPS + 2,
            Stage::Body => BLOCK2_AT + 2 * c.coding.pattern() * c.targets[c.next].0 + 2,
        }
    }

    pub(super) fn drain(
        &mut self,
        front: &Front,
        links: &[Link],
        channels: &[u8],
        stats: &mut ReceiverStats,
        on_packet: &mut impl FnMut(&BlePacket<'_>),
    ) {
        let sps = self.sps as u64;
        while let Some(&c) = self.pending.front() {
            // The latest timing is half a symbol after the match.
            if front.n < c.p0 + sps * (Self::needs(&c) as u64 + 1) {
                break;
            }
            if c.pos < self.good_end {
                self.pending.pop_front();
                continue;
            }
            match c.stage {
                Stage::Block1 => {
                    stats.aa_hits += 1;
                    match self.block1(front, c, links[c.link]) {
                        Some(next) => self.pending[0] = next,
                        None => {
                            self.pending.pop_front();
                        }
                    }
                }
                Stage::Header => {
                    let mut t = [(0usize, 0u8); 3];
                    let mut count = 0;
                    self.soft(front, &c, BLOCK2_AT, HEADER_STEPS);
                    let ok = self
                        .viterbi
                        .decode_only(&self.llr, Ends::Start(0), &mut self.bits[..HEADER_STEPS])
                        .is_ok();
                    if ok {
                        self.bytes.clear();
                        pack::pack(&self.bits[..16], BitOrder::Lsb, Pad::Zero, &mut self.bytes);
                        for &ch in channels {
                            let mut h = [self.bytes[0], self.bytes[1]];
                            let _ = whiten(&mut h, ch);
                            if let Some(len) = plausible(links[c.link].kind, ch, h) {
                                t[count] = (8 * (2 + len + 3) + 3, ch);
                                count += 1;
                            }
                        }
                    }
                    t[..count].sort_unstable_by_key(|x| x.0);
                    if count == 0 {
                        self.fail(c, stats);
                    } else {
                        let f = &mut self.pending[0];
                        f.stage = Stage::Body;
                        f.targets = t;
                        f.count = count;
                        f.next = 0;
                    }
                }
                Stage::Body => {
                    if self.body(front, c, links[c.link], stats, on_packet) {
                        self.pending.pop_front();
                    } else if c.next + 1 < c.count {
                        self.pending[0].next += 1;
                    } else {
                        self.fail(c, stats);
                    }
                }
            }
        }
    }

    fn fail(&mut self, c: Cand, stats: &mut ReceiverStats) {
        if c.exact && self.good_end <= c.pos {
            stats.crc_failures += 1;
        }
        self.pending.pop_front();
    }

    /// Fits the timing, the offset and the deviation of a candidate over its known symbols and decodes FEC block 1. Returns the
    /// candidate ready for its header, or `None` when it is no packet.
    fn block1(&mut self, front: &Front, c: Cand, link: Link) -> Option<Cand> {
        let sps = self.sps as u64;
        let known = &self.links[c.link].known;
        let half = (sps / 2) as i64;
        if c.p0 < front.hbase + sps * 2 + half as u64 {
            return None;
        }
        let mut best = (f32::MIN, c.p0);
        for delta in -half..=half {
            let p = (c.p0 as i64 + delta) as u64;
            let corr: f32 = known
                .iter()
                .enumerate()
                .map(|(i, &k)| k * front.f(p + sps * i as u64))
                .sum();
            if corr > best.0 {
                best = (corr, p);
            }
        }
        let p0 = best.1;
        let v = |i: usize| front.f(p0 + sps * i as u64);
        // The preamble has as many 1 symbols as 0 symbols: its mean is the offset.
        let dc = (0..CODED_PREAMBLE_SYMBOLS).map(v).sum::<f32>() / CODED_PREAMBLE_SYMBOLS as f32;
        let dev = known
            .iter()
            .enumerate()
            .map(|(i, &k)| (v(i) - dc) * k)
            .sum::<f32>()
            / KNOWN_LEN as f32;
        if dev < 0.25 {
            return None;
        }
        let errors = known
            .iter()
            .enumerate()
            .filter(|&(i, &k)| (v(i) - dc) * k < 0.0)
            .count();
        if errors > KNOWN_ERRORS {
            return None;
        }
        let resid = known
            .iter()
            .enumerate()
            .map(|(i, &k)| {
                let e = v(i) - dc - dev * k;
                e * e
            })
            .sum::<f32>()
            / KNOWN_LEN as f32;
        let snr_db = (10.0 * ((dev * dev) / resid.max(1e-6)).log10()).min(40.0) as f64;
        let mut d = Cand {
            p0,
            dc,
            dev,
            snr_db,
            ..c
        };
        self.soft(front, &d, CODED_PREAMBLE_SYMBOLS, 37);
        self.viterbi
            .decode_only(&self.llr, Ends::Zero, &mut self.bits[..34])
            .ok()?;
        let aa_errors = (0..32)
            .filter(|&i| self.bits[i] != (link.access_address >> i) as u8 & 1)
            .count();
        if aa_errors > 2 {
            return None;
        }
        d.coding = Coding::from_ci(self.bits[32] | self.bits[33] << 1)?;
        d.stage = Stage::Header;
        Some(d)
    }

    /// Writes the soft bits of `steps` trellis steps from symbol `at` to `self.llr` (positive means 0): FEC block 1 is S=8, FEC
    /// block 2 the candidate's coding.
    fn soft(&mut self, front: &Front, c: &Cand, at: usize, steps: usize) {
        let sps = self.sps as u64;
        let p = if at == CODED_PREAMBLE_SYMBOLS {
            4
        } else {
            c.coding.pattern()
        };
        let inv = 1.0 / c.dev;
        let scale = LLR_FULL / p as f32;
        let y = |i: usize| (front.f(c.p0 + sps * i as u64) - c.dc) * inv;
        self.llr.clear();
        for j in 0..2 * steps {
            let s = at + j * p;
            // Coded bit 0 is 0011 (P = 4) or 0 (P = 1): low symbols first.
            let l = if p == 4 {
                -y(s) - y(s + 1) + y(s + 2) + y(s + 3)
            } else {
                -y(s)
            };
            self.llr.push((l * scale).round().clamp(-127.0, 127.0) as i8);
        }
    }

    /// Decodes FEC block 2 at the candidate's next target and reports a good packet. True for a good packet.
    fn body(
        &mut self,
        front: &Front,
        c: Cand,
        link: Link,
        stats: &mut ReceiverStats,
        on_packet: &mut impl FnMut(&BlePacket<'_>),
    ) -> bool {
        let (steps, channel) = c.targets[c.next];
        let info = steps - 3;
        self.soft(front, &c, BLOCK2_AT, steps);
        if self
            .viterbi
            .decode_only(&self.llr, Ends::Zero, &mut self.bits[..info])
            .is_err()
        {
            return false;
        }
        self.trial.clear();
        pack::pack(&self.bits[..info], BitOrder::Lsb, Pad::Zero, &mut self.trial);
        let _ = whiten(&mut self.trial, channel);
        let total = self.trial.len();
        if !link_crc_ok(&self.trial, &link) || !well_formed(link.kind, &self.trial[..total - 3]) {
            return false;
        }
        let sps = self.sps as u64;
        let end = c.p0 + sps * (BLOCK2_AT + 2 * c.coding.pattern() * steps) as u64;
        self.good_end = end;
        stats.packets += 1;
        let deviation = Phy::LeCoded.deviation_hz();
        on_packet(&BlePacket {
            pdu: &self.trial[..total - 3],
            channel,
            sample: (c.p0 + sps * (BLOCK2_AT - 41) as u64).saturating_sub(front.delay),
            offset_hz: c.dc as f64 * deviation,
            deviation_hz: c.dev as f64 * deviation,
            snr_db: c.snr_db,
            level_dbfs: sdrkit_algo_dsp::meter::power_to_db(c.power),
            phy: Phy::LeCoded,
            coding: Some(c.coding),
            link,
        });
        true
    }
}
