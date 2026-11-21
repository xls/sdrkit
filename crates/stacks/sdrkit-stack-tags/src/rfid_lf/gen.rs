//! The EM4100 test signal: a reader carrier of amplitude 1 at 0 Hz whose amplitude a tag lowers by the depth while its
//! modulator is on (EM4100 data sheet, Code Description). The tag sends its 64-bit frame again and again from the start time,
//! at RF/64 or RF/32 of a 125 kHz carrier, in the Manchester code (the first half-bit is the bit, the second its inverse) or
//! the Biphase code (a change at the start of each bit, one more in the middle of a 0). The edges pass a one-pole low-pass at
//! 20 kHz. The noise is stated against the carrier in 10 kHz.

use super::frame;
use crate::typed::{RfidBitRate, RfidCoding};
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use sdrkit_util_bits::line::{Biphase, BiphaseKind, Manchester, ManchesterConv};

const SECTION: &str = "Test signal";

/// The carrier of the test signal, Hz (the data sheet's coil example).
pub const CARRIER_HZ: f64 = 125e3;
/// The band that the noise is stated in, Hz.
pub const NOISE_BAND_HZ: f64 = 10e3;
/// The cut-off of the edge filter, Hz.
pub const EDGE_HZ: f64 = 20e3;

/// The tags of the test signal.
pub const GEN_ID: Setting<field::Text> = Setting::new("gen_id", "Tags", "", "0123456789")
    .section(SECTION)
    .help("Tags of 10 hex digits (version and number), separated by commas; each is sent in turn for the time of gen_id_s.");
/// The time of each tag.
pub const GEN_ID_S: Setting<field::F64> = Setting::new("gen_id_s", "Time of each tag", "s", 1.0)
    .range(0.1, 3600.0, 0.0)
    .section(SECTION);
/// The modulation depth.
pub const GEN_DEPTH: Setting<field::F64> = Setting::new("gen_depth", "Modulation depth", "", 0.08)
    .range(0.001, 0.9, 0.0)
    .section(SECTION);
/// The bit rate.
pub const GEN_RF_DIV: Setting<field::Choice<RfidBitRate>> =
    Setting::choice("gen_rf_div", "Bit rate", RfidBitRate::Rf64).section(SECTION);
/// The code.
pub const GEN_CODING: Setting<field::Choice<RfidCoding>> =
    Setting::choice("gen_coding", "Code", RfidCoding::Manchester).section(SECTION);
/// The error of the tag clock.
pub const GEN_BIT_CLOCK_PPM: Setting<field::F64> =
    Setting::new("gen_bit_clock_ppm", "Tag clock error", "ppm", 0.0)
        .range(-200_000.0, 200_000.0, 0.0)
        .section(SECTION)
        .help("The bits run this much fast (positive) or slow.");
/// The time the tag starts.
pub const GEN_START_S: Setting<field::F64> =
    Setting::new("gen_start_s", "Tag starts at", "s", 0.05)
        .range(0.0, 3600.0, 0.0)
        .section(SECTION);
/// The polarity of the load.
pub const GEN_INVERT: Setting<field::Bool> =
    Setting::new("gen_invert", "Inverted load", "", false).section(SECTION);

/// Reads a list of tags; `None` when an entry is not 10 hex digits.
pub fn parse_ids(text: &str) -> Option<Vec<u64>> {
    text.split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| {
            (e.len() == 10)
                .then(|| u64::from_str_radix(e, 16).ok())
                .flatten()
        })
        .collect()
}

/// The line levels of one frame of `data`, two half-bits a bit (1: the modulator is on). Biphase starts from level `start`.
pub fn half_bits(data: u64, coding: RfidCoding, start: u8) -> Vec<u8> {
    let f = frame::encode(data);
    let mut out = Vec::with_capacity(2 * frame::FRAME_BITS);
    match coding {
        RfidCoding::Manchester => Manchester::encode(&f, ManchesterConv::Thomas, &mut out),
        RfidCoding::Biphase => Biphase::new(BiphaseKind::Space, start).encode(&f, &mut out),
    }
    out
}

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct RfidLfGenerator {
    rate: f64,
    ids: Vec<u64>,
    id_s: f64,
    depth: f64,
    half_s: f64,
    coding: RfidCoding,
    start_s: f64,
    invert: bool,
    alpha: f64,
    y: f64,
    n: u64,
    halves: Vec<u8>,
    frame: Option<u64>,
}

impl RfidLfGenerator {
    /// The signal of the settings `v` at `rate_hz`; a list of tags that cannot be read gives the default tag.
    pub fn new(rate_hz: f64, v: &Values) -> RfidLfGenerator {
        let ids = parse_ids(v.get(&GEN_ID))
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| vec![0x01_2345_6789]);
        let div = match v.get(&GEN_RF_DIV) {
            RfidBitRate::Rf64 => 64.0,
            RfidBitRate::Rf32 => 32.0,
        };
        let clock = 1.0 + v.get(&GEN_BIT_CLOCK_PPM) * 1e-6;
        RfidLfGenerator {
            rate: rate_hz,
            ids,
            id_s: v.get(&GEN_ID_S),
            depth: v.get(&GEN_DEPTH),
            half_s: div / CARRIER_HZ / 2.0 / clock,
            coding: v.get(&GEN_CODING),
            start_s: v.get(&GEN_START_S),
            invert: v.get(&GEN_INVERT),
            alpha: 1.0 - (-std::f64::consts::TAU * EDGE_HZ / rate_hz).exp(),
            y: 1.0,
            n: 0,
            halves: Vec::new(),
            frame: None,
        }
    }

    /// The modulator state at half-bit `k` from the start. Frames come in order; the tag changes only between frames.
    fn half(&mut self, k: u64) -> u8 {
        let per = (2 * frame::FRAME_BITS) as u64;
        let fi = k / per;
        if self.frame != Some(fi) {
            let start = self.halves.last().copied().unwrap_or(0);
            let which = ((fi * per) as f64 * self.half_s / self.id_s) as usize % self.ids.len();
            self.halves = half_bits(self.ids[which], self.coding, start);
            self.frame = Some(fi);
        }
        self.halves[(k % per) as usize]
    }
}

impl Generator for RfidLfGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            let on = if t < self.start_s {
                0
            } else {
                let k = ((t - self.start_s) / self.half_s) as u64;
                self.half(k)
            };
            let on = on ^ u8::from(self.invert);
            let a = 1.0 - self.depth * f64::from(on);
            self.y += self.alpha * (a - self.y);
            *z = Complex32::new(self.y as f32, 0.0);
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NOISE_BAND_HZ).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
