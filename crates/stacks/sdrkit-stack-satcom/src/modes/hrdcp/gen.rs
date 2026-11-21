//! The HRDCP test signal: platforms on their channels, each sending its message now and then: the 2 s carrier at 45°, the
//! preamble and marker in BPSK, then the coded message in offset QPSK at 1200 symbols a second on each rail, root raised
//! cosine (roll-off 0.5) throughout. Noise and impairments come from the standard `gen_*` settings.

use super::message::{self, Header};
use super::rx::{known_symbols, FOLLOW_RATE, KNOWN, SPS, SYMBOL_RATE};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::rrc;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo};

sdrkit_core::choice_enum! {
    /// The scene of the test signal.
    pub enum GenScene {
        /// Platform A (a SYNOP text, every 12 s) and platform B (900 binary bytes, every 20 s).
        Two = ("0", "Two platforms"),
        /// Platform A alone.
        One = ("1", "Platform A alone"),
    }
}

/// The scene of the test signal.
pub const GEN_SCENE: Setting<field::Choice<GenScene>> =
    Setting::choice("gen_scene", "Scene", GenScene::Two)
        .section("Test signal")
        .advanced();

/// The SYNOP text of platform A.
pub const SYNOP: &[u8] = b"SYNOP 06590 32970 10125 20087 39982 40213 58004=\r\n";
/// The carrier before the preamble, s.
pub const CARRIER_S: f64 = 2.0;
/// Tail bytes after the coded frames.
const TAIL_BYTES: usize = 3;

/// One platform of the scene.
#[derive(Clone, Debug, PartialEq)]
pub struct Platform {
    /// Its carrier, Hz from the dial.
    pub offset_hz: f64,
    /// The 21 data bits of its address.
    pub address21: u32,
    /// The sequence number of its first message (+1 each repeat).
    pub sequence: u16,
    /// Alert messages (else self-timed).
    pub alert: bool,
    /// The health bits.
    pub health: u16,
    /// The platform data.
    pub data: Vec<u8>,
    /// The start of its first message, s.
    pub start_s: f64,
    /// The time from one message to the next, s.
    pub period_s: f64,
    /// Its amplitude.
    pub amplitude: f32,
}

impl Platform {
    /// Platform A: address data 2C412, the SYNOP text, every 12 s from 0.2 s, 5.5 kHz below the dial.
    pub fn a() -> Platform {
        Platform {
            offset_hz: -5.5e3,
            address21: 0x2C412,
            sequence: 1,
            alert: false,
            health: 0x155,
            data: SYNOP.to_vec(),
            start_s: 0.2,
            period_s: 12.0,
            amplitude: 1.0,
        }
    }

    /// Platform B: address data 5800A, 900 bytes (byte i = (37 i + 11) mod 256), every 20 s from 2 s, 24.5 kHz above the dial.
    pub fn b() -> Platform {
        Platform {
            offset_hz: 24.5e3,
            address21: 0x5800A,
            sequence: 400,
            alert: false,
            health: 0x155,
            data: (0..900).map(|i| ((37 * i + 11) % 256) as u8).collect(),
            start_s: 2.0,
            period_s: 20.0,
            amplitude: 1.0,
        }
    }

    /// The baseband of its message number `k` at the follower rate, unit carrier amplitude.
    pub fn baseband(&self, k: u32) -> Vec<Complex32> {
        let h = Header {
            address21: self.address21,
            length: self.data.len() as u16,
            sequence: self.sequence.wrapping_add(k as u16),
            alert: self.alert,
            compression: 0,
            health: self.health,
        };
        let mut coded = message::coded(&message::message(&h, &self.data));
        coded.extend(std::iter::repeat_n(message::TAIL, TAIL_BYTES - 1));
        let sym = message::code_symbols(&coded);
        baseband(&sym)
    }
}

/// The baseband of a transmission whose code symbols are `sym` (G1, G2 inverted, ...): carrier, known symbols, offset QPSK.
pub fn baseband(sym: &[u8]) -> Vec<Complex32> {
    let carrier = (CARRIER_S * SYMBOL_RATE) as usize;
    let data = sym.len() / 2;
    let total = carrier + KNOWN + data + 12;
    let mut i_imp = vec![0f32; total * SPS];
    let mut q_imp = vec![0f32; total * SPS];
    for k in 0..carrier {
        i_imp[k * SPS] = 1.0;
        q_imp[k * SPS] = 1.0;
    }
    for (k, &s) in known_symbols().iter().enumerate() {
        i_imp[(carrier + k) * SPS] = s;
        q_imp[(carrier + k) * SPS] = s;
    }
    let level = |b: u8| if b == 0 { 1.0 } else { -1.0 };
    for k in 0..data {
        let at = (carrier + KNOWN + k) * SPS;
        i_imp[at] = level(sym[2 * k]);
        q_imp[at + SPS / 2] = level(sym[2 * k + 1]);
    }
    // Root raised cosine, ±5 symbols, scaled so that the carrier has amplitude 1 on each rail.
    let half = 5 * SPS as i64;
    let mut taps: Vec<f32> = (-half..=half)
        .map(|i| rrc(i as f64 / SPS as f64, 0.5) as f32)
        .collect();
    let dc: f32 = taps.iter().sum::<f32>() / SPS as f32;
    for t in &mut taps {
        *t /= dc;
    }
    let n = i_imp.len();
    let k = std::f32::consts::FRAC_1_SQRT_2;
    let mut out = vec![Complex32::default(); n];
    for (j, &tap) in taps.iter().enumerate() {
        let shift = j as i64 - half;
        for idx in 0..n {
            let src = idx as i64 - shift;
            if src < 0 || src >= n as i64 {
                continue;
            }
            let (a, b) = (i_imp[src as usize], q_imp[src as usize]);
            if a != 0.0 || b != 0.0 {
                out[idx] += Complex32::new(a, b) * (tap * k);
            }
        }
    }
    out
}

/// A platform's oscillator, the message being sent (its baseband and start sample) and the number of its next message.
type PlatformState = (Nco, Option<(Vec<Complex32>, u64)>, u32);

/// The test signal generator.
pub struct HrdcpGenerator {
    rate: f64,
    platforms: Vec<Platform>,
    /// Per platform: its oscillator, the message being sent (baseband and next message number) and its start sample.
    state: Vec<PlatformState>,
    n: u64,
}

impl HrdcpGenerator {
    /// The generator of a stack configuration.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let scene = match cfg.settings.get(&GEN_SCENE) {
            GenScene::Two => vec![Platform::a(), Platform::b()],
            GenScene::One => vec![Platform::a()],
        };
        Self::new(cfg.rate_hz, scene)
    }

    /// A generator at `rate` of the platforms `platforms`.
    pub fn new(rate: f64, platforms: Vec<Platform>) -> Self {
        let state = platforms
            .iter()
            .map(|p| (Nco::new(rate, p.offset_hz), None, 0))
            .collect();
        HrdcpGenerator {
            rate,
            platforms,
            state,
            n: 0,
        }
    }
}

impl Generator for HrdcpGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let ratio = FOLLOW_RATE / self.rate;
        for o in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let mut acc = Complex32::default();
            for (p, (nco, msg, next)) in self.platforms.iter().zip(self.state.iter_mut()) {
                let c = nco.next_sample();
                if msg.is_none() {
                    let due = p.start_s + f64::from(*next) * p.period_s;
                    if t >= due {
                        *msg = Some((p.baseband(*next), self.n));
                        *next += 1;
                    }
                }
                let Some((bb, start)) = msg.as_ref() else {
                    continue;
                };
                let pos = (self.n - start) as f64 * ratio;
                let i = pos.floor() as usize;
                if i + 1 >= bb.len() {
                    *msg = None;
                    continue;
                }
                let f = (pos - i as f64) as f32;
                acc += (bb[i] * (1.0 - f) + bb[i + 1] * f) * c * p.amplitude;
            }
            *o = acc;
            self.n += 1;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // Eb/N0 for one platform's 1200 bit/s, its carrier at amplitude 1.
        Some(
            SignalInfo::new(2250.0)
                .with_symbol_rate(SYMBOL_RATE)
                .with_bit_rate(SYMBOL_RATE),
        )
    }
}
