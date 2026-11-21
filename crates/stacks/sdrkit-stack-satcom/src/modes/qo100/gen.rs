//! The QO-100 test signal: the signal an exact LNB delivers to the radio. The LNB's error and drift, and the noise,
//! come from the standard impairments (`gen_offset_hz`, `gen_drift_hz_s`, `gen_noise`).

use super::{
    default_centre_rf, lo_of, LoChoice, BEACON_HIGH, BEACON_LOW, BEACON_PSK, DATV_BEACON, NB_HI,
};
use sdrkit_algo_dsp::converter::Converter;
use sdrkit_algo_dsp::fir::{lowpass, Fir};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo};
use std::collections::VecDeque;

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";
/// Send nothing but the noise of the impairments.
pub const GEN_EMPTY: Setting<field::Bool> = Setting::new("gen_empty", "No signals", "", false)
    .help("Send nothing: only the noise of the impairments.")
    .section(TEST_SIGNAL)
    .advanced();
/// The radio centre of the test signal (RF, 0: automatic).
pub const GEN_CENTRE_MHZ: Setting<field::F64> = Setting::new(
    "gen_centre_mhz",
    "Radio centre (RF, 0: automatic)",
    "MHz",
    0.0,
)
.range(0.0, 20000.0, 0.001)
.section(TEST_SIGNAL)
.advanced();
/// The nominal LO of the test signal.
pub const GEN_LO: Setting<field::Choice<LoChoice>> =
    Setting::choice("gen_lo", "Nominal LO", LoChoice::L9750)
        .section(TEST_SIGNAL)
        .advanced();

/// The test carrier (true RF).
pub const TEST_CARRIER: f64 = 10489.650e6;
/// The suppressed carriers of the SSB stations (true RF).
pub const SSB_STATIONS: [f64; 4] = [10489.540e6, 10489.600e6, 10489.820e6, 10489.905e6];
/// The audio tones of each SSB station, Hz above its carrier.
const SSB_TONES: [f64; 3] = [450.0, 900.0, 1700.0];
/// The narrowband carriers: (RF, amplitude, BPSK keyed).
const CARRIERS: [(f64, f32, bool); 4] = [
    (BEACON_LOW, 0.05, false),
    (BEACON_HIGH, 0.05, false),
    (TEST_CARRIER, 0.02, false),
    (BEACON_PSK, 0.04, true),
];
/// The amplitude of each SSB tone.
const TONE_AMPLITUDE: f32 = 0.01;
/// The mean of the squared syllable envelope `((1 + sin) / 2)^4`, times the share of time a station talks (3 of 4).
const SSB_POWER_SHARE: f64 = 0.2734375 * 0.75;

/// What the test signal is.
#[derive(Clone, Debug)]
pub struct GenParams {
    pub rate: f64,
    /// The LNB's nominal LO, Hz.
    pub lo: f64,
    /// The radio's centre (RF), Hz.
    pub centre_rf: f64,
    /// Send nothing.
    pub empty: bool,
    pub seed: u64,
}

impl GenParams {
    /// The test signal at `rate` with the 9750 MHz LO, the radio where the stack puts it.
    pub fn new(rate: f64) -> Self {
        GenParams {
            rate,
            lo: 9750e6,
            centre_rf: default_centre_rf(rate),
            empty: false,
            seed: 1,
        }
    }
    /// The radio's centre in the IF, Hz.
    pub fn centre_if(&self) -> f64 {
        self.converter().if_of(self.centre_rf)
    }
    /// The LNB of the LO `lo`.
    pub fn converter(&self) -> Converter {
        Converter::down(self.lo, NB_HI)
    }
}

/// A DATV carrier as flat-topped noise.
struct Wide {
    fir: Fir<Complex32>,
    rs: Resampler<Complex32>,
    buf: VecDeque<Complex32>,
    work: Vec<Complex32>,
    res: Vec<Complex32>,
    nco: Nco,
    gain: f32,
}

/// An SSB station: three tones with a syllable envelope.
struct Station {
    tones: [Nco; 3],
    syllable: Nco,
    talk_period_s: f64,
}

/// The QO-100 generator.
pub struct QoGenerator {
    p: GenParams,
    carriers: Vec<(Nco, f32, bool)>,
    stations: Vec<Station>,
    wides: Vec<Wide>,
    rng: Rng,
    psk_symbol: f32,
    psk_spb: u64,
    gain: f32,
    n: u64,
}

impl QoGenerator {
    /// The generator from the stack's test-signal settings.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let mut p = GenParams::new(cfg.rate_hz);
        p.empty = s.get(&GEN_EMPTY);
        let c = s.get(&GEN_CENTRE_MHZ);
        if c > 0.0 {
            p.centre_rf = c * 1e6;
        }
        p.lo = lo_of(s.get(&GEN_LO), 9750.0);
        p.seed = cfg.seed;
        Self::new(p)
    }

    /// The generator of `p`.
    pub fn new(p: GenParams) -> Self {
        let rate = p.rate;
        let centre_if = p.centre_if();
        let conv = p.converter();
        let off = |rf: f64| conv.if_of(rf) - centre_if;
        let carriers: Vec<(Nco, f32, bool)> = CARRIERS
            .iter()
            .map(|&(rf, a, psk)| (Nco::new(rate, off(rf)), a, psk))
            .collect();
        let stations = SSB_STATIONS
            .iter()
            .enumerate()
            .map(|(k, &rf)| Station {
                tones: SSB_TONES.map(|t| Nco::new(rate, off(rf + t))),
                syllable: Nco::new(rate, 3.0 + k as f64 * 0.7),
                talk_period_s: 6.0 + k as f64,
            })
            .collect();
        let mut power: f64 = CARRIERS.iter().map(|c| (c.1 as f64).powi(2)).sum();
        power += (SSB_STATIONS.len() * SSB_TONES.len()) as f64
            * (TONE_AMPLITUDE as f64).powi(2)
            * SSB_POWER_SHARE;
        let half = rate / 2.0 * 0.9;
        let mut wides = Vec::new();
        for (rf, width, gain) in [(DATV_BEACON, 2.0e6, 0.02f32), (10494.75e6, 0.45e6, 0.015)] {
            let f = off(rf);
            if f.abs() + width / 2.0 >= half {
                continue;
            }
            let wr = (width * 1.6).max(1e6);
            let taps = lowpass(wr, width / 2.0 * 0.9, width / 2.0 * 1.1, 40.0)
                .expect("the filter settings are valid");
            let Ok(rs) = Resampler::new(wr, rate) else {
                continue;
            };
            power += (gain as f64).powi(2) * taps.iter().map(|h| (*h as f64).powi(2)).sum::<f64>();
            wides.push(Wide {
                fir: Fir::new(&taps),
                rs,
                buf: VecDeque::new(),
                work: Vec::with_capacity(1024),
                res: Vec::new(),
                nco: Nco::new(rate, f),
                gain,
            });
        }
        QoGenerator {
            carriers,
            stations,
            wides,
            rng: Rng::new(p.seed),
            psk_symbol: 1.0,
            psk_spb: (rate / 400.0) as u64,
            gain: (1.0 / power.sqrt()) as f32,
            n: 0,
            p,
        }
    }
}

impl Generator for QoGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.p.rate))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        if self.p.empty {
            out.fill(Complex32::default());
            self.n += out.len() as u64;
            return out.len();
        }
        let rate = self.p.rate;
        for o in out.iter_mut() {
            let t = self.n as f64 / rate;
            if self.n.is_multiple_of(self.psk_spb) {
                self.psk_symbol = if self.rng.bit() == 1 { 1.0 } else { -1.0 };
            }
            let mut v = Complex32::default();
            for (nco, a, psk) in self.carriers.iter_mut() {
                let s = if *psk { self.psk_symbol } else { 1.0 };
                v += nco.next_sample() * (*a * s);
            }
            for st in self.stations.iter_mut() {
                let u = 0.5 + 0.5 * st.syllable.next_sample().im;
                let talk = if ((t / st.talk_period_s) as u64) % 4 == 3 {
                    0.0
                } else {
                    1.0
                };
                let a = TONE_AMPLITUDE * u * u * talk;
                for tone in st.tones.iter_mut() {
                    v += tone.next_sample() * a;
                }
            }
            *o = v;
            self.n += 1;
        }
        for w in self.wides.iter_mut() {
            for o in out.iter_mut() {
                while w.buf.is_empty() {
                    w.work.clear();
                    for _ in 0..1024 {
                        let x = Complex32::new(self.rng.normal_f32(), self.rng.normal_f32())
                            * std::f32::consts::FRAC_1_SQRT_2;
                        w.work.push(w.fir.process_sample(x));
                    }
                    w.res.clear();
                    w.rs.process(&w.work, &mut w.res);
                    w.buf.extend(w.res.iter().copied());
                }
                let s = w.buf.pop_front().unwrap_or_default();
                *o += s * w.nco.next_sample() * w.gain;
            }
        }
        for o in out.iter_mut() {
            *o *= self.gain;
        }
        out.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_power_while_transmitting() {
        let mut g = QoGenerator::new(GenParams::new(2e6));
        let mut x = vec![Complex32::default(); 2_000_000];
        g.generate(&mut x);
        let p = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
        assert!((p - 1.0).abs() < 0.1, "{p}");
    }
}
