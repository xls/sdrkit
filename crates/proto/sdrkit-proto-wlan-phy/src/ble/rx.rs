//! The LE receiver of one RF channel. The input is complex baseband centred on the channel at a whole number of samples per
//! microsecond (2 to 16). A lane per symbol rate runs a channel filter and an FM discriminator: the 1 Msym/s lane decodes the
//! LE 1M PHY and (when asked) the LE Coded PHY, the 2 Msym/s lane the LE 2M PHY. Each lane listens for every [`Link`] the
//! receiver holds.
//!
//! On the uncoded PHYs the receiver finds the access address with up to 2 bit errors, decides the bits with a 4-state
//! maximum-likelihood sequence estimator and outputs the first candidate PDU with a good CRC-24. On the LE Coded PHY it finds the
//! coded access address on hard symbols, demaps the pattern mapper's symbols to soft bits, decodes FEC block 1 (CI) and FEC
//! block 2 with a Viterbi decoder, and checks the CRC-24.
//!
//! Standard: Bluetooth Core Specification v5.4, Vol 6 Part A clause 3, Part B clauses 2 and 3.

use super::coded_rx::CodedLane;
use super::phy::{Coding, Link, LinkKind, Phy};
use super::{crc24_bytes, crc_ok, whiten, AdvPdu, ADV_CRC_INIT, MAX_PAYLOAD};
use crate::{finite, Complex32, Error, Param, Result};
use sdrkit_algo_dsp::fir::{self, Fir};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_modem::gmsk::{ls_channel, Mlse};
use sdrkit_util_bits::sync::SyncWord;
use sdrkit_util_bits::{pack, BitOrder, Pad};
use std::collections::VecDeque;

/// Bits after the access address that the receiver decides: header, the longest payload and the CRC.
const DATA_BITS: usize = (2 + MAX_PAYLOAD + 3) * 8;
/// The known bits: the last 8 preamble bits and the access address.
pub(super) const KNOWN: usize = 40;
/// Bits after the access address read first: the header and one byte.
const HEADER_BITS: usize = 24;
/// Bits decided past the end of a packet, so its last bits have their neighbours.
const MARGIN_BITS: usize = 4;
/// The most candidates that wait for their samples.
pub(super) const MAX_PENDING: usize = 64;
/// The largest discriminator value kept, in units of the nominal deviation.
const CLIP: f32 = 1.5;
/// Symbols of the slow average that sets the slicing level.
pub(super) const LEVEL_BITS: usize = 40;
/// The most links a receiver holds.
pub const MAX_LINKS: usize = 16;

/// A good packet. It borrows the receiver's buffer for the time of the callback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlePacket<'a> {
    /// The PDU (header and payload), dewhitened, without the CRC.
    pub pdu: &'a [u8],
    /// The RF channel whose whitening provided the good CRC.
    pub channel: u8,
    /// The input sample index of the last symbol of the access address.
    pub sample: u64,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The fitted deviation of an isolated symbol, Hz.
    pub deviation_hz: f64,
    /// The signal-to-noise ratio of the fitted levels, dB (40 dB at most).
    pub snr_db: f64,
    /// The mean power of the input over the preamble and access address, dBFS (dB relative to full scale).
    pub level_dbfs: f64,
    /// The PHY.
    pub phy: Phy,
    /// The coding of FEC block 2 on the LE Coded PHY; `None` on the uncoded PHYs.
    pub coding: Option<Coding>,
    /// The link the packet belongs to.
    pub link: Link,
}

impl BlePacket<'_> {
    /// The parsed PDU, for a packet of an advertising or periodic link.
    pub fn parse(&self) -> Result<AdvPdu<'_>> {
        AdvPdu::parse(self.pdu)
    }
}

/// The receiver's counters since it was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReceiverStats {
    /// Access-address matches (at most 2 bit errors uncoded; on the LE Coded PHY a coded access address match).
    pub aa_hits: u64,
    /// Exact access-address matches whose CRC failed on every channel tried.
    pub crc_failures: u64,
    /// Good packets.
    pub packets: u64,
}

impl ReceiverStats {
    fn add(&mut self, o: ReceiverStats) {
        self.aa_hits += o.aa_hits;
        self.crc_failures += o.crc_failures;
        self.packets += o.packets;
    }
}

/// The PHYs a receiver decodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Phys {
    /// LE 1M.
    pub le_1m: bool,
    /// LE 2M.
    pub le_2m: bool,
    /// LE Coded (S=2 and S=8).
    pub le_coded: bool,
}

impl Phys {
    /// LE 1M only.
    pub const LE_1M: Phys = Phys {
        le_1m: true,
        le_2m: false,
        le_coded: false,
    };
    /// The PHYs of the primary advertising channels: LE 1M and LE Coded (Vol 6 Part B clause 4.4.2.2).
    pub const PRIMARY: Phys = Phys {
        le_1m: true,
        le_2m: false,
        le_coded: true,
    };
    /// All three PHYs.
    pub const ALL: Phys = Phys {
        le_1m: true,
        le_2m: true,
        le_coded: true,
    };
}

/// The RF channel the input is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChannelPlan {
    /// A primary advertising channel, which one not known for sure: the whitening of 37, 38 and 39 is tried, `hint` first.
    Advertising {
        /// The likely channel (37, 38 or 39).
        hint: Option<u8>,
    },
    /// The channel index (0 to 39).
    Known(u8),
}

#[derive(Clone, Copy, Debug)]
struct Cand {
    pos: u64,
    link: usize,
    /// The mean input power `|x|^2` over the 40 symbols before the match.
    power: f64,
    exact: bool,
    /// The header has been read: `targets[..count]` are (bits to decide, channel) in increasing order, and `stage` is the next.
    read: bool,
    targets: [(usize, u8); 3],
    count: usize,
    stage: usize,
}

/// The front of a lane: the channel filter, the discriminator, the one-symbol average, the slicing level and the history of the
/// averaged discriminator.
#[derive(Clone, Debug)]
pub(super) struct Front {
    pub(super) sps: usize,
    filter: Fir<Complex32>,
    pub(super) delay: u64,
    disc: Discriminator,
    pub(super) n: u64,
    avg: Vec<f32>,
    avg_sum: f32,
    level: Vec<f32>,
    level_sum: f64,
    pow: Vec<f32>,
    pow_sum: f64,
    pub(super) hist: Vec<f32>,
    pub(super) hbase: u64,
}

impl Front {
    fn new(sps: usize, symbol_rate: f64) -> Result<Front> {
        let rate = sps as f64 * symbol_rate;
        let lp = fir::lowpass(
            rate,
            0.55 * symbol_rate,
            (1.1 * symbol_rate).min(rate / 2.0),
            40.0,
        )
        .map_err(|_| Error::BadParameter(Param::FilterDesign))?;
        Ok(Front {
            sps,
            delay: (lp.len() as u64 - 1) / 2,
            filter: Fir::new(&lp),
            disc: Discriminator::new(rate, 0.25 * symbol_rate)
                .map_err(|_| Error::BadParameter(Param::FilterDesign))?,
            n: 0,
            avg: vec![0.0; sps],
            avg_sum: 0.0,
            level: vec![0.0; LEVEL_BITS * sps],
            level_sum: 0.0,
            pow: vec![0.0; KNOWN * sps],
            pow_sum: 0.0,
            hist: Vec::with_capacity(1 << 17),
            hbase: 0,
        })
    }

    fn reset(&mut self) {
        self.disc.reset();
        self.filter.reset();
        self.avg.fill(0.0);
        self.avg_sum = 0.0;
        self.level.fill(0.0);
        self.level_sum = 0.0;
        self.pow.fill(0.0);
        self.pow_sum = 0.0;
    }

    /// Takes one sample: returns the hard symbol decision of the symbol that ends at it and the phase `n mod sps`.
    #[inline]
    fn push(&mut self, s: Complex32) -> (u8, usize) {
        let sps = self.sps;
        let s = finite(s);
        let pi = (self.n % self.pow.len() as u64) as usize;
        let pw = s.norm_sqr();
        self.pow_sum += (pw - self.pow[pi]) as f64;
        self.pow[pi] = pw;
        // Noise between packets makes discriminator spikes many times the deviation. Clipping them keeps the averages near the
        // signal.
        let f = self
            .disc
            .process_sample(self.filter.process_sample(s))
            .clamp(-CLIP, CLIP);
        let k = (self.n % sps as u64) as usize;
        self.avg_sum += f - self.avg[k];
        self.avg[k] = f;
        let sm = self.avg_sum / sps as f32;
        let li = (self.n % self.level.len() as u64) as usize;
        self.level_sum += (sm - self.level[li]) as f64;
        self.level[li] = sm;
        let level = (self.level_sum / self.level.len() as f64) as f32;
        self.hist.push(sm);
        ((sm > level) as u8, k)
    }

    /// Counts the sample taken by [`Front::push`] and re-anchors the running sums now and then.
    #[inline]
    fn advance(&mut self) {
        self.n += 1;
        if self.n.is_multiple_of(65_536) {
            // Re-anchor the running sums so rounding cannot drift.
            self.avg_sum = self.avg.iter().sum();
            self.level_sum = self.level.iter().map(|&v| v as f64).sum();
            self.pow_sum = self.pow.iter().map(|&v| v as f64).sum();
        }
    }

    /// The mean input power over the last 40 symbols.
    pub(super) fn power(&self) -> f64 {
        self.pow_sum.max(0.0) / self.pow.len() as f64
    }

    /// The averaged discriminator at sample `pos` (which the history must hold).
    #[inline]
    pub(super) fn f(&self, pos: u64) -> f32 {
        self.hist[(pos - self.hbase) as usize]
    }

    /// Drops the history before `keep`.
    pub(super) fn trim(&mut self, keep: u64) {
        if keep > self.hbase + 65_536 {
            let d = (keep - self.hbase) as usize;
            self.hist.drain(..d);
            self.hbase = keep;
        }
    }
}

/// The 40 known bits (the last 8 preamble bits and the access address) in air order, as `+1` for bit 0 and `-1` for bit 1.
fn known_symbols(aa: u32) -> [Complex32; KNOWN] {
    let mut bits = Vec::with_capacity(KNOWN);
    pack::push_u64_lsb(&mut bits, super::preamble(aa) as u64, 8);
    pack::push_u64_lsb(&mut bits, aa as u64, 32);
    let mut s = [Complex32::default(); KNOWN];
    for (v, &b) in s.iter_mut().zip(&bits) {
        *v = Complex32::new(1.0 - 2.0 * b as f32, 0.0);
    }
    s
}

/// The channels a candidate is tried on, in order.
pub(super) fn channel_order(plan: ChannelPlan) -> ([u8; 3], usize) {
    match plan {
        ChannelPlan::Known(c) => ([c, 0, 0], 1),
        ChannelPlan::Advertising { hint: Some(38) } => ([38, 37, 39], 3),
        ChannelPlan::Advertising { hint: Some(39) } => ([39, 37, 38], 3),
        ChannelPlan::Advertising { .. } => ([37, 38, 39], 3),
    }
}

/// Whether `pdu_and_crc` carries a good CRC for `link`.
pub(super) fn link_crc_ok(pdu_and_crc: &[u8], link: &Link) -> bool {
    if link.crc_init == ADV_CRC_INIT {
        return crc_ok(pdu_and_crc);
    }
    let n = pdu_and_crc.len();
    n >= 5 && crc24_bytes(&pdu_and_crc[..n - 3], link.crc_init) == pdu_and_crc[n - 3..]
}

/// Whether a dewhitened header has a length its PDU format allows on `channel` (Vol 6 Part B clauses 2.3, 2.4 and 2.6).
/// Returns the payload length.
pub(super) fn plausible(kind: LinkKind, channel: u8, h: [u8; 2]) -> Option<usize> {
    use super::PduType::*;
    let l = h[1];
    let ok = match kind {
        LinkKind::Advertising => {
            let t = super::PduType::from_code(h[0]);
            if channel >= 37 {
                match t {
                    AdvInd | AdvNonconnInd | AdvScanInd | ScanRsp => (6..=37).contains(&l),
                    AdvDirectInd | ScanReq => l == 12,
                    ConnectInd => l == 34,
                    AdvExtInd => l >= 1,
                    AuxConnectRsp | Reserved(_) => false,
                }
            } else {
                match t {
                    AdvExtInd | AuxConnectRsp => l >= 1,
                    ScanReq => l == 12,
                    ConnectInd => l == 34,
                    _ => false,
                }
            }
        }
        LinkKind::Periodic => super::PduType::from_code(h[0]) == AdvExtInd && l >= 1,
        // LLID 0b00 is reserved.
        LinkKind::Data => h[0] & 3 != 0,
        LinkKind::Isochronous => true,
    };
    ok.then_some(l as usize)
}

/// Whether a PDU with a good CRC is well formed for its link kind.
pub(super) fn well_formed(kind: LinkKind, pdu: &[u8]) -> bool {
    match kind {
        LinkKind::Advertising | LinkKind::Periodic => AdvPdu::parse(pdu).is_ok(),
        LinkKind::Data | LinkKind::Isochronous => pdu.len() >= 2,
    }
}

/// One symbol rate: its front, the uncoded detector and, on 1 Msym/s, the coded detector.
#[derive(Clone, Debug)]
pub(super) struct Lane {
    pub(super) front: Front,
    pub(super) phy: Phy,
    links: Vec<Link>,
    known: Vec<[Complex32; KNOWN]>,
    /// `sync[link * sps + phase]`.
    sync: Vec<SyncWord>,
    holdoff: u64,
    good_end: u64,
    pending: VecDeque<Cand>,
    pub(super) channels: [u8; 3],
    pub(super) nch: usize,
    pub(super) stats: ReceiverStats,
    uncoded: bool,
    mlse: Mlse,
    y: Vec<Complex32>,
    bits: Vec<u8>,
    bytes: Vec<u8>,
    trial: Vec<u8>,
    pub(super) coded: Option<Box<CodedLane>>,
}

impl Lane {
    fn new(sps: usize, phy: Phy, uncoded: bool, coded: bool) -> Result<Lane> {
        let taps = [Complex32::new(1.0, 0.0); 3];
        let front = Front::new(sps, phy.symbol_rate())?;
        Ok(Lane {
            front,
            phy,
            links: Vec::with_capacity(MAX_LINKS),
            known: Vec::with_capacity(MAX_LINKS),
            sync: Vec::with_capacity(MAX_LINKS * sps),
            holdoff: 0,
            good_end: 0,
            pending: VecDeque::with_capacity(MAX_PENDING),
            channels: [37, 38, 39],
            nch: 3,
            stats: ReceiverStats::default(),
            uncoded,
            mlse: Mlse::new(&taps, DATA_BITS + MARGIN_BITS)?,
            y: Vec::with_capacity(DATA_BITS + MARGIN_BITS),
            bits: Vec::with_capacity(DATA_BITS + MARGIN_BITS),
            bytes: Vec::with_capacity(DATA_BITS / 8 + 1),
            trial: Vec::with_capacity(DATA_BITS / 8 + 1),
            coded: if coded {
                Some(Box::new(CodedLane::new(sps)?))
            } else {
                None
            },
        })
    }

    fn add_link(&mut self, link: Link) -> Result<()> {
        // The pattern is the access address in air order, the first bit sent most significant.
        let pat = link.access_address.reverse_bits() as u128;
        let s = SyncWord::new(pat, 32, 2)?;
        self.links.push(link);
        self.known.push(known_symbols(link.access_address));
        for _ in 0..self.front.sps {
            self.sync.push(s);
        }
        if let Some(c) = &mut self.coded {
            c.add_link(&link)?;
        }
        Ok(())
    }

    fn remove_link(&mut self, i: usize) {
        let sps = self.front.sps;
        self.links.remove(i);
        self.known.remove(i);
        self.sync.drain(i * sps..(i + 1) * sps);
        self.pending.retain(|c| c.link != i);
        for c in &mut self.pending {
            if c.link > i {
                c.link -= 1;
            }
        }
        if let Some(c) = &mut self.coded {
            c.remove_link(i);
        }
    }

    fn reset(&mut self) {
        self.front.reset();
        for s in &mut self.sync {
            s.reset();
        }
        self.pending.clear();
        self.holdoff = self.front.n + (LEVEL_BITS * self.front.sps) as u64;
        if let Some(c) = &mut self.coded {
            c.reset(self.holdoff);
        }
    }

    fn process(&mut self, x: &[Complex32], on_packet: &mut impl FnMut(&BlePacket<'_>)) {
        let sps = self.front.sps;
        let warm = (LEVEL_BITS * sps) as u64;
        for &s in x {
            let (bit, k) = self.front.push(s);
            let n = self.front.n;
            if self.uncoded {
                for li in 0..self.links.len() {
                    let Some(h) = self.sync[li * sps + k].push(bit) else {
                        continue;
                    };
                    if n < self.holdoff || n < warm {
                        continue;
                    }
                    self.stats.aa_hits += 1;
                    self.holdoff = n + sps as u64;
                    if self.pending.len() == MAX_PENDING {
                        self.pending.pop_front();
                    }
                    self.pending.push_back(Cand {
                        pos: n,
                        link: li,
                        power: self.front.power(),
                        exact: h.errors() == 0,
                        read: false,
                        targets: [(0, 0); 3],
                        count: 0,
                        stage: 0,
                    });
                }
            }
            if let Some(c) = &mut self.coded {
                c.push_symbol(&self.front, bit, k, warm);
            }
            self.front.advance();
            if self.front.n.is_multiple_of(sps as u64) {
                if !self.pending.is_empty() {
                    self.drain(on_packet);
                }
                if let Some(c) = &mut self.coded {
                    c.drain(
                        &self.front,
                        &self.links,
                        &self.channels[..self.nch],
                        &mut self.stats,
                        &mut *on_packet,
                    );
                }
            }
        }
        self.drain(on_packet);
        if let Some(c) = &mut self.coded {
            c.drain(
                &self.front,
                &self.links,
                &self.channels[..self.nch],
                &mut self.stats,
                &mut *on_packet,
            );
        }
        let mut oldest = self.pending.front().map_or(self.front.n, |c| c.pos);
        if let Some(c) = &self.coded {
            oldest = oldest.min(c.oldest(self.front.n));
        }
        let keep_back = (self.front.sps * super::coded_rx::KEEP_BACK) as u64;
        self.front.trim(oldest.saturating_sub(keep_back));
    }

    fn drain(&mut self, on_packet: &mut impl FnMut(&BlePacket<'_>)) {
        let sps = self.front.sps;
        while let Some(&c) = self.pending.front() {
            let bits = if c.read {
                c.targets[c.stage].0
            } else {
                HEADER_BITS
            };
            if self.front.n < c.pos + (sps * (bits + 3)) as u64 {
                break;
            }
            if c.pos < self.good_end {
                self.pending.pop_front();
                continue;
            }
            if !c.read {
                // The header provides a length on each channel whose whitening makes it a plausible header. Each is tried as soon
                // as its samples are in, shortest first.
                let mut t = [(0usize, 0u8); 3];
                let mut count = 0;
                let kind = self.links[c.link].kind;
                if self.detect(c, HEADER_BITS).is_some() {
                    for &ch in &self.channels[..self.nch] {
                        let mut h = [self.bytes[0], self.bytes[1]];
                        let _ = whiten(&mut h, ch);
                        if let Some(len) = plausible(kind, ch, h) {
                            t[count] = (8 * (2 + len + 3) + MARGIN_BITS, ch);
                            count += 1;
                        }
                    }
                }
                t[..count].sort_unstable_by_key(|x| x.0);
                if count == 0 {
                    self.pending.pop_front();
                } else if let Some(f) = self.pending.front_mut() {
                    f.read = true;
                    f.targets = t;
                    f.count = count;
                }
                continue;
            }
            let (bits, ch) = c.targets[c.stage];
            if self.decode(c, bits, ch, on_packet) || c.stage + 1 == c.count {
                if c.exact && self.good_end <= c.pos {
                    self.stats.crc_failures += 1;
                }
                self.pending.pop_front();
            } else if let Some(f) = self.pending.front_mut() {
                f.stage += 1;
            }
        }
    }

    /// Timing, level fit and sequence estimation of `nbits` bits after the access address of candidate `c`, packed into
    /// `self.bytes`. Returns the timing, the fit and the deviation, or `None` when the candidate is no packet.
    fn detect(
        &mut self,
        c: Cand,
        nbits: usize,
    ) -> Option<(u64, sdrkit_algo_modem::gmsk::LsFit, f32, f64)> {
        let sps = self.front.sps as u64;
        let earliest = self.front.hbase + sps * (KNOWN as u64 + 2);
        if c.pos < earliest {
            return None;
        }
        let s = self.known[c.link];
        // Timing: the widest eye over the access-address bits, from a quarter symbol before the match to three quarters after.
        let lo = -((sps / 4) as i64);
        let hi = (3 * sps / 4) as i64;
        let mut best = (f32::MIN, c.pos);
        for delta in lo..=hi {
            let p = (c.pos as i64 + delta) as u64;
            let (mut s1, mut n1, mut s0, mut n0) = (0f32, 0u32, 0f32, 0u32);
            for j in 8..KNOWN {
                let v = self.front.f(p - sps * (KNOWN - 1 - j) as u64);
                if s[j].re < 0.0 {
                    s1 += v;
                    n1 += 1;
                } else {
                    s0 += v;
                    n0 += 1;
                }
            }
            let eye = s1 / n1 as f32 - s0 / n0 as f32;
            if eye > best.0 {
                best = (eye, p);
            }
        }
        // An eye under about half the nominal deviation is no packet.
        if best.0 < 0.48 {
            return None;
        }
        let p = best.1;
        let front = &self.front;
        let v = |j: usize| front.f(p + sps * j as u64 - sps * (KNOWN as u64 - 1));
        // Levels: y(j) = level of known bit j - 1 = g0 s(j) + g1 s(j-1) + g2 s(j-2) + dc. The centre tap g1 is the deviation.
        let mut ky = [Complex32::default(); KNOWN - 1];
        for (j, y) in ky.iter_mut().enumerate() {
            *y = Complex32::new(v(j), 0.0);
        }
        let mut taps = [Complex32::default(); 3];
        let fit = ls_channel(&ky, &s[1..], &mut taps, true).ok()?;
        // Bit 1 is the higher frequency, and a 1 is the symbol -1: the centre tap is negative.
        let dev = -taps[1].re;
        if dev < 0.12 {
            return None;
        }
        let noise = fit.noise_var.max(1e-6);
        let snr = (10.0 * ((dev * dev) / noise).log10()).min(40.0) as f64;
        self.mlse.set_channel(&taps).ok()?;
        // The samples after the access address: sample m is the level of bit 39 + m.
        self.y.clear();
        for m in 0..nbits {
            let lv = v(KNOWN - 1 + m);
            self.y.push(Complex32::new(lv - fit.dc.re, 0.0));
        }
        let bit_of = |s: Complex32| (s.re < 0.0) as u32;
        let start = bit_of(s[KNOWN - 1]) << 1 | bit_of(s[KNOWN - 2]);
        self.bits.clear();
        self.bits.resize(nbits, 0);
        self.mlse
            .decode(&self.y, noise, Some(start), &mut self.bits)
            .ok()?;
        self.bytes.clear();
        pack::pack(
            &self.bits[..nbits / 8 * 8],
            BitOrder::Lsb,
            Pad::Zero,
            &mut self.bytes,
        );
        Some((p, fit, dev, snr))
    }

    /// Decides `bits` bits of candidate `c` and tries them as a packet on `channel`. True for a good packet.
    fn decode(
        &mut self,
        c: Cand,
        bits: usize,
        channel: u8,
        on_packet: &mut impl FnMut(&BlePacket<'_>),
    ) -> bool {
        let sps = self.front.sps as u64;
        let Some((p, fit, dev, snr)) = self.detect(c, bits) else {
            return false;
        };
        let total = (bits - MARGIN_BITS) / 8;
        if total > self.bytes.len() {
            return false;
        }
        let link = self.links[c.link];
        self.trial.clear();
        self.trial.extend_from_slice(&self.bytes[..total]);
        let _ = whiten(&mut self.trial, channel);
        if !link_crc_ok(&self.trial, &link) || !well_formed(link.kind, &self.trial[..total - 3]) {
            return false;
        }
        self.good_end = p + sps * 8 * total as u64;
        self.stats.packets += 1;
        let deviation = self.phy.deviation_hz();
        on_packet(&BlePacket {
            pdu: &self.trial[..total - 3],
            channel,
            sample: p.saturating_sub(self.front.delay),
            offset_hz: fit.dc.re as f64 * deviation,
            deviation_hz: dev as f64 * deviation,
            snr_db: snr,
            level_dbfs: sdrkit_algo_dsp::meter::power_to_db(c.power),
            phy: self.phy,
            coding: None,
            link,
        });
        true
    }
}

/// The LE receiver of one RF channel (module documentation).
///
/// ```
/// use sdrkit_proto_wlan_phy::ble::{BleReceiver, ChannelPlan, Link, LinkKind, Phys};
/// let mut rx = BleReceiver::new(4, Phys::ALL).unwrap();
/// rx.set_channels(ChannelPlan::Known(12)).unwrap();
/// rx.add_link(Link::new(0x5065_17A3, 0x1234_56, LinkKind::Data)).unwrap();
/// assert_eq!(rx.links().len(), 2);
/// ```
#[derive(Clone, Debug)]
pub struct BleReceiver {
    sps: usize,
    lanes: Vec<Lane>,
    links: Vec<Link>,
    plan: ChannelPlan,
}

impl BleReceiver {
    /// A receiver for input at `sps` samples per microsecond (2 to 16; `sps` MHz), of the PHYs `phys`, holding the advertising
    /// link. LE 2M needs an even `sps` of 4 or more. Errors for another `sps`, or no PHY.
    pub fn new(sps: usize, phys: Phys) -> Result<BleReceiver> {
        if !(2..=16).contains(&sps) || (phys.le_2m && (sps < 4 || !sps.is_multiple_of(2))) {
            return Err(Error::BadParameter(Param::BleSamplesPerBit));
        }
        let mut lanes = Vec::with_capacity(2);
        if phys.le_1m || phys.le_coded {
            lanes.push(Lane::new(sps, Phy::Le1M, phys.le_1m, phys.le_coded)?);
        }
        if phys.le_2m {
            lanes.push(Lane::new(sps / 2, Phy::Le2M, true, false)?);
        }
        if lanes.is_empty() {
            return Err(Error::BadParameter(Param::BleSamplesPerBit));
        }
        let mut rx = BleReceiver {
            sps,
            lanes,
            links: Vec::with_capacity(MAX_LINKS),
            plan: ChannelPlan::Advertising { hint: None },
        };
        rx.add_link(Link::ADVERTISING)?;
        Ok(rx)
    }

    /// Samples per microsecond.
    pub fn sps(&self) -> usize {
        self.sps
    }

    /// The links listened for (the advertising link first).
    pub fn links(&self) -> &[Link] {
        &self.links
    }

    /// Listens for `link` as well. A link already held is not added twice. Errors when [`MAX_LINKS`] are held.
    pub fn add_link(&mut self, link: Link) -> Result<()> {
        if self
            .links
            .iter()
            .any(|l| l.access_address == link.access_address)
        {
            return Ok(());
        }
        if self.links.len() == MAX_LINKS {
            return Err(Error::TooLong);
        }
        for lane in &mut self.lanes {
            lane.add_link(link)?;
        }
        self.links.push(link);
        Ok(())
    }

    /// Stops listening for the link with `access_address`. The advertising link stays.
    pub fn remove_link(&mut self, access_address: u32) {
        if access_address == super::ADV_ACCESS_ADDRESS {
            return;
        }
        if let Some(i) = self
            .links
            .iter()
            .position(|l| l.access_address == access_address)
        {
            self.links.remove(i);
            for lane in &mut self.lanes {
                lane.remove_link(i);
            }
        }
    }

    /// The RF channel the input is on. Errors for a hint other than 37, 38 or 39, or a channel above 39.
    pub fn set_channels(&mut self, plan: ChannelPlan) -> Result<()> {
        match plan {
            ChannelPlan::Advertising { hint: Some(h) } if !(37..=39).contains(&h) => {
                return Err(Error::BadParameter(Param::BleAdvChannel))
            }
            ChannelPlan::Known(c) if c > 39 => {
                return Err(Error::BadParameter(Param::BleChannelIndex))
            }
            _ => {}
        }
        self.plan = plan;
        let (ch, n) = channel_order(plan);
        for lane in &mut self.lanes {
            lane.channels = ch;
            lane.nch = n;
        }
        Ok(())
    }

    /// The channel plan.
    pub fn channels(&self) -> ChannelPlan {
        self.plan
    }

    /// The counters of every lane.
    pub fn stats(&self) -> ReceiverStats {
        let mut s = ReceiverStats::default();
        for lane in &self.lanes {
            s.add(lane.stats);
        }
        s
    }

    /// Input samples taken since the receiver was made.
    pub fn samples(&self) -> u64 {
        self.lanes.first().map_or(0, |l| l.front.n)
    }

    /// Forgets the signal and the waiting candidates (after a retune). The counters, the links and the sample count stay.
    pub fn reset(&mut self) {
        for lane in &mut self.lanes {
            lane.reset();
        }
    }

    /// Runs the samples `x`. The receiver calls `on_packet` for each good packet: those of one lane in time order, the LE 2M
    /// lane's after the 1 Msym/s lane's.
    pub fn process(&mut self, x: &[Complex32], mut on_packet: impl FnMut(&BlePacket<'_>)) {
        // Every lane runs at the input rate: the LE 2M lane with half the samples per symbol.
        for lane in &mut self.lanes {
            lane.process(x, &mut on_packet);
        }
    }
}

/// The LE 1M receiver of the primary advertising channels: a [`BleReceiver`] of LE 1M that holds the advertising link only.
#[derive(Clone, Debug)]
pub struct AdvReceiver {
    rx: BleReceiver,
}

impl AdvReceiver {
    /// A receiver for input at `sps` samples per bit (2 to 16), that is `sps` MHz. Errors for another `sps`.
    pub fn new(sps: usize) -> Result<AdvReceiver> {
        Ok(AdvReceiver {
            rx: BleReceiver::new(sps, Phys::LE_1M)?,
        })
    }

    /// Samples per bit.
    pub fn sps(&self) -> usize {
        self.rx.sps()
    }

    /// The channel the input is on, when known (37, 38 or 39). The receiver then tries its whitening first. Errors for another
    /// channel.
    pub fn set_channel_hint(&mut self, channel: Option<u8>) -> Result<()> {
        self.rx.set_channels(ChannelPlan::Advertising { hint: channel })
    }

    /// The counters.
    pub fn stats(&self) -> ReceiverStats {
        self.rx.stats()
    }

    /// Input samples taken since the receiver was made.
    pub fn samples(&self) -> u64 {
        self.rx.samples()
    }

    /// Forgets the signal and the waiting candidates (after a retune). The counters and the sample count stay.
    pub fn reset(&mut self) {
        self.rx.reset();
    }

    /// Runs the samples `x`. The receiver calls `on_packet` for each good packet.
    pub fn process(&mut self, x: &[Complex32], on_packet: impl FnMut(&BlePacket<'_>)) {
        self.rx.process(x, on_packet);
    }
}
