//! A ring of the newest complex samples with absolute indices, for receivers that look back at the samples before a detection.

use sdrkit_core::Complex32;

/// A ring of the newest complex samples with absolute indices.
#[derive(Clone, Debug)]
pub struct History {
    buf: Vec<Complex32>,
    /// The absolute index of the next sample written.
    end: u64,
}

impl History {
    /// A ring of `cap` samples.
    pub fn new(cap: usize) -> Self {
        History {
            buf: vec![Complex32::default(); cap.max(1)],
            end: 0,
        }
    }
    /// The capacity.
    pub fn cap(&self) -> usize {
        self.buf.len()
    }
    /// Appends a sample.
    #[inline]
    pub fn push(&mut self, x: Complex32) {
        let n = self.buf.len();
        self.buf[(self.end % n as u64) as usize] = x;
        self.end += 1;
    }
    /// The index after the newest sample.
    pub fn end(&self) -> u64 {
        self.end
    }
    /// The oldest index still held.
    pub fn start(&self) -> u64 {
        self.end.saturating_sub(self.buf.len() as u64)
    }
    /// The sample at absolute index `i` (zero when not held).
    #[inline]
    pub fn get(&self, i: u64) -> Complex32 {
        if i >= self.end || i < self.start() {
            Complex32::default()
        } else {
            self.buf[(i % self.buf.len() as u64) as usize]
        }
    }
    /// Copies `[from, to)` into `out` (zeros for what is not held).
    pub fn copy(&self, from: u64, to: u64, out: &mut Vec<Complex32>) {
        out.clear();
        out.extend((from..to).map(|i| self.get(i)));
    }
    /// Clears it (the index carries on).
    pub fn clear(&mut self) {
        for v in self.buf.iter_mut() {
            *v = Complex32::default();
        }
    }
}
