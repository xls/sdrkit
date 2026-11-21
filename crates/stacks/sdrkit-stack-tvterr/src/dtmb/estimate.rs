//! The least-squares channel fit of a DTMB frame header: one core period against the known chips for PN420 and PN945 (a closed
//! form, the normal matrix of an m-sequence is `(P + 1) I - J`), the fixed chips with a small ridge for PN595. `dtmb.track`
//! uses it for PN595 and `dtmb.body` for every header.
//!
//! Standard: GB 20600-2006 clause 4.4.

use super::pn::{Header, HeaderSeq};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32;

/// A header's channel fit.
#[derive(Clone, Debug, Default)]
pub(crate) struct Fit {
    /// Cleaned taps at lags `-pre .. post`, index `lag + pre`, in body units.
    pub taps: Vec<Complex32>,
    pub pre: usize,
    pub post: usize,
    /// Noise per received sample.
    pub noise: f32,
    /// Variance of one tap.
    pub var: f32,
    /// The strongest tap stands 25 times above the variance.
    pub usable: bool,
}

impl Fit {
    /// The strongest tap's lag and its fractional place (a parabola through the amplitudes of it and its neighbours, within
    /// half a symbol).
    pub fn strongest(&self) -> (isize, f32) {
        let (mut j, mut best) = (0usize, -1f32);
        for (i, h) in self.taps.iter().enumerate() {
            if h.norm_sqr() > best {
                best = h.norm_sqr();
                j = i;
            }
        }
        let a = |i: isize| {
            if i < 0 || i as usize >= self.taps.len() {
                0.0
            } else {
                self.taps[i as usize].norm()
            }
        };
        let (l, c, r) = (a(j as isize - 1), a(j as isize), a(j as isize + 1));
        let den = l - 2.0 * c + r;
        let frac = if den < 0.0 {
            (0.5 * (l - r) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        (j as isize - self.pre as isize, frac)
    }
    /// Copies `other` into this fit, keeping this fit's buffer.
    pub fn copy_from(&mut self, other: &Fit) {
        self.taps.clear();
        self.taps.extend_from_slice(&other.taps);
        (self.pre, self.post, self.noise, self.var, self.usable) =
            (other.pre, other.post, other.noise, other.var, other.usable);
    }
    /// The energy of the cleaned taps.
    pub fn energy(&self) -> f32 {
        self.taps.iter().map(|h| h.norm_sqr()).sum()
    }

    /// The tap at `lag` (0 outside the window).
    pub fn tap(&self, lag: isize) -> Complex32 {
        let i = lag + self.pre as isize;
        if i < 0 || i as usize >= self.taps.len() {
            Complex32::new(0.0, 0.0)
        } else {
            self.taps[i as usize]
        }
    }
}

/// The widest window of a header (pre, post) and the cap of their sum.
pub(crate) fn widest(kind: Header) -> (usize, usize, usize) {
    match kind {
        Header::Pn420 => (80, 81, 165),
        Header::Pn595 => (110, 118, 229),
        Header::Pn945 => (215, 215, 434),
    }
}

/// The window a fit starts with.
pub(crate) fn tracking(kind: Header) -> (usize, usize) {
    match kind {
        Header::Pn945 => (40, 160),
        _ => (24, 100),
    }
}

/// Least-squares channel fits of one kind of header.
pub(crate) struct Estimator {
    pub kind: Header,
    pub seq: HeaderSeq,
    /// `a (1 + j)`.
    pub amp: Complex32,
    /// The core period (595 for PN595, which is fitted with its fixed chips).
    p: usize,
    fft: Fft,
    ifft: Fft,
    conj_core: Vec<Complex32>,
    twiddle: Vec<Complex32>,
    u: Vec<Complex32>,
    corr: Vec<Complex32>,
    chips: Vec<f32>,
    /// PN595: inverted normal matrices per window, newest last.
    inverses: Vec<((usize, usize), Vec<f32>)>,
    rhs: Vec<Complex32>,
    g: Vec<Complex32>,
}

impl Estimator {
    /// The estimator of a kind of header.
    pub fn new(kind: Header) -> Estimator {
        let seq = HeaderSeq::new(kind, false);
        let a = kind.amplitude();
        let p = seq.core().len();
        let mut conj_core: Vec<Complex32> = seq
            .core()
            .iter()
            .map(|c| Complex32::new(c.re / a, 0.0))
            .collect();
        Fft::forward(p).process(&mut conj_core);
        conj_core.iter_mut().for_each(|c| *c = c.conj());
        let twiddle = (0..p)
            .map(|n| Complex32::from_polar(1.0, -std::f32::consts::TAU * n as f32 / p as f32))
            .collect();
        let mut chips = vec![0f32; kind.len()];
        seq.chips(0, &mut chips);
        Estimator {
            kind,
            amp: Complex32::new(a, a),
            p,
            fft: Fft::forward(p),
            ifft: Fft::inverse(p),
            conj_core,
            twiddle,
            u: vec![Complex32::new(0.0, 0.0); p],
            corr: vec![Complex32::new(0.0, 0.0); p],
            chips,
            inverses: Vec::new(),
            rhs: Vec::new(),
            g: Vec::new(),
            seq,
        }
    }

    /// The first row of the fit of a periodic header for a window: the middle of the rows that keep every chip in the header.
    fn t0(&self, pre: usize, post: usize) -> usize {
        ((post - 1) + (self.kind.len() - self.p - pre)) / 2
    }

    /// The circular correlation of one core period of `y` from row `t0` with the phase-0 core, into `corr`: a channel tap at
    /// lag `d` under PN phase `ph` lands at `d - ph`. Returns the energy of the rows.
    fn correlate(&mut self, y: &[Complex32], t0: usize) -> f32 {
        let p = self.p;
        self.u.copy_from_slice(&y[t0..t0 + p]);
        let e: f32 = self.u.iter().map(|v| v.norm_sqr()).sum();
        self.fft.process(&mut self.u);
        for (k, (c, &u)) in self.corr.iter_mut().zip(&self.u).enumerate() {
            *c = u * self.conj_core[k] * self.twiddle[(k * t0) % p];
        }
        self.ifft.process(&mut self.corr);
        let s = 1.0 / p as f32;
        self.corr.iter_mut().for_each(|c| *c *= s);
        e
    }

    /// The right-hand side `C^T y` and the chip-unit taps `g` of a periodic header; returns (rows, row energy).
    fn solve_periodic(
        &mut self,
        y: &[Complex32],
        ph: usize,
        pre: usize,
        post: usize,
    ) -> (usize, f32) {
        let (p, t) = (self.p, pre + post);
        let t0 = self.t0(pre, post);
        let e = self.correlate(y, t0);
        self.rhs.clear();
        for j in 0..t {
            let lag = j as isize - pre as isize;
            self.rhs
                .push(self.corr[(lag - ph as isize).rem_euclid(p as isize) as usize]);
        }
        // C^T C = (P + 1) I - J, whose inverse is (I + J / (P + 1 - T)) / (P + 1).
        let add = self.rhs.iter().sum::<Complex32>() / (p + 1 - t) as f32;
        let pp = (p + 1) as f32;
        self.g.clear();
        self.g.extend(self.rhs.iter().map(|r| (r + add) / pp));
        (p, e)
    }

    /// The inverse of the normal matrix of PN595 for a window (with a ridge of 0.001), from the cache when it is there.
    fn inverse_595(&mut self, pre: usize, post: usize) -> usize {
        if let Some(i) = self.inverses.iter().position(|(w, _)| *w == (pre, post)) {
            return i;
        }
        let (l, t) = (self.kind.len(), pre + post);
        let c = &self.chips;
        let mut m = vec![0f64; t * t];
        for a in 0..t {
            for b in a..t {
                let (la, lb) = (a as isize - pre as isize, b as isize - pre as isize);
                let s: f64 = (post - 1..l - pre)
                    .map(|r| (c[(r as isize - la) as usize] * c[(r as isize - lb) as usize]) as f64)
                    .sum();
                m[a * t + b] = s;
                m[b * t + a] = s;
            }
            m[a * t + a] += 1e-3;
        }
        let inv = invert(&mut m, t);
        if self.inverses.len() >= 24 {
            self.inverses.remove(0);
        }
        self.inverses.push(((pre, post), inv));
        self.inverses.len() - 1
    }

    /// The right-hand side and taps of PN595 (fixed chips, rows `post - 1 .. L - pre`); returns (rows, row energy).
    fn solve_595(&mut self, y: &[Complex32], pre: usize, post: usize) -> (usize, f32) {
        let (l, t) = (self.kind.len(), pre + post);
        let k = self.inverse_595(pre, post);
        let rows = post - 1..l - pre;
        let e: f32 = y[rows.clone()].iter().map(|v| v.norm_sqr()).sum();
        self.rhs.clear();
        for j in 0..t {
            let lag = j as isize - pre as isize;
            let c0 = (rows.start as isize - lag) as usize;
            let chips = &self.chips[c0..c0 + rows.len()];
            let s: Complex32 = y[rows.clone()].iter().zip(chips).map(|(v, &c)| v * c).sum();
            self.rhs.push(s);
        }
        let inv = &self.inverses[k].1;
        self.g.clear();
        for a in 0..t {
            let row = &inv[a * t..(a + 1) * t];
            self.g
                .push(row.iter().zip(&self.rhs).map(|(&m, r)| r * m).sum());
        }
        (rows.len(), e)
    }

    /// Fits the channel of the header in `y` (its samples from its first) with PN phase `ph` over lags `-pre .. post` into
    /// `out` (its buffer is reused); cleans the taps below 12 times their variance.
    pub fn fit(&mut self, y: &[Complex32], ph: usize, pre: usize, post: usize, out: &mut Fit) {
        let t = pre + post;
        let (rows, energy) = if self.kind == Header::Pn595 {
            self.solve_595(y, pre, post)
        } else {
            self.solve_periodic(y, ph, pre, post)
        };
        let fitted: f32 = self
            .g
            .iter()
            .zip(&self.rhs)
            .map(|(g, r)| (g.conj() * r).re)
            .sum();
        let noise = ((energy - fitted.max(0.0)) / (rows - t).max(1) as f32).max(1e-20);
        let var = noise / (rows as f32 * self.amp.norm_sqr());
        let mut best = 0f32;
        out.taps.clear();
        for g in &self.g {
            let h = g / self.amp;
            let m = h.norm_sqr();
            best = best.max(m);
            out.taps.push(if m < 12.0 * var {
                Complex32::new(0.0, 0.0)
            } else {
                h
            });
        }
        out.pre = pre;
        out.post = post;
        out.noise = noise;
        out.var = var;
        out.usable = best > 25.0 * var;
    }
}

/// Inverts the symmetric positive definite `t x t` matrix `m` (row major) by Gauss-Jordan elimination.
fn invert(m: &mut [f64], t: usize) -> Vec<f32> {
    let mut inv = vec![0f64; t * t];
    for i in 0..t {
        inv[i * t + i] = 1.0;
    }
    for c in 0..t {
        let piv = m[c * t + c];
        let piv = if piv.abs() < 1e-12 { 1e-12 } else { piv };
        for j in 0..t {
            m[c * t + j] /= piv;
            inv[c * t + j] /= piv;
        }
        for r in 0..t {
            if r == c {
                continue;
            }
            let f = m[r * t + c];
            if f == 0.0 {
                continue;
            }
            for j in 0..t {
                m[r * t + j] -= f * m[c * t + j];
                inv[r * t + j] -= f * inv[c * t + j];
            }
        }
    }
    inv.into_iter().map(|v| v as f32).collect()
}
