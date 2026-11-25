//! The test signal of the pulse analyser of `ism`: groups of frames of one line code key a carrier of amplitude 1 at 0 Hz, one
//! group each second or more. The noise of the framework is stated in 100 kHz.
//!
//! | Pattern | One frame |
//! |---|---|
//! | PWM (PT2262 style), unit `T` | a 0 is `T` on and `3T` off, a 1 is `3T` on and `T` off; then a sync pulse of `T` on and `31T` off |
//! | PPM | a pulse, then for each bit a gap of `gap0` (0) or `gap1` (1) and a pulse; then 20 ms off |
//! | Manchester, half-bit `T` | a 1 is on then off, a 0 off then on (a leading off half-bit is not sent); then 20 ms off |
//!
//! Every run (mark or space) is stretched by its own uniform share in `[-jitter, jitter]`. `gen_frame_gap_s` adds a gap after each
//! frame.

use super::gen::{train_samples, TRAIN_RATE_HZ};
use crate::family::CleanScene;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::publish::field;
use sdrkit_core::{Setting, SignalInfo, Values};
use sdrkit_proto_ookdev::{encode, Modulation, Pulse, Train};
use sdrkit_util_bits::line::ManchesterConv;

sdrkit_core::choice_enum! {
    /// The line code of the test signal of the pulse analyser.
    pub enum GenPattern {
        /// Pulse width, PT2262 style.
        Pwm = ("pwm", "PWM (PT2262 style)"),
        /// Pulse position.
        Ppm = ("ppm", "PPM"),
        /// Manchester.
        Manchester = ("manchester", "Manchester"),
    }
}

/// The line code of the test signal.
pub const GEN_PATTERN: Setting<field::Choice<GenPattern>> = Setting::choice(
    "gen_pattern",
    "Pulse analyser test pattern",
    GenPattern::Pwm,
)
.section("Test signal")
.advanced();
/// The bits of each frame.
pub const GEN_BITS: Setting<field::Text> = Setting::new("gen_bits", "Pulse analyser test bits", "", "")
    .section("Test signal")
    .advanced()
    .help("0 and 1; other characters are left out. Empty: 101100101001110010100110 (PWM), 1100101001101011 (PPM), 10110010111000101101 (Manchester).");
/// The unit of PWM and the half-bit of Manchester.
pub const GEN_T_US: Setting<field::F64> =
    Setting::new("gen_t_us", "Pulse analyser test unit", "us", 0.0)
        .range(0.0, 5000.0, 1.0)
        .zero_means("350 µs for PWM, 250 µs for Manchester")
        .section("Test signal")
        .advanced();
/// The PPM pulse.
pub const GEN_PULSE_US: Setting<field::F64> =
    Setting::new("gen_pulse_us", "Pulse analyser test PPM pulse", "us", 300.0)
        .range(20.0, 5000.0, 1.0)
        .section("Test signal")
        .advanced();
/// The PPM gap of a 0.
pub const GEN_GAP0_US: Setting<field::F64> = Setting::new(
    "gen_gap0_us",
    "Pulse analyser test PPM gap of a 0",
    "us",
    500.0,
)
.range(20.0, 5000.0, 1.0)
.section("Test signal")
.advanced();
/// The PPM gap of a 1.
pub const GEN_GAP1_US: Setting<field::F64> = Setting::new(
    "gen_gap1_us",
    "Pulse analyser test PPM gap of a 1",
    "us",
    1000.0,
)
.range(20.0, 5000.0, 1.0)
.section("Test signal")
.advanced();
/// The timing jitter.
pub const GEN_JITTER: Setting<field::F64> =
    Setting::new("gen_jitter", "Pulse analyser test jitter", "", 0.0)
        .range(0.0, 0.2, 0.005)
        .help("Each mark and space is stretched by a uniform share up to this, either way.")
        .section("Test signal")
        .advanced();
/// The frames of a group.
pub const GEN_FRAMES: Setting<field::I64> =
    Setting::new("gen_frames", "Pulse analyser test frames", "", 4)
        .range(1.0, 50.0, 1.0)
        .section("Test signal")
        .advanced();
/// The gap added after each frame.
pub const GEN_FRAME_GAP_S: Setting<field::F64> = Setting::new(
    "gen_frame_gap_s",
    "Pulse analyser test gap after a frame",
    "s",
    0.0,
)
.range(0.0, 1.0, 0.001)
.section("Test signal")
.advanced();

/// The default bits of each pattern.
pub fn default_bits(p: GenPattern) -> &'static str {
    match p {
        GenPattern::Pwm => "101100101001110010100110",
        GenPattern::Ppm => "1100101001101011",
        GenPattern::Manchester => "10110010111000101101",
    }
}

/// The pattern of the test signal.
#[derive(Clone, Debug, PartialEq)]
pub struct PulseParams {
    /// The line code.
    pub pattern: GenPattern,
    /// The bits of each frame.
    pub bits: Vec<u8>,
    /// PWM unit or Manchester half-bit, µs.
    pub t_us: f32,
    /// PPM pulse, µs.
    pub pulse_us: f32,
    /// PPM gap of a 0, µs.
    pub gap0_us: f32,
    /// PPM gap of a 1, µs.
    pub gap1_us: f32,
    /// The largest share a run is stretched by.
    pub jitter: f64,
    /// Frames of a group.
    pub frames: usize,
    /// The gap added after each frame, seconds.
    pub frame_gap_s: f64,
}

impl PulseParams {
    /// The pattern of the settings `v`.
    pub fn of(v: &Values) -> PulseParams {
        let pattern = v.get(&GEN_PATTERN);
        let text = v.get(&GEN_BITS);
        let mut bits: Vec<u8> = text
            .bytes()
            .filter(|c| matches!(c, b'0' | b'1'))
            .map(|c| c - b'0')
            .collect();
        if bits.is_empty() {
            bits = default_bits(pattern).bytes().map(|c| c - b'0').collect();
        }
        let t = v.get(&GEN_T_US) as f32;
        PulseParams {
            pattern,
            bits,
            t_us: if t > 0.0 {
                t
            } else if pattern == GenPattern::Manchester {
                250.0
            } else {
                350.0
            },
            pulse_us: v.get(&GEN_PULSE_US) as f32,
            gap0_us: v.get(&GEN_GAP0_US) as f32,
            gap1_us: v.get(&GEN_GAP1_US) as f32,
            jitter: v.get(&GEN_JITTER),
            frames: usize::try_from(v.get(&GEN_FRAMES)).unwrap_or(1).max(1),
            frame_gap_s: v.get(&GEN_FRAME_GAP_S),
        }
    }

    /// One frame as pulses at one sample per µs, without jitter; the last space holds the gap after the frame.
    pub fn frame(&self) -> Train {
        let mut t = Train::new(Modulation::Ook, TRAIN_RATE_HZ);
        let after = (self.frame_gap_s * 1e6) as f32;
        let tu = self.t_us;
        match self.pattern {
            GenPattern::Pwm => {
                encode::pwm(&self.bits, (3.0 * tu, tu), (tu, 3.0 * tu), &mut t);
                t.pulses.push(Pulse::new(tu, 31.0 * tu + after));
            }
            GenPattern::Ppm => {
                t.pulses.push(Pulse::new(self.pulse_us, 0.0));
                for &b in &self.bits {
                    let gap = if b == 1 { self.gap1_us } else { self.gap0_us };
                    if let Some(p) = t.pulses.last_mut() {
                        p.space = gap;
                    }
                    t.pulses.push(Pulse::new(self.pulse_us, 0.0));
                }
                if let Some(p) = t.pulses.last_mut() {
                    p.space = 20e3 + after;
                }
            }
            GenPattern::Manchester => {
                let mut chips = Vec::new();
                encode::manchester(&self.bits, ManchesterConv::Thomas, tu, &mut chips, &mut t);
                if let Some(p) = t.pulses.last_mut() {
                    p.space += 20e3 + after;
                }
            }
        }
        t
    }

    /// One group: the frames back to back, each run stretched by its jitter from `rng`.
    pub fn group(&self, rng: &mut Rng) -> Train {
        let one = self.frame();
        let mut t = Train::new(Modulation::Ook, TRAIN_RATE_HZ);
        let j = self.jitter;
        let mut stretch = |x: f32| (f64::from(x) * (1.0 + j * (2.0 * rng.uniform() - 1.0))) as f32;
        for _ in 0..self.frames {
            for p in &one.pulses {
                t.pulses.push(Pulse::new(stretch(p.mark), stretch(p.space)));
            }
        }
        t
    }
}

/// The start of the first group, seconds.
pub const FIRST_S: f64 = 0.2;

/// The test signal of the pulse analyser at `rate` with the settings `v`, the jitter drawn from `seed` (module docs). A group
/// starts every `max(1, length + 0.8)` seconds.
pub fn generator(rate: f64, v: &Values, seed: u64) -> Option<CleanScene> {
    let p = PulseParams::of(v);
    let mut scene = Scene::new(rate).ok()?;
    let mut rng = Rng::new(seed);
    let mut start = FIRST_S;
    let src = move || {
        let t = p.group(&mut rng);
        let at = start;
        start += (t.len_samples() / TRAIN_RATE_HZ + 0.8).max(1.0);
        Some(Burst {
            start_s: at,
            offset_hz: 0.0,
            level_db: 0.0,
            samples: train_samples(&t, rate, 0.0),
        })
    };
    scene.add_source(Box::new(src));
    Some(CleanScene::new(scene, rate, SignalInfo::new(100e3)))
}
