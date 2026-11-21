//! The packets that a Qi power receiver sends to its power transmitter (Wireless Power Consortium, The Qi Wireless Power
//! Transfer System, Parts 1 and 2, Power Class 0, version 1.2.3, section 5.2): 11-bit bytes (start 0, 8 data bits least
//! significant first, odd parity, stop 1), a preamble of 11 to 25 ones, a header, the message and an XOR checksum. The length
//! of the message follows from the header (Table 25); the named packets of Table 26 and their fields (sections 5.2.3.1 to
//! 5.2.3.8) are read.

use crate::typed::QiMessage;

/// The fewest preamble bits that a receiver takes (section 5.2.2.4).
pub const MIN_PREAMBLE: usize = 4;
/// The fewest preamble bits that a sender sends.
pub const SEND_PREAMBLE_MIN: usize = 11;
/// The most preamble bits that a sender sends.
pub const SEND_PREAMBLE_MAX: usize = 25;

/// The bytes of the message after header `h` (Table 25).
pub fn message_len(h: u8) -> usize {
    let h = usize::from(h);
    match h {
        0x00..=0x1F => 1,
        0x20..=0x7F => 2 + (h - 0x20) / 16,
        0x80..=0xDF => 8 + (h - 0x80) / 8,
        _ => 20 + (h - 0xE0) / 4,
    }
}

/// The checksum of a header and its message: the XOR of all of them.
pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |a, &b| a ^ b)
}

/// The 11 bits of byte `b`, the first sent first.
pub fn byte_bits(b: u8) -> [u8; 11] {
    let mut out = [0u8; 11];
    for (k, o) in out[1..9].iter_mut().enumerate() {
        *o = (b >> k) & 1;
    }
    // The parity bit makes the count of ones of the data and the parity odd.
    out[9] = u8::from(b.count_ones().is_multiple_of(2));
    out[10] = 1;
    out
}

/// The bits of a packet: `preamble` ones, then each byte of `bytes` (header, message, checksum).
pub fn packet_bits(preamble: usize, bytes: &[u8]) -> Vec<u8> {
    let mut v = vec![1u8; preamble];
    for &b in bytes {
        v.extend_from_slice(&byte_bits(b));
    }
    v
}

/// The type of header `h` (Table 26).
pub fn message_of(h: u8) -> QiMessage {
    match h {
        0x01 => QiMessage::SignalStrength,
        0x02 => QiMessage::EndPowerTransfer,
        0x03 => QiMessage::ControlError,
        0x04 => QiMessage::ReceivedPower,
        0x05 => QiMessage::ChargeStatus,
        0x06 => QiMessage::HoldOff,
        0x07 => QiMessage::GeneralRequest,
        0x09 => QiMessage::Renegotiate,
        0x20 => QiMessage::SpecificRequest,
        0x22 => QiMessage::FodStatus,
        0x31 => QiMessage::ReceivedPower24,
        0x51 => QiMessage::Configuration,
        0x54 | 0x55 => QiMessage::Wpid,
        0x71 => QiMessage::Identification,
        0x81 => QiMessage::ExtendedIdentification,
        0x18 | 0x19 | 0x28 | 0x29 | 0x38 | 0x48 | 0x58 | 0x68 | 0x78 | 0x84 | 0xA4 | 0xC4
        | 0xE2 => QiMessage::Proprietary,
        _ => QiMessage::Other,
    }
}

/// The reason of an End Power Transfer code (Table 29).
pub fn end_reason(code: u8) -> &'static str {
    match code {
        0x00 => "unknown",
        0x01 => "charge complete",
        0x02 => "internal fault",
        0x03 => "over temperature",
        0x04 => "over voltage",
        0x05 => "over current",
        0x06 => "battery failure",
        0x07 => "reserved (reconfigure, no longer used)",
        0x08 => "no response",
        0x0A => "negotiation failure",
        0x0B => "restart power transfer",
        _ => "reserved",
    }
}

/// The fields of a packet that passed its checks.
#[derive(Clone, Debug, PartialEq)]
pub struct Packet {
    /// The header.
    pub header: u8,
    /// The message bytes.
    pub message: Vec<u8>,
}

/// The fields that a named packet carries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parsed {
    /// Signal strength, End Power Transfer code, control error (signed), received power, charge status or hold-off (ms).
    pub value: Option<i64>,
    /// The reason of an End Power Transfer packet.
    pub end_reason: Option<&'static str>,
    /// The meaning of a Charge Status value.
    pub charge: Option<String>,
    /// The power class of a Configuration packet.
    pub power_class: Option<i64>,
    /// The maximum power value of a Configuration packet.
    pub max_power: Option<i64>,
    /// The version of an Identification packet, "major.minor".
    pub version: Option<String>,
    /// The manufacturer code of an Identification packet, 4 hex digits.
    pub manufacturer: Option<String>,
    /// The basic device identifier of an Identification packet, 8 hex digits (31 bits).
    pub device_id: Option<String>,
    /// An Extended Identification packet follows.
    pub extended: Option<bool>,
}

impl Packet {
    /// The type of the packet.
    pub fn kind(&self) -> QiMessage {
        message_of(self.header)
    }

    /// The fields of the packet.
    pub fn parse(&self) -> Parsed {
        let m = &self.message;
        let b0 = m.first().copied().unwrap_or(0);
        let mut p = Parsed::default();
        match self.kind() {
            QiMessage::SignalStrength | QiMessage::ReceivedPower | QiMessage::HoldOff => {
                p.value = Some(i64::from(b0));
            }
            QiMessage::EndPowerTransfer => {
                p.value = Some(i64::from(b0));
                p.end_reason = Some(end_reason(b0));
            }
            QiMessage::ControlError => p.value = Some(i64::from(b0 as i8)),
            QiMessage::ChargeStatus => {
                p.value = Some(i64::from(b0));
                p.charge = Some(match b0 {
                    0..=100 => format!("{b0} %"),
                    0xFF => "no rechargeable store or no information".to_string(),
                    _ => "reserved".to_string(),
                });
            }
            QiMessage::Configuration => {
                p.power_class = Some(i64::from(b0 >> 6));
                p.max_power = Some(i64::from(b0 & 0x3F));
            }
            QiMessage::Identification if m.len() >= 7 => {
                p.version = Some(format!("{}.{}", b0 >> 4, b0 & 0xF));
                p.manufacturer = Some(format!("{:02X}{:02X}", m[1], m[2]));
                p.extended = Some(m[3] & 0x80 != 0);
                p.device_id = Some(format!(
                    "{:02X}{:02X}{:02X}{:02X}",
                    m[3] & 0x7F,
                    m[4],
                    m[5],
                    m[6]
                ));
            }
            _ => {}
        }
        p
    }
}

/// What one bit gives to a [`PacketReader`].
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Nothing yet.
    Nothing,
    /// A packet that passed every check.
    Packet(Packet),
    /// A packet that failed its parity, a stop bit, a start bit or its checksum.
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Preamble(usize),
    Byte { bits: Vec<u8> },
    Between,
}

/// Bytes and packets from bits (section 5.2.2.4).
#[derive(Clone, Debug, PartialEq)]
pub struct PacketReader {
    state: State,
    bytes: Vec<u8>,
    need: usize,
}

impl Default for PacketReader {
    fn default() -> Self {
        PacketReader {
            state: State::Preamble(0),
            bytes: Vec::with_capacity(32),
            need: 0,
        }
    }
}

impl PacketReader {
    /// Drops a packet in progress.
    pub fn reset(&mut self) {
        self.state = State::Preamble(0);
        self.bytes.clear();
        self.need = 0;
    }

    /// The header byte of the packet in progress has passed its checks.
    pub fn has_header(&self) -> bool {
        !self.bytes.is_empty()
    }

    /// The next bit is the stop bit of the checksum byte, the last of the packet.
    pub fn wants_last_bit(&self) -> bool {
        matches!(&self.state, State::Byte { bits } if bits.len() == 9)
            && self.need > 0
            && self.bytes.len() + 1 == self.need
    }

    /// Takes one bit.
    pub fn bit(&mut self, b: u8) -> Step {
        match &mut self.state {
            State::Preamble(ones) => {
                if b == 1 {
                    *ones = (*ones + 1).min(40);
                } else if *ones >= MIN_PREAMBLE {
                    self.state = State::Byte {
                        bits: Vec::with_capacity(10),
                    };
                } else {
                    *ones = 0;
                }
                Step::Nothing
            }
            State::Between => {
                if b == 0 {
                    self.state = State::Byte {
                        bits: Vec::with_capacity(10),
                    };
                    Step::Nothing
                } else {
                    self.reset();
                    Step::Failed
                }
            }
            State::Byte { bits } => {
                bits.push(b);
                if bits.len() < 10 {
                    return Step::Nothing;
                }
                let data = bits[..8]
                    .iter()
                    .enumerate()
                    .fold(0u8, |a, (k, &x)| a | (x << k));
                let ones = data.count_ones() + u32::from(bits[8]);
                if ones.is_multiple_of(2) || bits[9] != 1 {
                    self.reset();
                    return Step::Failed;
                }
                self.bytes.push(data);
                if self.bytes.len() == 1 {
                    self.need = 2 + message_len(data);
                }
                if self.bytes.len() < self.need {
                    self.state = State::Between;
                    return Step::Nothing;
                }
                let n = self.bytes.len();
                let ok = checksum(&self.bytes[..n - 1]) == self.bytes[n - 1];
                let p = Packet {
                    header: self.bytes[0],
                    message: self.bytes[1..n - 1].to_vec(),
                };
                self.reset();
                if ok {
                    Step::Packet(p)
                } else {
                    Step::Failed
                }
            }
        }
    }
}
