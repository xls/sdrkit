//! A fake radio, so every sweep feature is tested without one: carriers and wide signals at absolute frequencies, keyed and drifting
//! signals, noise, a settle transient after each retune, the anti-alias filter's roll-off, the IQ image, the DC spike, a spur at a fixed
//! offset from the tuning, and gain changes.

#![allow(dead_code)]

use sdrkit_algo_dsp::Complex32;
use std::f64::consts::TAU;

/// A small, fast random generator (xorshift64*), seeded, so tests repeat.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// A generator from a seed (any value).
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in [0, 1).
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Standard normal.
    pub fn gauss(&mut self) -> f64 {
        let u1 = self.uniform().max(1e-300);
        let u2 = self.uniform();
        (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
    }
}

/// A signal on the air.
#[derive(Clone, Debug, PartialEq)]
pub struct FakeSignal {
    /// Its centre at time 0, Hz.
    pub hz: f64,
    /// Its total power, dBFS.
    pub dbfs: f32,
    /// Its width, Hz (0: a pure carrier).
    pub width_hz: f64,
    /// Keyed: on for `.0` seconds of every `.1` seconds.
    pub keyed: Option<(f64, f64)>,
    /// Drift, Hz per second.
    pub drift_hz_per_s: f64,
    /// On the air from this time, s.
    pub start_s: f64,
    /// Off the air from this time, s.
    pub stop_s: f64,
}

impl FakeSignal {
    /// A carrier.
    pub fn carrier(hz: f64, dbfs: f32) -> Self {
        FakeSignal {
            hz,
            dbfs,
            width_hz: 0.0,
            keyed: None,
            drift_hz_per_s: 0.0,
            start_s: 0.0,
            stop_s: f64::INFINITY,
        }
    }
    /// A signal `width_hz` wide (a comb of tones with random phases, flat across the width).
    pub fn wide(hz: f64, dbfs: f32, width_hz: f64) -> Self {
        FakeSignal {
            width_hz,
            ..Self::carrier(hz, dbfs)
        }
    }
    /// Keyed on for `on_s` of every `period_s`.
    pub fn keyed(mut self, on_s: f64, period_s: f64) -> Self {
        self.keyed = Some((on_s, period_s));
        self
    }
    /// Drifting.
    pub fn drifting(mut self, hz_per_s: f64) -> Self {
        self.drift_hz_per_s = hz_per_s;
        self
    }
    /// On the air only between `start_s` and `stop_s`.
    pub fn between(mut self, start_s: f64, stop_s: f64) -> Self {
        self.start_s = start_s;
        self.stop_s = stop_s;
        self
    }
    fn on(&self, t: f64) -> bool {
        if t < self.start_s || t >= self.stop_s {
            return false;
        }
        match self.keyed {
            Some((on, period)) => (t - self.start_s).rem_euclid(period) < on,
            None => true,
        }
    }
}

#[derive(Clone, Debug)]
struct Tone {
    signal: usize,
    offset_hz: f64,
    amp: f32,
    phase: f64,
}

/// The fake radio.
#[derive(Clone, Debug)]
pub struct FakeRadio {
    rate_hz: f64,
    centre_hz: f64,
    signals: Vec<FakeSignal>,
    tones: Vec<Tone>,
    /// Noise power per sample, dBFS.
    pub noise_dbfs: f32,
    /// The settle time after a retune, s: the level ramps up from -30 dB over it.
    pub settle_s: f64,
    /// The IQ image level relative to each signal, dB (`None`: no image).
    pub image_db: Option<f32>,
    /// The DC spike's level, dBFS (`None`: none).
    pub dc_dbfs: Option<f32>,
    /// A one-pole roll-off of the capture (about -3 dB at ±0.375 × rate).
    pub rolloff: bool,
    /// A spur at a fixed offset from the tuning: (offset Hz, dBFS).
    pub spur: Option<(f64, f32)>,
    /// The radio's gain, dB, applied to everything.
    pub gain_db: f32,
    /// Retunes done.
    pub retunes: u64,
    settle_total: usize,
    settle_left: usize,
    samples: u64,
    lp: Complex32,
    spur_phase: f64,
    rng: Rng,
}

impl FakeRadio {
    /// A radio at `rate_hz`, tuned to `centre_hz`, with noise at -60 dBFS and nothing else.
    pub fn new(rate_hz: f64, centre_hz: f64, seed: u64) -> Self {
        FakeRadio {
            rate_hz,
            centre_hz,
            signals: Vec::new(),
            tones: Vec::new(),
            noise_dbfs: -60.0,
            settle_s: 0.0,
            image_db: None,
            dc_dbfs: None,
            rolloff: false,
            spur: None,
            gain_db: 0.0,
            retunes: 0,
            settle_total: 0,
            settle_left: 0,
            samples: 0,
            lp: Complex32::default(),
            spur_phase: 0.0,
            rng: Rng::new(seed),
        }
    }

    /// Adds a signal.
    pub fn add(&mut self, s: FakeSignal) {
        let idx = self.signals.len();
        let k = if s.width_hz <= 0.0 {
            1
        } else {
            ((s.width_hz / 1500.0).ceil() as usize).clamp(2, 64)
        };
        let amp = (10f32.powf(s.dbfs / 10.0) / k as f32).sqrt();
        for j in 0..k {
            let off = if k == 1 {
                0.0
            } else {
                -s.width_hz / 2.0 + s.width_hz * (j as f64 + 0.5) / k as f64
            };
            let phase = self.rng.uniform() * TAU;
            self.tones.push(Tone {
                signal: idx,
                offset_hz: off,
                amp,
                phase,
            });
        }
        self.signals.push(s);
    }

    /// The signals.
    pub fn signals(&self) -> &[FakeSignal] {
        &self.signals
    }

    /// Retunes (and changes the rate): the settle transient starts.
    pub fn tune(&mut self, centre_hz: f64, rate_hz: f64) {
        self.centre_hz = centre_hz;
        self.rate_hz = rate_hz;
        self.settle_total = (self.settle_s * rate_hz).round() as usize;
        self.settle_left = self.settle_total;
        self.retunes += 1;
    }

    /// Where it is tuned, Hz.
    pub fn centre_hz(&self) -> f64 {
        self.centre_hz
    }

    /// The rate, Hz.
    pub fn rate_hz(&self) -> f64 {
        self.rate_hz
    }

    /// Seconds of samples produced.
    pub fn time_s(&self) -> f64 {
        self.samples as f64 / self.rate_hz
    }

    /// Lets time pass without reading samples (a scheduled pause).
    pub fn skip(&mut self, seconds: f64) {
        self.samples += (seconds * self.rate_hz).round() as u64;
    }

    /// Fills `out` with the next samples.
    pub fn read(&mut self, out: &mut [Complex32]) {
        let sigma = (10f64.powf(self.noise_dbfs as f64 / 10.0) / 2.0).sqrt();
        let gain = 10f32.powf(self.gain_db / 20.0);
        let image = self.image_db.map(|d| 10f32.powf(d / 20.0));
        let dc = self
            .dc_dbfs
            .map(|d| Complex32::new(10f32.powf(d / 20.0), 0.0))
            .unwrap_or_default();
        let nyq = self.rate_hz / 2.0;
        let t0 = self.time_s();
        // Which signals are on during this block (blocks are short against keying).
        let on: Vec<bool> = self.signals.iter().map(|s| s.on(t0)).collect();
        for o in out.iter_mut() {
            let t = self.samples as f64 / self.rate_hz;
            let mut sig = Complex32::default();
            for tone in &mut self.tones {
                let s = &self.signals[tone.signal];
                let f = s.hz + tone.offset_hz + s.drift_hz_per_s * t - self.centre_hz;
                if f.abs() < nyq {
                    if on[tone.signal] {
                        sig += Complex32::from_polar(tone.amp, tone.phase as f32);
                    }
                    tone.phase = (tone.phase + TAU * f / self.rate_hz).rem_euclid(TAU);
                }
            }
            if let Some(e) = image {
                sig += sig.conj() * e;
            }
            if let Some((off, d)) = self.spur {
                sig += Complex32::from_polar(10f32.powf(d / 20.0), self.spur_phase as f32);
                self.spur_phase = (self.spur_phase + TAU * off / self.rate_hz).rem_euclid(TAU);
            }
            let noise = Complex32::new(
                (self.rng.gauss() * sigma) as f32,
                (self.rng.gauss() * sigma) as f32,
            );
            let mut x = (sig + noise) * gain + dc;
            if self.rolloff {
                self.lp += (x - self.lp) * 0.8;
                x = self.lp;
            }
            if self.settle_left > 0 {
                let frac = self.settle_left as f32 / self.settle_total.max(1) as f32;
                x *= 10f32.powf(-30.0 * frac / 20.0);
                self.settle_left -= 1;
            }
            *o = x;
            self.samples += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn power_db(v: &[Complex32]) -> f32 {
        let p: f32 = v.iter().map(|c| c.norm_sqr()).sum::<f32>() / v.len() as f32;
        10.0 * p.log10()
    }

    #[test]
    fn noise_level_and_carrier() {
        let mut r = FakeRadio::new(1e6, 100e6, 1);
        let mut buf = vec![Complex32::default(); 100_000];
        r.read(&mut buf);
        assert!((power_db(&buf) + 60.0).abs() < 0.2);
        r.add(FakeSignal::carrier(100.1e6, -20.0));
        r.read(&mut buf);
        assert!((power_db(&buf) + 20.0).abs() < 0.2);
        // Out of the capture: not heard.
        r.tune(200e6, 1e6);
        r.read(&mut buf);
        assert!((power_db(&buf) + 60.0).abs() < 0.2);
    }

    #[test]
    fn settle_ramps_and_keying() {
        let mut r = FakeRadio::new(1e6, 100e6, 2);
        r.settle_s = 0.01;
        r.add(FakeSignal::carrier(100.1e6, -10.0).keyed(0.5, 1.0));
        r.tune(100e6, 1e6);
        let mut buf = vec![Complex32::default(); 1000];
        r.read(&mut buf);
        assert!(power_db(&buf) < -25.0);
        let mut rest = vec![Complex32::default(); 20_000];
        r.read(&mut rest);
        assert!((power_db(&rest[10_000..]) + 10.0).abs() < 0.3);
        r.skip(0.5);
        r.read(&mut buf);
        assert!(power_db(&buf) < -55.0, "keyed off");
    }
}
