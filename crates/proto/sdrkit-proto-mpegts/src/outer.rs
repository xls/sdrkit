//! The outer code that carries a transport stream over DVB-C, DVB-S, DVB-T, ISDB-S, ISDB-T and DMB: the sync byte search,
//! energy dispersal, RS(204,188) and the Forney convolutional byte interleaver of 12 branches and depth unit 17 bytes.
//!
//! Standard: ETSI EN 300 421 clauses 4.4.1 to 4.4.3, EN 300 429 clauses 5 to 7, EN 300 744 clauses 4.3.1 and 4.3.2, ARIB STD-B31
//! clauses 3.3 to 3.5, ETSI TS 102 427 clause 5.

use sdrkit_algo_fec::rs::{codes::DVB_204_188, Rs};
use sdrkit_util_bits::interleave::DelayLanes;
use sdrkit_util_bits::{seq, BitOrder};
use std::sync::OnceLock;

/// Transport packet length in bytes.
pub const TS: usize = 188;
/// Reed-Solomon word length in bytes.
pub const RS_N: usize = 204;
/// Transport packets in one DVB energy dispersal group.
pub const GROUP: usize = 8;
/// Branches of the byte interleaver (EN 300 421 clause 4.4.3: I = 12).
pub const BRANCHES: u32 = 12;
/// Depth unit of the byte interleaver in bytes (EN 300 421 clause 4.4.3: M = 17).
pub const DEPTH: u32 = 17;
/// Words that the two interleavers together delay: (I - 1) M I bytes, 11 words.
pub const DELAY_WORDS: usize = ((BRANCHES - 1) * DEPTH * BRANCHES) as usize / RS_N;
/// The transport stream sync byte.
pub const SYNC: u8 = 0x47;
/// The inverted sync byte that starts each DVB dispersal group.
pub const SYNC_INVERTED: u8 = 0xB8;

/// The period of the dispersal sequence in bytes. The register has a period of 32,767 bits, and 8 is prime to it.
const SEQ_PERIOD: usize = 32_767;

/// The bytes of the energy dispersal sequence 1 + x^14 + x^15 from 100101010000000, most significant bit first, over one period
/// (EN 300 421 clause 4.4.1, figure 2). Byte 0 is the first byte the register outputs after a reload.
pub fn dispersal_bytes() -> &'static [u8] {
    static BYTES: OnceLock<Box<[u8]>> = OnceLock::new();
    BYTES.get_or_init(|| {
        let mut lfsr = seq::DVB_DISPERSAL;
        let mut out = vec![0u8; SEQ_PERIOD].into_boxed_slice();
        lfsr.fill_bytes(&mut out, BitOrder::Msb);
        out
    })
}

/// The RS(204,188) code of EN 300 421 clause 4.4.2, a shortened RS(255,239) over GF(256).
pub fn rs() -> Rs {
    // The code is a table constant of the shared crate, so it is always valid.
    Rs::new(DVB_204_188).unwrap_or_else(|_| unreachable!("RS(204,188) is a valid code"))
}

/// The byte interleaver of the transmitter (`receive` false) or the de-interleaver of the receiver, filled with zeros. Branch
/// `j` of the transmitter delays by `17 j` bytes. Sync bytes always pass branch 0 (EN 300 421 clause 4.4.3).
pub fn interleaver(receive: bool) -> DelayLanes<u8> {
    DelayLanes::forney(BRANCHES, DEPTH, receive, 0)
}

/// How the energy dispersal of a stream works.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dispersal {
    /// DVB-C, DVB-S, DVB-T, ISDB-S: the transmitter scrambles the 187 bytes after each sync byte before RS, the register
    /// reloads every 8 packets, and the first sync byte of each group is inverted (EN 300 421 clause 4.4.1).
    DvbGroups,
    /// ISDB-T: the transmitter scrambles bytes 1 to 203 of each RS word after RS, and the register reloads at the start of each
    /// OFDM frame of `words` words (ARIB STD-B31 clause 3.5). The register also runs through every sync byte, the first of
    /// the frame too, and does not change it: byte 1 of word 0 gets sequence byte 1. The sync byte is not inverted.
    IsdbFrame {
        /// RS words in one OFDM frame of the layer.
        words: usize,
    },
    /// DMB: no energy dispersal (ETSI TS 102 427 clause 5).
    None,
}

impl Dispersal {
    /// The sequence offset of byte 1 of word `n` (counted from a group or frame start).
    fn offset(self, n: usize) -> usize {
        match self {
            // Byte k of packet g has index 188 g + k - 1 in the group. The register runs through the 7 later sync bytes too.
            Dispersal::DvbGroups => (n % GROUP) * TS,
            Dispersal::IsdbFrame { words } => ((n % words.max(1)) * RS_N + 1) % SEQ_PERIOD,
            Dispersal::None => 0,
        }
    }
}

/// XORs `bytes` with the dispersal sequence from `offset` on.
fn scramble(bytes: &mut [u8], offset: usize) {
    let seq = dispersal_bytes();
    for (i, b) in bytes.iter_mut().enumerate() {
        *b ^= seq[(offset + i) % SEQ_PERIOD];
    }
}

/// What Reed-Solomon did with one word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordStatus {
    /// The word had no error.
    Clean,
    /// Reed-Solomon corrected the word.
    Corrected {
        /// Bytes changed.
        bytes: u16,
        /// Bits changed.
        bits: u16,
    },
    /// The word had more than 8 bytes in error. The packet carries the transport_error_indicator.
    Failed,
}

/// One transport packet out of the outer decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OuterWord {
    /// The packet. Its sync byte is 0x47. A failed word sets the transport_error_indicator.
    pub packet: [u8; TS],
    /// What Reed-Solomon did.
    pub status: WordStatus,
    /// The first byte of the word was a sync byte (0x47 or 0xB8) before Reed-Solomon.
    pub sync_seen: bool,
}

impl OuterWord {
    /// True when Reed-Solomon could not correct the word.
    pub fn failed(&self) -> bool {
        self.status == WordStatus::Failed
    }
}

/// Counters of an outer decoder.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OuterStats {
    /// RS words decoded (after the interleaver fill).
    pub words: u64,
    /// Words with no error.
    pub clean: u64,
    /// Words corrected.
    pub corrected: u64,
    /// Words that failed.
    pub failed: u64,
    /// Bytes corrected.
    pub bytes_corrected: u64,
    /// Bits corrected.
    pub bits_corrected: u64,
    /// Packets delivered.
    pub delivered: u64,
    /// Delivered packets with the transport_error_indicator set.
    pub flagged: u64,
    /// Consecutive failed words now.
    pub run_failed: u32,
    /// The fraction of bytes in error before Reed-Solomon, smoothed over groups of 64 words. A failed word counts as 8 bytes.
    pub pre_rs: f64,
    /// Byte alignments found by the owner since the start.
    pub alignments: u64,
}

impl sdrkit_graph::Item for OuterStats {
    const KIND: &'static str = "dvb.outer_stats";
}

/// The receive side: bytes aligned so that byte 0 is a sync byte position in, transport packets out. It allocates nothing after
/// [`OuterDecoder::new`].
///
/// ```
/// use sdrkit_proto_mpegts::outer::{Dispersal, OuterDecoder, OuterEncoder, RS_N, TS};
///
/// let mut enc = OuterEncoder::new(Dispersal::DvbGroups);
/// let mut dec = OuterDecoder::new(Dispersal::DvbGroups);
/// let mut got = Vec::new();
/// for n in 0..40u8 {
///     let mut p = [n; TS];
///     p[0] = 0x47;
///     let mut w = [0u8; RS_N];
///     enc.encode(&p, &mut w);
///     w[100] ^= 0x55; // one byte error in every word
///     dec.push(&w, &mut |word| got.push(word.packet[5]));
/// }
/// // The interleavers delay by 11 words. Every later word is corrected.
/// assert_eq!(got, (0..29u8).collect::<Vec<_>>());
/// assert_eq!(dec.stats.corrected, 29);
/// ```
#[derive(Clone, Debug)]
pub struct OuterDecoder {
    rs: Rs,
    deint: DelayLanes<u8>,
    dispersal: Dispersal,
    word: [u8; RS_N],
    fill: usize,
    skip: usize,
    /// Words decoded since the last restart, after the fill.
    n: usize,
    /// Packet index in the DVB group. `None` until a 0xB8 is seen.
    group: Option<usize>,
    batch_changed: usize,
    batch_bytes: usize,
    /// Counters.
    pub stats: OuterStats,
}

impl OuterDecoder {
    /// A decoder for a stream with this dispersal.
    pub fn new(dispersal: Dispersal) -> OuterDecoder {
        OuterDecoder {
            rs: rs(),
            deint: interleaver(true),
            dispersal,
            word: [0; RS_N],
            fill: 0,
            skip: DELAY_WORDS,
            n: 0,
            group: None,
            batch_changed: 0,
            batch_bytes: 0,
            stats: OuterStats::default(),
        }
    }

    /// Starts again at a new alignment: the de-interleaver is emptied and the next [`DELAY_WORDS`] words are its fill. For
    /// ISDB-T, the first word after the fill is word 0 of an OFDM frame. The counters stay.
    pub fn restart(&mut self) {
        self.deint.reset(0);
        self.fill = 0;
        self.skip = DELAY_WORDS;
        self.n = 0;
        self.group = None;
        self.stats.run_failed = 0;
    }

    /// Clears the counters.
    pub fn reset_stats(&mut self) {
        self.stats = OuterStats::default();
        self.batch_changed = 0;
        self.batch_bytes = 0;
    }

    /// Consecutive failed words now.
    pub fn run_failed(&self) -> u32 {
        self.stats.run_failed
    }

    /// Feeds aligned bytes. Each decoded packet goes to `out`. With [`Dispersal::DvbGroups`], packets before the first group
    /// start (0xB8) are counted but not given.
    pub fn push(&mut self, bytes: &[u8], out: &mut impl FnMut(&OuterWord)) {
        for &b in bytes {
            self.word[self.fill] = self.deint.push(b);
            self.fill += 1;
            if self.fill == RS_N {
                self.fill = 0;
                self.word_done(out);
            }
        }
    }

    fn word_done(&mut self, out: &mut impl FnMut(&OuterWord)) {
        if self.skip > 0 {
            self.skip -= 1;
            return;
        }
        let sync_seen = matches!(self.word[0], SYNC | SYNC_INVERTED);
        if let Dispersal::IsdbFrame { .. } = self.dispersal {
            scramble(&mut self.word[1..], self.dispersal.offset(self.n));
        }
        self.n += 1;
        let received = self.word;
        let status = match self.rs.decode(&mut self.word, &[]) {
            Ok(d) if d.corrected == 0 => WordStatus::Clean,
            Ok(d) => {
                let bits: u32 = self
                    .word
                    .iter()
                    .zip(&received)
                    .map(|(a, b)| (a ^ b).count_ones())
                    .sum();
                WordStatus::Corrected {
                    bytes: d.corrected as u16,
                    bits: bits as u16,
                }
            }
            Err(_) => WordStatus::Failed,
        };
        self.count(status);
        let failed = status == WordStatus::Failed;
        let mut packet = [0u8; TS];
        packet.copy_from_slice(&self.word[..TS]);
        if self.dispersal == Dispersal::DvbGroups {
            if !failed && packet[0] == SYNC_INVERTED {
                self.group = Some(0);
            }
            let Some(g) = self.group else {
                return;
            };
            scramble(&mut packet[1..], self.dispersal.offset(g));
            self.group = Some((g + 1) % GROUP);
        }
        packet[0] = SYNC;
        if failed {
            packet[1] |= 0x80;
            self.stats.flagged += 1;
        }
        self.stats.delivered += 1;
        out(&OuterWord {
            packet,
            status,
            sync_seen,
        });
    }

    fn count(&mut self, status: WordStatus) {
        let s = &mut self.stats;
        s.words += 1;
        match status {
            WordStatus::Clean => s.clean += 1,
            WordStatus::Corrected { bytes, bits } => {
                s.corrected += 1;
                s.bytes_corrected += bytes as u64;
                s.bits_corrected += bits as u64;
                self.batch_changed += bytes as usize;
            }
            WordStatus::Failed => {
                s.failed += 1;
                self.batch_changed += 8;
            }
        }
        s.run_failed = if status == WordStatus::Failed {
            s.run_failed + 1
        } else {
            0
        };
        self.batch_bytes += RS_N;
        if self.batch_bytes >= RS_N * 64 {
            let f = self.batch_changed as f64 / self.batch_bytes as f64;
            s.pre_rs += 0.1 * (f - s.pre_rs);
            self.batch_changed = 0;
            self.batch_bytes = 0;
        }
    }
}

/// The transmit side: transport packets in, interleaved Reed-Solomon words out. It allocates nothing after
/// [`OuterEncoder::new`].
#[derive(Clone, Debug)]
pub struct OuterEncoder {
    rs: Rs,
    int: DelayLanes<u8>,
    dispersal: Dispersal,
    n: usize,
}

impl OuterEncoder {
    /// An encoder at the start of a dispersal group or OFDM frame.
    pub fn new(dispersal: Dispersal) -> OuterEncoder {
        OuterEncoder {
            rs: rs(),
            int: interleaver(false),
            dispersal,
            n: 0,
        }
    }

    /// Encodes one packet into 204 interleaved bytes. The sync byte of `packet` is ignored: the encoder writes 0x47, or 0xB8
    /// at the start of a DVB group.
    pub fn encode(&mut self, packet: &[u8; TS], out: &mut [u8; RS_N]) {
        out[..TS].copy_from_slice(packet);
        out[0] = SYNC;
        let n = self.n;
        self.n = self.n.wrapping_add(1);
        if self.dispersal == Dispersal::DvbGroups {
            if n.is_multiple_of(GROUP) {
                out[0] = SYNC_INVERTED;
            }
            scramble(&mut out[1..TS], self.dispersal.offset(n));
        }
        // The word is 204 bytes and the code RS(204,188), so the length always matches.
        let _ = self.rs.encode_word(out);
        if let Dispersal::IsdbFrame { .. } = self.dispersal {
            scramble(&mut out[1..], self.dispersal.offset(n));
        }
        self.int.process(out);
    }
}

/// Where a transport stream starts in a bit stream.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyncHit {
    /// The bit index of a sync byte.
    pub start: usize,
    /// The stream is inverted: the bits must be complemented.
    pub inverted: bool,
    /// Places that hold a sync byte.
    pub hits: usize,
    /// Places looked at.
    pub places: usize,
}

/// What [`sync_search`] accepts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyncRule {
    /// Bytes from one sync byte to the next: 204 for an RS coded stream (also when interleaved), 188 for plain packets.
    pub spacing: usize,
    /// Fewest places that must be looked at.
    pub min_places: usize,
    /// Smallest fraction of places that must hold a sync byte.
    pub min_fraction: f32,
    /// The stream has DVB dispersal groups. Then one sync byte in 8 is inverted, both values must be seen, and the more frequent
    /// value determines the polarity. Without groups only 0x47 counts, and 0xB8 counts as an inverted 0x47.
    pub groups: bool,
    /// An inverted stream is accepted.
    pub allow_inverted: bool,
}

/// Searches hard bits (one per byte, 0 or 1) for the sync bytes of a transport stream at every bit phase within one spacing.
/// It returns the phase with the most sync bytes if that phase meets `rule`, else `None`. It allocates nothing.
pub fn sync_search(bits: &[u8], rule: &SyncRule) -> Option<SyncHit> {
    let words: &[u8] = if rule.groups {
        &[SYNC, SYNC_INVERTED]
    } else {
        &[SYNC]
    };
    let found = sdrkit_util_bits::sync::byte_sync_search(
        bits,
        words,
        rule.spacing,
        1,
        rule.allow_inverted && !rule.groups,
    )?;
    if found.places < rule.min_places
        || (found.hits as f32) < rule.min_fraction * found.places as f32
    {
        return None;
    }
    let mut inverted = found.inverted;
    if rule.groups {
        let (mut plain, mut inv) = (0usize, 0usize);
        let mut at = found.start;
        while at + 8 <= bits.len() {
            let b = bits[at..at + 8]
                .iter()
                .fold(0u8, |a, &x| (a << 1) | (x & 1));
            plain += (b == SYNC) as usize;
            inv += (b == SYNC_INVERTED) as usize;
            at += rule.spacing * 8;
        }
        if plain == 0 || inv == 0 {
            return None;
        }
        inverted = inv > plain;
    }
    if inverted && !rule.allow_inverted {
        return None;
    }
    Some(SyncHit {
        start: found.start,
        inverted,
        hits: found.hits,
        places: found.places,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(n: u32) -> [u8; TS] {
        let mut p = [0u8; TS];
        p[0] = SYNC;
        p[2] = 0x20;
        p[4..8].copy_from_slice(&n.to_be_bytes());
        p
    }

    fn bits_of(bytes: &[u8], lead: &[u8], invert: bool) -> Vec<u8> {
        let mut bits = lead.to_vec();
        for &b in bytes {
            let b = if invert { !b } else { b };
            for i in (0..8).rev() {
                bits.push((b >> i) & 1);
            }
        }
        bits
    }

    fn coded(dispersal: Dispersal, n: u32) -> Vec<u8> {
        let mut enc = OuterEncoder::new(dispersal);
        let mut bytes = Vec::new();
        for k in 0..n {
            let mut w = [0u8; RS_N];
            enc.encode(&packet(k), &mut w);
            bytes.extend_from_slice(&w);
        }
        bytes
    }

    #[test]
    fn dispersal_sequence_matches_en_300_421() {
        // EN 300 421 figure 2: the register 100101010000000 outputs 0000 0011 1111 0110 first.
        let s = dispersal_bytes();
        assert_eq!(&s[..2], &[0x03, 0xF6]);
        assert_eq!(s.len(), SEQ_PERIOD);
    }

    #[test]
    fn dvb_group_starts_with_an_inverted_sync_and_scrambles_the_rest() {
        // With all-zero packets, the 187 data bytes of packet 0 are the first 187 sequence bytes, and the sync byte is 0xB8.
        let mut w = [0u8; RS_N];
        let mut raw = OuterEncoder {
            int: DelayLanes::forney(1, 0, false, 0),
            ..OuterEncoder::new(Dispersal::DvbGroups)
        };
        raw.encode(&[0; TS], &mut w);
        assert_eq!(w[0], SYNC_INVERTED);
        assert_eq!(&w[1..TS], &dispersal_bytes()[..TS - 1]);
        // Packet 1 starts at sequence byte 188: byte 187 belongs to the sync byte of packet 1, which is not scrambled.
        raw.encode(&[0; TS], &mut w);
        assert_eq!(w[0], SYNC);
        assert_eq!(&w[1..TS], &dispersal_bytes()[TS..2 * TS - 1]);
    }

    #[test]
    fn isdb_frame_dispersal_covers_the_parity_and_runs_through_sync_bytes() {
        let mut raw = OuterEncoder {
            int: DelayLanes::forney(1, 0, false, 0),
            ..OuterEncoder::new(Dispersal::IsdbFrame { words: 2 })
        };
        let mut w = [0u8; RS_N];
        // The data bytes of an all-zero packet become the sequence from byte 1 on. The parity is scrambled too.
        raw.encode(&[0; TS], &mut w);
        assert_eq!(w[0], SYNC);
        assert_eq!(w[1], 0xF6);
        assert_eq!(&w[1..TS], &dispersal_bytes()[1..TS]);
        let mut plain = [0u8; RS_N];
        plain[0] = SYNC;
        rs().encode_word(&mut plain).unwrap();
        for j in TS..RS_N {
            assert_eq!(w[j], plain[j] ^ dispersal_bytes()[j], "parity byte {j}");
        }
        raw.encode(&[0; TS], &mut w);
        assert_eq!(&w[1..TS], &dispersal_bytes()[RS_N + 1..RS_N + TS]);
        // The third word starts a new frame.
        raw.encode(&[0; TS], &mut w);
        assert_eq!(w[1], 0xF6);
    }

    #[test]
    fn interleaver_keeps_sync_bytes_in_branch_0_and_delays_11_words() {
        assert_eq!(DELAY_WORDS, 11);
        assert_eq!(((BRANCHES - 1) * DEPTH * BRANCHES) as usize, 11 * RS_N);
        let bytes = coded(Dispersal::DvbGroups, 30);
        for k in 0..30 {
            assert!(matches!(bytes[k * RS_N], SYNC | SYNC_INVERTED), "word {k}");
        }
    }

    #[test]
    fn round_trips_every_dispersal_with_errors() {
        for d in [
            Dispersal::DvbGroups,
            Dispersal::IsdbFrame { words: 13 },
            Dispersal::None,
        ] {
            let mut bytes = coded(d, 100);
            for i in (500..bytes.len()).step_by(97) {
                bytes[i] ^= 0x5A;
            }
            let mut dec = OuterDecoder::new(d);
            let mut got = Vec::new();
            dec.push(&bytes, &mut |w| got.push(*w));
            assert_eq!(dec.stats.failed, 0, "{d:?}");
            assert!(got.len() >= 80, "{d:?}: {}", got.len());
            let first = u32::from_be_bytes(got[0].packet[4..8].try_into().unwrap());
            if d != Dispersal::DvbGroups {
                assert_eq!(first, 0, "{d:?}");
            }
            for (k, w) in got.iter().enumerate() {
                assert_eq!(w.packet, packet(first + k as u32), "{d:?} packet {k}");
            }
        }
    }

    #[test]
    fn a_failed_word_carries_the_error_indicator() {
        let mut bytes = coded(Dispersal::None, 20);
        // Two whole words in error: each de-interleaved word gets 34 bad bytes, more than t = 8.
        for b in &mut bytes[15 * RS_N..17 * RS_N] {
            *b ^= 0xFF;
        }
        let mut dec = OuterDecoder::new(Dispersal::None);
        let mut got = Vec::new();
        dec.push(&bytes, &mut |w| got.push(*w));
        let bad: Vec<_> = got.iter().filter(|w| w.failed()).collect();
        assert!(!bad.is_empty());
        assert!(bad
            .iter()
            .all(|w| w.packet[1] & 0x80 != 0 && w.packet[0] == SYNC));
        assert_eq!(dec.stats.flagged, bad.len() as u64);
    }

    #[test]
    fn sync_search_finds_phase_and_polarity() {
        let bytes = coded(Dispersal::DvbGroups, 60);
        let rule = SyncRule {
            spacing: RS_N,
            min_places: 20,
            min_fraction: 0.8,
            groups: true,
            allow_inverted: true,
        };
        for invert in [false, true] {
            let bits = bits_of(&bytes[50..], &[1, 0, 1], invert);
            let hit = sync_search(&bits, &rule).unwrap();
            assert_eq!((hit.start - 3) % 8, 0);
            assert_eq!(((hit.start - 3) / 8 + 50) % RS_N, 0);
            assert_eq!(hit.inverted, invert);
        }
        let bits = bits_of(&bytes, &[], true);
        let strict = SyncRule {
            allow_inverted: false,
            ..rule
        };
        assert_eq!(sync_search(&bits, &strict), None);
        // A stream without dispersal has no 0xB8.
        let plain = coded(Dispersal::None, 60);
        let bits = bits_of(&plain, &[], false);
        assert_eq!(sync_search(&bits, &rule), None);
        let hit = sync_search(
            &bits,
            &SyncRule {
                groups: false,
                ..rule
            },
        )
        .unwrap();
        assert_eq!(hit.start, 0);
    }
}
