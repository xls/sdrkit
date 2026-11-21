//! The TD-SCDMA cell search of one capture, steps 1 to 3 of the TS 25.224 Annex CA cell search: the SYNC-DL code by an FFT
//! correlation folded over the sub-frames, the cell parameter from the midamble of time slot 0, and the frame parity from the
//! phase quadruples of the SYNC-DL. Step 4 (the broadcast channel) is not done.

use super::codes::{
    basic_midamble, sync_dl, DWPTS_START, MIDAMBLE_CHIPS, MIDAMBLE_START, S1, S2,
    SUBFRAME_CHIPS, SYNC_DL_CHIPS, SYNC_DL_START,
};
use super::gen::{CHIP_RATE, NATIVE_RATE, ROLL_OFF};
use crate::decoder::Capture;
use crate::dsp::{resample_to, ResamplerCache};
use sdrkit_algo_dsp::fir::Convolver;
use sdrkit_algo_dsp::pulse::{self, Norm, Shape};
use sdrkit_algo_modem::template_bank::TemplateBank;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// Samples of a sub-frame at the working rate.
const SUBFRAME: usize = 2 * SUBFRAME_CHIPS;
/// The SYNC-DL start from the sub-frame start, samples.
const SYNC_AT: usize = 2 * SYNC_DL_START;
/// The midamble window: chips 368 to 495 of time slot 0 (the last 128 of the 144 midamble chips).
const MID_WINDOW: usize = MIDAMBLE_START + 16;
/// The time from the middle of the midamble window to the middle of the SYNC-DL, s.
const PHASE_SPAN_S: f64 = ((SYNC_DL_START + 32) - (MID_WINDOW + 64)) as f64 / CHIP_RATE;
/// The width of the channel low-pass of the front end (two-sided), Hz.
const CHANNEL_HZ: f64 = 1.6e6;
/// The midamble clarity above which the parameter pair is known.
const CLARITY_PAIR: f64 = 1.3;
/// The midamble clarity, with the SNR, that keeps a cell.
const CLARITY_KEEP: f64 = 1.5;
/// A phase within this many degrees of a quadruple's phase counts.
const PHASE_TOL: f64 = 40.0;

/// One cell found in a capture.
#[derive(Clone, Debug, PartialEq)]
pub struct TdFound {
    /// The SYNC-DL code (= code group).
    pub code: usize,
    /// The initial cell parameter and whether it is sure (a pair only when not).
    pub param: Option<(usize, bool)>,
    /// The DwPTS level, dBFS.
    pub level_dbfs: f64,
    /// The DwPTS SNR over its guard, dB.
    pub snr_db: f64,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// Quadruples S1 found.
    pub s1: u32,
    /// Quadruples S2 found.
    pub s2: u32,
    /// The SFN of the capture's first whole frame is even.
    pub parity_even: Option<bool>,
    /// The midamble clarity.
    pub clarity: f64,
}

/// The searcher: the front end, the SYNC-DL correlators and the buffers, kept from capture to capture.
pub struct TdSearcher {
    roll_off: f64,
    cache: ResamplerCache,
    mf: Option<Convolver<Complex32>>,
    taps: Vec<f32>,
    bank: Option<TemplateBank>,
    codes: Vec<Vec<Complex32>>,
    mids: Vec<Vec<Complex32>>,
    raw: Vec<Complex32>,
    y: Vec<Complex32>,
    /// The mean power of the samples of the newest capture.
    power: f64,
}

impl Default for TdSearcher {
    fn default() -> Self {
        Self::new(ROLL_OFF)
    }
}

impl TdSearcher {
    /// A searcher whose matched filter is a root raised cosine of `roll_off`.
    pub fn new(roll_off: f64) -> Self {
        let taps = pulse::taps(Shape::Rrc { beta: roll_off }, 2.0, 12.0, Norm::Energy)
            .unwrap_or_else(|_| vec![1.0]);
        let codes: Vec<Vec<Complex32>> = (0..32).map(sync_dl).collect();
        let templates: Vec<Vec<Complex32>> = codes.iter().map(|c| stuffed(c)).collect();
        let bank = TemplateBank::new(&templates, 4096).ok();
        TdSearcher {
            roll_off,
            cache: ResamplerCache::default(),
            mf: Convolver::new(&taps).ok(),
            taps,
            bank,
            codes,
            mids: (0..128).map(basic_midamble).collect(),
            raw: Vec::new(),
            y: Vec::new(),
            power: 0.0,
        }
    }

    /// The roll-off of the matched filter.
    pub fn roll_off(&self) -> f64 {
        self.roll_off
    }
}

/// A code at two samples a chip: chip `i` at sample `2 i`, zeros between.
fn stuffed(code: &[Complex32]) -> Vec<Complex32> {
    let mut t = vec![Complex32::default(); 2 * code.len()];
    for (i, &c) in code.iter().enumerate() {
        t[2 * i] = c;
    }
    t
}

/// `angle` wrapped to -180 .. 180 degrees.
fn wrap_deg(a: f64) -> f64 {
    (a + 180.0).rem_euclid(360.0) - 180.0
}

/// What one candidate measures in each sub-frame.
struct SubframeMeasure {
    /// The SYNC-DL correlation over 64 chips over 64.
    sd: Complex32,
    /// The best power of each of the four parameters of the group over the cyclic shifts.
    score: [f64; 4],
    /// The correlation with midamble m(1) of each parameter (its whole basic code).
    h1: [Complex32; 4],
    /// The mean power of the 32 guard chips of the DwPTS.
    guard: f64,
    /// The sample of the SYNC-DL start.
    at: usize,
}

impl TdSearcher {
    /// The cells of `cap`: SYNC-DL codes whose folded correlation exceeds `threshold` times the median, at most `max_cells`.
    pub fn search(&mut self, cap: &Capture, threshold: f64, max_cells: usize) -> Vec<TdFound> {
        if cap.rate <= 0.0 || cap.samples.is_empty() {
            return Vec::new();
        }
        let n_out = (cap.samples.len() as f64 * NATIVE_RATE / cap.rate) as usize;
        resample_to(
            &mut self.cache,
            &cap.samples,
            cap.rate,
            cap.dial_offset_hz,
            NATIVE_RATE,
            CHANNEL_HZ,
            0.0,
            0.0,
            n_out,
            &mut self.raw,
        );
        self.y.clear();
        self.y.resize(self.raw.len(), Complex32::default());
        match self.mf.as_ref() {
            Some(mf) if mf.centred(&self.raw, &mut self.y).is_ok() => {}
            _ => self.y.copy_from_slice(&self.raw),
        }
        let subframes = self.y.len() / SUBFRAME;
        if subframes < 2 {
            return Vec::new();
        }
        self.power = self.y.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / self.y.len() as f64;
        // Step 2: the correlation power of every code folded over the whole sub-frames.
        let mut folds = vec![vec![0f32; SUBFRAME]; self.codes.len()];
        let Some(bank) = self.bank.as_mut() else {
            return Vec::new();
        };
        let limit = subframes * SUBFRAME;
        bank.process(&self.y, |c, first, corr| {
            for (i, v) in corr.iter().enumerate() {
                let at = first + i;
                if at < limit {
                    folds[c][at % SUBFRAME] += v.norm_sqr();
                }
            }
        });
        let mut sample: Vec<f32> = folds
            .iter()
            .flat_map(|f| f.iter().step_by(8).copied())
            .collect();
        if sample.is_empty() {
            return Vec::new();
        }
        let mid = sample.len() / 2;
        let median = *sample
            .select_nth_unstable_by(mid, |a, b| a.total_cmp(b))
            .1;
        let mut cands: Vec<(usize, usize, f32)> = folds
            .iter()
            .enumerate()
            .filter_map(|(c, f)| {
                let (bin, &v) = f.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
                (f64::from(v) > threshold * f64::from(median)).then_some((c, bin, v))
            })
            .collect();
        cands.sort_by(|a, b| b.2.total_cmp(&a.2));
        let mut found = Vec::new();
        for (code, bin, _) in cands {
            if found.len() >= max_cells {
                break;
            }
            if let Some(f) = self.measure(code, bin) {
                found.push(f);
            }
        }
        found
    }

    /// The correlation of the SYNC-DL chips `a .. b` of code `code` with the samples from `at`, the carrier `f` (cycles per
    /// sample) taken off.
    fn corr(&self, code: usize, at: usize, a: usize, b: usize, f: f64) -> Complex32 {
        let c = &self.codes[code];
        let mut acc = Complex32::default();
        for i in a..b {
            let n = at + 2 * i;
            let Some(&v) = self.y.get(n) else { break };
            let rot = Complex32::from_polar(1.0, (-TAU * f * n as f64) as f32);
            acc += v * rot * c[i].conj();
        }
        acc
    }

    /// Measures the candidate of code `code` whose SYNC-DL peak sits at fold bin `bin`, keeps it or drops it, and takes a kept
    /// cell's SYNC-DL out of the samples.
    fn measure(&mut self, code: usize, bin: usize) -> Option<TdFound> {
        let start0 = (bin + SUBFRAME - SYNC_AT) % SUBFRAME;
        // Step 3: every whole sub-frame, the SYNC-DL start refined within 3 samples, and the carrier from the two halves.
        let mut ats = Vec::new();
        let mut s = start0;
        while s + SUBFRAME <= self.y.len() {
            let pred = s + SYNC_AT;
            let best = (pred.saturating_sub(3)..=pred + 3)
                .max_by(|&a, &b| {
                    self.corr(code, a, 0, 64, 0.0)
                        .norm_sqr()
                        .total_cmp(&self.corr(code, b, 0, 64, 0.0).norm_sqr())
                })
                .unwrap_or(pred);
            ats.push(best);
            s += SUBFRAME;
        }
        if ats.len() < 2 {
            return None;
        }
        let mut z = Complex32::default();
        for &at in &ats {
            z += self.corr(code, at, 32, 64, 0.0) * self.corr(code, at, 0, 32, 0.0).conj();
        }
        let mut f = f64::from(z.im.atan2(z.re)) / TAU / 64.0;
        // Step 4: per sub-frame, the SYNC-DL value and the midamble scores of the four parameters of the group.
        let first_param = 4 * code;
        let meas: Vec<SubframeMeasure> = ats
            .iter()
            .map(|&at| {
                let sd = self.corr(code, at, 0, 64, f) / SYNC_DL_CHIPS as f32;
                let sub0 = at - SYNC_AT;
                let w0 = sub0 + 2 * MID_WINDOW;
                let win: Vec<Complex32> = (0..MIDAMBLE_CHIPS)
                    .map(|i| {
                        let n = w0 + 2 * i;
                        self.y.get(n).copied().unwrap_or_default()
                            * Complex32::from_polar(1.0, (-TAU * f * n as f64) as f32)
                    })
                    .collect();
                let mut score = [0.0; 4];
                let mut h1 = [Complex32::default(); 4];
                for q in 0..4 {
                    let m = &self.mids[first_param + q];
                    let mut best = 0f64;
                    for shift in 0..MIDAMBLE_CHIPS {
                        let mut acc = Complex32::default();
                        for (i, &w) in win.iter().enumerate() {
                            acc += w * m[(i + shift) % MIDAMBLE_CHIPS].conj();
                        }
                        if shift == 0 {
                            h1[q] = acc;
                        }
                        best = best.max(f64::from(acc.norm_sqr()));
                    }
                    score[q] = best;
                }
                let g0 = sub0 + 2 * DWPTS_START;
                let guard = (g0..g0 + 64)
                    .filter_map(|n| self.y.get(n))
                    .map(|v| f64::from(v.norm_sqr()))
                    .sum::<f64>()
                    / 64.0;
                SubframeMeasure {
                    sd,
                    score,
                    h1,
                    guard,
                    at,
                }
            })
            .collect();
        // Step 5: level and SNR.
        let ps = meas.iter().map(|m| f64::from(m.sd.norm_sqr())).sum::<f64>() / meas.len() as f64;
        // The guard holds the noise. A capture without noise leaves only pulse tails there, so the guard power is taken as at
        // least 25 dB under the mean power of the capture: the dynamic range of the measurement.
        let pg = (meas.iter().map(|m| m.guard).sum::<f64>() / meas.len() as f64).max(10f64.powf(-2.5) * self.power);
        let level_dbfs = 10.0 * ps.max(1e-30).log10();
        let snr_db = 10.0 * (ps / pg.max(1e-30)).max(1e-30).log10();
        // Step 6: the parameter pair and the frame boundaries.
        let pair_sum = |q: usize, a: usize| -> f64 {
            meas.iter()
                .enumerate()
                .map(|(k, m)| {
                    let frame = (k + a) / 2;
                    m.score[if frame.is_multiple_of(2) { q } else { q ^ 1 }]
                })
                .sum()
        };
        let mut best = (0usize, 0usize, f64::NEG_INFINITY);
        for q in 0..4 {
            for a in 0..2 {
                let v = pair_sum(q, a);
                if v > best.2 {
                    best = (q, a, v);
                }
            }
        }
        let (q, a, top) = best;
        let other = [q ^ 2, q ^ 3]
            .into_iter()
            .flat_map(|o| [pair_sum(o, 0), pair_sum(o, 1)])
            .fold(0.0, f64::max);
        let clarity = if other > 0.0 { top / other } else { 0.0 };
        let in_use = |k: usize| -> usize {
            let frame = (k + a) / 2;
            if frame.is_multiple_of(2) {
                q
            } else {
                q ^ 1
            }
        };
        // Step 7: the quadruples. The phase of each sub-frame against m(1) of the parameter in use.
        let phases: Vec<f64> = meas
            .iter()
            .enumerate()
            .map(|(k, m)| {
                let z = m.sd * m.h1[in_use(k)].conj();
                f64::from(z.im.atan2(z.re)).to_degrees()
            })
            .collect();
        let classify = |k: usize, delta: f64| -> Option<bool> {
            let near = |pat: &[f64; 4]| {
                (0..4)
                    .filter(|&i| wrap_deg(phases[k + i] - delta - pat[i]).abs() <= PHASE_TOL)
                    .count()
            };
            let (n1, n2) = (near(&S1), near(&S2));
            if n1 >= 3 && n2 <= 1 {
                Some(true)
            } else if n2 >= 3 && n1 <= 1 {
                Some(false)
            } else {
                None
            }
        };
        let mut quad: Option<(f64, usize, u32, u32)> = None;
        for step in 0..72 {
            let delta = -175.0 + 5.0 * step as f64;
            for off in 0..4 {
                // A quadruple starts a frame.
                if (off + a) % 2 != 0 {
                    continue;
                }
                let (mut n1, mut n2) = (0u32, 0u32);
                let mut k = off;
                while k + 4 <= meas.len() {
                    match classify(k, delta) {
                        Some(true) => n1 += 1,
                        Some(false) => n2 += 1,
                        None => {}
                    }
                    k += 4;
                }
                let n = n1 + n2;
                let better = match quad {
                    None => n > 0,
                    Some((d0, _, a1, a2)) => {
                        n > a1 + a2 || (n == a1 + a2 && delta.abs() < d0.abs())
                    }
                };
                if better {
                    quad = Some((delta, off, n1, n2));
                }
            }
        }
        if let Some((delta, ..)) = quad {
            f += delta / 360.0 / PHASE_SPAN_S / NATIVE_RATE;
        }
        // Step 8: the initial parameter and the parity.
        let (param, parity_even) = match quad {
            Some((_, off, ..)) if clarity > CLARITY_PAIR => {
                let fq = (off + a) / 2;
                let p = first_param + if fq % 2 == 0 { q } else { q ^ 1 };
                (Some((p, true)), Some((a + fq) % 2 == 0))
            }
            None if clarity > CLARITY_PAIR => (Some((first_param + (q & !1), false)), None),
            _ => (None, None),
        };
        // Step 9: keep or drop.
        if !((clarity >= CLARITY_KEEP && snr_db > -3.0) || snr_db > 3.0) {
            return None;
        }
        // Step 10: take the cell's SYNC-DL out of the samples.
        self.cancel(code, &meas, f);
        let (s1, s2) = quad.map_or((0, 0), |(_, _, a1, a2)| (a1, a2));
        Some(TdFound {
            code,
            param,
            level_dbfs,
            snr_db,
            offset_hz: f * NATIVE_RATE,
            s1,
            s2,
            parity_even,
            clarity,
        })
    }

    /// Subtracts the SYNC-DL of code `code` from each sub-frame of `meas`: the code shaped by the pulse twice (sent and matched),
    /// with the least-squares amplitude of that sub-frame, the carrier `f` cycles per sample.
    fn cancel(&mut self, code: usize, meas: &[SubframeMeasure], f: f64) {
        let t = stuffed(&self.codes[code]);
        let l = self.taps.len();
        let half = l - 1;
        // The code through both pulse filters: chip 0 at sample `half`.
        let mut shaped = vec![Complex32::default(); t.len() + 2 * half];
        for (i, &v) in t.iter().enumerate() {
            if v == Complex32::default() {
                continue;
            }
            for (a, &ha) in self.taps.iter().enumerate() {
                for (b, &hb) in self.taps.iter().enumerate() {
                    shaped[i + a + b] += v * (ha * hb);
                }
            }
        }
        let e: f32 = shaped.iter().map(|v| v.norm_sqr()).sum();
        if e <= 0.0 {
            return;
        }
        for m in meas {
            let Some(start) = m.at.checked_sub(half) else {
                continue;
            };
            if start + shaped.len() > self.y.len() {
                continue;
            }
            let rot = |n: usize| Complex32::from_polar(1.0, (TAU * f * n as f64) as f32);
            let mut acc = Complex32::default();
            for (i, s) in shaped.iter().enumerate() {
                acc += self.y[start + i] * (s * rot(start + i)).conj();
            }
            let amp = acc / e;
            for (i, s) in shaped.iter().enumerate() {
                self.y[start + i] -= amp * s * rot(start + i);
            }
        }
    }
}
