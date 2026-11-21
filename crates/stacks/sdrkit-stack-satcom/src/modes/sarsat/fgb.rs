//! The first-generation burst finder on 32 ms Hann FFTs at 128 kHz: lines that were not there a moment ago in one frame, and
//! weak lines in the sum of four frames against a background learnt per bin. A finished burst goes to the coherent
//! demodulator of [`super::coherent`].

use crate::modes::sarsat::coherent::{Demodulator, FgbDemod};
use sdrkit_algo_dsp::floor::DEFAULT_MIN_FLOOR;
use sdrkit_algo_dsp::meter::power_to_db as db;
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::spectrum::{percentile, Scaling, Welch};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;

/// The finder's working rate.
pub const RATE: f64 = 128_000.0;
const N: usize = 4096;
/// Lines must stand this far over the noise (dB, against the mean noise of a bin).
const LINE_DB: f64 = 15.0;
/// The burst ends after this many blocks without its line.
const END_BLOCKS: usize = 6;
/// A burst lasts at least this long from its first line (seconds): a long message after a late first line.
const MIN_BURST_S: f64 = 0.62;
/// Ring of samples kept (seconds).
const RING_S: f64 = 2.5;
/// The longest burst of a first-generation beacon with its end (seconds): longer is a steady carrier.
pub const MAX_BURST_S: f64 = 1.2;
/// Spectra kept: a line is new when none of the blocks 3 to 10 back held it.
const HIST: usize = 10;
/// The weak test sums this many frames.
const SUM_FRAMES: usize = 4;
/// Frames before the weak test runs.
const WEAK_WARMUP: usize = 12;
/// The weak test: the sum of four frames over the background of one frame's bin.
const WEAK_OVER_BACKGROUND: f32 = 24.0;
/// The weak test: the sum over the median of the sum, and near an expected carrier.
const WEAK_OVER_MEDIAN: f32 = 8.5;
const EXPECTED_OVER_MEDIAN: f32 = 5.5;
/// The weak test: the most the median around a line may stand over the median of the band.
const LOCAL_OVER_MEDIAN: f32 = 2.0;
/// The median of a sum of four exponential values over their mean.
const SUM4_MEDIAN: f32 = 3.672;
/// The background follows a quiet frame by this share; a bin is capped at this many medians of its frame.
const BACKGROUND_ALPHA: f32 = 0.02;
const BACKGROUND_CAP: f32 = 4.0;
/// Bins either side of an expected carrier that take the lower weak gate.
const EXPECTED_BINS: f64 = 3.0;
/// The most expected carriers.
const MAX_EXPECTED: usize = 8;
/// A new expected carrier within this many Hz replaces an old one.
const EXPECTED_SAME_HZ: f64 = 60.0;

/// The run test: a line this many times over the median bin of its frame, in this many frames in a row within
/// `RUN_BINS` bins; an active burst takes such a line within `END_BINS` bins of its own.
const RUN_OVER_MEDIAN: f32 = 8.0;
const RUN_FRAMES: usize = 3;
const RUN_BINS: usize = 2;
const END_BINS: usize = 3;
/// A burst begun by the run test is dropped unless this many later frames hold its line before this many do not.
const RUN_CONFIRM_HITS: u8 = 2;
const RUN_CONFIRM_MISSES: u8 = 2;

/// What the finder reports about a burst.
#[derive(Clone, Debug)]
pub struct FgbBurst {
    /// Absolute index (at 128 kHz) of the first block with the line.
    pub start: u64,
    pub end: u64,
    /// Carrier offset from the finder's centre, Hz.
    pub carrier_hz: f64,
    /// Carrier to mean noise in one 31.25 Hz bin, dB.
    pub cn_db: f64,
    pub level_dbfs: f64,
    /// The weak test (four frames summed) found it.
    pub weak: bool,
    /// What the demodulator found, when the finder decodes.
    pub demod: Option<FgbDemod>,
    /// The burst's samples (at 128 kHz, from half a block before the line appeared) when the finder keeps them, and where the line
    /// appeared in them.
    pub samples: Option<(Vec<Complex32>, u64)>,
}

/// One burst finder with its demodulator, on a 128 kHz stream.
pub struct FgbFinder {
    /// One Hann frame per analysis: a periodogram with 0 Hz in bin `N / 2`.
    welch: Welch,
    spec: Vec<f32>,
    scratch: Vec<f32>,
    demod: Demodulator,
    /// The last spectra (a ring), to tell a new line from one that was there a moment ago.
    hist: Vec<Vec<f32>>,
    hist_pos: usize,
    /// The median of the allowed bins of each spectrum of `hist`, for the run test.
    hist_med: [f32; HIST],
    avg_init: usize,
    /// The sum of the last four spectra.
    sum4: Vec<f32>,
    /// The background of each bin, learnt while nothing is found.
    background: Vec<f32>,
    background_set: bool,
    /// Carriers (Hz from the centre) where a beacon is expected again, until an absolute index.
    expected: Vec<(f64, u64)>,
    block: Vec<Complex32>,
    /// Ring of the last samples and the absolute index of ring[0] position logic.
    ring: Vec<Complex32>,
    ring_pos: usize,
    /// Absolute index of the next sample.
    pub index: u64,
    state: Option<Active>,
    pub min_offset_hz: f64,
    pub max_offset_hz: f64,
    /// Decode the burst as a first-generation beacon.
    pub decode: bool,
    /// Hand the burst's samples on (for another demodulator).
    pub keep: bool,
    /// The longest burst with its end, seconds: a longer one is a steady carrier and is dropped.
    pub max_burst_s: f64,
    /// Take the run test: a line 8 times over the median bin in 3 frames in a row starts a burst, and one such line near an
    /// active burst holds it.
    pub run_line: bool,
}

struct Active {
    start: u64,
    last_seen: u64,
    misses: usize,
    lines: Vec<f64>,
    peak_db: f64,
    bin: usize,
    weak: bool,
    /// Frames with the line still needed to confirm a burst begun by the run test (0 for a confirmed burst), and its
    /// frames without the line so far.
    unconfirmed: u8,
    unconfirmed_misses: u8,
}

impl Default for FgbFinder {
    fn default() -> Self {
        Self::new()
    }
}

impl FgbFinder {
    pub fn new() -> Self {
        let ring_len = (RING_S * RATE) as usize;
        FgbFinder {
            welch: Welch::new(N, Window::Hann, 0.0, Scaling::Tone).expect("a valid frame size"),
            spec: vec![0.0; N],
            scratch: Vec::with_capacity(N),
            demod: Demodulator::new(),
            hist: vec![vec![0.0; N]; HIST],
            hist_pos: 0,
            hist_med: [0.0; HIST],
            avg_init: 0,
            sum4: vec![0.0; N],
            background: vec![0.0; N],
            background_set: false,
            expected: Vec::new(),
            block: Vec::with_capacity(N),
            ring: vec![Complex32::new(0.0, 0.0); ring_len],
            ring_pos: 0,
            index: 0,
            state: None,
            min_offset_hz: -46_000.0,
            max_offset_hz: 46_000.0,
            decode: true,
            keep: false,
            max_burst_s: MAX_BURST_S,
            run_line: false,
        }
    }

    pub fn reset(&mut self) {
        let mut f = FgbFinder::new();
        f.min_offset_hz = self.min_offset_hz;
        f.max_offset_hz = self.max_offset_hz;
        f.decode = self.decode;
        f.keep = self.keep;
        f.run_line = self.run_line;
        f.max_burst_s = self.max_burst_s;
        f.index = self.index;
        *self = f;
    }

    /// Whether a burst is being followed.
    pub fn busy(&self) -> bool {
        self.state.is_some()
    }

    /// Expects a beacon again on `carrier_hz` (from the finder's centre) until the absolute index `until`: the weak test takes
    /// a lower gate within 3 bins of it. A carrier within 60 Hz of one expected replaces it; at most 8 are kept (the one that
    /// ends first goes).
    pub fn expect(&mut self, carrier_hz: f64, until: u64) {
        self.expected
            .retain(|e| (e.0 - carrier_hz).abs() > EXPECTED_SAME_HZ);
        if self.expected.len() >= MAX_EXPECTED {
            if let Some(i) = (0..self.expected.len()).min_by_key(|&i| self.expected[i].1) {
                self.expected.remove(i);
            }
        }
        self.expected.push((carrier_hz, until));
    }

    /// Feeds samples at 128 kHz; appends finished bursts.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<FgbBurst>) {
        for &s in x {
            let rl = self.ring.len();
            self.ring[self.ring_pos] = s;
            self.ring_pos = (self.ring_pos + 1) % rl;
            self.index += 1;
            self.block.push(s);
            if self.block.len() == N {
                self.analyse(out);
                self.block.clear();
            }
        }
    }

    fn sample(&self, abs: u64) -> Complex32 {
        let rl = self.ring.len() as u64;
        if abs >= self.index || self.index - abs > rl {
            return Complex32::new(0.0, 0.0);
        }
        let back = (self.index - abs) as usize;
        let p = (self.ring_pos + self.ring.len() - back) % self.ring.len();
        self.ring[p]
    }

    /// The index in `hist` of the spectrum `back` frames before the current one.
    fn back(&self, back: usize) -> usize {
        (self.hist_pos + HIST - back) % HIST
    }

    /// The weak test (no burst active, the one-frame test failed): the strongest allowed bin of the sum of the last four
    /// spectra that stands 24 times over its background and 8.5 times (5.5 near an expected carrier) over the median of the
    /// sum, and whose line was not there in the four frames before. The burst starts three frames back.
    fn weak_start(&mut self, lo: usize, hi: usize, block_start: u64) -> Option<Active> {
        let backs = [self.back(1), self.back(2), self.back(3)];
        for k in 0..N {
            self.sum4[k] = self.spec[k] + backs.iter().map(|&b| self.hist[b][k]).sum::<f32>();
        }
        let med4 = percentile(&self.sum4[lo..=hi], 0.5, &mut self.scratch).max(DEFAULT_MIN_FLOOR);
        let bin_hz = RATE / N as f64;
        let centre = (N / 2) as f64;
        let mut best: Option<(usize, f32)> = None;
        for k in lo..=hi {
            let s = self.sum4[k];
            if s <= WEAK_OVER_BACKGROUND * self.background[k].max(DEFAULT_MIN_FLOOR) {
                continue;
            }
            let expected = self
                .expected
                .iter()
                .any(|e| (e.0 / bin_hz + centre - k as f64).abs() <= EXPECTED_BINS);
            let gate = if expected {
                EXPECTED_OVER_MEDIAN
            } else {
                WEAK_OVER_MEDIAN
            };
            if s >= gate * med4 && best.is_none_or(|b| s > b.1) {
                best = Some((k, s));
            }
        }
        let (k, s) = best?;
        // A line stands out from its own neighbourhood too, and the neighbourhood is not raised over the band: a
        // second-generation burst raises a wide region and has a line at its centre.
        let local = local_median(&self.sum4, k);
        if s < WEAK_OVER_MEDIAN * local || local > LOCAL_OVER_MEDIAN * med4 {
            return None;
        }
        // A steady line (a spur) was there in the four frames before as well.
        let before: f32 = (SUM_FRAMES..2 * SUM_FRAMES)
            .map(|b| {
                let h = &self.hist[self.back(b)];
                h[k - 1].max(h[k]).max(h[k + 1])
            })
            .sum();
        if before >= 0.5 * s {
            return None;
        }
        let off = (k as f64 - centre + parabolic_log(&self.sum4, k).clamp(-1.0, 1.0)) * bin_hz;
        let n1 = med4 / SUM4_MEDIAN;
        Some(Active {
            start: block_start.saturating_sub(3 * N as u64),
            last_seen: self.index,
            misses: 0,
            lines: vec![off],
            peak_db: db(f64::from(s / SUM_FRAMES as f32 / n1)),
            bin: k,
            weak: true,
            unconfirmed: 0,
            unconfirmed_misses: 0,
        })
    }

    /// The run test (no burst active, the other tests failed): the strongest allowed bin at least 8 times over the median of
    /// this frame, with such a line within 2 bins of it in each of the two frames before, and none in the four frames before
    /// those. The burst starts two frames back and is dropped unless two later frames hold its line before two do not.
    fn run_start(&self, lo: usize, hi: usize, med: f32, block_start: u64) -> Option<Active> {
        let held = |back: usize, k: usize| {
            let b = self.back(back);
            let gate = RUN_OVER_MEDIAN * self.hist_med[b].max(DEFAULT_MIN_FLOOR);
            self.hist[b][k - RUN_BINS..=k + RUN_BINS]
                .iter()
                .any(|&p| p >= gate)
        };
        let gate = RUN_OVER_MEDIAN * med;
        let mut best: Option<(usize, f32)> = None;
        for k in lo..=hi {
            let p = self.spec[k];
            if p >= gate
                && best.is_none_or(|b| p > b.1)
                && (1..RUN_FRAMES).all(|b| held(b, k))
                && !(RUN_FRAMES..RUN_FRAMES + 4).any(|b| held(b, k))
            {
                best = Some((k, p));
            }
        }
        let (k, p) = best?;
        let off = (k as f64 - (N / 2) as f64 + parabolic_log(&self.spec, k)) * RATE / N as f64;
        Some(Active {
            start: block_start.saturating_sub((RUN_FRAMES as u64 - 1) * N as u64),
            last_seen: self.index,
            misses: 0,
            lines: vec![off],
            peak_db: db(f64::from(p * std::f32::consts::LN_2 / med)),
            bin: k,
            weak: true,
            unconfirmed: RUN_CONFIRM_HITS,
            unconfirmed_misses: 0,
        })
    }

    fn analyse(&mut self, out: &mut Vec<FgbBurst>) {
        // The block is exactly one frame, so the call cannot fail.
        let _ = self.welch.compute(&self.block, &mut self.spec);
        let med = percentile(&self.spec, 0.5, &mut self.scratch).max(DEFAULT_MIN_FLOOR);
        let noise = med / std::f32::consts::LN_2;
        let bin_hz = RATE / N as f64;
        let lo =
            ((self.min_offset_hz / bin_hz) as i64 + (N / 2) as i64).clamp(2, N as i64 - 3) as usize;
        let hi =
            ((self.max_offset_hz / bin_hz) as i64 + (N / 2) as i64).clamp(2, N as i64 - 3) as usize;
        let thr = noise * 10f32.powf(LINE_DB as f32 / 10.0);
        let block_start = self.index - N as u64;
        let now = self.index;
        // The run test measures against the median of the allowed bins (the edges of the band may hold less noise).
        let run_med = if self.run_line {
            percentile(&self.spec[lo..=hi], 0.5, &mut self.scratch).max(DEFAULT_MIN_FLOOR)
        } else {
            med
        };
        self.expected.retain(|e| e.1 > now);
        // The strongest line in range.
        let mut best = (0usize, 0f32);
        for k in lo..=hi {
            if self.spec[k] > best.1 {
                best = (k, self.spec[k]);
            }
        }
        // A line must also stand out from its own neighbourhood (a spread-spectrum burst raises a wide region, not one bin).
        let line = best.1 > thr
            && best.1
                > local_median(&self.spec, best.0) / std::f32::consts::LN_2
                    * 10f32.powf(LINE_DB as f32 / 10.0);
        let idle = self.state.is_none();
        if idle {
            if line && self.avg_init >= HIST {
                let k = best.0;
                // Was it there a moment ago (100 to 320 ms back)?
                let mut before = 0f32;
                for back in 3..=HIST {
                    let h = &self.hist[self.back(back)];
                    before = h[k - 2..=k + 2].iter().cloned().fold(before, f32::max);
                }
                if before < thr * 0.3 {
                    let off = (k as f64 - (N / 2) as f64 + parabolic_log(&self.spec, k)) * bin_hz;
                    self.state = Some(Active {
                        start: block_start,
                        last_seen: self.index,
                        misses: 0,
                        lines: vec![off],
                        peak_db: db((best.1 / noise) as f64),
                        bin: k,
                        weak: false,
                        unconfirmed: 0,
                        unconfirmed_misses: 0,
                    });
                }
            }
            if self.state.is_none() && self.avg_init >= WEAK_WARMUP && self.background_set {
                self.state = self.weak_start(lo, hi, block_start);
            }
            if self.state.is_none() && self.run_line && self.avg_init >= HIST {
                self.state = self.run_start(lo, hi, run_med, block_start);
            }
        } else if let Some(a) = &mut self.state {
            // The line near the burst's bin.
            let k0 = a.bin;
            let mut b2 = (k0, 0f32);
            for k in k0.saturating_sub(END_BINS)..=(k0 + END_BINS).min(N - 1) {
                if self.spec[k] > b2.1 {
                    b2 = (k, self.spec[k]);
                }
            }
            // A line of the run test holds the burst only while the burst can still end inside its longest duration.
            let can_hold = (self.index - a.start + (END_BLOCKS as u64 + 1) * N as u64) as f64
                <= self.max_burst_s * RATE;
            if b2.1 > thr || (self.run_line && can_hold && b2.1 >= RUN_OVER_MEDIAN * run_med) {
                a.misses = 0;
                a.unconfirmed = a.unconfirmed.saturating_sub(1);
                a.last_seen = self.index;
                if a.lines.len() < 4 {
                    let off =
                        (b2.0 as f64 - (N / 2) as f64 + parabolic_log(&self.spec, b2.0)) * bin_hz;
                    a.lines.push(off);
                    a.peak_db = a.peak_db.max(db((b2.1 / noise) as f64));
                }
            } else {
                a.misses += 1;
                a.unconfirmed_misses = a.unconfirmed_misses.saturating_add(1);
            }
            let dropped = a.unconfirmed > 0 && a.unconfirmed_misses >= RUN_CONFIRM_MISSES;
            let age = (self.index - a.start) as f64;
            let too_long = age > self.max_burst_s * RATE;
            if dropped {
                self.state = None;
            } else if (a.misses >= END_BLOCKS && age >= MIN_BURST_S * RATE) || too_long {
                if let Some(a) = self.state.take().filter(|_| !too_long) {
                    let b = self.finish(&a);
                    out.push(b);
                }
            }
        }
        // The background learns from frames where nothing was found; a bin is capped at four medians of its frame, so a
        // carrier present at the start is not learnt.
        if idle && self.state.is_none() {
            let cap = BACKGROUND_CAP * med;
            if self.background_set {
                for (b, &p) in self.background.iter_mut().zip(&self.spec) {
                    *b += BACKGROUND_ALPHA * (p.min(cap) - *b);
                }
            } else {
                for (b, &p) in self.background.iter_mut().zip(&self.spec) {
                    *b = p.min(cap);
                }
                self.background_set = true;
            }
        }
        // This spectrum joins the history after the decision, as the newest.
        self.hist[self.hist_pos].copy_from_slice(&self.spec);
        self.hist_med[self.hist_pos] = run_med;
        self.hist_pos = (self.hist_pos + 1) % HIST;
        self.avg_init += 1;
    }

    fn finish(&mut self, a: &Active) -> FgbBurst {
        let mut lines = a.lines.clone();
        lines.sort_by(|x, y| x.total_cmp(y));
        let coarse = lines[lines.len() / 2];
        // Samples from half a frame before the start to now.
        let s0 = a.start.saturating_sub(N as u64 / 2);
        let s1 = self.index;
        let n = (s1 - s0) as usize;
        let x: Vec<Complex32> = (0..n).map(|i| self.sample(s0 + i as u64)).collect();
        let level = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / n.max(1) as f64;
        let demod = self
            .decode
            .then(|| self.demod.demodulate(&x, coarse, a.start - s0, true));
        FgbBurst {
            start: a.start,
            end: a.last_seen,
            carrier_hz: demod.as_ref().map_or(coarse, |d| d.carrier_hz),
            cn_db: a.peak_db,
            level_dbfs: db(level),
            weak: a.weak,
            demod,
            samples: self.keep.then(|| (x, a.start - s0)),
        }
    }
}

/// The median of the bins within 96 of `k`, leaving out the 4 either side of it.
fn local_median(spec: &[f32], k: usize) -> f32 {
    let a = k.saturating_sub(96);
    let b = (k + 96).min(spec.len() - 1);
    let mut v: Vec<f32> = spec[a..=b]
        .iter()
        .enumerate()
        .filter(|(i, _)| (a + i).abs_diff(k) > 4)
        .map(|(_, &x)| x)
        .collect();
    let m = v.len() / 2;
    v.select_nth_unstable_by(m, |x, y| x.total_cmp(y));
    v[m]
}

/// The fractional bin of the peak at `k` (a parabola through the logarithms of three powers).
fn parabolic_log(v: &[f32], k: usize) -> f64 {
    if k == 0 || k + 1 >= v.len() {
        return 0.0;
    }
    log_parabolic(v[k - 1] as f64, v[k] as f64, v[k + 1] as f64).offset
}
