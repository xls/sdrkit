//! Link 11 CLEW, structure only: the 16-tone parallel waveform with its 605 Hz Doppler tone and 15 data tones in frames of
//! 13.33 or 22 ms; message boundaries, the frame count and the raw phase steps. No bits are read.
//!
//! Standard: MIL-STD-188-203-1A and STANAG 5511 (not public); public summaries.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [link11: tone DFTs, preamble, onset, rate and alignment, phase steps] -> message table, text
//! ```

pub mod rx;

use crate::receiver::GEN_SECTION;
use crate::serial::gen::test_dial;
use rx::{data_tones, DOPPLER_HZ, FRAME_LEN, SYNC_HZ};
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, Hz, LockProfile, Receiver, Schema,
    Setting, SettingSet, SignalInfo, StackConfig, StackDescriptor,
};
use std::f64::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// The frame rate.
    pub enum FrameRate {
        /// Found from the signal.
        Auto = ("auto", "Automatic"),
        /// 75 frames a second.
        Fast = ("fast", "Fast (13.33 ms)"),
        /// 45.45 frames a second.
        Slow = ("slow", "Slow (22 ms)"),
    }
}

/// The frame rate the receiver reads (a fixed one replaces the rate search).
pub const FRAME_RATE: Setting<field::Choice<FrameRate>> =
    Setting::choice("frame_rate", "Frame rate", FrameRate::Auto);
/// The frame rate of the test signal.
pub const GEN_RATE: Setting<field::Choice<FrameRate>> =
    Setting::choice("gen_rate", "Frame rate", FrameRate::Fast)
        .help("Automatic sends fast frames.")
        .section(GEN_SECTION)
        .advanced();
/// The constellation offset of the test signal's phase steps.
pub const GEN_OFFSET_DEG: Setting<field::F64> =
    Setting::new("gen_offset_deg", "Phase step offset", "deg", 45.0)
        .range(0.0, 89.0, 1.0)
        .section(GEN_SECTION)
        .advanced();
/// Preamble frames of the test signal.
pub const GEN_PREAMBLE_FRAMES: Setting<field::I64> =
    Setting::new("gen_preamble_frames", "Preamble frames", "", 5)
        .range(2.0, 20.0, 1.0)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the Link 11 receiver.
    pub enum Link11RxSetting {
        /// Frame rate.
        FrameRate = FRAME_RATE,
    }
}

sdrkit_core::settings! {
    /// The settings of the Link 11 stack.
    pub enum Link11Setting {
        /// Frame rate.
        FrameRate = FRAME_RATE,
        /// Frame rate of the test signal.
        GenRate = GEN_RATE,
        /// Phase step offset of the test signal.
        GenOffsetDeg = GEN_OFFSET_DEG,
        /// Preamble frames of the test signal.
        GenPreambleFrames = GEN_PREAMBLE_FRAMES,
    }
}

/// Data frames of a test message.
pub const TEST_FRAMES: usize = 60;

/// The quarter turns of the data frames of test message `seed`: `frames` frames of 15 tones.
pub fn test_steps(frames: usize, seed: u32) -> Vec<[u8; 15]> {
    let mut x = 0x9E37_79B9u32 ^ seed.wrapping_mul(0x85EB_CA6B);
    (0..frames)
        .map(|_| {
            let mut f = [0u8; 15];
            for v in &mut f {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                *v = (x >> 7) as u8 & 3;
            }
            f
        })
        .collect()
}

/// One message of the test signal: `pre` preamble frames, the phase-start frame, the data frames of `steps`, at frame
/// length `len` samples at 24 kHz; the phase steps turned by `offset` radians.
#[derive(Clone, Debug)]
pub struct Message {
    /// Frame length, seconds.
    pub frame_s: f64,
    /// Preamble frames.
    pub pre: usize,
    /// The quarter turns of each data frame.
    pub steps: Vec<[u8; 15]>,
    /// The offset of the steps, radians.
    pub offset: f64,
    /// Silence after the message, seconds.
    pub gap_s: f64,
}

impl Message {
    fn frames(&self) -> usize {
        self.pre + 1 + self.steps.len()
    }
    fn length(&self) -> f64 {
        self.frames() as f64 * self.frame_s + self.gap_s
    }
}

/// The test signal: a list of messages, repeated.
pub struct Link11Gen {
    rate: f64,
    dial: f64,
    msgs: Vec<Message>,
    /// The phase of each data tone at each frame of each message.
    phases: Vec<Vec<[f64; 15]>>,
    period: f64,
    n: u64,
}

impl Link11Gen {
    /// The messages `msgs` at `rate`.
    pub fn new(rate: f64, msgs: Vec<Message>) -> Link11Gen {
        let phases = msgs
            .iter()
            .map(|m| {
                let mut p = [0.0f64; 15];
                for (k, v) in p.iter_mut().enumerate() {
                    *v = 0.7 * k as f64;
                }
                let mut all = vec![p];
                for s in &m.steps {
                    for k in 0..15 {
                        p[k] += f64::from(s[k]) * FRAC_PI_2 + m.offset;
                    }
                    all.push(p);
                }
                all
            })
            .collect();
        let period = msgs.iter().map(Message::length).sum();
        Link11Gen {
            rate,
            dial: test_dial(rate),
            msgs,
            phases,
            period,
            n: 0,
        }
    }
}

/// The amplitude of a data tone: the Doppler tone is 1.4 times it, the whole of a data frame unit power.
const A: f64 = 0.242_8;

impl Generator for Link11Gen {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(3000.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        self.dial
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let tones = data_tones();
        for o in out.iter_mut() {
            let ta = self.n as f64 / self.rate;
            self.n += 1;
            let mut t = ta % self.period;
            let mut v = num_complex::Complex64::default();
            let tone = |f: f64, ph: f64, a: f64| {
                let p = TAU * (f + self.dial) * ta + ph;
                num_complex::Complex64::new(a * p.cos(), a * p.sin())
            };
            for (mi, m) in self.msgs.iter().enumerate() {
                if t >= m.length() {
                    t -= m.length();
                    continue;
                }
                let fr = (t / m.frame_s) as usize;
                if fr >= m.frames() {
                    break;
                }
                v += tone(DOPPLER_HZ, 0.0, 1.4 * A);
                if fr < m.pre {
                    v += tone(SYNC_HZ, std::f64::consts::PI * fr as f64, 1.4 * A);
                } else {
                    let ph = &self.phases[mi][fr - m.pre];
                    for (k, &f) in tones.iter().enumerate() {
                        v += tone(f, ph[k], A);
                    }
                }
                break;
            }
            *o = Complex32::new(v.re as f32, v.im as f32);
        }
        out.len()
    }
}

/// The test message of `seed` at a frame rate (0 fast, 1 slow).
pub fn test_message(rate: usize, offset_deg: f64, pre: usize, seed: u32) -> Message {
    Message {
        frame_s: FRAME_LEN[rate] as f64 / 24_000.0,
        pre,
        steps: test_steps(TEST_FRAMES, seed),
        offset: offset_deg.to_radians(),
        gap_s: 0.4,
    }
}

/// The test signal at `cfg.rate_hz`: two messages, repeated.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let rate = usize::from(s.get(&GEN_RATE) == FrameRate::Slow);
    let off = s.get(&GEN_OFFSET_DEG);
    let pre = s.get(&GEN_PREAMBLE_FRAMES).clamp(2, 20) as usize;
    let msgs = vec![
        test_message(rate, off, pre, 1),
        test_message(rate, off, pre, 2),
    ];
    Ok(Box::new(Link11Gen::new(cfg.rate_hz, msgs)))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    Link11Setting::schema()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("link11"),
        "Link 11",
        rx::Link11Decoder::new(),
        cfg,
    )
}

/// Link 11 CLEW.
pub static LINK11: StackDescriptor = StackDescriptor {
    id: "link11",
    name: "Link 11 CLEW (structure)",
    group: "HF utility",
    blurb: "The 16-tone Link 11 waveform: messages, frame rate, frame count, Doppler offset and the raw phase steps of the 15 data tones (no bits).",
    tuning: crate::ms110::hf_tuning(crate::ms110::TEST_SIGNAL_HZ),
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::LINK11),
    benchmarks: Some(crate::benchmarks),
};
