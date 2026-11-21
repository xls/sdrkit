//! Snapshots of a spectrum and the narrow peaks in them, followed from snapshot to snapshot with a presence history.
//!
//! - [`Snapshots`]: the mean power spectrum of `K` consecutive, non-overlapping windowed FFT blocks (Hann unless another
//!   window is given), in signed bin order, with the mean noise per bin from the median bin ([`BinMedian`] of `K` frames).
//! - [`snapshot_peaks`]: bins at least a threshold over the noise with no strictly higher bin within 4, outside the DC bins;
//!   each with a refined frequency (log parabola), a level (the 5 bins around it over the noise) and a narrowness (those 5
//!   bins over the 17 around it: near 1 for a carrier, lower for a modulated signal).
//! - [`PeakTracker`]: follows the peaks with a [`LineTracker`] in bin units with a fixed gate, and keeps for each track the
//!   level and the frequency of each recent snapshot (empty when it was not seen), a smoothed frequency and narrowness.

use crate::fft::Fft;
use crate::floor::BinMedian;
use crate::linetrack::{LinePeak, LineTracker, TrackParams};
use crate::peak::log_parabolic;
use crate::window::Window;
use crate::{DspError, Param};
use num_complex::Complex32;
use std::collections::VecDeque;

/// One snapshot of [`Snapshots`].
#[derive(Debug)]
pub struct Snapshot<'a> {
    /// The snapshot number from 0.
    pub index: u64,
    /// The mean bin powers `|X|^2` of the blocks, signed order: element `i` is bin `i - n / 2`.
    pub power: &'a [f32],
    /// The mean noise per bin.
    pub noise: f32,
}

/// Averaged spectra of consecutive blocks (module docs).
#[derive(Clone, Debug)]
pub struct Snapshots {
    n: usize,
    blocks: usize,
    fft: Fft,
    win: Vec<f32>,
    buf: Vec<Complex32>,
    fill: usize,
    acc: Vec<f64>,
    done: usize,
    mean: Vec<f32>,
    floor: BinMedian,
    count: u64,
}

impl Snapshots {
    /// Snapshots of `blocks` blocks of `n` points (a power of two from 16).
    ///
    /// # Errors
    ///
    /// [`DspError::OutOfRange`] for another `n` or no block.
    pub fn new(n: usize, blocks: usize) -> Result<Self, DspError> {
        Self::with_window(n, blocks, Window::Hann)
    }

    /// The same with the periodic window `window` in place of Hann.
    ///
    /// # Errors
    ///
    /// As [`Snapshots::new`].
    pub fn with_window(n: usize, blocks: usize, window: Window) -> Result<Self, DspError> {
        if !n.is_power_of_two() || n < 16 {
            return Param::FftSize.at_least(16.0, n as f64);
        }
        if blocks == 0 {
            return Param::Frames.at_least(1.0, 0.0);
        }
        let mut floor = BinMedian::new(u32::try_from(blocks).unwrap_or(u32::MAX))?;
        floor.reserve(n);
        Ok(Snapshots {
            n,
            blocks,
            fft: Fft::forward(n),
            win: window.coefficients(n),
            buf: vec![Complex32::default(); n],
            fill: 0,
            acc: vec![0.0; n],
            done: 0,
            mean: vec![0.0; n],
            floor,
            count: 0,
        })
    }

    /// The points of a block.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Never true.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The blocks of a snapshot.
    pub fn blocks(&self) -> usize {
        self.blocks
    }

    /// The samples of a snapshot.
    pub fn samples(&self) -> usize {
        self.n * self.blocks
    }

    /// Forgets the part of a snapshot taken so far; numbers go on.
    pub fn reset(&mut self) {
        self.fill = 0;
        self.done = 0;
        self.acc.fill(0.0);
    }

    /// Takes `x` and calls `f` for each snapshot that it completes.
    pub fn push(&mut self, x: &[Complex32], mut f: impl FnMut(&Snapshot)) {
        let n = self.n;
        let mut rest = x;
        while !rest.is_empty() {
            let take = (n - self.fill).min(rest.len());
            for (k, z) in rest[..take].iter().enumerate() {
                let i = self.fill + k;
                self.buf[i] = z * self.win[i];
            }
            self.fill += take;
            rest = &rest[take..];
            if self.fill < n {
                break;
            }
            self.fill = 0;
            self.fft.process(&mut self.buf);
            for (i, a) in self.acc.iter_mut().enumerate() {
                *a += f64::from(self.buf[(i + n / 2) % n].norm_sqr());
            }
            self.done += 1;
            if self.done < self.blocks {
                continue;
            }
            let k = self.blocks as f64;
            for (m, a) in self.mean.iter_mut().zip(self.acc.iter_mut()) {
                *m = (*a / k) as f32;
                *a = 0.0;
            }
            self.done = 0;
            let noise = self
                .floor
                .estimate(self.mean.iter().copied())
                .unwrap_or(0.0);
            f(&Snapshot {
                index: self.count,
                power: &self.mean,
                noise,
            });
            self.count += 1;
        }
    }
}

/// A narrow peak of a snapshot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapshotPeak {
    /// The signed bin, refined (a fraction of a bin).
    pub bin: f64,
    /// The power of the 5 bins around the peak over the noise per bin, dB.
    pub level_db: f64,
    /// The power of the 5 bins around the peak over that of the 17 around it.
    pub narrowness: f64,
}

/// Appends to `out` the peaks of `power` (signed order) with `|signed bin| > dc`, power at least `threshold_db` over `noise`,
/// and no strictly higher bin within 4; at most the `max` strongest, the strongest first.
pub fn snapshot_peaks(
    power: &[f32],
    noise: f32,
    threshold_db: f64,
    dc: i64,
    max: usize,
    out: &mut Vec<SnapshotPeak>,
) {
    let start = out.len();
    let n = power.len() as i64;
    let half = n / 2;
    let noise = f64::from(noise).max(1e-30);
    let at = |i: i64| f64::from(power[i.clamp(0, n - 1) as usize]);
    let min = noise * 10f64.powf(threshold_db / 10.0);
    for i in 4..n - 4 {
        let j = i - half;
        let p = at(i);
        if j.abs() <= dc || p < min || (i - 4..=i + 4).any(|k| at(k) > p) {
            continue;
        }
        let r = log_parabolic(at(i - 1), p, at(i + 1));
        let near: f64 = (i - 2..=i + 2).map(at).sum();
        let wide: f64 = (i - 8..=i + 8).map(at).sum();
        out.push(SnapshotPeak {
            bin: j as f64 + r.offset,
            level_db: 10.0 * (near / noise).log10(),
            narrowness: near / wide.max(1e-30),
        });
    }
    out[start..].sort_by(|a, b| b.level_db.total_cmp(&a.level_db));
    out.truncate(start + max);
}

/// What a [`PeakTracker`] keeps for one snapshot of a track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    /// The level, dB.
    pub level_db: f64,
    /// The measured frequency, signed bins.
    pub bin: f64,
}

/// One track of a [`PeakTracker`].
#[derive(Clone, Debug, PartialEq)]
pub struct PeakTrack {
    /// The number of its line track.
    pub id: u64,
    /// One entry per snapshot since it started, the newest last; at most the history depth.
    pub history: VecDeque<Option<Seen>>,
    /// The index of the snapshot of the newest entry.
    pub last_snapshot: u64,
    /// The smoothed frequency, signed bins: `0.8 f + 0.2 f_new`.
    pub bin: f64,
    /// The smoothed narrowness: `0.9 n + 0.1 n_new`.
    pub narrowness: f64,
}

impl PeakTrack {
    /// Seen in one of the newest `k` snapshots.
    pub fn seen_within(&self, k: usize) -> bool {
        self.history.iter().rev().take(k).any(Option::is_some)
    }
}

/// Follows snapshot peaks (module docs).
#[derive(Clone, Debug)]
pub struct PeakTracker {
    lines: LineTracker,
    tracks: Vec<PeakTrack>,
    depth: usize,
    snapshots: u64,
}

impl PeakTracker {
    /// A tracker with a gate of `gate_bins`, a history of `depth` snapshots, that drops a track not seen for more than
    /// `drop_after` snapshots, with at most `max_tracks` tracks.
    pub fn new(gate_bins: f64, depth: usize, drop_after: usize, max_tracks: usize) -> Self {
        let mut p = TrackParams::new(0.0, gate_bins, 0.0);
        p.end_after_s = drop_after as f64 + 1.0;
        p.keep_min_s = f64::INFINITY;
        p.max_active = max_tracks;
        p.max_points = 2 * depth.max(16);
        PeakTracker {
            lines: LineTracker::new(p),
            tracks: Vec::new(),
            depth: depth.max(1),
            snapshots: 0,
        }
    }

    /// The snapshots taken.
    pub fn snapshots(&self) -> u64 {
        self.snapshots
    }

    /// The tracks, in the order they started.
    pub fn tracks(&self) -> &[PeakTrack] {
        &self.tracks
    }

    /// Forgets every track.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.tracks.clear();
        self.snapshots = 0;
    }

    /// Takes the peaks of the next snapshot.
    pub fn update(&mut self, peaks: &[SnapshotPeak]) {
        let t = self.snapshots as f64;
        let line: Vec<LinePeak> = peaks
            .iter()
            .map(|p| LinePeak {
                hz: p.bin,
                level_db: p.level_db,
            })
            .collect();
        self.lines.update(t, &line);
        let _ = self.lines.take_finished();
        let active = self.lines.active();
        self.tracks
            .retain(|tr| active.iter().any(|a| a.id() == tr.id));
        for a in active {
            let last = a.last();
            let seen = (last.t == t)
                .then(|| peaks.iter().find(|p| p.bin == last.hz))
                .flatten();
            let k = match self.tracks.iter().position(|tr| tr.id == a.id()) {
                Some(k) => k,
                None => {
                    self.tracks.push(PeakTrack {
                        id: a.id(),
                        history: VecDeque::new(),
                        last_snapshot: self.snapshots,
                        bin: last.hz,
                        narrowness: seen.map_or(0.0, |p| p.narrowness),
                    });
                    self.tracks.len() - 1
                }
            };
            let tr = &mut self.tracks[k];
            if let Some(p) = seen {
                if !tr.history.is_empty() {
                    tr.bin = 0.8 * tr.bin + 0.2 * p.bin;
                    tr.narrowness = 0.9 * tr.narrowness + 0.1 * p.narrowness;
                }
            }
            tr.history.push_back(seen.map(|p| Seen {
                level_db: p.level_db,
                bin: p.bin,
            }));
            if tr.history.len() > self.depth {
                tr.history.pop_front();
            }
            tr.last_snapshot = self.snapshots;
        }
        self.snapshots += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone_peak(bin: f64) -> SnapshotPeak {
        SnapshotPeak {
            bin,
            level_db: 30.0,
            narrowness: 0.95,
        }
    }

    #[test]
    fn a_carrier_in_noise_is_one_narrow_peak_at_its_frequency() {
        let n = 1024;
        let mut s = Snapshots::new(n, 8).unwrap();
        let mut seed = 11u64;
        let mut u = || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let x: Vec<Complex32> = (0..n * 8)
            .map(|i| {
                let r = (-u().ln()).sqrt() * 0.01;
                let a = std::f64::consts::TAU * u();
                let c = Complex32::from_polar(
                    0.2,
                    (std::f64::consts::TAU * 100.3 * i as f64 / n as f64) as f32,
                );
                c + Complex32::new((r * a.cos()) as f32, (r * a.sin()) as f32)
            })
            .collect();
        let mut peaks = Vec::new();
        let mut noise = 0.0;
        s.push(&x, |snap| {
            noise = snap.noise;
            snapshot_peaks(snap.power, snap.noise, 10.0, 3, 200, &mut peaks);
        });
        // Noise of power 1e-4 per sample in a Hann block: 1e-4 x 384 per bin.
        assert!(
            (f64::from(noise) / (1e-4 * 384.0) - 1.0).abs() < 0.1,
            "{noise}"
        );
        assert_eq!(peaks.len(), 1, "{peaks:?}");
        assert!((peaks[0].bin - 100.3).abs() < 0.05, "{:?}", peaks[0]);
        assert!(peaks[0].narrowness > 0.95);
    }

    #[test]
    fn a_steady_tone_keeps_one_track_and_a_stopped_one_is_dropped_after_60() {
        let mut t = PeakTracker::new(2.5, 240, 60, 200);
        for k in 0..300 {
            let mut p = vec![tone_peak(500.0 + 0.001 * k as f64)];
            if k < 100 {
                p.push(tone_peak(-300.0));
            }
            t.update(&p);
            // The last sight is snapshot 99: 60 misses keep the track, the 61st drops it.
            match k {
                159 => assert_eq!(t.tracks().len(), 2),
                160 => assert_eq!(t.tracks().len(), 1),
                _ => {}
            }
        }
        assert_eq!(t.tracks().len(), 1);
        let tr = &t.tracks()[0];
        assert_eq!(tr.history.len(), 240);
        assert!(tr.history.iter().all(Option::is_some));
        assert!((tr.bin - 500.3).abs() < 0.01);
    }

    #[test]
    fn two_tones_three_bins_apart_make_two_tracks() {
        let mut t = PeakTracker::new(2.5, 240, 60, 200);
        for _ in 0..50 {
            t.update(&[tone_peak(200.0), tone_peak(203.0)]);
        }
        assert_eq!(t.tracks().len(), 2);
        assert!(t
            .tracks()
            .iter()
            .all(|tr| tr.history.iter().all(Option::is_some)));
    }
}
