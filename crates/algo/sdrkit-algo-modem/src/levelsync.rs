//! Sync search over symbol levels (4FSK and other multi-level FSK): the normalised correlation of a run of symbol values with
//! patterns of levels, both with their mean removed, so a carrier offset (a constant added to every value) does not change it.
//!
//! ```
//! use sdrkit_algo_modem::levelsync::LevelPatterns;
//! let p = LevelPatterns::new(&[vec![3.0, -1.0, 1.0, -3.0], vec![-3.0, -3.0, 3.0, 3.0]]);
//! // The second pattern, scaled and offset.
//! let (i, c) = p.best(&[-0.5, -0.5, 1.5, 1.5]).unwrap();
//! assert_eq!(i, 1);
//! assert!((c - 1.0).abs() < 1e-6);
//! ```

/// Level patterns of one length, held with their mean removed and their norm 1.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelPatterns {
    len: usize,
    patterns: Vec<Vec<f32>>,
}

/// The values of `v` with their mean removed, and their norm.
fn centred(v: &[f32], out: &mut Vec<f32>) -> f32 {
    let m = v.iter().sum::<f32>() / v.len().max(1) as f32;
    out.clear();
    out.extend(v.iter().map(|x| x - m));
    out.iter().map(|x| x * x).sum::<f32>().sqrt()
}

impl LevelPatterns {
    /// Patterns of levels; each must have the length of the first.
    pub fn new(patterns: &[Vec<f32>]) -> LevelPatterns {
        let len = patterns.first().map_or(0, Vec::len);
        let mut tmp = Vec::new();
        let patterns = patterns
            .iter()
            .filter(|p| p.len() == len)
            .map(|p| {
                let n = centred(p, &mut tmp).max(1e-12);
                tmp.iter().map(|x| x / n).collect()
            })
            .collect();
        LevelPatterns { len, patterns }
    }

    /// The length of the patterns.
    pub fn len(&self) -> usize {
        self.len
    }

    /// No pattern is held.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// The pattern that correlates best with `values` (the pattern length) and the correlation, from −1 to 1. `None` for values
    /// of another length or without spread.
    pub fn best(&self, values: &[f32]) -> Option<(usize, f32)> {
        if values.len() != self.len {
            return None;
        }
        let m = values.iter().sum::<f32>() / self.len as f32;
        let norm = values.iter().map(|x| (x - m) * (x - m)).sum::<f32>().sqrt();
        if norm <= 1e-12 {
            return None;
        }
        self.patterns
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let c = p.iter().zip(values).map(|(a, x)| a * (x - m)).sum::<f32>() / norm;
                (i, c)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }
}
