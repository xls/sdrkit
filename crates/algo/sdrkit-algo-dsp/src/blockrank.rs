//! The k-th largest and the k-th smallest value of each block of a fixed length: a peak reader that a click of fewer than
//! `k` samples does not move. A modulation monitor takes the third largest excursion of each 5 ms block as the block's peak
//! (a noise click lasts one or two samples, a real peak longer).

/// What one block gave.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockValues {
    /// The k-th largest value.
    pub high: f64,
    /// The k-th smallest value.
    pub low: f64,
    /// The number of values in the block.
    pub len: usize,
}

/// Reads the k-th largest and the k-th smallest value of consecutive blocks.
#[derive(Clone, Debug)]
pub struct BlockRank {
    len: usize,
    k: usize,
    n: usize,
    top: Vec<f64>,
    bottom: Vec<f64>,
}

impl BlockRank {
    /// Blocks of `len` values (at least `k`), the rank `k` (at least 1).
    pub fn new(len: usize, k: usize) -> Self {
        let k = k.max(1);
        BlockRank {
            len: len.max(k),
            k,
            n: 0,
            top: Vec::with_capacity(k + 1),
            bottom: Vec::with_capacity(k + 1),
        }
    }

    /// The block length.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Always false.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Takes one value. At the end of a block, returns what the block gave.
    pub fn push(&mut self, x: f64) -> Option<BlockValues> {
        insert(&mut self.top, x, self.k, |a, b| a > b);
        insert(&mut self.bottom, x, self.k, |a, b| a < b);
        self.n += 1;
        if self.n < self.len {
            return None;
        }
        let v = BlockValues {
            high: self.top[self.k - 1],
            low: self.bottom[self.k - 1],
            len: self.n,
        };
        self.reset();
        Some(v)
    }

    /// Forgets the partial block.
    pub fn reset(&mut self) {
        self.n = 0;
        self.top.clear();
        self.bottom.clear();
    }
}

/// Keeps `list` the `k` first values in the order `before`.
fn insert(list: &mut Vec<f64>, x: f64, k: usize, before: impl Fn(f64, f64) -> bool) {
    let at = list.iter().position(|&v| before(x, v)).unwrap_or(list.len());
    if at < k {
        list.insert(at, x);
        list.truncate(k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_clicks_do_not_move_the_third_value() {
        let mut r = BlockRank::new(10, 3);
        let mut x = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 100.0, -50.0];
        x.swap(0, 9);
        let mut out = None;
        for v in x {
            out = r.push(v).or(out);
        }
        let b = out.unwrap();
        assert_eq!(b, BlockValues { high: 7.0, low: 2.0, len: 10 });
        // The next block starts empty.
        for v in 0..9 {
            assert!(r.push(f64::from(v)).is_none());
        }
        assert_eq!(r.push(9.0).unwrap().high, 7.0);
    }
}
