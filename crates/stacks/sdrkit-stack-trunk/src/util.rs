//! A ring of `f64` values with absolute indices (the shared `SampleRing` holds `f32` and complex samples only).

/// A ring of the last `cap` values with absolute indices, for reading back at any recent position.
#[derive(Clone, Debug)]
pub struct Ring<T: Copy + Default> {
    buf: Vec<T>,
    mask: usize,
    /// The index the next pushed value gets.
    pub next: u64,
}

impl<T: Copy + Default> Ring<T> {
    /// A ring of at least `cap` values (rounded up to a power of two).
    pub fn new(cap: usize) -> Ring<T> {
        let n = cap.next_power_of_two().max(2);
        Ring {
            buf: vec![T::default(); n],
            mask: n - 1,
            next: 0,
        }
    }
    /// Appends a value.
    #[inline]
    pub fn push(&mut self, v: T) {
        self.buf[(self.next as usize) & self.mask] = v;
        self.next += 1;
    }
    /// The value at absolute index `i` (must be among the last `cap`).
    #[inline]
    pub fn at(&self, i: u64) -> T {
        self.buf[(i as usize) & self.mask]
    }
    /// Whether index `i` is still held.
    pub fn holds(&self, i: u64) -> bool {
        i < self.next && i + self.buf.len() as u64 >= self.next
    }
    /// Capacity.
    pub fn cap(&self) -> usize {
        self.buf.len()
    }
    /// Forgets everything.
    pub fn reset(&mut self) {
        self.buf.iter_mut().for_each(|v| *v = T::default());
        self.next = 0;
    }
}
