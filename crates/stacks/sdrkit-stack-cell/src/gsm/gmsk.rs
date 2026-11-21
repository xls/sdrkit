//! The GMSK burst equaliser of the receiver: a least-squares channel estimate on the training sequence,
//! timing by explained energy, and a max-log BCJR over 16 states that uses the known tail bits and returns one soft value per
//! bit.

use sdrkit_algo_modem::gmsk::{ls_channel, Mlse};
use sdrkit_core::Complex32;

/// The symbol rate, symbols per second.
pub const SYMBOL_RATE: f64 = 1_625_000.0 / 6.0;

/// Taps of the channel model.
pub const TAPS: usize = 5;

/// A demodulated burst.
#[derive(Clone, Debug)]
pub struct BurstOut {
    /// One soft value per bit (positive: 0), 148 bits.
    pub soft: [f32; 148],
    /// The sample position of symbol 0 found.
    pub start: i64,
    /// Burst SNR, dB.
    pub snr_db: f32,
}

#[inline]
fn rot(k: usize) -> Complex32 {
    // (-j)^k
    match k & 3 {
        0 => Complex32::new(1.0, 0.0),
        1 => Complex32::new(0.0, -1.0),
        2 => Complex32::new(-1.0, 0.0),
        _ => Complex32::new(0.0, 1.0),
    }
}

/// The channel estimator of one training sequence: a least-squares fit of the linearised GMSK model on the training symbols.
pub struct Trainer {
    /// The burst position of the first training bit.
    pos: usize,
    /// The training symbols (+1 for bit 0, -1 for bit 1).
    s: Vec<Complex32>,
}

impl Trainer {
    /// For training bits `bits` (at most 64) at burst bit position `pos`.
    pub fn new(bits: &[u8], pos: usize) -> Trainer {
        let s = bits
            .iter()
            .take(64)
            .map(|&b| Complex32::new(1.0 - 2.0 * b as f32, 0.0))
            .collect();
        Trainer { pos, s }
    }

    /// The channel estimate at timing `t` (sample of symbol 0) in a 4-sps stream: taps, explained energy, noise variance.
    fn fit(&self, x: &[Complex32], t: i64) -> Option<([Complex32; TAPS], f32, f32)> {
        let n = self.s.len();
        let mut z = [Complex32::new(0.0, 0.0); 64];
        for (m, zm) in z.iter_mut().enumerate().take(n).skip(TAPS - 1) {
            let k = self.pos + m;
            let i = t + 4 * k as i64;
            if i < 0 || i as usize >= x.len() {
                return None;
            }
            *zm = x[i as usize] * rot(k);
        }
        let mut g = [Complex32::new(0.0, 0.0); TAPS];
        let fit = ls_channel(&z[..n], &self.s, &mut g, false).ok()?;
        let mut fit_e = 0f32;
        for k in TAPS - 1..n {
            let y: Complex32 = g.iter().enumerate().map(|(l, gl)| gl * self.s[k - l]).sum();
            fit_e += y.norm_sqr();
        }
        Some((g, fit_e, fit.noise_var))
    }

    /// Demodulates a burst whose symbol 0 is predicted at sample `pred` of the 4-sps stream `x`, searching +-`search` symbols.
    /// `known`: bits known to be zero (tails). None when no timing fits or the SNR is below 1.5 dB.
    pub fn demod(&self, x: &[Complex32], pred: f64, search: i64) -> Option<BurstOut> {
        let c = pred.round() as i64;
        let mut best: Option<(i64, [Complex32; TAPS], f32, f32)> = None;
        for t in c - 4 * search..=c + 4 * search {
            if let Some((g, e, n)) = self.fit(x, t) {
                if best.as_ref().is_none_or(|b| e > b.2) {
                    best = Some((t, g, e, n));
                }
            }
        }
        let (t, g, _, noise) = best?;
        let gp: f32 = g.iter().map(|v| v.norm_sqr()).sum();
        let snr_db = 10.0 * (gp / noise.max(1e-12)).log10();
        if snr_db <= 1.5 {
            return None;
        }
        let soft = burst_llrs(x, t, &g, noise.max(1e-9))?;
        Some(BurstOut {
            soft,
            start: t,
            snr_db,
        })
    }
}

/// The bits of a normal burst whose values are known: the three tail bits at each end are 0 (45.002 5.2.3).
fn known_zero(k: usize) -> bool {
    !(3..145).contains(&k)
}

/// The LLRs (positive means 0) of the 148 bits of the burst whose symbol 0 is at sample `t` of the 4-sps stream `x`, for channel
/// `g` and noise variance `noise`, by the shared max-log BCJR. The tail bits are set to a sure 0. `None` when the channel is not
/// usable.
fn burst_llrs(x: &[Complex32], t: i64, g: &[Complex32; TAPS], noise: f32) -> Option<[f32; 148]> {
    let mut y = [Complex32::new(0.0, 0.0); 148];
    for (k, v) in y.iter_mut().enumerate() {
        let i = t + 4 * k as i64;
        if i >= 0 && (i as usize) < x.len() {
            *v = x[i as usize] * rot(k);
        }
    }
    let mut mlse = Mlse::new(g, 148).ok()?;
    let mut llr = [0f32; 148];
    mlse.soft(&y, noise, None, &mut llr).ok()?;
    for (k, l) in llr.iter_mut().enumerate() {
        *l = if known_zero(k) {
            50.0
        } else {
            l.clamp(-50.0, 50.0)
        };
    }
    Some(llr)
}

#[cfg(test)]
mod tests {
    use super::super::codes::{normal_burst, TSC};
    use super::*;

    #[test]
    fn modulate_and_equalise_a_burst() {
        // GMSK of 45.004 with differential encoding, 20 zero bits on each side of the burst.
        let mut m = sdrkit_algo_sim::modulate::Cpm::gmsk(0.3, 4.0)
            .unwrap()
            .with_held_edges();
        let mut x = Vec::new();
        let mut data = [0u8; 114];
        let mut s = 12345u32;
        for d in data.iter_mut() {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
            *d = ((s >> 16) & 1) as u8;
        }
        let b = normal_burst(&data, 3, 1);
        let mut prev = 0;
        let levels: Vec<f32> = std::iter::repeat_n(0u8, 20)
            .chain(b.iter().copied())
            .chain(std::iter::repeat_n(0, 20))
            .map(|bit| {
                let d = bit ^ prev;
                prev = bit;
                1.0 - 2.0 * d as f32
            })
            .collect();
        m.push(&levels, &mut x);
        m.flush(&mut x);
        // A phase rotation and a gain change are absorbed by the channel estimate.
        for v in x.iter_mut() {
            *v *= Complex32::from_polar(0.5, 1.0);
        }
        let tr = Trainer::new(&TSC[3], 61);
        let out = tr.demod(&x, 80.0 + 3.0, 4).expect("demodulates");
        let mut errors = 0;
        for (i, &bit) in b.iter().enumerate() {
            if (out.soft[i] < 0.0) != (bit == 1) {
                errors += 1;
            }
        }
        assert_eq!(errors, 0, "timing {}", out.start);
        assert!(out.snr_db > 20.0);
    }
}
