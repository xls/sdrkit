//! Telegram splitting: a frame sent as short radio bursts spread in time and frequency by a known pattern. [`PilotSearch`]
//! finds the bursts by the waveform of their pilot; [`fits`] lists the patterns, starts and frequencies that explain enough
//! of the bursts found, the best first.
//!
//! [`PilotSearch`] takes a window of the template length every [`PilotConfig::step`] samples, multiplies it by the conjugate of
//! the template and transforms it with an FFT of [`PilotConfig::fft`] points. A bin over max([`PilotConfig::over_mean`] × the
//! mean bin power, [`PilotConfig::over_peak`] × the largest bin) that is the largest within [`PilotConfig::peak_bins`] is a
//! candidate at the window centre and the bin frequency. Candidates within [`PilotConfig::merge_samples`] and
//! [`PilotConfig::merge_hz`] merge (the stronger stays), a much weaker candidate near a stronger one is dropped
//! ([`PilotConfig::shadow`]), and a candidate older than [`PilotConfig::final_samples`] is a detection.
//!
//! ```
//! use sdrkit_algo_modem::tsma::{fits, Detection, FitConfig, Pattern};
//! let p = Pattern { carriers: vec![0, 3, 1], times: vec![0.0, 100.0, 250.0] };
//! let dets: Vec<Detection> = [(1000.0, 50.0), (1100.0, 350.0), (1250.0, 150.0)]
//!     .iter()
//!     .map(|&(t, hz)| Detection { t, hz, power: 1.0 })
//!     .collect();
//! let cfg = FitConfig { spacing_hz: 100.0, time_tol: 5.0, hz_tol: 20.0, min_score: 3 };
//! let fit = fits(&dets, &[false; 3], &[p], &cfg, 0).remove(0);
//! assert_eq!((fit.score, fit.t0, fit.f0), (3, 1000.0, 50.0));
//! ```

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fft::Fft;

/// The settings of a [`PilotSearch`].
#[derive(Clone, Debug, PartialEq)]
pub struct PilotConfig {
    /// The pilot waveform, at the input rate.
    pub template: Vec<Complex32>,
    /// FFT points: a power of two at least the template length.
    pub fft: usize,
    /// Samples between two windows.
    pub step: usize,
    /// A peak exceeds this many times the mean bin power.
    pub over_mean: f32,
    /// A peak exceeds this share of the largest bin.
    pub over_peak: f32,
    /// A peak is the largest within this many bins.
    pub peak_bins: usize,
    /// Candidates closer in time merge, samples.
    pub merge_samples: f64,
    /// Candidates closer in frequency merge, Hz.
    pub merge_hz: f64,
    /// A candidate within [`PilotConfig::merge_samples`] of a stronger one at another frequency, and weaker than this share of
    /// it, is dropped (the spread of a strong burst through a window that holds only part of its pilot). 0 keeps them all.
    pub shadow: f32,
    /// A candidate this old is final, samples.
    pub final_samples: f64,
    /// The sample rate, Hz.
    pub rate_hz: f64,
}

/// A burst found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detection {
    /// The stream index of the window centre (the pilot centre), samples.
    pub t: f64,
    /// The frequency, Hz from the centre of the samples.
    pub hz: f64,
    /// The peak power of its bin.
    pub power: f32,
}

/// The pilot search.
pub struct PilotSearch {
    cfg: PilotConfig,
    conj: Vec<Complex32>,
    fft: Fft,
    buf: Vec<Complex32>,
    hist: Vec<Complex32>,
    /// The stream index of `hist[0]`.
    hist_start: u64,
    /// The stream index of the start of the next window.
    next: u64,
    power: Vec<f32>,
    open: Vec<Detection>,
}

impl PilotSearch {
    /// A search with settings `cfg`. Errors for an empty template, an FFT size that is not a power of two at least its length,
    /// or a step of 0.
    pub fn new(cfg: PilotConfig) -> Result<PilotSearch, ModemError> {
        let n = cfg.template.len();
        if n == 0 {
            return Param::Template.degenerate();
        }
        if !cfg.fft.is_power_of_two() || cfg.fft < n {
            return Param::FftSize.range(n as f64, 65_536.0, cfg.fft as f64);
        }
        if cfg.step == 0 {
            return Param::Step.range(1.0, n as f64, 0.0);
        }
        Param::Rate.check_positive(cfg.rate_hz)?;
        Ok(PilotSearch {
            conj: cfg.template.iter().map(|z| z.conj()).collect(),
            fft: Fft::forward(cfg.fft),
            buf: vec![Complex32::default(); cfg.fft],
            hist: Vec::with_capacity(4 * n),
            hist_start: 0,
            next: 0,
            power: vec![0.0; cfg.fft],
            open: Vec::with_capacity(64),
            cfg,
        })
    }

    /// The settings.
    pub fn config(&self) -> &PilotConfig {
        &self.cfg
    }

    /// Forgets the stream and the open candidates.
    pub fn reset(&mut self) {
        self.hist.clear();
        self.hist_start = 0;
        self.next = 0;
        self.open.clear();
    }

    /// Takes `x`; each final detection goes to `found`, in order of time.
    pub fn process(&mut self, x: &[Complex32], mut found: impl FnMut(Detection)) {
        let n = self.conj.len();
        self.hist.extend_from_slice(x);
        let end = self.hist_start + self.hist.len() as u64;
        while self.next + n as u64 <= end {
            let from = (self.next - self.hist_start) as usize;
            self.window(from);
            self.next += self.cfg.step as u64;
            let centre = self.next as f64 - self.cfg.step as f64 + n as f64 / 2.0;
            let mut i = 0;
            while i < self.open.len() {
                if centre - self.open[i].t > self.cfg.final_samples {
                    found(self.open.remove(i));
                } else {
                    i += 1;
                }
            }
        }
        let keep_from = (self.next - self.hist_start) as usize;
        if keep_from > 0 {
            self.hist.drain(..keep_from.min(self.hist.len()));
            self.hist_start = self.next;
        }
    }

    /// Ends the stream: every open candidate is final.
    pub fn finish(&mut self, mut found: impl FnMut(Detection)) {
        for d in self.open.drain(..) {
            found(d);
        }
    }

    fn window(&mut self, from: usize) {
        let n = self.conj.len();
        let m = self.cfg.fft;
        for (b, (x, c)) in self
            .buf
            .iter_mut()
            .zip(self.hist[from..from + n].iter().zip(&self.conj))
        {
            *b = x * c;
        }
        self.buf[n..].fill(Complex32::default());
        self.fft.process(&mut self.buf);
        let mut sum = 0.0f32;
        let mut top = 0.0f32;
        for (p, z) in self.power.iter_mut().zip(&self.buf) {
            *p = z.norm_sqr();
            sum += *p;
            top = top.max(*p);
        }
        let thr = (self.cfg.over_mean * sum / m as f32).max(self.cfg.over_peak * top);
        let t = (self.hist_start + from as u64) as f64 + n as f64 / 2.0;
        let span = self.cfg.peak_bins as isize;
        for k in 0..m {
            let p = self.power[k];
            if p <= thr {
                continue;
            }
            let largest = (-span..=span).all(|d| {
                d == 0 || self.power[(k as isize + d).rem_euclid(m as isize) as usize] < p
            });
            if !largest {
                continue;
            }
            let s = if k < m / 2 {
                k as f64
            } else {
                k as f64 - m as f64
            };
            let hz = s * self.cfg.rate_hz / m as f64;
            self.candidate(Detection { t, hz, power: p });
        }
    }

    fn candidate(&mut self, d: Detection) {
        let near = self.cfg.merge_samples;
        let shadow = self.cfg.shadow;
        if self
            .open
            .iter()
            .any(|o| (o.t - d.t).abs() <= near && d.power < shadow * o.power)
        {
            return;
        }
        self.open
            .retain(|o| !((o.t - d.t).abs() <= near && o.power < shadow * d.power));
        for o in &mut self.open {
            if (o.t - d.t).abs() <= near && (o.hz - d.hz).abs() <= self.cfg.merge_hz {
                if d.power > o.power {
                    *o = d;
                }
                return;
            }
        }
        self.open.push(d);
    }
}

/// A pattern: for each burst, its carrier and its time after burst 0, samples.
#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    /// The carrier of each burst.
    pub carriers: Vec<u16>,
    /// The time of each burst after burst 0, samples (the first is 0).
    pub times: Vec<f64>,
}

/// The settings of [`fits`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FitConfig {
    /// The carrier spacing, Hz.
    pub spacing_hz: f64,
    /// A detection this near the expected time matches, samples.
    pub time_tol: f64,
    /// A detection this near the expected frequency matches, Hz.
    pub hz_tol: f64,
    /// The fewest matches of a fit.
    pub min_score: usize,
}

/// A pattern, start and frequency that explain some detections.
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
    /// The index of the pattern.
    pub pattern: usize,
    /// The time of burst 0, samples.
    pub t0: f64,
    /// The frequency of carrier 0, Hz.
    pub f0: f64,
    /// For each burst of the pattern, the index of the detection that matches it.
    pub matched: Vec<Option<usize>>,
    /// The number of matches.
    pub score: usize,
}

/// The detection of `dets` (sorted by time) near `t` and `hz`, not used, nearest in time.
fn near(dets: &[Detection], used: &[bool], t: f64, hz: f64, cfg: &FitConfig) -> Option<usize> {
    let lo = dets.partition_point(|d| d.t < t - cfg.time_tol);
    dets[lo..]
        .iter()
        .enumerate()
        .take_while(|(_, d)| d.t <= t + cfg.time_tol)
        .filter(|&(i, d)| !used[lo + i] && (d.hz - hz).abs() <= cfg.hz_tol)
        .min_by(|a, b| (a.1.t - t).abs().total_cmp(&(b.1.t - t).abs()))
        .map(|(i, _)| lo + i)
}

/// The fit of pattern `pi` that puts burst `s` on detection `a`.
fn fit_at(
    dets: &[Detection],
    used: &[bool],
    patterns: &[Pattern],
    cfg: &FitConfig,
    a: usize,
    pi: usize,
    s: usize,
) -> Option<Fit> {
    let p = &patterns[pi];
    let d = &dets[a];
    let t0 = d.t - p.times[s];
    let f0 = d.hz - f64::from(p.carriers[s]) * cfg.spacing_hz;
    let at = |k: usize| {
        let t = t0 + p.times[k];
        let hz = f0 + f64::from(p.carriers[k]) * cfg.spacing_hz;
        near(dets, used, t, hz, cfg)
    };
    // Count first: most hypotheses fall short and need no list of matches.
    let score = (0..p.carriers.len()).filter(|&k| at(k).is_some()).count();
    if score < cfg.min_score {
        return None;
    }
    Some(Fit {
        pattern: pi,
        t0,
        f0,
        matched: (0..p.carriers.len()).map(at).collect(),
        score,
    })
}

/// Every fit of `patterns` that reaches [`FitConfig::min_score`], the best first, with each unused detection from index `from`
/// on (sorted by time) tried as each burst of each pattern. Fits with the same matches are listed once.
pub fn fits(
    dets: &[Detection],
    used: &[bool],
    patterns: &[Pattern],
    cfg: &FitConfig,
    from: usize,
) -> Vec<Fit> {
    let mut out: Vec<Fit> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for a in from..dets.len() {
        if used[a] {
            continue;
        }
        for (pi, p) in patterns.iter().enumerate() {
            for s in 0..p.carriers.len() {
                let Some(fit) = fit_at(dets, used, patterns, cfg, a, pi, s) else {
                    continue;
                };
                if seen.insert((pi, fit.matched.clone())) {
                    out.push(fit);
                }
            }
        }
    }
    out.sort_by_key(|f| std::cmp::Reverse(f.score));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pilot_is_found_at_its_time_and_frequency() {
        let n = 64;
        let mut r = 0x1234_5678u32;
        let template: Vec<Complex32> = (0..n)
            .map(|_| {
                r = r.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                Complex32::new(if r >> 31 == 1 { 1.0 } else { -1.0 }, 0.0)
            })
            .collect();
        let rate = 10_000.0;
        let mut x = vec![Complex32::default(); 4000];
        let f = 625.0;
        for i in 0..n {
            let ph = std::f32::consts::TAU * f / rate as f32 * (1000 + i) as f32;
            x[1000 + i] = template[i] * Complex32::new(ph.cos(), ph.sin());
        }
        let mut s = PilotSearch::new(PilotConfig {
            template,
            fft: 256,
            step: 4,
            over_mean: 18.0,
            over_peak: 0.1,
            peak_bins: 4,
            merge_samples: 64.0,
            merge_hz: 300.0,
            shadow: 0.25,
            final_samples: 128.0,
            rate_hz: rate,
        })
        .unwrap();
        let mut got = Vec::new();
        s.process(&x, |d| got.push(d));
        s.finish(|d| got.push(d));
        assert_eq!(got.len(), 1, "{got:?}");
        assert!((got[0].t - 1032.0).abs() <= 2.0 && (got[0].hz - f64::from(f)).abs() < 1.0);
    }

    #[test]
    fn used_detections_and_earlier_anchors_are_left_out() {
        let p = Pattern {
            carriers: vec![0, 2, 1, 3],
            times: vec![0.0, 50.0, 120.0, 200.0],
        };
        let dets: Vec<Detection> = [(500.0, 0.0), (550.0, 200.0), (620.0, 100.0), (700.0, 300.0)]
            .iter()
            .map(|&(t, hz)| Detection { t, hz, power: 1.0 })
            .collect();
        let cfg = FitConfig {
            spacing_hz: 100.0,
            time_tol: 3.0,
            hz_tol: 10.0,
            min_score: 3,
        };
        let all = fits(&dets, &[false; 4], std::slice::from_ref(&p), &cfg, 0);
        assert_eq!(all[0].score, 4);
        let some_used = [true, true, false, false];
        assert!(fits(&dets, &some_used, std::slice::from_ref(&p), &cfg, 0).is_empty());
        assert!(fits(&dets, &[false; 4], &[p], &cfg, 4).is_empty());
    }
}
