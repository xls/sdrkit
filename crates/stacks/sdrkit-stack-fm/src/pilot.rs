//! The 19 kHz stereo pilot: the band filters and the pilot SNR, the phase-locked loop (`sdrkit_algo_modem::carrier::ToneLoop`) and
//! the lock decision.
//!
//! Standard: ITU-R BS.450 (pilot-tone system).

use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_algo_modem::carrier::ToneLoop;
use sdrkit_core::Complex32;

/// The loop's noise bandwidth, Hz: the bandwidth of a second-order loop of natural frequency 20 Hz and damping 0.707.
pub const LOOP_BN_HZ: f64 = 66.6;
/// The furthest the pilot can be from 19 kHz, Hz.
pub const MAX_OFFSET_HZ: f64 = 10.0;
/// The low-pass before the loop's phase detector, Hz.
const LOOP_LOWPASS_HZ: f64 = 500.0;

/// The output of the pilot loop for one MPX sample.
#[derive(Clone, Copy, Debug)]
pub struct PilotSample {
    /// `sin 2 theta` for the pilot `sin theta`: the 38 kHz carrier of the difference signal.
    pub sin2: f32,
    /// The 57 kHz phasor `e^(j 3 theta)`: the third harmonic of the pilot `sin theta`, the carrier the RDS demodulator uses.
    pub carrier57: Complex32,
    /// The lock decision.
    pub locked: bool,
    /// The pilot amplitude as a fraction of 75 kHz deviation.
    pub amplitude: f32,
}

/// Pilot detection, tracking and lock.
#[derive(Clone, Debug)]
pub struct Pilot {
    rate: f64,
    pll: ToneLoop,
    bands: [Biquad; 6],
    a50: f64,
    a20: f64,
    p19: f64,
    p17: f64,
    p21: f64,
    i: f64,
    q: f64,
    locked: bool,
    good_run: u32,
    bad_run: u32,
    lock_after: u32,
    unlock_after: u32,
}

impl Pilot {
    /// A pilot loop for MPX samples at `rate` (at least 50 kHz).
    pub fn new(rate: f64) -> Self {
        let bp = |f: f64| Biquad::new(BiquadCoeffs::bandpass(rate, f, 30.0));
        let smooth = |tau: f64| 1.0 - (-1.0 / (tau * rate)).exp();
        let pll = ToneLoop::new(rate, 19e3, LOOP_BN_HZ, MAX_OFFSET_HZ, LOOP_LOWPASS_HZ)
            .unwrap_or_else(|e| {
                unreachable!("the pilot loop fits every MPX rate above 50 kHz: {e}")
            });
        Pilot {
            rate,
            pll,
            bands: [bp(19e3), bp(19e3), bp(17e3), bp(17e3), bp(21e3), bp(21e3)],
            a50: smooth(0.050),
            a20: smooth(0.020),
            p19: 0.0,
            p17: 0.0,
            p21: 0.0,
            i: 0.0,
            q: 0.0,
            locked: false,
            good_run: 0,
            bad_run: 0,
            lock_after: (0.080 * rate) as u32,
            unlock_after: (0.200 * rate) as u32,
        }
    }

    /// Back to the state after construction.
    pub fn reset(&mut self) {
        *self = Self::new(self.rate);
    }

    /// Whether the pilot is locked.
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// The pilot amplitude as a fraction of 75 kHz (twice the in-phase correlation).
    pub fn amplitude(&self) -> f32 {
        (2.0 * self.i) as f32
    }

    /// The pilot SNR in dB (pilot band against the mean of the 17 and 21 kHz bands).
    pub fn snr_db(&self) -> f32 {
        let n = 0.5 * (self.p17 + self.p21);
        (10.0 * (self.p19 / n.max(1e-30)).log10()) as f32
    }

    /// The loop's frequency, Hz.
    pub fn freq_hz(&self) -> f64 {
        self.pll.freq_hz()
    }

    /// One MPX sample (DC removed, 75 kHz = 1.0).
    #[inline]
    pub fn process(&mut self, x: f32) -> PilotSample {
        let [b19a, b19b, b17a, b17b, b21a, b21b] = &mut self.bands;
        let p = b19b.process_sample(b19a.process_sample(x));
        let n17 = b17b.process_sample(b17a.process_sample(x)) as f64;
        let n21 = b21b.process_sample(b21a.process_sample(x)) as f64;
        let pf = p as f64;
        self.p19 += self.a50 * (pf * pf - self.p19);
        self.p17 += self.a50 * (n17 * n17 - self.p17);
        self.p21 += self.a50 * (n21 * n21 - self.p21);
        // The loop locks to `A cos psi`; the pilot is `A sin theta`, so theta = psi + pi / 2.
        let ph = self.pll.push_real(p);
        let (c, s) = (ph.re as f64, ph.im as f64);
        self.i += self.a20 * (pf * c - self.i);
        self.q += self.a20 * (pf * s - self.q);
        self.judge();
        let ph2 = ph * ph;
        // e^(j 3 theta) = -j e^(j 3 psi): the third harmonic of the pilot's own phase, as IEC 62106 locks the RDS carrier to.
        let c3 = ph2 * ph;
        PilotSample {
            sin2: -ph2.im,
            carrier57: Complex32::new(c3.im, -c3.re),
            locked: self.locked,
            amplitude: (2.0 * self.i) as f32,
        }
    }

    /// The lock decision, judged in time: "strong" (amplitude above 2.5 % and the pilot SNR above 12 dB to
    /// acquire, 8 dB to hold) and "clean" (in phase, the quadrature below 0.35 of it). Locks after 80 ms of both, unlocks after
    /// 200 ms without them or at once below 1.2 %.
    fn judge(&mut self) {
        let amp = 2.0 * self.i;
        let noise = (0.5 * (self.p17 + self.p21)).max(1e-30);
        let ratio = if self.locked {
            6.309_573_444_801_933
        } else {
            15.848_931_924_611_133
        };
        let strong = amp > 0.025 && self.p19 > ratio * noise;
        let clean = self.i > 0.0 && self.q.abs() < 0.35 * self.i;
        if self.locked {
            if amp < 0.012 {
                self.locked = false;
                self.bad_run = 0;
            } else if strong && clean {
                self.bad_run = 0;
            } else {
                self.bad_run += 1;
                if self.bad_run >= self.unlock_after {
                    self.locked = false;
                    self.bad_run = 0;
                }
            }
            self.good_run = 0;
        } else if strong && clean {
            self.good_run += 1;
            if self.good_run >= self.lock_after {
                self.locked = true;
                self.good_run = 0;
            }
        } else {
            self.good_run = 0;
        }
    }
}
