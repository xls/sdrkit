//! The test signal of `papr`: complex Gaussian noise, an FM carrier, QPSK with root-raised-cosine pulses, noise with its
//! magnitude limited, or noise with an impulse on every n-th sample, of a stated power. The noise of these signals is the
//! measured signal itself; the standard impairments add none unless asked.

use crate::txtest::FmTones;
use crate::typed::PaprGenKind;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::{FRAC_1_SQRT_2, TAU};

const SECTION: &str = "Test signal";

/// The signal.
pub const GEN_KIND: Setting<field::Choice<PaprGenKind>> =
    Setting::choice("gen_kind", "Signal", PaprGenKind::Noise).section(SECTION);
/// The mean power of the noise, the carrier or the symbols.
pub const GEN_POWER: Setting<field::F64> = Setting::new("gen_power", "Power", "", 0.01)
    .range(1e-8, 4.0, 0.0)
    .section(SECTION);
/// The magnitude limit of clipped noise.
pub const GEN_CLIP: Setting<field::F64> = Setting::new("gen_clip", "Magnitude limit", "", 1.0)
    .range(1e-4, 2.0, 0.0)
    .section(SECTION);
/// An impulse on every n-th sample.
pub const GEN_IMPULSE_EVERY: Setting<field::I64> =
    Setting::new("gen_impulse_every", "Impulse every", "samples", 500)
        .range(2.0, 1e7, 1.0)
        .section(SECTION);
/// The value added by an impulse.
pub const GEN_IMPULSE_AMP: Setting<field::F64> =
    Setting::new("gen_impulse_amp", "Impulse size", "", 2.0)
        .range(0.0, 10.0, 0.0)
        .section(SECTION);
/// The QPSK symbol rate.
pub const GEN_SYMBOL_RATE: Setting<field::F64> =
    Setting::new("gen_symbol_rate", "Symbol rate", "Bd", 25_000.0)
        .range(100.0, 1e6, 0.0)
        .section(SECTION);
/// The QPSK roll-off.
pub const GEN_ROLLOFF: Setting<field::F64> = Setting::new("gen_rolloff", "Roll-off", "", 0.35)
    .range(0.05, 1.0, 0.0)
    .section(SECTION);

/// The FM carrier: 30 kHz plus 20 kHz times a 440 Hz sine.
const FM_CENTRE_HZ: f64 = 30e3;
const FM_DEV_HZ: f64 = 20e3;
const FM_TONE_HZ: f64 = 440.0;

/// The test signal generator.
pub struct PaprGenerator {
    rate: f64,
    kind: PaprGenKind,
    power: f64,
    clip: f64,
    every: u64,
    impulse: f64,
    rng: Rng,
    fm: FmTones,
    qpsk: Option<Linear>,
    sps: f64,
    queue: Vec<Complex32>,
    n: u64,
}

impl PaprGenerator {
    /// The signal of the settings `v` at `rate_hz`, its noise drawn from `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> PaprGenerator {
        let sps = rate_hz / v.get(&GEN_SYMBOL_RATE);
        PaprGenerator {
            rate: rate_hz,
            kind: v.get(&GEN_KIND),
            power: v.get(&GEN_POWER),
            clip: v.get(&GEN_CLIP),
            every: v.get(&GEN_IMPULSE_EVERY).max(1) as u64,
            impulse: v.get(&GEN_IMPULSE_AMP),
            rng: Rng::new(seed ^ 0x7061_7072),
            fm: FmTones::default(),
            qpsk: Linear::rrc(v.get(&GEN_ROLLOFF), 12.0, sps).ok(),
            sps,
            queue: Vec::new(),
            n: 0,
        }
    }

    fn qpsk(&mut self, out: &mut [Complex32]) {
        let Some(m) = self.qpsk.as_mut() else {
            out.fill(Complex32::default());
            return;
        };
        let a = (self.power * self.sps).sqrt() * FRAC_1_SQRT_2;
        while self.queue.len() < out.len() {
            let sym: Vec<Complex32> = (0..64)
                .map(|_| {
                    let b = self.rng.below(4);
                    let i = if b & 1 == 0 { a } else { -a };
                    let q = if b & 2 == 0 { a } else { -a };
                    Complex32::new(i as f32, q as f32)
                })
                .collect();
            m.push(&sym, &mut self.queue);
        }
        out.copy_from_slice(&self.queue[..out.len()]);
        self.queue.drain(..out.len());
    }
}

impl Generator for PaprGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        if self.kind == PaprGenKind::Qpsk {
            self.qpsk(out);
            self.n += out.len() as u64;
            return out.len();
        }
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            *z = match self.kind {
                PaprGenKind::Fm => {
                    let f = FM_CENTRE_HZ + FM_DEV_HZ * (TAU * FM_TONE_HZ * t).sin();
                    self.fm.next(f, self.rate, self.power.sqrt())
                }
                PaprGenKind::ClippedNoise => {
                    let x = self.rng.complex_normal(self.power);
                    let m = f64::from(x.norm());
                    if m > self.clip {
                        x * (self.clip / m) as f32
                    } else {
                        x
                    }
                }
                PaprGenKind::NoiseWithImpulses => {
                    let x = self.rng.complex_normal(self.power);
                    if (self.n + 1).is_multiple_of(self.every) {
                        x + Complex32::new(self.impulse as f32, 0.0)
                    } else {
                        x
                    }
                }
                _ => self.rng.complex_normal(self.power),
            };
            self.n += 1;
        }
        out.len()
    }
    /// The signal fills the span; it passes the impairments at its own power.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.rate))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
