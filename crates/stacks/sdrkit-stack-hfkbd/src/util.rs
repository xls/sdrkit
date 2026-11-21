//! Small helpers of the family over the shared DSP types: the median, a tone source for the generators, and Walsh codes.

use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::Complex32;

pub use crate::front::db;

/// The median of `v`. `scratch` is reused, so nothing is allocated once it has grown. 0 for an empty slice.
pub fn median(v: &[f32], scratch: &mut Vec<f32>) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    sdrkit_algo_dsp::spectrum::percentile(v, 0.5, scratch)
}

/// A tone whose frequency can change at any sample without a phase jump, for the generators. It runs the shared oscillator.
#[derive(Clone, Debug)]
pub struct Tone {
    nco: Nco,
}

impl Tone {
    /// A tone source for samples at `rate` Hz.
    pub fn new(rate: f64) -> Self {
        Tone {
            nco: Nco::new(rate, 0.0),
        }
    }
    /// The next sample of `amp * e^(j phase)` at `freq` Hz.
    #[inline]
    pub fn next(&mut self, freq: f64, amp: f64) -> Complex32 {
        if self.nco.freq() != freq {
            self.nco.set_freq(freq);
        }
        self.nco.next_sample() * amp as f32
    }
}

/// The chip `i` (+1 or -1) of the first-order Reed-Muller code word of `code` with `2^m` chips: a Walsh-Hadamard row, inverted
/// when bit `m` of `code` is set.
#[inline]
pub fn walsh_chip(code: u32, i: u32, m: u32) -> f32 {
    let inv = (code >> m) & 1;
    if (((code & ((1 << m) - 1)) & i).count_ones() & 1) ^ inv == 1 {
        -1.0
    } else {
        1.0
    }
}

/// Decodes `2^m` soft chips (positive for +1) of a first-order Reed-Muller code by the fast Hadamard transform. `work` must have
/// the same length and is left holding the transform. Returns the code (inversion in bit `m`), the size of the best correlation
/// and the energy of all the others. A chip that is not finite returns code 0 and no correlation.
pub fn walsh_decode(soft: &[f32], work: &mut [f32]) -> (u32, f32, f32) {
    work.copy_from_slice(soft);
    match sdrkit_algo_fec::small::fht_decode(work) {
        Ok((code, best)) => {
            let tot: f32 = work.iter().map(|v| v * v).sum();
            (code, best, (tot - best * best).max(0.0))
        }
        Err(_) => (0, 0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walsh_decode_finds_every_code_word() {
        for code in [0u32, 5, 63, 64, 100, 127] {
            let x: Vec<f32> = (0..64).map(|i| walsh_chip(code, i, 6)).collect();
            let mut w = vec![0.0; 64];
            let (c, best, rest) = walsh_decode(&x, &mut w);
            assert_eq!(c, code);
            assert_eq!(best, 64.0);
            assert_eq!(rest, 0.0);
        }
    }
}
