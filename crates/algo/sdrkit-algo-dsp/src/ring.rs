//! Sample memories: a ring of the newest samples addressed by their absolute index in the stream, and a fixed delay line.

use crate::{DspError, Param, Sample};

/// A ring of the newest `capacity` samples of a stream, each addressed by its absolute index (the count of samples before it since
/// the ring was made). A receiver that finds something late can then look back at the samples before it. The ring keeps each sample
/// twice, so any run of held samples is one contiguous slice ([`SampleRing::last`], [`SampleRing::window`]). Nothing is allocated
/// after [`SampleRing::new`].
///
/// ```
/// use sdrkit_algo_dsp::ring::SampleRing;
/// let mut r = SampleRing::<f32>::new(4).unwrap();
/// r.extend(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
/// assert_eq!((r.start(), r.end()), (2, 6));
/// assert_eq!(r.get(3), Some(4.0));
/// assert_eq!(r.get(1), None); // already overwritten
/// assert_eq!(r.last(3), &[4.0, 5.0, 6.0]);
/// assert_eq!(r.window(2, 2), Some(&[3.0, 4.0][..]));
/// let mut out = [0.0; 3];
/// r.copy(4, &mut out); // indices 4, 5 and 6 (not yet there: 0)
/// assert_eq!(out, [5.0, 6.0, 0.0]);
/// ```
#[derive(Clone, Debug)]
pub struct SampleRing<T: Sample> {
    /// Two copies of the ring, one after the other.
    buf: Vec<T>,
    cap: usize,
    /// The place of the next sample in the first copy.
    pos: usize,
    end: u64,
}

impl<T: Sample> SampleRing<T> {
    /// A ring of `capacity` samples. Errors for a capacity of 0.
    pub fn new(capacity: usize) -> Result<Self, DspError> {
        if capacity == 0 {
            return Param::Capacity.at_least(1.0, 0.0);
        }
        Ok(SampleRing {
            buf: vec![T::default(); 2 * capacity],
            cap: capacity,
            pos: 0,
            end: 0,
        })
    }

    /// How many samples it holds at most.
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// The absolute index the next sample will get (the count of samples taken).
    pub fn end(&self) -> u64 {
        self.end
    }

    /// The absolute index of the oldest sample still held.
    pub fn start(&self) -> u64 {
        self.end.saturating_sub(self.cap as u64)
    }

    /// Appends one sample.
    #[inline]
    pub fn push(&mut self, x: T) {
        self.buf[self.pos] = x;
        self.buf[self.pos + self.cap] = x;
        self.pos += 1;
        if self.pos == self.cap {
            self.pos = 0;
        }
        self.end += 1;
    }

    /// Appends the samples of `x`, as [`push`](Self::push) of each would: the newest `capacity` of them are copied in one block
    /// from their slot, then the mirror of that block in the other copy in one block, or two when it wraps.
    pub fn extend(&mut self, x: &[T]) {
        let cap = self.cap;
        let keep = &x[x.len().saturating_sub(cap)..];
        let m = keep.len();
        let at = ((x.len() - m) % cap + self.pos) % cap;
        // `at < cap` and `m <= cap`: the block `at..at + m` spans the end of the first copy into the second.
        self.buf[at..at + m].copy_from_slice(keep);
        let first = (cap - at).min(m);
        self.buf[at + cap..at + cap + first].copy_from_slice(&keep[..first]);
        self.buf[..m - first].copy_from_slice(&keep[first..]);
        self.pos = (at + m) % cap;
        self.end += x.len() as u64;
    }

    /// The place in `buf` of the sample `back` places before the newest, in the second copy.
    #[inline]
    fn slot(&self, back: usize) -> usize {
        self.pos + self.cap - 1 - back
    }

    /// The sample at absolute index `i`, or `None` when it is not held (overwritten, or not yet taken).
    #[inline]
    pub fn get(&self, i: u64) -> Option<T> {
        if i >= self.end || i < self.start() {
            None
        } else {
            Some(self.buf[self.slot((self.end - 1 - i) as usize)])
        }
    }

    /// The sample `back` places before the newest (0 is the newest), or `None` when it is not held.
    #[inline]
    pub fn back(&self, back: usize) -> Option<T> {
        self.end
            .checked_sub(1 + back as u64)
            .and_then(|i| self.get(i))
    }

    /// The newest `n` samples, oldest first, as one slice. If `n` is more than the capacity, it returns the capacity. Places that no
    /// sample has filled yet read as zero.
    #[inline]
    pub fn last(&self, n: usize) -> &[T] {
        let n = n.min(self.cap);
        let end = self.pos + self.cap;
        &self.buf[end - n..end]
    }

    /// The `n` samples from absolute index `from`, as one slice, or `None` when one of them is not held.
    #[inline]
    pub fn window(&self, from: u64, n: usize) -> Option<&[T]> {
        let to = from.checked_add(n as u64)?;
        if from < self.start() || to > self.end {
            return None;
        }
        let end = self.pos + self.cap - (self.end - to) as usize;
        Some(&self.buf[end - n..end])
    }

    /// Copies the samples from absolute index `from` into `out` (as many as `out` holds). A sample that is not held reads as zero.
    pub fn copy(&self, from: u64, out: &mut [T]) {
        for (k, o) in out.iter_mut().enumerate() {
            *o = self.get(from + k as u64).unwrap_or_default();
        }
    }

    /// Sets every held sample to zero. The index carries on.
    pub fn clear(&mut self) {
        self.buf.fill(T::default());
    }

    /// Sets every held sample to zero and starts the index again at 0.
    pub fn reset(&mut self) {
        self.clear();
        self.pos = 0;
        self.end = 0;
    }
}

/// A delay of a whole number of samples: each sample comes out `delay` samples after it went in, zeros first. A delay of 0 passes
/// the input through. Nothing is allocated after [`DelayLine::new`].
///
/// ```
/// use sdrkit_algo_dsp::ring::DelayLine;
/// let mut d = DelayLine::<f32>::new(2);
/// let out: Vec<f32> = [1.0, 2.0, 3.0, 4.0].iter().map(|&x| d.push(x)).collect();
/// assert_eq!(out, [0.0, 0.0, 1.0, 2.0]);
/// ```
#[derive(Clone, Debug)]
pub struct DelayLine<T: Sample> {
    buf: Vec<T>,
    pos: usize,
}

impl<T: Sample> DelayLine<T> {
    /// A delay of `delay` samples.
    pub fn new(delay: usize) -> Self {
        DelayLine {
            buf: vec![T::default(); delay],
            pos: 0,
        }
    }

    /// The delay in samples.
    pub fn delay(&self) -> usize {
        self.buf.len()
    }

    /// Takes one sample and returns the sample taken `delay` samples before.
    #[inline]
    pub fn push(&mut self, x: T) -> T {
        if self.buf.is_empty() {
            return x;
        }
        let out = std::mem::replace(&mut self.buf[self.pos], x);
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
        out
    }

    /// Delays `buf` in place.
    pub fn process(&mut self, buf: &mut [T]) {
        for v in buf {
            *v = self.push(*v);
        }
    }

    /// Back to zeros.
    pub fn reset(&mut self) {
        self.buf.fill(T::default());
        self.pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Complex32;

    #[test]
    fn the_ring_keeps_absolute_indices() {
        let mut r = SampleRing::<Complex32>::new(5).unwrap();
        assert_eq!(r.get(0), None);
        for i in 0..12 {
            r.push(Complex32::new(i as f32, -(i as f32)));
        }
        assert_eq!((r.start(), r.end(), r.capacity()), (7, 12, 5));
        for i in 7..12u64 {
            assert_eq!(r.get(i), Some(Complex32::new(i as f32, -(i as f32))));
        }
        assert_eq!(r.get(6), None);
        assert_eq!(r.get(12), None);
        let mut out = [Complex32::new(9.0, 9.0); 4];
        r.copy(5, &mut out);
        assert_eq!(out[0], Complex32::default());
        assert_eq!(out[2], Complex32::new(7.0, -7.0));
        r.clear();
        assert_eq!(r.get(10), Some(Complex32::default()));
        assert_eq!(r.end(), 12);
        assert!(SampleRing::<f32>::new(0).is_err());
    }

    #[test]
    fn extend_leaves_the_ring_as_a_push_of_each_sample_does() {
        for cap in [1usize, 2, 5, 16] {
            for lens in [
                vec![0, 3, 0],
                vec![cap - 1, 1],
                vec![cap, cap],
                vec![cap + 3, 2, 2 * cap + 1],
                vec![1, 1, 1, 1, 1, 1, 1],
                vec![3, cap, 4, 7],
            ] {
                let (mut a, mut b) = (
                    SampleRing::<f32>::new(cap).unwrap(),
                    SampleRing::<f32>::new(cap).unwrap(),
                );
                let mut v = 0.0;
                for n in lens {
                    let x: Vec<f32> = (0..n).map(|k| v + k as f32).collect();
                    v += n as f32;
                    a.extend(&x);
                    x.iter().for_each(|&s| b.push(s));
                    assert_eq!(
                        (a.buf.as_slice(), a.pos, a.end),
                        (b.buf.as_slice(), b.pos, b.end)
                    );
                }
            }
        }
    }

    #[test]
    fn windows_are_contiguous_across_the_wrap() {
        let mut r = SampleRing::<f32>::new(4).unwrap();
        assert_eq!(r.last(2), &[0.0, 0.0]);
        assert_eq!(r.back(0), None);
        for i in 0..7 {
            r.push(i as f32);
        }
        // Held: indices 3 to 6. The ring wrapped, so these cross the end of the first copy.
        assert_eq!(r.last(4), &[3.0, 4.0, 5.0, 6.0]);
        assert_eq!(r.last(9), &[3.0, 4.0, 5.0, 6.0]);
        assert_eq!(r.window(4, 2), Some(&[4.0, 5.0][..]));
        assert_eq!(r.window(3, 4), Some(&[3.0, 4.0, 5.0, 6.0][..]));
        assert_eq!(r.window(2, 2), None);
        assert_eq!(r.window(6, 2), None);
        assert_eq!(r.window(u64::MAX, 2), None);
        assert_eq!(r.window(7, 0), Some(&[][..]));
        assert_eq!(
            (r.back(0), r.back(3), r.back(4)),
            (Some(6.0), Some(3.0), None)
        );
        r.reset();
        assert_eq!((r.end(), r.last(1)), (0, &[0.0][..]));
    }

    #[test]
    fn the_delay_line_delays_by_whole_samples() {
        let x: Vec<f32> = (1..=10).map(|v| v as f32).collect();
        for delay in [0usize, 1, 3, 10, 12] {
            let mut d = DelayLine::new(delay);
            let mut y = x.clone();
            // In two pieces: the cut does not matter.
            let (a, b) = y.split_at_mut(4);
            d.process(a);
            d.process(b);
            for (i, v) in y.iter().enumerate() {
                let want = if i >= delay { x[i - delay] } else { 0.0 };
                assert_eq!(*v, want, "delay {delay} at {i}");
            }
            d.reset();
            assert_eq!(d.push(5.0), if delay == 0 { 5.0 } else { 0.0 });
        }
    }
}
