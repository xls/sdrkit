//! Linear feedback shift registers and scramblers.
//!
//! # Polynomial notation
//!
//! A polynomial here describes the **recurrence**: `x^9 + x^5 + 1` (`0x221`) means each new bit is `s[t] = s[t-9] XOR s[t-5]`, the
//! exponents being delays. This is how DVB, DAB, 802.11 and most register figures read. Some documents label the same register by
//! the reciprocal polynomial (CCSDS writes `x^8 + x^7 + x^5 + x^3 + 1` for what is `0x12B` here); [`reciprocal`] converts.
//!
//! # Register layout
//!
//! For a Fibonacci register, state bit `k - 1` holds `s[t-k]`: bit 0 is the newest bit (stage 1), bit `n - 1` the oldest
//! (stage `n`). Seeds are written that way ("stage 1 at bit 0"). The output of a step is, by default, the new bit (the feedback);
//! [`Lfsr::with_output`] picks other stages, for example the oldest one for sequences that start with their seed (CCSDS, PN9).
//!
//! A Galois register shifts left: the bit leaving stage `n` (bit `n - 1`) is the output, and when it is 1 the low terms of the
//! polynomial are XORed into the state.
//!
//! ```
//! use sdrkit_util_bits::lfsr::Lfsr;
//! // The 802.11 scrambler x^7 + x^4 + 1 from all ones (IEEE 802.11-2020 clause 17.3.5.5) starts 0000 1110 1111 0010.
//! let mut s = Lfsr::fibonacci(0x91, 0x7F);
//! let mut first = [0u8; 16];
//! s.fill_bits(&mut first);
//! assert_eq!(first, [0, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 0, 1, 0]);
//! ```

use crate::pack::{gather8, mask, reverse_bits, BitOrder};
use crate::Param;
use crate::{Error, Soft};

/// The reciprocal of a polynomial of degree `n` (its coefficients in reverse order): the same register read from the other end.
pub const fn reciprocal(poly: u64, n: u32) -> u64 {
    reverse_bits(poly, n + 1)
}

/// The degree of a polynomial (the position of its top bit); 0 for 0 or 1.
pub const fn degree(poly: u64) -> u32 {
    if poly <= 1 {
        0
    } else {
        63 - poly.leading_zeros()
    }
}

/// The two register forms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// The new bit is the XOR of tapped stages, shifted in at stage 1.
    Fibonacci,
    /// The bit leaving the last stage is XORed into the tapped stages.
    Galois,
}

/// A linear feedback shift register of up to 64 stages. It is `Copy`: keep a copy as the seed and copy it back to restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lfsr {
    state: u64,
    taps: u64,
    out: u64,
    len: u8,
    form: Form,
}

impl Lfsr {
    /// A Fibonacci register for `poly` (recurrence notation, top term and `+ 1` included, degree 1 to 64) started at `seed` (stage
    /// 1 at bit 0). The output is the new bit. With a polynomial of degree 0 the register outputs zeros.
    pub const fn fibonacci(poly: u64, seed: u64) -> Self {
        let n = degree(poly);
        let taps = (poly >> 1) & mask(n);
        Lfsr {
            state: seed & mask(n),
            taps,
            out: taps,
            len: n as u8,
            form: Form::Fibonacci,
        }
    }

    /// A Fibonacci register of `n` stages for the recurrence `x(i + n) = XOR of x(i + k) for each bit k set in k_mask`, whose first
    /// `n` outputs are `x(0) .. x(n - 1)` given in `first` (bit `i` is `x(i)`). This is the form the 3GPP and GNSS documents use.
    pub const fn recurrence(n: u32, k_mask: u64, first: u64) -> Self {
        // x(i + n) = sum x(i + k): the delay of x(i + k) seen from x(i + n) is n - k.
        let mut poly = 1u64 << n;
        let mut k = 0;
        while k < n {
            if (k_mask >> k) & 1 == 1 {
                poly |= 1u64 << (n - k);
            }
            k += 1;
        }
        // Stage j + 1 (bit j) holds x(n - 1 - j) before the first step, so the oldest stage holds x(0).
        let seed = reverse_bits(first, n);
        Self::fibonacci(poly, seed).with_output(1u64 << (n - 1))
    }

    /// A Galois register for `poly` (top term included) started at `seed`.
    pub const fn galois(poly: u64, seed: u64) -> Self {
        let n = degree(poly);
        Lfsr {
            state: seed & mask(n),
            taps: poly & mask(n),
            out: 1u64 << (n.saturating_sub(1)),
            len: n as u8,
            form: Form::Galois,
        }
    }

    /// The same register with its output taken as the XOR of the stages in `out_mask` (read before each step). For a Fibonacci
    /// register `1 << (n - 1)` is the oldest stage, so the seed comes out first. A Galois register always outputs its last stage;
    /// the mask is ignored there.
    pub const fn with_output(mut self, out_mask: u64) -> Self {
        if let Form::Fibonacci = self.form {
            self.out = out_mask & mask(self.len as u32);
        }
        self
    }

    /// Number of stages.
    pub const fn len(&self) -> u32 {
        self.len as u32
    }

    /// Whether the register has no stages.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The register contents.
    pub const fn state(&self) -> u64 {
        self.state
    }

    /// Loads the register.
    pub fn set_state(&mut self, state: u64) {
        self.state = state & mask(self.len as u32);
    }

    /// One step, as a `const fn`: the register after it and the output bit.
    #[inline]
    pub const fn step(self) -> (Self, u8) {
        let n = self.len as u32;
        let mut next = self;
        match self.form {
            Form::Fibonacci => {
                let out = (self.state & self.out).count_ones() as u8 & 1;
                let fb = (self.state & self.taps).count_ones() as u64 & 1;
                next.state = ((self.state << 1) | fb) & mask(n);
                (next, out)
            }
            Form::Galois => {
                if n == 0 {
                    return (next, 0);
                }
                let out = ((self.state >> (n - 1)) & 1) as u8;
                let mut s = (self.state << 1) & mask(n);
                if out == 1 {
                    s ^= self.taps;
                }
                next.state = s;
                (next, out)
            }
        }
    }

    /// The next bit.
    #[inline]
    pub fn bit(&mut self) -> u8 {
        let (n, b) = self.step();
        *self = n;
        b
    }

    /// The next 8 bits as a byte; the first bit goes to the most significant bit for [`BitOrder::Msb`], the least for
    /// [`BitOrder::Lsb`]. A Fibonacci register that outputs its feedback makes as many bits per step as its shortest delay
    /// allows (8 at once for the DVB register), instead of one.
    #[inline]
    pub fn byte(&mut self, order: BitOrder) -> u8 {
        let w = self.parallel_width();
        let v = if w > 1 {
            let mut v = 0u64;
            let mut got = 0;
            while got < 8 {
                let k = w.min(8 - got);
                v = (v << k) | self.fib_chunk(k);
                got += k;
            }
            v as u8
        } else {
            let mut a = [0u8; 8];
            for x in a.iter_mut() {
                *x = self.bit();
            }
            gather8(a, BitOrder::Msb)
        };
        match order {
            BitOrder::Msb => v,
            BitOrder::Lsb => v.reverse_bits(),
        }
    }

    /// How many outputs a Fibonacci register with feedback output can make at once (its shortest delay, at most 8); 1 otherwise.
    #[inline]
    fn parallel_width(&self) -> u32 {
        match self.form {
            Form::Fibonacci if self.out == self.taps && self.taps != 0 => {
                (self.taps.trailing_zeros() + 1).min(8)
            }
            _ => 1,
        }
    }

    /// The next `w` outputs (first in the top bit of the result) of a Fibonacci register with feedback output, `w` at most its
    /// shortest delay: new bit `k` is the XOR over taps `p` of state bit `p - k`, all of which are already in the register.
    #[inline]
    fn fib_chunk(&mut self, w: u32) -> u64 {
        let mut v = 0u64;
        let mut t = self.taps;
        while t != 0 {
            let p = t.trailing_zeros();
            v ^= self.state >> (p + 1 - w);
            t &= t - 1;
        }
        v &= mask(w);
        self.state = ((self.state << w) | v) & mask(self.len as u32);
        v
    }

    /// Fills `out` with the next bits.
    pub fn fill_bits(&mut self, out: &mut [u8]) {
        for o in out {
            *o = self.bit();
        }
    }

    /// Fills `out` with the next bits, 8 per byte.
    pub fn fill_bytes(&mut self, out: &mut [u8], order: BitOrder) {
        for o in out {
            *o = self.byte(order);
        }
    }

    /// The step as a matrix over GF(2): column `j` is the image of state bit `j`.
    fn matrix(&self) -> [u64; 64] {
        let n = self.len as usize;
        let mut m = [0u64; 64];
        for (j, col) in m.iter_mut().enumerate().take(n) {
            let e = 1u64 << j;
            *col = match self.form {
                Form::Fibonacci => ((e << 1) & mask(n as u32)) | ((self.taps >> j) & 1),
                Form::Galois => {
                    if j + 1 == n {
                        self.taps
                    } else {
                        e << 1
                    }
                }
            };
        }
        m
    }

    /// Moves `n` steps ahead without producing the bits, in O(log n) matrix squarings (the 3GPP Nc = 1600 offset, long jumps in
    /// GNSS codes).
    pub fn skip(&mut self, n: u64) {
        if n < 128 {
            for _ in 0..n {
                self.bit();
            }
            return;
        }
        let apply = |m: &[u64; 64], v: u64| {
            let mut r = 0u64;
            let mut x = v;
            while x != 0 {
                let j = x.trailing_zeros() as usize;
                r ^= m[j];
                x &= x - 1;
            }
            r
        };
        let mul = |a: &[u64; 64], b: &[u64; 64]| {
            let mut c = [0u64; 64];
            for j in 0..64 {
                c[j] = apply(a, b[j]);
            }
            c
        };
        let mut p = self.matrix();
        let mut k = n;
        let mut s = self.state;
        while k != 0 {
            if k & 1 == 1 {
                s = apply(&p, s);
            }
            k >>= 1;
            if k != 0 {
                p = mul(&p, &p);
            }
        }
        self.state = s;
    }

    /// The period of the register from its current state, by stepping (at most `limit` steps); `None` when it is longer than
    /// `limit`. For tests and tools.
    pub fn period(&self, limit: u64) -> Option<u64> {
        let mut r = *self;
        for i in 1..=limit {
            r.bit();
            if r.state == self.state {
                return Some(i);
            }
        }
        None
    }
}

/// An additive (synchronous) scrambler: the data is XORed with the register's sequence, restarted from the seed by
/// [`Additive::reset`]. Scrambling and descrambling are the same operation. It works on hard bits, packed bytes or soft values
/// (it flips the LLR's sign and keeps its size).
///
/// ```
/// use sdrkit_util_bits::lfsr::{Additive, Lfsr};
/// let mut s = Additive::new(Lfsr::fibonacci(0xC001, 0x00A9)); // DVB energy dispersal
/// let mut data = [0u8; 2];
/// s.apply_bytes(&mut data, sdrkit_util_bits::BitOrder::Msb);
/// assert_eq!(data, [0x03, 0xF6]);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Additive {
    seed: Lfsr,
    now: Lfsr,
}

impl Additive {
    /// A scrambler that starts at the register's present state.
    pub const fn new(seed: Lfsr) -> Self {
        Additive { seed, now: seed }
    }

    /// Back to the seed.
    pub fn reset(&mut self) {
        self.now = self.seed;
    }

    /// The register as it is now.
    pub fn register(&self) -> &Lfsr {
        &self.now
    }

    /// XORs hard bits with the sequence.
    pub fn apply_bits(&mut self, bits: &mut [u8]) {
        for b in bits {
            *b = (*b ^ self.now.bit()) & 1;
        }
    }

    /// XORs bytes with the sequence, 8 sequence bits per byte in the given order.
    pub fn apply_bytes(&mut self, bytes: &mut [u8], order: BitOrder) {
        for b in bytes {
            *b ^= self.now.byte(order);
        }
    }

    /// Flips each value where the sequence is 1 (hard bits, `i8` or `f32` LLRs).
    pub fn apply_soft<T: Soft>(&mut self, x: &mut [T]) {
        for v in x {
            *v = v.flip_if(self.now.bit());
        }
    }
}

/// A self-synchronising (multiplicative) scrambler: the register holds the scrambled bits, so a receiver locks after `n` bits
/// without knowing the seed (G3RUH, 802.11b, Eurobalise).
///
/// ```
/// use sdrkit_util_bits::lfsr::Multiplicative;
/// let data = [1u8, 0, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0, 1];
/// let mut tx = Multiplicative::fibonacci(0x21001, 0); // G3RUH x^17 + x^12 + 1
/// let mut rx = Multiplicative::fibonacci(0x21001, 0x1ABCD); // any start state
/// let mut line = data.repeat(4);
/// tx.scramble(&mut line);
/// rx.descramble(&mut line);
/// assert_eq!(&line[17..], &data.repeat(4)[17..]); // right after 17 bits
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Multiplicative {
    reg: u64,
    taps: u64,
    len: u8,
    form: Form,
}

impl Multiplicative {
    /// A Fibonacci self-synchronising scrambler for `poly` (recurrence notation): sent bit `y[t] = x[t] XOR` the taps of the
    /// previous sent bits.
    pub const fn fibonacci(poly: u64, state: u64) -> Self {
        let n = degree(poly);
        Multiplicative {
            reg: state & mask(n),
            taps: (poly >> 1) & mask(n),
            len: n as u8,
            form: Form::Fibonacci,
        }
    }

    /// A Galois self-synchronising scrambler (the form Eurobalise uses): the sent bit is the input XOR the register's top stage,
    /// and the sent bit, when 1, XORs `poly`'s low terms into the register after it shifts left.
    pub const fn galois(poly: u64, state: u64) -> Self {
        let n = degree(poly);
        Multiplicative {
            reg: state & mask(n),
            taps: poly & mask(n),
            len: n as u8,
            form: Form::Galois,
        }
    }

    #[inline]
    fn key(&self) -> u8 {
        match self.form {
            Form::Fibonacci => (self.reg & self.taps).count_ones() as u8 & 1,
            Form::Galois => ((self.reg >> (self.len.max(1) - 1)) & 1) as u8,
        }
    }

    #[inline]
    fn push(&mut self, sent: u8) {
        let n = self.len as u32;
        match self.form {
            Form::Fibonacci => self.reg = ((self.reg << 1) | (sent & 1) as u64) & mask(n),
            Form::Galois => {
                self.reg = (self.reg << 1) & mask(n);
                if sent & 1 == 1 {
                    self.reg ^= self.taps;
                }
            }
        }
    }

    /// Scrambles hard bits in place.
    pub fn scramble(&mut self, bits: &mut [u8]) {
        for b in bits {
            let y = (*b ^ self.key()) & 1;
            self.push(y);
            *b = y;
        }
    }

    /// Descrambles hard bits in place.
    pub fn descramble(&mut self, bits: &mut [u8]) {
        for b in bits {
            let y = *b & 1;
            let x = y ^ self.key();
            self.push(y);
            *b = x;
        }
    }

    /// Descrambles soft values in place: the register takes the hard decisions, and each value is flipped where the key is 1.
    pub fn descramble_soft<T: Soft>(&mut self, x: &mut [T]) {
        for v in x {
            let k = self.key();
            self.push(v.hard());
            *v = v.flip_if(k);
        }
    }
}

/// A sequence whose period is precomputed into a table of `N` bytes (8 sequence bits per byte, in the order the table was made),
/// XORed onto data a byte at a time. Build it in a `static` with [`Period::of`]; the table is made at compile time.
///
/// ```
/// use sdrkit_util_bits::lfsr::{Lfsr, Period};
/// use sdrkit_util_bits::BitOrder;
/// // The CCSDS pseudo-randomiser (CCSDS 131.0-B annex): FF 48 0E C0 9A ...
/// static CCSDS: Period<255> = Period::of(Lfsr::fibonacci(0x12B, 0xFF).with_output(0x80), BitOrder::Msb);
/// assert_eq!(&CCSDS.bytes()[..5], &[0xFF, 0x48, 0x0E, 0xC0, 0x9A]);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Period<const N: usize> {
    table: [u8; N],
}

impl<const N: usize> Period<N> {
    /// The first `8 N` bits of `lfsr`, packed in `order`.
    pub const fn of(lfsr: Lfsr, order: BitOrder) -> Self {
        let mut table = [0u8; N];
        let mut r = lfsr;
        let mut i = 0;
        while i < N {
            let mut b = 0u8;
            let mut k = 0;
            while k < 8 {
                let (n, bit) = r.step();
                r = n;
                b |= match order {
                    BitOrder::Msb => bit << (7 - k),
                    BitOrder::Lsb => bit << k,
                };
                k += 1;
            }
            table[i] = b;
            i += 1;
        }
        Period { table }
    }

    /// A table given directly (sequences that are published as tables: M17, RS41, FT4).
    pub const fn from_table(table: [u8; N]) -> Self {
        Period { table }
    }

    /// The table.
    pub const fn bytes(&self) -> &[u8; N] {
        &self.table
    }

    /// XORs `data` with the sequence, starting at its byte `offset` and wrapping round after `N` bytes.
    pub fn apply(&self, data: &mut [u8], offset: usize) {
        if N == 0 {
            return;
        }
        let mut k = offset % N;
        for d in data {
            *d ^= self.table[k];
            k += 1;
            if k == N {
                k = 0;
            }
        }
    }
}

/// Fills `out` with the m-sequence of `lfsr` and checks that the register is maximal length (period `2^n - 1`), for building
/// tables at configure time. An error when it is not.
pub fn maximal(lfsr: Lfsr, out: &mut [u8]) -> Result<(), Error> {
    let n = lfsr.len();
    if n == 0 || n > 32 {
        return Err(Error::BadParameter(Param::RegisterLength));
    }
    let full = (1u64 << n) - 1;
    if lfsr.period(full) != Some(full) {
        return Err(Error::BadParameter(Param::Polynomial));
    }
    let mut r = lfsr;
    r.fill_bits(out);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notation_helpers() {
        assert_eq!(degree(0x221), 9);
        assert_eq!(degree(1), 0);
        // CCSDS writes x^8 + x^7 + x^5 + x^3 + 1 (0x1A9); in recurrence notation it is 0x12B.
        assert_eq!(reciprocal(0x1A9, 8), 0x12B);
    }

    #[test]
    fn recurrence_form_equals_the_definition() {
        // x(i + 5) = x(i + 2) + x(i), x(0..4) = 0 0 0 0 1 (3GPP TS 36.211 6.11.2.1).
        let mut l = Lfsr::recurrence(5, 0b101, 0b10000);
        let mut want = vec![0u8, 0, 0, 0, 1];
        for i in 0..40 {
            let v = want[i + 2] ^ want[i];
            want.push(v);
        }
        let mut got = vec![0u8; 45];
        l.fill_bits(&mut got);
        assert_eq!(got, want);
    }

    #[test]
    fn parallel_bytes_equal_single_steps() {
        for l in [
            Lfsr::fibonacci(0xC001, 0xA9),
            Lfsr::fibonacci(0x221, 0x1FF),
            Lfsr::fibonacci(0x91, 0x5D),
            Lfsr::fibonacci(0x8003, 0x4D4B),
            Lfsr::fibonacci(0x1_04C1_1DB7, 0xC000_1234),
        ] {
            for order in [BitOrder::Msb, BitOrder::Lsb] {
                let mut a = l;
                let mut b = l;
                for _ in 0..300 {
                    let mut bits = [0u8; 8];
                    b.fill_bits(&mut bits);
                    assert_eq!(a.byte(order), gather8(bits, order));
                }
                assert_eq!(a, b);
            }
        }
    }

    #[test]
    fn skip_equals_stepping() {
        for l in [
            Lfsr::fibonacci(0x8000_0009, 1),
            Lfsr::galois(0x2DB, 2),
            Lfsr::fibonacci(0xC001, 0xA9).with_output(0x4000),
            Lfsr::fibonacci(1 << 63 | 0x1B, 0x1234_5678_9ABC),
        ] {
            for n in [0u64, 1, 127, 128, 1600, 5000] {
                let mut a = l;
                let mut b = l;
                a.skip(n);
                for _ in 0..n {
                    b.bit();
                }
                assert_eq!(a, b, "skip {n}");
            }
        }
    }

    #[test]
    fn periods_of_primitive_polynomials() {
        assert_eq!(Lfsr::fibonacci(0x221, 0x1FF).period(1 << 12), Some(511));
        assert_eq!(Lfsr::fibonacci(0xC001, 1).period(1 << 16), Some(32767));
        assert_eq!(Lfsr::galois(0x2DB, 2).period(1 << 12), Some(511));
        let mut v = [0u8; 10];
        assert!(maximal(Lfsr::fibonacci(0x91, 1), &mut v).is_ok());
        assert!(maximal(Lfsr::fibonacci(0x81, 1), &mut v).is_err()); // x^7 + 1 is not primitive
        assert!(maximal(Lfsr::fibonacci(0x91, 0), &mut v).is_err());
    }

    #[test]
    fn additive_on_bits_bytes_and_soft_agree() {
        let l = Lfsr::fibonacci(0x221, 0x1FF);
        let mut bits = [0u8; 16];
        Additive::new(l).apply_bits(&mut bits);
        let mut bytes = [0u8; 2];
        Additive::new(l).apply_bytes(&mut bytes, BitOrder::Msb);
        assert_eq!(crate::pack::to_u64(&bits), u16::from_be_bytes(bytes) as u64);
        let mut soft = [10i8; 16];
        let mut s = Additive::new(l);
        s.apply_soft(&mut soft);
        for (x, b) in soft.iter().zip(bits) {
            assert_eq!(*x, if b == 1 { -10 } else { 10 });
        }
        s.reset();
        s.apply_soft(&mut soft);
        assert_eq!(soft, [10i8; 16]);
    }

    #[test]
    fn self_synchronising_both_forms() {
        let data: Vec<u8> = (0..300).map(|i| ((i * 7 + i / 5) % 3 == 0) as u8).collect();
        for (mut tx, mut rx) in [
            (
                Multiplicative::fibonacci(0x91, 0x55),
                Multiplicative::fibonacci(0x91, 0x0F),
            ),
            (
                Multiplicative::galois(0x1_EA00_0001, 7),
                Multiplicative::galois(0x1_EA00_0001, 99),
            ),
        ] {
            let mut line = data.clone();
            tx.scramble(&mut line);
            let mut soft: Vec<f32> = line.iter().map(|&b| f32::from_hard(b) * 3.0).collect();
            let mut rx2 = rx;
            rx.descramble(&mut line);
            rx2.descramble_soft(&mut soft);
            assert_eq!(&line[32..], &data[32..]);
            let hard: Vec<u8> = soft.iter().map(|v| v.hard()).collect();
            assert_eq!(&hard[32..], &data[32..]);
        }
    }

    #[test]
    fn period_table_wraps() {
        static P: Period<3> = Period::from_table([1, 2, 3]);
        let mut d = [0u8; 5];
        P.apply(&mut d, 1);
        assert_eq!(d, [2, 3, 1, 2, 3]);
    }
}
