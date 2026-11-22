//! Speech synthesis (TIA-102.BABA chapter 11): the unvoiced part by filtered noise and weighted overlap-add, the voiced part
//! by a bank of harmonic oscillators with phase continuity, and the phase update.

use crate::model::{Params, FRAME, MAX_L};
use crate::tables::full::W_R;
use sdrkit_algo_dsp::fft_exact::ExactFft;
use sdrkit_algo_dsp::Complex32;
use std::f64::consts::{PI, TAU};

/// DFT size of the unvoiced synthesis.
const N_DFT: usize = 256;
/// Half length of the synthesis window: w_S is zero outside -105..=105.
const WS_HALF: usize = 105;
/// Noise samples in one window, n = -104..=104.
const NOISE_LEN: usize = 209;
/// Modulus of the noise generator (Equation 117).
const NOISE_MOD: u32 = 53125;
/// Harmonics below this number may use the phase-interpolated oscillator (Equation 134).
const INTERP_BELOW: usize = 8;

/// The synthesis window w_S(n) for |n| <= 105 (Annex I, which the formula reproduces to all printed digits).
pub fn ws(n: i32) -> f64 {
    let a = n.unsigned_abs() as f64;
    if a <= 55.0 {
        1.0
    } else if a <= WS_HALF as f64 {
        (WS_HALF as f64 - a) / 50.0
    } else {
        0.0
    }
}

/// The noise generator step (Equation 117).
fn next_noise(u: u32) -> u32 {
    (171 * u + 11213) % NOISE_MOD
}

/// The synthesiser state.
pub struct Synth {
    prev: Params,
    phi: [f64; MAX_L + 1],
    psi: [f64; MAX_L + 1],
    noise: [f64; NOISE_LEN],
    noise_u: u32,
    started: bool,
    uw_prev: [f64; N_DFT],
    uw_cur: [f64; N_DFT],
    fwd: ExactFft,
    inv: ExactFft,
    buf: [Complex32; N_DFT],
    gamma_w: f64,
    ws: [f64; 2 * WS_HALF + 1],
}

impl Default for Synth {
    fn default() -> Self {
        Self::new()
    }
}

impl Synth {
    /// A synthesiser with the initial values of Annex A.
    pub fn new() -> Self {
        let ws_tab: [f64; 2 * WS_HALF + 1] = std::array::from_fn(|i| ws(i as i32 - WS_HALF as i32));
        let sum_wr: f64 = W_R[0] + 2.0 * W_R[1..].iter().sum::<f64>();
        let sum_wr2: f64 = W_R[0] * W_R[0] + 2.0 * W_R[1..].iter().map(|w| w * w).sum::<f64>();
        let sum_ws2: f64 = ws_tab.iter().map(|w| w * w).sum();
        Synth {
            prev: Params::silent(0.02985 * PI, 30),
            phi: [0.0; MAX_L + 1],
            psi: [0.0; MAX_L + 1],
            noise: [0.0; NOISE_LEN],
            noise_u: 3147,
            started: false,
            uw_prev: [0.0; N_DFT],
            uw_cur: [0.0; N_DFT],
            fwd: ExactFft::new(N_DFT, false).expect("256 is a power of two"),
            inv: ExactFft::new(N_DFT, true).expect("256 is a power of two"),
            buf: [Complex32::new(0.0, 0.0); N_DFT],
            gamma_w: sum_wr * (sum_ws2 / sum_wr2).sqrt(),
            ws: ws_tab,
        }
    }

    fn w(&self, n: i32) -> f64 {
        if n.unsigned_abs() as usize > WS_HALF {
            0.0
        } else {
            self.ws[(n + WS_HALF as i32) as usize]
        }
    }

    /// Advances the noise by one frame: the window keeps its last 49 samples and takes 160 new ones.
    fn advance_noise(&mut self) {
        let keep = if self.started { NOISE_LEN - FRAME } else { 0 };
        if self.started {
            self.noise.copy_within(FRAME.., 0);
        }
        for i in keep..NOISE_LEN {
            self.noise_u = next_noise(self.noise_u);
            self.noise[i] = self.noise_u as f64;
        }
        self.started = true;
    }

    /// The parameters of the last synthesised frame.
    pub fn last(&self) -> &Params {
        &self.prev
    }

    /// Synthesises one frame from `cur` (after enhancement and smoothing) into `out`. Without `random_phase` the phase of
    /// Equation 140 is psi for every harmonic, which keeps a tone's frequency steady.
    pub fn run(&mut self, cur: &Params, random_phase: bool, out: &mut [f64; FRAME]) {
        let cur = sanitise(cur);
        self.advance_noise();
        self.update_phase(&cur);
        self.unvoiced(&cur, out);
        self.voiced(&cur, random_phase, out);
        self.prev = cur;
        std::mem::swap(&mut self.uw_prev, &mut self.uw_cur);
    }

    /// The phase psi of Equation 139 for every harmonic, kept modulo 2 pi.
    fn update_phase(&mut self, cur: &Params) {
        let n = FRAME as f64;
        for l in 1..=MAX_L {
            let psi = self.psi[l] + (self.prev.w0 + cur.w0) * l as f64 * n / 2.0;
            self.psi[l] = psi.rem_euclid(TAU);
        }
    }

    fn unvoiced(&mut self, cur: &Params, out: &mut [f64; FRAME]) {
        for v in self.buf.iter_mut() {
            *v = Complex32::new(0.0, 0.0);
        }
        for (i, &u) in self.noise.iter().enumerate() {
            let n = i as i32 - 104;
            self.buf[n.rem_euclid(N_DFT as i32) as usize] = Complex32::new((u * self.w(n)) as f32, 0.0);
        }
        let _ = self.fwd.process(&mut self.buf);
        let spec = self.buf;
        for v in self.buf.iter_mut() {
            *v = Complex32::new(0.0, 0.0);
        }
        let f = N_DFT as f64 / TAU * cur.w0;
        for l in 1..=cur.l {
            if cur.voiced[l] {
                continue;
            }
            let a = (((l as f64 - 0.5) * f).ceil().max(1.0) as usize).min(N_DFT / 2 + 1);
            let b = (((l as f64 + 0.5) * f).ceil() as usize).min(N_DFT / 2 + 1);
            if b <= a {
                continue;
            }
            let e: f64 = (a..b).map(|m| spec[m].norm_sqr() as f64).sum::<f64>() / (b - a) as f64;
            if e <= 0.0 {
                continue;
            }
            let g = (self.gamma_w * cur.amp[l] / e.sqrt()) as f32;
            for m in a..b {
                self.buf[m] = spec[m] * g;
                let neg = (N_DFT - m) % N_DFT;
                self.buf[neg] = spec[neg] * g;
            }
        }
        let _ = self.inv.process(&mut self.buf);
        for (i, v) in self.buf.iter().enumerate() {
            self.uw_cur[i] = v.re as f64 / N_DFT as f64;
        }
        // Weighted overlap-add (Equation 126); u~_w is indexed by n mod 256 for n = -128..=127.
        let at = |w: &[f64; N_DFT], n: i32| -> f64 {
            if (-128..128).contains(&n) {
                w[n.rem_euclid(N_DFT as i32) as usize]
            } else {
                0.0
            }
        };
        for (n, o) in out.iter_mut().enumerate() {
            let n = n as i32;
            let (a, b) = (self.w(n), self.w(n - FRAME as i32));
            let den = a * a + b * b;
            *o = if den > 0.0 {
                (a * at(&self.uw_prev, n) + b * at(&self.uw_cur, n - FRAME as i32)) / den
            } else {
                0.0
            };
        }
    }

    /// The voiced part (Equations 127 to 138) added to `out`, with phi(0) of Equations 140 and 141: the random phase of
    /// harmonic `l` uses the noise sample at index `l` of the current frame's window.
    fn voiced(&mut self, cur: &Params, random_phase: bool, out: &mut [f64; FRAME]) {
        let prev = self.prev;
        let nf = FRAME as f64;
        let lmax = prev.l.max(cur.l);
        let l_uv = (1..=cur.l).filter(|&l| !cur.voiced[l]).count() as f64;
        let mut phi0 = [0.0f64; MAX_L + 1];
        for l in 1..=MAX_L {
            phi0[l] = if !random_phase || l <= cur.l / 4 || l > lmax || cur.l == 0 {
                self.psi[l]
            } else {
                let rho = TAU / NOISE_MOD as f64 * self.random_phase_sample(l) - PI;
                self.psi[l] + l_uv * rho / cur.l as f64
            };
        }
        let close = (cur.w0 - prev.w0).abs() < 0.1 * cur.w0;
        for l in 1..=lmax {
            let vp = l <= prev.l && prev.voiced[l];
            let vc = l <= cur.l && cur.voiced[l];
            let mp = if l <= prev.l { prev.amp[l] } else { 0.0 };
            let mc = if l <= cur.l { cur.amp[l] } else { 0.0 };
            let lf = l as f64;
            if vp && vc && l < INTERP_BELOW && close {
                // Equations 134 to 138.
                let dphi = phi0[l] - self.phi[l] - (prev.w0 + cur.w0) * lf * nf / 2.0;
                let dw = (dphi - TAU * ((dphi + PI) / TAU).floor()) / nf;
                for (n, o) in out.iter_mut().enumerate() {
                    let n = n as f64;
                    let a = mp + n / nf * (mc - mp);
                    let th = self.phi[l] + (prev.w0 * lf + dw) * n + (cur.w0 - prev.w0) * lf * n * n / (2.0 * nf);
                    *o += 2.0 * a * th.cos();
                }
                continue;
            }
            if vp {
                // Equation 131: the previous harmonic fades out over w_S(n).
                self.oscillate(out, 0, 2.0 * mp, prev.w0 * lf, self.phi[l]);
            }
            if vc {
                // Equation 132: the current harmonic fades in over w_S(n - N).
                self.oscillate(out, FRAME as i32, 2.0 * mc, cur.w0 * lf, phi0[l]);
            }
        }
        self.phi = phi0;
    }

    /// The noise sample u(l) of Equation 141: index `l` of the current frame's noise window (n = l, from its centre).
    fn random_phase_sample(&self, l: usize) -> f64 {
        self.noise[(NOISE_LEN / 2 + l).min(NOISE_LEN - 1)]
    }

    /// Adds `amp w_S(n - centre) cos(w (n - centre) + phase)` for `n = 0..160`.
    fn oscillate(&self, out: &mut [f64; FRAME], centre: i32, amp: f64, w: f64, phase: f64) {
        if amp == 0.0 {
            return;
        }
        let start = -centre;
        let mut z = (phase + w * start as f64).sin_cos();
        let (s, c) = w.sin_cos();
        for (n, o) in out.iter_mut().enumerate() {
            let k = n as i32 - centre;
            let win = self.w(k);
            if win != 0.0 {
                *o += amp * win * z.1;
            }
            // Rotate by w: (sin, cos) of the next angle.
            z = (z.0 * c + z.1 * s, z.1 * c - z.0 * s);
        }
    }
}

/// A copy of `p` with a finite fundamental, a harmonic count within range and finite amplitudes.
fn sanitise(p: &Params) -> Params {
    let mut q = *p;
    if !q.w0.is_finite() || q.w0 <= 0.0 {
        q.w0 = 0.02985 * PI;
    }
    q.l = q.l.min(MAX_L);
    for a in q.amp.iter_mut() {
        if !a.is_finite() {
            *a = 0.0;
        }
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_sums() {
        let s: f64 = (-105..=105).map(ws).sum();
        assert!((s - 160.0).abs() < 1e-9);
        for n in 0..160 {
            assert!((ws(n) + ws(n - 160) - 1.0).abs() < 1e-12);
        }
    }

    fn rms(x: &[f64]) -> f64 {
        (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt()
    }

    #[test]
    fn stationary_voiced_is_continuous() {
        let mut s = Synth::new();
        let mut p = Params::silent(TAU / 80.0, 36);
        for l in 1..=36 {
            p.voiced[l] = true;
            p.amp[l] = 300.0;
        }
        let mut all = Vec::new();
        let mut out = [0.0; FRAME];
        for _ in 0..8 {
            s.run(&p, true, &mut out);
            all.extend_from_slice(&out);
        }
        // After the first frames the signal repeats with period 80 samples.
        let tail = &all[4 * FRAME..];
        let mut worst = 0.0f64;
        for i in 80..tail.len() {
            worst = worst.max((tail[i] - tail[i - 80]).abs());
        }
        assert!(worst < 1e-3 * rms(tail), "{worst} vs {}", rms(tail));
    }

    #[test]
    fn unvoiced_band_energy_follows_amplitude() {
        let mut s = Synth::new();
        let mut p = Params::silent(TAU / 50.0, 21);
        for l in 1..=21 {
            p.amp[l] = 100.0;
        }
        let mut all = Vec::new();
        let mut out = [0.0; FRAME];
        for _ in 0..40 {
            s.run(&p, true, &mut out);
            all.extend_from_slice(&out);
        }
        let r = rms(&all[FRAME..]);
        assert!(r > 50.0 && r < 2000.0, "{r}");
    }
}
