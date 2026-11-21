//! The wave-buoy test signal generator: the clean 2FSK HXV vectors of a fixed test sea state, 4FSK carrying a
//! synthetic test frame of sdrkit's own (not the HVA format), a plain carrier, or nothing. Noise, clock error and frequency
//! errors come from the standard `gen_*` keys.

use super::hxv::{self, position_words};
use super::rx::BAUD;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;
use sdrkit_util_bits::crc::Trailer;
use std::f64::consts::TAU;

sdrkit_core::choice_enum! {
    /// What the generator sends.
    pub enum BuoySignal {
        /// Continuous-phase 2FSK, 129 Hz between the tones, carrying the HXV test vectors.
        Fsk2 = ("2fsk", "2FSK"),
        /// Continuous-phase 4FSK, 78 Hz between the tones, carrying the test frames.
        Fsk4 = ("4fsk", "4FSK"),
        /// Nothing (the noise of the `gen_*` settings alone).
        Nothing = ("nothing", "Nothing"),
        /// A plain carrier at the offset.
        Carrier = ("carrier", "Carrier"),
    }
}

/// The test system words (block number -> 12-bit value).
pub fn test_system_words() -> [u16; 16] {
    let (la, lb) = position_words(54.5, 90.0);
    let (oa, ob) = position_words(-5.2, 180.0);
    [
        0x0B2,
        340,
        56,
        1000,
        600,
        470,
        (26 << 4) | 1,
        160,
        0,
        0x800 | 80,
        la,
        lb,
        oa,
        ob,
        64,
        (6 << 8) | 225,
    ]
}

/// The four words of test spectrum entry `n`.
pub fn test_spectrum_words(n: u32) -> [u16; 4] {
    [
        (((n & 3) << 14) | (n << 8) | ((40 + 3 * n) & 0xFF)) as u16,
        ((((n + 1) & 3) << 14) | (((n + 2) & 3) << 12) | ((100 + 40 * n) & 0xFFF)) as u16,
        (((30 + n) << 8) | ((150 + n) & 0xFF)) as u16,
        ((((110 + n) & 0xFF) << 8) | ((100 + n) & 0xFF)) as u16,
    ]
}

/// The test displacements at time t: heave, north, west (cm).
pub fn test_waves(t: f64) -> (i32, i32, i32) {
    let h = 60.0 * (TAU * t / 7.2).sin() + 20.0 * (TAU * t / 3.1).sin();
    let n = 30.0 * (TAU * t / 7.2 + 1.0).sin();
    let w = 25.0 * (TAU * t / 7.2).cos();
    (h.round() as i32, n.round() as i32, w.round() as i32)
}

/// Test vector `i` (64 bits, the first sent the most significant).
pub fn test_vector(i: u64, wrsg: bool) -> u64 {
    let b = ((i / 18) % 16) as u32;
    let pos = (i % 18) as u32;
    let cyclic = match pos {
        0 => hxv::SYNC,
        1 => ((b as u16) << 12) | test_system_words()[b as usize],
        p => {
            let g = (p - 2) / 4;
            test_spectrum_words(4 * b + g)[((p - 2) % 4) as usize]
        }
    };
    let t = i as f64 / 1.28;
    let (h, n, w) = test_waves(t);
    if wrsg {
        let (h2, _, _) = test_waves(t + 0.390625);
        hxv::build(cyclic, h, h2, 0xAAA)
    } else {
        hxv::build(cyclic, h, n, hxv::sm(w))
    }
}

/// The 4FSK test frame number `k`: D391, 12 pseudo-random bytes, CRC-16/IBM-3740 over those bytes, big endian.
pub fn test_frame(k: u64) -> [u8; 16] {
    let mut rng = Rng::new(0xD391 ^ k);
    let mut body: Vec<u8> = (0..12).map(|_| rng.below(256) as u8).collect();
    CRC16_IBM_3740
        .append(&mut body, Trailer::Big)
        .expect("a 16-bit CRC fits");
    let mut f = [0u8; 16];
    f[..2].copy_from_slice(&[0xD3, 0x91]);
    f[2..].copy_from_slice(&body);
    f
}

/// The generator's options.
#[derive(Clone, Debug)]
pub struct BuoyGenOptions {
    /// Samples per second.
    pub rate: f64,
    /// What is sent.
    pub signal: BuoySignal,
    /// The signal centre from the radio centre, Hz.
    pub offset_hz: f64,
    /// The tones the other way round (a receiver set for the other sideband).
    pub inverted: bool,
    /// WR-SG vectors: two vertical samples and the dummy AAA in west.
    pub wrsg: bool,
    /// The seed of the random lead bits.
    pub seed: u64,
    /// Random bits before the first vector (a stream starting anywhere).
    pub lead_bits: usize,
}

impl Default for BuoyGenOptions {
    fn default() -> Self {
        BuoyGenOptions {
            rate: 250_000.0,
            signal: BuoySignal::Fsk2,
            offset_hz: 1500.0,
            inverted: false,
            wrsg: false,
            seed: 1,
            lead_bits: 0,
        }
    }
}

/// The generator: a constant envelope of unit power, made by the CPFSK modulator of `sdrkit_algo_sim` and moved to the offset.
pub struct BuoyGenerator {
    o: BuoyGenOptions,
    cpm: Cpm,
    nco: Nco,
    lead: Vec<u8>,
    k: u64,
    buf: Vec<Complex32>,
    pos: usize,
}

impl BuoyGenerator {
    /// The generator of `o`.
    pub fn new(o: BuoyGenOptions) -> Self {
        let deviation = if o.signal == BuoySignal::Fsk4 {
            super::rx::STEP_4FSK / 2.0
        } else {
            super::rx::STEP_2FSK / 2.0
        };
        let cpm = Cpm::with_deviation(Shape::Rect, 1.0, deviation, BAUD, o.rate)
            .expect("the buoy rates make a valid modulator");
        let mut rng = Rng::new(o.seed);
        let lead = (0..o.lead_bits).map(|_| rng.below(2) as u8).collect();
        BuoyGenerator {
            cpm,
            nco: Nco::new(o.rate, o.offset_hz),
            lead,
            k: 0,
            buf: Vec::new(),
            pos: 0,
            o,
        }
    }
    /// From the stack's generator settings: `gen_signal`, `gen_inverted` and `gen_wrsg`.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        BuoyGenerator::new(BuoyGenOptions {
            rate: cfg.rate_hz,
            signal: s.get(&super::GEN_SIGNAL),
            inverted: s.get(&super::GEN_INVERTED_TONES),
            wrsg: s.get(&super::GEN_WRSG),
            seed: cfg.seed,
            ..Default::default()
        })
    }

    /// The level of symbol `k`: the tone in units of half the tone step about the centre.
    fn level(&self, k: u64) -> f32 {
        let l = match self.o.signal {
            BuoySignal::Fsk2 => {
                let lead = self.lead.len() as u64;
                let bit = if k < lead {
                    self.lead[k as usize]
                } else {
                    let j = k - lead;
                    ((test_vector(j / 64, self.o.wrsg) >> (63 - j % 64)) & 1) as u8
                };
                2.0 * bit as f32 - 1.0
            }
            BuoySignal::Fsk4 => {
                let frame = test_frame(k / 64);
                let s = (k % 64) as usize;
                let sym = (frame[s / 4] >> (6 - 2 * (s % 4))) & 3;
                2.0 * sym as f32 - 3.0
            }
            _ => 0.0,
        };
        if self.o.inverted {
            -l
        } else {
            l
        }
    }
}

impl Generator for BuoyGenerator {
    fn rate_hz(&self) -> f64 {
        self.o.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The receiver reports the SNR in 100 Hz.
        Some(SignalInfo {
            symbol_rate_hz: Some(BAUD),
            ..SignalInfo::new(100.0)
        })
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        if self.o.signal == BuoySignal::Nothing {
            out.fill(Complex32::new(0.0, 0.0));
            return out.len();
        }
        let mut i = 0;
        while i < out.len() {
            if self.pos == self.buf.len() {
                self.buf.clear();
                self.pos = 0;
                let level = self.level(self.k);
                self.k += 1;
                self.cpm.push(&[level], &mut self.buf);
                continue;
            }
            let n = (self.buf.len() - self.pos).min(out.len() - i);
            out[i..i + n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            i += n;
        }
        self.nco.mix_in_place(out);
        out.len()
    }
}
