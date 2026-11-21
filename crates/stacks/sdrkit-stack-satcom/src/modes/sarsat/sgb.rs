//! The second-generation receiver on samples at exactly 153.6 kHz (4 per chip): a differential matched filter on the
//! known preamble (insensitive to the carrier offset), computed with FFTs over a ring of 1.7 s; the carrier from the filter's phase,
//! refined on the preamble; a decision-directed carrier loop and an early-late chip timing loop over the 150 channel bits of each arm;
//! BCH(250,202) and the T.018 decoder.

use crate::modes::sarsat::t018::{self, Sgb, BITS_PER_ARM, CHIPS_PER_BIT, PREAMBLE_BITS};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

pub const RATE: f64 = 153_600.0;
const SPC: usize = 4;
const PRE_SAMPLES: usize = PREAMBLE_BITS * CHIPS_PER_BIT * SPC; // 25 600
const BURST_SAMPLES: usize = BITS_PER_ARM * CHIPS_PER_BIT * SPC; // 153 600
const FFT_N: usize = 65_536;
const RING: usize = 262_144; // 1.7 s

/// The chip level (+1 for chip 0, -1 for chip 1) of arm `q` at chip `c`.
#[inline]
fn level(q: bool, c: usize) -> f32 {
    if t018::prn(q)[c] == 0 {
        1.0
    } else {
        -1.0
    }
}

/// The preamble waveform at 4 samples per chip (I leads, Q half a chip later).
fn preamble_wave() -> Vec<Complex32> {
    let n = PRE_SAMPLES + 2;
    (0..n)
        .map(|i| {
            let ci = i / SPC;
            let re = if ci < PREAMBLE_BITS * CHIPS_PER_BIT {
                level(false, ci)
            } else {
                0.0
            };
            let im = if i >= 2 && (i - 2) / SPC < PREAMBLE_BITS * CHIPS_PER_BIT {
                level(true, (i - 2) / SPC)
            } else {
                0.0
            };
            Complex32::new(re, im)
        })
        .collect()
}

/// A decoded (or failed) second-generation burst.
#[derive(Clone, Debug, PartialEq)]
pub struct SgbBurst {
    /// Absolute index (at 153.6 kHz) of the first chip.
    pub start: u64,
    /// Carrier offset from the receiver's centre, Hz.
    pub carrier_hz: f64,
    /// SNR of the despread channel bits, dB.
    pub snr_db: f64,
    pub message: Option<Sgb>,
}

/// The second-generation detector and despreader.
pub struct SgbReceiver {
    fft: Fft,
    ifft: Fft,
    /// The conjugate spectrum of the differential template.
    tmpl: Vec<Complex32>,
    ring: Vec<Complex32>,
    /// Absolute index of the next sample.
    pub index: u64,
    /// Absolute index up to which the detector has looked (start positions).
    searched: u64,
    pending: Option<(u64, f64, f32)>,
    /// A candidate waiting for the next block (an m-sequence's shifted copies give smaller peaks just before a burst).
    cand: Option<(u64, f64, f64)>,
    /// Index before which no new burst is looked for (the last burst's end).
    quiet_until: u64,
    work: Vec<Complex32>,
    energy: Vec<f64>,
    noise_ref: f32,
    tmpl_energy: f64,
    code: t018::Code,
}

impl Default for SgbReceiver {
    fn default() -> Self {
        Self::new()
    }
}

impl SgbReceiver {
    pub fn new() -> Self {
        let w = preamble_wave();
        let mut t = vec![Complex32::new(0.0, 0.0); FFT_N];
        for i in 1..w.len() {
            t[i - 1] = w[i] * w[i - 1].conj();
        }
        // Without its mean the template ignores the constant part of the products (|r|^2 times the carrier's turn per sample), so
        // only the exact alignment adds up.
        let m = w.len() - 1;
        let mean = t[..m].iter().fold(Complex32::new(0.0, 0.0), |a, v| a + v) / m as f32;
        for v in t[..m].iter_mut() {
            *v -= mean;
        }
        let tmpl_energy: f64 = t.iter().map(|v| v.norm_sqr() as f64).sum();
        let mut fft = Fft::forward(FFT_N);
        fft.process(&mut t);
        for v in t.iter_mut() {
            *v = v.conj();
        }
        SgbReceiver {
            fft,
            ifft: Fft::inverse(FFT_N),
            tmpl: t,
            ring: vec![Complex32::new(0.0, 0.0); RING],
            index: 0,
            searched: 0,
            pending: None,
            cand: None,
            quiet_until: 0,
            work: vec![Complex32::new(0.0, 0.0); FFT_N],
            energy: vec![0.0; FFT_N + 1],
            noise_ref: 0.0,
            tmpl_energy,
            code: t018::Code::new(),
        }
    }

    pub fn reset(&mut self) {
        let i = self.index;
        *self = SgbReceiver::new();
        self.index = i;
        self.searched = i;
        self.quiet_until = i;
    }

    #[inline]
    fn at(&self, abs: u64) -> Complex32 {
        if abs >= self.index || self.index - abs > RING as u64 {
            return Complex32::new(0.0, 0.0);
        }
        self.ring[(abs % RING as u64) as usize]
    }

    /// Feeds samples at 153.6 kHz; appends bursts whose last chip has arrived.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<SgbBurst>) {
        for &s in x {
            self.ring[(self.index % RING as u64) as usize] = s;
            self.index += 1;
            // A detected burst is decoded once all of it is in the ring.
            if let Some((st, f, _)) = self.pending {
                if self.index >= st + BURST_SAMPLES as u64 + 8 {
                    self.pending = None;
                    out.push(self.despread(st, f));
                    self.quiet_until = st + BURST_SAMPLES as u64;
                    self.searched = self.searched.max(self.quiet_until);
                }
            }
            // Search in steps: every start position from `searched` needs PRE_SAMPLES samples after it.
            let step = (FFT_N - PRE_SAMPLES) as u64;
            if self.pending.is_none() && self.index >= self.searched + step + PRE_SAMPLES as u64 {
                self.search(step);
            }
        }
    }

    fn search(&mut self, step: u64) {
        let base = self.searched;
        // Differential products d[n] = r[n] r[n-1]*, n from base.
        for i in 0..FFT_N {
            let a = base + i as u64;
            self.work[i] = if a + 1 < self.index {
                self.at(a + 1) * self.at(a).conj()
            } else {
                Complex32::new(0.0, 0.0)
            };
        }
        for i in 0..FFT_N {
            self.energy[i + 1] = self.energy[i] + self.work[i].norm_sqr() as f64;
        }
        self.fft.process(&mut self.work);
        for (w, t) in self.work.iter_mut().zip(self.tmpl.iter()) {
            *w *= t;
        }
        self.ifft.process(&mut self.work);
        // The peak of the normalised correlation over the valid lags 0..step (|c|^2 over the energies of the window and the
        // template). A window's energy is taken as at least the mean over the lags, so a window that is mostly silence before a
        // burst does not look like a preamble.
        let n_lags = (step as usize).min(self.work.len());
        let e_mean = (0..n_lags)
            .map(|i| self.energy[i + PRE_SAMPLES] - self.energy[i])
            .sum::<f64>()
            / n_lags.max(1) as f64;
        let mut best = (0usize, 0f32);
        let mut best_rho = 0f64;
        let mut mean = 0f64;
        for (i, v) in self.work.iter().enumerate().take(step as usize) {
            let p = v.norm_sqr();
            mean += p as f64;
            let e = (self.energy[i + PRE_SAMPLES] - self.energy[i]).max(e_mean);
            let rho = p as f64 / (e * self.tmpl_energy * (FFT_N as f64).powi(2) + 1e-30);
            if rho > best_rho {
                best_rho = rho;
                best = (i, p);
            }
        }
        mean /= step as f64;
        // The noise level follows the quiet stretches.
        if self.noise_ref == 0.0 {
            self.noise_ref = mean as f32;
        }
        let thr = 40.0 * self.noise_ref.min(mean as f32 * 4.0).max(1e-30);
        let found = best.1 > thr && best_rho > 1.5e-3 && base + (best.0 as u64) >= self.quiet_until;
        let here = found.then(|| {
            let c = self.work[best.0];
            (
                base + best.0 as u64,
                (c.im as f64).atan2(c.re as f64) * RATE / TAU,
                best_rho,
            )
        });
        match (self.cand.take(), here) {
            (None, Some(h)) => {
                // Look at the next block before deciding.
                self.cand = Some(h);
                self.searched = base + step;
            }
            (Some(c), h) => {
                let pick = match h {
                    Some(h) if h.2 > c.2 => h,
                    _ => c,
                };
                self.pending = Some((pick.0, pick.1, pick.2 as f32));
                self.searched = pick.0 + 1;
            }
            (None, None) => {
                self.noise_ref = 0.9 * self.noise_ref + 0.1 * mean as f32;
                self.searched = base + step;
            }
        }
    }

    /// Despreads a burst whose first I chip is at `st` (absolute), with the carrier at `f` Hz.
    fn despread(&mut self, st: u64, f_in: f64) -> SgbBurst {
        // The residual carrier from the despread preamble chips: a tone at 38.4 kHz, found with an FFT (6400 chips, zero padded).
        let mut f0 = f_in;
        {
            let w = TAU * f0 / RATE;
            let n = PREAMBLE_BITS * CHIPS_PER_BIT;
            let mut buf = vec![Complex32::new(0.0, 0.0); 16_384];
            for (c, b) in buf.iter_mut().enumerate().take(n) {
                let mut acc = Complex32::new(0.0, 0.0);
                for k in 0..SPC {
                    let i = (c * SPC + k) as u64;
                    let p = -w * i as f64;
                    let rot = Complex32::new(p.cos() as f32, p.sin() as f32);
                    acc += self.at(st + i) * rot * level(false, c);
                    let iq = i + 2;
                    let p = -w * iq as f64;
                    let rot = Complex32::new(p.cos() as f32, p.sin() as f32);
                    acc += self.at(st + iq) * rot * level(true, c) * Complex32::new(0.0, -1.0);
                }
                *b = acc;
            }
            let mut fft = Fft::forward(buf.len());
            fft.process(&mut buf);
            let mut best = (0usize, 0f32);
            for (i, v) in buf.iter().enumerate() {
                let p = v.norm_sqr();
                if p > best.1 {
                    best = (i, p);
                }
            }
            let nb = buf.len();
            let k = best.0;
            let mag = |i: usize| buf[i % nb].norm_sqr();
            let (a, b, c) = (mag(k + nb - 1), mag(k), mag(k + 1));
            let den = a - 2.0 * b + c;
            let frac = if den.abs() > 0.0 {
                (0.5 * (a - c) / den).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            let mut bin = k as f64 + frac as f64;
            if bin > (nb / 2) as f64 {
                bin -= nb as f64;
            }
            f0 += bin * t018::CHIP_RATE / nb as f64;
        }
        // Copy the burst out, rotated by the carrier estimate.
        let n = BURST_SAMPLES + 16;
        let mut x: Vec<Complex32> = Vec::with_capacity(n + 8);
        let w = TAU * f0 / RATE;
        for i in 0..n + 8 {
            let a = (st as i64 + i as i64 - 4).max(0) as u64;
            let p = -w * (i as f64 - 4.0);
            x.push(self.at(a) * Complex32::new(p.cos() as f32, p.sin() as f32));
        }
        // x[4] is the first chip's first sample.
        let samp = |t: f64| -> Complex32 {
            let t = t + 4.0;
            if t < 0.0 || t + 1.0 >= x.len() as f64 {
                return Complex32::new(0.0, 0.0);
            }
            let i = t.floor() as usize;
            let fr = (t - i as f64) as f32;
            x[i] * (1.0 - fr) + x[i + 1] * fr
        };
        // One arm's chip integral at time t (start of chip), 4 samples.
        let chip =
            |t: f64| -> Complex32 { samp(t) + samp(t + 1.0) + samp(t + 2.0) + samp(t + 3.0) };
        // Residual frequency and phase from the preamble: despread 25 bits per arm.
        let bit_val = |k: usize, tau: f64, q: bool| -> (Complex32, Complex32, Complex32) {
            let (mut p, mut e, mut l) = (
                Complex32::new(0.0, 0.0),
                Complex32::new(0.0, 0.0),
                Complex32::new(0.0, 0.0),
            );
            let off = if q { 2.0 } else { 0.0 };
            for c in k * CHIPS_PER_BIT..(k + 1) * CHIPS_PER_BIT {
                let t = tau + off + (c * SPC) as f64;
                let lv = level(q, c);
                p += chip(t) * lv;
                e += chip(t - 1.0) * lv;
                l += chip(t + 1.0) * lv;
            }
            (p, e, l)
        };
        // Preamble: I is A e^{j phi}, Q is j A e^{j phi}.
        let mut zs = Vec::with_capacity(PREAMBLE_BITS);
        for k in 0..PREAMBLE_BITS {
            let (pi, _, _) = bit_val(k, 0.0, false);
            let (pq, _, _) = bit_val(k, 0.0, true);
            zs.push(pi + pq * Complex32::new(0.0, -1.0));
        }
        // Frequency from the phase change across the preamble.
        let mut acc = Complex32::new(0.0, 0.0);
        for k in 1..zs.len() {
            acc += zs[k] * zs[k - 1].conj();
        }
        let tb = (CHIPS_PER_BIT * SPC) as f64 / RATE;
        let mut freq = (acc.im as f64).atan2(acc.re as f64) / (TAU * tb);
        let mut phase = {
            // Phase at the middle of the preamble, brought to bit 0 by the frequency.
            let mut s = Complex32::new(0.0, 0.0);
            for (k, z) in zs.iter().enumerate() {
                let p = -TAU * freq * tb * k as f64;
                s += z * Complex32::new(p.cos() as f32, p.sin() as f32);
            }
            (s.im as f64).atan2(s.re as f64)
        };
        let mut tau = 0.0f64;
        let mut bits_i = Vec::with_capacity(BITS_PER_ARM);
        let mut bits_q = Vec::with_capacity(BITS_PER_ARM);
        let mut amps = Vec::with_capacity(2 * BITS_PER_ARM);
        // A noise bandwidth of 5 % of the bit rate, damping 0.707: valid values, so the gains always exist.
        let (alpha, beta) =
            sdrkit_algo_dsp::loopfilter::loop_gains(0.05, 0.707, 1.0).unwrap_or((0.0, 0.0));
        for k in 0..BITS_PER_ARM {
            let (pi, ei, li) = bit_val(k, tau, false);
            let (pq, eq, lq) = bit_val(k, tau, true);
            // The carrier at this bit (bit k spans phase + 2 pi f tb k).
            let rot = Complex32::new((-phase).cos() as f32, (-phase).sin() as f32);
            let yi = pi * rot;
            let yq = pq * rot;
            let bi = yi.re < 0.0;
            let bq = yq.im < 0.0;
            bits_i.push(bi);
            bits_q.push(bq);
            amps.push(yi.re.abs());
            amps.push(yq.im.abs());
            // Phase error from both arms with their decisions.
            let si = if bi { -1.0 } else { 1.0 };
            let sq = if bq { -1.0 } else { 1.0 };
            let z = yi * si + yq * Complex32::new(0.0, -sq);
            let err = (z.im as f64).atan2(z.re as f64);
            freq += beta * err / (TAU * tb);
            phase += TAU * freq * tb + alpha * err;
            // Chip timing: early minus late on the decided values.
            let e = (ei.norm() + eq.norm()) as f64;
            let l = (li.norm() + lq.norm()) as f64;
            if e + l > 0.0 {
                tau += 0.5 * (l - e) / (e + l);
                tau = tau.clamp(-8.0, 8.0);
            }
        }
        // Message bits: odd bits (1, 3, ...) on I, even on Q, after the preamble.
        let mut msg = Vec::with_capacity(250);
        for k in PREAMBLE_BITS..BITS_PER_ARM {
            msg.push(bits_i[k]);
            msg.push(bits_q[k]);
        }
        let mean = amps.iter().map(|&a| a as f64).sum::<f64>() / amps.len() as f64;
        let var = amps.iter().map(|&a| (a as f64 - mean).powi(2)).sum::<f64>() / amps.len() as f64;
        let snr = 10.0 * (mean * mean / var.max(1e-30)).log10();
        SgbBurst {
            start: st,
            carrier_hz: f0 + freq,
            snr_db: snr,
            message: t018::decode(&msg, &mut self.code),
        }
    }
}
