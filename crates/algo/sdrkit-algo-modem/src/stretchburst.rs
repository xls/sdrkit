//! Bursts of one signal of known width anywhere in a band: short FFT frames, the strongest stretch of bins as wide as the
//! signal, and the noise of the same frame.
//!
//! Each frame of [`StretchConfig::frame`] samples is Hann-windowed and transformed. A stretch is `w` adjacent bins inside one
//! of the [`StretchConfig::bands`], `w` = [`StretchConfig::width_hz`] in bins (at least 3). Its **region** starts `w / 2` bins
//! before it and spans `2 w` bins, cut to its band. The noise of a bin is the median of the band bins outside the region over
//! `ln 2` (the median of an exponential power), or of all band bins when fewer than 8 lie outside. The ratio of a frame is the
//! mean power of the measured bins over that noise.
//!
//! - No burst open: the measured bins are the strongest stretch of the frame. [`StretchConfig::open_frames`] frames in a row
//!   with a ratio over [`StretchConfig::open_ratio`] open a burst, which starts [`StretchConfig::pre_frames`] frames before the
//!   first of them, and the stretch of the last fixes the region.
//! - A burst open: the measured bins are the whole region (twice the width: the stretch found at the open may hold one tone
//!   only). [`StretchConfig::close_frames`] frames in a row under [`StretchConfig::close_ratio`] close it, those frames
//!   included; at [`StretchConfig::max_len`] samples it closes and is marked cut.
//!
//! A closed burst of [`StretchConfig::min_len`] samples or more is ready for [`StretchDetector::take`]; a shorter one is
//! dropped. The first [`StretchConfig::settle`] samples of the stream are not analysed.
//!
//! [`crate::bandburst`] decides per bin against a floor learned over time. This detector sums as many bins as the signal
//! covers, so a signal spread over many bins keeps its whole power in the decision, and it takes the noise from the frame
//! itself, so it needs no learning time. Nothing is allocated after [`StretchDetector::new`].
//!
//! ```
//! use sdrkit_algo_modem::stretchburst::{StretchConfig, StretchDetector};
//! use sdrkit_algo_modem::Complex32;
//! let mut d = StretchDetector::new(StretchConfig::new(8000.0, 256, vec![(-3000.0, 3000.0)], 300.0)).unwrap();
//! let mut s = 1u32;
//! let mut u = move || {
//!     s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
//!     1e-2 * ((s >> 8) as f32 / (1u32 << 24) as f32 - 0.5)
//! };
//! let mut x: Vec<Complex32> = (0..16_000).map(|_| Complex32::new(u(), u())).collect();
//! for (i, v) in x[6000..10_000].iter_mut().enumerate() {
//!     let p = std::f32::consts::TAU * 1000.0 / 8000.0 * i as f32;
//!     *v += Complex32::new(0.1 * p.cos(), 0.1 * p.sin());
//! }
//! let mut at = 0;
//! let mut found = Vec::new();
//! while at < x.len() {
//!     at += d.process(&x[at..]);
//!     if let Some(b) = d.take() {
//!         found.push((b.start, b.samples.len(), b.centre_hz));
//!     }
//! }
//! assert_eq!(found.len(), 1);
//! assert!(found[0].0 <= 6000 && found[0].0 + found[0].1 as u64 >= 10_000);
//! assert!((found[0].2 - 1000.0).abs() < 200.0);
//! ```

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::window::Window;
use std::f64::consts::LN_2;

/// The settings of a [`StretchDetector`] (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct StretchConfig {
    /// The sample rate, Hz.
    pub rate_hz: f64,
    /// Samples per FFT frame, without overlap: a power of two from 16 to 8192.
    pub frame: usize,
    /// The bands searched, Hz from the centre of the input (lowest, highest), each inside half the rate.
    pub bands: Vec<(f64, f64)>,
    /// The width of the signal, Hz.
    pub width_hz: f64,
    /// A frame is on over this ratio.
    pub open_ratio: f64,
    /// A frame is off under this ratio (at most `open_ratio`).
    pub close_ratio: f64,
    /// On frames in a row that open a burst (at least 1).
    pub open_frames: u32,
    /// Off frames in a row that close it (at least 1).
    pub close_frames: u32,
    /// Frames given before the first on frame.
    pub pre_frames: usize,
    /// Samples at the start of the stream that are not analysed.
    pub settle: usize,
    /// Shorter bursts are dropped, samples.
    pub min_len: usize,
    /// The longest burst, samples.
    pub max_len: usize,
}

impl StretchConfig {
    /// `bands` at `rate_hz` in frames of `frame` samples for a signal `width_hz` wide: on over 5.5, off under 1.6, two frames
    /// open and three close a burst, three frames before it, no settling, bursts from 2 frames to 10 s.
    ///
    /// Adjacent Hann bins are correlated, so the strongest of many narrow stretches on noise reads high. Measured over
    /// 400 000 frames of noise: a stretch of 10 bins in a band of 215 passes 3 in 8 % of frames, 4.5 in 0.13 % and 5.5 in
    /// 0.0065 % (two frames in a row: about 4e-9 a frame); a stretch of 77 bins in two bands of 78 passes 3 in 0.001 %.
    pub fn new(rate_hz: f64, frame: usize, bands: Vec<(f64, f64)>, width_hz: f64) -> StretchConfig {
        StretchConfig {
            rate_hz,
            frame,
            bands,
            width_hz,
            open_ratio: 5.5,
            close_ratio: 1.6,
            open_frames: 2,
            close_frames: 3,
            pre_frames: 3,
            settle: 0,
            min_len: 2 * frame,
            max_len: (10.0 * rate_hz).round().max(1.0) as usize,
        }
    }
}

/// A finished burst, borrowed from the detector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StretchBurst<'a> {
    /// The samples, from `pre_frames` before the first on frame to the close.
    pub samples: &'a [Complex32],
    /// The stream index of `samples[0]`.
    pub start: u64,
    /// The centre of the stretch found at the open, Hz from the centre of the input.
    pub centre_hz: f64,
    /// The highest ratio of the burst.
    pub peak_ratio: f64,
    /// It reached `max_len` and was closed there.
    pub cut: bool,
}

/// The detector (module documentation).
pub struct StretchDetector {
    cfg: StretchConfig,
    fft: Fft,
    win: Vec<f32>,
    work: Vec<Complex32>,
    frame: Vec<Complex32>,
    /// Bin power in signed order: index `j` is `(j - frame / 2) x bin` Hz.
    power: Vec<f64>,
    /// The signed bins of each band, `(first, last)` inclusive.
    bands: Vec<(usize, usize)>,
    width: usize,
    scratch: Vec<f64>,
    /// The frames before an open, oldest first, at most `keep` samples.
    history: Vec<Complex32>,
    keep: usize,
    burst: Vec<Complex32>,
    open: bool,
    ready: bool,
    region: (usize, usize),
    centre_hz: f64,
    peak: f64,
    cut: bool,
    on: u32,
    off: u32,
    seen: u64,
    start: u64,
}

impl StretchDetector {
    /// A detector with settings `cfg`. Errors for a rate that is not positive, a frame that is not a power of two from 16 to
    /// 8192, no band, a band outside half the rate or narrower than the stretch, a width that is not positive, ratios that
    /// are not finite or a close ratio over the open ratio, zero frame counts, or a `max_len` under `min_len` or over 2^26.
    pub fn new(cfg: StretchConfig) -> Result<StretchDetector, ModemError> {
        Param::Rate.check_positive(cfg.rate_hz)?;
        let n = cfg.frame;
        if !n.is_power_of_two() || !(16..=8192).contains(&n) {
            return Param::FftSize.range(16.0, 8192.0, n as f64);
        }
        Param::Width.check_positive(cfg.width_hz)?;
        Param::OpenLevel.check_finite(cfg.open_ratio)?;
        Param::CloseLevel.check_in(cfg.close_ratio, 0.0, cfg.open_ratio)?;
        Param::Window.check_count(cfg.open_frames as usize, 1, 1 << 16)?;
        Param::Window.check_count(cfg.close_frames as usize, 1, 1 << 16)?;
        Param::MaxLen.check_count(cfg.max_len, cfg.min_len.max(1), 1 << 26)?;
        let bin = cfg.rate_hz / n as f64;
        let width = ((cfg.width_hz / bin).round() as usize).max(3);
        Param::Bins.check_count(cfg.bands.len(), 1, 64)?;
        let mut bands = Vec::with_capacity(cfg.bands.len());
        for &(lo, hi) in &cfg.bands {
            let nyq = cfg.rate_hz / 2.0;
            Param::NoiseBand.check_in(lo, -nyq, hi)?;
            Param::NoiseBand.check_in(hi, lo, nyq)?;
            let a = (lo / bin).ceil() as i64 + (n / 2) as i64;
            let b = (hi / bin).floor() as i64 + (n / 2) as i64;
            let (a, b) = (a.max(0) as usize, (b.max(0) as usize).min(n - 1));
            if b + 1 < a + width {
                return Param::Width.range(
                    0.0,
                    (b + 1).saturating_sub(a) as f64 * bin,
                    cfg.width_hz,
                );
            }
            bands.push((a, b));
        }
        let keep = (cfg.pre_frames + cfg.open_frames as usize - 1) * n;
        Ok(StretchDetector {
            fft: Fft::forward(n),
            win: Window::Hann.coefficients(n),
            work: vec![Complex32::default(); n],
            frame: Vec::with_capacity(n),
            power: vec![0.0; n],
            scratch: Vec::with_capacity(n),
            history: Vec::with_capacity(keep),
            keep,
            burst: Vec::with_capacity(keep + cfg.max_len + n),
            bands,
            width,
            open: false,
            ready: false,
            region: (0, 0),
            centre_hz: 0.0,
            peak: 0.0,
            cut: false,
            on: 0,
            off: 0,
            seen: 0,
            start: 0,
            cfg,
        })
    }

    /// The settings.
    pub fn config(&self) -> &StretchConfig {
        &self.cfg
    }

    /// A burst is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The stream index of the next sample.
    pub fn position(&self) -> u64 {
        self.seen
    }

    /// Forgets the stream and any open or unread burst.
    pub fn reset(&mut self) {
        self.frame.clear();
        self.history.clear();
        self.burst.clear();
        self.open = false;
        self.ready = false;
        self.on = 0;
        self.off = 0;
        self.seen = 0;
    }

    /// Takes samples of `x` until a burst is ready or `x` ends; returns how many were taken. A ready burst that was not read
    /// is dropped by the next call.
    pub fn process(&mut self, x: &[Complex32]) -> usize {
        if self.ready {
            self.ready = false;
            self.burst.clear();
        }
        let n = self.cfg.frame;
        for (i, &s) in x.iter().enumerate() {
            self.seen += 1;
            if self.seen <= self.cfg.settle as u64 {
                continue;
            }
            let s = if s.re.is_finite() && s.im.is_finite() {
                s
            } else {
                Complex32::default()
            };
            self.frame.push(s);
            if self.frame.len() == n {
                self.analyse();
                self.frame.clear();
                if self.ready {
                    return i + 1;
                }
            }
        }
        x.len()
    }

    /// Closes an open burst at the end of the stream; it is ready when it is long enough.
    pub fn end(&mut self) {
        if self.open {
            self.close(false);
        }
    }

    /// The ready burst, once.
    pub fn take(&mut self) -> Option<StretchBurst<'_>> {
        if !self.ready {
            return None;
        }
        self.ready = false;
        Some(StretchBurst {
            samples: &self.burst,
            start: self.start,
            centre_hz: self.centre_hz,
            peak_ratio: self.peak,
            cut: self.cut,
        })
    }

    fn analyse(&mut self) {
        let n = self.cfg.frame;
        for ((w, x), c) in self.work.iter_mut().zip(&self.frame).zip(&self.win) {
            *w = *x * *c;
        }
        self.fft.process(&mut self.work);
        for (j, p) in self.power.iter_mut().enumerate() {
            *p = f64::from(self.work[(j + n / 2) % n].norm_sqr());
        }
        let ratio = if self.open {
            let (a, b) = self.region;
            self.mean(a, b) / self.noise(a, b)
        } else {
            let (a, band) = self.strongest();
            let region = self.region_of(a, band);
            let r = self.mean(a, a + self.width - 1) / self.noise(region.0, region.1);
            if r > self.cfg.open_ratio && self.on + 1 >= self.cfg.open_frames {
                self.region = region;
                self.centre_hz = (a as f64 + (self.width - 1) as f64 / 2.0 - (n / 2) as f64)
                    * self.cfg.rate_hz
                    / n as f64;
            }
            r
        };
        if self.open {
            self.burst.extend_from_slice(&self.frame);
            self.peak = self.peak.max(ratio);
            self.off = if ratio < self.cfg.close_ratio {
                self.off + 1
            } else {
                0
            };
            if self.off >= self.cfg.close_frames {
                self.close(false);
            } else if self.burst.len() >= self.cfg.max_len {
                self.close(true);
            }
            return;
        }
        self.on = if ratio > self.cfg.open_ratio {
            self.on + 1
        } else {
            0
        };
        if self.on >= self.cfg.open_frames {
            self.open = true;
            self.on = 0;
            self.off = 0;
            self.cut = false;
            self.peak = ratio;
            self.burst.clear();
            self.burst.extend_from_slice(&self.history);
            self.burst.extend_from_slice(&self.frame);
            self.start = self.seen - self.burst.len() as u64;
            self.history.clear();
        } else {
            let keep = self.keep;
            if keep > 0 {
                if self.history.len() + n > keep {
                    self.history.drain(..self.history.len() + n - keep);
                }
                self.history.extend_from_slice(&self.frame);
            }
        }
    }

    fn close(&mut self, cut: bool) {
        self.open = false;
        self.off = 0;
        self.cut = cut;
        if self.burst.len() >= self.cfg.min_len {
            self.ready = true;
        } else {
            self.burst.clear();
        }
    }

    /// The first bin of the strongest stretch and its band.
    fn strongest(&self) -> (usize, usize) {
        let w = self.width;
        let mut best = (f64::NEG_INFINITY, 0, 0);
        for (k, &(lo, hi)) in self.bands.iter().enumerate() {
            let mut sum: f64 = self.power[lo..lo + w].iter().sum();
            for a in lo..=hi + 1 - w {
                if a > lo {
                    sum += self.power[a + w - 1] - self.power[a - 1];
                }
                if sum > best.0 {
                    best = (sum, a, k);
                }
            }
        }
        (best.1, best.2)
    }

    /// The region of the stretch at `a` in band `band`, inclusive.
    fn region_of(&self, a: usize, band: usize) -> (usize, usize) {
        let (lo, hi) = self.bands[band];
        let first = a.saturating_sub(self.width / 2).max(lo);
        (first, (first + 2 * self.width - 1).min(hi))
    }

    /// The mean power of signed bins `a..=b`.
    fn mean(&self, a: usize, b: usize) -> f64 {
        self.power[a..=b].iter().sum::<f64>() / (b + 1 - a) as f64
    }

    /// The noise per bin: the median of the band bins outside `a..=b` over `ln 2`.
    fn noise(&mut self, a: usize, b: usize) -> f64 {
        self.scratch.clear();
        for &(lo, hi) in &self.bands {
            for j in lo..=hi {
                if j < a || j > b {
                    self.scratch.push(self.power[j]);
                }
            }
        }
        if self.scratch.len() < 8 {
            self.scratch.clear();
            for &(lo, hi) in &self.bands {
                self.scratch.extend_from_slice(&self.power[lo..=hi]);
            }
        }
        let m = self.scratch.len() / 2;
        let (_, v, _) = self.scratch.select_nth_unstable_by(m, f64::total_cmp);
        (*v / LN_2).max(1e-30)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Complex white noise of power `p` from `seed`.
    fn noise(n: usize, p: f64, seed: u64) -> Vec<Complex32> {
        let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut u = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let sd = (p / 2.0).sqrt();
        (0..n)
            .map(|_| {
                let r = (-2.0 * u().ln()).sqrt() * sd;
                let t = std::f64::consts::TAU * u();
                Complex32::new((r * t.cos()) as f32, (r * t.sin()) as f32)
            })
            .collect()
    }

    /// A 2-FSK burst of `bits` at `baud`, tones `centre +- shift / 2`, unit power, from sample `at`.
    fn fsk(
        x: &mut [Complex32],
        at: usize,
        rate: f64,
        centre: f64,
        shift: f64,
        baud: f64,
        bits: usize,
    ) {
        let spb = rate / baud;
        let mut ph = 0f64;
        let mut s = 7u32;
        for i in 0..(bits as f64 * spb) as usize {
            if i % spb as usize == 0 {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
            }
            let f = centre
                + if (s >> 16) & 1 == 1 {
                    shift / 2.0
                } else {
                    -shift / 2.0
                };
            ph += std::f64::consts::TAU * f / rate;
            x[at + i] += Complex32::new(ph.cos() as f32, ph.sin() as f32);
        }
    }

    fn bursts(d: &mut StretchDetector, x: &[Complex32]) -> Vec<(u64, usize, f64)> {
        let mut out = Vec::new();
        let mut at = 0;
        while at < x.len() {
            at += d.process(&x[at..]);
            if let Some(b) = d.take() {
                out.push((b.start, b.samples.len(), b.centre_hz));
            }
        }
        d.end();
        if let Some(b) = d.take() {
            out.push((b.start, b.samples.len(), b.centre_hz));
        }
        out
    }

    const RATE: f64 = 8000.0;

    /// Noise power in the 8 kHz channel for an SNR of `db` in 3 kHz for a unit-power signal.
    fn noise_power(db: f64) -> f64 {
        RATE / 3000.0 / sdrkit_algo_dsp::meter::db_to_power(db)
    }

    #[test]
    fn a_narrow_burst_is_found_whole_with_its_centre() {
        // 100 Bd, 200 Hz shift, 0.7 s, at 2 dB in 3 kHz: 28 dB in the stretch of 310 Hz.
        for seed in 1..=8 {
            let mut x = noise(24_000, noise_power(2.0), seed);
            fsk(&mut x, 8000, RATE, 1700.0, 200.0, 100.0, 70);
            let mut d = StretchDetector::new(StretchConfig::new(
                RATE,
                256,
                vec![(-3360.0, 3360.0)],
                310.0,
            ))
            .unwrap();
            let b = bursts(&mut d, &x);
            assert_eq!(b.len(), 1, "seed {seed}: {b:?}");
            let (start, len, centre) = b[0];
            assert!(
                start <= 8000 && start + len as u64 >= 13_600,
                "seed {seed}: {b:?}"
            );
            assert!(start + 6 * 256 >= 8000, "seed {seed}: {b:?}");
            assert!((centre - 1700.0).abs() <= 100.0, "seed {seed}: {b:?}");
        }
    }

    #[test]
    fn a_wide_burst_in_one_sideband_is_found() {
        // 1200 Bd, 1000 Hz shift (2200 Hz wide) in the lower sideband at 6 dB in 3 kHz: a ratio near 6 in a stretch of 77
        // bins, which noise keeps under 3.
        let bands = vec![(-2800.0, -350.0), (350.0, 2800.0)];
        for seed in 1..=8 {
            let mut x = noise(24_000, noise_power(6.0), seed);
            fsk(&mut x, 8000, RATE, -1700.0, 1000.0, 1200.0, 600);
            let cfg = StretchConfig {
                open_ratio: 3.0,
                ..StretchConfig::new(RATE, 256, bands.clone(), 2400.0)
            };
            let mut d = StretchDetector::new(cfg).unwrap();
            let b = bursts(&mut d, &x);
            assert_eq!(b.len(), 1, "seed {seed}: {b:?}");
            assert!(
                b[0].0 <= 8000 && b[0].0 + b[0].1 as u64 >= 12_000,
                "seed {seed}: {b:?}"
            );
            assert!(b[0].2 < 0.0, "seed {seed}: {b:?}");
        }
    }

    #[test]
    fn noise_opens_no_burst() {
        // Over 60 s of noise for each of 8 seeds (15 000 frames): two frames in a row over 5.5 have a probability of about
        // 4e-9 a frame.
        for seed in 1..=8 {
            let x = noise(480_000, 1.0, 100 + seed);
            let mut d = StretchDetector::new(StretchConfig::new(
                RATE,
                256,
                vec![(-3360.0, 3360.0)],
                310.0,
            ))
            .unwrap();
            let b = bursts(&mut d, &x);
            assert!(b.is_empty(), "seed {seed}: {b:?}");
        }
    }

    #[test]
    fn settle_min_max_and_reset() {
        let mut cfg = StretchConfig::new(RATE, 256, vec![(-3360.0, 3360.0)], 310.0);
        cfg.settle = 4000;
        cfg.max_len = 8000;
        cfg.min_len = 2500;
        let mut x = noise(40_000, 1e-4, 9);
        fsk(&mut x, 0, RATE, 1000.0, 200.0, 100.0, 50);
        fsk(&mut x, 12_000, RATE, 1000.0, 200.0, 100.0, 2);
        fsk(&mut x, 20_000, RATE, 1000.0, 200.0, 100.0, 200);
        let mut d = StretchDetector::new(cfg).unwrap();
        let b = bursts(&mut d, &x);
        // The first burst ends inside the settling; the second is too short; the third is cut at 8000 samples and the
        // rest is another burst.
        assert_eq!(b.len(), 2, "{b:?}");
        assert!(
            b[0].0 < 20_000 && b[0].1 >= 8000 && b[0].1 < 8000 + 256,
            "{b:?}"
        );
        d.reset();
        assert!(!d.is_open() && d.position() == 0 && d.take().is_none());
    }

    #[test]
    fn bad_settings_are_refused() {
        let ok = StretchConfig::new(RATE, 256, vec![(-3000.0, 3000.0)], 300.0);
        assert!(StretchDetector::new(ok.clone()).is_ok());
        for bad in [
            StretchConfig {
                frame: 300,
                ..ok.clone()
            },
            StretchConfig {
                bands: vec![],
                ..ok.clone()
            },
            StretchConfig {
                bands: vec![(0.0, 5000.0)],
                ..ok.clone()
            },
            StretchConfig {
                bands: vec![(0.0, 50.0)],
                ..ok.clone()
            },
            StretchConfig {
                width_hz: 0.0,
                ..ok.clone()
            },
            StretchConfig {
                close_ratio: 6.0,
                ..ok.clone()
            },
            StretchConfig {
                open_frames: 0,
                ..ok.clone()
            },
            StretchConfig {
                max_len: 10,
                ..ok.clone()
            },
        ] {
            assert!(StretchDetector::new(bad).is_err());
        }
    }
}
