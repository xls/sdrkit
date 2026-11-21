//! Parametric stereo of HE-AAC v2 (the baseline decoder with 20 parameter bands): the PS bitstream in the SBR extended data,
//! the hybrid analysis of the low QMF bands, the decorrelator, the transient reduction and the stereo mixing.
//!
//! The filter prototypes, the IID and ICC values and the decorrelator coefficients are those of ISO/IEC 14496-3 as printed
//! in the PacketVideo AAC decoder (Copyright 1998-2009 PacketVideo, Apache License 2.0; files `ps_channel_filtering.cpp`,
//! `ps_init_stereo_mixing.cpp` and `ps_all_pass_filter_coeff.cpp`).
//!
//! Standard: ISO/IEC 14496-3 subpart 8, clause 8.6.4 (parametric stereo), and annex 8.A (baseline decoder).

#![allow(clippy::excessive_precision)]

use crate::huffman::Book;
use crate::sbr::tables::{self, Tree};
use sdrkit_algo_dsp::Complex32 as C;
use sdrkit_util_bits::BitReader;
use std::f64::consts::PI;
use std::sync::OnceLock;

/// The prototype of the 8-band hybrid filter (taps 0 to 6; the filter is symmetric about tap 6).
const G8: [f64; 7] = [
    0.00746082949812,
    0.02270420949825,
    0.04546865930473,
    0.07266113929591,
    0.09885108575264,
    0.11793710567217,
    0.125,
];
/// The prototype of the 2-band hybrid filter (taps 0 to 6; symmetric about tap 6).
const G2: [f64; 7] = [
    0.0,
    0.01899487526049,
    0.0,
    -0.07293139167538,
    0.0,
    0.30596630545168,
    0.5,
];
/// `c1` of the mixing by IID index -7 to 7, `sqrt(2 / (1 + 10^(iid / 10)))`.
const C1: [f32; 15] = [
    1.411983, 1.403138, 1.386877, 1.348400, 1.291249, 1.196037, 1.107372, 1.0, 0.879617, 0.754649,
    0.576780, 0.426401, 0.276718, 0.176645, 0.079402,
];
/// `c1` of the mixing by fine IID index -15 to 15.
const C1_FINE: [f32; 31] = [
    1.414207, 1.414191, 1.414143, 1.413990, 1.413507, 1.411983, 1.409773, 1.405395, 1.396780,
    1.380053, 1.348400, 1.313920, 1.264310, 1.196037, 1.107372, 1.0, 0.879617, 0.754649, 0.633656,
    0.523081, 0.426401, 0.308955, 0.221375, 0.157688, 0.111982, 0.079402, 0.044699, 0.025145,
    0.014141, 0.007953, 0.004472,
];
/// The inter-channel coherence by ICC index 0 to 7.
const ICC: [f32; 8] = [1.0, 0.937, 0.84118, 0.60092, 0.36764, 0.0, -0.589, -1.0];
/// The coefficients and the delays (in QMF slots) of the three all-pass links of the decorrelator.
const LINK_A: [f32; 3] = [0.65143905753106, 0.56471812200776, 0.97908331911390];
const LINK_Q: [f64; 3] = [0.43, 0.75, 0.347];
const LINK_D: [usize; 3] = [3, 4, 5];
/// The fractional delay of the decorrelator, in units of the band spacing.
const Q_FRACT: f64 = 0.39;
/// Peak decay, smoothing and impact of the transient reduction.
const PEAK_DECAY: f32 = 0.765_928_34;
const SMOOTH: f32 = 0.25;
const IMPACT: f32 = 1.5;

/// Hybrid bands from QMF bands 0, 1 and 2.
const N_HYBRID: usize = 10;
/// Bands of the stereo processing: the hybrid bands, then QMF bands 3 to 63.
const N_BANDS: usize = N_HYBRID + 61;
/// Bands with an all-pass decorrelator: the hybrid bands and QMF bands 3 to 22.
const N_ALLPASS: usize = N_HYBRID + 20;
/// The parameter band of each hybrid band.
const HYBRID_BIN: [usize; N_HYBRID] = [0, 1, 2, 3, 1, 0, 4, 5, 6, 7];
/// The centre frequency of each hybrid band in QMF band units, signed as the frequency of the band in the subband signal
/// of [`Ps::analyse`].
const HYBRID_CENTRE: [f64; N_HYBRID] = [
    -0.125, -0.375, -0.625, -0.875, 0.375, 0.125, -1.25, -1.75, -2.25, -2.75,
];
/// The first QMF band of parameter bands 8 to 19, and the end.
const QMF_GROUPS: [usize; 13] = [3, 4, 5, 6, 7, 8, 9, 11, 14, 18, 23, 35, 64];
/// Slots of delay of the hybrid filters, by which QMF bands 3 and up are delayed too.
const HYBRID_DELAY: usize = 6;
/// The longest delay of a band without an all-pass decorrelator (QMF bands 23 to 34); bands 35 and up use 1 slot.
const LONG_DELAY: usize = 14;

struct Books {
    iid: [Book; 2],
    iid_fine: [Book; 2],
    icc: [Book; 2],
}

fn book(t: &Tree) -> Book {
    Book::new(t.code.len(), |i| (t.code[i], t.len[i]))
}

fn books() -> &'static Books {
    static B: OnceLock<Books> = OnceLock::new();
    B.get_or_init(|| Books {
        iid: [book(&tables::IID_T), book(&tables::IID_F)],
        iid_fine: [book(&tables::IID_FINE_T), book(&tables::IID_FINE_F)],
        icc: [book(&tables::ICC_T), book(&tables::ICC_F)],
    })
}

/// The parameter band of each band of the stereo processing.
fn bin(k: usize) -> usize {
    if k < N_HYBRID {
        HYBRID_BIN[k]
    } else {
        let q = k - N_HYBRID + 3;
        8 + QMF_GROUPS.windows(2).position(|w| q < w[1]).unwrap_or(11)
    }
}

/// The all-pass decorrelator of one band.
#[derive(Clone, Debug)]
struct AllPass {
    phase: C,
    a: [f32; 3],
    q: [C; 3],
    pre: [C; 2],
    links: [[C; 5]; 3],
    at: [usize; 3],
}

impl AllPass {
    fn new(centre: f64, decay: f32) -> AllPass {
        let rot = |q: f64| {
            let a = -PI * q * centre;
            C::new(a.cos() as f32, a.sin() as f32)
        };
        AllPass {
            phase: rot(Q_FRACT),
            a: LINK_A.map(|a| a * decay),
            q: LINK_Q.map(rot),
            pre: [C::default(); 2],
            links: [[C::default(); 5]; 3],
            at: [0; 3],
        }
    }

    /// Filters one sample: `z^-2` with a fractional delay, then three all-pass links (clause 8.6.4.5.2).
    fn run(&mut self, x: C) -> C {
        let mut v = self.pre[1] * self.phase;
        self.pre[1] = self.pre[0];
        self.pre[0] = x;
        for (m, &d) in LINK_D.iter().enumerate() {
            let at = &mut self.at[m];
            let old = self.links[m][*at];
            let s = v + old * self.q[m] * self.a[m];
            v = old * self.q[m] - s * self.a[m];
            self.links[m][*at] = s;
            *at = (*at + 1) % d;
        }
        v
    }
}

/// The parameters of one frame.
#[derive(Clone, Debug, Default)]
struct Frame {
    n_env: usize,
    border: [usize; 6],
    iid: [[i32; 20]; 5],
    icc: [[i32; 20]; 5],
}

/// The parametric stereo decoder of one mono SBR element.
pub(crate) struct Ps {
    nslots: usize,
    header: bool,
    iid_mode: Option<u8>,
    icc_mode: Option<u8>,
    ext: bool,
    frame: Frame,
    fresh: bool,
    iid_prev: [i32; 20],
    icc_prev: [i32; 20],
    h_prev: [[f32; 4]; 20],
    hist: [[C; 13]; 3],
    delay: Vec<[C; 61]>,
    allpass: Vec<AllPass>,
    long: Vec<[C; LONG_DELAY]>,
    short: [C; 29],
    long_at: usize,
    peak: [f32; 20],
    smooth: [f32; 20],
    diff: [f32; 20],
    kernel8: [[C; 13]; 8],
}

impl Ps {
    /// A decoder for frames of `nslots` QMF slots.
    pub(crate) fn new(nslots: usize) -> Ps {
        let allpass = (0..N_ALLPASS)
            .map(|k| {
                if k < N_HYBRID {
                    AllPass::new(HYBRID_CENTRE[k], 1.0)
                } else {
                    let q = k - N_HYBRID + 3;
                    let decay = (1.0 - 0.05 * (q as f32 - 3.0)).clamp(0.0, 1.0);
                    AllPass::new(q as f64 + 0.5, decay)
                }
            })
            .collect();
        let mut kernel8 = [[C::default(); 13]; 8];
        for (q, row) in kernel8.iter_mut().enumerate() {
            for (i, v) in row.iter_mut().enumerate() {
                let g = G8[i.min(12 - i)];
                let a = 2.0 * PI / 8.0 * (q as f64 + 0.5) * (i as f64 - 6.0);
                *v = C::new((g * a.cos()) as f32, (g * a.sin()) as f32);
            }
        }
        Ps {
            nslots,
            header: false,
            iid_mode: None,
            icc_mode: None,
            ext: false,
            frame: Frame::default(),
            fresh: false,
            iid_prev: [0; 20],
            icc_prev: [0; 20],
            h_prev: [[1.0, 1.0, 0.0, 0.0]; 20],
            hist: [[C::default(); 13]; 3],
            delay: vec![[C::default(); 61]; HYBRID_DELAY],
            allpass,
            long: vec![[C::default(); LONG_DELAY]; 12],
            short: [C::default(); 29],
            long_at: 0,
            peak: [0.0; 20],
            smooth: [0.0; 20],
            diff: [0.0; 20],
            kernel8,
        }
    }

    /// Reads ps_data from at most `bits` bits of `r` and returns the bits it read.
    pub(crate) fn read(&mut self, r: &mut BitReader<'_>, bits: usize) -> usize {
        let start = r.pos();
        let ok = self.read_data(r).is_some() && r.pos() - start <= bits && !r.overrun();
        self.fresh = ok;
        if !ok {
            self.frame.n_env = 0;
        }
        (r.pos() - start).min(bits)
    }

    fn read_data(&mut self, r: &mut BitReader<'_>) -> Option<()> {
        if r.bit() {
            self.header = true;
            self.iid_mode = r.bit().then(|| r.u(3) as u8);
            self.icc_mode = r.bit().then(|| r.u(3) as u8);
            self.ext = r.bit();
        }
        if !self.header {
            return None;
        }
        let class = r.bit();
        let idx = r.u(2) as usize;
        let n_env = if class {
            [1, 2, 3, 4][idx]
        } else {
            [0, 1, 2, 4][idx]
        };
        let f = &mut self.frame;
        f.n_env = n_env;
        f.border[0] = 0;
        if class {
            for e in 1..=n_env {
                f.border[e] = r.u(5) as usize + 1;
            }
        } else {
            for e in 1..=n_env {
                f.border[e] = e * self.nslots / n_env;
            }
        }
        let b = books();
        // A mode of 34 bands is read and its values are not used.
        let width = |mode: u8| [10, 20, 34][usize::from(mode % 3)];
        let mut iid34 = false;
        if let Some(mode) = self.iid_mode {
            let n = width(mode);
            let fine = mode >= 3;
            let (book, tree, lim) = if fine {
                (&b.iid_fine, [&tables::IID_FINE_T, &tables::IID_FINE_F], 15)
            } else {
                (&b.iid, [&tables::IID_T, &tables::IID_F], 7)
            };
            let mut prev = self.iid_prev;
            for e in 0..n_env {
                let dt = r.bit();
                let mut v = [0i32; 34];
                read_par(
                    r,
                    &book[usize::from(!dt)],
                    tree[usize::from(!dt)].min,
                    n,
                    dt,
                    &prev,
                    &mut v,
                )?;
                iid34 |= n == 34;
                for (k, p) in f.iid[e].iter_mut().enumerate() {
                    *p = if n == 10 { v[k / 2] } else { v[k] }.clamp(-lim, lim);
                }
                prev = f.iid[e];
            }
        } else {
            f.iid = [[0; 20]; 5];
        }
        if let Some(mode) = self.icc_mode {
            let n = width(mode);
            let mut prev = self.icc_prev;
            for e in 0..n_env {
                let dt = r.bit();
                let mut v = [0i32; 34];
                let min = [&tables::ICC_T, &tables::ICC_F][usize::from(!dt)].min;
                read_par(r, &b.icc[usize::from(!dt)], min, n, dt, &prev, &mut v)?;
                iid34 |= n == 34;
                for (k, p) in f.icc[e].iter_mut().enumerate() {
                    *p = if n == 10 { v[k / 2] } else { v[k] }.clamp(0, 7);
                }
                prev = f.icc[e];
            }
        } else {
            f.icc = [[0; 20]; 5];
        }
        if self.ext {
            let mut count = r.u(4) as usize;
            if count == 15 {
                count += r.u(8) as usize;
            }
            r.skip(8 * count);
        }
        if iid34 {
            f.n_env = 0;
            return None;
        }
        if n_env > 0 {
            self.iid_prev = f.iid[n_env - 1];
            self.icc_prev = f.icc[n_env - 1];
        }
        // Borders rise, and the last envelope reaches the end of the frame.
        for e in 1..=n_env {
            let lo = f.border[e - 1] + 1;
            let hi = self.nslots - (n_env - e);
            f.border[e] = f.border[e].clamp(lo, hi.max(lo));
        }
        if n_env > 0 && f.border[n_env] < self.nslots && n_env < 5 {
            f.iid[n_env] = f.iid[n_env - 1];
            f.icc[n_env] = f.icc[n_env - 1];
            f.n_env += 1;
            f.border[n_env + 1] = self.nslots;
        }
        Some(())
    }

    /// Turns the mono QMF slots of `l` into the left (in `l`) and right (in `r`) slots of the frame.
    pub(crate) fn process(&mut self, l: &mut [[C; 64]], r: &mut [[C; 64]]) {
        // Without new parameters, one envelope holds the last ones.
        if !std::mem::take(&mut self.fresh) || self.frame.n_env == 0 {
            self.frame.n_env = 1;
            self.frame.border[1] = self.nslots;
            self.frame.iid[0] = self.iid_prev;
            self.frame.icc[0] = self.icc_prev;
        }
        let fine = self.iid_mode.is_some_and(|m| m >= 3);
        let mut e = 0;
        let mut h_from = self.h_prev;
        let mut h_to = mixing(&self.frame.iid[0], &self.frame.icc[0], fine);
        for n in 0..self.nslots {
            while n >= self.frame.border[e + 1] {
                e += 1;
                h_from = h_to;
                h_to = mixing(&self.frame.iid[e], &self.frame.icc[e], fine);
            }
            let (b0, b1) = (self.frame.border[e], self.frame.border[e + 1]);
            let t = (n + 1 - b0) as f32 / (b1 - b0) as f32;
            let mut s = [C::default(); N_BANDS];
            self.analyse(&l[n], &mut s);
            let mut d = [C::default(); N_BANDS];
            self.decorrelate(&s, &mut d);
            let mut lo = [C::default(); N_BANDS];
            let mut ro = [C::default(); N_BANDS];
            for k in 0..N_BANDS {
                let b = bin(k);
                let h: [f32; 4] =
                    std::array::from_fn(|i| h_from[b][i] + (h_to[b][i] - h_from[b][i]) * t);
                lo[k] = s[k] * h[0] + d[k] * h[2];
                ro[k] = s[k] * h[1] + d[k] * h[3];
            }
            synthesize(&lo, &mut l[n]);
            synthesize(&ro, &mut r[n]);
        }
        self.h_prev = h_to;
    }

    /// The hybrid analysis of one slot (clause 8.6.4.3): QMF bands 0 to 2 into 10 hybrid bands, QMF bands 3 and up delayed
    /// to match.
    fn analyse(&mut self, x: &[C; 64], s: &mut [C; N_BANDS]) {
        for (h, &v) in self.hist.iter_mut().zip(x) {
            h.copy_within(1..13, 0);
            h[12] = v;
        }
        // Band 0: eight complex bands, of which 2 and 5, and 3 and 4, are merged.
        let mut q = [C::default(); 8];
        for (k, out) in q.iter_mut().enumerate() {
            for i in 0..13 {
                *out += self.kernel8[k][i] * self.hist[0][12 - i];
            }
        }
        s[..6].copy_from_slice(&[q[0], q[1], q[2] + q[5], q[3] + q[4], q[6], q[7]]);
        // Bands 1 and 2: a low and a high half. In an odd band the high half holds the lower frequencies.
        for (b, at) in [(1, 6), (2, 8)] {
            let mut low = C::default();
            let mut high = C::default();
            for i in 0..13 {
                let v = self.hist[b][12 - i] * G2[i.min(12 - i)] as f32;
                low += v;
                high += if (i as i32 - 6) % 2 == 0 { v } else { -v };
            }
            let (first, second) = if b % 2 == 1 { (high, low) } else { (low, high) };
            s[at] = first;
            s[at + 1] = second;
        }
        let oldest = self.delay[0];
        self.delay.copy_within(1.., 0);
        self.delay[HYBRID_DELAY - 1].copy_from_slice(&x[3..]);
        s[N_HYBRID..].copy_from_slice(&oldest);
    }

    /// The decorrelated signal of one slot with the transient reduction (clauses 8.6.4.5.2 and 8.6.4.5.3).
    fn decorrelate(&mut self, s: &[C; N_BANDS], d: &mut [C; N_BANDS]) {
        for k in 0..N_ALLPASS {
            d[k] = self.allpass[k].run(s[k]);
        }
        let at = self.long_at;
        for (j, k) in (N_ALLPASS..N_BANDS).enumerate() {
            let q = k - N_HYBRID + 3;
            if q < 35 {
                d[k] = self.long[j][at];
                self.long[j][at] = s[k];
            } else {
                let i = q - 35;
                d[k] = self.short[i];
                self.short[i] = s[k];
            }
        }
        self.long_at = (at + 1) % LONG_DELAY;
        let mut power = [0f32; 20];
        for k in 0..N_BANDS {
            power[bin(k)] += s[k].norm_sqr();
        }
        let mut gain = [1f32; 20];
        for i in 0..20 {
            self.peak[i] = (self.peak[i] * PEAK_DECAY).max(power[i]);
            self.smooth[i] = SMOOTH * power[i] + (1.0 - SMOOTH) * self.smooth[i];
            self.diff[i] = SMOOTH * (self.peak[i] - power[i]) + (1.0 - SMOOTH) * self.diff[i];
            if IMPACT * self.diff[i] > self.smooth[i] {
                gain[i] = self.smooth[i] / (IMPACT * self.diff[i]);
            }
        }
        for k in 0..N_BANDS {
            d[k] *= gain[bin(k)];
        }
    }
}

/// Reads the values of one envelope, `n` of them, coded in time (`dt`, against `prev`) or in frequency.
fn read_par(
    r: &mut BitReader<'_>,
    book: &Book,
    min: i32,
    n: usize,
    dt: bool,
    prev: &[i32; 20],
    out: &mut [i32; 34],
) -> Option<()> {
    for k in 0..n {
        let delta = book.read(r)? as i32 + min;
        out[k] = if dt {
            // The last envelope has 20 values; a set of 10 takes every second one.
            let p = if n == 10 {
                prev[2 * k]
            } else {
                prev[k.min(19)]
            };
            p + delta
        } else if k == 0 {
            delta
        } else {
            out[k - 1] + delta
        };
    }
    Some(())
}

/// The mixing matrix (h11, h12, h21, h22) of each parameter band (clause 8.6.4.6.2, mixing procedure Ra).
fn mixing(iid: &[i32; 20], icc: &[i32; 20], fine: bool) -> [[f32; 4]; 20] {
    std::array::from_fn(|b| {
        // Values held from a frame of the other quantization are clamped to this one.
        let (c1, c2) = if fine {
            let i = iid[b].clamp(-15, 15);
            (C1_FINE[(15 + i) as usize], C1_FINE[(15 - i) as usize])
        } else {
            let i = iid[b].clamp(-7, 7);
            (C1[(7 + i) as usize], C1[(7 - i) as usize])
        };
        let alpha = 0.5 * ICC[icc[b].clamp(0, 7) as usize].acos();
        let beta = alpha * (c1 - c2) / std::f32::consts::SQRT_2;
        [
            c2 * (beta + alpha).cos(),
            c1 * (beta - alpha).cos(),
            c2 * (beta + alpha).sin(),
            c1 * (beta - alpha).sin(),
        ]
    })
}

/// The hybrid synthesis of one slot: the hybrid bands of each low QMF band summed.
fn synthesize(s: &[C; N_BANDS], out: &mut [C; 64]) {
    out[0] = s[..6].iter().copied().sum();
    out[1] = s[6] + s[7];
    out[2] = s[8] + s[9];
    out[3..].copy_from_slice(&s[N_HYBRID..]);
}
