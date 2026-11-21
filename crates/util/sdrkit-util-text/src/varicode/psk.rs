//! The PSK31 varicode (P. Martinez G3PLX, "PSK31: a new radio-teletype mode", RadCom 1998-1999, and the varicode table published
//! with it). Each of the 128 ASCII codes starts and ends with 1 and holds no "00", so two zeros end a character; the transmitter
//! sends "00" after every code.
//!
//! ```
//! use sdrkit_util_text::varicode::psk;
//!
//! let mut bits = Vec::new();
//! psk::encode(b"e t", &mut |b| bits.push(b)).unwrap();
//! assert_eq!(bits, [1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0]);
//! let mut d = psk::Decoder::new();
//! let text: Vec<u8> = bits.iter().filter_map(|&b| d.push(b)).filter_map(|r| r.ok()).collect();
//! assert_eq!(text, b"e t");
//! ```

use super::BitCode;
use crate::error::{Error, Result};

/// The codes of ASCII 0 to 127 (the value's bit length is the code length).
pub const CODES: [u16; 128] = [
    0x2AB, 0x2DB, 0x2ED, 0x377, 0x2EB, 0x35F, 0x2EF, 0x2FD, 0x2FF, 0x0EF, 0x01D, 0x36F, 0x2DD,
    0x01F, 0x375, 0x3AB, 0x2F7, 0x2F5, 0x3AD, 0x3AF, 0x35B, 0x36B, 0x36D, 0x357, 0x37B, 0x37D,
    0x3B7, 0x355, 0x35D, 0x3BB, 0x2FB, 0x37F, 0x001, 0x1FF, 0x15F, 0x1F5, 0x1DB, 0x2D5, 0x2BB,
    0x17F, 0x0FB, 0x0F7, 0x16F, 0x1DF, 0x075, 0x035, 0x057, 0x1AF, 0x0B7, 0x0BD, 0x0ED, 0x0FF,
    0x177, 0x15B, 0x16B, 0x1AD, 0x1AB, 0x1B7, 0x0F5, 0x1BD, 0x1ED, 0x055, 0x1D7, 0x2AF, 0x2BD,
    0x07D, 0x0EB, 0x0AD, 0x0B5, 0x077, 0x0DB, 0x0FD, 0x155, 0x07F, 0x1FD, 0x17D, 0x0D7, 0x0BB,
    0x0DD, 0x0AB, 0x0D5, 0x1DD, 0x0AF, 0x06F, 0x06D, 0x157, 0x1B5, 0x15D, 0x175, 0x17B, 0x2AD,
    0x1F7, 0x1EF, 0x1FB, 0x2BF, 0x16D, 0x2DF, 0x00B, 0x05F, 0x02F, 0x02D, 0x003, 0x03D, 0x05B,
    0x02B, 0x00D, 0x1EB, 0x0BF, 0x01B, 0x03B, 0x00F, 0x007, 0x03F, 0x1BF, 0x015, 0x017, 0x005,
    0x037, 0x07B, 0x06B, 0x0DF, 0x05D, 0x1D5, 0x2B7, 0x1BB, 0x2B5, 0x2D7, 0x3B5,
];

/// The longest code, in bits.
pub const MAX_LEN: u8 = 10;

/// Code value to ASCII + 1 (0: no character), indexed by the value (all values are below 1024).
const INDEX: [u8; 1024] = {
    let mut t = [0u8; 1024];
    let mut i = 0;
    while i < 128 {
        t[CODES[i] as usize] = i as u8 + 1;
        i += 1;
    }
    t
};

/// The code of an ASCII byte.
pub fn code(b: u8) -> Result<BitCode> {
    CODES
        .get(b as usize)
        .map(|&v| BitCode::from_value(v))
        .ok_or(Error::Unencodable(b as char))
}

/// The byte of a code value (`None` when no character has it).
pub fn lookup(value: u16) -> Option<u8> {
    match INDEX.get(value as usize) {
        Some(&i) if i > 0 => Some(i - 1),
        _ => None,
    }
}

/// Encodes ASCII text as bits, each code followed by "00". Stops at the first byte above 127.
pub fn encode(text: &[u8], out: &mut impl FnMut(u8)) -> Result<()> {
    for &b in text {
        code(b)?.iter().for_each(&mut *out);
        out(0);
        out(0);
    }
    Ok(())
}

/// The bit decoder: a character ends at two zeros.
#[derive(Clone, Copy, Debug, Default)]
pub struct Decoder {
    reg: u32,
    len: u8,
    zeros: u8,
}

impl Decoder {
    /// A decoder between characters.
    pub fn new() -> Self {
        Self::default()
    }

    /// One bit (only the low bit is used). Returns a byte when a character ends, or [`Error::UnknownCode`] for a code no
    /// character has (also for a run of more than ten bits without "00").
    pub fn push(&mut self, bit: u8) -> Option<Result<u8>> {
        let bit = bit & 1;
        if bit == 1 {
            if self.zeros == 1 {
                // A single zero inside a code.
                self.reg <<= 1;
                self.len += 1;
            }
            self.zeros = 0;
            self.reg = (self.reg << 1) | 1;
            self.len += 1;
            if self.len > 24 {
                // Never ends: drop it and wait for the next "00".
                self.reg = 0;
                self.len = 0;
            }
            return None;
        }
        self.zeros = self.zeros.saturating_add(1);
        if self.zeros != 2 || self.len == 0 {
            return None;
        }
        let (v, n) = (self.reg, self.len);
        self.reg = 0;
        self.len = 0;
        if n > MAX_LEN {
            return Some(Err(Error::UnknownCode(v)));
        }
        Some(lookup(v as u16).ok_or(Error::UnknownCode(v)))
    }

    /// Forgets a partial character.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_codes() {
        // Entries of the published table (as printed in the PSK31 specification).
        for (c, s) in [
            (b' ', "1"),
            (b'e', "11"),
            (b't', "101"),
            (b'o', "111"),
            (b'a', "1011"),
            (b'n', "1111"),
            (b'i', "1101"),
            (b'r', "10101"),
            (b's', "10111"),
            (b'p', "111111"),
            (b'I', "1111111"),
            (b'E', "1110111"),
            (b'\n', "11101"),
            (b'\r', "11111"),
            (b'0', "10110111"),
            (b'9', "110110111"),
            (b'.', "1010111"),
            (b'@', "1010111101"),
            (b'Z', "1010101101"),
            (b'`', "1011011111"),
            (0, "1010101011"),
            (127, "1110110101"),
        ] {
            assert_eq!(code(c).unwrap().to_bit_string(), s, "{}", c as char);
        }
    }

    #[test]
    fn table_rules() {
        let mut seen = std::collections::HashSet::new();
        for b in 0..128u8 {
            let c = code(b).unwrap();
            let s = c.to_bit_string();
            assert!(
                s.starts_with('1') && s.ends_with('1') && !s.contains("00"),
                "{b}: {s}"
            );
            assert!(c.len <= MAX_LEN);
            assert!(seen.insert(s));
            assert_eq!(lookup(c.bits), Some(b));
        }
        assert_eq!(code(128), Err(Error::Unencodable('\u{80}')));
    }

    #[test]
    fn decoder_round_trip_and_idle() {
        let all: Vec<u8> = (0..128).collect();
        let mut bits = vec![0, 0, 0, 0];
        encode(&all, &mut |b| bits.push(b)).unwrap();
        bits.extend([0; 10]);
        let mut d = Decoder::new();
        let got: Vec<u8> = bits
            .iter()
            .filter_map(|&b| d.push(b))
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(got, all);
        // An unknown code is reported, then decoding goes on.
        let mut d = Decoder::new();
        let r: Vec<_> = [1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0]
            .iter()
            .filter_map(|&b| d.push(b))
            .collect();
        assert!(r[0].is_err());
        assert_eq!(r[1], Ok(b'e'));
    }
}
