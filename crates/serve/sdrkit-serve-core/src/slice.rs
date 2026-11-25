//! A client's own part of the capture: an oscillator moves the client's centre to 0 Hz and a resampler brings the capture
//! rate down to the client's rate. Both run on the client's writer thread.

use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::Complex32;
use sdrkit_driver_core::sample::from_iq;
use sdrkit_driver_core::sample::{size, to_iq};
use sdrkit_graph::stream::SampleType;

/// The centre and rate of a stream, Hz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slice {
    /// The centre frequency.
    pub centre_hz: f64,
    /// The sample rate.
    pub rate_hz: f64,
}

/// Why a slice does not fit the capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SliceError {
    /// The rate is not positive or is above the capture rate.
    #[error("the slice rate does not fit the capture rate")]
    Rate,
    /// Part of the slice lies outside the captured band.
    #[error("the slice lies outside the captured band")]
    Outside,
    /// No resampler can be built from the capture rate to the slice rate.
    #[error("no resampler from the capture rate to the slice rate")]
    Filter,
}

impl Slice {
    /// Checks that the slice lies inside a capture of `capture`: a rate from 0 to the capture rate, and
    /// `|centre - capture centre| + rate / 2 <= capture rate / 2`.
    ///
    /// # Errors
    ///
    /// [`SliceError`] if it does not.
    pub fn fits(&self, capture: &Slice) -> Result<(), SliceError> {
        let tol = 1e-6;
        if !(self.rate_hz > 0.0 && self.rate_hz <= capture.rate_hz + tol) {
            return Err(SliceError::Rate);
        }
        let edge = (self.centre_hz - capture.centre_hz).abs() + self.rate_hz / 2.0;
        if edge.is_nan() || edge > capture.rate_hz / 2.0 + tol {
            return Err(SliceError::Outside);
        }
        Ok(())
    }

    /// Checks that the slice fits `capture` and that its resampler can be built.
    ///
    /// # Errors
    ///
    /// [`SliceError`] if either fails.
    pub fn check(&self, capture: &Slice) -> Result<(), SliceError> {
        self.fits(capture)?;
        Resampler::<Complex32>::new(capture.rate_hz, self.rate_hz)
            .map(|_| ())
            .map_err(|_| SliceError::Filter)
    }
}

/// The state of a slice on a writer thread. It rebuilds its filters when the capture or the slice changes.
pub(crate) struct Slicer {
    slice: Slice,
    capture: Slice,
    nco: Nco,
    resampler: Option<Resampler<Complex32>>,
    iq: Vec<Complex32>,
    out: Vec<Complex32>,
    /// Output samples owed while the slice lies outside the capture, kept to a fraction.
    owed: f64,
}

impl Slicer {
    pub(crate) fn new(slice: Slice, capture: Slice) -> Slicer {
        let mut s = Slicer {
            slice,
            capture,
            nco: Nco::new(capture.rate_hz.max(1.0), 0.0),
            resampler: None,
            iq: Vec::new(),
            out: Vec::new(),
            owed: 0.0,
        };
        s.rebuild();
        s
    }

    fn rebuild(&mut self) {
        self.nco = Nco::new(
            self.capture.rate_hz.max(1.0),
            self.capture.centre_hz - self.slice.centre_hz,
        );
        self.resampler = if self.slice.fits(&self.capture).is_ok() {
            Resampler::new(self.capture.rate_hz, self.slice.rate_hz).ok()
        } else {
            None
        };
        self.owed = 0.0;
    }

    /// Follows a new slice or a new capture. A change of the centre only keeps the filter state.
    pub(crate) fn update(&mut self, slice: Slice, capture: Slice) {
        if slice == self.slice && capture == self.capture {
            return;
        }
        let rates_same =
            slice.rate_hz == self.slice.rate_hz && capture.rate_hz == self.capture.rate_hz;
        self.slice = slice;
        self.capture = capture;
        if rates_same && self.resampler.is_some() && slice.fits(&capture).is_ok() {
            self.nco.set_freq(capture.centre_hz - slice.centre_hz);
        } else {
            self.rebuild();
        }
    }

    /// Appends the slice of the samples of type `from` in `bytes` to `out` as type `to`. While the slice lies outside the
    /// capture, it appends zero samples at the slice rate.
    pub(crate) fn process(
        &mut self,
        from: SampleType,
        bytes: &[u8],
        to: SampleType,
        out: &mut Vec<u8>,
    ) {
        let n = bytes.len() / size(from);
        self.out.clear();
        match self.resampler.as_mut() {
            Some(r) => {
                self.iq.clear();
                self.iq.resize(n, Complex32::default());
                to_iq(from, bytes, &mut self.iq);
                self.nco.mix_in_place(&mut self.iq);
                r.process(&self.iq, &mut self.out);
            }
            None => {
                self.owed += n as f64 * self.slice.rate_hz / self.capture.rate_hz.max(1.0);
                let k = self.owed.floor();
                self.owed -= k;
                self.out.resize(k as usize, Complex32::default());
            }
        }
        from_iq(to, &self.out, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slice_is_checked_against_the_capture_and_its_resampler() {
        let cap = Slice {
            centre_hz: 100e6,
            rate_hz: 2.4e6,
        };
        let s = |centre_hz, rate_hz| Slice { centre_hz, rate_hz };
        assert_eq!(s(100.3e6, 240e3).check(&cap), Ok(()));
        assert_eq!(s(100e6, 1e-3).check(&cap), Ok(()));
        assert_eq!(s(100e6, 3e6).check(&cap), Err(SliceError::Rate));
        assert_eq!(s(100e6, 0.0).check(&cap), Err(SliceError::Rate));
        assert_eq!(s(101.5e6, 240e3).check(&cap), Err(SliceError::Outside));
        assert_eq!(s(f64::NAN, 240e3).check(&cap), Err(SliceError::Outside));
    }
}
