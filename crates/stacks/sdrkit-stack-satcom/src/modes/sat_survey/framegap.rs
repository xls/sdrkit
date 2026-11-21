//! The frame-gap method: a signal sent in frames with a short gap between them leaves a line at the frame rate and its
//! harmonics in the spectrum of its power envelope. The envelope (mean power over groups of samples, about 200 ksps) is cut
//! into segments of 65 536 values whose power spectra add up; the harmonics are searched over a span of clock errors.

use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;

/// The rate the envelope aims at, Hz: samples are averaged in groups of max(1, floor(rate / this)).
pub const ENVELOPE_HZ: f64 = 200e3;
/// Envelope values of a segment.
pub const SEGMENT: usize = 65_536;
/// The clock errors searched, ppm: from, to, step.
pub const PPM_SPAN: (i32, i32, i32) = (-60, 60, 4);
/// The harmonics summed.
pub const HARMONICS: u32 = 8;
/// A harmonic is skipped below this bin ...
pub const LOW_BIN: usize = 45;
/// ... or within this many bins of half the segment.
pub const HIGH_GUARD: usize = 40;
/// The noise of a harmonic: the median of the bins this near and far on either side.
pub const NOISE_BINS: (usize, usize) = (10, 40);
/// The least segments before a detection.
pub const MIN_SEGMENTS: u64 = 4;
/// A detection needs a level over the threshold minus this, dB.
pub const MARGIN_DB: f64 = 3.0;

/// The frame line of the frame methods.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameLine {
    /// The frame rate searched, Hz.
    pub searched_hz: f64,
    /// The rate that fitted best, Hz.
    pub measured_hz: f64,
    /// The level of the fit, dB.
    pub level_db: f64,
    /// Detected.
    pub detected: bool,
    /// Searches made.
    pub searches: u64,
}

/// The frame-gap method at one input rate (module documentation).
pub struct FrameGap {
    dec: usize,
    env_rate: f64,
    frame_hz: f64,
    threshold_db: f64,
    acc: f64,
    in_acc: usize,
    env: Vec<Complex32>,
    fft: WindowedFft,
    sum: Vec<f64>,
    segments: u64,
    scratch: Vec<f64>,
    line: FrameLine,
}

impl FrameGap {
    /// The method for input at `rate`, a nominal frame rate `frame_hz` and the threshold `threshold_db`.
    pub fn new(rate: f64, frame_hz: f64, threshold_db: f64) -> Self {
        let dec = ((rate / ENVELOPE_HZ).floor() as usize).max(1);
        let fft = match WindowedFft::new(SEGMENT, SEGMENT, Window::Hann) {
            Ok(f) => f,
            Err(_) => unreachable!("the window fits the transform"),
        };
        FrameGap {
            dec,
            env_rate: rate / dec as f64,
            frame_hz,
            threshold_db,
            acc: 0.0,
            in_acc: 0,
            env: Vec::with_capacity(SEGMENT),
            fft,
            sum: vec![0.0; SEGMENT / 2],
            segments: 0,
            scratch: Vec::new(),
            line: FrameLine {
                searched_hz: frame_hz,
                ..FrameLine::default()
            },
        }
    }

    /// The envelope rate, Hz.
    pub fn envelope_hz(&self) -> f64 {
        self.env_rate
    }

    /// Sets the threshold, dB.
    pub fn set_threshold(&mut self, db: f64) {
        self.threshold_db = db;
    }

    /// The frame line after the last segment.
    pub fn line(&self) -> FrameLine {
        self.line
    }

    /// Forgets the summed spectra and the line.
    pub fn clear(&mut self) {
        self.sum.fill(0.0);
        self.segments = 0;
        self.line = FrameLine {
            searched_hz: self.frame_hz,
            ..FrameLine::default()
        };
    }

    /// Takes samples (finite values).
    pub fn process(&mut self, x: &[Complex32]) {
        for z in x {
            self.acc += f64::from(z.norm_sqr());
            self.in_acc += 1;
            if self.in_acc == self.dec {
                self.env
                    .push(Complex32::new((self.acc / self.dec as f64) as f32, 0.0));
                self.acc = 0.0;
                self.in_acc = 0;
                if self.env.len() == SEGMENT {
                    self.segment();
                    self.env.clear();
                }
            }
        }
    }

    fn segment(&mut self) {
        let mean = self.env.iter().map(|z| f64::from(z.re)).sum::<f64>() / SEGMENT as f64;
        for z in &mut self.env {
            z.re -= mean as f32;
        }
        let p = self.fft.run_power(&self.env);
        for (s, &v) in self.sum.iter_mut().zip(p) {
            *s += f64::from(v);
        }
        self.segments += 1;
        self.search();
    }

    /// The best clock error: harmonic peaks over the noise of their sides.
    fn search(&mut self) {
        let bin = self.env_rate / SEGMENT as f64;
        let half = SEGMENT / 2;
        let mut best: Option<(f64, f64)> = None;
        let mut ppm = PPM_SPAN.0;
        while ppm <= PPM_SPAN.1 {
            let fr = self.frame_hz * (1.0 + f64::from(ppm) * 1e-6);
            let (mut peaks, mut noise) = (0.0, 0.0);
            for h in 1..=HARMONICS {
                let k = (f64::from(h) * fr / bin).round() as usize;
                if k < LOW_BIN || k + HIGH_GUARD >= half {
                    continue;
                }
                peaks += self.sum[k - 1..=k + 1].iter().copied().fold(0.0, f64::max);
                self.scratch.clear();
                self.scratch
                    .extend_from_slice(&self.sum[k - NOISE_BINS.1..=k - NOISE_BINS.0]);
                self.scratch
                    .extend_from_slice(&self.sum[k + NOISE_BINS.0..=k + NOISE_BINS.1]);
                let mid = self.scratch.len() / 2;
                let (_, med, _) = self.scratch.select_nth_unstable_by(mid, f64::total_cmp);
                noise += *med;
            }
            if noise > 0.0 {
                let r = peaks / noise;
                if best.is_none_or(|(b, _)| r > b) {
                    best = Some((r, fr));
                }
            }
            ppm += PPM_SPAN.2;
        }
        if let Some((r, fr)) = best {
            self.line.level_db = 10.0 * r.max(1e-12).log10();
            self.line.measured_hz = fr;
            self.line.detected =
                self.segments >= MIN_SEGMENTS && self.line.level_db > self.threshold_db - MARGIN_DB;
        }
        self.line.searches = self.segments;
    }
}
