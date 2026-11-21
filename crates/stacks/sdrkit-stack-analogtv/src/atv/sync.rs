//! The video decoder's synchronisation: the carrier phase loop and the detector, the vestigial-sideband correction, the sync slicer,
//! the line loop, the field sync and the levels of each line. It passes every line to the picture decoder as a [`LineInfo`].

use super::{last, LineInfo, SyncReport, Tune};
use crate::systems::{RasterGroup, System, SystemId, R405, R525, R625, R819};
use crate::util::{DoubleMean, Ring, RingRead};
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_core::{Complex32, ConfigError};
use sdrkit_graph::{Component, ComponentInfo, Format, Io, Kind, Port, Timed};
use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};

/// The result of an acquisition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Acquired {
    /// The polarity (+1: sync tip the largest of `z`).
    pub pol: f32,
    /// Lines a frame.
    pub lines: u32,
    /// Line period, in samples of the analysed signal.
    pub period: f64,
    /// A line start (sample of the analysed signal).
    pub t: f64,
    /// The threshold used.
    pub threshold: f32,
    /// Sync tip level.
    pub tip: f32,
    /// Share of lines found.
    pub share: f64,
}

fn raster_of(lines: u32) -> crate::systems::Raster {
    match lines {
        525 => R525,
        405 => R405,
        819 => R819,
        _ => R625,
    }
}

fn percentile(x: &[f32], q: f64) -> f32 {
    let step = (x.len() / 4000).max(1);
    let mut v: Vec<f32> = x.iter().step_by(step).cloned().collect();
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f64 * q) as usize]
}

/// Acquisition by threshold on a slicer signal `z` at `rate`, trying `pols` and `groups`.
pub fn acquire(z: &[f32], rate: f64, groups: &[RasterGroup], pols: &[f32]) -> Option<Acquired> {
    let mut cands: Vec<(f64, f32, RasterGroup, f32)> = Vec::new();
    let mut shares = Vec::new();
    for &pol in pols {
        let lo = percentile(z, 0.005) * pol;
        let hi = percentile(z, 0.995) * pol;
        let (lo, hi) = (lo.min(hi), lo.max(hi));
        if hi - lo <= 1e-12 {
            continue;
        }
        for g in groups {
            let (line, _, smin, smax, _, _) = g.limits();
            let expected = z.len() as f64 / (line * 1e-6 * rate);
            shares.clear();
            for k in 0..24 {
                let th = lo + (hi - lo) * (0.2 + 0.7 * k as f32 / 23.0);
                let mut count = 0usize;
                let mut start: Option<usize> = None;
                for (i, &v) in z.iter().enumerate() {
                    let a = v * pol >= th;
                    match (a, start) {
                        (true, None) => start = Some(i),
                        (false, Some(s)) => {
                            let w = (i - s) as f64 / rate * 1e6;
                            if w >= smin && w <= smax {
                                count += 1;
                            }
                            start = None;
                        }
                        _ => {}
                    }
                }
                let share = if count as f64 > 1.15 * expected {
                    0.0
                } else {
                    count as f64 / expected
                };
                shares.push((share, th));
            }
            let top = shares.iter().map(|s| s.0).fold(0.0, f64::max);
            if top <= 0.0 {
                continue;
            }
            let good: Vec<f32> = shares
                .iter()
                .filter(|s| s.0 >= 0.9 * top)
                .map(|s| s.1)
                .collect();
            let th = good[good.len() / 2];
            if top >= 0.5 {
                cands.push((top, th, *g, pol));
            }
        }
    }
    // The best share first; if a candidate's pulses do not form a line grid, the next candidate is
    // tried.
    cands.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    cands
        .into_iter()
        .find_map(|(share, th, g, pol)| fit_grid(z, rate, share, th, g, pol))
}

/// The line grid of the pulses of line-sync width at threshold `th`: the period and the time of the last line.
fn fit_grid(
    z: &[f32],
    rate: f64,
    share: f64,
    th: f32,
    g: RasterGroup,
    pol: f32,
) -> Option<Acquired> {
    let (line, _, smin, smax, lmin, lmax) = g.limits();
    // Pulse starts of line-sync width.
    let mut starts = Vec::new();
    let mut start: Option<f64> = None;
    for i in 1..z.len() {
        let (a, b) = (z[i - 1] * pol, z[i] * pol);
        if a < th && b >= th {
            start = Some(i as f64 - 1.0 + ((th - a) / (b - a)) as f64);
        } else if a >= th && b < th {
            if let Some(s) = start.take() {
                let e = i as f64 - 1.0 + ((a - th) / (a - b)) as f64;
                let w = (e - s) / rate * 1e6;
                if w >= smin && w <= smax {
                    starts.push(s);
                }
            }
        }
    }
    if starts.len() < 20 {
        return None;
    }
    let p0 = line * 1e-6 * rate;
    let mut diffs: Vec<f64> = starts
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|d| (d / p0 - 1.0).abs() < 0.05)
        .collect();
    if diffs.len() < 10 {
        return None;
    }
    diffs.sort_by(|a, b| a.total_cmp(b));
    let mut p = diffs[diffs.len() / 2];
    let fit = |p_est: f64, starts: &[f64]| -> Option<(f64, f64, usize)> {
        let mut nums = Vec::with_capacity(starts.len());
        let mut n = 0i64;
        for (i, &t) in starts.iter().enumerate() {
            if i > 0 {
                n += ((t - starts[i - 1]) / p_est).round() as i64;
            }
            nums.push(n);
        }
        let mut keep: Vec<bool> = vec![true; starts.len()];
        let mut ab = (starts[0], p_est);
        for _ in 0..3 {
            let (mut sn, mut st, mut snn, mut snt, mut c) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for i in 0..starts.len() {
                if keep[i] {
                    let x = nums[i] as f64;
                    sn += x;
                    st += starts[i];
                    snn += x * x;
                    snt += x * starts[i];
                    c += 1.0;
                }
            }
            if c < 10.0 {
                return None;
            }
            let den = c * snn - sn * sn;
            if den.abs() < 1e-9 {
                return None;
            }
            let slope = (c * snt - sn * st) / den;
            let icpt = (st - slope * sn) / c;
            ab = (icpt, slope);
            for i in 0..starts.len() {
                keep[i] = (starts[i] - (icpt + slope * nums[i] as f64)).abs() < 0.08 * p_est;
            }
        }
        let last = (0..starts.len()).rev().find(|&i| keep[i])?;
        Some((ab.0 + ab.1 * nums[last] as f64, ab.1, last))
    };
    let (t_last, slope, _) = fit(p, &starts)?;
    p = slope;
    let us = p / rate * 1e6;
    if us < lmin || us > lmax {
        return None;
    }
    let lines = match g {
        RasterGroup::L625 => {
            if (us - 64.0).abs() < (us - 1e6 / R525.line_hz).abs() {
                625
            } else {
                525
            }
        }
        RasterGroup::L405 => 405,
        RasterGroup::L819 => 819,
    };
    let tip = percentile(z, 0.995) * pol;
    let tip = if pol < 0.0 {
        -percentile(z, 0.005)
    } else {
        tip
    };
    Some(Acquired {
        pol,
        lines,
        period: p,
        t: t_last,
        threshold: th,
        tip,
        share,
    })
}

/// Acquisition by folding for weak signals: the stretch folded modulo line periods near the nominal ones, a matched
/// filter (sync window against the back porch) at every phase. Returns (pol, lines, period, phase, score).
pub fn fold(z: &[f32], rate: f64, groups: &[RasterGroup], pols: &[f32]) -> Option<Acquired> {
    let mut best: Option<(f64, Acquired)> = None;
    let nominal: Vec<(u32, f64, f64)> = groups
        .iter()
        .flat_map(|g| match g {
            RasterGroup::L625 => vec![(625u32, 64.0, 4.7), (525, 1e6 / R525.line_hz, 4.7)],
            RasterGroup::L405 => vec![(405, 1e6 / 10_125.0, 9.0)],
            RasterGroup::L819 => vec![(819, 1e6 / 20_475.0, 2.5)],
        })
        .collect();
    for &(lines, line_us, sync_us) in &nominal {
        let p0 = line_us * 1e-6 * rate;
        let sync = sync_us * 1e-6 * rate;
        let bp = (3.0e-6 * rate).max(1.0);
        let periods = z.len() as f64 / p0;
        let dp = (0.3 * sync / periods).max(p0 * 2e-6);
        let steps = ((p0 * 400e-6) / dp).ceil() as i64;
        let nb = p0.round() as usize;
        let mut bins = vec![0f64; nb];
        let mut cnt = vec![0u32; nb];
        for s in -steps..=steps {
            let p = p0 + s as f64 * dp;
            bins.iter_mut().for_each(|b| *b = 0.0);
            cnt.iter_mut().for_each(|b| *b = 0);
            for (i, &v) in z.iter().enumerate() {
                let ph = (i as f64 % p) / p * nb as f64;
                let b = (ph as usize).min(nb - 1);
                bins[b] += v as f64;
                cnt[b] += 1;
            }
            for b in 0..nb {
                if cnt[b] > 0 {
                    bins[b] /= cnt[b] as f64;
                }
            }
            for &pol in pols {
                let ws = (sync / p * nb as f64).round().max(1.0) as usize;
                let wb = (bp / p * nb as f64).round().max(1.0) as usize;
                let mut mf = vec![0f64; nb];
                for (b, m) in mf.iter_mut().enumerate() {
                    let mut a = 0.0;
                    for j in 0..ws {
                        a += bins[(b + j) % nb];
                    }
                    let mut c = 0.0;
                    for j in 0..wb {
                        c += bins[(b + ws + j) % nb];
                    }
                    *m = pol as f64 * (a / ws as f64 - c / wb as f64);
                }
                let mean = mf.iter().sum::<f64>() / nb as f64;
                let sd = (mf.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / nb as f64)
                    .sqrt()
                    .max(1e-12);
                let (bi, &mx) = mf
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .expect("a line period has at least one bin");
                let score = (mx - mean) / sd;
                // The sync tip must be the highest part of the folded line.
                let tipv = (0..ws)
                    .map(|j| pol as f64 * bins[(bi + j) % nb])
                    .sum::<f64>()
                    / ws as f64;
                let top = bins
                    .iter()
                    .map(|&v| pol as f64 * v)
                    .fold(f64::MIN, f64::max);
                let low = bins
                    .iter()
                    .map(|&v| pol as f64 * v)
                    .fold(f64::MAX, f64::min);
                if tipv < low + 0.6 * (top - low) {
                    continue;
                }
                if score > 4.0 && best.as_ref().is_none_or(|b| score > b.0) {
                    let phase = bi as f64 / nb as f64 * p;
                    let k = ((z.len() as f64 - phase) / p).floor() - 1.0;
                    best = Some((
                        score,
                        Acquired {
                            pol,
                            lines,
                            period: p,
                            t: phase + k * p,
                            threshold: 0.0,
                            tip: (tipv * pol as f64) as f32,
                            share: 0.0,
                        },
                    ));
                }
            }
        }
    }
    best.map(|b| b.1)
}

/// The video decoder's synchronisation.
pub struct VideoSync {
    fv: f64,
    /// FM video: no carrier loop, the input is the analytic video.
    pub fm: bool,
    tune: Tune,
    epoch: u32,
    // Carrier loop.
    ph: Complex32,
    step: Complex32,
    omega: f64,
    acc: Complex32,
    acc_n: usize,
    block: usize,
    perr2: f64,
    good_blocks: u32,
    phase_locked: bool,
    amp_ref: f64,
    synchronous: bool,
    level_restart: u32,
    // VSB correction.
    vsb: Option<Fir<f32>>,
    vsb_key: (u64, u64),
    vsb_delay: usize,
    dc_gain: f64,
    vsb_ng: f64,
    base_delay: Ring<Complex32>,
    y_ring: Ring<Complex32>,
    line_phase: bool,
    // Slicer and rings.
    pol: f32,
    slicer: DoubleMean,
    s_ring: Ring<f32>,
    n: u64,
    prev_z: f32,
    acq: Vec<f32>,
    acq_sum: f32,
    acq_k: u32,
    acq_start: u64,
    looks: u32,
    // Loop.
    running: bool,
    lines: u32,
    period: f64,
    period_avg: f64,
    next0h: f64,
    kp: f64,
    ki: f64,
    loop_lines: u64,
    unexpected: u32,
    /// In matched-filter mode: lines in a row whose threshold crossing came inside the gate (a clean signal after all).
    clean_hits: u32,
    hit: bool,
    mf_cum: Vec<f64>,
    mf_vals: Vec<f64>,
    counter: u64,
    pending: VecDeque<(u64, f64, bool)>,
    tip: f64,
    blank: f64,
    noise_var: f64,
    levels_n: u32,
    quality: f64,
    low_quality: u32,
    noisy: bool,
    // Field sync.
    slots: VecDeque<(f64, bool, u64)>,
    run: Vec<(f64, bool, u64)>,
    numbering: Option<(u64, u32)>,
    last_detect: Option<(u64, u8)>,
    last_field_counter: u64,
    field_misses: u32,
    last_field: u8,
    fields: u64,
    lost_samples: u64,
    weak: bool,
    report_tick: bool,
    /// The samples of the two outputs of one pass.
    out_v: Vec<f32>,
    out_b: Vec<Complex32>,
}

impl Default for VideoSync {
    fn default() -> Self {
        VideoSync {
            fv: 1.0,
            fm: false,
            tune: Tune::default(),
            epoch: u32::MAX,
            ph: Complex32::new(1.0, 0.0),
            step: Complex32::new(1.0, 0.0),
            omega: 0.0,
            acc: Complex32::default(),
            acc_n: 0,
            block: 640,
            perr2: 1.0,
            good_blocks: 0,
            phase_locked: false,
            amp_ref: 0.0,
            synchronous: false,
            level_restart: 0,
            vsb: None,
            vsb_key: (0, 0),
            vsb_delay: 0,
            dc_gain: 1.0,
            vsb_ng: 1.0,
            base_delay: crate::util::ring(256),
            y_ring: crate::util::ring(4096),
            line_phase: false,
            pol: 1.0,
            slicer: DoubleMean::new(10),
            s_ring: crate::util::ring(4096),
            n: 0,
            prev_z: 0.0,
            acq: Vec::new(),
            acq_sum: 0.0,
            acq_k: 0,
            acq_start: 0,
            looks: 0,
            running: false,
            lines: 0,
            period: 640.0,
            period_avg: 640.0,
            next0h: 0.0,
            kp: 0.5,
            ki: 0.06,
            loop_lines: 0,
            unexpected: 0,
            clean_hits: 0,
            hit: false,
            mf_cum: Vec::new(),
            mf_vals: Vec::new(),
            counter: 0,
            pending: VecDeque::with_capacity(16),
            tip: 1.0,
            blank: 0.0,
            noise_var: 0.0,
            levels_n: 0,
            quality: 0.0,
            low_quality: 0,
            noisy: false,
            slots: VecDeque::with_capacity(16),
            run: Vec::with_capacity(16),
            numbering: None,
            last_detect: None,
            last_field_counter: 0,
            field_misses: 0,
            last_field: 0,
            fields: 0,
            lost_samples: 0,
            weak: false,
            report_tick: false,
            out_v: Vec::new(),
            out_b: Vec::new(),
        }
    }
}

impl VideoSync {
    fn us(&self, x: f64) -> f64 {
        x * 1e-6 * self.fv
    }

    fn restart(&mut self) {
        self.line_phase = false;
        let w = self.us(if self.weak { 2.4 } else { 1.0 }).round().max(1.0) as usize;
        self.slicer = DoubleMean::new(w);
        self.running = false;
        self.acq.clear();
        self.acq_sum = 0.0;
        self.acq_k = 0;
        self.acq_start = self.n;
        self.pending.clear();
        self.slots.clear();
        self.run.clear();
        self.numbering = None;
        self.last_detect = None;
        self.quality = 0.0;
        self.levels_n = 0;
        self.counter = 0;
        self.loop_lines = 0;
        self.noisy = false;
        self.field_misses = 0;
        self.fields = 0;
        self.last_field = 0;
        self.unexpected = 0;
    }

    fn reset_all(&mut self) {
        self.ph = Complex32::new(1.0, 0.0);
        self.omega = 0.0;
        self.step = Complex32::new(1.0, 0.0);
        self.acc = Complex32::default();
        self.acc_n = 0;
        self.perr2 = 1.0;
        self.good_blocks = 0;
        self.phase_locked = false;
        self.synchronous = false;
        self.amp_ref = 0.0;
        self.looks = 0;
        self.lost_samples = 0;
        self.restart();
    }

    fn groups(&self) -> Vec<RasterGroup> {
        match (self.tune.forced, self.tune.system) {
            (true, Some(s)) => vec![System::get(s).raster.group()],
            _ => RasterGroup::ALL.to_vec(),
        }
    }

    fn pols(&self) -> Vec<f32> {
        if self.fm {
            return vec![self.pol];
        }
        match (self.tune.forced, self.tune.system) {
            (true, Some(s)) => vec![if System::get(s).positive { -1.0 } else { 1.0 }],
            _ => {
                if self.pol > 0.0 {
                    vec![1.0, -1.0]
                } else {
                    vec![-1.0, 1.0]
                }
            }
        }
    }

    fn design_vsb(&mut self) {
        if self.fm {
            self.vsb = None;
            self.dc_gain = 1.0;
            self.vsb_ng = 1.0;
            return;
        }
        let a = ((self.tune.dsb_db + 8.0) / 8.0).clamp(0.0, 1.0);
        let vest = match self.tune.system {
            Some(s) if self.tune.forced || self.tune.spacing_hz == 0.0 => {
                System::get(s).vestige_mhz * 1e6
            }
            _ => {
                let sp = self.tune.spacing_hz.abs();
                if (sp - 6.0e6).abs() < 0.2e6 {
                    1.25e6
                } else if sp > 8e6 {
                    2.0e6
                } else {
                    0.75e6
                }
            }
        };
        let key = (a.to_bits(), vest.to_bits());
        if key == self.vsb_key && self.vsb.is_some() {
            return;
        }
        self.vsb_key = key;
        if a <= 0.0 {
            self.vsb = None;
            self.vsb_delay = 0;
            self.dc_gain = 1.0;
            self.vsb_ng = 1.0;
            return;
        }
        let n = (self.us(4.0).round() as usize) | 1;
        let taps =
            sdrkit_algo_dsp::fir::design_by_response(n, self.fv, crate::util::DESIGN_BETA, |f| {
                let s = if f <= vest {
                    1.0
                } else if f < vest + 0.5e6 {
                    (PI / 2.0 * (f - vest) / 0.5e6).cos().powi(2)
                } else {
                    0.0
                };
                1.0 / (1.0 + a * s)
            });
        let Ok(taps) = taps else {
            self.vsb = None;
            self.vsb_delay = 0;
            self.dc_gain = 1.0;
            self.vsb_ng = 1.0;
            return;
        };
        self.vsb_delay = n / 2;
        self.dc_gain = sdrkit_algo_dsp::fir::response(&taps, 0.0).norm();
        self.vsb_ng = taps.iter().map(|&t| (t as f64) * (t as f64)).sum();
        self.vsb = Some(Fir::new(&taps));
    }

    fn carrier_block(&mut self) {
        let m = self.acc / self.acc_n as f32;
        self.acc = Complex32::default();
        self.acc_n = 0;
        let amp = m.norm() as f64;
        if amp <= 0.0 {
            return;
        }
        self.amp_ref = if self.amp_ref == 0.0 {
            amp
        } else {
            self.amp_ref + 0.02 * (amp - self.amp_ref)
        };
        let trust = (amp / self.amp_ref.max(1e-12)).min(1.0);
        let err = (m.im as f64).atan2(m.re as f64);
        self.ph *= Complex32::from_polar(1.0, (-0.15 * trust * err) as f32);
        self.ph /= self.ph.norm();
        let k = 0.0056 * 15_625.0 / self.fv;
        self.omega += k * trust * err;
        let lim = TAU * 30e3 / self.fv;
        self.omega = self.omega.clamp(-lim, lim);
        self.step = Complex32::from_polar(1.0, -self.omega as f32);
        self.perr2 += 0.03 * (err * err - self.perr2);
        if self.perr2 < 0.12 {
            self.good_blocks += 1;
        } else {
            self.good_blocks = 0;
        }
        self.phase_locked = self.good_blocks > 60;
        let want = self.phase_locked && self.running && !self.tune.envelope_only;
        if want != self.synchronous {
            self.synchronous = want;
            self.level_restart = 3;
        }
    }

    fn start_loop(&mut self, a: Acquired, decim: f64, offset: f64) {
        self.pol = a.pol;
        self.lines = a.lines;
        let p = a.period * decim;
        self.period = p;
        self.period_avg = p;
        // The analysed signal's index i is input sample offset + i * decim (+ half the mean), less the slicer delay.
        let t = offset + a.t * decim + (decim - 1.0) / 2.0 - self.slicer.delay();
        let now = self.n as f64 - self.slicer.delay();
        let k = ((now - t) / p).floor() + 1.0;
        self.next0h = t + k * p;
        self.running = true;
        self.kp = 0.5;
        self.ki = 0.06;
        self.loop_lines = 0;
        self.quality = 0.6;
        self.unexpected = 0;
        self.low_quality = 0;
        self.levels_n = 0;
        if a.threshold != 0.0 {
            self.tip = a.tip as f64;
            self.blank = 2.0 * a.threshold as f64 - a.tip as f64;
        } else {
            self.noisy = true;
            self.tip = a.tip as f64;
            self.blank = a.tip as f64 * 0.5;
        }
    }

    fn try_acquire(&mut self) {
        let rate4 = self.fv / 4.0;
        let groups = self.groups();
        let pols = self.pols();
        self.looks += 1;
        let mut got = acquire(&self.acq, rate4, &groups, &pols);
        if got.is_none() && !self.fm {
            let old_every = if self.tune.forced { 4 } else { 16 };
            if self.looks.is_multiple_of(4) {
                let gs: Vec<RasterGroup> = groups
                    .iter()
                    .cloned()
                    .filter(|g| *g == RasterGroup::L625 || self.looks.is_multiple_of(old_every))
                    .collect();
                if !gs.is_empty() {
                    got = fold(&self.acq, rate4, &gs, &pols);
                }
            }
        }
        if let Some(a) = got {
            let offset = self.acq_start as f64;
            self.start_loop(a, 4.0, offset);
        }
        self.acq.clear();
        self.acq_start = self.n;
    }

    fn emit_line(&mut self, t0: f64, found: bool) {
        self.counter += 1;
        self.loop_lines += 1;
        if self.pending.len() >= 12 {
            self.pending.pop_front();
        }
        self.pending.push_back((self.counter, t0, found));
        self.quality += if found {
            (1.0 - self.quality) / 32.0
        } else {
            -self.quality / 32.0
        };
        if self.slots.len() >= 12 {
            self.slots.pop_front();
        }
        self.slots.push_back((t0, false, self.counter));
        self.slots
            .push_back((t0 + self.period / 2.0, true, self.counter));
        if self.loop_lines == 40 {
            self.kp = 0.1;
            self.ki = 0.0025;
        }
        if self.noisy {
            self.kp = 0.03;
            self.ki = 0.0002;
        }
        self.period_avg += 0.01 * (self.period - self.period_avg);
        if self.quality < 0.25 && self.loop_lines > 100 {
            self.low_quality += 1;
            if self.low_quality > 200 {
                self.restart();
            }
        } else {
            self.low_quality = 0;
        }
    }

    fn accept(&mut self, t: f64) {
        let err = t - self.next0h;
        let t0 = self.next0h + self.kp * err;
        self.period += self.ki * err;
        let (_, _, _, _, lmin, lmax) = raster_of(self.lines).group().limits();
        self.period = self.period.clamp(self.us(lmin), self.us(lmax));
        self.next0h = t0 + self.period;
        self.unexpected = 0;
        self.emit_line(t0, true);
    }

    fn miss(&mut self) {
        let t0 = self.next0h;
        self.next0h += self.period;
        self.emit_line(t0, false);
    }

    /// The matched-filter position of the sync near `t` (noisy mode), searched ±gate.
    fn matched(&mut self, t: f64, gate: f64) -> Option<f64> {
        let mut cum = std::mem::take(&mut self.mf_cum);
        let mut vals = std::mem::take(&mut self.mf_vals);
        let r = self.matched_in(t, gate, &mut cum, &mut vals);
        self.mf_cum = cum;
        self.mf_vals = vals;
        r
    }

    fn matched_in(
        &self,
        t: f64,
        gate: f64,
        cum: &mut Vec<f64>,
        vals: &mut Vec<f64>,
    ) -> Option<f64> {
        let r = raster_of(self.lines);
        let sync = self.us(r.sync_us);
        let fp = self.us(1.0).max(1.0);
        let bp = self.us(2.5).max(1.0);
        let a = (t - gate - fp).floor() as i64;
        let b = (t + gate + sync + bp).ceil() as i64 + 1;
        if !self.s_ring.has(a) || !self.s_ring.has(b - 1) {
            return None;
        }
        cum.clear();
        let mut s = 0.0f64;
        cum.push(0.0);
        for i in a..b {
            s += self.s_ring.at(i) as f64;
            cum.push(s);
        }
        let sum = |x: f64, y: f64| {
            let xi = ((x - a as f64).round() as i64).clamp(0, cum.len() as i64 - 1) as usize;
            let yi = ((y - a as f64).round() as i64).clamp(0, cum.len() as i64 - 1) as usize;
            (cum[yi] - cum[xi]) / ((yi - xi).max(1)) as f64
        };
        let mut best = (f64::MIN, t);
        vals.clear();
        let steps = (2.0 * gate).ceil() as i64;
        for k in 0..=steps {
            let p = t - gate + k as f64;
            let m = sum(p, p + sync) - 0.5 * (sum(p - fp, p) + sum(p + sync, p + sync + bp));
            vals.push(m);
            if m > best.0 {
                best = (m, p);
            }
        }
        let height = self.tip - self.blank;
        if best.0 < 0.35 * height {
            return None;
        }
        let i = ((best.1 - (t - gate)).round() as usize).min(vals.len() - 1);
        if i > 0 && i + 1 < vals.len() {
            let (y0, y1, y2) = (vals[i - 1], vals[i], vals[i + 1]);
            let den = y0 - 2.0 * y1 + y2;
            if den.abs() > 1e-12 {
                return Some(best.1 + (0.5 * (y0 - y2) / den).clamp(-0.5, 0.5));
            }
        }
        Some(best.1)
    }

    fn measure_line(&mut self, counter: u64, t0: f64, found: bool, io: &mut Io) {
        let r = raster_of(self.lines);
        let at = |x: f64| (t0 + self.us(x)).round() as i64;
        let (tip_a, tip_b) = if r.is_old() {
            (0.3 * r.sync_us, 0.8 * r.sync_us)
        } else {
            (1.4, r.sync_us - 0.7)
        };
        let (bp_a, bp_b) = match r.lines {
            625 => (8.3, 10.0),
            525 => (8.1, 9.1),
            _ => (
                r.sync_us + 0.45 * (r.blank_end_us - r.sync_us),
                r.blank_end_us - 0.4,
            ),
        };
        let tip = self.s_ring.mean_of(at(tip_a), at(tip_b)) as f64;
        let (bpm, bpv) = crate::util::mean_var(&self.s_ring, at(bp_a), at(bp_b));
        let (mut blank, mut var) = (bpm as f64, bpv as f64);
        if r.front_porch_us >= 1.2 {
            let (fpm, fpv) = crate::util::mean_var(&self.s_ring, at(-1.15), at(-0.35));
            let (lb, lf) = (bp_b - bp_a, 0.8);
            blank = (blank * lb + fpm as f64 * lf) / (lb + lf);
            var = (var * lb + fpv as f64 * lf) / (lb + lf);
        }
        if self.pol > 0.0 && !self.fm {
            // With negative modulation the sync tip is steady carrier too. A SECAM signal has its subcarrier on the back porch, so
            // the tip serves when the porches carry far more than noise.
            let (_, tv) = crate::util::mean_var(&self.s_ring, at(tip_a), at(tip_b));
            if var > 2.0 * tv as f64 {
                var = tv as f64;
            }
        }
        if !self.fm && found {
            // The carrier phase where the carrier is steady: the sync tip (negative modulation) or the back porch (positive).
            let m = if self.pol > 0.0 {
                self.y_ring.mean_of(at(tip_a), at(tip_b))
            } else {
                self.y_ring.mean_of(at(bp_a), at(bp_b))
            };
            if m.norm() > 0.0 {
                let err = (m.im as f64).atan2(m.re as f64);
                self.line_phase = true;
                self.ph *= Complex32::from_polar(1.0, (-0.15 * err) as f32);
                self.ph /= self.ph.norm();
                let k = 0.0056 * 15_625.0 / self.fv;
                self.omega += k * err;
                let lim = TAU * 30e3 / self.fv;
                self.omega = self.omega.clamp(-lim, lim);
                self.step = Complex32::from_polar(1.0, -self.omega as f32);
                self.perr2 += 0.03 * (err * err - self.perr2);
                if self.perr2 < 0.12 {
                    self.good_blocks += 1;
                } else {
                    self.good_blocks = 0;
                }
                self.phase_locked = self.good_blocks > 60;
                let want = self.phase_locked && self.running && !self.tune.envelope_only;
                if want != self.synchronous {
                    self.synchronous = want;
                    self.level_restart = 3;
                }
            }
        }
        let mid = 0.5 * (self.tip + self.blank);
        let looks_like_sync = tip - blank > 0.4 * (self.tip - self.blank) && blank < mid;
        let number = self.number_of(counter);
        let vbi = matches!(number, 1..=6 | 310..=319 | 622..=625) && self.lines == 625
            || matches!(number, 1..=9 | 263..=272) && self.lines == 525;
        if self.level_restart > 0 {
            self.level_restart -= 1;
            self.levels_n = 0;
        }
        // A line whose porch is at the sync level (a broad pulse of a raster whose field blanking is not excluded by number, or
        // a line before the numbers are known) never sets the levels, even right after they restarted.
        let known = (self.tip - self.blank).abs() > 1e-12;
        if (found || (self.fm && looks_like_sync))
            && !vbi
            && (looks_like_sync || (self.levels_n < 3 && !known))
        {
            let w = if self.levels_n < 20 { 0.3 } else { 0.1 };
            if self.levels_n == 0 {
                self.tip = tip;
                self.blank = blank;
                self.noise_var = var;
            } else {
                self.tip += w * (tip - self.tip);
                self.blank += w * (blank - self.blank);
                self.noise_var += w * (var - self.noise_var);
            }
            self.levels_n += 1;
        }
        let gain = (self.tip - self.blank) / -r.sync_level;
        let info = LineInfo {
            counter,
            number,
            t0,
            period: self.period,
            blank: self.blank as f32,
            gain: gain as f32,
            pol: self.pol,
            lines: self.lines,
            found,
            odd: counter % 2 == 1,
        };
        let t = t0 / self.fv;
        io.items_out::<LineInfo>(2).push(Timed {
            time: t,
            value: info,
        });
    }

    fn number_of(&self, counter: u64) -> u32 {
        match self.numbering {
            Some((c0, n0)) if self.lines > 0 => {
                let l = self.lines as i64;
                (((counter as i64 - c0 as i64) + n0 as i64 - 1).rem_euclid(l) + 1) as u32
            }
            _ => 0,
        }
    }

    fn check_slot(&mut self, ts: f64, half: bool, counter: u64) {
        let r = raster_of(self.lines);
        let a = (ts + self.us(0.2 * r.broad_us)).round() as i64;
        let b = (ts + self.us(0.8 * r.broad_us)).round() as i64;
        let m = self.s_ring.mean_of(a, b) as f64;
        let broad = m > 0.5 * (self.tip + self.blank);
        let half_line = self.period / 2.0;
        if broad {
            if let Some(&(lt, _, _)) = self.run.last() {
                let d = (ts - lt) / half_line;
                if (d - 1.0).abs() < 0.3 || (d - 2.0).abs() < 0.3 {
                    self.run.push((ts, half, counter));
                } else {
                    self.run.clear();
                    self.run.push((ts, half, counter));
                }
            } else {
                self.run.push((ts, half, counter));
            }
        } else if let Some(&(last_t, _, _)) = self.run.last() {
            let need = r.broad_count as usize - 1;
            let span = ((last_t - self.run[0].0) / half_line).round() as usize + 1;
            if self.run.len() >= need
                && span >= r.broad_count as usize
                && span <= r.broad_count as usize + 1
            {
                let (_, h, c) = self.run[0];
                self.field_sync(h, c);
            }
            self.run.clear();
        }
    }

    fn field_sync(&mut self, half: bool, counter: u64) {
        let r = raster_of(self.lines);
        let n0 = if half {
            r.first_broad[1] / 2 + 1
        } else {
            r.first_broad[0] / 2 + 1
        };
        let field = if half { 2u8 } else { 1 };
        let hf = self.lines as f64 / 2.0;
        let consistent = |from: u64, f: u8| {
            let d = counter.saturating_sub(from) as f64;
            (1..=3).any(|k| (d - k as f64 * hf).abs() <= 3.0) && field != f
        };
        let ok = match self.numbering {
            None => true,
            Some(_) => {
                consistent(self.last_field_counter, self.last_field)
                    || self.field_misses > 6
                    || self.last_detect.is_some_and(|(c, f)| consistent(c, f))
            }
        };
        self.last_detect = Some((counter, field));
        if ok {
            self.numbering = Some((counter, n0));
            self.last_field_counter = counter;
            self.last_field = field;
            self.field_misses = 0;
            self.fields += 1;
        }
    }

    fn line_step(&mut self, z: f32) {
        let sd = self.slicer.delay();
        let th = (0.5 * (self.tip + self.blank)) as f32;
        let now = self.n as f64 - sd;
        let gate = self.us(if self.loop_lines < 40 { 4.0 } else { 2.5 });
        if !self.noisy {
            if self.prev_z < th && z >= th {
                let t = now - 1.0 + ((th - self.prev_z) / (z - self.prev_z)) as f64;
                let d = t - self.next0h;
                if d.abs() < gate {
                    self.accept(t);
                } else {
                    let dh = (d + self.period / 2.0).abs();
                    if dh > gate && d.abs() > gate {
                        self.unexpected += 1;
                        if self.unexpected > 12 {
                            self.restart();
                            return;
                        }
                    }
                }
            }
            if now > self.next0h + gate {
                self.miss();
            }
        } else {
            // The threshold slicer keeps watching: a signal whose every sync crosses it inside the gate is clean.
            if self.prev_z < th && z >= th && (now - 1.0 - self.next0h).abs() < gate {
                self.hit = true;
            }
            let r = raster_of(self.lines);
            let need = self.next0h + gate + self.us(r.sync_us + 3.0);
            if (self.n as f64) > need {
                self.clean_hits = if self.hit { self.clean_hits + 1 } else { 0 };
                self.hit = false;
                match self.matched(self.next0h, gate) {
                    Some(t) => self.accept(t),
                    None => self.miss(),
                }
                if self.clean_hits >= 32 {
                    self.noisy = false;
                    self.clean_hits = 0;
                    self.kp = 0.1;
                    self.ki = 0.0025;
                    self.levels_n = 0;
                }
            }
        }
    }
}

impl Component for VideoSync {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.sync",
            vec![Port::iq("in"), Port::typed::<Tune>("control")],
            vec![
                Port::real("video"),
                Port::iq("base"),
                Port::typed::<LineInfo>("lines"),
                Port::typed::<SyncReport>("report"),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fv = inputs[0].rate;
        self.block = (self.fv / 15_625.0).round().max(1.0) as usize;
        self.s_ring = crate::util::ring((self.us(200.0)) as usize + 64);
        self.y_ring = crate::util::ring((self.us(200.0)) as usize + 64);
        self.base_delay = crate::util::ring(self.us(10.0) as usize + 64);
        self.n = 0;
        self.epoch = u32::MAX;
        self.tune = Tune::default();
        self.vsb = None;
        self.vsb_key = (0, 0);
        self.pol = 1.0;
        self.reset_all();
        self.design_vsb();
        let mut v = inputs[0].with_kind(Kind::Real);
        v.bandwidth_hz = self.fv / 2.0;
        Ok(vec![
            v,
            inputs[0].clone(),
            Format::new(Kind::typed::<LineInfo>(), 0.0),
            Format::new(Kind::typed::<SyncReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(t) = last::<Tune>(io, 1) {
            let new_epoch = t.epoch != self.epoch;
            let weak_changed = t.weak != self.weak;
            let pol_changed = self.fm && t.inverted != (self.pol < 0.0);
            let forced_changed =
                t.forced != self.tune.forced || (t.forced && t.system != self.tune.system);
            self.tune = t;
            self.epoch = t.epoch;
            self.weak = t.weak;
            if self.fm {
                self.pol = if t.inverted { -1.0 } else { 1.0 };
            }
            if new_epoch {
                self.reset_all();
            } else if weak_changed || pol_changed || forced_changed {
                self.restart();
            }
            self.design_vsb();
        }
        let x = io.iq(0);
        let n_in = x.len();
        let acq_len = (0.05 * self.fv / 4.0) as usize;
        // The two sample outputs are gathered here and written after the loop.
        let (mut out_v, mut out_b) = (
            std::mem::take(&mut self.out_v),
            std::mem::take(&mut self.out_b),
        );
        out_v.clear();
        out_b.clear();
        for i in 0..n_in {
            let y = if self.fm {
                x[i]
            } else {
                let y = x[i] * self.ph;
                self.ph *= self.step;
                self.acc += y;
                self.acc_n += 1;
                if self.acc_n >= self.block {
                    if self.line_phase && self.running {
                        self.acc = Complex32::default();
                        self.acc_n = 0;
                    } else {
                        self.carrier_block();
                    }
                }
                y
            };
            let raw = if self.fm || self.synchronous {
                y.re
            } else {
                y.norm()
            };
            let (v, b) = match &mut self.vsb {
                Some(f) => {
                    self.base_delay.push(y);
                    let d = self
                        .base_delay
                        .at(self.base_delay.end() as i64 - 1 - self.vsb_delay as i64);
                    (f.process_sample(raw), d)
                }
                None => (raw, y),
            };
            out_v.push(v);
            out_b.push(b);
            self.y_ring.push(b);
            let s = self.pol * v;
            self.s_ring.push(s);
            self.n += 1;
            let z = self.slicer.push(s);
            if !self.running {
                self.acq_sum += z;
                self.acq_k += 1;
                if self.acq_k == 4 {
                    self.acq.push(self.acq_sum / 4.0 * self.pol);
                    self.acq_sum = 0.0;
                    self.acq_k = 0;
                    if self.acq.len() >= acq_len {
                        // The stored copy is in detected units (polarity-free); acquisition judges the polarities on it.
                        self.try_acquire();
                    }
                }
                self.lost_samples += 1;
            } else {
                self.line_step(z);
                if self.quality > 0.5 {
                    self.lost_samples = 0;
                } else {
                    self.lost_samples += 1;
                }
            }
            self.prev_z = z;
            // Lines whose levels can be measured.
            while let Some(&(c, t0, found)) = self.pending.front() {
                let r = raster_of(self.lines);
                let end = t0 + self.us(r.blank_end_us + 0.5);
                if (self.n as f64) < end {
                    break;
                }
                self.pending.pop_front();
                self.measure_line(c, t0, found, io);
            }
            while let Some(&(ts, half, c)) = self.slots.front() {
                let r = raster_of(self.lines);
                if (self.n as f64) < ts + self.us(0.8 * r.broad_us) + 1.0 {
                    break;
                }
                self.slots.pop_front();
                self.check_slot(ts, half, c);
            }
        }
        io.real_out(0).extend_from_slice(&out_v);
        io.iq_out(1).extend_from_slice(&out_b);
        (self.out_v, self.out_b) = (out_v, out_b);
        // Field syncs that do not come.
        if self.running && self.numbering.is_some() {
            let hf = self.lines as u64 / 2;
            if self.counter > self.last_field_counter + hf + hf / 2 {
                self.field_misses += 1;
                self.last_field_counter += hf;
                self.last_field = if self.last_field == 1 { 2 } else { 1 };
                if self.field_misses > 12 {
                    self.numbering = None;
                }
                let height = self.tip - self.blank;
                if self.field_misses >= 2 && height < 1.6 * self.noise_var.sqrt() {
                    self.noisy = true;
                }
            }
        }
        if self.noisy && self.levels_n > 20 && (self.tip - self.blank) > 2.5 * self.noise_var.sqrt()
        {
            self.noisy = false;
        }
        if io.tick() {
            let state = if !self.running {
                0
            } else if self.quality > 0.5 && self.numbering.is_some() && self.field_misses < 6 {
                2
            } else if self.quality > 0.5 {
                1
            } else {
                0
            };
            let rep = SyncReport {
                state,
                lines: if self.running { self.lines } else { 0 },
                quality: if self.running { self.quality } else { 0.0 },
                line_hz: self.fv / self.period_avg,
                line_count: self.counter,
                fields: self.fields,
                last_field: self.last_field,
                positive: !self.fm && self.pol < 0.0,
                tip: (self.pol as f64) * self.tip,
                blank: (self.pol as f64) * self.blank,
                gain: (self.tip - self.blank) / -raster_of(self.lines).sync_level,
                noise_var: self.noise_var,
                synchronous: self.synchronous,
                phase_locked: self.phase_locked,
                trim_hz: self.omega * self.fv / TAU,
                noisy: self.noisy,
                field_misses: self.field_misses,
                lost_s: self.lost_samples as f64 / self.fv,
                dc_gain: self.dc_gain,
                vsb_noise_gain: self.vsb_ng,
                rate_hz: self.fv,
            };
            let t = io.input_format(0).time_of(io.input(0).first());
            io.items_out::<SyncReport>(3).push(Timed {
                time: t,
                value: rep,
            });
        }
        self.report_tick = false;
    }
}

impl VideoSync {
    /// The system E plan is not chosen here; kept for symmetry with the vision channel.
    pub fn forced_system(&self) -> Option<SystemId> {
        self.tune.system.filter(|_| self.tune.forced)
    }
}
