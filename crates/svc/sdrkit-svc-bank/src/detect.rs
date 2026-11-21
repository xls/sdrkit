//! Carrier detection on a raster: the level and signal-to-noise ratio of any number of channels of a capture from one averaged
//! power spectrum, and a [`Gate`] that turns a level into on and off with hysteresis and a hang time.

use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_dsp::{DspError, Param};
use serde::{Deserialize, Serialize};

/// The level of one watched channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    /// The power inside the channel, in dBFS.
    pub level_dbfs: f32,
    /// That power over the noise the channel would hold at the floor, in dB.
    pub snr_db: f32,
}

#[derive(Clone, Copy, Debug)]
struct Target {
    /// First bin (lowest frequency first, may lie outside the frame) and the number of bins.
    first: isize,
    count: usize,
}

/// Measures the level of channels of a capture from its spectrum frames. The noise floor is the median of the averaged bins, so it
/// holds while up to half the capture is occupied.
#[derive(Clone, Debug)]
pub struct CarrierDetector {
    rate: f64,
    n: usize,
    /// The equivalent noise bandwidth of the window, in bins.
    enbw: f32,
    alpha: f32,
    avg: Vec<f32>,
    frames: u64,
    scratch: Vec<f32>,
    targets: Vec<Target>,
    readings: Vec<Reading>,
    floor_bin: f32,
    updates: u64,
}

impl CarrierDetector {
    /// A detector for frames of `bins` bins in dBFS, lowest frequency first, of a capture at `rate`. The frames come through
    /// `window` (scaled so that a tone reads its own level), at `frames_per_second`. They go into an exponential average of time
    /// constant `tau_s` (a carrier that stops falls by about 4.3 dB every `tau_s`).
    ///
    /// # Errors
    ///
    /// [`DspError`] if a number is not finite or not above 0, or `bins` is below 2.
    pub fn new(
        rate: f64,
        bins: usize,
        window: Window,
        frames_per_second: f64,
        tau_s: f64,
    ) -> Result<Self, DspError> {
        for (param, v) in [
            (Param::Rate, rate),
            (Param::FrameRate, frames_per_second),
            (Param::Tau, tau_s),
        ] {
            if !v.is_finite() {
                return Err(DspError::NotFinite { param });
            }
            if v <= 0.0 {
                return Err(DspError::OutOfRange {
                    param,
                    min: f64::MIN_POSITIVE,
                    max: f64::MAX,
                    got: v,
                });
            }
        }
        if bins < 2 {
            return Err(DspError::NotAllowed {
                param: Param::Bins,
                got: bins as f64,
            });
        }
        let w = window.coefficients(bins);
        let sum: f64 = w.iter().map(|&v| f64::from(v)).sum();
        let sum_sq: f64 = w.iter().map(|&v| f64::from(v) * f64::from(v)).sum();
        Ok(CarrierDetector {
            rate,
            n: bins,
            enbw: (bins as f64 * sum_sq / (sum * sum).max(1e-30)) as f32,
            alpha: (1.0 - (-(1.0 / frames_per_second) / tau_s).exp()).clamp(1e-4, 1.0) as f32,
            avg: vec![0.0; bins],
            frames: 0,
            scratch: Vec::with_capacity(bins),
            targets: Vec::new(),
            readings: Vec::new(),
            floor_bin: 0.0,
            updates: 0,
        })
    }

    /// The number of bins of a frame.
    pub fn bins(&self) -> usize {
        self.n
    }

    /// The width of one bin, in Hz.
    pub fn resolution_hz(&self) -> f64 {
        self.rate / self.n as f64
    }

    /// Sets the channels to watch: `(offset from the capture's centre, width)` in Hz. Readings follow this order.
    pub fn set_targets(&mut self, channels: &[(f64, f64)]) {
        let res = self.resolution_hz();
        let mid = (self.n / 2) as isize;
        self.targets.clear();
        for &(o, b) in channels {
            let lo = ((o - b / 2.0) / res).round() as isize;
            let hi = ((o + b / 2.0) / res).round() as isize;
            let count = ((hi - lo).max(0) as usize + 1).min(self.n);
            self.targets.push(Target {
                first: lo + mid,
                count,
            });
        }
        self.readings.clear();
        self.readings.resize(
            self.targets.len(),
            Reading {
                level_dbfs: -200.0,
                snr_db: 0.0,
            },
        );
        if self.frames > 0 {
            self.measure();
        }
    }

    /// The readings, one per target, from the last measurement.
    pub fn readings(&self) -> &[Reading] {
        &self.readings
    }

    /// The noise floor in one bin, in dBFS (a channel of width `B` holds `B / resolution` bins of it).
    pub fn floor_dbfs(&self) -> f32 {
        10.0 * self.floor_bin.max(1e-30).log10()
    }

    /// How many measurements there have been.
    pub fn updates(&self) -> u64 {
        self.updates
    }

    /// Forgets the averaged spectrum (after a retune).
    pub fn reset(&mut self) {
        self.frames = 0;
        self.avg.fill(0.0);
    }

    /// Takes one frame of [`CarrierDetector::bins`] values in dBFS, lowest frequency first, and updates the readings. A frame of
    /// another length is ignored.
    pub fn push_frame(&mut self, frame_db: &[f32]) {
        if frame_db.len() != self.n {
            return;
        }
        let a = if self.frames == 0 { 1.0 } else { self.alpha };
        let k = 1.0 / self.enbw;
        for (p, &db) in self.avg.iter_mut().zip(frame_db) {
            *p += a * (10f32.powf(db / 10.0) * k - *p);
        }
        self.frames += 1;
        self.measure();
    }

    fn measure(&mut self) {
        self.scratch.clear();
        self.scratch.extend_from_slice(&self.avg);
        let mid = self.scratch.len() / 2;
        let (_, med, _) = self
            .scratch
            .select_nth_unstable_by(mid, |a, b| a.total_cmp(b));
        // The median of an average of k exponential powers is below their mean, at about (1 - 1/(9k))^3 of it (0.70 for one
        // frame). An exponential average of factor a is worth (2 - a) / a frames once it has settled.
        let settled = (2.0 - self.alpha) / self.alpha;
        let k = (self.frames as f32).min(settled).max(1.0);
        let bias = (1.0 - 1.0 / (9.0 * k)).powi(3);
        self.floor_bin = (*med / bias).max(1e-30);
        let n = self.n as isize;
        for (t, r) in self.targets.iter().zip(self.readings.iter_mut()) {
            let p: f32 = (t.first..t.first + t.count as isize)
                .filter(|i| (0..n).contains(i))
                .map(|i| self.avg[i as usize])
                .sum();
            r.level_dbfs = 10.0 * p.max(1e-30).log10();
            r.snr_db = 10.0 * (p / (self.floor_bin * t.count as f32)).max(1e-30).log10();
        }
        self.updates += 1;
    }
}

/// On and off from a level, with hysteresis and a hang time: opens at `open_db`, closes once it has stayed below
/// `open_db - hysteresis_db` for `hang_s` seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gate {
    /// The level that opens it.
    pub open_db: f32,
    /// How far below `open_db` it must fall to start closing.
    pub hysteresis_db: f32,
    /// How long it stays open after the level falls.
    pub hang_s: f64,
    open: bool,
    below_since: Option<f64>,
}

impl Gate {
    /// A closed gate.
    pub fn new(open_db: f32, hysteresis_db: f32, hang_s: f64) -> Self {
        Gate {
            open_db,
            hysteresis_db,
            hang_s,
            open: false,
            below_since: None,
        }
    }
    /// Whether it is open.
    pub fn is_open(&self) -> bool {
        self.open
    }
    /// Forces the state (a channel without squelch is always open).
    pub fn force(&mut self, open: bool) {
        self.open = open;
        self.below_since = None;
    }
    /// Feeds a level at signal time `t` (seconds); returns whether it is open.
    pub fn update(&mut self, level_db: f32, t: f64) -> bool {
        if level_db >= self.open_db {
            self.open = true;
            self.below_since = None;
        } else if self.open && level_db < self.open_db - self.hysteresis_db {
            let since = *self.below_since.get_or_insert(t);
            if t - since >= self.hang_s {
                self.open = false;
                self.below_since = None;
            }
        } else if self.open {
            self.below_since = None;
        }
        self.open
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::spectrum::{SpectrumAnalyzer, SpectrumConfig};
    use sdrkit_algo_dsp::Complex32;

    fn noise(n: usize, sigma: f32, seed: &mut u64) -> Vec<Complex32> {
        let mut u = || {
            *seed ^= *seed << 13;
            *seed ^= *seed >> 7;
            *seed ^= *seed << 17;
            ((*seed >> 11) as f64 / (1u64 << 53) as f64).max(1e-300)
        };
        (0..n)
            .map(|_| {
                let (a, b) = (u(), u());
                let r = (-2.0 * a.ln()).sqrt() * sigma as f64 / std::f64::consts::SQRT_2;
                let p = std::f64::consts::TAU * b;
                Complex32::new((r * p.cos()) as f32, (r * p.sin()) as f32)
            })
            .collect()
    }

    #[test]
    fn levels_and_snr_of_carriers_over_noise() {
        let rate = 1e6;
        let fps = 500.0;
        let mut d = CarrierDetector::new(rate, 1024, Window::BlackmanHarris4, fps, 0.016).unwrap();
        assert_eq!(d.bins(), 1024);
        d.set_targets(&[(100e3, 10e3), (-250e3, 10e3), (300e3, 10e3)]);
        let mut cfg = SpectrumConfig::new(1024, rate);
        cfg.window = Window::BlackmanHarris4;
        cfg.frames_per_second = fps;
        let mut a = SpectrumAnalyzer::new(cfg).unwrap();
        let mut seed = 7u64;
        let mut x = noise(200_000, 0.01, &mut seed);
        // A carrier of power 1e-2 (-20 dBFS) at 100 kHz, 1e-3 (-30 dBFS) at -250 kHz.
        for (i, v) in x.iter_mut().enumerate() {
            let t = i as f64 / rate;
            let p = std::f64::consts::TAU * 100e3 * t;
            let q = std::f64::consts::TAU * -250e3 * t;
            *v += Complex32::new(p.cos() as f32, p.sin() as f32) * 0.1;
            *v += Complex32::new(q.cos() as f32, q.sin() as f32) * 0.0316;
        }
        let hop = (a.hop().floor() as usize).max(1);
        for c in x.chunks(hop) {
            if a.push(c) > 0 {
                d.push_frame(a.spectrum_db());
            }
        }
        assert!(d.updates() >= 50, "{}", d.updates());
        let r = d.readings();
        assert!((r[0].level_dbfs + 20.0).abs() < 0.5, "{:?}", r[0]);
        assert!((r[1].level_dbfs + 30.0).abs() < 0.5, "{:?}", r[1]);
        // Noise of power 1e-4 spread over 1 MHz: 1e-10 per Hz, 1e-6 in 10 kHz (-60 dBFS): the empty channel reads near 0 dB SNR.
        assert!(r[2].snr_db.abs() < 1.5, "{:?}", r[2]);
        assert!((r[2].level_dbfs + 60.0).abs() < 1.5, "{:?}", r[2]);
        assert!((r[0].snr_db - 40.0).abs() < 1.5, "{:?}", r[0]);
        assert!(
            (d.floor_dbfs() + 70.0 - 0.1).abs() < 1.5,
            "{}",
            d.floor_dbfs()
        );
    }

    #[test]
    fn a_gate_opens_holds_and_closes() {
        let mut g = Gate::new(10.0, 3.0, 0.5);
        assert!(!g.update(9.0, 0.0));
        assert!(g.update(10.5, 0.1));
        // Between the two thresholds it stays open with no hang running.
        assert!(g.update(8.0, 0.2));
        assert!(g.update(5.0, 0.3));
        assert!(g.update(5.0, 0.7));
        assert!(!g.update(5.0, 0.81));
        g.force(true);
        assert!(g.is_open());
    }
}
