//! Parts that the custom stacks share: the quasi-cyclic LDPC family, the Gray QAM map, bit fields, CRCs over bit arrays, the
//! station callsign rules and the default dials.

pub mod fit;
pub mod ldpc;
pub mod qam;

use sdrkit_util_bits::crc::catalog::{CRC16_IBM_3740, CRC32_ISO_HDLC};

/// Appends the `width` low bits of `value` to `out`, most significant first.
pub fn push_bits(out: &mut Vec<u8>, value: u64, width: usize) {
    for i in (0..width).rev() {
        out.push(((value >> i) & 1) as u8);
    }
}

/// The `width` bits of `bits` from `at`, most significant first, as a number.
pub fn read_bits(bits: &[u8], at: usize, width: usize) -> u64 {
    bits[at..at + width].iter().fold(0u64, |a, &b| (a << 1) | u64::from(b & 1))
}

/// Appends the bits of `bytes` to `out`, most significant bit of each byte first.
pub fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    for &b in bytes {
        push_bits(out, u64::from(b), 8);
    }
}

/// Packs `bits` into bytes, most significant bit first; a last partial byte is filled with zero bits.
pub fn pack(bits: &[u8]) -> Vec<u8> {
    bits.chunks(8)
        .map(|c| {
            let v = c.iter().fold(0u8, |a, &b| (a << 1) | (b & 1));
            v << (8 - c.len())
        })
        .collect()
}

/// The CRC-16 (CCITT-FALSE: polynomial 0x1021, register all ones) of a bit array, bit by bit.
pub fn crc16_bits(bits: &[u8]) -> u16 {
    CRC16_IBM_3740.bits(bits)
}

/// The CRC-16 (CCITT-FALSE) of bytes.
pub fn crc16(bytes: &[u8]) -> u16 {
    CRC16_IBM_3740.checksum(bytes)
}

/// The CRC-32 (zlib) of bytes.
pub fn crc32(bytes: &[u8]) -> u32 {
    CRC32_ISO_HDLC.checksum(bytes)
}

/// Bytes from hex digits (spaces allowed between them); `None` for any other character or an odd count.
pub fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_digit(16).map(|d| d as u8))
        .collect::<Option<_>>()?;
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    Some(digits.chunks(2).map(|p| (p[0] << 4) | p[1]).collect())
}

/// The callsign that a transmitter sends when its setting is empty.
pub const DEMO_CALLSIGN: &str = "TEST1";

/// The data file of the default dials of the custom stacks.
pub const DIALS_FILE: &str = "data/custom/custom_dials.csv";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_helpers_and_crcs() {
        let mut b = Vec::new();
        push_bits(&mut b, 0b1011, 4);
        push_bytes(&mut b, &[0xA5]);
        assert_eq!(read_bits(&b, 0, 4), 0b1011);
        assert_eq!(pack(&b), vec![0xBA, 0x50]);
        let mut nine = Vec::new();
        push_bytes(&mut nine, b"123456789");
        assert_eq!(crc16_bits(&nine), 0x29B1);
        assert_eq!(crc16(b"123456789"), 0x29B1);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(hex_bytes("54 45 53 54"), Some(b"TEST".to_vec()));
        assert_eq!(hex_bytes("5"), None);
        assert_eq!(hex_bytes("zz"), None);
    }
}
