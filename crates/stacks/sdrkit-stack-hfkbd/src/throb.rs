//! Throb and ThrobX: one or two tones at a time out of 9 (Throb) or 11 (ThrobX), each pair a character. The receiver finds the
//! tone comb in an averaged spectrum, then the variant, the symbol timing and the pairs, every 6 s over the last 30 s of audio,
//! and reports the characters that are new.
//!
//! Standard: the Throb description by L. Sear G3PPT (1998) and its ThrobX extension. No formal specification exists.

use crate::decoder::{
    self, flush_logs, last_call, put_text, Heard, HeardCommand, Modulator, GEN_MESSAGE, GEN_SECTION,
};
use crate::typed::{ThrobFields, ThrobTextEvent, THROB_TEXT};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Complex32, ConfigError, GenConfig, Generator, LockState, Record, Schema, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, Status, Value, Values,
};
use sdrkit_graph::{values_of, Format, Kept, Outbox, Port};
use sdrkit_util_text::store::TextStore;
use std::f64::consts::{PI, TAU};
use std::sync::Arc;

/// A Throb variant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Variant {
    /// Its name.
    pub name: &'static str,
    /// Tones: 9 (Throb) or 11 (ThrobX).
    pub tones: usize,
    /// Tone spacing, Hz.
    pub spacing: f64,
    /// Symbol length, seconds.
    pub symbol_s: f64,
    /// The pulse is a full raised cosine (else a raised-cosine rise and fall over 20 % of the symbol each).
    pub full_cosine: bool,
}

impl Variant {
    /// ThrobX (11 tones, the idle and space pairs alternating).
    pub fn is_x(&self) -> bool {
        self.tones == 11
    }
    /// The offset of tone `k` (1-based) from the centre, Hz.
    pub fn tone_offset(&self, k: usize) -> f64 {
        (k as f64 - (self.tones as f64 + 1.0) / 2.0) * self.spacing
    }
    /// The pair table of the variant.
    pub fn pairs(&self) -> &'static [(u8, u8)] {
        if self.is_x() {
            &X_PAIRS
        } else {
            &PAIRS
        }
    }
}

/// Every variant.
pub const VARIANTS: [Variant; 6] = [
    Variant {
        name: "Throb1",
        tones: 9,
        spacing: 8.0,
        symbol_s: 1.024,
        full_cosine: false,
    },
    Variant {
        name: "Throb2",
        tones: 9,
        spacing: 8.0,
        symbol_s: 0.512,
        full_cosine: false,
    },
    Variant {
        name: "Throb4",
        tones: 9,
        spacing: 16.0,
        symbol_s: 0.256,
        full_cosine: true,
    },
    Variant {
        name: "ThrobX1",
        tones: 11,
        spacing: 7.8125,
        symbol_s: 1.024,
        full_cosine: false,
    },
    Variant {
        name: "ThrobX2",
        tones: 11,
        spacing: 7.8125,
        symbol_s: 0.512,
        full_cosine: false,
    },
    Variant {
        name: "ThrobX4",
        tones: 11,
        spacing: 15.625,
        symbol_s: 0.256,
        full_cosine: true,
    },
];

/// The Throb pairs (1-based tones) by symbol number: 0 idle, 5 shift, 44 space.
pub const PAIRS: [(u8, u8); 45] = [
    (5, 5),
    (4, 5),
    (1, 2),
    (1, 3),
    (1, 4),
    (4, 6),
    (1, 5),
    (1, 6),
    (1, 7),
    (3, 7),
    (1, 8),
    (2, 3),
    (2, 4),
    (2, 8),
    (2, 5),
    (5, 6),
    (2, 6),
    (2, 9),
    (3, 4),
    (3, 5),
    (1, 9),
    (3, 6),
    (8, 9),
    (3, 8),
    (3, 3),
    (2, 2),
    (1, 1),
    (3, 9),
    (4, 7),
    (4, 8),
    (4, 9),
    (5, 7),
    (5, 8),
    (5, 9),
    (6, 7),
    (6, 8),
    (6, 9),
    (7, 8),
    (7, 9),
    (8, 8),
    (7, 7),
    (6, 6),
    (4, 4),
    (9, 9),
    (2, 7),
];
/// The characters of the Throb symbols (`\0` idle, `\u{1}` shift).
pub const CHARS: &[u8; 45] = b"\0ABCD\x01FGHIJKLMNOPQRSTUVWXYZ1234567890,.'/)(E ";
/// The Throb symbol of the shift.
pub const SHIFT: usize = 5;
/// The Throb shifted characters: (the symbol after the shift, the character).
pub const SHIFTED: [(usize, u8); 4] = [(20, b'?'), (13, b'@'), (9, b'='), (0, b'\n')];

/// The ThrobX pairs by symbol number: 0 and 1 the alternating idle and space, 54 a line feed.
pub const X_PAIRS: [(u8, u8); 55] = [
    (6, 11),
    (1, 6),
    (2, 6),
    (2, 5),
    (2, 7),
    (2, 8),
    (5, 6),
    (2, 9),
    (2, 10),
    (4, 8),
    (4, 6),
    (2, 11),
    (3, 4),
    (3, 5),
    (3, 6),
    (6, 9),
    (6, 10),
    (3, 7),
    (3, 8),
    (3, 9),
    (6, 8),
    (6, 7),
    (3, 10),
    (3, 11),
    (4, 5),
    (4, 7),
    (4, 9),
    (4, 10),
    (1, 2),
    (1, 3),
    (1, 4),
    (1, 5),
    (1, 7),
    (1, 8),
    (1, 9),
    (1, 10),
    (2, 3),
    (2, 4),
    (4, 11),
    (5, 7),
    (5, 8),
    (5, 9),
    (5, 10),
    (5, 11),
    (7, 8),
    (7, 9),
    (7, 10),
    (7, 11),
    (8, 9),
    (8, 10),
    (8, 11),
    (9, 10),
    (9, 11),
    (10, 11),
    (1, 11),
];
/// The characters of the ThrobX symbols (`\0` for the idle and space pairs).
pub const X_CHARS: &[u8; 55] = b"\0\0ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890,.'/)(#\"+-;:?!@=\n";

/// The symbol of a pair (in either order) in `pairs`.
pub fn symbol_of(pairs: &[(u8, u8)], a: u8, b: u8) -> Option<usize> {
    let (a, b) = (a.min(b), a.max(b));
    pairs.iter().position(|&p| p == (a, b))
}

/// The symbols that send `text` in variant `v`: 4 idle symbols, the text (lower case folded, other characters as a space), one
/// idle symbol. ThrobX swaps its idle and space pairs after each use.
pub fn encode(v: &Variant, text: &str) -> Vec<usize> {
    let mut out = Vec::new();
    if v.is_x() {
        let (mut idle, mut space) = (0usize, 1usize);
        let mut blank = |out: &mut Vec<usize>, is_idle: bool| {
            out.push(if is_idle { idle } else { space });
            std::mem::swap(&mut idle, &mut space);
        };
        for _ in 0..4 {
            blank(&mut out, true);
        }
        for c in text.chars() {
            let c = c.to_ascii_uppercase();
            match X_CHARS[2..].iter().position(|&x| char::from(x) == c) {
                Some(i) if c != ' ' => out.push(i + 2),
                _ if c == '\r' => {}
                _ => blank(&mut out, false),
            }
        }
        blank(&mut out, true);
    } else {
        out.extend([0; 4]);
        for c in text.chars() {
            let c = c.to_ascii_uppercase();
            if c == '\r' {
                continue;
            }
            if let Some(&(s, _)) = SHIFTED.iter().find(|&&(_, x)| char::from(x) == c) {
                out.extend([SHIFT, s]);
                continue;
            }
            match CHARS.iter().position(|&x| char::from(x) == c) {
                Some(i) if i != 0 && i != SHIFT => out.push(i),
                _ => out.push(44),
            }
        }
        out.push(0);
    }
    out
}

/// The text of a run of symbols (`None`: no symbol was read there).
pub fn text_of(v: &Variant, symbols: &[Option<usize>]) -> String {
    let mut s = String::new();
    let mut shifted = false;
    for sym in symbols.iter().flatten() {
        if v.is_x() {
            if *sym <= 1 {
                if !s.ends_with(' ') {
                    s.push(' ');
                }
            } else {
                s.push(char::from(X_CHARS[*sym]));
            }
            continue;
        }
        if shifted {
            shifted = false;
            if let Some(&(_, c)) = SHIFTED.iter().find(|&&(x, _)| x == *sym) {
                s.push(char::from(c));
            }
            continue;
        }
        match *sym {
            0 => {}
            SHIFT => shifted = true,
            i => s.push(char::from(CHARS[i])),
        }
    }
    s
}

/// What a decode of a buffer found.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    /// The variant.
    pub variant: Variant,
    /// The centre of the tones, Hz.
    pub centre_hz: f64,
    /// The text.
    pub text: String,
    /// The score (at least 0.55).
    pub score: f64,
    /// The two tones of each symbol over the others, dB (not in 2500 Hz).
    pub snr_db: f64,
}

/// The rate of the baseband the symbols are read from, Hz (8 kHz averaged over 17 samples).
const AVERAGE_N: usize = 17;

/// The averaged power spectrum of `x` (8 kHz complex): Hann windows of `n` points, half overlapping.
fn spectrum(x: &[Complex32], n: usize) -> Vec<f64> {
    let mut fft = Fft::forward(n);
    let w: Vec<f32> = (0..n)
        .map(|i| (0.5 - 0.5 * (TAU * i as f64 / n as f64).cos()) as f32)
        .collect();
    let mut acc = vec![0f64; n];
    let mut buf = vec![Complex32::default(); n];
    let mut count = 0;
    let mut s = 0;
    while s + n <= x.len() {
        for i in 0..n {
            buf[i] = x[s + i] * w[i];
        }
        fft.process(&mut buf);
        for (a, b) in acc.iter_mut().zip(&buf) {
            *a += f64::from(b.norm_sqr());
        }
        count += 1;
        s += n / 2;
    }
    if count > 0 {
        for a in &mut acc {
            *a /= f64::from(count);
        }
    }
    acc
}

/// The tones of a variant read at baseband: prefix sums of the baseband mixed to each tone.
struct ToneSums {
    sums: Vec<Vec<(f64, f64)>>,
}

impl ToneSums {
    fn new(base: &[Complex32], fb: f64, offsets: &[f64]) -> ToneSums {
        let sums = offsets
            .iter()
            .map(|&f| {
                let w = -TAU * f / fb;
                let mut acc = Vec::with_capacity(base.len() + 1);
                let (mut re, mut im) = (0.0, 0.0);
                acc.push((re, im));
                for (i, x) in base.iter().enumerate() {
                    let (s, c) = (w * i as f64).sin_cos();
                    re += f64::from(x.re) * c - f64::from(x.im) * s;
                    im += f64::from(x.re) * s + f64::from(x.im) * c;
                    acc.push((re, im));
                }
                acc
            })
            .collect();
        ToneSums { sums }
    }
    fn energy(&self, t: usize, a: f64, b: f64) -> f64 {
        let s = &self.sums[t];
        let n = s.len() - 1;
        let (a, b) = (
            (a.round().max(0.0) as usize).min(n),
            (b.round().max(0.0) as usize).min(n),
        );
        let (re, im) = (s[b].0 - s[a].0, s[b].1 - s[a].1);
        re * re + im * im
    }
}

/// One reading of a candidate centre as one variant.
struct Try {
    variant: Variant,
    centre: f64,
    score: f64,
    symbols: Vec<Option<usize>>,
    snr_db: f64,
}

/// Reads the baseband of one candidate centre as variant `v`. `spec` is the averaged spectrum (bins of `bin` Hz).
fn try_variant(x: &[Complex32], centre: f64, v: &Variant, spec: &[f64], bin: f64) -> Option<Try> {
    let fb = 8000.0 / AVERAGE_N as f64;
    // Mix down by the centre and average over groups of samples.
    let mut base = Vec::with_capacity(x.len() / AVERAGE_N + 1);
    let w = -TAU * centre / 8000.0;
    for k in 0..x.len() / AVERAGE_N {
        let chunk = &x[k * AVERAGE_N..(k + 1) * AVERAGE_N];
        let mut acc = Complex32::default();
        for (i, s) in chunk.iter().enumerate() {
            let p = w * (k * AVERAGE_N + i) as f64;
            acc += s * Complex32::new(p.cos() as f32, p.sin() as f32);
        }
        base.push(acc / AVERAGE_N as f32);
    }
    let ls = v.symbol_s * fb;
    let nsym = (base.len() as f64 / ls).floor() as usize;
    if nsym < 4 {
        return None;
    }
    let nt = v.tones;
    let wide: Vec<f64> = (0..nt + 12)
        .map(|j| (j as f64 - 6.0 + 1.0 - (nt as f64 + 1.0) / 2.0) * v.spacing)
        .collect();
    let sums = ToneSums::new(&base, fb, &wide);
    let mid = |k: usize, off: f64| (off + (k as f64 + 0.2) * ls, off + (k as f64 + 0.8) * ls);
    let metric = |off: f64| -> f64 {
        let n = ((base.len() as f64 - off) / ls).floor() as usize;
        (0..n)
            .map(|k| {
                let (a, b) = mid(k, off);
                (6..6 + nt).map(|t| sums.energy(t, a, b)).sum::<f64>()
            })
            .sum()
    };
    let mut best = (f64::MIN, 0.0);
    for j in 0..24 {
        let off = j as f64 * ls / 24.0;
        let m = metric(off);
        if m > best.0 {
            best = (m, off);
        }
    }
    let coarse = best.1;
    for j in -3i32..=3 {
        let off = (coarse + f64::from(j) * ls / 96.0).rem_euclid(ls);
        let m = metric(off);
        if m > best.0 {
            best = (m, off);
        }
    }
    let off = best.1;
    let nsym = ((base.len() as f64 - off) / ls).floor() as usize;
    // Every symbol's energy of all nt + 12 tones.
    let e: Vec<Vec<f64>> = (0..nsym)
        .map(|k| {
            let (a, b) = mid(k, off);
            (0..nt + 12).map(|t| sums.energy(t, a, b)).collect()
        })
        .collect();
    // Comb alignment: the span of tones in use.
    let tot: Vec<f64> = (0..nt + 12).map(|t| e.iter().map(|s| s[t]).sum()).collect();
    let largest = tot.iter().copied().fold(0.0, f64::max);
    let mut sorted = tot.clone();
    sorted.sort_by(f64::total_cmp);
    let thr = (0.08 * largest).max(4.0 * (sorted[0] + sorted[1] + sorted[2]) / 3.0);
    let used: Vec<usize> = (0..nt + 12).filter(|&t| tot[t] > thr).collect();
    let (first, last) = (*used.first()?, *used.last()?);
    let shift = (((first + last) as f64 / 2.0) - (6.0 + (nt as f64 - 1.0) / 2.0)).round() as i64;
    let lo = (6 + shift).clamp(0, 12) as usize;
    let capture = tot[lo..lo + nt].iter().sum::<f64>() / tot.iter().sum::<f64>().max(1e-30);
    let centre = centre + (lo as f64 - 6.0) * v.spacing;
    // Pairs.
    let totals: Vec<f64> = e.iter().map(|s| s[lo..lo + nt].iter().sum()).collect();
    let mut by = totals.clone();
    by.sort_by(|a, b| b.total_cmp(a));
    let top = &by[..(by.len() / 4).max(1)];
    let active_thr = 0.1 * top.iter().sum::<f64>() / top.len() as f64;
    let pairs = v.pairs();
    let (mut share_num, mut share_den) = (0.0, 0.0);
    let (mut active, mut in_table) = (0usize, 0usize);
    let mut use_count = vec![0usize; nt];
    let mut snr = (0.0, 0usize);
    let mut symbols = Vec::with_capacity(nsym);
    for (k, s) in e.iter().enumerate() {
        let t = &s[lo..lo + nt];
        let (mut a, mut b) = (0usize, 1usize);
        if t[b] > t[a] {
            (a, b) = (b, a);
        }
        for i in 2..nt {
            if t[i] > t[a] {
                b = a;
                a = i;
            } else if t[i] > t[b] {
                b = i;
            }
        }
        share_num += t[a] + t[b];
        share_den += totals[k];
        if totals[k] < active_thr {
            symbols.push(None);
            continue;
        }
        active += 1;
        let single = !v.is_x() && t[a].sqrt() > 2.0 * t[b].sqrt();
        let (ta, tb) = if single { (a, a) } else { (a, b) };
        use_count[ta] += 1;
        if tb != ta {
            use_count[tb] += 1;
        }
        let others = (totals[k] - t[a] - t[b]) / (nt - 2) as f64;
        snr.0 += 0.5 * (t[a] + t[b]) / others.max(1e-30);
        snr.1 += 1;
        let sym = symbol_of(pairs, ta as u8 + 1, tb as u8 + 1);
        // A ThrobX pair sends both tones at one level: a strong tone with a weak one is no pair (a carrier in noise).
        if sym.is_some() && (!v.is_x() || t[b] >= 0.25 * t[a]) {
            in_table += 1;
        }
        symbols.push(sym);
    }
    if active == 0 {
        return None;
    }
    // The spectrum near the comb's tones against all of the comb's band.
    let half = (v.spacing / 3.0).min(1.5 / v.symbol_s);
    let (mut near, mut band) = (0.0, 0.0);
    let f_first = centre + v.tone_offset(1) - v.spacing / 2.0;
    let f_last = centre + v.tone_offset(nt) + v.spacing / 2.0;
    let mut b = (f_first / bin).ceil() as usize;
    while (b as f64) * bin <= f_last && b < spec.len() / 2 {
        let f = b as f64 * bin;
        band += spec[b];
        if (1..=nt).any(|k| (f - centre - v.tone_offset(k)).abs() <= half) {
            near += spec[b];
        }
        b += 1;
    }
    let mut score = (share_num / share_den.max(1e-30)) * capture * (near / band.max(1e-30));
    if v.is_x() {
        // The share of active symbols that are table pairs of two tones of one level.
        score *= in_table as f64 / active as f64;
    }
    let tones_used = use_count
        .iter()
        .filter(|&&c| c as f64 >= 0.05 * active as f64)
        .count();
    if tones_used < 4 {
        score *= 0.5;
    }
    score -= 0.002 * nt as f64;
    Some(Try {
        variant: *v,
        centre,
        score,
        symbols,
        snr_db: 10.0 * (snr.0 / snr.1.max(1) as f64).max(1e-6).log10(),
    })
}

/// The variants a setting allows: all of them, or one (Throb1 with Throb2, ThrobX1 with ThrobX2).
fn allowed(choice: VariantChoice) -> Vec<Variant> {
    let names: &[&str] = match choice {
        VariantChoice::Auto => &[
            "Throb1", "Throb2", "Throb4", "ThrobX1", "ThrobX2", "ThrobX4",
        ],
        VariantChoice::Throb1 => &["Throb1", "Throb2"],
        VariantChoice::Throb2 => &["Throb2"],
        VariantChoice::Throb4 => &["Throb4"],
        VariantChoice::ThrobX1 => &["ThrobX1", "ThrobX2"],
        VariantChoice::ThrobX2 => &["ThrobX2"],
        VariantChoice::ThrobX4 => &["ThrobX4"],
    };
    VARIANTS
        .iter()
        .filter(|v| names.contains(&v.name))
        .copied()
        .collect()
}

/// Decodes a buffer of 8 kHz complex audio (at least 8 s): the comb, the variant, the timing and the text. `None` below a
/// score of 0.55.
pub fn decode(x: &[Complex32], choice: VariantChoice) -> Option<Reading> {
    if x.len() < 8 * 8000 {
        return None;
    }
    let n = 16384;
    let spec = spectrum(x, n);
    let bin = 8000.0 / n as f64;
    let (b_lo, b_hi) = (
        (300.0 / bin).ceil() as usize,
        (3500.0 / bin).floor() as usize,
    );
    let mut band: Vec<f64> = spec[b_lo..=b_hi].to_vec();
    band.sort_by(f64::total_cmp);
    let med = band[band.len() / 2].max(1e-30);
    let logv: Vec<f64> = spec.iter().map(|&p| (p / med).max(1e-12).log10()).collect();
    // The comb score of every centre, the best over the four tone sets.
    let sets = [(9usize, 8.0f64), (9, 16.0), (11, 7.8125), (11, 15.625)];
    let mut scored: Vec<(f64, f64)> = Vec::new();
    let mut c2 = 2 * b_lo;
    while c2 <= 2 * b_hi {
        let c = c2 as f64 * bin / 2.0;
        let mut best = f64::MIN;
        for &(nt, sp) in &sets {
            let lo = c - (nt as f64 - 1.0) / 2.0 * sp;
            let hi = c + (nt as f64 - 1.0) / 2.0 * sp;
            if lo < 300.0 || hi > 3500.0 {
                continue;
            }
            let mut m = 0.0;
            for k in 0..nt {
                let b = ((lo + k as f64 * sp) / bin).round() as usize;
                m += logv[b - 1].max(logv[b]).max(logv[b + 1]);
            }
            best = best.max(m / nt as f64);
        }
        if best > f64::MIN {
            scored.push((best, c));
        }
        c2 += 1;
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut cands: Vec<f64> = Vec::new();
    for &(_, c) in &scored {
        if cands.iter().all(|&o| (o - c).abs() >= 40.0) {
            cands.push(c);
            if cands.len() == 2 {
                break;
            }
        }
    }
    let mut best: Option<Try> = None;
    for &c in &cands {
        for v in allowed(choice) {
            if let Some(t) = try_variant(x, c, &v, &spec, bin) {
                if best.as_ref().is_none_or(|b| t.score > b.score) {
                    best = Some(t);
                }
            }
        }
    }
    let mut best = best?;
    if best.score < 0.55 {
        return None;
    }
    // A slow signal read at half its symbol length gives each pair twice.
    if choice == VariantChoice::Auto && (best.variant.symbol_s - 0.512).abs() < 1e-9 {
        let s = &best.symbols;
        let doubled = (0..2).any(|parity| {
            let (mut eq, mut n) = (0, 0);
            let mut i = parity;
            while i + 1 < s.len() {
                if s[i].is_some() || s[i + 1].is_some() {
                    n += 1;
                    if s[i] == s[i + 1] {
                        eq += 1;
                    }
                }
                i += 2;
            }
            n > 0 && eq * 5 > n * 4
        });
        if doubled {
            let slow = VARIANTS
                .iter()
                .find(|v| v.is_x() == best.variant.is_x() && (v.symbol_s - 1.024).abs() < 1e-9)
                .copied();
            if let Some(t) = slow.and_then(|v| try_variant(x, best.centre, &v, &spec, bin)) {
                if t.score > 0.5 {
                    best = t;
                }
            }
        }
    }
    Some(Reading {
        variant: best.variant,
        centre_hz: best.centre,
        text: text_of(&best.variant, &best.symbols),
        score: best.score,
        snr_db: best.snr_db,
    })
}

sdrkit_core::choice_enum! {
    /// The variant setting.
    pub enum VariantChoice {
        /// Found from the signal.
        Auto = ("auto", "Automatic"),
        /// Throb1 (or Throb2).
        Throb1 = ("throb1", "Throb1"),
        /// Throb2.
        Throb2 = ("throb2", "Throb2"),
        /// Throb4.
        Throb4 = ("throb4", "Throb4"),
        /// ThrobX1 (or ThrobX2).
        ThrobX1 = ("throbx1", "ThrobX1"),
        /// ThrobX2.
        ThrobX2 = ("throbx2", "ThrobX2"),
        /// ThrobX4.
        ThrobX4 = ("throbx4", "ThrobX4"),
    }
}

/// The variant setting.
pub const VARIANT_SETTING: Setting<field::Choice<VariantChoice>> = Setting::choice(
    "variant",
    "Variant",
    VariantChoice::Auto,
)
.help(
    "Automatic finds the variant from the signal. Throb1 also reads Throb2, ThrobX1 also ThrobX2.",
);

sdrkit_core::settings! {
    /// The settings of [`ThrobDecoder`].
    pub enum ThrobSetting {
        /// The variant.
        Variant = VARIANT_SETTING,
    }
}

/// The note of the stack.
pub const NOTE_TEXT: &str =
    "Not confirmed by a public document: both tables of tone pairs, the pulse shapes and the \
alternation of the ThrobX idle and space pairs.";

/// Audio kept, seconds.
const KEEP_S: f64 = 30.0;
/// Audio needed for the first decode, seconds.
const FIRST_S: f64 = 14.0;
/// A decode every, seconds.
const EVERY_S: f64 = 6.0;
/// Characters of the running text compared with each new reading.
const COMPARE: usize = 400;

/// The characters of `new` that `old` does not end with: after the longest overlap of the end of `old` with a part of `new`. With
/// an overlap of fewer than 3 characters, nothing when `new` is already inside `old`, else all of `new`.
pub fn new_part<'a>(old: &str, new: &'a str) -> &'a str {
    let o: Vec<char> = old.chars().collect();
    let n: Vec<char> = new.chars().collect();
    let mut best = (0usize, 0usize);
    for start in 0..n.len() {
        for len in (1..=(n.len() - start).min(o.len())).rev() {
            if o[o.len() - len..] == n[start..start + len] {
                if len > best.0 {
                    best = (len, start + len);
                }
                break;
            }
        }
    }
    if best.0 >= 3 {
        let byte = new.char_indices().nth(best.1).map_or(new.len(), |(i, _)| i);
        return &new[byte..];
    }
    if old.contains(new) {
        ""
    } else {
        new
    }
}

/// The Throb receiver: 8 kHz complex sideband audio in, the running text out.
pub struct ThrobDecoder {
    choice: VariantChoice,
    buf: Vec<Complex32>,
    n: u64,
    next: u64,
    seen: String,
    store: TextStore,
    heard: Heard,
    chars: u64,
    last: Option<Reading>,
    dial: f64,
    status: Status,
}

impl Default for ThrobDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ThrobDecoder {
    /// A receiver at the default settings.
    pub fn new() -> Self {
        ThrobDecoder {
            choice: VariantChoice::Auto,
            buf: Vec::new(),
            n: 0,
            next: (FIRST_S * 8000.0) as u64,
            seen: String::new(),
            store: decoder::store(8000, 0),
            heard: Heard::new("Throb"),
            chars: 0,
            last: None,
            dial: 0.0,
            status: Status::default(),
        }
    }

    fn reset(&mut self) {
        self.buf.clear();
        self.next = self.n + (FIRST_S * 8000.0) as u64;
    }

    fn run(&mut self, out: &mut Outbox) {
        let Some(r) = decode(&self.buf, self.choice) else {
            return;
        };
        let new = new_part(&self.seen, &r.text).to_owned();
        let time = self.n as f64 / 8000.0;
        let freq = self.dial + r.centre_hz;
        for c in new.bytes() {
            self.chars += 1;
            self.heard.char(
                &mut self.store,
                out,
                c,
                time,
                (r.score.min(1.0) * 100.0) as u8,
                0,
                freq,
                r.snr_db as f32,
            );
        }
        if !new.is_empty() {
            use decoder::Emit;
            out.event(THROB_TEXT.event(
                time,
                freq,
                &ThrobTextEvent {
                    text: new.as_str(),
                    variant: r.variant.name,
                    centre_hz: Hertz(r.centre_hz),
                    family: Family::Message,
                },
            ));
            self.seen.push_str(&new);
            let extra = self.seen.chars().count().saturating_sub(COMPARE);
            if extra > 0 {
                let byte = self.seen.char_indices().nth(extra).map_or(0, |(i, _)| i);
                self.seen.drain(..byte);
            }
        }
        self.last = Some(r);
    }
}

impl sdrkit_graph::Decoder for ThrobDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.throb"
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn settings(&self) -> Arc<Schema> {
        ThrobSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match ThrobSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ThrobSetting::Variant => self.choice = VARIANT_SETTING.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([(ThrobSetting::Variant, Value::Choice(self.choice.index()))])
    }
    fn results(&self) -> Published {
        crate::typed::results("throb")
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.dial = input.tuned_hz;
        self.reset();
        Ok(decoder::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.reset();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        let mut rest = x;
        while !rest.is_empty() {
            let room = (self.next.saturating_sub(self.n) as usize).max(1);
            let (now, later) = rest.split_at(room.min(rest.len()));
            self.buf.extend(now.iter().map(|s| {
                if s.re.is_finite() && s.im.is_finite() {
                    *s
                } else {
                    Complex32::default()
                }
            }));
            self.n += now.len() as u64;
            rest = later;
            if self.n >= self.next {
                let keep = (KEEP_S * 8000.0) as usize;
                if self.buf.len() > keep {
                    self.buf.drain(..self.buf.len() - keep);
                }
                self.run(out);
                self.next += (EVERY_S * 8000.0) as u64;
            }
        }
    }
    fn finish(&mut self, out: &mut Outbox) {
        self.run(out);
    }
    fn report(&mut self, res: &mut Published) {
        let st = &mut self.status;
        st.lock_state = if self.last.is_some() {
            LockState::Locked
        } else {
            LockState::Searching
        };
        st.snr_db = self.last.as_ref().map(|r| r.snr_db as f32);
        st.offset_hz = Some(0.0);
        match &self.last {
            Some(r) => put_text(
                &mut st.summary,
                format_args!("{} at {:.0} Hz", r.variant.name, r.centre_hz),
            ),
            None => put_text(&mut st.summary, format_args!("Throb: listening")),
        }
        res.results_mut().set_status(st);
        self.heard.update(&self.store);
        let last = self.last.as_ref();
        res.put(&ThrobFields {
            text: self.heard.text(),
            callsign: last_call(&self.store),
            variant: last.map(|r| r.variant.name),
            centre_hz: last.map(|r| Hertz(r.centre_hz)),
            score: last.map(|r| (r.score * 1000.0).round() / 1000.0),
            snr_db: last.map(|r| (r.snr_db * 10.0).round() / 10.0),
            characters: self.chars as i64,
            note: NOTE_TEXT,
        });
        self.heard.report(res);
        flush_logs(&mut self.heard.logs, res);
    }
    fn commands(&self) -> &'static [&'static Command] {
        HeardCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let p = self.results();
        self.heard.invoke(id, args, &p)
    }
    fn clear(&mut self) {
        self.store.clear();
        self.heard.clear();
        self.seen.clear();
        self.chars = 0;
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        self.last = None;
        self.seen.clear();
        if !kept.field("text") {
            self.store.clear();
        }
        self.heard.forget(kept);
    }
}

/// The variant of the test signal.
pub const GEN_VARIANT: Setting<field::Choice<GenVariant>> =
    Setting::choice("gen_variant", "Test signal variant", GenVariant::Throb1)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::choice_enum! {
    /// The variant of the test signal.
    pub enum GenVariant {
        /// Throb1.
        Throb1 = ("throb1", "Throb1"),
        /// Throb2.
        Throb2 = ("throb2", "Throb2"),
        /// Throb4.
        Throb4 = ("throb4", "Throb4"),
        /// ThrobX1.
        ThrobX1 = ("throbx1", "ThrobX1"),
        /// ThrobX2.
        ThrobX2 = ("throbx2", "ThrobX2"),
        /// ThrobX4.
        ThrobX4 = ("throbx4", "ThrobX4"),
    }
}

sdrkit_core::settings! {
    /// The test-signal settings of the Throb stack.
    pub enum ThrobGenSetting {
        /// The text sent.
        Message = GEN_MESSAGE,
        /// The variant.
        Variant = GEN_VARIANT,
        /// The centre.
        Centre = crate::decoder::GEN_CENTRE_HZ,
    }
}

/// The Throb transmitter: the symbols of a text, each pair of tones at half amplitude (one tone at full amplitude), shaped by
/// the variant's pulse, every tone from a free-running oscillator. The symbols repeat.
pub struct ThrobModulator {
    v: Variant,
    rate: f64,
    centre: f64,
    symbols: Vec<usize>,
    n: u64,
    phase: Vec<f64>,
}

impl ThrobModulator {
    /// Sends `text` in variant `v` at `rate`, the tones around `centre` Hz from the dial (the dial at 0 Hz of the samples).
    pub fn new(v: Variant, rate: f64, centre: f64, text: &str) -> Self {
        ThrobModulator {
            v,
            rate,
            centre,
            symbols: encode(&v, text),
            n: 0,
            phase: vec![0.0; v.tones],
        }
    }

    /// The pulse at `x` (0 to 1) of a symbol.
    fn pulse(&self, x: f64) -> f64 {
        if self.v.full_cosine {
            return 0.5 * (1.0 - (TAU * x).cos());
        }
        if x < 0.2 {
            0.5 * (1.0 - (PI * x / 0.2).cos())
        } else if x > 0.8 {
            0.5 * (1.0 - (PI * (1.0 - x) / 0.2).cos())
        } else {
            1.0
        }
    }
}

impl Modulator for ThrobModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        let ls = self.v.symbol_s * self.rate;
        for o in out.iter_mut() {
            let t = self.n as f64 / ls;
            let k = t.floor() as usize;
            let env = self.pulse(t - k as f64);
            let (a, b) = self.v.pairs()[self.symbols[k % self.symbols.len()]];
            let mut s = Complex32::default();
            for (i, ph) in self.phase.iter_mut().enumerate() {
                let f = self.centre + self.v.tone_offset(i + 1);
                *ph = (*ph + TAU * f / self.rate).rem_euclid(TAU);
                let amp = if a == b {
                    if i + 1 == usize::from(a) {
                        1.0
                    } else {
                        0.0
                    }
                } else if i + 1 == usize::from(a) || i + 1 == usize::from(b) {
                    0.5
                } else {
                    0.0
                };
                if amp > 0.0 {
                    s += Complex32::new(ph.cos() as f32, ph.sin() as f32) * (amp * env) as f32;
                }
            }
            *o = s;
            self.n += 1;
        }
    }
}

/// The test text of Throb.
pub const TEST_TEXT: &str = "CQ CQ DE K0TST K";

/// The generator: `gen_message` (else [`TEST_TEXT`]) in the variant of `gen_variant` around `gen_centre_hz`.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let v = VARIANTS[cfg.settings.get(&GEN_VARIANT).index() as usize];
    let centre = cfg.settings.get(&crate::decoder::GEN_CENTRE_HZ);
    let m = ThrobModulator::new(v, cfg.rate_hz, centre, &decoder::message(cfg, TEST_TEXT));
    let info = SignalInfo::new(2500.0)
        .with_symbol_rate(1.0 / v.symbol_s)
        .with_power(0.4);
    Ok(decoder::placed(cfg.rate_hz, m, info))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables() {
        let mut p: Vec<(u8, u8)> = PAIRS.to_vec();
        p.sort_unstable();
        p.dedup();
        assert_eq!(p.len(), 45);
        let mut x: Vec<(u8, u8)> = X_PAIRS.to_vec();
        x.sort_unstable();
        x.dedup();
        assert_eq!(x.len(), 55);
        let v = VARIANTS[0];
        let s = encode(&v, "CQ");
        assert_eq!(s, vec![0, 0, 0, 0, 3, 17, 0]);
        assert_eq!(PAIRS[3], (1, 3));
        assert_eq!((v.tone_offset(1), v.tone_offset(3)), (-32.0, -16.0));
        assert_eq!(PAIRS[17], (2, 9));
        assert_eq!((v.tone_offset(2), v.tone_offset(9)), (-24.0, 32.0));
        assert_eq!(VARIANTS[3].tone_offset(1), -39.0625);
        assert_eq!(VARIANTS[5].tone_offset(11), 78.125);
    }

    #[test]
    fn text_with_shifts_and_alternating_spaces() {
        let v = VARIANTS[1];
        let s: Vec<Option<usize>> = encode(&v, "TEST? A@B=C").into_iter().map(Some).collect();
        assert_eq!(text_of(&v, &s), "TEST? A@B=C");
        let x = VARIANTS[3];
        let e = encode(&x, "A  B");
        assert_eq!(e, vec![0, 1, 0, 1, 2, 1, 0, 3, 0]);
        let s: Vec<Option<usize>> = e.into_iter().map(Some).collect();
        assert_eq!(text_of(&x, &s), " A B ");
    }

    #[test]
    fn the_new_part_of_a_reading() {
        assert_eq!(new_part("", "CQ CQ"), "CQ CQ");
        assert_eq!(new_part("CQ CQ DE", "CQ DE K0TST"), " K0TST");
        assert_eq!(new_part("CQ CQ DE K0TST", "DE K0"), "");
        assert_eq!(new_part("ABC", "XY"), "XY");
    }
}
