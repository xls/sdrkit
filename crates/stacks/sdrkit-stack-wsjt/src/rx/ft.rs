//! FT8, FT4 and FT2 decoding, and the search, fit and tone measurements JS8 shares. Each slot is searched and decoded, and its
//! decodes are rebuilt and subtracted before the next search. A plain stage is followed by a deep stage; a candidate that the
//! deep stage cannot decode is combined with the failed candidates of earlier slots, then tried with the messages heard on the
//! band assumed, then stored ([`super::ftmem`]). An early slot (the audio so far) runs the plain stage only, with the symbols
//! after the audio as erasures.

use super::ftmem::{FtMemory, Hypothesis, SoftStore, N, SETS};
use super::slot::*;
use crate::encode::{self, COSTAS8, GRAY4, GRAY8, SYNC4};
use crate::mode::{Mode, Sub};
use crate::proto::codes;
use crate::proto::msg77::{unpack77_learn, CallHashes, RVEC};
use num_complex::Complex32;
use sdrkit_util_bits::crc::cyclic::FT8_CRC14;
use sdrkit_util_bits::pack::{push_u64, to_u64 as get_bits};
use std::f64::consts::PI;

/// What a decoder uses besides the slot.
pub struct Ctx<'a> {
    /// The receiver's callsign hash table.
    pub hashes: &'a mut CallHashes,
    /// Search upside-down signals too.
    pub mirrored: bool,
    /// Run the deep stage.
    pub deep: bool,
    /// What the stack keeps across slots, for combining and a-priori decoding; `None` for neither.
    pub memory: Option<&'a mut FtMemory>,
}

/// The frame of an FT-like mode.
#[derive(Clone, Debug)]
pub struct Layout {
    /// Samples per symbol at 8 kHz.
    pub nss: usize,
    /// Tones.
    pub m: usize,
    /// Bits per data symbol.
    pub bits: usize,
    /// Symbols of the frame (ramp symbols included).
    pub nsym: usize,
    /// Sync symbols: (index, tone).
    pub sync: Vec<(usize, u8)>,
    /// Data symbol indices.
    pub data: Vec<usize>,
    /// `label[tone]`: the bits a data tone carries.
    pub label: Vec<u8>,
    /// Baseband decimation from 8 kHz.
    pub decim: usize,
    /// Candidates per pass and orientation.
    pub max_cand: usize,
    /// Passes in each stage.
    pub passes: usize,
    /// The coherent group sizes of the deep stage.
    pub groups: [usize; 2],
    /// Tone spacing, Hz.
    pub spacing: f64,
}

/// The layout of FT8, FT4 or FT2.
pub fn layout(sub: &Sub) -> Layout {
    let spacing = sub.spacing_hz();
    match sub.mode {
        Mode::Ft8 => {
            let mut sync = Vec::new();
            for b in [0, 36, 72] {
                for (i, &t) in COSTAS8.iter().enumerate() {
                    sync.push((b + i, t));
                }
            }
            let data: Vec<usize> = (7..36).chain(43..72).collect();
            let mut label = vec![0u8; 8];
            for (v, &t) in GRAY8.iter().enumerate() {
                label[t as usize] = v as u8;
            }
            Layout {
                nss: 1280,
                m: 8,
                bits: 3,
                nsym: 79,
                sync,
                data,
                label,
                decim: 40,
                max_cand: 150,
                passes: 3,
                groups: [2, 3],
                spacing,
            }
        }
        _ => {
            let ft2 = sub.mode == Mode::Ft2;
            let mut sync = Vec::new();
            for (w, b) in [1usize, 34, 67, 100].into_iter().enumerate() {
                for (i, &t) in SYNC4[w].iter().enumerate() {
                    sync.push((b + i, t));
                }
            }
            let data: Vec<usize> = (5..34).chain(38..67).chain(71..100).collect();
            let mut label = vec![0u8; 4];
            for (v, &t) in GRAY4.iter().enumerate() {
                label[t as usize] = v as u8;
            }
            Layout {
                nss: if ft2 { 192 } else { 384 },
                m: 4,
                bits: 2,
                nsym: 105,
                sync,
                data,
                label,
                decim: if ft2 { 12 } else { 24 },
                max_cand: if ft2 { 60 } else { 100 },
                passes: 2,
                groups: [2, 4],
                spacing,
            }
        }
    }
}

/// A place where a signal can be.
#[derive(Clone, Copy, Debug)]
pub struct Cand {
    /// The spectrogram row of the transmission's start.
    pub row: i64,
    /// The bin of tone 0.
    pub bin: i64,
    /// The sync score.
    pub score: f32,
    /// Tones running downwards.
    pub mirrored: bool,
}

/// Sync candidates of one orientation: per bin the best start, local maxima over +-2 bins above
/// max(1.45, 1.15 x median), best first.
pub fn candidates(
    sp: &Spectrogram,
    lay: &Layout,
    slot: &Slot,
    mirrored: bool,
    f_lo: f64,
    f_hi: f64,
    cap: usize,
) -> Vec<Cand> {
    let sub = &slot.sub;
    let (lo, hi) = slot.dt_range();
    let step = sp.step as f64;
    let r_lo = (slot.sample_of(sub.t0() + lo) / step).floor() as i64;
    let r_hi = (slot.sample_of(sub.t0() + hi) / step).ceil() as i64;
    let span = (lay.m - 1) as f64 * lay.spacing;
    let (b_lo, b_hi) = if mirrored {
        (
            ((f_lo + span) / sp.df).ceil() as i64,
            ((f_hi + span).min(3900.0) / sp.df).floor() as i64,
        )
    } else {
        (
            (f_lo / sp.df).ceil() as i64,
            (f_hi.min(3900.0 - span) / sp.df).floor() as i64,
        )
    };
    if b_hi <= b_lo || r_hi < r_lo {
        return Vec::new();
    }
    let sgn: i64 = if mirrored { -1 } else { 1 };
    let per_sym = (lay.nss / sp.step) as i64;
    let nb = (b_hi - b_lo + 1) as usize;
    let mut best = vec![0f32; nb];
    let mut best_row = vec![0i64; nb];
    // Per row: the sum over the m tone bins of every tone-0 bin (a sliding sum along the bins).
    let width = 2 * (lay.m - 1);
    let mut tone_sum = vec![0f32; sp.rows * nb];
    for r in 0..sp.rows {
        let row = &sp.p[r * sp.bins..(r + 1) * sp.bins];
        let out = &mut tone_sum[r * nb..(r + 1) * nb];
        for (i, o) in out.iter_mut().enumerate() {
            let b = b_lo + i as i64;
            let mut acc = 0f32;
            let mut t = 0;
            while t <= width {
                let bin = b + sgn * t as i64;
                if bin >= 0 && (bin as usize) < sp.bins {
                    acc += row[bin as usize];
                }
                t += 2;
            }
            *o = acc;
        }
    }
    let mut num = vec![0f32; nb];
    let mut all = vec![0f32; nb];
    let inv = 1.0 / (lay.m - 1) as f32;
    for r in r_lo..=r_hi {
        num.iter_mut().for_each(|x| *x = 0.0);
        all.iter_mut().for_each(|x| *x = 0.0);
        for &(k, t) in &lay.sync {
            let row = r + per_sym * k as i64;
            if row < 0 || row as usize >= sp.rows {
                continue;
            }
            let p = &sp.p[row as usize * sp.bins..(row as usize + 1) * sp.bins];
            let off = b_lo + sgn * 2 * t as i64;
            let ts = &tone_sum[row as usize * nb..(row as usize + 1) * nb];
            for i in 0..nb {
                let bin = off + i as i64;
                if bin >= 0 && (bin as usize) < sp.bins {
                    num[i] += p[bin as usize];
                }
                all[i] += ts[i];
            }
        }
        for i in 0..nb {
            let den = (all[i] - num[i]) * inv;
            if den > 0.0 {
                let s = num[i] / den;
                if s > best[i] {
                    best[i] = s;
                    best_row[i] = r;
                }
            }
        }
    }
    let med = median(&best);
    let thr = 1.45f32.max(1.15 * med);
    let mut out = Vec::new();
    for i in 0..nb {
        let s = best[i];
        if s < thr {
            continue;
        }
        let lo_i = i.saturating_sub(2);
        let hi_i = (i + 2).min(nb - 1);
        if (lo_i..=hi_i).any(|j| j != i && (best[j] > s || (best[j] == s && j < i))) {
            continue;
        }
        out.push(Cand {
            row: best_row[i],
            bin: b_lo + i as i64,
            score: s,
            mirrored,
        });
    }
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out.truncate(cap);
    out
}

/// A fitted signal in a baseband.
#[derive(Clone, Debug)]
pub struct Fitted {
    /// The baseband sample of the frame's start.
    pub i0: i64,
    /// The fractional part of the start, samples.
    pub frac: f64,
    /// The frequency offset from the baseband's 0 Hz, Hz.
    pub df: f64,
    /// The coherent sync energy.
    pub energy: f32,
}

/// Tone correlations of a baseband: the tone tables for one frequency offset.
pub struct Refs {
    sps: usize,
    m: usize,
    sgn: f64,
    /// `tab[t * sps + n]`.
    tab: Vec<Complex32>,
}

impl Refs {
    /// The tone tables of the `m` tones `spacing` apart (`sgn` -1: downwards), offset `df`, at `fs` with `sps` samples a symbol.
    pub fn new(m: usize, spacing: f64, sgn: f64, df: f64, fs: f64, sps: usize) -> Refs {
        let mut tab = Vec::with_capacity(m * sps);
        for t in 0..m {
            let f = sgn * t as f64 * spacing + df;
            for n in 0..sps {
                let p = -2.0 * PI * f * n as f64 / fs;
                tab.push(Complex32::new(p.cos() as f32, p.sin() as f32));
            }
        }
        Refs { sps, m, sgn, tab }
    }
    /// The correlation of the symbol starting at `start` with tone `t`.
    #[inline]
    pub fn corr(&self, bb: &[Complex32], start: i64, t: usize) -> Complex32 {
        let r = &self.tab[t * self.sps..(t + 1) * self.sps];
        let mut acc = Complex32::default();
        if start < 0 || start as usize + self.sps > bb.len() {
            for (n, w) in r.iter().enumerate() {
                let j = start + n as i64;
                if j >= 0 && (j as usize) < bb.len() {
                    acc += bb[j as usize] * w;
                }
            }
            return acc;
        }
        let s = &bb[start as usize..start as usize + self.sps];
        for (x, w) in s.iter().zip(r) {
            acc += x * w;
        }
        acc
    }
    /// The orientation.
    pub fn sign(&self) -> f64 {
        self.sgn
    }
    /// Tones.
    pub fn tones(&self) -> usize {
        self.m
    }
}

/// The coherent sync energy of a frame starting at baseband sample `i`: the correlations of the symbols of each sync array added
/// coherently (a continuous-phase transmission keeps its phase from symbol to symbol; the frequency offset `df` of the tone tables
/// is referred to the frame's start), the arrays summed in power.
pub fn sync_energy(
    bb: &[Complex32],
    refs: &Refs,
    sync: &[(usize, u8)],
    sps: usize,
    i: i64,
    df: f64,
    fs: f64,
) -> f32 {
    if fs <= 0.0 {
        // Each symbol on its own (the frequency only roughly known).
        return sync
            .iter()
            .map(|&(k, t)| refs.corr(bb, i + (k * sps) as i64, t as usize).norm_sqr())
            .sum();
    }
    let mut total = 0f32;
    let mut acc = Complex32::default();
    let mut last: Option<usize> = None;
    for &(k, t) in sync {
        if last.is_some_and(|l| k != l + 1) {
            total += acc.norm_sqr();
            acc = Complex32::default();
        }
        last = Some(k);
        let p = -2.0 * PI * df * (k * sps) as f64 / fs;
        acc += refs.corr(bb, i + (k * sps) as i64, t as usize)
            * Complex32::new(p.cos() as f32, p.sin() as f32);
    }
    total + acc.norm_sqr()
}

/// The fine fit: time over +-1/2 symbol, frequency over +-1/2 spacing in sixteenths, time again, a half-step frequency
/// refinement, then parabolic interpolation in time and frequency.
pub fn fit(
    bb: &[Complex32],
    lay: &Layout,
    sync: &[(usize, u8)],
    sgn: f64,
    fs: f64,
    i_guess: i64,
    df_guess: f64,
) -> Fitted {
    let sps = lay.nss / lay.decim;
    let sp = lay.spacing;
    // `coherent`: the symbols of each sync array added coherently (fine frequency); else each symbol on its own (rough).
    let e_at = |i: i64, df: f64, coherent: bool| -> f32 {
        let r = Refs::new(lay.m, sp, sgn, df, fs, sps);
        sync_energy(bb, &r, sync, sps, i, df, if coherent { fs } else { 0.0 })
    };
    let half = sps as i64 / 2;
    // Time over +-1/2 symbol at the guessed frequency.
    let r0 = Refs::new(lay.m, sp, sgn, df_guess, fs, sps);
    let mut bi = i_guess;
    let mut be = f32::MIN;
    for i in i_guess - half..=i_guess + half {
        let e = sync_energy(bb, &r0, sync, sps, i, df_guess, 0.0);
        if e > be {
            be = e;
            bi = i;
        }
    }
    // Frequency over +-1/2 spacing in sixteenths.
    let mut bf = df_guess;
    for k in -8..=8 {
        let f = df_guess + k as f64 * sp / 16.0;
        let e = e_at(bi, f, false);
        if e > be {
            be = e;
            bf = f;
        }
    }
    // Time again.
    let r1 = Refs::new(lay.m, sp, sgn, bf, fs, sps);
    for i in bi - 4..=bi + 4 {
        let e = sync_energy(bb, &r1, sync, sps, i, bf, 0.0);
        if e > be {
            be = e;
            bi = i;
        }
    }
    // The coherent refinement: frequency in steps of 1/64 spacing over +-1/4 spacing, then time, then parabolic interpolation.
    let step = sp / 64.0;
    let mut ce = f32::MIN;
    let centre = bf;
    for k in -16..=16 {
        let f = centre + k as f64 * step;
        let e = e_at(bi, f, true);
        if e > ce {
            ce = e;
            bf = f;
        }
    }
    let rb = Refs::new(lay.m, sp, sgn, bf, fs, sps);
    for i in bi - 2..=bi + 2 {
        let e = sync_energy(bb, &rb, sync, sps, i, bf, fs);
        if e > ce {
            ce = e;
            bi = i;
        }
    }
    let em = sync_energy(bb, &rb, sync, sps, bi - 1, bf, fs);
    let ep = sync_energy(bb, &rb, sync, sps, bi + 1, bf, fs);
    let frac = parabola(em, ce, ep);
    let fm = e_at(bi, bf - step, true);
    let fp = e_at(bi, bf + step, true);
    let ff = parabola(fm, ce, fp) * step;
    Fitted {
        i0: bi,
        frac,
        df: bf + ff,
        energy: ce,
    }
}

fn parabola(a: f32, b: f32, c: f32) -> f64 {
    let d = a - 2.0 * b + c;
    if d.abs() < 1e-20 || d >= 0.0 {
        return 0.0;
    }
    (0.5 * (a - c) / d).clamp(-0.5, 0.5) as f64
}

/// The complex amplitudes of every tone of every symbol: `out[k * m + t]`, with the frequency offset's phase referred to the
/// frame's start (so neighbouring symbols are coherent).
pub fn tone_amplitudes(
    bb: &[Complex32],
    lay: &Layout,
    sgn: f64,
    fs: f64,
    fit: &Fitted,
    out: &mut Vec<Complex32>,
) {
    let sps = lay.nss / lay.decim;
    let refs = Refs::new(lay.m, lay.spacing, sgn, fit.df, fs, sps);
    out.clear();
    for k in 0..lay.nsym {
        let start = fit.i0 + (k * sps) as i64;
        let rot = {
            let p = -2.0 * PI * fit.df * (k * sps) as f64 / fs;
            Complex32::new(p.cos() as f32, p.sin() as f32)
        };
        for t in 0..lay.m {
            out.push(refs.corr(bb, start, t) * rot);
        }
    }
}

/// Coherent soft bits (positive means 0) of groups of `g` neighbouring data symbols, scaled to the spread of
/// single-symbol soft bits.
#[allow(clippy::needless_range_loop)]
pub fn group_llrs(c: &[Complex32], lay: &Layout, g: usize, out: &mut Vec<f32>) {
    out.clear();
    let m = lay.m;
    let bits = lay.bits;
    let parts: Vec<&[usize]> = if lay.m == 8 {
        vec![&lay.data[..29], &lay.data[29..]]
    } else {
        vec![&lay.data[..29], &lay.data[29..58], &lay.data[58..]]
    };
    for blk in parts {
        for chunk in blk.chunks(g) {
            let gs = chunk.len();
            let nseq = m.pow(gs as u32);
            let nb = gs * bits;
            let mut m1 = vec![f32::MIN; nb];
            let mut m0 = vec![f32::MIN; nb];
            for seq in 0..nseq {
                let mut s = Complex32::default();
                let mut x = seq;
                let mut tones = [0usize; 4];
                for j in (0..gs).rev() {
                    tones[j] = x % m;
                    x /= m;
                }
                for j in 0..gs {
                    s += c[chunk[j] * m + tones[j]];
                }
                let mag = s.norm();
                for j in 0..gs {
                    let lab = lay.label[tones[j]];
                    for b in 0..bits {
                        let idx = j * bits + b;
                        if (lab >> (bits - 1 - b)) & 1 == 1 {
                            if mag > m1[idx] {
                                m1[idx] = mag;
                            }
                        } else if mag > m0[idx] {
                            m0[idx] = mag;
                        }
                    }
                }
            }
            for i in 0..nb {
                out.push(m0[i] - m1[i]);
            }
        }
    }
    normalise_llrs(out);
}

/// Scales soft bits to a standard deviation of 2.83, clamped to +-25.
pub fn normalise_llrs(v: &mut [f32]) {
    let n = v.len().max(1) as f32;
    let mean = v.iter().sum::<f32>() / n;
    let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n;
    let sd = var.sqrt().max(1e-12);
    // Spread of the magnitudes: use the RMS about zero.
    let rms = (v.iter().map(|x| x * x).sum::<f32>() / n).sqrt().max(sd);
    let k = 2.83 / rms;
    for x in v.iter_mut() {
        *x = (*x * k).clamp(-25.0, 25.0);
    }
}

/// The SNR of a decode from its tone amplitudes and decoded tones.
pub fn snr_of(c: &[Complex32], lay: &Layout, tones: &[u8]) -> f64 {
    snr_before(c, lay, tones, lay.nsym)
}

/// The SNR of a decode from the data symbols before symbol `live` (the symbols from `live` on are erasures).
pub fn snr_before(c: &[Complex32], lay: &Layout, tones: &[u8], live: usize) -> f64 {
    let (mut ps, mut po) = (0f64, 0f64);
    for &k in lay.data.iter().filter(|&&k| k < live) {
        let sent = tones[k] as usize;
        for t in 0..lay.m {
            let p = c[k * lay.m + t].norm_sqr() as f64;
            if t == sent {
                ps += p;
            } else {
                po += p / (lay.m - 1) as f64;
            }
        }
    }
    snr_2500(ps, po, lay.spacing)
}

/// The sent tones of a decoded codeword (FT8 79, FT4 / FT2 105 with ramps).
pub fn tones_of(sub: &Sub, cw: &[u8]) -> Vec<u8> {
    match sub.mode {
        Mode::Ft8 => encode::ft8_tones(cw),
        _ => encode::frame_tones(sub, &encode::ft4_tones(cw)),
    }
}

struct Decoded {
    text: String,
    cw: Vec<u8>,
    deep: bool,
    how: How,
}

/// When [`try_llrs`] applies the plausible-message guard (type 1, 2 or 4, no unknown hashed call).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Guard {
    /// To a code word that OSD found.
    WhenOsd,
    /// To every code word.
    Always,
    /// To every code word, which must also be a CQ.
    Cq,
}

/// Belief propagation of an a-priori trial stops after this many iterations without fewer unsatisfied checks.
const APRIORI_PATIENCE: u32 = 8;
/// An a-priori decode whose SNR, measured on its own tones, is below this (dB in 2500 Hz) is refused: noise that the assumed
/// bits pulled into a code word carries little power on the tones of that code word.
const APRIORI_MIN_SNR_DB: f64 = -24.0;

/// Tries a set of soft bits (positive means 0): BP of at most 40 iterations (stopping early after `patience` iterations without
/// progress, 0: never), then (`osd`) OSD; checks the CRC, `accept` on the code word, and the message with the guard `guard`.
#[allow(clippy::too_many_arguments)]
fn try_llrs(
    llr: &[f32],
    scramble: bool,
    osd: bool,
    patience: u32,
    dec: &mut LdpcDecoders,
    hashes: &mut CallHashes,
    guard: Guard,
    accept: impl Fn(&[u8]) -> bool,
) -> Option<Decoded> {
    let mut cw = vec![0u8; 174];
    let r = dec.decode_patient(llr, 40, patience, osd, &mut cw)?;
    if r.osd && r.errors > 30 {
        return None;
    }
    let deep = r.osd;
    if cw.iter().all(|&b| b == 0) {
        return None;
    }
    if !FT8_CRC14.check(&cw[..91]) || !accept(&cw) {
        return None;
    }
    let cq_only = guard == Guard::Cq;
    let guarded = deep || guard != Guard::WhenOsd;
    let mut msg: Vec<u8> = cw[..77].to_vec();
    if scramble {
        for (b, r) in msg.iter_mut().zip(RVEC.iter()) {
            *b ^= r;
        }
    }
    // The false-decode policy for deep results: plain QSO types only, no unknown hashed call.
    let i3 = get_bits(&msg[74..77]);
    if guarded && !matches!(i3, 1 | 2 | 4) {
        return None;
    }
    let probe = crate::proto::msg77::unpack77(&msg, hashes)?;
    if guarded && probe.unknown_hash {
        return None;
    }
    if cq_only && !probe.text.starts_with("CQ ") {
        return None;
    }
    let u = unpack77_learn(&msg, hashes)?;
    Some(Decoded {
        text: u.text,
        cw,
        deep: guarded,
        how: How::Alone,
    })
}

/// Cross-slot combining: the sets of this candidate with the stored entries `m` added, each entry alone, then (two or more) all
/// of them. For each sum: BP and the CRC on each set, then, when `score` is at least 2.5, OSD on each set with the plausible-message
/// guard.
#[allow(clippy::too_many_arguments)]
fn combine(
    store: &SoftStore,
    m: &[usize],
    sets: &[Vec<f32>],
    score: f32,
    scramble: bool,
    dec: &mut LdpcDecoders,
    hashes: &mut CallHashes,
    sum: &mut [f32],
) -> Option<Decoded> {
    let alone = m.chunks(1);
    let all = (m.len() >= 2).then_some(m);
    for group in alone.chain(all) {
        for osd in [false, true] {
            if osd && score < 2.5 {
                break;
            }
            for (k, set) in sets.iter().enumerate().take(SETS) {
                sum.copy_from_slice(set);
                for &i in group {
                    for (s, v) in sum.iter_mut().zip(store.set(i, k)) {
                        *s += v;
                    }
                }
                if let Some(mut d) = try_llrs(sum, scramble, osd, 0, dec, hashes, Guard::WhenOsd, |_| true) {
                    d.deep = true;
                    d.how = How::Combined;
                    return Some(d);
                }
            }
        }
    }
    None
}

/// A-priori decoding: for each hypothesis and the plain and 2-symbol sets, the known bits forced to 1.01 x the largest magnitude
/// of the set, then BP (and OSD for a pair). Accepted with the plausible-message guard and when, over code word bits 0-90 not
/// forced, the magnitudes of the set whose hard decision agrees with the decode make at least 0.7 of the total.
fn apriori(
    hyps: &[Hypothesis],
    sets: &[Vec<f32>],
    scramble: bool,
    dec: &mut LdpcDecoders,
    hashes: &mut CallHashes,
    ap: &mut [f32],
) -> Option<Decoded> {
    let mut forced = [false; N];
    for h in hyps {
        forced.fill(false);
        for i in h.known() {
            forced[i] = true;
        }
        for set in sets.iter().take(2) {
            let big = 1.01 * set.iter().fold(0f32, |a, &x| a.max(x.abs()));
            ap.copy_from_slice(set);
            for i in h.known() {
                let b = if scramble { h.bits[i] ^ RVEC[i] } else { h.bits[i] };
                ap[i] = if b == 1 { -big } else { big };
            }
            let agrees = |cw: &[u8]| {
                let (mut agree, mut all) = (0f32, 0f32);
                for i in (0..91).filter(|&i| !forced[i]) {
                    let a = set[i].abs();
                    all += a;
                    if u8::from(set[i] < 0.0) == cw[i] {
                        agree += a;
                    }
                }
                all > 0.0 && agree >= 0.7 * all
            };
            if let Some(mut d) = try_llrs(
                ap,
                scramble,
                h.pair,
                APRIORI_PATIENCE,
                dec,
                hashes,
                Guard::Always,
                agrees,
            ) {
                d.deep = true;
                d.how = How::APriori;
                return Some(d);
            }
        }
    }
    None
}

/// The a-priori soft bits (positive means 0) of "CQ ??? ???": the first call field set to CQ and the type to 1.
fn apply_cq_apriori(llr: &mut [f32], scramble: bool) {
    let big = 1.01 * llr.iter().fold(0f32, |a, &x| a.max(x.abs()));
    let mut bits = Vec::new();
    push_u64(&mut bits, 2, 28);
    bits.push(0);
    for (i, &b) in bits.iter().enumerate() {
        let b = if scramble { b ^ RVEC[i] } else { b };
        llr[i] = if b == 1 { -big } else { big };
    }
    for (j, &b) in [0u8, 0, 1].iter().enumerate() {
        let i = 74 + j;
        let b = if scramble { b ^ RVEC[i] } else { b };
        llr[i] = if b == 1 { -big } else { big };
    }
}

/// The FT8 / FT4 / FT2 decoder of one slot.
pub fn decode(slot: &Slot, ctx: &mut Ctx) -> Vec<Decode> {
    let sub = slot.sub;
    let lay = layout(&sub);
    let scramble = sub.mode != Mode::Ft8;
    let mut audio: Vec<f32> = slot
        .audio
        .iter()
        .map(|&x| if x.is_finite() { x } else { 0.0 })
        .collect();
    // An early slot: the time its audio ends, and zeros after it up to the end of the window.
    let early_end = slot.early.then(|| slot.time_of(audio.len() as f64));
    if slot.early {
        let full = slot.sample_of(sub.window().1).ceil().max(0.0) as usize;
        if audio.len() < full {
            audio.resize(full, 0.0);
        }
    }
    let mut matched = Vec::new();
    let mut work = vec![0f32; N];
    let fs = slot.rate / lay.decim as f64;
    let m_len = smooth_at_least(audio.len().div_ceil(lay.decim));
    let n_fft = m_len * lay.decim;
    let mut out: Vec<Decode> = Vec::new();
    // (frequency of tone 0, start time s, mirrored) of every decode made, for the duplicate and deep-stage checks.
    let mut made: Vec<(f64, f64, bool)> = Vec::new();
    let mut s = LdpcDecoders::new(codes::ft8());
    let mut bb = Vec::new();
    let mut c = Vec::new();
    let mut llr = Vec::with_capacity(174);
    let span = (lay.m - 1) as f64 * lay.spacing;
    let orientations: &[bool] = if ctx.mirrored {
        &[false, true]
    } else {
        &[false]
    };
    let mut pass_no = 0u32;
    for stage in 0..(1 + (ctx.deep && !slot.early) as usize) {
        let deep = stage == 1;
        for _ in 0..lay.passes {
            let sp = Spectrogram::new(&audio, lay.nss, lay.nss / 4, 2 * lay.nss, false, slot.rate);
            let spec = SlotSpectrum::new(&audio, n_fft);
            let mut found = 0;
            for &mir in orientations {
                let cands = candidates(&sp, &lay, slot, mir, 200.0, 3000.0, lay.max_cand);
                for cand in cands {
                    let sgn = if mir { -1.0 } else { 1.0 };
                    let f0 = cand.bin as f64 * sp.df;
                    let t_start = slot.time_of((cand.row * sp.step as i64) as f64);
                    // The deep stage skips candidates on top of a decode already made.
                    if deep
                        && made.iter().any(|&(f, t, m)| {
                            m == mir
                                && (f - f0).abs() < 0.75 * lay.spacing
                                && (t - t_start).abs() < 1.5 * sub.symbol_s()
                        })
                    {
                        continue;
                    }
                    let (flo, fhi) = if mir {
                        (f0 - span - 1.5 * lay.spacing, f0 + 1.5 * lay.spacing)
                    } else {
                        (f0 - 1.5 * lay.spacing, f0 + span + 1.5 * lay.spacing)
                    };
                    let shift = spec.baseband(f0, flo, fhi, 1.5 * lay.spacing, m_len, &mut bb);
                    let i_guess =
                        ((cand.row * sp.step as i64) as f64 / lay.decim as f64).round() as i64;
                    let offsets: &[f64] = if deep && lay.m == 4 {
                        &[0.0, 1.0 / 48.0, -1.0 / 48.0, 2.0 / 48.0, -2.0 / 48.0]
                    } else {
                        &[0.0]
                    };
                    let ft = fit(&bb, &lay, &lay.sync, sgn, fs, i_guess, f0 - shift);
                    // The symbols received whole: all but in an early slot, whose audio ends before the frame does.
                    let live = match early_end {
                        None => lay.nsym,
                        Some(end) => {
                            let t_frame =
                                slot.time_of((ft.i0 as f64 + ft.frac) * lay.decim as f64);
                            ((end - t_frame) / sub.symbol_s())
                                .floor()
                                .clamp(0.0, lay.nsym as f64) as usize
                        }
                    };
                    if 2 * lay.data.iter().filter(|&&k| k >= live).count() > lay.data.len() {
                        continue;
                    }
                    let mut got = None;
                    let mut first_sets: Option<Vec<Vec<f32>>> = None;
                    for (oi, &off) in offsets.iter().enumerate() {
                        let mut f = ft.clone();
                        f.df += off * lay.spacing;
                        tone_amplitudes(&bb, &lay, sgn, fs, &f, &mut c);
                        // Erasures: no tone amplitude, no part in the noise estimate.
                        let from = (live * lay.m).min(c.len());
                        c[from..].fill(Complex32::default());
                        let amps: Vec<f32> = lay
                            .data
                            .iter()
                            .filter(|&&k| k < live)
                            .flat_map(|&k| (0..lay.m).map(move |t| (k, t)))
                            .map(|(k, t)| c[k * lay.m + t].norm())
                            .collect();
                        llr.clear();
                        fsk_llrs(&amps, lay.m, lay.bits, &lay.label, &mut llr);
                        if live < lay.nsym && !llr.is_empty() {
                            llr.resize(lay.data.len() * lay.bits, 0.0);
                        }
                        let mut sets = vec![llr.clone()];
                        if deep {
                            for g in lay.groups {
                                let mut v = Vec::new();
                                group_llrs(&c, &lay, g, &mut v);
                                sets.push(v);
                            }
                        }
                        let osd = deep && oi == 0 && cand.score >= 2.5;
                        for set in &sets {
                            if let Some(d) = try_llrs(
                                set,
                                scramble,
                                osd,
                                0,
                                &mut s,
                                ctx.hashes,
                                Guard::WhenOsd,
                                |_| true,
                            ) {
                                got = Some((d, f.clone()));
                                break;
                            }
                        }
                        if got.is_none() && deep && oi == 0 {
                            let mut ap = sets[0].clone();
                            apply_cq_apriori(&mut ap, scramble);
                            if let Some(d) = try_llrs(
                                &ap,
                                scramble,
                                cand.score >= 2.5,
                                0,
                                &mut s,
                                ctx.hashes,
                                Guard::Cq,
                                |_| true,
                            ) {
                                got = Some((d, f.clone()));
                            }
                        }
                        if got.is_some() {
                            break;
                        }
                        if deep && oi == 0 {
                            first_sets = Some(sets);
                        }
                    }
                    // A deep candidate that failed at its fitted frequency: combined with earlier slots, tried with the calls
                    // heard, then stored. One stored for this slot in an earlier pass has failed both already.
                    let hz0 = shift + ft.df;
                    let dt0 =
                        slot.time_of((ft.i0 as f64 + ft.frac) * lay.decim as f64) - sub.t0();
                    if let (None, Some(sets), Some(mem), false) = (
                        got.as_ref(),
                        first_sets.as_ref(),
                        ctx.memory.as_deref_mut(),
                        slot.search,
                    ) {
                        if mem.store.has(mir, slot.utc, hz0, dt0) {
                            continue;
                        }
                        mem.store
                            .matches(mir, slot.utc, sub.period, hz0, dt0, &mut matched);
                        let mut d = combine(
                            &mem.store,
                            &matched,
                            sets,
                            cand.score,
                            scramble,
                            &mut s,
                            ctx.hashes,
                            &mut work,
                        );
                        if d.is_none() && cand.score >= 2.0 {
                            d = apriori(
                                mem.heard.hypotheses(),
                                sets,
                                scramble,
                                &mut s,
                                ctx.hashes,
                                &mut work,
                            )
                            .filter(|d| {
                                tone_amplitudes(&bb, &lay, sgn, fs, &ft, &mut c);
                                snr_of(&c, &lay, &tones_of(&sub, &d.cw)) >= APRIORI_MIN_SNR_DB
                            });
                        }
                        match d {
                            Some(d) => got = Some((d, ft.clone())),
                            None if sets.len() == SETS => mem.store.put(
                                mir,
                                slot.utc,
                                hz0,
                                dt0,
                                [&sets[0], &sets[1], &sets[2]],
                            ),
                            None => {}
                        }
                    }
                    let Some((d, f)) = got else { continue };
                    let tones = tones_of(&sub, &d.cw);
                    // Measure on the decoded tones.
                    tone_amplitudes(&bb, &lay, sgn, fs, &f, &mut c);
                    if live < lay.nsym {
                        let from = (live * lay.m).min(c.len());
                        c[from..].fill(Complex32::default());
                    }
                    let snr = snr_before(&c, &lay, &tones, live);
                    let hz = shift + f.df;
                    let start_sample = (f.i0 as f64 + f.frac) * lay.decim as f64;
                    let t_tx = slot.time_of(start_sample);
                    let dt = t_tx - sub.t0();
                    // Subtract (always, duplicates too).
                    let r = clean_copy(&sub, &tones, hz, mir, slot.rate);
                    subtract(&mut audio, start_sample.round() as i64, &r, lay.nss);
                    found += 1;
                    let dup = out.iter().any(|o| o.text == d.text)
                        || made.iter().any(|&(fm, tm, mm)| {
                            mm == mir
                                && (fm - hz).abs() < 1.5 * lay.spacing
                                && (tm - t_tx).abs() < 2.0 * sub.symbol_s()
                        });
                    made.push((hz, t_tx, mir));
                    if dup {
                        continue;
                    }
                    let mut rec = Decode::new(slot, d.text, snr, dt, hz, mir, pass_no);
                    rec.deep = d.deep;
                    rec.how = d.how;
                    out.push(rec);
                }
            }
            pass_no += 1;
            if found == 0 {
                break;
            }
        }
    }
    out.sort_by_key(|d| std::cmp::Reverse(d.snr));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rx::testutil::slot_of;

    #[test]
    fn ft8_generator_stations_decode() {
        let st = [
            ("CQ K1ABC FN42", 600.0, -6.0, 0.0, false),
            ("K1ABC W9XYZ -12", 900.0, -10.0, 0.3, false),
            ("W9XYZ K1ABC R-05", 1250.0, -14.0, -0.4, false),
            ("CQ PJ4/K1ABC", 1500.0, -8.0, 0.1, false),
            ("TNX BOB 73 GL", 1800.0, -16.0, 0.6, false),
            ("CQ DL1XYZ JO62", 2000.0, -10.0, 0.2, true),
            ("G4ABC DL1XYZ RR73", 2100.0, -12.0, 0.0, false),
            ("CQ JA1XYZ PM95", 2450.0, -18.0, 1.0, false),
            ("7123456789ABCDEF01", 2800.0, -9.0, -0.2, false),
        ];
        let slot = slot_of(Sub::new(Mode::Ft8), &st, 1);
        let mut h = CallHashes::default();
        let t = std::time::Instant::now();
        let d = decode(
            &slot,
            &mut Ctx {
                hashes: &mut h,
                mirrored: true,
                deep: true,
                memory: None,
            },
        );
        eprintln!("{} ms", t.elapsed().as_millis());
        for x in &d {
            eprintln!(
                "{:>4} {:6.2} {:7.1} {} {} {}",
                x.snr, x.dt, x.hz, x.text, x.mirrored, x.pass
            );
        }
        for s in &st {
            let x = d
                .iter()
                .find(|x| x.text == s.0)
                .unwrap_or_else(|| panic!("{} missing", s.0));
            assert!((x.hz - s.1).abs() < 1.0, "{} hz {}", s.0, x.hz);
            assert!((x.dt - s.3).abs() < 0.06, "{} dt {}", s.0, x.dt);
            assert_eq!(x.mirrored, s.4);
        }
        assert_eq!(d.len(), st.len());
        // The same slot through the slot decoder with its memory, and its early cut at 11.3 s.
        let mut state = crate::runner::DecodeState::default();
        let t = std::time::Instant::now();
        let n = crate::runner::decode_slot(&slot, &mut state).decodes.len();
        eprintln!("{} ms through the slot decoder", t.elapsed().as_millis());
        assert_eq!(n, st.len());
        let mut early = slot.clone();
        early.early = true;
        let keep = early.sample_of(11.3) as usize;
        early.audio = std::sync::Arc::new(early.audio[..keep].to_vec());
        let mut state = crate::runner::DecodeState::default();
        let t = std::time::Instant::now();
        let n = crate::runner::decode_slot(&early, &mut state).decodes.len();
        eprintln!("{} ms early, {n} decodes", t.elapsed().as_millis());
    }

    /// The decode time of the slot of `ft8_generator_stations_decode` after three slots of a conversation, with the failed
    /// candidates stored and the calls heard.
    #[test]
    fn ft8_decode_time_after_a_conversation() {
        let sub = Sub::new(Mode::Ft8);
        let talk = [
            ("CQ W9XYZ EN37", 600.0, -6.0, 0.0, false),
            ("CQ DL1ABC JO62", 1000.0, -6.0, 0.0, false),
            ("W9XYZ K1ABC FN42", 1400.0, -6.0, 0.0, false),
            ("DL1ABC G4XYZ IO91", 1800.0, -6.0, 0.0, false),
            ("K1ABC W9XYZ -10", 2200.0, -6.0, 0.0, false),
        ];
        let mut state = crate::runner::DecodeState::default();
        for k in 0..3u64 {
            let mut slot = slot_of(sub, &talk, 10 + k);
            slot.utc = 15.0 * k as f64;
            assert_eq!(crate::runner::decode_slot(&slot, &mut state).decodes.len(), 5);
        }
        let st = [
            ("CQ K1ABC FN42", 600.0, -6.0, 0.0, false),
            ("K1ABC W9XYZ -12", 900.0, -10.0, 0.3, false),
            ("W9XYZ K1ABC R-05", 1250.0, -14.0, -0.4, false),
            ("CQ PJ4/K1ABC", 1500.0, -8.0, 0.1, false),
            ("TNX BOB 73 GL", 1800.0, -16.0, 0.6, false),
            ("CQ DL1XYZ JO62", 2000.0, -10.0, 0.2, true),
            ("G4ABC DL1XYZ RR73", 2100.0, -12.0, 0.0, false),
            ("CQ JA1XYZ PM95", 2450.0, -18.0, 1.0, false),
            ("7123456789ABCDEF01", 2800.0, -9.0, -0.2, false),
        ];
        let mut slot = slot_of(sub, &st, 1);
        slot.utc = 45.0;
        let t = std::time::Instant::now();
        let d = crate::runner::decode_slot(&slot, &mut state).decodes;
        eprintln!(
            "{} ms with {} stored candidates",
            t.elapsed().as_millis(),
            state.ft.store.len()
        );
        assert_eq!(d.len(), st.len());
    }
}
