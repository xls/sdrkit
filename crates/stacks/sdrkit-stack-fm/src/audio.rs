//! The audio stages after the stereo matrix: de-emphasis, the tone and bandwidth filters, and the squelch. Each works in place
//! on interleaved frames of one or two channels.

use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs, Butterworth4};
use sdrkit_algo_modem::squelch::Gate;

/// The audio output rate.
pub const AUDIO_RATE: f64 = 48_000.0;
/// The most channels an audio stage takes.
pub const MAX_CHANNELS: usize = 2;
const SHELF_Q: f64 = std::f64::consts::FRAC_1_SQRT_2; // RBJ shelf slope 1

/// Runs `f(channel, sample)` on every sample of the interleaved frames `x` of `channels` channels.
fn each(x: &mut [f32], channels: usize, mut f: impl FnMut(usize, &mut f32)) {
    for frame in x.chunks_exact_mut(channels.max(1)) {
        for (c, s) in frame.iter_mut().enumerate() {
            f(c, s);
        }
    }
}

/// De-emphasis on each channel: `1 / (1 + s tau)` by the bilinear transform (`sdrkit_algo_dsp::iir::Deemphasis::bilinear`). A
/// time constant of 0 passes the audio unchanged.
/// Standard: ITU-R BS.450 (pre-emphasis of 50 or 75 µs).
#[derive(Clone, Debug)]
pub struct Deemphasis {
    ch: Option<[sdrkit_algo_dsp::iir::Deemphasis; MAX_CHANNELS]>,
    rate: f64,
    channels: usize,
}

impl Deemphasis {
    /// De-emphasis with `tau_s` (0 is off) at `rate`, on frames of `channels` channels (1 or 2).
    pub fn new(tau_s: f64, rate: f64, channels: usize) -> Self {
        let ch = sdrkit_algo_dsp::iir::Deemphasis::bilinear(tau_s, rate)
            .ok()
            .map(|d| [d.clone(), d]);
        Deemphasis {
            ch,
            rate,
            channels: channels.clamp(1, MAX_CHANNELS),
        }
    }

    /// Changes the time constant. The state starts again.
    pub fn set_tau(&mut self, tau_s: f64) {
        *self = Self::new(tau_s, self.rate, self.channels);
    }

    /// Filters interleaved frames in place.
    pub fn process(&mut self, x: &mut [f32]) {
        let Some(ch) = &mut self.ch else { return };
        each(x, self.channels, |c, s| *s = ch[c].process_sample(*s));
    }
}

/// Tone (bass and treble shelves, Warm) and the audio bandwidth low-pass, at 48 kHz, in place.
///
/// Bass and treble are -6 to +6 dB (0 is exactly the identity). Warm takes 3 dB more off the treble. A bandwidth of 0, or of
/// 15 kHz and more, is full. Another bandwidth is a 4th-order Butterworth low-pass at 2 kHz or more.
#[derive(Clone, Debug)]
pub struct AudioFilters {
    channels: usize,
    bass: Option<[Biquad; MAX_CHANNELS]>,
    treble: Option<[Biquad; MAX_CHANNELS]>,
    bandwidth: Option<[Butterworth4; MAX_CHANNELS]>,
}

impl Default for AudioFilters {
    fn default() -> Self {
        Self::new(1)
    }
}

impl AudioFilters {
    /// Filters off, for frames of `channels` channels (1 or 2).
    pub fn new(channels: usize) -> Self {
        AudioFilters {
            channels: channels.clamp(1, MAX_CHANNELS),
            bass: None,
            treble: None,
            bandwidth: None,
        }
    }

    /// Bass and treble shelves. `warm` takes 3 dB more off the treble.
    pub fn set_tone(&mut self, bass_db: f64, treble_db: f64, warm: bool) {
        let bass = bass_db.clamp(-6.0, 6.0);
        let treble = treble_db.clamp(-6.0, 6.0) - if warm { 3.0 } else { 0.0 };
        self.bass = (bass != 0.0).then(|| {
            let c = BiquadCoeffs::low_shelf(AUDIO_RATE, 200.0, SHELF_Q, bass);
            [Biquad::new(c), Biquad::new(c)]
        });
        self.treble = (treble != 0.0).then(|| {
            let c = BiquadCoeffs::high_shelf(AUDIO_RATE, 6000.0, SHELF_Q, treble);
            [Biquad::new(c), Biquad::new(c)]
        });
    }

    /// The audio bandwidth in Hz.
    pub fn set_bandwidth(&mut self, hz: f64) {
        self.bandwidth = (hz > 0.0 && hz < 15e3).then(|| {
            let f = hz.max(2e3);
            let lp = Butterworth4::lowpass(AUDIO_RATE, f);
            [lp.clone(), lp]
        });
    }

    /// Filters interleaved frames in place.
    pub fn process(&mut self, x: &mut [f32]) {
        let (bass, treble, bandwidth) = (&mut self.bass, &mut self.treble, &mut self.bandwidth);
        each(x, self.channels, |c, s| {
            if let Some(b) = bass {
                *s = b[c].process_sample(*s);
            }
            if let Some(t) = treble {
                *s = t[c].process_sample(*s);
            }
            if let Some(w) = bandwidth {
                *s = w[c].process_sample(*s);
            }
        });
    }
}

/// The squelch: a gain that ramps over 10 ms to 0 while the SNR measure is below the threshold, at 48 kHz in place. The
/// decision is a `sdrkit_algo_modem::squelch::Gate`: it opens at the threshold and closes 2 dB below it. A threshold of 0
/// turns it off.
#[derive(Clone, Debug)]
pub struct Squelch {
    gate: Option<Gate>,
    gain: f32,
}

impl Default for Squelch {
    fn default() -> Self {
        Squelch {
            gate: None,
            gain: 1.0,
        }
    }
}

impl Squelch {
    /// A squelch at `threshold_db`. With a threshold it starts closed, because no SNR is known yet.
    pub fn new(threshold_db: f32) -> Self {
        let mut s = Squelch::default();
        s.set_threshold(threshold_db);
        if s.gate.is_some() {
            s.gain = 0.0;
        }
        s
    }

    /// The threshold (0 is off). The gate keeps its state.
    pub fn set_threshold(&mut self, threshold_db: f32) {
        if threshold_db.is_nan() || threshold_db <= 0.0 {
            self.gate = None;
            return;
        }
        match &mut self.gate {
            // The thresholds are finite and in order, so the gate takes them.
            Some(g) => {
                let _ = g.set_thresholds(threshold_db, threshold_db - 2.0);
            }
            None => self.gate = Gate::new(threshold_db, threshold_db - 2.0, 1, 0).ok(),
        }
    }

    /// The latest SNR measure.
    pub fn update(&mut self, snr_db: f32) {
        if let Some(g) = &mut self.gate {
            g.update(Some(snr_db));
        }
    }

    /// Whether the squelch is closed.
    pub fn squelched(&self) -> bool {
        self.gate.as_ref().is_some_and(|g| !g.is_open())
    }

    /// Applies the gain in place to interleaved frames of `channels` channels.
    pub fn process(&mut self, x: &mut [f32], channels: usize) {
        let step = (1.0 / (0.010 * AUDIO_RATE)) as f32;
        let target = if self.squelched() { 0.0 } else { 1.0 };
        for frame in x.chunks_exact_mut(channels.max(1)) {
            if self.gain != target {
                self.gain = if self.gain < target {
                    (self.gain + step).min(1.0)
                } else {
                    (self.gain - step).max(0.0)
                };
            }
            for s in frame {
                *s *= self.gain;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_dev_test::measure::tone_amplitude;
    use std::f64::consts::TAU;

    fn tone(f: f64, rate: f64, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (0.5 * (TAU * f * i as f64 / rate).sin()) as f32)
            .collect()
    }

    #[test]
    fn components_do_what_they_say() {
        // Bandwidth 5 kHz: 10 kHz down more than 20 dB.
        let mut f = AudioFilters::new(1);
        f.set_bandwidth(5000.0);
        let mut a = tone(10_000.0, AUDIO_RATE, 48_000);
        f.process(&mut a);
        assert!(tone_amplitude(&a[4800..], AUDIO_RATE, 10_000.0) < 0.05);
        // Zero tone is the identity.
        let mut f = AudioFilters::new(1);
        f.set_tone(0.0, 0.0, false);
        let mut a = tone(300.0, AUDIO_RATE, 1000);
        let b = a.clone();
        f.process(&mut a);
        assert_eq!(a, b);
        // De-emphasis: 10 kHz about 10 dB down at 50 us.
        let mut d = Deemphasis::new(50e-6, 50e3, 1);
        let mut a = tone(10_000.0, 50e3, 50_000);
        d.process(&mut a);
        let db = 20.0 * (0.5 / tone_amplitude(&a[5000..], 50e3, 10_000.0)).log10();
        assert!((9.0..12.5).contains(&db), "{db}");
        // Squelch: closed until an SNR above the threshold, then ramps up in 10 ms.
        let mut s = Squelch::new(15.0);
        let mut a = tone(1000.0, AUDIO_RATE, 960);
        s.process(&mut a, 1);
        assert!(a.iter().all(|&v| v == 0.0) && s.squelched());
        s.update(16.0);
        let mut a = vec![1.0f32; 2 * 960];
        s.process(&mut a, 2);
        assert!(!s.squelched() && a[2 * 478] < 1.0 && a[2 * 480] == 1.0);
        s.update(13.5);
        assert!(!s.squelched(), "within the 2 dB hysteresis");
        s.update(12.5);
        assert!(s.squelched());
    }
}
