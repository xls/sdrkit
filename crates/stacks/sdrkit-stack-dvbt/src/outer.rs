//! The DVB-T outer decoder: the byte and packet alignment of the bits after Viterbi, the lock and phase-search rules, and the BER
//! after Viterbi. The outer code itself is `sdrkit_proto_mpegts::outer`.
//!
//! Standard: ETSI EN 300 744 clauses 4.3.1 and 4.3.2.

use sdrkit_proto_mpegts::outer::{
    self, Dispersal, OuterDecoder as Words, OuterStats, SyncRule, WordStatus, RS_N as N,
};
use std::collections::VecDeque;

/// The packet sync rule in the interleaved bits: 204-byte spacing, 12 words looked at, 3 in 4 with a sync byte.
const RULE: SyncRule = SyncRule {
    spacing: N,
    min_places: 12,
    min_fraction: 0.75,
    groups: true,
    allow_inverted: false,
};

/// The outer decoder.
pub struct OuterDecoder {
    words: Words,
    bits: Vec<u8>,
    bits_start: usize,
    bytes: Vec<u8>,
    locked: bool,
    bad_run: u32,
    /// Packet alignment holds.
    pub fec_sync: bool,
    /// Alignment searches since the last success.
    pub searches: u32,
    /// Packet sync was not found after 32 searches: the inner decoder must pick its phase again (taken by the owner).
    pub rephase: bool,
    /// The words that Reed-Solomon decoded in the last second: (time, bits corrected).
    window: VecDeque<(f64, u64)>,
    /// BER after Viterbi over the last second: the bits that Reed-Solomon corrected over the bits of the words that it
    /// decoded (204 bytes each). The words that it could not correct are not in it; the counter of failed words has
    /// them. 0 while no word decoded in that second.
    pub ber_post: f64,
}

impl Default for OuterDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// The bytes of a delivered transport packet.
pub type PacketBytes = [u8; 188];

impl OuterDecoder {
    /// A new decoder.
    pub fn new() -> Self {
        OuterDecoder {
            words: Words::new(Dispersal::DvbGroups),
            bits: Vec::new(),
            bits_start: 0,
            bytes: Vec::with_capacity(N),
            locked: false,
            bad_run: 0,
            fec_sync: false,
            searches: 0,
            rephase: false,
            window: VecDeque::new(),
            ber_post: 0.0,
        }
    }

    /// Starts afresh (keeps the counters).
    pub fn reset(&mut self) {
        self.bits.clear();
        self.bits_start = 0;
        self.locked = false;
        self.bad_run = 0;
        self.words.restart();
        self.fec_sync = false;
        self.searches = 0;
    }

    /// The RS and packet counters since the last [`OuterDecoder::reset_counters`].
    pub fn stats(&self) -> &OuterStats {
        &self.words.stats
    }

    /// Clears the counters too.
    pub fn reset_counters(&mut self) {
        self.reset();
        self.words.reset_stats();
        self.window.clear();
        self.ber_post = 0.0;
    }

    fn search(&mut self) -> bool {
        let avail = self.bits.len() - self.bits_start;
        if avail < 12 * N * 8 + 8 {
            return false;
        }
        let blocks = (avail - 8) / (N * 8);
        let window = &self.bits[self.bits_start..self.bits_start + blocks * N * 8 + 8];
        let hit = outer::sync_search(window, &RULE).filter(|h| h.hits >= 8);
        self.searches += 1;
        if let Some(h) = hit {
            self.bits_start += h.start;
            self.locked = true;
            self.fec_sync = true;
            self.words.stats.alignments += 1;
            self.searches = 0;
            self.words.restart();
            self.bad_run = 0;
            return true;
        }
        if avail >= 40 * N * 8 {
            // Keep the newest 20 blocks: the search stays bounded on an undecodable signal.
            let keep = 20 * N * 8;
            self.bits_start = self.bits.len() - keep;
        }
        if self.searches >= 32 {
            self.rephase = true;
            self.searches = 0;
        }
        false
    }

    /// Decoded bits (one per byte). Delivered packets go to `out` with their TEI flag.
    pub fn push(&mut self, bits: &[u8], time: f64, out: &mut Vec<(PacketBytes, bool, f64)>) {
        self.bits.extend_from_slice(bits);
        if self.locked || self.search() {
            let whole = (self.bits.len() - self.bits_start) / 8;
            self.bytes.clear();
            self.bytes.extend(
                self.bits[self.bits_start..self.bits_start + 8 * whole]
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|b| b.iter().fold(0u8, |a, &x| (a << 1) | x)),
            );
            self.bits_start += 8 * whole;
            let (window, bad_run) = (&mut self.window, &mut self.bad_run);
            let mut lost = false;
            for chunk in self.bytes.chunks(N) {
                self.words.push(chunk, &mut |w| {
                    // A word that Reed-Solomon could not correct tells nothing about its bit errors: it is left out.
                    match w.status {
                        WordStatus::Clean => window.push_back((time, 0)),
                        WordStatus::Corrected { bits, .. } => window.push_back((time, bits as u64)),
                        WordStatus::Failed => {}
                    }
                    if w.failed() && !w.sync_seen {
                        *bad_run += 1;
                    } else {
                        *bad_run = 0;
                    }
                    out.push((w.packet, w.failed(), time));
                });
                if *bad_run > 100 {
                    lost = true;
                    break;
                }
            }
            if lost {
                self.locked = false;
                self.fec_sync = false;
                self.rephase = true;
            }
            while self.window.front().is_some_and(|w| time - w.0 > 1.0) {
                self.window.pop_front();
            }
            let cb: u64 = self.window.iter().map(|w| w.1).sum();
            let n = self.window.len();
            self.ber_post = if n > 0 {
                cb as f64 / (n * N * 8) as f64
            } else {
                0.0
            };
        }
        // Bounded memory.
        if self.bits_start > 1 << 16 {
            self.bits.drain(..self.bits_start);
            self.bits_start = 0;
        }
    }
}
