//! The burst finder test signal: bursts of carriers at offsets from the centre. A burst of width `W` of 4 kHz or more is
//! `W / 4000` carriers 4 kHz apart with quadratic phases (`k^2` radians, so that every short stretch holds the whole band),
//! sharing the power of the burst. The default
//! is a key fob (24 bursts of 3.2 ms every 0.25 s with 0.1 % jitter at +120 kHz), a sensor (9 bursts of 1.1 ms at -310
//! kHz), a wide burst (6 of 1 ms, 400 kHz wide, amplitude 3, every 0.9 s at +500 kHz) and a fast one (5 of 0.35 ms every
//! 1.1 s at -700 kHz). The noise reference is a carrier of power 1 in 25 kHz.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The default scene: start s, length s, offset Hz, width Hz, amplitude, count, every s, jitter.
pub const DEFAULT_BURSTS: &str = "0.1, 0.0032, 120000, 0, 1, 24, 0.25, 0.001; 0.7, 0.0011, -310000, 0, 1; \
1.1, 0.0011, -310000, 0, 1; 1.9, 0.0011, -310000, 0, 1; 2.0, 0.0011, -310000, 0, 1; 3.3, 0.0011, -310000, 0, 1; \
3.7, 0.0011, -310000, 0, 1; 4.4, 0.0011, -310000, 0, 1; 5.0, 0.0011, -310000, 0, 1; 5.4, 0.0011, -310000, 0, 1; \
0.5, 0.001, 500000, 400000, 3, 6, 0.9; 1.3, 0.00035, -700000, 0, 1, 5, 1.1";

/// The bursts of the test signal.
pub const GEN_BURSTS: Setting<field::Text> = Setting::new("gen_bursts", "Bursts", "", DEFAULT_BURSTS)
    .section(SECTION)
    .help(
        "Bursts separated by ';', each 'start s, length s, offset Hz, width Hz, amplitude' and for a train 'count, every \
         s, jitter' (a share of the interval).",
    );

/// The band that the noise of the impairments is stated in, Hz.
pub const NOISE_BAND_HZ: f64 = 25e3;

/// One burst.
#[derive(Clone, Debug, PartialEq)]
pub struct Burst {
    /// The start, s.
    pub start_s: f64,
    /// The length, s.
    pub length_s: f64,
    /// The centre from the centre of the capture, Hz.
    pub offset_hz: f64,
    /// The width, Hz: 0 for one carrier.
    pub width_hz: f64,
    /// The amplitude: the burst has the power of a carrier of this amplitude.
    pub amplitude: f64,
}

/// The bursts of a list (see [`GEN_BURSTS`]), the trains spread out with jitter from `seed`, sorted by start. `None` for a
/// value that is not a number.
pub fn parse_bursts(text: &str, seed: u64) -> Option<Vec<Burst>> {
    let mut rng = Rng::new(seed ^ 0x6275_7273_7473);
    let mut out = Vec::new();
    for entry in text.split(';').map(str::trim).filter(|e| !e.is_empty()) {
        let v: Vec<f64> = entry
            .split(',')
            .map(|x| x.trim().parse::<f64>().ok())
            .collect::<Option<_>>()?;
        if v.len() < 5 {
            return None;
        }
        let at = |k: usize, d: f64| v.get(k).copied().unwrap_or(d);
        let (count, every, jitter) = (at(5, 1.0).max(1.0) as usize, at(6, 0.0), at(7, 0.0));
        for k in 0..count {
            let j = jitter * every * (2.0 * rng.uniform() - 1.0);
            out.push(Burst {
                start_s: v[0] + k as f64 * every + j,
                length_s: v[1],
                offset_hz: v[2],
                width_hz: v[3],
                amplitude: v[4],
            });
        }
    }
    out.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));
    Some(out)
}

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct BurstGenerator {
    rate: f64,
    bursts: Vec<Burst>,
    n: u64,
}

impl BurstGenerator {
    /// The scene of the settings `v` at `rate_hz`; a list that cannot be read is no burst.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> BurstGenerator {
        BurstGenerator {
            rate: rate_hz,
            bursts: parse_bursts(v.get(&GEN_BURSTS), seed).unwrap_or_default(),
            n: 0,
        }
    }
}

impl Generator for BurstGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::default());
        let t0 = self.n as f64 / self.rate;
        let t1 = (self.n + out.len() as u64) as f64 / self.rate;
        for b in self
            .bursts
            .iter()
            .filter(|b| b.start_s < t1 && b.start_s + b.length_s > t0)
        {
            let tones = if b.width_hz >= 4000.0 {
                (b.width_hz / 4000.0).round() as usize
            } else {
                1
            };
            let amp = (b.amplitude / (tones as f64).sqrt()) as f32;
            let first = ((b.start_s * self.rate).ceil() as u64).max(self.n);
            let last = (((b.start_s + b.length_s) * self.rate).ceil() as u64)
                .min(self.n + out.len() as u64);
            for k in 0..tones {
                let f = b.offset_hz + 4000.0 * (k as f64 - 0.5 * (tones as f64 - 1.0));
                let phase0 = (k * k) as f64;
                let step = Complex32::from_polar(1.0, (TAU * f / self.rate) as f32);
                let mut z = Complex32::from_polar(
                    amp,
                    (phase0 + TAU * (f * (first as f64 / self.rate - b.start_s)).fract()) as f32,
                );
                for s in &mut out[(first - self.n) as usize..(last.max(first) - self.n) as usize] {
                    *s += z;
                    z *= step;
                }
            }
        }
        self.n += out.len() as u64;
        out.len()
    }
    /// A carrier of power 1 is the reference; the noise is stated in 25 kHz.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NOISE_BAND_HZ).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
