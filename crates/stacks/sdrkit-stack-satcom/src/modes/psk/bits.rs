//! Symbols to bits: Gray demapping, differential decoding, descramblers, the sync word hunt with the phase ambiguity
//! search (rotation, spectral inversion, OQPSK rail shift). Bits are `u8` values 0 or 1.

use super::rx::PskItem;
use sdrkit_algo_modem::psk::{bits_index, index_bits, PskOrder};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Schema, Setting, SettingError, SettingId, SettingSet, Value,
    Values,
};
use sdrkit_graph::{values_of, Decoder, Format, Kind, Outbox, Port, SlicePool, Timed};
use sdrkit_proto_ccsds::randomiser::tm_bit;
use sdrkit_util_bits::lfsr::Multiplicative;
use sdrkit_util_bits::line::{NrzM, Nrzi};
use sdrkit_util_bits::seq;
use sdrkit_util_bits::sync::SyncWord;
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// The differential code of the bits.
    #[derive(Default)]
    pub enum Differential {
        /// No differential code.
        #[default]
        None = ("none", "None"),
        /// Symbol differential.
        Dpsk = ("dpsk", "Differential PSK"),
        /// NRZ-M: a 1 changes the level.
        NrzM = ("nrzm", "NRZ-M"),
        /// NRZ-S (NRZI): a 0 changes the level, as AX.25 and HDLC send.
        NrzS = ("nrzs", "NRZ-S"),
    }
}

sdrkit_core::choice_enum! {
    /// The descrambler of the bits.
    #[derive(Default)]
    pub enum Descrambler {
        /// No descrambler.
        #[default]
        None = ("none", "None"),
        /// The G3RUH scrambler `x^17 + x^12 + 1`.
        G3ruh = ("g3ruh", "G3RUH"),
        /// The CCSDS randomiser, restarted after each sync word.
        ///
        /// Standard: CCSDS 131.0-B-5 clause 10.4.
        Ccsds = ("ccsds", "CCSDS"),
    }
}

/// The differential code.
pub const DIFFERENTIAL: Setting<field::Choice<Differential>> =
    Setting::choice("differential", "Differential", Differential::None);
/// The descrambler.
pub const DESCRAMBLER: Setting<field::Choice<Descrambler>> =
    Setting::choice("descrambler", "Descrambler", Descrambler::None);
/// The sync word, as bits or hexadecimal digits.
pub const SYNC: Setting<field::Text> = Setting::new("sync", "Sync word", "", "")
    .help("Bits or hex (\"1ACFFC1D\"); every rotation and both spectral senses are tried.");
/// Invert the bits.
pub const INVERT: Setting<field::Bool> = Setting::new("invert", "Invert the bits", "", false);

sdrkit_core::settings! {
    /// The settings of [`BitStage`].
    pub enum BitStageSetting {
        /// Differential.
        Differential = DIFFERENTIAL,
        /// Descrambler.
        Descrambler = DESCRAMBLER,
        /// Sync word.
        Sync = SYNC,
        /// Invert the bits.
        Invert = INVERT,
    }
}

/// Parses a sync word given as bits ("0101...", more than 8) or hex ("1ACFFC1D", spaces and a "0x" allowed). `None` when it is
/// empty or neither.
pub fn parse_sync(s: &str) -> Option<Vec<u8>> {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if t.is_empty() {
        return None;
    }
    if t.chars().all(|c| c == '0' || c == '1') && t.len() > 8 {
        return Some(t.bytes().map(|c| c - b'0').collect());
    }
    let t = t.trim_start_matches("0x").trim_start_matches("0X");
    let mut v = Vec::new();
    sdrkit_util_bits::pack::hex_to_bits(t, &mut v).ok()?;
    (!v.is_empty()).then_some(v)
}

/// One hypothesis of the ambiguity search.
#[derive(Clone, Debug)]
struct Hyp {
    rot: usize,
    conj: bool,
    shift: bool,
    prev_k: usize,
    nrzm: NrzM,
    nrzi: Nrzi,
    g3: Multiplicative,
    sync: Option<SyncWord>,
    syncs: u32,
    prev_q: f32,
    pn_pos: Option<usize>,
}

/// Where a sync word was found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resolution {
    pub rotation_deg: f64,
    pub conjugated: bool,
    pub rail_shift: bool,
    pub resolved: bool,
}

/// The bit chain: symbols in, bits, soft bits and sync word positions out.
pub struct BitChain {
    pub order: PskOrder,
    pub diff: Differential,
    pub descr: Descrambler,
    pub invert: bool,
    hyps: Vec<Hyp>,
    with_sync: bool,
    chosen: Option<usize>,
    /// Bits produced so far (the index of the next bit).
    pub index: u64,
    /// Sync words found by the chosen hypothesis (or the best one).
    pub syncs: u64,
    scratch: Vec<u8>,
    call_base: u64,
}

/// The sync word matcher of the last 128 bits (or fewer) of `sync`, allowing one error in 12 bits.
fn matcher(sync: &[u8]) -> Option<SyncWord> {
    let l = sync.len().min(128);
    let pat = sync[sync.len() - l..]
        .iter()
        .fold(0u128, |a, &b| (a << 1) | (b & 1) as u128);
    SyncWord::new(pat, l as u32, (sync.len() / 12).min((l - 1) / 2) as u32).ok()
}

impl BitChain {
    /// A chain for `order` symbols. With a sync word, every rotation, both spectral senses and (OQPSK) both rail alignments are
    /// tried, and the first to find two sync words is kept.
    pub fn new(
        order: PskOrder,
        diff: Differential,
        descr: Descrambler,
        invert: bool,
        sync: Option<Vec<u8>>,
    ) -> Self {
        let m = order.m();
        let word = sync.as_deref().and_then(matcher);
        let multi = word.is_some();
        let mut hyps = Vec::new();
        for shift in [false, true] {
            if shift && (order != PskOrder::Oqpsk || !multi) {
                continue;
            }
            for conj in [false, true] {
                if conj && !multi {
                    continue;
                }
                for rot in 0..m {
                    if rot > 0 && !multi {
                        continue;
                    }
                    hyps.push(Hyp {
                        rot,
                        conj,
                        shift,
                        prev_k: 0,
                        nrzm: NrzM::new(0),
                        nrzi: Nrzi::new(0),
                        g3: seq::g3ruh(0),
                        sync: word,
                        syncs: 0,
                        prev_q: 0.0,
                        pn_pos: None,
                    });
                }
            }
        }
        BitChain {
            order,
            diff,
            descr,
            invert,
            hyps,
            with_sync: multi,
            chosen: None,
            index: 0,
            syncs: 0,
            scratch: Vec::with_capacity(3),
            call_base: 0,
        }
    }

    /// The hypothesis chosen by the sync words.
    pub fn resolution(&self) -> Resolution {
        match self.chosen {
            Some(i) => {
                let h = &self.hyps[i];
                Resolution {
                    rotation_deg: h.rot as f64 * 360.0 / self.order.m() as f64,
                    conjugated: h.conj,
                    rail_shift: h.shift,
                    resolved: true,
                }
            }
            None => Resolution::default(),
        }
    }

    /// Runs one hypothesis over one symbol; appends its bits (with soft values) and whether a sync word ended on each.
    fn run(
        &mut self,
        hi: usize,
        z: Complex32,
        out: Option<(&mut Vec<u8>, &mut Vec<f32>, &mut Vec<u64>)>,
    ) {
        let order = self.order;
        let m = order.m();
        let h = &mut self.hyps[hi];
        let mut y = z;
        if h.shift {
            std::mem::swap(&mut y.im, &mut h.prev_q);
        }
        if h.conj {
            y = y.conj();
        }
        if h.rot > 0 {
            y *= PskOrder::Psk8.point((8 - 8 * h.rot / m) % 8);
        }
        let k = order.decide(y);
        let idx = match self.diff {
            Differential::Dpsk => {
                let d = (k + m - h.prev_k) % m;
                h.prev_k = k;
                d
            }
            _ => k,
        };
        self.scratch.clear();
        index_bits(order, idx, &mut self.scratch);
        match self.diff {
            Differential::NrzM => h.nrzm.decode(&mut self.scratch),
            Differential::NrzS => h.nrzi.decode(&mut self.scratch),
            _ => {}
        }
        if self.descr == Descrambler::G3ruh {
            h.g3.descramble(&mut self.scratch);
        }
        let soft_mag = y.norm().min(2.0);
        let mut outp = out;
        for &b0 in &self.scratch {
            let mut b = b0 ^ self.invert as u8;
            let found = match h.sync.as_mut() {
                Some(w) => w.push(b).is_some(),
                None => false,
            };
            if found {
                h.syncs += 1;
            }
            // The CCSDS randomiser restarts after each sync word.
            if self.descr == Descrambler::Ccsds {
                if let Some(p) = h.pn_pos {
                    b ^= tm_bit(p);
                    h.pn_pos = Some(p + 1);
                }
                if found {
                    h.pn_pos = Some(0);
                }
            }
            if let Some((bits, soft, syncs)) = outp.as_mut() {
                bits.push(b);
                // Positive means 0.
                soft.push(if b == 1 { -soft_mag } else { soft_mag });
                if found {
                    syncs.push(self.call_base + bits.len() as u64);
                }
            }
        }
    }

    /// Runs symbols through the chain: the bits of the chosen hypothesis (or the plain one) are appended, with the bit index just
    /// after every sync word found.
    pub fn process(
        &mut self,
        syms: &[Complex32],
        bits: &mut Vec<u8>,
        soft: &mut Vec<f32>,
        syncs: &mut Vec<u64>,
    ) {
        self.call_base = self.index.wrapping_sub(bits.len() as u64);
        for &z in syms {
            match self.chosen {
                Some(c) => self.run(c, z, Some((bits, soft, syncs))),
                None => {
                    // Every hypothesis runs; the first to find two sync words is chosen.
                    for i in 1..self.hyps.len() {
                        self.run(i, z, None);
                    }
                    self.run(0, z, Some((bits, soft, syncs)));
                    if self.with_sync {
                        if let Some(i) = (0..self.hyps.len()).find(|&i| self.hyps[i].syncs >= 2) {
                            self.chosen = Some(i);
                        }
                    }
                }
            }
        }
        self.index = self.call_base + bits.len() as u64;
        self.syncs = match self.chosen {
            Some(c) => self.hyps[c].syncs as u64,
            None => self.hyps.iter().map(|h| h.syncs as u64).max().unwrap_or(0),
        };
    }
}

/// The transmit side of the chain, for the generator: frame bits to symbol indices.
pub struct BitEncoder {
    pub order: PskOrder,
    pub diff: Differential,
    pub descr: Descrambler,
    pub invert: bool,
    g3: Multiplicative,
    nrzm: NrzM,
    nrzi: Nrzi,
    prev_k: usize,
    pending: Vec<u8>,
}

impl BitEncoder {
    /// An encoder for `order` symbols with these codes.
    pub fn new(order: PskOrder, diff: Differential, descr: Descrambler, invert: bool) -> Self {
        BitEncoder {
            order,
            diff,
            descr,
            invert,
            g3: seq::g3ruh(0),
            nrzm: NrzM::new(0),
            nrzi: Nrzi::new(0),
            prev_k: 0,
            pending: Vec::with_capacity(3),
        }
    }
    /// Pushes frame bits (the CCSDS randomising, when used, already applied); appends whole symbol indices.
    pub fn push(&mut self, bits: &[u8], out: &mut Vec<usize>) {
        for &b0 in bits {
            let mut b = [(b0 & 1) ^ self.invert as u8];
            if self.descr == Descrambler::G3ruh {
                self.g3.scramble(&mut b);
            }
            match self.diff {
                Differential::NrzM => self.nrzm.encode(&mut b),
                Differential::NrzS => self.nrzi.encode(&mut b),
                _ => {}
            }
            self.pending.push(b[0]);
            if self.pending.len() == self.order.bits() {
                let k = bits_index(self.order, &self.pending).unwrap_or(0);
                self.pending.clear();
                let k = if self.diff == Differential::Dpsk {
                    self.prev_k = (self.prev_k + k) % self.order.m();
                    self.prev_k
                } else {
                    k
                };
                out.push(k);
            }
        }
    }
}

/// The bit stage: symbols in, the symbols and their bits out.
#[derive(Default)]
pub struct BitStage {
    diff: Differential,
    descr: Descrambler,
    invert: bool,
    sync_text: String,
    sync: Option<Vec<u8>>,
    chain: Option<BitChain>,
    bits: Vec<u8>,
    soft: Vec<f32>,
    syncs: Vec<u64>,
    bit_pool: SlicePool<u8>,
    soft_pool: SlicePool<f32>,
    sync_pool: SlicePool<u64>,
}

impl Decoder for BitStage {
    type Input = Timed<PskItem>;
    fn name(&self) -> &'static str {
        "satcom.psk_demod_bits"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<PskItem>("out").same_format_as(0)]
    }
    fn settings(&self) -> Arc<Schema> {
        BitStageSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (
                BitStageSetting::Differential,
                Value::Choice(self.diff.index()),
            ),
            (
                BitStageSetting::Descrambler,
                Value::Choice(self.descr.index()),
            ),
            (
                BitStageSetting::Sync,
                Value::Text(self.sync_text.as_str().into()),
            ),
            (BitStageSetting::Invert, Value::Bool(self.invert)),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match BitStageSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BitStageSetting::Differential => self.diff = DIFFERENTIAL.read(id, v)?,
            BitStageSetting::Descrambler => self.descr = DESCRAMBLER.read(id, v)?,
            BitStageSetting::Sync => {
                let t = SYNC.read(id, v)?;
                self.sync = if t.trim().is_empty() {
                    None
                } else {
                    Some(parse_sync(t).ok_or(SettingError::Malformed { id, at: Some(0) })?)
                };
                self.sync_text = t.to_string();
            }
            BitStageSetting::Invert => self.invert = INVERT.read(id, v)?,
        }
        self.chain = None;
        Ok(())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.chain = None;
        Ok(vec![f.with_kind(Kind::typed::<PskItem>())])
    }
    fn process(&mut self, x: &[Timed<PskItem>], _first: u64, out: &mut Outbox) {
        let out = out.items::<PskItem>(0);
        for it in x {
            out.push(it.clone());
            match &it.value {
                PskItem::Symbols { order, syms } => {
                    if self.chain.as_ref().is_none_or(|c| c.order != *order) {
                        self.chain = Some(BitChain::new(
                            *order,
                            self.diff,
                            self.descr,
                            self.invert,
                            self.sync.clone(),
                        ));
                    }
                    let Some(c) = self.chain.as_mut() else {
                        continue;
                    };
                    let first = c.index;
                    self.bits.clear();
                    self.soft.clear();
                    self.syncs.clear();
                    c.process(syms, &mut self.bits, &mut self.soft, &mut self.syncs);
                    let (b, s, y) = (&self.bits, &self.soft, &self.syncs);
                    out.push(Timed {
                        time: it.time,
                        value: PskItem::Bits {
                            bits: self.bit_pool.exact(b.len(), |o| o.copy_from_slice(b)),
                            soft: self.soft_pool.exact(s.len(), |o| o.copy_from_slice(s)),
                            first,
                            syncs: self.sync_pool.exact(y.len(), |o| o.copy_from_slice(y)),
                            resolution: c.resolution(),
                            total_syncs: c.syncs,
                        },
                    });
                }
                // A new lock: a new chain.
                PskItem::Analysis(_) => self.chain = None,
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_words_parse() {
        let s = parse_sync("1A CF FC 1D").unwrap();
        assert_eq!(s.len(), 32);
        assert_eq!(s[..8], [0, 0, 0, 1, 1, 0, 1, 0]);
        assert_eq!(parse_sync("1ACFFC1D"), Some(s));
        assert_eq!(parse_sync("0101010101").unwrap().len(), 10);
        assert!(parse_sync("xyz").is_none());
        // The CCSDS sequence starts FF 48 (CCSDS 131.0-B-5 clause 10.4).
        let pn: Vec<u8> = (0..16).map(tm_bit).collect();
        assert_eq!(pn, [1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 0, 1, 0, 0, 0]);
    }

    fn chain_round_trip(
        order: PskOrder,
        diff: Differential,
        descr: Descrambler,
        rot: usize,
        conj: bool,
    ) {
        let sync = parse_sync("1ACFFC1D").unwrap();
        let mut enc = BitEncoder::new(order, diff, descr, false);
        let mut frame = sync.clone();
        let payload: Vec<u8> = (0..240).map(|i| ((i * 7 + i / 3) % 5 < 2) as u8).collect();
        let mut p2 = payload.clone();
        if descr == Descrambler::Ccsds {
            for (i, b) in p2.iter_mut().enumerate() {
                *b ^= tm_bit(i);
            }
        }
        frame.extend_from_slice(&p2);
        let mut ks = Vec::new();
        // 72 bits of filler first: the sync is found at bit 72 + 32.
        let filler: Vec<u8> = (0..72).map(|i| (i % 3 == 0) as u8).collect();
        enc.push(&filler, &mut ks);
        for _ in 0..4 {
            enc.push(&frame, &mut ks);
        }
        let a = rot as f64 * std::f64::consts::TAU / order.m() as f64;
        let syms: Vec<Complex32> = ks
            .iter()
            .map(|&k| {
                let p = order.point(k) * Complex32::new(a.cos() as f32, a.sin() as f32);
                if conj {
                    p.conj()
                } else {
                    p
                }
            })
            .collect();
        let mut ch = BitChain::new(order, diff, descr, false, Some(sync.clone()));
        let (mut bits, mut soft, mut syncs) = (Vec::new(), Vec::new(), Vec::new());
        ch.process(&syms, &mut bits, &mut soft, &mut syncs);
        assert!(
            ch.resolution().resolved,
            "{order:?} {diff:?} {descr:?} rot {rot} conj {conj}"
        );
        // After resolution, every payload after a sync comes back.
        let good = syncs
            .iter()
            .filter(|&&s| (s as usize) + 240 <= bits.len())
            .filter(|&&s| bits[s as usize..s as usize + 240] == payload[..])
            .count();
        assert!(
            good >= 1,
            "{order:?} {diff:?} {descr:?} rot {rot} conj {conj}: {syncs:?}"
        );
    }

    #[test]
    fn coding_round_trips_with_ambiguity() {
        for order in [PskOrder::Bpsk, PskOrder::Qpsk, PskOrder::Psk8] {
            for diff in [
                Differential::None,
                Differential::Dpsk,
                Differential::NrzM,
                Differential::NrzS,
            ] {
                for descr in [Descrambler::None, Descrambler::G3ruh, Descrambler::Ccsds] {
                    for (rot, conj) in [(0, false), (1, true), (order.m() - 1, false)] {
                        chain_round_trip(order, diff, descr, rot, conj);
                    }
                }
            }
        }
    }

    #[test]
    fn sync_found_at_bit_72() {
        let sync = parse_sync("1ACFFC1D").unwrap();
        let mut bits: Vec<u8> = (0..72).map(|i| (i % 2 == 0) as u8).collect();
        bits.extend_from_slice(&sync);
        bits.extend((0..64).map(|i| (i % 3 == 0) as u8));
        let syms: Vec<Complex32> = bits
            .iter()
            .map(|&b| Complex32::new(if b == 1 { -1.0 } else { 1.0 }, 0.0))
            .collect();
        let mut ch = BitChain::new(
            PskOrder::Bpsk,
            Differential::None,
            Descrambler::None,
            false,
            Some(sync),
        );
        let (mut b, mut s, mut y) = (Vec::new(), Vec::new(), Vec::new());
        ch.process(&syms, &mut b, &mut s, &mut y);
        assert_eq!(y, vec![72 + 32]);
    }
}
