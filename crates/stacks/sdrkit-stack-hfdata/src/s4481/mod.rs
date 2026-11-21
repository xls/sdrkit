//! STANAG 4481 FSK naval broadcasts: one channel of +-425 Hz, or up to 16 sub-carriers of +-42.5 Hz at 255 + 170 n Hz; the speed
//! of each channel, plain ITA2 start-stop text, and synchronous (encrypted) channels measured.
//!
//! Standard: NATO STANAG 4481 Annex C (not public).
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [s4481: tone filters, noise floor, single-channel search, decisions, speed, start-stop readers] -> text, channel table
//! ```

pub mod rx;

use crate::receiver::GEN_SECTION;
use crate::serial::gen::test_dial;
use crate::serial::TEST_TEXT;
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, Hz, LockProfile, Receiver, Schema,
    Setting, SettingSet, SignalInfo, StackConfig, StackDescriptor,
};
use sdrkit_util_text::ita2;
use std::f64::consts::TAU;
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// Which channels are read.
    pub enum ChannelMode {
        /// Found from the signal.
        Auto = ("auto", "Automatic"),
        /// One channel of +-425 Hz.
        Single = ("single", "Single (+-425 Hz)"),
        /// Sub-carriers of +-42.5 Hz.
        Multi = ("multi", "Multi (+-42.5 Hz)"),
    }
}

sdrkit_core::choice_enum! {
    /// The speed.
    pub enum Speed {
        /// Found from the run lengths.
        Auto = ("auto", "Automatic"),
        /// 45.45 Bd.
        B45 = ("45.45", "45.45 Bd"),
        /// 50 Bd.
        B50 = ("50", "50 Bd"),
        /// 75 Bd.
        B75 = ("75", "75 Bd"),
        /// 100 Bd.
        B100 = ("100", "100 Bd"),
    }
}

sdrkit_core::choice_enum! {
    /// The figures case of the ITA2 alphabet.
    pub enum Alphabet {
        /// The US teleprinter figures.
        Us = ("us", "US"),
        /// The ITU figures.
        Itu = ("itu", "ITU"),
    }
}

/// Which channels are read.
pub const CHANNEL_MODE: Setting<field::Choice<ChannelMode>> =
    Setting::choice("channel_mode", "Channels", ChannelMode::Auto);
/// The speed.
pub const BAUD: Setting<field::Choice<Speed>> = Setting::choice("baud", "Speed", Speed::Auto);
/// The figures case.
pub const ALPHABET: Setting<field::Choice<Alphabet>> =
    Setting::choice("alphabet", "Figures case", Alphabet::Us);
/// Multi-channel test signal (else one channel).
pub const GEN_MULTI: Setting<field::Bool> = Setting::new("gen_multi", "Multi-channel", "", true)
    .section(GEN_SECTION)
    .advanced();
/// The speed of the test signal.
pub const GEN_BAUD: Setting<field::Choice<Speed>> =
    Setting::choice("gen_baud", "Speed", Speed::B75)
        .help("Automatic sends 75 Bd.")
        .section(GEN_SECTION)
        .advanced();
/// The mark tone of the single-channel test signal.
pub const GEN_SINGLE_MARK_HZ: Setting<field::Hz> =
    Setting::new("gen_single_mark_hz", "Single-channel mark", "Hz", 1275.0)
        .range(400.0, 2400.0, 5.0)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the STANAG 4481 receiver.
    pub enum S4481RxSetting {
        /// Channels.
        ChannelMode = CHANNEL_MODE,
        /// Speed.
        Baud = BAUD,
        /// Figures case.
        Alphabet = ALPHABET,
    }
}

sdrkit_core::settings! {
    /// The settings of the STANAG 4481 stack.
    pub enum S4481Setting {
        /// Channels.
        ChannelMode = CHANNEL_MODE,
        /// Speed.
        Baud = BAUD,
        /// Figures case.
        Alphabet = ALPHABET,
        /// Multi-channel test signal.
        GenMulti = GEN_MULTI,
        /// Speed of the test signal.
        GenBaud = GEN_BAUD,
        /// Mark tone of the single-channel test signal.
        GenSingleMarkHz = GEN_SINGLE_MARK_HZ,
    }
}

/// The elements of a start-stop text at `baud`: (duration in seconds, mark), with `lead` seconds of mark before.
pub fn start_stop(text: &str, baud: f64, lead: f64) -> Vec<(f64, bool)> {
    let mut codes = Vec::new();
    let _ = ita2::Encoder::new(&ita2::US_TTY).encode_str(text, &mut codes);
    let bit = 1.0 / baud;
    let mut v = vec![(lead, true)];
    for c in codes {
        v.push((bit, false));
        for i in 0..5 {
            v.push((bit, (c >> i) & 1 == 1));
        }
        v.push((1.5 * bit, true));
    }
    v
}

/// The elements of `n` synchronous bits at `baud` from a fixed pseudo-random sequence, with `lead` seconds of mark before.
pub fn synchronous(n: usize, baud: f64, lead: f64) -> Vec<(f64, bool)> {
    let mut x = 0x1234_5678u32;
    let mut v = vec![(lead, true)];
    for _ in 0..n {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        v.push((1.0 / baud, x & 1 == 1));
    }
    v
}

/// A keyed FSK channel: mark and space tones and its elements.
#[derive(Clone, Debug)]
pub struct Keyed {
    /// The mark tone, Hz above the dial.
    pub mark_hz: f64,
    /// The space tone, Hz above the dial.
    pub space_hz: f64,
    /// The elements: duration in seconds and mark.
    pub elements: Vec<(f64, bool)>,
}

/// A generator of FSK channels, phase continuous, repeated with `period` seconds a cycle (idle mark after the elements).
pub struct FskGen {
    rate: f64,
    dial: f64,
    chans: Vec<Keyed>,
    phases: Vec<f64>,
    /// The end time of each element of each channel, and the element now sent.
    ends: Vec<Vec<f64>>,
    at: Vec<usize>,
    last_t: f64,
    amp: f32,
    period: f64,
    n: u64,
}

impl FskGen {
    /// The channels `chans` at `rate`, a cycle of `period` seconds.
    pub fn new(rate: f64, chans: Vec<Keyed>, period: f64) -> FskGen {
        let amp = (1.0 / chans.len().max(1) as f64).sqrt() as f32;
        FskGen {
            rate,
            dial: test_dial(rate),
            phases: vec![0.0; chans.len()],
            ends: chans
                .iter()
                .map(|c| {
                    let mut acc = 0.0;
                    c.elements
                        .iter()
                        .map(|e| {
                            acc += e.0;
                            acc
                        })
                        .collect()
                })
                .collect(),
            at: vec![0; chans.len()],
            last_t: 0.0,
            chans,
            amp,
            period,
            n: 0,
        }
    }
}

impl Generator for FskGen {
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
        for o in out.iter_mut() {
            let t = (self.n as f64 / self.rate) % self.period;
            self.n += 1;
            if t < self.last_t {
                self.at.iter_mut().for_each(|a| *a = 0);
            }
            self.last_t = t;
            let mut v = Complex32::default();
            for (ci, (c, ph)) in self.chans.iter().zip(self.phases.iter_mut()).enumerate() {
                let ends = &self.ends[ci];
                while self.at[ci] < ends.len() && t >= ends[self.at[ci]] {
                    self.at[ci] += 1;
                }
                let mark = c.elements.get(self.at[ci]).is_none_or(|e| e.1);
                let f = if mark { c.mark_hz } else { c.space_hz };
                *ph = (*ph + TAU * (f + self.dial) / self.rate).rem_euclid(TAU);
                v += Complex32::new(ph.cos() as f32, ph.sin() as f32);
            }
            *o = v * self.amp;
        }
        out.len()
    }
}

/// The length of a cycle of elements, seconds.
fn length(e: &[(f64, bool)]) -> f64 {
    e.iter().map(|x| x.0).sum()
}

/// The test signal: multi-channel (channel 3 the test text, channel 7 600 synchronous bits, channel 11 the test text) or one
/// channel of the test text; 0.5 s of mark before and 0.3 s after, repeated.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let baud = match s.get(&GEN_BAUD) {
        Speed::B45 => 45.45,
        Speed::B50 => 50.0,
        Speed::B100 => 100.0,
        _ => 75.0,
    };
    let chans = if s.get(&GEN_MULTI) {
        let sub = |n: usize| (rx::subcarrier(n) - 42.5, rx::subcarrier(n) + 42.5);
        let (m3, s3) = sub(3);
        let (m7, s7) = sub(7);
        let (m11, s11) = sub(11);
        vec![
            Keyed {
                mark_hz: m3,
                space_hz: s3,
                elements: start_stop(TEST_TEXT, baud, 0.5),
            },
            Keyed {
                mark_hz: m7,
                space_hz: s7,
                elements: synchronous(600, baud, 0.5),
            },
            Keyed {
                mark_hz: m11,
                space_hz: s11,
                elements: start_stop(TEST_TEXT, baud, 0.5),
            },
        ]
    } else {
        let m = s.get(&GEN_SINGLE_MARK_HZ);
        vec![Keyed {
            mark_hz: m,
            space_hz: m + 850.0,
            elements: start_stop(TEST_TEXT, baud, 0.5),
        }]
    };
    let period = chans
        .iter()
        .map(|c| length(&c.elements))
        .fold(0.0, f64::max)
        + 0.3;
    Ok(Box::new(FskGen::new(cfg.rate_hz, chans, period)))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    S4481Setting::schema()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("s4481"),
        "STANAG 4481",
        rx::S4481Decoder::new(),
        cfg,
    )
}

/// STANAG 4481 FSK.
pub static S4481: StackDescriptor = StackDescriptor {
    id: "s4481",
    name: "STANAG 4481 FSK",
    group: "HF utility",
    blurb: "Naval broadcast FSK: one +-425 Hz channel or up to 16 sub-carriers of +-42.5 Hz; the speed of each, plain ITA2 text, encrypted channels measured.",
    tuning: crate::ms110::hf_tuning(crate::ms110::TEST_SIGNAL_HZ),
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::S4481),
    benchmarks: Some(crate::benchmarks),
};
