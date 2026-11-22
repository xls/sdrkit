//! Spectral amplitude enhancement (TIA-102.BABA chapter 8) and adaptive smoothing (chapter 9).

use crate::model::Params;
use std::f64::consts::PI;

/// Start value of tau_M(-1) (Equation 115), which Annex A does not print: the value Equation 115 sets on a clean channel.
pub const TAU_M_START: f64 = 20480.0;

/// Start value of the local energy S_E (Annex A).
const S_E_START: f64 = 75000.0;

/// The state of enhancement and smoothing.
#[derive(Clone, Copy, Debug)]
pub struct Enhancer {
    s_e: f64,
    tau_m: f64,
}

impl Default for Enhancer {
    fn default() -> Self {
        Enhancer {
            s_e: S_E_START,
            tau_m: TAU_M_START,
        }
    }
}

impl Enhancer {
    /// Enhances the amplitudes of `p` in place (Equations 105 to 111) and updates the local energy.
    pub fn enhance(&mut self, p: &mut Params) {
        let l = p.l;
        let w0 = p.w0;
        let mut r0 = 0.0f64;
        let mut r1 = 0.0f64;
        for li in 1..=l {
            let m2 = p.amp[li] * p.amp[li];
            r0 += m2;
            r1 += m2 * (w0 * li as f64).cos();
        }
        let den = w0 * r0 * (r0 * r0 - r1 * r1);
        if r0 > 0.0 && den > 0.0 && den.is_finite() {
            let mut sum = 0.0;
            for li in 1..=l {
                let m = p.amp[li];
                let e = if 8 * li <= l {
                    m
                } else {
                    let num = 0.96 * PI * (r0 * r0 + r1 * r1 - 2.0 * r0 * r1 * (w0 * li as f64).cos());
                    let w = m.sqrt() * (num / den).max(0.0).powf(0.25);
                    if w > 1.2 {
                        1.2 * m
                    } else if w < 0.5 {
                        0.5 * m
                    } else {
                        w * m
                    }
                };
                p.amp[li] = e;
                sum += e * e;
            }
            if sum > 0.0 {
                let g = (r0 / sum).sqrt();
                for a in p.amp.iter_mut().take(l + 1).skip(1) {
                    *a *= g;
                }
            }
        }
        let s = 0.95 * self.s_e + 0.05 * r0;
        self.s_e = if s.is_finite() && s >= 10000.0 { s } else { 10000.0 };
    }

    /// Adaptive smoothing of voicing and amplitudes (Equations 112 to 116) for the error rate `eps_r`, the frame's error
    /// count `eps_t` and the error count `eps_4` of the first Hamming word.
    pub fn smooth(&mut self, p: &mut Params, eps_r: f64, eps_t: u32, eps_4: u32) {
        let vm = if eps_r <= 0.005 && eps_t <= 4 {
            f64::INFINITY
        } else if eps_r <= 0.0125 && eps_4 == 0 {
            45.255 * self.s_e.powf(0.375) / (277.26 * eps_r).exp()
        } else {
            1.414 * self.s_e.powf(0.375)
        };
        let mut am = 0.0;
        for li in 1..=p.l {
            if p.amp[li] > vm {
                p.voiced[li] = true;
            }
            am += p.amp[li];
        }
        self.tau_m = if eps_r <= 0.005 && eps_t <= 6 {
            20480.0
        } else {
            6000.0 - 300.0 * eps_t as f64 + self.tau_m
        };
        // A threshold that Equation 115 drives below 0 scales to silence, not to a negative gain.
        if self.tau_m <= am && am > 0.0 {
            let g = (self.tau_m / am).max(0.0);
            for a in p.amp.iter_mut().take(p.l + 1).skip(1) {
                *a *= g;
            }
        }
    }
}
