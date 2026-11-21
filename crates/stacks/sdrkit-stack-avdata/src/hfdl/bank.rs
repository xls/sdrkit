//! The shared channeliser for wide HFDL captures: the polyphase FFT filter bank of sdrkit-algo-dsp, oversampled by two, with the
//! band spacing B = rate / N at most 64 kHz. Each channel takes the band nearest its carrier and goes on at 2B with its own
//! small channel filter, so a wide capture with many channels costs about what one channel does.

use sdrkit_algo_dsp::bank::PolyphaseBank;
use sdrkit_algo_dsp::DspError;
use sdrkit_core::Complex32;

/// Above this rate (with at least three channels in band) the shared channeliser replaces the per-channel mixers.
pub const SHARED_ABOVE_HZ: f64 = 16e6;

/// Prototype taps per band (the prototype has 4N taps).
const TAPS_PER_BAND: usize = 4;
/// The prototype's stop band: 50 dB down from about 1.37 B (what folds onto a channel at 2B).
const ATTEN_DB: f64 = 50.0;

/// The channeliser: the bank, with the rate that sets its spacing.
pub struct PolyBank {
    bank: PolyphaseBank,
    n: usize,
    rate: f64,
}

impl PolyBank {
    /// The number of bands for a rate: the smallest power of two giving a spacing of at most 64 kHz.
    pub fn size_for(rate: f64) -> usize {
        let mut n = 2usize;
        while rate / n as f64 > 64e3 && n < 65_536 {
            n *= 2;
        }
        n
    }

    /// A bank for `rate` giving the bands `bands` (indices modulo N). An error for a band out of range.
    pub fn new(rate: f64, bands: Vec<usize>) -> Result<Self, DspError> {
        let n = Self::size_for(rate);
        let read: Vec<usize> = bands.iter().map(|&k| k % n).collect();
        Ok(PolyBank {
            bank: PolyphaseBank::new(n, TAPS_PER_BAND, 2, ATTEN_DB, &read)?,
            n,
            rate,
        })
    }

    /// The band spacing B, Hz.
    pub fn spacing(&self) -> f64 {
        self.rate / self.n as f64
    }

    /// The band rate, 2B.
    pub fn band_rate(&self) -> f64 {
        2.0 * self.spacing()
    }

    /// The band holding an offset from the centre, and the offset left inside it.
    pub fn band_of(&self, offset_hz: f64) -> (usize, f64) {
        let b = self.spacing();
        let k = (offset_hz / b).round();
        (
            (k as i64).rem_euclid(self.n as i64) as usize,
            offset_hz - k * b,
        )
    }

    /// Feeds input samples. Appends each chosen band's new samples to `out[i]` (in the order of the bands given).
    pub fn process(&mut self, input: &[Complex32], out: &mut [Vec<Complex32>]) {
        let _ = self.bank.process(input, out);
    }

    /// Forgets the input.
    pub fn reset(&mut self) {
        self.bank.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_lands_in_its_band_at_its_offset() {
        let rate = 32e6;
        let f = 3 * 62_500 + 10_000;
        let mut bank = PolyBank::new(rate, vec![3, 4, 509]).unwrap();
        assert_eq!(bank.size_for_check(), 512);
        let (k, r) = bank.band_of(f as f64);
        assert_eq!((k, r.round()), (3, 10_000.0));
        let n = 200_000;
        let x: Vec<Complex32> = (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * f as f64 * i as f64 / rate;
                Complex32::new(a.cos() as f32, a.sin() as f32)
            })
            .collect();
        let mut out = vec![Vec::new(), Vec::new(), Vec::new()];
        bank.process(&x, &mut out);
        let y = &out[0][400..];
        // Unit gain, and the tone at +10 kHz in a 125 kHz band: the phase step is 2 pi 10/125 per sample.
        let p: f32 = y.iter().map(|v| v.norm_sqr()).sum::<f32>() / y.len() as f32;
        assert!((p - 1.0).abs() < 0.02, "{p}");
        let step = (y[101] * y[100].conj()).arg() as f64;
        assert!(
            (step - std::f64::consts::TAU * 10e3 / 125e3).abs() < 1e-3,
            "{step}"
        );
        // Far bands hold nothing.
        let q: f32 = out[2][400..].iter().map(|v| v.norm_sqr()).sum::<f32>() / y.len() as f32;
        assert!(q < 1e-5, "{q}");
    }

    impl PolyBank {
        fn size_for_check(&self) -> usize {
            self.n
        }
    }
}
