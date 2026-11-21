//! The test signal of `spur`: signals at stated offsets whose levels swing, through a radio with an IQ mismatch of a stated
//! image rejection and image phase (the mixer of `sdrkit-algo-sim::front`), a DC offset, the square and the cube of the
//! first signal added with stated weights, and a fixed tone; an optional steady real signal. The shift moves the signals,
//! not the faults of the radio. The noise is that of the standard impairments.

use crate::ptt_log::gen::parse_spans;
use sdrkit_algo_sim::front::IqImbalance;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The signals.
pub const GEN_SIGNALS: Setting<field::Text> =
    Setting::new("gen_signals", "Signals", "", "130000, 0.2; -210000, 0.2")
        .section(SECTION)
        .help("Entries separated by ';', each 'offset Hz, amplitude'.");
/// Their swing.
pub const GEN_SWING_DB: Setting<field::F64> =
    Setting::new("gen_swing_db", "Level swing", "dB", 5.0)
        .range(0.0, 20.0, 0.0)
        .section(SECTION);
/// The image rejection of the radio (0: no mismatch).
pub const GEN_IMAGE_DB: Setting<field::F64> =
    Setting::new("gen_image_db", "Image rejection", "dB", 34.0)
        .range(0.0, 120.0, 0.0)
        .section(SECTION);
/// The phase of the image ratio.
pub const GEN_IMAGE_PHASE: Setting<field::F64> =
    Setting::new("gen_image_phase", "Image phase", "rad", 0.7)
        .range(-3.2, 3.2, 0.0)
        .section(SECTION);
/// The DC offset of the radio.
pub const GEN_DC: Setting<field::F64> = Setting::new("gen_dc", "DC offset", "", 0.01)
    .range(0.0, 1.0, 0.0)
    .section(SECTION);
/// The weight of the square of the first signal.
pub const GEN_H2: Setting<field::F64> = Setting::new("gen_h2", "Second-order term", "", 0.5)
    .range(0.0, 10.0, 0.0)
    .section(SECTION);
/// The weight of its cube.
pub const GEN_H3: Setting<field::F64> = Setting::new("gen_h3", "Third-order term", "", 1.5)
    .range(0.0, 10.0, 0.0)
    .section(SECTION);
/// The fixed tone.
pub const GEN_SPUR_HZ: Setting<field::F64> =
    Setting::new("gen_spur_hz", "Fixed tone", "Hz", 250_000.0)
        .range(-1e7, 1e7, 0.0)
        .section(SECTION);
/// Its amplitude.
pub const GEN_SPUR_AMP: Setting<field::F64> =
    Setting::new("gen_spur_amp", "Fixed tone amplitude", "", 0.005)
        .range(0.0, 1.0, 0.0)
        .section(SECTION);
/// A steady real signal.
pub const GEN_REAL_HZ: Setting<field::F64> = Setting::new("gen_real_hz", "Real signal", "Hz", 0.0)
    .range(-1e7, 1e7, 0.0)
    .section(SECTION);
/// Its amplitude (0: none).
pub const GEN_REAL_AMP: Setting<field::F64> =
    Setting::new("gen_real_amp", "Real signal amplitude", "", 0.0)
        .range(0.0, 1.0, 0.0)
        .section(SECTION);
/// The shift of the signals.
pub const GEN_SHIFT_HZ: Setting<field::F64> =
    Setting::new("gen_shift_hz", "Shift of the signals", "Hz", 0.0)
        .range(-1e6, 1e6, 0.0)
        .section(SECTION);

/// The swing rates and phases of the signals, in turn.
const SWINGS: [(f64, f64); 2] = [(0.067, 0.0), (0.101, 2.0)];

/// The Q gain (dB) and phase (degrees) of the mixer of `front` whose image ratio is `beta`: `z = g e^(jp)` solves
/// `z + beta conj(z) = 1 - beta`.
pub fn mismatch_of(beta: Complex32) -> (f64, f64) {
    let (br, bi) = (f64::from(beta.re), f64::from(beta.im));
    let det = 1.0 - br * br - bi * bi;
    let a = ((1.0 - br).powi(2) + bi * bi) / det;
    let b = -2.0 * bi / det;
    (20.0 * a.hypot(b).log10(), b.atan2(a).to_degrees())
}

/// The test signal generator.
pub struct SpurGenerator {
    rate: f64,
    signals: Vec<(f64, f64)>,
    swing: f64,
    iq: Option<IqImbalance>,
    dc: f32,
    h2: f32,
    h3: f32,
    spur: (f64, f64),
    real: (f64, f64),
    buf: Vec<Complex32>,
    n: u64,
}

impl SpurGenerator {
    /// The signal of the settings `v` at `rate_hz`.
    pub fn new(rate_hz: f64, v: &Values) -> SpurGenerator {
        let shift = v.get(&GEN_SHIFT_HZ);
        let img = v.get(&GEN_IMAGE_DB);
        let iq = (img > 0.0).then(|| {
            let beta = Complex32::from_polar(
                10f64.powf(-img / 20.0) as f32,
                v.get(&GEN_IMAGE_PHASE) as f32,
            );
            let (g, p) = mismatch_of(beta);
            IqImbalance::new(g, p).ok()
        });
        SpurGenerator {
            rate: rate_hz,
            signals: parse_spans(v.get(&GEN_SIGNALS))
                .unwrap_or_default()
                .into_iter()
                .map(|(f, a)| (f + shift, a))
                .collect(),
            swing: v.get(&GEN_SWING_DB),
            iq: iq.flatten(),
            dc: v.get(&GEN_DC) as f32,
            h2: v.get(&GEN_H2) as f32,
            h3: v.get(&GEN_H3) as f32,
            spur: (v.get(&GEN_SPUR_HZ), v.get(&GEN_SPUR_AMP)),
            real: (v.get(&GEN_REAL_HZ) + shift, v.get(&GEN_REAL_AMP)),
            buf: Vec::new(),
            n: 0,
        }
    }
}

fn tone(f: f64, a: f64, t: f64) -> Complex32 {
    Complex32::from_polar(a as f32, (TAU * f * t).rem_euclid(TAU) as f32)
}

impl Generator for SpurGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let n0 = self.n;
        self.buf.clear();
        let mut firsts = Vec::with_capacity(out.len());
        for k in 0..out.len() as u64 {
            let t = (n0 + k) as f64 / self.rate;
            let mut s = Complex32::default();
            for (j, &(f, a)) in self.signals.iter().enumerate() {
                let (r, ph) = SWINGS[j % SWINGS.len()];
                let a = a * 10f64.powf(self.swing * (TAU * r * t + ph).sin() / 20.0);
                let e = tone(f, a, t);
                if j == 0 {
                    firsts.push(e);
                }
                s += e;
            }
            if self.signals.is_empty() {
                firsts.push(Complex32::default());
            }
            if self.real.1 > 0.0 {
                s += tone(self.real.0, self.real.1, t);
            }
            self.buf.push(s);
        }
        if let Some(iq) = &self.iq {
            iq.process(&mut self.buf);
        }
        for (k, ((z, b), e)) in out.iter_mut().zip(&self.buf).zip(&firsts).enumerate() {
            let t = (n0 + k as u64) as f64 / self.rate;
            let e2 = e * e;
            *z = b
                + Complex32::new(self.dc, 0.0)
                + e2 * self.h2
                + e2 * e * self.h3
                + tone(self.spur.0, self.spur.1, t);
        }
        self.n += out.len() as u64;
        out.len()
    }
    /// The noise of the impairments is stated against a power of 1.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.rate))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
