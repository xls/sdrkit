//! One code word from several received copies of it.
//!
//! Paging networks send many messages more than once. When a word is lost in one copy, the other copies can restore it. The
//! vote follows these steps:
//!
//! 1. Each copy that passes the hard repair votes with its repaired word. The word with the most votes wins. If two words have
//!    the same number of votes, the word that needed fewer repairs wins, then the earlier copy.
//! 2. If no copy passes and there are three or more copies, each bit is voted. A tie takes the bit of the first copy. Then the
//!    hard repair runs on the result.
//! 3. If no copy passes and there are two copies, every combination of the bits where they differ is tried (at most 10 bits).
//!    The vote returns a word only when exactly one combination is a code word.
//!
//! ```
//! use sdrkit_proto_pager::vote::vote;
//! use sdrkit_proto_pager::word::{self, Checked};
//! let cw = word::encode(0x1ABCDE);
//! let copies = [Checked::hard(cw ^ 0b111), Checked::hard(cw ^ 0b111 << 8), Checked::hard(cw ^ 0b111 << 16)];
//! assert!(copies.iter().all(|c| !c.valid));
//! assert_eq!(vote(&copies).map(|c| c.cw), Some(cw));
//! ```

use crate::word::{self, Checked};

/// The most copies that take part in one vote. More copies are ignored.
pub const MAX_COPIES: usize = 8;

/// The most bits where two copies can differ for the two-copy search.
pub const MAX_DIFFERENT_BITS: u32 = 10;

/// One word from up to [`MAX_COPIES`] copies (the current copy first). `None` when the copies give no word.
pub fn vote(copies: &[Checked]) -> Option<Checked> {
    let copies = &copies[..copies.len().min(MAX_COPIES)];
    let first = copies.first()?;
    // (word, votes, fewest repairs, first index)
    let mut tally = [(0u32, 0u32, 0u8, 0usize); MAX_COPIES];
    let mut used = 0;
    for (i, c) in copies.iter().enumerate() {
        let r = if c.valid {
            Some((c.cw, c.fixes))
        } else {
            word::repair(c.raw)
        };
        if let Some((w, f)) = r {
            match tally[..used].iter_mut().find(|t| t.0 == w) {
                Some(t) => {
                    t.1 += 1;
                    t.2 = t.2.min(f);
                }
                None => {
                    tally[used] = (w, 1, f, i);
                    used += 1;
                }
            }
        }
    }
    if used > 0 {
        let best = tally[..used].iter().copied().reduce(|a, b| {
            let better = b.1 > a.1 || (b.1 == a.1 && (b.2 < a.2 || (b.2 == a.2 && b.3 < a.3)));
            if better {
                b
            } else {
                a
            }
        })?;
        return Some(Checked {
            cw: best.0,
            raw: first.raw,
            valid: true,
            fixes: best.2,
        });
    }
    if copies.len() >= 3 {
        let mut v = 0u32;
        for b in 0..32 {
            let ones = copies.iter().filter(|c| (c.raw >> b) & 1 == 1).count();
            let zeros = copies.len() - ones;
            let bit = if ones != zeros {
                ones > zeros
            } else {
                (first.raw >> b) & 1 == 1
            };
            v |= (bit as u32) << b;
        }
        return word::repair(v).map(|(cw, fixes)| Checked {
            cw,
            raw: first.raw,
            valid: true,
            fixes,
        });
    }
    if copies.len() == 2 {
        let (a, b) = (copies[0].raw, copies[1].raw);
        let diff = a ^ b;
        let n = diff.count_ones();
        if n == 0 || n > MAX_DIFFERENT_BITS {
            return None;
        }
        let mut pos = [0u32; MAX_DIFFERENT_BITS as usize];
        let mut k = 0;
        let mut d = diff;
        while d != 0 {
            pos[k] = d.trailing_zeros();
            d &= d - 1;
            k += 1;
        }
        let mut found: Option<u32> = None;
        for m in 0u32..(1 << n) {
            let mut w = a;
            for (j, &p) in pos[..k].iter().enumerate() {
                if m & (1 << j) != 0 {
                    w ^= 1 << p;
                }
            }
            if word::is_valid(w) {
                if found.is_some_and(|f| f != w) {
                    return None;
                }
                found = Some(w);
            }
        }
        return found.map(|cw| Checked {
            cw,
            raw: a,
            valid: true,
            fixes: 0,
        });
    }
    None
}
