//! The HF waveform identifier's measurements and candidates. Identification only: nothing is decoded, no
//! message content is read and no encryption is touched; the candidates are estimates from physical-layer features.

use crate::util::close_to;
use num_complex::Complex64;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::goertzel::goertzel;
use sdrkit_algo_dsp::peak::parabolic;
use sdrkit_algo_dsp::spectrum::{Scaling, Welch};
use sdrkit_algo_dsp::stats::{centred_mean, quantile_f64};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

/// The measured physical-layer features.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Features {
    pub fs: f64,
    pub signal: bool,
    pub band: Option<(f64, f64)>,
    pub band_snr_db: f64,
    pub floor_dbfs: f64,
    pub carrier_line: Option<(f64, f64)>,
    pub tone_605: Option<f64>,
    pub tone_2915: bool,
    /// Tone comb: spacing (Hz), regularity (0 - 1), tone count.
    pub comb: Option<(f64, f64, usize)>,
    pub shift_hz: Option<f64>,
    /// Clock line: baud and dB over its surroundings.
    pub clock: Option<(f64, f64)>,
    /// Slow envelope modulation: Hz and dB.
    pub env_mod: Option<(f64, f64)>,
    /// Bursts: count and duty cycle (0 bursts: continuous).
    pub bursts: usize,
    pub duty: f64,
    /// Single carrier: order (2, 4, 8, or 99 for higher or unclear).
    pub order: Option<u32>,
    /// Repetition: symbols, ms, correlation.
    pub repeat: Option<(usize, f64, f64)>,
}

impl Features {
    pub fn centre(&self) -> f64 {
        self.band.map_or(0.0, |b| (b.0 + b.1) / 2.0)
    }
    pub fn width(&self) -> f64 {
        self.band.map_or(0.0, |b| b.1 - b.0)
    }
}

/// One candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    /// The waveform.
    pub waveform: SignalClass,
    /// The confidence of the estimate, 0 to 0.95.
    pub confidence: f64,
    /// The measurements the score rests on, for people.
    pub basis: String,
}

fn fft_real(x: &[f32]) -> Vec<Complex32> {
    let mut b: Vec<Complex32> = x.iter().map(|&v| Complex32::new(v, 0.0)).collect();
    Fft::forward(b.len()).process(&mut b);
    b
}

/// The Welch spectrum (Hann, 50 % overlap) of a real signal in `n`-point frames: the mean `|X|^2` of bins 0 to n/2. A signal
/// shorter than one frame returns zeros.
fn welch_real(x: &[f32], n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n / 2 + 1];
    let Ok(mut w) = Welch::new(n, Window::Hann, 0.5, Scaling::Tone) else {
        return out;
    };
    let mut sp = vec![0.0f32; n];
    if w.compute_real(x, &mut sp).is_err() {
        return out;
    }
    // Tone scaling divides by (sum of the window)^2, which is (n / 2)^2 for Hann. The centred spectrum holds bin k at n/2 + k.
    let s = (n as f64 / 2.0).powi(2);
    for (k, o) in out.iter_mut().enumerate() {
        *o = sp[(n / 2 + k) % n] as f64 * s;
    }
    out
}

/// The occupied band from a 2048-point Welch spectrum.
fn occupied(x: &[f32], fs: f64) -> (Option<(f64, f64)>, f64, f64) {
    let n = 2048;
    let mut sp = Vec::new();
    centred_mean(&welch_real(x, n), 3, &mut sp);
    let bin = fs / n as f64;
    let lo = (50.0 / bin) as usize;
    let hi = ((0.45 * fs).min(6000.0) / bin) as usize;
    let floor = quantile_f64(&sp[lo..hi.min(sp.len())], 0.2, &mut Vec::new())
        .unwrap_or(0.0)
        .max(1e-30);
    let thr = floor * 10f64.powf(0.4);
    // Runs of signal bins, bridging gaps of up to 4 bins.
    let mut runs: Vec<(usize, usize, f64)> = Vec::new();
    let mut cur: Option<(usize, usize, f64)> = None;
    let mut gap = 0;
    for (i, &spi) in sp.iter().enumerate().take(hi.min(sp.len())).skip(lo) {
        if spi > thr {
            match cur.as_mut() {
                Some(c) => {
                    c.1 = i;
                    c.2 += spi - floor;
                }
                None => cur = Some((i, i, spi - floor)),
            }
            gap = 0;
        } else if let Some(c) = cur {
            gap += 1;
            if gap > 4 {
                runs.push(c);
                cur = None;
                gap = 0;
            }
        }
    }
    if let Some(c) = cur {
        runs.push(c);
    }
    let best = runs
        .into_iter()
        .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
    let floor_dbfs = 10.0 * (floor / (n as f64 * 3.0 / 8.0)).max(1e-30).log10();
    match best {
        Some((a, b, _)) if (b - a) as f64 * bin >= 150.0 => {
            let mean = sp[a..=b].iter().sum::<f64>() / (b - a + 1) as f64;
            let snr = 10.0 * (mean / floor - 1.0).max(1e-3).log10();
            (Some((a as f64 * bin, b as f64 * bin)), snr, floor_dbfs)
        }
        _ => (None, 0.0, floor_dbfs),
    }
}

/// Measures every feature of `x` (real audio at `fs`, a power of two of samples).
pub fn measure(x: &[f32], fs: f64) -> Features {
    let mut f = Features {
        fs,
        ..Default::default()
    };
    let (band, snr, floor_db) = occupied(x, fs);
    f.band = band;
    f.band_snr_db = snr;
    f.floor_dbfs = floor_db;
    // Steady lines (16 384 points).
    let n16 = 16_384.min(x.len());
    let sl = welch_real(x, n16);
    let b16 = fs / n16 as f64;
    let mut best_line: Option<(f64, f64)> = None;
    let mut scratch = Vec::new();
    let mut sorted = Vec::new();
    for i in 3..sl.len().saturating_sub(3) {
        let v = sl[i];
        if (i - 3..=i + 3).any(|j| j != i && sl[j] > v) {
            continue;
        }
        let a = i.saturating_sub(80);
        let b = (i + 80).min(sl.len() - 1);
        let med = quantile_f64(&sl[a..=b], 0.5, &mut scratch)
            .unwrap_or(0.0)
            .max(1e-30);
        let rel = 10.0 * (v / med).log10();
        let hz = i as f64 * b16;
        if rel >= 15.0 && best_line.is_none_or(|l| rel > l.1) {
            best_line = Some((hz, rel));
        }
        if rel >= 12.0 && (hz - 605.0).abs() <= 6.0 {
            f.tone_605 = Some(rel);
        }
        if rel >= 12.0 && (hz - 2915.0).abs() <= 6.0 {
            f.tone_2915 = true;
        }
    }
    f.carrier_line = best_line;
    f.signal = f.band.is_some() || f.carrier_line.is_some();
    if !f.signal {
        return f;
    }
    // The 4096-point spectrum for the comb and the shift.
    let mut s4 = Vec::new();
    centred_mean(&welch_real(x, 4096), 1, &mut s4);
    let b4 = fs / 4096.0;
    if let Some((lo, hi)) = f.band {
        let (a, b) = ((lo / b4) as usize, ((hi / b4) as usize).min(s4.len() - 1));
        if hi - lo >= 300.0 && b > a + 20 {
            let seg = &s4[a..=b];
            let mean = seg.iter().sum::<f64>() / seg.len() as f64;
            let d: Vec<f64> = seg.iter().map(|v| v - mean).collect();
            let r0: f64 = d.iter().map(|v| v * v).sum::<f64>().max(1e-30);
            let maxlag = 140.min(d.len() / 3);
            let ac: Vec<f64> = (0..=maxlag + 1)
                .map(|l| {
                    d.iter()
                        .zip(d.iter().skip(l))
                        .map(|(p, q)| p * q)
                        .sum::<f64>()
                        / r0
                })
                .collect();
            let mut best: Option<(usize, f64)> = None;
            for l in 6..maxlag.min(ac.len() - 1) {
                if ac[l] > ac[l - 1] && ac[l] >= ac[l + 1] && best.is_none_or(|b| ac[l] > b.1) {
                    best = Some((l, ac[l]));
                }
            }
            if let Some((l, v)) = best {
                let lag = l as f64 + parabolic(ac[l - 1], ac[l], ac[l + 1]).offset;
                let w = (3.0f64).max(0.4 * lag).round() as usize;
                let count = (0..seg.len())
                    .filter(|&i| {
                        let lo = i.saturating_sub(w);
                        let hi = (i + w).min(seg.len() - 1);
                        seg[i] > 1.1 * mean && (lo..=hi).all(|j| j == i || seg[j] <= seg[i])
                    })
                    .count();
                f.comb = Some((lag * b4, v.clamp(0.0, 1.0), count));
            }
        }
        if hi - lo < 700.0 {
            let (a, b) = ((lo / b4) as usize, ((hi / b4) as usize).min(s4.len() - 1));
            if b > a {
                let (i1, v1) =
                    (a..=b)
                        .map(|i| (i, s4[i]))
                        .fold((a, 0.0), |m, x| if x.1 > m.1 { x } else { m });
                let sep = (60.0 / b4).ceil() as usize;
                let second = (a..=b)
                    .filter(|&i| i.abs_diff(i1) >= sep)
                    .map(|i| (i, s4[i]))
                    .fold((a, 0.0), |m, x| if x.1 > m.1 { x } else { m });
                if second.1 > 0.35 * v1 {
                    f.shift_hz = Some(second.0.abs_diff(i1) as f64 * b4);
                }
            }
        }
    }
    // The analytic signal of the band and its envelope.
    let n = x.len();
    if let Some((lo, hi)) = f.band {
        let spec = fft_real(x);
        let binn = fs / n as f64;
        let (a, b) = (
            ((lo - 40.0).max(0.0) / binn) as usize,
            (((hi + 40.0) / binn) as usize).min(n / 2),
        );
        let mut an = vec![Complex32::new(0.0, 0.0); n];
        for i in a..=b {
            an[i] = spec[i] * 2.0;
        }
        Fft::inverse(n).process(&mut an);
        let scale = 1.0 / n as f32;
        let analytic: Vec<Complex32> = an.iter().map(|v| v * scale).collect();
        let env: Vec<f32> = analytic.iter().map(|v| v.norm_sqr()).collect();
        let mean = env.iter().map(|&v| v as f64).sum::<f64>() / n as f64;
        let ed: Vec<f32> = env.iter().map(|&v| (v as f64 - mean) as f32).collect();
        let es = fft_real(&ed);
        let pw: Vec<f64> = es[..n / 2].iter().map(|c| c.norm_sqr() as f64).collect();
        // The clock line, 40 Hz to min(3500, 0.4 fs).
        let i0 = (40.0 / binn) as usize;
        let i1 = ((3500.0f64).min(0.4 * fs) / binn) as usize;
        let mut best: Option<(usize, f64)> = None;
        for i in i0.max(1)..i1.min(pw.len() - 1) {
            let v = (pw[i - 1] + pw[i] + pw[i + 1]) / 3.0;
            if best.is_none_or(|b| v > b.1) {
                best = Some((i, v));
            }
        }
        if let Some((i, v)) = best {
            scratch.clear();
            let a = i.saturating_sub(400);
            let b = (i + 400).min(pw.len() - 1);
            for (j, &p) in pw.iter().enumerate().take(b + 1).skip(a) {
                if j.abs_diff(i) > 8 {
                    scratch.push(p);
                }
            }
            let med = quantile_f64(&scratch, 0.5, &mut sorted)
                .unwrap_or(0.0)
                .max(1e-30);
            let rel = 10.0 * (v / med).log10();
            if rel >= 10.0 {
                let fr = parabolic(
                    pw[i - 1].max(1e-30).ln(),
                    pw[i].max(1e-30).ln(),
                    pw[i + 1].max(1e-30).ln(),
                )
                .offset;
                f.clock = Some(((i as f64 + fr) * binn, rel));
            }
        }
        // Slow envelope modulation, 0.5 to 40 Hz against the median of 0.5 to 200 Hz.
        let (j0, j1, j2) = (
            ((0.5 / binn).ceil() as usize).max(1),
            (40.0 / binn) as usize,
            (200.0 / binn) as usize,
        );
        let med = quantile_f64(&pw[j0..j2.min(pw.len())], 0.5, &mut sorted)
            .unwrap_or(0.0)
            .max(1e-30);
        if let Some((i, v)) = (j0..j1.min(pw.len()))
            .map(|i| (i, pw[i]))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        {
            let rel = 10.0 * (v / med).log10();
            if rel >= 12.0 {
                f.env_mod = Some((i as f64 * binn, rel));
            }
        }
        // Bursts: energies of 20 ms windows.
        let w = (0.02 * fs) as usize;
        let en: Vec<f64> = env
            .chunks(w)
            .filter(|c| c.len() == w)
            .map(|c| c.iter().map(|&v| v as f64).sum())
            .collect();
        if !en.is_empty() {
            let mut s = Vec::new();
            let p10 = quantile_f64(&en, 0.1, &mut s).unwrap_or(0.0);
            let p90 = quantile_f64(&en, 0.9, &mut s).unwrap_or(0.0);
            if p90 > 2.0 * p10 {
                let mid = (p10 + p90) / 2.0;
                let on: Vec<bool> = en.iter().map(|&e| e > mid).collect();
                f.duty = on.iter().filter(|&&o| o).count() as f64 / on.len() as f64;
                f.bursts = on.windows(2).filter(|p| !p[0] && p[1]).count() + on[0] as usize;
            } else {
                f.duty = 1.0;
                f.bursts = 0;
            }
        }
        // Single carrier: the symbols at the clock, their order and repetition.
        if let Some((baud, _)) = f.clock {
            let width = hi - lo;
            if baud >= 100.0 && baud >= 0.5 * width && baud <= 1.1 * width {
                let (ord, rep) = single_carrier(&analytic, &env, fs, baud, (lo + hi) / 2.0);
                f.order = Some(ord);
                f.repeat = rep;
            }
        }
    }
    f
}

/// The order and the repetition of a single carrier's symbols.
fn single_carrier(
    an: &[Complex32],
    env: &[f32],
    fs: f64,
    baud: f64,
    centre: f64,
) -> (u32, Option<(usize, f64, f64)>) {
    let sps = fs / baud;
    // The phase of the envelope line determines the symbol timing.
    let w = TAU * baud / fs;
    let line = goertzel(env, baud / fs);
    let t0 = (-line.arg() / w).rem_euclid(sps);
    let mut best: (f64, u32, Vec<Complex64>) = (0.0, 99, Vec::new());
    let mut phase_lines = Vec::new();
    for q in 0..4 {
        let mut syms = Vec::new();
        let mut t = t0 + q as f64 * sps / 4.0;
        while t + 1.0 < an.len() as f64 {
            let i = t as usize;
            let fr = t - i as f64;
            let a = an[i] + (an[i + 1] - an[i]) * fr as f32;
            let p = -TAU * centre * t / fs;
            let z = Complex64::new(a.re as f64, a.im as f64) * Complex64::new(p.cos(), p.sin());
            let m = z.norm();
            syms.push(if m > 0.0 {
                z / m
            } else {
                Complex64::new(0.0, 0.0)
            });
            t += sps;
        }
        if syms.len() < 64 {
            continue;
        }
        let mut lines = [0.0; 3];
        for (k, m) in [2u32, 4, 8].iter().enumerate() {
            let n = syms.len().next_power_of_two();
            let mut b = vec![Complex32::new(0.0, 0.0); n];
            for (i, s) in syms.iter().enumerate() {
                let p = s.powu(*m);
                b[i] = Complex32::new(p.re as f32, p.im as f32);
            }
            Fft::forward(n).process(&mut b);
            let pw: Vec<f64> = b.iter().map(|c| c.norm_sqr() as f64).collect();
            let mean = pw.iter().sum::<f64>() / n as f64;
            let peak = pw.iter().cloned().fold(0.0, f64::max);
            lines[k] = peak / mean.max(1e-30);
        }
        let strongest = lines.iter().cloned().fold(0.0, f64::max);
        phase_lines.push(lines);
        if strongest > best.0 {
            best = (strongest, 0, syms);
            best.1 = {
                let thr = (40.0f64).max(0.4 * strongest);
                [2u32, 4, 8]
                    .iter()
                    .zip(lines.iter())
                    .find(|(_, &l)| l >= thr)
                    .map_or(99, |(m, _)| *m)
            };
        }
    }
    let syms = best.2;
    if syms.is_empty() {
        return (99, None);
    }
    // The circular autocorrelation of the symbols.
    let n = syms.len().next_power_of_two() * 2;
    let mut b = vec![Complex32::new(0.0, 0.0); n];
    for (i, s) in syms.iter().enumerate() {
        b[i] = Complex32::new(s.re as f32, s.im as f32);
    }
    Fft::forward(n).process(&mut b);
    for v in b.iter_mut() {
        *v = Complex32::new(v.norm_sqr(), 0.0);
    }
    Fft::inverse(n).process(&mut b);
    let ac: Vec<f64> = b.iter().map(|c| c.norm() as f64).collect();
    let r0 = ac[0].max(1e-30);
    let maxlag = 2048.min(syms.len() / 4);
    let mut rep = None;
    if maxlag > 8 {
        let seg = &ac[8..=maxlag];
        let (pi, pv) = seg
            .iter()
            .enumerate()
            .fold((0, 0.0), |m, (i, &v)| if v > m.1 { (i, v) } else { m });
        let med = quantile_f64(seg, 0.5, &mut Vec::new()).unwrap_or(0.0);
        if pv > 0.08 * r0 && pv > 5.0 * med {
            let first = seg.iter().position(|&v| v >= 0.7 * pv).unwrap_or(pi);
            let lag = first + 8;
            rep = Some((lag, lag as f64 / baud * 1000.0, pv / r0));
        }
    }
    (best.1, rep)
}

sdrkit_core::choice_enum! {
    /// The signal classes that the identifier can name.
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum SignalClass {
        /// 2400 Bd serial-tone PSK family.
        Serial2400 = ("serial_2400", "2400 Bd serial-tone PSK family"),
        /// STANAG 4285.
        Stanag4285 = ("stanag_4285", "STANAG 4285"),
        /// MIL-STD-188-110A serial tone.
        Mil110a = ("mil_110a", "MIL-STD-188-110A serial tone"),
        /// MIL-STD-188-110B / STANAG 4539 high rate.
        Mil110b = ("mil_110b", "MIL-STD-188-110B / STANAG 4539 high rate"),
        /// STANAG 4481 PSK: the STANAG 4285 waveform on a naval broadcast.
        Stanag4481 = ("stanag_4481", "STANAG 4481 PSK (unverified match)"),
        /// 3G-ALE burst (8-PSK, 2400 Bd).
        Ale3g = ("ale_3g", "3G-ALE burst (8-PSK, 2400 Bd)"),
        /// Link 11 CLEW (parallel tones).
        Link11 = ("link_11", "Link 11 CLEW (parallel tones)"),
        /// 2G-ALE (8-FSK, 125 Bd).
        Ale2g = ("ale_2g", "2G-ALE (8-FSK, 125 Bd)"),
        /// CIS-36 (36-tone MFSK).
        Cis36 = ("cis_36", "CIS-36 (36-tone MFSK)"),
        /// Multi-tone MFSK (CIS-12 / Chinese-style).
        MultiTone = ("multi_tone", "Multi-tone MFSK (CIS-12 / Chinese-style, unverified)"),
        /// PACTOR-I / 200 Bd FSK (200 Hz shift).
        Pactor1 = ("pactor1", "PACTOR-I / 200 Bd FSK (200 Hz shift)"),
        /// PACTOR-II-like (100 Bd DPSK, narrow).
        Pactor2 = ("pactor2", "PACTOR-II-like (100 Bd DPSK, narrow)"),
        /// Buzzer (UVB-76 style: carrier with a periodic buzz).
        Buzzer = ("buzzer", "Buzzer (UVB-76 style: carrier with a periodic buzz)"),
        /// AM voice / numbers-station style carrier.
        Carrier = ("carrier", "AM voice / numbers-station style carrier"),
        /// Unclassified signal.
        Unclassified = ("unclassified", "Unclassified signal"),
    }
}

impl SignalClass {
    /// The name for people.
    pub fn name(self) -> &'static str {
        use sdrkit_core::Choice;
        match &Self::CHOICES[usize::from(self.index())].label {
            std::borrow::Cow::Borrowed(s) => s,
            std::borrow::Cow::Owned(_) => "",
        }
    }

    /// What the waveform is.
    pub fn note(self) -> &'static str {
        match self {
            SignalClass::Serial2400 => "Single-carrier 2400 symbols/s on an 1800 Hz sub-carrier, shared by STANAG 4285, MIL-STD-188-110A/B, STANAG 4539 and others; the repeat period and constellation separate them.",
            SignalClass::Stanag4285 => "2400 Bd, BPSK/QPSK/8-PSK, 256-symbol frames (80 known + data) at 106.7 ms; STANAG 5066 users often ride on it (physical layer only).",
            SignalClass::Mil110a => "US serial-tone modem, 2400 Bd 8-PSK, known probe symbols between data blocks.",
            SignalClass::Mil110b => "The same tone with higher-order QAM up to 12.8 kbit/s.",
            SignalClass::Stanag4481 => "The PSK part of a STANAG 4481 naval broadcast is the continuous STANAG 4285 waveform; its FSK part is read by the STANAG 4481 FSK stack. The match is not verified.",
            SignalClass::Ale3g => "Third-generation ALE uses the same 2400 Bd 8-PSK in short bursts; see the HF ALE stack for 2G.",
            SignalClass::Link11 => "A comb of DPSK tones with a Doppler tone at 605 Hz and a sync tone at 2915 Hz; SLEW is a different, serial waveform.",
            SignalClass::Ale2g => "The decoder is the HF ALE stack.",
            SignalClass::Cis36 => "A 36-tone MFSK system, 40 Hz apart; the CIS HF stack identifies it (submode cis36). CIS-36-50 (BEE) is a different, 2-FSK signal.",
            SignalClass::MultiTone => "A comb of FSK tones; which system is not verified.",
            SignalClass::Pactor1 => "Identification by shift only; open the PACTOR-I stack to try to read it. PACTOR-II to IV have their physical-layer stacks.",
            SignalClass::Pactor2 => "A 100 baud differential PSK signal; the PACTOR-II stack measures it.",
            SignalClass::Buzzer => "A carrier with a periodic buzz.",
            SignalClass::Carrier => "A steady carrier.",
            SignalClass::Unclassified => "Something is there, but none of the known physical-layer patterns match.",
        }
    }

    /// A public source about the waveform (empty for none).
    pub fn link(self) -> &'static str {
        match self {
            SignalClass::Serial2400 => "https://en.wikipedia.org/wiki/MIL-STD-188",
            SignalClass::Stanag4285 => concat!("https://www.sigidwiki.com/wiki/", "STANAG_4285"),
            SignalClass::Mil110a => "https://en.wikipedia.org/wiki/MIL-STD-188",
            SignalClass::Mil110b => "https://en.wikipedia.org/wiki/MIL-STD-188",
            SignalClass::Stanag4481 => {
                concat!("https://www.sigidwiki.com/wiki/", "Category:Military")
            }
            SignalClass::Ale3g => "https://en.wikipedia.org/wiki/Automatic_link_establishment",
            SignalClass::Link11 => "https://en.wikipedia.org/wiki/Link_11",
            SignalClass::Ale2g => "https://en.wikipedia.org/wiki/Automatic_link_establishment",
            SignalClass::Cis36 => concat!("https://www.sigidwiki.com/wiki/", "CIS-36"),
            SignalClass::MultiTone => {
                concat!("https://www.sigidwiki.com/wiki/", "Category:Military")
            }
            SignalClass::Pactor1 => "https://en.wikipedia.org/wiki/PACTOR",
            SignalClass::Pactor2 => "https://en.wikipedia.org/wiki/PACTOR",
            SignalClass::Buzzer => concat!("https://www.sigidwiki.com/wiki/", "UVB-76"),
            SignalClass::Carrier => concat!("https://www.sigidwiki.com/wiki/", "Category:Military"),
            SignalClass::Unclassified => "",
        }
    }
}

/// Ranks the candidates (at least 0.15, capped at 0.95, at most 6, best first).
pub fn candidates(f: &Features) -> Vec<Candidate> {
    let mut v: Vec<Candidate> = Vec::new();
    if !f.signal {
        return v;
    }
    let mut add = |waveform: SignalClass, score: f64, basis: String| {
        v.push(Candidate {
            waveform,
            confidence: score.min(0.95),
            basis,
        });
    };
    let (lo, hi) = f.band.unwrap_or((0.0, 0.0));
    let width = hi - lo;
    let centre = f.centre();
    let comb_ok = f.comb.filter(|c| c.1 >= 0.30 && c.2 >= 3);
    // The single-carrier family.
    if let (Some(order), Some((baud, cdb))) = (f.order, f.clock) {
        if width >= 1000.0 {
            let base = 0.40 * close_to(baud, 2400.0, 80.0)
                + 0.20 * close_to(centre, 1800.0, 200.0)
                + 0.15 * ((2000.0..=3300.0).contains(&width) as u8 as f64)
                + 0.15 * (matches!(order, 2 | 4 | 8) as u8 as f64)
                + 0.10 * ((cdb >= 15.0) as u8 as f64);
            if (baud - 2400.0).abs() <= 80.0 {
                let rep = f.repeat.map(|r| r.0);
                let basis = format!(
                    "{baud:.0} Bd, band {lo:.0} - {hi:.0} Hz (centre {centre:.0}), order {}, repeat {}",
                    if order == 99 { "high/unclear".to_string() } else { order.to_string() },
                    rep.map_or("none".to_string(), |r| format!("{r} sym"))
                );
                add(SignalClass::Serial2400, base * 0.9, basis.clone());
                add(
                    SignalClass::Stanag4285,
                    base * if rep.is_some_and(|r| (r as i64 - 256).abs() <= 1) {
                        1.0
                    } else {
                        0.45
                    },
                    basis.clone(),
                );
                add(
                    SignalClass::Mil110a,
                    base * if rep.is_some_and(|r| r == 48 || r == 96) {
                        1.0
                    } else {
                        0.40
                    },
                    basis.clone(),
                );
                add(
                    SignalClass::Mil110b,
                    base * if order == 99 { 0.80 } else { 0.22 },
                    basis.clone(),
                );
                add(
                    SignalClass::Stanag4481,
                    base * if f.duty < 0.7 { 0.35 } else { 0.12 },
                    basis.clone(),
                );
                add(
                    SignalClass::Ale3g,
                    base * if f.bursts >= 1 && f.duty < 0.85 {
                        0.70
                    } else {
                        0.10
                    },
                    basis,
                );
            }
        }
    }
    // Link 11 CLEW.
    let comb110 = f.comb.filter(|c| (c.0 - 110.0).abs() <= 8.0);
    if f.tone_605.is_some() || comb110.is_some() {
        let (sp, tones) = f.comb.map_or((0.0, 0), |c| (c.0, c.2));
        let s = 0.35 * f.tone_605.is_some() as u8 as f64
            + 0.30 * close_to(sp, 110.0, 12.0)
            + 0.15 * ((12..=22).contains(&tones) as u8 as f64)
            + 0.10 * comb_ok.is_some() as u8 as f64
            + 0.10 * f.tone_2915 as u8 as f64;
        add(
            SignalClass::Link11,
            s,
            format!(
                "{tones} tones {sp:.1} Hz apart{}{}",
                if f.tone_605.is_some() {
                    ", 605 Hz tone"
                } else {
                    ""
                },
                if f.tone_2915 { ", 2915 Hz tone" } else { "" }
            ),
        );
    }
    if let Some((sp, reg, tones)) = f.comb {
        if (sp - 250.0).abs() <= 35.0 {
            let s = 0.40 * close_to(sp, 250.0, 35.0)
                + 0.25 * ((6..=9).contains(&tones) as u8 as f64)
                + 0.15 * ((1500.0..=2400.0).contains(&width) as u8 as f64)
                + 0.20 * (f.bursts > 0) as u8 as f64;
            add(
                SignalClass::Ale2g,
                s,
                format!("{tones} tones {sp:.0} Hz apart"),
            );
        }
        if tones >= 28 && (sp - 40.0).abs() <= 8.0 {
            add(
                SignalClass::Cis36,
                0.35 + 0.35 * close_to(sp, 40.0, 8.0) + 0.2 * close_to(tones as f64, 36.0, 10.0),
                format!("{tones} tones {sp:.1} Hz apart"),
            );
        }
        if (8..=16).contains(&tones) && (sp - 110.0).abs() > 8.0 && (sp - 250.0).abs() > 35.0 {
            add(
                SignalClass::MultiTone,
                0.20 + 0.45 * reg.min(1.0),
                format!("{tones} tones {sp:.1} Hz apart, regularity {reg:.2}"),
            );
        }
    }
    if let Some(shift) = f.shift_hz {
        if width < 700.0 {
            add(
                SignalClass::Pactor1,
                0.55 * close_to(shift, 200.0, 40.0) + 0.1,
                format!("two peaks {shift:.0} Hz apart"),
            );
        }
    }
    if let Some((baud, _)) = f.clock {
        if (baud - 100.0).abs() <= 15.0 && width < 1200.0 {
            add(
                SignalClass::Pactor2,
                0.30 + 0.2 * close_to(baud, 100.0, 15.0),
                format!("{baud:.1} Bd, band {lo:.0} - {hi:.0} Hz"),
            );
        }
    }
    if let Some((_, line)) = f.carrier_line {
        if line >= 15.0 && f.order.is_none() && comb_ok.is_none() {
            if let Some((mf, md)) = f.env_mod {
                if md >= 12.0 && mf >= 1.0 {
                    add(
                        SignalClass::Buzzer,
                        0.30 + 0.30 * ((line - 15.0) / 20.0).min(1.0)
                            + 0.15 * ((md - 12.0) / 15.0).min(1.0),
                        format!("carrier line {line:.0} dB, envelope {mf:.1} Hz"),
                    );
                }
            }
            add(
                SignalClass::Carrier,
                0.30 + 0.2 * ((line - 15.0) / 20.0).min(1.0),
                format!("carrier line {line:.0} dB"),
            );
        }
    }
    v.retain(|c| c.confidence >= 0.15);
    v.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    v.truncate(6);
    if v.is_empty() {
        v.push(Candidate {
            waveform: SignalClass::Unclassified,
            confidence: 0.10,
            basis: String::new(),
        });
    }
    v
}

/// The measured-parameter lines of the screen.
pub fn describe(f: &Features) -> Vec<String> {
    let mut v = Vec::new();
    if let Some((lo, hi)) = f.band {
        v.push(format!(
            "Occupied band: {lo:.0} - {hi:.0} Hz ({:.0} Hz wide, centre {:.0} Hz), SNR {:.1} dB",
            hi - lo,
            f.centre(),
            f.band_snr_db
        ));
    }
    match f.clock {
        Some((b, d)) => v.push(format!(
            "Clock line: {b:.1} Bd ({d:.0} dB over its surroundings)"
        )),
        None => v.push("Clock line: none found".into()),
    }
    match f.order {
        Some(99) => v.push("Constellation guess at that clock: higher order or unclear".into()),
        Some(o) => v.push(format!(
            "Constellation guess at that clock: {o}-PSK (rough)"
        )),
        None => {}
    }
    if let Some((s, ms, c)) = f.repeat {
        v.push(format!(
            "Repetition period: {s} symbols ({ms:.1} ms), correlation {c:.2}"
        ));
    }
    if let Some((sp, reg, n)) = f.comb {
        if reg >= 0.3 {
            v.push(format!(
                "Tone comb: {n} peaks, spacing {sp:.1} Hz (regularity {reg:.2})"
            ));
        }
    }
    if let Some(s) = f.shift_hz {
        v.push(format!("Two peaks {s:.0} Hz apart"));
    }
    if let Some(d) = f.tone_605 {
        v.push(format!(
            "Steady tone at 605 Hz ({d:.0} dB){}",
            if f.tone_2915 { ", and at 2915 Hz" } else { "" }
        ));
    }
    if let Some((hz, d)) = f.carrier_line {
        v.push(format!("Carrier line: {hz:.1} Hz ({d:.0} dB)"));
    }
    if let Some((hz, d)) = f.env_mod {
        v.push(format!("Envelope modulation: {hz:.1} Hz ({d:.0} dB)"));
    }
    if f.bursts > 0 {
        v.push(format!(
            "Bursts: {}, on {:.0} % of the time",
            f.bursts,
            f.duty * 100.0
        ));
    } else {
        v.push("Continuous (no bursts)".into());
    }
    v
}
