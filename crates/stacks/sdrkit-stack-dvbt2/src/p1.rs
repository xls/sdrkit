//! The P1 symbol: its generation, and a streaming detector that finds it, measures the carrier offset and reads S1 and S2.
//! The detector also distinguishes a DVB-T2 signal from DVB-T, which has no P1.
//!
//! Standard: ETSI EN 302 755 V1.4.1 clauses 7.2.1 and 9.8, and ETSI TS 102 831 V1.2.1 clause 10.2.2.5 (P1 detection).

use crate::params::P1Info;
use crate::tables;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32;
use sdrkit_util_bits::lfsr::Lfsr;

/// The samples of P1 (2 048 T).
pub const P1_LEN: usize = 2048;
/// The length of part C (542 T).
pub const P1_C: usize = 542;
/// The length of part B (482 T).
pub const P1_B: usize = 482;
/// The active carriers of P1.
pub const P1_CARRIERS: usize = 384;

/// The P1 scrambling sequence: the BB scrambling PRBS 1 + x^14 + x^15 started from 100111001000110 (clause 9.8.2.2).
pub const P1_PRBS: Lfsr = Lfsr::fibonacci(0xC001, 0x3139);

/// MSS_SEQ: CSS_S1, CSS_S2 and CSS_S1 again, 384 bits (clause 9.8.2.2 step 1).
pub fn mss_seq(s1: u8, s2: u8) -> [u8; P1_CARRIERS] {
    let mut seq = [0u8; P1_CARRIERS];
    let a = tables::P1_S1[(s1 & 7) as usize];
    let b = tables::P1_S2[(s2 & 15) as usize];
    for i in 0..64 {
        let bit = ((a >> (63 - i)) & 1) as u8;
        seq[i] = bit;
        seq[320 + i] = bit;
    }
    for i in 0..256 {
        seq[64 + i] = ((b[i / 64] >> (63 - i % 64)) & 1) as u8;
    }
    seq
}

/// The P1 scrambling signs, +1 for a 0 of the PRBS and -1 for a 1.
pub fn p1_scrambling() -> [f32; P1_CARRIERS] {
    let mut r = P1_PRBS;
    let mut s = [0f32; P1_CARRIERS];
    for v in s.iter_mut() {
        *v = 1.0 - 2.0 * r.bit() as f32;
    }
    s
}

/// MSS_SCR, the modulation of the 384 active carriers: DBPSK from a +1 start, then scrambled (clause 9.8.2.2 steps 2 and 3).
pub fn carrier_values(info: P1Info) -> [f32; P1_CARRIERS] {
    let seq = mss_seq(info.s1, info.s2);
    let scr = p1_scrambling();
    let mut out = [0f32; P1_CARRIERS];
    let mut d = 1.0f32;
    for i in 0..P1_CARRIERS {
        if seq[i] == 1 {
            d = -d;
        }
        out[i] = d * scr[i];
    }
    out
}

/// The 2 048 samples of P1 at the elementary period T, with mean power 1 (clauses 9.8.2.3 and 9.8.2.4): part A is the 1K
/// IFFT of the carriers scaled by 1/sqrt(384), parts C and B are its first 542 and last 482 samples shifted up by 1/(1024 T).
pub fn generate(info: P1Info) -> Vec<Complex32> {
    let vals = carrier_values(info);
    let mut a = vec![Complex32::default(); 1024];
    for (i, &k) in tables::P1_CARRIERS.iter().enumerate() {
        a[(k as i32 - 426).rem_euclid(1024) as usize] =
            Complex32::new(vals[i] / (P1_CARRIERS as f32).sqrt(), 0.0);
    }
    Fft::inverse(1024).process(&mut a);
    let shift = |n: usize| {
        let ph = 2.0 * std::f64::consts::PI * (n % 1024) as f64 / 1024.0;
        Complex32::new(ph.cos() as f32, ph.sin() as f32)
    };
    let mut out = Vec::with_capacity(P1_LEN);
    for n in 0..P1_LEN {
        out.push(if n < P1_C {
            a[n] * shift(n)
        } else if n < P1_C + 1024 {
            a[n - P1_C]
        } else {
            a[n - 1024] * shift(n)
        });
    }
    out
}

/// One P1 found by [`P1Detector`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct P1Found {
    /// The absolute sample index of the first sample of P1.
    pub start: u64,
    /// The normalised correlation at the peak (about 1 for a clean P1).
    pub metric: f32,
    /// The carrier offset in cycles per sample, integer part from the carrier pattern and fraction from the C-A-B phases.
    pub freq: f64,
    /// The S1 and S2 fields, when the carriers decode.
    pub info: Option<P1Info>,
}

/// A streaming P1 detector (TS 102 831 clause 10.2.2.5): two sliding correlations, part C with part A and part B with part A,
/// each with the 1/(1024 T) shift removed. Their peaks coincide at the end of P1. At a peak it FFTs part A, finds the integer
/// carrier offset from the carrier pattern, and reads S1 and S2 by differential correlation with the patterns of table 69.
/// Memory is fixed at [`P1Detector::new`].
pub struct P1Detector {
    hist: Vec<Complex32>,
    shift: Vec<Complex32>,
    pos: u64,
    s1: Complex32,
    s2: Complex32,
    e1: f32,
    e2: f32,
    m1: Vec<f32>,
    threshold: f32,
    max_shift: i32,
    best: Option<(u64, f32)>,
    since_best: usize,
    fft: Fft,
    buf: Vec<Complex32>,
    scr: [f32; P1_CARRIERS],
    patterns1: Vec<[f32; 64]>,
    patterns2: Vec<[f32; 256]>,
    resum: usize,
}

/// The history kept: P1 itself plus the delays of the two correlations.
const HIST: usize = 4096;

impl P1Detector {
    /// A detector with peak threshold `threshold` (normalised, 0 to 1, where 0.4 suits signals down to about 0 dB SNR) that
    /// searches integer carrier offsets up to `max_shift` 1K carriers each way (8.9 kHz each at 8 MHz).
    pub fn new(threshold: f32, max_shift: i32) -> P1Detector {
        let shift = (0..1024)
            .map(|n| {
                let ph = -2.0 * std::f64::consts::PI * n as f64 / 1024.0;
                Complex32::new(ph.cos() as f32, ph.sin() as f32)
            })
            .collect();
        let pm = |v: u64, n: usize| -> f32 { 1.0 - 2.0 * ((v >> (63 - n)) & 1) as f32 };
        let patterns1 = tables::P1_S1
            .iter()
            .map(|&v| {
                let mut p = [0f32; 64];
                for (i, x) in p.iter_mut().enumerate() {
                    *x = pm(v, i);
                }
                p
            })
            .collect();
        let patterns2 = tables::P1_S2
            .iter()
            .map(|w| {
                let mut p = [0f32; 256];
                for (i, x) in p.iter_mut().enumerate() {
                    *x = pm(w[i / 64], i % 64);
                }
                p
            })
            .collect();
        P1Detector {
            hist: vec![Complex32::default(); HIST],
            shift,
            pos: 0,
            s1: Complex32::default(),
            s2: Complex32::default(),
            e1: 0.0,
            e2: 0.0,
            m1: vec![0.0; 1024],
            threshold,
            max_shift,
            best: None,
            since_best: 0,
            fft: Fft::forward(1024),
            buf: vec![Complex32::default(); 1024],
            scr: p1_scrambling(),
            patterns1,
            patterns2,
            resum: 0,
        }
    }

    /// Forgets everything seen.
    pub fn reset(&mut self) {
        self.hist.fill(Complex32::default());
        self.s1 = Complex32::default();
        self.s2 = Complex32::default();
        self.e1 = 0.0;
        self.e2 = 0.0;
        self.m1.fill(0.0);
        self.best = None;
        self.since_best = 0;
    }

    /// The samples seen so far.
    pub fn position(&self) -> u64 {
        self.pos
    }

    /// Sets the absolute index of the next sample (after a gap in the input).
    pub fn set_position(&mut self, pos: u64) {
        self.reset();
        self.pos = pos;
    }

    fn h(&self, back: usize) -> Complex32 {
        self.hist[((self.pos as usize).wrapping_sub(back)) % HIST]
    }

    /// Feeds samples. Each P1 found goes to `found`, about 600 samples after its end.
    pub fn process(&mut self, x: &[Complex32], mut found: impl FnMut(P1Found)) {
        for &s in x {
            let s = if s.re.is_finite() && s.im.is_finite() {
                s
            } else {
                Complex32::default()
            };
            let n = self.pos as usize;
            self.hist[n % HIST] = s;
            // y(n) = x(n) e^(-j 2 pi n / 1024).
            let y = |d: &Self, back: usize| -> Complex32 {
                let idx = n.wrapping_sub(back);
                d.h(back) * d.shift[idx % 1024]
            };
            // Correlation 1 (C with A): y(n - 542) conj(x(n)), window 542.
            let add1 = y(self, P1_C) * s.conj();
            let old1 = y(self, 2 * P1_C) * self.h(P1_C).conj();
            self.s1 += add1 - old1;
            self.e1 += s.norm_sqr() - self.h(P1_C).norm_sqr();
            // Correlation 2 (B with A): conj(x(n - 482)) y(n), window 482.
            let add2 = self.h(P1_B).conj() * y(self, 0);
            let old2 = self.h(2 * P1_B).conj() * y(self, P1_B);
            self.s2 += add2 - old2;
            self.e2 += s.norm_sqr() - self.h(P1_B).norm_sqr();
            self.resum += 1;
            if self.resum >= 8192 {
                self.resum = 0;
                self.resum_now();
            }
            // Correlation 1 peaks 964 samples before correlation 2.
            let m1 = self.s1.norm() / self.e1.max(1e-12);
            self.m1[n % 1024] = m1;
            let m1d = self.m1[n.wrapping_sub(964) % 1024];
            let m2 = self.s2.norm() / self.e2.max(1e-12);
            let metric = 0.5 * (m1d + m2);
            self.pos += 1;
            if metric > self.threshold
                && self.pos > P1_LEN as u64
                && self.best.is_none_or(|(_, m)| metric > m)
            {
                self.best = Some((self.pos - 1, metric));
                self.since_best = 0;
            }
            if let Some((at, m)) = self.best {
                self.since_best += 1;
                if self.since_best > 600 {
                    self.best = None;
                    let start = at + 1 - P1_LEN as u64;
                    let p = self.decode(start, m);
                    found(p);
                }
            }
        }
    }

    fn resum_now(&mut self) {
        let n = self.pos as usize;
        let mut s1 = Complex32::default();
        let mut s2 = Complex32::default();
        let (mut e1, mut e2) = (0f32, 0f32);
        for b in 0..P1_C {
            let idx = n.wrapping_sub(b);
            let x = self.h(b);
            let yy = self.h(b + P1_C) * self.shift[idx.wrapping_sub(P1_C) % 1024];
            s1 += yy * x.conj();
            e1 += x.norm_sqr();
        }
        for b in 0..P1_B {
            let idx = n.wrapping_sub(b);
            let yy = self.h(b) * self.shift[idx % 1024];
            s2 += self.h(b + P1_B).conj() * yy;
            e2 += self.h(b).norm_sqr();
        }
        // The sums cover the samples up to the last one written (pos - 1 + 1 = pos), so shift by one.
        self.s1 = s1;
        self.s2 = s2;
        self.e1 = e1;
        self.e2 = e2;
        let _ = n;
    }

    /// Reads P1 that starts at absolute sample `start` (still in the history).
    fn decode(&mut self, start: u64, metric: f32) -> P1Found {
        // The fraction of a 1K carrier from the two correlation phases: angle2 - angle1 = 2 pi 1024 eps.
        let end = start + P1_LEN as u64 - 1;
        let back = (self.pos - 1 - end) as usize;
        let (c1, c2) = self.correlations_at(back);
        let frac = (c2 * c1.conj()).arg() as f64 / (2.0 * std::f64::consts::PI * 1024.0);
        // Part A starts 542 samples into P1.
        let a0 = self.pos - (start + P1_C as u64);
        for i in 0..1024 {
            let abs = start + P1_C as u64 + i as u64;
            let ph = -2.0 * std::f64::consts::PI * frac * abs as f64;
            let rot = Complex32::new(ph.cos() as f32, ph.sin() as f32);
            self.buf[i] = self.h(a0 as usize - i) * rot;
        }
        self.fft.process(&mut self.buf);
        // Integer offset: the shift that puts most energy on the 384 carriers.
        let mut best = (0i32, -1f32);
        for sh in -self.max_shift..=self.max_shift {
            let e: f32 = tables::P1_CARRIERS
                .iter()
                .map(|&k| self.buf[(k as i32 - 426 + sh).rem_euclid(1024) as usize].norm_sqr())
                .sum();
            if e > best.1 {
                best = (sh, e);
            }
        }
        let sh = best.0;
        let freq = (sh as f64 + frac * 1024.0) / 1024.0;
        // Differential demodulation after descrambling: d_i > 0 means MSS_SEQ_i = 0.
        let mut d = [0f32; P1_CARRIERS];
        let mut prev = Complex32::default();
        for (i, &k) in tables::P1_CARRIERS.iter().enumerate() {
            let v = self.buf[(k as i32 - 426 + sh).rem_euclid(1024) as usize] * self.scr[i];
            if i > 0 {
                d[i] = (v * prev.conj()).re;
            }
            prev = v;
        }
        let pick1 = |pats: &[[f32; 64]]| -> (usize, f32) {
            let mut b = (0, f32::MIN);
            for (j, p) in pats.iter().enumerate() {
                let c: f32 = (1..64).map(|i| p[i] * d[i]).sum::<f32>()
                    + (0..64).map(|i| p[i] * d[320 + i]).sum::<f32>();
                if c > b.1 {
                    b = (j, c);
                }
            }
            b
        };
        let (s1, c1v) = pick1(&self.patterns1);
        let mut b2 = (0usize, f32::MIN);
        for (j, p) in self.patterns2.iter().enumerate() {
            let c: f32 = (0..256).map(|i| p[i] * d[64 + i]).sum();
            if c > b2.1 {
                b2 = (j, c);
            }
        }
        // A pattern counts when it explains at least half of the differential energy of its carriers.
        let e1: f32 = d[1..64].iter().chain(&d[320..]).map(|v| v.abs()).sum();
        let e2: f32 = d[64..320].iter().map(|v| v.abs()).sum();
        let info = (c1v > 0.5 * e1 && b2.1 > 0.5 * e2 && e1 > 0.0).then_some(P1Info {
            s1: s1 as u8,
            s2: b2.0 as u8,
        });
        P1Found {
            start,
            metric,
            freq,
            info,
        }
    }

    /// The two correlations with their windows ending `back` samples before the newest sample, recomputed from the history.
    fn correlations_at(&self, back: usize) -> (Complex32, Complex32) {
        let n = (self.pos as usize).wrapping_sub(1 + back);
        let at = |m: usize| self.hist[m % HIST];
        let sh = |m: usize| self.shift[m % 1024];
        let mut c1 = Complex32::default();
        let mut c2 = Complex32::default();
        let end1 = n.wrapping_sub(964);
        for j in 0..P1_C {
            let m = end1.wrapping_sub(j);
            c1 += at(m.wrapping_sub(P1_C)) * sh(m.wrapping_sub(P1_C)) * at(m).conj();
        }
        for j in 0..P1_B {
            let m = n.wrapping_sub(j);
            c2 += at(m.wrapping_sub(P1_B)).conj() * at(m) * sh(m);
        }
        (c1, c2)
    }
}
