//! Test scenes of the spectrum watchers: lists of keyed carriers written as text, and the generator that sums them. A
//! scene adds no noise: the standard impairments of the test signal add it.

use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// One keyed carrier of a scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tone {
    /// The frequency at time 0 from the centre, Hz.
    pub offset_hz: f64,
    /// The amplitude (1 is a carrier of power 1).
    pub amplitude: f64,
    /// The keying period, s; 0 for a carrier that is not keyed.
    pub period_s: f64,
    /// The share of each period that the carrier is on.
    pub duty: f64,
    /// The time of the first key-down, s.
    pub phase_s: f64,
    /// The change of frequency per second, Hz/s.
    pub drift_hz_s: f64,
    /// The carrier exists from this time, s ...
    pub from_s: f64,
    /// ... until this time, s.
    pub to_s: f64,
}

impl Tone {
    /// A steady carrier.
    pub const fn steady(offset_hz: f64, amplitude: f64) -> Tone {
        Tone {
            offset_hz,
            amplitude,
            period_s: 0.0,
            duty: 1.0,
            phase_s: 0.0,
            drift_hz_s: 0.0,
            from_s: 0.0,
            to_s: f64::INFINITY,
        }
    }

    /// The carrier is on at `t`.
    pub fn on(&self, t: f64) -> bool {
        if t < self.from_s || t >= self.to_s {
            return false;
        }
        if self.period_s <= 0.0 || self.duty >= 1.0 {
            return true;
        }
        let u = (t - self.phase_s).rem_euclid(self.period_s);
        u < self.duty * self.period_s
    }
}

/// Reads a list of carriers: entries separated by `;`, each `offset Hz, amplitude, period s, duty, phase s, drift Hz/s,
/// from s, to s`, of which the values after the amplitude can be left out (a steady carrier from the start). An empty text
/// is no carrier. `None` for a value that is not a number.
pub fn parse_tones(text: &str) -> Option<Vec<Tone>> {
    let mut out = Vec::new();
    for entry in text.split(';').map(str::trim).filter(|e| !e.is_empty()) {
        let v: Vec<f64> = entry
            .split(',')
            .map(|x| x.trim().parse::<f64>().ok())
            .collect::<Option<_>>()?;
        if v.len() < 2 {
            return None;
        }
        let at = |k: usize, d: f64| v.get(k).copied().unwrap_or(d);
        out.push(Tone {
            offset_hz: v[0],
            amplitude: v[1],
            period_s: at(2, 0.0),
            duty: at(3, 1.0),
            phase_s: at(4, 0.0),
            drift_hz_s: at(5, 0.0),
            from_s: at(6, 0.0),
            to_s: at(7, f64::INFINITY),
        });
    }
    Some(out)
}

/// The sum of a list of keyed carriers at a rate. Each carrier keeps a continuous phase.
#[derive(Clone, Debug)]
pub struct ToneScene {
    rate: f64,
    tones: Vec<Tone>,
    phase: Vec<f64>,
    n: u64,
}

impl ToneScene {
    /// The carriers `tones` at `rate_hz`.
    pub fn new(rate_hz: f64, tones: Vec<Tone>) -> ToneScene {
        let phase = vec![0.0; tones.len()];
        ToneScene {
            rate: rate_hz,
            tones,
            phase,
            n: 0,
        }
    }

    /// The rate, Hz.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Adds the next `out.len()` samples of the scene to `out`.
    pub fn add_into(&mut self, out: &mut [Complex32]) {
        const BLOCK: usize = 512;
        let t0 = self.n as f64 / self.rate;
        for (b, chunk) in out.chunks_mut(BLOCK).enumerate() {
            let tb = t0 + (b * BLOCK) as f64 / self.rate;
            let len = chunk.len();
            for (tone, ph) in self.tones.iter().zip(self.phase.iter_mut()) {
                // The frequency at the middle of the block; the phase is carried exactly from block to block.
                let f = tone.offset_hz + tone.drift_hz_s * (tb + 0.5 * len as f64 / self.rate);
                let step = TAU * f / self.rate;
                let rot = Complex32::from_polar(1.0, step as f32);
                let mut z = Complex32::from_polar(tone.amplitude as f32, *ph as f32);
                let keyed_all = tone.on(tb) && tone.on(tb + (len - 1) as f64 / self.rate);
                for (i, s) in chunk.iter_mut().enumerate() {
                    if keyed_all || tone.on(tb + i as f64 / self.rate) {
                        *s += z;
                    }
                    z *= rot;
                }
                *ph = (*ph + step * len as f64).rem_euclid(TAU);
            }
        }
        self.n += out.len() as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_reads_and_a_keyed_carrier_is_on_for_its_duty() {
        let t = parse_tones("1000, 0.5, 2, 0.25, 0.1; -50, 1").unwrap();
        assert_eq!(t.len(), 2);
        assert!(t[0].on(0.2) && !t[0].on(0.7) && t[0].on(2.15));
        assert!(t[1].on(100.0));
        assert_eq!(parse_tones(""), Some(Vec::new()));
        assert_eq!(parse_tones("1, x"), None);
        let mut s = ToneScene::new(8000.0, vec![Tone::steady(1000.0, 2.0)]);
        let mut x = vec![Complex32::default(); 4000];
        s.add_into(&mut x);
        // An eighth of a turn per sample: sample 1001 is at 45 degrees.
        assert!(
            (x[1001] - Complex32::from_polar(2.0, std::f32::consts::FRAC_PI_4)).norm() < 1e-3,
            "{}",
            x[1001]
        );
    }
}
