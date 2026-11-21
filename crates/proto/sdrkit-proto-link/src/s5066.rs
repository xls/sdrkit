//! STANAG 5066 Annex C D_PDUs found in a bit stream: the sync bytes, the header with its CRC-16, and the data segment with its
//! CRC-32. The bits of each byte arrive least significant first.
//!
//! Layout (bytes from the first sync byte): 0 and 1 the sync `90 EB`; 2 the D_PDU type (high 4 bits) and the high 4 bits of the
//! engineering orderwire; 3 the low 8 bits of the orderwire; 4 the EOT; 5 the address size `A` in bytes (high 3 bits) and the
//! header size `H` (low 5 bits, at least 4, counted from byte 2 and without the address); then `A` address bytes (the first half
//! of their hex digits the destination, the second half the source); then the rest of the header; then the header CRC (2 bytes,
//! low byte first) over bytes 2 to `2 + H + A - 1`. Types 0, 2, 4, 7 and 8 carry a segment: its size is the low 2 bits of the byte
//! after the address and the next byte (10 bits); the segment follows the header CRC, then its CRC-32 (4 bytes, low byte first).

use sdrkit_util_bits::crc::{Crc, CrcSpec};

/// The header CRC: polynomial `9949` processed reflected, register 0, no final XOR.
pub const CRC16_DPDU: Crc<u16> = Crc::<u16>::new(CrcSpec::new(
    "CRC-16/S5066-HEADER",
    16,
    0x9949,
    0,
    true,
    true,
    0,
    0x7D01,
));

/// The segment CRC: polynomial `0AA725CF` processed reflected, register 0, no final XOR.
pub const CRC32_DPDU: Crc<u32> = Crc::<u32>::new(CrcSpec::new(
    "CRC-32/S5066-DATA",
    32,
    0x0AA7_25CF,
    0,
    true,
    true,
    0,
    0x6738_A384,
));

/// The two sync bytes.
pub const SYNC: [u8; 2] = [0x90, 0xEB];
/// The most bytes buffered for one D_PDU.
pub const MAX_BYTES: usize = 1200;

/// The name of D_PDU type `t`.
pub fn type_name(t: u8) -> &'static str {
    match t {
        0 => "DATA_ONLY",
        1 => "ACK_ONLY",
        2 => "DATA_ACK",
        3 => "RESET_WIN_RESYNC",
        4 => "EXP_DATA_ONLY",
        5 => "EXP_ACK_ONLY",
        6 => "MANAGEMENT",
        7 => "NON_ARQ_DATA",
        8 => "EXP_NON_ARQ_DATA",
        15 => "WARNING",
        _ => "reserved",
    }
}

/// Whether D_PDU type `t` carries a data segment.
pub fn has_segment(t: u8) -> bool {
    matches!(t, 0 | 2 | 4 | 7 | 8)
}

/// The check of a data segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataCrc {
    /// The CRC-32 matches.
    Good,
    /// The CRC-32 does not match.
    Bad,
    /// The type has no segment.
    None,
}

impl DataCrc {
    /// "good", "bad" or "none".
    pub fn name(self) -> &'static str {
        match self {
            DataCrc::Good => "good",
            DataCrc::Bad => "bad",
            DataCrc::None => "none",
        }
    }
}

/// One D_PDU with a good header CRC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DPdu {
    /// The type, 0 to 15.
    pub kind: u8,
    /// The engineering orderwire, 12 bits.
    pub eow: u16,
    /// The EOT byte.
    pub eot: u8,
    /// The destination address, hex digits.
    pub destination: String,
    /// The source address, hex digits.
    pub source: String,
    /// The header bytes from byte 2 to the header CRC (the address included).
    pub header: Vec<u8>,
    /// The data segment (empty for a type without one).
    pub segment: Vec<u8>,
    /// The check of the segment.
    pub data_crc: DataCrc,
}

/// What [`encode`] writes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DPduSpec {
    /// The type, 0 to 15.
    pub kind: u8,
    /// The engineering orderwire, 12 bits.
    pub eow: u16,
    /// The EOT byte.
    pub eot: u8,
    /// The address bytes (at most 7).
    pub address: Vec<u8>,
    /// The header bytes after the address (the first two hold the segment size of a type with a segment, which [`encode`]
    /// writes). Byte 5 counts 4 more than these.
    pub rest: Vec<u8>,
    /// The data segment (at most 1023 bytes).
    pub segment: Vec<u8>,
}

/// The bytes of a D_PDU: sync, header, header CRC, and for a type with a segment the segment and its CRC-32. `None` when a size
/// does not fit its field.
pub fn encode(s: &DPduSpec) -> Option<Vec<u8>> {
    let a = s.address.len();
    let mut rest = s.rest.clone();
    if has_segment(s.kind) {
        if s.segment.len() > 1023 {
            return None;
        }
        while rest.len() < 2 {
            rest.push(0);
        }
        rest[0] = (rest[0] & !3) | (s.segment.len() >> 8) as u8;
        rest[1] = s.segment.len() as u8;
    }
    let h = 4 + rest.len();
    if a > 7 || h > 31 || s.kind > 15 {
        return None;
    }
    let mut b = SYNC.to_vec();
    b.push((s.kind << 4) | ((s.eow >> 8) & 0xF) as u8);
    b.push(s.eow as u8);
    b.push(s.eot);
    b.push(((a as u8) << 5) | h as u8);
    b.extend_from_slice(&s.address);
    b.extend_from_slice(&rest);
    let c = CRC16_DPDU.checksum(&b[2..]);
    b.extend_from_slice(&c.to_le_bytes());
    if has_segment(s.kind) {
        b.extend_from_slice(&s.segment);
        let c = CRC32_DPDU.checksum(&s.segment);
        b.extend_from_slice(&c.to_le_bytes());
    }
    Some(b)
}

/// The hex digits of `bytes`.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02X}");
    }
    s
}

/// Parses the D_PDU at the start of `b` (its sync bytes first). `Ok(None)` while more bytes are needed; `Err(())` when the bytes
/// cannot be a D_PDU (a bad header CRC, a header size below 4, too long).
fn parse(b: &[u8]) -> Result<Option<DPdu>, ()> {
    if b.len() < 6 {
        return Ok(None);
    }
    let a = usize::from(b[5] >> 5);
    let h = usize::from(b[5] & 0x1F);
    if h < 4 {
        return Err(());
    }
    let crc_at = 2 + h + a;
    if b.len() < crc_at + 2 {
        return Ok(None);
    }
    let got = u16::from_le_bytes([b[crc_at], b[crc_at + 1]]);
    if CRC16_DPDU.checksum(&b[2..crc_at]) != got {
        return Err(());
    }
    let kind = b[2] >> 4;
    let addr = &b[6..6 + a];
    let digits = hex(addr);
    let (destination, source) = digits.split_at(a);
    let mut d = DPdu {
        kind,
        eow: (u16::from(b[2] & 0xF) << 8) | u16::from(b[3]),
        eot: b[4],
        destination: destination.to_owned(),
        source: source.to_owned(),
        header: b[2..crc_at].to_vec(),
        segment: Vec::new(),
        data_crc: DataCrc::None,
    };
    if !has_segment(kind) || h < 6 {
        return Ok(Some(d));
    }
    let size = (usize::from(b[6 + a] & 3) << 8) | usize::from(b[7 + a]);
    let seg_at = crc_at + 2;
    let end = seg_at + size + 4;
    if end > MAX_BYTES {
        return Err(());
    }
    if b.len() < end {
        return Ok(None);
    }
    d.segment = b[seg_at..seg_at + size].to_vec();
    let c = u32::from_le_bytes([b[end - 4], b[end - 3], b[end - 2], b[end - 1]]);
    d.data_crc = if CRC32_DPDU.checksum(&d.segment) == c {
        DataCrc::Good
    } else {
        DataCrc::Bad
    };
    Ok(Some(d))
}

/// Finds D_PDUs in a stream of bits (one bit per call, each byte least significant bit first). A D_PDU is reported once its last
/// byte is in and its header CRC is good.
#[derive(Clone, Debug, Default)]
pub struct DPduFinder {
    window: u16,
    seen: u8,
    bytes: Vec<u8>,
    acc: u8,
    nbits: u8,
    open: bool,
}

impl DPduFinder {
    /// A finder hunting for the sync bytes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets a D_PDU half received and hunts again.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Takes the next bit (its lowest bit); returns a D_PDU when this bit completes one.
    pub fn push_bit(&mut self, bit: u8) -> Option<DPdu> {
        let bit = bit & 1;
        if !self.open {
            self.window = (self.window >> 1) | (u16::from(bit) << 15);
            self.seen = self.seen.saturating_add(1);
            if self.seen >= 16 && self.window == u16::from_le_bytes(SYNC) {
                self.open = true;
                self.bytes.clear();
                self.bytes.extend_from_slice(&SYNC);
                self.acc = 0;
                self.nbits = 0;
            }
            return None;
        }
        self.acc |= bit << self.nbits;
        self.nbits += 1;
        if self.nbits < 8 {
            return None;
        }
        self.bytes.push(self.acc);
        self.acc = 0;
        self.nbits = 0;
        match parse(&self.bytes) {
            Ok(None) => None,
            Ok(Some(d)) => {
                self.close();
                Some(d)
            }
            Err(()) => {
                self.close();
                None
            }
        }
    }

    fn close(&mut self) {
        self.open = false;
        self.window = 0;
        self.seen = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_values() {
        assert_eq!(CRC16_DPDU.checksum(b"123456789"), 0x7D01);
        assert_eq!(CRC32_DPDU.checksum(b"123456789"), 0x6738_A384);
    }

    fn bits_lsb(bytes: &[u8]) -> Vec<u8> {
        bytes
            .iter()
            .flat_map(|b| (0..8).map(move |i| (b >> i) & 1))
            .collect()
    }

    #[test]
    fn a_non_arq_data_pdu_round_trips() {
        let payload: Vec<u8> = (0..50u8).collect();
        let s = DPduSpec {
            kind: 7,
            eow: 0xABC,
            eot: 9,
            address: vec![0x56, 0x78, 0x12, 0x34],
            rest: vec![0, 0, 1, 2, 3],
            segment: payload.clone(),
        };
        let b = encode(&s).unwrap();
        let mut bits = vec![1, 0, 1, 1, 0, 0, 1];
        bits.extend(bits_lsb(&b));
        bits.extend([0, 1, 1]);
        let mut f = DPduFinder::new();
        let found: Vec<DPdu> = bits.iter().filter_map(|&x| f.push_bit(x)).collect();
        assert_eq!(found.len(), 1);
        let d = &found[0];
        assert_eq!(type_name(d.kind), "NON_ARQ_DATA");
        assert_eq!(d.eow, 0xABC);
        assert_eq!(d.eot, 9);
        assert_eq!(d.destination, "5678");
        assert_eq!(d.source, "1234");
        assert_eq!(d.segment, payload);
        assert_eq!(d.data_crc, DataCrc::Good);
    }

    #[test]
    fn a_damaged_header_or_segment_is_caught() {
        let s = DPduSpec {
            kind: 0,
            address: vec![1, 2],
            segment: vec![7; 10],
            ..DPduSpec::default()
        };
        let mut b = encode(&s).unwrap();
        let n = b.len();
        b[n - 6] ^= 1;
        let mut f = DPduFinder::new();
        let d: Vec<DPdu> = bits_lsb(&b).iter().filter_map(|&x| f.push_bit(x)).collect();
        assert_eq!(d[0].data_crc, DataCrc::Bad);
        b[3] ^= 0x10;
        let mut f = DPduFinder::new();
        assert!(bits_lsb(&b).iter().all(|&x| f.push_bit(x).is_none()));
        let ack = DPduSpec {
            kind: 1,
            address: vec![0xAA],
            ..DPduSpec::default()
        };
        let b = encode(&ack).unwrap();
        let mut f = DPduFinder::new();
        let d: Vec<DPdu> = bits_lsb(&b).iter().filter_map(|&x| f.push_bit(x)).collect();
        assert_eq!(d[0].data_crc, DataCrc::None);
        assert_eq!(d[0].destination.len(), 1);
    }
}
