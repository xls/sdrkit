//! Futaba S-FHSS packets after the sync word, and the hop rule. No public document gives them; which CRC the link uses is not
//! confirmed, so both are tried.

use sdrkit_util_bits::crc::catalog::{CRC16_CMS, CRC16_IBM_3740};

/// The rate, bit/s.
pub const RATE: f64 = 128_143.0;
/// The air pattern: the last preamble bytes and the sync word.
pub const PATTERN: [u8; 4] = [0xAA, 0xAA, 0xD3, 0x91];
/// Bytes of a packet after the sync word (13 and the CRC).
pub const LEN: usize = 15;
/// Hop channels.
pub const HOP_CHANNELS: u8 = 30;

sdrkit_core::choice_enum! {
    /// The CRC of an S-FHSS packet.
    pub enum SfhssCrc {
        /// Either; the one that matched is shown.
        Either = ("either", "Either"),
        /// CRC-16/IBM-3740 (polynomial 1021, initial FFFF).
        Ccitt = ("ccitt", "CRC-16/IBM-3740"),
        /// CRC-16/CMS (polynomial 8005, initial FFFF).
        Cc2500 = ("cc2500", "CRC-16/CMS"),
    }
}

/// A packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sfhss {
    /// The first byte (80, 81 or 82).
    pub head: u8,
    /// The transmitter id.
    pub id: u16,
    /// The hop channel, 0 to 29.
    pub hop: u8,
    /// The hop code, 0 to 27.
    pub code: u8,
    /// The 6 command bits.
    pub command: u8,
    /// The four 12-bit channel values.
    pub channels: [u16; 4],
    /// The CRC that matched.
    pub crc: SfhssCrc,
}

impl Sfhss {
    /// The second packet of a pair.
    pub fn second(&self) -> bool {
        self.command & 1 != 0
    }
    /// A failsafe packet.
    pub fn failsafe(&self) -> bool {
        self.command & 4 != 0
    }
    /// The values are channels 5 to 8.
    pub fn upper(&self) -> bool {
        self.command & 8 != 0
    }
    /// The type text of the frame log.
    pub fn kind(&self) -> &'static str {
        if self.failsafe() {
            "S-FHSS failsafe"
        } else {
            "S-FHSS channels"
        }
    }
    /// The fields as text: "CH1-4: 1520 1020 2020 1777, hop channel 7, hop code 11, packet 1".
    pub fn detail(&self) -> String {
        let v: Vec<String> = self.channels.iter().map(u16::to_string).collect();
        format!(
            "CH{}: {}, hop channel {}, hop code {}, packet {}",
            if self.upper() { "5-8" } else { "1-4" },
            v.join(" "),
            self.hop,
            self.code,
            if self.second() { 2 } else { 1 }
        )
    }
}

/// The hop channel after `hop` with code `code`: `hop + code + 2`; when that exceeds 29, `code + 2` more if it is below 31,
/// then 31 less.
pub fn next_hop(hop: u8, code: u8) -> u8 {
    let mut n = u16::from(hop) + u16::from(code) + 2;
    if n > 29 {
        if n < 31 {
            n += u16::from(code) + 2;
        }
        n -= 31;
    }
    n as u8
}

/// The 15 bytes of a packet with the CRC `crc` (`Either` makes CRC-16/IBM-3740).
pub fn build(p: &Sfhss) -> [u8; LEN] {
    let mut b = [0u8; LEN];
    b[0] = p.head;
    b[1..3].copy_from_slice(&p.id.to_be_bytes());
    let mut w: u64 = u64::from(p.hop & 0x1F);
    for &c in &p.channels {
        w = w << 12 | u64::from(c & 0x0FFF);
    }
    w = w << 5 | u64::from(p.code & 0x1F);
    w = w << 6 | u64::from(p.command & 0x3F);
    b[5..13].copy_from_slice(&w.to_be_bytes());
    let c = match p.crc {
        SfhssCrc::Cc2500 => CRC16_CMS.checksum(&b[..13]),
        _ => CRC16_IBM_3740.checksum(&b[..13]),
    };
    b[13..].copy_from_slice(&c.to_be_bytes());
    b
}

/// Reads a packet with the CRC choice `which`. `None` for a wrong first byte or CRC, a hop channel over 29, a code over 27, or
/// (unless failsafe) a value outside 900 to 2150.
pub fn parse(b: &[u8], which: SfhssCrc) -> Option<Sfhss> {
    if b.len() < LEN || !(0x80..=0x82).contains(&b[0]) {
        return None;
    }
    let got = u16::from_be_bytes([b[13], b[14]]);
    let ccitt = || CRC16_IBM_3740.checksum(&b[..13]) == got;
    let cc2500 = || CRC16_CMS.checksum(&b[..13]) == got;
    let crc = match which {
        SfhssCrc::Ccitt if ccitt() => SfhssCrc::Ccitt,
        SfhssCrc::Cc2500 if cc2500() => SfhssCrc::Cc2500,
        SfhssCrc::Either if ccitt() => SfhssCrc::Ccitt,
        SfhssCrc::Either if cc2500() => SfhssCrc::Cc2500,
        _ => return None,
    };
    let mut w = u64::from_be_bytes(b[5..13].try_into().ok()?);
    let command = (w & 0x3F) as u8;
    w >>= 6;
    let code = (w & 0x1F) as u8;
    w >>= 5;
    let mut channels = [0u16; 4];
    for c in channels.iter_mut().rev() {
        *c = (w & 0x0FFF) as u16;
        w >>= 12;
    }
    let hop = (w & 0x1F) as u8;
    let p = Sfhss {
        head: b[0],
        id: u16::from_be_bytes([b[1], b[2]]),
        hop,
        code,
        command,
        channels,
        crc,
    };
    if hop > 29 || code > 27 {
        return None;
    }
    if !p.failsafe() && channels.iter().any(|&v| !(900..=2150).contains(&v)) {
        return None;
    }
    Some(p)
}
