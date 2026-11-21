//! The LTR test signal: words with their check, NRZ at 300 bit/s smoothed at 150 Hz, FM, and a voice tone. Noise and the
//! receiver's impairments come from the framework's `gen_*` settings.

use super::code::{encode, LtrFields};
use num_complex::Complex32;
use sdrkit_algo_dsp::iir::OnePole;
use sdrkit_core::{GenConfig, Generator, SignalInfo};
use std::f64::consts::TAU;

/// The six words of the test case L1 (area, go-to, home, id, free).
pub fn six_words() -> Vec<LtrFields> {
    [
        (0, 3, 3, 17, 5),
        (0, 5, 1, 101, 7),
        (1, 12, 12, 254, 2),
        (0, 0, 8, 255, 9),
        (0, 7, 2, 33, 20),
        (0, 19, 19, 200, 1),
    ]
    .iter()
    .map(|&(area, goto, home, id, free)| LtrFields {
        area,
        goto,
        home,
        id,
        free,
    })
    .collect()
}

/// What the test signal is made of.
#[derive(Clone, Debug)]
pub struct LtrSignal {
    /// Sample rate.
    pub rate: f64,
    /// The channel's place in the capture, Hz from the centre.
    pub offset_hz: f64,
    /// Data deviation, Hz.
    pub deviation: f64,
    /// Voice tone frequency (0: none) and deviation.
    pub voice_hz: f64,
    /// Voice deviation, Hz.
    pub voice_dev: f64,
    /// Inverted data polarity.
    pub invert: bool,
    /// Baud scale (1 = 300 bit/s).
    pub baud_scale: f64,
    /// The words, sent in order …
    pub words: Vec<LtrFields>,
    /// … this many times.
    pub repeats: usize,
    /// Nothing (no carrier) after the words, s.
    pub tail_s: f64,
}

impl Default for LtrSignal {
    fn default() -> Self {
        LtrSignal {
            rate: 240e3,
            offset_hz: 0.0,
            deviation: 600.0,
            voice_hz: 1000.0,
            voice_dev: 1500.0,
            invert: false,
            baud_scale: 1.0,
            words: six_words(),
            repeats: 3,
            tail_s: 0.3,
        }
    }
}

impl LtrSignal {
    /// The 40-bit words in the order they are sent.
    pub fn sequence(&self) -> Vec<u64> {
        (0..self.repeats)
            .flat_map(|_| self.words.iter().map(encode))
            .collect()
    }
    /// The bit stream: 40 alternating lead-in bits, each word followed by 12 filler bits.
    pub fn bits_of(words: &[u64]) -> Vec<bool> {
        let mut b: Vec<bool> = (0..40).map(|i| i % 2 == 0).collect();
        for &w in words {
            for i in (0..40).rev() {
                b.push((w >> i) & 1 == 1);
            }
            b.extend((0..12).map(|i| i % 2 == 0));
        }
        b
    }
    /// The IQ samples of these bits, at unit amplitude while the carrier is on.
    pub fn iq_of(&self, bits: &[bool]) -> Vec<Complex32> {
        let spb = self.rate / (300.0 * self.baud_scale);
        let n = (bits.len() as f64 * spb) as usize;
        let mut lp = OnePole::new(self.rate, 150.0);
        let audio: Vec<f32> = (0..n)
            .map(|i| {
                let b = bits[((i as f64 / spb) as usize).min(bits.len() - 1)];
                let x = if b != self.invert { 1.0 } else { -1.0 };
                let y = lp.process_sample(x) as f64 * self.deviation;
                let v = if self.voice_hz > 0.0 {
                    self.voice_dev * (TAU * self.voice_hz * i as f64 / self.rate).sin()
                } else {
                    0.0
                };
                (y + v) as f32
            })
            .collect();
        let mut out = crate::fmtx::fm(&audio, self.rate, 1.0, self.offset_hz).unwrap_or_default();
        out.extend(std::iter::repeat_n(
            Complex32::new(0.0, 0.0),
            (self.tail_s * self.rate) as usize,
        ));
        out
    }
    /// The IQ samples.
    pub fn iq(&self) -> Vec<Complex32> {
        self.iq_of(&Self::bits_of(&self.sequence()))
    }
}

/// The demonstration generator: the six words, looping, the channel at 0 Hz.
pub struct LtrGenerator {
    rate: f64,
    cycle: Vec<Complex32>,
    pos: usize,
}

impl LtrGenerator {
    /// From the stack's generator config.
    pub fn from_config(cfg: &GenConfig) -> LtrGenerator {
        let s = LtrSignal {
            rate: cfg.rate_hz,
            tail_s: 0.0,
            ..Default::default()
        };
        LtrGenerator {
            rate: cfg.rate_hz,
            cycle: s.iq(),
            pos: 0,
        }
    }
}

impl Generator for LtrGenerator {
    /// The station is made at 0 Hz. The framework places it on the tuned frequency.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        if self.cycle.is_empty() {
            return 0;
        }
        for z in out.iter_mut() {
            *z = self.cycle[self.pos];
            self.pos = (self.pos + 1) % self.cycle.len();
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo {
            symbol_rate_hz: Some(300.0),
            bit_rate_hz: Some(300.0),
            ..SignalInfo::new(11e3)
        })
    }
}
