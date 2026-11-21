//! The PSK test signal: a known payload in frames after a sync word, any order, rate, pulse shape and coding, with the
//! carrier at 0 Hz. Offsets, Doppler, clock errors and noise come from the standard impairments.

use super::bits::{parse_sync, BitEncoder, Descrambler, Differential};
use sdrkit_algo_dsp::pulse;
use sdrkit_algo_modem::psk::{PskOrder, PskPulse};
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo};
use sdrkit_proto_ccsds::randomiser::tm_bit;
use std::collections::VecDeque;

sdrkit_core::choice_enum! {
    /// The coding of the test signal.
    pub enum GenCoding {
        /// No coding.
        None = ("none", "None"),
        /// Symbol differential.
        Dpsk = ("dpsk", "Differential PSK"),
        /// NRZ-M.
        NrzM = ("nrzm", "NRZ-M"),
        /// NRZ-S.
        NrzS = ("nrzs", "NRZ-S"),
        /// The G3RUH scrambler.
        G3ruh = ("g3ruh", "G3RUH"),
        /// The CCSDS randomiser.
        Ccsds = ("ccsds", "CCSDS"),
    }
}

impl GenCoding {
    /// The differential code and the scrambler of the coding.
    pub fn parts(self) -> (Differential, Descrambler) {
        match self {
            GenCoding::None => (Differential::None, Descrambler::None),
            GenCoding::Dpsk => (Differential::Dpsk, Descrambler::None),
            GenCoding::NrzM => (Differential::NrzM, Descrambler::None),
            GenCoding::NrzS => (Differential::NrzS, Descrambler::None),
            GenCoding::G3ruh => (Differential::None, Descrambler::G3ruh),
            GenCoding::Ccsds => (Differential::None, Descrambler::Ccsds),
        }
    }
}

sdrkit_core::choice_enum! {
    /// The order of the test signal.
    pub enum GenOrder {
        /// BPSK.
        Bpsk = ("bpsk", "BPSK"),
        /// QPSK.
        Qpsk = ("qpsk", "QPSK"),
        /// Offset QPSK.
        Oqpsk = ("oqpsk", "OQPSK"),
        /// 8PSK.
        Psk8 = ("8psk", "8PSK"),
    }
}

impl GenOrder {
    /// The order.
    pub fn order(self) -> PskOrder {
        match self {
            GenOrder::Bpsk => PskOrder::Bpsk,
            GenOrder::Qpsk => PskOrder::Qpsk,
            GenOrder::Oqpsk => PskOrder::Oqpsk,
            GenOrder::Psk8 => PskOrder::Psk8,
        }
    }
}

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";
/// The order of the test signal.
pub const GEN_ORDER: Setting<field::Choice<GenOrder>> =
    Setting::choice("gen_order", "Order", GenOrder::Qpsk)
        .section(TEST_SIGNAL)
        .advanced();
/// The symbol rate of the test signal.
pub const GEN_BAUD: Setting<field::Hz> = Setting::new("gen_baud", "Symbol rate", "Bd", 4800.0)
    .range(75.0, 100_000.0, 1.0)
    .section(TEST_SIGNAL)
    .advanced();
/// The roll-off of the test signal (0: rectangular pulses).
pub const GEN_ROLLOFF: Setting<field::F64> =
    Setting::new("gen_rolloff", "Roll-off (0: rectangular)", "", 0.35)
        .range(0.0, 1.0, 0.05)
        .section(TEST_SIGNAL)
        .advanced();
/// The coding of the test signal.
pub const GEN_CODING: Setting<field::Choice<GenCoding>> =
    Setting::choice("gen_coding", "Coding", GenCoding::None)
        .section(TEST_SIGNAL)
        .advanced();
/// The sync word of the test signal.
pub const GEN_SYNC: Setting<field::Text> = Setting::new("gen_sync", "Sync word", "", "1ACFFC1D")
    .section(TEST_SIGNAL)
    .advanced();
/// The test signal swaps I and Q.
pub const GEN_CONJUGATE: Setting<field::Bool> =
    Setting::new("gen_conjugate", "Conjugated symbols", "", false)
        .help("The transmitter swaps I and Q (the decoder must find the spectral inversion).")
        .section(TEST_SIGNAL)
        .advanced();

/// What the test signal is.
#[derive(Clone, Debug)]
pub struct GenParams {
    pub rate: f64,
    pub order: PskOrder,
    pub baud: f64,
    pub shape: PskPulse,
    pub diff: Differential,
    pub descr: Descrambler,
    pub sync: String,
    pub payload_bits: usize,
    pub conjugate: bool,
    pub seed: u64,
}

impl GenParams {
    /// QPSK at 4800 Bd, RRC 0.35, at `rate`.
    pub fn new(rate: f64) -> Self {
        GenParams {
            rate,
            order: PskOrder::Qpsk,
            baud: 4800.0,
            shape: PskPulse::Rrc(0.35),
            diff: Differential::None,
            descr: Descrambler::None,
            sync: "1ACFFC1D".into(),
            payload_bits: 224,
            conjugate: false,
            seed: 1,
        }
    }
}

/// The payload of frame `k` (repeatable from the seed; every frame differs, so the signal has no spectral lines of its own).
pub fn payload(seed: u64, k: u64, n: usize) -> Vec<u8> {
    let mut r = Rng::stream(seed ^ 0xC0FFEE, k);
    (0..n).map(|_| r.bit()).collect()
}

/// The bits of frame `k`: the sync word, then the payload (CCSDS randomised when that coding is used).
pub fn frame_bits(p: &GenParams, k: u64) -> Vec<u8> {
    let mut frame = parse_sync(&p.sync).unwrap_or_default();
    let mut pl = payload(p.seed, k, p.payload_bits);
    if p.descr == Descrambler::Ccsds {
        for (i, b) in pl.iter_mut().enumerate() {
            *b ^= tm_bit(i);
        }
    }
    frame.extend_from_slice(&pl);
    frame
}

/// The PSK generator.
pub struct PskGenerator {
    p: GenParams,
    m: Linear,
    queue: VecDeque<Complex32>,
    enc: BitEncoder,
    frames: u64,
    ks: Vec<usize>,
    syms: Vec<Complex32>,
    samples: Vec<Complex32>,
}

impl PskGenerator {
    /// The generator from the stack's test-signal settings.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let mut p = GenParams::new(cfg.rate_hz);
        p.order = s.get(&GEN_ORDER).order();
        p.baud = s.get(&GEN_BAUD);
        let r = s.get(&GEN_ROLLOFF);
        p.shape = if r <= 0.0 {
            PskPulse::Rect
        } else {
            PskPulse::Rrc(r)
        };
        (p.diff, p.descr) = s.get(&GEN_CODING).parts();
        p.sync = s.get(&GEN_SYNC).to_string();
        p.conjugate = s.get(&GEN_CONJUGATE);
        p.seed = cfg.seed;
        Self::new(p)
    }

    /// The generator of `p`.
    pub fn new(p: GenParams) -> Self {
        let sps = p.rate / p.baud.max(1.0);
        let (shape, span) = match p.shape {
            PskPulse::Rect => (pulse::Shape::Rect, 1.0),
            PskPulse::Rrc(b) => (
                pulse::Shape::Rrc {
                    beta: b.clamp(0.0, 1.0),
                },
                16.0,
            ),
        };
        let mut m = Linear::new(shape, span, sps)
            .or_else(|_| Linear::new(pulse::Shape::Rect, 1.0, sps.max(1.0)))
            .expect("a rectangular pulse at a positive rate");
        if p.order == PskOrder::Oqpsk {
            m = m.with_offset_q();
        }
        PskGenerator {
            enc: BitEncoder::new(p.order, p.diff, p.descr, false),
            frames: 0,
            m,
            queue: VecDeque::new(),
            ks: Vec::new(),
            syms: Vec::new(),
            samples: Vec::new(),
            p,
        }
    }

    /// Modulates the next frame into the queue.
    fn next_frame(&mut self) {
        let bits = frame_bits(&self.p, self.frames);
        self.frames += 1;
        self.ks.clear();
        self.enc.push(&bits, &mut self.ks);
        let (order, conj) = (self.p.order, self.p.conjugate);
        self.syms.clear();
        self.syms.extend(self.ks.iter().map(|&k| {
            let pt = order.point(k);
            if conj {
                pt.conj()
            } else {
                pt
            }
        }));
        self.samples.clear();
        self.m.push(&self.syms, &mut self.samples);
        self.queue.extend(self.samples.iter().copied());
    }
}

impl Generator for PskGenerator {
    /// The dial frequency is at 0 Hz.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn rate_hz(&self) -> f64 {
        self.p.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        let rolloff = match self.p.shape {
            PskPulse::Rrc(b) => b,
            PskPulse::Rect => 1.0,
        };
        let sync = parse_sync(&self.p.sync).map_or(0, |s| s.len()) as f64;
        let share = self.p.payload_bits as f64 / (self.p.payload_bits as f64 + sync);
        Some(SignalInfo {
            bandwidth_hz: self.p.baud * (1.0 + rolloff),
            symbol_rate_hz: Some(self.p.baud),
            bit_rate_hz: Some(self.p.baud * self.p.order.bits() as f64 * share),
            power: 1.0,
        })
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            while self.queue.is_empty() {
                self.next_frame();
            }
            *o = self.queue.pop_front().unwrap_or_default();
        }
        out.len()
    }
}
