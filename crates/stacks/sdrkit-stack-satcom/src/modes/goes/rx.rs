//! The GOES DCS receiver: the scanner (averaged spectrum, local floor, persistent carrier lines) and, per followed
//! carrier, a slot: mixer and decimator to 9.6 kHz, a decision-directed carrier loop for the +-60 degree states, the data as the
//! quadrature part, five rate and coding hypotheses with eight bit phases each hunting the preamble and frame sync, the address
//! BCH, and the body.

use super::body::{read_body, AddressCode, Body, EOT, SYNC};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::loopfilter::{loop_gains, LoopFilter};
use sdrkit_algo_dsp::meter::power_to_db as db;
use sdrkit_algo_dsp::spectrum::{percentile, Scaling, Welch};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::{Complex32, ConfigError};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};

pub const SLOT_RATE: f64 = 9600.0;
/// The hypotheses: bit rate and Manchester.
pub const HYPS: [(f64, bool); 5] = [
    (100.0, true),
    (300.0, false),
    (300.0, true),
    (1200.0, false),
    (1200.0, true),
];
pub const PHASES: usize = 8;

/// A report decoded.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub start: f64,
    pub address: String,
    pub address_ok: bool,
    pub corrected: usize,
    pub rate: u32,
    pub manchester: bool,
    /// Offset from the input's 0 Hz.
    pub offset_hz: f64,
    pub snr_db: f64,
    pub body: Body,
    pub raw: Vec<u8>,
    pub cut_off: bool,
}

#[derive(Clone, Debug)]
struct Cand {
    errs: u32,
    hyp: usize,
    phase: usize,
    inv: bool,
    bits: Vec<bool>,
    at: u64,
}

/// The width a slot keeps around its carrier, Hz (1200 bit/s Manchester and the carrier error).
const SLOT_WIDTH_HZ: f64 = 6_000.0;
/// The worst candidate taken: twice the address corrections plus the sync errors. A random word passes the address code with up
/// to 2 corrections about half the time, so a weaker candidate is more likely noise in a preamble than a platform.
const MAX_SCORE: usize = 4;
/// The noise bandwidth of the bit clock loop, per bit.
const CLOCK_BN: f64 = 0.01;
/// The clock range the bit clock loop can pull to (5000 ppm).
const CLOCK_RANGE: f64 = 5e-3;

/// One followed carrier.
pub struct Slot {
    /// The carrier, Hz from the input's 0 Hz.
    pub offset: f64,
    /// The carrier line over the noise when it was found, dB.
    pub snr_db: f64,
    mix: Channelizer,
    tmp: Vec<Complex32>,
    theta: f64,
    freq: f64,
    alpha: f64,
    beta: f64,
    /// Prefix sums of the data samples (a window of the recent ones).
    data: VecDeque<f64>,
    base: u64,
    n: u64,
    regs: Vec<u64>,
    cands: Vec<Cand>,
    first_cand: Option<u64>,
    chosen: Option<(usize, usize, bool, usize, u64)>,
    /// The fractional slot index of the next bit end, and the bit clock loop, once a candidate is chosen.
    next_end: f64,
    clock: LoopFilter,
    last_v: f64,
    code: AddressCode,
    body_bits: Vec<bool>,
    level: f64,
    level_ref: f64,
    pub state: u8,
    pub rate: u32,
    pub done: bool,
    pub created: f64,
    pub reports: Vec<Report>,
    pub bad: u64,
    start_time: f64,
    /// When the scanner last saw the slot's line.
    pub last_present: f64,
}

/// The preamble's last 16 bits and the sync: 31 bits, newest in bit 0.
fn pattern() -> u64 {
    // The preamble alternates and ends with a 0 before the sync's leading 1.
    let mut b: Vec<bool> = (0..16).map(|i| i % 2 == 0).collect();
    b.extend_from_slice(&SYNC);
    b.iter().fold(0u64, |v, &x| (v << 1) | x as u64)
}

impl Slot {
    /// A slot for the carrier at `offset` Hz in an input at `rate_in`, found at signal time `created`. Errors when the rate is not
    /// positive.
    pub fn new(rate_in: f64, offset: f64, created: f64, snr_db: f64) -> Result<Self, ConfigError> {
        let (alpha, beta) = loop_gains(15.0 / SLOT_RATE, std::f64::consts::FRAC_1_SQRT_2, 1.0)?;
        let mut clock = LoopFilter::new(CLOCK_BN, std::f64::consts::FRAC_1_SQRT_2)?;
        clock.set_clamp(CLOCK_RANGE);
        Ok(Slot {
            offset,
            snr_db,
            mix: {
                let mut c = ChannelConfig::new(rate_in, offset, SLOT_WIDTH_HZ, SLOT_RATE);
                c.exact_rate = true;
                Channelizer::new(c)?
            },
            tmp: Vec::new(),
            theta: 0.0,
            freq: 0.0,
            alpha,
            beta,
            data: VecDeque::new(),
            base: 0,
            n: 0,
            regs: vec![0; HYPS.len() * PHASES * 2],
            cands: Vec::new(),
            first_cand: None,
            chosen: None,
            next_end: 0.0,
            clock,
            last_v: 0.0,
            code: AddressCode::new(),
            body_bits: Vec::new(),
            level: 0.0,
            level_ref: 0.0,
            state: 0,
            rate: 0,
            done: false,
            created,
            reports: Vec::new(),
            bad: 0,
            start_time: created,
            last_present: created,
        })
    }

    /// The sum of the data samples [a, b) (absolute slot indices).
    fn sum(&self, a: u64, b: u64) -> f64 {
        // data[i] holds the prefix sum up to (and with) sample base + i.
        let at = |k: u64| -> f64 {
            if k <= self.base {
                0.0
            } else {
                self.data[((k - self.base - 1) as usize).min(self.data.len() - 1)]
            }
        };
        at(b) - at(a)
    }

    fn bit_value(&self, hyp: usize, end: u64) -> f64 {
        let (r, man) = HYPS[hyp];
        let spb = (SLOT_RATE / r) as u64;
        let start = end - spb;
        if man {
            self.sum(start, start + spb / 2) - self.sum(start + spb / 2, end)
        } else {
            self.sum(start, end)
        }
    }

    pub fn process(&mut self, x: &[Complex32]) {
        if self.done {
            return;
        }
        self.tmp.clear();
        self.mix.process(x, &mut self.tmp);
        let tmp = std::mem::take(&mut self.tmp);
        let pat = pattern();
        let mask = (1u64 << 31) - 1;
        for &v in &tmp {
            self.n += 1;
            let n = self.n;
            self.theta = (self.theta + self.freq) % TAU;
            let y = v * Complex32::new(self.theta.cos() as f32, -self.theta.sin() as f32);
            let a = (y.im as f64).atan2(y.re as f64);
            // The nearest of the states -60, 0 (the plain carrier), +60 degrees.
            let s = (a / (PI / 3.0)).round().clamp(-1.0, 1.0) * PI / 3.0;
            let e = a - s;
            self.freq += self.beta * e;
            self.theta += self.alpha * e;
            let m = y.norm() as f64;
            self.level = if n == 1 {
                m
            } else {
                0.995 * self.level + 0.005 * m
            };
            let d = if m > 0.0 { y.im as f64 / m } else { 0.0 };
            let last = self.data.back().copied().unwrap_or(0.0);
            self.data.push_back(last + d);
            if self.data.len() > 4096 {
                self.data.pop_front();
                self.base += 1;
            }
            match self.chosen {
                None => {
                    for (h, &(r, _)) in HYPS.iter().enumerate() {
                        let spb = (SLOT_RATE / r) as u64;
                        for p in 0..PHASES {
                            let off = p as u64 * spb / PHASES as u64;
                            if n < spb || !(n - off).is_multiple_of(spb) {
                                continue;
                            }
                            let bv = self.bit_value(h, n) > 0.0;
                            for inv in [false, true] {
                                let i = (h * PHASES + p) * 2 + inv as usize;
                                self.regs[i] = (self.regs[i] << 1) | (bv ^ inv) as u64;
                                let errs = ((self.regs[i] & mask) ^ pat).count_ones();
                                if errs <= 2
                                    && self
                                        .first_cand
                                        .is_none_or(|f| n - f < (0.5 * SLOT_RATE) as u64)
                                    && !self
                                        .cands
                                        .iter()
                                        .any(|c| c.hyp == h && c.phase == p && c.inv == inv)
                                {
                                    self.first_cand.get_or_insert(n);
                                    self.cands.push(Cand {
                                        errs,
                                        hyp: h,
                                        phase: p,
                                        inv,
                                        bits: Vec::new(),
                                        at: n,
                                    });
                                }
                                // Candidates keep every bit after their sync (the body starts after the address).
                                for c in self.cands.iter_mut() {
                                    if c.hyp == h && c.phase == p && c.inv == inv && c.at < n {
                                        c.bits.push(bv ^ inv);
                                    }
                                }
                            }
                        }
                    }
                    // A random word lies within two bits of an address half the time, so a false candidate can pass: the
                    // decision waits until later candidates had time to show (0.4 s) and have their address too.
                    let young = self
                        .first_cand
                        .is_some_and(|f| n - f < (0.4 * SLOT_RATE) as u64);
                    if !self.cands.is_empty()
                        && !young
                        && self.cands.iter().all(|c| c.bits.len() >= 31)
                    {
                        // The candidate whose address checks with the fewest corrections takes the carrier.
                        let score = |k: usize, c: &Cand| 2 * k + c.errs as usize;
                        let mut scored: Vec<(usize, usize, usize, Vec<bool>)> = Vec::new();
                        for (i, c) in self.cands.iter().enumerate() {
                            let mut w = c.bits[..31].to_vec();
                            if let Some(k) = self.code.decode(&mut w) {
                                scored.push((score(k, c), i, k, w));
                            }
                        }
                        let best = scored
                            .iter()
                            .map(|x| x.0)
                            .min()
                            .filter(|&bs| bs <= MAX_SCORE)
                            .map(|bs| {
                                // The hypothesis of the first best candidate; of its best candidates, the polarity whose eye is widest (a
                                // Manchester signal read half a bit off and inverted also passes the checks); then of its bit phases, the
                                // middle one.
                                let h_first = scored
                                    .iter()
                                    .find(|x| x.0 == bs)
                                    .map(|x| self.cands[x.1].hyp)
                                    .expect("a best");
                                let first = scored
                                    .iter()
                                    .filter(|x| x.0 == bs && self.cands[x.1].hyp == h_first)
                                    .max_by(|a, b| {
                                        let ea = self.eye(&self.cands[a.1], n);
                                        let eb = self.eye(&self.cands[b.1], n);
                                        ea.total_cmp(&eb)
                                    })
                                    .expect("a best");
                                let (h0, inv0) = (self.cands[first.1].hyp, self.cands[first.1].inv);
                                let same: Vec<&(usize, usize, usize, Vec<bool>)> = scored
                                    .iter()
                                    .filter(|x| {
                                        x.0 == bs
                                            && self.cands[x.1].hyp == h0
                                            && self.cands[x.1].inv == inv0
                                    })
                                    .collect();
                                let dist = |a: usize, b: usize| {
                                    let d = a.abs_diff(b);
                                    d.min(PHASES - d)
                                };
                                let pick = same
                                    .iter()
                                    .min_by_key(|x| {
                                        same.iter()
                                            .map(|y| {
                                                dist(self.cands[x.1].phase, self.cands[y.1].phase)
                                            })
                                            .sum::<usize>()
                                    })
                                    .expect("one");
                                (pick.1, pick.2, pick.3.clone())
                            });
                        match best {
                            Some((i, k, w)) => {
                                let c = self.cands[i].clone();
                                self.chosen = Some((
                                    c.hyp,
                                    c.phase,
                                    c.inv,
                                    k,
                                    w.iter().fold(0u64, |v, &b| (v << 1) | b as u64),
                                ));
                                self.rate = HYPS[c.hyp].0 as u32;
                                self.state = 2;
                                self.level_ref = self.level;
                                self.start_time = self.created + c.at as f64 / SLOT_RATE;
                                self.body_bits = c.bits[31..].to_vec();
                                // The bit clock starts at the chosen phase: the next bit ends at the next multiple of the
                                // bit length after this sample.
                                let spb = (SLOT_RATE / HYPS[c.hyp].0) as u64;
                                let off = c.phase as u64 * spb / PHASES as u64;
                                self.next_end = (n + spb - (n - off) % spb) as f64;
                                self.clock.reset();
                                self.last_v = if c.bits.last().copied().unwrap_or(false) ^ c.inv {
                                    1.0
                                } else {
                                    -1.0
                                };
                                // The body can be whole already (the decision waited).
                                let ext = HYPS[c.hyp].0 > 100.0;
                                if let Some(l) =
                                    (8..=self.body_bits.len()).find(|&l| self.ends_at(l, ext))
                                {
                                    self.body_bits.truncate(l);
                                    let word = w.iter().fold(0u64, |v, &b| (v << 1) | b as u64);
                                    self.finish(c.hyp, k, word, false);
                                    break;
                                }
                            }
                            None => {
                                self.bad += 1;
                                self.cands.clear();
                                self.first_cand = None;
                            }
                        }
                    }
                }
                Some((h, _, inv, k, word)) => {
                    let spb = (SLOT_RATE / HYPS[h].0) as u64;
                    let q = (spb / 4).max(1);
                    let lost = self.level < 0.3 * self.level_ref;
                    if n as f64 >= self.next_end && !lost {
                        let b = self.next_end.round() as u64;
                        let v = self.bit_value(h, b);
                        // The timing error from a transition: Manchester has one in the middle of every bit, NRZ between two
                        // bits that differ. A window of half a bit centred on it holds as much of each side when the clock is
                        // right; a clock early by tau samples leaves 2 tau more of the earlier side in it.
                        let (edge, before) = if HYPS[h].1 {
                            let m = b - spb / 2;
                            (Some(m), self.sum(b - spb, m))
                        } else {
                            let changed = (v > 0.0) != (self.last_v > 0.0);
                            (changed.then_some(b - spb), self.last_v)
                        };
                        let amp = v.abs() / spb as f64;
                        let err = match edge {
                            Some(e) if amp > 0.0 => {
                                let mid = self.sum(e - q, e + q);
                                (before.signum() * mid / (2.0 * amp) / spb as f64)
                                    .clamp(-0.25, 0.25)
                            }
                            _ => 0.0,
                        };
                        self.last_v = v;
                        self.next_end += spb as f64 * (1.0 + self.clock.update(err));
                        let bv = (v > 0.0) ^ inv;
                        self.body_bits.push(bv);
                        if self.ends_at(self.body_bits.len(), HYPS[h].0 > 100.0) {
                            self.finish(h, k, word, false);
                            break;
                        }
                    }
                    if lost || self.body_bits.len() > 4096 * 8 {
                        self.finish(h, k, word, true);
                        break;
                    }
                }
            }
        }
        self.tmp = tmp;
        if self.chosen.is_none() && !self.cands.is_empty() {
            self.state = 1;
        }
    }

    /// The eye of a candidate: the mean size of its last bits (up to 24) at its bit phase, up to sample `n`.
    fn eye(&self, c: &Cand, n: u64) -> f64 {
        let spb = (SLOT_RATE / HYPS[c.hyp].0) as u64;
        let off = c.phase as u64 * spb / PHASES as u64;
        if n < off + spb {
            return 0.0;
        }
        let last = n - (n - off) % spb;
        let mut sum = 0.0;
        let mut k = 0;
        while k < 24 && last > (k + 1) * spb + self.base {
            sum += self.bit_value(c.hyp, last - k * spb).abs();
            k += 1;
        }
        if k == 0 {
            0.0
        } else {
            sum / (k * spb) as f64
        }
    }

    /// Whether the first `l` body bits end with an EOT on a byte boundary of either data offset, the characters before it holding
    /// their parity (a wrong offset fails it half the time).
    fn ends_at(&self, l: usize, extended: bool) -> bool {
        let ends = |o: usize| {
            if l < o + 8 || !(l - o).is_multiple_of(8) {
                return false;
            }
            let byte = |s: usize| {
                self.body_bits[s..s + 8]
                    .iter()
                    .enumerate()
                    .fold(0u8, |v, (i, &x)| v | (x as u8) << i)
            };
            if byte(l - 8) != EOT {
                return false;
            }
            let nb = (l - o) / 8 - 1;
            let errs = (0..nb)
                .filter(|j| !super::body::parity_ok(byte(o + 8 * j)))
                .count();
            errs <= (nb / 10).max(1)
        };
        ends(0) || (extended && ends(9))
    }

    fn finish(&mut self, h: usize, k: usize, word: u64, cut: bool) {
        let body = read_body(&self.body_bits, HYPS[h].0 > 100.0);
        let raw: Vec<u8> = body.chars.iter().take(4096).copied().collect();
        self.reports.push(Report {
            start: self.start_time,
            address: format!("{word:08X}"),
            address_ok: true,
            corrected: k,
            rate: HYPS[h].0 as u32,
            manchester: HYPS[h].1,
            offset_hz: self.offset + self.freq * SLOT_RATE / TAU,
            snr_db: self.snr_db,
            cut_off: cut && !body.eot,
            body,
            raw,
        });
        self.done = true;
    }
}

/// The scanner: an averaged spectrum, the local noise floor and persistent carrier lines.
pub struct Scanner {
    /// The frame length.
    pub n: usize,
    rate: f64,
    /// One Hann frame per call: a periodogram with 0 Hz in bin `n / 2`.
    welch: Welch,
    frame: Vec<Complex32>,
    pow: Vec<f32>,
    scratch: Vec<f32>,
    pub acc: Vec<f32>,
    pub frames: usize,
    skip: u64,
    count: u64,
    prev: Vec<(f64, f64)>,
    last: Vec<f64>,
}

impl Scanner {
    /// The lines of the last evaluation (confirmed or not).
    pub fn last_lines(&self) -> &[f64] {
        &self.last
    }

    pub fn new(rate: f64) -> Self {
        let n = ((rate / 25.0) as usize)
            .next_power_of_two()
            .clamp(1024, 65_536);
        let per_eval = (rate * 0.25 / n as f64).floor().max(1.0);
        Scanner {
            n,
            rate,
            welch: Welch::new(n, Window::Hann, 0.0, Scaling::Tone).expect("a valid frame size"),
            frame: Vec::with_capacity(n),
            pow: vec![0.0; n],
            scratch: Vec::with_capacity(129),
            acc: vec![0.0; n],
            frames: 0,
            skip: (per_eval / 8.0).floor().max(1.0) as u64,
            count: 0,
            prev: Vec::new(),
            last: Vec::new(),
        }
    }

    pub fn push(&mut self, x: &[Complex32], first: u64) {
        let n = self.n as u64;
        for (i, &v) in x.iter().enumerate() {
            let at = first + i as u64;
            if (at / n).is_multiple_of(self.skip) {
                self.frame.push(v);
                if self.frame.len() == self.n {
                    // The frame is exactly one Welch frame, so the call cannot fail.
                    let _ = self.welch.compute(&self.frame, &mut self.pow);
                    for (a, p) in self.acc.iter_mut().zip(self.pow.iter()) {
                        *a += *p;
                    }
                    self.frames += 1;
                    self.frame.clear();
                }
            }
        }
        self.count += x.len() as u64;
    }

    /// The lines standing `thr_db` over the local floor (30th percentile of +-64 bins), narrow (6 dB over the bins 150 to 400 Hz
    /// away), and seen in the previous evaluation too: (offset Hz, dB over the floor). Also the spectrum in dB over the floor.
    pub fn evaluate(&mut self, thr_db: f64) -> (Vec<(f64, f64)>, Vec<f32>) {
        let n = self.n;
        let bin = self.rate / n as f64;
        let p: Vec<f32> = self
            .acc
            .iter()
            .map(|v| v / self.frames.max(1) as f32)
            .collect();
        // The local floor every 16th bin, interpolated.
        let step = 16;
        let fl: Vec<f32> = (0..n.div_ceil(step))
            .map(|j| {
                let c = j * step;
                let a = c.saturating_sub(64);
                let b = (c + 64).min(n - 1);
                percentile(&p[a..=b], 0.3, &mut self.scratch)
            })
            .collect();
        let floor_at = |i: usize| {
            let j = i / step;
            let f = (i % step) as f32 / step as f32;
            let a = fl[j.min(fl.len() - 1)];
            let b = fl[(j + 1).min(fl.len() - 1)];
            (a * (1.0 - f) + b * f).max(1e-30)
        };
        let thr = 10f32.powf(thr_db as f32 / 10.0);
        let k1 = (150.0 / bin).ceil() as usize;
        let k2 = (400.0 / bin).ceil() as usize;
        let mut lines = Vec::new();
        let edge = (0.05 * n as f64) as usize;
        for i in edge.max(k2 + 1)..n - edge.max(k2 + 1) {
            let v = p[i];
            if v < p[i - 1] || v < p[i + 1] {
                continue;
            }
            let f = floor_at(i);
            if v / f < thr {
                continue;
            }
            // Narrow: data sidebands are wide, a carrier line is not.
            let side = (k1..=k2)
                .map(|k| p[i - k].max(p[i + k]))
                .fold(0.0f32, f32::max);
            if v < side * 4.0 {
                continue;
            }
            let off = (i as f64 - (n / 2) as f64) * bin;
            lines.push((off, db((v / f) as f64)));
        }
        // The sidebands of a periodic preamble are lines too: a line within 1.3 kHz of a stronger one (closer than the channel
        // spacing) is not a carrier.
        let all = lines.clone();
        lines.retain(|l| !all.iter().any(|o| o.1 > l.1 && (o.0 - l.0).abs() < 1300.0));
        let spec_db: Vec<f32> = (0..n)
            .map(|i| db((p[i] / floor_at(i)) as f64) as f32)
            .collect();
        // Persistence: seen in the previous evaluation within 3 bins.
        let confirmed: Vec<(f64, f64)> = lines
            .iter()
            .filter(|l| self.prev.iter().any(|q| (q.0 - l.0).abs() <= 3.0 * bin))
            .cloned()
            .collect();
        self.last = lines.iter().map(|l| l.0).collect();
        self.prev = lines;
        self.acc.iter_mut().for_each(|v| *v = 0.0);
        self.frames = 0;
        (confirmed, spec_db)
    }
}
