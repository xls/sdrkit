//! The RDS block layer: block synchronisation, error correction at three levels with voting, and group assembly.
//!
//! The code itself (the (26,16) shortened cyclic code and its syndromes) is `sdrkit_algo_fec::small::SmallCode`.
//! Standard: IEC 62106 (EN 50067 clause 2.3 and Annex B).

use sdrkit_algo_fec::small::SmallCode;
use std::collections::VecDeque;
use std::sync::OnceLock;

/// Offset word of block A (IEC 62106 Annex A).
pub const OFFSET_A: u16 = 0x0FC;
/// Offset word of block B.
pub const OFFSET_B: u16 = 0x198;
/// Offset word of block C (version A groups).
pub const OFFSET_C: u16 = 0x168;
/// Offset word of block C′ (version B groups, where block 3 repeats the PI).
pub const OFFSET_CP: u16 = 0x350;
/// Offset word of block D.
pub const OFFSET_D: u16 = 0x1B4;

/// The generator polynomial `x^10 + x^8 + x^7 + x^5 + x^4 + x^3 + 1` (IEC 62106 Annex B).
const GENERATOR: u64 = 0x5B9;

/// The 26 bits of a block.
const MASK26: u32 = (1 << 26) - 1;

/// The RDS block code with a table of the bursts of 1 or 2 bits (the longest bursts the decoder repairs).
fn code() -> &'static SmallCode {
    static CODE: OnceLock<SmallCode> = OnceLock::new();
    CODE.get_or_init(|| {
        SmallCode::cyclic(26, GENERATOR)
            .and_then(|c| c.with_burst_table(2))
            .unwrap_or_else(|_| unreachable!("the RDS generator is a valid degree-10 polynomial"))
    })
}

/// The syndrome of a 26-bit word (the first bit sent is bit 25). A block received without error has its offset word as its
/// syndrome.
pub fn syndrome(word: u32) -> u16 {
    code().syndrome(word as u128) as u16
}

/// A whole 26-bit block: 16 information bits, then the checkword with `offset` added.
pub fn encode_block(info: u16, offset: u16) -> u32 {
    (code().encode(info as u128) as u32) ^ offset as u32
}

/// The burst of 1 or 2 bits that turns `word` into a block with `offset`, or `None`.
fn burst(word: u32, offset: u16) -> Option<u32> {
    let (fixed, _) = code().decode((word ^ offset as u32) as u128)?;
    let e = (fixed as u32 ^ offset as u32) ^ word;
    (e != 0).then_some(e)
}

/// One block as the group decoder gets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    /// The information bits, after any correction.
    pub data: u16,
    /// The information bits as received, before correction.
    pub raw: u16,
    /// The block needed a correction (bursts, weak symbols or voting).
    pub corrected: bool,
    /// The block was recovered by voting over repeated copies (a subset of `corrected`).
    pub voted: bool,
    /// At position C, the block carried offset C′ (a version B group).
    pub c_prime: bool,
}

/// A group of four blocks (A, B, C or C′, D); `None` where a block was not usable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Group {
    /// The blocks in group order.
    pub blocks: [Option<Block>; 4],
}

impl Group {
    /// A group of four clean blocks (for tests and the generator's own checks).
    pub fn clean(a: u16, b: u16, c: u16, d: u16) -> Self {
        let c_prime = b & 0x0800 != 0;
        let mk = |v: u16, cp: bool| {
            Some(Block {
                data: v,
                raw: v,
                corrected: false,
                voted: false,
                c_prime: cp,
            })
        };
        Group {
            blocks: [mk(a, false), mk(b, false), mk(c, c_prime), mk(d, false)],
        }
    }

    /// The group monitor line, `"PPPP BBBB CCCC DDDD"`, with `----` for a block that was not usable.
    pub fn hex(&self) -> String {
        let parts: Vec<String> = self
            .blocks
            .iter()
            .map(|b| b.map_or_else(|| "----".to_string(), |b| format!("{:04X}", b.data)))
            .collect();
        parts.join(" ")
    }
}

/// The block-quality and counter values of a [`BlockLayer`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlockStats {
    /// Blocks whose checkword was right.
    pub good: u64,
    /// Blocks repaired (including voted ones).
    pub corrected: u64,
    /// Blocks that were not usable.
    pub bad: u64,
    /// Blocks recovered by voting.
    pub voted: u64,
}

#[derive(Clone, Debug)]
struct Stored {
    word: u32,
    rel: [f32; 26],
    block_no: u64,
}

/// The error pattern and syndrome of each of the 27 symbol positions of a block (an edge symbol flips one bit, the others two).
struct Symbols {
    pat: [u32; 27],
    syn: [u16; 27],
}

fn symbols() -> &'static Symbols {
    static T: OnceLock<Symbols> = OnceLock::new();
    T.get_or_init(|| {
        let mut pat = [0u32; 27];
        let mut syn = [0u16; 27];
        for (j, (p, s)) in pat.iter_mut().zip(syn.iter_mut()).enumerate() {
            // Symbol j is the one before arrival bit j; it flips arrival bits j - 1 and j. Arrival bit k is word bit 25 - k.
            let mut e = 0;
            if j >= 1 {
                e |= 1 << (25 - (j - 1));
            }
            if j <= 25 {
                e |= 1 << (25 - j);
            }
            *p = e;
            *s = syndrome(e);
        }
        Symbols { pat, syn }
    })
}

/// The offsets accepted at a position (C and C′ at position C).
fn offsets_at(pos: usize) -> &'static [u16] {
    match pos {
        0 => &[OFFSET_A],
        1 => &[OFFSET_B],
        2 => &[OFFSET_C, OFFSET_CP],
        _ => &[OFFSET_D],
    }
}

/// Block synchronisation, error correction (levels 0, 1, 2, and voting) and group assembly. Feed it data bits (already differentially
/// decoded) with the reliability of the symbol each bit was decided from.
#[derive(Clone, Debug)]
pub struct BlockLayer {
    level: u8,
    voting: bool,
    reg: u32,
    rel: [f32; 32],
    bits: u64,
    synced: bool,
    pos: usize,
    in_block: u32,
    candidates: VecDeque<(u64, usize)>,
    quality: f32,
    bad_run: u32,
    stats: BlockStats,
    group: Group,
    has_group: bool,
    block_no: u64,
    store: [VecDeque<Stored>; 4],
}

impl Default for BlockLayer {
    fn default() -> Self {
        Self::new()
    }
}

/// Bad blocks in a row after which sync is lost (about 0.5 s).
pub const SYNC_LOSS_BAD_BLOCKS: u32 = 24;

impl BlockLayer {
    /// A block layer at the default correction: level 2 with voting.
    pub fn new() -> Self {
        BlockLayer {
            level: 2,
            voting: true,
            reg: 0,
            rel: [1.0; 32],
            bits: 0,
            synced: false,
            pos: 0,
            in_block: 0,
            candidates: VecDeque::new(),
            quality: 0.0,
            bad_run: 0,
            stats: BlockStats::default(),
            group: Group::default(),
            has_group: false,
            block_no: 0,
            store: Default::default(),
        }
    }

    /// The correction level: 0 (none), 1 (bursts of 1 or 2 bits), 2 (also two weak symbols).
    pub fn set_level(&mut self, level: u8) {
        self.level = level.min(2);
    }

    /// Voting over repeated copies (used at level 2 only).
    pub fn set_voting(&mut self, on: bool) {
        self.voting = on;
        if !on {
            self.store.iter_mut().for_each(VecDeque::clear);
        }
    }

    /// Back to the state after construction, keeping the correction settings.
    pub fn reset(&mut self) {
        let (level, voting) = (self.level, self.voting);
        *self = Self::new();
        self.level = level;
        self.voting = voting;
    }

    /// Whether the layer is synchronised.
    pub fn synced(&self) -> bool {
        self.synced
    }

    /// The usable-block percentage (0 when not synchronised).
    pub fn quality(&self) -> f32 {
        if self.synced {
            self.quality
        } else {
            0.0
        }
    }

    /// The block counters.
    pub fn stats(&self) -> BlockStats {
        self.stats
    }

    /// One data bit with the reliability of its symbol. Returns a group when its block D has been judged.
    pub fn push(&mut self, bit: bool, rel: f32) -> Option<Group> {
        self.reg = ((self.reg << 1) | bit as u32) & MASK26;
        self.rel[(self.bits % 32) as usize] = rel;
        self.bits += 1;
        if self.synced {
            self.in_block += 1;
            if self.in_block < 26 {
                return None;
            }
            self.in_block = 0;
            return self.judge_block();
        }
        if self.bits < 26 {
            return None;
        }
        self.acquire()
    }

    fn acquire(&mut self) -> Option<Group> {
        let s = syndrome(self.reg);
        let pos = match s {
            OFFSET_A => 0,
            OFFSET_B => 1,
            OFFSET_C | OFFSET_CP => 2,
            OFFSET_D => 3,
            _ => return None,
        };
        let t = self.bits;
        while self
            .candidates
            .front()
            .is_some_and(|&(t0, _)| t - t0 > 6 * 26)
        {
            self.candidates.pop_front();
        }
        let hit = self.candidates.iter().any(|&(t0, p0)| {
            let d = t - t0;
            d.is_multiple_of(26)
                && (1..=6).contains(&(d / 26))
                && (p0 + (d / 26) as usize) % 4 == pos
        });
        if !hit {
            self.candidates.push_back((t, pos));
            while self.candidates.len() > 8 {
                self.candidates.pop_front();
            }
            return None;
        }
        self.candidates.clear();
        self.synced = true;
        self.quality = 50.0;
        self.bad_run = 0;
        self.in_block = 0;
        self.group = Group::default();
        self.has_group = false;
        let data = (self.reg >> 10) as u16;
        self.stats.good += 1;
        self.place(
            pos,
            Block {
                data,
                raw: data,
                corrected: false,
                voted: false,
                c_prime: s == OFFSET_CP,
            },
        )
    }

    fn place(&mut self, pos: usize, block: Block) -> Option<Group> {
        self.group.blocks[pos] = Some(block);
        self.has_group = true;
        self.after(pos)
    }

    fn after(&mut self, pos: usize) -> Option<Group> {
        self.pos = (pos + 1) % 4;
        self.block_no += 1;
        if pos == 3 {
            let g = std::mem::take(&mut self.group);
            let had = std::mem::replace(&mut self.has_group, false);
            return had.then_some(g);
        }
        None
    }

    /// The reliabilities of the 27 symbols of the block that just ended.
    fn symbol_rels(&self) -> [f32; 27] {
        let mut r = [1.0f32; 27];
        let end = self.bits - 1; // arrival index of the last bit
        for (j, v) in r.iter_mut().enumerate() {
            // Symbol j is the symbol of arrival bit end - 26 + j.
            let idx = end as i64 - 26 + j as i64;
            if idx >= 0 {
                *v = self.rel[(idx as u64 % 32) as usize];
            }
        }
        r
    }

    fn judge_block(&mut self) -> Option<Group> {
        let pos = self.pos;
        let word = self.reg;
        let s = syndrome(word);
        let offs = offsets_at(pos);
        let raw = (word >> 10) as u16;
        // Good.
        if let Some(&o) = offs.iter().find(|&&o| o == s) {
            self.stats.good += 1;
            self.bad_run = 0;
            self.quality += 0.05 * (100.0 - self.quality);
            return self.place(
                pos,
                Block {
                    data: raw,
                    raw,
                    corrected: false,
                    voted: false,
                    c_prime: o == OFFSET_CP,
                },
            );
        }
        let syms = self.symbol_rels();
        if let Some((fixed, o, voted)) = self.correct(pos, word, s, offs, &syms) {
            self.stats.corrected += 1;
            if voted {
                self.stats.voted += 1;
            }
            self.quality += 0.05 * (100.0 - self.quality);
            let data = (fixed >> 10) as u16;
            return self.place(
                pos,
                Block {
                    data,
                    raw,
                    corrected: true,
                    voted,
                    c_prime: o == OFFSET_CP,
                },
            );
        }
        self.stats.bad += 1;
        self.quality -= 0.05 * self.quality;
        self.bad_run += 1;
        if self.bad_run >= SYNC_LOSS_BAD_BLOCKS {
            self.synced = false;
            self.quality = 0.0;
            self.candidates.clear();
            self.group = Group::default();
            self.has_group = false;
            return None;
        }
        self.after(pos)
    }

    fn correct(
        &mut self,
        pos: usize,
        word: u32,
        s: u16,
        offs: &[u16],
        syms: &[f32; 27],
    ) -> Option<(u32, u16, bool)> {
        if self.level == 0 {
            return None;
        }
        let t = symbols();
        // A burst of 1 or 2 bits, with the reliability of the symbols that explain it.
        let mut found: Option<(f32, u32, u16)> = None;
        for &o in offs {
            if let Some(e) = burst(word, o) {
                let low = e.trailing_zeros() as usize; // word bit
                let cost = if e.count_ones() == 2 {
                    // Arrival bits k, k + 1 (k = 24 - low): symbol k + 1.
                    syms[25 - low]
                } else {
                    let k = 25 - low;
                    syms[k] + syms[k + 1]
                };
                found = Some((cost, e, o));
                break;
            }
        }
        if self.level < 2 {
            return found.map(|(_, e, o)| (word ^ e, o, false));
        }
        // Two weak symbols: every pair of the six weakest; the pair with the lowest total reliability wins, against the burst too.
        let mut idx: [usize; 27] = std::array::from_fn(|i| i);
        idx.sort_by(|&a, &b| syms[a].total_cmp(&syms[b]));
        let weak = &idx[..6];
        let mut best = found;
        for (n, &i) in weak.iter().enumerate() {
            for &j in &weak[n + 1..] {
                let syn = t.syn[i] ^ t.syn[j];
                for &o in offs {
                    if syn == s ^ o {
                        let cost = syms[i] + syms[j];
                        if best.is_none_or(|(c, _, _)| cost < c) {
                            best = Some((cost, t.pat[i] ^ t.pat[j], o));
                        }
                    }
                }
            }
        }
        if let Some((_, e, o)) = best {
            return Some((word ^ e, o, false));
        }
        if self.voting {
            return self.vote(pos, word, offs, syms);
        }
        None
    }

    fn vote(
        &mut self,
        pos: usize,
        word: u32,
        offs: &[u16],
        syms: &[f32; 27],
    ) -> Option<(u32, u16, bool)> {
        // Bit reliabilities: a data bit is as reliable as the weaker of its two symbols. Indexed by word bit (25 = first arrived).
        let mut rel = [0f32; 26];
        for (k, r) in (0..26).map(|k| (k, syms[k].min(syms[k + 1]))) {
            rel[25 - k] = r;
        }
        let now = self.block_no;
        let store = &mut self.store[pos];
        let near: Vec<usize> = store
            .iter()
            .enumerate()
            .filter(|(_, c)| now - c.block_no < 160 && (c.word ^ word).count_ones() <= 8)
            .map(|(i, _)| i)
            .collect();
        if near.len() >= 2 {
            let mut result = 0u32;
            for (b, &r) in rel.iter().enumerate() {
                let mut sum = if word >> b & 1 == 1 { r } else { -r };
                for &i in &near {
                    let c = &store[i];
                    sum += if c.word >> b & 1 == 1 {
                        c.rel[b]
                    } else {
                        -c.rel[b]
                    };
                }
                if sum > 0.0 {
                    result |= 1 << b;
                }
            }
            let s = syndrome(result);
            let close = (result ^ word).count_ones() <= 5
                && near
                    .iter()
                    .all(|&i| (store[i].word ^ result).count_ones() <= 5);
            if let Some(&o) = offs.iter().find(|&&o| o == s) {
                if close {
                    for &i in near.iter().rev() {
                        store.remove(i);
                    }
                    return Some((result, o, true));
                }
            }
        }
        store.push_back(Stored {
            word,
            rel,
            block_no: now,
        });
        while store.len() > 8 {
            store.pop_front();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits_of(word: u32) -> impl Iterator<Item = bool> {
        (0..26).rev().map(move |b| word >> b & 1 == 1)
    }

    #[test]
    fn a_clean_block_has_its_offset_as_syndrome() {
        for (info, off) in [
            (0x4A21u16, OFFSET_A),
            (0x0548, OFFSET_B),
            (0xCDCD, OFFSET_C),
            (0x4A21, OFFSET_CP),
            (0x5445, OFFSET_D),
        ] {
            assert_eq!(syndrome(encode_block(info, off)), off);
        }
    }

    #[test]
    fn every_burst_of_one_or_two_bits_is_found_and_no_longer_one() {
        let w = encode_block(0x0548, OFFSET_B);
        let mut n = 0;
        for i in 0..26 {
            assert_eq!(burst(w ^ (1 << i), OFFSET_B), Some(1 << i));
            n += 1;
            if i < 25 {
                assert_eq!(burst(w ^ (3 << i), OFFSET_B), Some(3 << i));
                n += 1;
            }
        }
        assert_eq!(n, 51);
        assert_eq!(burst(w, OFFSET_B), None);
        assert_eq!(burst(w ^ 0b101, OFFSET_B), None);
        let t = symbols();
        assert_eq!(t.pat[0].count_ones(), 1);
        assert_eq!(t.pat[26].count_ones(), 1);
        assert_eq!(t.pat[13].count_ones(), 2);
    }

    #[test]
    fn syncs_and_assembles_groups() {
        let mut l = BlockLayer::new();
        let g = [
            encode_block(0x4A21, OFFSET_A),
            encode_block(0x0548, OFFSET_B),
            encode_block(0xCDCD, OFFSET_C),
            encode_block(0x5445, OFFSET_D),
        ];
        let mut out = Vec::new();
        for _ in 0..3 {
            for w in g {
                for b in bits_of(w) {
                    if let Some(gr) = l.push(b, 1.0) {
                        out.push(gr);
                    }
                }
            }
        }
        assert!(l.synced());
        assert!(out.len() >= 2);
        assert_eq!(out.last().unwrap().hex(), "4A21 0548 CDCD 5445");
    }

    #[test]
    fn one_clean_block_in_three_is_enough_to_sync() {
        let mut l = BlockLayer::new();
        l.set_level(0);
        let infos = [0x4A21u16, 0x0548, 0xCDCD, 0x5445];
        let offs = [OFFSET_A, OFFSET_B, OFFSET_C, OFFSET_D];
        let mut synced_at = None;
        for n in 0..12 {
            let mut w = encode_block(infos[n % 4], offs[n % 4]);
            if n % 3 != 0 {
                w ^= 0b1010_0000_0100_0000_0001; // damaged
            }
            for b in bits_of(w) {
                l.push(b, 1.0);
            }
            if l.synced() && synced_at.is_none() {
                synced_at = Some(n + 1);
            }
        }
        assert!(
            synced_at.is_some_and(|n| n <= 5),
            "synced after {synced_at:?} blocks"
        );
    }

    #[test]
    fn bursts_and_weak_pairs_are_corrected() {
        let mut l = BlockLayer::new();
        // Sync on clean blocks first.
        let infos = [0x4A21u16, 0x0548, 0xCDCD, 0x5445];
        let offs = [OFFSET_A, OFFSET_B, OFFSET_C, OFFSET_D];
        let feed = |l: &mut BlockLayer, w: u32, rel: &dyn Fn(usize) -> f32| -> Option<Group> {
            let mut g = None;
            for (k, b) in bits_of(w).enumerate() {
                if let Some(x) = l.push(b, rel(k)) {
                    g = Some(x);
                }
            }
            g
        };
        for n in 0..8 {
            feed(&mut l, encode_block(infos[n % 4], offs[n % 4]), &|_| 1.0);
        }
        assert!(l.synced());
        // Block A with a 2-bit burst, block B with two weak 2-bit bursts.
        feed(&mut l, encode_block(infos[0], offs[0]) ^ (3 << 7), &|_| 1.0);
        let b = encode_block(infos[1], offs[1]) ^ (3 << 20) ^ (3 << 13);
        // Arrival bits 4, 5 and 11, 12 are weak.
        feed(&mut l, b, &|k| {
            if (4..=5).contains(&k) {
                0.1
            } else if (11..=12).contains(&k) {
                0.15
            } else {
                1.0
            }
        });
        feed(&mut l, encode_block(infos[2], offs[2]), &|_| 1.0);
        let g = feed(&mut l, encode_block(infos[3], offs[3]), &|_| 1.0).unwrap();
        assert_eq!(g.hex(), "4A21 0548 CDCD 5445");
        assert!(g.blocks[0].unwrap().corrected && g.blocks[1].unwrap().corrected);
        assert_eq!(l.stats().corrected, 2);
    }
}
