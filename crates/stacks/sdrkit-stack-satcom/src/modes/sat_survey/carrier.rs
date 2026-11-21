//! The carrier method: averaged spectra of about 0.25 s, a background of the medians of groups of bins, peaks over a threshold that noise
//! reaches about once in a hundred frames, and the shared multi-line tracker (`sdrkit_algo_dsp::linetrack`) that follows each
//! narrow carrier through the pass. Every decision follows the absolute sample index, so pieces of any size give the same
//! output.

use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::linetrack::{LinePeak, LineTrack, LineTracker, TrackParams};
use sdrkit_algo_dsp::peak::parabolic;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;

/// The smallest transform, as a power of two.
pub const MIN_LOG2: u32 = 8;
/// The largest transform of the carrier method, as a power of two.
pub const MAX_LOG2: u32 = 20;
/// The length of a frame, s.
pub const FRAME_S: f64 = 0.25;
/// Background parts across the band: the transform size over this, clamped to [`BACKGROUND_PARTS`].
pub const BACKGROUND_DIV: usize = 64;
/// The least and the most background parts.
pub const BACKGROUND_PARTS: (usize, usize) = (4, 64);
/// The median of noise power lies this far under its mean, dB (10 log10(1 / ln 2)).
pub const MEDIAN_TO_MEAN_DB: f64 = 1.6;
/// The threshold is the level that noise reaches about once in this many frames.
pub const FALSE_PEAK_FRAMES: f64 = 100.0;
/// Bins kept off each edge: the transform size over this.
pub const EDGE_DIV: usize = 40;
/// The most peaks of a frame.
pub const MAX_PEAKS: usize = 200;
/// The fastest drift of a carrier: this many Hz/s per MHz of the dial ...
pub const SLOPE_PER_MHZ: f64 = 0.032;
/// ... plus this many bins per second.
pub const SLOPE_BINS: f64 = 20.0;
/// The least half-width of a track's gate, bins.
pub const GATE_MIN_BINS: f64 = 3.0;
/// Added to the widened gate, bins.
pub const GATE_EXTRA_BINS: f64 = 2.0;
/// Values of the published spectrum.
pub const SPECTRUM_POINTS: usize = 256;

/// The transform size 2^l: l from [`MIN_LOG2`] up to `max_log2` while `rate / 2^l` is over `bin_wanted`.
pub fn fft_size(rate: f64, bin_wanted: f64, max_log2: u32) -> usize {
    let mut l = MIN_LOG2;
    while l < max_log2 && rate / f64::from(1u32 << l) > bin_wanted {
        l += 1;
    }
    1usize << l
}

/// The level, as a factor of the mean, that noise in the mean of `m` power spectra of `n` bins reaches about once in
/// [`FALSE_PEAK_FRAMES`] frames: ln(100 n) for one spectrum, else 1 + sqrt(2 ln(100 n) / m) + ln(100 n) / m.
pub fn false_peak_factor(n: usize, m: usize) -> f64 {
    let l = (FALSE_PEAK_FRAMES * n as f64).ln();
    if m <= 1 {
        l
    } else {
        let m = m as f64;
        1.0 + (2.0 * l / m).sqrt() + l / m
    }
}

/// The carrier method at one input rate (module documentation).
pub struct CarrierSurvey {
    rate: f64,
    n: usize,
    hop: usize,
    m: usize,
    bin: f64,
    threshold_db: f64,
    fft: WindowedFft,
    buf: Vec<Complex32>,
    acc: Vec<f32>,
    in_frame: usize,
    ffts: u64,
    frames: u64,
    db: Vec<f32>,
    scratch: Vec<f32>,
    peaks: Vec<LinePeak>,
    tracker: LineTracker,
    spectrum: Vec<f32>,
}

impl CarrierSurvey {
    /// The method for input at `rate` with bins of about `bin_wanted` Hz, the peak threshold `threshold_db` (before the
    /// noise limit) and the dial `dial_hz`, which sets the fastest drift followed.
    pub fn new(rate: f64, bin_wanted: f64, threshold_db: f64, dial_hz: f64) -> Self {
        let n = fft_size(rate, bin_wanted, MAX_LOG2);
        let hop = n / 2;
        let m = ((FRAME_S * rate / hop as f64).round() as usize).max(1);
        let bin = rate / n as f64;
        let slope = SLOPE_PER_MHZ * dial_hz.abs() / 1e6 + SLOPE_BINS * bin;
        let fft = match WindowedFft::new(n, n, Window::Hann) {
            Ok(f) => f,
            Err(_) => unreachable!("the window fits the transform"),
        };
        CarrierSurvey {
            rate,
            n,
            hop,
            m,
            bin,
            threshold_db,
            fft,
            buf: Vec::with_capacity(n),
            acc: vec![0.0; n],
            in_frame: 0,
            ffts: 0,
            frames: 0,
            db: vec![0.0; n],
            scratch: Vec::new(),
            peaks: Vec::new(),
            tracker: LineTracker::new(TrackParams::new(
                slope,
                GATE_MIN_BINS * bin,
                GATE_EXTRA_BINS * bin,
            )),
            spectrum: vec![0.0; SPECTRUM_POINTS],
        }
    }

    /// The bin width, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.bin
    }

    /// The transform size.
    pub fn fft_len(&self) -> usize {
        self.n
    }

    /// Spectra averaged in a frame.
    pub fn spectra_per_frame(&self) -> usize {
        self.m
    }

    /// The peak threshold in force, dB over the background: the threshold, or the noise limit when that is higher.
    pub fn peak_threshold_db(&self) -> f64 {
        self.threshold_db
            .max(10.0 * false_peak_factor(self.n, self.m).log10() + MEDIAN_TO_MEAN_DB)
    }

    /// Sets the threshold before the noise limit, dB.
    pub fn set_threshold(&mut self, db: f64) {
        self.threshold_db = db;
    }

    /// Frames made.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// The tracker.
    pub fn tracker(&self) -> &LineTracker {
        &self.tracker
    }

    /// The tracks that ended and were kept since the last call.
    pub fn take_finished(&mut self) -> Vec<LineTrack> {
        self.tracker.take_finished()
    }

    /// The last frame in dB over the background, reduced to [`SPECTRUM_POINTS`] values by maximum, lowest frequency first.
    pub fn spectrum(&self) -> &[f32] {
        &self.spectrum
    }

    /// Forgets every track.
    pub fn clear(&mut self) {
        self.tracker.clear();
    }

    /// Takes samples (finite values).
    pub fn process(&mut self, x: &[Complex32]) {
        let mut rest = x;
        while !rest.is_empty() {
            let take = (self.n - self.buf.len()).min(rest.len());
            self.buf.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.buf.len() == self.n {
                self.transform();
                self.buf.drain(..self.hop);
            }
        }
    }

    fn transform(&mut self) {
        let half = self.n / 2;
        let p = self.fft.run_power(&self.buf);
        // Centred: index j holds the frequency (j - n/2) × bin.
        for (k, &v) in p.iter().enumerate() {
            self.acc[(k + half) % self.n] += v;
        }
        self.ffts += 1;
        self.in_frame += 1;
        if self.in_frame == self.m {
            let t = ((self.ffts - 1) as f64 * self.hop as f64 + self.n as f64) / self.rate;
            self.frame(t);
            self.acc.fill(0.0);
            self.in_frame = 0;
        }
    }

    /// One frame at signal time `t`: background, values over it, peaks, tracks, spectrum.
    fn frame(&mut self, t: f64) {
        let n = self.n;
        let parts = (n / BACKGROUND_DIV).clamp(BACKGROUND_PARTS.0, BACKGROUND_PARTS.1);
        let len = n / parts;
        for b in 0..parts {
            let part = &self.acc[b * len..(b + 1) * len];
            self.scratch.clear();
            self.scratch.extend_from_slice(part);
            let mid = self.scratch.len() / 2;
            let (_, med, _) = self.scratch.select_nth_unstable_by(mid, f32::total_cmp);
            let bg = med.max(f32::MIN_POSITIVE);
            for (d, &v) in self.db[b * len..(b + 1) * len].iter_mut().zip(part) {
                *d = 10.0 * (v.max(f32::MIN_POSITIVE) / bg).log10();
            }
        }
        let thr = self.peak_threshold_db() as f32;
        let edge = (n / EDGE_DIV).max(1);
        self.peaks.clear();
        for j in edge..n - edge {
            let (a, v, c) = (self.db[j - 1], self.db[j], self.db[j + 1]);
            if v > thr && v > a && v >= c {
                let pk = parabolic(f64::from(a), f64::from(v), f64::from(c));
                self.peaks.push(LinePeak {
                    hz: (j as f64 + pk.offset - (n / 2) as f64) * self.bin,
                    level_db: pk.value,
                });
            }
        }
        self.peaks.sort_by(|a, b| {
            b.level_db
                .total_cmp(&a.level_db)
                .then(a.hz.total_cmp(&b.hz))
        });
        self.peaks.truncate(MAX_PEAKS);
        self.tracker.update(t, &self.peaks);
        let per = n / SPECTRUM_POINTS;
        for (s, chunk) in self.spectrum.iter_mut().zip(self.db.chunks(per)) {
            *s = chunk.iter().copied().fold(f32::MIN, f32::max);
        }
        self.frames += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_sizes_and_the_noise_limit() {
        assert_eq!(fft_size(1e6, 8.0, MAX_LOG2), 1 << 17);
        assert_eq!(fft_size(10e6, 1000.0, 18), 1 << 14);
        assert_eq!(fft_size(250e3, 1e6, MAX_LOG2), 256);
        // Four averaged spectra of 2^17 bins: about 9 dB, 10.6 dB with the median correction.
        let c = CarrierSurvey::new(1e6, 8.0, 10.0, 150e6);
        assert_eq!(c.spectra_per_frame(), 4);
        assert!(
            (c.peak_threshold_db() - 10.6).abs() < 0.1,
            "{}",
            c.peak_threshold_db()
        );
    }
}
