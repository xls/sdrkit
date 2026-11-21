//! The LE 1M advertising receiver. The input is complex baseband centred on one RF channel, at a whole number of samples per bit
//! (2 to 16). The receiver demodulates with an FM discriminator, finds the access address with up to 2 bit errors, decides the
//! bits with a 4-state maximum-likelihood sequence estimator and outputs the first candidate PDU with a good CRC-24.
//!
//! Standard: Bluetooth Core Specification v5.4, Vol 6 Part B.

use super::{crc_ok, whiten, AdvPdu, ADV_ACCESS_ADDRESS, DEVIATION_HZ, MAX_PAYLOAD};
use crate::{finite, Complex32, Error, Param, Result};
use sdrkit_algo_dsp::fir::{self, Fir};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_modem::gmsk::{ls_channel, Mlse};
use sdrkit_util_bits::sync::SyncWord;
use sdrkit_util_bits::{pack, BitOrder, Pad};
use std::collections::VecDeque;

/// Bits after the access address that the receiver decides: header, the longest payload and the CRC.
const DATA_BITS: usize = (2 + MAX_PAYLOAD + 3) * 8;
/// The known bits: preamble and access address.
const KNOWN: usize = 40;
/// Bits after the access address read first: the header and one byte.
const HEADER_BITS: usize = 24;
/// Bits decided past the end of a packet, so its last bits have their neighbours.
const MARGIN_BITS: usize = 4;
/// The most candidates that wait for their samples.
const MAX_PENDING: usize = 64;
/// The largest discriminator value kept, in units of the nominal deviation.
const CLIP: f32 = 1.5;
/// Bits of the slow average that sets the slicing level.
const LEVEL_BITS: usize = 40;

/// A good advertising packet. It borrows the receiver's buffer for the time of the callback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlePacket<'a> {
    /// The PDU (header and payload), dewhitened, without the CRC.
    pub pdu: &'a [u8],
    /// The advertising channel whose whitening provided the good CRC (37, 38 or 39).
    pub channel: u8,
    /// The input sample index of the last bit of the access address.
    pub sample: u64,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The fitted deviation of an isolated bit, Hz.
    pub deviation_hz: f64,
    /// The signal-to-noise ratio of the fitted levels, dB (40 dB at most).
    pub snr_db: f64,
    /// The mean power of the input over the preamble and access address, dBFS (dB relative to full scale).
    pub level_dbfs: f64,
}

impl BlePacket<'_> {
    /// The parsed PDU.
    pub fn parse(&self) -> Result<AdvPdu<'_>> {
        AdvPdu::parse(self.pdu)
    }
}

/// The receiver's counters since it was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReceiverStats {
    /// Access-address matches with at most 2 bit errors.
    pub aa_hits: u64,
    /// Exact access-address matches whose CRC failed on every channel.
    pub crc_failures: u64,
    /// Good packets.
    pub packets: u64,
}

#[derive(Clone, Copy, Debug)]
struct Cand {
    pos: u64,
    /// The mean input power `|x|^2` over the 40 bits before the match.
    power: f64,
    exact: bool,
    /// The header has been read: `targets[..count]` are (bits to decide, channel) in increasing order, and `stage` is the next.
    read: bool,
    targets: [(usize, u8); 3],
    count: usize,
    stage: usize,
}

/// The LE 1M advertising receiver (module documentation).
#[derive(Clone, Debug)]
pub struct AdvReceiver {
    sps: usize,
    filter: Fir<Complex32>,
    delay: u64,
    disc: Discriminator,
    n: u64,
    avg: Vec<f32>,
    avg_sum: f32,
    level: Vec<f32>,
    level_sum: f64,
    pow: Vec<f32>,
    pow_sum: f64,
    sync: Vec<SyncWord>,
    holdoff: u64,
    good_end: u64,
    hist: Vec<f32>,
    hbase: u64,
    pending: VecDeque<Cand>,
    channels: [u8; 3],
    stats: ReceiverStats,
    mlse: Mlse,
    y: Vec<Complex32>,
    s: [Complex32; KNOWN],
    bits: Vec<u8>,
    bytes: Vec<u8>,
    trial: Vec<u8>,
}

/// The 40 known bits (preamble and access address) in air order, as `+1` for bit 0 and `-1` for bit 1.
fn known_symbols() -> [Complex32; KNOWN] {
    let mut bits = Vec::with_capacity(KNOWN);
    pack::push_u64_lsb(&mut bits, super::preamble(ADV_ACCESS_ADDRESS) as u64, 8);
    pack::push_u64_lsb(&mut bits, ADV_ACCESS_ADDRESS as u64, 32);
    let mut s = [Complex32::default(); KNOWN];
    for (v, &b) in s.iter_mut().zip(&bits) {
        *v = Complex32::new(1.0 - 2.0 * b as f32, 0.0);
    }
    s
}

impl AdvReceiver {
    /// A receiver for input at `sps` samples per bit (2 to 16), that is `sps` MHz. Errors for another `sps`.
    pub fn new(sps: usize) -> Result<AdvReceiver> {
        if !(2..=16).contains(&sps) {
            return Err(Error::BadParameter(Param::BleSamplesPerBit));
        }
        // The pattern is the access address in air order, the first bit sent most significant.
        let pat = (ADV_ACCESS_ADDRESS.reverse_bits()) as u128;
        let sync = SyncWord::new(pat, 32, 2)?;
        let taps = [Complex32::new(1.0, 0.0); 3];
        let rate = sps as f64 * 1e6;
        let lp = fir::lowpass(rate, 550e3, (1.1e6f64).min(rate / 2.0), 40.0)
            .map_err(|_| Error::BadParameter(Param::FilterDesign))?;
        Ok(AdvReceiver {
            sps,
            delay: (lp.len() as u64 - 1) / 2,
            filter: Fir::new(&lp),
            disc: Discriminator::new(sps as f64 * 1e6, DEVIATION_HZ)
                .map_err(|_| Error::BadParameter(Param::FilterDesign))?,
            n: 0,
            avg: vec![0.0; sps],
            avg_sum: 0.0,
            level: vec![0.0; LEVEL_BITS * sps],
            level_sum: 0.0,
            pow: vec![0.0; KNOWN * sps],
            pow_sum: 0.0,
            sync: vec![sync; sps],
            holdoff: 0,
            good_end: 0,
            hist: Vec::with_capacity(1 << 17),
            hbase: 0,
            pending: VecDeque::with_capacity(MAX_PENDING),
            channels: [37, 38, 39],
            stats: ReceiverStats::default(),
            mlse: Mlse::new(&taps, DATA_BITS + MARGIN_BITS)?,
            y: Vec::with_capacity(DATA_BITS + MARGIN_BITS),
            s: known_symbols(),
            bits: Vec::with_capacity(DATA_BITS + MARGIN_BITS),
            bytes: Vec::with_capacity(DATA_BITS / 8 + 1),
            trial: Vec::with_capacity(DATA_BITS / 8 + 1),
        })
    }

    /// Samples per bit.
    pub fn sps(&self) -> usize {
        self.sps
    }

    /// The channel the input is on, when known (37, 38 or 39). The receiver then tries its whitening first. Errors for another
    /// channel.
    pub fn set_channel_hint(&mut self, channel: Option<u8>) -> Result<()> {
        self.channels = match channel {
            None => [37, 38, 39],
            Some(37) => [37, 38, 39],
            Some(38) => [38, 37, 39],
            Some(39) => [39, 37, 38],
            Some(_) => return Err(Error::BadParameter(Param::BleAdvChannel)),
        };
        Ok(())
    }

    /// The counters.
    pub fn stats(&self) -> ReceiverStats {
        self.stats
    }

    /// Input samples taken since the receiver was made.
    pub fn samples(&self) -> u64 {
        self.n
    }

    /// Forgets the signal and the waiting candidates (after a retune). The counters and the sample count stay.
    pub fn reset(&mut self) {
        self.disc.reset();
        self.filter.reset();
        self.avg.fill(0.0);
        self.avg_sum = 0.0;
        self.level.fill(0.0);
        self.level_sum = 0.0;
        self.pow.fill(0.0);
        self.pow_sum = 0.0;
        for s in &mut self.sync {
            s.reset();
        }
        self.pending.clear();
        self.holdoff = self.n + (LEVEL_BITS * self.sps) as u64;
    }

    #[inline]
    fn f(&self, pos: u64) -> f32 {
        self.hist[(pos - self.hbase) as usize]
    }

    /// Runs the samples `x`. The receiver calls `on_packet` for each good packet.
    pub fn process(&mut self, x: &[Complex32], mut on_packet: impl FnMut(&BlePacket<'_>)) {
        let sps = self.sps;
        let inv = 1.0 / sps as f32;
        let warm = (LEVEL_BITS * sps) as u64;
        for &s in x {
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
            let sm = self.avg_sum * inv;
            let li = (self.n % self.level.len() as u64) as usize;
            self.level_sum += (sm - self.level[li]) as f64;
            self.level[li] = sm;
            let level = (self.level_sum / self.level.len() as f64) as f32;
            self.hist.push(sm);
            let bit = (sm > level) as u8;
            let hit = self.sync[k].push(bit);
            if let Some(h) = hit {
                if self.n >= self.holdoff && self.n >= warm {
                    self.stats.aa_hits += 1;
                    self.holdoff = self.n + sps as u64;
                    if self.pending.len() == MAX_PENDING {
                        self.pending.pop_front();
                    }
                    self.pending.push_back(Cand {
                        pos: self.n,
                        power: self.pow_sum.max(0.0) / self.pow.len() as f64,
                        exact: h.errors() == 0,
                        read: false,
                        targets: [(0, 0); 3],
                        count: 0,
                        stage: 0,
                    });
                }
            }
            self.n += 1;
            if self.n.is_multiple_of(65_536) {
                // Re-anchor the running sums so rounding cannot drift.
                self.avg_sum = self.avg.iter().sum();
                self.level_sum = self.level.iter().map(|&v| v as f64).sum();
                self.pow_sum = self.pow.iter().map(|&v| v as f64).sum();
            }
            if self.n.is_multiple_of(sps as u64) && !self.pending.is_empty() {
                self.drain(&mut on_packet);
            }
        }
        self.drain(&mut on_packet);
        self.trim();
    }

    fn drain(&mut self, on_packet: &mut impl FnMut(&BlePacket<'_>)) {
        while let Some(&c) = self.pending.front() {
            let bits = if c.read {
                c.targets[c.stage].0
            } else {
                HEADER_BITS
            };
            if self.n < c.pos + (self.sps * (bits + 3)) as u64 {
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
                if self.detect(c, HEADER_BITS).is_some() {
                    for ch in self.channels {
                        self.trial.clear();
                        self.trial.extend_from_slice(&self.bytes[..2]);
                        let _ = whiten(&mut self.trial, ch);
                        if let Some(h) = super::Header::parse(&self.trial) {
                            if plausible(&h) {
                                t[count] = (8 * (2 + h.length as usize + 3) + MARGIN_BITS, ch);
                                count += 1;
                            }
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

    fn trim(&mut self) {
        let oldest = self.pending.front().map_or(self.n, |c| c.pos);
        let keep = oldest.saturating_sub((self.sps * (KNOWN + 8)) as u64);
        if keep > self.hbase + 65_536 {
            let d = (keep - self.hbase) as usize;
            self.hist.drain(..d);
            self.hbase = keep;
        }
    }

    /// Timing, level fit and sequence estimation of `nbits` bits after the access address of candidate `c`, packed into
    /// `self.bytes`. Returns the timing, the fit and the deviation, or `None` when the candidate is no packet.
    fn detect(
        &mut self,
        c: Cand,
        nbits: usize,
    ) -> Option<(u64, sdrkit_algo_modem::gmsk::LsFit, f32, f64)> {
        let sps = self.sps as u64;
        let earliest = self.hbase + sps * (KNOWN as u64 + 2);
        if c.pos < earliest {
            return None;
        }
        // Timing: the widest eye over the access-address bits, from a quarter bit before the match to three quarters after.
        let lo = -((sps / 4) as i64);
        let hi = (3 * sps / 4) as i64;
        let mut best = (f32::MIN, c.pos);
        for delta in lo..=hi {
            let p = (c.pos as i64 + delta) as u64;
            let (mut s1, mut n1, mut s0, mut n0) = (0f32, 0u32, 0f32, 0u32);
            for j in 8..KNOWN {
                let v = self.f(p - sps * (KNOWN - 1 - j) as u64);
                if self.s[j].re < 0.0 {
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
        let v = |rx: &AdvReceiver, j: usize| rx.f(p + sps * j as u64 - sps * (KNOWN as u64 - 1));
        // Levels: y(j) = level of known bit j - 1 = g0 s(j) + g1 s(j-1) + g2 s(j-2) + dc. The centre tap g1 is the deviation.
        let mut ky = [Complex32::default(); KNOWN - 1];
        for (j, y) in ky.iter_mut().enumerate() {
            *y = Complex32::new(v(self, j), 0.0);
        }
        let mut taps = [Complex32::default(); 3];
        let fit = ls_channel(&ky, &self.s[1..], &mut taps, true).ok()?;
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
            let lv = v(self, KNOWN - 1 + m);
            self.y.push(Complex32::new(lv - fit.dc.re, 0.0));
        }
        let bit_of = |s: Complex32| (s.re < 0.0) as u32;
        let start = bit_of(self.s[KNOWN - 1]) << 1 | bit_of(self.s[KNOWN - 2]);
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
        let sps = self.sps as u64;
        let Some((p, fit, dev, snr)) = self.detect(c, bits) else {
            return false;
        };
        let total = (bits - MARGIN_BITS) / 8;
        if total > self.bytes.len() {
            return false;
        }
        self.trial.clear();
        self.trial.extend_from_slice(&self.bytes[..total]);
        let _ = whiten(&mut self.trial, channel);
        if !crc_ok(&self.trial) || AdvPdu::parse(&self.trial[..total - 3]).is_err() {
            return false;
        }
        self.good_end = p + sps * 8 * total as u64;
        self.stats.packets += 1;
        on_packet(&BlePacket {
            pdu: &self.trial[..total - 3],
            channel,
            sample: p.saturating_sub(self.delay),
            offset_hz: fit.dc.re as f64 * DEVIATION_HZ,
            deviation_hz: dev as f64 * DEVIATION_HZ,
            snr_db: snr,
            level_dbfs: sdrkit_algo_dsp::meter::power_to_db(c.power),
        });
        true
    }
}

/// Whether a dewhitened header has a length its PDU type allows (Vol 6 Part B clause 2.3).
fn plausible(h: &super::Header) -> bool {
    use super::PduType::*;
    let l = h.length;
    match h.pdu_type {
        AdvInd | AdvNonconnInd | AdvScanInd | ScanRsp => (6..=37).contains(&l),
        AdvDirectInd | ScanReq => l == 12,
        ConnectInd => l == 34,
        AdvExtInd | AuxConnectRsp => l >= 1,
        Reserved(_) => false,
    }
}
