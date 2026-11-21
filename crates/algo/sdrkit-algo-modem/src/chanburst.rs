//! A burst detector over a grid of equal channels: short FFT frames, the power of each channel and its own noise floor, and
//! one burst state for each channel, so that bursts on several channels at once are all found.
//!
//! Each frame of [`GridConfig::frame`] samples is Hann-windowed and transformed. The power of a channel is the sum of the bins
//! within [`GridConfig::half_width_hz`] of its centre. The floor of each channel learns over the first
//! [`GridConfig::learn_frames`] frames, then follows its power while the channel is idle. A channel is active when its power
//! exceeds the floor by [`GridConfig::threshold_db`].
//!
//! In a run of adjacent active channels the strongest is taken and the run is extended left and right while a neighbour holds
//! at least [`GridConfig::leak_fraction`] of its power. An extent of [`GridConfig::wide_channels`] or more is one wideband
//! signal: none of its channels is analysed in that frame, and a wideband signal that lasts two frames is counted once. A
//! shorter extent is the strongest channel with its leakage: only the strongest is analysed. The rest of the run is taken in
//! the same way.
//!
//! A burst closes after [`GridConfig::close_frames`] frames without its channel. A burst shorter than
//! [`GridConfig::min_len_s`] or longer than [`GridConfig::max_len_s`] is dropped; one longer than [`GridConfig::cut_len_s`] is
//! cut there and its channel then waits for [`GridConfig::rearm_frames`] idle frames. [`GridConfig::guard_s`] of samples are
//! kept before and after.
//!
//! ```
//! use sdrkit_algo_modem::chanburst::{ChannelGrid, GridConfig};
//! use sdrkit_algo_modem::Complex32;
//! let cfg = GridConfig::new(8e6, vec![-1e6, 0.0, 1e6], 64);
//! let mut grid = ChannelGrid::new(cfg).unwrap();
//! let mut x: Vec<Complex32> = (0..40_000).map(|i| Complex32::new(1e-3 * ((i * 7 % 13) as f32 - 6.0), 0.0)).collect();
//! for (i, v) in x[20_000..21_000].iter_mut().enumerate() {
//!     let p = std::f32::consts::TAU * 1e6 / 8e6 * i as f32;
//!     *v += Complex32::new(p.cos(), p.sin());
//! }
//! let mut found = Vec::new();
//! grid.process(&x, |b| found.push(b.channel));
//! assert_eq!(found, [2]);
//! ```

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::window::Window;

/// The settings of a [`ChannelGrid`].
#[derive(Clone, Debug, PartialEq)]
pub struct GridConfig {
    /// The sample rate, Hz.
    pub rate_hz: f64,
    /// The channel centres, Hz from the centre of the samples, in grid order: neighbours in the list are neighbours in
    /// frequency.
    pub centres_hz: Vec<f64>,
    /// Bins within this distance of a centre make the power of its channel, Hz.
    pub half_width_hz: f64,
    /// Samples per FFT frame: a power of two from 16 to 4096.
    pub frame: usize,
    /// How far over its floor a channel is active, dB.
    pub threshold_db: f64,
    /// Frames over which the floor learns (old × (1 − `learn_alpha`) + new × `learn_alpha`).
    pub learn_frames: u32,
    /// The weight of a new frame while the floor learns.
    pub learn_alpha: f32,
    /// The weight of a new frame afterwards, while the channel is idle.
    pub follow_alpha: f32,
    /// Idle frames that close a burst.
    pub close_frames: u32,
    /// Idle frames that a channel waits after a cut or dropped burst.
    pub rearm_frames: u32,
    /// Shorter bursts are dropped, s.
    pub min_len_s: f64,
    /// Longer bursts are dropped, s.
    pub max_len_s: f64,
    /// Longer bursts are cut to this length, s.
    pub cut_len_s: f64,
    /// Samples kept before and after a burst, s.
    pub guard_s: f64,
    /// An extent of this many channels or more is wideband.
    pub wide_channels: usize,
    /// A neighbour with at least this share of the strongest channel's power belongs to its extent.
    pub leak_fraction: f32,
}

impl GridConfig {
    /// A grid at `rate_hz` with channels at `centres_hz` and frames of `frame` samples, half a channel spacing wide (from the
    /// first two centres), 10 dB threshold, a floor that learns over 40 frames (0.2) and follows with 0.02, close after 2 idle
    /// frames, rearm after 3, bursts of 40 µs to 3.3 ms cut at 500 µs, 6 µs guards, wideband from 5 channels, leakage at 3 %.
    pub fn new(rate_hz: f64, centres_hz: Vec<f64>, frame: usize) -> GridConfig {
        let spacing = match centres_hz.as_slice() {
            [a, b, ..] => (b - a).abs(),
            _ => rate_hz,
        };
        GridConfig {
            rate_hz,
            centres_hz,
            half_width_hz: 0.45 * spacing,
            frame,
            threshold_db: 10.0,
            learn_frames: 40,
            learn_alpha: 0.2,
            follow_alpha: 0.02,
            close_frames: 2,
            rearm_frames: 3,
            min_len_s: 40e-6,
            max_len_s: 3.3e-3,
            cut_len_s: 500e-6,
            guard_s: 6e-6,
            wide_channels: 5,
            leak_fraction: 0.03,
        }
    }
}

/// A burst on one channel.
#[derive(Clone, Debug, PartialEq)]
pub struct GridBurst {
    /// The index of the channel in [`GridConfig::centres_hz`].
    pub channel: usize,
    /// The stream index of the first sample of `samples`.
    pub start: u64,
    /// The samples at the input rate, with the guards.
    pub samples: Vec<Complex32>,
    /// The mean power of the channel over the burst above its floor, dB.
    pub snr_db: f32,
    /// The burst was longer than the cut length.
    pub cut: bool,
}

/// The burst state of a channel.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Burst {
    Idle,
    Open {
        start: u64,
        end: u64,
        idle: u32,
        power: f64,
        frames: u32,
    },
    /// The channel waits for `idle` frames without signal (after a cut or dropped burst).
    Wait {
        idle: u32,
    },
}

/// The detector.
pub struct ChannelGrid {
    cfg: GridConfig,
    fft: Fft,
    win: Vec<f32>,
    frame: Vec<Complex32>,
    bins: Vec<Vec<usize>>,
    power: Vec<f32>,
    floor: Vec<f32>,
    active: Vec<bool>,
    analysed: Vec<bool>,
    wide: Vec<bool>,
    was_wide: Vec<bool>,
    taken: Vec<bool>,
    state: Vec<Burst>,
    hist: SampleRing<Complex32>,
    frames: u64,
    wideband: u64,
    wide_frames: u32,
    thr: f32,
    min_len: u64,
    max_len: u64,
    cut_len: u64,
    guard: u64,
}

impl ChannelGrid {
    /// A detector with settings `cfg`. Errors for a rate that is not positive, no channel, a frame size that is not a power of
    /// two from 16 to 4096, or a threshold or length that is not finite and positive.
    pub fn new(cfg: GridConfig) -> Result<ChannelGrid, ModemError> {
        Param::Rate.check_positive(cfg.rate_hz)?;
        Param::Carriers.check_count(cfg.centres_hz.len(), 1, 4096)?;
        if !cfg.frame.is_power_of_two() || !(16..=4096).contains(&cfg.frame) {
            return Param::FftSize.range(16.0, 4096.0, cfg.frame as f64);
        }
        Param::Threshold.check_positive(cfg.threshold_db)?;
        Param::MaxLen.check_positive(cfg.max_len_s)?;
        Param::Width.check_positive(cfg.half_width_hz)?;
        let n = cfg.frame;
        let bin_hz = cfg.rate_hz / n as f64;
        let bins = cfg
            .centres_hz
            .iter()
            .map(|&c| {
                (0..n)
                    .filter(|&k| {
                        let s = if k < n / 2 {
                            k as f64
                        } else {
                            k as f64 - n as f64
                        };
                        (s * bin_hz - c).abs() <= cfg.half_width_hz
                    })
                    .collect()
            })
            .collect();
        let m = cfg.centres_hz.len();
        let samples = |s: f64| (s * cfg.rate_hz).round().max(0.0) as u64;
        let keep = (samples(cfg.max_len_s + 2.0 * cfg.guard_s) as usize
            + (cfg.rearm_frames.max(cfg.close_frames) as usize + 4) * n)
            .next_power_of_two();
        Ok(ChannelGrid {
            fft: Fft::forward(n),
            win: Window::Hann.coefficients(n),
            frame: Vec::with_capacity(n),
            bins,
            power: vec![0.0; m],
            floor: vec![0.0; m],
            active: vec![false; m],
            analysed: vec![false; m],
            wide: vec![false; m],
            was_wide: vec![false; m],
            taken: vec![false; m],
            state: vec![Burst::Idle; m],
            hist: SampleRing::new(keep).map_err(ModemError::Dsp)?,
            frames: 0,
            wideband: 0,
            wide_frames: 0,
            thr: 10f32.powf(cfg.threshold_db as f32 / 10.0),
            min_len: samples(cfg.min_len_s),
            max_len: samples(cfg.max_len_s),
            cut_len: samples(cfg.cut_len_s),
            guard: samples(cfg.guard_s),
            cfg,
        })
    }

    /// The settings.
    pub fn config(&self) -> &GridConfig {
        &self.cfg
    }

    /// Wideband bursts counted so far.
    pub fn wideband(&self) -> u64 {
        self.wideband
    }

    /// The floor of channel `c` (the sum of its bins' power), 0 before the first frame.
    pub fn floor(&self, c: usize) -> f32 {
        self.floor.get(c).copied().unwrap_or(0.0)
    }

    /// The stream index of the next sample.
    pub fn position(&self) -> u64 {
        self.hist.end()
    }

    /// Forgets the stream, the floors and every open burst.
    pub fn reset(&mut self) {
        self.frame.clear();
        self.hist.reset();
        self.frames = 0;
        self.floor.fill(0.0);
        self.state.fill(Burst::Idle);
        self.was_wide.fill(false);
        self.wide_frames = 0;
    }

    /// Takes `x`; each finished burst goes to `found`.
    pub fn process(&mut self, x: &[Complex32], mut found: impl FnMut(GridBurst)) {
        let n = self.cfg.frame;
        for &s in x {
            self.hist.push(s);
            self.frame.push(s);
            if self.frame.len() == n {
                let end = self.hist.end();
                self.analyse(end - n as u64, &mut found);
                self.frame.clear();
            }
        }
    }

    fn analyse(&mut self, at: u64, found: &mut impl FnMut(GridBurst)) {
        for (x, w) in self.frame.iter_mut().zip(&self.win) {
            *x *= *w;
        }
        self.fft.process(&mut self.frame);
        for (p, bins) in self.power.iter_mut().zip(&self.bins) {
            *p = bins.iter().map(|&k| self.frame[k].norm_sqr()).sum();
        }
        self.frames += 1;
        let learning = self.frames <= u64::from(self.cfg.learn_frames);
        if learning {
            let a = if self.frames == 1 {
                1.0
            } else {
                self.cfg.learn_alpha
            };
            for (f, &p) in self.floor.iter_mut().zip(&self.power) {
                *f = ((1.0 - a) * *f + a * p).max(1e-30);
            }
            return;
        }
        for c in 0..self.power.len() {
            self.active[c] = self.power[c] > self.floor[c] * self.thr;
        }
        self.extents();
        let n = self.cfg.frame as u64;
        for c in 0..self.power.len() {
            if !self.active[c] {
                let a = self.cfg.follow_alpha;
                self.floor[c] = ((1.0 - a) * self.floor[c] + a * self.power[c]).max(1e-30);
            }
            let on = self.analysed[c];
            let over = f64::from(self.power[c] / self.floor[c]);
            // The first frame of a strong burst can look wide (its edge spreads over the band): the burst starts there.
            let back = if self.was_wide[c] { n } else { 0 };
            self.state[c] = match self.state[c] {
                Burst::Idle if on => Burst::Open {
                    start: at.saturating_sub(back),
                    end: at + n,
                    idle: 0,
                    power: over,
                    frames: 1,
                },
                Burst::Idle => Burst::Idle,
                Burst::Open {
                    start,
                    end,
                    idle,
                    power,
                    frames,
                } => {
                    let (end, idle, power, frames) = if on {
                        (at + n, 0, power + over, frames + 1)
                    } else {
                        (end, idle + 1, power, frames)
                    };
                    if end - start > self.max_len {
                        Burst::Wait { idle: 0 }
                    } else if idle >= self.cfg.close_frames
                        && (end - start <= self.cut_len || idle >= self.cfg.rearm_frames)
                    {
                        self.emit(c, start, end, power / f64::from(frames), found);
                        Burst::Idle
                    } else {
                        Burst::Open {
                            start,
                            end,
                            idle,
                            power,
                            frames,
                        }
                    }
                }
                Burst::Wait { idle } => {
                    let idle = if self.active[c] { 0 } else { idle + 1 };
                    if idle >= self.cfg.rearm_frames {
                        Burst::Idle
                    } else {
                        Burst::Wait { idle }
                    }
                }
            };
        }
        self.was_wide.copy_from_slice(&self.wide);
    }

    /// Marks the analysed channels of this frame and counts new wideband signals.
    fn extents(&mut self) {
        let m = self.power.len();
        self.analysed.fill(false);
        self.wide.fill(false);
        let mut taken = std::mem::take(&mut self.taken);
        taken.clear();
        taken.resize(m, false);
        // The strongest active channel not yet taken, until none is left.
        while let Some(s) = (0..m)
            .filter(|&c| self.active[c] && !taken[c])
            .max_by(|&a, &b| self.power[a].total_cmp(&self.power[b]))
        {
            let keep = self.power[s] * self.cfg.leak_fraction;
            let joins =
                |c: usize, taken: &[bool]| self.active[c] && !taken[c] && self.power[c] >= keep;
            let (mut lo, mut hi) = (s, s);
            while lo > 0 && joins(lo - 1, &taken) {
                lo -= 1;
            }
            while hi + 1 < m && joins(hi + 1, &taken) {
                hi += 1;
            }
            for t in &mut taken[lo..=hi] {
                *t = true;
            }
            if hi - lo + 1 >= self.cfg.wide_channels {
                for w in &mut self.wide[lo..=hi] {
                    *w = true;
                }
            } else {
                self.analysed[s] = true;
            }
        }
        self.taken = taken;
        // A wide signal counts once it lasts two frames: the edge of a strong burst alone spreads over one frame.
        self.wide_frames = if self.wide.iter().any(|&w| w) {
            self.wide_frames + 1
        } else {
            0
        };
        if self.wide_frames == 2 {
            self.wideband += 1;
        }
    }

    fn emit(
        &mut self,
        c: usize,
        start: u64,
        end: u64,
        mean_over: f64,
        found: &mut impl FnMut(GridBurst),
    ) {
        let len = end - start;
        if len < self.min_len {
            return;
        }
        let cut = len > self.cut_len;
        let end = if cut { start + self.cut_len } else { end };
        let from = start.saturating_sub(self.guard).max(self.hist.start());
        let to = (end + self.guard).min(self.hist.end());
        if to <= from {
            return;
        }
        let mut samples = vec![Complex32::default(); (to - from) as usize];
        self.hist.copy(from, &mut samples);
        found(GridBurst {
            channel: c,
            start: from,
            samples,
            snr_db: (10.0 * (mean_over - 1.0).max(1e-3).log10()) as f32,
            cut,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(n: usize, sigma: f32, seed: u64) -> Vec<Complex32> {
        let mut s = seed | 1;
        let mut u = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f32 / (1u64 << 53) as f32 - 0.5
        };
        (0..n)
            .map(|_| Complex32::new(sigma * 3.46 * u(), sigma * 3.46 * u()))
            .collect()
    }

    fn tone(x: &mut [Complex32], rate: f64, hz: f64, amp: f32) {
        for (i, v) in x.iter_mut().enumerate() {
            let p = (std::f64::consts::TAU * hz / rate * i as f64) as f32;
            *v += Complex32::new(amp * p.cos(), amp * p.sin());
        }
    }

    fn grid(rate: f64) -> ChannelGrid {
        let centres = (-9..=9).map(|k| f64::from(k) * 1e6).collect();
        ChannelGrid::new(GridConfig::new(rate, centres, 128)).unwrap()
    }

    #[test]
    fn bursts_on_two_channels_at_once_are_both_found() {
        let rate = 20e6;
        let mut x = noise(400_000, 0.01, 3);
        tone(&mut x[100_000..106_000], rate, 3e6, 0.5);
        tone(&mut x[101_000..107_000], rate, -5e6, 0.3);
        let mut g = grid(rate);
        let mut got = Vec::new();
        g.process(&x, |b| {
            got.push((b.channel, b.start, b.samples.len(), b.cut))
        });
        got.sort();
        assert_eq!(got.len(), 2, "{got:?}");
        assert_eq!((got[0].0, got[1].0), (4, 12));
        for (_, start, len, cut) in got {
            assert!(start <= 101_000 && start + len as u64 >= 106_000);
            assert!(!cut);
        }
    }

    #[test]
    fn a_wide_signal_is_counted_once_and_not_analysed() {
        let rate = 20e6;
        let mut x = noise(300_000, 0.01, 5);
        for k in -3..=3 {
            tone(
                &mut x[100_000..110_000],
                rate,
                f64::from(k) * 1e6 + 0.1e6,
                0.3,
            );
        }
        let mut g = grid(rate);
        let mut got = 0;
        g.process(&x, |_| got += 1);
        assert_eq!((got, g.wideband()), (0, 1));
    }

    #[test]
    fn long_bursts_are_cut_and_longer_ones_dropped() {
        let rate = 20e6;
        let mut x = noise(600_000, 0.01, 9);
        tone(&mut x[100_000..120_000], rate, 0.0, 0.5);
        tone(&mut x[300_000..400_000], rate, 0.0, 0.5);
        let mut g = grid(rate);
        let mut got = Vec::new();
        g.process(&x, |b| got.push((b.start, b.samples.len(), b.cut)));
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].2 && got[0].1 < 10_500);
    }

    #[test]
    fn a_strong_neighbour_does_not_hide_a_weak_channel() {
        let rate = 20e6;
        let mut x = noise(300_000, 0.003, 11);
        tone(&mut x[100_000..106_000], rate, 1e6, 1.0);
        tone(&mut x[100_000..106_000], rate, 0.0, 0.03);
        let mut g = grid(rate);
        let mut got = Vec::new();
        g.process(&x, |b| got.push(b.channel));
        assert!(got.contains(&9) && got.contains(&10), "{got:?}");
    }
}
