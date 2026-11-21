//! Convolutional codes: the code description, encoders, puncturing, the Viterbi decoder (whole frame and streaming), trellis-coded
//! modulation (the general [`TrellisDecoder`], and the DMR and P25 4-level trellis codes in [`tcm`]) and sequential (Fano)
//! decoding.
//!
//! # How a code is written
//!
//! A [`ConvCode`] is written exactly as the standards write it: constraint length `K`, then one generator per output **in octal,
//! newest input bit in the most significant place** of the `K`-bit register, **in the order the outputs are sent**. DVB's
//! `X = 171, Y = 133` is `ConvCode::new(7, &[0o171, 0o133])`. Polynomials in D notation (`1 + D^3 + D^4`) have the coefficient of
//! `D^0` (the newest bit) as the most significant bit: `0b10011 = 0o23`. Code that keeps its register with the newest bit at
//! bit 0 (`reg = (reg << 1) | bit`) can pass its masks to [`ConvCode::from_lsb_masks`], which reverses them. Outputs sent
//! inverted (CCSDS, Galileo) are marked with [`ConvCode::inverted`].
//!
//! # State convention
//!
//! The encoder state is the last `K − 1` input bits, the newest in bit `K − 2`. With input `b`, the register is
//! `reg = (b << (K − 1)) | state`, output `i` is the parity of `reg & poly[i]` (inverted when marked), and the next state is
//! `reg >> 1`. The Viterbi decoder and the trellis decoders use the same numbering, so a state can be passed from one to the other.

pub mod codes;
mod fano;
mod puncture;
pub mod tcm;
mod trellis;
mod viterbi;
mod viterbi_simd;

pub use fano::{Fano, FanoParams};
pub use puncture::{depuncture_frame, puncture, Depuncturer, Puncture};
pub use trellis::{BranchMetrics, LabelMetrics, Trellis, TrellisDecoder};
pub use viterbi::{
    check_backend, reencode_errors, ButterflyState, Ends, Kernel, Viterbi, ViterbiBackend,
    BACKENDS, MAX_HALF,
};

use crate::error::{FecError, Param, Result};

/// The largest number of outputs per input bit.
pub const MAX_OUTPUTS: usize = 8;

/// A rate `1/n` convolutional code: constraint length `K` (2 to 32), `n` generators (1 to 8) in sending order, newest bit in the
/// most significant place, and a mask of outputs sent inverted. See the [module documentation](self).
///
/// ```
/// use sdrkit_algo_fec::conv::{ConvCode, Encoder};
/// // DVB (EN 300 744 clause 4.3.3): G1 = 171 (X, sent first), G2 = 133 (Y).
/// let dvb = ConvCode::new(7, &[0o171, 0o133]);
/// let mut enc = Encoder::new(dvb);
/// let mut out = Vec::new();
/// enc.encode(&[1, 0, 0, 0, 0, 0, 0], &mut out);
/// // X reads the taps of 171 = 1 111 001 from the newest bit; Y those of 133 = 1 011 011.
/// let x: Vec<u8> = out.iter().step_by(2).copied().collect();
/// let y: Vec<u8> = out.iter().skip(1).step_by(2).copied().collect();
/// assert_eq!(x, [1, 1, 1, 1, 0, 0, 1]);
/// assert_eq!(y, [1, 0, 1, 1, 0, 1, 1]);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConvCode {
    k: u8,
    n: u8,
    polys: [u32; MAX_OUTPUTS],
    invert: u8,
}

/// Refuses a constraint length outside 2 to 32 or a generator count outside 1 to [`MAX_OUTPUTS`].
const fn check_shape(k: u8, count: usize) -> Result<()> {
    if k < 2 || k > 32 {
        return Err(FecError::OutOfRange {
            param: Param::ConstraintLength,
            min: 2,
            max: 32,
            got: k as i64,
        });
    }
    if count == 0 || count > MAX_OUTPUTS {
        return Err(FecError::OutOfRange {
            param: Param::Generators,
            min: 1,
            max: MAX_OUTPUTS as i64,
            got: count as i64,
        });
    }
    Ok(())
}

impl ConvCode {
    /// The code with constraint length `k` and generators `polys` (octal as printed, newest bit most significant, in sending
    /// order). For constants: an invalid code stops compilation of the constant (and panics if called at run time); use
    /// [`ConvCode::try_new`] for values that come from outside.
    pub const fn new(k: u8, polys: &[u32]) -> Self {
        match Self::try_new(k, polys) {
            Ok(c) => c,
            Err(_) => panic!("invalid convolutional code"),
        }
    }

    /// The code with constraint length `k` and generators `polys`, or an error when `k` is not 2 to 32, there are not 1 to 8
    /// generators, or a generator is 0 or has bits at or above `k`.
    pub const fn try_new(k: u8, polys: &[u32]) -> Result<Self> {
        if let Err(e) = check_shape(k, polys.len()) {
            return Err(e);
        }
        let mut p = [0u32; MAX_OUTPUTS];
        let mut i = 0;
        while i < polys.len() {
            if polys[i] == 0 || (k < 32 && polys[i] >> k != 0) {
                return Err(FecError::NotAllowed {
                    param: Param::Generator,
                    got: polys[i] as i64,
                });
            }
            p[i] = polys[i];
            i += 1;
        }
        Ok(ConvCode {
            k,
            n: polys.len() as u8,
            polys: p,
            invert: 0,
        })
    }

    /// The code from masks of a register kept with the **newest bit at bit 0** (`reg = (reg << 1) | bit`): each mask is reversed
    /// over `k` bits. Panics like [`ConvCode::new`] on an invalid code.
    pub const fn from_lsb_masks(k: u8, masks: &[u32]) -> Self {
        match Self::try_from_lsb_masks(k, masks) {
            Ok(c) => c,
            Err(_) => panic!("invalid convolutional code"),
        }
    }

    /// [`ConvCode::from_lsb_masks`] returning an error instead of panicking.
    pub const fn try_from_lsb_masks(k: u8, masks: &[u32]) -> Result<Self> {
        if let Err(e) = check_shape(k, masks.len()) {
            return Err(e);
        }
        let mut p = [0u32; MAX_OUTPUTS];
        let mut i = 0;
        while i < masks.len() {
            p[i] = reverse_bits(masks[i], k);
            i += 1;
        }
        let mut c = match Self::try_new(k, p.split_at(masks.len()).0) {
            Ok(c) => c,
            Err(e) => return Err(e),
        };
        c.polys = p;
        Ok(c)
    }

    /// The same code with the outputs whose bit is set in `mask` sent inverted (bit 0 is the first output).
    pub const fn inverted(self, mask: u8) -> Self {
        let keep = if self.n >= 8 {
            0xFF
        } else {
            (1u8 << self.n) - 1
        };
        ConvCode {
            invert: mask & keep,
            ..self
        }
    }

    /// Constraint length `K`.
    pub const fn k(&self) -> u32 {
        self.k as u32
    }

    /// Outputs per input bit (`n` of rate `1/n`).
    pub const fn n(&self) -> usize {
        self.n as usize
    }

    /// The generators in sending order (newest bit most significant).
    pub fn polys(&self) -> &[u32] {
        &self.polys[..self.n as usize]
    }

    /// Mask of the outputs sent inverted (bit 0 is the first output).
    pub const fn invert_mask(&self) -> u8 {
        self.invert
    }

    /// Number of encoder states, `2^(K-1)`.
    pub const fn states(&self) -> u64 {
        1u64 << (self.k - 1)
    }

    /// The outputs for the `K`-bit register `reg` (input bit in bit `K − 1`), output `i` in bit `i`, inversion applied.
    #[inline]
    pub fn label(&self, reg: u32) -> u8 {
        let mut l = 0u8;
        for i in 0..self.n as usize {
            l |= ((reg & self.polys[i]).count_ones() as u8 & 1) << i;
        }
        l ^ self.invert
    }

    /// Whether every generator taps both the newest and the oldest bit. Then the four branches of each butterfly carry a label and
    /// its complement (the property the fast Viterbi kernels use). True for every code in [`codes`].
    pub fn is_symmetric(&self) -> bool {
        let top = 1u32 << (self.k - 1);
        self.polys().iter().all(|&p| p & top != 0 && p & 1 != 0)
    }
}

const fn reverse_bits(x: u32, k: u8) -> u32 {
    let mut r = 0u32;
    let mut i = 0;
    while i < k {
        if x >> i & 1 != 0 {
            r |= 1 << (k - 1 - i);
        }
        i += 1;
    }
    r
}

/// A convolutional encoder (any [`ConvCode`]), starting in state 0 unless told otherwise.
///
/// Output bits are hard bits (`u8`, 0 or 1), `n` per input bit in sending order.
#[derive(Clone, Copy, Debug)]
pub struct Encoder {
    code: ConvCode,
    state: u32,
}

impl Encoder {
    /// An encoder in state 0.
    pub fn new(code: ConvCode) -> Self {
        Encoder { code, state: 0 }
    }

    /// An encoder in the given state (the last `K − 1` inputs, newest in bit `K − 2`; extra bits are ignored).
    pub fn with_state(code: ConvCode, state: u32) -> Self {
        let mut e = Encoder::new(code);
        e.set_state(state);
        e
    }

    /// The code.
    pub fn code(&self) -> &ConvCode {
        &self.code
    }

    /// The current state.
    pub fn state(&self) -> u32 {
        self.state
    }

    /// Sets the state (extra bits are ignored).
    pub fn set_state(&mut self, state: u32) {
        self.state = state & state_mask(self.code.k());
    }

    /// Encodes one bit (its lowest bit) and returns its outputs, output `i` in bit `i`.
    #[inline]
    pub fn push_bit(&mut self, bit: u8) -> u8 {
        let k = self.code.k();
        let reg = ((bit as u32 & 1) << (k - 1)) | self.state;
        self.state = reg >> 1;
        self.code.label(reg)
    }

    /// Encodes `bits`, appending `n` output bits per input bit to `out`.
    pub fn encode(&mut self, bits: &[u8], out: &mut Vec<u8>) {
        let n = self.code.n();
        for &b in bits {
            let l = self.push_bit(b);
            for i in 0..n {
                out.push((l >> i) & 1);
            }
        }
    }

    /// Encodes `bits` into `out` (at least `n` times as long); returns the number of output bits written.
    pub fn encode_into(&mut self, bits: &[u8], out: &mut [u8]) -> Result<usize> {
        let n = self.code.n();
        let need = bits.len() * n;
        if out.len() < need {
            return Err(FecError::OutputTooShort { need });
        }
        for (&b, o) in bits.iter().zip(out.chunks_exact_mut(n)) {
            let l = self.push_bit(b);
            for (i, v) in o.iter_mut().enumerate() {
                *v = (l >> i) & 1;
            }
        }
        Ok(need)
    }

    /// Appends the `K − 1` zero tail bits' outputs, which bring the encoder back to state 0.
    pub fn terminate(&mut self, out: &mut Vec<u8>) {
        for _ in 1..self.code.k() {
            let l = self.push_bit(0);
            for i in 0..self.code.n() {
                out.push((l >> i) & 1);
            }
        }
    }
}

/// The state mask for constraint length `k`.
#[inline]
pub(crate) const fn state_mask(k: u32) -> u32 {
    if k >= 33 {
        u32::MAX
    } else {
        ((1u64 << (k - 1)) - 1) as u32
    }
}

/// The tail-biting start state of `bits`: the last `K − 1` bits (wrapping when the frame is shorter), newest in bit `K − 2`.
pub fn tail_biting_state(code: &ConvCode, bits: &[u8]) -> u32 {
    let k = code.k();
    if bits.is_empty() {
        return 0;
    }
    let len = bits.len();
    let mut s = 0u32;
    for j in 0..(k - 1) as usize {
        // Bit K-2-j is the input j steps before the last one.
        let b = bits[(len - 1 - j % len) % len] as u32 & 1;
        s |= b << (k - 2 - j as u32);
    }
    s
}

/// Encodes `bits` tail-biting (TS 36.212 clause 5.1.3.1): the encoder starts in the state its last `K − 1` inputs leave it in,
/// so it ends where it started. Writes `bits.len() * n` output bits into `out`.
///
/// ```
/// use sdrkit_algo_fec::conv::{codes, encode_tail_biting};
/// let bits = [1, 0, 1, 1, 0, 0, 1, 0, 1, 1];
/// let mut out = [0u8; 30];
/// encode_tail_biting(&codes::LTE_K7_R3, &bits, &mut out).unwrap();
/// ```
pub fn encode_tail_biting(code: &ConvCode, bits: &[u8], out: &mut [u8]) -> Result<()> {
    let mut e = Encoder::with_state(*code, tail_biting_state(code, bits));
    e.encode_into(bits, out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsb_masks_are_reversed() {
        // 0x6D / 0x4F (newest at bit 0) are 133 / 171 written newest-first.
        let c = ConvCode::from_lsb_masks(7, &[0x6D, 0x4F]);
        assert_eq!(c.polys(), &[0o133, 0o171]);
        assert!(ConvCode::try_new(7, &[0o400]).is_err());
        assert!(ConvCode::try_new(1, &[1]).is_err());
        assert!(ConvCode::try_new(7, &[]).is_err());
        assert!(ConvCode::try_new(7, &[0]).is_err());
    }

    #[test]
    fn tail_biting_ends_where_it_starts() {
        let code = ConvCode::new(7, &[0o133, 0o171, 0o165]);
        let bits: Vec<u8> = (0..40).map(|i| ((i * 7 + 3) % 5 % 2) as u8).collect();
        let s = tail_biting_state(&code, &bits);
        let mut e = Encoder::with_state(code, s);
        let mut out = Vec::new();
        e.encode(&bits, &mut out);
        assert_eq!(e.state(), s);
        // A frame shorter than K - 1 wraps.
        let short = [1u8, 0];
        let s = tail_biting_state(&code, &short);
        let mut e = Encoder::with_state(code, s);
        e.encode(&short, &mut out);
        assert_eq!(e.state(), s);
    }

    #[test]
    fn terminate_returns_to_zero() {
        let code = ConvCode::new(5, &[0o23, 0o33]);
        let mut e = Encoder::new(code);
        let mut out = Vec::new();
        e.encode(&[1, 1, 0, 1], &mut out);
        e.terminate(&mut out);
        assert_eq!(e.state(), 0);
        assert_eq!(out.len(), 16);
    }
}
