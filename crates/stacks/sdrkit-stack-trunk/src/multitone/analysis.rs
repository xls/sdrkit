//! The measurements of one Multitone transmission: deviation, spectral lines, steady tones, a repeating pattern, and the FSK
//! measurements (audio FFSK, two-level FSK, bit analysis, framing, verdict). No message text is produced.

use num_complex::Complex32;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_proto_pager::{pocsag, word};
use std::f64::consts::TAU;

/// Discriminator rate.
pub const RATE: f64 = 48000.0;

/// A steady tone heard.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tone {
    /// Hz.
    pub hz: f64,
    /// Seconds.
    pub seconds: f64,
    /// Start within the transmission, s.
    pub start: f64,
}

/// A spectral line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    /// Hz.
    pub hz: f64,
    /// dB over the band's median.
    pub db: f64,
}

/// `multitone.burst`: the measurements of an FSK data burst.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Burst {
    /// Audio FFSK or two-level FSK.
    pub kind: BurstKind,
    /// Symbol rate.
    pub baud: f64,
    /// The nearest standard rate within 1.5 % (0 when none).
    pub standard: u32,
    /// Audio FFSK tones (low, high), Hz.
    pub tones: (f64, f64),
    /// 1 − misfit share (FFSK) or the edge coherence (FSK).
    pub coherence: f64,
    /// Bits.
    pub bits: usize,
    /// Manchester decoded.
    pub manchester: bool,
    /// The first bytes, hex.
    pub hex: String,
    /// POCSAG sync found at this bit.
    pub pocsag_sync: Option<usize>,
    /// Count of the 16-bit word 605A.
    pub w605a: usize,
    /// Its first offsets.
    pub w605a_at: Vec<usize>,
    /// The words after the first double 605A (or the first one).
    pub after_605a: Vec<String>,
    /// The bit pattern repeats every so many bits (0: no).
    pub repeat: usize,
    /// Valid BCH(31,21)+parity words at the best alignment, the words there, the longest run, the offset.
    pub bch_valid: usize,
    /// Words at that alignment.
    pub bch_total: usize,
    /// The longest run of valid words.
    pub bch_run: usize,
    /// The bit offset.
    pub bch_offset: usize,
    /// Inverted stream.
    pub bch_inverted: bool,
    /// Up to 32 words with their validity.
    pub words: Vec<(String, bool)>,
    /// Every bit sent twice.
    pub doubled: bool,
    /// A repeating marker: length, pattern, count, gap.
    pub marker: Option<(usize, String, usize, usize)>,
    /// Frame words after the markers: distinct, repeating the one before, a first byte and a value sent twice.
    pub frame_words: (usize, usize, usize),
    /// Frame words, hex (up to 64).
    pub frame_list: Vec<String>,
    /// The verdict.
    pub verdict: String,
    /// Signal time, s.
    pub time: f64,
    /// Serial.
    pub id: u64,
}

impl sdrkit_graph::Item for Burst {
    const KIND: &'static str = "multitone.burst";
}

sdrkit_core::choice_enum! {
    /// The modulation of an FSK burst.
    #[derive(Default)]
    pub enum BurstKind {
        /// Audio FFSK.
        #[default]
        Ffsk = ("ffsk", "FFSK"),
        /// Two-level FSK.
        Fsk = ("fsk", "FSK"),
    }
}

sdrkit_core::choice_enum! {
    /// What a transmission holds.
    #[derive(Default)]
    pub enum TxKind {
        /// Audio FFSK data.
        FfskData = ("ffsk_data", "FFSK data"),
        /// Two-level FSK data.
        FskData = ("fsk_data", "FSK data"),
        /// A sequence of steady tones.
        ToneSequence = ("tone_sequence", "Tone sequence"),
        /// Multi-tone audio that repeats.
        Repeating = ("repeating", "Repeating multi-tone audio"),
        /// Multi-tone audio.
        MultiTone = ("multi_tone", "Multi-tone audio"),
        /// Anything else.
        #[default]
        Other = ("other", "Other"),
    }
}

/// `multitone.tx`: one measured transmission.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Transmission {
    /// Signal time of the start, s.
    pub time: f64,
    /// Seconds.
    pub duration: f64,
    /// Peak SNR, dB.
    pub snr_db: f64,
    /// Deviation, Hz.
    pub deviation_hz: f64,
    /// What it holds.
    pub kind: TxKind,
    /// Steady tones.
    pub tones: Vec<Tone>,
    /// the tones as text, "{Hz} Hz {s} s" for each.
    pub tones_text: String,
    /// Spectral lines.
    pub lines: Vec<Line>,
    /// Repeating period, ms (0: none) and its strength.
    pub period_ms: f64,
    /// Its strength.
    pub period_strength: f64,
    /// The burst's id (0: none).
    pub burst: u64,
}

impl sdrkit_graph::Item for Transmission {
    const KIND: &'static str = "multitone.tx";
}

fn pct(v: &[f32], p: f64) -> f32 {
    let mut s = v.to_vec();
    s.sort_unstable_by(|a, b| a.total_cmp(b));
    if s.is_empty() {
        return 0.0;
    }
    s[((s.len() - 1) as f64 * p).round() as usize]
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    if s.is_empty() {
        0.0
    } else {
        s[s.len() / 2]
    }
}

/// The 8 kHz audio of a piece (mean of 6), mean removed, in Hz.
pub fn audio8k(disc: &[f32]) -> Vec<f32> {
    let k = (RATE / TAU) as f32;
    let mut a: Vec<f32> = disc
        .as_chunks::<6>()
        .0
        .iter()
        .map(|c| c.iter().sum::<f32>() / 6.0 * k)
        .collect();
    let m = a.iter().sum::<f32>() / a.len().max(1) as f32;
    a.iter_mut().for_each(|v| *v -= m);
    a
}

/// Spectra of the audio: 512-sample Hann windows zero-padded to 1024 points, hop 256. Power per bin (513 bins, 7.8125 Hz).
pub fn spectra(audio: &[f32]) -> Vec<Vec<f64>> {
    let mut fft = Fft::forward(1024);
    let mut w = vec![0.0f32; 512];
    Window::Hann.fill_symmetric(&mut w);
    let mut out = Vec::new();
    let mut buf = vec![Complex32::new(0.0, 0.0); 1024];
    let mut s = 0;
    while s + 512 <= audio.len() {
        buf.iter_mut().for_each(|z| *z = Complex32::new(0.0, 0.0));
        for i in 0..512 {
            buf[i] = Complex32::new(audio[s + i] * w[i], 0.0);
        }
        fft.process(&mut buf);
        out.push((0..=512).map(|k| buf[k].norm_sqr() as f64).collect());
        s += 256;
    }
    out
}

const BIN: f64 = 8000.0 / 1024.0;

fn parabolic(p: &[f64], k: usize) -> f64 {
    if k == 0 || k + 1 >= p.len() {
        return k as f64 * BIN;
    }
    let (a, b, c) = (
        p[k - 1].max(1e-30).ln(),
        p[k].max(1e-30).ln(),
        p[k + 1].max(1e-30).ln(),
    );
    let den = a - 2.0 * b + c;
    let d = if den.abs() > 1e-12 {
        0.5 * (a - c) / den
    } else {
        0.0
    };
    (k as f64 + d.clamp(-0.5, 0.5)) * BIN
}

/// The spectral lines of 9.3.
pub fn lines(sp: &[Vec<f64>]) -> Vec<Line> {
    if sp.len() < 4 {
        return vec![];
    }
    let lo = (60.0 / BIN).ceil() as usize;
    let hi = (3300.0 / BIN).floor() as usize;
    let avg: Vec<f64> = (0..=512)
        .map(|k| sp.iter().map(|s| s[k]).sum::<f64>() / sp.len() as f64)
        .collect();
    let band: Vec<f64> = avg[lo..=hi].to_vec();
    let med = median(&band).max(1e-30);
    let mut cands: Vec<(f64, usize)> = Vec::new();
    for k in lo + 3..=hi - 3 {
        if (k - 3..=k + 3).all(|j| j == k || avg[j] < avg[k]) && avg[k] >= med * 10f64.powf(1.2) {
            cands.push((avg[k], k));
        }
    }
    cands.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out: Vec<Line> = Vec::new();
    for (p, k) in cands {
        let f = parabolic(&avg, k);
        if out.iter().all(|l| (l.hz - f).abs() >= 30.0) {
            out.push(Line {
                hz: f,
                db: 10.0 * (p / med).log10(),
            });
            if out.len() == 8 {
                break;
            }
        }
    }
    out
}

/// The steady tones of 9.3.
pub fn steady_tones(sp: &[Vec<f64>]) -> Vec<Tone> {
    let lo = (60.0 / BIN).ceil() as usize;
    let hi = (3300.0 / BIN).floor() as usize;
    let hop = 256.0 / 8000.0;
    let mut per: Vec<Option<f64>> = Vec::with_capacity(sp.len());
    for s in sp {
        let band = &s[lo..=hi];
        let med = median(band).max(1e-30);
        let total: f64 = band.iter().sum();
        let (mut k, mut best) = (lo, 0.0);
        for (j, &v) in s.iter().enumerate().take(hi + 1).skip(lo) {
            if v > best {
                best = v;
                k = j;
            }
        }
        let near: f64 = s[k.saturating_sub(3)..=(k + 3).min(512)].iter().sum();
        let ok = best >= med * 10f64.powf(1.5) && near >= 0.4 * total && k > lo + 3 && k + 3 < hi;
        per.push(ok.then(|| parabolic(s, k)));
    }
    let mut tones = Vec::new();
    let mut run: Vec<(usize, f64)> = Vec::new();
    let mut missed = 0;
    let close = |run: &mut Vec<(usize, f64)>, tones: &mut Vec<Tone>| {
        if run.len() >= 3 {
            let mean = run.iter().map(|r| r.1).sum::<f64>() / run.len() as f64;
            let first = run[0].0;
            let last = run[run.len() - 1].0;
            tones.push(Tone {
                hz: mean,
                seconds: (last - first + 1) as f64 * hop + 0.032,
                start: first as f64 * hop,
            });
        }
        run.clear();
    };
    for (i, f) in per.iter().enumerate() {
        match f {
            Some(f) => {
                let mean = if run.is_empty() {
                    *f
                } else {
                    run.iter().map(|r| r.1).sum::<f64>() / run.len() as f64
                };
                if run.is_empty() || (f - mean).abs() <= 14.0 {
                    run.push((i, *f));
                    missed = 0;
                } else {
                    close(&mut run, &mut tones);
                    run.push((i, *f));
                    missed = 0;
                }
            }
            None => {
                missed += 1;
                if missed > 1 {
                    close(&mut run, &mut tones);
                }
            }
        }
    }
    close(&mut run, &mut tones);
    tones
}

/// The repeating pattern of 9.3: (period ms, strength), (0, 0) when none.
pub fn period(audio: &[f32]) -> (f64, f64) {
    let n = audio.len().min(16000);
    let a = &audio[..n];
    if n < 800 {
        return (0.0, 0.0);
    }
    let e0: f64 = a.iter().map(|&x| (x * x) as f64).sum();
    if e0 <= 0.0 {
        return (0.0, 0.0);
    }
    let lags: Vec<usize> = (80..=3200.min(n - 1)).collect();
    let r: Vec<f64> = lags
        .iter()
        .map(|&l| {
            let s: f64 = (0..n - l).map(|i| (a[i] * a[i + l]) as f64).sum();
            s / e0 * n as f64 / (n - l) as f64
        })
        .collect();
    let peaks: Vec<usize> = (1..r.len().saturating_sub(1))
        .filter(|&i| r[i] > r[i - 1] && r[i] >= r[i + 1])
        .collect();
    let Some(&best) = peaks.iter().max_by(|&&a, &&b| r[a].total_cmp(&r[b])) else {
        return (0.0, 0.0);
    };
    if r[best] < 0.35 {
        return (0.0, 0.0);
    }
    let first = peaks
        .iter()
        .copied()
        .find(|&i| r[i] >= 0.8 * r[best])
        .unwrap_or(best);
    (lags[first] as f64 / 8.0, r[first])
}

/// Audio FFSK: the bits, the baud, the tones (low, high) and the coherence.
pub type FfskReading = (Vec<bool>, f64, (f64, f64), f64);

/// Audio FFSK: bits, baud, tones, coherence. `None` when it is not FFSK data.
pub fn audio_ffsk(disc: &[f32]) -> Option<FfskReading> {
    let k = (RATE / 3000.0).round() as usize;
    let box1 = boxcar(disc, k);
    let x = boxcar(&box1, k);
    if x.len() < 1000 {
        return None;
    }
    let med = pct(&x, 0.5);
    let range = pct(&x, 0.95) - pct(&x, 0.05);
    let hyst = 0.12 * range;
    let mut crossings = Vec::new();
    let mut state = x[0] > med;
    for (i, &v) in x.iter().enumerate() {
        if state && v < med - hyst / 2.0 {
            state = false;
            crossings.push(i);
        } else if !state && v > med + hyst / 2.0 {
            state = true;
            crossings.push(i);
        }
    }
    let iv: Vec<f64> = crossings.windows(2).map(|w| (w[1] - w[0]) as f64).collect();
    if iv.len() < 40 {
        return None;
    }
    let ivf: Vec<f32> = iv.iter().map(|&v| v as f32).collect();
    let p10 = pct(&ivf, 0.10) as f64;
    let short: Vec<f64> = iv
        .iter()
        .copied()
        .filter(|&v| v >= 0.75 * p10 && v <= 1.3 * p10)
        .collect();
    let u = median(&short);
    if u <= 0.0 {
        return None;
    }
    let is_u = |v: f64| (v - u).abs() <= 0.3 * u;
    let is_2u = |v: f64| (v - 2.0 * u).abs() <= 0.4 * u;
    let nu = iv.iter().filter(|&&v| is_u(v)).count();
    let n2 = iv.iter().filter(|&&v| is_2u(v)).count();
    let n = iv.len();
    if (nu + n2) * 100 < n * 85 || nu * 100 < n * 15 || n2 * 100 < n * 10 {
        return None;
    }
    // Between two 2u intervals the u intervals come in pairs.
    let (mut runs, mut even) = (0, 0);
    let mut cnt = 0;
    let mut seen2 = false;
    for &v in &iv {
        if is_2u(v) {
            if seen2 && cnt > 0 {
                runs += 1;
                if cnt % 2 == 0 {
                    even += 1;
                }
            }
            seen2 = true;
            cnt = 0;
        } else if is_u(v) {
            cnt += 1;
        }
    }
    if runs < 10 || even * 10 < runs * 9 {
        return None;
    }
    // Symbols: 2u = 0, u u = 1. The start parity with fewer misfits wins.
    let mut best: Option<(Vec<bool>, usize)> = None;
    for start in 0..2 {
        let mut bits = Vec::new();
        let mut mis = 0;
        let mut i = start;
        while i < iv.len() {
            if is_2u(iv[i]) {
                bits.push(false);
                i += 1;
            } else if is_u(iv[i]) && i + 1 < iv.len() && is_u(iv[i + 1]) {
                bits.push(true);
                i += 2;
            } else {
                mis += 1;
                i += 1;
            }
        }
        if best.as_ref().is_none_or(|b| mis < b.1) {
            best = Some((bits, mis));
        }
    }
    let (bits, mis) = best?;
    let changes = bits.windows(2).filter(|w| w[0] != w[1]).count();
    if bits.len() < 64 || mis * 100 > (bits.len() + mis) * 3 || changes * 10 < bits.len() {
        return None;
    }
    let us = u / RATE;
    let coherence = 1.0 - mis as f64 / (bits.len() + mis) as f64;
    Some((
        bits,
        1.0 / (2.0 * us),
        (1.0 / (4.0 * us), 1.0 / (2.0 * us)),
        coherence,
    ))
}

/// The mean of the last `k` values up to each value (fewer at the start). The window trails the value, so this is not
/// `sdrkit_algo_dsp::stats::centred_mean`.
fn boxcar(x: &[f32], k: usize) -> Vec<f32> {
    let k = k.max(1);
    let mut out = Vec::with_capacity(x.len());
    let mut acc = 0.0f32;
    for (i, &v) in x.iter().enumerate() {
        acc += v;
        if i >= k {
            acc -= x[i - k];
        }
        out.push(acc / k.min(i + 1) as f32);
    }
    out
}

/// Two-level FSK: bits, baud, coherence. `None` when it is not data.
pub fn two_level(disc: &[f32]) -> Option<(Vec<bool>, f64, f64)> {
    if disc.len() < 2000 {
        return None;
    }
    let lo = pct(disc, 0.05);
    let hi = pct(disc, 0.95);
    let range = hi - lo;
    if range < 0.12 {
        return None;
    }
    let mid = 0.5 * (hi + lo);
    let far = disc
        .iter()
        .filter(|&&v| (v - mid).abs() > 0.375 * range)
        .count();
    if far * 100 < disc.len() * 60 {
        return None;
    }
    let hyst = 0.15 * range;
    let mut edges = Vec::new();
    let mut state = disc[0] > mid;
    for (i, &v) in disc.iter().enumerate() {
        if state && v < mid - hyst / 2.0 {
            state = false;
            edges.push(i as f64);
        } else if !state && v > mid + hyst / 2.0 {
            state = true;
            edges.push(i as f64);
        }
    }
    if edges.len() < 20 {
        return None;
    }
    let iv: Vec<f32> = edges.windows(2).map(|w| (w[1] - w[0]) as f32).collect();
    let guess = RATE / pct(&iv, 0.15) as f64;
    let dur = disc.len() as f64 / RATE;
    let coh = |b: f64| {
        let (mut re, mut im) = (0.0, 0.0);
        for &e in &edges {
            let p = TAU * b * e / RATE;
            re += p.cos();
            im += p.sin();
        }
        (re * re + im * im).sqrt() / edges.len() as f64
    };
    let (lo_b, hi_b) = ((0.8 * guess).max(60.0), (1.25 * guess).min(12000.0));
    if lo_b >= hi_b {
        return None;
    }
    let mut best = (0.0, lo_b);
    let step = 0.2 / dur;
    let mut b = lo_b;
    while b <= hi_b {
        let c = coh(b);
        if c > best.0 {
            best = (c, b);
        }
        b += step;
    }
    let fine = step / 8.0;
    let centre = best.1;
    let mut b = centre - step;
    while b <= centre + step {
        let c = coh(b);
        if c > best.0 {
            best = (c, b);
        }
        b += fine;
    }
    let (mut c, mut baud) = best;
    let half = coh(baud / 2.0);
    if baud / 2.0 >= 60.0 && half >= 0.8 * c {
        baud /= 2.0;
        c = half;
    }
    let bit = RATE / baud;
    let long = iv.iter().filter(|&&v| v as f64 > 1.5 * bit).count();
    if c < 0.3 || long * 100 < iv.len() * 12 {
        return None;
    }
    // Read at the bit centres, the clock pulled 0.15 of the error by an edge within a quarter bit of a boundary.
    let mut bits = Vec::new();
    let mut t = edges[0] + bit / 2.0;
    let mut ei = 0;
    while t < disc.len() as f64 {
        let boundary = t - bit / 2.0;
        while ei < edges.len() && edges[ei] < boundary - bit / 2.0 {
            ei += 1;
        }
        if ei < edges.len() && (edges[ei] - boundary).abs() < bit / 4.0 {
            t += 0.15 * (edges[ei] - boundary);
        }
        bits.push(disc[t as usize] > mid);
        t += bit;
    }
    Some((bits, baud, c))
}

/// The nearest standard rate within 1.5 %.
pub fn standard_rate(baud: f64) -> u32 {
    [300u32, 512, 600, 1200, 2400, 4800, 9600]
        .into_iter()
        .find(|&r| (baud - r as f64).abs() <= 0.015 * r as f64)
        .unwrap_or(0)
}

fn word_at(bits: &[bool], at: usize, n: usize, inv: bool) -> u64 {
    bits[at..at + n]
        .iter()
        .fold(0u64, |a, &b| (a << 1) | (b != inv) as u64)
}

/// The analysis of the bits into `b`.
pub fn analyse_bits(bits: &[bool], two_level: bool, b: &mut Burst) {
    let mut bits = bits.to_vec();
    // Manchester (two-level FSK only).
    if two_level && bits.len() >= 64 {
        for start in 0..2 {
            let pairs: Vec<(bool, bool)> = bits[start..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| (c[0], c[1]))
                .collect();
            let slips = pairs.iter().filter(|p| p.0 == p.1).count();
            if !pairs.is_empty() && slips * 100 <= pairs.len() * 15 {
                bits = pairs.iter().map(|p| p.0).collect();
                b.manchester = true;
                break;
            }
        }
    }
    b.bits = bits.len();
    b.hex = bits
        .as_chunks::<8>()
        .0
        .iter()
        .take(64)
        .map(|c| format!("{:02X}", c.iter().fold(0u8, |a, &x| (a << 1) | x as u8)))
        .collect::<Vec<_>>()
        .join(" ");
    for inv in [false, true] {
        if b.pocsag_sync.is_some() {
            break;
        }
        for at in 0..bits.len().saturating_sub(31) {
            if (word_at(&bits, at, 32, inv) as u32 ^ pocsag::SYNC).count_ones() <= 2 {
                b.pocsag_sync = Some(at);
                break;
            }
        }
    }
    let mut hits = Vec::new();
    let mut at = 0;
    while at + 16 <= bits.len() {
        let w = word_at(&bits, at, 16, false) as u16;
        if (w ^ 0x605A).count_ones() <= 1 || (!w ^ 0x605A).count_ones() <= 1 {
            hits.push(at);
            at += 16;
        } else {
            at += 1;
        }
    }
    b.w605a = hits.len();
    b.w605a_at = hits.iter().take(8).copied().collect();
    if let Some(&first) = hits.first() {
        let start = hits
            .windows(2)
            .find(|w| w[1] == w[0] + 16)
            .map(|w| w[1] + 16)
            .unwrap_or(first + 16);
        let inv = word_at(&bits, first, 16, false) as u16 != 0x605A
            && (word_at(&bits, first, 16, false) as u16 ^ 0x605A).count_ones() > 1;
        let mut p = start;
        while p + 16 <= bits.len() && b.after_605a.len() < 16 {
            b.after_605a
                .push(format!("{:04X}", word_at(&bits, p, 16, inv)));
            p += 16;
        }
    }
    b.repeat = (8..=512.min(bits.len() / 2))
        .find(|&s| {
            let n = bits.len() - s;
            let eq = (0..n).filter(|&i| bits[i] == bits[i + s]).count();
            n >= 32 && eq * 100 >= n * 92
        })
        .unwrap_or(0);
    let mut best = (0usize, 0usize, 0usize, 0usize, false);
    for inv in [false, true] {
        for off in 0..32.min(bits.len()) {
            let words: Vec<bool> = (0..)
                .map(|k| off + 32 * k)
                .take_while(|&p| p + 32 <= bits.len())
                .map(|p| word::is_valid(word_at(&bits, p, 32, inv) as u32))
                .collect();
            let valid = words.iter().filter(|&&v| v).count();
            let mut run = 0;
            let mut cur = 0;
            for &v in &words {
                cur = if v { cur + 1 } else { 0 };
                run = run.max(cur);
            }
            if valid > best.0 || (valid == best.0 && run > best.2) {
                best = (valid, words.len(), run, off, inv);
            }
        }
    }
    (
        b.bch_valid,
        b.bch_total,
        b.bch_run,
        b.bch_offset,
        b.bch_inverted,
    ) = best;
    b.words = (0..)
        .map(|k| best.3 + 32 * k)
        .take_while(|&p| p + 32 <= bits.len())
        .take(32)
        .map(|p| {
            let w = word_at(&bits, p, 32, best.4) as u32;
            (format!("{w:08X}"), word::is_valid(w))
        })
        .collect();
    // Framing: every bit sent twice.
    for start in 0..2 {
        let pairs: Vec<(bool, bool)> = bits[start..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| (c[0], c[1]))
            .collect();
        let eq = pairs.iter().filter(|p| p.0 == p.1).count();
        if pairs.len() >= 32 && eq * 100 >= pairs.len() * 95 {
            b.doubled = true;
            bits = pairs.iter().map(|p| p.0).collect();
            break;
        }
    }
    // A repeating marker.
    'outer: for len in 8..=32usize {
        if bits.len() < len * 8 {
            break;
        }
        for start in 0..(bits.len() - len).min(256) {
            let pat = &bits[start..start + len];
            if pat.windows(2).filter(|w| w[0] != w[1]).count() < 5 {
                continue;
            }
            let found: Vec<usize> = (start..bits.len() - len + 1)
                .filter(|&p| bits[p..p + len] == *pat)
                .collect();
            if found.len() < 4 {
                continue;
            }
            let gaps: Vec<usize> = found.windows(2).map(|w| w[1] - w[0]).collect();
            let multiples = gaps.iter().filter(|&&g| g % len == 0).count();
            if multiples * 100 < gaps.len() * 80 {
                continue;
            }
            let gap = {
                let mut g = gaps.clone();
                g.sort_unstable();
                g[g.len() / 2]
            };
            let pattern: String = pat.iter().map(|&x| if x { '1' } else { '0' }).collect();
            b.marker = Some((len, pattern, found.len(), gap));
            let mut words: Vec<u64> = Vec::new();
            for &p in &found {
                let s = p + len;
                let e = (p + gap).min(bits.len());
                if e > s && e - s <= 64 {
                    words.push(word_at(&bits, s, e - s, false));
                }
                if words.len() >= 64 {
                    break;
                }
            }
            let distinct = {
                let mut d = words.clone();
                d.sort_unstable();
                d.dedup();
                d.len()
            };
            let repeats = words.windows(2).filter(|w| w[0] == w[1]).count();
            let twice = words
                .iter()
                .filter(|&&w| {
                    let n = gap - len;
                    n >= 16 && {
                        let h = (n - 8) / 2;
                        let x = (w >> h) & ((1 << h) - 1);
                        let y = w & ((1 << h) - 1);
                        (x ^ y).count_ones() <= 1
                    }
                })
                .count();
            b.frame_words = (distinct, repeats, twice);
            b.frame_list = words.iter().map(|w| format!("{w:X}")).collect();
            break 'outer;
        }
    }
    b.verdict = verdict(b);
}

fn verdict(b: &Burst) -> String {
    let mut v = if let Some(at) = b.pocsag_sync {
        format!(
            "POCSAG sync word 0x7CD215D8 found at bit {at}: this stream is standard POCSAG framing, read it in the POCSAG mode. Not Multitone's own format (unverified)"
        )
    } else if b.bch_run >= 2 {
        format!(
            "{} consecutive 32-bit words pass the POCSAG BCH(31,21) + parity check ({} of {} at bit offset {}): the same polynomial (an inverted stream passes too). Header and framing are NOT known (Multitone's format is not public); unverified",
            b.bch_run, b.bch_valid, b.bch_total, b.bch_offset
        )
    } else if b.kind == BurstKind::Ffsk {
        format!(
            "Audio FFSK: tones {:.1} and {:.1} Hz, {:.1} baud; no BCH words. The 16-bit word 0x605A is in the stream {} times{}. The meaning of the words after it is not worked out; unverified.",
            b.tones.0,
            b.tones.1,
            b.baud,
            b.w605a,
            if b.w605a >= 2 { " (as in the recordings of the real signal)" } else { "" }
        )
    } else {
        format!(
            "FSK data at {:.0} baud{}; no run of valid BCH(31,21) words (best {} of {}): not POCSAG-coded, or too damaged. Format unknown; unverified",
            b.baud,
            if b.manchester { ", Manchester coded" } else { "" },
            b.bch_valid,
            b.bch_total
        )
    };
    if b.manchester {
        v.push_str(" (Manchester decoded.)");
    }
    if b.repeat > 0 {
        v.push_str(&format!(
            " The bit pattern repeats every {} bits.",
            b.repeat
        ));
    }
    if b.doubled {
        v.push_str(&format!(
            " Every bit is sent twice (data rate {:.1} baud).",
            b.baud / 2.0
        ));
    }
    if let Some((len, pat, count, gap)) = &b.marker {
        v.push_str(&format!(
            " Framing measured: a {len}-bit marker {pat} {count} times, one every {gap} bits, {} different words after it ({} repeat the one before, {} are a first byte and a value sent twice). Their meaning is not worked out.",
            b.frame_words.0, b.frame_words.1, b.frame_words.2
        ));
    }
    v
}

/// The FSK measurements of a transmission: the whole, then halves, then quarters. The first part that produces 64 bits wins.
pub fn fsk(disc: &[f32]) -> Option<Burst> {
    let n = disc.len();
    let parts: Vec<(usize, usize)> = vec![
        (0, n),
        (0, n / 2),
        (n / 2, n),
        (0, n / 4),
        (n / 4, n / 2),
        (n / 2, 3 * n / 4),
        (3 * n / 4, n),
    ];
    for (a, b) in parts {
        let x = &disc[a..b];
        if let Some((bits, baud, tones, coh)) = audio_ffsk(x) {
            let mut bu = Burst {
                kind: BurstKind::Ffsk,
                baud,
                standard: standard_rate(baud),
                tones,
                coherence: coh,
                ..Default::default()
            };
            analyse_bits(&bits, false, &mut bu);
            return Some(bu);
        }
        if let Some((bits, baud, coh)) = two_level(x) {
            if bits.len() >= 64 {
                let mut bu = Burst {
                    kind: BurstKind::Fsk,
                    baud,
                    standard: standard_rate(baud),
                    coherence: coh,
                    ..Default::default()
                };
                analyse_bits(&bits, true, &mut bu);
                return Some(bu);
            }
        }
    }
    None
}

/// The measurements of one transmission (the discriminator in rad/sample at 48 kHz, already the mean of 3).
pub fn measure(disc: &[f32], time: f64, snr_db: f64) -> (Transmission, Option<Burst>) {
    let k = RATE / TAU;
    let dev = 0.5 * (pct(disc, 0.98) - pct(disc, 0.02)) as f64 * k;
    let audio = audio8k(disc);
    let sp = spectra(&audio);
    let ln = lines(&sp);
    let tones = steady_tones(&sp);
    let (period_ms, strength) = period(&audio);
    let burst = fsk(disc);
    let kind = if let Some(b) = &burst {
        match b.kind {
            BurstKind::Ffsk => TxKind::FfskData,
            BurstKind::Fsk => TxKind::FskData,
        }
    } else if !tones.is_empty() && (ln.len() <= 2 || tones.len() >= 2) {
        TxKind::ToneSequence
    } else if ln.len() >= 2 && period_ms > 0.0 {
        TxKind::Repeating
    } else if ln.len() >= 2 {
        TxKind::MultiTone
    } else {
        TxKind::Other
    };
    let tones_text = tones
        .iter()
        .map(|t| format!("{:.1} Hz {:.2} s", t.hz, t.seconds))
        .collect::<Vec<_>>()
        .join("; ");
    (
        Transmission {
            time,
            duration: disc.len() as f64 / RATE,
            snr_db,
            deviation_hz: dev,
            kind,
            tones,
            tones_text,
            lines: ln,
            period_ms,
            period_strength: strength,
            burst: 0,
        },
        burst.map(|mut b| {
            b.time = time;
            b
        }),
    )
}
