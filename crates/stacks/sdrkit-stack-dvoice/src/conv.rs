//! A convolutional channel of a voice protocol on the shared codes of `sdrkit-algo-fec`: the code, an optional puncturing pattern, and
//! a Viterbi decoder with its buffers, so that a frame decodes with no allocation.

use sdrkit_algo_fec::conv::{
    depuncture_frame, puncture, ConvCode, Encoder, Ends, Puncture, Viterbi,
};
use sdrkit_algo_fec::{FecError, Quantizer};

/// One convolutional channel: a code, its puncturing, and a decoder sized for the longest frame.
#[derive(Clone, Debug)]
pub struct ConvChannel {
    code: ConvCode,
    puncture: Option<Puncture<'static>>,
    viterbi: Viterbi,
    full: Vec<f32>,
    llr: Vec<i8>,
    out: Vec<u8>,
}

impl ConvChannel {
    /// A channel of `code`, punctured by `puncture` (from the first value of the pattern), for frames of up to `max_steps`
    /// input bits (the tail included).
    pub fn new(code: ConvCode, puncture: Option<Puncture<'static>>, max_steps: usize) -> Self {
        let n = code.n();
        ConvChannel {
            code,
            puncture,
            viterbi: Viterbi::new(code, max_steps).expect("a valid code and frame length"),
            full: vec![0.0; max_steps * n],
            llr: vec![0; max_steps * n],
            out: vec![0; max_steps],
        }
    }

    /// The bits sent for `steps` input bits.
    pub fn coded_len(&self, steps: usize) -> usize {
        let n = steps * self.code.n();
        self.puncture.map_or(n, |p| p.punctured_len(0, n))
    }

    /// Encodes `bits` from state 0 (the caller adds the tail bits of a terminated frame) and appends the bits sent to `out`.
    pub fn encode(&self, bits: &[u8], out: &mut Vec<u8>) {
        let mut coded = Vec::with_capacity(bits.len() * self.code.n());
        Encoder::new(self.code).encode(bits, &mut coded);
        match &self.puncture {
            Some(p) => puncture(p, 0, &coded, out),
            None => out.extend_from_slice(&coded),
        }
    }

    /// Decodes a frame of `steps` input bits from the soft values of the bits sent (the first [`ConvChannel::coded_len`] of
    /// `soft`. Positive means 1, as the symbol layer outputs them). A `terminated` frame ends with the `K - 1` zero tail bits,
    /// which are not returned. Returns the decoded bits and the received bits the decoder corrected. An error when `soft` is
    /// too short or the frame is longer than the channel was made for.
    pub fn decode(
        &mut self,
        soft: &[f32],
        steps: usize,
        terminated: bool,
    ) -> Result<(&[u8], usize), FecError> {
        let n = steps * self.code.n();
        let sent = self.coded_len(steps);
        if soft.len() < sent {
            return Err(FecError::InputLength {
                expected: sent,
                got: soft.len(),
            });
        }
        if n > self.full.len() {
            return Err(FecError::TooLong {
                max: self.full.len() / self.code.n(),
            });
        }
        let full = &mut self.full[..n];
        match &self.puncture {
            Some(p) => depuncture_frame(p, 0, &soft[..sent], full)?,
            None => full.copy_from_slice(&soft[..n]),
        }
        // Soft values say "positive means 1". LLRs say "positive means 0".
        for v in full.iter_mut() {
            *v = -*v;
        }
        let llr = &mut self.llr[..n];
        Quantizer::default().quantize_auto(full, llr)?;
        let (ends, len) = if terminated {
            (Ends::Zero, steps + 1 - self.code.k() as usize)
        } else {
            (Ends::Start(0), steps)
        };
        let d = self.viterbi.decode(llr, ends, &mut self.out[..len])?;
        Ok((&self.out[..len], d.corrected as usize))
    }
}
