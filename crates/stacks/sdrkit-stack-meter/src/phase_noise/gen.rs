//! The test signal of `phase_noise`: a carrier of amplitude 1 whose phase is a Wiener process of linewidth `dnu` (a random walk
//! of step variance `2 pi dnu / rate`, so `L(f) = dnu / (2 pi f^2)`) plus a white phase floor, optionally phase-modulated by a
//! sine (index `2 x 10^(dBc / 20)` for a spur of that level each side). The carrier is made at four times the rate and
//! filtered to the rate, as the anti-alias filter of a radio does, so the sidebands beyond half the rate do not fold into
//! the band and `L(f)` holds to 0.45 times the rate. The offset, the drift and the additive noise are the standard
//! impairments; an SNR is the CNR in 200 kHz.

use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_sim::phase_noise::PhaseNoise;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The linewidth of the random walk.
pub const GEN_LINEWIDTH_HZ: Setting<field::F64> =
    Setting::new("gen_linewidth_hz", "Linewidth", "Hz", 20.0)
        .range(0.0, 1e4, 0.0)
        .section(SECTION);
/// The white phase floor.
pub const GEN_FLOOR_DBC: Setting<field::F64> =
    Setting::new("gen_floor_dbc", "White phase floor", "dBc/Hz", -125.0)
        .range(-200.0, -40.0, 0.0)
        .section(SECTION);
/// The frequency of a spur (0 for none).
pub const GEN_SPUR_HZ: Setting<field::F64> = Setting::new("gen_spur_hz", "Spur offset", "Hz", 0.0)
    .range(0.0, 1e6, 0.0)
    .section(SECTION);
/// The level of the spur.
pub const GEN_SPUR_DBC: Setting<field::F64> =
    Setting::new("gen_spur_dbc", "Spur level", "dBc", -62.0)
        .range(-150.0, -10.0, 0.0)
        .section(SECTION);

/// The carrier is made at this many times the rate.
pub const OVERSAMPLE: usize = 4;

/// The band of a noise stated as an SNR, Hz.
pub const NOISE_BAND_HZ: f64 = 200e3;

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct PhaseNoiseGenerator {
    rate: f64,
    pn: Option<PhaseNoise>,
    dec: Option<FirDecimator<Complex32>>,
    fine: Vec<Complex32>,
    coarse: Vec<Complex32>,
    spur_hz: f64,
    beta: f64,
    n: u64,
}

impl PhaseNoiseGenerator {
    /// The signal of the settings `v` at `rate_hz` with the random stream of `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> PhaseNoiseGenerator {
        let lw = v.get(&GEN_LINEWIDTH_HZ);
        let floor = 10f64.powf(v.get(&GEN_FLOOR_DBC) / 10.0);
        let fine = rate_hz * OVERSAMPLE as f64;
        let pn = PhaseNoise::new(
            fine,
            (TAU * lw / fine).sqrt(),
            (floor * fine).sqrt(),
            seed ^ 0x5EED_0047,
        )
        .ok();
        PhaseNoiseGenerator {
            rate: rate_hz,
            pn,
            dec: FirDecimator::design(fine, OVERSAMPLE, 0.45 * rate_hz, 0.45 * rate_hz, 90.0).ok(),
            fine: Vec::new(),
            coarse: Vec::new(),
            spur_hz: v.get(&GEN_SPUR_HZ),
            beta: 2.0 * 10f64.powf(v.get(&GEN_SPUR_DBC) / 20.0),
            n: 0,
        }
    }
}

impl Generator for PhaseNoiseGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let fine_rate = self.rate * OVERSAMPLE as f64;
        self.fine.clear();
        for _ in 0..out.len() * OVERSAMPLE {
            let ph = if self.spur_hz > 0.0 {
                self.beta * (TAU * (self.spur_hz * self.n as f64 / fine_rate).fract()).sin()
            } else {
                0.0
            };
            self.fine.push(Complex32::from_polar(1.0, ph as f32));
            self.n += 1;
        }
        if let Some(pn) = self.pn.as_mut() {
            pn.process(&mut self.fine);
        }
        self.coarse.clear();
        match self.dec.as_mut() {
            Some(d) => d.process(&self.fine, &mut self.coarse),
            None => self.coarse.extend(self.fine.iter().step_by(OVERSAMPLE)),
        }
        let k = self.coarse.len().min(out.len());
        out[..k].copy_from_slice(&self.coarse[..k]);
        out[k..]
            .iter_mut()
            .for_each(|z| *z = Complex32::new(1.0, 0.0));
        out.len()
    }
    /// The noise is stated against a carrier of power 1; an SNR is stated in 200 kHz.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NOISE_BAND_HZ).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
