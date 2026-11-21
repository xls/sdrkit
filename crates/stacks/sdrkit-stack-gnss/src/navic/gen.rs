//! The NavIC test signal: satellites on L5 with their ranging codes, Doppler and code phases, sending subframes with a rising
//! time of week count, IDs 1 to 4 in turn, the week number in subframe 1 and random data, below the noise of the sky as a
//! real signal is (45 dB-Hz for each satellite), at unit mean power. More noise comes from the standard `gen_*` settings.

use super::code::{bits_of, ranging_code, symbols, Band, Subframe, CODE_LEN, L5_HZ};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo};
use std::f64::consts::TAU;

/// Satellites of the test signal.
pub const GEN_SATS: Setting<field::I64> = Setting::new("gen_sats", "Satellites", "", 4)
    .range(1.0, 7.0, 1.0)
    .section("Test signal")
    .advanced();
/// The Doppler scale of the test signal.
pub const GEN_DOPPLER_HZ: Setting<field::F64> =
    Setting::new("gen_sat_doppler_hz", "Satellite Doppler", "Hz", 2500.0)
        .range(-9000.0, 9000.0, 1.0)
        .section("Test signal")
        .advanced()
        .help("Satellite i has this Doppler times 1, -0.5, 0.5, 0.75, 1, 1.25, 1.5.");

/// The PRNs of the test signal, in order.
pub const PRNS: [u16; 7] = [2, 5, 9, 3, 11, 7, 13];
/// The Doppler factors, in the order of [`PRNS`].
pub const DOPPLER: [f64; 7] = [1.0, -0.5, 0.5, 0.75, 1.0, 1.25, 1.5];
/// The week number the subframes 1 carry.
pub const WN: u16 = 1234;
/// The first time of week count (satellite i starts at this plus i).
pub const TOWC0: u32 = 5000;
/// The C/N0 of each satellite against the noise of the sky in the test signal, dB-Hz.
pub const SKY_CN0_DBHZ: f64 = 45.0;

/// One satellite of the test signal.
#[derive(Clone, Debug, PartialEq)]
pub struct GenSat {
    /// The PRN.
    pub prn: u16,
    /// The Doppler, Hz.
    pub doppler_hz: f64,
    /// The code phase at the start, chips.
    pub code_start: f64,
    /// The time of week count of the first subframe.
    pub towc: u32,
    /// The amplitude.
    pub amplitude: f32,
}

struct State {
    sat: GenSat,
    code: Vec<f32>,
    symbols: Vec<u8>,
    subframe: u64,
    rng: Rng,
}

impl State {
    fn subframe_symbols(&mut self) {
        let k = self.subframe;
        let id = (k % 4) as u8 + 1;
        let s = Subframe {
            towc: self.sat.towc + k as u32,
            id,
            wn: (id == 1).then_some(WN),
            message_id: (id >= 3).then_some(26),
            prn_id: (id >= 3).then_some(self.sat.prn as u8),
            ..Subframe::default()
        };
        let rng = &mut self.rng;
        let bits = bits_of(&s, || (rng.next_u32() & 1) as u8);
        self.symbols = symbols(&bits);
    }
}

/// The test signal generator.
pub struct NavicGenerator {
    rate: f64,
    sats: Vec<State>,
    n: u64,
    sky: Option<sdrkit_algo_sim::noise::Awgn>,
}

impl NavicGenerator {
    /// The generator of a stack configuration.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let n = usize::try_from(cfg.settings.get(&GEN_SATS)).unwrap_or(4);
        let d = cfg.settings.get(&GEN_DOPPLER_HZ);
        let sats: Vec<GenSat> = (0..n.min(PRNS.len()))
            .map(|i| GenSat {
                prn: PRNS[i],
                doppler_hz: d * DOPPLER[i],
                code_start: 37.5 * (i + 1) as f64,
                towc: TOWC0 + i as u32,
                amplitude: 1.0,
            })
            .collect();
        // Unit mean power: the satellites and the noise of the sky together. A satellite of power p has C/N0 p / N0.
        let noise_per_sat = cfg.rate_hz / 10f64.powf(SKY_CN0_DBHZ / 10.0);
        let p = 1.0 / (sats.len() as f64 + noise_per_sat);
        let sats: Vec<GenSat> = sats
            .into_iter()
            .map(|s| GenSat {
                amplitude: p.sqrt() as f32,
                ..s
            })
            .collect();
        let mut g = Self::new(cfg.rate_hz, sats, cfg.seed);
        g.sky =
            sdrkit_algo_sim::noise::Awgn::with_power(p * noise_per_sat, cfg.seed.wrapping_add(99))
                .ok();
        g
    }

    /// A generator at `rate` of the satellites `sats`, without the noise of the sky.
    pub fn new(rate: f64, sats: Vec<GenSat>, seed: u64) -> Self {
        let sats = sats
            .into_iter()
            .enumerate()
            .map(|(i, sat)| {
                let code = ranging_code(sat.prn, Band::L5)
                    .unwrap_or_else(|| vec![0; CODE_LEN])
                    .iter()
                    .map(|&c| if c == 0 { 1.0 } else { -1.0 })
                    .collect();
                let mut s = State {
                    sat,
                    code,
                    symbols: Vec::new(),
                    subframe: 0,
                    rng: Rng::new(seed.wrapping_add(17 * i as u64 + 1)),
                };
                s.subframe_symbols();
                s
            })
            .collect();
        NavicGenerator {
            rate,
            sats,
            n: 0,
            sky: None,
        }
    }
}

impl Generator for NavicGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let mut acc = Complex32::default();
            for s in &mut self.sats {
                let chips = s.sat.code_start + t * 1.023e6 * (1.0 + s.sat.doppler_hz / L5_HZ);
                let period = (chips / CODE_LEN as f64).floor() as u64;
                let chip = s.code[(chips as u64 % CODE_LEN as u64) as usize];
                // 20 code periods a symbol, 600 symbols a subframe.
                let sym = (period / 20) as usize;
                let sf = (sym / 600) as u64;
                if sf > s.subframe {
                    s.subframe = sf;
                    s.subframe_symbols();
                }
                let d = if s.symbols[sym % 600] == 0 { 1.0 } else { -1.0 };
                let ph = (TAU * (s.sat.doppler_hz * t).fract()) as f32;
                acc += Complex32::from_polar(s.sat.amplitude * chip * d, ph);
            }
            *o = acc;
            self.n += 1;
        }
        if let Some(s) = self.sky.as_mut() {
            s.process(out);
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The code main lobe and the symbol rate. With the sky the mean power is 1; without it each satellite has its
        // amplitude.
        Some(SignalInfo::new(2.046e6).with_symbol_rate(50.0))
    }
}
