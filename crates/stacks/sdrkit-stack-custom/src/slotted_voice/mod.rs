//! `slotted_voice`: digital voice with a station beacon. Frame version 3 carries one voice stream in 160-symbol 16-QAM frames
//! at 8000 or 16000 symbols a second, with a sweep signature that spells the station and a beacon slice; frame version 4
//! carries four slots in 40 ms superframes.

pub mod beacon;
pub mod phy;
pub mod stack;
pub mod sweep;
pub mod v3;
pub mod v4;

pub use stack::SLOTTED_VOICE;

/// A speech-like test signal at 16 kHz: a vowel of harmonics on a pitch that glides, with syllables of 0.3 s, amplitude
/// about 8000. `seed` moves the pitch.
pub fn test_speech(samples: usize, seed: u64) -> Vec<i16> {
    let f0 = 110.0 + (seed % 7) as f64 * 9.0;
    (0..samples)
        .map(|i| {
            let t = i as f64 / 16_000.0;
            let pitch = f0 * (1.0 + 0.15 * (std::f64::consts::TAU * 0.7 * t).sin());
            let env = (std::f64::consts::PI * (t / 0.3).fract()).sin().powi(2);
            let mut v = 0.0;
            for h in 1..12 {
                let fh = pitch * h as f64;
                // A formant near 700 Hz and one near 1800 Hz.
                let a = (-((fh - 700.0) / 300.0).powi(2)).exp() + 0.5 * (-((fh - 1800.0) / 400.0).powi(2)).exp();
                v += a * (std::f64::consts::TAU * fh * t + h as f64).sin();
            }
            (8000.0 * env * v / 1.5).clamp(-32767.0, 32767.0) as i16
        })
        .collect()
}
