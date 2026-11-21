//! The COSPAS-SARSAT test signal: a loop of made-up beacon bursts at the dial, with silence between them.
//!
//! Standard: C/S T.001 section 2.2 (first generation), C/S T.018 section 2.2 (second generation).

use super::{t001, t018, SGB_OFFSET_HZ};
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

/// The phase deviation of the first-generation bi-phase L modulation, radians (C/S T.001 section 2.2.3).
const FGB_DEVIATION_RAD: f64 = 1.1;
/// The unmodulated carrier before a first-generation message, seconds (C/S T.001 section 2.2.4).
const FGB_CARRIER_S: f64 = 0.160;

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";
/// The test signal sends second-generation beacons.
pub const GEN_SECOND_GENERATION: Setting<field::Bool> =
    Setting::new("gen_second_generation", "Second generation", "", false)
        .section(TEST_SIGNAL)
        .advanced();
/// The test beacon sent (0: every one in turn).
pub const GEN_BEACON: Setting<field::I64> = Setting::new("gen_beacon", "Test beacon", "", 0)
    .range(0.0, 5.0, 1.0)
    .help("0 sends every test beacon in turn.")
    .section(TEST_SIGNAL)
    .advanced();
/// The test signal reverses the phase polarity.
pub const GEN_REVERSE: Setting<field::Bool> =
    Setting::new("gen_reverse", "Reversed phase polarity", "", false)
        .section(TEST_SIGNAL)
        .advanced();
/// The first-generation bit rate error.
pub const GEN_BITRATE_PCT: Setting<field::F64> =
    Setting::new("gen_bitrate_pct", "Bit rate error", "%", 0.0)
        .range(-2.0, 2.0, 0.0)
        .help("The first-generation bit rate away from 400 bit/s (T.001 allows 1 %).")
        .section(TEST_SIGNAL)
        .advanced();
/// The silence between bursts.
pub const GEN_GAP_S: Setting<field::Duration> =
    Setting::new("gen_gap_s", "Seconds between bursts", "s", 3.0)
        .range(0.2, 60.0, 0.0)
        .section(TEST_SIGNAL)
        .advanced();

/// Options of the generator.
#[derive(Clone, Debug)]
pub struct GenParams {
    /// The sample rate, Hz.
    pub rate: f64,
    /// Send second-generation bursts.
    pub second_generation: bool,
    /// The test beacon to send (1 based); 0 sends all in turn.
    pub beacon: usize,
    /// Reverse the phase polarity of first-generation bursts.
    pub reverse: bool,
    /// The first-generation bit rate error, percent.
    pub bitrate_pct: f64,
    /// Silence between bursts, seconds.
    pub gap_s: f64,
}

impl GenParams {
    /// The defaults at `rate`.
    pub fn new(rate: f64) -> Self {
        GenParams {
            rate,
            second_generation: false,
            beacon: 0,
            reverse: false,
            bitrate_pct: 0.0,
            gap_s: 3.0,
        }
    }

    /// The options of a configuration (a setting that is missing keeps its default).
    pub fn from_values(rate: f64, s: &Values) -> Self {
        let mut p = GenParams::new(rate);
        p.second_generation = s.get(&GEN_SECOND_GENERATION);
        p.beacon = usize::try_from(s.get(&GEN_BEACON)).unwrap_or(0);
        p.reverse = s.get(&GEN_REVERSE);
        p.bitrate_pct = s.get(&GEN_BITRATE_PCT);
        p.gap_s = s.get(&GEN_GAP_S);
        p
    }
}

/// The generator. It makes unit power while a burst is sent.
pub struct SarsatGenerator {
    p: GenParams,
    burst: Vec<Complex32>,
    pos: usize,
    gap_left: usize,
    next: usize,
    phase: f64,
    /// The identity of each burst sent, in order (15 hex digits, or 23 for the second generation).
    pub sent: Vec<String>,
}

impl SarsatGenerator {
    /// The generator of a stack configuration.
    pub fn from_config(cfg: &GenConfig) -> Self {
        Self::new(GenParams::from_values(cfg.rate_hz, &cfg.settings))
    }

    /// The generator with these options.
    pub fn new(p: GenParams) -> Self {
        SarsatGenerator {
            burst: Vec::new(),
            pos: 0,
            gap_left: (p.gap_s * p.rate) as usize,
            next: 0,
            phase: 0.0,
            sent: Vec::new(),
            p,
        }
    }

    fn pick(&mut self, n: usize) -> usize {
        let i = if self.p.beacon == 0 {
            self.next % n
        } else {
            (self.p.beacon - 1).min(n - 1)
        };
        self.next += 1;
        i
    }

    fn make_burst(&mut self) {
        let fs = self.p.rate;
        if self.p.second_generation {
            let list = t018::test_beacons();
            let i = self.pick(list.len());
            let bits = t018::encode(&list[i]);
            self.sent.push(
                t018::decode(&bits, &mut t018::Code::new())
                    .map(|d| d.hex_id_23)
                    .unwrap_or_default(),
            );
            self.burst = sgb_wave(&bits, fs);
            return;
        }
        let list = t001::test_beacons();
        let i = self.pick(list.len());
        let bits = t001::encode(&list[i]);
        self.sent.push(
            t001::decode(&bits, &mut t001::Codes::new())
                .map(|d| d.hex_id)
                .unwrap_or_default(),
        );
        let carrier = (FGB_CARRIER_S * fs) as usize;
        let tb = fs / (400.0 * (1.0 + self.p.bitrate_pct / 100.0));
        let n = carrier + (bits.len() as f64 * tb).ceil() as usize;
        let pol = if self.p.reverse { -1.0 } else { 1.0 };
        self.burst = (0..n)
            .map(|k| {
                let ph: f64 = if k < carrier {
                    0.0
                } else {
                    let t = (k - carrier) as f64 / tb;
                    let bi = (t.floor() as usize).min(bits.len() - 1);
                    let first_half = t.fract() < 0.5;
                    // Bi-phase L: a 1 is +1.1 then -1.1 rad, a 0 the reverse.
                    let s = if bits[bi] == first_half {
                        FGB_DEVIATION_RAD
                    } else {
                        -FGB_DEVIATION_RAD
                    };
                    s * pol
                };
                Complex32::new(ph.cos() as f32, ph.sin() as f32)
            })
            .collect();
    }
}

/// The second-generation burst at `rate`: the 150 bits of each arm spread by its PRN, OQPSK with rectangular chips, unit power.
///
/// Standard: C/S T.018 section 2.2 (odd message bits on I, even on Q, the Q arm half a chip late).
pub fn sgb_wave(bits250: &[bool], rate: f64) -> Vec<Complex32> {
    use t018::{BITS_PER_ARM, CHIPS_PER_BIT, PREAMBLE_BITS};
    let mut data_i = [false; BITS_PER_ARM];
    let mut data_q = [false; BITS_PER_ARM];
    for k in 0..125.min(bits250.len() / 2) {
        data_i[PREAMBLE_BITS + k] = bits250[2 * k];
        data_q[PREAMBLE_BITS + k] = bits250[2 * k + 1];
    }
    let total = BITS_PER_ARM * CHIPS_PER_BIT;
    let lvl = |q: bool, c: usize, b: bool| -> f32 {
        let chip = if t018::prn(q)[c] == 0 { 1.0 } else { -1.0 };
        if b {
            -chip
        } else {
            chip
        }
    };
    let s = std::f32::consts::FRAC_1_SQRT_2;
    let chips: Vec<Complex32> = (0..total)
        .map(|c| {
            Complex32::new(
                s * lvl(false, c, data_i[c / CHIPS_PER_BIT]),
                s * lvl(true, c, data_q[c / CHIPS_PER_BIT]),
            )
        })
        .collect();
    let mut out = Vec::with_capacity(((total + 2) as f64 * rate / t018::CHIP_RATE) as usize);
    // A rectangular pulse of one chip at a positive rate is always valid.
    if let Ok(m) = Linear::new(Shape::Rect, 1.0, rate / t018::CHIP_RATE) {
        let mut m = m.with_offset_q();
        m.push(&chips, &mut out);
        m.flush(&mut out);
    }
    out
}

impl Generator for SarsatGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let f = if self.p.second_generation {
            SGB_OFFSET_HZ
        } else {
            0.0
        };
        let w = TAU * f / self.p.rate;
        for o in out.iter_mut() {
            let mut s = Complex32::new(0.0, 0.0);
            if self.gap_left > 0 {
                self.gap_left -= 1;
            } else {
                if self.burst.is_empty() {
                    self.make_burst();
                    self.pos = 0;
                }
                s = self.burst[self.pos];
                self.pos += 1;
                if self.pos >= self.burst.len() {
                    self.burst.clear();
                    self.gap_left = (self.p.gap_s * self.p.rate) as usize;
                }
            }
            let rot = Complex32::new(self.phase.cos() as f32, self.phase.sin() as f32);
            self.phase = (self.phase + w) % TAU;
            *o = s * rot;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(if self.p.second_generation {
            // 38 400 chips per second on each arm, 300 bit/s of message and parity.
            SignalInfo::new(2.0 * t018::CHIP_RATE)
                .with_symbol_rate(t018::CHIP_RATE)
                .with_bit_rate(300.0)
        } else {
            // C/N in 2.4 kHz. 400 bit/s.
            SignalInfo::new(2400.0)
                .with_symbol_rate(400.0)
                .with_bit_rate(400.0)
        })
    }
}
