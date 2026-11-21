//! The MPT 1327 test signal: codewords, the transmission (preamble, SYNC, codewords between CCSCs), FFSK audio at any rate and
//! FM onto a carrier. Noise and the receiver's impairments come from the framework's `gen_*` settings.

use super::code::{ccsc, SYNC};
use super::msg::build::eight_messages;
use num_complex::Complex32;
use sdrkit_algo_sim::modulate::Afsk;
use sdrkit_core::{GenConfig, Generator, SignalInfo};

/// The bits of a transmission: 16 bit reversals ending in 0, the sync word, the first codeword, then a CCSC and the codeword for every
/// further one, and a final CCSC. `sync` is the sync word of the CCSCs (and of the start-up sequence).
pub fn transmission_bits(codewords: &[u64], sys: u32, sync: u16) -> Vec<bool> {
    let mut bits = Vec::new();
    let push = |bits: &mut Vec<bool>, v: u64, n: u32| {
        for i in (0..n).rev() {
            bits.push((v >> i) & 1 == 1);
        }
    };
    push(&mut bits, 0xAAAA, 16);
    push(&mut bits, sync as u64, 16);
    let c = ccsc(sys, sync).expect("a CCSC exists for every system");
    for (i, &cw) in codewords.iter().enumerate() {
        if i > 0 {
            push(&mut bits, c, 64);
        }
        push(&mut bits, cw, 64);
    }
    push(&mut bits, c, 64);
    bits
}

/// FFSK audio (MPT 1317): bit 1 is 1200 Hz and bit 0 is 1800 Hz (`swap` exchanges them), continuous phase, amplitude `amp`,
/// at 1200 × `rate_scale` bit/s. Made by `sdrkit_algo_sim::modulate::Afsk`.
pub fn ffsk_audio(bits: &[bool], rate: f64, swap: bool, rate_scale: f64, amp: f32) -> Vec<f32> {
    let (one, zero) = if swap {
        (1800.0, 1200.0)
    } else {
        (1200.0, 1800.0)
    };
    let mut out = Vec::new();
    if let Ok(a) = Afsk::new(
        rate,
        1200.0 * rate_scale,
        one * rate_scale,
        zero * rate_scale,
    ) {
        let mut a = a.with_amplitude(amp);
        let b: Vec<u8> = bits.iter().map(|&b| b as u8).collect();
        a.push(&b, &mut out);
        a.flush(&mut out);
    }
    out
}

/// What the test signal is made of.
#[derive(Clone, Debug)]
pub struct MptSignal {
    /// Sample rate.
    pub rate: f64,
    /// The channel's place in the capture, Hz from the centre.
    pub offset_hz: f64,
    /// Deviation per audio unit, Hz.
    pub dev_per_unit: f64,
    /// FFSK amplitude (audio units).
    pub audio_amp: f32,
    /// Swap the tones.
    pub swap: bool,
    /// Bit-rate scale (1 = 1200 bit/s).
    pub rate_scale: f64,
    /// Carrier alone before and after, s.
    pub pad_s: f64,
    /// System identity.
    pub system: u32,
    /// The sync word of the CCSCs.
    pub sync: u16,
    /// The address codewords.
    pub codewords: Vec<u64>,
}

impl Default for MptSignal {
    fn default() -> Self {
        MptSignal {
            rate: 240e3,
            offset_hz: 0.0,
            dev_per_unit: 3000.0,
            audio_amp: 0.5,
            swap: false,
            rate_scale: 1.0,
            pad_s: 0.5,
            system: 20001,
            sync: SYNC,
            codewords: eight_messages(),
        }
    }
}

impl MptSignal {
    /// The audio (FFSK with silence around it) at `rate`.
    pub fn audio(&self, rate: f64) -> Vec<f32> {
        let bits = transmission_bits(&self.codewords, self.system, self.sync);
        let pad = (self.pad_s * rate) as usize;
        let mut a = vec![0.0f32; pad];
        a.extend(ffsk_audio(
            &bits,
            rate,
            self.swap,
            self.rate_scale,
            self.audio_amp,
        ));
        a.extend(std::iter::repeat_n(0.0, pad));
        a
    }
    /// The IQ samples: the audio frequency modulated at unit amplitude (empty when the carrier does not fit the rate).
    pub fn iq(&self) -> Vec<Complex32> {
        crate::fmtx::fm(
            &self.audio(self.rate),
            self.rate,
            self.dev_per_unit,
            self.offset_hz,
        )
        .unwrap_or_default()
    }
}

/// The demonstration generator: system 20001 and the eight test messages, looping, the channel at 0 Hz.
pub struct MptGenerator {
    rate: f64,
    cycle: Vec<Complex32>,
    pos: usize,
}

impl MptGenerator {
    /// From the stack's generator config.
    pub fn from_config(cfg: &GenConfig) -> MptGenerator {
        let s = MptSignal {
            rate: cfg.rate_hz,
            ..Default::default()
        };
        MptGenerator {
            rate: cfg.rate_hz,
            cycle: s.iq(),
            pos: 0,
        }
    }
}

impl Generator for MptGenerator {
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
            symbol_rate_hz: Some(1200.0),
            bit_rate_hz: Some(1200.0),
            ..SignalInfo::new(9e3)
        })
    }
}
