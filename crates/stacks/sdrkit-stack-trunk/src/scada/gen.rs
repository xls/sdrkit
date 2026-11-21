//! The SCADA test signal, made from the standards: Modbus RTU, Modbus ASCII or DNP3 frames, polls and replies with a response
//! delay, in async characters (8N1, 8N2, 8E1, 8O1, 7E1, 7O1, 7N2) or HDLC (NRZ or NRZI, FCS CRC-16/X.25), on audio FSK (Bell 202,
//! V.23) or direct 2-FSK or 4-FSK (dibits +3 01, +1 00, -1 10, -3 11), each burst after a preamble and 12 idle bits.

use super::frame::{ascii_frame, dnp3_frame};
use num_complex::Complex32;
use sdrkit_core::publish::field;
use sdrkit_core::{ConfigError, DeclareSetting, GenConfig, Generator, Hz, Setting, SignalInfo};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
use sdrkit_util_bits::hdlc::hdlc_encode;
use sdrkit_util_bits::line::Nrzi;
use std::f64::consts::TAU;

/// The physical link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TxLink {
    /// Bell 202: mark 1200 Hz, space 2200 Hz, 1200 Bd.
    Bell202,
    /// ITU-T V.23: mark 1300 Hz, space 2100 Hz, 1200 Bd.
    V23,
    /// Direct 2-FSK at this bit rate.
    Fsk2(f64),
    /// Direct 4-FSK at this bit rate.
    Fsk4(f64),
}

impl TxLink {
    /// Symbols a second.
    pub fn symbol_rate(self) -> f64 {
        match self {
            TxLink::Bell202 | TxLink::V23 => 1200.0,
            TxLink::Fsk2(r) => r,
            TxLink::Fsk4(r) => r / 2.0,
        }
    }

    /// The deviation the receiver's chain expects: the FM deviation on audio FSK, the 2-FSK deviation, the outer 4-FSK one.
    pub fn default_deviation(self) -> f64 {
        match self {
            TxLink::Bell202 | TxLink::V23 => 3000.0,
            TxLink::Fsk2(r) if r <= 4800.0 => 2000.0,
            TxLink::Fsk2(r) if r <= 9600.0 => 3000.0,
            TxLink::Fsk2(_) => 4800.0,
            TxLink::Fsk4(r) if r <= 4800.0 => 1800.0,
            TxLink::Fsk4(r) if r <= 9600.0 => 2400.0,
            TxLink::Fsk4(_) => 4000.0,
        }
    }

    fn tones(self) -> Option<(f64, f64)> {
        match self {
            TxLink::Bell202 => Some((1200.0, 2200.0)),
            TxLink::V23 => Some((1300.0, 2100.0)),
            _ => None,
        }
    }
}

/// The character format or HDLC line code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxFraming {
    /// 8 data bits, no parity, one stop bit.
    A8N1,
    /// 8 data bits, no parity, two stop bits (Modbus RTU without parity).
    A8N2,
    /// 8 data bits, even parity.
    A8E1,
    /// 8 data bits, odd parity.
    A8O1,
    /// 7 data bits, even parity.
    A7E1,
    /// 7 data bits, odd parity.
    A7O1,
    /// 7 data bits, no parity, two stop bits.
    A7N2,
    /// HDLC, NRZ.
    HdlcNrz,
    /// HDLC, NRZI.
    HdlcNrzi,
}

/// One burst: the bytes of a frame as sent and the time without a burst after it.
#[derive(Clone, Debug, PartialEq)]
pub struct TxBurst {
    /// The bytes (an RTU frame with its CRC, a DNP3 frame with its CRCs, or the characters of an ASCII frame).
    pub bytes: Vec<u8>,
    /// The gap after it, s.
    pub gap_s: f64,
}

/// The options of the signal.
#[derive(Clone, Debug, PartialEq)]
pub struct ScadaOptions {
    /// The link.
    pub link: TxLink,
    /// The deviation, Hz; `None` for the link's own.
    pub deviation_hz: Option<f64>,
    /// The sense is inverted (direct FSK: the lower frequency is 1; audio FSK: the tones swap).
    pub inverted: bool,
    /// The framing.
    pub framing: TxFraming,
    /// Idle bits after each character.
    pub extra_idle_bits: usize,
    /// Preamble bits.
    pub preamble_bits: usize,
    /// Idle bits after the frame.
    pub tail_bits: usize,
    /// The carrier stays on (idle) between bursts.
    pub carrier_between: bool,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The drift of the carrier, Hz/s.
    pub drift_hz_s: f64,
    /// The amplitude.
    pub level: f64,
    /// The bit clock error, ppm.
    pub clock_ppm: f64,
    /// The level of the space tone over the mark tone, dB.
    pub tilt_db: f64,
    /// The bursts.
    pub bursts: Vec<TxBurst>,
}

impl ScadaOptions {
    /// The options of a link, a framing and bursts: the link's deviation, a preamble of 48 bits, 16 tail bits, unit amplitude.
    pub fn new(link: TxLink, framing: TxFraming, bursts: Vec<TxBurst>) -> ScadaOptions {
        ScadaOptions {
            link,
            deviation_hz: None,
            inverted: false,
            framing,
            extra_idle_bits: 0,
            preamble_bits: 48,
            tail_bits: 16,
            carrier_between: false,
            offset_hz: 0.0,
            drift_hz_s: 0.0,
            level: 1.0,
            clock_ppm: 0.0,
            tilt_db: 0.0,
            bursts,
        }
    }
}

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|h| u8::from_str_radix(h, 16).expect("hex"))
        .collect()
}

/// The Modbus test traffic: worked frames 1 and 2 (a poll and its reply), then 3 and 4 (a poll and its exception).
pub fn modbus_traffic() -> Vec<TxBurst> {
    vec![
        TxBurst {
            bytes: hex("11 03 00 6B 00 03 76 87"),
            gap_s: 0.05,
        },
        TxBurst {
            bytes: hex("11 03 06 02 2B 00 00 00 64 C8 BA"),
            gap_s: 0.2,
        },
        TxBurst {
            bytes: hex("01 03 00 00 00 0A C5 CD"),
            gap_s: 0.05,
        },
        TxBurst {
            bytes: hex("01 83 02 C0 F1"),
            gap_s: 0.2,
        },
    ]
}

/// The Modbus ASCII test traffic: the bytes of worked frames 1 and 2 as ASCII frames.
pub fn modbus_ascii_traffic() -> Vec<TxBurst> {
    vec![
        TxBurst {
            bytes: ascii_frame(&hex("11 03 00 6B 00 03")),
            gap_s: 0.05,
        },
        TxBurst {
            bytes: ascii_frame(&hex("11 03 06 02 2B 00 00 00 64")),
            gap_s: 0.2,
        },
    ]
}

/// The DNP3 test traffic: worked frame 5 (request link status) and its link status, worked frame 6 (class polls) and a
/// response with two analog inputs and two binary inputs.
pub fn dnp3_traffic() -> Vec<TxBurst> {
    let mut response = vec![0xC0, 0xC1, 129, 0x00, 0x00];
    response.extend_from_slice(&[
        30, 1, 0x00, 0, 1, 0x01, 0xE8, 0x03, 0, 0, 0x01, 0xFE, 0xFF, 0xFF, 0xFF,
    ]);
    response.extend_from_slice(&[1, 2, 0x00, 0, 1, 0x81, 0x01]);
    vec![
        TxBurst {
            bytes: hex("05 64 05 C9 01 00 03 00 75 3E"),
            gap_s: 0.05,
        },
        TxBurst {
            bytes: dnp3_frame(0x0B, 3, 1, &[]),
            gap_s: 0.2,
        },
        TxBurst {
            bytes: hex(
                "05 64 14 C4 01 00 03 00 04 09 C0 C1 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06 7A 6F",
            ),
            gap_s: 0.05,
        },
        TxBurst {
            bytes: dnp3_frame(0x44, 3, 1, &response),
            gap_s: 0.2,
        },
    ]
}

/// The data bits of one burst's frame in the framing, without the preamble.
fn frame_bits(o: &ScadaOptions, bytes: &[u8], out: &mut Vec<u8>) {
    let (data, parity, stops): (usize, Option<bool>, usize) = match o.framing {
        TxFraming::A8N1 => (8, None, 1),
        TxFraming::A8N2 => (8, None, 2),
        TxFraming::A8E1 => (8, Some(true), 1),
        TxFraming::A8O1 => (8, Some(false), 1),
        TxFraming::A7E1 => (7, Some(true), 1),
        TxFraming::A7O1 => (7, Some(false), 1),
        TxFraming::A7N2 => (7, None, 2),
        TxFraming::HdlcNrz | TxFraming::HdlcNrzi => {
            let mut bits = Vec::new();
            hdlc_encode(&[bytes], &CRC16_IBM_SDLC, 4, &mut bits);
            if o.framing == TxFraming::HdlcNrzi {
                Nrzi::new(1).encode(&mut bits);
            }
            out.extend_from_slice(&bits);
            return;
        }
    };
    for &b in bytes {
        out.push(0);
        let mut ones = 0;
        for k in 0..data {
            let v = b >> k & 1;
            ones += u32::from(v);
            out.push(v);
        }
        if let Some(even) = parity {
            out.push(u8::from((ones % 2 == 1) == even));
        }
        out.extend(std::iter::repeat_n(1, stops + o.extra_idle_bits));
    }
}

/// The levels of the symbols of one burst: preamble, 12 idle bits, the frame, the tail. Levels are ±1 and ±1/3 of the
/// deviation on direct FSK; on audio FSK +1 is mark and -1 space.
pub fn burst_levels(o: &ScadaOptions, bytes: &[u8]) -> Vec<f32> {
    let mut bits = Vec::new();
    let four = matches!(o.link, TxLink::Fsk4(_));
    let audio = o.link.tones().is_some();
    if !audio {
        for k in 0..o.preamble_bits {
            bits.push(if four {
                // +3 -3 alternating: dibits 01 and 11.
                if (k / 2) % 2 == 0 {
                    (k % 2) as u8
                } else {
                    1
                }
            } else {
                u8::from(k % 2 == 0)
            });
        }
    } else {
        bits.extend(std::iter::repeat_n(1, o.preamble_bits));
    }
    bits.extend(std::iter::repeat_n(1, 12));
    frame_bits(o, bytes, &mut bits);
    bits.extend(std::iter::repeat_n(1, o.tail_bits));
    if four {
        if bits.len() % 2 == 1 {
            bits.push(1);
        }
        bits.chunks(2)
            .map(|p| match (p[0], p[1]) {
                (0, 1) => 1.0,
                (0, 0) => 1.0 / 3.0,
                (1, 0) => -1.0 / 3.0,
                _ => -1.0,
            })
            .collect()
    } else if audio {
        bits.iter()
            .map(|&b| if b != 0 { 1.0 } else { -1.0 })
            .collect()
    } else {
        bits.iter()
            .map(|&b| if b != 0 { 1.0 } else { -1.0 })
            .collect()
    }
}

/// A piece of the signal.
enum Piece {
    Symbols(Vec<f32>),
    Gap(f64),
}

/// The generator: the bursts, repeated without end.
pub struct ScadaGenerator {
    rate: f64,
    channel_hz: f64,
    o: ScadaOptions,
    pieces: Vec<Piece>,
    at: usize,
    pos: f64,
    t: f64,
    level: f64,
    alpha: f64,
    phase: f64,
    tone_phase: f64,
}

impl ScadaGenerator {
    /// A generator of `o` at `rate`, the channel `channel_hz` from the centre. A gap of 0.05 s comes first.
    pub fn new(rate: f64, channel_hz: f64, o: ScadaOptions) -> ScadaGenerator {
        let mut pieces = vec![Piece::Gap(0.05)];
        for b in &o.bursts {
            pieces.push(Piece::Symbols(burst_levels(&o, &b.bytes)));
            pieces.push(Piece::Gap(b.gap_s));
        }
        let smooth = if o.link.tones().is_some() {
            20e3
        } else {
            o.link.symbol_rate()
        };
        ScadaGenerator {
            rate,
            channel_hz,
            alpha: 1.0 - (-TAU * smooth / rate).exp(),
            o,
            pieces,
            at: 0,
            pos: 0.0,
            t: 0.0,
            level: 0.0,
            phase: 0.0,
            tone_phase: 0.0,
        }
    }

    /// The length of one pass, s.
    pub fn length_s(&self) -> f64 {
        let sr = self.o.link.symbol_rate();
        self.pieces
            .iter()
            .map(|p| match p {
                Piece::Symbols(s) => s.len() as f64 / sr,
                Piece::Gap(g) => *g,
            })
            .sum()
    }

    /// The test signal of the settings of `cfg`, for a stack tuned `tune_offset_hz` above the channel.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] if the rate cannot hold the channel.
    pub fn from_config(
        cfg: &GenConfig,
        tune_offset_hz: f64,
    ) -> Result<ScadaGenerator, ConfigError> {
        let need = 2.0 * (tune_offset_hz + 20e3);
        if cfg.rate_hz < need {
            return Err(ConfigError::RateTooLow {
                need,
                got: cfg.rate_hz,
            });
        }
        let link = match cfg.settings.get(&GEN_LINK) {
            TestLink::Bell202 => TxLink::Bell202,
            TestLink::V23 => TxLink::V23,
            TestLink::Fsk2x4800 => TxLink::Fsk2(4800.0),
            TestLink::Fsk2x9600 => TxLink::Fsk2(9600.0),
            TestLink::Fsk2x19200 => TxLink::Fsk2(19_200.0),
            TestLink::Fsk4x4800 => TxLink::Fsk4(4800.0),
            TestLink::Fsk4x9600 => TxLink::Fsk4(9600.0),
            TestLink::Fsk4x19200 => TxLink::Fsk4(19_200.0),
        };
        let traffic = cfg.settings.get(&GEN_TRAFFIC);
        let framing = match cfg.settings.get(&GEN_FRAMING) {
            _ if traffic == TestTraffic::ModbusAscii => TxFraming::A7E1,
            TestFraming::A8E1 => TxFraming::A8E1,
            TestFraming::A8O1 => TxFraming::A8O1,
            TestFraming::A8N1 => TxFraming::A8N1,
            TestFraming::A8N2 => TxFraming::A8N2,
            TestFraming::HdlcNrz => TxFraming::HdlcNrz,
            TestFraming::HdlcNrzi => TxFraming::HdlcNrzi,
        };
        let bursts = match traffic {
            TestTraffic::Modbus => modbus_traffic(),
            TestTraffic::ModbusAscii => modbus_ascii_traffic(),
            TestTraffic::Dnp3 => dnp3_traffic(),
        };
        Ok(ScadaGenerator::new(
            cfg.rate_hz,
            -tune_offset_hz,
            ScadaOptions::new(link, framing, bursts),
        ))
    }

    /// The level of this sample and whether the carrier is on; moves on by one sample.
    fn step(&mut self) -> (f64, bool) {
        let dt = 1.0 / self.rate;
        let sr = self.o.link.symbol_rate() * (1.0 + self.o.clock_ppm * 1e-6);
        loop {
            match self.pieces.get(self.at) {
                None => {
                    self.at = 0;
                    self.pos = 0.0;
                }
                Some(Piece::Symbols(s)) => match s.get(self.pos as usize) {
                    Some(&l) => {
                        self.pos += sr * dt;
                        return (f64::from(l), true);
                    }
                    None => {
                        self.at += 1;
                        self.pos = 0.0;
                    }
                },
                Some(Piece::Gap(g)) => {
                    if self.pos < *g {
                        self.pos += dt;
                        // The idle level: the upper frequency on 2-FSK and mark on audio FSK, -3 on 4-FSK.
                        let idle = if matches!(self.o.link, TxLink::Fsk4(_)) {
                            -1.0
                        } else {
                            1.0
                        };
                        return (idle, self.o.carrier_between);
                    }
                    self.at += 1;
                    self.pos = 0.0;
                }
            }
        }
    }
}

impl Generator for ScadaGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let dt = 1.0 / self.rate;
        let dev = self
            .o
            .deviation_hz
            .unwrap_or(self.o.link.default_deviation());
        let space_amp = 10f64.powf(self.o.tilt_db / 20.0);
        for z in out.iter_mut() {
            let (target, on) = self.step();
            self.t += dt;
            if !on {
                *z = Complex32::new(0.0, 0.0);
                self.level = target;
                continue;
            }
            let shift = match self.o.link.tones() {
                Some((mark, space)) => {
                    let is_mark = (target > 0.0) != self.o.inverted;
                    let (f, a) = if is_mark {
                        (mark, 1.0)
                    } else {
                        (space, space_amp)
                    };
                    self.tone_phase = (self.tone_phase + TAU * f * dt) % TAU;
                    a * self.tone_phase.sin()
                }
                None => {
                    self.level += self.alpha * (target - self.level);
                    if self.o.inverted {
                        -self.level
                    } else {
                        self.level
                    }
                }
            };
            let f = self.channel_hz + self.o.offset_hz + self.o.drift_hz_s * self.t + dev * shift;
            self.phase = (self.phase + TAU * f * dt) % TAU;
            let (s, c) = self.phase.sin_cos();
            *z = Complex32::new((self.o.level * c) as f32, (self.o.level * s) as f32);
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of the test signal is the carrier over the noise in 25 kHz.
        let sr = self.o.link.symbol_rate();
        Some(SignalInfo::new(25e3).with_symbol_rate(sr))
    }
}

sdrkit_core::choice_enum! {
    /// The link of the test signal.
    pub enum TestLink {
        /// Bell 202 audio FSK.
        Bell202 = ("bell202", "Bell 202 AFSK 1200 Bd"),
        /// V.23 audio FSK.
        V23 = ("v23", "V.23 AFSK 1200 Bd"),
        /// 2-FSK, 4800 bit/s.
        Fsk2x4800 = ("fsk2_4800", "2-FSK 4800 bit/s"),
        /// 2-FSK, 9600 bit/s.
        Fsk2x9600 = ("fsk2_9600", "2-FSK 9600 bit/s"),
        /// 2-FSK, 19 200 bit/s.
        Fsk2x19200 = ("fsk2_19200", "2-FSK 19200 bit/s"),
        /// 4-FSK, 4800 bit/s.
        Fsk4x4800 = ("fsk4_4800", "4-FSK 4800 bit/s"),
        /// 4-FSK, 9600 bit/s.
        Fsk4x9600 = ("fsk4_9600", "4-FSK 9600 bit/s"),
        /// 4-FSK, 19 200 bit/s.
        Fsk4x19200 = ("fsk4_19200", "4-FSK 19200 bit/s"),
    }
}

sdrkit_core::choice_enum! {
    /// The framing of the test signal.
    pub enum TestFraming {
        /// 8 data bits, even parity.
        A8E1 = ("8e1", "8E1"),
        /// 8 data bits, odd parity.
        A8O1 = ("8o1", "8O1"),
        /// 8 data bits, no parity, one stop bit.
        A8N1 = ("8n1", "8N1"),
        /// 8 data bits, no parity, two stop bits.
        A8N2 = ("8n2", "8N2"),
        /// HDLC, NRZ.
        HdlcNrz = ("hdlc_nrz", "HDLC NRZ"),
        /// HDLC, NRZI.
        HdlcNrzi = ("hdlc_nrzi", "HDLC NRZI"),
    }
}

sdrkit_core::choice_enum! {
    /// The traffic of the test signal.
    pub enum TestTraffic {
        /// Modbus RTU polls and replies.
        Modbus = ("modbus", "Modbus RTU"),
        /// Modbus ASCII polls and replies (7E1).
        ModbusAscii = ("modbus_ascii", "Modbus ASCII"),
        /// DNP3 link status, class polls and a response.
        Dnp3 = ("dnp3", "DNP3"),
    }
}

/// The link of the test signal.
pub const GEN_LINK: Setting<field::Choice<TestLink>> =
    Setting::choice("gen_link", "Test signal link", TestLink::Bell202)
        .section("Test signal")
        .advanced();
/// The framing of the test signal (Modbus ASCII is always 7E1).
pub const GEN_FRAMING: Setting<field::Choice<TestFraming>> =
    Setting::choice("gen_framing", "Test signal framing", TestFraming::A8E1)
        .section("Test signal")
        .advanced();
/// The traffic of the test signal.
pub const GEN_TRAFFIC: Setting<field::Choice<TestTraffic>> =
    Setting::choice("gen_traffic", "Test signal traffic", TestTraffic::Modbus)
        .section("Test signal")
        .advanced();

/// The test signal settings. The framework adds the standard `gen_*` settings.
pub const SETTINGS: &[&dyn DeclareSetting] = &[&GEN_LINK, &GEN_FRAMING, &GEN_TRAFFIC];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters_have_their_bits() {
        let o = ScadaOptions::new(TxLink::Fsk2(9600.0), TxFraming::A8E1, Vec::new());
        let mut bits = Vec::new();
        frame_bits(&o, &[0x03], &mut bits);
        // Start, 1 1 0 0 0 0 0 0, even parity 0, stop.
        assert_eq!(bits, [0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1]);
        let o = ScadaOptions::new(TxLink::Fsk2(9600.0), TxFraming::A8N2, Vec::new());
        let mut bits = Vec::new();
        frame_bits(&o, &[0x80], &mut bits);
        assert_eq!(bits, [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1]);
        let o = ScadaOptions::new(TxLink::Fsk4(9600.0), TxFraming::A8N1, Vec::new());
        let l = burst_levels(&o, &[]);
        assert_eq!(&l[..4], &[1.0, -1.0, 1.0, -1.0]);
    }
}
