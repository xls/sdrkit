//! The IZ8BLY varicode of MFSK16 (N. Porcino IZ8BLY, published by M. Greenman ZL1BPU with the MFSK16 description), which THOR also
//! uses, and THOR's secondary characters.
//!
//! Every code starts with 1 and ends with "00" (the zeros are part of the code as written), and none holds "001", so a decoder ends a
//! character when "00" is followed by the 1 that starts the next one. A transmitter therefore ends a transmission with a 1 (or any
//! code) so that the receiver can end the last character.
//!
//! ```
//! use sdrkit_util_text::varicode::mfsk::{self, Decoder, Symbol};
//!
//! let mut bits = Vec::new();
//! mfsk::encode(b"hi", &mut |b| bits.push(b)).unwrap();
//! bits.push(1); // ends the last character
//! let mut d = Decoder::new();
//! let got: Vec<Symbol> = bits.iter().filter_map(|&b| d.push(b)).map(|r| r.unwrap()).collect();
//! assert_eq!(got, [Symbol::Primary(b'h'), Symbol::Primary(b'i')]);
//! ```

use super::BitCode;
use crate::error::{Error, Result};

/// The codes of bytes 0 to 255, with their trailing zeros (the value's bit length is the code length).
pub const CODES: [u16; 256] = [
    0x75C, 0x760, 0x768, 0x76C, 0x770, 0x774, 0x778, 0x77C, 0x0A8, 0x780, 0x7A0, 0x7A8, 0x7AC,
    0x0AC, 0x7B0, 0x7B4, 0x7B8, 0x7BC, 0x7C0, 0x7D0, 0x7D4, 0x7D8, 0x7DC, 0x7E0, 0x7E8, 0x7EC,
    0x7F0, 0x7F4, 0x7F8, 0x7FC, 0x800, 0xA00, 0x004, 0x1C0, 0x1FC, 0x2D8, 0x2A8, 0x2A0, 0x200,
    0x1BC, 0x1F4, 0x1F0, 0x2B4, 0x1E0, 0x0A0, 0x1D8, 0x1D4, 0x1E8, 0x0E0, 0x0F0, 0x140, 0x154,
    0x174, 0x160, 0x16C, 0x1A0, 0x180, 0x1AC, 0x1EC, 0x1F8, 0x2C0, 0x1DC, 0x2BC, 0x1D0, 0x280,
    0x0BC, 0x100, 0x0D4, 0x0DC, 0x0B8, 0x0F8, 0x150, 0x158, 0x0C0, 0x1B4, 0x17C, 0x0F4, 0x0E8,
    0x0FC, 0x0D0, 0x0EC, 0x1B0, 0x0D8, 0x0B4, 0x0B0, 0x15C, 0x1A8, 0x168, 0x170, 0x178, 0x1B8,
    0x2E8, 0x2D0, 0x2EC, 0x2D4, 0x2B0, 0x2AC, 0x014, 0x060, 0x038, 0x034, 0x008, 0x050, 0x058,
    0x030, 0x018, 0x080, 0x070, 0x02C, 0x040, 0x01C, 0x010, 0x054, 0x078, 0x020, 0x028, 0x00C,
    0x03C, 0x06C, 0x068, 0x074, 0x05C, 0x07C, 0x2DC, 0x2B8, 0x2E0, 0x2F0, 0xA80, 0xAA0, 0xAA8,
    0xAAC, 0xAB0, 0xAB4, 0xAB8, 0xABC, 0xAC0, 0xAD0, 0xAD4, 0xAD8, 0xADC, 0xAE0, 0xAE8, 0xAEC,
    0xAF0, 0xAF4, 0xAF8, 0xAFC, 0xB00, 0xB40, 0xB50, 0xB54, 0xB58, 0xB5C, 0xB60, 0xB68, 0xB6C,
    0xB70, 0xB74, 0xB78, 0xB7C, 0x2F4, 0x2F8, 0x2FC, 0x300, 0x340, 0x350, 0x354, 0x358, 0x35C,
    0x360, 0x368, 0x36C, 0x370, 0x374, 0x378, 0x37C, 0x380, 0x3A0, 0x3A8, 0x3AC, 0x3B0, 0x3B4,
    0x3B8, 0x3BC, 0x3C0, 0x3D0, 0x3D4, 0x3D8, 0x3DC, 0x3E0, 0x3E8, 0x3EC, 0x3F0, 0x3F4, 0x3F8,
    0x3FC, 0x400, 0x500, 0x540, 0x550, 0x554, 0x558, 0x55C, 0x560, 0x568, 0x56C, 0x570, 0x574,
    0x578, 0x57C, 0x580, 0x5A0, 0x5A8, 0x5AC, 0x5B0, 0x5B4, 0x5B8, 0x5BC, 0x5C0, 0x5D0, 0x5D4,
    0x5D8, 0x5DC, 0x5E0, 0x5E8, 0x5EC, 0x5F0, 0x5F4, 0x5F8, 0x5FC, 0x600, 0x680, 0x6A0, 0x6A8,
    0x6AC, 0x6B0, 0x6B4, 0x6B8, 0x6BC, 0x6C0, 0x6D0, 0x6D4, 0x6D8, 0x6DC, 0x6E0, 0x6E8, 0x6EC,
    0x6F0, 0x6F4, 0x6F8, 0x6FC, 0x700, 0x740, 0x750, 0x754, 0x758,
];

/// THOR's secondary characters, for bytes 0x20 to 0x7A (THOR sends them in its idle time, for a beacon text).
pub const THOR_SECONDARY: [u16; 91] = [
    0xB80, 0xBA0, 0xBA8, 0xBAC, 0xBB0, 0xBB4, 0xBB8, 0xBBC, 0xBC0, 0xBD0, 0xBD4, 0xBD8, 0xBDC,
    0xBE0, 0xBE8, 0xBEC, 0xBF0, 0xBF4, 0xBF8, 0xBFC, 0xC00, 0xD00, 0xD40, 0xD54, 0xD58, 0xD5C,
    0xD60, 0xD68, 0xD6C, 0xD70, 0xD74, 0xD78, 0xD7C, 0xD80, 0xDA0, 0xDA8, 0xDAC, 0xDB0, 0xDB4,
    0xDB8, 0xDBC, 0xDC0, 0xDD0, 0xDD4, 0xDD8, 0xDDC, 0xDE0, 0xDE8, 0xDEC, 0xDF0, 0xDF4, 0xDF8,
    0xDFC, 0xE00, 0xE80, 0xEA0, 0xEAC, 0xEB0, 0xEB4, 0xEB8, 0xEBC, 0xEC0, 0xED0, 0xED4, 0xED8,
    0xEDC, 0xEE0, 0xEE8, 0xEEC, 0xEF0, 0xEF4, 0xEF8, 0xEFC, 0xF00, 0xF40, 0xF50, 0xF54, 0xF58,
    0xF5C, 0xF60, 0xF68, 0xF6C, 0xF70, 0xF74, 0xF78, 0xF7C, 0xF80, 0xFA0, 0xFA8, 0xFAC, 0xFB0,
];

/// A decoded character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    /// A byte of the main text.
    Primary(u8),
    /// A THOR secondary character (0x20 to 0x7A).
    Secondary(u8),
}

/// Code value to symbol: 0 none, 1 to 256 primary byte + 1, 0x8000 | byte for secondary. Every value is below 4096.
const INDEX: [u16; 4096] = {
    let mut t = [0u16; 4096];
    let mut i = 0;
    while i < 256 {
        t[CODES[i] as usize] = i as u16 + 1;
        i += 1;
    }
    let mut i = 0;
    while i < 91 {
        t[THOR_SECONDARY[i] as usize] = 0x8000 | (0x20 + i as u16);
        i += 1;
    }
    t
};

/// The code of a byte.
pub fn code(b: u8) -> BitCode {
    BitCode::from_value(CODES[b as usize])
}

/// The THOR secondary code of a character (0x20 to 0x7A).
pub fn secondary_code(b: u8) -> Result<BitCode> {
    match b {
        0x20..=0x7A => Ok(BitCode::from_value(THOR_SECONDARY[(b - 0x20) as usize])),
        _ => Err(Error::Unencodable(b as char)),
    }
}

/// The symbol of a code value (with its trailing zeros).
pub fn lookup(value: u16) -> Option<Symbol> {
    match INDEX.get(value as usize).copied().unwrap_or(0) {
        0 => None,
        v if v & 0x8000 != 0 => Some(Symbol::Secondary(v as u8)),
        v => Some(Symbol::Primary((v - 1) as u8)),
    }
}

/// Encodes bytes as bits (codes back to back).
pub fn encode(text: &[u8], out: &mut impl FnMut(u8)) -> Result<()> {
    for &b in text {
        code(b).iter().for_each(&mut *out);
    }
    Ok(())
}

/// The bit decoder.
#[derive(Clone, Copy, Debug, Default)]
pub struct Decoder {
    reg: u32,
    len: u8,
}

impl Decoder {
    /// A decoder waiting for the first 1.
    pub fn new() -> Self {
        Self::default()
    }

    /// One bit. Returns a symbol when "00" is followed by a 1, or [`Error::UnknownCode`] for a code no character has.
    pub fn push(&mut self, bit: u8) -> Option<Result<Symbol>> {
        if self.len == 0 && bit & 1 == 0 {
            return None; // idle zeros before a code
        }
        self.reg = (self.reg << 1) | (bit & 1) as u32;
        self.len = self.len.saturating_add(1);
        if self.reg & 7 != 1 || self.len < 4 {
            if self.len > 24 {
                // No end in sight: start again at the next 1.
                self.reg = 0;
                self.len = 0;
            }
            return None;
        }
        let v = self.reg >> 1;
        let n = self.len - 1;
        self.reg = 1;
        self.len = 1;
        if n > 12 {
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
        for (b, s) in [
            (b' ', "100"),
            (b'e', "1000"),
            (b't', "1100"),
            (b'a', "10100"),
            (b'o', "10000"),
            (b'i', "11000"),
            (b'n', "11100"),
            (b's', "101000"),
            (b'r', "100000"),
            (b'\r', "10101100"),
            (8u8, "10101000"),
            (0u8, "11101011100"),
            (b'!', "111000000"),
            (b'"', "111111100"),
        ] {
            assert_eq!(code(b).to_bit_string(), s, "{b}");
        }
        assert_eq!(
            secondary_code(b' ').unwrap().to_bit_string(),
            "101110000000"
        );
        assert!(secondary_code(b'{').is_err());
    }

    #[test]
    fn table_rules() {
        let mut seen = std::collections::HashSet::new();
        for b in 0..=255u8 {
            let s = code(b).to_bit_string();
            assert!(
                s.starts_with('1') && s.ends_with("00") && !s.contains("001"),
                "{b}: {s}"
            );
            assert!(seen.insert(s));
            assert_eq!(lookup(code(b).bits), Some(Symbol::Primary(b)));
        }
        for b in 0x20..=0x7Au8 {
            let s = secondary_code(b).unwrap().to_bit_string();
            assert!(
                s.starts_with('1') && s.ends_with("00") && !s.contains("001"),
                "{b}: {s}"
            );
            assert!(seen.insert(s), "{b}");
            assert_eq!(
                lookup(secondary_code(b).unwrap().bits),
                Some(Symbol::Secondary(b))
            );
        }
    }

    #[test]
    fn decoder_round_trip() {
        let all: Vec<u8> = (0..=255).collect();
        let mut bits = vec![0, 0, 0];
        encode(&all, &mut |b| bits.push(b)).unwrap();
        secondary_code(b'K')
            .unwrap()
            .iter()
            .for_each(|b| bits.push(b));
        bits.push(1);
        let mut d = Decoder::new();
        let got: Vec<Symbol> = bits
            .iter()
            .filter_map(|&b| d.push(b))
            .map(|r| r.unwrap())
            .collect();
        let mut want: Vec<Symbol> = all.iter().map(|&b| Symbol::Primary(b)).collect();
        want.push(Symbol::Secondary(b'K'));
        assert_eq!(got, want);
    }
}
