//! The search, fit and tone measurements shared by the slow FSK modes (WSPR, JT65, JT9, JT4, Q65, FST4, FST4W): a spectrogram
//! search for the mode's sync pattern over frequency and start (with linear drift where the mode requires it), a fit of time
//! and frequency on a narrow baseband, and the complex amplitude of every tone of every symbol.

use super::slot::*;
use crate::mode::Sub;
use num_complex::Complex32;
use std::f64::consts::PI;

/// How a mode marks its frame.
#[derive(Clone, Debug)]
pub enum SyncKind {
    /// Tone = sync bit + 2 x data bit (WSPR, JT4): the sync bit of every symbol.
    Parity(Vec<u8>),
    /// Tone 0 at the sync symbols, data tones elsewhere (JT65, JT9, Q65).
    Tone0(Vec<bool>),
    /// Known tones at known symbols (FST4): (symbol, tone).
    Words(Vec<(usize, u8)>),
}

/// The frame of a slow FSK mode.
#[derive(Clone, Debug)]
pub struct SlowSpec {
    /// The mode.
    pub sub: Sub,
    /// Symbols.
    pub nsym: usize,
    /// Tones.
    pub ntones: usize,
    /// Tone spacing, Hz.
    pub spacing: f64,
    /// Seconds per symbol.
    pub sym_s: f64,
    /// Baseband samples per symbol.
    pub sps2: usize,
    /// The sync pattern.
    pub sync: SyncKind,
    /// Drifts searched, in spectrogram bins over the transmission.
    pub drifts: Vec<f64>,
    /// Hann-windowed spectrogram (wide sub-modes splash otherwise).
    pub hann: bool,
    /// Lowest and highest frequency of tone 0 searched, Hz.
    pub band: (f64, f64),
    /// Candidate threshold and local-maximum half-width (bins).
    pub threshold: f32,
    /// Local maximum over +-this many bins.
    pub peak_bins: i64,
    /// Most candidates per pass.
    pub max_cand: usize,
    /// Spectrogram bins per symbol rate (2: bins of half the symbol rate).
    pub bins_per_tone: usize,
    /// The frequency unit of the fit, Hz (the tone spacing; the symbol rate when the tones are wider apart than it).
    pub fit_hz: f64,
    /// The band kept below tone 0 and above the top tone in the baseband, Hz.
    pub guard_hz: f64,
}

impl SlowSpec {
    /// Spectrogram bins between neighbouring tones.
    pub fn tone_bins(&self) -> i64 {
        ((self.spacing * self.sym_s).round() as i64).max(1) * self.bins_per_tone as i64
    }
}

/// A candidate of a slow mode.
#[derive(Clone, Copy, Debug)]
pub struct SlowCand {
    /// Start, seconds from the slot start.
    pub t: f64,
    /// Tone 0, Hz.
    pub hz: f64,
    /// Drift over the transmission, Hz.
    pub drift_hz: f64,
    /// Sync score.
    pub score: f32,
    /// Inverted sync (JT4).
    pub inverted: bool,
}

/// The search.
pub fn search(audio: &[f32], slot: &Slot, s: &SlowSpec, invert_too: bool) -> Vec<SlowCand> {
    let sym = s.sym_s * slot.rate;
    let nwin = sym.round() as usize;
    let nfft = (s.bins_per_tone as f64 * sym).round() as usize;
    let step = (sym / 4.0).round().max(1.0) as usize;
    let sp = Spectrogram::new(audio, nwin, step, nfft, s.hann, slot.rate);
    if sp.rows == 0 {
        return Vec::new();
    }
    let df = sp.df;
    let bpt = s.tone_bins();
    let (lo, hi) = slot.dt_range();
    let t0 = s.sub.t0();
    let r_lo = (slot.sample_of(t0 + lo) / step as f64).floor() as i64;
    let r_hi = (slot.sample_of(t0 + hi) / step as f64).ceil() as i64;
    let span_bins = (s.ntones as i64 - 1) * bpt;
    let b_lo = (s.band.0 / df).ceil() as i64;
    let b_hi = ((s.band.1 / df).floor() as i64).min(sp.bins as i64 - 1 - span_bins);
    if b_hi <= b_lo {
        return Vec::new();
    }
    let rows_per_sym = sym / step as f64;
    // The noise level: the mean of the lower half of each bin's column (an exponential variable's lower half has mean 0.3069
    // of its mean), and its median over the band searched, so that a strong signal's own bins do not raise their noise.
    let col: Vec<f32> = (0..sp.bins)
        .map(|b| {
            let mut c: Vec<f32> = (0..sp.rows).map(|r| sp.p[r * sp.bins + b]).collect();
            c.sort_by(|a, x| a.total_cmp(x));
            let h = c.len().div_ceil(2).max(1);
            (c[..h].iter().sum::<f32>() / h as f32 / 0.3069).max(1e-30)
        })
        .collect();
    let floor = median(&col[b_lo as usize..=(b_hi + span_bins).min(sp.bins as i64 - 1) as usize])
        .max(1e-30);
    let noise: Vec<f32> = col
        .iter()
        .map(|&c| c.min(4.0 * floor).max(0.25 * floor))
        .collect();
    let mut out = Vec::new();
    let polarities: &[bool] = if invert_too { &[false, true] } else { &[false] };
    for &inv in polarities {
        let nb = (b_hi - b_lo + 1) as usize;
        let mut best = vec![f32::MIN; nb];
        let mut best_at = vec![(0i64, 0f64); nb];
        for &drift in &s.drifts {
            // The row and the bin of each symbol against the first, for this drift.
            let n = s.nsym as f64;
            let at: Vec<(i64, i64)> = (0..s.nsym)
                .map(|k| {
                    (
                        (k as f64 * rows_per_sym).round() as i64,
                        (drift * (k as f64 / n - 0.5)).round() as i64,
                    )
                })
                .collect();
            for r in r_lo..=r_hi {
                for (bi, b) in (b_lo..=b_hi).enumerate() {
                    let sc = score(&sp, s, r, b, &at, inv, &noise);
                    if sc > best[bi] {
                        best[bi] = sc;
                        best_at[bi] = (r, drift);
                    }
                }
            }
        }
        let med = median(&best);
        for i in 0..nb {
            let v = best[i];
            if v < s.threshold
                || v < med + 0.5 * s.threshold.min(0.3)
                    && matches!(s.sync, SyncKind::Parity(_))
                    && v < med + 0.08
            {
                continue;
            }
            let lo_i = i.saturating_sub(s.peak_bins as usize);
            let hi_i = (i + s.peak_bins as usize).min(nb - 1);
            if (lo_i..=hi_i).any(|j| j != i && (best[j] > v || (best[j] == v && j < i))) {
                continue;
            }
            // Clicks and noise crashes: a peak must stand above its surroundings when they are high.
            if matches!(s.sync, SyncKind::Tone0(_)) {
                let around: Vec<f32> = (0..nb)
                    .filter(|&j| {
                        let d = (j as i64 - i as i64).abs();
                        (5..=25).contains(&d)
                    })
                    .map(|j| best[j])
                    .collect();
                let m = median(&around);
                if m >= 3.0 && v < 1.5 * m + 0.2 {
                    continue;
                }
            }
            let (r, drift) = best_at[i];
            out.push(SlowCand {
                t: slot.time_of((r * step as i64) as f64),
                hz: (b_lo + i as i64) as f64 * df,
                drift_hz: drift * df,
                score: v,
                inverted: inv,
            });
        }
    }
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out.truncate(s.max_cand);
    out
}

/// The sync score of a frame whose first symbol is at row `r` and whose tone 0 is at bin `b`. `at[k]` is the row and the bin of
/// symbol `k` against those.
fn score(
    sp: &Spectrogram,
    s: &SlowSpec,
    r: i64,
    b: i64,
    at: &[(i64, i64)],
    inv: bool,
    noise: &[f32],
) -> f32 {
    let bpt = s.tone_bins();
    let at = |k: usize| -> (i64, i64) { (r + at[k].0, b + at[k].1) };
    match &s.sync {
        SyncKind::Parity(v) => {
            let (mut num, mut den) = (0f32, 0f32);
            for (k, &sb) in v.iter().enumerate() {
                let (row, bin) = at(k);
                let sb = sb ^ inv as u8;
                let p: [f32; 4] = std::array::from_fn(|t| sp.at(row, bin + bpt * t as i64));
                let hi = p[sb as usize] + p[sb as usize + 2];
                let lo = p[1 - sb as usize] + p[3 - sb as usize];
                num += hi - lo;
                den += hi + lo;
            }
            if den > 0.0 {
                num / den
            } else {
                0.0
            }
        }
        SyncKind::Tone0(v) => {
            // Tone 0 high at the sync symbols and low at the data symbols (which use the other tones), against the bin's noise.
            let (mut a, mut na, mut d, mut nd) = (0f32, 0, 0f32, 0);
            for (k, &is) in v.iter().enumerate() {
                let (row, bin) = at(k);
                if bin < 0 || (bin as usize) >= noise.len() {
                    continue;
                }
                let p = sp.at(row, bin) / noise[bin as usize];
                if is {
                    a += p;
                    na += 1;
                } else {
                    d += p;
                    nd += 1;
                }
            }
            if na == 0 || nd == 0 {
                0.0
            } else {
                a / na as f32 - d / nd as f32
            }
        }
        SyncKind::Words(w) => {
            let (mut num, mut all) = (0f32, 0f32);
            for &(k, t) in w {
                let (row, bin) = at(k);
                for tt in 0..s.ntones as i64 {
                    let p = sp.at(row, bin + bpt * tt);
                    all += p;
                    if tt == t as i64 {
                        num += p;
                    }
                }
            }
            let den = (all - num) / (s.ntones - 1) as f32;
            if den > 0.0 {
                num / den
            } else {
                0.0
            }
        }
    }
}

/// A candidate cut to a narrow baseband and fitted.
pub struct Fit {
    /// The baseband.
    pub bb: Vec<Complex32>,
    /// Its rate.
    pub fs: f64,
    /// The audio frequency at its 0 Hz.
    pub shift: f64,
    /// Start, seconds from the first audio sample.
    pub t_audio: f64,
    /// Tone 0 relative to the shift, Hz.
    pub df: f64,
    /// Drift, Hz over the transmission.
    pub drift_hz: f64,
    /// The fit metric.
    pub metric: f32,
}

impl Fit {
    /// The audio frequency of tone 0.
    pub fn hz(&self) -> f64 {
        self.shift + self.df
    }
}

/// Seconds per symbol as a fraction (numerator, denominator).
pub fn sym_fraction(sub: &Sub) -> (u64, u64) {
    use crate::mode::Mode;
    match sub.mode {
        Mode::Jt65 => (4096, 11025),
        Mode::Jt4 => (2520, 11025),
        _ => (sub.samples_per_symbol_12k().round() as u64, 12000),
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// The slot spectrum size `n` and baseband size `m` for `len` audio samples, so that the baseband has exactly `sps2` samples a
/// symbol.
pub fn sizes(s: &SlowSpec, len: usize, rate: f64) -> (usize, usize) {
    let (a, b) = sym_fraction(&s.sub);
    let num = s.sps2 as u64 * b;
    let den = rate as u64 * a;
    let g = gcd(num, den);
    let (p, q) = ((num / g) as usize, (den / g) as usize);
    let j = smooth_at_least(len.div_ceil(q));
    (q * j, p * j)
}

/// Cuts the baseband of a candidate and fits time (+-2/16 symbol in 1/16 steps) and frequency (+-1/2 of the fit unit in
/// quarters), then once more at half the steps.
pub fn fit(spec: &SlotSpectrum, slot: &Slot, s: &SlowSpec, c: &SlowCand, m: usize) -> Fit {
    let fs = slot.rate * m as f64 / spec.n as f64;
    let span = (s.ntones as f64 - 1.0) * s.spacing;
    let skirt = 2.0 * s.spacing;
    let lo = c.hz - s.guard_hz - c.drift_hz.abs();
    let hi = c.hz + span + s.guard_hz + c.drift_hz.abs();
    let mut bb = Vec::new();
    let shift = spec.baseband(c.hz, lo, hi, skirt, m, &mut bb);
    let t_audio = c.t - slot.t_first;
    let mut f = Fit {
        bb,
        fs,
        shift,
        t_audio,
        df: c.hz - shift,
        drift_hz: c.drift_hz,
        metric: f32::MIN,
    };
    let mut amps = Vec::new();
    let mut best = (f.t_audio, f.df, f32::MIN);
    for round in 0..2 {
        let (tstep, fstep) = if round == 0 {
            (s.sym_s / 16.0, s.fit_hz / 4.0)
        } else {
            (s.sym_s / 32.0, s.fit_hz / 8.0)
        };
        let (bt, bf) = (best.0, best.1);
        for it in -2..=2 {
            for jf in -2..=2 {
                f.t_audio = bt + it as f64 * tstep;
                f.df = bf + jf as f64 * fstep;
                amplitudes_of(&f, s, matches!(s.sync, SyncKind::Tone0(_)), &mut amps);
                let mt = metric(&amps, s, c.inverted);
                if mt > best.2 {
                    best = (f.t_audio, f.df, mt);
                }
            }
        }
    }
    f.t_audio = best.0;
    f.df = best.1;
    f.metric = best.2;
    f
}

/// The fit metric: the sync energy of the frame against the rest.
pub fn metric(c: &[Complex32], s: &SlowSpec, inverted: bool) -> f32 {
    let m = s.ntones;
    match &s.sync {
        SyncKind::Parity(v) => {
            let (mut num, mut den) = (0f32, 0f32);
            for (k, &sb) in v.iter().enumerate() {
                let sb = (sb ^ inverted as u8) as usize;
                let p: [f32; 4] = std::array::from_fn(|t| c[k * m + t].norm_sqr());
                num += p[sb] + p[sb + 2] - p[1 - sb] - p[3 - sb];
                den += p.iter().sum::<f32>();
            }
            num / den.max(1e-30)
        }
        SyncKind::Tone0(v) => {
            let (mut a, mut b, mut na, mut nb) = (0f32, 0f32, 0, 0);
            for (k, &is) in v.iter().enumerate() {
                let p = c[k * m].norm_sqr();
                if is {
                    a += p;
                    na += 1;
                } else {
                    b += p;
                    nb += 1;
                }
            }
            (a / na.max(1) as f32) / (b / nb.max(1) as f32 + 1e-30)
        }
        SyncKind::Words(w) => {
            // The power of the sync tones against the other tones of the same symbols.
            let (mut num, mut all) = (0f32, 0f32);
            for &(k, t) in w {
                for tt in 0..m {
                    let p = c[k * m + tt].norm_sqr();
                    all += p;
                    if tt == t as usize {
                        num += p;
                    }
                }
            }
            num / ((all - num) / (m - 1) as f32).max(1e-30)
        }
    }
}

/// The complex amplitude of every tone of every symbol (`out[k * ntones + t]`), with drift, referred to the frame's start: one
/// FFT of a symbol (after removing the frequency offset) per symbol, tone `t` at bin `t x spacing x symbol length`.
pub fn amplitudes(f: &Fit, s: &SlowSpec, out: &mut Vec<Complex32>) {
    amplitudes_of(f, s, false, out)
}

/// As [`amplitudes`]; with `tone0_only`, tone 0 alone (by direct correlation; the other tones are left 0).
pub fn amplitudes_of(f: &Fit, s: &SlowSpec, tone0_only: bool, out: &mut Vec<Complex32>) {
    let m = s.ntones;
    out.clear();
    out.resize(s.nsym * m, Complex32::default());
    let n = s.sps2;
    let bpt = (s.spacing * s.sym_s).round().max(1.0) as usize;
    let mut buf = vec![Complex32::default(); n];
    let scale = 1.0 / n as f32;
    for k in 0..s.nsym {
        let t_sym = f.t_audio + k as f64 * s.sym_s;
        let start = (t_sym * f.fs).round() as i64;
        let drift = f.drift_hz * (k as f64 / s.nsym as f64 - 0.5);
        let freq = f.df + drift;
        let w = -2.0 * PI * freq / f.fs;
        let (wc, ws) = (w.cos(), w.sin());
        let (mut c, mut si) = (1.0f64, 0.0f64);
        for (i, b) in buf.iter_mut().enumerate() {
            let j = start + i as i64;
            *b = if j >= 0 && (j as usize) < f.bb.len() {
                f.bb[j as usize] * Complex32::new(c as f32, si as f32)
            } else {
                Complex32::default()
            };
            let nc = c * wc - si * ws;
            si = c * ws + si * wc;
            c = nc;
        }
        let p = -2.0 * PI * freq * (start as f64 / f.fs - f.t_audio);
        let rot = Complex32::new(p.cos() as f32, p.sin() as f32) * scale;
        if tone0_only {
            out[k * m] = buf.iter().sum::<Complex32>() * rot;
            continue;
        }
        fft_in_place(&mut buf, false);
        for t in 0..m {
            out[k * m + t] = buf[(t * bpt) % n] * rot;
        }
    }
}

/// The SNR of a decode from its tone amplitudes and sent tones, over the symbols of `which`.
pub fn snr(c: &[Complex32], s: &SlowSpec, tones: &[u8]) -> f64 {
    let m = s.ntones;
    let (mut ps, mut po) = (0f64, 0f64);
    for (k, &tk) in tones.iter().enumerate().take(s.nsym) {
        for t in 0..m {
            let p = c[k * m + t].norm_sqr() as f64;
            if t == tk as usize {
                ps += p;
            } else {
                po += p / (m - 1) as f64;
            }
        }
    }
    snr_2500(ps, po, s.spacing)
}

/// Subtracts a decoded signal from the audio.
pub fn subtract_decode(audio: &mut [f32], slot: &Slot, f: &Fit, s: &SlowSpec, tones: &[u8]) {
    let mut r = clean_copy(&s.sub, tones, f.hz(), false, slot.rate);
    if f.drift_hz != 0.0 {
        let n = r.len() as f64;
        for (i, x) in r.iter_mut().enumerate() {
            // Linear drift: the phase of the integrated frequency offset.
            let t = i as f64 / slot.rate;
            let tt = n / slot.rate;
            let ph = 2.0 * PI * f.drift_hz * (t * t / (2.0 * tt) - 0.5 * t);
            *x *= Complex32::new(ph.cos() as f32, ph.sin() as f32);
        }
    }
    let start = (f.t_audio * slot.rate).round() as i64;
    subtract(audio, start, &r, (s.sym_s * slot.rate) as usize);
}
