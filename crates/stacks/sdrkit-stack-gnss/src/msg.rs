//! The message layers of a tracking channel: bit or symbol sync, frame sync, the checks, and the time derived from each unit.
//! A layer gets the prompt of every code period (an epoch) once the carrier is locked, and returns events.

use crate::codes::{GLO_TIME_MARK, NH20};
use crate::fec::{glonass, SoftViterbi, K7_171_133};
use crate::inav::{self, PartDecoder};
use crate::{bds, lnav, sbas};
use sdrkit_algo_fec::conv::Ends;
use sdrkit_algo_fec::small::GlonassCheck;
use sdrkit_core::Complex32;
use serde::{Deserialize, Serialize};

/// What a decoded unit holds (the `gnss.nav_frame` stream).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NavUnit {
    /// An LNAV subframe: 240 data bits (parity removed), its ID and TOW count.
    Lnav {
        data: Vec<u8>,
        id: u8,
        tow_count: u32,
    },
    /// An SBAS message: 250 bits and its type.
    Sbas { bits: Vec<u8>, msg_type: u8 },
    /// An I/NAV word (128 bits) and its type.
    Inav { word: Vec<u8>, word_type: u8 },
    /// A BeiDou subframe: 224 information bits after correction.
    Bds {
        info: Vec<u8>,
        id: u8,
        sow: u32,
        d2: bool,
    },
    /// A GLONASS string (85 bits, bit 85 first) after the Hamming check.
    Glo { bits: Vec<u8>, number: u32 },
    /// A NavIC subframe whose CRC passed.
    Navic(crate::navic::code::Subframe),
}

/// An event from a layer.
#[derive(Clone, Debug, PartialEq)]
pub enum MsgEvent {
    /// A unit passed its check. `start_epoch`: the epoch its first symbol began in; `time`: the satellite time of that epoch
    /// (seconds of week; of day for GLONASS) when the unit carries one.
    Unit {
        unit: NavUnit,
        start_epoch: u64,
        time: Option<f64>,
    },
    /// A unit failed its check (counted bad).
    Bad,
    /// The layer found the carrier locked off by this much (Hz, sign as the layer measured) and requests the channel to move.
    CarrierStep(f64),
    /// A false lock half the epoch rate away was seen: move by half the epoch rate towards the starting frequency.
    FalseLock,
    /// Frame sync was lost: drop the time.
    LostFrame,
    Log(String),
}

/// Sync level a layer reached.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Sync {
    #[default]
    None,
    Bit,
    Frame,
}

/// Bit sync by sign changes counted per position modulo `period` (GPS: 20 epochs; GLONASS symbols: 10).
#[derive(Clone, Debug)]
struct EdgeCounter {
    period: usize,
    hist: Vec<u32>,
    changes: u32,
    prev: f32,
    min_changes: u32,
    min_best: u32,
    clear_after: u32,
}

impl EdgeCounter {
    fn new(period: usize, min_changes: u32, min_best: u32, clear_after: u32) -> Self {
        EdgeCounter {
            period,
            hist: vec![0; period],
            changes: 0,
            prev: 0.0,
            min_changes,
            min_best,
            clear_after,
        }
    }
    fn clear(&mut self) {
        self.hist.iter_mut().for_each(|h| *h = 0);
        self.changes = 0;
    }
    /// Feeds value `v` of epoch `e`; returns the phase (position modulo period where a bit starts) once found.
    fn feed(&mut self, e: u64, v: f32) -> Option<usize> {
        let changed = self.prev != 0.0 && (v < 0.0) != (self.prev < 0.0);
        self.prev = v;
        if changed {
            self.hist[(e % self.period as u64) as usize] += 1;
            self.changes += 1;
        }
        if self.changes >= self.min_changes {
            let mut idx: Vec<usize> = (0..self.period).collect();
            idx.sort_by_key(|&i| std::cmp::Reverse(self.hist[i]));
            let (b, s) = (self.hist[idx[0]], self.hist[idx[1]]);
            if b >= self.min_best && b >= 3 * s {
                return Some(idx[0]);
            }
            if self.clear_after > 0 && self.changes > self.clear_after {
                self.clear();
            }
        }
        None
    }
}

/// Symbol sync by comparing the power of pair sums at even and odd starts (SBAS, BeiDou D2).
#[derive(Clone, Debug, Default)]
struct PairSync {
    prev: f32,
    pow: [f64; 2],
    count: [u32; 2],
}

impl PairSync {
    fn clear(&mut self) {
        self.pow = [0.0; 2];
        self.count = [0; 2];
    }
    /// Feeds epoch `e`; returns the parity of the epoch that starts a symbol once decided.
    fn feed(&mut self, e: u64, v: f32) -> Option<usize> {
        let s = self.prev + v;
        self.prev = v;
        // The pair (e-1, e) starts at parity (e - 1) % 2.
        let par = ((e + 1) % 2) as usize;
        self.pow[par] += (s as f64) * (s as f64);
        self.count[par] += 1;
        if self.count[0] >= 1000 && self.count[1] >= 1000 {
            let (a, b) = (self.pow[0], self.pow[1]);
            let big = a.max(b);
            if (a - b).abs() > 0.12 * big {
                return Some(if a > b { 0 } else { 1 });
            }
            self.clear();
        }
        None
    }
}

/// GPS and QZSS LNAV.
#[derive(Clone, Debug)]
pub struct LnavLayer {
    edges: EdgeCounter,
    phase: Option<usize>,
    acc: f32,
    acc_n: u32,
    bits: Vec<u8>,
    bit_epochs: Vec<u64>,
    framed: bool,
    since_frame: usize,
    bad_run: u32,
    sync_epoch: u64,
}

impl Default for LnavLayer {
    fn default() -> Self {
        LnavLayer {
            edges: EdgeCounter::new(20, 24, 12, 0),
            phase: None,
            acc: 0.0,
            acc_n: 0,
            bits: Vec::with_capacity(1200),
            bit_epochs: Vec::with_capacity(1200),
            framed: false,
            since_frame: 0,
            bad_run: 0,
            sync_epoch: 0,
        }
    }
}

impl LnavLayer {
    fn epoch(&mut self, e: u64, p: Complex32, out: &mut Vec<MsgEvent>) {
        let v = p.re;
        let Some(ph) = self.phase else {
            if let Some(ph) = self.edges.feed(e, v) {
                self.phase = Some(ph);
                self.sync_epoch = e;
                self.acc = 0.0;
                self.acc_n = 0;
                out.push(MsgEvent::Log("bit sync".into()));
            }
            return;
        };
        if (e % 20) as usize == ph && self.acc_n > 0 {
            if self.acc_n == 20 {
                self.bits.push((self.acc < 0.0) as u8);
                self.bit_epochs.push(e - 20);
                self.on_bit(e, out);
            }
            self.acc = 0.0;
            self.acc_n = 0;
        }
        self.acc += v;
        self.acc_n += 1;
        if !self.framed && e.saturating_sub(self.sync_epoch) > 40_000 {
            self.reset();
            out.push(MsgEvent::Log(
                "no frame within 40 s of the bit sync: bit sync again".into(),
            ));
        }
    }
    fn on_bit(&mut self, _e: u64, out: &mut Vec<MsgEvent>) {
        if self.bits.len() > 1200 {
            self.bits.drain(..600);
            self.bit_epochs.drain(..600);
        }
        let n = self.bits.len();
        if n < 300 {
            return;
        }
        let last = &self.bits[n - 300..];
        if !self.framed {
            if let Some(sf) = lnav::check_subframe(last) {
                self.framed = true;
                self.since_frame = 0;
                self.bad_run = 0;
                out.push(MsgEvent::Log(format!("frame sync (subframe {})", sf.id)));
                self.emit(sf, self.bit_epochs[n - 300], out);
            }
            return;
        }
        self.since_frame += 1;
        if self.since_frame < 300 {
            return;
        }
        self.since_frame = 0;
        match lnav::check_subframe(last) {
            Some(sf) => {
                self.bad_run = 0;
                self.emit(sf, self.bit_epochs[n - 300], out);
            }
            None => {
                out.push(MsgEvent::Bad);
                self.bad_run += 1;
                if self.bad_run >= 4 {
                    self.framed = false;
                    out.push(MsgEvent::LostFrame);
                    out.push(MsgEvent::Log("four bad subframes: frame sync lost".into()));
                }
            }
        }
    }
    fn emit(&self, sf: lnav::Subframe, start_epoch: u64, out: &mut Vec<MsgEvent>) {
        let time = sf.start_tow();
        out.push(MsgEvent::Unit {
            unit: NavUnit::Lnav {
                data: sf.data,
                id: sf.id,
                tow_count: sf.tow_count,
            },
            start_epoch,
            time: Some(time),
        });
    }
    fn reset(&mut self) {
        *self = LnavLayer::default();
    }
    fn sync(&self) -> Sync {
        if self.framed {
            Sync::Frame
        } else if self.phase.is_some() {
            Sync::Bit
        } else {
            Sync::None
        }
    }
}

/// SBAS: symbol sync, Viterbi, message search.
#[derive(Clone, Debug)]
pub struct SbasLayer {
    pairs: PairSync,
    parity: Option<usize>,
    prev_v: f32,
    soft: Vec<f32>,
    /// Absolute symbol number of `soft[0]`, and the epoch each symbol began.
    first_sym: u64,
    sym_epoch: Vec<u64>,
    new_syms: usize,
    align: Option<usize>,
    viterbi: SoftViterbi,
    word: Vec<u8>,
    norm: Vec<f32>,
    dec: Vec<u8>,
    last_start: Option<u64>,
    framed: bool,
    sync_epoch: u64,
    quiet_s: u32,
    quiet_bad: u32,
}

impl Default for SbasLayer {
    fn default() -> Self {
        SbasLayer {
            pairs: PairSync::default(),
            parity: None,
            prev_v: 0.0,
            soft: Vec::with_capacity(2400),
            first_sym: 0,
            sym_epoch: Vec::with_capacity(2400),
            new_syms: 0,
            align: None,
            viterbi: SoftViterbi::new(K7_171_133, 600),
            word: Vec::with_capacity(250),
            norm: Vec::with_capacity(1200),
            dec: Vec::with_capacity(600),
            last_start: None,
            framed: false,
            sync_epoch: 0,
            quiet_s: 0,
            quiet_bad: 0,
        }
    }
}

impl SbasLayer {
    fn epoch(&mut self, e: u64, p: Complex32, out: &mut Vec<MsgEvent>) {
        let v = p.re;
        let Some(par) = self.parity else {
            if let Some(par) = self.pairs.feed(e, v) {
                self.parity = Some(par);
                self.sync_epoch = e;
                out.push(MsgEvent::Log("symbol sync".into()));
            }
            self.prev_v = v;
            return;
        };
        if (e % 2) as usize != par {
            // The second epoch of a symbol.
            self.soft.push(self.prev_v + v);
            self.sym_epoch.push(e - 1);
            self.new_syms += 1;
            if self.soft.len() > 2400 {
                self.soft.drain(..1200);
                self.sym_epoch.drain(..1200);
                self.first_sym += 1200;
            }
            if self.new_syms >= 500 {
                self.new_syms = 0;
                self.search(out);
            }
        }
        self.prev_v = v;
        if !self.framed && e.saturating_sub(self.sync_epoch) > 20_000 {
            *self = SbasLayer::default();
            out.push(MsgEvent::Log(
                "no message within 20 s of the symbol sync: symbol sync again".into(),
            ));
        }
    }

    fn search(&mut self, out: &mut Vec<MsgEvent>) {
        let n = self.soft.len().min(1200);
        if n < 600 {
            return;
        }
        let base = self.soft.len() - n;
        let mean = self.soft[base..].iter().map(|s| s.abs()).sum::<f32>() / n as f32;
        let k = 1.0 / mean.max(1e-9);
        let (aligns, count) = match self.align {
            Some(a) => ([a, 0], 1),
            None => ([0, 1], 2),
        };
        let mut found_any = false;
        for &a in &aligns[..count] {
            self.norm.clear();
            self.norm
                .extend(self.soft[base + a..].iter().map(|s| s * k));
            if self.norm.len() % 2 == 1 {
                self.norm.pop();
            }
            self.viterbi.decode(&self.norm, Ends::Open, &mut self.dec);
            let nb = self.dec.len();
            if nb < 250 + 48 {
                continue;
            }
            let mut j = 24;
            while j + 250 + 24 <= nb {
                let mut hit = None;
                for inv in [0u8, 1] {
                    self.word.clear();
                    self.word
                        .extend(self.dec[j..j + 250].iter().map(|b| b ^ inv));
                    if let Some((ty, _)) = sbas::check(&self.word) {
                        hit = Some(ty);
                        break;
                    }
                }
                if let Some(ty) = hit {
                    let w = self.word.clone();
                    found_any = true;
                    self.align = Some(a);
                    let sym_abs = self.first_sym + (base + a + 2 * j) as u64;
                    if self.last_start.is_none_or(|l| sym_abs > l + 10) {
                        if let Some(l) = self.last_start {
                            let missed = ((sym_abs - l) as f64 / 500.0).round() as i64 - 1;
                            for _ in 0..missed.max(0) {
                                out.push(MsgEvent::Bad);
                            }
                        }
                        self.last_start = Some(sym_abs);
                        if !self.framed {
                            out.push(MsgEvent::Log("frame sync (SBAS message)".into()));
                        }
                        self.framed = true;
                        let epoch = self.sym_epoch[base + a + 2 * j];
                        out.push(MsgEvent::Unit {
                            unit: NavUnit::Sbas {
                                bits: w,
                                msg_type: ty,
                            },
                            start_epoch: epoch,
                            time: None,
                        });
                    }
                    j += 250;
                } else {
                    j += 1;
                }
            }
            if found_any {
                break;
            }
        }
        if self.framed {
            if found_any {
                self.quiet_s = 0;
                self.quiet_bad = 0;
            } else {
                self.quiet_s += 1;
                if self.quiet_s >= 4 {
                    self.quiet_s = 0;
                    out.push(MsgEvent::Bad);
                    self.quiet_bad += 1;
                    if self.quiet_bad >= 3 {
                        self.framed = false;
                        self.align = None;
                        self.quiet_bad = 0;
                        out.push(MsgEvent::LostFrame);
                    }
                }
            }
        }
    }
    fn sync(&self) -> Sync {
        if self.framed {
            Sync::Frame
        } else if self.parity.is_some() {
            Sync::Bit
        } else {
            Sync::None
        }
    }
}

/// Galileo I/NAV on E1-B.
pub struct InavLayer {
    soft: Vec<f32>,
    epochs: Vec<u64>,
    last_epoch: Option<u64>,
    polarity: Option<u8>,
    /// Index in `soft` of the next part's first (sync) symbol, when in sync.
    next_part: Option<usize>,
    misses: u32,
    bad_run: u32,
    even: Option<(Vec<u8>, u64)>,
    decoder: PartDecoder,
    coded: Vec<f32>,
    locked_symbols: u32,
    try_k: usize,
    framed: bool,
}

impl Default for InavLayer {
    fn default() -> Self {
        InavLayer {
            soft: Vec::with_capacity(2000),
            epochs: Vec::with_capacity(2000),
            last_epoch: None,
            polarity: None,
            next_part: None,
            misses: 0,
            bad_run: 0,
            even: None,
            decoder: PartDecoder::default(),
            coded: vec![0.0; 240],
            locked_symbols: 0,
            try_k: 0,
            framed: false,
        }
    }
}

/// The false-lock trial steps (multiples of 125 Hz from where the carrier stood): +1, −1, +2, −2, then back.
const INAV_TRIALS: [f64; 5] = [1.0, -1.0, 2.0, -2.0, 0.0];

impl InavLayer {
    fn hard(&self, i: usize) -> u8 {
        (self.soft[i] < 0.0) as u8
    }
    fn pattern_at(&self, i: usize, pol: u8) -> usize {
        (0..10)
            .filter(|&k| self.hard(i + k) ^ pol == inav::SYNC[k])
            .count()
    }
    fn epoch(&mut self, e: u64, p: Complex32, out: &mut Vec<MsgEvent>) {
        if let Some(l) = self.last_epoch {
            if e != l + 1 {
                self.soft.clear();
                self.epochs.clear();
                self.next_part = None;
            }
        }
        self.last_epoch = Some(e);
        self.soft.push(p.re);
        self.epochs.push(e);
        if self.soft.len() > 2000 {
            self.soft.drain(..1000);
            self.epochs.drain(..1000);
            if let Some(np) = self.next_part.as_mut() {
                *np = np.saturating_sub(1000);
            }
        }
        let n = self.soft.len();
        if self.next_part.is_none() {
            self.locked_symbols += 1;
            if n >= 260 {
                let a = n - 260;
                let b = n - 10;
                for pol in [0u8, 1] {
                    if self.polarity.is_some_and(|p| p != pol) {
                        continue;
                    }
                    if self.pattern_at(a, pol) == 10 && self.pattern_at(b, pol) == 10 {
                        self.polarity = Some(pol);
                        self.next_part = Some(a);
                        self.misses = 0;
                        self.framed = true;
                        out.push(MsgEvent::Log("page sync".into()));
                        break;
                    }
                }
            }
            if self.next_part.is_none() && self.locked_symbols >= 1000 {
                // Try the carrier half the epoch rate away, and further.
                self.locked_symbols = 0;
                let prev = if self.try_k == 0 {
                    0.0
                } else {
                    INAV_TRIALS[self.try_k - 1]
                };
                let next = INAV_TRIALS[self.try_k % INAV_TRIALS.len()];
                self.try_k = (self.try_k + 1) % INAV_TRIALS.len();
                out.push(MsgEvent::CarrierStep((next - prev) * 125.0));
            }
        }
        // Decode every complete part.
        while let Some(start) = self.next_part {
            if start + 250 > self.soft.len() {
                break;
            }
            let pol = self.polarity.unwrap_or(0);
            if self.pattern_at(start, pol) < 8 {
                self.misses += 1;
                if self.misses >= 3 {
                    self.next_part = None;
                    self.framed = false;
                    self.even = None;
                    out.push(MsgEvent::LostFrame);
                    out.push(MsgEvent::Log("page sync lost".into()));
                    break;
                }
            } else {
                self.misses = 0;
            }
            let s = if pol == 1 { -1.0 } else { 1.0 };
            for k in 0..240 {
                self.coded[k] = self.soft[start + 10 + k] * s;
            }
            let bits = self.decoder.decode(&self.coded).to_vec();
            let ep = self.epochs[start];
            if bits[0] == 0 {
                self.even = Some((bits, ep));
            } else if let Some((even, even_ep)) = self.even.take() {
                match inav::assemble(&even, &bits) {
                    Some(page) => {
                        self.bad_run = 0;
                        let time = match inav::decode_word(&page.word) {
                            inav::InavWord::W0 { tow, .. }
                            | inav::InavWord::W5 { tow, .. }
                            | inav::InavWord::W6 { tow, .. } => Some(tow as f64),
                            _ => None,
                        };
                        out.push(MsgEvent::Unit {
                            unit: NavUnit::Inav {
                                word: page.word,
                                word_type: page.word_type,
                            },
                            start_epoch: even_ep,
                            time,
                        });
                    }
                    None => {
                        out.push(MsgEvent::Bad);
                        self.bad_run += 1;
                        if self.bad_run >= 6 {
                            self.next_part = None;
                            self.framed = false;
                            out.push(MsgEvent::LostFrame);
                            break;
                        }
                    }
                }
            }
            self.next_part = Some(start + 250);
        }
    }
    fn sync(&self) -> Sync {
        if self.framed {
            Sync::Frame
        } else {
            Sync::None
        }
    }
}

/// BeiDou D1 (NH code) and D2.
#[derive(Clone, Debug)]
pub struct BdsLayer {
    d2: bool,
    hist: Vec<f32>,
    plain: [f64; 20],
    alt: [f64; 20],
    count: u32,
    pairs: PairSync,
    phase: Option<usize>,
    prev_v: f32,
    acc: f32,
    acc_n: usize,
    bits: Vec<u8>,
    bit_epochs: Vec<u64>,
    framed: bool,
    since_frame: usize,
    bad_run: u32,
    sync_epoch: u64,
}

impl BdsLayer {
    pub fn new(d2: bool) -> Self {
        BdsLayer {
            d2,
            hist: Vec::with_capacity(20),
            plain: [0.0; 20],
            alt: [0.0; 20],
            count: 0,
            pairs: PairSync::default(),
            phase: None,
            prev_v: 0.0,
            acc: 0.0,
            acc_n: 0,
            bits: Vec::with_capacity(1200),
            bit_epochs: Vec::with_capacity(1200),
            framed: false,
            since_frame: 0,
            bad_run: 0,
            sync_epoch: 0,
        }
    }
    fn epoch(&mut self, e: u64, p: Complex32, out: &mut Vec<MsgEvent>) {
        let v = p.re;
        let per = if self.d2 { 2 } else { 20 };
        let Some(ph) = self.phase else {
            if self.d2 {
                if let Some(par) = self.pairs.feed(e, v) {
                    self.phase = Some(par);
                }
            } else {
                self.hist.push(v);
                if self.hist.len() > 20 {
                    self.hist.remove(0);
                }
                if self.hist.len() == 20 {
                    // A bit starting at epoch e - 19.
                    let ph = ((e - 19) % 20) as usize;
                    let (mut a, mut b) = (0f32, 0f32);
                    for i in 0..20 {
                        let s = if NH20[i] == 0 { 1.0 } else { -1.0 };
                        a += self.hist[i] * s;
                        b += self.hist[i] * s * if i % 2 == 0 { 1.0 } else { -1.0 };
                    }
                    self.plain[ph] += a.abs() as f64;
                    self.alt[ph] += b.abs() as f64;
                    self.count += 1;
                    if self.count >= 1000 {
                        let mut idx: Vec<usize> = (0..20).collect();
                        idx.sort_by(|&x, &y| self.plain[y].total_cmp(&self.plain[x]));
                        let best_alt = self.alt.iter().cloned().fold(0.0, f64::max);
                        let (b0, b1) = (self.plain[idx[0]], self.plain[idx[1]]);
                        if best_alt > 1.3 * b0 {
                            out.push(MsgEvent::FalseLock);
                        } else if b0 > 1.3 * b1 {
                            self.phase = Some(idx[0]);
                        }
                        self.plain = [0.0; 20];
                        self.alt = [0.0; 20];
                        self.count = 0;
                    }
                }
            }
            if let Some(_ph) = self.phase {
                self.sync_epoch = e;
                self.acc = 0.0;
                self.acc_n = 0;
                out.push(MsgEvent::Log("bit sync".into()));
            }
            self.prev_v = v;
            return;
        };
        let pos = ((e + per as u64 - ph as u64) % per as u64) as usize;
        if pos == 0 {
            self.acc = 0.0;
            self.acc_n = 0;
        }
        let s = if self.d2 || NH20[pos] == 0 { 1.0 } else { -1.0 };
        self.acc += v * s;
        self.acc_n += 1;
        if pos == per - 1 && self.acc_n == per {
            self.bits.push((self.acc < 0.0) as u8);
            self.bit_epochs.push(e + 1 - per as u64);
            self.on_bit(out);
        }
        self.prev_v = v;
        if !self.framed && e.saturating_sub(self.sync_epoch) > 40_000 {
            *self = BdsLayer::new(self.d2);
            out.push(MsgEvent::Log(
                "no frame within 40 s of the bit sync: bit sync again".into(),
            ));
        }
    }
    fn try_decode(&self, last: &[u8]) -> Option<bds::BdsSubframe> {
        let inv = if last[..11] == bds::PREAMBLE {
            0
        } else if last[..11].iter().zip(bds::PREAMBLE).all(|(a, b)| *a != b) {
            1
        } else {
            return None;
        };
        let bits: Vec<u8> = last.iter().map(|b| b ^ inv).collect();
        bds::decode_subframe(&bits, self.d2)
    }
    fn on_bit(&mut self, out: &mut Vec<MsgEvent>) {
        if self.bits.len() > 1200 {
            self.bits.drain(..600);
            self.bit_epochs.drain(..600);
        }
        let n = self.bits.len();
        if n < 300 {
            return;
        }
        let last = &self.bits[n - 300..];
        if !self.framed {
            if let Some(sf) = self.try_decode(last) {
                self.framed = true;
                self.since_frame = 0;
                self.bad_run = 0;
                out.push(MsgEvent::Log(format!("frame sync (subframe {})", sf.id)));
                let ep = self.bit_epochs[n - 300];
                self.emit(sf, ep, out);
            }
            return;
        }
        self.since_frame += 1;
        if self.since_frame < 300 {
            return;
        }
        self.since_frame = 0;
        match self.try_decode(last) {
            Some(sf) => {
                self.bad_run = 0;
                let ep = self.bit_epochs[n - 300];
                self.emit(sf, ep, out);
            }
            None => {
                out.push(MsgEvent::Bad);
                self.bad_run += 1;
                if self.bad_run >= 4 {
                    self.framed = false;
                    out.push(MsgEvent::LostFrame);
                }
            }
        }
    }
    fn emit(&self, sf: bds::BdsSubframe, ep: u64, out: &mut Vec<MsgEvent>) {
        let time = if self.d2 {
            sf.sow as f64 + 0.6 * (sf.id as f64 - 1.0)
        } else {
            sf.sow as f64
        };
        out.push(MsgEvent::Unit {
            unit: NavUnit::Bds {
                info: sf.info,
                id: sf.id,
                sow: sf.sow,
                d2: self.d2,
            },
            start_epoch: ep,
            time: Some(time),
        });
    }
    fn sync(&self) -> Sync {
        if self.framed {
            Sync::Frame
        } else if self.phase.is_some() {
            Sync::Bit
        } else {
            Sync::None
        }
    }
}

/// GLONASS strings.
#[derive(Clone, Debug)]
pub struct GloLayer {
    edges: EdgeCounter,
    phase: Option<usize>,
    acc: f32,
    acc_n: u32,
    syms: Vec<f32>,
    sym_epochs: Vec<u64>,
    since_mark: Option<usize>,
    misses: u32,
    bad_run: u32,
    sync_epoch: u64,
    pending_time: Option<(f64, u32)>,
    framed: bool,
}

impl Default for GloLayer {
    fn default() -> Self {
        GloLayer {
            edges: EdgeCounter::new(10, 40, 24, 400),
            phase: None,
            acc: 0.0,
            acc_n: 0,
            syms: Vec::with_capacity(1200),
            sym_epochs: Vec::with_capacity(1200),
            since_mark: None,
            misses: 0,
            bad_run: 0,
            sync_epoch: 0,
            pending_time: None,
            framed: false,
        }
    }
}

impl GloLayer {
    fn mark_matches(&self, end: usize) -> (usize, usize) {
        let s = &self.syms[end - 30..end];
        let m = (0..30)
            .filter(|&k| ((s[k] < 0.0) as u8) == GLO_TIME_MARK[k])
            .count();
        (m, 30 - m)
    }
    fn epoch(&mut self, e: u64, p: Complex32, out: &mut Vec<MsgEvent>) {
        let v = p.re;
        let Some(ph) = self.phase else {
            if let Some(ph) = self.edges.feed(e, v) {
                self.phase = Some(ph);
                self.sync_epoch = e;
                out.push(MsgEvent::Log("symbol sync".into()));
            }
            return;
        };
        if (e % 10) as usize == ph && self.acc_n > 0 {
            if self.acc_n == 10 {
                self.syms.push(self.acc);
                self.sym_epochs.push(e - 10);
                self.on_symbol(out);
            }
            self.acc = 0.0;
            self.acc_n = 0;
        }
        self.acc += v;
        self.acc_n += 1;
        if !self.framed && e.saturating_sub(self.sync_epoch) > 60_000 {
            *self = GloLayer::default();
            out.push(MsgEvent::Log(
                "no string within 60 s of the symbol sync: symbol sync again".into(),
            ));
        }
    }
    fn on_symbol(&mut self, out: &mut Vec<MsgEvent>) {
        if self.syms.len() > 1200 {
            self.syms.drain(..600);
            self.sym_epochs.drain(..600);
        }
        let n = self.syms.len();
        if n < 200 {
            return;
        }
        match self.since_mark {
            None => {
                let (a, b) = self.mark_matches(n);
                if a == 30 || b == 30 {
                    self.since_mark = Some(0);
                    self.misses = 0;
                    self.framed = true;
                    out.push(MsgEvent::Log("time mark found".into()));
                    self.string(n, out);
                }
            }
            Some(k) => {
                let k = k + 1;
                if k < 200 {
                    self.since_mark = Some(k);
                    return;
                }
                self.since_mark = Some(0);
                let (a, b) = self.mark_matches(n);
                let m = a.max(b);
                if m <= 25 {
                    self.misses += 1;
                    if self.misses >= 3 {
                        self.since_mark = None;
                        self.framed = false;
                        self.pending_time = None;
                        out.push(MsgEvent::LostFrame);
                        out.push(MsgEvent::Log("time mark lost".into()));
                        return;
                    }
                } else {
                    self.misses = 0;
                }
                self.string(n, out);
            }
        }
    }
    /// Decodes the string whose time mark ends at symbol `end`.
    fn string(&mut self, end: usize, out: &mut Vec<MsgEvent>) {
        let d0 = end - 200;
        // Relative-code bits from the symbol pairs (bit 85 first).
        let mut r = [0u8; 85];
        for (j, rj) in r.iter_mut().enumerate() {
            let a = self.syms[d0 + 2 * j];
            let b = self.syms[d0 + 2 * j + 1];
            *rj = (a - b < 0.0) as u8;
        }
        let mut s = [0u8; 85];
        for j in 1..85 {
            s[j] = r[j] ^ r[j - 1];
        }
        let start_epoch = self.sym_epochs[d0];
        if !matches!(
            glonass().check_bits(&mut s),
            Ok(GlonassCheck::Good | GlonassCheck::Corrected)
        ) {
            self.bad(out);
            return;
        }
        let m = crate::glo::string_number(&s);
        if !(1..=15).contains(&m) {
            self.bad(out);
            return;
        }
        self.bad_run = 0;
        // Time: string 1 carries tk; taken once the next string falls where it says.
        let mut time = None;
        if let Some((t1, _)) = self.pending_time.take() {
            let t = t1 + 2.0;
            let want = (((t % 86_400.0) % 30.0) / 2.0) as u32 % 15 + 1;
            if want == m {
                time = Some(t);
            }
        }
        if m == 1 {
            let tk = crate::glo::tk_seconds(&s);
            self.pending_time = Some((tk, m));
        }
        out.push(MsgEvent::Unit {
            unit: NavUnit::Glo {
                bits: s.to_vec(),
                number: m,
            },
            start_epoch,
            time,
        });
    }
    fn bad(&mut self, out: &mut Vec<MsgEvent>) {
        out.push(MsgEvent::Bad);
        self.bad_run += 1;
        if self.bad_run >= 6 {
            self.since_mark = None;
            self.framed = false;
            out.push(MsgEvent::LostFrame);
        }
    }
    fn sync(&self) -> Sync {
        if self.framed {
            Sync::Frame
        } else if self.phase.is_some() {
            Sync::Bit
        } else {
            Sync::None
        }
    }
}

/// The NavIC SPS layer: bit sync on 20 epochs, symbols of 20 ms, the subframe of 600 symbols found by its sync word in
/// either polarity, then de-interleaved, decoded and checked (NavIC SPS ICD section 5).
pub struct NavicLayer {
    edges: EdgeCounter,
    phase: Option<usize>,
    acc: f32,
    acc_n: u32,
    syms: std::collections::VecDeque<(u64, f32)>,
    soft: Vec<f32>,
    framed: bool,
    decoder: crate::navic::code::Decoder,
}

impl Default for NavicLayer {
    fn default() -> Self {
        NavicLayer {
            edges: EdgeCounter::new(20, 24, 12, 0),
            phase: None,
            acc: 0.0,
            acc_n: 0,
            syms: std::collections::VecDeque::with_capacity(
                crate::navic::code::SUBFRAME_SYMBOLS + 1,
            ),
            soft: Vec::with_capacity(crate::navic::code::CODED),
            framed: false,
            decoder: crate::navic::code::Decoder::default(),
        }
    }
}

impl NavicLayer {
    fn epoch(&mut self, e: u64, p: Complex32, out: &mut Vec<MsgEvent>) {
        use crate::navic::code::{SUBFRAME_SYMBOLS, SYNC};
        let v = p.re;
        let Some(ph) = self.phase else {
            self.phase = self.edges.feed(e, v);
            return;
        };
        let mut pushed = false;
        if (e % 20) as usize == ph {
            if self.acc_n == 20 {
                self.syms.push_back((e - 20, self.acc));
                if self.syms.len() > SUBFRAME_SYMBOLS {
                    self.syms.pop_front();
                }
                pushed = true;
            }
            self.acc = 0.0;
            self.acc_n = 0;
        }
        self.acc += v;
        self.acc_n += 1;
        if !pushed || self.syms.len() < SUBFRAME_SYMBOLS {
            return;
        }
        // At least 15 of the 16 sync symbols, or of their complement (the Costas loop may sit at 180 degrees).
        let agree = (0..16)
            .filter(|&k| (self.syms[k].1 < 0.0) == ((SYNC >> (15 - k)) & 1 == 1))
            .count();
        if agree > 1 && agree < 15 {
            return;
        }
        let sign = if agree >= 15 { 1.0 } else { -1.0 };
        let rms = (self.syms.iter().skip(16).map(|s| s.1 * s.1).sum::<f32>()
            / (SUBFRAME_SYMBOLS - 16) as f32)
            .sqrt()
            .max(1e-30);
        self.soft.clear();
        self.soft
            .extend(self.syms.iter().skip(16).map(|s| sign * s.1 / rms));
        let start = self.syms[0].0;
        match self.decoder.decode(&self.soft) {
            Some(sf) => {
                self.framed = true;
                out.push(MsgEvent::Unit {
                    unit: NavUnit::Navic(sf),
                    start_epoch: start,
                    time: None,
                });
                self.syms.clear();
            }
            None => {
                if self.framed {
                    out.push(MsgEvent::Bad);
                    self.syms.clear();
                }
            }
        }
    }

    fn sync(&self) -> Sync {
        if self.framed {
            Sync::Frame
        } else if self.phase.is_some() {
            Sync::Bit
        } else {
            Sync::None
        }
    }
}

/// The message layer of a channel.
pub enum Layer {
    Lnav(Box<LnavLayer>),
    Sbas(Box<SbasLayer>),
    Inav(Box<InavLayer>),
    Bds(Box<BdsLayer>),
    Glo(Box<GloLayer>),
    Navic(Box<NavicLayer>),
}

impl Layer {
    /// The layer of a satellite.
    pub fn for_sat(sat: crate::system::SatId) -> Layer {
        use crate::system::System::*;
        match sat.system {
            Gps | Qzss => Layer::Lnav(Box::default()),
            Sbas => Layer::Sbas(Box::default()),
            Galileo => Layer::Inav(Box::default()),
            Beidou => Layer::Bds(Box::new(BdsLayer::new(crate::system::bds_is_geo(sat.prn)))),
            Glonass => Layer::Glo(Box::default()),
        }
    }
    /// One epoch's prompt.
    pub fn epoch(&mut self, e: u64, p: Complex32, out: &mut Vec<MsgEvent>) {
        match self {
            Layer::Lnav(l) => l.epoch(e, p, out),
            Layer::Sbas(l) => l.epoch(e, p, out),
            Layer::Inav(l) => l.epoch(e, p, out),
            Layer::Bds(l) => l.epoch(e, p, out),
            Layer::Glo(l) => l.epoch(e, p, out),
            Layer::Navic(l) => l.epoch(e, p, out),
        }
    }
    /// The sync reached.
    pub fn sync(&self) -> Sync {
        match self {
            Layer::Lnav(l) => l.sync(),
            Layer::Sbas(l) => l.sync(),
            Layer::Inav(l) => l.sync(),
            Layer::Bds(l) => l.sync(),
            Layer::Glo(l) => l.sync(),
            Layer::Navic(l) => l.sync(),
        }
    }
    /// Starts again from nothing (after a phase-lock drop).
    pub fn reset(&mut self) {
        match self {
            Layer::Lnav(l) => **l = LnavLayer::default(),
            Layer::Sbas(l) => **l = SbasLayer::default(),
            Layer::Inav(l) => **l = InavLayer::default(),
            Layer::Bds(l) => **l = BdsLayer::new(l.d2),
            Layer::Glo(l) => **l = GloLayer::default(),
            Layer::Navic(l) => **l = NavicLayer::default(),
        }
    }
    /// Whether a data symbol lasts two or more epochs (the false-lock check applies).
    pub fn long_symbols(&self) -> bool {
        matches!(
            self,
            Layer::Lnav(_) | Layer::Sbas(_) | Layer::Glo(_) | Layer::Navic(_)
        ) || matches!(self, Layer::Bds(l) if l.d2)
    }
}
