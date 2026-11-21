//! Every error pattern up to a weight for each syndrome of a small block code ([`SmallCode`]): the list decoder for a code
//! whose unique-decoding radius is too small, where a check above the code (a CRC, a frame layout) chooses among the
//! readings. A (26,16) code with 10 check bits corrects every single error but not every double one; with the list, each
//! word keeps all its readings of up to two errors and the frame check picks the one that holds.
//!
//! ```
//! use sdrkit_algo_fec::small::SmallCode;
//! use sdrkit_algo_fec::syndromelist::SyndromeList;
//! let code = SmallCode::cyclic(26, 0x5B9).unwrap();
//! let list = SyndromeList::new(&code, 2);
//! let cw = code.encode(0xBEEF);
//! let readings: Vec<_> = list.readings(&code, cw ^ 0b101 << 7).collect();
//! assert!(readings.contains(&(cw, 2)));
//! ```

use crate::small::SmallCode;

/// The error patterns of each syndrome, lightest first.
#[derive(Clone, Debug)]
pub struct SyndromeList {
    lists: Vec<Vec<(u128, u8)>>,
    max_weight: usize,
}

impl SyndromeList {
    /// The patterns of `code` of weight 0 to `max_weight` (at most 3), grouped by syndrome.
    pub fn new(code: &SmallCode, max_weight: usize) -> SyndromeList {
        let max_weight = max_weight.min(3);
        let n = code.n();
        let mut lists = vec![Vec::new(); 1usize << code.r()];
        let mut add = |e: u128, w: u8| lists[code.syndrome(e) as usize].push((e, w));
        add(0, 0);
        for i in 0..n {
            if max_weight >= 1 {
                add(1u128 << i, 1);
            }
            for j in i + 1..n {
                if max_weight >= 2 {
                    add(1u128 << i | 1u128 << j, 2);
                }
                for k in j + 1..n {
                    if max_weight >= 3 {
                        add(1u128 << i | 1u128 << j | 1u128 << k, 3);
                    }
                }
            }
        }
        for l in lists.iter_mut() {
            l.sort_by_key(|&(_, w)| w);
        }
        SyndromeList { lists, max_weight }
    }

    /// The largest weight listed.
    pub fn max_weight(&self) -> usize {
        self.max_weight
    }

    /// The patterns (error, weight) whose syndrome is `syndrome`, lightest first.
    pub fn patterns(&self, syndrome: u32) -> &[(u128, u8)] {
        self.lists.get(syndrome as usize).map_or(&[], Vec::as_slice)
    }

    /// Every reading of the received `word`: (codeword, number of bits corrected), lightest first.
    pub fn readings<'a>(&'a self, code: &SmallCode, word: u128) -> impl Iterator<Item = (u128, u8)> + 'a {
        self.patterns(code.syndrome(word))
            .iter()
            .map(move |&(e, w)| (word ^ e, w))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_26_16_code_lists_every_pattern_of_two_errors_once() {
        let code = SmallCode::cyclic(26, 0x5B9).unwrap();
        let list = SyndromeList::new(&code, 2);
        let total: usize = (0..1024).map(|s| list.patterns(s).len()).sum();
        assert_eq!(total, 1 + 26 + 325);
        let cw = code.encode(0x1234);
        for i in 0..26 {
            for j in i + 1..26 {
                let r: Vec<_> = list.readings(&code, cw ^ (1u128 << i) ^ (1u128 << j)).collect();
                assert!(r.contains(&(cw, 2)), "{i} {j}");
                assert!(r.iter().all(|&(c, _)| code.syndrome(c) == 0));
            }
        }
        assert_eq!(list.readings(&code, cw).next(), Some((cw, 0)));
    }
}
