//! The FM monitor test signal: a multiplex of two audio tones, the 19 kHz pilot, an L-R tone on 38 kHz, a random RDS
//! stream on 57 kHz and two optional subcarriers at 67 and 92 kHz, frequency modulated on a carrier at the centre. Noise,
//! the carrier offset and its drift come from the standard impairments of the test signal.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

/// The section of the test signal settings.
const SECTION: &str = "Test signal";

macro_rules! khz {
    ($(#[$m:meta])* $name:ident = ($key:literal, $label:literal, $default:expr)) => {
        $(#[$m])*
        pub const $name: Setting<field::F64> = Setting::new($key, $label, "kHz", $default)
            .range(0.0, 150.0, 0.0)
            .section(SECTION);
    };
}

khz!(
    /// The deviation of the 1 kHz audio tone.
    GEN_AUDIO_KHZ = ("gen_audio_khz", "Audio tone deviation", 30.0)
);
khz!(
    /// The deviation of the 3.3 kHz audio tone.
    GEN_TONE2_KHZ = ("gen_tone2_khz", "Second tone deviation", 12.0)
);
khz!(
    /// The pilot deviation.
    GEN_PILOT_KHZ = ("gen_pilot_khz", "Pilot deviation", 7.5)
);
khz!(
    /// The deviation of the 2 kHz L-R tone on 38 kHz.
    GEN_STEREO_KHZ = ("gen_stereo_khz", "L-R tone deviation", 15.0)
);
khz!(
    /// The RDS deviation.
    GEN_RDS_KHZ = ("gen_rds_khz", "RDS deviation", 2.0)
);
khz!(
    /// The deviation of the 67 kHz subcarrier (0 for none).
    GEN_SCA67_KHZ = ("gen_sca67_khz", "67 kHz subcarrier deviation", 0.0)
);
khz!(
    /// The deviation of the 92 kHz subcarrier (0 for none).
    GEN_SCA92_KHZ = ("gen_sca92_khz", "92 kHz subcarrier deviation", 0.0)
);
/// The scale of the whole multiplex.
pub const GEN_SCALE: Setting<field::F64> = Setting::new("gen_scale", "Multiplex scale", "", 1.0)
    .range(0.0, 4.0, 0.0)
    .section(SECTION);
/// The pilot is off before this time.
pub const GEN_PILOT_FROM_S: Setting<field::F64> = Setting::new("gen_pilot_from_s", "Pilot from", "s", 0.0)
    .range(0.0, 1e6, 0.0)
    .section(SECTION);
/// The pilot is off from this time; a negative value keeps it on.
pub const GEN_PILOT_UNTIL_S: Setting<field::F64> = Setting::new("gen_pilot_until_s", "Pilot until", "s", -1.0)
    .range(-1.0, 1e6, 0.0)
    .section(SECTION)
    .help("A negative value keeps the pilot on.");
/// RDS is off before this time.
pub const GEN_RDS_FROM_S: Setting<field::F64> = Setting::new("gen_rds_from_s", "RDS from", "s", 0.0)
    .range(0.0, 1e6, 0.0)
    .section(SECTION);
/// RDS is off from this time; a negative value keeps it on.
pub const GEN_RDS_UNTIL_S: Setting<field::F64> = Setting::new("gen_rds_until_s", "RDS until", "s", -1.0)
    .range(-1.0, 1e6, 0.0)
    .section(SECTION)
    .help("A negative value keeps RDS on.");
/// The subcarriers are off before this time.
pub const GEN_SCA_FROM_S: Setting<field::F64> = Setting::new("gen_sca_from_s", "Subcarriers from", "s", 0.0)
    .range(0.0, 1e6, 0.0)
    .section(SECTION);

/// The test multiplex. Deviations are in Hz.
#[derive(Clone, Debug, PartialEq)]
pub struct Mpx {
    /// The 1 kHz and 3.3 kHz audio tones.
    pub audio: (f64, f64),
    /// The pilot.
    pub pilot: f64,
    /// The L-R tone.
    pub stereo: f64,
    /// RDS.
    pub rds: f64,
    /// The 67 kHz and 92 kHz subcarriers.
    pub sca: (f64, f64),
    /// The scale of the whole multiplex.
    pub scale: f64,
    /// The pilot is on from, until (s; `None` for always).
    pub pilot_on: (f64, Option<f64>),
    /// RDS is on from, until.
    pub rds_on: (f64, Option<f64>),
    /// The subcarriers are on from.
    pub sca_from: f64,
}

impl Default for Mpx {
    fn default() -> Self {
        Mpx {
            audio: (30e3, 12e3),
            pilot: 7.5e3,
            stereo: 15e3,
            rds: 2e3,
            sca: (0.0, 0.0),
            scale: 1.0,
            pilot_on: (0.0, None),
            rds_on: (0.0, None),
            sca_from: 0.0,
        }
    }
}

impl Mpx {
    /// The multiplex that the settings `v` give.
    pub fn from_values(v: &Values) -> Mpx {
        let until = |s: &Setting<field::F64>| Some(v.get(s)).filter(|&t| t >= 0.0);
        Mpx {
            audio: (1e3 * v.get(&GEN_AUDIO_KHZ), 1e3 * v.get(&GEN_TONE2_KHZ)),
            pilot: 1e3 * v.get(&GEN_PILOT_KHZ),
            stereo: 1e3 * v.get(&GEN_STEREO_KHZ),
            rds: 1e3 * v.get(&GEN_RDS_KHZ),
            sca: (1e3 * v.get(&GEN_SCA67_KHZ), 1e3 * v.get(&GEN_SCA92_KHZ)),
            scale: v.get(&GEN_SCALE),
            pilot_on: (v.get(&GEN_PILOT_FROM_S), until(&GEN_PILOT_UNTIL_S)),
            rds_on: (v.get(&GEN_RDS_FROM_S), until(&GEN_RDS_UNTIL_S)),
            sca_from: v.get(&GEN_SCA_FROM_S),
        }
    }
}

fn on(t: f64, (from, until): (f64, Option<f64>)) -> bool {
    t >= from && until.is_none_or(|u| t < u)
}

/// Makes the test signal at a rate.
#[derive(Clone, Debug)]
pub struct FmGenerator {
    rate: f64,
    mpx: Mpx,
    rng: Rng,
    n: u64,
    symbol: i64,
    level: f64,
    phase: f64,
    peak: f64,
}

/// The RDS symbol rate, symbols per second (IEC 62106).
pub const RDS_RATE: f64 = 1187.5;

impl FmGenerator {
    /// The multiplex `mpx` at `rate_hz`, the RDS stream from `seed`.
    pub fn new(rate_hz: f64, mpx: Mpx, seed: u64) -> FmGenerator {
        FmGenerator {
            rate: rate_hz,
            mpx,
            rng: Rng::new(seed ^ 0x464d_4d4f),
            n: 0,
            symbol: -1,
            level: 1.0,
            phase: 0.0,
            peak: 0.0,
        }
    }

    /// The largest |MPX| produced so far, Hz.
    pub fn peak_hz(&self) -> f64 {
        self.peak
    }

    /// The multiplex at time `t`, Hz.
    fn mpx_at(&mut self, t: f64) -> f64 {
        let m = &self.mpx;
        let sym = (t * RDS_RATE).floor() as i64;
        if sym != self.symbol {
            self.symbol = sym;
            self.level = if self.rng.uniform() < 0.5 { -1.0 } else { 1.0 };
        }
        let mut v = m.audio.0 * (TAU * 1000.0 * t).sin()
            + m.audio.1 * (TAU * 3300.0 * t + 1.0).sin()
            + m.stereo * (TAU * 2000.0 * t).sin() * (TAU * 38_000.0 * t).sin();
        if on(t, m.pilot_on) {
            v += m.pilot * (TAU * 19_000.0 * t).sin();
        }
        if on(t, m.rds_on) {
            v += m.rds * self.level * (TAU * 57_000.0 * t).sin();
        }
        if t >= m.sca_from {
            v += m.sca.0 * (TAU * 67_000.0 * t + 0.4 * (TAU * 600.0 * t).sin()).sin()
                + m.sca.1 * (TAU * 92_000.0 * t + 0.4 * (TAU * 450.0 * t).sin()).sin();
        }
        v * m.scale
    }
}

impl Generator for FmGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            let f = self.mpx_at(t);
            self.peak = self.peak.max(f.abs());
            *z = Complex32::new(self.phase.cos() as f32, self.phase.sin() as f32);
            self.phase = (self.phase + TAU * f / self.rate).rem_euclid(TAU);
        }
        out.len()
    }
    /// A carrier of amplitude 1; the noise of the impairments is set against it in 200 kHz.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(200e3).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
