//! The GSM search of one capture: carrier finder, channeliser, FCCH, SCH and every BCCH block.

use super::codes::{GsmDecoder, Sch, SCH_TRAINING, TSC};
use super::gmsk::{Trainer, SYMBOL_RATE};
use crate::decoder::Capture;
use crate::dsp::{resample_to, welch, ResamplerCache};
use num_complex::Complex64;
use sdrkit_algo_dsp::meter::power_to_db as db;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_core::Complex32;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// The channel stream rate: 4 samples per symbol.
pub const STREAM_RATE: f64 = 4.0 * SYMBOL_RATE;
/// Samples of the stream per TDMA frame.
pub const FRAME: i64 = 5000;

/// A carrier found in the spectrum.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Carrier {
    /// Offset from 0 Hz in the capture.
    pub offset_hz: f64,
    /// dB over the floor.
    pub level_db: f64,
    /// Power, dBFS.
    pub power_dbfs: f64,
}

/// The carrier finder: at most `max` carriers, strongest first.
pub fn find_carriers(x: &[Complex32], fs: f64, max: usize) -> Vec<Carrier> {
    let nfft = ((fs / 10e3).ceil() as usize).next_power_of_two();
    if nfft < 64 || x.len() < 8 * nfft {
        return Vec::new();
    }
    let s = welch(x, nfft, 300);
    let bin = fs / nfft as f64;
    let hw = (100e3 / bin).round() as usize;
    let n = s.len();
    let mut sums = vec![0f32; n];
    for (i, v) in sums.iter_mut().enumerate() {
        let lo = i.saturating_sub(hw);
        let hi = (i + hw).min(n - 1);
        *v = s[lo..=hi].iter().sum();
    }
    let floor = percentile(&sums, 0.25, &mut Vec::new()).max(1e-30);
    let edge = (hw + 1).max((0.04 * nfft as f64).ceil() as usize);
    let mut out = Vec::new();
    for i in edge..n.saturating_sub(edge) {
        let v = sums[i];
        if v < 5.0 * floor {
            continue;
        }
        let lo = i.saturating_sub(hw);
        let hi = (i + hw).min(n - 1);
        // The highest within +-100 kHz; ties go to the lower bin.
        let mut top = true;
        for (j, &w) in sums.iter().enumerate().take(hi + 1).skip(lo) {
            if j == i {
                continue;
            }
            if w > v || (w == v && j < i) {
                top = false;
                break;
            }
        }
        if top {
            out.push(Carrier {
                offset_hz: (i as f64 - (n / 2) as f64) * bin,
                level_db: db((v / floor) as f64),
                power_dbfs: db(v as f64),
            });
        }
    }
    out.sort_by(|a, b| b.level_db.total_cmp(&a.level_db));
    out.truncate(max);
    out
}

/// One cell found on a carrier.
#[derive(Clone, Debug, PartialEq)]
pub struct GsmFound {
    /// The carrier, Hz from 0 in the capture.
    pub offset_hz: f64,
    /// The carrier's power, dBFS.
    pub level_dbfs: f64,
    /// The SCH burst's SNR, dB.
    pub snr_db: f64,
    /// BSIC.
    pub bsic: u8,
    /// The SCH's frame number.
    pub fnum: u32,
    /// Frames that passed the FIRE check, with their frame numbers.
    pub frames: Vec<(u32, [u8; 23])>,
    /// FIRE failures with all four bursts demodulated.
    pub failed: u32,
}

/// The results of a capture.
#[derive(Clone, Debug, Default)]
pub struct GsmSearchOut {
    /// The carriers of the spectrum, with whether each synchronised.
    pub carriers: Vec<(Carrier, bool)>,
    /// Cells.
    pub cells: Vec<GsmFound>,
}

/// Two cells with the same BSIC closer than this in one capture are one cell seen through two humps, Hz.
const SAME_CELL_HZ: f64 = 60e3;

/// The GSM searcher.
pub struct GsmSearcher {
    cache: ResamplerCache,
    sch: Trainer,
    bcch: Vec<Trainer>,
    codec: GsmDecoder,
}

impl Default for GsmSearcher {
    fn default() -> Self {
        GsmSearcher {
            cache: ResamplerCache::default(),
            sch: Trainer::new(&SCH_TRAINING, 42),
            bcch: (0..8).map(|t| Trainer::new(&TSC[t], 61)).collect(),
            codec: GsmDecoder::new(),
        }
    }
}

/// An FCCH burst found in a channel stream.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FcchHit {
    /// The middle of the burst, in stream samples.
    pub centre: usize,
    /// The peak coherence of the burst's windows, 0 to 1.
    pub coherence: f32,
    /// The frequency error of the burst, Hz.
    pub err_hz: f64,
}

/// The FCCH window, symbols x 4 samples.
const FCCH_WINDOW: usize = 512;

/// One value of d: `y[j]` times the conjugate of `y[j - 4]`, one symbol apart, in f64. `j` is at least 4.
fn fcch_d(y: &[Complex32], j: usize) -> Complex64 {
    let d = y[j] * y[j - 4].conj();
    Complex64::new(d.re as f64, d.im as f64)
}

/// The angle error of a sum of d against a quarter turn, wrapped to -pi to pi.
fn quarter_turn_error(sd: Complex64) -> f64 {
    (sd.arg() - FRAC_PI_2 + PI).rem_euclid(TAU) - PI
}

/// The FCCH bursts in `y`, a channel stream at 4 samples per symbol, in stream order.
///
/// The window sums are running sums in f64, so a weak burst late in a long capture after a strong signal keeps its precision.
pub fn fcch_hits(y: &[Complex32]) -> Vec<FcchHit> {
    let n = y.len();
    let w = FCCH_WINDOW;
    let mut hits: Vec<FcchHit> = Vec::new();
    if n < 600 {
        return hits;
    }
    // The sums of d and of |d| over the window [i, i + w).
    let mut i = 4;
    let mut sd = Complex64::new(0.0, 0.0);
    let mut ad = 0f64;
    for j in i..i + w {
        let d = fcch_d(y, j);
        sd += d;
        ad += d.norm();
    }
    let mut run_start = 0;
    let mut cohs: Vec<f32> = Vec::new();
    loop {
        let more = i + w <= n;
        let tone = if more && ad > 0.0 {
            let coh = sd.norm() / ad;
            (coh > 0.55 && quarter_turn_error(sd).abs() < 1.5).then_some(coh as f32)
        } else {
            None
        };
        match tone {
            Some(c) => {
                if cohs.is_empty() {
                    run_start = i;
                }
                cohs.push(c);
            }
            None if !cohs.is_empty() => {
                fcch_run(y, run_start, &cohs, &mut hits);
                cohs.clear();
            }
            None => {}
        }
        if !more {
            break;
        }
        // Slide the window by one sample.
        let out = fcch_d(y, i);
        sd -= out;
        ad -= out.norm();
        if i + w < n {
            let d = fcch_d(y, i + w);
            sd += d;
            ad += d.norm();
        }
        i += 1;
    }
    hits
}

/// Turns a run of tone windows that starts at window `start` into a hit, or merges it into the last hit when they are closer
/// than a burst.
fn fcch_run(y: &[Complex32], start: usize, cohs: &[f32], hits: &mut Vec<FcchHit>) {
    let peak = cohs.iter().copied().fold(0f32, f32::max);
    if cohs.len() < 60 || peak <= 0.6 {
        return;
    }
    let first = cohs.iter().position(|&c| c >= 0.85 * peak).unwrap_or(0);
    let last = cohs.iter().rposition(|&c| c >= 0.85 * peak).unwrap_or(0);
    let centre = start + (first + last) / 2 + FCCH_WINDOW / 2;
    // The frequency from the middle 100 symbols only: the edges of the burst are not at the tone frequency.
    let lo = centre.saturating_sub(200).max(4);
    let hi = (centre + 200).min(y.len());
    let sd: Complex64 = (lo..hi).map(|j| fcch_d(y, j)).sum();
    let err_hz = quarter_turn_error(sd) * STREAM_RATE / (TAU * 4.0);
    let hit = FcchHit {
        centre,
        coherence: peak,
        err_hz,
    };
    match hits.last_mut() {
        Some(h) if centre - h.centre < 592 => {
            if peak > h.coherence {
                *h = hit;
            }
        }
        _ => hits.push(hit),
    }
}

impl GsmSearcher {
    /// Searches the whole capture: at most `max_carriers` carriers, strongest first, each searched for a cell.
    pub fn search(&mut self, cap: &Capture, max_carriers: usize) -> GsmSearchOut {
        let mut out = GsmSearchOut::default();
        for c in find_carriers(&cap.samples, cap.rate, max_carriers) {
            let found = self.search_carrier(cap, &c);
            out.carriers.push((c, found.is_some()));
            if let Some(f) = found {
                // The same cell seen through a neighbouring hump is listed once.
                if !out
                    .cells
                    .iter()
                    .any(|x| x.bsic == f.bsic && (x.offset_hz - f.offset_hz).abs() < SAME_CELL_HZ)
                {
                    out.cells.push(f);
                }
            }
        }
        out
    }

    /// Channelises `x` (at `fs`) around `f` Hz to 4 samples per symbol, from input sample `start` for `len` input samples.
    pub fn channelise(
        &mut self,
        x: &[Complex32],
        fs: f64,
        f: f64,
        start: usize,
        len: usize,
        out: &mut Vec<Complex32>,
    ) {
        out.clear();
        let end = (start + len).min(x.len());
        if end <= start {
            return;
        }
        let step = fs / STREAM_RATE;
        let n = ((end - start) as f64 / step).floor() as usize;
        resample_to(
            &mut self.cache,
            x,
            fs,
            f,
            STREAM_RATE,
            270e3,
            start as f64,
            0.0,
            n,
            out,
        );
    }

    fn median(v: &[f64]) -> f64 {
        let mut s = v.to_vec();
        s.sort_by(|a, b| a.total_cmp(b));
        s[s.len() / 2]
    }

    /// Searches one carrier: the cell when an SCH decodes.
    pub fn search_carrier(&mut self, cap: &Capture, c: &Carrier) -> Option<GsmFound> {
        let fs = cap.rate;
        let x = &cap.samples;
        let mut y = Vec::new();
        let first = ((0.5 * fs) as usize).min(x.len());
        self.channelise(x, fs, c.offset_hz, 0, first, &mut y);
        let hits = fcch_hits(&y);
        if hits.is_empty() {
            return None;
        }
        let mut f = c.offset_hz + Self::median(&hits.iter().map(|h| h.err_hz).collect::<Vec<_>>());
        self.channelise(x, fs, f, 0, x.len(), &mut y);
        let mut hits = fcch_hits(&y);
        if hits.is_empty() {
            return None;
        }
        let r = Self::median(&hits.iter().map(|h| h.err_hz).collect::<Vec<_>>());
        f += r;
        if r.abs() > 150.0 {
            self.channelise(x, fs, f, 0, x.len(), &mut y);
            hits = fcch_hits(&y);
        }
        hits.sort_by(|a, b| b.coherence.total_cmp(&a.coherence));
        // SCH one frame after an FCCH.
        let mut sync: Option<(i64, Sch, f32)> = None;
        for h in &hits {
            let fcch_start = h.centre as f64 - 74.0 * 4.0;
            let pred = fcch_start + FRAME as f64;
            if let Some(b) = self.sch.demod(&y, pred, 12) {
                let mut coded = Vec::with_capacity(78);
                coded.extend_from_slice(&b.soft[3..42]);
                coded.extend_from_slice(&b.soft[106..145]);
                if let Some(s) = self.codec.decode_sch(&coded) {
                    sync = Some((b.start, s, b.snr_db));
                    break;
                }
            }
        }
        let (p_sch, sch, snr) = sync?;
        let fn_sch = sch.fnum() as i64;
        let tsc = (sch.bsic & 7) as usize;
        // Every BCCH group (FN mod 51 = 2..5) inside the capture.
        let n = y.len() as i64;
        let f_lo = fn_sch - p_sch / FRAME - 2;
        let f_hi = fn_sch + (n - p_sch) / FRAME + 2;
        let mut frames = Vec::new();
        let mut failed = 0;
        let mut drift = 0.0f64;
        let mut g = f_lo;
        while g <= f_hi {
            if g.rem_euclid(51) != 2 {
                g += 1;
                continue;
            }
            let p0 = p_sch as f64 + ((g - fn_sch) * FRAME) as f64;
            let p3 = p0 + 3.0 * FRAME as f64;
            if p0 < -200.0 || p3 + 700.0 > n as f64 {
                g += 51;
                continue;
            }
            let mut soft = [[0f32; 114]; 4];
            let mut all = true;
            for (k, sb) in soft.iter_mut().enumerate() {
                let pred = p0 + (k as f64) * FRAME as f64 + drift;
                let search = if k == 0 { 10 } else { 6 };
                match self.bcch[tsc].demod(&y, pred, search) {
                    Some(b) => {
                        drift = b.start as f64 - (p0 + (k as f64) * FRAME as f64);
                        sb[..57].copy_from_slice(&b.soft[3..60]);
                        sb[57..].copy_from_slice(&b.soft[88..145]);
                    }
                    None => all = false,
                }
            }
            match self.codec.decode_bcch(&soft) {
                Some(m) => frames.push((g.rem_euclid(2_715_648) as u32, m)),
                None => {
                    if all {
                        failed += 1;
                    }
                }
            }
            g += 51;
        }
        Some(GsmFound {
            offset_hz: f,
            level_dbfs: c.power_dbfs,
            snr_db: snr as f64,
            bsic: sch.bsic,
            fnum: fn_sch as u32,
            frames,
            failed,
        })
    }
}
