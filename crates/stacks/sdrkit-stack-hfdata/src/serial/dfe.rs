//! The decision-feedback equaliser of the serial-tone modems: 33 feed-forward taps at T/2 and 12 feedback taps, normalised LMS
//! with step 0.25, adapting on known symbols only; a second-order phase loop after it; the noise estimate of the known symbols.
//!
//! Each symbol is taken in the descrambled frame: the scrambler value and the loop's phase are the equaliser's rotation, so a
//! decision is the nearest point of the rate's own constellation and a known symbol is its tribit before scrambling.

use super::front::cis;
use super::psk8;
use sdrkit_algo_dsp::constellation::{PointSet, Shape};
use sdrkit_algo_modem::equalizer::{AdaptiveEq, EqConfig, Mode};
use sdrkit_algo_modem::Constellation;
use sdrkit_core::Complex32;
use std::f64::consts::FRAC_PI_4;

/// Feed-forward taps (T/2).
pub const FF_TAPS: usize = 33;
/// Feedback taps (T).
pub const FB_TAPS: usize = 12;
/// The lag of the equaliser: symbol `k` (sample `2k`) is out when sample `2k + LAG` is in.
pub const LAG: usize = FF_TAPS / 2;

/// The equaliser, its phase loop and its noise estimate.
pub struct Dfe {
    eq: AdaptiveEq,
    phase: f64,
    freq: f64,
    sigma2: f32,
    noise_floor: f32,
}

/// The constellation of the allowed tribits `pts` (each `n` the point at `n` x 45 degrees).
pub fn constellation(pts: &[u8]) -> Constellation {
    let p: Vec<Complex32> = pts.iter().map(|&n| psk8(n)).collect();
    let set = PointSet::new(
        p,
        Shape::Psk {
            m: pts.len() as u32,
        },
    )
    .or_else(|_| PointSet::psk(8, 0.0))
    .expect("an 8-PSK set");
    Constellation::new(set).expect("a valid constellation")
}

impl Dfe {
    /// An equaliser deciding on the tribits `pts`, with a noise floor of `noise_floor` for the soft bits.
    pub fn new(pts: &[u8], noise_floor: f32) -> Self {
        let mut cfg = EqConfig::new(FF_TAPS, 2);
        cfg.feedback = FB_TAPS;
        cfg.mu = 0.25;
        cfg.normalised = true;
        cfg.mode = Mode::Frozen;
        cfg.dd_switch_mse = None;
        cfg.mse_alpha = 0.01;
        Dfe {
            eq: AdaptiveEq::new(cfg, constellation(pts)).expect("a valid equaliser"),
            phase: 0.0,
            freq: 0.0,
            sigma2: 0.1,
            noise_floor,
        }
    }

    /// The same equaliser deciding on `pts`: the feed-forward taps, the loop and the noise are kept; the feedback starts again.
    pub fn with_points(&self, pts: &[u8]) -> Self {
        let mut d = Dfe::new(pts, self.noise_floor);
        let mut w = vec![Complex32::default(); FF_TAPS];
        if self.eq.feed_forward(&mut w).is_ok() {
            let _ = d.eq.set_feed_forward(&w);
        }
        d.phase = self.phase;
        d.freq = self.freq;
        d.sigma2 = self.sigma2;
        d
    }

    /// Symbol `k` from samples `2k + LAG - 1` and `2k + LAG` (sample `2k` the symbol's centre), with scrambler tribit `scr`. With `known`, the equaliser adapts towards it and the loop and the noise follow. Returns the
    /// equalised symbol in the descrambled frame.
    pub fn step(&mut self, pair: [Complex32; 2], scr: u8, known: Option<u8>) -> Complex32 {
        let rot = cis(-(self.phase + f64::from(scr & 7) * FRAC_PI_4));
        self.eq.set_rotation(rot);
        let a = known.map(psk8);
        let yd = self.eq.step(&pair, a).unwrap_or_default();
        match a {
            Some(a) => {
                let e = f64::from((yd * a.conj()).arg());
                self.freq = (self.freq + 0.0008 * e).clamp(-0.02, 0.02);
                self.phase += self.freq + 0.05 * e;
                let d = (a - yd).norm_sqr();
                if d.is_finite() {
                    self.sigma2 = 0.995 * self.sigma2 + 0.005 * d;
                }
            }
            None => self.phase += self.freq,
        }
        self.phase = self.phase.rem_euclid(std::f64::consts::TAU);
        yd
    }

    /// The noise variance for soft bits (at least the floor).
    pub fn sigma2(&self) -> f32 {
        self.sigma2.max(self.noise_floor)
    }

    /// The measured noise variance.
    pub fn noise(&self) -> f32 {
        self.sigma2
    }

    /// The SNR the noise gives, dB.
    pub fn snr_db(&self) -> f32 {
        -10.0 * self.sigma2.max(1e-6).log10()
    }

    /// The loop's frequency, radians a symbol.
    pub fn loop_freq(&self) -> f64 {
        self.freq
    }
}
