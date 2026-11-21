//! FMCW sweep detection: a Hann FFT of about 1 ms with 50 % overlap, the per-bin floor with its burst
//! guard, peaks, tracks and the straight-line fit of each sweep. It also outputs the excess over the floor in 5 ms frames, for
//! the waterfall and the Morse ID.

use sdrkit_algo_dsp::peak::parabolic;
use sdrkit_algo_dsp::spectrum::{SpectrumAnalyzer, SpectrumConfig};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::{Complex32, ConfigError, Values};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Item, Kind, Port, TagKind, Timed,
};
use serde::{Deserialize, Serialize};

/// The FFT size for a rate: 512 doubled while below 0.75 x rate / 1000, at most 8192.
pub fn fft_size(rate: f64) -> usize {
    let mut n = 512;
    while (n as f64) < 0.75 * rate / 1000.0 && n < 8192 {
        n *= 2;
    }
    n
}

/// A fitted sweep (`hfradar.sweep`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sweep {
    /// Signal time of its start, seconds.
    pub start: f64,
    pub duration: f64,
    /// Offsets from the radio centre at the start and the end, Hz.
    pub f_start: f64,
    pub f_end: f64,
    pub centre: f64,
    pub width: f64,
    /// Hz per second (signed).
    pub rate: f64,
    pub snr_db: f64,
    pub level_dbfs: f64,
}

/// What the detector reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Detection {
    Sweep(Sweep),
    /// A track that was not a sweep (carriers, ionosondes, other chirps).
    Rejected,
    /// A broadband burst frame.
    Burst,
    /// The input of one block: the sum of the sample powers and the number of samples.
    Power {
        /// The sum of |x|^2.
        sum: f64,
        /// Samples.
        n: u64,
    },
    /// The detector restarted (a retune of more than 1 kHz): the new radio centre and tuned frequency, Hz.
    Reset {
        /// The frequency at 0 Hz in the stream.
        centre_hz: f64,
        /// Where the radio is tuned.
        tuned_hz: f64,
    },
}

impl Item for Detection {
    const KIND: &'static str = "hfradar.sweep";
}

#[derive(Clone, Debug, Default)]
struct Track {
    // Least-squares sums over all points (t in frames from the first, f in bins).
    n: u64,
    st: f64,
    sf: f64,
    stt: f64,
    stf: f64,
    sff: f64,
    first: u64,
    last: u64,
    last_f: f64,
    /// The newest points: frames from the first, bins, excess dB.
    recent: [(f64, f64, f64); 10],
    ri: usize,
    slope: f64,
    excess_db: f64,
    peak: f64,
    used: bool,
    /// Frames since the last point whose predicted bin was masked: they do not count as idle.
    coast: u64,
}

impl Track {
    fn new(frame: u64, f: f64, ex_db: f64, p: f64) -> Track {
        let mut t = Track {
            first: frame,
            ..Default::default()
        };
        t.add(frame, f, ex_db, p);
        t
    }
    fn add(&mut self, frame: u64, f: f64, ex_db: f64, p: f64) {
        let t = (frame - self.first) as f64;
        self.n += 1;
        self.st += t;
        self.sf += f;
        self.stt += t * t;
        self.stf += t * f;
        self.sff += f * f;
        self.last = frame;
        self.last_f = f;
        self.coast = 0;
        self.recent[self.ri % 10] = (t, f, ex_db);
        self.ri += 1;
        self.excess_db += ex_db;
        self.peak = self.peak.max(p);
        if self.n >= 4 {
            let k = self.ri.min(10);
            let (mut a, mut b, mut c, mut d) = (0.0, 0.0, 0.0, 0.0);
            for &(x, y, _) in &self.recent[..k] {
                a += x;
                b += y;
                c += x * x;
                d += x * y;
            }
            let kf = k as f64;
            let den = kf * c - a * a;
            self.slope = if den.abs() > 1e-12 {
                (kf * d - a * b) / den
            } else {
                0.0
            };
        }
    }
    /// Drops the newest 1 to 3 points while they follow a gap of more than `gap` frames: noise peaks taken near the
    /// predicted line after the sweep ended would stretch the fitted width.
    fn trim_tail(&mut self, gap: f64) {
        let mut k = self.ri.min(10);
        let base = self.ri - k;
        let at = |i: usize| self.recent[(base + i) % 10];
        let before = k;
        'trim: loop {
            for tail in 1..=3 {
                if k < tail + 2 {
                    break 'trim;
                }
                if at(k - tail).0 - at(k - tail - 1).0 > gap {
                    for i in k - tail..k {
                        let (t, f, ex) = at(i);
                        self.n -= 1;
                        self.st -= t;
                        self.sf -= f;
                        self.stt -= t * t;
                        self.stf -= t * f;
                        self.sff -= f * f;
                        self.excess_db -= ex;
                    }
                    k -= tail;
                    continue 'trim;
                }
            }
            break;
        }
        if k < before {
            let (t, f, _) = at(k - 1);
            self.last = self.first + t as u64;
            self.last_f = f;
        }
    }
    fn predict(&self, frame: u64) -> f64 {
        self.last_f + self.slope * (frame - self.last) as f64
    }
    /// Slope (bins per frame), intercept (bins at t = 0) and the rms residual (bins).
    fn fit(&self) -> (f64, f64, f64) {
        let n = self.n as f64;
        let den = n * self.stt - self.st * self.st;
        let b = if den.abs() > 1e-12 {
            (n * self.stf - self.st * self.sf) / den
        } else {
            0.0
        };
        let a = (self.sf - b * self.st) / n;
        let sse = self.sff - 2.0 * a * self.sf - 2.0 * b * self.stf
            + n * a * a
            + 2.0 * a * b * self.st
            + b * b * self.stt;
        (b, a, (sse.max(0.0) / n).sqrt())
    }
}

/// The detector as a component: `Iq` in; `hfradar.sweep` and the 5 ms excess frames (`Spectrum`, linear excess over the floor) out.
pub struct SweepDetector {
    rate: f64,
    n: usize,
    hop: usize,
    spectrum: Option<SpectrumAnalyzer>,
    pow: Vec<f64>,
    floor: Vec<f64>,
    /// A second floor held at 1.5 x itself, for the mask: a passing sweep barely raises it, a steady signal does.
    steady: Vec<f64>,
    frames: u64,
    lo: usize,
    hi: usize,
    tracks: Vec<Track>,
    peaks: Vec<(f64, f64, f64)>,
    kept: Vec<(f64, f64, f64)>,
    /// Bins held by a steady carrier and its sidebands: they take no peaks.
    mask: Vec<bool>,
    scratch: Vec<f64>,
    out: Vec<Detection>,
    acc: Vec<f64>,
    acc_n: u32,
    bucket: u64,
    bucket_len: f64,
    samples: u64,
    frames_out: Vec<f32>,
    centre_hz: f64,
    since_start: u64,
    /// Samples before the last restart, so sweep times run on across a retune.
    base: u64,
}

impl Default for SweepDetector {
    fn default() -> Self {
        SweepDetector {
            rate: 1.0,
            n: 512,
            hop: 256,
            spectrum: None,
            pow: Vec::new(),
            floor: Vec::new(),
            steady: Vec::new(),
            frames: 0,
            lo: 0,
            hi: 0,
            tracks: Vec::new(),
            peaks: Vec::new(),
            kept: Vec::with_capacity(8),
            mask: Vec::new(),
            scratch: Vec::new(),
            out: Vec::new(),
            acc: Vec::new(),
            acc_n: 0,
            bucket: 0,
            bucket_len: 1.0,
            samples: 0,
            frames_out: Vec::new(),
            centre_hz: 0.0,
            since_start: 0,
            base: 0,
        }
    }
}

impl SweepDetector {
    fn setup(&mut self, rate: f64) {
        let n = fft_size(rate);
        self.rate = rate;
        self.n = n;
        self.hop = n / 2;
        // The hop is a power of two, so rate / (rate / hop) equals the hop exactly.
        let cfg = SpectrumConfig {
            window: Window::Hann,
            frames_per_second: rate / self.hop as f64,
            ..SpectrumConfig::new(n, rate)
        };
        self.spectrum = SpectrumAnalyzer::new(cfg).ok();
        self.pow = vec![0.0; n];
        self.floor = vec![0.0; n];
        self.steady = vec![0.0; n];
        self.mask = vec![false; n];
        self.scratch = Vec::with_capacity(n);
        self.frames = 0;
        let half = (0.45 * n as f64) as usize;
        self.lo = n / 2 - half;
        self.hi = n / 2 + half;
        self.tracks.clear();
        self.acc = vec![0.0; self.hi - self.lo];
        self.acc_n = 0;
        self.bucket_len = rate * 0.005;
        self.bucket = (self.samples as f64 / self.bucket_len) as u64;
        self.since_start = 0;
        self.base = self.samples;
    }

    /// Bin width, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.rate / self.n as f64
    }

    fn frame(&mut self) {
        let Some(sa) = self.spectrum.as_ref() else {
            return;
        };
        // Centred: index i is bin i - n/2. A tone of magnitude 1 on a bin reads 1.
        for (p, &v) in self.pow.iter_mut().zip(sa.power()) {
            *p = v as f64;
        }
        self.frames += 1;
        let fps = self.rate / self.hop as f64;
        if self.frames <= 64 {
            // The steady floor starts from the mean of the first 64 frames with each power held at 3 x the median bin
            // power of its frame: a sweep passing then barely raises it, a steady carrier does.
            self.scratch.clear();
            self.scratch.extend_from_slice(&self.pow[self.lo..self.hi]);
            let mid = self.scratch.len() / 2;
            let (_, &mut median, _) = self.scratch.select_nth_unstable_by(mid, f64::total_cmp);
            for i in self.lo..self.hi {
                self.floor[i] += self.pow[i] / 64.0;
                self.steady[i] += self.pow[i].min(3.0 * median) / 64.0;
            }
            if self.frames == 64 {
                self.update_mask();
            }
            self.bucket_add();
            return;
        }
        // A floor never lies more than 60 dB under the mean bin power. Without noise (a clean test signal) the floor is rounding
        // noise, and the far sidelobes of the window stand above it.
        let mean = self.pow[self.lo..self.hi].iter().sum::<f64>() / (self.hi - self.lo) as f64;
        for f in &mut self.floor[self.lo..self.hi] {
            *f = f.max(1e-6 * mean);
        }
        for f in &mut self.steady[self.lo..self.hi] {
            *f = f.max(1e-6 * mean);
        }
        // The burst guard.
        let k8 = 10f64.powf(0.8);
        let over = (self.lo..self.hi)
            .filter(|&i| self.pow[i] > k8 * self.floor[i])
            .count();
        if over * 10 > self.hi - self.lo {
            self.out.push(Detection::Burst);
            return;
        }
        let a = 1.0 / fps.max(50.0);
        for i in self.lo..self.hi {
            let p = self.pow[i].min(3.0 * self.floor[i]);
            self.floor[i] += a * (p - self.floor[i]);
            let q = self.pow[i].min(1.5 * self.steady[i]);
            self.steady[i] += a * (q - self.steady[i]);
        }
        if self.frames.is_multiple_of(64) {
            self.update_mask();
        }
        self.bucket_add();
        self.peaks_and_tracks();
    }

    /// Marks the bins within 3 kHz (the band of speech) of a bin whose steady floor stands more than 3 dB over the median steady floor: a
    /// steady carrier and the sidebands of its speech. Peaks there would chain from carrier to carrier into false sweeps.
    /// A track coasts through masked bins. Made every 64 frames.
    fn update_mask(&mut self) {
        self.scratch.clear();
        self.scratch
            .extend_from_slice(&self.steady[self.lo..self.hi]);
        let mean = self.scratch.iter().sum::<f64>() / self.scratch.len() as f64;
        let mid = self.scratch.len() / 2;
        let (_, &mut median, _) = self.scratch.select_nth_unstable_by(mid, f64::total_cmp);
        let k = (3e3 / self.bin_hz()).ceil() as usize;
        self.mask.fill(false);
        // Without noise the floors sit at their lower limit, far under their mean: no mask.
        if median <= 1e-4 * mean {
            return;
        }
        for i in self.lo..self.hi {
            if self.steady[i] > 2.0 * median {
                let a = i.saturating_sub(k).max(self.lo);
                let b = (i + k).min(self.hi - 1);
                self.mask[a..=b].fill(true);
            }
        }
    }

    fn bucket_add(&mut self) {
        if self.frames <= 64 {
            return;
        }
        let centre = self.samples.saturating_sub(self.n as u64 / 2);
        let b = (centre as f64 / self.bucket_len) as u64;
        if b != self.bucket {
            if self.acc_n > 0 {
                for v in self.acc.iter_mut() {
                    self.frames_out.push((*v / self.acc_n as f64) as f32);
                    *v = 0.0;
                }
                self.acc_n = 0;
            }
            self.bucket = b;
        }
        for (k, i) in (self.lo..self.hi).enumerate() {
            self.acc[k] += self.pow[i] / self.floor[i].max(1e-30);
        }
        self.acc_n += 1;
    }

    fn peaks_and_tracks(&mut self) {
        let k10 = 10.0;
        self.peaks.clear();
        for i in self.lo + 2..self.hi - 2 {
            let p = self.pow[i];
            if p < k10 * self.floor[i] || self.mask[i] {
                continue;
            }
            if (i - 2..=i + 2).any(|j| j != i && self.pow[j] > p) {
                continue;
            }
            let l = |x: f64| x.max(1e-30).ln();
            let f = i as f64 + parabolic(l(self.pow[i - 1]), l(p), l(self.pow[i + 1])).offset;
            let ex = 10.0 * (p / self.floor[i]).log10();
            self.peaks.push((f, ex, p));
        }
        self.peaks
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut kept = std::mem::take(&mut self.kept);
        kept.clear();
        for &pk in &self.peaks {
            if kept.iter().all(|k| (k.0 - pk.0).abs() > 6.0) {
                kept.push(pk);
                if kept.len() == 8 {
                    break;
                }
            }
        }
        let frame = self.frames;
        for t in self.tracks.iter_mut() {
            t.used = false;
        }
        let mut taken = [false; 8];
        let coast_max = (0.5 * self.rate / self.hop as f64) as u64;
        // Each track takes the nearest unused peak within its tolerance.
        for t in self.tracks.iter_mut() {
            let pred = t.predict(frame);
            let mv = (pred - t.last_f).abs();
            let tol =
                3.0 + if t.n < 4 {
                    0.7 * (frame - t.last) as f64
                } else {
                    0.0
                } + 0.25 * mv;
            let mut best: Option<(usize, f64)> = None;
            for (k, pk) in kept.iter().enumerate() {
                if taken[k] {
                    continue;
                }
                let d = (pk.0 - pred).abs();
                if d <= tol && best.is_none_or(|b| d < b.1) {
                    best = Some((k, d));
                }
            }
            if let Some((k, _)) = best {
                taken[k] = true;
                t.add(frame, kept[k].0, kept[k].1, kept[k].2);
                t.used = true;
            } else if t.n >= 4
                && t.coast < coast_max
                && pred.round() >= self.lo as f64
                && pred.round() < self.hi as f64
                && self.mask[pred.round() as usize]
            {
                // A sweep of a known slope coasts through masked bins, at most 0.5 s.
                t.coast += 1;
            }
        }
        for (k, pk) in kept.iter().enumerate() {
            if !taken[k] && self.tracks.len() < 32 {
                self.tracks.push(Track::new(frame, pk.0, pk.1, pk.2));
            }
        }
        self.kept = kept;
        self.end_tracks();
    }

    fn end_tracks(&mut self) {
        let frame = self.frames;
        let fps = self.rate / self.hop as f64;
        let gap80 = (0.08 * fps).ceil() as u64;
        let mut i = 0;
        while i < self.tracks.len() {
            let t = &self.tracks[i];
            let idle = (frame - t.last).saturating_sub(t.coast);
            let done = (t.n >= 4 && idle > gap80) || (t.n < 4 && idle > 6);
            if done {
                let mut t = self.tracks.swap_remove(i);
                self.finish(&mut t);
            } else {
                i += 1;
            }
        }
    }

    fn finish(&mut self, t: &mut Track) {
        let fps = self.rate / self.hop as f64;
        t.trim_tail(0.02 * fps);
        let bin = self.bin_hz();
        let span = (t.last - t.first + 1) as f64;
        let dur = span / fps;
        if t.n < 12 || dur < 0.03 {
            return;
        }
        let (b, a, rms) = t.fit();
        let rate = b * bin * fps;
        let width = rate.abs() * dur;
        let ok = (10e3..=3e6).contains(&rate.abs())
            && (4e3..=500e3).contains(&width)
            && rms <= 2.5 + 0.03 * width / bin
            && t.n as f64 >= 0.1 * span;
        if !ok {
            self.out.push(Detection::Rejected);
            return;
        }
        let start = (self.base as f64 + t.first as f64 * self.hop as f64) / self.rate;
        let to_hz = |bins: f64| (bins - (self.n / 2) as f64) * bin;
        let f0 = to_hz(a);
        let f1 = to_hz(a + b * (span - 1.0));
        self.out.push(Detection::Sweep(Sweep {
            start,
            duration: dur,
            f_start: f0,
            f_end: f1,
            centre: (f0 + f1) / 2.0,
            width,
            rate,
            snr_db: t.excess_db / t.n as f64,
            level_dbfs: 10.0 * t.peak.max(1e-30).log10(),
        }));
    }

    /// Feeds samples in pieces that end on a hop, so each frame is read before the next one starts.
    fn run(&mut self, mut x: &[Complex32]) {
        let hop = self.hop as u64;
        while !x.is_empty() {
            let take = ((hop - self.since_start % hop) as usize).min(x.len());
            let made = self.spectrum.as_mut().map_or(0, |s| s.push(&x[..take]));
            self.samples += take as u64;
            self.since_start += take as u64;
            x = &x[take..];
            if made > 0 {
                self.frame();
            }
        }
    }
}

impl Component for SweepDetector {
    fn values(&self) -> Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.hfradar_detector",
            vec![Port::iq("in")],
            vec![
                Port::typed::<Detection>("sweeps"),
                Port::new("excess", Kind::Spectrum),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.samples = 0;
        self.setup(f.rate);
        self.centre_hz = f.centre_hz;
        let mut s = Format::new(Kind::typed::<Detection>(), 0.0);
        s.centre_hz = f.centre_hz;
        s.tuned_hz = f.tuned_hz;
        // The sweeps cover the whole capture: the bandwidth is the sample rate.
        s.bandwidth_hz = f.rate;
        // The excess frames: `frame_len` bins of `bandwidth_hz / frame_len`, centred on the radio's frequency.
        let mut e = Format::new(Kind::Spectrum, 200.0);
        e.frame_len = self.hi - self.lo;
        e.centre_hz = f.centre_hz;
        e.bandwidth_hz = (self.hi - self.lo) as f64 * self.bin_hz();
        Ok(vec![s, e])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let first = io.input(0).first();
        let mut start = 0;
        let tags: Vec<(usize, f64, f64)> = io
            .input(0)
            .tags()
            .iter()
            .filter_map(|t| match t.kind {
                TagKind::Retuned {
                    centre_hz,
                    tuned_hz,
                } => Some((
                    (t.at.saturating_sub(first) as usize).min(x.len()),
                    centre_hz,
                    tuned_hz,
                )),
                _ => None,
            })
            .collect();
        for (at, c, tuned) in tags {
            self.run(&x[start..at]);
            start = at;
            if (c - self.centre_hz).abs() > 1e3 {
                self.centre_hz = c;
                let r = self.rate;
                self.setup(r);
                self.out.push(Detection::Reset {
                    centre_hz: c,
                    tuned_hz: tuned,
                });
            }
        }
        self.run(&x[start..]);
        if !x.is_empty() {
            let sum = x.iter().map(|v| f64::from(v.norm_sqr())).sum();
            self.out.push(Detection::Power {
                sum,
                n: x.len() as u64,
            });
        }
        let o = io.items_out::<Detection>(0);
        for d in self.out.drain(..) {
            o.push(Timed {
                time: 0.0,
                value: d,
            });
        }
        let s = io.spectrum_out(1);
        s.values.extend_from_slice(&self.frames_out);
        self.frames_out.clear();
    }
}
