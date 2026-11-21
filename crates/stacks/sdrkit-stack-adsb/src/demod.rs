//! The Mode S pulse demodulator: power on a 0.25 µs grid, the noise floor, the preamble search, and the bit slicers. Each
//! candidate leaves with its slicing attempts in order. [`Demod`] is the plain type, [`ModeSDemod`] the component.
//!
//! Standard: ICAO Annex 10 Vol. IV 3.1.2.1 and 3.1.2.2

// Loops over several parallel arrays read best with an index.
#![allow(clippy::needless_range_loop)]

use crate::frontend::ChainDelay;
use crate::message::df_of;
use crate::settings::EFFORT;
use num_complex::Complex64;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom,
    RunContext,
};
use std::collections::VecDeque;
use std::time::Instant;

/// How a slice was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// Power slicer, 0.5 µs windows.
    Power,
    /// Coherent slicer.
    Coherent,
    /// Sequence estimation.
    Sequence,
    /// Power slicer with the timing taken from the data.
    Timing,
    /// Power slicer with a narrower window (the width in tenths of 0.5 µs: 6, 4 or 2 for 0.25).
    Narrow(u8),
    /// Power slicer with ties at a threshold (tenths: 3, 4, 5).
    Ties(u8),
    /// Given from outside (a test, a feed).
    Given,
}

/// 112 sliced bits with their confidences.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slice {
    /// The bits, the first transmitted the most significant bit of byte 0.
    pub bytes: [u8; 14],
    /// Confidence of each bit (larger is surer).
    pub conf: [f32; 112],
    /// How it was made.
    pub method: Method,
}

impl Slice {
    /// A slice of given bytes with equal confidences (for frames fed as sliced frames).
    pub fn given(bytes: &[u8]) -> Slice {
        let mut b = [0u8; 14];
        let n = bytes.len().min(14);
        b[..n].copy_from_slice(&bytes[..n]);
        Slice {
            bytes: b,
            conf: [1.0; 112],
            method: Method::Given,
        }
    }
    /// Bit `i` (0-based).
    pub fn bit(&self, i: usize) -> bool {
        self.bytes[i / 8] >> (7 - i % 8) & 1 == 1
    }
    fn set(&mut self, i: usize, v: bool) {
        let m = 0x80 >> (i % 8);
        if v {
            self.bytes[i / 8] |= m;
        } else {
            self.bytes[i / 8] &= !m;
        }
    }
}

/// A preamble candidate and its slicing attempts.
#[derive(Clone, Debug)]
pub struct Candidate {
    /// Stream time of the refined preamble start, seconds of signal (delays taken out).
    pub time: f64,
    /// Level, dBFS (mean power of the preamble pulses above the noise).
    pub level_dbfs: f32,
    /// Pulse power over noise per working-rate sample, dB.
    pub snr_db: f32,
    /// It passed the strict preamble test.
    pub strict: bool,
    /// Worth retrying.
    pub worth: bool,
    /// The attempts, in order.
    pub attempts: Attempts,
}

/// The most slicing attempts of one candidate.
pub const MAX_ATTEMPTS: usize = 10;

/// The slicing attempts of a candidate, in order, held without allocation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attempts {
    slices: [Slice; MAX_ATTEMPTS],
    len: usize,
}

impl Default for Attempts {
    fn default() -> Self {
        Attempts {
            slices: [Slice::given(&[]); MAX_ATTEMPTS],
            len: 0,
        }
    }
}

impl Attempts {
    /// Adds an attempt. An attempt beyond [`MAX_ATTEMPTS`] is not kept.
    pub fn push(&mut self, s: Slice) {
        if let Some(slot) = self.slices.get_mut(self.len) {
            *slot = s;
            self.len += 1;
        }
    }
}

impl std::ops::Deref for Attempts {
    type Target = [Slice];
    fn deref(&self) -> &[Slice] {
        &self.slices[..self.len]
    }
}

impl<'a> IntoIterator for &'a Attempts {
    type Item = &'a Slice;
    type IntoIter = std::slice::Iter<'a, Slice>;
    fn into_iter(self) -> Self::IntoIter {
        self.slices[..self.len].iter()
    }
}

/// Measurements the demodulator reports at every report (clock tick).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DemodStats {
    /// Signal time at the end of the pass, s.
    pub time: f64,
    /// Noise floor, dBFS (-120 before the first estimate).
    pub noise_dbfs: f32,
    /// Candidates sliced since the start.
    pub candidates: u64,
    /// Candidates refused by the data check.
    pub data_rejects: u64,
    /// The effort in use (0 to 2).
    pub effort: u8,
    /// A line for the log (an effort change).
    pub log: Option<&'static str>,
}

/// What the demodulator outputs.
#[derive(Clone, Debug)]
// A candidate travels by value, so making one allocates nothing. Most items are candidates.
#[allow(clippy::large_enum_variant)]
pub enum DemodOut {
    /// A candidate.
    Candidate(Candidate),
    /// The measurements, at a report.
    Stats(DemodStats),
}

impl Item for DemodOut {
    const KIND: &'static str = "modes.candidate";
}

/// The effort setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effort {
    /// Measured: steps down when busy.
    Auto,
    /// A fixed level 0 to 2.
    Fixed(u8),
}

sdrkit_core::choice_enum! {
    /// The effort setting: automatic, or a fixed level.
    pub enum EffortChoice {
        /// Steps down when the computer is busy.
        Auto = ("auto", "Automatic"),
        /// All passes (level 2).
        All = ("2", "All passes"),
        /// Strong candidates only (level 1).
        Strong = ("1", "Strong candidates only"),
        /// The plain detector (level 0).
        Plain = ("0", "Plain detector"),
    }
}

impl EffortChoice {
    /// The effort of this choice.
    pub fn effort(self) -> Effort {
        match self {
            EffortChoice::Auto => Effort::Auto,
            EffortChoice::All => Effort::Fixed(2),
            EffortChoice::Strong => Effort::Fixed(1),
            EffortChoice::Plain => Effort::Fixed(0),
        }
    }
}

/// Detector thresholds (test knobs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    /// Pulse windows above n0 (1 + k cv): k.
    pub pulse_k: f32,
    /// Data check: mean |E1 - E0| above k cv n0.
    pub data_k: f32,
    /// Strict test: pulses above n0 max(strict_min, 1 + strict_k cv).
    pub strict_min: f32,
    /// See `strict_min`.
    pub strict_k: f32,
    /// The coherent slicer and sequence estimation are used.
    pub coherent: bool,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            pulse_k: 0.0,
            data_k: 1.8,
            strict_min: 3.0,
            strict_k: 2.4,
            coherent: true,
        }
    }
}

const BLOCK_BINS: usize = 256;
const NOISE_BLOCKS: usize = 16;
const FFT_N: usize = 2048;

/// The demodulator.
pub struct Demod {
    fs: f64,
    us: f64,
    spb: f64,
    delay_s: f64,
    epoch: f64,
    th: Thresholds,
    effort_setting: Effort,
    effort: u8,
    // samples
    base: u64,
    z: Vec<Complex32>,
    p: Vec<f32>,
    cum: Vec<f64>,
    total: u64,
    // bins
    bin_base: u64,
    bins: Vec<f32>,
    wins: Vec<f32>,
    scores: Vec<f32>,
    next_bin: u64,
    next_score: u64,
    next_peak: u64,
    // noise
    blk_sum: f64,
    blk_wsum: f64,
    blk_wsq: f64,
    blk_n: usize,
    prev_bin: f32,
    blocks: VecDeque<(f32, f32)>,
    n0: Option<f32>,
    cv: f32,
    // candidates
    pending: VecDeque<(u64, f32, f32)>,
    skip_until: u64,
    // counters
    candidates: u64,
    data_rejects: u64,
    // effort measurement
    busy_s: f64,
    measured_s: f64,
    log: Option<&'static str>,
    // scratch
    fft: Fft,
    fft_buf: Vec<Complex32>,
    zr: Vec<Complex64>,
    cz: Vec<Complex64>,
    live: bool,
    last_wall: Option<Instant>,
}

impl Demod {
    /// A demodulator for a working rate (after any interpolation), with the delay (s) the front end added.
    pub fn new(fs: f64, delay_s: f64) -> Demod {
        let mut d = Demod {
            fs,
            us: fs * 1e-6,
            spb: fs * 0.25e-6,
            delay_s,
            epoch: 0.0,
            th: Thresholds::default(),
            effort_setting: Effort::Auto,
            effort: 2,
            base: 0,
            z: Vec::new(),
            p: Vec::new(),
            cum: Vec::new(),
            total: 0,
            bin_base: 0,
            bins: Vec::new(),
            wins: Vec::new(),
            scores: Vec::new(),
            next_bin: 0,
            next_score: 0,
            next_peak: 0,
            blk_sum: 0.0,
            blk_wsum: 0.0,
            blk_wsq: 0.0,
            blk_n: 0,
            prev_bin: 0.0,
            blocks: VecDeque::with_capacity(NOISE_BLOCKS + 1),
            n0: None,
            cv: 1.0,
            pending: VecDeque::new(),
            skip_until: 0,
            candidates: 0,
            data_rejects: 0,
            busy_s: 0.0,
            measured_s: 0.0,
            log: None,
            fft: Fft::forward(FFT_N),
            fft_buf: vec![Complex32::new(0.0, 0.0); FFT_N],
            zr: Vec::new(),
            cz: Vec::new(),
            live: false,
            last_wall: None,
        };
        d.reset_stream(fs, delay_s);
        d
    }

    /// Sets the delay of the chain before the demodulator, seconds. Times of later candidates are earlier by this much.
    pub fn set_delay(&mut self, delay_s: f64) {
        self.delay_s = delay_s;
    }

    /// Starts a new stream at this rate; the time base continues from where the last stream ended.
    pub fn reset_stream(&mut self, fs: f64, delay_s: f64) {
        if self.fs > 0.0 {
            self.epoch += self.total as f64 / self.fs;
        }
        self.fs = fs;
        self.us = fs * 1e-6;
        self.spb = fs * 0.25e-6;
        self.delay_s = delay_s;
        self.base = 0;
        self.z.clear();
        self.p.clear();
        self.cum.clear();
        self.cum.push(0.0);
        self.total = 0;
        self.bin_base = 0;
        self.bins.clear();
        self.wins.clear();
        self.scores.clear();
        self.next_bin = 0;
        self.next_score = 0;
        self.next_peak = 0;
        self.blk_sum = 0.0;
        self.blk_wsum = 0.0;
        self.blk_wsq = 0.0;
        self.blk_n = 0;
        self.prev_bin = 0.0;
        self.blocks.clear();
        self.n0 = None;
        self.cv = 1.0;
        self.pending.clear();
        self.skip_until = 0;
        self.last_wall = None;
    }

    /// Sets the thresholds.
    pub fn set_thresholds(&mut self, th: Thresholds) {
        self.th = th;
    }
    /// The thresholds.
    pub fn thresholds(&self) -> Thresholds {
        self.th
    }
    /// Sets the effort.
    pub fn set_effort(&mut self, e: Effort) {
        self.effort_setting = e;
        if let Effort::Fixed(l) = e {
            self.effort = l.min(2);
        }
    }
    /// Marks the input as live (a radio), so a long pause moves the signal clock on.
    pub fn set_live(&mut self, live: bool) {
        self.live = live;
    }
    /// The noise floor in dBFS.
    pub fn noise_dbfs(&self) -> f32 {
        self.n0.map_or(-120.0, |n| 10.0 * n.log10())
    }
    /// The signal time at the end of what was fed.
    pub fn time(&self) -> f64 {
        self.epoch + self.total as f64 / self.fs - self.delay_s
    }

    /// The measurements now (takes the pending log line).
    pub fn stats(&mut self) -> DemodStats {
        DemodStats {
            time: self.time(),
            noise_dbfs: self.noise_dbfs(),
            candidates: self.candidates,
            data_rejects: self.data_rejects,
            effort: self.effort,
            log: self.log.take(),
        }
    }

    /// Feeds samples; candidates are appended to `out`.
    pub fn feed(&mut self, iq: &[Complex32], out: &mut Vec<Candidate>) {
        let t0 = Instant::now();
        if self.live {
            if let Some(w) = self.last_wall {
                let gap = t0.duration_since(w).as_secs_f64();
                if gap > 1.0 {
                    self.epoch += gap;
                }
            }
        }
        let mut c = *self.cum.last().unwrap_or(&0.0);
        let mut prev = self.p.last().copied();
        for &s in iq {
            let pw = s.norm_sqr();
            if let Some(q) = prev {
                c += 0.5 * (q + pw) as f64;
                self.cum.push(c);
            }
            self.z.push(s);
            self.p.push(pw);
            prev = Some(pw);
        }
        self.total += iq.len() as u64;
        self.run(out);
        self.trim();
        let busy = t0.elapsed().as_secs_f64();
        self.measure(busy, iq.len() as f64 / self.fs);
        self.last_wall = Some(Instant::now());
    }

    fn measure(&mut self, busy: f64, signal: f64) {
        if self.effort_setting != Effort::Auto {
            return;
        }
        self.busy_s += busy;
        self.measured_s += signal;
        if self.measured_s >= 0.5 {
            let share = self.busy_s / self.measured_s;
            if share > 0.5 && self.effort > 0 {
                self.effort -= 1;
                self.log =
                    Some("the computer is busy, fewer extra decoding passes for weak signals");
            } else if share < 0.25 && self.effort < 2 {
                self.effort += 1;
                self.log = Some("back to more decoding passes for weak signals");
            }
            self.busy_s = 0.0;
            self.measured_s = 0.0;
        }
    }

    // ---- integrals ----

    #[inline]
    fn integ(&self, x: f64) -> f64 {
        let l = x - self.base as f64;
        let n = (l.floor() as usize).min(self.p.len().saturating_sub(2));
        let f = l - n as f64;
        let p0 = self.p[n] as f64;
        let p1 = self.p[n + 1] as f64;
        self.cum[n] + p0 * f + (p1 - p0) * f * f * 0.5
    }

    /// Energy (power integral, sample units) over [a, a + len).
    #[inline]
    fn energy(&self, a: f64, len: f64) -> f64 {
        self.integ(a + len) - self.integ(a)
    }

    fn score_of(&self, k: u64) -> f32 {
        self.scores[(k - self.bin_base) as usize]
    }

    fn run(&mut self, out: &mut Vec<Candidate>) {
        // Bins.
        loop {
            let k = self.next_bin;
            let end = (k + 1) as f64 * self.spb;
            let need = end.floor() as u64 + 1;
            if need >= self.total {
                break;
            }
            let b = if self.spb.fract() == 0.0 {
                // A whole number of samples per bin: the integral at sample positions is the running sum itself.
                let st = (k * self.spb as u64 - self.base) as usize;
                ((self.cum[st + self.spb as usize] - self.cum[st]) / self.spb) as f32
            } else {
                ((self.integ(end) - self.integ(k as f64 * self.spb)) / self.spb) as f32
            };
            if let Some(&last) = self.bins.last() {
                self.wins.push(0.5 * (last + b));
            }
            self.bins.push(b);
            self.scores.push(0.0);
            self.next_bin += 1;
            self.noise_bin(b);
            // Score the bin whose windows are now complete.
            while self.next_score + 32 <= self.next_bin {
                let s = self.score(self.next_score);
                let i = (self.next_score - self.bin_base) as usize;
                self.scores[i] = s;
                self.next_score += 1;
            }
            while self.next_peak + 7 <= self.next_score {
                let b = self.next_peak;
                self.next_peak += 1;
                if self.is_peak(b) {
                    if let Some(n0) = self.n0 {
                        self.pending.push_back((b, n0, self.cv));
                    }
                }
            }
        }
        // Candidates whose frame is held.
        while let Some(&(b, n0, cv)) = self.pending.front() {
            let need = b as f64 * self.spb + 124.0 * self.us + 2.0;
            if need as u64 + 2 >= self.total {
                break;
            }
            self.pending.pop_front();
            if b < self.skip_until {
                continue;
            }
            if let Some(c) = self.candidate(b, n0, cv) {
                out.push(c);
            }
        }
    }

    fn noise_bin(&mut self, b: f32) {
        let w = 0.5 * (b + self.prev_bin);
        self.prev_bin = b;
        self.blk_sum += b as f64;
        self.blk_wsum += w as f64;
        self.blk_wsq += (w as f64) * (w as f64);
        self.blk_n += 1;
        if self.blk_n == BLOCK_BINS {
            let n = BLOCK_BINS as f64;
            let mean = (self.blk_sum / n) as f32;
            let wm = self.blk_wsum / n;
            let var = (self.blk_wsq / n - wm * wm).max(0.0);
            let cv = if wm > 0.0 {
                (var.sqrt() / wm) as f32
            } else {
                1.0
            };
            if self.blocks.len() == NOISE_BLOCKS {
                self.blocks.pop_front();
            }
            self.blocks.push_back((mean, cv));
            if self.blocks.len() >= 4 {
                let mut v: [(f32, f32); NOISE_BLOCKS] = [(0.0, 0.0); NOISE_BLOCKS];
                let n = self.blocks.len();
                for (d, s) in v.iter_mut().zip(&self.blocks) {
                    *d = *s;
                }
                let s = &mut v[..n];
                s.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
                let (m, c) = s[n / 4];
                let est = (m / 0.93).max(1e-9);
                match self.n0 {
                    None => {
                        self.n0 = Some(est);
                        self.cv = c;
                    }
                    Some(n0) => {
                        self.n0 = Some(0.5 * n0 + 0.5 * est);
                        self.cv = 0.5 * self.cv + 0.5 * c;
                    }
                }
            }
            self.blk_sum = 0.0;
            self.blk_wsum = 0.0;
            self.blk_wsq = 0.0;
            self.blk_n = 0;
        }
    }

    fn score(&self, b: u64) -> f32 {
        let Some(n0) = self.n0 else { return 0.0 };
        if b < self.bin_base {
            return 0.0;
        }
        let th = n0 * (1.0 + self.th.pulse_k * self.cv);
        let i = (b - self.bin_base) as usize;
        let w = &self.wins[i..i + 31];
        let (p1, p2) = (w[0], w[4]);
        if p1 <= th || p2 <= th {
            return 0.0;
        }
        let (p3, p4) = (w[14], w[18]);
        if p3 <= th || p4 <= th {
            return 0.0;
        }
        let qi = w[2] + w[6] + w[8] + w[10] + w[12] + w[16];
        let qo = w[20] + w[22] + w[24] + w[26] + w[28] + w[30];
        let pm = (p1 + p2 + p3 + p4) * 0.25;
        let q = (qi + qo) / 12.0;
        let qim = qi / 6.0;
        if pm > 2.0 * q && pm > 2.0 * qim && pm - q > 4.0 * 0.58 * self.cv * n0 {
            pm - q
        } else {
            0.0
        }
    }

    fn is_peak(&self, b: u64) -> bool {
        let s = self.score_of(b);
        if s <= 0.0 {
            return false;
        }
        for k in 1..=6u64 {
            if b >= self.bin_base + k && self.score_of(b - k) > s {
                return false;
            }
            if self.score_of(b + k) >= s {
                return false;
            }
        }
        true
    }

    /// Mean power of the 0.5 µs window starting `t` µs after `x0`.
    fn wmean(&self, x0: f64, t: f64) -> f64 {
        self.energy(x0 + t * self.us, 0.5 * self.us) / (0.5 * self.us)
    }

    fn candidate(&mut self, b: u64, n0: f32, cv: f32) -> Option<Candidate> {
        let us = self.us;
        let xb = b as f64 * self.spb;
        if xb - 0.6 * us < self.base as f64 {
            return None;
        }
        // Timing refinement.
        let mut best = (f64::MIN, xb);
        for k in -10..=10 {
            let x = xb + k as f64 * 0.05 * us;
            let e = self.energy(x, 0.5 * us)
                + self.energy(x + us, 0.5 * us)
                + self.energy(x + 3.5 * us, 0.5 * us)
                + self.energy(x + 4.5 * us, 0.5 * us);
            if e > best.0 {
                best = (e, x);
            }
        }
        let x0 = best.1;
        // Fine test.
        let (n0d, cvd) = (n0 as f64, cv as f64);
        let pulses = [
            self.wmean(x0, 0.0),
            self.wmean(x0, 1.0),
            self.wmean(x0, 3.5),
            self.wmean(x0, 4.5),
        ];
        let th = n0d * (1.0 + self.th.pulse_k as f64 * cvd);
        if pulses.iter().any(|&p| p <= th) {
            return None;
        }
        let mut qi = 0.0;
        for t in [0.5, 1.5, 2.0, 2.5, 3.0, 4.0] {
            qi += self.wmean(x0, t);
        }
        let mut qo = 0.0;
        for t in [5.0, 5.5, 6.0, 6.5, 7.0, 7.5] {
            qo += self.wmean(x0, t);
        }
        let pm = pulses.iter().sum::<f64>() / 4.0;
        let q = (qi + qo) / 12.0;
        let qim = qi / 6.0;
        if !(pm > 2.0 * q && pm > 2.0 * qim && pm - q > 4.0 * 0.58 * cvd * n0d) {
            return None;
        }
        let mut dsum = 0.0;
        for k in 0..56 {
            let t = 8.0 + k as f64;
            dsum += (self.wmean(x0, t) - self.wmean(x0, t + 0.5)).abs();
        }
        if dsum / 56.0 <= self.th.data_k as f64 * cvd * n0d {
            self.data_rejects += 1;
            return None;
        }
        // Strict test.
        let sth = n0d * (self.th.strict_min as f64).max(1.0 + self.th.strict_k as f64 * cvd);
        let strict = pulses.iter().all(|&p| p > sth) && pm > 2.0 * q && pm > 2.0 * qim;
        if self.effort == 0 && !strict {
            return None;
        }
        // Level, SNR, worth retrying.
        let s = (pm - n0d).max(1e-12);
        let level = 10.0 * s.log10();
        let snr = 10.0 * (s / n0d).log10();
        let worth = 10.0 * (s / (n0d * (cvd * cvd).max(1e-3))).log10() >= 5.0;
        self.candidates += 1;
        let mut attempts = Attempts::default();
        let first = self.power_slice(x0, 0.5, None, s);
        let good = |sl: &Slice| clean_pi(sl);
        let mut done = good(&first);
        attempts.push(first);
        let extra = self.effort == 2 || strict;
        if !done && extra && self.effort > 0 && self.th.coherent {
            let guide = attempts[0];
            if let Some((coh, seq)) = self.coherent(x0, &guide) {
                done = good(&coh);
                attempts.push(coh);
                if !done {
                    if let Some(seq) = seq {
                        done = good(&seq);
                        attempts.push(seq);
                    }
                }
            }
        }
        if !done && extra && worth {
            // Timing from the data.
            let mut bestt = (f64::MIN, 0i32);
            for k in -4..=4 {
                let x = x0 + k as f64 * 0.04 * us;
                let mut e = 0.0;
                for t in [0.0, 1.0, 3.5, 4.5] {
                    e += self.energy(x + t * us, 0.5 * us);
                }
                for i in 0..56 {
                    let a = x + (8.0 + i as f64) * us;
                    e += self
                        .energy(a, 0.5 * us)
                        .max(self.energy(a + 0.5 * us, 0.5 * us));
                }
                if e > bestt.0 {
                    bestt = (e, k);
                }
            }
            if bestt.1 != 0 {
                let mut sl = self.power_slice(x0 + bestt.1 as f64 * 0.04 * us, 0.5, None, s);
                sl.method = Method::Timing;
                done = good(&sl);
                attempts.push(sl);
            }
            for (w, tag) in [(0.6, 6u8), (0.4, 4), (0.25, 2)] {
                if done {
                    break;
                }
                let mut sl = self.power_slice(x0, 0.5 * w, None, s);
                sl.method = Method::Narrow(tag);
                done = good(&sl);
                attempts.push(sl);
            }
            for (t, tag) in [(0.3, 3u8), (0.4, 4), (0.5, 5)] {
                if done {
                    break;
                }
                let mut sl = self.power_slice(x0, 0.5, Some(t), s);
                sl.method = Method::Ties(tag);
                done = good(&sl);
                attempts.push(sl);
            }
        }
        if done {
            let bits = if attempts.last().is_some_and(|a| df_of(a.bytes[0]) >= 16) {
                112
            } else {
                56
            };
            self.skip_until = b + (8 + bits) * 4 - 4;
        }
        Some(Candidate {
            time: self.epoch + x0 / self.fs - self.delay_s,
            level_dbfs: level as f32,
            snr_db: snr as f32,
            strict,
            worth,
            attempts,
        })
    }

    /// Power slice: windows of `w_us` centred at +0.25 and +0.75 µs of each bit.
    fn power_slice(&self, x0: f64, w_us: f64, ties: Option<f64>, s: f64) -> Slice {
        let us = self.us;
        let w = w_us * us;
        let mut sl = Slice {
            bytes: [0; 14],
            conf: [0.0; 112],
            method: Method::Power,
        };
        let mut prev = true;
        for k in 0..112 {
            let c = x0 + (8.0 + k as f64) * us;
            let e1 = self.energy(c + 0.25 * us - 0.5 * w, w);
            let e0 = self.energy(c + 0.75 * us - 0.5 * w, w);
            let d = e1 - e0;
            let mut v = d > 0.0;
            if let Some(t) = ties {
                if d.abs() < t * s * w {
                    v = prev;
                }
            }
            sl.set(k, v);
            sl.conf[k] = (d.abs() / w) as f32;
            prev = v;
        }
        sl
    }

    /// The coherent slice and, when it differs, the sequence-estimation slice.
    fn coherent(&mut self, x0: f64, guide: &Slice) -> Option<(Slice, Option<Slice>)> {
        let us = self.us;
        let len = if guide.bit(0) { 112 } else { 56 };
        let lo = (x0 - 1.0).floor() as i64 - self.base as i64;
        let hi = (x0 + (8.0 + len as f64 + 1.0) * us).ceil() as i64 + 2 - self.base as i64;
        if lo < 0 || hi as usize >= self.z.len() {
            return None;
        }
        let (lo, hi) = (lo as usize, hi as usize);
        let rel = |x: f64| x - self.base as f64 - lo as f64;
        // Pulse windows (start in local samples) chosen by the guide.
        let hw = 0.5 * us;
        let mut starts = [0f64; 4 + 112];
        for (i, t) in [0.0, 1.0, 3.5, 4.5].into_iter().enumerate() {
            starts[i] = rel(x0 + t * us);
        }
        for k in 0..len {
            let t = 8.0 + k as f64 + if guide.bit(k) { 0.0 } else { 0.5 };
            starts[4 + k] = rel(x0 + t * us);
        }
        let pulse_starts = &starts[..4 + len];
        // Coarse offset from lag-one products inside the pulses.
        let mut acc = Complex64::new(0.0, 0.0);
        for &a in pulse_starts {
            let (mut s, mut e) = (
                (a + 0.1 * hw).ceil() as usize,
                (a + 0.9 * hw).floor() as usize,
            );
            if e <= s {
                s = a.round().max(0.0) as usize;
                e = (a + hw).round() as usize;
            }
            for n in s..e.min(hi - lo - 1) {
                let z0 = self.z[lo + n];
                let z1 = self.z[lo + n + 1];
                let v = z1 * z0.conj();
                acc += Complex64::new(v.re as f64, v.im as f64);
            }
        }
        let w0 = acc.arg();
        // Derotated samples and their cumulative trapezoid integral.
        self.zr.clear();
        self.cz.clear();
        self.cz.push(Complex64::new(0.0, 0.0));
        let rot = Complex64::from_polar(1.0, -w0);
        let mut ph = Complex64::new(1.0, 0.0);
        for n in lo..=hi {
            let v = self.z[n];
            self.zr.push(Complex64::new(v.re as f64, v.im as f64) * ph);
            ph *= rot;
            if self.zr.len() >= 2 {
                let m = self.zr.len();
                let c = self.cz[m - 2] + (self.zr[m - 2] + self.zr[m - 1]) * 0.5;
                self.cz.push(c);
            }
        }
        let zr = &self.zr;
        let cz = &self.cz;
        let cint = |x: f64| -> Complex64 {
            let n = (x.floor().max(0.0) as usize).min(zr.len() - 2);
            let f = x - n as f64;
            cz[n] + zr[n] * f + (zr[n + 1] - zr[n]) * (f * f * 0.5)
        };
        let phasor = |a: f64| cint(a + hw) - cint(a);
        // Half-bit phasors (window 0.5 µs centred at +0.25 / +0.75), and preamble phasors.
        let mut a1 = [Complex64::new(0.0, 0.0); 112];
        let mut a0 = [Complex64::new(0.0, 0.0); 112];
        for k in 0..len {
            let c = rel(x0 + (8.0 + k as f64) * us);
            a1[k] = phasor(c);
            a0[k] = phasor(c + hw);
        }
        let pre = [0, 1, 2, 3].map(|i| phasor(pulse_starts[i]));
        let pre_g = [1.0, 5.0, 15.0, 19.0];
        let g1 = |k: usize| 33.0 + 4.0 * k as f64;
        let mut bits = [false; 112];
        for (k, b) in bits.iter_mut().enumerate().take(len) {
            *b = guide.bit(k);
        }
        let mut conf = [0f32; 112];
        let mut theta = 0.0;
        let mut refs = [Complex64::new(0.0, 0.0); 112];
        let mut plist = [Complex64::new(0.0, 0.0); 4 + 112];
        for _pass in 0..2 {
            // Fine offset: FFT of the pulse phasors on the 0.25 µs grid.
            for v in self.fft_buf.iter_mut() {
                *v = Complex32::new(0.0, 0.0);
            }
            let mut add = |g: f64, v: Complex64| {
                let i = g.round() as usize;
                if i < FFT_N {
                    self.fft_buf[i] += Complex32::new(v.re as f32, v.im as f32);
                }
            };
            for i in 0..4 {
                add(pre_g[i], pre[i]);
            }
            for k in 0..len {
                if bits[k] {
                    add(g1(k), a1[k]);
                } else {
                    add(g1(k) + 2.0, a0[k]);
                }
            }
            let mut fb = std::mem::take(&mut self.fft_buf);
            let mut held = [(0f64, Complex64::new(0.0, 0.0)); 4 + 112];
            let mut n_held = 0;
            for (i, v) in fb.iter().enumerate() {
                if (v.re != 0.0 || v.im != 0.0) && n_held < held.len() {
                    held[n_held] = (i as f64, Complex64::new(v.re as f64, v.im as f64));
                    n_held += 1;
                }
            }
            let samples = &held[..n_held];
            self.fft.process(&mut fb);
            let m_max = (0.3 * FFT_N as f64 / std::f64::consts::TAU).ceil() as i64;
            let mag = |m: i64| fb[m.rem_euclid(FFT_N as i64) as usize].norm() as f64;
            let mut bm = (0.0, 0i64);
            for m in -m_max..=m_max {
                let v = mag(m);
                if v > bm.0 {
                    bm = (v, m);
                }
            }
            let (ya, yb, yc) = (mag(bm.1 - 1), mag(bm.1), mag(bm.1 + 1));
            let den = ya - 2.0 * yb + yc;
            let delta = if den.abs() > 1e-30 {
                (0.5 * (ya - yc) / den).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            let th = std::f64::consts::TAU * (bm.1 as f64 + delta) / FFT_N as f64;
            let sum = |t: f64| {
                samples
                    .iter()
                    .map(|(g, v)| v * Complex64::from_polar(1.0, -t * g))
                    .sum::<Complex64>()
                    .norm()
            };
            theta = if sum(th) >= sum(-th) { th } else { -th };
            self.fft_buf = fb;
            // Derotate pulse phasors and decide.
            let d = |v: Complex64, g: f64| v * Complex64::from_polar(1.0, -theta * g);
            for i in 0..4 {
                plist[i] = d(pre[i], pre_g[i]);
            }
            for k in 0..len {
                plist[4 + k] = if bits[k] {
                    d(a1[k], g1(k))
                } else {
                    d(a0[k], g1(k) + 2.0)
                };
            }
            let plist = &plist[..4 + len];
            for k in 0..len {
                let i = 4 + k;
                let n = plist.len();
                let mut s = i.saturating_sub(16);
                let mut e = s + 33;
                if e > n {
                    e = n;
                    s = n.saturating_sub(33);
                }
                let mut r = Complex64::new(0.0, 0.0);
                for (j, v) in plist.iter().enumerate().take(e).skip(s) {
                    if j != i {
                        r += v;
                    }
                }
                let rn = r.norm();
                refs[k] = if rn > 0.0 {
                    r / rn
                } else {
                    Complex64::new(1.0, 0.0)
                };
            }
            for k in 0..len {
                let diff = d(a1[k], g1(k)) - d(a0[k], g1(k) + 2.0);
                let v = (diff * refs[k].conj()).re;
                bits[k] = v > 0.0;
                conf[k] = (v.abs() / hw) as f32;
            }
        }
        let mut coh = *guide;
        coh.method = Method::Coherent;
        for k in 0..len {
            coh.set(k, bits[k]);
            coh.conf[k] = conf[k];
        }
        // Sequence estimation along the carrier phase.
        let mut seq_bits = bits;
        for _pass in 0..2 {
            let d = |v: Complex64, g: f64| v * Complex64::from_polar(1.0, -theta * g);
            let mut a = [0f64; 2 * 112];
            for k in 0..len {
                a[2 * k] = (d(a1[k], g1(k)) * refs[k].conj()).re;
                a[2 * k + 1] = (d(a0[k], g1(k) + 2.0) * refs[k].conj()).re;
            }
            let p = |j: i64, b: &[bool]| -> f64 {
                if j < 0 || j >= 2 * len as i64 {
                    return 0.0;
                }
                let k = (j / 2) as usize;
                if (j % 2 == 0) == b[k] {
                    1.0
                } else {
                    0.0
                }
            };
            // Least squares for (g-, g0, g+).
            let mut m = [[0f64; 3]; 3];
            let mut r = [0f64; 3];
            for j in 0..2 * len as i64 {
                let x = [p(j - 1, &seq_bits), p(j, &seq_bits), p(j + 1, &seq_bits)];
                for u in 0..3 {
                    r[u] += x[u] * a[j as usize];
                    for v in 0..3 {
                        m[u][v] += x[u] * x[v];
                    }
                }
            }
            let Some(g) = solve3(m, r) else { break };
            if g[1] <= 0.0 || g[0].abs() > g[1] || g[2].abs() > g[1] {
                break;
            }
            // Two-state Viterbi: state = previous bit.
            let pred = |b_prev: Option<bool>, b: bool, b_next: Option<bool>| -> (f64, f64) {
                let pm1 = match b_prev {
                    Some(v) => !v as u8 as f64,
                    None => 0.0,
                };
                let (h0, h1) = (b as u8 as f64, !b as u8 as f64);
                let pn = match b_next {
                    Some(v) => v as u8 as f64,
                    None => 0.0,
                };
                (
                    g[0] * pm1 + g[1] * h0 + g[2] * h1,
                    g[0] * h0 + g[1] * h1 + g[2] * pn,
                )
            };
            let mut back = [[[0u8; 2]; 2]; 112];
            // A half bit depends on both neighbouring bits, so the programme runs over pairs (b_k, b_k+1).
            let mut dp = [[f64::MAX; 2]; 2]; // dp[b_k][b_{k+1}]
            if len >= 2 {
                for b0 in 0..2 {
                    for b1 in 0..2 {
                        let (e0, e1) = pred(None, b0 == 1, Some(b1 == 1));
                        dp[b0][b1] = (a[0] - e0).powi(2) + (a[1] - e1).powi(2);
                    }
                }
                for k in 1..len {
                    let mut nd = [[f64::MAX; 2]; 2];
                    for bk in 0..2 {
                        for bn in 0..2 {
                            let next = (k + 1 < len).then_some(bn == 1);
                            if k + 1 >= len && bn == 1 {
                                continue;
                            }
                            for bp in 0..2 {
                                let prevc = dp[bp][bk];
                                if prevc == f64::MAX {
                                    continue;
                                }
                                let (e0, e1) = pred(Some(bp == 1), bk == 1, next);
                                let c =
                                    prevc + (a[2 * k] - e0).powi(2) + (a[2 * k + 1] - e1).powi(2);
                                if c < nd[bk][bn] {
                                    nd[bk][bn] = c;
                                    back[k][bk][bn] = bp as u8;
                                }
                            }
                        }
                    }
                    dp = nd;
                }
                // Best final (b_{len-1}, dummy 0).
                let (mut bk, mut bestc) = (0usize, f64::MAX);
                for (b, row) in dp.iter().enumerate() {
                    if row[0] < bestc {
                        bestc = row[0];
                        bk = b;
                    }
                }
                let mut bn = 0usize;
                let mut out = [false; 112];
                for k in (1..len).rev() {
                    out[k] = bk == 1;
                    let bp = back[k][bk][bn] as usize;
                    bn = bk;
                    bk = bp;
                }
                out[0] = bk == 1;
                seq_bits = out;
            }
            // The carrier phase from the sequence, for the second pass.
            let mut plist = [Complex64::new(0.0, 0.0); 4 + 112];
            for i in 0..4 {
                plist[i] = d(pre[i], pre_g[i]);
            }
            for k in 0..len {
                plist[4 + k] = if seq_bits[k] {
                    d(a1[k], g1(k))
                } else {
                    d(a0[k], g1(k) + 2.0)
                };
            }
            let plist = &plist[..4 + len];
            for k in 0..len {
                let i = 4 + k;
                let n = plist.len();
                let mut s = i.saturating_sub(16);
                let mut e = s + 33;
                if e > n {
                    e = n;
                    s = n.saturating_sub(33);
                }
                let mut r = Complex64::new(0.0, 0.0);
                for (j, v) in plist.iter().enumerate().take(e).skip(s) {
                    if j != i {
                        r += v;
                    }
                }
                let rn = r.norm();
                refs[k] = if rn > 0.0 {
                    r / rn
                } else {
                    Complex64::new(1.0, 0.0)
                };
            }
        }
        let seq = if seq_bits[..len] != bits[..len] {
            let mut s = coh;
            s.method = Method::Sequence;
            for (k, &b) in seq_bits.iter().enumerate().take(len) {
                s.set(k, b);
            }
            Some(s)
        } else {
            None
        };
        Some((coh, seq))
    }

    fn trim(&mut self) {
        // Samples: keep what pending candidates and the bin search still need.
        let first_bin = self
            .pending
            .front()
            .map_or(self.next_peak, |p| p.0)
            .min(self.next_peak)
            .saturating_sub(16);
        let keep_from = ((first_bin as f64 * self.spb - 2.0 * self.us)
            .floor()
            .max(0.0) as u64)
            .min(self.total.saturating_sub(4));
        if keep_from > self.base + 65536 {
            let d = (keep_from - self.base) as usize;
            self.z.drain(..d);
            self.p.drain(..d);
            self.cum.drain(..d);
            let c0 = self.cum[0];
            for c in self.cum.iter_mut() {
                *c -= c0;
            }
            self.base = keep_from;
        }
        let bkeep = first_bin.saturating_sub(8);
        if bkeep > self.bin_base + 16384 {
            let d = (bkeep - self.bin_base) as usize;
            self.bins.drain(..d);
            self.wins.drain(..d);
            self.scores.drain(..d);
            self.bin_base = bkeep;
        }
    }
}

fn solve3(m: [[f64; 3]; 3], r: [f64; 3]) -> Option<[f64; 3]> {
    let det = |a: [[f64; 3]; 3]| {
        a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
            - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
    };
    let d = det(m);
    let scale = m
        .iter()
        .flatten()
        .map(|v| v.abs())
        .fold(0.0, f64::max)
        .powi(3);
    if d.abs() <= 1e-9 * scale.max(1e-300) {
        return None;
    }
    let mut out = [0.0; 3];
    for (c, o) in out.iter_mut().enumerate() {
        let mut a = m;
        for row in 0..3 {
            a[row][c] = r[row];
        }
        *o = det(a) / d;
    }
    Some(out)
}

/// A slice that is a PI format with a clean parity: it will certainly be accepted.
fn clean_pi(s: &Slice) -> bool {
    let df = s.bytes[0] >> 3;
    match df {
        17..=19 => crate::crc::remainder(&s.bytes) == 0,
        11 => crate::crc::remainder(&s.bytes[..7]) == 0,
        _ => false,
    }
}

const COHERENT: Setting<field::Bool> = Setting::new("coherent", "Coherent slicer", "", true);
const PULSE_K: Setting<field::F64> =
    Setting::new("pulse_k", "Pulse threshold", "", 0.0).range(0.0, 10.0, 0.0);
const DATA_K: Setting<field::F64> =
    Setting::new("data_k", "Data check", "", 1.8).range(0.0, 10.0, 0.0);
const STRICT_MIN: Setting<field::F64> =
    Setting::new("strict_min", "Strict minimum", "", 3.0).range(0.0, 20.0, 0.0);
const STRICT_K: Setting<field::F64> =
    Setting::new("strict_k", "Strict factor", "", 2.4).range(0.0, 10.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`ModeSDemod`]: the effort, and the detector thresholds.
    pub enum DemodSetting {
        /// Effort.
        Effort = EFFORT,
        /// The coherent slicer and sequence estimation.
        Coherent = COHERENT,
        /// Pulse threshold.
        PulseK = PULSE_K,
        /// Data check.
        DataK = DATA_K,
        /// Strict minimum.
        StrictMin = STRICT_MIN,
        /// Strict factor.
        StrictK = STRICT_K,
    }
}

/// The demodulator as a component (`adsb.demod`): IQ in, `modes.candidate` out. Input 1 takes the delay of the chain before
/// it. It reports at each tick of the graph's clock.
pub struct ModeSDemod {
    d: Demod,
    effort: EffortChoice,
    out: Vec<Candidate>,
}

impl Default for ModeSDemod {
    fn default() -> Self {
        ModeSDemod {
            d: Demod::new(4e6, 0.0),
            effort: EFFORT.default,
            out: Vec::new(),
        }
    }
}

impl ModeSDemod {
    /// The plain demodulator.
    pub fn demod(&self) -> &Demod {
        &self.d
    }
}

impl Component for ModeSDemod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "adsb.demod",
            vec![
                Port::iq("in"),
                Port::typed::<ChainDelay>("delay").optional("no delay"),
            ],
            vec![Port::typed::<DemodOut>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(DemodSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate < 3.9e6 {
            return Err(ConfigError::RateTooLow {
                need: 4e6,
                got: f.rate,
            });
        }
        let delay_s = self.d.delay_s;
        self.d.reset_stream(f.rate, delay_s);
        Ok(vec![Format::new(Kind::typed::<DemodOut>(), 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.d.set_live(ctx.live);
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(d) = io.items::<ChainDelay>(1).last() {
            self.d.set_delay(d.value.delay_s);
        }
        let iq = io.iq(0);
        self.out.clear();
        let mut out = std::mem::take(&mut self.out);
        self.d.feed(iq, &mut out);
        let tick = io.tick();
        let stats = tick.then(|| self.d.stats());
        let o = io.items_out::<DemodOut>(0);
        for c in out.drain(..) {
            let t = c.time;
            o.push(sdrkit_graph::Timed {
                time: t,
                value: DemodOut::Candidate(c),
            });
        }
        if let Some(s) = stats {
            o.push(sdrkit_graph::Timed {
                time: s.time,
                value: DemodOut::Stats(s),
            });
        }
        self.out = out;
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let mut th = self.d.thresholds();
        match DemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DemodSetting::Effort => {
                self.effort = EFFORT.read(id, v)?;
                self.d.set_effort(self.effort.effort());
            }
            DemodSetting::Coherent => th.coherent = COHERENT.read(id, v)?,
            DemodSetting::PulseK => th.pulse_k = PULSE_K.read(id, v)? as f32,
            DemodSetting::DataK => th.data_k = DATA_K.read(id, v)? as f32,
            DemodSetting::StrictMin => th.strict_min = STRICT_MIN.read(id, v)? as f32,
            DemodSetting::StrictK => th.strict_k = STRICT_K.read(id, v)? as f32,
        }
        self.d.set_thresholds(th);
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let th = self.d.thresholds();
        values_of([
            (DemodSetting::Effort, Value::Choice(self.effort.index())),
            (DemodSetting::Coherent, Value::Bool(th.coherent)),
            (DemodSetting::PulseK, Value::F64(f64::from(th.pulse_k))),
            (DemodSetting::DataK, Value::F64(f64::from(th.data_k))),
            (
                DemodSetting::StrictMin,
                Value::F64(f64::from(th.strict_min)),
            ),
            (DemodSetting::StrictK, Value::F64(f64::from(th.strict_k))),
        ])
    }
}
