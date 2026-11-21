//! Puncturing: a repeating keep pattern over the serial output stream of an encoder (outputs in sending order, `n` per input
//! bit), and the matching streaming depuncturer that puts an erasure (0) back in every removed place.

use crate::error::{FecError, Param, Result};
use sdrkit_util_bits::Soft;

/// A puncturing pattern: `keep[i]` is 1 when the `i`-th value of each period of the serial code stream is sent, 0 when it is
/// removed. The period need not be a multiple of the code's `n` (DAB's vectors span 32 values of a rate 1/4 code).
///
/// ```
/// use sdrkit_algo_fec::conv::{codes, puncture, Depuncturer};
/// // DVB rate 3/4 (EN 300 744 table 2): X1 Y1 Y2 X3 out of X1 Y1 X2 Y2 X3 Y3.
/// let coded = [1u8, 2, 3, 4, 5, 6];
/// let mut sent = Vec::new();
/// puncture(&codes::DVB_3_4, 0, &coded, &mut sent);
/// assert_eq!(sent, [1, 2, 4, 5]);
/// let mut d = Depuncturer::new(codes::DVB_3_4);
/// let mut back = Vec::new();
/// d.push(&[10i8, 20, 40, 50], &mut back);
/// assert_eq!(back, [10, 20, 0, 40, 50, 0]);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Puncture<'a> {
    keep: &'a [u8],
}

impl<'a> Puncture<'a> {
    /// The pattern `keep` (values 0 or 1). For constants: panics (stopping compilation of the constant) when the pattern is empty
    /// or keeps nothing; use [`Puncture::try_new`] for patterns from outside.
    pub const fn new(keep: &'a [u8]) -> Self {
        match Self::try_new(keep) {
            Ok(p) => p,
            Err(_) => panic!("invalid puncturing pattern"),
        }
    }

    /// The pattern `keep`, or an error when it is empty, holds a value other than 0 or 1, or keeps nothing.
    pub const fn try_new(keep: &'a [u8]) -> Result<Self> {
        if keep.is_empty() {
            return Err(FecError::OutOfRange {
                param: Param::Puncturing,
                min: 1,
                max: i64::MAX,
                got: 0,
            });
        }
        let mut any = false;
        let mut i = 0;
        while i < keep.len() {
            if keep[i] > 1 {
                return Err(FecError::NotAllowed {
                    param: Param::Puncturing,
                    got: keep[i] as i64,
                });
            }
            any |= keep[i] == 1;
            i += 1;
        }
        if !any {
            return Param::Puncturing.degenerate();
        }
        Ok(Puncture { keep })
    }

    /// The pattern.
    pub fn keep(&self) -> &'a [u8] {
        self.keep
    }

    /// Values per period.
    pub fn period(&self) -> usize {
        self.keep.len()
    }

    /// Values kept per period.
    pub fn kept(&self) -> usize {
        self.keep.iter().filter(|&&k| k == 1).count()
    }

    /// How many values of `len` serial code values, starting at `phase`, are sent.
    pub fn punctured_len(&self, phase: usize, len: usize) -> usize {
        let p = self.period();
        let full = len / p * self.kept();
        let rest = (0..len % p)
            .filter(|i| self.keep[(phase + i) % p] == 1)
            .count();
        full + rest
    }
}

/// Appends to `out` the values of `coded` that `p` keeps, the first value at position `phase` of the pattern.
pub fn puncture<T: Copy>(p: &Puncture<'_>, phase: usize, coded: &[T], out: &mut Vec<T>) {
    let keep = p.keep;
    let mut ph = phase % keep.len();
    for &v in coded {
        if keep[ph] == 1 {
            out.push(v);
        }
        ph += 1;
        if ph == keep.len() {
            ph = 0;
        }
    }
}

/// Depunctures one whole frame: fills `out` (the full serial code stream, `out.len()` values) from the sent values `sent`,
/// putting 0 in every removed place, the first value at position `phase` of the pattern. An error unless `sent` holds exactly
/// the number of values the pattern keeps over `out.len()`. The work is one pass over the frame for each kept place of the
/// pattern, with a fixed stride and no test of the pattern for a value.
pub fn depuncture_frame<T: Soft>(
    p: &Puncture<'_>,
    phase: usize,
    sent: &[T],
    out: &mut [T],
) -> Result<()> {
    let need = p.punctured_len(phase % p.period(), out.len());
    if sent.len() != need {
        return Err(FecError::InputLength {
            expected: need,
            got: sent.len(),
        });
    }
    let keep = p.keep;
    let period = keep.len();
    let kept = p.kept();
    if kept == period {
        out.copy_from_slice(sent);
        return Ok(());
    }
    out.fill(T::ZERO);
    let mut ph = phase % period;
    // The place in `sent` of the value for place `j` of the first period.
    let mut first = 0;
    for j in 0..period.min(out.len()) {
        if keep[ph] == 1 {
            // Place `j` of every period takes every `kept`-th sent value from `first`.
            let to = out[j..].iter_mut().step_by(period);
            for (o, v) in to.zip(sent[first..].iter().step_by(kept)) {
                *o = *v;
            }
            first += 1;
        }
        ph += 1;
        if ph == period {
            ph = 0;
        }
    }
    Ok(())
}

/// The most kept places of a period that [`Depuncturer::push`] writes period by period.
const WHOLE_KEPT: usize = 16;

/// A streaming depuncturer: each received value goes to the next kept place, and the removed places after it are filled with 0 at
/// once, so the output does not depend on how the input is split.
#[derive(Clone, Copy, Debug)]
pub struct Depuncturer<'a> {
    keep: &'a [u8],
    phase: usize,
}

impl<'a> Depuncturer<'a> {
    /// A depuncturer at position 0 of the pattern.
    pub fn new(p: Puncture<'a>) -> Self {
        Depuncturer {
            keep: p.keep,
            phase: 0,
        }
    }

    /// A depuncturer at position `phase` of the pattern (for puncturing-phase searches).
    pub fn with_phase(p: Puncture<'a>, phase: usize) -> Self {
        Depuncturer {
            keep: p.keep,
            phase: phase % p.keep.len(),
        }
    }

    /// The position in the pattern of the next output value.
    pub fn phase(&self) -> usize {
        self.phase
    }

    /// Goes back to position `phase`.
    pub fn reset(&mut self, phase: usize) {
        self.phase = phase % self.keep.len();
    }

    #[inline]
    fn fill_removed<T: Soft>(&mut self, out: &mut Vec<T>) {
        while self.keep[self.phase] == 0 {
            out.push(T::ZERO);
            self.advance();
        }
    }

    #[inline]
    fn advance(&mut self) {
        self.phase += 1;
        if self.phase == self.keep.len() {
            self.phase = 0;
        }
    }

    /// Appends the depunctured stream for the received values `sent` to `out`.
    pub fn push<T: Soft>(&mut self, sent: &[T], out: &mut Vec<T>) {
        let Some((&first, mut rest)) = sent.split_first() else {
            return;
        };
        self.push_one(first, out);
        // The next place is now a kept one. Whole periods from here: the places of their kept values, relative to it.
        let period = self.keep.len();
        let mut at = [0u8; WHOLE_KEPT];
        let mut kept = 0;
        for i in 0..period {
            if self.keep[(self.phase + i) % period] != 0 {
                if kept == WHOLE_KEPT || i > u8::MAX as usize {
                    kept = 0;
                    break;
                }
                at[kept] = i as u8;
                kept += 1;
            }
        }
        if let Some(whole) = rest.len().checked_div(kept) {
            let from = out.len();
            out.resize(from + whole * period, T::ZERO);
            let (head, tail) = rest.split_at(whole * kept);
            for (o, v) in out[from..]
                .chunks_exact_mut(period)
                .zip(head.chunks_exact(kept))
            {
                for (&i, &x) in at[..kept].iter().zip(v) {
                    o[i as usize] = x;
                }
            }
            rest = tail;
        }
        for &v in rest {
            self.push_one(v, out);
        }
    }

    #[inline]
    fn push_one<T: Soft>(&mut self, v: T, out: &mut Vec<T>) {
        self.fill_removed(out);
        out.push(v);
        self.advance();
        self.fill_removed(out);
    }

    /// How many output values [`Depuncturer::push`] writes for `count` received values from the current position.
    pub fn output_len(&self, count: usize) -> usize {
        let mut d = *self;
        let mut n = 0;
        for _ in 0..count {
            while d.keep[d.phase] == 0 {
                n += 1;
                d.advance();
            }
            n += 1;
            d.advance();
            while d.keep[d.phase] == 0 {
                n += 1;
                d.advance();
            }
        }
        n
    }

    /// Like [`Depuncturer::push`], into a slice: returns the number of values written, or an error (and no change) when `out` is
    /// shorter than [`Depuncturer::output_len`].
    pub fn push_into<T: Soft>(&mut self, sent: &[T], out: &mut [T]) -> Result<usize> {
        let need = self.output_len(sent.len());
        if out.len() < need {
            return Err(FecError::OutputTooShort { need });
        }
        let mut w = 0;
        for &v in sent {
            while self.keep[self.phase] == 0 {
                out[w] = T::ZERO;
                w += 1;
                self.advance();
            }
            out[w] = v;
            w += 1;
            self.advance();
            while self.keep[self.phase] == 0 {
                out[w] = T::ZERO;
                w += 1;
                self.advance();
            }
        }
        Ok(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: Puncture<'static> = Puncture::new(&[1, 1, 0, 1, 1, 0]);

    #[test]
    fn any_cut_produces_the_same_stream() {
        let sent: Vec<i8> = (1..=40).collect();
        let mut whole = Vec::new();
        Depuncturer::new(P).push(&sent, &mut whole);
        for cut in 0..sent.len() {
            let mut d = Depuncturer::new(P);
            let mut parts = Vec::new();
            d.push(&sent[..cut], &mut parts);
            d.push(&sent[cut..], &mut parts);
            assert_eq!(parts, whole);
        }
        let mut into = vec![0i8; whole.len()];
        let mut d = Depuncturer::new(P);
        assert_eq!(d.push_into(&sent, &mut into).unwrap(), whole.len());
        assert_eq!(into, whole);
        assert!(Depuncturer::new(P).push_into(&sent, &mut [0i8; 3]).is_err());
    }

    #[test]
    fn whole_periods_are_written_as_value_by_value() {
        use crate::conv::codes::{DVB_2_3, DVB_3_4, DVB_5_6, DVB_7_8, NONE_1_2};
        let sent: Vec<i8> = (1..=100).collect();
        for p in [NONE_1_2, DVB_2_3, DVB_3_4, DVB_5_6, DVB_7_8] {
            for phase in 0..p.period() {
                for len in [0, 1, 2, 7, 100] {
                    let mut want = vec![0i8; 4 * len + 16];
                    let n = Depuncturer::with_phase(p, phase)
                        .push_into(&sent[..len], &mut want)
                        .unwrap();
                    let mut got = Vec::new();
                    let mut d = Depuncturer::with_phase(p, phase);
                    d.push(&sent[..len], &mut got);
                    assert_eq!(got, want[..n], "phase {phase}, {len} values");
                    let mut by_one = Depuncturer::with_phase(p, phase);
                    by_one.push_into(&sent[..len], &mut want).unwrap();
                    assert_eq!(d.phase(), by_one.phase());
                }
            }
        }
    }

    #[test]
    fn frame_round_trip() {
        let coded: Vec<i8> = (1..=30).collect();
        let mut sent = Vec::new();
        puncture(&P, 2, &coded, &mut sent);
        assert_eq!(sent.len(), P.punctured_len(2, 30));
        let mut back = vec![0i8; 30];
        depuncture_frame(&P, 2, &sent, &mut back).unwrap();
        for (i, (&a, &b)) in coded.iter().zip(&back).enumerate() {
            if P.keep()[(i + 2) % 6] == 1 {
                assert_eq!(a, b);
            } else {
                assert_eq!(b, 0);
            }
        }
        assert!(depuncture_frame(&P, 2, &sent[1..], &mut back).is_err());
        // Any pattern, position and length against the stream form.
        let pats: [&[u8]; 5] = [
            &[1, 1],
            &[1, 1, 1, 0],
            &[1, 1, 0, 1, 0, 1, 0],
            &[0, 0, 1],
            &[1],
        ];
        for keep in pats {
            let p = Puncture::new(keep);
            for phase in 0..2 * keep.len() {
                for len in 0..40 {
                    let n = p.punctured_len(phase % keep.len(), len);
                    let sent: Vec<i8> = (1..=n as i8).collect();
                    let mut got = vec![99i8; len];
                    depuncture_frame(&p, phase, &sent, &mut got).unwrap();
                    let mut want = Vec::new();
                    let mut src = sent.iter();
                    for i in 0..len {
                        let k = keep[(phase + i) % keep.len()] == 1;
                        want.push(if k { *src.next().unwrap() } else { 0 });
                    }
                    assert_eq!(got, want, "{keep:?} {phase} {len}");
                }
            }
        }
        assert!(Puncture::try_new(&[0, 0]).is_err());
        assert!(Puncture::try_new(&[]).is_err());
        assert!(Puncture::try_new(&[2]).is_err());
    }
}
