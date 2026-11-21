//! The Type II test signal: a control channel (FM, phase continuous, ±1200 Hz, the bit edges smoothed by a one-pole low-pass at
//! 3000 Hz, amplitude 0.5) with system ID, site broadcast, system status and idle words, and a scene of three calls: a grant
//! when a call starts, a call update every 0.25 s while it lasts, and an FM carrier with a tone on the voice channel of each
//! clear call. The scene lasts 6 s and repeats.

use super::code::{encode, Osw, WORD_BITS};
use num_complex::Complex32;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{ConfigError, DeclareSetting, GenConfig, Generator, Hz, Setting, SignalInfo};
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The bit rate, bit/s.
pub const BIT_RATE: f64 = 3600.0;
/// The length of the scene, s.
pub const SCENE_S: f64 = 6.0;
/// The system of the test signal.
pub const SYSTEM: u16 = 0x4A2C;
/// The control channel number of the test signal.
pub const CONTROL_CHANNEL: u16 = 0x014;
/// The deviation of the voice carriers, Hz.
const VOICE_DEVIATION_HZ: f64 = 3000.0;
/// The time between call updates, s.
const UPDATE_S: f64 = 0.25;

/// The deviation of the control channel.
pub const GEN_DEVIATION_HZ: Setting<field::Hz> =
    Setting::new("gen_deviation_hz", "Test signal deviation", "Hz", 1200.0)
        .range(300.0, 3000.0, 10.0)
        .section("Test signal")
        .advanced();
/// Inverted polarity.
pub const GEN_INVERT: Setting<field::Bool> =
    Setting::new("gen_invert", "Test signal inverted", "", false)
        .section("Test signal")
        .advanced();
/// The share of words with 4 bit errors.
pub const GEN_WORD_ERRORS: Setting<field::F64> =
    Setting::new("gen_word_errors", "Test signal bad words", "", 0.0)
        .range(0.0, 1.0, 0.01)
        .section("Test signal")
        .advanced();
/// An idle word inside the site broadcast.
pub const GEN_IDLE_INSIDE: Setting<field::Bool> = Setting::new(
    "gen_idle_inside",
    "Test signal idle inside a message",
    "",
    false,
)
.section("Test signal")
.advanced();

/// The test signal settings. The framework adds the standard `gen_*` settings.
pub const SETTINGS: &[&dyn DeclareSetting] = &[
    &GEN_DEVIATION_HZ,
    &GEN_INVERT,
    &GEN_WORD_ERRORS,
    &GEN_IDLE_INSIDE,
];

/// A call of the scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCall {
    /// The talkgroup with its option bits.
    pub talkgroup: u16,
    /// The calling unit.
    pub source: u16,
    /// The channel number.
    pub channel: u16,
    /// The voice channel from the control channel, Hz.
    pub offset_hz: f64,
    /// Start and end, s.
    pub start_s: f64,
    /// End, s.
    pub end_s: f64,
    /// The tone on the voice carrier, Hz; `None` for no carrier.
    pub tone_hz: Option<f64>,
    /// A digital grant (321) instead of an analog one (308).
    pub digital: bool,
}

/// The scene: talkgroup 1230 from 12345 on channel 018 (100 kHz above the control channel), 4560 from 54321 on 01A (150 kHz
/// above), and 7770 encrypted from 777 on 022 with no voice carrier.
pub fn scene() -> Vec<SceneCall> {
    vec![
        SceneCall {
            talkgroup: 0x1230,
            source: 12345,
            channel: 0x018,
            offset_hz: 100e3,
            start_s: 1.0,
            end_s: 3.0,
            tone_hz: Some(1000.0),
            digital: false,
        },
        SceneCall {
            talkgroup: 0x4560,
            source: 54321,
            channel: 0x01A,
            offset_hz: 150e3,
            start_s: 3.5,
            end_s: 5.0,
            tone_hz: Some(1500.0),
            digital: false,
        },
        SceneCall {
            talkgroup: 0x7778,
            source: 777,
            channel: 0x022,
            offset_hz: 250e3,
            start_s: 2.0,
            end_s: 3.0,
            tone_hz: None,
            digital: false,
        },
    ]
}

/// The site broadcast words of the test system: site 3, band 3, the feature bits `features`, this site on the control channel.
pub fn site_words(features: u8, idle_inside: bool) -> Vec<Osw> {
    let mut v = vec![
        Osw::new(SYSTEM, true, 0x308),
        Osw::new(
            (2 << 10) | (3 << 7) | u16::from(features & 0x3F),
            true,
            0x320,
        ),
    ];
    if idle_inside {
        v.push(Osw::new(0, false, 0x2F8));
    }
    v.push(Osw::new(0x6000 | CONTROL_CHANNEL, false, 0x30B));
    v
}

/// The options of the generator.
#[derive(Clone, Debug)]
pub struct Type2GenOptions {
    /// The control channel relative to the capture centre, Hz.
    pub channel_hz: f64,
    /// The deviation, Hz.
    pub deviation_hz: f64,
    /// Inverted polarity.
    pub invert: bool,
    /// The share of words with 4 bit errors.
    pub word_errors: f64,
    /// An idle word inside the site broadcast.
    pub idle_inside: bool,
    /// The feature bits of the site broadcast (1F: a wide-area site).
    pub features: u8,
    /// The calls.
    pub calls: Vec<SceneCall>,
    /// The seed of the word errors.
    pub seed: u64,
}

/// The generator.
pub struct Type2Generator {
    rate: f64,
    opt: Type2GenOptions,
    rng: Rng,
    pending: VecDeque<Osw>,
    bits: [u8; WORD_BITS],
    bit: usize,
    pos: f64,
    words: u64,
    /// The scene time of the next word, s.
    t: f64,
    level: f64,
    alpha: f64,
    phase: f64,
    voice_phase: Vec<f64>,
    tone_phase: Vec<f64>,
    started: Vec<bool>,
    next_update: Vec<f64>,
    n: u64,
}

impl Type2Generator {
    /// A generator at `rate`.
    pub fn new(rate: f64, opt: Type2GenOptions) -> Type2Generator {
        let calls = opt.calls.len();
        let mut g = Type2Generator {
            rate,
            rng: Rng::new(opt.seed),
            opt,
            pending: VecDeque::new(),
            bits: [0; WORD_BITS],
            bit: 0,
            pos: 0.0,
            words: 0,
            t: 0.0,
            level: 0.0,
            alpha: 1.0 - (-TAU * 3000.0 / rate).exp(),
            phase: 0.0,
            voice_phase: vec![0.0; calls],
            tone_phase: vec![0.0; calls],
            started: vec![false; calls],
            next_update: vec![0.0; calls],
            n: 0,
        };
        g.next_word();
        g
    }

    /// The test signal of the settings of `cfg`, the control channel where a radio tuned `tune_offset_hz` above it sees it.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] if the rate cannot hold the control channel and the voice channels.
    pub fn from_config(
        cfg: &GenConfig,
        tune_offset_hz: f64,
    ) -> Result<Type2Generator, ConfigError> {
        let calls = scene();
        let far = calls
            .iter()
            .filter(|c| c.tone_hz.is_some())
            .map(|c| (c.offset_hz - tune_offset_hz).abs())
            .fold(tune_offset_hz, f64::max);
        let need = 2.0 * (far + 12.5e3);
        if cfg.rate_hz < need {
            return Err(ConfigError::RateTooLow {
                need,
                got: cfg.rate_hz,
            });
        }
        let s = &cfg.settings;
        Ok(Type2Generator::new(
            cfg.rate_hz,
            Type2GenOptions {
                channel_hz: -tune_offset_hz,
                deviation_hz: s.get(&GEN_DEVIATION_HZ),
                invert: s.get(&GEN_INVERT),
                word_errors: s.get(&GEN_WORD_ERRORS),
                idle_inside: s.get(&GEN_IDLE_INSIDE),
                features: 0x1F,
                calls,
                seed: cfg.seed,
            },
        ))
    }

    /// The words of the calls due at scene time `t`.
    fn call_words(&mut self, t: f64) {
        for (i, c) in self.opt.calls.iter().enumerate() {
            let on = (c.start_s..c.end_s).contains(&t);
            if !on {
                self.started[i] = false;
                continue;
            }
            if !self.started[i] {
                self.started[i] = true;
                self.next_update[i] = t + UPDATE_S;
                let cmd = if c.digital { 0x321 } else { 0x308 };
                self.pending.push_back(Osw::new(c.source, true, cmd));
                self.pending
                    .push_back(Osw::new(c.talkgroup, true, c.channel));
            } else if t >= self.next_update[i] {
                self.next_update[i] += UPDATE_S;
                self.pending
                    .push_back(Osw::new(c.talkgroup, true, c.channel));
            }
        }
    }

    /// Puts the next word into the bit buffer.
    fn next_word(&mut self) {
        let t = self.t;
        self.call_words(t);
        if self.pending.is_empty() {
            match self.words % 24 {
                0 => {
                    self.pending.push_back(Osw::new(SYSTEM, true, 0x308));
                    self.pending
                        .push_back(Osw::new(0x1F12, false, CONTROL_CHANNEL));
                }
                12 => self
                    .pending
                    .extend(site_words(self.opt.features, self.opt.idle_inside)),
                6 => self
                    .pending
                    .push_back(Osw::new((1 << 13) | 0x1000 | 5, false, 0x3C0)),
                _ => self.pending.push_back(Osw::new(0, false, 0x2F8)),
            }
        }
        let w = self.pending.pop_front().unwrap_or_default();
        self.bits = encode(w);
        if self.rng.uniform() < self.opt.word_errors {
            let mut flipped = 0;
            while flipped < 4 {
                let k =
                    8 + (self.rng.uniform() * (WORD_BITS - 8) as f64) as usize % (WORD_BITS - 8);
                self.bits[k] ^= 1;
                flipped += 1;
            }
        }
        self.bit = 0;
        self.words += 1;
        self.t += WORD_BITS as f64 / BIT_RATE;
        if self.t >= SCENE_S {
            self.t -= SCENE_S;
        }
    }
}

impl Generator for Type2Generator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let step = BIT_RATE / self.rate;
        for z in out.iter_mut() {
            let b = self.bits[self.bit] == 1;
            let target = if b != self.opt.invert { 1.0 } else { -1.0 };
            self.level += self.alpha * (target - self.level);
            self.phase = (self.phase
                + TAU * (self.opt.channel_hz + self.opt.deviation_hz * self.level) / self.rate)
                % TAU;
            let mut s = Complex32::from_polar(0.5, self.phase as f32);
            let t = (self.n as f64 / self.rate) % SCENE_S;
            for (i, c) in self.opt.calls.iter().enumerate() {
                let Some(tone) = c.tone_hz else { continue };
                if !(c.start_s..c.end_s).contains(&t) {
                    continue;
                }
                self.tone_phase[i] = (self.tone_phase[i] + TAU * tone / self.rate) % TAU;
                let f = self.opt.channel_hz
                    + c.offset_hz
                    + VOICE_DEVIATION_HZ * self.tone_phase[i].sin();
                self.voice_phase[i] = (self.voice_phase[i] + TAU * f / self.rate) % TAU;
                s += Complex32::from_polar(0.5, self.voice_phase[i] as f32);
            }
            *z = s;
            self.n += 1;
            self.pos += step;
            if self.pos >= 1.0 {
                self.pos -= 1.0;
                self.bit += 1;
                if self.bit >= WORD_BITS {
                    self.next_word();
                }
            }
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of the test signal is the carrier of the control channel over the noise in 25 kHz.
        Some(
            SignalInfo::new(25e3)
                .with_symbol_rate(BIT_RATE)
                .with_bit_rate(BIT_RATE)
                .with_power(0.25),
        )
    }
}
