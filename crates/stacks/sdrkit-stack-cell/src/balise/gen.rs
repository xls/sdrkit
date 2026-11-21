//! The Eurobalise test signal: a passing train as the up-link sees it, bursts of repeated telegrams as continuous-phase FSK at
//! unit mean power.

use super::telegram::{encode_first, test_user_bits, Format};
use super::uplink::{BIT_RATE, DEV};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::SimError;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, DeclareSetting, GenConfig, Generator, Hz, Setting, SignalInfo,
};

sdrkit_core::choice_enum! {
    /// What the test signal sends.
    pub enum BaliseScene {
        /// Long telegrams.
        Long = ("0", "Long telegrams"),
        /// A short telegram.
        Short = ("1", "Short telegram"),
        /// The three test telegrams in turn.
        AllThree = ("2", "All three in turn"),
        /// Random bits.
        Random = ("3", "Random bits"),
    }
}
/// The tolerance of the up-link bit rate, percent (SUBSET-036 4.2).
pub const BIT_RATE_TOLERANCE_PCT: f64 = 2.5;
const BURST_MS: (f64, f64, f64) = (0.5, 100.0, 6.0);
const GAP_MS: (f64, f64, f64) = (1.0, 1000.0, 20.0);
const RANDOM_BITS: (i64, i64, i64) = (24, 4000, 400);
/// Silence before the first burst, seconds.
const LEAD_S: f64 = 2e-3;

/// The options of the generator.
#[derive(Clone, Debug)]
pub struct BaliseGenOptions {
    /// Scene 0 to 3.
    pub scene: u32,
    /// Burst length, seconds.
    pub burst_s: f64,
    /// Gap between bursts, seconds.
    pub gap_s: f64,
    /// The up-link centre relative to the radio centre, Hz.
    pub uplink_hz: f64,
    /// The bit rate's departure from 564.48 kbit/s, percent (within the tolerance of SUBSET-036 4.2).
    pub bit_rate_pct: f64,
    /// Random bits per burst in scene 3.
    pub random_bits: usize,
    /// The seed of the start bits and the random bits.
    pub seed: u64,
}

impl Default for BaliseGenOptions {
    fn default() -> Self {
        BaliseGenOptions {
            scene: 2,
            burst_s: BURST_MS.2 * 1e-3,
            gap_s: GAP_MS.2 * 1e-3,
            uplink_hz: -1e6,
            bit_rate_pct: 0.0,
            random_bits: RANDOM_BITS.2 as usize,
            seed: 1,
        }
    }
}

impl BaliseGenOptions {
    /// The options that the test signal settings of `cfg` give.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        BaliseGenOptions {
            seed: cfg.seed,
            scene: u32::from(s.get(&GEN_SCENE).index()),
            burst_s: s.get(&GEN_BURST_MS) * 1e-3,
            gap_s: s.get(&GEN_GAP_MS) * 1e-3,
            random_bits: s.get(&GEN_RANDOM_BITS) as usize,
            bit_rate_pct: s.get(&GEN_BIT_RATE_PCT),
            ..Default::default()
        }
    }
}

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";
/// What the test signal sends.
pub const GEN_SCENE: Setting<field::Choice<BaliseScene>> =
    Setting::choice("gen_scene", "Test signal scene", BaliseScene::AllThree)
        .section(TEST_SIGNAL)
        .advanced();
/// The length of a burst.
pub const GEN_BURST_MS: Setting<field::F64> =
    Setting::new("gen_burst_ms", "Test signal burst", "ms", BURST_MS.2)
        .range(BURST_MS.0, BURST_MS.1, 0.5)
        .section(TEST_SIGNAL)
        .advanced();
/// The gap between bursts.
pub const GEN_GAP_MS: Setting<field::F64> =
    Setting::new("gen_gap_ms", "Test signal gap", "ms", GAP_MS.2)
        .range(GAP_MS.0, GAP_MS.1, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// Random bits per burst of the random scene.
pub const GEN_RANDOM_BITS: Setting<field::I64> = Setting::new(
    "gen_random_bits",
    "Test signal random bits per burst",
    "",
    RANDOM_BITS.2,
)
.range(RANDOM_BITS.0 as f64, RANDOM_BITS.1 as f64, 1.0)
.section(TEST_SIGNAL)
.advanced();
/// The departure of the bit rate from 564.48 kbit/s.
pub const GEN_BIT_RATE_PCT: Setting<field::F64> =
    Setting::new("gen_bit_rate_pct", "Test signal bit rate error", "%", 0.0)
        .range(-BIT_RATE_TOLERANCE_PCT, BIT_RATE_TOLERANCE_PCT, 0.1)
        .section(TEST_SIGNAL)
        .advanced();

/// The test signal settings of the stack. The framework adds the standard `gen_*` settings (noise, offsets, impairments).
pub const SETTINGS: &[&dyn DeclareSetting] = &[
    &GEN_SCENE,
    &GEN_BURST_MS,
    &GEN_GAP_MS,
    &GEN_RANDOM_BITS,
    &GEN_BIT_RATE_PCT,
];

/// The telegrams of a scene (in the order sent).
pub fn scene_telegrams(scene: u32) -> Vec<Vec<u8>> {
    let t = |f: Format, g: u16, p: u8| {
        encode_first(f, &test_user_bits(f, g, p))
            .map(|x| x.0)
            .unwrap_or_default()
    };
    match scene {
        0 => vec![t(Format::Long, 1201, 0), t(Format::Long, 1201, 1)],
        1 => vec![t(Format::Short, 1202, 0)],
        2 => vec![
            t(Format::Long, 1201, 0),
            t(Format::Long, 1201, 1),
            t(Format::Short, 1202, 0),
        ],
        _ => vec![],
    }
}

/// The Eurobalise generator.
pub struct BaliseGenerator {
    fs: f64,
    opt: BaliseGenOptions,
    rb: f64,
    telegrams: Vec<Vec<u8>>,
    /// None for a generator that makes nothing (its settings were refused).
    modulator: Option<Cpm>,
    osc: Nco,
    rng: Rng,
    burst_no: usize,
    /// The burst being sent and the next sample of it.
    samples: Vec<Complex32>,
    at: usize,
    /// Silence left before the next burst.
    quiet: usize,
}

impl BaliseGenerator {
    /// A generator at `fs`. An error for a rate the modulator refuses.
    pub fn new(fs: f64, opt: BaliseGenOptions) -> Result<Self, SimError> {
        let rb = BIT_RATE * (1.0 + opt.bit_rate_pct / 100.0);
        let modulator = Cpm::cpfsk(2.0 * DEV / rb, fs / rb)?.with_held_edges();
        Ok(BaliseGenerator {
            fs,
            rb,
            telegrams: scene_telegrams(opt.scene),
            modulator: Some(modulator),
            osc: Nco::new(fs, opt.uplink_hz),
            rng: Rng::new(opt.seed),
            burst_no: 0,
            samples: Vec::new(),
            at: 0,
            quiet: (LEAD_S * fs).round() as usize,
            opt,
        })
    }

    /// The generator of the test signal settings in `cfg`.
    ///
    /// # Errors
    ///
    /// The [`SimError`] of a rate that the modulator cannot serve.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, SimError> {
        BaliseGenerator::new(cfg.rate_hz, BaliseGenOptions::from_config(cfg))
    }

    /// Modulates the next burst: the next telegram of the scene repeated from a random bit, or random bits.
    fn start_burst(&mut self) {
        let mut levels = Vec::new();
        if self.telegrams.is_empty() {
            for _ in 0..self.opt.random_bits {
                levels.push(if self.rng.bit() == 1 { 1.0 } else { -1.0 });
            }
        } else {
            let tel = &self.telegrams[self.burst_no % self.telegrams.len()];
            let n = tel.len().max(1);
            let start = (self.rng.next_u64() % n as u64) as usize;
            let len = (self.opt.burst_s * self.rb).ceil() as usize + 1;
            for i in 0..len {
                // A 1 is the higher frequency.
                let b = tel.get((start + i) % n).copied().unwrap_or(0);
                levels.push(if b == 1 { 1.0 } else { -1.0 });
            }
        }
        self.samples.clear();
        if let Some(m) = self.modulator.as_mut() {
            m.push(&levels, &mut self.samples);
            m.flush(&mut self.samples);
        }
        self.osc.mix_in_place(&mut self.samples);
        self.at = 0;
        self.burst_no += 1;
    }
}

impl Generator for BaliseGenerator {
    fn rate_hz(&self) -> Hz {
        self.fs
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        if self.modulator.is_none() {
            return 0;
        }
        let mut written = 0;
        while written < out.len() {
            if self.quiet > 0 {
                let n = self.quiet.min(out.len() - written);
                out[written..written + n].fill(Complex32::new(0.0, 0.0));
                self.quiet -= n;
                written += n;
            } else if self.at < self.samples.len() {
                let n = (self.samples.len() - self.at).min(out.len() - written);
                out[written..written + n].copy_from_slice(&self.samples[self.at..self.at + n]);
                self.at += n;
                written += n;
                if self.at == self.samples.len() {
                    self.quiet = (self.opt.gap_s * self.fs).round() as usize;
                }
            } else {
                self.start_burst();
            }
        }
        written
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // Carson's bandwidth of the up-link FSK; Eb/N0 is per bit of the up-link.
        Some(
            SignalInfo::new(2.0 * DEV + self.rb)
                .with_symbol_rate(self.rb)
                .with_bit_rate(self.rb),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bursts_have_unit_power_and_the_scene_timing() {
        let fs = 10e6;
        let mut g = BaliseGenerator::new(fs, BaliseGenOptions::default()).unwrap();
        let mut x = vec![Complex32::new(0.0, 0.0); 600_000];
        assert_eq!(g.generate(&mut x), x.len());
        // 2 ms of silence, then a burst of about 6 ms.
        assert!(x[..20_000].iter().all(|s| s.norm_sqr() == 0.0));
        let on: Vec<usize> = (0..x.len()).filter(|&i| x[i].norm_sqr() > 0.0).collect();
        assert_eq!(on[0], 20_000);
        assert!(on.iter().all(|&i| (x[i].norm_sqr() - 1.0).abs() < 1e-3));
        let first_end = on
            .windows(2)
            .find(|w| w[1] != w[0] + 1)
            .map(|w| w[0])
            .unwrap();
        let ms = (first_end + 1 - 20_000) as f64 / fs * 1e3;
        assert!((ms - 6.0).abs() < 0.01, "{ms} ms");
    }
}
