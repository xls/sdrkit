//! The CubeSat Space Protocol (CSP) version 1 header and CRC32, and the GomSpace AX100 Mode 5 and Mode 6 framings that carry
//! CSP packets.
//!
//! Standard: libcsp 1.x, GomSpace AX100 manual

use crate::{Error, Result};
use crate::{Param, Part};
use sdrkit_algo_fec::rs::{codes, Rs};
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_util_bits::crc::catalog::CRC32_ISCSI;
use sdrkit_util_bits::crc::Trailer;
use sdrkit_util_bits::seq;

/// The CSP version 1 header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Header {
    /// Priority, 0 (critical) to 3 (low).
    pub priority: u8,
    /// Source address, 0 to 31.
    pub source: u8,
    /// Destination address, 0 to 31.
    pub destination: u8,
    /// Destination port, 0 to 63.
    pub dest_port: u8,
    /// Source port, 0 to 63.
    pub source_port: u8,
    /// The low 8 bits: reserved (4), then [`FLAG_HMAC`], [`FLAG_XTEA`], [`FLAG_RDP`], [`FLAG_CRC32`].
    pub flags: u8,
}

/// The packet carries an HMAC-SHA1 authentication code.
pub const FLAG_HMAC: u8 = 0x08;
/// The packet is XTEA encrypted.
pub const FLAG_XTEA: u8 = 0x04;
/// The packet belongs to an RDP (reliable datagram) connection.
pub const FLAG_RDP: u8 = 0x02;
/// The packet ends with a CRC32.
pub const FLAG_CRC32: u8 = 0x01;

impl Header {
    /// Reads the header from the first 4 bytes.
    pub fn parse(b: &[u8]) -> Result<Header> {
        let b = b.get(..4).ok_or(Error::Short)?;
        let v = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        Ok(Header {
            priority: (v >> 30) as u8,
            source: ((v >> 25) & 0x1F) as u8,
            destination: ((v >> 20) & 0x1F) as u8,
            dest_port: ((v >> 14) & 0x3F) as u8,
            source_port: ((v >> 8) & 0x3F) as u8,
            flags: v as u8,
        })
    }

    /// The 4 bytes of the header. Fields wider than their place are cut to it.
    pub fn to_bytes(&self) -> [u8; 4] {
        let v = (self.priority as u32 & 3) << 30
            | (self.source as u32 & 0x1F) << 25
            | (self.destination as u32 & 0x1F) << 20
            | (self.dest_port as u32 & 0x3F) << 14
            | (self.source_port as u32 & 0x3F) << 8
            | self.flags as u32;
        v.to_be_bytes()
    }
}

/// What the CSP CRC32 covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CrcScope {
    /// The data after the header (libcsp 1.x).
    Data,
    /// The header and the data (libcsp with `CSP_CRC32_INCLUDE_HEADER`).
    HeaderAndData,
}

fn crc_range(packet_len: usize, scope: CrcScope) -> Result<usize> {
    if packet_len < 4 {
        return Err(Error::Short);
    }
    Ok(match scope {
        CrcScope::Data => 4,
        CrcScope::HeaderAndData => 0,
    })
}

/// Appends the CRC32 to a packet (header and data). The caller sets [`FLAG_CRC32`] in the header.
pub fn append_crc32(packet: &mut Vec<u8>, scope: CrcScope) -> Result<()> {
    let from = crc_range(packet.len(), scope)?;
    let c = CRC32_ISCSI.checksum(&packet[from..]);
    packet.extend_from_slice(&c.to_be_bytes());
    Ok(())
}

/// Whether the last 4 bytes of a packet are the CRC32 of the rest.
pub fn check_crc32(packet: &[u8], scope: CrcScope) -> bool {
    match crc_range(packet.len().saturating_sub(4), scope) {
        Ok(from) => CRC32_ISCSI.verify(&packet[from..], Trailer::Big),
        Err(_) => false,
    }
}

/// The AX100 attached sync word of Mode 5, sent most significant bit first.
pub const AX100_SYNC: u32 = 0x930B_51DE;
/// Mode 5 length field flag: the packet is convolutionally coded (not used by the AX100 in practice).
pub const FLAG_VITERBI: u16 = 0x100;
/// Mode 5 length field flag: the coded packet is CCSDS randomised.
pub const FLAG_SCRAMBLER: u16 = 0x200;
/// Mode 5 length field flag: the packet is Reed-Solomon coded.
pub const FLAG_RS: u16 = 0x400;

/// The rows of the Golay parity matrix B, data bit 11 first: an 11 by 11 circulant of 11011100010 bordered with ones.
const fn golay_rows() -> [u32; 12] {
    let first: [u32; 11] = [1, 1, 0, 1, 1, 1, 0, 0, 0, 1, 0];
    let mut rows = [0u32; 12];
    let mut r = 0;
    while r < 11 {
        let mut v = 0u32;
        let mut c = 0;
        while c < 11 {
            if first[(c + 11 - r) % 11] == 1 {
                v |= 1 << (11 - c);
            }
            c += 1;
        }
        rows[r] = v | 1;
        r += 1;
    }
    rows[11] = 0xFFE;
    rows
}

/// The AX100 framings: Mode 5 (a sync word, a Golay (24,12) length field, the CCSDS randomiser and CCSDS RS(255,223)) and
/// Mode 6 (a Reed-Solomon coded packet in an HDLC frame without FCS). The Golay check bits go before the data bits. It holds the Reed-Solomon decoder, the Golay code and a work buffer, all made in [`Ax100::new`].
/// Decoding then allocates nothing.
///
/// ```
/// use sdrkit_proto_link::csp::{Ax100, Header};
/// let h = Header { priority: 2, source: 1, destination: 10, dest_port: 31, source_port: 7, flags: 0 };
/// let mut packet = h.to_bytes().to_vec();
/// packet.extend_from_slice(b"HELLO");
/// let mut ax = Ax100::new().unwrap();
/// let mut frame = Vec::new();
/// ax.mode5_encode(&packet, &mut frame).unwrap();
/// let (len, flags, _) = ax.mode5_length([frame[0], frame[1], frame[2]]).unwrap();
/// let mut coded = frame[3..3 + len].to_vec();
/// let (n, _) = ax.mode5_decode(&mut coded, flags).unwrap();
/// assert_eq!(&coded[..n], &packet[..]);
/// ```
#[derive(Clone, Debug)]
pub struct Ax100 {
    rs: Rs,
    golay: SmallCode,
    word: [u8; 255],
}

impl Ax100 {
    /// The coders.
    pub fn new() -> Result<Ax100> {
        Ok(Ax100 {
            rs: Rs::new(codes::CCSDS_255_223_CONVENTIONAL).map_err(Error::Fec)?,
            golay: SmallCode::from_parity_rows(24, 12, &golay_rows()).map_err(Error::Fec)?,
            word: [0; 255],
        })
    }

    /// The 24-bit Mode 5 length field (as sent: check bits, then data bits) for a value of 12 bits.
    pub fn golay_encode(&self, value: u16) -> u32 {
        let w = self.golay.encode((value & 0xFFF) as u128) as u32;
        // SmallCode puts the data first. AX100 sends the check bits first.
        ((w & 0xFFF) << 12) | (w >> 12)
    }

    /// Decodes a 24-bit length field: the 12-bit value and the bits corrected. An error past 3 bit errors.
    pub fn golay_decode(&self, field: u32) -> Result<(u16, u8)> {
        let w = ((field & 0xFFF) << 12) | ((field >> 12) & 0xFFF);
        let (cw, n) = self.golay.decode(w as u128).ok_or(Error::Uncorrectable)?;
        Ok((self.golay.info(cw) as u16, n))
    }

    /// Reads the length field after the sync word: the coded length in bytes, the flags, and the bits corrected.
    pub fn mode5_length(&self, field: [u8; 3]) -> Result<(usize, u16, u8)> {
        let f = u32::from_be_bytes([0, field[0], field[1], field[2]]);
        let (v, n) = self.golay_decode(f)?;
        Ok(((v & 0xFF) as usize, v & 0xF00, n))
    }

    fn rs_encode(&mut self, data: &[u8], out: &mut Vec<u8>) -> Result<()> {
        if data.len() > 223 {
            return Err(Error::TooLong);
        }
        let pad = 223 - data.len();
        self.word[..pad].fill(0);
        self.word[pad..223].copy_from_slice(data);
        self.rs.encode_word(&mut self.word).map_err(Error::Fec)?;
        out.extend_from_slice(&self.word[pad..]);
        Ok(())
    }

    /// Decodes a shortened RS(255,223) word in place. Returns the data length and the symbols corrected.
    fn rs_decode(&mut self, coded: &mut [u8]) -> Result<(usize, u32)> {
        if coded.len() <= 32 || coded.len() > 255 {
            return Err(Error::Syntax(Part::RsWordLength));
        }
        let pad = 255 - coded.len();
        self.word[..pad].fill(0);
        self.word[pad..].copy_from_slice(coded);
        let d = self
            .rs
            .decode(&mut self.word, &[])
            .map_err(|_| Error::Uncorrectable)?;
        coded.copy_from_slice(&self.word[pad..]);
        Ok((coded.len() - 32, d.corrected))
    }

    /// Appends a Mode 5 frame after the sync word: the length field (3 bytes) and the packet, RS coded and randomised. The
    /// packet must be 1 to 223 bytes.
    pub fn mode5_encode(&mut self, packet: &[u8], out: &mut Vec<u8>) -> Result<()> {
        if packet.is_empty() {
            return Err(Error::Short);
        }
        let start = out.len() + 3;
        out.extend_from_slice(&[0, 0, 0]);
        self.rs_encode(packet, out)?;
        let coded_len = out.len() - start;
        seq::CCSDS.apply(&mut out[start..], 0);
        let field = self.golay_encode(coded_len as u16 | FLAG_RS | FLAG_SCRAMBLER);
        out[start - 3..start].copy_from_slice(&field.to_be_bytes()[1..]);
        Ok(())
    }

    /// Decodes the coded part of a Mode 5 frame in place, by its flags: derandomise, then RS decode. Returns the packet length
    /// (the packet is at the start of `coded`) and the symbols corrected.
    pub fn mode5_decode(&mut self, coded: &mut [u8], flags: u16) -> Result<(usize, u32)> {
        if flags & FLAG_VITERBI != 0 {
            return Err(Error::BadParameter(Param::Mode5Convolutional));
        }
        if flags & FLAG_SCRAMBLER != 0 {
            seq::CCSDS.apply(coded, 0);
        }
        if flags & FLAG_RS != 0 {
            self.rs_decode(coded)
        } else {
            Ok((coded.len(), 0))
        }
    }

    /// Appends the Mode 6 HDLC payload of a packet: the packet and its 32 RS parity bytes. Send it in an HDLC frame with no FCS.
    pub fn mode6_encode(&mut self, packet: &[u8], out: &mut Vec<u8>) -> Result<()> {
        if packet.is_empty() {
            return Err(Error::Short);
        }
        self.rs_encode(packet, out)
    }

    /// Decodes a Mode 6 HDLC payload in place. Returns the packet length and the symbols corrected.
    pub fn mode6_decode(&mut self, payload: &mut [u8]) -> Result<(usize, u32)> {
        self.rs_decode(payload)
    }
}
