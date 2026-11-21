//! Field reads that give `None` past the end of the data, on the shared bit reader, so a parser of a FIG or a header stops at
//! the first field that does not fit instead of reading zeros.

pub use sdrkit_util_bits::{BitReader, BitWriter};

/// Reads that stop at the end of the data. Each one leaves the reader where it was when the field does not fit.
pub trait Fields<'a> {
    /// The next `n` bits (at most 32), most significant first.
    fn read(&mut self, n: usize) -> Option<u32>;
    /// The next `n` bits (at most 64), most significant first.
    fn read_wide(&mut self, n: usize) -> Option<u64>;
    /// The next bit.
    fn flag(&mut self) -> Option<bool>;
    /// Skips `n` bits.
    fn pass(&mut self, n: usize) -> Option<()>;
    /// The next `n` whole bytes, without copying. The reader must be at a byte boundary.
    fn take_bytes(&mut self, n: usize) -> Option<&'a [u8]>;
}

impl<'a> Fields<'a> for BitReader<'a> {
    fn read(&mut self, n: usize) -> Option<u32> {
        if n > 32 || n > self.left() {
            return None;
        }
        Some(self.u(n as u32) as u32)
    }

    fn read_wide(&mut self, n: usize) -> Option<u64> {
        if n > 64 || n > self.left() {
            return None;
        }
        Some(self.u(n as u32))
    }

    fn flag(&mut self) -> Option<bool> {
        self.read(1).map(|b| b == 1)
    }

    fn pass(&mut self, n: usize) -> Option<()> {
        if n > self.left() {
            return None;
        }
        self.skip(n);
        Some(())
    }

    fn take_bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        if !self.is_aligned() || n * 8 > self.left() {
            return None;
        }
        Some(self.bytes(n))
    }
}

/// Writes fields with the shared writer into a new byte string: `f` gets the writer, and the bytes come back padded with zero
/// bits to a whole byte.
pub fn written(f: impl FnOnce(&mut BitWriter<'_>)) -> Vec<u8> {
    let mut v = Vec::new();
    let mut w = BitWriter::new(&mut v);
    f(&mut w);
    w.align();
    v
}
