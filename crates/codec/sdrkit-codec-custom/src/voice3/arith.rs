//! The binary arithmetic coder of V3 and V1: 32-bit registers, 12-bit probabilities, and the adaptive bit model.

const TOP: u64 = 0xFFFF_FFFF;
const HALF: u64 = 1 << 31;
const Q1: u64 = 1 << 30;
const Q3: u64 = 3 << 30;

/// An adaptive model: the probability of a 0 bit in 1/4096 and a count.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Model {
    p: u32,
    n: u32,
}

impl Model {
    /// A model that starts at `p` with count `n`.
    pub(crate) const fn new(p: u32, n: u32) -> Self {
        Self { p, n }
    }

    /// The V3 start rule: a trained value that is not 0 starts at count 12, else the default at count 0.
    pub(crate) fn trained(trained: u16, default: u32) -> Self {
        if trained != 0 {
            Self::new(trained as u32, 12)
        } else {
            Self::new(default, 0)
        }
    }

    fn update(&mut self, bit: bool) {
        let sh = if self.n < 4 {
            2
        } else if self.n < 12 {
            3
        } else {
            4
        };
        self.n += 1;
        if bit {
            self.p -= self.p >> sh;
        } else {
            self.p += (4096 - self.p) >> sh;
        }
        self.p = self.p.clamp(6, 4090);
    }
}

/// The encoder. It writes one bit per byte into a caller-owned buffer, at most 640 bits.
pub(crate) struct ArithEncoder<'a> {
    low: u64,
    high: u64,
    pending: u32,
    count: usize,
    out: &'a mut Vec<u8>,
}

/// The encoder keeps at most this many bits. It counts the rest.
const KEEP: usize = 640;

impl<'a> ArithEncoder<'a> {
    /// Starts a coder that appends to `out` (cleared first).
    pub(crate) fn new(out: &'a mut Vec<u8>) -> Self {
        out.clear();
        Self {
            low: 0,
            high: TOP,
            pending: 0,
            count: 0,
            out,
        }
    }

    fn push(&mut self, b: u8) {
        if self.count < KEEP {
            self.out.push(b);
        }
        self.count += 1;
    }

    fn emit(&mut self, b: u8) {
        self.push(b);
        for _ in 0..self.pending {
            self.push(1 - b);
        }
        self.pending = 0;
    }

    fn code(&mut self, bit: bool, p0: u32) {
        let range = self.high - self.low + 1;
        let split = self.low + (range * p0 as u64) / 4096 - 1;
        if bit {
            self.low = split + 1;
        } else {
            self.high = split;
        }
        loop {
            if self.high < HALF {
                self.emit(0);
            } else if self.low >= HALF {
                self.emit(1);
                self.low -= HALF;
                self.high -= HALF;
            } else if self.low >= Q1 && self.high < Q3 {
                self.pending += 1;
                self.low -= Q1;
                self.high -= Q1;
            } else {
                break;
            }
            self.low = (2 * self.low) & TOP;
            self.high = ((2 * self.high) & TOP) | 1;
        }
    }

    /// Codes a bit with an adaptive model.
    pub(crate) fn bit(&mut self, bit: bool, m: &mut Model) {
        self.code(bit, m.p);
        m.update(bit);
    }

    /// Codes `n` raw bits of `v`, most significant first.
    pub(crate) fn raw(&mut self, v: u32, n: u32) {
        for i in (0..n).rev() {
            self.code((v >> i) & 1 == 1, 2048);
        }
    }

    /// The number of bits if the coder finishes now.
    pub(crate) fn bits_if_finished(&self) -> usize {
        self.count + self.pending as usize + 2
    }

    /// Finishes the code.
    pub(crate) fn finish(mut self) {
        self.pending += 1;
        if self.low < Q1 {
            self.emit(0);
        } else {
            self.emit(1);
        }
    }
}

/// The decoder over one bit per byte. Bits past the end read as 0.
pub(crate) struct ArithDecoder<'a> {
    low: u64,
    high: u64,
    val: u64,
    bits: &'a [u8],
    pos: usize,
}

impl<'a> ArithDecoder<'a> {
    pub(crate) fn new(bits: &'a [u8]) -> Self {
        let mut d = Self {
            low: 0,
            high: TOP,
            val: 0,
            bits,
            pos: 0,
        };
        for _ in 0..32 {
            d.val = (d.val << 1) | d.next() as u64;
        }
        d
    }

    fn next(&mut self) -> u8 {
        let b = self.bits.get(self.pos).copied().unwrap_or(0) & 1;
        self.pos += 1;
        b
    }

    /// The number of bits read so far, the first 32 included.
    pub(crate) fn bits_read(&self) -> usize {
        self.pos
    }

    fn code(&mut self, p0: u32) -> bool {
        let range = self.high - self.low + 1;
        let split = self.low + (range * p0 as u64) / 4096 - 1;
        let bit = if self.val <= split {
            self.high = split;
            false
        } else {
            self.low = split + 1;
            true
        };
        loop {
            if self.high < HALF {
            } else if self.low >= HALF {
                self.low -= HALF;
                self.high -= HALF;
                self.val -= HALF;
            } else if self.low >= Q1 && self.high < Q3 {
                self.low -= Q1;
                self.high -= Q1;
                self.val -= Q1;
            } else {
                break;
            }
            self.low = (2 * self.low) & TOP;
            self.high = ((2 * self.high) & TOP) | 1;
            let b = self.next() as u64;
            self.val = ((2 * self.val) & TOP) | b;
        }
        bit
    }

    /// Decodes a bit with an adaptive model.
    pub(crate) fn bit(&mut self, m: &mut Model) -> bool {
        let b = self.code(m.p);
        m.update(b);
        b
    }

    /// Decodes `n` raw bits, most significant first.
    pub(crate) fn raw(&mut self, n: u32) -> u32 {
        let mut v = 0;
        for _ in 0..n {
            v = (v << 1) | self.code(2048) as u32;
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coded_bits_decode_back() {
        let mut out = Vec::new();
        let mut m = Model::new(3000, 0);
        let data: Vec<bool> = (0..500u32).map(|i| (i * 7919) % 11 < 3).collect();
        let mut e = ArithEncoder::new(&mut out);
        for &b in &data {
            e.bit(b, &mut m);
        }
        e.raw(0x2A5, 10);
        e.finish();
        let mut m = Model::new(3000, 0);
        let mut d = ArithDecoder::new(&out);
        for &b in &data {
            assert_eq!(d.bit(&mut m), b);
        }
        assert_eq!(d.raw(10), 0x2A5);
    }
}
