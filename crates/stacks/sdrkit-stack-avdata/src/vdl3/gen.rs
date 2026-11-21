//! The VDL Mode 3 test signal: slots of 30 ms, four to a frame of 120 ms. Each slot holds an M burst 2 ms after its start and,
//! 12 symbols after its end, a voice or data burst; an M uplink of three slots fills its slot. The kinds of the M bursts run
//! through a cycle of 12 slots; every second slot holds an aircraft burst 6 dB below the ground station, with an offset of its
//! own within 300 Hz. Two of three voice or data bursts carry the text "VDL3 TEST" and a counter, the third random vocoder
//! bits. D8PSK with raised-cosine pulses (roll-off 0.6), unit power while the ground station sends. The slot plan is made up
//! for the test.

use super::phy::{
    payload_bits, steps_of_bits, BurstKind, Content, OctetOrder, RAMP, RAMP_LEN, RS_N, SYMBOL_RATE,
    TRAINING_LEN,
};
use sdrkit_algo_dsp::pulse::rc;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, Hz, SignalInfo};
use std::f64::consts::{FRAC_PI_4, PI, TAU};

/// A slot, s.
pub const SLOT_S: f64 = 0.03;
/// The slots of the kind cycle.
pub const CYCLE_SLOTS: usize = 12;
/// The M burst kinds of the cycle; the odd slots are aircraft bursts.
pub const CYCLE: [BurstKind; CYCLE_SLOTS] = [
    BurstKind::MUplink,
    BurstKind::MDownlink,
    BurstKind::MUplink,
    BurstKind::NetEntry,
    BurstKind::Handoff,
    BurstKind::PollResponse,
    BurstKind::MUplink,
    BurstKind::MDownlink,
    BurstKind::MUplink3T,
    BurstKind::NetEntry,
    BurstKind::Handoff,
    BurstKind::PollResponse,
];
/// The header word of the voice and data bursts.
pub const HEADER: u16 = 0x123;
/// The start of the M burst in its slot, s.
pub const M_START_S: f64 = 0.002;
/// Symbols between the end of an M burst and the ramp of the voice or data burst.
pub const GAP_SYMBOLS: usize = 12;
/// The half length of the pulse, symbols.
const PULSE_HALF: usize = 5;
/// Points of the pulse table per symbol.
const PULSE_RES: usize = 64;
/// The mean power of raised-cosine D8PSK of unit symbols, `1 - alpha / 4`.
const RC_POWER: f64 = 0.85;

/// The test signal settings.
#[derive(Clone, Debug, PartialEq)]
pub struct GenParams {
    /// Sample rate, Hz.
    pub rate_hz: f64,
    /// Where the channel lies, Hz from the centre.
    pub channel_hz: f64,
    /// Seed of the random content.
    pub seed: u64,
    /// A second station 25 kHz above at the same level.
    pub adjacent: bool,
}

/// One burst as sent.
#[derive(Clone, Debug, PartialEq)]
pub struct Sent {
    /// Signal time of the end of the training, s.
    pub time_s: f64,
    /// The kind.
    pub kind: BurstKind,
    /// What it carries.
    pub content: Content,
    /// Sent by an aircraft.
    pub aircraft: bool,
}

/// The VDL Mode 3 generator.
pub struct Vdl3Generator {
    p: GenParams,
    rng: Rng,
    pulse: Vec<f64>,
    buf: Vec<Complex32>,
    pos: usize,
    /// Samples made before the buffer.
    done: u64,
    cycle: u64,
    data_count: u64,
    vd_count: u64,
    sent: Vec<Sent>,
}

impl Vdl3Generator {
    /// A generator for `p`.
    pub fn new(p: GenParams) -> Self {
        let n = 2 * PULSE_HALF * PULSE_RES;
        let pulse = (0..=n)
            .map(|i| {
                let t = (i as f64 - (PULSE_HALF * PULSE_RES) as f64) / PULSE_RES as f64;
                rc(t, 0.6) * (0.5 + 0.5 * (PI * t / PULSE_HALF as f64).cos())
            })
            .collect();
        Vdl3Generator {
            rng: Rng::new(p.seed ^ 0x5644_4C33),
            p,
            pulse,
            buf: Vec::new(),
            pos: 0,
            done: 0,
            cycle: 0,
            data_count: 0,
            vd_count: 0,
            sent: Vec::new(),
        }
    }

    /// The generator of the settings of `cfg`, the channel where a radio tuned `tune_offset_hz` above it sees it.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] when the rate cannot hold the channel.
    pub fn from_config(cfg: &GenConfig, tune_offset_hz: f64) -> Result<Self, ConfigError> {
        let need = 2.0 * (tune_offset_hz.abs() + 12.5e3);
        if cfg.rate_hz < need {
            return Err(ConfigError::RateTooLow {
                need,
                got: cfg.rate_hz,
            });
        }
        Ok(Vdl3Generator::new(GenParams {
            rate_hz: cfg.rate_hz,
            channel_hz: -tune_offset_hz,
            seed: cfg.seed,
            adjacent: cfg.settings.get(&super::GEN_ADJACENT),
        }))
    }

    /// The bursts sent so far, oldest first.
    pub fn sent(&self) -> &[Sent] {
        &self.sent
    }

    fn pulse_at(&self, t: f64) -> f64 {
        let x = (t + PULSE_HALF as f64) * PULSE_RES as f64;
        if x < 0.0 || x >= (2 * PULSE_HALF * PULSE_RES) as f64 {
            return 0.0;
        }
        let i = x as usize;
        let f = x - i as f64;
        self.pulse[i] * (1.0 - f) + self.pulse[i + 1] * f
    }

    /// Adds a burst whose first ramp symbol is at `t0` (s from the start of the buffer) to the buffer.
    fn add(&mut self, t0: f64, kind: BurstKind, content: &Content, amp: f64, freq_hz: f64) -> f64 {
        let mut bits = Vec::new();
        payload_bits(kind, content, OctetOrder::LsbFirst, &mut bits);
        let mut steps = Vec::new();
        steps_of_bits(&bits, &mut steps);
        let training = kind.training().steps();
        let mut syms: Vec<Complex32> = Vec::with_capacity(RAMP_LEN + TRAINING_LEN + steps.len());
        let mut ph = 0i32;
        for e in RAMP {
            syms.push(Complex32::new(e, 0.0));
        }
        for s in training {
            ph += s;
            syms.push(Complex32::from_polar(
                1.0,
                (FRAC_PI_4 * f64::from(ph)) as f32,
            ));
        }
        for &s in &steps {
            ph += i32::from(s);
            syms.push(Complex32::from_polar(
                1.0,
                (FRAC_PI_4 * f64::from(ph)) as f32,
            ));
        }
        let rate = self.p.rate_hz;
        let sps = rate / SYMBOL_RATE;
        let first = ((t0 * rate) - PULSE_HALF as f64 * sps).floor().max(0.0) as usize;
        let last = ((t0 * rate) + (syms.len() + PULSE_HALF) as f64 * sps).ceil() as usize;
        if self.buf.len() < last + 1 {
            self.buf.resize(last + 1, Complex32::default());
        }
        let abs0 = self.done as f64;
        for n in first..=last {
            let u = (n as f64 / rate - t0) * SYMBOL_RATE;
            let k0 = (u - PULSE_HALF as f64).ceil().max(0.0) as usize;
            let k1 = ((u + PULSE_HALF as f64).floor() as usize).min(syms.len().saturating_sub(1));
            let mut v = Complex32::default();
            for (k, a) in syms.iter().enumerate().take(k1 + 1).skip(k0) {
                v += a * self.pulse_at(u - k as f64) as f32;
            }
            let ph = TAU * freq_hz * (abs0 + n as f64) / rate;
            self.buf[n] += v * Complex32::from_polar(amp as f32, ph.rem_euclid(TAU) as f32);
        }
        t0 + (RAMP_LEN + TRAINING_LEN + steps.len()) as f64 / SYMBOL_RATE
    }

    fn words(&mut self, kind: BurstKind) -> Vec<u16> {
        (0..kind.words())
            .map(|i| match i {
                0 => 0xA5C,
                1 => (self.cycle as u16).wrapping_mul(7) & 0xFFF,
                _ => (self.rng.next_u64() & 0xFFF) as u16,
            })
            .collect()
    }

    fn vd_content(&mut self) -> Content {
        self.vd_count += 1;
        if self.vd_count.is_multiple_of(3) {
            let bits = (0..RS_N * 8)
                .map(|_| (self.rng.next_u64() & 1) as u8)
                .collect();
            Content::Voice(HEADER, bits)
        } else {
            self.data_count += 1;
            Content::Data(
                HEADER,
                format!("VDL3 TEST {}", self.data_count).into_bytes(),
            )
        }
    }

    /// Makes the next cycle of 12 slots into the buffer.
    fn cycle(&mut self) {
        let rest = self.buf.split_off(self.pos.min(self.buf.len()));
        self.done += self.pos as u64;
        self.buf = rest;
        self.pos = 0;
        let start_s = self.done as f64 / self.p.rate_hz;
        let n = (CYCLE_SLOTS as f64 * SLOT_S * self.p.rate_hz).round() as usize;
        if self.buf.len() < n {
            self.buf.resize(n, Complex32::default());
        }
        let ground = 1.0 / RC_POWER.sqrt();
        let stations: &[(f64, f64)] = if self.p.adjacent {
            &[(0.0, 0.0), (25e3, 0.015)]
        } else {
            &[(0.0, 0.0)]
        };
        for &(df, dt) in stations {
            for (s, &kind) in CYCLE.iter().enumerate() {
                let t_slot = s as f64 * SLOT_S + dt;
                let aircraft = s % 2 == 1;
                let (amp, f) = if aircraft {
                    (ground * 0.5, self.rng.range(-300.0, 300.0))
                } else {
                    (ground, 0.0)
                };
                let content = Content::Words(self.words(kind));
                let t0 = t_slot + M_START_S;
                let end = self.add(t0, kind, &content, amp, self.p.channel_hz + df + f);
                if df == 0.0 {
                    self.sent.push(Sent {
                        time_s: start_s + t0 + (RAMP_LEN + TRAINING_LEN) as f64 / SYMBOL_RATE,
                        kind,
                        content,
                        aircraft,
                    });
                }
                if kind == BurstKind::MUplink3T {
                    continue;
                }
                let content = self.vd_content();
                let kind = match content {
                    Content::Voice(..) => BurstKind::VdVoice,
                    _ => BurstKind::VdData,
                };
                let t1 = end + GAP_SYMBOLS as f64 / SYMBOL_RATE;
                self.add(t1, kind, &content, ground, self.p.channel_hz + df);
                if df == 0.0 {
                    self.sent.push(Sent {
                        time_s: start_s + t1 + (RAMP_LEN + TRAINING_LEN) as f64 / SYMBOL_RATE,
                        kind,
                        content,
                        aircraft: false,
                    });
                }
            }
        }
        if self.sent.len() > 4096 {
            self.sent.drain(..2048);
        }
        self.cycle += 1;
    }
}

impl Generator for Vdl3Generator {
    fn rate_hz(&self) -> Hz {
        self.p.rate_hz
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let cycle_len = (CYCLE_SLOTS as f64 * SLOT_S * self.p.rate_hz).round() as usize;
        let mut i = 0;
        while i < out.len() {
            if self.buf.is_empty() || self.pos >= cycle_len {
                self.cycle();
            }
            let n = (cycle_len - self.pos).min(out.len() - i);
            out[i..i + n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            i += n;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(25e3)
                .with_symbol_rate(SYMBOL_RATE)
                .with_bit_rate(3.0 * SYMBOL_RATE),
        )
    }
}
