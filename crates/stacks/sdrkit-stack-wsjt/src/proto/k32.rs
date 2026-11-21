//! The K = 32 rate 1/2 convolutional code of WSPR, JT9 and JT4 with its bit-reversal interleaver, on the shared encoder and the
//! shared Fano decoder.
//! Standard: A. Talbot (G4JNT), "The WSPR Coding Process" (2009).

use sdrkit_algo_fec::conv::{codes::WSPR_K32, Encoder, Fano, FanoParams};
use sdrkit_util_bits::interleave::Permutation;

/// Tail bits the encoder adds after the message (`K - 1`).
pub const TAIL: usize = 31;

/// The interleaver of `n` channel bits (162 for WSPR, 206 for JT9 and JT4).
fn interleaver(n: usize) -> Option<Permutation> {
    Permutation::bit_reverse8(n).ok()
}

/// The channel bits of `bits` (the message without its tail): encoded with the tail added, then interleaved. Empty when the
/// result has more than 256 channel bits.
pub fn encode(bits: &[u8]) -> Vec<u8> {
    let mut enc = Encoder::new(WSPR_K32);
    let mut cw = Vec::with_capacity(2 * (bits.len() + TAIL));
    enc.encode(bits, &mut cw);
    enc.terminate(&mut cw);
    let Some(p) = interleaver(cw.len()) else {
        return Vec::new();
    };
    let mut out = vec![0u8; cw.len()];
    if p.apply(&cw, &mut out).is_err() {
        return Vec::new();
    }
    out
}

/// What [`decode`] found.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    /// The message bits (the tail removed).
    pub bits: Vec<u8>,
    /// Code bits whose hard decision disagrees with the re-encoded message.
    pub errors: usize,
}

/// Decodes `nbits` message bits from channel-bit LLRs in transmission order (positive means 0). `None` when the Fano decoder
/// stops without a result, or when more than 30 % of the code bits disagree with the result.
pub fn decode(llr_sent: &[f32], nbits: usize) -> Option<Decoded> {
    let n = 2 * (nbits + TAIL);
    let llr_sent = llr_sent.get(..n)?;
    let p = interleaver(n)?;
    let mut llr = vec![0f32; n];
    p.invert(llr_sent, &mut llr).ok()?;
    let params = FanoParams {
        max_cycles_per_bit: 20_000,
        ..FanoParams::default()
    };
    let mut fano = Fano::new(WSPR_K32, nbits + TAIL, params).ok()?;
    let mut bits = vec![0u8; nbits];
    fano.decode(&llr, &mut bits).ok()?;
    let mut enc = Encoder::new(WSPR_K32);
    let mut cw = Vec::with_capacity(n);
    enc.encode(&bits, &mut cw);
    enc.terminate(&mut cw);
    let errors = cw
        .iter()
        .zip(&llr)
        .filter(|(&b, &l)| (b == 1) != (l < 0.0))
        .count();
    if errors > n * 3 / 10 {
        return None;
    }
    Some(Decoded { bits, errors })
}
