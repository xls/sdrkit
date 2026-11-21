//! FrSky D8 and D16 packets after the sync word. No public document gives the layouts or the D16 checksum.

use sdrkit_util_bits::crc::catalog::CRC16_CMS;

/// The D8 rate, baud.
pub const D8_RATE: f64 = 31_044.0;
/// The D16 rates, baud.
pub const D16_RATES: [f64; 2] = [70_000.0, 99_970.0];
/// The air pattern: the last preamble bytes and the sync word.
pub const PATTERN: [u8; 4] = [0xAA, 0xAA, 0xD3, 0x91];
/// Bytes of a D8 packet after the sync word.
pub const D8_LEN: usize = 20;
/// The longest D16 packet after the sync word (the length byte, 34 bytes).
pub const D16_MAX: usize = 35;

/// The table of the D16 checksum.
const D16_TABLE: [u16; 16] = [
    0x0000, 0x1189, 0x2312, 0x329B, 0x4624, 0x57AD, 0x6536, 0x74BF, 0x8C48, 0x9DC1, 0xAF5A, 0xBED3,
    0xCA6C, 0xDBE5, 0xE97E, 0xF8F7,
];

/// The D16 checksum of `data` from `init`: for each byte `b`, `v = (crc >> 8) ^ b`,
/// `crc = (crc << 8) ^ T[v & 15] ^ (0x1081 × (v >> 4))` in 16 bits.
pub fn d16_crc(init: u16, data: &[u8]) -> u16 {
    let mut crc = init;
    for &b in data {
        let v = (crc >> 8) as u8 ^ b;
        crc = (crc << 8) ^ D16_TABLE[usize::from(v & 15)] ^ 0x1081u16.wrapping_mul(u16::from(v >> 4));
    }
    crc
}

/// A D8 or D16 packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frsky {
    /// D8 channel values (12-bit raw values) and the packet counter.
    D8Channels {
        /// The transmitter id.
        id: u16,
        /// The packet counter.
        counter: u8,
        /// Channels 1 to 8.
        channels: [u16; 8],
    },
    /// A D8 bind packet.
    D8Bind {
        /// The transmitter id.
        id: u16,
        /// The hop table index.
        index: u8,
    },
    /// A D16 packet: its id, hop index and channel skip; the channel data are not unpacked.
    D16 {
        /// The transmitter id.
        id: u16,
        /// The hop index.
        hop: u8,
        /// The channel skip.
        skip: u16,
        /// A bind packet.
        bind: bool,
        /// Bytes of the packet after the sync word.
        len: usize,
    },
}

impl Frsky {
    /// The type text of the frame log.
    pub fn kind(&self) -> &'static str {
        match self {
            Frsky::D8Channels { .. } => "D8 channels",
            Frsky::D8Bind { .. } => "D8 bind",
            Frsky::D16 { bind: false, .. } => "D16 frame",
            Frsky::D16 { bind: true, .. } => "D16 bind",
        }
    }
    /// The transmitter id.
    pub fn id(&self) -> u16 {
        match *self {
            Frsky::D8Channels { id, .. } | Frsky::D8Bind { id, .. } | Frsky::D16 { id, .. } => id,
        }
    }
    /// The fields as text.
    pub fn detail(&self) -> String {
        match self {
            Frsky::D8Channels {
                counter, channels, ..
            } => {
                let v: Vec<String> = channels.iter().map(u16::to_string).collect();
                format!("CH1-8: {}, packet {counter}", v.join(" "))
            }
            Frsky::D8Bind { index, .. } => format!("hop table index {index}"),
            Frsky::D16 { hop, skip, .. } => format!("hop index {hop}, channel skip {skip}"),
        }
    }
}

/// The 20 bytes of a D8 channel packet.
pub fn d8_channels(id: u16, counter: u8, ch: &[u16; 8]) -> [u8; D8_LEN] {
    let mut p = [0u8; D8_LEN];
    p[0] = 0x11;
    p[1..3].copy_from_slice(&id.to_be_bytes());
    p[3] = counter;
    p[5] = 1;
    // Each group of four channels: the four low bytes, then the high nibbles two to a byte.
    for half in 0..2 {
        let c = &ch[4 * half..4 * half + 4];
        let o = 6 + 6 * half;
        for k in 0..4 {
            p[o + k] = c[k] as u8;
        }
        let nib = |v: u16| ((v >> 8) & 0x0F) as u8;
        p[o + 4] = nib(c[0]) | nib(c[1]) << 4;
        p[o + 5] = nib(c[2]) | nib(c[3]) << 4;
    }
    let c = CRC16_CMS.checksum(&p[..18]);
    p[18..].copy_from_slice(&c.to_be_bytes());
    p
}

/// Reads a D8 packet. `None` for a wrong length byte or CRC.
pub fn parse_d8(b: &[u8]) -> Option<Frsky> {
    if b.len() < D8_LEN || b[0] != 0x11 {
        return None;
    }
    if CRC16_CMS.checksum(&b[..18]) != u16::from_be_bytes([b[18], b[19]]) {
        return None;
    }
    if b[1] == 0x03 && b[2] == 0x01 {
        return Some(Frsky::D8Bind {
            id: u16::from_be_bytes([b[3], b[4]]),
            index: b[5],
        });
    }
    let mut channels = [0u16; 8];
    for half in 0..2 {
        let o = 6 + 6 * half;
        let hi = [b[o + 4] & 0x0F, b[o + 4] >> 4, b[o + 5] & 0x0F, b[o + 5] >> 4];
        for k in 0..4 {
            channels[4 * half + k] = u16::from(b[o + k]) | u16::from(hi[k]) << 8;
        }
    }
    Some(Frsky::D8Channels {
        id: u16::from_be_bytes([b[1], b[2]]),
        counter: b[3],
        channels,
    })
}

/// The bytes of a D16 packet of length byte `len` (8 to 34): `p1 p2` the id, `p3` 1, `p4` the hop index (bits 0..5) and the
/// low bits of the skip, `p5` the rest of the skip, `p6` 3 for a bind packet, `fill` after it, the checksum last.
pub fn d16(id: u16, hop: u8, skip: u16, bind: bool, len: u8, fill: u8) -> Vec<u8> {
    let l = usize::from(len.clamp(8, 34));
    let mut p = vec![fill; l + 1];
    p[0] = l as u8;
    p[1..3].copy_from_slice(&id.to_be_bytes());
    p[3] = 1;
    p[4] = (hop & 0x3F) | ((skip & 3) as u8) << 6;
    p[5] = (skip >> 2) as u8;
    p[6] = if bind { 3 } else { 0 };
    let c = d16_crc(0, &p[3..l - 1]);
    p[l - 1..].copy_from_slice(&c.to_be_bytes());
    p
}

/// Reads a D16 packet. `None` for a length byte out of range or a wrong checksum.
pub fn parse_d16(b: &[u8]) -> Option<Frsky> {
    let l = usize::from(*b.first()?);
    if !(8..=34).contains(&l) || b.len() < l + 1 {
        return None;
    }
    if d16_crc(0, &b[3..l - 1]) != u16::from_be_bytes([b[l - 1], b[l]]) {
        return None;
    }
    Some(Frsky::D16 {
        id: u16::from_be_bytes([b[1], b[2]]),
        hop: b[4] & 0x3F,
        skip: u16::from(b[4] >> 6) + 4 * u16::from(b[5]),
        bind: b[6] == 3,
        len: l + 1,
    })
}
