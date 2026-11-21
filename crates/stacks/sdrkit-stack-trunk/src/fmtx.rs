//! The FM transmitter of the crate's generators: audio at its own rate, resampled to the generator's rate and frequency
//! modulated onto a carrier. Noise and the receiver's impairments come from the framework's `gen_*` settings.

use num_complex::Complex32;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_sim::modulate::Fm;
use sdrkit_algo_sim::{Param, SimError};

/// Makes sure that a carrier at `carrier_hz` with `deviation_hz` of deviation fits in half of `rate`.
fn fits(rate: f64, deviation_hz: f64, carrier_hz: f64) -> Result<(), SimError> {
    let max = rate / 2.0 - carrier_hz.abs();
    if deviation_hz.abs() >= max || rate.is_nan() {
        return Err(SimError::OutOfRange {
            param: Param::Deviation,
            min: 0.0,
            max,
            got: deviation_hz.abs(),
        });
    }
    Ok(())
}

/// Audio from a closure, resampled to `rate` and frequency modulated (1.0 of audio is `deviation_hz`), at `carrier_hz` from
/// the centre. It keeps its state between calls, so any cut of the output produces the same signal.
pub struct AudioFm {
    res: FractionalResampler<f32>,
    fm: Fm,
    carrier: f32,
    audio_in: Vec<f32>,
    audio_out: Vec<f32>,
    used: usize,
}

impl AudioFm {
    /// A transmitter of audio at `audio_rate` into samples at `rate`.
    ///
    /// # Errors
    ///
    /// [`SimError`] when a rate is not positive or the deviation and the carrier do not fit in half the rate.
    pub fn new(
        audio_rate: f64,
        rate: f64,
        deviation_hz: f64,
        carrier_hz: f64,
    ) -> Result<AudioFm, SimError> {
        let pass = 0.42 * audio_rate.min(rate);
        let stop = 0.5 * audio_rate.min(rate);
        let res = FractionalResampler::new(audio_rate, rate, pass, stop, 60.0)?;
        fits(rate, deviation_hz, carrier_hz)?;
        let fm = Fm::new(rate, deviation_hz)?;
        Ok(AudioFm {
            res,
            fm,
            carrier: (carrier_hz / deviation_hz) as f32,
            audio_in: Vec::with_capacity(1024),
            audio_out: Vec::with_capacity(4096),
            used: 0,
        })
    }

    /// Fills `out`, taking audio from `next` in pieces of 256 samples (`next` writes into the slice it gets).
    pub fn fill(&mut self, out: &mut [Complex32], mut next: impl FnMut(&mut [f32])) {
        let mut done = 0;
        while done < out.len() {
            if self.used >= self.audio_out.len() {
                self.refill(&mut next);
                let c = self.carrier;
                self.audio_out.iter_mut().for_each(|v| *v += c);
                continue;
            }
            let n = (self.audio_out.len() - self.used).min(out.len() - done);
            let a = &self.audio_out[self.used..self.used + n];
            // The lengths are equal, so the modulator cannot fail.
            let _ = self.fm.process(a, &mut out[done..done + n]);
            self.used += n;
            done += n;
        }
    }

    /// Fills `out` with the resampled audio itself (no modulation and no carrier), taking audio from `next` as [`AudioFm::fill`]
    /// does.
    pub fn fill_audio(&mut self, out: &mut [f32], mut next: impl FnMut(&mut [f32])) {
        let mut done = 0;
        while done < out.len() {
            if self.used >= self.audio_out.len() {
                self.refill(&mut next);
                continue;
            }
            let n = (self.audio_out.len() - self.used).min(out.len() - done);
            out[done..done + n].copy_from_slice(&self.audio_out[self.used..self.used + n]);
            self.used += n;
            done += n;
        }
    }

    fn refill(&mut self, next: &mut impl FnMut(&mut [f32])) {
        self.audio_out.clear();
        self.used = 0;
        self.audio_in.clear();
        self.audio_in.resize(256, 0.0);
        next(&mut self.audio_in);
        self.res.process(&self.audio_in, &mut self.audio_out);
    }
}

/// Audio that loops: the next samples of `audio`, from the start again after its end.
pub struct Looping {
    audio: Vec<f32>,
    pos: usize,
}

impl Looping {
    /// Loops `audio` (silence when it is empty).
    pub fn new(audio: Vec<f32>) -> Looping {
        Looping { audio, pos: 0 }
    }
    /// Writes the next samples into `out`.
    pub fn fill(&mut self, out: &mut [f32]) {
        if self.audio.is_empty() {
            out.iter_mut().for_each(|v| *v = 0.0);
            return;
        }
        for v in out.iter_mut() {
            *v = self.audio[self.pos];
            self.pos += 1;
            if self.pos == self.audio.len() {
                self.pos = 0;
            }
        }
    }
}

/// Audio at `rate` frequency modulated with `deviation_hz` for 1.0, on a carrier at `carrier_hz` from the centre, at unit
/// amplitude.
///
/// # Errors
///
/// [`SimError`] when the deviation and the carrier do not fit in half the rate.
pub fn fm(
    audio: &[f32],
    rate: f64,
    deviation_hz: f64,
    carrier_hz: f64,
) -> Result<Vec<Complex32>, SimError> {
    fits(rate, deviation_hz, carrier_hz)?;
    let mut m = Fm::new(rate, deviation_hz)?;
    let c = (carrier_hz / deviation_hz) as f32;
    let shifted: Vec<f32> = audio.iter().map(|&a| a + c).collect();
    let mut out = vec![Complex32::new(0.0, 0.0); audio.len()];
    m.process(&shifted, &mut out)?;
    Ok(out)
}
