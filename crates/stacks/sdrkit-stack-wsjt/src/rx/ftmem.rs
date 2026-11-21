//! What the FT8, FT4 and FT2 decoders keep across slots: the soft bits of deep-stage candidates that failed, for cross-slot
//! combining, and the calls heard on the band with the a-priori hypotheses made from them.

use crate::proto::msg77::{is_call_word, pack77};
use sdrkit_util_bits::pack::to_u64 as get_bits;
use std::collections::VecDeque;

/// The bits of a code word.
pub const N: usize = 174;
/// The soft-bit sets of a candidate: plain, and the two coherent group sizes.
pub const SETS: usize = 3;
/// The most candidates the store holds.
pub const STORE_CAP: usize = 1500;
/// Stored candidates older than this many periods are dropped.
pub const STORE_AGE: f64 = 6.5;
/// Heard calls older than this many periods are dropped.
pub const HEARD_AGE: f64 = 8.5;
/// The pairs and the CQ callers that make hypotheses: the newest this many of each.
pub const HYPOTHESIS_SOURCES: usize = 6;
/// Two candidates are the same signal within this frequency, Hz ...
pub const SAME_HZ: f64 = 3.0;
/// ... and this DT, seconds.
pub const SAME_DT: f64 = 0.4;

/// A deep-stage candidate that failed every decode.
#[derive(Clone, Debug)]
pub struct Stored {
    /// Heard upside down.
    pub mirrored: bool,
    /// Its slot start, UTC seconds.
    pub slot: f64,
    /// The frequency of tone 0, Hz.
    pub hz: f64,
    /// Its DT, seconds.
    pub dt: f64,
    /// The three soft-bit sets, `N` values each, one after the other.
    pub llr: Box<[f32]>,
}

/// The candidates of the deep stage that failed, newest last, at most [`STORE_CAP`].
#[derive(Clone, Debug, Default)]
pub struct SoftStore {
    entries: VecDeque<Stored>,
}

impl SoftStore {
    /// Drops the entries older than [`STORE_AGE`] periods before the slot starting at `utc`.
    pub fn begin_slot(&mut self, utc: f64, period: f64) {
        self.entries.retain(|e| utc - e.slot <= STORE_AGE * period);
    }

    /// The entries of the same sideband from slots that start more than half a period before `utc`, within [`SAME_HZ`] and
    /// [`SAME_DT`]; their indices go to `out`.
    pub fn matches(
        &self,
        mirrored: bool,
        utc: f64,
        period: f64,
        hz: f64,
        dt: f64,
        out: &mut Vec<usize>,
    ) {
        out.clear();
        out.extend(
            self.entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.mirrored == mirrored
                        && e.slot < utc - 0.5 * period
                        && (e.hz - hz).abs() <= SAME_HZ
                        && (e.dt - dt).abs() <= SAME_DT
                })
                .map(|(i, _)| i),
        );
    }

    /// Set `k` (0 plain, 1 and 2 the coherent groups) of entry `i`.
    pub fn set(&self, i: usize, k: usize) -> &[f32] {
        &self.entries[i].llr[k * N..(k + 1) * N]
    }

    /// Whether a candidate within [`SAME_HZ`] and [`SAME_DT`] of the same slot and sideband is stored.
    pub fn has(&self, mirrored: bool, slot: f64, hz: f64, dt: f64) -> bool {
        self.entries.iter().rev().any(|e| {
            e.slot == slot
                && e.mirrored == mirrored
                && (e.hz - hz).abs() <= SAME_HZ
                && (e.dt - dt).abs() <= SAME_DT
        })
    }

    /// Stores a failed candidate, unless [`SoftStore::has`] one of the same slot and sideband. The oldest entry goes when the
    /// store is full. `sets` holds the three sets of `N` values.
    pub fn put(&mut self, mirrored: bool, slot: f64, hz: f64, dt: f64, sets: [&[f32]; SETS]) {
        if sets.iter().any(|s| s.len() != N) || self.has(mirrored, slot, hz, dt) {
            return;
        }
        let llr: Box<[f32]> = sets.concat().into_boxed_slice();
        self.entries.push_back(Stored {
            mirrored,
            slot,
            hz,
            dt,
            llr,
        });
        while self.entries.len() > STORE_CAP {
            self.entries.pop_front();
        }
    }

    /// Entries held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Forgets every entry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// A message assumed for a-priori decoding: the 77 packed bits and which of them are known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hypothesis {
    /// The bits of the packed message (before scrambling).
    pub bits: [u8; 77],
    /// A pair "A B": bits 0-57 and 74-76 known. Otherwise a CQ caller "X": bits 0-28 and 74-76 known.
    pub pair: bool,
}

impl Hypothesis {
    /// The indices of the known bits.
    pub fn known(&self) -> impl Iterator<Item = usize> {
        let calls = if self.pair { 58 } else { 29 };
        (0..calls).chain(74..77)
    }

    /// The hypothesis of the message `text` (a standard message of type 1), or `None` when it does not pack as one.
    fn of(text: &str, pair: bool) -> Option<Hypothesis> {
        let v = pack77(text).ok()?;
        if v.len() != 77 || get_bits(&v[74..77]) != 1 {
            return None;
        }
        let mut bits = [0u8; 77];
        bits.copy_from_slice(&v);
        Some(Hypothesis { bits, pair })
    }
}

/// The calls heard on the band by one stack, and the hypotheses made from them.
#[derive(Clone, Debug, Default)]
pub struct HeardCalls {
    /// (slot start, first call, second call), oldest first.
    pairs: VecDeque<(f64, String, String)>,
    /// (slot start, caller), oldest first.
    callers: VecDeque<(f64, String)>,
    hypotheses: Vec<Hypothesis>,
    changed: bool,
}

impl HeardCalls {
    /// Drops the calls older than [`HEARD_AGE`] periods before the slot starting at `utc`.
    pub fn begin_slot(&mut self, utc: f64, period: f64) {
        let (p, c) = (self.pairs.len(), self.callers.len());
        self.pairs.retain(|e| utc - e.0 <= HEARD_AGE * period);
        self.callers.retain(|e| utc - e.0 <= HEARD_AGE * period);
        self.changed |= p != self.pairs.len() || c != self.callers.len();
    }

    /// Takes the text of a decode of the slot starting at `slot`. The words are split at spaces and their angle brackets
    /// removed. A first word `CQ` makes the first later word that is a call a CQ caller; a first and second word that are both
    /// calls make a pair. Each caller and pair is taken once per slot.
    pub fn take(&mut self, slot: f64, text: &str) {
        let words: Vec<&str> = text
            .split_whitespace()
            .map(|w| w.trim_matches(|c| c == '<' || c == '>'))
            .collect();
        match words.as_slice() {
            ["CQ", rest @ ..] => {
                if let Some(x) = rest.iter().find(|w| is_call_word(w)) {
                    if !self.callers.iter().any(|e| e.0 == slot && e.1 == *x) {
                        self.callers.push_back((slot, (*x).to_owned()));
                        self.changed = true;
                    }
                }
            }
            [a, b, ..]
                if is_call_word(a)
                    && is_call_word(b)
                    && !self
                        .pairs
                        .iter()
                        .any(|e| e.0 == slot && e.1 == *a && e.2 == *b) =>
            {
                self.pairs
                    .push_back((slot, (*a).to_owned(), (*b).to_owned()));
                self.changed = true;
            }
            _ => {}
        }
    }

    /// The hypotheses, rebuilt when the calls changed: for each of the newest [`HYPOTHESIS_SOURCES`] pairs (A, B), "A B RR73"
    /// and "B A RR73"; for each of the newest [`HYPOTHESIS_SOURCES`] callers X, "X K1ABC FN42". A message that does not pack as
    /// type 1, and a repeated one, is skipped.
    pub fn hypotheses(&mut self) -> &[Hypothesis] {
        if std::mem::take(&mut self.changed) {
            let mut h: Vec<Hypothesis> = Vec::new();
            let mut add = |x: Option<Hypothesis>| {
                if let Some(x) = x {
                    if !h.contains(&x) {
                        h.push(x);
                    }
                }
            };
            for (_, a, b) in self.pairs.iter().rev().take(HYPOTHESIS_SOURCES) {
                add(Hypothesis::of(&format!("{a} {b} RR73"), true));
                add(Hypothesis::of(&format!("{b} {a} RR73"), true));
            }
            for (_, x) in self.callers.iter().rev().take(HYPOTHESIS_SOURCES) {
                add(Hypothesis::of(&format!("{x} K1ABC FN42"), false));
            }
            self.hypotheses = h;
        }
        &self.hypotheses
    }

    /// Forgets every call.
    pub fn clear(&mut self) {
        self.pairs.clear();
        self.callers.clear();
        self.hypotheses.clear();
        self.changed = false;
    }
}

/// What one stack's FT decoder keeps across slots, for one dial.
#[derive(Clone, Debug, Default)]
pub struct FtMemory {
    /// The failed candidates, for combining.
    pub store: SoftStore,
    /// The calls heard, for a-priori decoding.
    pub heard: HeardCalls,
    /// The dial and the start of the last slot.
    last: Option<(f64, f64)>,
}

impl FtMemory {
    /// Drops what is too old before the slot starting at `utc` on the dial `dial_hz`; everything when the dial changed (a
    /// retune, a band hop) or the slots went back in time.
    pub fn begin_slot(&mut self, utc: f64, period: f64, dial_hz: f64) {
        if self.last.is_some_and(|(d, t)| d != dial_hz || utc < t) {
            self.clear();
        }
        self.last = Some((dial_hz, utc));
        self.store.begin_slot(utc, period);
        self.heard.begin_slot(utc, period);
    }

    /// Forgets everything.
    pub fn clear(&mut self) {
        self.store.clear();
        self.heard.clear();
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_and_hypotheses() {
        let mut h = HeardCalls::default();
        h.take(0.0, "CQ W9XYZ EN37");
        h.take(0.0, "CQ W9XYZ EN37");
        h.take(15.0, "W9XYZ <K1ABC> FN42");
        h.take(15.0, "CQ DX G4ABC IO91");
        h.take(15.0, "TNX BOB 73 GL");
        h.take(15.0, "K1ABC RR73 W9XYZ");
        assert_eq!(h.callers.len(), 2);
        assert_eq!(h.pairs.len(), 1);
        let x = h.hypotheses().to_vec();
        assert_eq!(x.len(), 4);
        assert!(x[0].pair && x[0].known().count() == 61);
        assert!(!x[2].pair && x[2].known().count() == 32);
        assert_eq!(&x[2].bits[..29], &pack77("G4ABC K1ABC FN42").unwrap()[..29]);
        h.begin_slot(15.0 + 9.0 * 15.0, 15.0);
        assert!(h.hypotheses().is_empty());
    }

    #[test]
    fn the_store_matches_and_ages() {
        let mut s = SoftStore::default();
        let v = vec![1.0f32; N];
        s.put(false, 0.0, 1000.0, 0.1, [&v, &v, &v]);
        s.put(false, 0.0, 1002.0, 0.2, [&v, &v, &v]);
        s.put(true, 0.0, 1000.0, 0.1, [&v, &v, &v]);
        assert_eq!(s.len(), 2);
        let mut m = Vec::new();
        s.matches(false, 15.0, 15.0, 1002.5, 0.4, &mut m);
        assert_eq!(m, vec![0]);
        s.matches(false, 5.0, 15.0, 1000.0, 0.1, &mut m);
        assert!(m.is_empty());
        s.begin_slot(6.5 * 15.0 + 1.0, 15.0);
        assert!(s.is_empty());
    }

    #[test]
    fn a_new_dial_forgets_the_memory() {
        let mut m = FtMemory::default();
        let v = vec![1.0f32; N];
        m.begin_slot(0.0, 15.0, 14.074e6);
        m.store.put(false, 0.0, 1000.0, 0.1, [&v, &v, &v]);
        m.heard.take(0.0, "CQ W9XYZ EN37");
        m.begin_slot(15.0, 15.0, 14.074e6);
        assert_eq!(m.store.len(), 1);
        assert_eq!(m.heard.hypotheses().len(), 1);
        m.begin_slot(30.0, 15.0, 7.074e6);
        assert!(m.store.is_empty() && m.heard.hypotheses().is_empty());
        m.store.put(false, 30.0, 1000.0, 0.1, [&v, &v, &v]);
        m.begin_slot(15.0, 15.0, 7.074e6);
        assert!(m.store.is_empty());
    }
}
