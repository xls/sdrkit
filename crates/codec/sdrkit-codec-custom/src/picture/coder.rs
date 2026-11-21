//! The probability model of the picture codec and the binary range coder with a carry cache and 15-bit probabilities.

/// One context: two estimates of the probability of a 0 bit, in 1/32768.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Prob {
    a: u16,
    b: u16,
}

impl Prob {
    const fn new() -> Self {
        Self { a: 16384, b: 16384 }
    }

    #[inline]
    fn p(&self) -> u32 {
        ((self.a as u32 + self.b as u32) >> 1).clamp(48, 32720)
    }

    #[inline]
    fn update(&mut self, bit: bool) {
        if bit {
            self.a -= self.a >> 4;
            self.b -= self.b >> 7;
        } else {
            self.a += (32768 - self.a) >> 4;
            self.b += (32768 - self.b) >> 7;
        }
    }
}

/// All contexts of one picture.
pub(crate) struct Contexts {
    pub sig: [[[[Prob; 2]; 6]; 2]; 4],
    pub group: [[[Prob; 4]; 2]; 4],
    pub refine: [Prob; 3],
    pub sign: [Prob; 9],
}

impl Contexts {
    pub(crate) fn new() -> Self {
        Self {
            sig: [[[[Prob::new(); 2]; 6]; 2]; 4],
            group: [[[Prob::new(); 4]; 2]; 4],
            refine: [Prob::new(); 3],
            sign: [Prob::new(); 9],
        }
    }
}

/// The encoder. It stops for good at the first decision that does not fit the budget.
pub(crate) struct RangeEncoder<'a> {
    low: u64,
    range: u32,
    cache: u8,
    cache_size: u64,
    out: &'a mut Vec<u8>,
    budget: usize,
    decisions: u32,
    stopped: bool,
}

impl<'a> RangeEncoder<'a> {
    /// Starts an encoder that writes to `out` (cleared first) with a budget in bytes.
    pub(crate) fn new(out: &'a mut Vec<u8>, budget: usize) -> Self {
        out.clear();
        Self {
            low: 0,
            range: 0xFFFF_FFFF,
            cache: 0,
            cache_size: 1,
            out,
            budget,
            decisions: 0,
            stopped: false,
        }
    }

    fn shift(&mut self) {
        if (self.low as u32) < 0xFF00_0000 || self.low >= 1 << 32 {
            let carry = (self.low >> 32) as u8;
            self.out.push(self.cache.wrapping_add(carry));
            for _ in 1..self.cache_size {
                self.out.push(0xFFu8.wrapping_add(carry));
            }
            self.cache_size = 0;
            self.cache = ((self.low >> 24) & 0xFF) as u8;
        }
        self.cache_size += 1;
        self.low = (self.low & 0x00FF_FFFF) << 8;
    }

    /// Codes `bit` with `ctx`. Returns false when the budget stops the coder.
    pub(crate) fn encode(&mut self, ctx: &mut Prob, bit: bool) -> bool {
        if self.stopped || self.out.len() as u64 + self.cache_size + 8 > self.budget as u64 + 1 {
            self.stopped = true;
            return false;
        }
        let bound = (self.range >> 15) * ctx.p();
        if bit {
            self.low += bound as u64;
            self.range -= bound;
        } else {
            self.range = bound;
        }
        ctx.update(bit);
        while self.range < 1 << 24 {
            self.range <<= 8;
            self.shift();
        }
        self.decisions += 1;
        true
    }

    /// Finishes the stream and returns the number of decisions.
    pub(crate) fn finish(mut self) -> u32 {
        for _ in 0..5 {
            self.shift();
        }
        if !self.out.is_empty() {
            self.out.remove(0);
        }
        while self.out.last() == Some(&0) {
            self.out.pop();
        }
        self.out.truncate(self.budget);
        self.decisions
    }
}

/// The decoder. It stops after `limit` decisions or at the first decision that needs a byte it does not have.
pub(crate) struct RangeDecoder<'a> {
    range: u32,
    code: u32,
    data: &'a [u8],
    pos: usize,
    limit: u32,
    decisions: u32,
    stopped: bool,
}

impl<'a> RangeDecoder<'a> {
    pub(crate) fn new(data: &'a [u8], limit: u32) -> Self {
        let mut d = Self {
            range: 0xFFFF_FFFF,
            code: 0,
            data,
            pos: 0,
            limit,
            decisions: 0,
            stopped: false,
        };
        for _ in 0..4 {
            d.code = (d.code << 8) | d.byte() as u32;
        }
        d
    }

    fn byte(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        b
    }

    /// The number of decisions decoded.
    pub(crate) fn decisions(&self) -> u32 {
        self.decisions
    }

    /// Decodes one bit with `ctx`. Returns `None` once the decoder has stopped.
    pub(crate) fn decode(&mut self, ctx: &mut Prob) -> Option<bool> {
        if self.stopped {
            return None;
        }
        if self.decisions >= self.limit {
            self.stopped = true;
            return None;
        }
        while self.range < 1 << 24 {
            self.range <<= 8;
            self.code = (self.code << 8) | self.byte() as u32;
        }
        if self.pos > self.data.len() {
            self.stopped = true;
            return None;
        }
        let bound = (self.range >> 15) * ctx.p();
        let bit = if self.code < bound {
            self.range = bound;
            false
        } else {
            self.code -= bound;
            self.range -= bound;
            true
        };
        ctx.update(bit);
        self.decisions += 1;
        Some(bit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_decode_back() {
        let mut out = Vec::new();
        let bits: Vec<bool> = (0..3000u32)
            .map(|i| i.wrapping_mul(2_654_435_761) >> 29 == 0)
            .collect();
        let mut c = Prob::new();
        let mut e = RangeEncoder::new(&mut out, 1 << 20);
        for &b in &bits {
            assert!(e.encode(&mut c, b));
        }
        let n = e.finish();
        let mut padded = out.clone();
        padded.extend_from_slice(&[0; 4]);
        let mut c = Prob::new();
        let mut d = RangeDecoder::new(&padded, n);
        for &b in &bits {
            assert_eq!(d.decode(&mut c), Some(b));
        }
        assert_eq!(d.decode(&mut c), None);
    }
}
