//! The polyphase analysis filterbank: one capture split into `M` evenly spaced bins at once, each brought to baseband at twice
//! the bin spacing. Its cost per input sample does not grow with the number of bins that are read.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::{design_beta, lowpass_len, windowed_sinc};
use sdrkit_algo_dsp::{Complex32, DspError, Param};

/// The flat part of a bin, as a fraction of the bin spacing on each side of its centre.
pub const PASS_FRACTION: f64 = 0.75;

/// A streaming polyphase analysis filterbank, twice oversampled. The prototype filter is flat to 0.75 of the bin spacing and stops
/// from 1.25 of it, so a channel of width `B` at `r` from its nearest bin centre is clean when `|r| + B / 2 <= 0.75 * spacing`.
#[derive(Clone, Debug)]
pub struct Filterbank {
    rate: f64,
    m: usize,
    d: usize,
    /// The prototype, time-reversed, `K * M` taps.
    taps_rev: Vec<f32>,
    /// The last `L - 1` input samples.
    hist: Vec<Complex32>,
    /// One window that spans the history and the new input.
    window: Vec<Complex32>,
    fold: Vec<Complex32>,
    spectrum: Vec<Complex32>,
    fft: Fft,
    /// `e^{-j 2 pi i / M}`.
    twiddle: Vec<Complex32>,
    /// The absolute index of the next input sample.
    n: u64,
    /// Samples since the last output.
    phase: usize,
}

impl Filterbank {
    /// A filterbank of `bins` bins (a power of two, at least 2) over a capture at `rate`, with a prototype of `atten_db` stopband
    /// attenuation.
    pub fn new(rate: f64, bins: usize, atten_db: f64) -> Result<Self, DspError> {
        if !rate.is_finite() {
            return Err(DspError::NotFinite { param: Param::Rate });
        }
        if rate <= 0.0 {
            return Err(DspError::OutOfRange {
                param: Param::Rate,
                min: f64::MIN_POSITIVE,
                max: f64::MAX,
                got: rate,
            });
        }
        if bins < 2 || !bins.is_power_of_two() || bins > 1 << 16 {
            return Err(DspError::NotAllowed {
                param: Param::Bins,
                got: bins as f64,
            });
        }
        if !(20.0..=150.0).contains(&atten_db) {
            return Err(DspError::OutOfRange {
                param: Param::AttenuationDb,
                min: 20.0,
                max: 150.0,
                got: atten_db,
            });
        }
        let m = bins;
        let spacing = rate / m as f64;
        let pass = PASS_FRACTION * spacing;
        let stop = (2.0 - PASS_FRACTION) * spacing;
        // For two bins the stop edge reaches the Nyquist frequency of the capture: clamp it.
        let stop = stop.min(rate / 2.0);
        let need = lowpass_len(rate, pass, stop, atten_db)?;
        let k = need.div_ceil(m).max(1);
        let l = k * m;
        let proto = windowed_sinc(l, (pass + stop) / 2.0 / rate, design_beta(atten_db))?;
        let taps_rev: Vec<f32> = proto.iter().rev().map(|&v| v as f32).collect();
        let twiddle = (0..m)
            .map(|i| {
                let a = -std::f64::consts::TAU * i as f64 / m as f64;
                Complex32::new(a.cos() as f32, a.sin() as f32)
            })
            .collect();
        Ok(Filterbank {
            rate,
            m,
            d: (m / 2).max(1),
            taps_rev,
            hist: vec![Complex32::default(); l - 1],
            window: Vec::with_capacity(l),
            fold: vec![Complex32::default(); m],
            spectrum: vec![Complex32::default(); m],
            fft: Fft::forward(m),
            twiddle,
            n: 0,
            phase: 0,
        })
    }

    /// The capture rate.
    pub fn input_rate(&self) -> f64 {
        self.rate
    }

    /// The number of bins.
    pub fn bins(&self) -> usize {
        self.m
    }

    /// The distance between bin centres, in Hz.
    pub fn spacing(&self) -> f64 {
        self.rate / self.m as f64
    }

    /// The rate of every bin's output: twice the spacing.
    pub fn output_rate(&self) -> f64 {
        self.rate / self.d as f64
    }

    /// The prototype's length (`taps per bin * bins`).
    pub fn taps(&self) -> usize {
        self.taps_rev.len()
    }

    /// The delay through the filterbank, in input samples.
    pub fn delay(&self) -> f64 {
        (self.taps_rev.len() - 1) as f64 / 2.0
    }

    /// The centre of bin `k`, in Hz from the capture's centre (bins above `M / 2` are the negative frequencies).
    pub fn bin_offset_hz(&self, k: usize) -> f64 {
        bin_offset(self.rate, self.m, k)
    }

    /// The bin nearest to `offset_hz` (from the capture's centre) and the signal's distance from its centre.
    pub fn nearest(&self, offset_hz: f64) -> (usize, f64) {
        nearest(self.rate, self.m, offset_hz)
    }

    /// Whether a channel of `bandwidth_hz` at `offset_hz` lies inside the flat part of its nearest bin.
    pub fn fits(&self, offset_hz: f64, bandwidth_hz: f64) -> bool {
        fits(self.rate, self.m, offset_hz, bandwidth_hz)
    }

    /// Clears the history (after a retune of the radio).
    pub fn reset(&mut self) {
        self.hist.clear();
        self.hist
            .resize(self.taps_rev.len() - 1, Complex32::default());
        self.n = 0;
        self.phase = 0;
    }

    /// Runs `input` through the filterbank and appends the outputs of the bins in `bins` to the matching vectors of `outs`
    /// (`outs[i]` gets bin `bins[i]`). Every vector gets the same number of samples. `outs` must be at least as long as `bins`.
    /// The filterbank reads `input` in place. It keeps only the last `taps() - 1` samples for the next call.
    pub fn process(&mut self, input: &[Complex32], bins: &[usize], outs: &mut [Vec<Complex32>]) {
        assert!(outs.len() >= bins.len(), "one output vector per bin");
        let l = self.taps_rev.len();
        let m = self.m;
        let keep = l - 1;
        // Index e counts samples of the history followed by the input: the input starts at e = keep.
        let mut e = keep;
        let end = keep + input.len();
        let base = self.n;
        while e < end {
            let todo = self.d - self.phase;
            if e + todo > end {
                self.phase += end - e;
                break;
            }
            e += todo;
            self.phase = 0;
            // The output for the sample at index e - 1. Its window is the last l samples up to there.
            let last = e - 1;
            let abs = base + (last - keep) as u64;
            let win: &[Complex32] = if last + 1 >= l + keep {
                &input[last + 1 - l - keep..=last - keep]
            } else {
                let from_hist = keep - (last + 1 - l);
                self.window.clear();
                self.window
                    .extend_from_slice(&self.hist[keep - from_hist..]);
                self.window.extend_from_slice(&input[..l - from_hist]);
                &self.window
            };
            self.fold.fill(Complex32::default());
            for (t, w) in self.taps_rev.chunks_exact(m).zip(win.chunks_exact(m)) {
                for ((f, &h), &x) in self.fold.iter_mut().zip(t).zip(w) {
                    f.re += h * x.re;
                    f.im += h * x.im;
                }
            }
            // v[i] = fold[(i - 1) mod M].
            self.spectrum[0] = self.fold[m - 1];
            self.spectrum[1..].copy_from_slice(&self.fold[..m - 1]);
            self.fft.process(&mut self.spectrum);
            let nm = (abs % m as u64) as usize;
            for (&k, out) in bins.iter().zip(outs.iter_mut()) {
                let rot = self.twiddle[(k * nm) % m];
                out.push(self.spectrum[k] * rot);
            }
        }
        self.n += input.len() as u64;
        if input.len() >= keep {
            self.hist.copy_from_slice(&input[input.len() - keep..]);
        } else {
            self.hist.copy_within(input.len().., 0);
            self.hist[keep - input.len()..].copy_from_slice(input);
        }
    }
}

/// The centre of bin `k` of `m` over a capture at `rate`, in Hz from the capture's centre.
pub fn bin_offset(rate: f64, m: usize, k: usize) -> f64 {
    let k = k % m;
    let s = rate / m as f64;
    if k < m.div_ceil(2) {
        k as f64 * s
    } else {
        (k as f64 - m as f64) * s
    }
}

/// The bin of `m` nearest to `offset_hz`, and the distance from its centre.
pub fn nearest(rate: f64, m: usize, offset_hz: f64) -> (usize, f64) {
    let s = rate / m as f64;
    let i = (offset_hz / s).round();
    let k = (i as i64).rem_euclid(m as i64) as usize;
    (k, offset_hz - i * s)
}

/// Whether a channel of `bandwidth_hz` at `offset_hz` lies inside the flat part of its nearest bin of `m`.
pub fn fits(rate: f64, m: usize, offset_hz: f64, bandwidth_hz: f64) -> bool {
    let (_, r) = nearest(rate, m, offset_hz);
    r.abs() + bandwidth_hz / 2.0 <= PASS_FRACTION * rate / m as f64 + 1e-9
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, rate: f64, f: f64, start: u64) -> Vec<Complex32> {
        (0..n)
            .map(|i| {
                let p = std::f64::consts::TAU * ((f * (i as u64 + start) as f64 / rate).fract());
                Complex32::new(p.cos() as f32, p.sin() as f32)
            })
            .collect()
    }

    fn power(x: &[Complex32]) -> f64 {
        x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len().max(1) as f64
    }

    #[test]
    fn a_tone_lands_in_its_bin_at_the_residual_frequency() {
        let rate = 2.4e6;
        let mut fb = Filterbank::new(rate, 64, 80.0).unwrap();
        let k = 5;
        let f = fb.bin_offset_hz(k) + 7e3;
        let x = tone(200_000, rate, f, 0);
        let mut outs = vec![Vec::new(), Vec::new()];
        fb.process(&x, &[k, k + 2], &mut outs);
        assert_eq!(outs[0].len(), 200_000 / 32);
        let y = &outs[0][200..];
        assert!((power(y) - 1.0).abs() < 0.01, "power in bin {}", power(y));
        // The residual is a tone at +7 kHz at the bin rate.
        let r = fb.output_rate();
        let mut acc = Complex32::default();
        for (i, v) in y.iter().enumerate() {
            let p = -std::f64::consts::TAU * 7e3 * i as f64 / r;
            acc += *v * Complex32::new(p.cos() as f32, p.sin() as f32);
        }
        assert!((acc.norm() as f64 / y.len() as f64) > 0.99);
        // Two bins away it is gone.
        assert!(
            power(&outs[1][200..]) < 1e-7,
            "leak {}",
            power(&outs[1][200..])
        );
    }

    #[test]
    fn negative_frequencies_and_any_cut_of_the_stream_give_the_same_output() {
        let rate = 1e6;
        let mut a = Filterbank::new(rate, 16, 70.0).unwrap();
        let mut b = a.clone();
        let (k, r) = a.nearest(-180e3);
        assert_eq!(k, 13);
        assert!((r - (-180e3 + 187.5e3)).abs() < 1e-6);
        let x = tone(30_000, rate, -180e3, 0);
        let mut oa = vec![Vec::new()];
        a.process(&x, &[k], &mut oa);
        let mut ob = vec![Vec::new()];
        for piece in x.chunks(7) {
            b.process(piece, &[k], &mut ob);
        }
        assert_eq!(oa[0].len(), ob[0].len());
        for (p, q) in oa[0].iter().zip(&ob[0]) {
            assert!((p - q).norm() < 1e-5);
        }
        assert!((power(&oa[0][100..]) - 1.0).abs() < 0.01);
    }

    #[test]
    fn the_passband_is_flat_and_the_stopband_deep() {
        let rate = 1e6;
        let mut fb = Filterbank::new(rate, 32, 80.0).unwrap();
        let s = fb.spacing();
        for frac in [-0.7, -0.4, 0.0, 0.3, 0.74] {
            fb.reset();
            let x = tone(40_000, rate, 3.0 * s + frac * s, 0);
            let mut o = vec![Vec::new()];
            fb.process(&x, &[3], &mut o);
            let p = power(&o[0][100..]);
            assert!((10.0 * p.log10()).abs() < 0.05, "{frac}: {p}");
        }
        for frac in [1.3, 1.6, -1.4, 3.0] {
            fb.reset();
            let x = tone(40_000, rate, 3.0 * s + frac * s, 0);
            let mut o = vec![Vec::new()];
            fb.process(&x, &[3], &mut o);
            let p = power(&o[0][100..]);
            assert!(10.0 * p.log10() < -70.0, "{frac}: {p}");
        }
    }

    #[test]
    fn fitting_follows_the_residual() {
        let rate = 2.4e6;
        // 64 bins of 37.5 kHz: flat to 28.125 kHz from a centre.
        assert!(fits(rate, 64, 37.5e3 + 10e3, 25e3));
        assert!(!fits(rate, 64, 37.5e3 + 18e3, 25e3));
        assert!(fits(rate, 64, 18.75e3, 18.75e3));
        assert!(Filterbank::new(rate, 3, 80.0).is_err());
        assert!(Filterbank::new(rate, 2, 80.0).is_ok());
    }
}
