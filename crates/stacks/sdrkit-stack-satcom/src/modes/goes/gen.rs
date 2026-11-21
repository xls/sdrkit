//! The GOES DCS test signal: platforms on their channels, each sending its report now and then (carrier, preamble, sync,
//! address, data, EOT), with plus or minus 60 degree phase modulation, NRZ or Manchester.

use super::body::{channel_hz, message_bits};
use super::DESCRIPTOR;
use sdrkit_core::publish::field;
use sdrkit_core::{Choice, Complex32, GenConfig, Generator, Setting, SignalInfo};
use std::f64::consts::{PI, TAU};

sdrkit_core::choice_enum! {
    /// The scene of the test signal.
    pub enum Scenario {
        /// Three platforms, Manchester coded.
        Three = ("0", "Three platforms (100, 300, 1200 bit/s Manchester)"),
        /// Only the 300 bit/s platform.
        One = ("1", "Only the 300 bit/s platform"),
        /// One of each rate, 300 and 1200 as NRZ.
        Nrz = ("2", "One of each rate, 300 and 1200 as NRZ"),
        /// Three platforms and a steady carrier.
        Carrier = ("3", "Three platforms and a steady carrier"),
    }
}

/// The scene of the test signal.
pub const GEN_SCENARIO: Setting<field::Choice<Scenario>> =
    Setting::choice("gen_scenario", "Scenario", Scenario::Three)
        .section("Test signal")
        .advanced();

/// The data every test platform sends.
pub const TEXT: &str = "WL 12.34 TW 18.5 BV 12.6";

/// One transmitter of the test scene, with its own frequency and clock errors.
#[derive(Clone, Debug)]
pub struct Tx {
    /// Its channel (1 to 266).
    pub channel: usize,
    /// The 21 data bits of its address.
    pub address21: u32,
    /// Its bit rate.
    pub rate: f64,
    /// Manchester coded.
    pub manchester: bool,
    /// The flag byte (above 100 bit/s).
    pub flags: Option<u8>,
    /// When it first sends, seconds.
    pub start_s: f64,
    /// The unmodulated carrier before the preamble, seconds.
    pub carrier_s: f64,
    /// Its data.
    pub data: Vec<u8>,
    /// Seconds between two of its reports.
    pub repeat_s: f64,
    /// Its own clock error, ppm.
    pub clock_ppm: f64,
    /// Its own frequency error, Hz.
    pub freq_err_hz: f64,
    /// Its amplitude (1 is unit power).
    pub amplitude: f32,
    /// A steady unmodulated carrier.
    pub steady: bool,
}

impl Tx {
    /// A platform on `channel` with this address, bit rate and coding, first sending at `start_s`.
    pub fn new(channel: usize, address21: u32, rate: f64, manchester: bool, start_s: f64) -> Self {
        Tx {
            channel,
            address21,
            rate,
            manchester,
            flags: (rate > 100.0).then_some(0x5A),
            start_s,
            carrier_s: 0.6,
            data: TEXT.as_bytes().to_vec(),
            repeat_s: 16.0,
            clock_ppm: 0.0,
            freq_err_hz: 0.0,
            amplitude: 1.0,
            steady: false,
        }
    }
    /// The bits of one transmission after the carrier.
    pub fn bits(&self) -> Vec<bool> {
        let pre = if self.rate <= 100.0 { 250 } else { 450 };
        message_bits(pre, self.address21, self.flags, &self.data)
    }
    /// The length of one transmission at the platform's own clock, seconds.
    pub fn length_s(&self) -> f64 {
        self.carrier_s + self.bits().len() as f64 / (self.rate * (1.0 + self.clock_ppm * 1e-6))
    }
}

/// The transmitters of a scenario (0 to 3, as the setting lists them).
pub fn scenario(k: u8) -> Vec<Tx> {
    let mut v = match k {
        1 => vec![Tx::new(100, 0x0A_2B3C, 300.0, true, 1.0)],
        2 => vec![
            Tx::new(60, 0x01_2345, 100.0, true, 1.0),
            Tx::new(100, 0x0A_2B3C, 300.0, false, 6.0),
            Tx::new(140, 0x15_4D5E, 1200.0, false, 11.0),
        ],
        _ => vec![
            Tx::new(60, 0x01_2345, 100.0, true, 1.0),
            Tx::new(100, 0x0A_2B3C, 300.0, true, 6.0),
            Tx::new(140, 0x15_4D5E, 1200.0, true, 11.0),
        ],
    };
    if k == 3 {
        let mut s = Tx::new(120, 0, 100.0, false, 0.0);
        s.steady = true;
        v.push(s);
    }
    v
}

/// The generator: the scene's transmitters, summed.
pub struct GoesGenerator {
    rate: f64,
    centre: f64,
    txs: Vec<(Tx, Vec<bool>, f64)>,
    n: u64,
}

impl GoesGenerator {
    /// The generator of a stack configuration: a scenario around the default dial.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let k = cfg.settings.get(&GEN_SCENARIO).index() as u8;
        Self::new(cfg.rate_hz, DESCRIPTOR.tuning.default_hz, scenario(k))
    }

    /// The transmitters `txs` at `rate`, the radio centred on `centre` (absolute, Hz).
    pub fn new(rate: f64, centre: f64, txs: Vec<Tx>) -> Self {
        GoesGenerator {
            rate,
            centre,
            txs: txs
                .into_iter()
                .map(|t| {
                    let b = t.bits();
                    (t, b, 0.0)
                })
                .collect(),
            n: 0,
        }
    }
}

impl Generator for GoesGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let mut v = Complex32::default();
            for (tx, bits, ph) in self.txs.iter_mut() {
                let f = channel_hz(tx.channel) - self.centre + tx.freq_err_hz;
                *ph = (*ph + TAU * f / self.rate) % TAU;
                if tx.steady {
                    v += Complex32::new(ph.cos() as f32, ph.sin() as f32) * tx.amplitude;
                    continue;
                }
                if t < tx.start_s {
                    continue;
                }
                let rel = (t - tx.start_s) % tx.repeat_s;
                let len =
                    tx.carrier_s + bits.len() as f64 / (tx.rate * (1.0 + tx.clock_ppm * 1e-6));
                if rel >= len {
                    continue;
                }
                let m = if rel < tx.carrier_s {
                    0.0
                } else {
                    let x = (rel - tx.carrier_s) * tx.rate * (1.0 + tx.clock_ppm * 1e-6);
                    let k = (x as usize).min(bits.len() - 1);
                    let b = bits[k];
                    let level = if tx.manchester {
                        let first = x.fract() < 0.5;
                        if first == b {
                            1.0
                        } else {
                            -1.0
                        }
                    } else if b {
                        1.0
                    } else {
                        -1.0
                    };
                    level * PI / 3.0
                };
                let a = *ph + m;
                v += Complex32::new(a.cos() as f32, a.sin() as f32) * tx.amplitude;
            }
            *o = v;
            self.n += 1;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of a platform is its carrier over the noise in 100 Hz.
        Some(SignalInfo::new(100.0))
    }
}
