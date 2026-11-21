//! Interleavers that move blocks of items or keep part of a permuted block.
//!
//! - [`ConvBlocks`]: a convolutional interleaver whose cells hold a block of `U` items, with any delay per branch, and which
//!   tells the receiver which outputs were not yet filled (warm-up).
//! - [`KeptPermutation`]: the multiplicative permutation `b_w = a_(m w mod N)` followed by a keep pattern (keep the first `keep`
//!   of every `period` items), as ETSI EN 302 583 clause 5.4.2 (DVB-SH) uses it with 126 of 128.

use crate::error::{FecError, Param, Result};

/// A convolutional interleaver on blocks: `B` branches, branch `j` a FIFO of `delays[j]` cells of `U` items. The input switch
/// gives one block to each branch in turn and the output switch reads in step.
#[derive(Clone, Debug)]
pub struct ConvBlocks<T: Copy> {
    u: usize,
    delays: Vec<u32>,
    start: Vec<usize>,
    pos: Vec<u32>,
    filled: Vec<u32>,
    buf: Vec<T>,
    lane: usize,
}

impl<T: Copy> ConvBlocks<T> {
    /// Branches with `delays` (cells per branch) and blocks of `u` items, filled with `fill`. An error for no branch or `u = 0`.
    pub fn new(delays: &[u32], u: usize, fill: T) -> Result<Self> {
        if delays.is_empty() || u == 0 {
            return Err(FecError::OutOfRange {
                param: Param::Depth,
                min: 1,
                max: i64::MAX,
                got: 0,
            });
        }
        let mut start = Vec::with_capacity(delays.len());
        let mut total = 0usize;
        for &d in delays {
            start.push(total);
            total += d as usize * u;
        }
        Ok(ConvBlocks {
            u,
            delays: delays.to_vec(),
            start,
            pos: vec![0; delays.len()],
            filled: vec![0; delays.len()],
            buf: vec![fill; total],
            lane: 0,
        })
    }

    /// The transmitter's branches for receiver delays `rx`: branch `j` holds `max(rx) - rx[j]` cells, so every block passes
    /// `max(rx)` cells in all.
    pub fn transmit_delays(rx: &[u32]) -> Vec<u32> {
        let m = rx.iter().copied().max().unwrap_or(0);
        rx.iter().map(|&d| m - d).collect()
    }

    /// Block size `U`.
    pub fn block(&self) -> usize {
        self.u
    }

    /// The branch the next block enters.
    pub fn lane(&self) -> usize {
        self.lane
    }

    /// Makes the next block enter branch 0. The branches keep their contents.
    pub fn sync(&mut self) {
        self.lane = 0;
    }

    /// Puts `input` (`U` items) into the current branch and writes what leaves it into `out`; moves to the next branch. Returns
    /// false while the output is a cell that no input has filled yet.
    pub fn push(&mut self, input: &[T], out: &mut [T]) -> bool {
        let u = self.u;
        let l = self.lane;
        self.lane = if l + 1 == self.delays.len() { 0 } else { l + 1 };
        let d = self.delays[l];
        if d == 0 {
            out[..u].copy_from_slice(&input[..u]);
            return true;
        }
        let at = self.start[l] + self.pos[l] as usize * u;
        out[..u].copy_from_slice(&self.buf[at..at + u]);
        self.buf[at..at + u].copy_from_slice(&input[..u]);
        self.pos[l] = if self.pos[l] + 1 == d {
            0
        } else {
            self.pos[l] + 1
        };
        let valid = self.filled[l] >= d;
        if !valid {
            self.filled[l] += 1;
        }
        valid
    }

    /// Fills every branch with `fill`, marks all cells unfilled and goes back to branch 0.
    pub fn reset(&mut self, fill: T) {
        self.buf.fill(fill);
        self.pos.fill(0);
        self.filled.fill(0);
        self.lane = 0;
    }
}

/// The permutation `b_w = a_(m w mod N)` followed by keeping the first `keep` of every `period` permuted items.
#[derive(Clone, Debug)]
pub struct KeptPermutation {
    src: Vec<u32>,
    keep: usize,
    period: usize,
}

impl KeptPermutation {
    /// The permutation for multiplier `m` over `n` items (`m` prime to `n`), keeping `keep` of every `period` (`keep = period`
    /// keeps all). An error when `m` is not prime to `n` or `keep` is 0 or above `period`.
    pub fn new(m: usize, n: usize, keep: usize, period: usize) -> Result<Self> {
        if n == 0 || gcd(m % n, n) != 1 {
            return Err(FecError::Degenerate {
                param: Param::Permutation,
            });
        }
        if keep == 0 || keep > period {
            return Err(FecError::Conflict {
                param: Param::E,
                with: Param::N,
            });
        }
        let src = (0..n)
            .map(|w| ((m as u64 * w as u64) % n as u64) as u32)
            .collect();
        Ok(KeptPermutation { src, keep, period })
    }

    /// Items before the keep step (`N`).
    pub fn len(&self) -> usize {
        self.src.len()
    }

    /// Whether the permutation is empty (never: `N` > 0).
    pub fn is_empty(&self) -> bool {
        self.src.is_empty()
    }

    /// Items kept.
    pub fn kept(&self) -> usize {
        let n = self.src.len();
        n / self.period * self.keep + (n % self.period).min(self.keep)
    }

    /// Permutes `a` (`N`) and writes the kept items to `out` (`kept()`).
    pub fn apply<T: Copy>(&self, a: &[T], out: &mut [T]) -> Result<()> {
        if a.len() != self.src.len() || out.len() != self.kept() {
            return Err(FecError::InputLength {
                expected: self.src.len(),
                got: a.len(),
            });
        }
        let mut o = 0;
        for (w, &s) in self.src.iter().enumerate() {
            if w % self.period < self.keep {
                out[o] = a[s as usize];
                o += 1;
            }
        }
        Ok(())
    }

    /// Puts the kept items `x` (`kept()`) back in code order into `out` (`N`); dropped places get `erased`.
    pub fn invert<T: Copy>(&self, x: &[T], erased: T, out: &mut [T]) -> Result<()> {
        if x.len() != self.kept() || out.len() != self.src.len() {
            return Err(FecError::InputLength {
                expected: self.kept(),
                got: x.len(),
            });
        }
        let mut o = 0;
        for (w, &s) in self.src.iter().enumerate() {
            out[s as usize] = if w % self.period < self.keep {
                o += 1;
                x[o - 1]
            } else {
                erased
            };
        }
        Ok(())
    }
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}
