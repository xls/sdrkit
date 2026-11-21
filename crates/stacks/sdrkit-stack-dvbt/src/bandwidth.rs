//! Automatic channel bandwidth: the flat top of the averaged spectrum is measured as the occupied width, which maps to 5, 6, 7
//! or 8 MHz. The same answer must come twice before the bandwidth changes.
//!
//! Standard: ETSI EN 300 744 clause 4.4 and Annex G.

use crate::params::Bandwidth;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32;

/// FFT size of the spectrum frames.
pub const FRAME_FFT: usize = 2048;
/// The occupied part of a channel: 6817 carriers of 8192 at the 8/7 x width sample rate (EN 300 744 table 2), 0.952.
pub const OCCUPIED_FRACTION: f64 = 6817.0 / 8192.0 * 8.0 / 7.0;
/// The edge test's strips: inside from this far within the edge ...
const EDGE_IN_HZ: f64 = 0.30e6;
/// ... to this far within it, and outside from this far beyond it ...
const EDGE_GAP_HZ: f64 = 0.05e6;
/// ... to this far beyond it.
const EDGE_OUT_HZ: f64 = 0.20e6;
/// The least fall at an edge for a width to be taken, dB.
const MIN_EDGE_DB: f64 = 6.0;

/// The detector.
pub struct BandwidthDetector {
    rate: f64,
    fft: Fft,
    window: Vec<f32>,
    buf: Vec<Complex32>,
    fill: Vec<Complex32>,
    acc: Vec<f64>,
    frames: usize,
    total: usize,
    candidate: Option<Bandwidth>,
    agree: u32,
    since_refresh: f64,
    hop: usize,
    skip: usize,
    /// The last measured width, Hz.
    pub width_hz: Option<f64>,
    /// The in-band against out-of-band level of the last estimate, dB (channel occupancy for the scanner).
    pub occupancy_db: Option<f64>,
}

impl BandwidthDetector {
    /// A detector for samples at `rate`.
    pub fn new(rate: f64) -> Self {
        let window = (0..FRAME_FFT)
            .map(|i| {
                (0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / FRAME_FFT as f64).cos())
                    as f32
            })
            .collect();
        BandwidthDetector {
            rate,
            fft: Fft::forward(FRAME_FFT),
            window,
            buf: vec![Complex32::default(); FRAME_FFT],
            fill: Vec::with_capacity(FRAME_FFT),
            acc: vec![0.0; FRAME_FFT],
            frames: 0,
            total: 0,
            candidate: None,
            agree: 0,
            since_refresh: 0.0,
            hop: ((rate * 0.005) as usize).max(FRAME_FFT),
            skip: 0,
            width_hz: None,
            occupancy_db: None,
        }
    }

    /// Feeds samples. Returns a bandwidth when the decision changes to it.
    pub fn push(&mut self, x: &[Complex32]) -> Option<Bandwidth> {
        let mut change = None;
        self.since_refresh += x.len() as f64 / self.rate;
        if self.since_refresh > 6.0 {
            self.since_refresh = 0.0;
            self.acc.iter_mut().for_each(|a| *a = 0.0);
            self.frames = 0;
        }
        for &s in x {
            if self.skip > 0 {
                self.skip -= 1;
                continue;
            }
            self.fill.push(s);
            if self.fill.len() < FRAME_FFT {
                continue;
            }
            for (i, b) in self.buf.iter_mut().enumerate() {
                *b = self.fill[i] * self.window[i];
            }
            self.fill.clear();
            self.skip = self.hop - FRAME_FFT;
            self.fft.process(&mut self.buf);
            for (i, b) in self.buf.iter().enumerate() {
                self.acc[(i + FRAME_FFT / 2) % FRAME_FFT] += b.norm_sqr() as f64;
            }
            self.frames += 1;
            self.total += 1;
            if self.frames >= 12 {
                if let Some(bw) = self.estimate() {
                    if Some(bw) == self.candidate {
                        self.agree += 1;
                    } else {
                        self.candidate = Some(bw);
                        self.agree = 1;
                    }
                    if self.agree >= 2 && self.total >= 20 {
                        change = Some(bw);
                    }
                }
            }
        }
        change
    }

    /// The bandwidth from the averaged spectrum, when there is a clear flat top with sharp edges.
    ///
    /// A DVB-T signal fills 6817 (8K) or 1705 (2K) carriers: 0.952 of the channel width (EN 300 744 table 2: 7.61 MHz of an 8 MHz
    /// channel, 6.66 MHz of 7 MHz). Each standard width is tried as a template: its edge must be a sharp fall on both sides (the
    /// level just inside the occupied band against the level just outside it). A neighbouring multiplex on the next channel fills
    /// the space beyond a too-wide template, so that template has no edge there and neighbours never merge into one wide signal.
    pub fn estimate(&mut self) -> Option<Bandwidth> {
        let n = FRAME_FFT;
        let hz_per_bin = self.rate / n as f64;
        let frames = self.frames.max(1) as f64;
        let lin: Vec<f64> = self.acc.iter().map(|&p| (p / frames).max(1e-30)).collect();
        // Smoothed over 50 kHz, in dB.
        let w = ((0.05e6 / hz_per_bin).round() as usize).max(1);
        let sm: Vec<f64> = (0..n)
            .map(|i| {
                let a = i.saturating_sub(w / 2);
                let b = (i + w / 2 + 1).min(n);
                10.0 * (lin[a..b].iter().sum::<f64>() / (b - a) as f64).log10()
            })
            .collect();
        let centre = n / 2;
        let nyq = self.rate / 2.0;
        let pct = |v: &mut Vec<f64>, p: f64| -> f64 {
            v.sort_by(f64::total_cmp);
            v[((v.len() - 1) as f64 * p).round() as usize]
        };
        let bin = |hz: f64| -> i64 { centre as i64 + (hz / hz_per_bin).round() as i64 };
        let usable = 0.96 * nyq;
        let near = ((0.3 * nyq).min(2e6) / hz_per_bin) as usize;
        let dc = (0.15e6 / hz_per_bin) as usize;
        let mut plat: Vec<f64> = (centre.saturating_sub(near)..(centre + near).min(n))
            .filter(|&i| i.abs_diff(centre) > dc)
            .map(|i| sm[i])
            .collect();
        if plat.is_empty() {
            return None;
        }
        let p = pct(&mut plat, 0.85);
        let edge = (usable / hz_per_bin) as usize;
        let mut all: Vec<f64> = (centre.saturating_sub(edge)..(centre + edge).min(n))
            .map(|i| sm[i])
            .collect();
        let f = pct(&mut all, 0.05);
        self.occupancy_db = Some(p - f);
        if p - f < 8.0 {
            return None;
        }
        // The mean level (dB) from `lo_hz` to `hi_hz` off the centre, or None outside the usable span.
        let mean = |lo_hz: f64, hi_hz: f64| -> Option<f64> {
            if lo_hz.abs().max(hi_hz.abs()) > usable {
                return None;
            }
            let (a, b) = (bin(lo_hz.min(hi_hz)), bin(lo_hz.max(hi_hz)));
            let v: Vec<f64> = (a..=b).map(|i| sm[i as usize]).collect();
            Some(v.iter().sum::<f64>() / v.len() as f64)
        };
        let mut best: Option<(Bandwidth, f64)> = None;
        for b in Bandwidth::ALL {
            let half = OCCUPIED_FRACTION * b.mhz() as f64 * 1e6 / 2.0;
            let side = |sign: f64| -> Option<f64> {
                let inside = mean(sign * (half - EDGE_IN_HZ), sign * (half - EDGE_GAP_HZ))?;
                let outside = mean(sign * (half + EDGE_GAP_HZ), sign * (half + EDGE_OUT_HZ))?;
                Some(inside - outside)
            };
            let (Some(lo), Some(hi)) = (side(-1.0), side(1.0)) else {
                continue;
            };
            let score = lo.min(hi);
            if best.is_none_or(|(_, s)| score > s) {
                best = Some((b, score));
            }
        }
        let (b, score) = best?;
        let need = MIN_EDGE_DB.max(0.4 * (p - f));
        if score < need {
            return None;
        }
        // The occupied width: where the level falls below halfway (in dB) from the plateau to the floor, looked for near the
        // template's edges only.
        let thr = f + (p - f) / 2.0;
        let half = OCCUPIED_FRACTION * b.mhz() as f64 * 1e6 / 2.0;
        let walk = |sign: f64| -> f64 {
            let mut hz = half - 0.3e6;
            while hz < half + 0.3e6 && hz < usable {
                if sm[bin(sign * hz) as usize] < thr {
                    break;
                }
                hz += hz_per_bin;
            }
            hz
        };
        self.width_hz = Some(walk(-1.0) + walk(1.0));
        Some(b)
    }

    /// Starts again.
    pub fn reset(&mut self) {
        self.acc.iter_mut().for_each(|a| *a = 0.0);
        self.frames = 0;
        self.total = 0;
        self.candidate = None;
        self.agree = 0;
        self.since_refresh = 0.0;
    }
}

/// The bandwidth to start with when Auto: the one whose natural rate equals the input rate, or the widest that fits.
pub fn initial_guess(rate: f64) -> Bandwidth {
    for b in Bandwidth::ALL {
        if (b.natural_rate() - rate).abs() < 1e-4 * rate {
            return b;
        }
    }
    Bandwidth::ALL
        .into_iter()
        .rev()
        .find(|b| b.min_rate() <= rate * 1.0001)
        .unwrap_or(Bandwidth::B5)
}

sdrkit_core::choice_enum! {
    /// Where the channel width in use came from.
    pub enum WidthSource {
        /// The `bandwidth` setting.
        Setting = ("setting", "Setting"),
        /// The first guess from the sample rate, before a measurement.
        Guess = ("guess", "Guess"),
        /// Measured from the spectrum.
        Measured = ("measured", "Measured"),
        /// Tried after no lock at the other widths.
        Tried = ("tried", "Tried"),
    }
}

/// How long a width has to lock (TPS) before Auto tries another, s. An 8K frame with the longest guard lasts
/// 0.1 s at 5 MHz.
pub const WIDTH_TRY_S: f64 = 2.0;

/// The next width to try after `current` failed: the standard width that the rate allows and that is not in `tried`, nearest
/// to `current` first (the wider of two equally near).
pub fn next_width(current: Bandwidth, tried: &[Bandwidth], rate_hz: f64) -> Option<Bandwidth> {
    Bandwidth::ALL
        .into_iter()
        .filter(|b| !tried.contains(b) && *b != current && b.min_rate() <= rate_hz * 1.0001)
        .min_by_key(|b| (b.mhz().abs_diff(current.mhz()), std::cmp::Reverse(b.mhz())))
}
