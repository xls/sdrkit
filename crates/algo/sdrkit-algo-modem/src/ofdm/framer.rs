//! The FFT framer: one OFDM symbol from the samples to its used cells, with the carrier offset removed in time and the early
//! window's phase ramp removed per carrier.

use super::OfdmLayout;
use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::{fft::Fft, nco::Nco};

/// The oscillator `e^(j 2 pi (start + n step))` (cycles), a `sdrkit_algo_dsp::nco::Nco` re-anchored from its exact `f64` phase every 64
/// samples: no library `sin`/`cos` per sample, no drift.
pub(crate) fn rotator(start: f64, step: f64) -> Nco {
    let mut n = Nco::new(1.0, step);
    n.set_phase_cycles(start);
    n
}

/// Converts the samples of one symbol into its used cells (see [`FftFramer::demodulate`]).
///
/// The FFT window starts `backoff` samples before the end of the cyclic prefix, so a timing error of up to `backoff` samples late,
/// or echoes that arrive early, stay inside the prefix. The framer removes the phase slope `e^(-j 2 pi k d / N)` that a start `d`
/// samples early puts on carrier `k`. Cells are the FFT divided by `sqrt(N)`, so a transmitter that sends
/// `IFFT(cells) * sqrt(N) / N` (unit power per active cell) is received with the same cell values.
#[derive(Clone, Debug)]
pub struct FftFramer {
    layout: OfdmLayout,
    fft: Fft,
    buf: Vec<Complex32>,
    ramp: Vec<Complex32>,
    backoff: usize,
    timing: f64,
    freq: f64,
    /// The oscillator phase is `phase_ref - freq (abs - ref_abs)` cycles at absolute sample `abs`. Both 0 is the absolute form.
    phase_ref: f64,
    ref_abs: f64,
    /// The absolute index of the last window start.
    last_abs: f64,
}

impl FftFramer {
    /// A framer for `layout` with the window `backoff` samples early. Errors when `backoff` exceeds the shortest prefix.
    pub fn new(layout: &OfdmLayout, backoff: usize) -> Result<Self, ModemError> {
        let shortest = (0..layout.period().max(1) as u64 * 64)
            .map(|l| layout.cp(l))
            .min()
            .unwrap_or(0);
        if backoff > shortest {
            return Param::Backoff.range(0.0, shortest as f64, backoff as f64);
        }
        let n = layout.nfft();
        let mut f = FftFramer {
            layout: layout.clone(),
            fft: Fft::forward(n),
            buf: vec![Complex32::default(); n],
            ramp: vec![Complex32::default(); layout.used()],
            backoff,
            timing: 0.0,
            freq: 0.0,
            phase_ref: 0.0,
            ref_abs: 0.0,
            last_abs: 0.0,
        };
        f.make_ramp();
        Ok(f)
    }

    fn make_ramp(&mut self) {
        // The window starts backoff + timing samples before the symbol: carrier k is turned by e^(-j 2 pi k d / N); multiply by the
        // inverse.
        let n = self.layout.nfft() as f64;
        let d = self.backoff as f64 + self.timing;
        let first = self.layout.first() as f64;
        rotator(first * d / n, d / n).fill(&mut self.ramp);
        let s = 1.0 / (self.layout.nfft() as f32).sqrt();
        self.ramp.iter_mut().for_each(|v| *v *= s);
    }

    /// The layout.
    pub fn layout(&self) -> &OfdmLayout {
        &self.layout
    }

    /// The backoff in samples.
    pub fn backoff(&self) -> usize {
        self.backoff
    }

    /// Sets the carrier offset to remove, in cycles per sample (fractional plus `integer shift / nfft`). The oscillator phase is
    /// then `-freq * abs` at absolute sample `abs`, so a change of the offset turns every later symbol by `-delta * abs`: a phase
    /// jump that grows with the time since the stream began. A tracking loop that changes the offset while it runs uses
    /// [`set_freq_continuous`](Self::set_freq_continuous) instead.
    pub fn set_freq(&mut self, cycles_per_sample: f64) {
        if cycles_per_sample.is_finite() {
            self.freq = cycles_per_sample;
            self.phase_ref = 0.0;
            self.ref_abs = 0.0;
        }
    }

    /// Sets the carrier offset to remove, in cycles per sample, keeping the oscillator's phase continuous: the new offset starts
    /// from the phase the old one had at the start of the last window demodulated. Between two symbols the cells then turn only by
    /// the new offset times the time between them, as the signal does. A value that is not finite is ignored.
    pub fn set_freq_continuous(&mut self, cycles_per_sample: f64) {
        if !cycles_per_sample.is_finite() || cycles_per_sample == self.freq {
            return;
        }
        self.phase_ref =
            (self.phase_ref - self.freq * (self.last_abs - self.ref_abs)).rem_euclid(1.0);
        self.ref_abs = self.last_abs;
        self.freq = cycles_per_sample;
    }

    /// The carrier offset removed.
    pub fn freq(&self) -> f64 {
        self.freq
    }

    /// Sets a fractional timing offset (samples; positive when the symbol arrives later than the window assumes), removed as a phase
    /// slope over the carriers. [`super::phase_fit`] measures it.
    pub fn set_timing(&mut self, samples: f64) {
        if samples.is_finite() && samples != self.timing {
            self.timing = samples;
            self.make_ramp();
        }
    }

    /// The fractional timing removed.
    pub fn timing(&self) -> f64 {
        self.timing
    }

    /// Demodulates one symbol: `x[useful_start]` is the first sample after its cyclic prefix and `x0_index` the absolute index of
    /// `x[0]` (the carrier offset is removed with a phase continuous over absolute time, so symbols keep their relative phase). Reads
    /// `x[useful_start - backoff ..][..nfft]`; writes the used cells into `cells` (`layout.used()` values). The full FFT output (natural
    /// order, before the carrier corrections) stays available from [`bins`](Self::bins).
    pub fn demodulate(
        &mut self,
        x: &[Complex32],
        useful_start: usize,
        x0_index: u64,
        cells: &mut [Complex32],
    ) -> Result<(), ModemError> {
        let n = self.layout.nfft();
        let used = self.layout.used();
        if cells.len() < used {
            return Err(ModemError::OutputTooShort { need: used });
        }
        let start = useful_start
            .checked_sub(self.backoff)
            .ok_or(ModemError::InputTooShort { need: self.backoff })?;
        if x.len() < start + n {
            return Err(ModemError::InputTooShort { need: start + n });
        }
        let abs = (x0_index + start as u64) as f64;
        self.last_abs = abs;
        // Phase -freq * abs (or from the last continuous retune), reduced in f64 so long runs stay exact.
        let p0 = if self.phase_ref == 0.0 && self.ref_abs == 0.0 {
            -(self.freq * abs).rem_euclid(1.0)
        } else {
            (self.phase_ref - self.freq * (abs - self.ref_abs)).rem_euclid(1.0)
        };
        rotator(p0, -self.freq).mix(&x[start..start + n], &mut self.buf);
        self.fft.process(&mut self.buf);
        for (i, (c, r)) in cells[..used].iter_mut().zip(&self.ramp).enumerate() {
            *c = self.buf[self.layout.bin(i)] * r;
        }
        Ok(())
    }

    /// The FFT output of the last symbol, natural order, scaled by nothing (for [`super::integer_cfo`]).
    pub fn bins(&self) -> &[Complex32] {
        &self.buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ofdm::{OfdmLayout, PilotPattern};

    fn modulate(layout: &OfdmLayout, cells: &[Complex32]) -> Vec<Complex32> {
        let n = layout.nfft();
        let mut f = vec![Complex32::default(); n];
        for (i, &c) in cells.iter().enumerate() {
            f[layout.bin(i)] = c;
        }
        let mut ifft = sdrkit_algo_dsp::fft::Fft::inverse(n);
        ifft.process(&mut f);
        let s = 1.0 / (n as f32).sqrt();
        let mut x: Vec<Complex32> = f[n - layout.cp(0)..].to_vec();
        x.extend_from_slice(&f);
        x.iter_mut().for_each(|v| *v *= s);
        x
    }

    #[test]
    fn early_window_offset_and_timing_are_removed() {
        let layout = OfdmLayout::new(64, &[16], -26, 26, &[0], &[], &PilotPattern::none()).unwrap();
        let cells: Vec<Complex32> = (0..53)
            .map(|i| {
                if i == 26 {
                    Complex32::default()
                } else {
                    Complex32::new(((i * 7) % 5) as f32 - 2.0, ((i * 3) % 4) as f32 - 1.5)
                }
            })
            .collect();
        let mut x = vec![Complex32::default(); 10];
        x.extend(modulate(&layout, &cells));
        x.extend(vec![Complex32::default(); 10]);
        // A carrier offset of 0.37 carriers, with x[0] at absolute sample 1000.
        let f = 0.37 / 64.0;
        for (i, v) in x.iter_mut().enumerate() {
            let ph = std::f64::consts::TAU * f * (1000 + i) as f64;
            *v *= Complex32::new(ph.cos() as f32, ph.sin() as f32);
        }
        for backoff in [0usize, 4, 16] {
            let mut fr = FftFramer::new(&layout, backoff).unwrap();
            fr.set_freq(f);
            let mut out = vec![Complex32::default(); 53];
            fr.demodulate(&x, 10 + 16, 1000, &mut out).unwrap();
            // Up to the common phase of the offset at the window start, the cells come back.
            let cpe = out[1] / cells[1];
            assert!((cpe.norm() - 1.0).abs() < 1e-3, "{backoff}");
            for i in 0..53 {
                assert!(
                    (out[i] - cells[i] * cpe).norm() < 2e-3,
                    "backoff {backoff} cell {i}"
                );
            }
        }
        // A symbol 2 samples late, told to the framer, also comes back.
        let mut late = vec![Complex32::default(); 12];
        late.extend(modulate(&layout, &cells));
        late.extend(vec![Complex32::default(); 10]);
        let mut fr = FftFramer::new(&layout, 4).unwrap();
        fr.set_timing(2.0);
        let mut out = vec![Complex32::default(); 53];
        fr.demodulate(&late, 10 + 16, 0, &mut out).unwrap();
        for i in 0..53 {
            assert!((out[i] - cells[i]).norm() < 2e-3, "cell {i}");
        }
        assert!(fr.demodulate(&late, 2, 0, &mut out).is_err());
        assert!(fr.demodulate(&late[..40], 26, 0, &mut out).is_err());
        assert!(FftFramer::new(&layout, 17).is_err());
    }

    #[test]
    fn a_continuous_retune_turns_later_symbols_only_by_the_new_offset() {
        let layout = OfdmLayout::new(64, &[16], -26, 26, &[0], &[], &PilotPattern::none()).unwrap();
        let cells: Vec<Complex32> = (0..53)
            .map(|i| {
                if i == 26 {
                    Complex32::default()
                } else {
                    Complex32::from_polar(
                        1.0,
                        std::f32::consts::FRAC_PI_2 * ((i * i * 7 + 3 * i) % 4) as f32 + 0.785,
                    )
                }
            })
            .collect();
        // Twenty symbols with an offset of 0.2 carriers, the stream starting at absolute sample 100 000.
        let f = 0.2 / 64.0;
        let x0 = 100_000u64;
        let mut x = Vec::new();
        for _ in 0..20 {
            x.extend(modulate(&layout, &cells));
        }
        for (i, v) in x.iter_mut().enumerate() {
            let ph = std::f64::consts::TAU * ((f * (x0 + i as u64) as f64) % 1.0);
            *v *= Complex32::new(ph.cos() as f32, ph.sin() as f32);
        }
        // Common phase of symbol `s` against the cells sent.
        let cpe = |fr: &mut FftFramer, s: usize| -> f64 {
            let mut out = vec![Complex32::default(); 53];
            fr.demodulate(&x, s * 80 + 16, x0, &mut out).unwrap();
            let c: Complex32 = out.iter().zip(&cells).map(|(o, c)| o * c.conj()).sum();
            (c.im as f64).atan2(c.re as f64)
        };
        // A retune by delta between symbols 4 and 5: the continuous form turns symbol 5 by -2 pi delta 80 against symbol 4, the
        // absolute form by -2 pi delta (x0 + 416), the start of its window. Both also turn it by the mean of the ramp the wrong offset leaves over the
        // window, -2 pi delta 31.5.
        let delta = 1e-4;
        let wrap = |a: f64| {
            (a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
        };
        let mut fr = FftFramer::new(&layout, 0).unwrap();
        fr.set_freq(f);
        let a = cpe(&mut fr, 4);
        fr.set_freq_continuous(f + delta);
        let b = cpe(&mut fr, 5);
        let want = -std::f64::consts::TAU * delta * (80.0 + 31.5);
        assert!(wrap(b - a - want).abs() < 2e-3, "{} {want}", b - a);
        let mut fr = FftFramer::new(&layout, 0).unwrap();
        fr.set_freq(f);
        let a = cpe(&mut fr, 4);
        fr.set_freq(f + delta);
        let b = cpe(&mut fr, 5);
        let jump = -std::f64::consts::TAU * delta * ((x0 + 5 * 80 + 16) as f64 + 31.5);
        assert!(wrap(b - a - jump).abs() < 2e-3, "{} {jump}", b - a);
        // Back to the same offset: no change.
        let mut fr = FftFramer::new(&layout, 0).unwrap();
        fr.set_freq(f);
        let a = cpe(&mut fr, 2);
        fr.set_freq_continuous(f);
        assert!(wrap(cpe(&mut fr, 2) - a).abs() < 1e-6);
    }
}
