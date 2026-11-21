//! The blind PSK analyser: the band of a signal in a spectrum, then the symbol rate from the envelope line, the order from the
//! power-law lines and trial demodulations, the pulse shape and the carrier.

use super::{PskDemodConfig, PskDemodulator, PskOrder, PskPulse, SpectrumSum};
use crate::Complex32;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::spectrum::percentile;

/// The median of `v`.
fn median(v: &[f32]) -> f32 {
    percentile(v, 0.5, &mut Vec::with_capacity(v.len()))
}

/// The fractional offset of the peak of power bins `p` at bin `i`, from a parabola through the logarithms of its neighbours.
fn peak_offset(p: &[f32], i: usize) -> f64 {
    if i == 0 || i + 1 >= p.len() {
        return 0.0;
    }
    log_parabolic(p[i - 1] as f64, p[i] as f64, p[i + 1] as f64).offset
}

/// The standard rates a measured rate is snapped to.
pub fn standard_rates() -> Vec<f64> {
    let mut v = Vec::new();
    let mut r = 75.0;
    while r <= 9600.0 * 128.0 {
        v.push(r);
        r *= 2.0;
    }
    for k in 1..=200 {
        v.push(1200.0 * k as f64);
    }
    v.extend_from_slice(&[
        50.0,
        100.0,
        125.0,
        250.0,
        500.0,
        1000.0,
        2000.0,
        4000.0,
        8000.0,
        10_000.0,
        12_500.0,
        16_000.0,
        20_000.0,
        25_000.0,
        32_000.0,
        50_000.0,
        64_000.0,
        100_000.0,
        125_000.0,
        250_000.0,
        500_000.0,
        1_000_000.0,
        10_500.0,
        8400.0,
        1500.0,
        3000.0,
        6000.0,
    ]);
    v.sort_by(|a, b| a.total_cmp(b));
    v.dedup();
    v
}

/// The standard rate within 0.6 % of `r`, if any.
pub fn snap(r: f64) -> Option<f64> {
    standard_rates()
        .into_iter()
        .filter(|s| (r - s).abs() <= 0.006 * s)
        .min_by(|a, b| (r - a).abs().total_cmp(&(r - b).abs()))
}

/// One signal found by the channel finder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    /// Centre, Hz from the dial.
    pub centre_hz: f64,
    /// Width at 3 dB below the top, Hz.
    pub width_3db_hz: f64,
    /// Width at 10 dB below the top, Hz.
    pub width_10db_hz: f64,
    /// The band that holds 99 % of the power, Hz.
    pub occupied_99_hz: f64,
    /// The power in the band over the noise in the same bins, dB.
    pub snr_db: f64,
    /// The highest bin over the noise floor, dB.
    pub peak_db: f64,
}

/// Finds the strongest region above the noise in a power spectrum (bin 0 at `f0`, bins `bin_hz` apart), limited to `[lo, hi]` Hz,
/// skipping a DC spike at `dc` (one or two bins), near `want` when given (a click or the dial) within `reach`.
pub fn find_region(
    p: &[f32],
    f0: f64,
    bin_hz: f64,
    lo: f64,
    hi: f64,
    dc: Option<f64>,
    want: Option<(f64, f64)>,
) -> Option<Found> {
    let n = p.len();
    let f = |i: usize| f0 + i as f64 * bin_hz;
    let idx: Vec<usize> = (0..n).filter(|&i| f(i) >= lo && f(i) <= hi).collect();
    if idx.len() < 8 {
        return None;
    }
    let mut q: Vec<f32> = idx.iter().map(|&i| p[i]).collect();
    // Remove a DC spike one or two bins wide.
    if let Some(d) = dc {
        for (k, &i) in idx.iter().enumerate() {
            if (f(i) - d).abs() <= 1.5 * bin_hz {
                let nb = |j: isize| q[(k as isize + j).clamp(0, q.len() as isize - 1) as usize];
                q[k] = nb(-3).min(nb(3));
            }
        }
    }
    // Smooth over 3 bins.
    let sm: Vec<f32> = (0..q.len())
        .map(|k| {
            let a = q[k.saturating_sub(1)];
            let c = q[(k + 1).min(q.len() - 1)];
            (a + 2.0 * q[k] + c) / 4.0
        })
        .collect();
    let floor = median(&sm).max(1e-30);
    let thr = floor * 4.0;
    // Regions above 6 dB over the floor.
    let mut regions: Vec<(usize, usize, f64)> = Vec::new();
    let mut k = 0;
    while k < sm.len() {
        if sm[k] > thr {
            let s = k;
            let mut e = k;
            let mut gap = 0;
            let mut m = k + 1;
            while m < sm.len() {
                if sm[m] > thr {
                    e = m;
                    gap = 0;
                } else {
                    gap += 1;
                    if gap > 2 {
                        break;
                    }
                }
                m += 1;
            }
            let pw: f64 = (s..=e).map(|j| (sm[j] - floor) as f64).sum();
            regions.push((s, e, pw));
            k = e + 1;
        } else {
            k += 1;
        }
    }
    let pick = match want {
        Some((w, reach)) => regions
            .iter()
            .filter(|r| {
                let (a, b) = (f(idx[r.0]), f(idx[r.1]));
                (w >= a && w <= b) || (w - a).abs().min((w - b).abs()) <= reach
            })
            .min_by(|a, b| {
                let ca = (f(idx[a.0]) + f(idx[a.1])) / 2.0;
                let cb = (f(idx[b.0]) + f(idx[b.1])) / 2.0;
                (ca - w).abs().total_cmp(&(cb - w).abs())
            }),
        None => regions.iter().max_by(|a, b| a.2.total_cmp(&b.2)),
    }?;
    let (s, e, _) = *pick;
    // Grow the region down to 1 dB over the floor for the totals.
    let mut s2 = s;
    while s2 > 0 && sm[s2 - 1] > floor * 1.26 {
        s2 -= 1;
    }
    let mut e2 = e;
    while e2 + 1 < sm.len() && sm[e2 + 1] > floor * 1.26 {
        e2 += 1;
    }
    let ex: Vec<f64> = (s2..=e2).map(|j| (sm[j] - floor).max(0.0) as f64).collect();
    let tot: f64 = ex.iter().sum();
    let cen = (s2..=e2)
        .zip(ex.iter())
        .map(|(j, w)| f(idx[j]) * w)
        .sum::<f64>()
        / tot.max(1e-30);
    let peak = (s2..=e2).map(|j| sm[j]).fold(0.0f32, f32::max);
    let peak_ex = (peak - floor).max(1e-30) as f64;
    // Widths at -3 dB and -10 dB of the top of the signal (noise subtracted). The top is a high percentile of the region, not its
    // single highest bin, which noise lifts.
    let top = {
        let mut v: Vec<f32> = (s2..=e2).map(|j| sm[j] - floor).collect();
        v.sort_by(|a, b| a.total_cmp(b));
        v[(v.len() as f64 * 0.8) as usize].max(1e-30) as f64
    };
    let width_at = |frac: f64| {
        let lvl = top * frac;
        let a = (s2..=e2)
            .find(|&j| (sm[j] - floor) as f64 >= lvl)
            .unwrap_or(s2);
        let b = (s2..=e2)
            .rev()
            .find(|&j| (sm[j] - floor) as f64 >= lvl)
            .unwrap_or(e2);
        (b - a + 1) as f64 * bin_hz
    };
    // 99 % occupied bandwidth.
    let mut acc = 0.0;
    let mut a99 = s2;
    for (j, w) in (s2..=e2).zip(ex.iter()) {
        acc += w;
        if acc >= 0.005 * tot {
            a99 = j;
            break;
        }
    }
    acc = 0.0;
    let mut b99 = e2;
    for (j, w) in (s2..=e2).rev().zip(ex.iter().rev()) {
        acc += w;
        if acc >= 0.005 * tot {
            b99 = j;
            break;
        }
    }
    let nbins = (e2 - s2 + 1) as f64;
    Some(Found {
        centre_hz: cen,
        width_3db_hz: width_at(0.5),
        width_10db_hz: width_at(0.1),
        occupied_99_hz: (b99.saturating_sub(a99) + 1) as f64 * bin_hz,
        snr_db: power_to_db(tot / (floor as f64 * nbins)),
        peak_db: power_to_db(peak_ex / floor as f64),
    })
}

/// Why the analyser found no PSK signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisNote {
    /// The capture is shorter than 1024 samples.
    TooShort,
    /// No symbol rate line stands out of the envelope spectrum.
    NoRateLine,
    /// No trial demodulation could run.
    NoTrial,
    /// The best trial's EVM is too high for a PSK signal.
    NoConstellation,
}

/// One trial demodulation of the analyser.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrderTrial {
    /// The order tried.
    pub order: PskOrder,
    /// The symbol rate tried, Hz.
    pub baud: f64,
    /// The EVM over the second half of the capture.
    pub evm: f64,
}

/// What the analyser found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Analysis {
    /// A PSK signal was found.
    pub signal: bool,
    /// The symbol rate as measured, Hz.
    pub baud_raw: f64,
    /// The symbol rate in Hz, snapped to a standard rate when requested.
    pub baud: f64,
    /// The rate was snapped.
    pub snapped: bool,
    /// The confidence in the rate, 0 to 1.
    pub rate_confidence: f64,
    /// The strongest lines of the envelope spectrum: (Hz, dB over the median).
    pub rate_candidates: Vec<(f64, f64)>,
    /// The order, when a signal was found.
    pub order: Option<PskOrder>,
    /// The confidence in the order, 0 to 1.
    pub order_confidence: f64,
    /// The trial demodulations.
    pub order_trials: Vec<OrderTrial>,
    /// The pulse shape.
    pub shape: Option<PskPulse>,
    /// The carrier, Hz from the centre of the capture.
    pub carrier_hz: f64,
    /// The heights of the lines of the signal squared, to the fourth and to the eighth, dB over the median.
    pub line_x2_db: f64,
    /// See `line_x2_db`.
    pub line_x4_db: f64,
    /// See `line_x2_db`.
    pub line_x8_db: f64,
    /// The height of the symbol rate line of the envelope, dB over the median.
    pub envelope_db: f64,
    /// The EVM of the best trial.
    pub evm: f64,
    /// Es/N0 from the EVM, dB.
    pub esn0_db: f64,
    /// Why no signal was found.
    pub note: Option<AnalysisNote>,
    /// The envelope spectrum (0 to half the rate, dB over the median), reduced to about 512 points.
    pub envelope_spectrum: Vec<f32>,
    /// The M-th power spectrum of the first order tried (centred, dB over the median), reduced to about 256 points.
    pub power_spectrum: Vec<f32>,
}

/// The strongest line of `|FFT(x)|^2` within `|f| <= lim` (centred, Hz), as (frequency, dB over the median).
fn line(x: &[Complex32], rate: f64, lim: f64, spread: f64) -> (f64, f64, Vec<f32>) {
    let n = x.len().next_power_of_two();
    let mut fft = Fft::forward(n);
    let mut b: Vec<Complex32> = x.to_vec();
    b.resize(n, Complex32::new(0.0, 0.0));
    fft.process(&mut b);
    let p: Vec<f32> = (0..n).map(|i| b[(i + n / 2) % n].norm_sqr()).collect();
    let bin = rate / n as f64;
    // Three neighbouring bins summed (a line spread over two bins counts whole), against the median of the same sums over the band
    // the power spreads over.
    let p3: Vec<f32> = (0..n)
        .map(|i| p[i.saturating_sub(1)] + p[i] + p[(i + 1).min(n - 1)])
        .collect();
    let near: Vec<f32> = p3
        .iter()
        .enumerate()
        .filter(|(i, _)| ((*i as f64 - (n / 2) as f64) * bin).abs() <= spread.max(8.0 * bin))
        .map(|(_, v)| *v)
        .collect();
    let med = median(&near).max(1e-30);
    let mut best = (n / 2, 0.0f32);
    for (i, &v) in p3.iter().enumerate() {
        let f = (i as f64 - (n / 2) as f64) * bin;
        if f.abs() <= lim && v > best.1 {
            best = (i, v);
        }
    }
    let best = (best.0, best.1);
    let fr = peak_offset(&p, best.0);
    let f = (best.0 as f64 + fr - (n / 2) as f64) * bin;
    let small: Vec<f32> = p
        .chunks((n / 256).max(1))
        .map(|c| power_to_db(c.iter().cloned().fold(0.0, f32::max) as f64 / med as f64) as f32)
        .collect();
    (f, power_to_db(best.1 as f64 / med as f64), small)
}

/// The envelope line between 1.5 and 2.6 times the width, with a filter of 1.6 times the width: (frequency, dB) when it stands out.
fn wide_envelope_line(xs: &[Complex32], rate: f64, width: f64) -> Option<(f64, f64)> {
    let cut = (1.6 * width / rate).clamp(0.02, 0.45);
    let lp =
        sdrkit_algo_dsp::fir::windowed_sinc(63, cut, 6.0).expect("the filter settings are valid");
    let mut f = sdrkit_algo_dsp::fir::Fir::<Complex32>::new(
        &lp.iter().map(|v| *v as f32).collect::<Vec<_>>(),
    );
    let env: Vec<f32> = xs
        .iter()
        .map(|v| f.process_sample(*v).norm_sqr())
        .skip(64)
        .collect();
    let mean = env.iter().sum::<f32>() / env.len().max(1) as f32;
    let nn = env.len().next_power_of_two();
    let mut b: Vec<Complex32> = env.iter().map(|v| Complex32::new(v - mean, 0.0)).collect();
    b.resize(nn, Complex32::new(0.0, 0.0));
    Fft::forward(nn).process(&mut b);
    let p: Vec<f32> = (0..nn / 2).map(|i| b[i].norm_sqr()).collect();
    let bin = rate / nn as f64;
    let lo = ((1.5 * width / bin) as usize).max(2);
    let hi = ((2.6 * width / bin) as usize).min(nn / 2 - 2);
    if hi <= lo + 4 {
        return None;
    }
    let med = median(&p[lo..hi]).max(1e-30) as f64;
    let i = (lo..hi).max_by(|&a, &b| p[a].total_cmp(&p[b]))?;
    let s = power_to_db(p[i] as f64 / med);
    (s >= 16.0).then(|| ((i as f64 + peak_offset(&p, i)) * bin, s))
}

/// An envelope line.
struct EnvLine {
    freq: f64,
    db: f64,
    cands: Vec<(f64, f64)>,
    spectrum: Vec<f32>,
}

/// The envelope spectrum of `xs` through a low-pass of `k` times the width, and its strongest line between 0.35 and 2.4 times the
/// width. Returns the filtered signal too.
fn envelope_rate(xs: &[Complex32], rate: f64, width: f64, k: f64) -> (Vec<Complex32>, EnvLine) {
    let cut = (k * width / rate).clamp(0.02, 0.45);
    let lp =
        sdrkit_algo_dsp::fir::windowed_sinc(63, cut, 6.0).expect("the filter settings are valid");
    let mut f = sdrkit_algo_dsp::fir::Fir::<Complex32>::new(
        &lp.iter().map(|v| *v as f32).collect::<Vec<_>>(),
    );
    let filt: Vec<Complex32> = xs.iter().map(|v| f.process_sample(*v)).collect();
    let env: Vec<f32> = filt[64..].iter().map(|v| v.norm_sqr()).collect();
    let mean = env.iter().sum::<f32>() / env.len() as f32;
    let nn = env.len().next_power_of_two();
    let mut b: Vec<Complex32> = env.iter().map(|v| Complex32::new(v - mean, 0.0)).collect();
    b.resize(nn, Complex32::new(0.0, 0.0));
    Fft::forward(nn).process(&mut b);
    let p: Vec<f32> = (0..nn / 2).map(|i| b[i].norm_sqr()).collect();
    let bin = rate / nn as f64;
    let lo = ((0.35 * width / bin) as usize).max(2);
    let hi = ((2.4 * width / bin) as usize).clamp(lo + 3, nn / 2 - 2);
    // The envelope reaches twice the filter's width: the noise level is taken inside that.
    let mhi = ((1.9 * k * width / bin) as usize).clamp(lo + 2, hi);
    let med = median(&p[lo..mhi]).max(1e-30);
    let mut cands: Vec<(usize, f32)> = (lo..hi)
        .filter(|&i| p[i] > p[i - 1] && p[i] >= p[i + 1])
        .map(|i| (i, p[i]))
        .collect();
    cands.sort_by(|a, b| b.1.total_cmp(&a.1));
    let cl: Vec<(f64, f64)> = cands
        .iter()
        .take(4)
        .map(|&(i, v)| {
            (
                (i as f64 + peak_offset(&p, i)) * bin,
                power_to_db(v as f64 / med as f64),
            )
        })
        .collect();
    let best = cands.first().cloned().unwrap_or((lo, 0.0));
    let spectrum: Vec<f32> = p
        .chunks((nn / 512).max(1))
        .map(|c| power_to_db(c.iter().cloned().fold(0.0, f32::max) as f64 / med as f64) as f32)
        .collect();
    (
        filt,
        EnvLine {
            freq: (best.0 as f64 + peak_offset(&p, best.0)) * bin,
            db: power_to_db(best.1 as f64 / med as f64),
            cands: cl,
            spectrum,
        },
    )
}

/// The pair of lines of a squared OQPSK signal: (rate, carrier, strength in dB), the lines at 2 carrier +- rate.
fn line_pair(x2: &[Complex32], rate: f64, width: f64) -> Option<(f64, f64, f64)> {
    let n = x2.len().next_power_of_two();
    let mut fft = Fft::forward(n);
    let mut b: Vec<Complex32> = x2.to_vec();
    b.resize(n, Complex32::new(0.0, 0.0));
    fft.process(&mut b);
    let p: Vec<f32> = (0..n).map(|i| b[(i + n / 2) % n].norm_sqr()).collect();
    let p3: Vec<f32> = (0..n)
        .map(|i| p[i.saturating_sub(1)] + p[i] + p[(i + 1).min(n - 1)])
        .collect();
    let bin = rate / n as f64;
    let fq = |i: usize| (i as f64 - (n / 2) as f64) * bin;
    let lim = (2.0 * width).min(0.45 * rate);
    let idx: Vec<usize> = (1..n - 1).filter(|&i| fq(i).abs() <= lim).collect();
    let med = median(&idx.iter().map(|&i| p3[i]).collect::<Vec<_>>()).max(1e-30) as f64;
    // Local maxima, strongest first.
    let mut peaks: Vec<usize> = idx
        .iter()
        .copied()
        .filter(|&i| p3[i] > p3[i - 1] && p3[i] >= p3[i + 1])
        .collect();
    peaks.sort_by(|a, b| p3[*b].total_cmp(&p3[*a]));
    peaks.truncate(8);
    let mut best: Option<(f64, f64, f64)> = None;
    for (ai, &a) in peaks.iter().enumerate() {
        for &b in &peaks[ai + 1..] {
            // The lines lie 2 rates apart, a rate between 0.6 and 1.6 times the 3 dB width, and the channel is centred on the
            // signal, so their midpoint (twice the carrier) is near 0 Hz.
            let sep = (fq(a) - fq(b)).abs();
            if sep < 1.2 * width || sep > 3.2 * width || (fq(a) + fq(b)).abs() > 0.5 * width {
                continue;
            }
            let s = power_to_db(p3[a].min(p3[b]) as f64 / med);
            if s < 10.0 {
                continue;
            }
            if best.as_ref().is_none_or(|x| s > x.2) {
                let fa = fq(a) + peak_offset(&p, a) * bin;
                let fb = fq(b) + peak_offset(&p, b) * bin;
                best = Some(((fa - fb).abs() / 2.0, (fa + fb) / 4.0, s));
            }
        }
    }
    best
}

/// Runs the analysis on a block of channel samples at `rate` (the signal near 0 Hz, about `width` wide).
#[allow(clippy::too_many_arguments)]
pub fn analyse(
    x: &[Complex32],
    rate: f64,
    width: f64,
    force_order: Option<PskOrder>,
    force_baud: f64,
    force_shape: Option<PskPulse>,
    snap_on: bool,
) -> Analysis {
    let mut a = Analysis::default();
    let n = x.len();
    if n < 1024 {
        return a;
    }
    let pw: f64 = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / n as f64;
    let xs: Vec<Complex32> = x.iter().map(|v| v / (pw.sqrt() as f32 + 1e-20)).collect();
    // The envelope line, seen through two low-pass filters (a narrow one makes rectangular pulses dip at every symbol change; a wide
    // one keeps the roll-off band that makes the line of a root raised cosine signal): the clearer line wins.
    let (filt, e1) = envelope_rate(&xs, rate, width, 0.75);
    let (_, e2) = envelope_rate(&xs, rate, width, 1.0);
    let margin = |e: &EnvLine| e.db - e.cands.get(1).map(|c| c.1).unwrap_or(0.0);
    let e = if margin(&e2) > margin(&e1) { e2 } else { e1 };
    let (fr, edb, espec) = (e.freq, e.db, e.spectrum);
    a.rate_candidates = e.cands;
    a.envelope_db = edb;
    a.envelope_spectrum = espec;
    a.baud_raw = fr;
    a.rate_confidence = ((edb - 8.0) / 20.0).clamp(0.0, 1.0);
    let snapped = |r: f64| if snap_on { snap(r).unwrap_or(r) } else { r };
    // An OQPSK signal has no envelope line (the rails' lines cancel), but its square holds two lines at twice the carrier
    // offset +- the rate.
    let filt_sq: Vec<Complex32> = filt.iter().map(|v| v * v).collect();
    let pair = line_pair(&filt_sq, rate, width);
    // An envelope line must stand out from the other peaks of the envelope spectrum.
    let second = a.rate_candidates.get(1).map(|c| c.1).unwrap_or(0.0);
    let has_line = edb >= 20.0 || (edb >= 14.0 && edb - second >= 5.0);
    // Rectangular OQPSK: the envelope dips at every rail change, half a symbol apart: a line at twice the rate, seen with a wider
    // filter.
    let wide = wide_envelope_line(&xs, rate, width).map(|(f, s)| (f / 2.0, f64::NAN, s));
    let oq_rates: Vec<(f64, f64, f64)> = [pair, wide].into_iter().flatten().collect();
    let pair = oq_rates.iter().cloned().max_by(|a, b| a.2.total_cmp(&b.2));
    let oq =
        force_baud <= 0.0 && (!has_line || force_order == Some(PskOrder::Oqpsk)) && pair.is_some();
    let edb = if has_line || oq {
        edb.max(10.0)
    } else {
        edb.min(9.9)
    };
    let (fr, edb) = match (oq, pair) {
        (true, Some((r, _, s))) => (r, s),
        _ => (fr, edb),
    };
    if oq {
        a.baud_raw = fr;
        a.envelope_db = edb;
    }
    let mut baud = if force_baud > 0.0 { force_baud } else { fr };
    if force_baud <= 0.0 {
        if edb < 10.0 {
            a.note = Some(AnalysisNote::NoRateLine);
            return a;
        }
        let s = snapped(baud);
        a.snapped = s != baud;
        baud = s;
    }
    a.baud = baud;
    // Power-law lines.
    // The signal is within half its width of 0 Hz, so the M-th power line is within M times that.
    let lim = |m: f64| (m * 0.5 * width).min(0.45 * rate);
    let spread = |m: f64| (m * 0.5 * width).min(0.45 * rate);
    let pw2: Vec<Complex32> = filt.iter().map(|v| v * v).collect();
    let (f2, s2, sp2) = line(&pw2, rate, lim(2.0), spread(2.0));
    let pw4: Vec<Complex32> = pw2.iter().map(|v| v * v).collect();
    let (f4, s4, sp4) = line(&pw4, rate, lim(4.0), spread(4.0));
    let pw8: Vec<Complex32> = pw4.iter().map(|v| v * v).collect();
    let (f8, s8, sp8) = line(&pw8, rate, lim(8.0), spread(8.0));
    a.line_x2_db = s2;
    a.line_x4_db = s4;
    a.line_x8_db = s8;
    // The shape: rectangular pulses leave sinc sidelobes beyond 0.75 of the rate.
    let spec = {
        let nn = 4096.min(n.next_power_of_two());
        let Ok(mut sa) = SpectrumSum::new(nn) else {
            return a;
        };
        sa.push(&xs);
        sa
    };
    let bin = rate / spec.len() as f64;
    let floor = median(spec.sum()) as f64;
    let sm: Vec<f64> = (0..spec.len())
        .map(|i| {
            let lo = i.saturating_sub(2);
            let hi = (i + 2).min(spec.len() - 1);
            spec.sum()[lo..=hi].iter().map(|v| *v as f64).sum::<f64>() / (hi - lo + 1) as f64
        })
        .collect();
    let peak = sm.iter().cloned().fold(0.0f64, f64::max);
    // The pulse shape for a symbol rate: each rate tried is judged with its own shape.
    let shape_of = |baud: f64| -> PskPulse {
        if let Some(s) = force_shape {
            return s;
        }
        let (mut side, mut inb, mut edge15) = (0.0, 0.0, 0.0f64);
        for (i, v) in sm.iter().enumerate() {
            let fq = (i as f64 - (spec.len() / 2) as f64) * bin;
            let e = (*v - floor).max(0.0);
            if fq.abs() >= 0.75 * baud && fq.abs() <= 1.5 * baud {
                side += e;
            }
            if fq.abs() < 0.75 * baud {
                inb += e;
            }
            if fq.abs() < 1.5 * baud && e > 0.0316 * (peak - floor) {
                edge15 = edge15.max(fq.abs());
            }
        }
        let side_ratio = side / (inb + side).max(1e-30);
        // Rectangular pulses keep sinc sidelobes (-13 dB at 1.43 times the rate) beyond any root raised cosine edge.
        if edge15 > 1.1 * baud || side_ratio > 0.06 {
            PskPulse::Rect
        } else {
            // The RRC spectrum falls to -15 dB at (1 - b) / 2 + 0.887 b of the rate.
            let b = ((edge15 / baud - 0.5) / 0.387).clamp(0.1, 1.0);
            PskPulse::Rrc((b * 20.0).round() / 20.0)
        }
    };
    let shape = shape_of(baud);
    a.shape = Some(shape);
    // Candidate orders.
    let thr = 12.0;
    let cands: Vec<PskOrder> = match force_order {
        Some(o) => vec![o],
        None => {
            if oq {
                vec![PskOrder::Oqpsk]
            } else if s2 > thr {
                vec![PskOrder::Bpsk]
            } else if s4 > thr && s8 > s4 + 6.0 {
                // The fourth-power line of 8PSK is ISI only: a much stronger eighth-power line keeps 8PSK in the trials.
                vec![PskOrder::Qpsk, PskOrder::Oqpsk, PskOrder::Psk8]
            } else if s4 > thr {
                vec![PskOrder::Qpsk, PskOrder::Oqpsk]
            } else if s8 > thr {
                vec![PskOrder::Psk8]
            } else {
                vec![
                    PskOrder::Bpsk,
                    PskOrder::Qpsk,
                    PskOrder::Oqpsk,
                    PskOrder::Psk8,
                ]
            }
        }
    };
    let pair_carrier = pair.map(|p| p.1);
    let carrier_of = |o: PskOrder| match o {
        PskOrder::Oqpsk if oq => pair_carrier.filter(|c| c.is_finite()).unwrap_or(f4 / 4.0),
        PskOrder::Bpsk => f2 / 2.0,
        PskOrder::Qpsk | PskOrder::Oqpsk => f4 / 4.0,
        PskOrder::Psk8 => f8 / 8.0,
    };
    a.power_spectrum = match cands[0] {
        PskOrder::Bpsk => sp2,
        PskOrder::Qpsk | PskOrder::Oqpsk => sp4,
        PskOrder::Psk8 => sp8,
    };
    // Trial demodulations, scored by EVM over the second half.
    let mut trials: Vec<(PskOrder, f64)> = cands.iter().map(|&o| (o, baud)).collect();
    if oq {
        for r in &oq_rates {
            let b = snapped(r.0);
            if !trials.iter().any(|t| (t.1 - b).abs() < 1e-6) {
                trials.push((PskOrder::Oqpsk, b));
            }
        }
    } else if force_baud <= 0.0 && cands.contains(&PskOrder::Oqpsk) && fr / 2.0 >= 0.35 * width {
        trials.push((PskOrder::Oqpsk, snapped(fr / 2.0)));
    }
    let mut best: Option<(PskOrder, f64, f64, f64, f64)> = None;
    for &(o, baud) in &trials {
        let sps = rate / baud;
        let c = carrier_of(o);
        let mut cfg = PskDemodConfig::new(o, sps);
        cfg.pulse = shape_of(baud);
        let Ok(mut d) = PskDemodulator::new(cfg) else {
            continue;
        };
        let mut nco = sdrkit_algo_dsp::nco::Nco::new(rate, -c);
        let mut evm_sum = 0.0f64;
        let mut evm_n = 0.0f64;
        let mut syms = 0usize;
        let total = (n as f64 / sps) as usize;
        let mut mixed = xs.clone();
        nco.mix_in_place(&mut mixed);
        d.process_with(&mixed, |_, run| {
            for &z in run {
                syms += 1;
                if syms > total / 2 {
                    evm_sum += (z - o.nearest(z)).norm_sqr() as f64;
                    evm_n += 1.0;
                }
            }
            std::ops::ControlFlow::Continue(())
        });
        let evm = (evm_sum / evm_n.max(1.0)).sqrt();
        a.order_trials.push(OrderTrial {
            order: o,
            baud,
            evm,
        });
        let cf = c + d.carrier_offset() * rate;
        if best.as_ref().is_none_or(|b| evm < b.1) {
            best = Some((o, evm, cf, d.lock_metric() as f64, baud));
        }
    }
    let Some((o, evm, cf, _lock, baud)) = best else {
        a.note = Some(AnalysisNote::NoTrial);
        return a;
    };
    let shape = shape_of(baud);
    a.shape = Some(shape);
    a.baud = baud;
    a.evm = evm;
    a.esn0_db = -power_to_db(evm * evm);
    a.carrier_hz = cf;
    let others: Vec<f64> = a
        .order_trials
        .iter()
        .filter(|t| t.order != o)
        .map(|t| t.evm)
        .collect();
    a.order_confidence = if others.is_empty() {
        if force_order.is_some() {
            1.0
        } else {
            ((s2.max(s4).max(s8) - thr) / 10.0).clamp(0.3, 1.0)
        }
    } else {
        let next = others.iter().cloned().fold(f64::MAX, f64::min);
        ((next - evm) / next.max(1e-9)).clamp(0.0, 1.0)
    };
    // An EVM this high is no PSK signal.
    if evm > 0.75 {
        a.note = Some(AnalysisNote::NoConstellation);
        return a;
    }
    a.order = Some(o);
    a.signal = true;
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::modulate::Linear;

    #[test]
    fn snap_rules() {
        assert_eq!(snap(1199.0), Some(1200.0));
        assert_eq!(snap(9598.0), Some(9600.0));
        assert_eq!(snap(1210.0), None);
    }

    /// `n` symbols of `o` at 2400 Bd and 8 samples a symbol (19.2 kHz), root raised cosine 0.35, the carrier at `f` Hz.
    fn signal(o: PskOrder, n: usize, f: f64) -> Vec<Complex32> {
        let rate = 19_200.0;
        let mut m =
            Linear::new(sdrkit_algo_dsp::pulse::Shape::Rrc { beta: 0.35 }, 16.0, 8.0).unwrap();
        if o == PskOrder::Oqpsk {
            m = m.with_offset_q();
        }
        let mut s = 11u32;
        let syms: Vec<Complex32> = (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                o.point((s >> 16) as usize)
            })
            .collect();
        let mut x = Vec::new();
        m.push(&syms, &mut x);
        for (i, v) in x.iter_mut().enumerate() {
            *v *= Complex32::from_polar(1.0, (std::f64::consts::TAU * f * i as f64 / rate) as f32);
        }
        x
    }

    #[test]
    fn finds_rate_order_and_carrier() {
        for o in [PskOrder::Bpsk, PskOrder::Qpsk, PskOrder::Psk8] {
            let x = signal(o, 4000, 150.0);
            let a = analyse(&x, 19_200.0, 3240.0, None, 0.0, None, true);
            assert!(a.signal, "{o:?} {:?}", a.note);
            assert_eq!(a.baud, 2400.0, "{o:?}");
            assert_eq!(a.order, Some(o), "{:?}", a.order_trials);
            assert!((a.carrier_hz - 150.0).abs() < 5.0, "{o:?} {}", a.carrier_hz);
            assert!(a.evm < 0.2, "{o:?} {}", a.evm);
        }
        let a = analyse(
            &[Complex32::default(); 100],
            19_200.0,
            3240.0,
            None,
            0.0,
            None,
            true,
        );
        assert!(!a.signal);
    }
}
