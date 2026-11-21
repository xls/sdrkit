//! The order of a PSK from the moments of its differential phase steps: for unit steps `u`, `M2 = |mean u^2|`, `M4 = |mean u^4|`
//! and `M8 = |mean u^8|`, each averaged with a decay per symbol. BPSK steps make `M2` near 1; QPSK leaves `M2` small and `M4` near
//! 1; 8-PSK leaves both small and `M8` near 1.
//!
//! ```
//! use sdrkit_algo_modem::pskorder::PskOrder;
//! use sdrkit_algo_modem::Complex32;
//! let mut m = PskOrder::new(0.995);
//! for k in 0..400 {
//!     let step = (k * 7 % 4) as f32 * std::f32::consts::FRAC_PI_2;
//!     m.push(Complex32::from_polar(1.0, step + 0.01));
//!     m.decay();
//! }
//! assert_eq!(m.order(), Some(4));
//! ```

use crate::Complex32;
use num_complex::Complex64;

/// The moment measure. See the module documentation.
#[derive(Clone, Debug)]
pub struct PskOrder {
    keep: f64,
    m2: Complex64,
    m4: Complex64,
    m8: Complex64,
    weight: f64,
    /// `M2` above this is order 2.
    pub th2: f64,
    /// Else `M4` above this is order 4.
    pub th4: f64,
    /// Else `M8` above this is order 8.
    pub th8: f64,
}

impl PskOrder {
    /// A measure whose sums are multiplied by `keep` at each [`PskOrder::decay`]; thresholds 0.55, 0.5 and 0.35.
    pub fn new(keep: f64) -> Self {
        PskOrder {
            keep,
            m2: Complex64::default(),
            m4: Complex64::default(),
            m8: Complex64::default(),
            weight: 0.0,
            th2: 0.55,
            th4: 0.5,
            th8: 0.35,
        }
    }

    /// Forgets the steps.
    pub fn reset(&mut self) {
        let k = self.keep;
        *self = PskOrder {
            th2: self.th2,
            th4: self.th4,
            th8: self.th8,
            ..PskOrder::new(k)
        };
    }

    /// Adds a step (its amplitude is dropped; a step of no amplitude or not finite is skipped).
    pub fn push(&mut self, u: Complex32) {
        let n = u.norm();
        if !(n > 0.0 && n.is_finite()) {
            return;
        }
        let u = Complex64::new(f64::from(u.re / n), f64::from(u.im / n));
        let u2 = u * u;
        let u4 = u2 * u2;
        self.m2 += u2;
        self.m4 += u4;
        self.m8 += u4 * u4;
        self.weight += 1.0;
    }

    /// Ages the sums by one symbol.
    pub fn decay(&mut self) {
        self.m2 *= self.keep;
        self.m4 *= self.keep;
        self.m8 *= self.keep;
        self.weight *= self.keep;
    }

    fn mean(&self, m: Complex64) -> f64 {
        if self.weight > 0.0 {
            m.norm() / self.weight
        } else {
            0.0
        }
    }

    /// `M2`.
    pub fn m2(&self) -> f64 {
        self.mean(self.m2)
    }
    /// `M4`.
    pub fn m4(&self) -> f64 {
        self.mean(self.m4)
    }
    /// `M8`.
    pub fn m8(&self) -> f64 {
        self.mean(self.m8)
    }

    /// The order: 2, 4 or 8, or `None` when no moment stands out.
    pub fn order(&self) -> Option<u32> {
        if self.m2() > self.th2 {
            Some(2)
        } else if self.m4() > self.th4 {
            Some(4)
        } else if self.m8() > self.th8 {
            Some(8)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_order() {
        for order in [2u32, 4, 8] {
            let mut m = PskOrder::new(0.995);
            for k in 0..600u32 {
                let a = (k.wrapping_mul(2_654_435_761) >> 7) % order;
                let ph = std::f32::consts::TAU * a as f32 / order as f32;
                m.push(Complex32::from_polar(1.0, ph));
                m.decay();
            }
            assert_eq!(m.order(), Some(order));
        }
    }
}
