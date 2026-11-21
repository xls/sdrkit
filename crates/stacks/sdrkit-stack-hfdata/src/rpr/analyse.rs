//! The Robust Packet analyser on up to 30 s of 8 kHz real audio: the comb of eight carriers, the bursts, the symbol clock of each
//! burst, the differential phase steps of every carrier, the modulation order and the step indices. Nothing is decoded: the
//! channel coding is not public.
//!
//! 1. Comb: Hann FFTs of 4096 samples every 2048; the frames whose power between 600 Hz below and above the nominal centre is at
//!    or above the 60th percentile are averaged. For every centre within 500 Hz of the nominal one, in 1 Hz steps, the score is
//!    the mean power within 240 Hz less the mean power from 252 to 372 Hz on both sides; the best centre wins (the middle of
//!    equal best scores). The comb counts when the inside mean is at least twice the outside mean.
//! 2. Carriers: each carrier is mixed to 0 Hz and summed over 1/60 s (133 samples), which nulls the neighbours 60 Hz away.
//! 3. Bursts: the summed power of the carriers in frames of 5 ms; the threshold is the larger of 3 times the 20th percentile and
//!    1 % of the 95th percentile. A burst runs from a frame over it until more than 12 frames below it; it is trimmed by 1.5
//!    symbols at both ends and must keep more than 8 symbols. A burst that has not closed before the end of the audio waits for
//!    the next analysis.
//! 4. Clock: the 50 Hz line of the summed carrier power, taken over its mean of one symbol period (so fades do not count),
//!    places the symbol centres at its maxima. A burst of more than 150
//!    symbols also gives the clock error from the line phase of its two halves (held within 500 ppm).
//! 5. Steps: each carrier at the symbol centres; `d = s[m] conj(s[m - 1])`. Per carrier `r2 = |mean (d/|d|)^2|` and `r4 = |mean
//!    (d/|d|)^4|`; a mean `r2` over 0.4 is D-BPSK, else a mean `r4` over 0.25 is D-QPSK, else no burst.
//! 6. Decisions: per carrier the constant turn `arg(sum (d/|d|)^M) / M` is removed and each step is decided on the M steps of
//!    the order: index 0 to M - 1, relative to the observed axes (the phase convention is not public). The turn also gives the
//!    residual frequency of each carrier, and the slope of the carrier frequencies the spacing.
//! 7. A burst counts when at least 5 carriers have the concentration of its order at 0.3 or more, at least 20 symbols remain,
//!    at least 6 carriers have a power within 10 dB of the strongest (one or two tones, as of an FSK signal, leak into the
//!    neighbouring carriers with a clear step pattern but little power), and the clock line holds at least 0.5 % of the
//!    variance of the summed carrier power.

use super::{BAUD, CARRIERS, SPACING_HZ};
use crate::pactorhf::analyse::{conc, envelope, line, mix, steps, symbols};
use sdrkit_algo_dsp::constellation::Shape;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::stats::quantile_f64;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_dsp::Complex32;
use sdrkit_algo_modem::differential::Differential;
use sdrkit_algo_modem::PointSet;
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

/// The audio rate.
pub const FS: f64 = 8000.0;
const NFFT: usize = 4096;
const HOP: usize = 2048;
/// The samples of the active part of a symbol (1/60 s).
const ACTIVE: usize = 133;
/// The samples of a power frame (5 ms).
const FRAME: usize = 40;
/// The samples of a symbol period.
const PERIOD: f64 = FS / BAUD;
/// The least share of the power variance in the 50 Hz clock line: a burst of 50 Bd symbols with fades between them has a
/// clear line, an FSK burst has next to none.
const MIN_LINE: f64 = 0.005;
/// A carrier counts as carrying the burst when its power is at least this share of the strongest carrier (-10 dB).
const EVEN_POWER: f64 = 0.1;
/// The carriers that must carry the burst.
const EVEN_CARRIERS: usize = 6;
/// Half the active window: a moving sum that starts at sample `i` is centred on `i + HALF`.
const HALF: usize = ACTIVE / 2;

/// The comb found.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Comb {
    /// The centre of the comb, Hz.
    pub centre_hz: f64,
    /// The inside mean over the outside mean.
    pub contrast: f64,
    /// The carriers whose cell is at least twice the outside mean.
    pub carriers_seen: usize,
}

/// One burst demodulated.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Burst {
    /// The start, seconds into the analysed audio.
    pub start_s: f64,
    /// The length as detected, seconds.
    pub length_s: f64,
    /// The order of the phase steps: 2 (D-BPSK) or 4 (D-QPSK).
    pub order: u32,
    /// Per carrier: the concentration of the steps for the order.
    pub concentration: [f64; CARRIERS],
    /// Per carrier: the measured frequency (grid frequency and residual turn), Hz.
    pub carrier_hz: [f64; CARRIERS],
    /// The carriers with a concentration of 0.3 or more.
    pub carriers_seen: usize,
    /// The slope of the measured carrier frequencies, Hz.
    pub spacing_hz: f64,
    /// The symbols demodulated on each carrier.
    pub symbols: usize,
    /// The step indices, one byte each: for each step in time, carrier 0 to 7.
    pub steps: Vec<u8>,
    /// `10 log10(1 / mean squared phase error)` of the decided steps, dB.
    pub snr_db: f64,
    /// The symbol rate at the peak of the clock line, Hz.
    pub symbol_rate_hz: f64,
    /// The clock error of a burst of more than 150 symbols, ppm.
    pub clock_ppm: Option<f64>,
    /// The share of the variance of the summed carrier power in the clock line, 0 to 1.
    pub clock_line: f64,
}

/// What an analysis found.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    /// The comb, when found.
    pub comb: Option<Comb>,
    /// The bursts that closed in the audio and start at or after the start asked for, oldest first.
    pub bursts: Vec<Burst>,
}

/// Scratch buffers reused between analyses.
#[derive(Default)]
pub struct Scratch {
    z: Vec<Complex32>,
    e: Vec<f32>,
    d: Vec<f32>,
    power: Vec<f32>,
    frames: Vec<f64>,
    q: Vec<f64>,
    spec: Vec<f32>,
    fft: Vec<Complex32>,
}

/// The audio frequency of carrier `c` of a comb centred on `centre_hz`.
pub fn grid_hz(centre_hz: f64, c: usize) -> f64 {
    super::gen::carrier_hz(centre_hz, c)
}

/// The comb around `nominal_hz` (step 1). `None` with less than two FFT lengths of audio or a contrast under 2.
pub fn find_comb(x: &[f32], nominal_hz: f64, s: &mut Scratch) -> Option<Comb> {
    if x.len() < 2 * NFFT {
        return None;
    }
    let bin = FS / NFFT as f64;
    let lo_bin = (((nominal_hz - 500.0 - 380.0) / bin).floor().max(1.0)) as usize;
    let hi_bin = (((nominal_hz + 500.0 + 380.0) / bin).ceil() as usize).min(NFFT / 2 - 1);
    let band = (
        ((nominal_hz - 600.0) / bin).round().max(0.0) as usize,
        ((nominal_hz + 600.0) / bin).round() as usize,
    );
    let width = hi_bin - lo_bin + 1;
    let hann = Window::Hann.coefficients(NFFT);
    let mut fft = Fft::forward(NFFT);
    s.spec.clear();
    s.frames.clear();
    let mut k = 0;
    while k + NFFT <= x.len() {
        s.fft.clear();
        s.fft
            .extend(x[k..k + NFFT].iter().zip(&hann).map(|(&v, &w)| Complex32::new(v * w, 0.0)));
        fft.process(&mut s.fft);
        let mut bp = 0.0f64;
        for (i, v) in s.fft[..NFFT / 2].iter().enumerate() {
            let p = v.norm_sqr();
            if (band.0..=band.1).contains(&i) {
                bp += p as f64;
            }
            if (lo_bin..=hi_bin).contains(&i) {
                s.spec.push(p);
            }
        }
        s.frames.push(bp);
        k += HOP;
    }
    let thr = quantile_f64(&s.frames, 0.6, &mut s.q)?;
    let mut avg = vec![0.0f64; width];
    let mut used = 0usize;
    for (f, &bp) in s.frames.iter().enumerate() {
        if bp >= thr {
            used += 1;
            for (a, &p) in avg.iter_mut().zip(&s.spec[f * width..(f + 1) * width]) {
                *a += p as f64;
            }
        }
    }
    if used == 0 {
        return None;
    }
    // Prefix sums of the averaged power by bin.
    let mut cum = vec![0.0f64; width + 1];
    for i in 0..width {
        cum[i + 1] = cum[i] + avg[i] / used as f64;
    }
    let mean = |lo_hz: f64, hi_hz: f64| -> Option<f64> {
        let a = ((lo_hz / bin).ceil() as usize).max(lo_bin);
        let b = ((hi_hz / bin).floor() as usize).min(hi_bin);
        (b >= a).then(|| (cum[b - lo_bin + 1] - cum[a - lo_bin]) / (b - a + 1) as f64)
    };
    let mut best = (f64::MIN, 0.0, 0usize, 0.0, 0.0);
    let mut c = nominal_hz - 500.0;
    while c <= nominal_hz + 500.0 {
        let (Some(inside), Some(l), Some(r)) = (
            mean(c - 240.0, c + 240.0),
            mean(c - 372.0, c - 252.0),
            mean(c + 252.0, c + 372.0),
        ) else {
            c += 1.0;
            continue;
        };
        let outside = 0.5 * (l + r);
        let score = inside - outside;
        if score > best.0 + 1e-12 * score.abs().max(1e-30) {
            best = (score, c, 1, inside, outside);
        } else if (score - best.0).abs() <= 1e-12 * score.abs().max(1e-30) {
            best.1 += c;
            best.2 += 1;
        }
        c += 1.0;
    }
    let (_, sum_c, n, inside, outside) = best;
    if n == 0 || outside <= 0.0 || inside < 2.0 * outside {
        return None;
    }
    let centre = sum_c / n as f64;
    let carriers_seen = (0..CARRIERS)
        .filter(|&k| {
            let f = grid_hz(centre, k);
            mean(f - 20.0, f + 20.0).is_some_and(|m| m >= 2.0 * outside)
        })
        .count();
    Some(Comb {
        centre_hz: centre,
        contrast: inside / outside,
        carriers_seen,
    })
}

/// The summed carrier power of a comb centred on `centre_hz`: `s.power[i]` is the power of the 1/60 s sums that start at sample
/// `i`, so centred on `i + 66`.
fn carrier_power(x: &[f32], centre_hz: f64, s: &mut Scratch) {
    s.power.clear();
    for c in 0..CARRIERS {
        mix(x, grid_hz(centre_hz, c), &mut s.z);
        envelope(&s.z, ACTIVE, &mut s.e);
        if s.power.is_empty() {
            s.power.extend(s.e.iter().map(|v| v * v));
        } else {
            for (p, v) in s.power.iter_mut().zip(&s.e) {
                *p += v * v;
            }
        }
    }
}

/// The closed bursts of the frame powers (step 3), as power-sample ranges `[a, b)`.
fn segments(power: &[f32], s: &mut Scratch) -> Vec<(usize, usize)> {
    let frames: Vec<f64> = power
        .as_chunks::<FRAME>()
        .0
        .iter()
        .map(|c| c.iter().map(|&v| v as f64).sum::<f64>())
        .collect();
    if frames.len() < 100 {
        return Vec::new();
    }
    let (Some(floor), Some(p95)) = (
        quantile_f64(&frames, 0.2, &mut s.q),
        quantile_f64(&frames, 0.95, &mut s.q),
    ) else {
        return Vec::new();
    };
    let thr = (3.0 * floor).max(0.01 * p95);
    if p95 <= 0.0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut open: Option<(usize, usize)> = None;
    for (j, &f) in frames.iter().enumerate() {
        if f > thr {
            open = Some(open.map_or((j, j), |(a, _)| (a, j)));
        } else if let Some((a, b)) = open {
            if j - b > 12 {
                out.push((a * FRAME, (b + 1) * FRAME));
                open = None;
            }
        }
    }
    out
}

/// `p` over its mean of one symbol period around each sample, less 1: the fades of the channel and the ramps leave it, the
/// dips between symbols stay.
fn flatten(p: &[f32]) -> Vec<f32> {
    let half = (PERIOD / 2.0) as usize;
    let mut cum = Vec::with_capacity(p.len() + 1);
    cum.push(0.0f64);
    for &v in p {
        cum.push(cum[cum.len() - 1] + f64::from(v));
    }
    (0..p.len())
        .map(|i| {
            let a = i.saturating_sub(half);
            let b = (i + half).min(p.len());
            let mean = (cum[b] - cum[a]) / (b - a) as f64;
            if mean > 0.0 {
                (f64::from(p[i]) / mean - 1.0) as f32
            } else {
                0.0
            }
        })
        .collect()
}

/// The line of the 50 Hz clock: (the first symbol centre from `p[0]` in samples, the period in samples, the clock error in ppm
/// when the burst is long enough, the share of the power variance in the line).
fn clock(p: &[f32], s: &mut Scratch) -> (f64, f64, Option<f64>, f64) {
    let w = TAU * BAUD / FS;
    let n_sym = p.len() as f64 / PERIOD;
    let mut ppm = None;
    let mut baud = BAUD;
    if n_sym > 150.0 {
        let h = p.len() / 2;
        let (_, _, x1) = line(&p[..h], BAUD, &mut s.d);
        let (_, _, x2) = line(&p[h..2 * h], BAUD, &mut s.d);
        let ph1 = x1.arg();
        let ph2 = x2.arg() - w * h as f64;
        let d = (ph2 - ph1 + PI).rem_euclid(TAU) - PI;
        let e = (d / (w * h as f64)).clamp(-500e-6, 500e-6);
        ppm = Some(e * 1e6);
        baud = BAUD * (1.0 + e);
    }
    let (num, den, x) = line(p, baud, &mut s.d);
    let period = FS / baud;
    let wb = TAU * baud / FS;
    let n0 = (-x.arg() / wb).rem_euclid(period);
    (n0, period, ppm, 2.0 * num / (p.len().max(1) as f64 * den))
}

/// The symbol rate at the peak of the clock line of `p` between 48 and 52 Hz (0.05 Hz steps, parabolic refinement).
fn line_peak(p: &[f32], s: &mut Scratch) -> f64 {
    let fs: Vec<f64> = (0..=80).map(|i| 48.0 + 0.05 * i as f64).collect();
    let mags: Vec<f64> = fs.iter().map(|&f| line(p, f, &mut s.d).0).collect();
    let (k, _) = mags
        .iter()
        .enumerate()
        .fold((0, f64::MIN), |b, (i, &m)| if m > b.1 { (i, m) } else { b });
    if k == 0 || k == fs.len() - 1 {
        return fs[k];
    }
    let (a, b, c) = (mags[k - 1], mags[k], mags[k + 1]);
    let den = a - 2.0 * b + c;
    let dk = if den.abs() > 1e-30 {
        (0.5 * (a - c) / den).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    fs[k] + dk * 0.05
}

/// The step detector of order `m`: label `k` turns the phase by `2 pi k / m`.
fn detector(m: u32) -> Differential {
    let points = (0..m)
        .map(|k| Complex32::from_polar(1.0, (TAU * k as f64 / m as f64) as f32))
        .collect();
    PointSet::new(points, Shape::Psk { m })
        .ok()
        .and_then(|p| Differential::new(p).ok())
        .unwrap_or_else(Differential::dbpsk)
}

/// Demodulates the burst over the audio samples `[a, b)` with the comb centred on `centre_hz` (steps 4 to 7). `p` is the
/// summed carrier power of the whole audio.
fn demod_burst(x: &[f32], centre_hz: f64, a: usize, b: usize, s: &mut Scratch) -> Option<Burst> {
    let p0 = a.saturating_sub(HALF);
    let p1 = b.saturating_sub(HALF).min(s.power.len());
    if p1 <= p0 + 2 * PERIOD as usize {
        return None;
    }
    let pw = flatten(&s.power[p0..p1]);
    let (n0, period, ppm, share) = clock(&pw, s);
    if share < MIN_LINE {
        return None;
    }
    let rate = line_peak(&pw, s);
    let mut centres = Vec::new();
    let mut t = (p0 + HALF) as f64 + n0;
    while t < b as f64 {
        centres.push(t);
        t += period;
    }
    if centres.len() < 21 {
        return None;
    }
    let rect = vec![1.0f32; ACTIVE];
    let mut ds: Vec<Vec<(f64, f64)>> = Vec::with_capacity(CARRIERS);
    let mut syms: Vec<Vec<(f64, f64)>> = Vec::with_capacity(CARRIERS);
    let (mut r2, mut r4) = (0.0, 0.0);
    for c in 0..CARRIERS {
        let sy = symbols(x, grid_hz(centre_hz, c), &centres, &rect);
        let d = steps(&sy);
        r2 += conc(&d, 2).0;
        r4 += conc(&d, 4).0;
        ds.push(d);
        syms.push(sy);
    }
    r2 /= CARRIERS as f64;
    r4 /= CARRIERS as f64;
    // The eight carriers carry about the same power; one or two strong tones leak into their neighbours only.
    let power: Vec<f64> = syms
        .iter()
        .map(|sy| sy.iter().map(|v| v.0 * v.0 + v.1 * v.1).sum::<f64>())
        .collect();
    let strongest = power.iter().cloned().fold(0.0, f64::max);
    if power.iter().filter(|&&p| p >= EVEN_POWER * strongest).count() < EVEN_CARRIERS {
        return None;
    }
    let order = if r2 > 0.4 {
        2
    } else if r4 > 0.25 {
        4
    } else {
        return None;
    };
    let n_steps = centres.len() - 1;
    let mut out = Burst {
        start_s: a as f64 / FS,
        order,
        symbols: centres.len(),
        steps: vec![0u8; n_steps * CARRIERS],
        clock_ppm: ppm,
        symbol_rate_hz: rate,
        clock_line: share,
        ..Default::default()
    };
    let step_angle = TAU / order as f64;
    let (mut err2, mut n_err) = (0.0f64, 0usize);
    for c in 0..CARRIERS {
        let (r, ang) = conc(&ds[c], order);
        out.concentration[c] = r;
        let turn = ang / order as f64;
        out.carrier_hz[c] = grid_hz(centre_hz, c) + turn / TAU * FS / period;
        let mut det = detector(order);
        let back = Complex32::from_polar(1.0, -turn as f32);
        let mut rot = Complex32::new(1.0, 0.0);
        for (m, &(re, im)) in syms[c].iter().enumerate() {
            let z = Complex32::new(re as f32, im as f32) * rot;
            rot *= back;
            if m == 0 {
                det.set_reference(z);
                continue;
            }
            let (dre, dim) = ds[c][m - 1];
            let k = det.decide(z);
            out.steps[(m - 1) * CARRIERS + c] = k as u8;
            let e = (dim.atan2(dre) - turn - step_angle * f64::from(k) + PI).rem_euclid(TAU) - PI;
            err2 += e * e;
            n_err += 1;
        }
    }
    out.carriers_seen = out.concentration.iter().filter(|&&r| r >= 0.3).count();
    if out.carriers_seen < 5 {
        return None;
    }
    out.snr_db = 10.0 * (n_err as f64 / err2.max(1e-12)).log10();
    // The slope of the measured frequencies over the carrier number.
    let km = (CARRIERS as f64 - 1.0) / 2.0;
    let fm = out.carrier_hz.iter().sum::<f64>() / CARRIERS as f64;
    let (mut num, mut den) = (0.0, 0.0);
    for (k, f) in out.carrier_hz.iter().enumerate() {
        num += (k as f64 - km) * (f - fm);
        den += (k as f64 - km) * (k as f64 - km);
    }
    out.spacing_hz = if den > 0.0 { num / den } else { SPACING_HZ };
    Some(out)
}

/// Analyses the audio `x` around the nominal comb centre `nominal_hz`. Bursts that start before `after_s` (seconds into `x`)
/// are left out, and so is a burst that starts at the first frame when `whole` is false (its start is not in `x`).
pub fn analyse(x: &[f32], nominal_hz: f64, after_s: f64, whole: bool, s: &mut Scratch) -> Analysis {
    let Some(comb) = find_comb(x, nominal_hz, s) else {
        return Analysis::default();
    };
    carrier_power(x, comb.centre_hz, s);
    let mut bursts = Vec::new();
    let power = std::mem::take(&mut s.power);
    let segs = segments(&power, s);
    s.power = power;
    let trim = (1.5 * PERIOD) as usize;
    for (pa, pb) in segs {
        if pa == 0 && !whole {
            continue;
        }
        // Power index i is centred on audio sample i + HALF.
        let (a, b) = (pa + HALF, pb + HALF);
        if (a as f64 / FS) < after_s {
            continue;
        }
        let (ta, tb) = (a + trim, b.saturating_sub(trim));
        if tb <= ta || (tb - ta) as f64 <= 8.0 * PERIOD {
            continue;
        }
        if let Some(mut burst) = demod_burst(x, comb.centre_hz, ta, tb, s) {
            burst.start_s = a as f64 / FS;
            burst.length_s = (b - a) as f64 / FS;
            bursts.push(burst);
        }
    }
    Analysis {
        comb: Some(comb),
        bursts,
    }
}

/// The step indices of `steps` (eight a symbol time) as hexadecimal: for each of the first `times` symbol times, carrier 0 to
/// 7 in turn, each index written with `bits` bits, most significant first, four bits to a digit.
pub fn hex(steps: &[u8], bits: u32, times: usize) -> String {
    let mut out = String::new();
    let (mut acc, mut n) = (0u32, 0u32);
    for &k in steps.iter().take(times * CARRIERS) {
        for i in (0..bits).rev() {
            acc = (acc << 1) | ((u32::from(k) >> i) & 1);
            n += 1;
            if n == 4 {
                out.push(char::from_digit(acc, 16).unwrap_or('0').to_ascii_uppercase());
                acc = 0;
                n = 0;
            }
        }
    }
    if n > 0 {
        out.push(
            char::from_digit(acc << (4 - n), 16)
                .unwrap_or('0')
                .to_ascii_uppercase(),
        );
    }
    out
}
