//! The PACTOR-II, -III and -IV physical-layer analyser: clock lines, the PACTOR-III grid scan, the PACTOR-II
//! pair, the common symbol clock, differential phase symbols, the repeating frame structure, and the PACTOR-IV constellation
//! estimate. All on 8 kHz real audio. Nothing is decoded: the channel coding is proprietary.

use num_complex::Complex64;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::{design_by_taps, Fir};
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_dsp::goertzel::{goertzel, goertzel_windowed};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::stats::{plain_kurtosis_iq, power_concentration, WindowSum};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_dsp::Complex32;
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

/// The audio rate.
pub const FS: f64 = 8000.0;

/// One carrier of the analysis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CarrierInfo {
    pub hz: f64,
    pub line_db: f64,
    /// Phase steps per symbol (2, 4, 8, 16).
    pub order: u32,
    pub concentration: f64,
    pub symbols: Vec<u8>,
}

/// The repeating frame structure.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub found: bool,
    pub period_symbols: usize,
    pub period_s: f64,
    pub const_start: usize,
    pub const_len: usize,
    pub const_hex: String,
    pub copies: usize,
    pub agreement: f64,
    pub z: f64,
}

/// What an analysis found (`pactorhf.analysis`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    pub found: bool,
    /// 2, 3 or 4 (0: nothing).
    pub variant: u8,
    pub baud: f64,
    pub centre_hz: f64,
    pub spacing_hz: f64,
    pub bandwidth_hz: f64,
    pub clock_line_db: f64,
    pub clock_ppm: f64,
    /// The demodulated stretch: start and length, seconds into the analysed audio.
    pub start_s: f64,
    pub length_s: f64,
    pub carrier_count: usize,
    pub level: u8,
    pub carriers: Vec<CarrierInfo>,
    pub symbol_count: usize,
    pub frame: Frame,
    /// PACTOR-IV: 2, 4, 8, 16 (QAM), 0 unknown.
    pub constellation: u8,
    pub note: String,
}

/// The PACTOR-III speed level of a carrier count (2 -> 1, 6 -> 2, 14 -> 3 or 4, 16 -> 5, 18 -> 6; others 0).
pub fn level_of(count: usize, order: u32) -> u8 {
    match count {
        2 => 1,
        6 => 2,
        14 => {
            if order >= 4 {
                4
            } else {
                3
            }
        }
        16 => 5,
        18 => 6,
        _ => 0,
    }
}

/// The carrier count of a PACTOR-III level.
pub fn count_of(level: u8) -> usize {
    match level {
        1 => 2,
        2 => 6,
        3 | 4 => 14,
        5 => 16,
        6 => 18,
        _ => 0,
    }
}

/// The grid frequencies of a level: the centred set of its count on 480 + 120 n.
pub fn grid_of(level: u8) -> Vec<f64> {
    let c = count_of(level);
    let first = (18 - c) / 2;
    (first..first + c)
        .map(|n| 480.0 + 120.0 * n as f64)
        .collect()
}

/// Mixes the audio `x` down by `fc` Hz into `out`: `out[n] = x[n] e^(-j 2 pi fc n / FS)`.
pub(crate) fn mix(x: &[f32], fc: f64, out: &mut Vec<Complex32>) {
    out.clear();
    out.resize(x.len(), Complex32::new(0.0, 0.0));
    Nco::new(FS, -fc).mix_real(x, out);
}

/// The magnitude of the moving sum of `l` samples of `z`, one value per full window, into `out`.
pub(crate) fn envelope(z: &[Complex32], l: usize, out: &mut Vec<f32>) {
    out.clear();
    let Ok(mut w) = WindowSum::<Complex32>::new(l) else {
        return;
    };
    for &v in z {
        let s = w.push(v);
        if w.is_full() {
            out.push(s.norm() as f32);
        }
    }
}

/// The line at `f` Hz of the real sequence `e` at FS. Returns `|X|^2`, `sum (e - mean)^2` and `X = sum (e - mean) e^(-j 2 pi f n /
/// FS)`. `d` is scratch space.
pub(crate) fn line(e: &[f32], f: f64, d: &mut Vec<f32>) -> (f64, f64, Complex64) {
    if e.is_empty() {
        return (0.0, 1.0, Complex64::new(0.0, 0.0));
    }
    let mean = e.iter().map(|&v| v as f64).sum::<f64>() / e.len() as f64;
    d.clear();
    d.extend(e.iter().map(|&v| (v as f64 - mean) as f32));
    let den = d.iter().map(|&v| (v as f64) * (v as f64)).sum::<f64>();
    let x = goertzel(d, f / FS);
    (x.norm_sqr(), den.max(1e-30), x)
}

/// Scratch buffers reused between calls.
#[derive(Default)]
pub struct Scratch {
    z: Vec<Complex32>,
    e: Vec<f32>,
    d: Vec<f32>,
}

/// The clock line of a carrier at `fc` and baud `baud`: (dB, mean envelope power).
pub fn clock_line(x: &[f32], fc: f64, baud: f64, s: &mut Scratch) -> (f64, f64) {
    let l = ((0.7 * FS / baud).round() as usize).max(2);
    mix(x, fc, &mut s.z);
    envelope(&s.z, l, &mut s.e);
    let (num, den, _) = line(&s.e, baud, &mut s.d);
    let pw = s.e.iter().map(|&v| (v as f64) * (v as f64)).sum::<f64>() / s.e.len().max(1) as f64;
    (power_to_db((num / den).max(1e-12)), pw)
}

/// Symbols of a carrier at `f`: the complex integral of the window `win` around each symbol centre, `sum x[n] w e^(-j 2 pi f n /
/// FS)`. A window that does not fit in `x` returns 0.
pub(crate) fn symbols(x: &[f32], f: f64, centres: &[f64], win: &[f32]) -> Vec<(f64, f64)> {
    let h = win.len() as i64 / 2;
    let w = TAU * f / FS;
    centres
        .iter()
        .map(|&c| {
            let c0 = c.round() as i64 - h;
            if c0 < 0 || c0 as usize + win.len() > x.len() {
                return (0.0, 0.0);
            }
            let s = c0 as usize;
            let v = goertzel_windowed(&x[s..s + win.len()], win, f / FS)
                * Complex64::from_polar(1.0, -w * c0 as f64);
            (v.re, v.im)
        })
        .collect()
}

pub(crate) fn steps(s: &[(f64, f64)]) -> Vec<(f64, f64)> {
    s.windows(2)
        .map(|p| {
            let (a, b) = (p[1], p[0]);
            (a.0 * b.0 + a.1 * b.1, a.1 * b.0 - a.0 * b.1)
        })
        .collect()
}

/// |mean of (d / |d|)^m| and its angle (0 and 0 for no steps).
pub(crate) fn conc(d: &[(f64, f64)], m: u32) -> (f64, f64) {
    power_concentration(d.iter().map(|&(a, b)| Complex64::new(a, b)), m).unwrap_or((0.0, 0.0))
}

/// The grid score of a carrier at `f`: |mean (d/|d|)^4| x (mean symbol power)^2.
fn grid_score(x: &[f32], f: f64, centres: &[f64], win: &[f32]) -> f64 {
    let s = symbols(x, f, centres, win);
    let p = s.iter().map(|v| v.0 * v.0 + v.1 * v.1).sum::<f64>() / s.len().max(1) as f64;
    conc(&steps(&s), 4).0 * p * p
}

/// The symbol clock from the summed envelopes of the carriers: (true baud, first symbol centre in samples, ppm, line dB).
fn symbol_clock(x: &[f32], freqs: &[f64], baud: f64, s: &mut Scratch) -> (f64, f64, f64, f64) {
    let l = ((0.7 * FS / baud).round() as usize).max(2);
    let mut sum: Vec<f32> = Vec::new();
    for &f in freqs {
        mix(x, f, &mut s.z);
        envelope(&s.z, l, &mut s.e);
        if sum.is_empty() {
            sum = s.e.clone();
        } else {
            for (a, b) in sum.iter_mut().zip(&s.e) {
                *a += b;
            }
        }
    }
    let (num, den, _) = line(&sum, baud, &mut s.d);
    let line_db = power_to_db((num / den).max(1e-12));
    // Phase of the line per block of 200 symbols.
    let blk = (200.0 * FS / baud) as usize;
    let mut pts: Vec<(f64, f64, f64)> = Vec::new();
    let mut k = 0;
    while k + blk <= sum.len() {
        let (_, _, v) = line(&sum[k..k + blk], baud, &mut s.d);
        // The line's phase relative to the stream start.
        let ph = v.arg() - TAU * baud * k as f64 / FS;
        pts.push(((k + blk / 2) as f64, ph, v.norm()));
        k += blk / 2;
    }
    if pts.is_empty() {
        let (_, _, v) = line(&sum, baud, &mut s.d);
        pts.push((sum.len() as f64 / 2.0, v.arg(), 1.0));
    }
    // Unwrap.
    for i in 1..pts.len() {
        let mut d = pts[i].1 - pts[i - 1].1;
        while d > PI {
            pts[i].1 -= TAU;
            d -= TAU;
        }
        while d < -PI {
            pts[i].1 += TAU;
            d += TAU;
        }
    }
    // A weighted straight line: phase = a + b t.
    let (mut sw, mut st, mut sp, mut stt, mut stp) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(t, p, w) in &pts {
        sw += w;
        st += w * t;
        sp += w * p;
        stt += w * t * t;
        stp += w * t * p;
    }
    let den = sw * stt - st * st;
    let b = if pts.len() >= 2 && den.abs() > 1e-9 {
        (sw * stp - st * sp) / den
    } else {
        0.0
    };
    let a = (sp - b * st) / sw.max(1e-30);
    // The measured phase grows by b per sample when the true clock is faster than the nominal one.
    let true_baud = baud + b * FS / TAU;
    // The envelope peaks where 2 pi baud' i / FS + a = 0 (i an envelope index); envelope index i is the moving sum over samples
    // i .. i + l - 1, centred (l - 1) / 2 later.
    let w = TAU * true_baud / FS;
    let period = FS / true_baud;
    let n0 = ((-a / w).rem_euclid(period) + (l as f64 - 1.0) / 2.0).rem_euclid(period);
    (true_baud, n0, (true_baud / baud - 1.0) * 1e6, line_db)
}

fn rc_window(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| (0.5 - 0.5 * (TAU * (i as f64 + 0.5) / n as f64).cos()) as f32)
        .collect()
}

/// Demodulates one carrier: frequency correction, order, the constant step removed per block, steps rounded.
fn demod_carrier(x: &[f32], f0: f64, centres: &[f64], win: &[f32], baud: f64) -> CarrierInfo {
    let mut f = f0;
    let mut d = steps(&symbols(x, f, centres, win));
    for it in 0..7 {
        let (_, ang) = conc(&d, 4);
        let df = ang / 4.0 / TAU * baud;
        if it >= 2 && df.abs() < 0.15 {
            break;
        }
        if it >= 3 && df.abs() <= 2.0 {
            break;
        }
        f += df;
        d = steps(&symbols(x, f, centres, win));
    }
    let c2 = conc(&d, 2).0;
    let c4 = conc(&d, 4).0;
    let c8 = conc(&d, 8).0;
    let c16 = conc(&d, 16).0;
    let (order, cc) = if c2 > 0.7 {
        (2, c2)
    } else if c4 > 0.5 {
        (4, c4)
    } else if c8 > 0.5 {
        (8, c8)
    } else if c16 > 0.4 {
        (16, c16)
    } else {
        (4, c4)
    };
    // The constant step per block of 96 symbols (with its neighbours), removed; each step rounded to the order's grid.
    let mut syms = Vec::with_capacity(d.len());
    let blk = 96;
    let nb = d.len().div_ceil(blk);
    for b in 0..nb {
        let a = (b * blk).saturating_sub(blk);
        let e = ((b + 2) * blk).min(d.len());
        let (_, ang) = conc(&d[a..e], order);
        let off = ang / order as f64;
        for &(re, im) in &d[b * blk..((b + 1) * blk).min(d.len())] {
            let ph = im.atan2(re) - off;
            let k = (ph / (TAU / order as f64)).round().rem_euclid(order as f64) as u8;
            syms.push(k);
        }
    }
    CarrierInfo {
        hz: f,
        line_db: 0.0,
        order,
        concentration: cc,
        symbols: syms,
    }
}

/// The repeating frame structure of up to 4 carriers' symbols.
pub fn frame_structure(carriers: &[&[u8]], order: u32, baud: f64) -> Frame {
    let s = carriers.iter().map(|c| c.len()).min().unwrap_or(0);
    let max_lag = (s / 5).min(3000);
    let p0 = 1.0 / order as f64;
    let mut best: (f64, usize) = (f64::MIN, 0);
    let mut scores: Vec<(usize, f64)> = Vec::new();
    let mut agree = vec![0u32; max_lag + 1];
    let mut count = vec![0u32; max_lag + 1];
    for lag in 40..=max_lag {
        agree[..lag].iter_mut().for_each(|v| *v = 0);
        count[..lag].iter_mut().for_each(|v| *v = 0);
        for c in carriers {
            for k in 0..s - lag {
                let p = k % lag;
                count[p] += 1;
                agree[p] += (c[k] == c[k + lag]) as u32;
            }
        }
        // The best cyclic window of 24 positions.
        let w = 24.min(lag);
        let (mut a, mut n) = (0u64, 0u64);
        for p in 0..w {
            a += agree[p] as u64;
            n += count[p] as u64;
        }
        let mut bz = f64::MIN;
        for p in 0..lag {
            let z = (a as f64 - n as f64 * p0) / (n as f64 * p0 * (1.0 - p0)).max(1e-9).sqrt();
            bz = bz.max(z);
            let out = p;
            let inn = (p + w) % lag;
            a = a + agree[inn] as u64 - agree[out] as u64;
            n = n + count[inn] as u64 - count[out] as u64;
        }
        scores.push((lag, bz));
        if bz > best.0 {
            best = (bz, lag);
        }
    }
    if best.0 < 9.0 {
        return Frame {
            z: best.0.max(0.0),
            ..Default::default()
        };
    }
    let lag = scores
        .iter()
        .find(|x| x.1 >= 0.97 * best.0)
        .map_or(best.1, |x| x.0);
    // Agreement per position at that lag, and the constant part.
    let mut ag = vec![0u32; lag];
    let mut ct = vec![0u32; lag];
    for c in carriers {
        for k in 0..s - lag {
            ct[k % lag] += 1;
            ag[k % lag] += (c[k] == c[k + lag]) as u32;
        }
    }
    let frac: Vec<f64> = (0..lag)
        .map(|p| ag[p] as f64 / ct[p].max(1) as f64)
        .collect();
    let thr = 0.5 + 0.5 / order as f64;
    let good: Vec<bool> = frac.iter().map(|&f| f > thr).collect();
    let mut run = (0usize, 0usize);
    if let Some(start0) = good.iter().position(|&g| !g) {
        let (mut rs, mut rl) = (0, 0);
        for i in 1..=lag {
            let j = (start0 + i) % lag;
            if good[j] {
                if rl == 0 {
                    rs = j;
                }
                rl += 1;
                if rl > run.1 {
                    run = (rs, rl);
                }
            } else {
                rl = 0;
            }
        }
    }
    let mut hex = String::new();
    let c0 = carriers[0];
    let take = run.1.min(64);
    let mut bits: Vec<u8> = Vec::new();
    for i in 0..take {
        let p = (run.0 + i) % lag;
        let mut votes = [0u32; 16];
        let mut k = p;
        while k < c0.len() {
            votes[c0[k] as usize % 16] += 1;
            k += lag;
        }
        let m = (0..16).max_by_key(|&v| votes[v]).unwrap_or(0) as u8;
        bits.push((m >> 1) & 1);
        bits.push(m & 1);
    }
    for ch in bits.chunks(4) {
        let mut v = 0u8;
        for (i, &b) in ch.iter().enumerate() {
            v |= b << (3 - i);
        }
        hex.push(
            char::from_digit(v as u32, 16)
                .unwrap_or('0')
                .to_ascii_uppercase(),
        );
    }
    let agreement = frac.iter().sum::<f64>() / lag as f64;
    Frame {
        found: true,
        period_symbols: lag,
        period_s: lag as f64 / baud,
        const_start: run.0,
        const_len: run.1,
        const_hex: hex,
        copies: s / lag + 1,
        agreement,
        z: best.0,
    }
}

/// The 100 baud analysis (PACTOR-II and -III).
fn analyse_100(x: &[f32], asked: u8, sc: &mut Scratch) -> Analysis {
    let mut a = Analysis::default();
    let n12 = (12.0 * FS) as usize;
    let seg = &x[x.len().saturating_sub(n12)..];
    // Clock lines every 10 Hz from 300 to 3300 Hz.
    let lines: Vec<(f64, f64, f64)> = (0..=300)
        .map(|i| {
            let f = 300.0 + 10.0 * i as f64;
            let (d, p) = clock_line(seg, f, 100.0, sc);
            (f, d, p)
        })
        .collect();
    let pmax = lines.iter().map(|l| l.2).fold(0.0, f64::max);
    let valid: Vec<bool> = lines.iter().map(|l| l.2 >= 0.06 * pmax).collect();
    let best = lines
        .iter()
        .zip(&valid)
        .filter(|(_, &v)| v)
        .map(|(l, _)| l.1)
        .fold(f64::MIN, f64::max);
    if best < 22.0 {
        return a;
    }
    // Local maxima within 12 dB of the best (at least 10 dB, over +-6 steps), refined in 3 Hz steps over +-12 Hz.
    let mut refined: Vec<(f64, f64)> = Vec::new();
    for i in 0..lines.len() {
        if !valid[i] || lines[i].1 < best - 12.0 || lines[i].1 < 10.0 {
            continue;
        }
        let lo = i.saturating_sub(6);
        let hi = (i + 6).min(lines.len() - 1);
        if (lo..=hi).any(|j| j != i && valid[j] && lines[j].1 > lines[i].1) {
            continue;
        }
        let mut b = (lines[i].0, lines[i].1);
        for k in -4..=4 {
            let f = lines[i].0 + 3.0 * k as f64;
            let (d, _) = clock_line(seg, f, 100.0, sc);
            if d > b.1 {
                b = (f, d);
            }
        }
        refined.push(b);
    }
    // The PACTOR-III grid scan on the first 6 s of the buffer.
    let n6 = (6.0 * FS) as usize;
    let first6 = &x[..x.len().min(n6)];
    let freqs_guess: Vec<f64> = refined.iter().map(|r| r.0).collect();
    let (clk_baud, clk_n0, _, _) = symbol_clock(first6, &freqs_guess, 100.0, sc);
    let period = FS / clk_baud;
    let centres = |len: usize, n0: f64, period: f64| -> Vec<f64> {
        let mut v = Vec::new();
        let mut c = n0;
        while c + 40.0 < len as f64 {
            if c >= 40.0 {
                v.push(c);
            }
            c += period;
        }
        v
    };
    let rect = vec![1.0f32; 67];
    let mut p3: Option<(f64, Vec<f64>)> = None;
    // PACTOR-II requested: the grid is scanned only when three or more lines are there (a PACTOR-III signal on the channel).
    if asked != 2 || refined.len() >= 3 {
        let cen6 = centres(first6.len(), clk_n0, period);
        let scan = |err: f64| -> Vec<f64> {
            (-1..=18)
                .map(|n| grid_score(first6, 480.0 + 120.0 * n as f64 + err, &cen6, &rect))
                .collect()
        };
        let mut best_err = (0.0, f64::MIN, Vec::new());
        let mut err = -60.0;
        while err <= 52.0 {
            let s = scan(err);
            let m = s.iter().cloned().fold(0.0, f64::max);
            let tot: f64 = s.iter().filter(|&&v| v >= 0.3 * m).sum();
            if tot > best_err.1 {
                best_err = (err, tot, s);
            }
            err += 8.0;
        }
        let e0 = best_err.0;
        let mut e = e0 - 6.0;
        while e <= e0 + 6.0 {
            let s = scan(e);
            let m = s.iter().cloned().fold(0.0, f64::max);
            let tot: f64 = s.iter().filter(|&&v| v >= 0.3 * m).sum();
            if tot > best_err.1 {
                best_err = (e, tot, s);
            }
            e += 2.0;
        }
        let (err, _, s) = best_err;
        let m = s.iter().cloned().fold(0.0, f64::max);
        let on: Vec<usize> = (0..s.len())
            .filter(|&i| s[i] >= 0.3 * m && m > 0.0)
            .collect();
        let need = if asked == 3 { 2 } else { 3 };
        let need = if asked == 2 { need.max(3) } else { need };
        if on.len() >= need {
            let (a0, a1) = (on[0], on[on.len() - 1]);
            let freqs: Vec<f64> = (a0..=a1)
                .map(|i| 480.0 + 120.0 * (i as f64 - 1.0) + err)
                .collect();
            p3 = Some((err, freqs));
        }
    }
    let (variant, freqs) = if let Some((_, f)) = p3 {
        (3u8, f)
    } else {
        // The PACTOR-II pair: two refined lines 170 to 270 Hz apart, the strongest weaker line wins.
        let mut pair: Option<(f64, f64, f64)> = None;
        for i in 0..refined.len() {
            for j in i + 1..refined.len() {
                let d = (refined[j].0 - refined[i].0).abs();
                if (170.0..=270.0).contains(&d) {
                    let w = refined[i].1.min(refined[j].1);
                    if pair.is_none_or(|p| w > p.2) {
                        pair = Some((
                            refined[i].0.min(refined[j].0),
                            refined[i].0.max(refined[j].0),
                            w,
                        ));
                    }
                }
            }
        }
        let Some((f1, f2, _)) = pair else {
            return a;
        };
        // The centre searched +-24 Hz in 2 Hz steps with the pair exactly 200 Hz apart, scored on the first 8 s.
        let n8 = (8.0 * FS) as usize;
        let first8 = &x[..x.len().min(n8)];
        let cen8 = centres(first8.len(), clk_n0, period);
        let win = rc_window(56);
        let c0 = (f1 + f2) / 2.0;
        let mut best_c = (c0, f64::MIN);
        let mut c = c0 - 24.0;
        while c <= c0 + 24.0 {
            let s = grid_score(first8, c - 100.0, &cen8, &win)
                + grid_score(first8, c + 100.0, &cen8, &win);
            if s > best_c.1 {
                best_c = (c, s);
            }
            c += 2.0;
        }
        (2u8, vec![best_c.0 - 100.0, best_c.0 + 100.0])
    };
    // The common symbol clock and the symbols over the whole buffer (up to 40 s).
    let (baud, n0, ppm, line_db) = symbol_clock(x, &freqs, 100.0, sc);
    let period = FS / baud;
    let cen = centres(x.len(), n0, period);
    let win = if variant == 2 {
        rc_window(56)
    } else {
        rect.clone()
    };
    let mut carriers: Vec<CarrierInfo> = freqs
        .iter()
        .map(|&f| {
            let mut c = demod_carrier(x, f, &cen, &win, baud);
            c.line_db = clock_line(seg, c.hz, 100.0, sc).0;
            c
        })
        .collect();
    carriers.sort_by(|a, b| a.hz.partial_cmp(&b.hz).unwrap_or(std::cmp::Ordering::Equal));
    let nsym = carriers.iter().map(|c| c.symbols.len()).min().unwrap_or(0);
    // The frame structure on up to 4 carriers spread over the set.
    let n = carriers.len();
    let pick: Vec<usize> = if n <= 4 {
        (0..n).collect()
    } else {
        let mut v = vec![0, n / 3, 2 * n / 3, n - 1];
        v.dedup();
        v
    };
    let order = carriers.iter().map(|c| c.order).max().unwrap_or(4);
    let sl: Vec<&[u8]> = pick.iter().map(|&i| &carriers[i].symbols[..nsym]).collect();
    let frame = frame_structure(&sl, order, baud);
    let lo = carriers.first().map_or(0.0, |c| c.hz);
    let hi = carriers.last().map_or(0.0, |c| c.hz);
    a.found = true;
    a.variant = if variant == 3 && n == 2 { 2 } else { variant };
    a.baud = baud;
    a.centre_hz = (lo + hi) / 2.0;
    a.spacing_hz = if n >= 2 {
        (hi - lo) / (n - 1) as f64
    } else {
        0.0
    };
    a.bandwidth_hz = (hi - lo) + if a.variant == 2 { 100.0 } else { 130.0 };
    a.clock_line_db = line_db;
    a.clock_ppm = ppm;
    a.start_s = cen.first().map_or(0.0, |c| c / FS);
    a.length_s = x.len() as f64 / FS - a.start_s;
    a.carrier_count = n;
    a.level = if a.variant == 3 {
        level_of(n, order)
    } else {
        0
    };
    a.symbol_count = nsym;
    a.frame = frame;
    a.carriers = carriers;
    a
}

/// The audio `seg` mixed down by `fc` and low-passed by `fir` into `y`. The first `taps - 1` outputs are dropped, so `y[i]` is the
/// filter output over `seg[i ..= i + taps - 1]`.
fn mix_filter(
    seg: &[f32],
    fc: f64,
    fir: &mut Fir<Complex32>,
    s: &mut Scratch,
    y: &mut Vec<Complex32>,
) {
    mix(seg, fc, &mut s.z);
    fir.reset();
    fir.process_in_place(&mut s.z);
    let skip = (fir.len() - 1).min(s.z.len());
    y.clear();
    y.extend_from_slice(&s.z[skip..]);
}

/// The strongest spectral line of `(z / |z|)^m` within `max_f` cycles per symbol of 0. Returns its magnitude `|mean (z / |z|)^m e^(-j
/// 2 pi f k)|` (0 to 1) and its frequency `f` in cycles per symbol. The search is one FFT of the symbols zero-padded to at least
/// twice their count. Fewer than 2 symbols give (0, 0).
pub fn power_line(z: &[Complex32], m: u32, max_f: f64) -> (f64, f64) {
    if z.len() < 2 {
        return (0.0, 0.0);
    }
    let n = (2 * z.len()).next_power_of_two();
    let mut b = vec![Complex32::new(0.0, 0.0); n];
    for (o, v) in b.iter_mut().zip(z) {
        let r = v.norm();
        *o = if r > 0.0 {
            (v / r).powu(m)
        } else {
            Complex32::new(1.0, 0.0)
        };
    }
    Fft::forward(n).process(&mut b);
    let mut best = (0.0, 0.0);
    for (k, v) in b.iter().enumerate() {
        let f = if k < n / 2 {
            k as f64 / n as f64
        } else {
            k as f64 / n as f64 - 1.0
        };
        if f.abs() > max_f {
            continue;
        }
        let mag = v.norm() as f64 / z.len() as f64;
        if mag > best.0 {
            best = (mag, f);
        }
    }
    best
}

/// The 1800 baud analysis (PACTOR-IV).
fn analyse_1800(x: &[f32], sc: &mut Scratch) -> Analysis {
    let mut a = Analysis::default();
    let n10 = (10.0 * FS) as usize;
    let seg = &x[x.len().saturating_sub(n10)..];
    // A 41-tap Hamming low pass at 1350 Hz.
    let taps = design_by_taps(41, 1350.0 / FS, Window::Hamming).expect("a valid low pass");
    let mut fir = Fir::<Complex32>::new(&taps);
    let mut y: Vec<Complex32> = Vec::new();
    let blk = (2.0 * FS) as usize;
    let mut d = Vec::new();
    let mut line_db = |y: &[Complex32]| -> (f64, Vec<(f64, f64, f64)>) {
        let e: Vec<f32> = y.iter().map(|v| v.norm_sqr()).collect();
        let (mut num, mut den) = (0.0, 0.0);
        let mut ph = Vec::new();
        for (bi, ch) in e.chunks(blk).enumerate() {
            if ch.len() < blk / 2 {
                continue;
            }
            let (nm, dn, v) = line(ch, 1800.0, &mut d);
            num += nm;
            den += dn;
            let ang = v.arg() - TAU * 1800.0 * (bi * blk) as f64 / FS;
            ph.push(((bi * blk + ch.len() / 2) as f64, ang, nm.sqrt()));
        }
        (power_to_db((num / den.max(1e-30)).max(1e-12)), ph)
    };
    let mut best: (f64, f64, Vec<(f64, f64, f64)>) = (0.0, f64::MIN, Vec::new());
    let mut fc = 1220.0;
    while fc <= 1780.0 {
        mix_filter(seg, fc, &mut fir, sc, &mut y);
        let (d, ph) = line_db(&y);
        if d > best.1 {
            best = (fc, d, ph);
        }
        fc += 20.0;
    }
    let (fc, ldb, mut ph) = best;
    a.clock_line_db = ldb;
    if ldb < 9.0 {
        return a;
    }
    // The symbol clock: the line phase per block, unwrapped and fitted.
    for i in 1..ph.len() {
        let mut d = ph[i].1 - ph[i - 1].1;
        while d > PI {
            ph[i].1 -= TAU;
            d -= TAU;
        }
        while d < -PI {
            ph[i].1 += TAU;
            d += TAU;
        }
    }
    let (mut sw, mut st, mut sp, mut stt, mut stp) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for &(t, p, w) in &ph {
        sw += w;
        st += w * t;
        sp += w * p;
        stt += w * t * t;
        stp += w * t * p;
    }
    let den = sw * stt - st * st;
    let b = if ph.len() >= 2 && den.abs() > 1e-9 {
        (sw * stp - st * sp) / den
    } else {
        0.0
    };
    let a0 = (sp - b * st) / sw.max(1e-30);
    let baud = 1800.0 + b * FS / TAU;
    let period = FS / baud;
    let w = TAU * baud / FS;
    // Envelope maxima: 2 pi baud n / FS + a0 = 0 (the filter delay is already in y's indices).
    let n0 = (-a0 / w).rem_euclid(period);
    mix_filter(seg, fc, &mut fir, sc, &mut y);
    let mut z: Vec<Complex32> = Vec::new();
    let mut t = n0;
    while t + 1.0 < y.len() as f64 {
        let i = t.floor() as usize;
        let fr = (t - i as f64) as f32;
        z.push(y[i] + (y[i + 1] - y[i]) * fr);
        t += period;
    }
    a.baud = baud;
    a.found = true;
    a.variant = 4;
    a.carrier_count = 1;
    a.symbol_count = z.len();
    a.bandwidth_hz = 2400.0;
    a.start_s = (x.len() - seg.len()) as f64 / FS;
    a.length_s = seg.len() as f64 / FS;
    if z.len() < 500 {
        a.centre_hz = fc;
        a.note = format!("1800 baud clock line {ldb:.1} dB; too few symbols");
        return a;
    }
    let spread = plain_kurtosis_iq(&z).unwrap_or(f64::NAN) - 1.0;
    // The m-th power lines of the unit symbols. The carrier error is within +-40 Hz: m times that in cycles per symbol.
    let pline = |m: u32| power_line(&z, m, (m as f64 * 40.0 / baud).min(0.5));
    let (p2, f2) = pline(2);
    let (p4, f4) = pline(4);
    let (p8, f8) = pline(8);
    let (cons, ferr) = if spread >= 0.28 {
        (16u8, f4 / 4.0)
    } else if p2 > 0.5 {
        (2u8, f2 / 2.0)
    } else if p4 > 0.7 {
        (4u8, f4 / 4.0)
    } else if p8 > 0.5 && p4 < 0.3 {
        (8u8, f8 / 8.0)
    } else {
        (0u8, f4 / 4.0)
    };
    a.constellation = cons;
    a.centre_hz = fc + ferr * baud;
    a.note = format!(
        "1800 baud clock line {ldb:.1} dB; symbol power lines p2 {p2:.2} p4 {p4:.2} p8 {p8:.2}, modulus spread {spread:.2}"
    );
    a
}

/// Analyses up to 40 s of 8 kHz audio for the variant `asked` (2, 3 or 4).
pub fn analyse(x: &[f32], asked: u8) -> Analysis {
    if x.len() < (2.0 * FS) as usize {
        return Analysis::default();
    }
    let mut sc = Scratch::default();
    if asked == 4 {
        return analyse_1800(x, &mut sc);
    }
    let p4 = analyse_1800(x, &mut sc);
    if p4.found && p4.clock_line_db > 12.0 {
        return p4;
    }
    analyse_100(x, asked, &mut sc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a6_levels() {
        assert_eq!(level_of(2, 4), 1);
        assert_eq!(level_of(6, 2), 2);
        assert_eq!(level_of(18, 4), 6);
        assert_eq!(level_of(5, 4), 0);
        assert_eq!(grid_of(4).len(), 14);
        assert_eq!(grid_of(4)[0], 480.0 + 240.0);
    }
}
