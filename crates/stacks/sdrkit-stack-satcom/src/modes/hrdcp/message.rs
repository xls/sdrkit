//! The HRDCP message: the 12-byte header (BCH address, length, sequence, engineering word), the platform data and the
//! CRC-32; its coding into frames of three interleaved RS(255,223) words, randomised, with a tail byte; and the K = 7 code.
//!
//! Standard: EUMETSAT TD 16, Meteosat Data Collection and Distribution Service, sections 3.2 to 3.4 and table 4.

use crate::modes::goes::body::{encode_address, AddressCode};
use sdrkit_algo_fec::conv::{codes, Encoder};
use sdrkit_proto_ccsds::channel::{ChannelCoder, ChannelConfig, ChannelDecoder};
use sdrkit_proto_link::crc::CRC32_HRDCP;

/// Header bytes.
pub const HEADER_BYTES: usize = 12;
/// Data bytes of a coded frame: three RS(255,223) words.
pub const FRAME_DATA: usize = 3 * 223;
/// Bytes of a coded frame.
pub const FRAME_CODED: usize = 3 * 255;
/// The tail byte after the last frame.
pub const TAIL: u8 = 0x80;
/// The engineering word version.
pub const VERSION: u16 = 1;

/// The channel coding of a frame.
pub fn channel() -> ChannelConfig {
    ChannelConfig::rs_223(3)
}

/// What the header says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// The 21 data bits of the address.
    pub address21: u32,
    /// Platform data bytes, N.
    pub length: u16,
    /// The sequence counter.
    pub sequence: u16,
    /// An alert message (else self-timed).
    pub alert: bool,
    /// The compression: 0 none, 1 gzip.
    pub compression: u8,
    /// The 10 health bits.
    pub health: u16,
}

impl Header {
    /// The 12 header bytes.
    pub fn bytes(&self) -> [u8; HEADER_BYTES] {
        let mut b = [0u8; HEADER_BYTES];
        let word = address_word(self.address21) | 1;
        b[..4].copy_from_slice(&word.to_be_bytes());
        b[4..6].copy_from_slice(&self.length.to_be_bytes());
        b[6..8].copy_from_slice(&self.sequence.to_be_bytes());
        let eng = (VERSION << 13)
            | (u16::from(self.alert) << 12)
            | (u16::from(self.compression & 3) << 10)
            | (self.health & 0x3FF);
        b[8..10].copy_from_slice(&eng.to_be_bytes());
        b
    }
}

/// The 31-bit BCH address word of 21 data bits, left-aligned in 32 bits (bit 0 clear).
pub fn address_word(data21: u32) -> u32 {
    encode_address(data21)
        .iter()
        .fold(0u32, |a, &b| (a << 1) | u32::from(b))
        << 1
}

/// Coded frames of a message of `n` platform data bytes.
pub fn frames_of(n: usize) -> usize {
    (n + HEADER_BYTES + 4).div_ceil(FRAME_DATA)
}

/// The message bytes: header, data and CRC-32.
pub fn message(h: &Header, data: &[u8]) -> Vec<u8> {
    let mut m = h.bytes().to_vec();
    m.extend_from_slice(data);
    let crc = CRC32_HRDCP.checksum(&m);
    m.extend_from_slice(&crc.to_be_bytes());
    m
}

/// The coded bytes of a message: zero filled to whole frames, each frame Reed-Solomon coded and randomised, then the tail byte.
pub fn coded(msg: &[u8]) -> Vec<u8> {
    let frames = msg.len().div_ceil(FRAME_DATA);
    let mut padded = msg.to_vec();
    padded.resize(frames * FRAME_DATA, 0);
    let mut coder = ChannelCoder::new(channel()).expect("the CCSDS code");
    let mut out = Vec::with_capacity(frames * FRAME_CODED + 1);
    let mut one = Vec::with_capacity(FRAME_CODED + 4);
    for f in padded.chunks(FRAME_DATA) {
        one.clear();
        coder
            .encode(f, &mut one)
            .expect("a frame of the code's length");
        // The channel coder puts a marker first; HRDCP sends none.
        out.extend_from_slice(&one[4..]);
    }
    out.push(TAIL);
    out
}

/// The code symbols (G1, then G2 inverted) of `bytes`, from encoder state 0.
pub fn code_symbols(bytes: &[u8]) -> Vec<u8> {
    let bits: Vec<u8> = bytes
        .iter()
        .flat_map(|v| (0..8).rev().map(move |i| (v >> i) & 1))
        .collect();
    let mut out = Vec::with_capacity(2 * bits.len());
    Encoder::new(codes::CCSDS_K7).encode(&bits, &mut out);
    out
}

/// The header of a decoded first frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeaderRead {
    /// The corrected 31-bit address word, left-aligned in 32 bits.
    pub address: u32,
    /// Address bits corrected.
    pub corrected: usize,
    /// N.
    pub length: usize,
    /// The sequence counter.
    pub sequence: u16,
    /// An alert message.
    pub alert: bool,
    /// The compression field.
    pub compression: u8,
    /// The health bits.
    pub health: u16,
}

/// Reads the header of the first frame's data. `None` when the address does not decode or the version is not 1.
pub fn read_header(b: &[u8], code: &mut AddressCode) -> Option<HeaderRead> {
    if b.len() < HEADER_BYTES {
        return None;
    }
    let word = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    let mut bits: Vec<bool> = (0..31).map(|i| (word >> (31 - i)) & 1 == 1).collect();
    let corrected = code.decode(&mut bits)?;
    let address = bits.iter().fold(0u32, |a, &x| (a << 1) | u32::from(x)) << 1;
    let eng = u16::from_be_bytes([b[8], b[9]]);
    if eng >> 13 != VERSION {
        return None;
    }
    let length = usize::from(u16::from_be_bytes([b[4], b[5]]));
    if length == 0 {
        return None;
    }
    Some(HeaderRead {
        address,
        corrected,
        length,
        sequence: u16::from_be_bytes([b[6], b[7]]),
        alert: eng & 0x1000 != 0,
        compression: ((eng >> 10) & 3) as u8,
        health: eng & 0x3FF,
    })
}

/// A message decoded from its coded frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decoded {
    /// The header.
    pub header: HeaderRead,
    /// The platform data.
    pub data: Vec<u8>,
    /// The CRC passed.
    pub crc_ok: bool,
    /// Coded frames.
    pub frames: usize,
    /// Frames with a word that did not decode.
    pub frames_bad: usize,
    /// Reed-Solomon bytes corrected.
    pub rs_corrected: u32,
}

/// Decodes the coded frames `coded` (whole frames of 765 bytes): each frame derandomised and corrected, the header read from
/// the first, the data cut to N and the CRC checked with the corrected address put back. `None` when the first frame or its
/// header does not decode.
pub fn decode(
    coded: &mut [u8],
    rs: &mut ChannelDecoder,
    code: &mut AddressCode,
) -> Option<Decoded> {
    let mut out = Decoded::default();
    let mut msg = Vec::with_capacity(coded.len());
    for (i, f) in coded.chunks_mut(FRAME_CODED).enumerate() {
        if f.len() < FRAME_CODED {
            break;
        }
        let r = rs.decode(f).ok()?;
        if i == 0 && !r.ok() {
            return None;
        }
        out.frames += 1;
        out.frames_bad += usize::from(!r.ok());
        out.rs_corrected += r.corrected;
        msg.extend_from_slice(&f[..FRAME_DATA]);
    }
    let h = read_header(&msg, code)?;
    let n = h.length;
    if msg.len() < n + HEADER_BYTES + 4 {
        out.header = h;
        return Some(out);
    }
    // The CRC covers the address as corrected, with bit 0 as received.
    let word = h.address | u32::from(msg[3] & 1);
    msg[..4].copy_from_slice(&word.to_be_bytes());
    let end = HEADER_BYTES + n;
    let crc = u32::from_be_bytes([msg[end], msg[end + 1], msg[end + 2], msg[end + 3]]);
    out.crc_ok = CRC32_HRDCP.checksum(&msg[..end]) == crc;
    out.data = msg[HEADER_BYTES..end].to_vec();
    out.header = h;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_of_the_worked_example() {
        assert_eq!(address_word(0x2C412) >> 1, 0xB104B62);
        assert_eq!(address_word(0x2C412), 0x1620_96C4);
        let h = Header {
            address21: 0x2C412,
            length: 1234,
            sequence: 65535,
            alert: true,
            compression: 1,
            health: 0x2A5,
        };
        assert_eq!(
            h.bytes(),
            [0x16, 0x20, 0x96, 0xC5, 0x04, 0xD2, 0xFF, 0xFF, 0x36, 0xA5, 0x00, 0x00]
        );
    }

    #[test]
    fn frame_counts() {
        for (n, f) in [(653, 1), (654, 2), (1322, 2), (7343, 11)] {
            assert_eq!(frames_of(n), f, "{n}");
        }
        let h = Header {
            address21: 0x5800A,
            length: 700,
            ..Header::default()
        };
        let m = message(&h, &[7u8; 700]);
        assert_eq!(coded(&m).len(), 2 * 765 + 1);
    }

    #[test]
    fn a_message_round_trips_with_errors() {
        let data: Vec<u8> = (0..1000).map(|i| ((131 * i + 7) % 256) as u8).collect();
        let h = Header {
            address21: 0x5800A,
            length: data.len() as u16,
            sequence: 300,
            alert: true,
            compression: 0,
            health: 0x0F3,
        };
        let m = message(&h, &data);
        let mut c = coded(&m);
        let n = frames_of(data.len()) * FRAME_CODED;
        for k in 0..20 {
            c[k * 37] ^= 0x5A;
        }
        let mut rs = ChannelDecoder::new(channel()).unwrap();
        let mut code = AddressCode::new();
        let d = decode(&mut c[..n], &mut rs, &mut code).unwrap();
        assert!(d.crc_ok);
        assert!(d.rs_corrected >= 20);
        assert_eq!(d.data, data);
        assert_eq!(
            (d.header.sequence, d.header.health, d.header.alert),
            (300, 0xF3, true)
        );
        // Too many errors in frame 2: the message is complete, the CRC bad, one frame bad.
        let mut c = coded(&m);
        for k in 0..60 {
            c[765 + k * 3] ^= 0xFF;
        }
        let d = decode(&mut c[..n], &mut rs, &mut code).unwrap();
        assert!(!d.crc_ok);
        assert_eq!((d.frames, d.frames_bad), (2, 1));
    }
}
