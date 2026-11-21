//! Hypothesis searches: decode a short window under each hypothesis (puncturing phase, I/Q swap, polarity, pairing), score it by
//! re-encoding, and keep the best with its margin over the runner-up.

use crate::conv::{reencode_errors, Ends, Viterbi};
use crate::error::Result;
use crate::llr::Llr8;

/// The outcome of a search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ranked {
    /// The best hypothesis.
    pub best: usize,
    /// Its score (lower is better).
    pub score: f32,
    /// The runner-up's score minus the best's (infinite with one candidate).
    pub margin: f32,
}

/// Scores `count` hypotheses with `score` (lower is better; `None` to skip one) and returns the best with its margin.
///
/// ```
/// use sdrkit_algo_fec::search::best_of;
/// let scores = [0.3f32, 0.02, 0.25];
/// let r = best_of(3, |i| Some(scores[i])).unwrap();
/// assert_eq!(r.best, 1);
/// assert!((r.margin - 0.23).abs() < 1e-6);
/// ```
pub fn best_of(count: usize, mut score: impl FnMut(usize) -> Option<f32>) -> Option<Ranked> {
    let (mut best, mut b, mut second) = (0usize, f32::INFINITY, f32::INFINITY);
    let mut any = false;
    for i in 0..count {
        let Some(s) = score(i) else { continue };
        if s.is_nan() {
            continue;
        }
        any = true;
        if s < b {
            second = b;
            b = s;
            best = i;
        } else if s < second {
            second = s;
        }
    }
    any.then_some(Ranked {
        best,
        score: b,
        margin: second - b,
    })
}

/// The re-encode error rate of a window of LLRs (`n` per step, punctured places 0) under a Viterbi decoder: the fraction of
/// non-zero values whose hard decision disagrees with the re-encoded best path. For a right hypothesis the result is about the
/// channel bit error rate; for a wrong one it is about 0.3 to 0.5. `out` is scratch of at least the window's steps.
pub fn viterbi_error_rate(v: &mut Viterbi, llr: &[Llr8], out: &mut [u8]) -> Result<f32> {
    let n = v.code().n();
    let steps = llr.len() / n;
    let llr = &llr[..steps * n];
    v.decode(llr, Ends::Open, out)?;
    // Re-encode from the decoded start: drop the first K steps, whose start state is uncertain.
    let skip = (v.code().k() as usize).min(steps);
    let mut state = 0u32;
    for &b in &out[..skip] {
        state = ((b as u32) << (v.code().k() - 2)) | (state >> 1);
    }
    let errs = reencode_errors(v.code(), state, &out[skip..steps], &llr[skip * n..])?;
    let nonzero = llr[skip * n..].iter().filter(|&&x| x != 0).count().max(1);
    Ok(errs as f32 / nonzero as f32)
}
