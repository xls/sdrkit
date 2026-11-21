//! Burst detection: finds the stretches of a stream where the power is above the noise floor and outputs each one as a
//! slice, with some samples before and after it.
//!
//! [`BurstDetector`] averages `|x|^2` over windows of `window` samples. A `sdrkit_algo_dsp::floor::FloorTracker` (a minimum follower or a
//! chunk percentile, chosen by the caller) follows the noise from the windows seen while no burst is open, so a burst does not raise
//! its own floor. A burst **opens** when a window's power reaches `floor x 10^(open_db / 10)` and **closes** when a window falls
//! below `floor x 10^(close_db / 10)` (hysteresis: `close_db <= open_db`); `post` more samples are then kept, and a window back
//! above the open level during them continues the burst. Each burst starts `pre` samples before the window that opened it, or at
//! the first sample of the stream when fewer have been seen (positions are counted, never subtracted below zero).
//!
//! Finished bursts go into a ring of `pool` buffers made at construction; [`BurstDetector::take`] reads the oldest. When the ring
//! is full the oldest unread burst is dropped and counted. A burst longer than `max_len` samples is cut there and marked
//! truncated; the detector then waits for the power to fall below the close level before it opens again (a continuous carrier produces
//! one truncated burst, not a stream of them). Each burst carries its start index, its level over the floor, and a frequency guess
//! from the phase of its lag-one autocorrelation (one `approx::atan2` per burst).
//!
//! ```
//! use sdrkit_algo_dsp::floor::{FloorTracker, MinFollower};
//! use sdrkit_algo_modem::burst::{BurstConfig, BurstDetector};
//! use sdrkit_algo_modem::Complex32;
//! let floor = FloorTracker::MinFollower(MinFollower::new(0.5, 0.1).unwrap());
//! let mut d = BurstDetector::new(BurstConfig::new(16, 4096), floor).unwrap();
//! let mut x = vec![Complex32::new(0.01, 0.0); 3000];
//! for v in &mut x[1000..1400] {
//!     *v = Complex32::new(1.0, 0.0);
//! }
//! d.process(&x);
//! let b = d.take().unwrap();
//! assert!(b.start <= 1000 && b.start + b.samples.len() as u64 >= 1400);
//! ```

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::floor::FloorTracker;

/// The settings of a [`BurstDetector`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BurstConfig {
    /// Samples per power window (1 to 65536).
    pub window: usize,
    /// Level over the floor that opens a burst, dB.
    pub open_db: f32,
    /// Level over the floor below which a burst closes, dB (at most `open_db`).
    pub close_db: f32,
    /// Samples kept before the window that opened the burst.
    pub pre: usize,
    /// Samples kept after the window that closed it.
    pub post: usize,
    /// The longest burst, in samples from the opening window (a longer one is cut and marked truncated).
    pub max_len: usize,
    /// Finished bursts held for [`BurstDetector::take`] (1 to 64).
    pub pool: usize,
    /// The least floor, mean power per sample: a floor tracker that reads less (input of exact zeros) is taken as this.
    pub floor_min: f32,
}

impl BurstConfig {
    /// Windows of `window` samples, bursts up to `max_len`: opens at 10 dB, closes below 6 dB, `pre` and `post` one window each,
    /// a pool of 4, a least floor of 1e-12 (-120 dBFS).
    pub fn new(window: usize, max_len: usize) -> Self {
        BurstConfig {
            window,
            open_db: 10.0,
            close_db: 6.0,
            pre: window,
            post: window,
            max_len,
            pool: 4,
            floor_min: 1e-12,
        }
    }
}

/// A finished burst, borrowed from the detector's pool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burst<'a> {
    /// The samples, from `pre` before the opening window to `post` after the closing one.
    pub samples: &'a [Complex32],
    /// The stream index of `samples[0]` (counted from the first sample given since `new` or `reset`).
    pub start: u64,
    /// The stream index of the first sample of the window that opened the burst.
    pub open: u64,
    /// The SNR estimate in dB: mean power from the opening window on, over the floor, less one (the floor's own share).
    pub snr_db: f32,
    /// The floor at the opening, as a mean power per sample.
    pub floor: f32,
    /// A frequency guess, cycles per sample: the phase of the lag-one autocorrelation over the burst.
    pub freq: f64,
    /// Cut at `max_len`.
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct Meta {
    start: u64,
    open: u64,
    lead: usize,
    floor: f32,
    truncated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Closed,
    Open,
    Post(usize),
    /// After a truncated burst: wait for the power to fall below the close level.
    Hold,
}

/// A power burst detector over a noise-floor tracker (module documentation).
#[derive(Clone, Debug)]
pub struct BurstDetector {
    cfg: BurstConfig,
    floor: FloorTracker,
    open_ratio: f32,
    close_ratio: f32,
    hist: Vec<Complex32>,
    hpos: usize,
    hlen: usize,
    acc: f64,
    acc_n: usize,
    total: u64,
    state: State,
    slots: Vec<Vec<Complex32>>,
    meta: Vec<Meta>,
    head: usize,
    ready: usize,
    dropped: u64,
    open_floor: f32,
}

impl BurstDetector {
    /// A detector with settings `cfg` and the floor tracker `floor`. Errors for settings out of range.
    pub fn new(cfg: BurstConfig, floor: FloorTracker) -> Result<Self, ModemError> {
        if cfg.window == 0 || cfg.window > 65_536 {
            return Param::Window.range(1.0, 65_536.0, cfg.window as f64);
        }
        Param::OpenLevel.check_finite(cfg.open_db)?;
        Param::CloseLevel.check_finite(cfg.close_db)?;
        if cfg.close_db > cfg.open_db {
            return Param::CloseLevel.conflict(Param::OpenLevel);
        }
        Param::Pool.check_count(cfg.pool, 1, 64)?;
        Param::MaxLen.check_count(cfg.max_len, 1, 1 << 26)?;
        Param::Pre.check_count(cfg.pre, 0, 1 << 24)?;
        Param::Post.check_count(cfg.post, 0, 1 << 24)?;
        if !(cfg.floor_min >= 0.0 && cfg.floor_min.is_finite()) {
            return Param::Floor.range(0.0, f64::from(f32::MAX), cfg.floor_min);
        }
        let cap = cfg.pre + cfg.window + cfg.max_len + cfg.post;
        let h = cfg.pre + cfg.window;
        Ok(BurstDetector {
            cfg,
            floor,
            open_ratio: 10f32.powf(cfg.open_db / 10.0),
            close_ratio: 10f32.powf(cfg.close_db / 10.0),
            hist: vec![Complex32::default(); h],
            hpos: 0,
            hlen: 0,
            acc: 0.0,
            acc_n: 0,
            total: 0,
            state: State::Closed,
            slots: (0..cfg.pool + 1).map(|_| Vec::with_capacity(cap)).collect(),
            meta: vec![Meta::default(); cfg.pool + 1],
            head: 0,
            ready: 0,
            dropped: 0,
            open_floor: 0.0,
        })
    }

    /// Forgets the stream and every burst held (the floor tracker is kept).
    pub fn reset(&mut self) {
        self.hpos = 0;
        self.hlen = 0;
        self.acc = 0.0;
        self.acc_n = 0;
        self.total = 0;
        self.state = State::Closed;
        self.head = 0;
        self.ready = 0;
        for s in &mut self.slots {
            s.clear();
        }
    }

    /// The settings.
    pub fn config(&self) -> &BurstConfig {
        &self.cfg
    }

    /// The current floor, mean power per sample.
    pub fn floor(&self) -> Option<f32> {
        self.floor.floor()
    }

    /// Whether a burst is being collected.
    pub fn is_open(&self) -> bool {
        matches!(self.state, State::Open | State::Post(_))
    }

    /// Finished bursts waiting to be taken.
    pub fn ready(&self) -> usize {
        self.ready
    }

    /// Bursts dropped because the pool was full.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Samples seen.
    pub fn samples_seen(&self) -> u64 {
        self.total
    }

    #[inline]
    fn active(&self) -> usize {
        (self.head + self.ready) % self.slots.len()
    }

    /// Takes the oldest finished burst.
    pub fn take(&mut self) -> Option<Burst<'_>> {
        if self.ready == 0 {
            return None;
        }
        let i = self.head;
        self.head = (self.head + 1) % self.slots.len();
        self.ready -= 1;
        let m = self.meta[i];
        let s = &self.slots[i];
        // Statistics over the burst from its opening window on.
        let body = &s[m.lead.min(s.len())..];
        let (mut p, mut c) = (0f64, Complex32::default());
        for (k, z) in body.iter().enumerate() {
            p += z.norm_sqr() as f64;
            if k > 0 {
                c += z * body[k - 1].conj();
            }
        }
        let mean = if body.is_empty() {
            0.0
        } else {
            p / body.len() as f64
        };
        let snr = if m.floor > 0.0 {
            (mean / m.floor as f64 - 1.0).max(1e-6)
        } else {
            1e6
        };
        Some(Burst {
            samples: s,
            start: m.start,
            open: m.open,
            snr_db: (10.0 * snr.log10()) as f32,
            floor: m.floor,
            freq: approx::atan2(c.im, c.re) as f64 / std::f64::consts::TAU,
            truncated: m.truncated,
        })
    }

    fn open_burst(&mut self, floor: f32) {
        let a = self.active();
        let n = self.hlen;
        let h = self.hist.len();
        let slot = &mut self.slots[a];
        slot.clear();
        // The history, oldest first: it ends at the newest sample, which ends the opening window.
        let first = (self.hpos + h - n) % h;
        for k in 0..n {
            slot.push(self.hist[(first + k) % h]);
        }
        let win = (self.cfg.window as u64).min(self.total);
        let open = self.total - win;
        self.meta[a] = Meta {
            start: self.total - n as u64,
            open,
            lead: n - win as usize,
            floor,
            truncated: false,
        };
        self.open_floor = floor;
        self.state = State::Open;
    }

    fn finish_burst(&mut self, truncated: bool) {
        let a = self.active();
        self.meta[a].truncated = truncated;
        self.ready += 1;
        if self.ready == self.slots.len() {
            // No free slot for the next burst: drop the oldest unread one.
            self.head = (self.head + 1) % self.slots.len();
            self.ready -= 1;
            self.dropped += 1;
        }
        self.state = if truncated {
            State::Hold
        } else {
            State::Closed
        };
    }

    /// The input ended. A burst that is open, or in its samples after the close, is finished with the samples it holds. It then
    /// waits for [`take`](Self::take) like any other burst.
    pub fn end(&mut self) {
        if matches!(self.state, State::Open | State::Post(_)) {
            self.finish_burst(false);
            self.state = State::Closed;
        }
    }

    /// Takes any number of samples. Finished bursts wait in the pool for [`take`](Self::take). Non-finite samples count as 0.
    pub fn process(&mut self, x: &[Complex32]) {
        let cap = self.cfg.pre + self.cfg.window + self.cfg.max_len + self.cfg.post;
        for &s in x {
            let s = if s.re.is_finite() && s.im.is_finite() {
                s
            } else {
                Complex32::default()
            };
            self.total += 1;
            let h = self.hist.len();
            self.hist[self.hpos] = s;
            self.hpos = if self.hpos + 1 == h { 0 } else { self.hpos + 1 };
            self.hlen = (self.hlen + 1).min(h);
            if matches!(self.state, State::Open | State::Post(_)) {
                let a = self.active();
                self.slots[a].push(s);
                if let State::Post(left) = self.state {
                    if left <= 1 {
                        self.finish_burst(false);
                    } else {
                        self.state = State::Post(left - 1);
                    }
                } else if self.slots[a].len() >= cap - self.cfg.post {
                    self.finish_burst(true);
                }
            }
            self.acc += s.norm_sqr() as f64;
            self.acc_n += 1;
            if self.acc_n == self.cfg.window {
                let p = (self.acc / self.cfg.window as f64) as f32;
                self.acc = 0.0;
                self.acc_n = 0;
                self.window_done(p);
            }
        }
    }

    fn window_done(&mut self, p: f32) {
        match self.state {
            State::Closed => {
                let f = self.floor.floor().map(|f| f.max(self.cfg.floor_min));
                match f {
                    Some(f) if p >= f * self.open_ratio => self.open_burst(f),
                    _ => {
                        self.floor.update(&[p]);
                    }
                }
            }
            State::Hold => {
                if p < self.open_floor * self.close_ratio {
                    self.state = State::Closed;
                    self.floor.update(&[p]);
                }
            }
            State::Open => {
                if p < self.open_floor * self.close_ratio {
                    if self.cfg.post == 0 {
                        self.finish_burst(false);
                    } else {
                        self.state = State::Post(self.cfg.post);
                    }
                }
            }
            State::Post(_) => {
                if p >= self.open_floor * self.open_ratio {
                    self.state = State::Open;
                }
            }
        }
    }
}

/// Benchmark: burst detection at 2.4 MS/s (64-sample windows, a minimum-follower floor).
pub(crate) fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    use sdrkit_algo_dsp::floor::MinFollower;
    use sdrkit_core::bench::measure;
    use sdrkit_core::Benchmark;
    vec![Benchmark {
        name: "modem.burst_detect_2400k",
        run: |sec| {
            let mut s = 0x2545_F491u32;
            let x: Vec<Complex32> = (0..65_536)
                .map(|n| {
                    s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let a = if n % 8192 < 1000 { 10.0 } else { 1.0 };
                    Complex32::new(
                        a * ((s >> 16) as f32 / 32768.0 - 1.0),
                        a * ((s & 0xFFFF) as f32 / 32768.0 - 1.0),
                    )
                })
                .collect();
            let floor = FloorTracker::MinFollower(MinFollower::new(0.2, 0.05).expect("valid"));
            let mut d = BurstDetector::new(BurstConfig::new(64, 4096), floor).expect("valid");
            measure(sec, x.len(), x.len() as f64 / 2.4e6, || {
                d.process(&x);
                while d.take().is_some() {}
            })
        },
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::floor::{ChunkPercentile, MinFollower};
    use sdrkit_algo_sim::rng::Rng;

    fn min_floor() -> FloorTracker {
        FloorTracker::MinFollower(MinFollower::new(0.3, 0.05).unwrap())
    }

    #[test]
    fn the_end_of_the_input_finishes_an_open_burst() {
        let mut rng = Rng::new(5);
        let mut x: Vec<Complex32> = (0..20_000).map(|_| rng.complex_normal(1.0)).collect();
        // A burst that runs to the last sample: the power never falls, so only the end finishes it.
        for v in &mut x[15_000..] {
            *v += Complex32::new(10.0, 0.0);
        }
        let floor = FloorTracker::ChunkPercentile(ChunkPercentile::new(32, 0.5, 64, 0.2).unwrap());
        let mut d = BurstDetector::new(BurstConfig::new(64, 8192), floor).unwrap();
        d.process(&x);
        assert!(d.take().is_none() && d.is_open());
        d.end();
        assert!(!d.is_open());
        let b = d.take().expect("the open burst");
        assert_eq!(b.start + b.samples.len() as u64, 20_000);
        assert!(!b.truncated);
        d.end();
        assert!(d.take().is_none());
    }

    #[test]
    fn catches_tone_bursts_at_their_snr_with_start_and_frequency() {
        let mut rng = Rng::new(13);
        let mut x: Vec<Complex32> = (0..200_000).map(|_| rng.complex_normal(1.0)).collect();
        let f = 0.031;
        let starts = [10_000usize, 47_123, 90_001, 150_500];
        for &st in &starts {
            for k in 0..3000 {
                let ph = std::f64::consts::TAU * f * k as f64;
                x[st + k] += Complex32::new(ph.cos() as f32, ph.sin() as f32) * 10f32.sqrt();
            }
        }
        let floor = FloorTracker::ChunkPercentile(ChunkPercentile::new(32, 0.5, 64, 0.2).unwrap());
        let mut d = BurstDetector::new(BurstConfig::new(64, 8192), floor).unwrap();
        let mut found = Vec::new();
        for piece in x.chunks(4097) {
            d.process(piece);
            while let Some(b) = d.take() {
                found.push((
                    b.start,
                    b.open,
                    b.samples.len(),
                    b.snr_db,
                    b.freq,
                    b.truncated,
                ));
            }
        }
        assert_eq!(found.len(), starts.len(), "{found:?}");
        for (&st, &(start, open, len, snr, freq, tr)) in starts.iter().zip(&found) {
            assert!(
                start <= st as u64 && open <= st as u64 + 64 && open + 64 > st as u64,
                "{st} {start} {open}"
            );
            assert!(start + len as u64 >= st as u64 + 3000);
            assert!((snr - 10.0).abs() < 1.0, "snr {snr}");
            assert!((freq - f).abs() < 2e-3, "freq {freq}");
            assert!(!tr);
        }
    }

    #[test]
    fn a_burst_in_the_first_samples_never_underflows() {
        for st in 0..40usize {
            let mut x = vec![Complex32::new(0.01, 0.0); 600];
            for v in &mut x[16 + st..216 + st] {
                *v = Complex32::new(1.0, 0.0);
            }
            let mut cfg = BurstConfig::new(16, 1000);
            cfg.pre = 100;
            let mut d = BurstDetector::new(cfg, min_floor()).unwrap();
            d.process(&x);
            let b = d.take().unwrap_or_else(|| panic!("start {st}"));
            assert!(b.start <= (16 + st) as u64);
            assert_eq!(b.samples[0], x[b.start as usize]);
        }
    }

    #[test]
    fn long_carriers_truncate_once_and_the_pool_drops_the_oldest() {
        let mut x = vec![Complex32::new(0.01, 0.0); 64];
        x.extend(std::iter::repeat_n(Complex32::new(1.0, 0.0), 5000));
        x.extend(std::iter::repeat_n(Complex32::new(0.01, 0.0), 200));
        let mut d = BurstDetector::new(BurstConfig::new(16, 1000), min_floor()).unwrap();
        d.process(&x);
        assert_eq!(d.ready(), 1);
        assert!(d.take().unwrap().truncated);
        // Many short bursts with a pool of 2: the oldest are dropped.
        let mut cfg = BurstConfig::new(16, 1000);
        cfg.pool = 2;
        let mut d = BurstDetector::new(cfg, min_floor()).unwrap();
        let mut y = vec![Complex32::new(0.01, 0.0); 64];
        for _ in 0..5 {
            y.extend(std::iter::repeat_n(Complex32::new(1.0, 0.0), 100));
            y.extend(std::iter::repeat_n(Complex32::new(0.01, 0.0), 200));
        }
        d.process(&y);
        assert_eq!(d.ready(), 2);
        assert_eq!(d.dropped(), 3);
        // Bad settings are refused; NaN input is survived.
        let mut c = BurstConfig::new(16, 1000);
        c.close_db = 20.0;
        assert!(BurstDetector::new(c, min_floor()).is_err());
        assert!(BurstDetector::new(BurstConfig::new(0, 1000), min_floor()).is_err());
        d.process(&[Complex32::new(f32::NAN, 0.0); 100]);
    }

    #[test]
    fn input_of_exact_zeros_opens_no_burst() {
        let mut d = BurstDetector::new(BurstConfig::new(16, 1000), min_floor()).unwrap();
        d.process(&vec![Complex32::default(); 20_000]);
        assert!(!d.is_open() && d.ready() == 0);
        let floor = FloorTracker::ChunkPercentile(ChunkPercentile::new(32, 0.5, 16, 0.2).unwrap());
        let mut d = BurstDetector::new(BurstConfig::new(16, 1000), floor).unwrap();
        d.process(&vec![Complex32::default(); 20_000]);
        assert!(!d.is_open() && d.ready() == 0);
    }
}
