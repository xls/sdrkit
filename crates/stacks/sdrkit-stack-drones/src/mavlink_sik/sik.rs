//! SiK telemetry radio packets after the sync word: the packet of the radio's packet handler (net id, length, data, CRC) and
//! the packet with Golay (23,12) error correction, and the 2-byte trailer at the end of the data. No public document gives
//! the layout; the coverage of the packet handler's CRC (net id, length and data) is assumed.

use sdrkit_algo_fec::small::SmallCode;
use sdrkit_util_bits::crc::catalog::CRC16_CMS;

/// The air rates of the radios, bit/s.
pub const AIR_RATES: [f64; 13] = [
    2e3, 4e3, 8e3, 16e3, 19e3, 24e3, 32e3, 48e3, 64e3, 96e3, 128e3, 192e3, 250e3,
];
/// The sync word after the preamble.
pub const SYNC: [u8; 2] = [0x2D, 0xD4];
/// The longest data of one packet, bytes.
pub const MAX_DATA: usize = 252;

/// The trailer at the end of the data (little-endian): bits 0..12 the TDM window, bit 13 command, bit 14 bonus, bit 15 resend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Trailer {
    /// The TDM window.
    pub window: u16,
    /// A command packet.
    pub command: bool,
    /// Sent in the other side's time.
    pub bonus: bool,
    /// A resent packet.
    pub resend: bool,
}

impl Trailer {
    /// The trailer of the two bytes `b`.
    pub fn read(b: [u8; 2]) -> Trailer {
        let v = u16::from_le_bytes(b);
        Trailer {
            window: v & 0x1FFF,
            command: v & 1 << 13 != 0,
            bonus: v & 1 << 14 != 0,
            resend: v & 1 << 15 != 0,
        }
    }
    /// The two bytes.
    pub fn bytes(&self) -> [u8; 2] {
        let v = (self.window & 0x1FFF)
            | u16::from(self.command) << 13
            | u16::from(self.bonus) << 14
            | u16::from(self.resend) << 15;
        v.to_le_bytes()
    }
}

/// A packet read from the air.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    /// The net id.
    pub net_id: u16,
    /// The data without the trailer (the MAVLink bytes).
    pub data: Vec<u8>,
    /// The trailer.
    pub trailer: Trailer,
    /// Sent with error correction.
    pub ecc: bool,
    /// Bits the Golay code corrected.
    pub corrected: u32,
    /// Bytes the packet took after the sync word.
    pub air_len: usize,
}

/// The SiK CRC of `data`: a 16-bit register `c = 0`; for each byte `b`, `t` is `c` with its low 8 bits cleared, shifted left 8
/// times through the polynomial 1021 (feedback on the top bit), and `c = ((c << 8) | b) ^ t`.
pub fn crc(data: &[u8]) -> u16 {
    let mut c = 0u16;
    for &b in data {
        let mut t = c & 0xFF00;
        for _ in 0..8 {
            t = if t & 0x8000 != 0 {
                (t << 1) ^ 0x1021
            } else {
                t << 1
            };
        }
        c = ((c << 8) | u16::from(b)) ^ t;
    }
    c
}

/// The Golay (23,12) code of the error-corrected packets.
pub fn golay() -> SmallCode {
    SmallCode::golay_23_12()
}

/// Codes 3 bytes into 6: two 23-bit words, each the 12 information bits above the 11 check bits.
pub fn golay_encode(code: &SmallCode, b: [u8; 3], out: &mut Vec<u8>) {
    let v = u128::from(b[0]) | u128::from(b[1] & 0x0F) << 8;
    let w = u128::from(b[2]) | u128::from(b[1] & 0xF0) << 4;
    for x in [v, w] {
        let c = code.encode(x) as u32;
        out.extend([c as u8, (c >> 8) as u8, (c >> 16) as u8 & 0x7F]);
    }
}

/// Decodes 6 bytes into 3; `None` when a word holds more errors than the code corrects. The bits corrected are added to
/// `corrected`.
pub fn golay_decode(code: &SmallCode, g: &[u8], corrected: &mut u32) -> Option<[u8; 3]> {
    let word = |o: usize| u128::from(g[o]) | u128::from(g[o + 1]) << 8 | u128::from(g[o + 2] & 0x7F) << 16;
    let mut info = [0u16; 2];
    for (k, i) in info.iter_mut().enumerate() {
        let (cw, n) = code.decode(word(3 * k))?;
        *corrected += u32::from(n);
        *i = code.info(cw) as u16;
    }
    Some([
        info[0] as u8,
        ((info[0] >> 8) as u8 & 0x0F) | ((info[1] >> 4) as u8 & 0xF0),
        info[1] as u8,
    ])
}

/// The bytes after the sync word of a packet of `net_id` with `data` (the trailer included), without error correction.
pub fn build_plain(net_id: u16, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 5);
    out.extend(net_id.to_be_bytes());
    out.push(data.len() as u8);
    out.extend_from_slice(data);
    let c = CRC16_CMS.checksum(&out);
    out.extend(c.to_be_bytes());
    out
}

/// The bytes after the sync word of a packet of `net_id` with `data` (the trailer included), with error correction: the
/// length `H`, the coded header (net id low, net id high, length), the coded CRC (low, high, length) and the coded data padded
/// with zeros to a multiple of 3.
pub fn build_ecc(code: &SmallCode, net_id: u16, data: &[u8]) -> Vec<u8> {
    let l = data.len() as u8;
    let mut body = Vec::with_capacity(12 + 2 * data.len() + 6);
    let id = net_id.to_le_bytes();
    golay_encode(code, [id[0], id[1], l], &mut body);
    let c = crc(data).to_le_bytes();
    golay_encode(code, [c[0], c[1], l], &mut body);
    for chunk in data.chunks(3) {
        let mut b = [0u8; 3];
        b[..chunk.len()].copy_from_slice(chunk);
        golay_encode(code, b, &mut body);
    }
    let mut out = Vec::with_capacity(body.len() + 1);
    out.push(body.len() as u8);
    out.extend(body);
    out
}

fn split(net_id: u16, all: &[u8], ecc: bool, corrected: u32, air_len: usize) -> Option<Packet> {
    if all.len() < 2 {
        return None;
    }
    let (data, t) = all.split_at(all.len() - 2);
    Some(Packet {
        net_id,
        data: data.to_vec(),
        trailer: Trailer::read([t[0], t[1]]),
        ecc,
        corrected,
        air_len,
    })
}

/// Reads a packet without error correction from the bytes after the sync word. `None` when the length or the CRC is wrong.
pub fn parse_plain(b: &[u8]) -> Option<Packet> {
    let l = usize::from(*b.get(2)?);
    if !(2..=MAX_DATA).contains(&l) || b.len() < 5 + l {
        return None;
    }
    let c = u16::from_be_bytes([b[3 + l], b[4 + l]]);
    if CRC16_CMS.checksum(&b[..3 + l]) != c {
        return None;
    }
    split(u16::from_be_bytes([b[0], b[1]]), &b[3..3 + l], false, 0, 5 + l)
}

/// Reads a packet with error correction from the bytes after the sync word. `None` when the length does not fit, a word cannot
/// be corrected, the two copies of the length differ, or the CRC is wrong.
pub fn parse_ecc(code: &SmallCode, b: &[u8]) -> Option<Packet> {
    let h = usize::from(*b.first()?);
    if h < 12 || h % 6 != 0 || b.len() < 1 + h {
        return None;
    }
    let mut corrected = 0;
    let head = golay_decode(code, &b[1..7], &mut corrected)?;
    let crcb = golay_decode(code, &b[7..13], &mut corrected)?;
    let l = usize::from(head[2]);
    if head[2] != crcb[2] || !(2..=MAX_DATA).contains(&l) || 12 + l.div_ceil(3) * 6 != h {
        return None;
    }
    let mut data = Vec::with_capacity(l + 2);
    for g in b[13..1 + h].as_chunks::<6>().0 {
        data.extend(golay_decode(code, g, &mut corrected)?);
    }
    data.truncate(l);
    if crc(&data) != u16::from_le_bytes([crcb[0], crcb[1]]) {
        return None;
    }
    split(u16::from_le_bytes([head[0], head[1]]), &data, true, corrected, 1 + h)
}
