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

/// One template of a correlator: its conjugate spectrum, its energy, the frequency hypotheses, the threshold and the peak
/// picker, with the best hypothesis at each position of a frame.
struct Template {
    l: usize,
    tspec: Vec<Complex32>,
    et: f64,
    shifts: Vec<i32>,
    best: Vec<(f32, u16, Complex32)>,
    threshold: f32,
    floor_ratio: f32,
    picker: Picker<(u16, Complex32)>,
}

impl Template {
    /// The template `template` for FFTs of `fft_size` points made by `fwd`, with the hypotheses `shifts` and the
    /// threshold `threshold` ([`SampleSync::new`] gives the limits).
    fn new(
        template: &[Complex32],
        fft_size: usize,
        fwd: &mut Fft,
        shifts: &[i32],
        threshold: f32,
    ) -> Result<Self, ModemError> {
        let l = template.len();
        if l < 2 {
            return Param::Template.at_least(2.0, l as f64);
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
        let mut tspec = vec![Complex32::default(); fft_size];
        tspec[..l].copy_from_slice(template);
        fwd.process(&mut tspec);
        for v in tspec.iter_mut() {
            *v = v.conj();
        }
        Ok(Template {
            l,
            tspec,
            et,
            shifts: shifts.to_vec(),
            best: Vec::new(),
            threshold,
            floor_ratio: 0.0,
            picker: Picker::new(l as u64),
        })
    }

    fn to_peak(&self, n: usize, (idx, v, off, (si, c)): Picked<(u16, Complex32)>) -> SyncPeak {
        let shift = self.shifts[si as usize];
        SyncPeak {
            index: idx,
            offset: off as f32,
            shift,
            freq: shift as f64 / n as f64,
            metric: v,
            value: c,
        }
    }
}

/// The stream side of a correlator: the FFT plans, the frame of samples, its spectrum and the prefix sums of its energy.
/// Each frame of `n` samples moves on by `hop`.
struct Frames {
    n: usize,
    hop: usize,
    fwd: Fft,
    inv: Fft,
    buf: Vec<Complex32>,
    fill: usize,
    base: u64,
    spec: Vec<Complex32>,
    work: Vec<Complex32>,
    prefix: Vec<f64>,
}

impl Frames {
    /// The frames of `fft_size` points (a power of two up to 2^20) for `templates`, with the forward plan `fwd`. Each
    /// template gets room for the best hypothesis at each position of a frame.
    fn new(fft_size: usize, templates: &mut [Template], fwd: Fft) -> Self {
        let longest = templates.iter().map(|t| t.l).max().unwrap_or(1);
        let hop = fft_size - longest + 1;
        for t in templates.iter_mut() {
            t.best = vec![(0.0, 0, Complex32::default()); hop];
        }
        Frames {
            n: fft_size,
            hop,
            fwd,
            inv: Fft::inverse(fft_size),
            buf: vec![Complex32::default(); fft_size],
            fill: 0,
            base: 0,
            spec: vec![Complex32::default(); fft_size],
            work: vec![Complex32::default(); fft_size],
            prefix: vec![0.0; fft_size + 1],
        }
    }

    /// Checks `fft_size` and returns its forward plan.
    fn plan(fft_size: usize) -> Result<Fft, ModemError> {
        if !fft_size.is_power_of_two() || fft_size > 1 << 20 {
            return Param::FftSize.not_allowed(fft_size as f64);
        }
        Ok(Fft::forward(fft_size))
    }

    /// The stream starts again: the next sample is index 0.
    fn reset(&mut self) {
        self.fill = 0;
        self.base = 0;
    }

    /// The spectrum and the energy prefix sums of the frame in `buf`.
    fn prep(&mut self) {
        self.spec.copy_from_slice(&self.buf);
        self.fwd.process(&mut self.spec);
        self.prefix[0] = 0.0;
        for i in 0..self.n {
            self.prefix[i + 1] = self.prefix[i] + self.buf[i].norm_sqr() as f64;
        }
    }

    /// Correlates the frame with template `t` for one shift into `work` (unnormalised correlation at each position, times
    /// `n`).
    fn correlate(&mut self, t: &Template, shift: i32) {
        let n = self.n;
        // work[k] = spec[k] tspec[(k - shift) mod n], as two contiguous runs.
        let s = shift.rem_euclid(n as i32) as usize;
        let (w_lo, w_hi) = self.work.split_at_mut(s);
        let (x_lo, x_hi) = self.spec.split_at(s);
        for ((w, x), t) in w_hi.iter_mut().zip(x_hi).zip(&t.tspec[..n - s]) {
            *w = x * t;
        }
        for ((w, x), t) in w_lo.iter_mut().zip(x_lo).zip(&t.tspec[n - s..]) {
            *w = x * t;
        }
        self.inv.process(&mut self.work);
    }

    /// The normalised metric of template `t` at position `pos` of the frame for the correlation `c`.
    #[inline]
    fn metric(&self, t: &Template, pos: usize, c: Complex32) -> f32 {
        let ex = self.prefix[pos + t.l] - self.prefix[pos];
        let d = ex * t.et;
        if d > 0.0 {
            ((c.norm_sqr() as f64 / d) as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Searches the frame (after [`Frames::prep`]) for template `t` and appends the peaks that became final to `peaks`.
    fn search(&mut self, t: &mut Template, peaks: &mut Vec<SyncPeak>) {
        let scale = 1.0 / self.n as f32;
        for b in t.best.iter_mut() {
            *b = (0.0, 0, Complex32::default());
        }
        for si in 0..t.shifts.len() {
            self.correlate(t, t.shifts[si]);
            for p in 0..self.hop {
                let c = self.work[p] * scale;
                let m = self.metric(t, p, c);
                if m > t.best[p].0 {
                    t.best[p] = (m, si as u16, c);
                }
            }
        }
        let thr = if t.floor_ratio > 0.0 {
            let mean = t.best.iter().map(|b| b.0).sum::<f32>() / self.hop as f32;
            t.threshold.max(t.floor_ratio * mean)
        } else {
            t.threshold
        };
        for p in 0..self.hop {
            let (m, si, c) = t.best[p];
            if let Some(pk) = t.picker.push(self.base + p as u64, m, thr, (si, c)) {
                peaks.push(t.to_peak(self.n, pk));
            }
        }
    }

    /// Takes the samples of `x` into frames; `frame` runs on each full frame after [`Frames::prep`]. Non-finite samples
    /// count as 0.
    fn stream(&mut self, x: &[Complex32], mut frame: impl FnMut(&mut Frames)) {
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
                self.prep();
                frame(self);
                self.buf.copy_within(self.hop.., 0);
                self.fill = self.n - self.hop;
                self.base += self.hop as u64;
            }
        }
    }
}

/// FFT fast correlation of a complex template over a stream, with frequency hypotheses (module documentation).
pub struct SampleSync {
    f: Frames,
    t: Template,
}

impl std::fmt::Debug for SampleSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleSync")
            .field("template", &self.t.l)
            .field("fft_size", &self.f.n)
            .field("shifts", &self.t.shifts)
            .field("threshold", &self.t.threshold)
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
        if template.len() < 2 {
            return Param::Template.at_least(2.0, template.len() as f64);
        }
        let mut fwd = Frames::plan(fft_size)?;
        let mut t = [Template::new(
            template, fft_size, &mut fwd, shifts, threshold,
        )?];
        let f = Frames::new(fft_size, &mut t, fwd);
        let [t] = t;
        Ok(SampleSync { f, t })
    }

    /// Also requires every peak to reach `ratio` times the mean metric of its FFT frame (0 turns this off). Errors unless finite and
    /// at least 0.
    pub fn with_floor_ratio(mut self, ratio: f32) -> Result<Self, ModemError> {
        if !(ratio >= 0.0 && ratio.is_finite()) {
            return Param::FloorRatio.at_least(0.0, ratio);
        }
        self.t.floor_ratio = ratio;
        Ok(self)
    }

    /// Sets the guard: a peak is suppressed by a larger one within this many samples (the template length by default).
    pub fn with_guard(mut self, guard: usize) -> Self {
        self.t.picker = Picker::new(guard as u64);
        self
    }

    /// The template length.
    pub fn template_len(&self) -> usize {
        self.t.l
    }

    /// The FFT size.
    pub fn fft_size(&self) -> usize {
        self.f.n
    }

    /// New samples consumed per FFT frame (`fft_size - template_len + 1`).
    pub fn hop(&self) -> usize {
        self.f.hop
    }

    /// The spacing of the frequency hypotheses, in cycles per sample (`1 / fft_size`).
    pub fn bin(&self) -> f64 {
        1.0 / self.f.n as f64
    }

    /// Forgets the stream: the next sample is index 0.
    pub fn reset(&mut self) {
        self.f.reset();
        self.t.picker.reset();
    }

    /// Streaming: takes any number of samples and appends the peaks that became final to `peaks`. Non-finite samples count as 0.
    pub fn process(&mut self, x: &[Complex32], peaks: &mut Vec<SyncPeak>) {
        let t = &mut self.t;
        self.f.stream(x, |f| f.search(t, peaks));
    }

    /// Reports the peak still waiting for its guard to pass (at the end of a capture). Positions whose window has not yet filled an
    /// FFT frame are not searched; feed `fft_size` zeros first to flush them.
    pub fn finish(&mut self, peaks: &mut Vec<SyncPeak>) {
        if let Some(pk) = self.t.picker.finish() {
            peaks.push(self.t.to_peak(self.f.n, pk));
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
        let (l, n) = (self.t.l, self.f.n);
        if x.len() < l {
            return Err(ModemError::InputTooShort { need: l });
        }
        if shift.unsigned_abs() as usize >= n / 2 {
            return Param::Shift.range(-((n / 2) as f64), (n / 2) as f64, shift as f64);
        }
        let positions = x.len() - l + 1;
        if out.len() < positions {
            return Err(ModemError::OutputTooShort { need: positions });
        }
        self.reset();
        let scale = 1.0 / n as f32;
        let hop = self.f.hop;
        let mut start = 0;
        while start < positions {
            let end = (start + n).min(x.len());
            let f = &mut self.f;
            f.buf.fill(Complex32::default());
            for (d, &s) in f.buf.iter_mut().zip(&x[start..end]) {
                *d = if s.re.is_finite() && s.im.is_finite() {
                    s
                } else {
                    Complex32::default()
                };
            }
            f.prep();
            f.correlate(&self.t, shift);
            for p in 0..hop.min(positions - start) {
                out[start + p] = f.metric(&self.t, p, f.work[p] * scale);
            }
            start += hop;
        }
        Ok(positions)
    }
}

/// FFT fast correlation of several complex templates over one stream: each frame of the stream has one forward FFT that
/// every template shares, then one inverse FFT per template and hypothesis. Each template finds its peaks as a
/// [`SampleSync`] of its own does; templates of one length find the same peaks, bit for bit. A frame moves on by
/// `fft_size - L + 1` samples for the longest template of `L` samples.
///
/// ```
/// use sdrkit_algo_modem::correlate::SampleSyncSet;
/// use sdrkit_algo_modem::Complex32;
/// let chip = |k: usize, a: usize| Complex32::new(if (k * k + a * k) % 7 < 3 { 1.0 } else { -1.0 }, 0.0);
/// let a: Vec<Complex32> = (0..31).map(|k| chip(k, 3)).collect();
/// let b: Vec<Complex32> = (0..31).map(|k| chip(k, 5)).collect();
/// let mut x = vec![Complex32::default(); 1000];
/// x[300..331].copy_from_slice(&a);
/// x[600..631].copy_from_slice(&b);
/// let mut s = SampleSyncSet::new(&[&a, &b], 128, &[0], 0.8).unwrap();
/// let mut peaks = [Vec::new(), Vec::new()];
/// s.process(&x, &mut peaks);
/// s.finish(&mut peaks);
/// assert_eq!(peaks[0].iter().map(|p| p.index).collect::<Vec<_>>(), [300]);
/// assert_eq!(peaks[1].iter().map(|p| p.index).collect::<Vec<_>>(), [600]);
/// ```
pub struct SampleSyncSet {
    f: Frames,
    t: Vec<Template>,
}

impl std::fmt::Debug for SampleSyncSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleSyncSet")
            .field("templates", &self.t.iter().map(|t| t.l).collect::<Vec<_>>())
            .field("fft_size", &self.f.n)
            .finish()
    }
}

impl SampleSyncSet {
    /// A correlator for `templates` (1 to 64), each as [`SampleSync::new`] takes it, with FFTs of `fft_size` points (at
    /// least twice the longest template), the same frequency hypotheses `shifts` and the same threshold for each.
    pub fn new(
        templates: &[&[Complex32]],
        fft_size: usize,
        shifts: &[i32],
        threshold: f32,
    ) -> Result<Self, ModemError> {
        Param::Template.check_count(templates.len(), 1, 64)?;
        if let Some(short) = templates.iter().find(|t| t.len() < 2) {
            return Param::Template.at_least(2.0, short.len() as f64);
        }
        let mut fwd = Frames::plan(fft_size)?;
        let mut t = templates
            .iter()
            .map(|t| Template::new(t, fft_size, &mut fwd, shifts, threshold))
            .collect::<Result<Vec<_>, _>>()?;
        let f = Frames::new(fft_size, &mut t, fwd);
        Ok(SampleSyncSet { f, t })
    }

    /// Also requires every peak to reach `ratio` times the mean metric of its template in its FFT frame (0 turns this off).
    /// Errors unless finite and at least 0.
    pub fn with_floor_ratio(mut self, ratio: f32) -> Result<Self, ModemError> {
        if !(ratio >= 0.0 && ratio.is_finite()) {
            return Param::FloorRatio.at_least(0.0, ratio);
        }
        for t in &mut self.t {
            t.floor_ratio = ratio;
        }
        Ok(self)
    }

    /// Sets the guard of every template: a peak is suppressed by a larger one within this many samples (the length of its
    /// template by default).
    pub fn with_guard(mut self, guard: usize) -> Self {
        for t in &mut self.t {
            t.picker = Picker::new(guard as u64);
        }
        self
    }

    /// The number of templates.
    pub fn templates(&self) -> usize {
        self.t.len()
    }

    /// The length of template `i`. `None` past the last.
    pub fn template_len(&self, i: usize) -> Option<usize> {
        self.t.get(i).map(|t| t.l)
    }

    /// The FFT size.
    pub fn fft_size(&self) -> usize {
        self.f.n
    }

    /// New samples consumed per FFT frame (`fft_size` less the longest template, plus 1).
    pub fn hop(&self) -> usize {
        self.f.hop
    }

    /// Forgets the stream: the next sample is index 0.
    pub fn reset(&mut self) {
        self.f.reset();
        for t in &mut self.t {
            t.picker.reset();
        }
    }

    /// Streaming: takes any number of samples and appends the peaks of template `i` that became final to `peaks[i]`. A
    /// template without a list in `peaks` is searched and its peaks are dropped. Non-finite samples count as 0.
    pub fn process(&mut self, x: &[Complex32], peaks: &mut [Vec<SyncPeak>]) {
        let ts = &mut self.t;
        self.f.stream(x, |f| {
            for (i, t) in ts.iter_mut().enumerate() {
                match peaks.get_mut(i) {
                    Some(p) => f.search(t, p),
                    None => {
                        let mut none = Vec::new();
                        f.search(t, &mut none);
                    }
                }
            }
        });
    }

    /// Reports the peaks still waiting for their guard to pass (at the end of a capture), each in its list as
    /// [`SampleSyncSet::process`] does.
    pub fn finish(&mut self, peaks: &mut [Vec<SyncPeak>]) {
        for (i, t) in self.t.iter_mut().enumerate() {
            if let Some(pk) = t.picker.finish() {
                if let Some(p) = peaks.get_mut(i) {
                    p.push(t.to_peak(self.f.n, pk));
                }
            }
        }
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
    fn a_set_finds_the_peaks_of_separate_correlators_bit_for_bit() {
        // Two templates of one length and a third shorter one, each hidden twice in noise with a frequency offset.
        let mut rng = Rng::new(21);
        let template = |rng: &mut Rng, len: usize| -> Vec<Complex32> {
            (0..len).map(|_| rng.complex_normal(1.0)).collect()
        };
        let ts = [
            template(&mut rng, 100),
            template(&mut rng, 100),
            template(&mut rng, 61),
        ];
        let mut x: Vec<Complex32> = (0..20_000).map(|_| rng.complex_normal(0.5)).collect();
        for (i, at) in [
            (0, 1_000),
            (1, 4_321),
            (2, 9_000),
            (0, 12_345),
            (1, 15_000),
            (2, 17_777),
        ] {
            for (k, z) in ts[i].iter().enumerate() {
                let ph = std::f64::consts::TAU * 2.0 / 512.0 * (at + k) as f64;
                x[at + k] += z * Complex32::new(ph.cos() as f32, ph.sin() as f32);
            }
        }
        let shifts = [-2, 0, 2];
        let one = |t: &[Complex32]| -> Vec<SyncPeak> {
            let mut s = SampleSync::new(t, 512, &shifts, 0.3).unwrap();
            let mut p = Vec::new();
            for piece in x.chunks(777) {
                s.process(piece, &mut p);
            }
            s.finish(&mut p);
            p
        };
        let run = |ts: &[&[Complex32]]| -> Vec<Vec<SyncPeak>> {
            let mut s = SampleSyncSet::new(ts, 512, &shifts, 0.3).unwrap();
            let mut p = vec![Vec::new(); ts.len()];
            for piece in x.chunks(777) {
                s.process(piece, &mut p);
            }
            s.finish(&mut p);
            p
        };
        // Templates of one length: the same peaks, to the bit.
        let same = run(&[&ts[0], &ts[1]]);
        assert_eq!(same[0], one(&ts[0]));
        assert_eq!(same[1], one(&ts[1]));
        assert_eq!(
            same[0]
                .iter()
                .map(|p| (p.index, p.shift))
                .collect::<Vec<_>>(),
            [(1_000, 2), (12_345, 2)]
        );
        // With a shorter template the frames move on by the hop of the longest: the same places and hypotheses.
        let mixed = run(&[&ts[0], &ts[1], &ts[2]]);
        assert_eq!(mixed[..2], same[..]);
        let alone = one(&ts[2]);
        let places = |p: &[SyncPeak]| p.iter().map(|p| (p.index, p.shift)).collect::<Vec<_>>();
        assert_eq!(places(&mixed[2]), places(&alone));
        assert_eq!(places(&alone), [(9_000, 2), (17_777, 2)]);
        for (a, b) in mixed[2].iter().zip(&alone) {
            assert!((a.metric - b.metric).abs() < 1e-4, "{a:?} {b:?}");
        }
        // A list for one template only: the others are searched and dropped.
        let mut s = SampleSyncSet::new(&[&ts[0], &ts[1]], 512, &shifts, 0.3).unwrap();
        let mut first = [Vec::new()];
        s.process(&x, &mut first);
        s.finish(&mut first);
        assert_eq!(first[0], same[0]);
        assert!(SampleSyncSet::new(&[], 512, &shifts, 0.3).is_err());
        assert!(SampleSyncSet::new(&[&ts[0]], 128, &shifts, 0.3).is_err());
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
