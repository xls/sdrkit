//! The generator's audio content: tones and the tune, periodic over the 12 s loop.

/// Seconds in the loop.
pub const LOOP_S: f64 = 12.0;

/// What a service plays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Programme {
    /// A tone on each channel (Hz), amplitude 0.5.
    Tones(f64, f64),
    /// The tune: left the tune, right an octave higher.
    Melody,
    /// Silence.
    Silence,
}

const C4: f64 = 261.63;
const D4: f64 = 293.66;
const E4: f64 = 329.63;
const F4: f64 = 349.23;
const G4: f64 = 392.00;

/// The tune as (frequency, quarter notes), 32 quarter notes of 0.375 s.
pub fn tune() -> Vec<(f64, f64)> {
    let phrase = [E4, E4, F4, G4, G4, F4, E4, D4, C4, C4, D4, E4];
    let mut v: Vec<(f64, f64)> = phrase.iter().map(|&f| (f, 1.0)).collect();
    v.extend([(E4, 1.5), (D4, 0.5), (D4, 2.0)]);
    v.extend(phrase.iter().map(|&f| (f, 1.0)));
    v.extend([(D4, 1.5), (C4, 0.5), (C4, 2.0)]);
    v
}

/// The notes of the tune (for checks).
pub const NOTES: [f64; 5] = [C4, D4, E4, F4, G4];

/// Renders one channel of the loop at `rate`.
pub fn render(p: Programme, right: bool, rate: u32) -> Vec<f32> {
    let n = (LOOP_S * rate as f64).round() as usize;
    let amp = 0.5;
    match p {
        Programme::Silence => vec![0.0; n],
        Programme::Tones(l, r) => {
            let f = if right { r } else { l };
            (0..n)
                .map(|i| (amp * (std::f64::consts::TAU * f * i as f64 / rate as f64).sin()) as f32)
                .collect()
        }
        Programme::Melody => {
            let mut out = vec![0f32; n];
            let q = 0.375;
            let mut t0 = 0.0;
            for (f, dur) in tune() {
                let f = if right { 2.0 * f } else { f };
                let d = dur * q;
                let a = (t0 * rate as f64).round() as usize;
                let b = (((t0 + d) * rate as f64).round() as usize).min(n);
                for (k, o) in out[a..b].iter_mut().enumerate() {
                    let t = k as f64 / rate as f64;
                    let attack = (t / 0.012).min(1.0);
                    let release = ((d - t) / 0.080).clamp(0.0, 1.0);
                    let env = (0.7 + 0.3 * (-t / 0.12).exp()) * attack * release;
                    let w = std::f64::consts::TAU * f * t;
                    let s = w.sin()
                        + 0.45 * (2.0 * w).sin()
                        + 0.2 * (3.0 * w).sin()
                        + 0.08 * (4.0 * w).sin();
                    *o = (amp * env * s / 1.73) as f32;
                }
                t0 += d;
            }
            out
        }
    }
}
