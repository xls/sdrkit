//! Forward error correction of the message layers: the soft-symbol front of the shared Viterbi decoder, and the GLONASS string
//! Hamming check.
//!
//! Soft symbols: positive means a 0 symbol (+1 on the carrier), negative a 1.
//!
//! Standard: GLONASS ICD edition 5.1 clause 4.7 (the Hamming check).

use sdrkit_algo_fec::conv::{ConvCode, Ends, Viterbi};
use sdrkit_algo_fec::llr::{Llr8, Quantizer};
use sdrkit_algo_fec::small::GlonassHamming;
use std::sync::OnceLock;

pub use sdrkit_algo_fec::conv::codes::{GALILEO_K7, K7_171_133};

/// The shared Viterbi decoder fed with `f32` soft symbols. It keeps its buffers, so it allocates nothing once they have their
/// size.
#[derive(Clone, Debug)]
pub struct SoftViterbi {
    viterbi: Viterbi,
    quantizer: Quantizer,
    llr: Vec<Llr8>,
}

impl SoftViterbi {
    /// A decoder for `code` with frames of up to `max_steps` input bits.
    pub fn new(code: ConvCode, max_steps: usize) -> SoftViterbi {
        SoftViterbi {
            // A K = 7 code is in range of the shared decoder, so `new` cannot fail here.
            viterbi: Viterbi::new(code, max_steps).expect("a K = 7 code"),
            quantizer: Quantizer::default(),
            llr: Vec::with_capacity(2 * max_steps),
        }
    }

    /// Decodes `symbols.len() / 2` steps into `out` (cleared first). With [`Ends::Zero`] the six tail bits are written as zeros
    /// at the end, so `out` always holds one bit per step. If the frame is longer than the decoder accepts, `out` stays empty.
    pub fn decode(&mut self, symbols: &[f32], ends: Ends, out: &mut Vec<u8>) {
        out.clear();
        let n = symbols.len() / 2 * 2;
        let steps = n / 2;
        if steps == 0 {
            return;
        }
        self.llr.clear();
        self.llr.resize(n, 0);
        let x = &symbols[..n];
        let scale = self.quantizer.scale_for(x);
        // A NaN symbol is written as an erasure, so the error only reports it.
        let _ = self.quantizer.quantize(x, scale, &mut self.llr);
        let info = match ends {
            Ends::Zero => steps.saturating_sub(6),
            _ => steps,
        };
        out.resize(info, 0);
        if self.viterbi.decode(&self.llr, ends, out).is_err() {
            out.clear();
            return;
        }
        out.resize(steps, 0);
    }
}

/// The GLONASS string code, built once.
pub fn glonass() -> &'static GlonassHamming {
    static CODE: OnceLock<GlonassHamming> = OnceLock::new();
    CODE.get_or_init(GlonassHamming::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_fec::conv::Encoder;
    use sdrkit_algo_fec::small::GlonassCheck;

    fn soft(sym: &[u8]) -> Vec<f32> {
        sym.iter().map(|&s| 1.0 - 2.0 * s as f32).collect()
    }

    #[test]
    fn soft_viterbi_round_trip_with_errors() {
        for code in [K7_171_133, GALILEO_K7] {
            let bits: Vec<u8> = (0..200).map(|i| ((i * 37 + 11) % 7 < 3) as u8).collect();
            let mut tail = bits.clone();
            tail.extend_from_slice(&[0; 6]);
            let mut sym = Vec::new();
            Encoder::new(code).encode(&tail, &mut sym);
            let mut s = soft(&sym);
            for i in (5..s.len()).step_by(23) {
                s[i] = -s[i];
            }
            let mut v = SoftViterbi::new(code, 300);
            let mut out = Vec::new();
            v.decode(&s, Ends::Zero, &mut out);
            assert_eq!(&out[..200], &bits[..]);
            // Unknown start: decoding from the middle of a stream.
            v.decode(&s[40..], Ends::Open, &mut out);
            assert_eq!(&out[10..170], &bits[30..190]);
        }
    }

    #[test]
    fn glonass_hamming_n13() {
        // The code is tested in sdrkit-algo-fec; here the string layout the message layer uses: s[85 - k] is string bit k.
        let mut s = [0u8; 85];
        for k in 9..=84 {
            s[85 - k] = ((k * 13 + 5) % 3 == 0) as u8;
        }
        glonass().encode_bits(&mut s).unwrap();
        let good = s;
        assert_eq!(glonass().check_bits(&mut s), Ok(GlonassCheck::Good));
        for k in 9..=85 {
            let mut t = good;
            t[85 - k] ^= 1;
            assert_eq!(
                glonass().check_bits(&mut t),
                Ok(GlonassCheck::Corrected),
                "bit {k}"
            );
            assert_eq!(t, good);
        }
        let mut t = good;
        t[85 - 20] ^= 1;
        t[85 - 40] ^= 1;
        assert_eq!(glonass().check_bits(&mut t), Ok(GlonassCheck::Erased));
    }
}
