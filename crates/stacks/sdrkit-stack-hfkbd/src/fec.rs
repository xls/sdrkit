//! The convolutional codes of the family, and a streaming soft-decision decoder over the shared Viterbi decoder that takes one
//! pair of soft bits at a time.

use sdrkit_algo_fec::conv::{codes, ConvCode, Viterbi};
use sdrkit_algo_fec::Llr8;

/// The K = 7 code of MFSK and THOR (IZ8BLY MFSK16 description): masks 0x6D and 0x4F of a register with the newest bit at bit 0,
/// which are the octal generators 133 and 171.
pub const MFSK_K7: ConvCode = codes::INMARSAT_K7;
/// The K = 15 code of the fast THOR variants.
pub const THOR_K15: ConvCode = codes::THOR_K15;
/// The K = 5 code of QPSK31 (G3PLX's PSK31 description).
pub const QPSK_K5: ConvCode = codes::QPSK31_K5;

/// LLR units for a soft bit of size 1.
const SCALE: f32 = 100.0;

/// A rate-1/2 soft-decision decoder that is fed one pair of soft bits per step and returns the decided bits in order, one per
/// step after a fixed delay.
#[derive(Clone, Debug)]
pub struct SoftViterbi {
    v: Viterbi,
    out: Vec<u8>,
    next: usize,
    /// The growth of the best path metric in the last step, in soft-bit units.
    pub gain: f32,
}

impl SoftViterbi {
    /// A decoder for `code` (rate 1/2) that decides each bit after `depth` further steps.
    pub fn new(code: ConvCode, depth: usize) -> Self {
        let mut v = Viterbi::new(code, 4 * depth).expect("a rate-1/2 code with K of at most 16");
        v.set_traceback(depth, depth.div_ceil(4))
            .expect("a traceback depth from 1 to 2^20");
        SoftViterbi {
            v,
            out: Vec::with_capacity(2 * depth),
            next: 0,
            gain: 0.0,
        }
    }

    /// One step with the soft values of the two code bits (positive means 1, size about 1 for a clean bit). Returns the oldest
    /// bit not yet given, when there is one.
    pub fn push(&mut self, s0: f32, s1: f32) -> Option<u8> {
        let llr = [to_llr(s0), to_llr(s1)];
        if self.next == self.out.len() {
            self.out.clear();
            self.next = 0;
        }
        self.v.push(&llr, &mut self.out);
        self.gain = self.v.last_gain() / SCALE;
        let b = self.out.get(self.next).copied();
        if b.is_some() {
            self.next += 1;
        }
        b
    }

    /// Starts again with every state equally likely.
    pub fn reset(&mut self) {
        self.v.reset();
        self.out.clear();
        self.next = 0;
        self.gain = 0.0;
    }
}

/// A soft bit (positive means 1) as a shared LLR (positive means 0).
#[inline]
fn to_llr(s: f32) -> Llr8 {
    if s.is_finite() {
        (-s * SCALE).round().clamp(-127.0, 127.0) as Llr8
    } else {
        0
    }
}
