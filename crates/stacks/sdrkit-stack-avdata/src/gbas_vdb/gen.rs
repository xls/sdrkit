//! The GBAS VDB test signal: random D8PSK at 10 500 symbol/s with a half-sine envelope on each symbol, in bursts of 50 ms
//! that start at chosen slots of the 500 ms frame. Unit power while a burst is sent.

use super::{FRAME_S, SLOTS, SLOT_S, SYMBOL_RATE};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::{FRAC_PI_4, PI, TAU};

/// The slots of the test signal.
pub const GEN_SLOTS: Setting<field::Text> =
    Setting::new("gen_slots", "Test signal slots", "", "1 3 5")
        .help("The slots (0 to 7) of the 500 ms frame that hold a burst, separated by spaces.")
        .section("Test signal")
        .advanced();
/// The frames of the test signal.
pub const GEN_FRAMES: Setting<field::I64> = Setting::new("gen_frames", "Test signal frames", "", 0)
    .range(0.0, 10_000.0, 1.0)
    .help("Frames with bursts, then silence; 0 sends without end.")
    .section("Test signal")
    .advanced();

sdrkit_core::settings! {
    /// The settings of the `gbas_vdb` stack: the test signal settings.
    pub enum GbasSetting {
        /// The slots of the test signal.
        GenSlots = GEN_SLOTS,
        /// The frames of the test signal.
        GenFrames = GEN_FRAMES,
    }
}

/// The length of a burst of the test signal, s.
pub const BURST_S: f64 = 0.05;

/// The slots of the text of [`GEN_SLOTS`]: numbers 0 to 7, other words ignored.
pub fn slots_of(text: &str) -> [bool; SLOTS] {
    let mut s = [false; SLOTS];
    for w in text.split(|c: char| c.is_whitespace() || c == ',') {
        if let Ok(n) = w.parse::<usize>() {
            if n < SLOTS {
                s[n] = true;
            }
        }
    }
    s
}

/// The generator.
pub struct GbasGenerator {
    rate: f64,
    channel_hz: f64,
    slots: [bool; SLOTS],
    frames: u64,
    rng: Rng,
    n: u64,
    symbol: i64,
    phase: f64,
}

impl GbasGenerator {
    /// A generator at `rate` with the bursts in `slots` for `frames` frames (0: without end), the channel `channel_hz` from
    /// the centre.
    pub fn new(rate: f64, slots: [bool; SLOTS], frames: u64, channel_hz: f64, seed: u64) -> Self {
        GbasGenerator {
            rate,
            channel_hz,
            slots,
            frames,
            rng: Rng::new(seed ^ 0x4742_4153),
            n: 0,
            symbol: -1,
            phase: 0.0,
        }
    }

    /// The generator of the settings of `cfg`.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] below 62.5 ksps.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, ConfigError> {
        if cfg.rate_hz < super::MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: super::MIN_RATE_HZ,
                got: cfg.rate_hz,
            });
        }
        let frames = u64::try_from(cfg.settings.get(&GEN_FRAMES)).unwrap_or(0);
        Ok(GbasGenerator::new(
            cfg.rate_hz,
            slots_of(cfg.settings.get(&GEN_SLOTS)),
            frames,
            0.0,
            cfg.seed,
        ))
    }
}

impl Generator for GbasGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            let frame = (t / FRAME_S) as u64;
            let in_frame = t - frame as f64 * FRAME_S;
            let slot = ((in_frame / SLOT_S) as usize).min(SLOTS - 1);
            let in_slot = in_frame - slot as f64 * SLOT_S;
            let on =
                self.slots[slot] && in_slot < BURST_S && (self.frames == 0 || frame < self.frames);
            if !on {
                *z = Complex32::default();
                continue;
            }
            let u = in_slot * SYMBOL_RATE;
            let k = u as i64 + (frame * SLOTS as u64 + slot as u64) as i64 * 1_000_000;
            if k != self.symbol {
                self.symbol = k;
                self.phase += FRAC_PI_4 * (self.rng.next_u64() % 8) as f64;
            }
            let env = (2.0f64).sqrt() * (PI * u.fract()).sin();
            let ph = self.phase + TAU * self.channel_hz * t;
            *z = Complex32::from_polar(env as f32, ph.rem_euclid(TAU) as f32);
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(25e3).with_symbol_rate(SYMBOL_RATE))
    }
}
