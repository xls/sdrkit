//! Chase soft-decision decoding over an algebraic (hard-decision) decoder, with the margin to the next code word and the cost
//! of the repair.
//!
//! Given the hard bits `b` of a message, soft values `y` (`|y|` the reliability of each bit), a code word that spans some of the
//! message, a hard decoder that repairs a word or refuses it, and the message's own checks:
//!
//! 1. take the `L` least reliable positions of the code word (smallest `|y|`);
//! 2. for each of the `2^L` patterns (or those of at most `max_flips` bits) flip them and run the hard decoder; skip a pattern
//!    the decoder refuses, one whose repaired word is one already kept, and one that already failed the checks (the last
//!    `max_failed` failed words are kept);
//! 3. a repaired word that passes the checks has the distance `sum |y_k|` over the positions where it differs from `b`; the
//!    smallest is `d1` (the winner), the second smallest of another word `d2`;
//! 4. the margin is `(d2 - d1) / mean |y|` ([`NO_SECOND`] without a second word) and the cost `d1 / mean |y|`, `mean |y|` taken
//!    over the bits after the sync.
//!
//! A hard decoder is anything that implements [`HardDecoder`]: [`Bch`], [`SmallCode`], or a closure. [`ChaseDecoder`] keeps its
//! work space between messages, so after the first message of a size it allocates nothing.
//!
//! ```
//! use sdrkit_algo_fec::bch::{codes, Bch};
//! use sdrkit_algo_fec::chase::ChaseDecoder;
//! let mut bch = Bch::from_code(&codes::SARSAT_38_26).unwrap();
//! let mut word = vec![0u8; 38];
//! for (i, b) in word[..26].iter_mut().enumerate() {
//!     *b = (i % 3 == 1) as u8;
//! }
//! bch.encode(&mut word).unwrap();
//! let sent = word.clone();
//! // Three errors (beyond t = 2), all on the weakest bits.
//! let mut soft: Vec<f32> = word.iter().map(|&b| if b == 1 { 1.0 } else { -1.0 }).collect();
//! for i in [3, 17, 30] {
//!     word[i] ^= 1;
//!     soft[i] = -0.1 * soft[i];
//! }
//! let mut out = vec![0u8; 38];
//! let r = ChaseDecoder::new(7, 7, 32).unwrap().decode(&mut bch, &word, &soft, 0..38, 0, |_| true, &mut out).unwrap();
//! assert_eq!(out, sent);
//! assert_eq!(r.changed, 3);
//! ```

use crate::bch::Bch;
use crate::error::{Param, Result};
use crate::small::{SmallCode, MAX_CHASE};
use std::ops::Range;

/// The margin reported when no second code word passes.
pub const NO_SECOND: f32 = 99.0;
/// Failed words kept to skip their repeats.
pub const DEFAULT_MAX_FAILED: usize = 32;

/// A decoder that repairs a word in place or refuses it.
pub trait HardDecoder {
    /// Repairs `word` (one bit per byte) in place; `false` when the decoder refuses it (the word may then hold anything).
    fn repair(&mut self, word: &mut [u8]) -> bool;
}

impl HardDecoder for Bch {
    fn repair(&mut self, word: &mut [u8]) -> bool {
        self.decode(word).is_ok()
    }
}

impl HardDecoder for SmallCode {
    fn repair(&mut self, word: &mut [u8]) -> bool {
        self.decode_bits(word).is_ok()
    }
}

impl<F: FnMut(&mut [u8]) -> bool> HardDecoder for F {
    fn repair(&mut self, word: &mut [u8]) -> bool {
        self(word)
    }
}

/// The outcome of [`ChaseDecoder::decode`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChaseOutcome {
    /// Bits of the message the winner changes.
    pub changed: u32,
    /// `d1`: the sum of `|y|` over the changed bits.
    pub distance: f32,
    /// `(d2 - d1) / mean |y|`, or [`NO_SECOND`].
    pub margin: f32,
    /// `d1 / mean |y|`: the repaired-bit cost.
    pub cost: f32,
    /// Patterns the hard decoder repaired (accepted or not by the checks).
    pub repaired: u32,
}

/// Chase soft-decision decoding over a hard decoder (module docs).
#[derive(Clone, Debug)]
pub struct ChaseDecoder {
    weakest: usize,
    max_flips: u32,
    max_failed: usize,
    weak: Vec<(f32, usize)>,
    trial: Vec<u8>,
    msg: Vec<u8>,
    best: Vec<u8>,
    second: Vec<u8>,
    failed: Vec<u8>,
    nfailed: usize,
    next_failed: usize,
}

impl ChaseDecoder {
    /// A decoder over the `weakest` least reliable bits (1 to [`MAX_CHASE`]), flipping at most `max_flips` of them at once
    /// (`weakest` for every pattern), keeping `max_failed` failed words ([`DEFAULT_MAX_FAILED`]). Errors for another `weakest`.
    pub fn new(weakest: usize, max_flips: usize, max_failed: usize) -> Result<Self> {
        Param::Bits.check(weakest, 1, MAX_CHASE as i64)?;
        Ok(ChaseDecoder {
            weakest,
            max_flips: max_flips.min(weakest) as u32,
            max_failed,
            weak: Vec::with_capacity(MAX_CHASE),
            trial: Vec::new(),
            msg: Vec::new(),
            best: Vec::new(),
            second: Vec::new(),
            failed: Vec::new(),
            nfailed: 0,
            next_failed: 0,
        })
    }

    /// Whether `w` is one of the failed words kept.
    fn has_failed(&self, w: &[u8]) -> bool {
        let n = w.len();
        (0..self.nfailed).any(|i| &self.failed[i * n..(i + 1) * n] == w)
    }

    /// Keeps a failed word (the oldest goes when full).
    fn add_failed(&mut self, n: usize) {
        if self.max_failed == 0 {
            return;
        }
        let i = self.next_failed;
        self.failed[i * n..(i + 1) * n].copy_from_slice(&self.trial);
        self.next_failed = (i + 1) % self.max_failed;
        self.nfailed = (self.nfailed + 1).min(self.max_failed);
    }

    /// Decodes the message `hard` (one bit per byte) with soft values `soft` (the same length, `|soft|` the reliability). The code
    /// word is `hard[span]`, which `decoder` repairs; the first `sync` bits are left out of the mean `|y|` and of the distances.
    /// `check` gets each whole repaired message and accepts or refuses it. The winning message goes to `out` (the length of
    /// `hard`). `None` when no pattern gives a word that passes, or when the lengths, the span or `sync` do not fit (`out` is
    /// then a copy of `hard`, when its length allows).
    #[allow(clippy::too_many_arguments)]
    pub fn decode(
        &mut self,
        decoder: &mut impl HardDecoder,
        hard: &[u8],
        soft: &[f32],
        span: Range<usize>,
        sync: usize,
        mut check: impl FnMut(&[u8]) -> bool,
        out: &mut [u8],
    ) -> Option<ChaseOutcome> {
        let len = hard.len();
        if out.len() == len {
            out.copy_from_slice(hard);
        }
        if soft.len() != len
            || out.len() != len
            || span.start >= span.end
            || span.end > len
            || sync >= len
        {
            return None;
        }
        let n = span.len();
        let rel = |v: f32| if v.is_nan() { 0.0 } else { v.abs() };
        let mean = (soft[sync..].iter().map(|&v| rel(v) as f64).sum::<f64>() / (len - sync) as f64)
            .max(1e-30) as f32;
        // The weakest positions of the code word.
        self.weak.clear();
        self.weak.extend(span.clone().map(|i| (rel(soft[i]), i)));
        let l = self.weakest.min(n);
        if l < self.weak.len() {
            self.weak
                .select_nth_unstable_by(l, |a, b| a.0.total_cmp(&b.0));
            self.weak.truncate(l);
        }
        self.trial.resize(n, 0);
        self.msg.clear();
        self.msg.extend_from_slice(hard);
        self.best.resize(n, 0);
        self.second.resize(n, 0);
        self.failed.resize(self.max_failed * n, 0);
        self.nfailed = 0;
        self.next_failed = 0;
        let (mut d1, mut d2) = (f32::INFINITY, f32::INFINITY);
        let mut repaired = 0u32;
        let distance = |w: &[u8]| -> f32 {
            w.iter()
                .zip(&hard[span.clone()])
                .enumerate()
                .filter(|&(k, (a, b))| (a & 1) != (b & 1) && span.start + k >= sync)
                .map(|(k, _)| rel(soft[span.start + k]))
                .sum()
        };
        for mask in 0u32..(1 << l) {
            if mask.count_ones() > self.max_flips {
                continue;
            }
            self.trial.copy_from_slice(&hard[span.clone()]);
            for (b, &(_, p)) in self.weak.iter().enumerate() {
                if mask >> b & 1 != 0 {
                    self.trial[p - span.start] ^= 1;
                }
            }
            if !decoder.repair(&mut self.trial) {
                continue;
            }
            repaired += 1;
            if (d1.is_finite() && self.trial == self.best)
                || (d2.is_finite() && self.trial == self.second)
                || self.has_failed(&self.trial)
            {
                continue;
            }
            self.msg[span.clone()].copy_from_slice(&self.trial);
            if !check(&self.msg) {
                self.add_failed(n);
                continue;
            }
            let d = distance(&self.trial);
            if d < d1 {
                std::mem::swap(&mut self.best, &mut self.second);
                d2 = d1;
                self.best.copy_from_slice(&self.trial);
                d1 = d;
            } else if d < d2 {
                self.second.copy_from_slice(&self.trial);
                d2 = d;
            }
        }
        if !d1.is_finite() {
            return None;
        }
        out[span.clone()].copy_from_slice(&self.best);
        let changed = out
            .iter()
            .zip(hard)
            .filter(|(a, b)| (*a & 1) != (*b & 1))
            .count() as u32;
        Some(ChaseOutcome {
            changed,
            distance: d1,
            margin: if d2.is_finite() {
                (d2 - d1) / mean
            } else {
                NO_SECOND
            },
            cost: d1 / mean,
            repaired,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bch::codes;

    /// Repeatable Gaussian values (xorshift and Box-Muller).
    struct Rng(u64);
    impl Rng {
        fn uniform(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        }
        fn gauss(&mut self) -> f64 {
            let (a, b) = (self.uniform(), self.uniform());
            (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
        }
    }

    /// A 112-bit message: 24 sync bits, the 82-bit BCH-1 word of the 406 MHz beacon, 6 more bits.
    fn message(bch: &Bch, rng: &mut Rng) -> Vec<u8> {
        let mut m = vec![0u8; 112];
        m[..15].fill(1);
        m[15..24].copy_from_slice(&[0, 0, 0, 1, 0, 1, 1, 1, 1]);
        for (i, b) in m.iter_mut().enumerate().skip(24) {
            if !(85..106).contains(&i) {
                *b = (rng.uniform() < 0.5) as u8;
            }
        }
        bch.encode(&mut m[24..106]).unwrap();
        m
    }

    #[test]
    fn chase_decodes_more_words_than_hard_decoding() {
        let mut bch = Bch::from_code(&codes::SARSAT_82_61).unwrap();
        let mut chase = ChaseDecoder::new(7, 7, DEFAULT_MAX_FAILED).unwrap();
        let mut rng = Rng(0x5eed);
        // BPSK at Eb/N0 = 2 dB on unit levels: about 3.8 % of the bits wrong before decoding.
        let sigma = (1.0 / (2.0 * 10f64.powf(0.2))).sqrt();
        let trials = 2000;
        let (mut hard_ok, mut chase_ok, mut chase_wrong) = (0, 0, 0);
        let mut out = vec![0u8; 112];
        for _ in 0..trials {
            let sent = message(&bch, &mut rng);
            let soft: Vec<f32> = sent
                .iter()
                .map(|&b| ((2.0 * b as f64 - 1.0) + sigma * rng.gauss()) as f32)
                .collect();
            let hard: Vec<u8> = soft.iter().map(|&v| (v > 0.0) as u8).collect();
            let mut w = hard[24..106].to_vec();
            if bch.decode(&mut w).is_ok() && w == sent[24..106] {
                hard_ok += 1;
            }
            if let Some(r) = chase.decode(&mut bch, &hard, &soft, 24..106, 24, |_| true, &mut out) {
                if out[24..106] == sent[24..106] {
                    chase_ok += 1;
                } else {
                    chase_wrong += 1;
                }
                assert!(r.cost >= 0.0 && r.margin >= 0.0);
            }
        }
        let (fh, fc) = (trials - hard_ok, trials - chase_ok);
        // Chase fails on under an eighth of the words hard decoding fails on. With no message checks a failure is a wrong word,
        // which the margin and cost rules of a stack refuse.
        assert!(fc * 8 < fh, "hard {fh} chase {fc} wrong {chase_wrong}");
    }

    #[test]
    fn margin_cost_and_checks() {
        let mut bch = Bch::from_code(&codes::SARSAT_82_61).unwrap();
        let mut chase = ChaseDecoder::new(7, 7, 4).unwrap();
        let mut rng = Rng(7);
        let sent = message(&bch, &mut rng);
        let soft: Vec<f32> = sent
            .iter()
            .map(|&b| if b == 1 { 1.0 } else { -1.0 })
            .collect();
        let mut out = vec![0u8; 112];
        // A clean word: nothing changed, no cost, a wide margin (the next code word is at least 7 bits away).
        let r = chase
            .decode(&mut bch, &sent, &soft, 24..106, 24, |_| true, &mut out)
            .unwrap();
        assert_eq!((r.changed, r.cost), (0, 0.0));
        assert!(r.margin >= 3.0, "{}", r.margin);
        assert_eq!(out, sent);
        // Four errors (beyond t = 3) on weak bits: repaired, at the cost of the four weak bits.
        let mut hard = sent.clone();
        let mut soft2 = soft.clone();
        for i in [30, 50, 70, 90] {
            hard[i] ^= 1;
            soft2[i] *= -0.2;
        }
        let r = chase
            .decode(&mut bch, &hard, &soft2, 24..106, 24, |_| true, &mut out)
            .unwrap();
        assert_eq!(out, sent);
        assert_eq!(r.changed, 4);
        let mean = soft2[24..].iter().map(|v| v.abs()).sum::<f32>() / 88.0;
        assert!((r.cost - 0.8 / mean).abs() < 1e-5);
        // Checks that refuse every word: nothing decodes and `out` is the received message.
        assert!(chase
            .decode(&mut bch, &hard, &soft2, 24..106, 24, |_| false, &mut out)
            .is_none());
        assert_eq!(out, hard);
        // A closure as the hard decoder, and bad ranges.
        let mut refuse = |_: &mut [u8]| false;
        assert!(chase
            .decode(&mut refuse, &hard, &soft2, 24..106, 24, |_| true, &mut out)
            .is_none());
        assert!(chase
            .decode(&mut bch, &hard, &soft2, 24..120, 24, |_| true, &mut out)
            .is_none());
        assert!(ChaseDecoder::new(0, 0, 0).is_err());
    }
}
