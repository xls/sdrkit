//! A finder of byte-aligned CADUs in a byte stream: the attached sync marker 1ACFFC1D at a fixed spacing, with no
//! randomiser and no channel code between the markers. Links that carry CADUs in the data field of a DVB-S2 generic stream
//! (or in transport packets) use it.
//!
//! At each position p of the joined stream: when the bytes at p are the marker and the bytes at p + L start the next marker
//! (1A CF), the L bytes from p are one CADU and p moves on by L; otherwise p moves on by one byte. A CADU is held until the
//! start of the marker after it is seen. [`CaduFinder::reset`] empties the stream after a loss.
//!
//! Standard: CCSDS 131.0-B-5 clause 9 (the attached sync marker).

use crate::marker::TM;

/// Counters of a [`CaduFinder`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaduFinderStats {
    /// CADUs given out.
    pub cadus: u64,
    /// Bytes stepped over between CADUs.
    pub skipped: u64,
    /// Resets (losses before the stream).
    pub resets: u64,
}

/// The finder. See the [module documentation](self).
#[derive(Clone, Debug)]
pub struct CaduFinder {
    len: usize,
    buf: Vec<u8>,
    /// The counters.
    pub stats: CaduFinderStats,
}

impl CaduFinder {
    /// A finder of CADUs of `len` bytes, marker included. A `len` below 6 is taken as 6.
    pub fn new(len: usize) -> Self {
        let len = len.max(6);
        CaduFinder {
            len,
            buf: Vec::with_capacity(4 * len),
            stats: CaduFinderStats::default(),
        }
    }

    /// The CADU length, marker included.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Always false: a finder always has a CADU length.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Forgets the bytes held: the next CADU starts in the next bytes pushed.
    pub fn reset(&mut self) {
        if !self.buf.is_empty() {
            self.stats.resets += 1;
        }
        self.buf.clear();
    }

    /// Appends `bytes` to the stream and calls `out` with each CADU (marker first) that the stream now completes.
    pub fn push(&mut self, bytes: &[u8], mut out: impl FnMut(&[u8])) {
        self.buf.extend_from_slice(bytes);
        let m = TM.bytes();
        let l = self.len;
        let mut p = 0;
        while p + l + 2 <= self.buf.len() {
            if self.buf[p..p + 4] == *m && self.buf[p + l..p + l + 2] == m[..2] {
                out(&self.buf[p..p + l]);
                self.stats.cadus += 1;
                p += l;
            } else {
                p += 1;
                self.stats.skipped += 1;
            }
        }
        self.buf.drain(..p);
    }
}
