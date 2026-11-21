//! Demodulation of one locked frame: FFT windows, fractional timing, differential cells with common phase error correction, the
//! SNR measure, and soft bits by coherent detection with a cleaned channel estimate or by differential detection.

use super::{carrier_index, carrier_of, load_window};
use crate::tables::{self, CARRIERS, SYMBOLS, SYMBOL_BITS, TS, TU};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32 as C;
use std::f32::consts::FRAC_PI_4;

/// How the soft bits of a frame were made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Detection {
    /// With the channel estimate.
    Coherent,
    /// From the product of consecutive symbols.
    #[default]
    Differential,
}

/// What a frame's demodulation measured.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameMeasure {
    /// Mean magnitude of the differential cells.
    pub mean_cell: f64,
    /// Mean |LLR| of the coherent path (0 when not used).
    pub mean_llr: f64,
    /// The SNR measure in dB.
    pub snr_db: f32,
    /// The detection used.
    pub detection: Detection,
}

/// Per-frame demodulator state and scratch.
pub struct Demod {
    fft: Fft,
    ifft: Fft,
    prs_k: Vec<C>,
    map: Vec<u16>,
    /// Received carriers of the 76 symbols, active carriers in k order.
    pub(crate) sym: Vec<C>,
    /// Differential cells of symbols 1 … 75.
    pub(crate) cells: Vec<C>,
    h: Vec<C>,
    sigma2: Vec<f32>,
    llr: Vec<f32>,
    win: Vec<C>,
    tmp: Vec<C>,
    dec: Vec<C>,
    mask: Vec<bool>,
    pw: Vec<f32>,
    slope: Vec<f32>,
    lse_table: [f32; 65],
}

impl Default for Demod {
    fn default() -> Self {
        Self::new()
    }
}

impl Demod {
    /// A demodulator.
    pub fn new() -> Self {
        let prs = tables::prs_bins();
        let prs_k = (0..CARRIERS)
            .map(|i| prs[tables::bin(carrier_of(i))])
            .collect();
        let map = tables::frequency_interleaver()
            .iter()
            .map(|&k| carrier_index(k) as u16)
            .collect();
        let mut lse_table = [0f32; 65];
        for (i, t) in lse_table.iter_mut().enumerate() {
            *t = (1.0 + (-(i as f32) / 8.0).exp()).ln();
        }
        Demod {
            fft: Fft::forward(TU),
            ifft: Fft::inverse(TU),
            prs_k,
            map,
            sym: vec![C::default(); SYMBOLS * CARRIERS],
            cells: vec![C::default(); (SYMBOLS - 1) * CARRIERS],
            h: vec![C::default(); SYMBOLS * CARRIERS],
            sigma2: vec![0.0; SYMBOLS],
            llr: vec![0.0; (SYMBOLS - 1) * SYMBOL_BITS],
            win: vec![C::default(); TU],
            tmp: vec![C::default(); TU],
            dec: vec![C::default(); CARRIERS],
            mask: vec![false; TU],
            pw: Vec::with_capacity(TU),
            slope: (0..CARRIERS).map(|i| carrier_of(i) as f32).collect(),
            lse_table,
        }
    }

    /// Transforms the 76 symbols of the frame whose PRS starts (useful part) at absolute `u0`, with carrier offset
    /// `cfo` (spacings) and relative clock error `eps`.
    pub fn transform(&mut self, buf: &[C], buf_start: i64, u0: f64, cfo: f64, eps: f64) {
        let k = 1.0 + eps;
        for l in 0..SYMBOLS {
            let pos = u0 + (l * TS) as f64 * k;
            let ri = pos.round();
            let r = (pos - ri) as f32;
            load_window(buf, buf_start, ri as i64 - 16, cfo, &mut self.win);
            self.fft.process(&mut self.win);
            let dst = &mut self.sym[l * CARRIERS..(l + 1) * CARRIERS];
            let w = std::f32::consts::TAU * r / TU as f32;
            for (i, d) in dst.iter_mut().enumerate() {
                let v = self.win[tables::bin(carrier_of(i))];
                *d = if r != 0.0 {
                    let a = w * self.slope[i];
                    v * C::new(a.cos(), a.sin())
                } else {
                    v
                };
            }
        }
    }

    /// Differential cells with common phase error correction; returns their mean magnitude.
    fn differential(&mut self) -> f64 {
        let mut total = 0f64;
        for l in 1..SYMBOLS {
            let (prev, cur) = self.sym.split_at(l * CARRIERS);
            let prev = &prev[(l - 1) * CARRIERS..];
            let cur = &cur[..CARRIERS];
            let cells = &mut self.cells[(l - 1) * CARRIERS..l * CARRIERS];
            let mut s4 = C::default();
            let mut pw = 0f32;
            for i in 0..CARRIERS {
                let c = cur[i] * prev[i].conj();
                cells[i] = c;
                let m2 = c.norm_sqr();
                if m2 > 0.0 {
                    let c2 = c * c;
                    s4 += c2 * c2 / m2;
                }
                pw += m2;
            }
            if s4.norm() > 0.1 * pw {
                let th = (-s4).arg() / 4.0;
                let rot = C::new(th.cos(), -th.sin());
                for c in cells.iter_mut() {
                    *c *= rot;
                }
            }
            total += cells.iter().map(|c| c.norm() as f64).sum::<f64>();
        }
        total / ((SYMBOLS - 1) * CARRIERS) as f64
    }

    /// The SNR measure over the differential cells of symbols 9 … 12 (section 5.6).
    fn snr(&self) -> f32 {
        let mut e2 = 0f64;
        let mut n = 0usize;
        for l in 9..=12 {
            for c in &self.cells[(l - 1) * CARRIERS..l * CARRIERS] {
                let a = c.arg() - FRAC_PI_4;
                let q = std::f32::consts::FRAC_PI_2;
                let e = a - (a / q).round() * q;
                e2 += (e * e) as f64;
                n += 1;
            }
        }
        let m = (e2 / n.max(1) as f64).max(1e-3);
        (-10.0 * m.log10()) as f32
    }

    /// The constellation: the cells of symbol 9, radius square-rooted, unit mean radius.
    pub fn constellation(&self, out: &mut [C]) {
        let cells = &self.cells[8 * CARRIERS..9 * CARRIERS];
        let mut mean = 0f32;
        for (o, c) in out.iter_mut().zip(cells) {
            let r = c.norm();
            let s = r.sqrt();
            *o = if r > 0.0 { c * (s / r) } else { C::default() };
            mean += s;
        }
        mean /= CARRIERS as f32;
        if mean > 0.0 {
            for o in out.iter_mut() {
                *o /= mean;
            }
        }
    }

    /// Cleans an estimate in the delay domain with the current mask.
    fn clean(&mut self, l: usize) {
        self.tmp.iter_mut().for_each(|v| *v = C::default());
        for i in 0..CARRIERS {
            self.tmp[tables::bin(carrier_of(i))] = self.h[l * CARRIERS + i];
        }
        self.ifft.process(&mut self.tmp);
        for (v, &m) in self.tmp.iter_mut().zip(&self.mask) {
            if !m {
                *v = C::default();
            }
        }
        self.fft.process(&mut self.tmp);
        let s = 1.0 / TU as f32;
        for i in 0..CARRIERS {
            self.h[l * CARRIERS + i] = self.tmp[tables::bin(carrier_of(i))] * s;
        }
    }

    /// Builds the delay mask from the PRS estimate; false when the channel is not clean enough for coherent detection.
    fn make_mask(&mut self) -> bool {
        self.tmp.iter_mut().for_each(|v| *v = C::default());
        for i in 0..CARRIERS {
            self.tmp[tables::bin(carrier_of(i))] = self.h[i];
        }
        self.ifft.process(&mut self.tmp);
        self.pw.clear();
        self.pw.extend(self.tmp.iter().map(|v| v.norm_sqr()));
        let mid = self.pw.len() / 2;
        let med = *self.pw.select_nth_unstable_by(mid, |a, b| a.total_cmp(b)).1;
        let thr = 12.0 * med.max(1e-30);
        self.mask.iter_mut().for_each(|m| *m = false);
        let mut any = 0usize;
        for i in 0..TU {
            if self.tmp[i].norm_sqr() > thr {
                any += 1;
                for d in -2i32..=2 {
                    self.mask[(i as i32 + d).rem_euclid(TU as i32) as usize] = true;
                }
            }
        }
        let count = self.mask.iter().filter(|&&m| m).count();
        any > 0 && count <= TU / 4
    }

    #[inline]
    fn lse(&self, a: f32, b: f32) -> f32 {
        let (m, d) = if a > b { (a, a - b) } else { (b, b - a) };
        let i = (d * 8.0) as usize;
        m + if i < 64 { self.lse_table[i] } else { 0.0 }
    }

    /// The coherent path (section 5.5): fills `self.llr`; returns the mean |LLR|, or `None` when the frame must use differential
    /// detection.
    fn coherent(&mut self) -> Option<f64> {
        // Initial estimate from the PRS.
        for i in 0..CARRIERS {
            self.h[i] = self.sym[i] * self.prs_k[i].conj();
        }
        if !self.make_mask() {
            return None;
        }
        self.clean(0);
        let mut s2 = 0f32;
        for i in 0..CARRIERS {
            s2 += (self.sym[i] - self.h[i] * self.prs_k[i]).norm_sqr();
        }
        self.sigma2[0] = s2 / CARRIERS as f32;
        let mut change = 0f64;
        let mut power = 0f64;
        let diag = C::new(FRAC_PI_4.cos(), FRAC_PI_4.sin());
        for l in 1..SYMBOLS {
            let odd = l % 2 == 1;
            for i in 0..CARRIERS {
                let pred = if l == 1 {
                    self.h[i]
                } else {
                    self.h[(l - 1) * CARRIERS + i] * 2.0 - self.h[(l - 2) * CARRIERS + i]
                };
                let y = self.sym[l * CARRIERS + i];
                let mut q = y * pred.conj();
                if odd {
                    q *= diag.conj();
                }
                // Nearest axis point.
                let a = if q.re.abs() >= q.im.abs() {
                    C::new(q.re.signum(), 0.0)
                } else {
                    C::new(0.0, q.im.signum())
                };
                let a = if odd { a * diag } else { a };
                self.h[l * CARRIERS + i] = y * a.conj();
                self.dec[i] = a;
            }
            self.clean(l);
            let mut s2 = 0f32;
            for i in 0..CARRIERS {
                let hl = self.h[l * CARRIERS + i];
                s2 += (self.sym[l * CARRIERS + i] - hl * self.dec[i]).norm_sqr();
                change += (hl - self.h[(l - 1) * CARRIERS + i]).norm_sqr() as f64;
                power += hl.norm_sqr() as f64;
            }
            self.sigma2[l] = s2 / CARRIERS as f32;
        }
        if power <= 0.0 || change >= 0.05 * power {
            return None;
        }
        // Soft bits.
        let inv_sqrt2 = std::f32::consts::FRAC_1_SQRT_2;
        let ds = [
            C::new(inv_sqrt2, inv_sqrt2),
            C::new(-inv_sqrt2, inv_sqrt2),
            C::new(inv_sqrt2, -inv_sqrt2),
            C::new(-inv_sqrt2, -inv_sqrt2),
        ];
        let mut sum_abs = 0f64;
        for l in 1..SYMBOLS {
            let sig = 0.5 * (self.sigma2[l - 1] + self.sigma2[l]);
            let k2 = 2.0 / sig.max(1e-12);
            // The absolute phase set of the earlier symbol: axes for even l − 1, diagonals for odd.
            let s0 = if (l - 1) % 2 == 1 {
                diag
            } else {
                C::new(1.0, 0.0)
            };
            let base = (l - 1) * SYMBOL_BITS;
            for n in 0..CARRIERS {
                let i = self.map[n] as usize;
                let a = self.h[(l - 1) * CARRIERS + i].conj() * self.sym[(l - 1) * CARRIERS + i];
                let b = self.h[l * CARRIERS + i].conj() * self.sym[l * CARRIERS + i];
                let mut m = [0f32; 4];
                for (j, d) in ds.iter().enumerate() {
                    let w = s0.conj() * (a + b * d.conj());
                    let x = self.lse(self.lse(w.re, -w.re), self.lse(w.im, -w.im));
                    m[j] = k2 * x;
                }
                // bit n: 1 when Re(d) < 0 (d1, d3); bit n + 1536: 1 when Im(d) < 0 (d2, d3). LLRs: positive means 0.
                let l0 = self.lse(m[0], m[2]) - self.lse(m[1], m[3]);
                let l1 = self.lse(m[0], m[1]) - self.lse(m[2], m[3]);
                self.llr[base + n] = l0;
                self.llr[base + CARRIERS + n] = l1;
                sum_abs += (l0.abs() + l1.abs()) as f64;
            }
        }
        Some(sum_abs / self.llr.len() as f64)
    }

    /// Demodulates the transformed frame. `slow_cell` and `slow_llr` are the slow averages over found frames (0 before any).
    /// Writes 75 × 3072 soft values into `soft`.
    pub fn demodulate(
        &mut self,
        coherent: bool,
        slow_cell: f64,
        slow_llr: f64,
        erased: &[bool; SYMBOLS],
        soft: &mut [i8],
    ) -> FrameMeasure {
        let m = self.demodulate_inner(coherent, slow_cell, slow_llr, soft);
        // Symbols whose window does not hold the symbol (samples were lost inside the frame) carry no information: their soft
        // bits, and those of the symbol after them (the differential pair), are erased.
        for l in 1..SYMBOLS {
            if erased[l] || erased[l - 1] {
                soft[(l - 1) * SYMBOL_BITS..l * SYMBOL_BITS].fill(0);
            }
        }
        m
    }

    fn demodulate_inner(
        &mut self,
        coherent: bool,
        slow_cell: f64,
        slow_llr: f64,
        soft: &mut [i8],
    ) -> FrameMeasure {
        let mean_cell = self.differential();
        let snr_db = self.snr();
        let mut m = FrameMeasure {
            mean_cell,
            mean_llr: 0.0,
            snr_db,
            detection: Detection::Differential,
        };
        if coherent {
            if let Some(mean_llr) = self.coherent() {
                m.mean_llr = mean_llr;
                m.detection = Detection::Coherent;
                let scale = (40.0 / mean_llr.max(slow_llr).max(1e-12)) as f32;
                for (o, &v) in soft.iter_mut().zip(&self.llr) {
                    *o = (v * scale).round().clamp(-127.0, 127.0) as i8;
                }
                return m;
            }
        }
        let scale = (40.0 / mean_cell.max(slow_cell).max(1e-20)) as f32;
        for l in 1..SYMBOLS {
            let cells = &self.cells[(l - 1) * CARRIERS..l * CARRIERS];
            let out = &mut soft[(l - 1) * SYMBOL_BITS..l * SYMBOL_BITS];
            for n in 0..CARRIERS {
                let c = cells[self.map[n] as usize];
                out[n] = (c.re * scale).round().clamp(-127.0, 127.0) as i8;
                out[CARRIERS + n] = (c.im * scale).round().clamp(-127.0, 127.0) as i8;
            }
        }
        m
    }
}
