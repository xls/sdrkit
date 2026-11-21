//! Repairs a frame whose CRC fails: the list search over the least certain symbols, and the search for lost samples.

use crate::coding::{Codec, Header, Payload, SymbolSoft};

/// The number of least certain symbols the list search changes, in every combination (`2^6 - 1` tries).
pub const LIST_SYMBOLS: usize = 6;

/// The work buffers of the recovery searches.
#[derive(Clone, Debug, Default)]
pub struct Recovery {
    work: Vec<SymbolSoft>,
    idx: Vec<usize>,
    trial: Payload,
    shifts: Vec<i32>,
    jumps: Vec<(usize, i32)>,
    found: Payload,
    found_jumps: u32,
    found_count: u32,
    budget: u64,
    max_jumps: usize,
}

impl Recovery {
    /// Empty buffers. They grow on the first frames.
    pub fn new() -> Self {
        Self::default()
    }

    /// The list search. The [`LIST_SYMBOLS`] least certain symbols (the smallest log-likelihood gap) change to their runner-up
    /// bin in every combination. Of the combinations with a right CRC, the one with the smallest total gap wins. If one wins,
    /// this writes it to `out` and returns true.
    ///
    /// The search works with soft and with hard decoding. In soft decoding the runner-up bin takes the strongest
    /// log-likelihood ([`SymbolSoft::runner_up`]), so each try decodes a different word.
    #[allow(clippy::too_many_arguments)]
    pub fn list_decode(
        &mut self,
        codec: &Codec,
        syms: &[SymbolSoft],
        ldro: bool,
        implicit: bool,
        h: Header,
        soft: bool,
        out: &mut Payload,
    ) -> bool {
        self.idx.clear();
        self.idx
            .extend((0..syms.len()).filter(|&i| syms[i].count >= 2));
        if self.idx.is_empty() {
            return false;
        }
        self.idx
            .sort_unstable_by(|&a, &b| syms[a].gap().total_cmp(&syms[b].gap()).then(a.cmp(&b)));
        self.idx.truncate(LIST_SYMBOLS);
        self.work.clear();
        self.work.extend_from_slice(syms);
        let mut best: Option<f32> = None;
        for mask in 1u32..(1 << self.idx.len()) {
            let mut cost = 0.0;
            for (b, &i) in self.idx.iter().enumerate() {
                if mask >> b & 1 == 1 {
                    self.work[i] = syms[i].runner_up();
                    cost += syms[i].gap();
                } else {
                    self.work[i] = syms[i];
                }
            }
            if best.is_some_and(|c| cost >= c) {
                continue;
            }
            codec.decode_payload(&self.work, ldro, implicit, h, soft, &mut self.trial);
            if self.trial.crc_ok {
                best = Some(cost);
                out.clone_from(&self.trial);
            }
        }
        best.is_some()
    }

    /// The search for lost samples. A receiver that drops samples moves every later symbol by the same number of bins (a
    /// jump). The search finds up to 3 jumps from the interleaver blocks whose hard bits stop being codewords. `q` is the jump
    /// size that the radio usually makes, in chips. If exactly one payload with clean blocks and a right CRC comes out, this
    /// writes it to `out` and returns the number of jumps undone.
    pub fn jump_search(
        &mut self,
        codec: &Codec,
        syms: &[SymbolSoft],
        ldro: bool,
        h: Header,
        q: f64,
        out: &mut Payload,
    ) -> Option<u32> {
        if syms.len() <= 8 || !q.is_finite() {
            return None;
        }
        let n = 1u32 << codec.sf();
        self.shifts.clear();
        for m in 1..=15i32 {
            for sign in [1i32, -1] {
                let base = (sign as f64 * m as f64 * q).round() as i32;
                for d in -2..=2 {
                    let s = base + d;
                    if s.rem_euclid(n as i32) != 0 && !self.shifts.contains(&s) {
                        self.shifts.push(s);
                    }
                }
            }
        }
        self.found_count = 0;
        self.budget = 200_000;
        self.max_jumps = 3;
        self.jumps.clear();
        self.go(codec, syms, ldro, h, 8);
        if self.found_count == 0 {
            self.shifts.clear();
            self.shifts.extend(1..n as i32);
            self.budget = 1_500_000;
            self.max_jumps = 2;
            self.jumps.clear();
            self.go(codec, syms, ldro, h, 8);
        }
        if self.found_count == 1 {
            out.clone_from(&self.found);
            Some(self.found_jumps)
        } else {
            None
        }
    }

    fn shift_at(jumps: &[(usize, i32)], k: usize) -> i32 {
        jumps.iter().filter(|j| j.0 <= k).map(|j| j.1).sum()
    }

    fn block_ok(
        &mut self,
        codec: &Codec,
        syms: &[SymbolSoft],
        h: Header,
        ldro: bool,
        at: usize,
    ) -> bool {
        if self.budget == 0 {
            return false;
        }
        self.budget -= 1;
        let n = 1u32 << codec.sf();
        let c = h.cr as usize;
        let mut local = [SymbolSoft::hard(1); 8];
        let end = (at + c).min(syms.len());
        for k in at..end {
            local[k - at] = syms[k].shifted(Self::shift_at(&self.jumps, k), n);
        }
        codec.block_clean(&local[..end - at], 0, h.cr, ldro)
    }

    /// The first later block from `from` that is not clean: `None` when the budget is spent, `Some(None)` when all are clean.
    fn first_bad(
        &mut self,
        codec: &Codec,
        syms: &[SymbolSoft],
        h: Header,
        ldro: bool,
        from: usize,
    ) -> Option<Option<usize>> {
        let c = h.cr as usize;
        let mut at = from;
        while at < syms.len() {
            if self.budget == 0 {
                return None;
            }
            if !self.block_ok(codec, syms, h, ldro, at) {
                return Some(Some(at));
            }
            at += c;
        }
        Some(None)
    }

    fn go(&mut self, codec: &Codec, syms: &[SymbolSoft], ldro: bool, h: Header, from: usize) {
        if self.budget == 0 || self.found_count > 1 {
            return;
        }
        let Some(bad) = self.first_bad(codec, syms, h, ldro, from) else {
            return;
        };
        match bad {
            None => {
                if self.jumps.is_empty() {
                    return;
                }
                let n = 1u32 << codec.sf();
                self.work.clear();
                for (k, s) in syms.iter().enumerate() {
                    self.work.push(s.shifted(Self::shift_at(&self.jumps, k), n));
                }
                codec.decode_payload(&self.work, ldro, false, h, true, &mut self.trial);
                if !self.trial.crc_ok {
                    return;
                }
                if self.found_count == 0 {
                    self.found.clone_from(&self.trial);
                    self.found_jumps = self.jumps.len() as u32;
                    self.found_count = 1;
                } else if self.found.bytes != self.trial.bytes {
                    self.found_count = 2;
                }
            }
            Some(at) => {
                if self.jumps.len() >= self.max_jumps {
                    return;
                }
                let c = h.cr as usize;
                let end = (at + c).min(syms.len());
                for first in at..end {
                    for si in 0..self.shifts.len() {
                        if self.budget == 0 || self.found_count > 1 {
                            return;
                        }
                        let d = self.shifts[si];
                        self.jumps.push((first, d));
                        if self.block_ok(codec, syms, h, ldro, at) {
                            self.go(codec, syms, ldro, h, at + c);
                        }
                        self.jumps.pop();
                    }
                }
            }
        }
    }
}
