//! Tone presence: which of a set of tones are in real audio, decided block by block, with the instants each starts and stops.
//!
//! [`ToneDetector`] runs one Goertzel filter per tone over blocks of `N` samples (`sdrkit_algo_dsp::goertzel::ToneBank`, all the tones
//! in one pass). A tone is present in a block when its mean power is at least `threshold_db` relative to the block's whole mean
//! power: the decision is by the tone's share of the signal, so it does not depend on the level, and a tone in noise or among
//! other tones is judged the same way. A tone starts (or stops) after `min_blocks` blocks in a row say so; the change is dated at
//! the first of those blocks.
//!
//! The tone sets of the signalling standards (CTCSS, DTMF, selcall, SELCAL) are tables in `sdrkit-util-ids` (`tones::plan`); the
//! decoders that turn tone changes into keys, digits and calls belong to the signalling crate. DCS is a coded bit stream, not a
//! set of tones, and is decoded elsewhere.
//!
//! The block length sets the resolution: two tones need about `2 / block` Hz between them to be told apart (DTMF at 8 kHz works
//! with 10 ms blocks; the 50 CTCSS tones, 2.5 Hz apart at their closest, need about 0.8 s).
//!
//! ```
//! use sdrkit_algo_modem::tone::{ToneDetector, ToneDetectorConfig};
//! // DTMF key 5 (770 and 1336 Hz, ITU-T Q.23) for 0.1 s at 8 kHz, among the eight DTMF tones.
//! let rate = 8000.0;
//! let freqs = [697.0, 770.0, 852.0, 941.0, 1209.0, 1336.0, 1477.0, 1633.0];
//! let mut d = ToneDetector::with_rate(rate, &freqs, ToneDetectorConfig::new(80)).unwrap();
//! let x: Vec<f32> = (0..1600)
//!     .map(|n| {
//!         let t = n as f64 / rate;
//!         let on = (0.05..0.15).contains(&t);
//!         let s = (std::f64::consts::TAU * 770.0 * t).sin() + (std::f64::consts::TAU * 1336.0 * t).sin();
//!         if on { 0.3 * s as f32 } else { 0.0 }
//!     })
//!     .collect();
//! let mut changes = Vec::new();
//! d.process(&x, |c| changes.push((c.tone, c.on, c.at)));
//! assert_eq!(changes, [(1, true, 400), (5, true, 400), (1, false, 1200), (5, false, 1200)]);
//! ```

use crate::{ModemError, Param};
use sdrkit_algo_dsp::goertzel::ToneBank;

/// Samples cleaned of values that are not numbers at a time (the size of the scratch buffer).
const PIECE: usize = 1024;

/// Settings of [`ToneDetector`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneDetectorConfig {
    /// Samples per decision (at least 2).
    pub block: usize,
    /// A tone's mean power relative to the block's whole mean power for it to count as present, dB (at most 0).
    pub threshold_db: f32,
    /// Blocks in a row a tone must be present (or absent) before it starts (or stops), at least 1.
    pub min_blocks: u32,
}

impl ToneDetectorConfig {
    /// Blocks of `block` samples, threshold -6 dB (a quarter of the power: a tone among up to three others of the same level),
    /// two blocks to decide.
    pub fn new(block: usize) -> Self {
        ToneDetectorConfig {
            block,
            threshold_db: -6.0,
            min_blocks: 2,
        }
    }
}

/// A tone starting or stopping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneChange {
    /// The tone's index in the list the detector was made with.
    pub tone: usize,
    /// Started (`true`) or stopped.
    pub on: bool,
    /// The position (samples since the detector was made) of the first sample of the first block that showed the change.
    pub at: u64,
    /// The tone's mean power in the block that decided the change, dB relative to a full-scale sine's (`10 log10 A^2` for a tone
    /// of amplitude `A`).
    pub level_db: f32,
}

/// One tone's state.
#[derive(Clone, Copy, Debug, Default)]
struct Track {
    on: bool,
    /// Blocks in a row that disagree with `on`, and where the first of them started.
    run: u32,
    run_start: u64,
}

/// Tone presence over blocks (module documentation). The changes do not depend on how the input is split across calls.
#[derive(Clone, Debug)]
pub struct ToneDetector {
    bank: ToneBank,
    freqs: Vec<f64>,
    tracks: Vec<Track>,
    block: usize,
    threshold: f64,
    min_blocks: u32,
    /// Power summed over the block so far.
    power: f64,
    /// Position of the next sample, and of the block's first sample.
    position: u64,
    block_start: u64,
    scratch: Vec<f32>,
}

impl ToneDetector {
    /// A detector of tones at `freqs` cycles per sample (each in `(0, 0.5)`, at least one). Errors for a bad tone or setting.
    pub fn new(freqs: &[f64], cfg: ToneDetectorConfig) -> Result<Self, ModemError> {
        if freqs.is_empty() {
            return Param::Tones.at_least(1.0, 0.0);
        }
        for &f in freqs {
            Param::Frequency.check_open(f, 0.0, 0.5)?;
        }
        if cfg.block < 2 {
            return Param::BlockLen.at_least(2.0, cfg.block as f64);
        }
        if !(cfg.threshold_db <= 0.0 && cfg.threshold_db.is_finite()) {
            return Param::Threshold.range(f64::NEG_INFINITY, 0.0, cfg.threshold_db);
        }
        if cfg.min_blocks == 0 {
            return Param::MinBlocks.at_least(1.0, 0.0);
        }
        Ok(ToneDetector {
            bank: ToneBank::new(freqs, cfg.block)?,
            freqs: freqs.to_vec(),
            tracks: vec![Track::default(); freqs.len()],
            block: cfg.block,
            threshold: 10f64.powf(cfg.threshold_db as f64 / 10.0),
            min_blocks: cfg.min_blocks,
            power: 0.0,
            position: 0,
            block_start: 0,
            scratch: Vec::with_capacity(PIECE),
        })
    }

    /// Tones at `freqs` Hz for samples at `rate` Hz.
    pub fn with_rate(
        rate: f64,
        freqs: &[f64],
        cfg: ToneDetectorConfig,
    ) -> Result<Self, ModemError> {
        if !(rate > 0.0 && rate.is_finite()) {
            return Param::Rate.positive(rate);
        }
        let norm: Vec<f64> = freqs.iter().map(|f| f / rate).collect();
        Self::new(&norm, cfg)
    }

    /// The tones, cycles per sample.
    pub fn tones(&self) -> &[f64] {
        &self.freqs
    }

    /// Samples per decision.
    pub fn block(&self) -> usize {
        self.block
    }

    /// Whether tone `k` is on (false for an index past the list).
    pub fn is_on(&self, k: usize) -> bool {
        self.tracks.get(k).is_some_and(|t| t.on)
    }

    /// The position (samples since the detector was made) of the next sample.
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Forgets the tones and starts a new block at the next sample (no change is reported for tones that were on).
    pub fn reset(&mut self) {
        self.bank.reset();
        self.tracks.fill(Track::default());
        self.power = 0.0;
        self.block_start = self.position;
    }

    /// Takes the next samples, calling `on_change` for every tone that starts or stops, in order of time and then of tone. A
    /// sample that is not a number counts as 0.
    pub fn process(&mut self, x: &[f32], mut on_change: impl FnMut(ToneChange)) {
        let mut i = 0;
        while i < x.len() {
            // Up to the end of the block, at most a scratch buffer's worth.
            let left = self.block - (self.position - self.block_start) as usize;
            let take = left.min(x.len() - i).min(PIECE);
            let mut piece = &x[i..i + take];
            if piece.iter().any(|v| !v.is_finite()) {
                self.scratch.clear();
                self.scratch
                    .extend(piece.iter().map(|&v| if v.is_finite() { v } else { 0.0 }));
                piece = &self.scratch;
            }
            self.power += piece.iter().map(|&v| (v * v) as f64).sum::<f64>();
            let mut done = false;
            self.bank.process(piece, |_, _| done = true);
            i += take;
            self.position += take as u64;
            if done {
                self.decide(&mut on_change);
            }
        }
    }

    /// The decisions of the block the bank has just finished.
    fn decide(&mut self, on_change: &mut impl FnMut(ToneChange)) {
        let mean = self.power / self.block as f64;
        let start = self.block_start;
        for (k, (a, t)) in self
            .bank
            .amplitudes()
            .iter()
            .zip(&mut self.tracks)
            .enumerate()
        {
            // A sine of amplitude A has mean power A^2 / 2: the phasor's magnitude is A.
            let p = a.norm_sqr() as f64 / 2.0;
            let present = mean > 1e-20 && p >= self.threshold * mean;
            if present == t.on {
                t.run = 0;
                continue;
            }
            if t.run == 0 {
                t.run_start = start;
            }
            t.run += 1;
            if t.run >= self.min_blocks {
                t.on = present;
                t.run = 0;
                on_change(ToneChange {
                    tone: k,
                    on: present,
                    at: t.run_start,
                    level_db: (10.0 * (2.0 * p).max(1e-20).log10()) as f32,
                });
            }
        }
        self.power = 0.0;
        self.block_start = self.position;
    }
}

pub(crate) fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    use sdrkit_core::bench::measure;
    use sdrkit_core::{BenchResult, Benchmark};
    fn dtmf(seconds: f64) -> BenchResult {
        // The eight DTMF tones at 8 kHz in 10 ms blocks, on audio that keys digits.
        let n = 65_536;
        let x: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f64 / 8000.0;
                let s = (std::f64::consts::TAU * 770.0 * t).sin()
                    + (std::f64::consts::TAU * 1336.0 * t).sin();
                if (i / 800) % 2 == 0 {
                    0.3 * s as f32
                } else {
                    0.0
                }
            })
            .collect();
        let freqs = [697.0, 770.0, 852.0, 941.0, 1209.0, 1336.0, 1477.0, 1633.0];
        let mut d = ToneDetector::with_rate(8000.0, &freqs, ToneDetectorConfig::new(80))
            .unwrap_or_else(|_| unreachable!("benchmark settings are valid"));
        let mut changes = 0usize;
        measure(seconds, n, n as f64 / 8000.0, || {
            d.process(&x, |_| changes += 1);
        })
    }
    fn ctcss(seconds: f64) -> BenchResult {
        // The 50 CTCSS tones (67 to 254.1 Hz) at 8 kHz in 0.8 s blocks.
        let n = 65_536;
        let x: Vec<f32> = (0..n)
            .map(|i| 0.1 * (std::f64::consts::TAU * 123.0 * i as f64 / 8000.0).sin() as f32)
            .collect();
        let freqs: Vec<f64> = (0..50).map(|k| 67.0 + 3.8 * k as f64).collect();
        let mut d = ToneDetector::with_rate(8000.0, &freqs, ToneDetectorConfig::new(6400))
            .unwrap_or_else(|_| unreachable!("benchmark settings are valid"));
        measure(seconds, n, n as f64 / 8000.0, || {
            d.process(&x, |_| {});
        })
    }
    vec![
        Benchmark {
            name: "modem.tone_detect_dtmf_8k",
            run: dtmf,
        },
        Benchmark {
            name: "modem.tone_detect_ctcss50_8k",
            run: ctcss,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    const RATE: f64 = 8000.0;

    /// 0.1 s quiet, 0.2 s of 770 + 1336 Hz, 0.1 s quiet, 0.2 s of 1000 Hz, 0.1 s quiet; amplitude 0.3 each, noise 0.01.
    fn audio() -> Vec<f32> {
        let mut rng = Rng::new(3);
        let tone =
            |f: f64, n: usize| (std::f64::consts::TAU * f * n as f64 / RATE).sin() as f32 * 0.3;
        (0..5600)
            .map(|n| {
                let t = n as f64 / RATE;
                let s = if (0.1..0.3).contains(&t) {
                    tone(770.0, n) + tone(1336.0, n)
                } else if (0.4..0.6).contains(&t) {
                    tone(1000.0, n)
                } else {
                    0.0
                };
                s + 0.01 * rng.normal_f32()
            })
            .collect()
    }

    fn run(d: &mut ToneDetector, x: &[f32], cut: usize) -> Vec<ToneChange> {
        let mut v = Vec::new();
        for p in x.chunks(cut) {
            d.process(p, |c| v.push(c));
        }
        v
    }

    #[test]
    fn tones_start_and_stop_at_their_times_whatever_the_cuts() {
        let x = audio();
        let make = || {
            ToneDetector::with_rate(RATE, &[770.0, 1336.0, 1000.0], ToneDetectorConfig::new(80))
                .unwrap()
        };
        let whole = run(&mut make(), &x, x.len());
        let got: Vec<(usize, bool)> = whole.iter().map(|c| (c.tone, c.on)).collect();
        assert_eq!(
            got,
            [
                (0, true),
                (1, true),
                (0, false),
                (1, false),
                (2, true),
                (2, false)
            ]
        );
        for (c, t) in whole.iter().zip([0.1, 0.1, 0.3, 0.3, 0.4, 0.6]) {
            assert!((c.at as f64 / RATE - t).abs() <= 0.0101, "{c:?}");
            assert_eq!(c.at % 80, 0);
        }
        // Amplitude 0.3: 10 log10 0.09 = -10.5 dB.
        assert!(
            (whole[0].level_db + 10.5).abs() < 1.0,
            "{}",
            whole[0].level_db
        );
        for cut in [1, 77, 1024, 5000] {
            assert_eq!(run(&mut make(), &x, cut), whole, "{cut}");
        }
    }

    #[test]
    fn the_decision_is_by_share_not_level_and_needs_min_blocks() {
        // The same tone 60 dB down is still present (alone, with a little noise).
        let x: Vec<f32> = (0..800)
            .map(|n| 3e-4 * (std::f64::consts::TAU * 1000.0 * n as f64 / RATE).sin() as f32)
            .collect();
        let mut d =
            ToneDetector::with_rate(RATE, &[1000.0, 1500.0], ToneDetectorConfig::new(80)).unwrap();
        let v = run(&mut d, &x, 800);
        assert_eq!(v.len(), 1);
        assert!(d.is_on(0) && !d.is_on(1) && !d.is_on(9));
        assert!((v[0].level_db + 70.5).abs() < 0.5, "{}", v[0].level_db);
        // One block of tone among blocks of silence is not enough with min_blocks 2, and is with 1.
        let mut y = vec![0f32; 800];
        y[160..240].copy_from_slice(&x[160..240]);
        let mut d = ToneDetector::with_rate(RATE, &[1000.0], ToneDetectorConfig::new(80)).unwrap();
        assert!(run(&mut d, &y, 800).is_empty());
        let mut cfg = ToneDetectorConfig::new(80);
        cfg.min_blocks = 1;
        let mut d = ToneDetector::with_rate(RATE, &[1000.0], cfg).unwrap();
        let v = run(&mut d, &y, 800);
        assert_eq!(
            v.iter().map(|c| (c.on, c.at)).collect::<Vec<_>>(),
            [(true, 160), (false, 240)]
        );
    }

    #[test]
    fn samples_that_are_not_numbers_count_as_zero_and_reset_forgets() {
        let mut x: Vec<f32> = (0..800)
            .map(|n| 0.3 * (std::f64::consts::TAU * 1000.0 * n as f64 / RATE).sin() as f32)
            .collect();
        let clean = run(
            &mut ToneDetector::with_rate(RATE, &[1000.0], ToneDetectorConfig::new(80)).unwrap(),
            &x,
            800,
        );
        x[100] = f32::NAN;
        x[300] = f32::INFINITY;
        let mut d = ToneDetector::with_rate(RATE, &[1000.0], ToneDetectorConfig::new(80)).unwrap();
        let v = run(&mut d, &x, 33);
        assert_eq!(v.len(), clean.len());
        assert_eq!((v[0].on, v[0].at), (clean[0].on, clean[0].at));
        assert!(d.is_on(0));
        assert_eq!(d.position(), 800);
        d.reset();
        assert!(!d.is_on(0));
        // After a reset, blocks start at the reset.
        let v = run(&mut d, &x[..85], 85);
        assert!(v.is_empty());
        let v = run(&mut d, &x[85..245], 160);
        assert_eq!(
            v.iter().map(|c| (c.on, c.at)).collect::<Vec<_>>(),
            [(true, 800)]
        );
    }

    #[test]
    fn bad_settings_are_refused() {
        let cfg = ToneDetectorConfig::new(80);
        assert!(ToneDetector::new(&[], cfg).is_err());
        assert!(ToneDetector::new(&[0.5], cfg).is_err());
        assert!(ToneDetector::new(&[0.0], cfg).is_err());
        assert!(ToneDetector::new(&[f64::NAN], cfg).is_err());
        assert!(ToneDetector::with_rate(0.0, &[1.0], cfg).is_err());
        assert!(ToneDetector::with_rate(8000.0, &[4000.0], cfg).is_err());
        for c in [
            ToneDetectorConfig { block: 1, ..cfg },
            ToneDetectorConfig {
                threshold_db: 1.0,
                ..cfg
            },
            ToneDetectorConfig {
                threshold_db: f32::NAN,
                ..cfg
            },
            ToneDetectorConfig {
                min_blocks: 0,
                ..cfg
            },
        ] {
            assert!(ToneDetector::new(&[0.1], c).is_err(), "{c:?}");
        }
        let d = ToneDetector::new(&[0.1, 0.2], cfg).unwrap();
        assert_eq!((d.tones(), d.block()), (&[0.1, 0.2][..], 80));
    }
}
