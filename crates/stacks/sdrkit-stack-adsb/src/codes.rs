//! The altitude codes (13-bit AC, 12-bit airborne altitude, Gillham Mode C code), the identity code (squawk) and the aircraft
//! identification field.
//!
//! Standard: ICAO Annex 10 Vol. IV 3.1.1.6, 3.1.2.6.5.4 and 3.1.2.9.1.2

use sdrkit_util_geo::Text;
use sdrkit_util_text::sixbit;
use std::fmt::Write;

/// The 13-bit AC field (C1 A1 C2 A2 C4 A4 M B1 Q B2 D2 B4 D4, C1 the most significant bit) in feet: `None` for no altitude, a
/// metric altitude (M = 1) or an undefined Gillham code.
pub fn ac13_ft(ac: u32) -> Option<i32> {
    let ac = ac & 0x1FFF;
    if ac == 0 || ac & 0x40 != 0 {
        return None;
    }
    if ac & 0x10 != 0 {
        let n = ((ac & 0x1F80) >> 2) | ((ac & 0x20) >> 1) | (ac & 0x0F);
        return Some(25 * n as i32 - 1000);
    }
    // Q = 0: drop M and Q to get C1 A1 C2 A2 C4 A4 B1 B2 D2 B4 D4.
    let g = ((ac & 0x1F80) >> 2) | ((ac & 0x20) >> 1) | (ac & 0x0F);
    gillham_ft(g)
}

/// The 12-bit airborne altitude of an extended squitter (the AC field without M) in feet.
pub fn ac12_ft(a: u32) -> Option<i32> {
    let a = a & 0xFFF;
    if a == 0 {
        return None;
    }
    // Put the M bit (0) back in and decode as AC13.
    let ac = ((a & 0xFC0) << 1) | (a & 0x3F);
    ac13_ft(ac)
}

/// Encodes an altitude in feet as the 12-bit airborne field with 25 ft steps (Q = 1); `None` outside -1000 .. 50175 ft.
pub fn encode_ac12_25ft(ft: i32) -> Option<u32> {
    if !(-1000..=50175).contains(&ft) {
        return None;
    }
    let n = ((ft + 1000) / 25) as u32;
    Some(((n & 0x7F0) << 1) | 0x10 | (n & 0xF))
}

/// Encodes an altitude as the 12-bit airborne field in Gillham code (100 ft steps, Q = 0).
pub fn encode_ac12_gillham(ft: i32) -> Option<u32> {
    let g = encode_gillham(ft)?;
    // C1 A1 C2 A2 C4 A4 B1 | Q=0 | B2 D2 B4 D4
    Some(((g & 0x7F0) << 1) | (g & 0x0F))
}

/// Encodes an altitude as the 13-bit AC field with 25 ft steps (M = 0, Q = 1).
pub fn encode_ac13_25ft(ft: i32) -> Option<u32> {
    let a = encode_ac12_25ft(ft)?;
    Some(((a & 0xFC0) << 1) | (a & 0x3F))
}

/// Encodes an altitude as the 13-bit AC field in Gillham code.
pub fn encode_ac13_gillham(ft: i32) -> Option<u32> {
    let a = encode_ac12_gillham(ft)?;
    Some(((a & 0xFC0) << 1) | (a & 0x3F))
}

const C_EVEN: [u32; 5] = [0b001, 0b011, 0b010, 0b110, 0b100];

/// The Gillham (Mode C) code, 11 bits C1 A1 C2 A2 C4 A4 B1 B2 D2 B4 D4 (C1 the most significant), in feet; `None` for an undefined
/// code. Defined from -1200 to 126 700 ft.
pub fn gillham_ft(g: u32) -> Option<i32> {
    let bit = |i: u32| (g >> i) & 1;
    let (c1, a1, c2, a2, c4, a4, b1, b2, d2, b4, d4) = (
        bit(10),
        bit(9),
        bit(8),
        bit(7),
        bit(6),
        bit(5),
        bit(4),
        bit(3),
        bit(2),
        bit(1),
        bit(0),
    );
    let gray =
        (d2 << 7) | (d4 << 6) | (a1 << 5) | (a2 << 4) | (a4 << 3) | (b1 << 2) | (b2 << 1) | b4;
    let mut n = 0u32;
    let mut g2 = gray;
    while g2 != 0 {
        n ^= g2;
        g2 >>= 1;
    }
    let c = (c1 << 2) | (c2 << 1) | c4;
    let pos = C_EVEN.iter().position(|&x| x == c)? as i32;
    let level = if n.is_multiple_of(2) { pos } else { 4 - pos };
    Some(-1200 + 500 * n as i32 + 100 * level)
}

/// Encodes an altitude (rounded to 100 ft) as an 11-bit Gillham code.
pub fn encode_gillham(ft: i32) -> Option<u32> {
    let h = ((ft as f64) / 100.0).round() as i32 * 100;
    if !(-1200..=126_700).contains(&h) {
        return None;
    }
    let total = ((h + 1200) / 100) as u32;
    let (n, level) = (total / 5, total % 5);
    let gray = n ^ (n >> 1);
    let c = if n % 2 == 0 {
        C_EVEN[level as usize]
    } else {
        C_EVEN[4 - level as usize]
    };
    let gb = |i: u32| (gray >> i) & 1;
    let (d2, d4, a1, a2, a4, b1, b2, b4) = (gb(7), gb(6), gb(5), gb(4), gb(3), gb(2), gb(1), gb(0));
    let (c1, c2, c4) = (c >> 2 & 1, c >> 1 & 1, c & 1);
    Some(
        (c1 << 10)
            | (a1 << 9)
            | (c2 << 8)
            | (a2 << 7)
            | (c4 << 6)
            | (a4 << 5)
            | (b1 << 4)
            | (b2 << 3)
            | (d2 << 2)
            | (b4 << 1)
            | d4,
    )
}

/// The 13-bit identity field (C1 A1 C2 A2 C4 A4 X B1 D1 B2 D2 B4 D4) as the four octal digits ABCD, as a number (0o0356 = 0356).
pub fn squawk(id: u32) -> u16 {
    let b = |i: u32| ((id >> i) & 1) as u16;
    let (c1, a1, c2, a2, c4, a4, b1, d1, b2, d2, b4, d4) = (
        b(12),
        b(11),
        b(10),
        b(9),
        b(8),
        b(7),
        b(5),
        b(4),
        b(3),
        b(2),
        b(1),
        b(0),
    );
    let a = a4 * 4 + a2 * 2 + a1;
    let bb = b4 * 4 + b2 * 2 + b1;
    let c = c4 * 4 + c2 * 2 + c1;
    let d = d4 * 4 + d2 * 2 + d1;
    (a << 9) | (bb << 6) | (c << 3) | d
}

/// The squawk (four octal digits as a number, 0o7700) as the 13-bit identity field (X = 0).
pub fn encode_squawk(sq: u16) -> u32 {
    let (a, b, c, d) = ((sq >> 9) & 7, (sq >> 6) & 7, (sq >> 3) & 7, sq & 7);
    let bit = |v: u16, i: u16| ((v >> i) & 1) as u32;
    (bit(c, 0) << 12)
        | (bit(a, 0) << 11)
        | (bit(c, 1) << 10)
        | (bit(a, 1) << 9)
        | (bit(c, 2) << 8)
        | (bit(a, 2) << 7)
        | (bit(b, 0) << 5)
        | (bit(d, 0) << 4)
        | (bit(b, 1) << 3)
        | (bit(d, 1) << 2)
        | (bit(b, 2) << 1)
        | bit(d, 2)
}

/// The squawk as four octal digits ("0356").
pub fn squawk_text(sq: u16) -> Text<4> {
    let mut t = Text::new();
    // Four octal digits always fit four bytes.
    let _ = write!(t, "{:04o}", sq & 0o7777);
    t
}

/// A flight identification: up to eight characters of the ICAO 6-bit set.
pub type Callsign = Text<8>;

/// Parses four octal digits.
pub fn parse_squawk(s: &str) -> Option<u16> {
    let s = s.trim();
    if s.is_empty() || s.len() > 4 || !s.bytes().all(|c| (b'0'..=b'7').contains(&c)) {
        return None;
    }
    u16::from_str_radix(s, 8).ok()
}

/// The eight characters of a 48-bit identification field (the first in the top bits), undefined codes as spaces, trailing spaces
/// removed. The character set is the ICAO 6-bit set of `sdrkit_util_text::sixbit` (Annex 10 Vol. IV 3.1.2.9.1.2).
pub fn callsign(v: u64) -> Callsign {
    let mut chars = [b' '; 8];
    for (i, ch) in chars.iter_mut().enumerate() {
        let c = ((v >> (42 - 6 * i)) & 0x3F) as u8;
        // The ICAO 6-bit set is ASCII.
        *ch = sixbit::icao_char(c).map_or(b' ', |c| c as u8);
    }
    let text = std::str::from_utf8(&chars).unwrap_or_default().trim_end();
    Callsign::try_from_str(text).unwrap_or_default()
}

/// Whether every one of the eight characters of a 48-bit identification field is a defined code.
pub fn callsign_defined(v: u64) -> bool {
    (0..8).all(|i| sixbit::icao_char(((v >> (42 - 6 * i)) & 0x3F) as u8).is_ok())
}

/// The 48-bit identification field of up to eight characters (padded with spaces). A character outside the set becomes a space.
pub fn encode_callsign(s: &str) -> u64 {
    let mut v = 0u64;
    let mut it = s.chars();
    for _ in 0..8 {
        let c = sixbit::icao_code(it.next().unwrap_or(' ')).unwrap_or(32);
        v = (v << 6) | c as u64;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn altitude_fields() {
        assert_eq!(ac12_ft(0xC38), Some(38000));
        assert_eq!(ac13_ft(0x0390), Some(4600));
        assert_eq!(ac13_ft(0), None);
        assert_eq!(ac13_ft(0x0150), None);
        for ft in (-1000..50000).step_by(25) {
            assert_eq!(ac12_ft(encode_ac12_25ft(ft).unwrap()), Some(ft));
            assert_eq!(ac13_ft(encode_ac13_25ft(ft).unwrap()), Some(ft));
        }
        assert_eq!(ac12_ft(encode_ac12_gillham(35000).unwrap()), Some(35000));
        assert_eq!(ac13_ft(encode_ac13_gillham(12300).unwrap()), Some(12300));
    }

    #[test]
    fn gillham_table() {
        assert_eq!(gillham_ft(0x10A), Some(0));
        assert_eq!(gillham_ft(0x50A), Some(100));
        assert_eq!(gillham_ft(0x048), Some(700));
        let mut alts: Vec<i32> = (0..2048).filter_map(gillham_ft).collect();
        assert_eq!(alts.len(), 1280);
        alts.sort_unstable();
        alts.dedup();
        assert_eq!(alts.len(), 1280);
        assert_eq!((alts[0], alts[1279]), (-1200, 126_700));
        for ft in (-1200..126_600).step_by(100) {
            let (a, b) = (
                encode_gillham(ft).unwrap(),
                encode_gillham(ft + 100).unwrap(),
            );
            assert_eq!(gillham_ft(a), Some(ft));
            assert_eq!((a ^ b).count_ones(), 1, "{ft}");
        }
    }

    #[test]
    fn squawks_round_trip() {
        for sq in 0..4096u16 {
            assert_eq!(squawk(encode_squawk(sq)), sq);
        }
        assert_eq!(squawk_text(0o356), "0356");
        assert_eq!(parse_squawk("7700"), Some(0o7700));
        assert_eq!(parse_squawk("7800"), None);
    }

    #[test]
    fn characters() {
        assert_eq!(callsign(encode_callsign("KLM1023")), "KLM1023");
        assert!(callsign_defined(encode_callsign("AB 12")));
    }
}
