//! The Fleet Broadcast test signal: differential BPSK at 1200 Bd with half-cosine edges over a quarter symbol either side of
//! each boundary, 16 channels of one bit a frame. Channel 0 carries a 31-bit m-sequence of the test signal's own; it is not
//! the pattern of a real broadcast, which is not public. Noise, offset and drift come from the standard `gen_*` settings.

use super::rx::{BAUD, COLUMNS};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo};

sdrkit_core::choice_enum! {
    /// What the channels of the test signal carry.
    pub enum GenScenario {
        /// Sync on channel 0, reversals on 1 and 4, ones on 2, zeros on 3, random bits on the rest.
        Mixed = ("0", "Sync, reversals, idle and random channels"),
        /// Sync on channel 0, ones on the odd channels, zeros on the even ones.
        Idle = ("1", "Sync, idle channels"),
        /// Noise only.
        Noise = ("2", "Noise only"),
        /// A plain carrier.
        Carrier = ("3", "A plain carrier"),
        /// Random bits on every channel, no sync.
        Random = ("4", "Random bits everywhere"),
    }
}

/// What the channels of the test signal carry.
pub const GEN_SCENARIO: Setting<field::Choice<GenScenario>> =
    Setting::choice("gen_scenario", "Scenario", GenScenario::Mixed)
        .section("Test signal")
        .advanced();
/// The carrier of the test signal from the dial.
pub const GEN_CARRIER_HZ: Setting<field::F64> =
    Setting::new("gen_carrier_hz", "Carrier from the dial", "Hz", 1500.0)
        .range(-4500.0, 4500.0, 1.0)
        .section("Test signal")
        .advanced();
/// Slow flat fading of the test signal.
pub const GEN_FADE_DB: Setting<field::F64> = Setting::new("gen_fade_db", "Fading depth", "dB", 0.0)
    .range(0.0, 30.0, 0.5)
    .section("Test signal")
    .advanced()
    .help("0 for none; else the depth of a flat fading at 0.5 Hz.");
/// Inverted bits.
pub const GEN_INVERT: Setting<field::Bool> = Setting::new("gen_invert", "Invert bits", "", false)
    .section("Test signal")
    .advanced();

/// The bits of frame `f` of a scenario, channel 0 first.
pub fn frame_bits(scenario: GenScenario, f: u64, rng: &mut Rng) -> [u8; COLUMNS] {
    let m = m_sequence_bit(f);
    let mut out = [0u8; COLUMNS];
    for (c, o) in out.iter_mut().enumerate() {
        let random = (rng.next_u32() & 1) as u8;
        *o = match (scenario, c) {
            (GenScenario::Random, _) => random,
            (_, 0) => m,
            (GenScenario::Idle, c) => (c % 2) as u8,
            (_, 1 | 4) => (f % 2) as u8,
            (_, 2) => 1,
            (_, 3) => 0,
            _ => random,
        };
    }
    out
}

/// Bit `f` of the 31-bit m-sequence of channel 0: a 5-bit register from 11111, output bit 0, new bit = bit 4 XOR bit 2
/// shifted in at the bottom.
pub fn m_sequence_bit(f: u64) -> u8 {
    let mut r = 0x1Fu8;
    for _ in 0..f % 31 {
        let new = ((r >> 4) ^ (r >> 2)) & 1;
        r = ((r << 1) | new) & 0x1F;
    }
    r & 1
}

/// The test signal generator.
pub struct FleetGenerator {
    rate: f64,
    scenario: GenScenario,
    invert: bool,
    fade_db: f64,
    rng: Rng,
    nco: Nco,
    /// Symbols per output sample.
    step: f64,
    /// The position in the current symbol, 0 to 1.
    frac: f64,
    /// The levels of the previous, the current and the next symbol.
    levels: [f32; 3],
    bits: Vec<u8>,
    sym: u64,
    t: f64,
}

impl FleetGenerator {
    /// The generator of a stack configuration.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let mut g = FleetGenerator::new(
            cfg.rate_hz,
            cfg.settings.get(&GEN_SCENARIO),
            cfg.settings.get(&GEN_CARRIER_HZ),
            super::DESCRIPTOR.tuning.tune_offset_hz,
        );
        g.invert = cfg.settings.get(&GEN_INVERT);
        g.fade_db = cfg.settings.get(&GEN_FADE_DB);
        g
    }

    /// A generator at `rate` of `scenario` with the carrier `carrier_hz` from the dial, the dial `tune_offset_hz` below the
    /// centre of the samples.
    pub fn new(rate: f64, scenario: GenScenario, carrier_hz: f64, tune_offset_hz: f64) -> Self {
        FleetGenerator {
            rate,
            scenario,
            invert: false,
            fade_db: 0.0,
            rng: Rng::new(0xF1EE7),
            nco: Nco::new(rate, carrier_hz - tune_offset_hz),
            step: BAUD / rate,
            frac: 0.0,
            levels: [1.0; 3],
            bits: Vec::new(),
            sym: 0,
            t: 0.0,
        }
    }

    /// The next differential bit (1 is a phase change).
    fn next_bit(&mut self) -> u8 {
        if self.bits.is_empty() {
            let f = self.sym / COLUMNS as u64;
            let mut b = frame_bits(self.scenario, f, &mut self.rng).to_vec();
            b.reverse();
            self.bits = b;
        }
        self.sym += 1;
        self.bits.pop().unwrap_or(0) ^ u8::from(self.invert)
    }

    /// The level at the current position: constant in the middle of a symbol, a half-cosine edge over a quarter symbol either
    /// side of each boundary.
    fn level(&self) -> f32 {
        let [p, c, n] = self.levels;
        let edge = |x: f64| (0.5 - 0.5 * (std::f64::consts::PI * x / 0.5).cos()) as f32;
        if self.frac < 0.25 {
            p + (c - p) * edge(self.frac + 0.25)
        } else if self.frac > 0.75 {
            c + (n - c) * edge(self.frac - 0.75)
        } else {
            c
        }
    }
}

impl Generator for FleetGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            let c = self.nco.next_sample();
            self.t += 1.0 / self.rate;
            let v = match self.scenario {
                GenScenario::Noise => 0.0,
                GenScenario::Carrier => 1.0,
                _ => {
                    self.frac += self.step;
                    if self.frac >= 1.0 {
                        self.frac -= 1.0;
                        let b = self.next_bit();
                        let [_, c, n] = self.levels;
                        self.levels = [c, n, if b == 1 { -n } else { n }];
                    }
                    self.level()
                }
            };
            let fade = if self.fade_db > 0.0 {
                let depth = 10f64.powf(-self.fade_db / 20.0);
                let m = 0.5 + 0.5 * (std::f64::consts::TAU * 0.5 * self.t).cos();
                (depth + (1.0 - depth) * m) as f32
            } else {
                1.0
            };
            *o = c * v * fade;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // Symbol power over the noise in 1.2 kHz.
        Some(SignalInfo::new(BAUD).with_symbol_rate(BAUD))
    }
}
