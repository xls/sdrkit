//! Sync correlators in the sample and symbol domains.
//!
//! - [`SampleSync`] finds a known complex waveform (a preamble, a PSS, a training sequence, a chirp) in a stream by FFT fast
//!   correlation (overlap-save, plans and buffers held), for several frequency hypotheses at once: a hypothesis is a shift of the
//!   template's spectrum by whole bins, so `S` hypotheses cost one forward FFT and `S` inverse FFTs per hop. Each position gets
//!   the **normalised metric** `|sum x[n+k] t*[k]|^2 / (E_t E_x(n))` in `[0, 1]` (Cauchy-Schwarz), with the window energy
//!   `E_x(n)` from prefix sums in `f64` over the current FFT frame (exact, and no precision lost late in a long capture). For
//!   white Gaussian noise the metric at one position follows Beta(1, L - 1): `P(metric > g) = (1 - g)^(L - 1)` for a template of
//!   `L` samples, which sets the false-alarm rate of a threshold.
//! - [`SoftCorrelator`] finds a pattern of levels in a stream of real values (discriminator output, soft symbols) at `sps`
//!   samples per symbol by Pearson correlation, which ignores gain and offset, and fits that gain and offset (the level and centre
//!   a 2- or 4-level slicer needs).
//!
//! Peaks are reported by one rule: a position counts when its metric reaches the threshold, and it is reported when no larger
//! value follows within the guard distance. [`SampleSync`] can also require a peak to stand `floor_ratio` times above the mean
//! metric of its FFT frame; the threshold `max(threshold, floor_ratio x mean)` is the same for every peak, first or later, so a
//! second peak never passes on a lower bar. The fractional position comes from a parabola through the peak and its neighbours.
//!
//! - [`Autocorrelation`] computes the autocorrelation of a whole capture at every lag up to a limit, in one FFT pair: a cyclic prefix
//!   or a repeated preamble shows as a peak at its lag (the FFT size of an unknown OFDM signal).
//!
//! Sync words in hard or soft **bits** are matched by `sdrkit_util_bits::sync` (`SyncWord` with allowed errors in either polarity,
//! `SoftSync` over LLRs); this module works before bits exist.
//!
//! ```
//! use sdrkit_algo_modem::correlate::SampleSync;
//! use sdrkit_algo_modem::Complex32;
//! // A 31-chip pattern, hidden at sample 300 of a stream.
//! let t: Vec<Complex32> = (0..31).map(|k| Complex32::new(if (k * k + 3 * k) % 7 < 3 { 1.0 } else { -1.0 }, 0.0)).collect();
//! let mut x = vec![Complex32::default(); 1000];
//! x[300..331].copy_from_slice(&t);
//! let mut s = SampleSync::new(&t, 128, &[0], 0.8).unwrap();
//! let mut peaks = Vec::new();
//! s.process(&x, &mut peaks);
//! s.finish(&mut peaks);
//! assert_eq!(peaks.len(), 1);
//! assert_eq!(peaks[0].index, 300);
//! ```

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::peak::parabolic;

/// The peak picker: a candidate above the threshold is kept until no larger value follows within `guard` positions.
#[derive(Clone, Debug)]
struct Picker<P: Copy> {
    guard: u64,
    cand: Option<Cand<P>>,
    prev: Option<(u64, f32)>,
}

#[derive(Clone, Copy, Debug)]
struct Cand<P: Copy> {
    idx: u64,
    v: f32,
    left: f32,
    right: Option<f32>,
    p: P,
}

/// A reported peak: index, value, fractional offset, payload.
type Picked<P> = (u64, f32, f64, P);

impl<P: Copy> Picker<P> {
    fn new(guard: u64) -> Self {
        Picker {
            guard,
            cand: None,
            prev: None,
        }
    }

    fn reset(&mut self) {
        self.cand = None;
        self.prev = None;
    }

    fn emit(c: Cand<P>) -> Picked<P> {
        let pk = parabolic(c.left as f64, c.v as f64, c.right.unwrap_or(0.0) as f64);
        (c.idx, c.v, pk.offset, c.p)
    }

    /// Value `v` at position `idx` (increasing); positions under `thr` can still be neighbours. Returns a peak when one is final.
    #[inline]
    fn push(&mut self, idx: u64, v: f32, thr: f32, p: P) -> Option<Picked<P>> {
        let mut out = None;
        if let Some(c) = &mut self.cand {
            if c.right.is_none() && idx == c.idx + 1 {
                c.right = Some(v);
            }
            if idx > c.idx + self.guard {
                out = self.cand.take().map(Self::emit);
            }
        }
        if v >= thr && self.cand.is_none_or_less(v) {
            let left = match self.prev {
                Some((i, pv)) if i + 1 == idx => pv,
                _ => 0.0,
            };
            self.cand = Some(Cand {
                idx,
                v,
                left,
                right: None,
                p,
            });
        }
        self.prev = Some((idx, v));
        out
    }

    fn finish(&mut self) -> Option<Picked<P>> {
        self.cand.take().map(Self::emit)
    }
}

trait LessThan {
    fn is_none_or_less(&self, v: f32) -> bool;
}

impl<P: Copy> LessThan for Option<Cand<P>> {
    #[inline]
    fn is_none_or_less(&self, v: f32) -> bool {
        match self {
            None => true,
            Some(c) => v > c.v,
        }
    }
}

/// A match found by [`SampleSync`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyncPeak {
    /// The stream index of the sample where the template starts (counted from the first sample given since `new` or `reset`).
    pub index: u64,
    /// The fractional correction to `index`, in `[-0.5, 0.5]` samples (the parabola through the metric).
    pub offset: f32,
    /// The frequency hypothesis, in FFT bins.
    pub shift: i32,
    /// The signal's frequency relative to the template, in cycles per sample (`shift / fft_size`).
    pub freq: f64,
    /// The normalised metric, 0 to 1.
    pub metric: f32,
    /// The correlation `sum x[n+k] conj(t[k] e^(j 2 pi freq k))` at the peak: its phase is the carrier phase there.
    pub value: Complex32,
}

/// FFT fast correlation of a complex template over a stream, with frequency hypotheses (module documentation).
pub struct SampleSync {
    l: usize,
    n: usize,
    hop: usize,
    tspec: Vec<Complex32>,
    et: f64,
    shifts: Vec<i32>,
    fwd: Fft,
    inv: Fft,
    buf: Vec<Complex32>,
    fill: usize,
    base: u64,
    spec: Vec<Complex32>,
    work: Vec<Complex32>,
    prefix: Vec<f64>,
    best: Vec<(f32, u16, Complex32)>,
    threshold: f32,
    floor_ratio: f32,
    picker: Picker<(u16, Complex32)>,
}

impl std::fmt::Debug for SampleSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleSync")
            .field("template", &self.l)
            .field("fft_size", &self.n)
            .field("shifts", &self.shifts)
            .field("threshold", &self.threshold)
            .finish()
    }
}

impl SampleSync {
    /// A correlator for `template` (at least 2 samples, some energy), with FFTs of `fft_size` points (a power of two, at least twice
    /// the template), the frequency hypotheses `shifts` in bins (each below `fft_size / 2` in size; at least one, at most 256), and a
    /// metric threshold in `(0, 1]`. Peaks closer than the template length to a larger one are suppressed.
    pub fn new(
        template: &[Complex32],
        fft_size: usize,
        shifts: &[i32],
        threshold: f32,
    ) -> Result<Self, ModemError> {
        let l = template.len();
        if l < 2 {
            return Param::Template.at_least(2.0, l as f64);
        }
        if !fft_size.is_power_of_two() || fft_size > 1 << 20 {
            return Param::FftSize.not_allowed(fft_size as f64);
        }
        if fft_size < 2 * l {
            return Param::FftSize.conflict(Param::Template);
        }
        Param::Shifts.check_count(shifts.len(), 1, 256)?;
        if let Some(&s) = shifts
            .iter()
            .find(|s| s.unsigned_abs() as usize >= fft_size / 2)
        {
            let half = (fft_size / 2) as f64;
            return Param::Shift.range(-half, half, s as f64);
        }
        if !(threshold > 0.0 && threshold <= 1.0) {
            return Param::Threshold.range(0.0, 1.0, threshold);
        }
        if template
            .iter()
            .any(|z| !(z.re.is_finite() && z.im.is_finite()))
        {
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        let et: f64 = template.iter().map(|z| z.norm_sqr() as f64).sum();
        if et <= 0.0 {
            return Param::Template.degenerate();
        }
        let mut fwd = Fft::forward(fft_size);
        let inv = Fft::inverse(fft_size);
        let mut tspec = vec![Complex32::default(); fft_size];
        tspec[..l].copy_from_slice(template);
        fwd.process(&mut tspec);
        for v in tspec.iter_mut() {
            *v = v.conj();
        }
        let hop = fft_size - l + 1;
        Ok(SampleSync {
            l,
            n: fft_size,
            hop,
            tspec,
            et,
            shifts: shifts.to_vec(),
            fwd,
            inv,
            buf: vec![Complex32::default(); fft_size],
            fill: 0,
            base: 0,
            spec: vec![Complex32::default(); fft_size],
            work: vec![Complex32::default(); fft_size],
            prefix: vec![0.0; fft_size + 1],
            best: vec![(0.0, 0, Complex32::default()); hop],
            threshold,
            floor_ratio: 0.0,
            picker: Picker::new(l as u64),
        })
    }

    /// Also requires every peak to reach `ratio` times the mean metric of its FFT frame (0 turns this off). Errors unless finite and
    /// at least 0.
    pub fn with_floor_ratio(mut self, ratio: f32) -> Result<Self, ModemError> {
        if !(ratio >= 0.0 && ratio.is_finite()) {
            return Param::FloorRatio.at_least(0.0, ratio);
        }
        self.floor_ratio = ratio;
        Ok(self)
    }

    /// Sets the guard: a peak is suppressed by a larger one within this many samples (the template length by default).
    pub fn with_guard(mut self, guard: usize) -> Self {
        self.picker = Picker::new(guard as u64);
        self
    }

    /// The template length.
    pub fn template_len(&self) -> usize {
        self.l
    }

    /// The FFT size.
    pub fn fft_size(&self) -> usize {
        self.n
    }

    /// New samples consumed per FFT frame (`fft_size - template_len + 1`).
    pub fn hop(&self) -> usize {
        self.hop
    }

    /// The spacing of the frequency hypotheses, in cycles per sample (`1 / fft_size`).
    pub fn bin(&self) -> f64 {
        1.0 / self.n as f64
    }

    /// Forgets the stream: the next sample is index 0.
    pub fn reset(&mut self) {
        self.fill = 0;
        self.base = 0;
        self.picker.reset();
    }

    /// Correlates the frame in `buf` for one shift into `work` (unnormalised correlation at each position, divided by `n`).
    fn correlate(&mut self, shift: i32) {
        let n = self.n;
        // work[k] = spec[k] tspec[(k - shift) mod n], as two contiguous runs.
        let s = shift.rem_euclid(n as i32) as usize;
        let (w_lo, w_hi) = self.work.split_at_mut(s);
        let (x_lo, x_hi) = self.spec.split_at(s);
        for ((w, x), t) in w_hi.iter_mut().zip(x_hi).zip(&self.tspec[..n - s]) {
            *w = x * t;
        }
        for ((w, x), t) in w_lo.iter_mut().zip(x_lo).zip(&self.tspec[n - s..]) {
            *w = x * t;
        }
        self.inv.process(&mut self.work);
    }

    fn frame_prep(&mut self) {
        self.spec.copy_from_slice(&self.buf);
        self.fwd.process(&mut self.spec);
        self.prefix[0] = 0.0;
        for i in 0..self.n {
            self.prefix[i + 1] = self.prefix[i] + self.buf[i].norm_sqr() as f64;
        }
    }

    #[inline]
    fn metric(&self, pos: usize, c: Complex32) -> f32 {
        let ex = self.prefix[pos + self.l] - self.prefix[pos];
        let d = ex * self.et;
        if d > 0.0 {
            ((c.norm_sqr() as f64 / d) as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn run_frame(&mut self, peaks: &mut Vec<SyncPeak>) {
        self.frame_prep();
        let scale = 1.0 / self.n as f32;
        for b in self.best.iter_mut() {
            *b = (0.0, 0, Complex32::default());
        }
        for si in 0..self.shifts.len() {
            self.correlate(self.shifts[si]);
            for p in 0..self.hop {
                let c = self.work[p] * scale;
                let m = self.metric(p, c);
                if m > self.best[p].0 {
                    self.best[p] = (m, si as u16, c);
                }
            }
        }
        let thr = if self.floor_ratio > 0.0 {
            let mean = self.best.iter().map(|b| b.0).sum::<f32>() / self.hop as f32;
            self.threshold.max(self.floor_ratio * mean)
        } else {
            self.threshold
        };
        for p in 0..self.hop {
            let (m, si, c) = self.best[p];
            if let Some(pk) = self.picker.push(self.base + p as u64, m, thr, (si, c)) {
                peaks.push(self.to_peak(pk));
            }
        }
    }

    fn to_peak(&self, (idx, v, off, (si, c)): Picked<(u16, Complex32)>) -> SyncPeak {
        let shift = self.shifts[si as usize];
        SyncPeak {
            index: idx,
            offset: off as f32,
            shift,
            freq: shift as f64 / self.n as f64,
            metric: v,
            value: c,
        }
    }

    /// Streaming: takes any number of samples and appends the peaks that became final to `peaks`. Non-finite samples count as 0.
    pub fn process(&mut self, x: &[Complex32], peaks: &mut Vec<SyncPeak>) {
        let mut i = 0;
        while i < x.len() {
            let take = (self.n - self.fill).min(x.len() - i);
            for (d, &s) in self.buf[self.fill..self.fill + take]
                .iter_mut()
                .zip(&x[i..i + take])
            {
                *d = if s.re.is_finite() && s.im.is_finite() {
                    s
                } else {
                    Complex32::default()
                };
            }
            self.fill += take;
            i += take;
            if self.fill == self.n {
                self.run_frame(peaks);
                self.buf.copy_within(self.hop.., 0);
                self.fill = self.l - 1;
                self.base += self.hop as u64;
            }
        }
    }

    /// Reports the peak still waiting for its guard to pass (at the end of a capture). Positions whose window has not yet filled an
    /// FFT frame are not searched; feed `fft_size` zeros first to flush them.
    pub fn finish(&mut self, peaks: &mut Vec<SyncPeak>) {
        if let Some(pk) = self.picker.finish() {
            peaks.push(self.to_peak(pk));
        }
    }

    /// Burst form: the normalised metric of hypothesis `shift` (bins) at every position of a stored slice, `out[p]` for the window
    /// `x[p .. p + L]`, `p` in `0..=x.len() - L`; returns the number of positions written. Uses the streaming buffers, so it resets
    /// the stream.
    pub fn metrics(
        &mut self,
        x: &[Complex32],
        shift: i32,
        out: &mut [f32],
    ) -> Result<usize, ModemError> {
        if x.len() < self.l {
            return Err(ModemError::InputTooShort { need: self.l });
        }
        if shift.unsigned_abs() as usize >= self.n / 2 {
            return Param::Shift.range(-((self.n / 2) as f64), (self.n / 2) as f64, shift as f64);
        }
        let positions = x.len() - self.l + 1;
        if out.len() < positions {
            return Err(ModemError::OutputTooShort { need: positions });
        }
        self.reset();
        let scale = 1.0 / self.n as f32;
        let mut start = 0;
        while start < positions {
            let end = (start + self.n).min(x.len());
            self.buf.fill(Complex32::default());
            for (d, &s) in self.buf.iter_mut().zip(&x[start..end]) {
                *d = if s.re.is_finite() && s.im.is_finite() {
                    s
                } else {
                    Complex32::default()
                };
            }
            self.frame_prep();
            self.correlate(shift);
            for p in 0..self.hop.min(positions - start) {
                out[start + p] = self.metric(p, self.work[p] * scale);
            }
            start += self.hop;
        }
        Ok(positions)
    }
}

/// A match found by [`SoftCorrelator`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoftPeak {
    /// The stream position of the pattern's first symbol, in samples (fractional: the parabola through the score).
    pub index: f64,
    /// The Pearson correlation, -1 to 1; negative for the inverted pattern.
    pub score: f32,
    /// The least-squares gain: received level per pattern level (negative when inverted).
    pub gain: f32,
    /// The least-squares offset: the received value of pattern level 0 (a DC or frequency offset after a discriminator).
    pub offset: f32,
}

/// Pearson correlation of a pattern of levels against real values at `sps` samples per symbol, with the gain and offset fit
/// (module documentation). Values between samples are read by linear interpolation.
///
/// ```
/// use sdrkit_algo_modem::correlate::SoftCorrelator;
/// // A 4-level pattern at 5 samples per symbol, received at gain 2 with an offset of 0.5.
/// let pat = [3.0f32, -1.0, 1.0, -3.0, 3.0, 3.0, -3.0, 1.0];
/// let x: Vec<f32> = (0..200).map(|n| if (50..90).contains(&n) { pat[(n - 50) / 5] * 2.0 + 0.5 } else { 0.5 }).collect();
/// let mut c = SoftCorrelator::new(&pat, 5.0, 0.9).unwrap();
/// let mut peaks = Vec::new();
/// c.process(&x, &mut peaks);
/// c.finish(&mut peaks);
/// assert_eq!(peaks.len(), 1);
/// assert!((peaks[0].gain - 2.0).abs() < 1e-3 && (peaks[0].offset - 0.5).abs() < 1e-3);
/// ```
#[derive(Clone, Debug)]
pub struct SoftCorrelator {
    tc: Vec<f32>,
    t_mean: f32,
    t_ss: f32,
    pos: Vec<(usize, f32)>,
    span: usize,
    ring: Vec<f32>,
    rpos: usize,
    seen: u64,
    threshold: f32,
    inverted: bool,
    picker: Picker<(f32, f32, f32)>,
}

impl SoftCorrelator {
    /// A correlator for `pattern` (2 to 1024 levels, not all equal) at `sps` samples per symbol (1 to 1000), reporting scores of at
    /// least `threshold` (in `(0, 1]`).
    pub fn new(pattern: &[f32], sps: f64, threshold: f32) -> Result<Self, ModemError> {
        let k = pattern.len();
        Param::Levels.check_count(k, 2, 1024)?;
        Param::Sps.check_in(sps, 1.0, 1000.0)?;
        Param::Threshold.check_above(threshold, 0.0, 1.0)?;
        if pattern.iter().any(|v| !v.is_finite()) {
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        let t_mean = pattern.iter().sum::<f32>() / k as f32;
        let tc: Vec<f32> = pattern.iter().map(|v| v - t_mean).collect();
        let t_ss: f32 = tc.iter().map(|v| v * v).sum();
        if t_ss.is_nan() || t_ss <= 1e-12 {
            return Param::Template.degenerate();
        }
        let pos: Vec<(usize, f32)> = (0..k)
            .map(|j| {
                let p = j as f64 * sps;
                (p.floor() as usize, (p - p.floor()) as f32)
            })
            .collect();
        let span = pos[k - 1].0 + 2;
        let guard = (sps.ceil() as u64).max(1);
        Ok(SoftCorrelator {
            tc,
            t_mean,
            t_ss,
            pos,
            span,
            ring: vec![0.0; 2 * span],
            rpos: 0,
            seen: 0,
            threshold,
            inverted: false,
            picker: Picker::new(guard),
        })
    }

    /// Sets the guard: a peak is suppressed by a larger one within this many samples (one symbol by default).
    pub fn with_guard(mut self, guard: usize) -> Self {
        self.picker = Picker::new(guard as u64);
        self
    }

    /// Also reports the inverted pattern (a negative score).
    pub fn with_inverted(mut self, yes: bool) -> Self {
        self.inverted = yes;
        self
    }

    /// Forgets the stream: the next value is index 0.
    pub fn reset(&mut self) {
        self.ring.fill(0.0);
        self.rpos = 0;
        self.seen = 0;
        self.picker.reset();
    }

    /// Takes one value; returns a peak that became final.
    #[inline]
    pub fn push(&mut self, x: f32) -> Option<SoftPeak> {
        let x = if x.is_finite() { x } else { 0.0 };
        let p = self.rpos;
        self.ring[p] = x;
        self.ring[p + self.span] = x;
        self.rpos = if p + 1 == self.span { 0 } else { p + 1 };
        self.seen += 1;
        if self.seen < self.span as u64 {
            return None;
        }
        // The window, oldest first, starts at stream index seen - span.
        let w = &self.ring[self.rpos..self.rpos + self.span];
        let k = self.tc.len() as f32;
        let (mut s, mut s2, mut st) = (0f32, 0f32, 0f32);
        for (&(i, f), &t) in self.pos.iter().zip(&self.tc) {
            let v = w[i] + (w[i + 1] - w[i]) * f;
            s += v;
            s2 += v * v;
            st += v * t;
        }
        let var = s2 - s * s / k;
        let r = if var > 0.0 {
            st / (var * self.t_ss).sqrt()
        } else {
            0.0
        };
        let gain = st / self.t_ss;
        let offset = s / k - gain * self.t_mean;
        let v = if self.inverted { r.abs() } else { r };
        let start = self.seen - self.span as u64;
        self.picker
            .push(start, v, self.threshold, (r, gain, offset))
            .map(Self::to_peak)
    }

    fn to_peak((idx, _, off, (r, gain, offset)): Picked<(f32, f32, f32)>) -> SoftPeak {
        SoftPeak {
            index: idx as f64 + off,
            score: r,
            gain,
            offset,
        }
    }

    /// Takes any number of values and appends the peaks that became final.
    pub fn process(&mut self, x: &[f32], peaks: &mut Vec<SoftPeak>) {
        for &v in x {
            if let Some(p) = self.push(v) {
                peaks.push(p);
            }
        }
    }

    /// Reports the peak still waiting for its guard to pass.
    pub fn finish(&mut self, peaks: &mut Vec<SoftPeak>) {
        if let Some(p) = self.picker.finish() {
            peaks.push(Self::to_peak(p));
        }
    }
}

/// The autocorrelation of a capture at every lag: `r[l] = sum_i x[i + l] conj(x[i])` for `l = 0 ..= max_lag`, by the FFT of the
/// zero-padded capture (Wiener-Khinchin: the inverse FFT of `|X|^2`), padded so no lag wraps round. `r[0]` is the energy, so
/// `|r[l]| / r[0]` is between 0 and 1, and a periodic part of period `L` produces a peak at `l = L`. Holds its FFT plans and buffer
/// for captures up to the size it was made for.
///
/// ```
/// use sdrkit_algo_modem::correlate::Autocorrelation;
/// use sdrkit_algo_modem::Complex32;
/// // A tone of 1000 samples: |r[l]| / r[0] = 1 - l / 1000.
/// let x: Vec<Complex32> = (0..1000).map(|n| Complex32::from_polar(1.0, 0.3 * n as f32)).collect();
/// let mut a = Autocorrelation::new(1000, 100).unwrap();
/// let mut r = vec![Complex32::default(); 101];
/// a.process(&x, &mut r).unwrap();
/// assert!((r[100].norm() / r[0].norm() - 0.9).abs() < 1e-4);
/// ```
#[derive(Clone, Debug)]
pub struct Autocorrelation {
    max_len: usize,
    max_lag: usize,
    fft: Fft,
    ifft: Fft,
    buf: Vec<Complex32>,
}

impl Autocorrelation {
    /// For captures of up to `max_len` samples and lags up to `max_lag` (below `max_len`, the FFT being the next power of two at or
    /// above `max_len + max_lag`, at most 2^26). Errors for sizes out of range.
    pub fn new(max_len: usize, max_lag: usize) -> Result<Self, ModemError> {
        if max_len == 0 {
            return Param::Input.at_least(1.0, 0.0);
        }
        if max_lag >= max_len {
            return Param::Lag.range(0.0, (max_len - 1) as f64, max_lag as f64);
        }
        let n = (max_len + max_lag).next_power_of_two();
        if n > 1 << 26 {
            return Param::FftSize.range(1.0, (1u64 << 26) as f64, n as f64);
        }
        Ok(Autocorrelation {
            max_len,
            max_lag,
            fft: Fft::forward(n),
            ifft: Fft::inverse(n),
            buf: vec![Complex32::default(); n],
        })
    }

    /// The largest lag.
    pub fn max_lag(&self) -> usize {
        self.max_lag
    }

    /// Writes `r[0 ..= max_lag]` of `x` into `out` (`max_lag + 1` values). A sample that is not finite counts as 0. Errors when `x`
    /// is longer than the size given to [`Autocorrelation::new`] or `out` is too short.
    pub fn process(&mut self, x: &[Complex32], out: &mut [Complex32]) -> Result<(), ModemError> {
        if x.len() > self.max_len {
            return Param::Input.range(0.0, self.max_len as f64, x.len() as f64);
        }
        if out.len() <= self.max_lag {
            return Err(ModemError::OutputTooShort {
                need: self.max_lag + 1,
            });
        }
        self.buf.fill(Complex32::default());
        for (b, &v) in self.buf.iter_mut().zip(x) {
            if v.re.is_finite() && v.im.is_finite() {
                *b = v;
            }
        }
        self.fft.process(&mut self.buf);
        for b in self.buf.iter_mut() {
            *b = Complex32::new(b.norm_sqr(), 0.0);
        }
        self.ifft.process(&mut self.buf);
        let s = 1.0 / self.buf.len() as f32;
        for (o, b) in out[..=self.max_lag].iter_mut().zip(&self.buf) {
            *o = *b * s;
        }
        Ok(())
    }
}

/// Benchmarks: an LTE-style PSS search (128-sample template, 1024-point FFT, 9 frequency hypotheses) at 1.92 MS/s, and a 24-symbol
/// soft sync pattern at 10 samples per symbol at 48 kHz.
pub(crate) fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    use sdrkit_core::bench::measure;
    use sdrkit_core::Benchmark;
    fn noise(n: usize) -> Vec<Complex32> {
        let mut s = 0x9E37_79B9u32;
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                Complex32::new(
                    (s >> 16) as f32 / 32768.0 - 1.0,
                    (s & 0xFFFF) as f32 / 32768.0 - 1.0,
                )
            })
            .collect()
    }
    vec![
        Benchmark {
            name: "modem.sample_sync_128x9_1920k",
            run: |sec| {
                let x = noise(65_536);
                let shifts: Vec<i32> = (-4..=4).collect();
                let mut s = SampleSync::new(&x[..128], 1024, &shifts, 0.5).expect("valid");
                let mut peaks = Vec::with_capacity(1024);
                measure(sec, x.len(), x.len() as f64 / 1.92e6, || {
                    peaks.clear();
                    s.process(&x, &mut peaks);
                })
            },
        },
        Benchmark {
            name: "modem.soft_correlator_24x10_48k",
            run: |sec| {
                let x: Vec<f32> = noise(48_000).iter().map(|z| z.re).collect();
                let pat: Vec<f32> = (0..24).map(|k| [3.0, 1.0, -1.0, -3.0][k * 7 % 4]).collect();
                let mut c = SoftCorrelator::new(&pat, 10.0, 0.85).expect("valid");
                let mut peaks = Vec::with_capacity(1024);
                measure(sec, x.len(), x.len() as f64 / 48e3, || {
                    peaks.clear();
                    c.process(&x, &mut peaks);
                })
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::pulse::Shape;
    use sdrkit_algo_sim::modulate::Linear;
    use sdrkit_algo_sim::rng::Rng;

    #[test]
    fn noise_metric_follows_its_beta_distribution() {
        let mut rng = Rng::new(21);
        let l = 32;
        let t: Vec<Complex32> = (0..l).map(|_| rng.complex_normal(1.0)).collect();
        let x: Vec<Complex32> = (0..300_000).map(|_| rng.complex_normal(2.0)).collect();
        let mut s = SampleSync::new(&t, 256, &[0, 3], 0.5).unwrap();
        let mut m = vec![0f32; x.len()];
        for shift in [0, 3] {
            let n = s.metrics(&x, shift, &mut m).unwrap();
            for g in [0.05f32, 0.1, 0.2] {
                let frac = m[..n].iter().filter(|&&v| v > g).count() as f64 / n as f64;
                let want = (1.0 - g as f64).powi(l - 1);
                assert!((frac / want - 1.0).abs() < 0.08, "g {g}: {frac} vs {want}");
            }
        }
    }

    #[test]
    fn finds_a_shaped_template_with_frequency_and_fractional_timing() {
        // A QPSK preamble at 4 samples per symbol (RRC 0.35, sdrkit-algo-sim), the template at phase 0; the received copy is read from
        // a 40-times finer version 0.3 samples later, carries 3 bins of frequency offset, and is in noise at 0 dB.
        let mut rng = Rng::new(9);
        let syms: Vec<Complex32> = (0..32)
            .map(|_| crate::PointSet::qpsk().points()[rng.below(4) as usize])
            .collect();
        let shape = Shape::Rrc { beta: 0.35 };
        let mut coarse = Vec::new();
        let mut m = Linear::new(shape, 8.0, 4.0).unwrap();
        m.push(&syms, &mut coarse);
        m.flush(&mut coarse);
        let mut fine = Vec::new();
        let mut m = Linear::new(shape, 8.0, 40.0).unwrap();
        m.push(&syms, &mut fine);
        m.flush(&mut fine);
        let template = &coarse[..120];
        let n = 512;
        let (at, frac) = (1000usize, 3usize);
        let mut x: Vec<Complex32> = (0..4000).map(|_| rng.complex_normal(0.25)).collect();
        let f = 3.0 / n as f64;
        for k in 0..130 {
            let z = fine[k * 10 + frac] * 1.0;
            let ph = std::f64::consts::TAU * f * (at + k) as f64;
            x[at + k] += z * Complex32::new(ph.cos() as f32, ph.sin() as f32);
        }
        let shifts: Vec<i32> = (-5..=5).collect();
        let mut s = SampleSync::new(template, n, &shifts, 0.4).unwrap();
        let mut peaks = Vec::new();
        // Cut into odd pieces: the stream form keeps its state.
        for piece in x.chunks(333) {
            s.process(piece, &mut peaks);
        }
        s.process(&vec![Complex32::default(); n], &mut peaks);
        s.finish(&mut peaks);
        assert_eq!(peaks.len(), 1, "{peaks:?}");
        let p = peaks[0];
        assert_eq!(p.shift, 3);
        assert!((p.freq - f).abs() < 1e-12);
        let pos = p.index as f64 + p.offset as f64;
        assert!((pos - (at as f64 - 0.3)).abs() < 0.15, "position {pos}");
        assert!(p.metric > 0.5);
    }

    #[test]
    fn every_peak_meets_the_same_threshold() {
        let mut rng = Rng::new(4);
        let t: Vec<Complex32> = (0..64)
            .map(|_| crate::PointSet::qpsk().points()[rng.below(4) as usize])
            .collect();
        let mut x: Vec<Complex32> = (0..20_000).map(|_| rng.complex_normal(1.0)).collect();
        // A strong copy, a second weaker copy above the threshold, and a third far below it.
        for (at, a) in [(3000usize, 3.0f32), (9000, 1.0), (15000, 0.15)] {
            for k in 0..64 {
                x[at + k] += t[k] * a;
            }
        }
        let mut s = SampleSync::new(&t, 256, &[0], 0.3)
            .unwrap()
            .with_floor_ratio(4.0)
            .unwrap();
        let mut peaks = Vec::new();
        s.process(&x, &mut peaks);
        s.finish(&mut peaks);
        let idx: Vec<u64> = peaks.iter().map(|p| p.index).collect();
        assert_eq!(idx, [3000, 9000], "{peaks:?}");
        assert!(peaks.iter().all(|p| p.metric >= 0.3));
        assert!(SampleSync::new(&t, 100, &[0], 0.3).is_err());
        assert!(SampleSync::new(&t, 256, &[200], 0.3).is_err());
        assert!(SampleSync::new(&t, 256, &[0], 0.0).is_err());
        assert!(s.metrics(&x[..10], 0, &mut [0.0; 10]).is_err());
    }

    #[test]
    fn soft_correlator_finds_levels_in_noise_and_inverted() {
        let mut rng = Rng::new(8);
        let pat: Vec<f32> = (0..24)
            .map(|_| [3.0f32, 1.0, -1.0, -3.0][rng.below(4) as usize])
            .collect();
        let sps = 10.0;
        let mut x: Vec<f32> = (0..3000).map(|_| 0.3 * rng.normal_f32()).collect();
        for (at, g) in [(500usize, 0.5f32), (1800, -0.5)] {
            for (k, &v) in pat.iter().enumerate() {
                for j in 0..10 {
                    x[at + k * 10 + j] += v * g + 0.2;
                }
            }
        }
        let mut c = SoftCorrelator::new(&pat, sps, 0.8)
            .unwrap()
            .with_inverted(true);
        let mut peaks = Vec::new();
        c.process(&x, &mut peaks);
        c.finish(&mut peaks);
        assert_eq!(peaks.len(), 2, "{peaks:?}");
        // Each peak is within a sample of the symbol's start (the correlation is flat over the symbol's 10 samples, so any
        // position inside the plateau is right; the picker takes the first maximum).
        assert!((peaks[0].index - 500.0).abs() <= 9.5 && peaks[0].score > 0.8);
        assert!((peaks[0].gain - 0.5).abs() < 0.05 && (peaks[0].offset - 0.2).abs() < 0.1);
        assert!(peaks[1].score < -0.8 && (peaks[1].gain + 0.5).abs() < 0.05);
        assert!(SoftCorrelator::new(&[1.0, 1.0], 4.0, 0.5).is_err());
        assert!(SoftCorrelator::new(&pat, 0.5, 0.5).is_err());
    }

    #[test]
    fn autocorrelation_at_every_lag() {
        // Against the direct sum, on noise.
        let mut r = sdrkit_algo_sim::rng::Rng::new(3);
        let x: Vec<Complex32> = (0..3000).map(|_| r.complex_normal(1.0)).collect();
        let mut a = Autocorrelation::new(4000, 300).unwrap();
        let mut out = vec![Complex32::default(); 301];
        a.process(&x, &mut out).unwrap();
        for l in [0usize, 1, 17, 300] {
            let want: num_complex::Complex64 = (0..x.len() - l)
                .map(|i| {
                    let p = x[i + l] * x[i].conj();
                    num_complex::Complex64::new(p.re as f64, p.im as f64)
                })
                .sum();
            assert!(
                (out[l].re as f64 - want.re).abs() < 1e-2
                    && (out[l].im as f64 - want.im).abs() < 1e-2,
                "lag {l}: {} {want}",
                out[l]
            );
        }
        // An OFDM-like signal of 64-point symbols with 16-sample prefixes: a peak at lag 64 of about 16 / 80 = 0.2.
        let mut ofdm = Vec::new();
        for _ in 0..40 {
            let body: Vec<Complex32> = (0..64).map(|_| r.complex_normal(1.0)).collect();
            ofdm.extend_from_slice(&body[48..]);
            ofdm.extend_from_slice(&body);
        }
        let mut a = Autocorrelation::new(ofdm.len(), 100).unwrap();
        let mut out = vec![Complex32::default(); 101];
        a.process(&ofdm, &mut out).unwrap();
        let best = (1..=100)
            .max_by(|&p, &q| out[p].norm().total_cmp(&out[q].norm()))
            .unwrap();
        assert_eq!(best, 64);
        assert!((out[64].norm() / out[0].norm() - 0.2).abs() < 0.05);
        assert!(a
            .process(&vec![Complex32::default(); ofdm.len() + 1], &mut out)
            .is_err());
        assert!(a.process(&ofdm, &mut out[..50]).is_err());
        assert!(Autocorrelation::new(100, 100).is_err());
    }
}
