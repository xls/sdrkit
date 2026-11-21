//! The channel method: power spectra averaged over about 20 ms, a floor at the 40th percentile of the bins, and the channels
//! of a fixed grid of absolute frequencies. A channel is hit when its largest bin is over the on level, active after two hits
//! in a row; a burst is counted when it turns active. Energy and width only: nothing is decoded.

use super::carrier::{false_peak_factor, fft_size};
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;

/// The largest transform of the channel method, as a power of two.
pub const MAX_LOG2: u32 = 18;
/// The time of one decision, s (at least [`MIN_SPECTRA`] spectra).
pub const DECISION_S: f64 = 0.02;
/// The least spectra of a decision.
pub const MIN_SPECTRA: usize = 2;
/// The quantile of the bins that is the floor.
pub const FLOOR_QUANTILE: f64 = 0.4;
/// The noise limit of the on level is this many times the false-peak factor.
pub const NOISE_MARGIN: f64 = 1.5;
/// Hits in a row that make a channel active.
pub const HITS_ACTIVE: u32 = 2;
/// The duty follows over this time, s: each decision moves it by the decision time over this.
pub const DUTY_S: f64 = 20.0;

/// What is known of one channel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChannelState {
    /// Its centre on the grid, Hz (absolute when the centre of the samples is known).
    pub grid_hz: f64,
    /// The frequency of its largest bin at the last hit, Hz from the centre of the samples.
    pub offset_hz: f64,
    /// The bins over the on level at the last hit, times the bin width, Hz.
    pub width_hz: f64,
    /// The largest bin over the floor at the last hit, dB.
    pub snr_db: f64,
    /// The share of time it is hit, followed over [`DUTY_S`].
    pub duty: f64,
    /// Times it turned active.
    pub bursts: u64,
    /// The signal time of the last hit, s.
    pub last_seen_s: f64,
    /// It is active now.
    pub active: bool,
    /// It has been active.
    pub ever: bool,
    hits: u32,
}

/// The channel method at one input rate (module documentation).
pub struct ChannelSurvey {
    rate: f64,
    n: usize,
    m: usize,
    bin: f64,
    on_factor: f64,
    threshold_db: f64,
    fft: WindowedFft,
    buf: Vec<Complex32>,
    acc: Vec<f32>,
    in_decision: usize,
    ffts: u64,
    decisions: u64,
    group_of: Vec<u32>,
    groups: Vec<ChannelState>,
    max: Vec<(f32, usize, u32)>,
    scratch: Vec<f32>,
}

impl ChannelSurvey {
    /// The method for input at `rate` centred on `centre_hz` (0 when not known), with bins of about `bin_wanted` Hz,
    /// channels every `grid_hz` and the threshold `threshold_db`.
    pub fn new(
        rate: f64,
        centre_hz: f64,
        bin_wanted: f64,
        grid_hz: f64,
        threshold_db: f64,
    ) -> Self {
        let n = fft_size(rate, bin_wanted, MAX_LOG2);
        let m = ((DECISION_S * rate / n as f64).floor() as usize).max(MIN_SPECTRA);
        let bin = rate / n as f64;
        let grid = grid_hz.max(bin);
        let number =
            |j: usize| ((centre_hz + (j as f64 - (n / 2) as f64) * bin) / grid).round() as i64;
        let base = number(0);
        let group_of: Vec<u32> = (0..n).map(|j| (number(j) - base) as u32).collect();
        let count = group_of.last().map_or(1, |&g| g as usize + 1);
        let groups = (0..count)
            .map(|g| ChannelState {
                grid_hz: (base + g as i64) as f64 * grid,
                ..ChannelState::default()
            })
            .collect();
        let fft = match WindowedFft::new(n, n, Window::Hann) {
            Ok(f) => f,
            Err(_) => unreachable!("the window fits the transform"),
        };
        let mut s = ChannelSurvey {
            rate,
            n,
            m,
            bin,
            on_factor: 0.0,
            threshold_db,
            fft,
            buf: Vec::with_capacity(n),
            acc: vec![0.0; n],
            in_decision: 0,
            ffts: 0,
            decisions: 0,
            group_of,
            groups,
            max: vec![(0.0, 0, 0); count],
            scratch: Vec::new(),
        };
        s.set_threshold(threshold_db);
        s
    }

    /// Sets the threshold, dB.
    pub fn set_threshold(&mut self, db: f64) {
        self.threshold_db = db;
        self.on_factor = 10f64
            .powf(db / 10.0)
            .max(NOISE_MARGIN * false_peak_factor(self.n, self.m));
    }

    /// The on level in force, dB over the floor.
    pub fn on_level_db(&self) -> f64 {
        10.0 * self.on_factor.log10()
    }

    /// The bin width, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.bin
    }

    /// Spectra of a decision.
    pub fn spectra_per_decision(&self) -> usize {
        self.m
    }

    /// Decisions made.
    pub fn decisions(&self) -> u64 {
        self.decisions
    }

    /// Every channel of the grid, lowest frequency first.
    pub fn channels(&self) -> &[ChannelState] {
        &self.groups
    }

    /// Forgets what the channels did.
    pub fn clear(&mut self) {
        for g in &mut self.groups {
            *g = ChannelState {
                grid_hz: g.grid_hz,
                ..ChannelState::default()
            };
        }
    }

    /// Takes samples (finite values).
    pub fn process(&mut self, x: &[Complex32]) {
        let mut rest = x;
        while !rest.is_empty() {
            let take = (self.n - self.buf.len()).min(rest.len());
            self.buf.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.buf.len() == self.n {
                let half = self.n / 2;
                let p = self.fft.run_power(&self.buf);
                for (k, &v) in p.iter().enumerate() {
                    self.acc[(k + half) % self.n] += v;
                }
                self.buf.clear();
                self.ffts += 1;
                self.in_decision += 1;
                if self.in_decision == self.m {
                    self.decide(self.ffts as f64 * self.n as f64 / self.rate);
                    self.acc.fill(0.0);
                    self.in_decision = 0;
                }
            }
        }
    }

    /// One decision at signal time `t`.
    fn decide(&mut self, t: f64) {
        self.scratch.clear();
        self.scratch.extend_from_slice(&self.acc);
        let k = ((FLOOR_QUANTILE * (self.n - 1) as f64).round() as usize).min(self.n - 1);
        let (_, floor, _) = self.scratch.select_nth_unstable_by(k, f32::total_cmp);
        let floor = f64::from(floor.max(f32::MIN_POSITIVE));
        let on = (self.on_factor * floor) as f32;
        self.max.fill((0.0, 0, 0));
        for (j, (&v, &g)) in self.acc.iter().zip(&self.group_of).enumerate() {
            let e = &mut self.max[g as usize];
            if v > e.0 {
                e.0 = v;
                e.1 = j;
            }
            if v > on {
                e.2 += 1;
            }
        }
        let dt = self.m as f64 * self.n as f64 / self.rate;
        let half = (self.n / 2) as f64;
        for (g, &(v, j, over)) in self.groups.iter_mut().zip(&self.max) {
            if v > on {
                g.hits += 1;
                g.duty = (g.duty + dt / DUTY_S).min(1.0);
                g.offset_hz = (j as f64 - half) * self.bin;
                g.width_hz = f64::from(over) * self.bin;
                g.snr_db = 10.0 * (f64::from(v) / floor).log10();
                g.last_seen_s = t;
                if g.hits >= HITS_ACTIVE && !g.active {
                    g.active = true;
                    g.ever = true;
                    g.bursts += 1;
                }
            } else {
                g.hits = 0;
                g.active = false;
                g.duty = (g.duty - dt / DUTY_S).max(0.0);
            }
        }
        self.decisions += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_of_absolute_frequencies() {
        let c = ChannelSurvey::new(1e6, 255e6, 1000.0, 5000.0, 8.0);
        assert_eq!(c.bin_hz(), 1e6 / 1024.0);
        assert_eq!(c.spectra_per_decision(), 19);
        let ch = c.channels();
        assert!(ch.iter().any(|g| g.grid_hz == 255e6));
        assert!(ch.windows(2).all(|w| w[1].grid_hz - w[0].grid_hz == 5000.0));
        // 8 dB: the noise limit, 1.5 × 2.71, is lower.
        assert!((c.on_level_db() - 8.0).abs() < 1e-9, "{}", c.on_level_db());
    }
}
