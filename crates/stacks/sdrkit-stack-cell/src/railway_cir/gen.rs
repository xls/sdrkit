//! The railway CIR test signal: a burst of 160 alternating bits (the first 0), the sync and the words of one worked frame
//! as FFSK at 1200 bit/s (1200 Hz for a 1, 1800 Hz for a 0, the tone phase continuous), frequency modulated with a peak
//! deviation of 2500 Hz on a carrier at 0 Hz; then 0.3 s of unmodulated carrier; repeated. Noise and a
//! carrier error come from the standard impairments of the test signal.

use super::air;
use super::frame::{build, Protocol};
use super::modem::{BAUD, MARK_HZ, SPACE_HZ};
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

/// Every air bit of the test signal inverted.
pub const GEN_INVERT_BITS: Setting<field::Bool> =
    Setting::new("gen_invert_bits", "Invert the air bits", "", false).section("Test signal");
/// The peak deviation of the tone.
pub const GEN_DEVIATION_HZ: Setting<field::F64> = Setting::new("gen_deviation_hz", "Tone deviation", "Hz", 2500.0)
    .range(300.0, 6000.0, 0.0)
    .section("Test signal");

/// The silence after each burst, s.
pub const GAP_S: f64 = 0.3;
/// The amplitude of the carrier.
pub const AMPLITUDE: f64 = 0.4;
/// The preamble bits.
pub const PREAMBLE_BITS: usize = 160;

/// The worked TDCS payload: train T112, locomotive HXD3D-0521, 1161.011 km, 124 km/h.
pub const TDCS_PAYLOAD: [u8; 32] = [
    0x03, 0x03, 0x01, 0x54, 0x00, 0x00, 0x00, 0x70, 0x00, 0x00, 0xF2, 0x00, 0x09, 0x02, 0x33, 0xB7, 0x11, 0x7C, 0x00, 0x00,
    0x20, 0x03, 0x9A, 0x01, 0x12, 0x82, 0x00, 0x04, 0x05, 0x40, 0xE2, 0x01,
];
/// The worked KLW payload: locomotive HXD3C-0375, pressure response of 486 kPa, tail 123456.
pub const KLW_PAYLOAD: [u8; 11] = [0x23, 0x90, 0x03, 0x75, 0x00, 0x92, 0x12, 0x34, 0x56, 0x04, 0x86];

/// The worked frame of a protocol.
pub fn worked_frame(p: Protocol) -> Vec<u8> {
    match p {
        Protocol::Klw => build(p, &KLW_PAYLOAD),
        Protocol::Tdcs => build(p, &TDCS_PAYLOAD),
    }
}

/// The bits of one burst: the preamble, the sync and the words of `frame`, each inverted when `invert`.
pub fn burst_bits(frame: &[u8], invert: bool) -> Vec<u8> {
    let mut bits: Vec<u8> = (0..PREAMBLE_BITS).map(|i| (i % 2) as u8).collect();
    bits.extend(air::encode(frame));
    if invert {
        bits.iter_mut().for_each(|b| *b ^= 1);
    }
    bits
}

/// FFSK audio of `bits` at `rate_hz`, amplitude 1, the tone phase continuous, the bit clock scaled by `1 + clock`.
pub fn ffsk_audio(bits: &[u8], rate_hz: f64, clock: f64) -> Vec<f32> {
    let per_bit = rate_hz / (BAUD * (1.0 + clock));
    let n = (bits.len() as f64 * per_bit).round() as usize;
    let mut ph = 0.0f64;
    (0..n)
        .map(|i| {
            let b = bits[((i as f64 / per_bit) as usize).min(bits.len() - 1)];
            let f = if b == 1 { MARK_HZ } else { SPACE_HZ };
            ph = (ph + TAU * f / rate_hz).rem_euclid(TAU);
            ph.sin() as f32
        })
        .collect()
}

/// The test signal.
pub struct CirGenerator {
    rate: f64,
    bits: Vec<u8>,
    deviation: f64,
    n: u64,
    tone: f64,
    phase: f64,
}

impl CirGenerator {
    /// The worked frame of `protocol` at `rate_hz`.
    pub fn new(rate_hz: f64, protocol: Protocol, v: &Values) -> CirGenerator {
        CirGenerator {
            rate: rate_hz,
            bits: burst_bits(&worked_frame(protocol), v.get(&GEN_INVERT_BITS)),
            deviation: v.get(&GEN_DEVIATION_HZ),
            n: 0,
            tone: 0.0,
            phase: 0.0,
        }
    }
}

impl Generator for CirGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let burst_s = self.bits.len() as f64 / BAUD;
        let period = burst_s + GAP_S;
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            let at = t.rem_euclid(period);
            let mut f = 0.0;
            if at < burst_s {
                let b = self.bits[((at * BAUD) as usize).min(self.bits.len() - 1)];
                let tone = if b == 1 { MARK_HZ } else { SPACE_HZ };
                self.tone = (self.tone + TAU * tone / self.rate).rem_euclid(TAU);
                f = self.deviation * self.tone.sin();
            }
            self.phase = (self.phase + TAU * f / self.rate).rem_euclid(TAU);
            *z = Complex32::from_polar(AMPLITUDE as f32, self.phase as f32);
        }
        out.len()
    }
    /// The noise of the impairments is set against the carrier in 25 kHz.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(25e3).with_power(AMPLITUDE * AMPLITUDE))
    }
    /// The station is made at 0 Hz; the framework moves it to where the receiver looks.
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
