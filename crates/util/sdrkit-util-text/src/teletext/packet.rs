//! Teletext packets (ETS 300 706 clauses 7 and 9) and their carriage in DVB (ETSI EN 300 472): the address, the page header, the
//! display rows, the enhancement packets X/26, X/28 and M/29 (Hamming 24/18 triplets) and the editorial links X/27/0.

use super::hamming::{decode_24_18, decode_8_4, encode_24_18, encode_8_4, with_parity};
use crate::error::{Error, Result};
use std::fmt;

/// A page number: magazine 1 to 8 and two hexadecimal digits (pages with digits A to F are not for display: fillers, data pages).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageNo(u16);

impl PageNo {
    /// From a magazine (1 to 8) and the tens and units digits (0 to 15 each).
    pub fn new(magazine: u8, tens: u8, units: u8) -> Result<PageNo> {
        if !(1..=8).contains(&magazine) || tens > 15 || units > 15 {
            return Err(Error::OutOfRange);
        }
        Ok(PageNo(
            (magazine as u16) << 8 | (tens as u16) << 4 | units as u16,
        ))
    }

    /// From a decimal page number 100 to 899.
    pub fn from_decimal(n: u16) -> Result<PageNo> {
        if !(100..=899).contains(&n) {
            return Err(Error::OutOfRange);
        }
        PageNo::new((n / 100) as u8, (n / 10 % 10) as u8, (n % 10) as u8)
    }

    /// From text such as "100" or "1A3".
    pub fn parse(s: &str) -> Result<PageNo> {
        let b = s.trim().as_bytes();
        if b.len() != 3 {
            return Err(Error::Invalid);
        }
        let d = |c: u8| {
            (c as char)
                .to_digit(16)
                .map(|v| v as u8)
                .ok_or(Error::Invalid)
        };
        PageNo::new(d(b[0])?, d(b[1])?, d(b[2])?)
    }

    /// The magazine, 1 to 8.
    pub fn magazine(self) -> u8 {
        (self.0 >> 8) as u8
    }

    /// The tens digit (0 to 15).
    pub fn tens(self) -> u8 {
        (self.0 >> 4 & 15) as u8
    }

    /// The units digit (0 to 15).
    pub fn units(self) -> u8 {
        (self.0 & 15) as u8
    }

    /// The decimal number (100 to 899) when both digits are decimal.
    pub fn decimal(self) -> Option<u16> {
        (self.tens() <= 9 && self.units() <= 9)
            .then(|| self.magazine() as u16 * 100 + self.tens() as u16 * 10 + self.units() as u16)
    }
}

impl fmt::Display for PageNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{:X}{:X}", self.magazine(), self.tens(), self.units())
    }
}

/// The control bits of a page header (C4 to C11; C12 to C14 are in [`Header::national`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Control {
    /// C4: erase page.
    pub erase: bool,
    /// C5: newsflash.
    pub newsflash: bool,
    /// C6: subtitle.
    pub subtitle: bool,
    /// C7: suppress header.
    pub suppress_header: bool,
    /// C8: update indicator.
    pub update: bool,
    /// C9: interrupted sequence.
    pub interrupted: bool,
    /// C10: inhibit display.
    pub inhibit_display: bool,
    /// C11: magazine serial (a header ends the page of every magazine).
    pub serial: bool,
}

/// A page header (packet X/0, Figure 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The magazine (1 to 8).
    pub magazine: u8,
    /// The page tens and units digits (`None` when a digit failed Hamming 8/4).
    pub digits: Option<(u8, u8)>,
    /// The subcode with S4, S3, S2 and S1 in its four nibbles, as subcodes are written ("0001", "3F7F"); 0 when a part failed
    /// Hamming 8/4.
    pub subcode: u16,
    /// C4 to C11.
    pub control: Control,
    /// C12, C13, C14 as a number (C12 the most significant), the national option of Table 32.
    pub national: u8,
    /// The 32 display bytes (columns 8 to 39), still with their parity bit.
    pub text: [u8; 32],
}

impl Header {
    /// The page number, when the digits decoded.
    pub fn page(&self) -> Option<PageNo> {
        let (t, u) = self.digits?;
        PageNo::new(self.magazine, t, u).ok()
    }
}

/// An editorial link (packet X/27/0): a page and subcode, `None` when the page digits failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    /// The page.
    pub page: PageNo,
    /// The subcode (0x3F7F means "any").
    pub subcode: u16,
}

/// One decoded packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Packet<'a> {
    /// X/0.
    Header(Header),
    /// X/1 to X/25: 40 display bytes with their parity bit.
    Row {
        /// The magazine.
        magazine: u8,
        /// The row.
        row: u8,
        /// The bytes.
        data: &'a [u8; 40],
    },
    /// X/26, X/28 or M/29: a designation code and 13 triplets (`None` for a triplet with a double error).
    Triplets {
        /// The magazine.
        magazine: u8,
        /// The packet number (26, 28 or 29).
        row: u8,
        /// The designation code (0 to 15).
        designation: u8,
        /// The triplets (18 data bits each).
        triplets: [Option<u32>; 13],
    },
    /// X/27/0 to X/27/3: six links and a link control byte.
    Links {
        /// The magazine.
        magazine: u8,
        /// The designation code.
        designation: u8,
        /// Red, green, yellow, cyan, the fifth link, the index link.
        links: [Option<Link>; 6],
    },
    /// Any other packet (X/27 with another format, 30, 31).
    Other {
        /// The magazine.
        magazine: u8,
        /// The packet number.
        row: u8,
    },
}

fn nibble(b: u8) -> Option<u8> {
    decode_8_4(b).map(|d| d.data)
}

/// Decodes the address of a packet: magazine (1 to 8) and packet number (0 to 31).
pub fn address(b: &[u8; 42]) -> Result<(u8, u8)> {
    let (a, p) = (
        nibble(b[0]).ok_or(Error::Invalid)?,
        nibble(b[1]).ok_or(Error::Invalid)?,
    );
    let mag = if a & 7 == 0 { 8 } else { a & 7 };
    Ok((mag, (a >> 3) | (p << 1)))
}

/// Decodes one packet of 42 bytes (two address bytes and 40 data bytes, first bit sent in bit 0). An address that fails Hamming
/// 8/4 is [`Error::Invalid`].
pub fn parse(b: &[u8; 42]) -> Result<Packet<'_>> {
    let (magazine, row) = address(b)?;
    let d: &[u8; 40] = b[2..].try_into().map_err(|_| Error::TooShort)?;
    Ok(match row {
        0 => {
            let h: [Option<u8>; 8] = core::array::from_fn(|i| nibble(d[i]));
            let digits = h[0].zip(h[1]).map(|(u, t)| (t, u));
            let subcode = match (h[2], h[3], h[4], h[5]) {
                (Some(s1), Some(s2), Some(s3), Some(s4)) => {
                    s1 as u16 | ((s2 & 7) as u16) << 4 | (s3 as u16) << 8 | ((s4 & 3) as u16) << 12
                }
                _ => 0,
            };
            let c = |n: Option<u8>, bit: u8| n.is_some_and(|v| v >> bit & 1 == 1);
            let control = Control {
                erase: c(h[3], 3),
                newsflash: c(h[5], 2),
                subtitle: c(h[5], 3),
                suppress_header: c(h[6], 0),
                update: c(h[6], 1),
                interrupted: c(h[6], 2),
                inhibit_display: c(h[6], 3),
                serial: c(h[7], 0),
            };
            let national = h[7].map_or(0, |v| (v >> 1 & 1) << 2 | (v >> 2 & 1) << 1 | (v >> 3 & 1));
            let mut text = [0u8; 32];
            text.copy_from_slice(&d[8..]);
            Packet::Header(Header {
                magazine,
                digits,
                subcode,
                control,
                national,
                text,
            })
        }
        1..=25 => Packet::Row {
            magazine,
            row,
            data: d,
        },
        26 | 28 | 29 => {
            let designation = nibble(d[0]).ok_or(Error::Invalid)?;
            let triplets = core::array::from_fn(|i| {
                decode_24_18([d[1 + 3 * i], d[2 + 3 * i], d[3 + 3 * i]]).map(|x| x.data)
            });
            Packet::Triplets {
                magazine,
                row,
                designation,
                triplets,
            }
        }
        27 => {
            let designation = nibble(d[0]).ok_or(Error::Invalid)?;
            if designation > 3 {
                return Ok(Packet::Other { magazine, row });
            }
            let links = core::array::from_fn(|l| {
                let x = &d[1 + 6 * l..7 + 6 * l];
                let h = Nibbles6::new(x)?;
                let m = (h.0[3] >> 3 & 1) | (h.0[5] >> 1 & 6);
                let lmag = (magazine ^ m) & 7;
                let lmag = if lmag == 0 { 8 } else { lmag };
                let subcode = h.0[2] as u16
                    | ((h.0[3] & 7) as u16) << 4
                    | (h.0[4] as u16) << 8
                    | ((h.0[5] & 3) as u16) << 12;
                Some(Link {
                    page: PageNo::new(lmag, h.0[1], h.0[0]).ok()?,
                    subcode,
                })
            });
            Packet::Links {
                magazine,
                designation,
                links,
            }
        }
        _ => Packet::Other { magazine, row },
    })
}

/// Six decoded nibbles of a link.
struct Nibbles6([u8; 6]);

impl Nibbles6 {
    fn new(x: &[u8]) -> Option<Nibbles6> {
        let mut o = [0u8; 6];
        for (o, &b) in o.iter_mut().zip(x) {
            *o = nibble(b)?;
        }
        Some(Nibbles6(o))
    }
}

/// The 42-byte packets carried in a teletext PES payload (EN 300 472 4.3): `data_identifier` 0x10 to 0x1F, then data units of
/// type 0x02 (EBU teletext) or 0x03 (subtitle) of 44 bytes whose bytes are sent most significant bit first (reversed here).
pub fn pes_packets(payload: &[u8]) -> impl Iterator<Item = [u8; 42]> + '_ {
    let ok = payload.first().is_some_and(|d| (0x10..=0x1F).contains(d));
    let mut i = 1;
    core::iter::from_fn(move || {
        if !ok {
            return None;
        }
        while i + 2 <= payload.len() {
            let (id, len) = (payload[i], payload[i + 1] as usize);
            let unit = payload.get(i + 2..i + 2 + len)?;
            i += 2 + len;
            if (id == 0x02 || id == 0x03) && len == 44 && unit[1] == 0xE4 {
                let mut b = [0u8; 42];
                for (o, x) in b.iter_mut().zip(&unit[2..44]) {
                    *o = x.reverse_bits();
                }
                return Some(b);
            }
        }
        None
    })
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Building packets (generators and tests).

fn addr(mag: u8, row: u8) -> [u8; 2] {
    [encode_8_4((mag & 7) | (row & 1) << 3), encode_8_4(row >> 1)]
}

/// A page header packet. `text` fills columns 8 to 39 (padded with spaces).
pub fn build_header(
    page: PageNo,
    subcode: u16,
    control: Control,
    national: u8,
    text: &[u8],
) -> [u8; 42] {
    let mut b = [0u8; 42];
    b[..2].copy_from_slice(&addr(page.magazine(), 0));
    b[2] = encode_8_4(page.units());
    b[3] = encode_8_4(page.tens());
    b[4] = encode_8_4((subcode & 0xF) as u8);
    b[5] = encode_8_4(((subcode >> 4) & 7) as u8 | (control.erase as u8) << 3);
    b[6] = encode_8_4(((subcode >> 8) & 0xF) as u8);
    b[7] = encode_8_4(
        ((subcode >> 12) & 3) as u8
            | (control.newsflash as u8) << 2
            | (control.subtitle as u8) << 3,
    );
    b[8] = encode_8_4(
        control.suppress_header as u8
            | (control.update as u8) << 1
            | (control.interrupted as u8) << 2
            | (control.inhibit_display as u8) << 3,
    );
    b[9] = encode_8_4(
        control.serial as u8
            | (national >> 2 & 1) << 1
            | (national >> 1 & 1) << 2
            | (national & 1) << 3,
    );
    for c in 0..32 {
        b[10 + c] = with_parity(*text.get(c).unwrap_or(&b' '));
    }
    b
}

/// A header with non-decimal digits (a time filler: it ends the magazine's page and opens none).
pub fn build_filler(magazine: u8) -> [u8; 42] {
    let p = PageNo::new(magazine.clamp(1, 8), 0xF, 0xF).unwrap_or_default();
    build_header(p, 0, Control::default(), 0, b"")
}

/// A display row packet (1 to 25) with 40 codes (padded with spaces).
pub fn build_row(magazine: u8, row: u8, codes: &[u8]) -> [u8; 42] {
    let mut b = [0u8; 42];
    b[..2].copy_from_slice(&addr(magazine, row));
    for c in 0..40 {
        b[2 + c] = with_parity(*codes.get(c).unwrap_or(&b' '));
    }
    b
}

/// A triplet packet (26, 28 or 29) with a designation code and up to 13 triplets (the rest are termination markers).
pub fn build_triplets(magazine: u8, row: u8, designation: u8, triplets: &[u32]) -> [u8; 42] {
    let mut b = [0u8; 42];
    b[..2].copy_from_slice(&addr(magazine, row));
    b[2] = encode_8_4(designation);
    for i in 0..13 {
        let t = triplets.get(i).copied().unwrap_or(63 | 0x1F << 6);
        b[3 + 3 * i..6 + 3 * i].copy_from_slice(&encode_24_18(t));
    }
    b
}

/// A triplet from its address (6 bits), mode (5 bits) and data (7 bits).
pub fn triplet(address: u8, mode: u8, data: u8) -> u32 {
    (address & 63) as u32 | ((mode & 31) as u32) << 6 | ((data & 127) as u32) << 11
}

/// A packet X/27/0 with six links (`None`: a link to page xFF with subcode 3F7F, "no link").
pub fn build_links(magazine: u8, links: [Option<Link>; 6]) -> [u8; 42] {
    let mut b = [0u8; 42];
    b[..2].copy_from_slice(&addr(magazine, 27));
    b[2] = encode_8_4(0);
    for (l, link) in links.iter().enumerate() {
        let x = &mut b[3 + l * 6..9 + l * 6];
        let (lm, t, u, s) = match link {
            Some(k) => (k.page.magazine(), k.page.tens(), k.page.units(), k.subcode),
            None => (magazine, 0xF, 0xF, 0x3F7F),
        };
        let m = (lm ^ magazine) & 7;
        x[0] = encode_8_4(u);
        x[1] = encode_8_4(t);
        x[2] = encode_8_4((s & 0xF) as u8);
        x[3] = encode_8_4((s >> 4 & 7) as u8 | (m & 1) << 3);
        x[4] = encode_8_4((s >> 8 & 0xF) as u8);
        x[5] = encode_8_4((s >> 12 & 3) as u8 | (m & 6) << 1);
    }
    b
}

/// Appends a teletext PES payload holding these packets (EN 300 472: data identifier 0x10, data units 0x02 of 44 bytes).
pub fn build_pes(packets: &[[u8; 42]], out: &mut Vec<u8>) {
    out.push(0x10);
    for p in packets {
        out.extend_from_slice(&[0x02, 44, 0xE0 | 21, 0xE4]);
        out.extend(p.iter().map(|x| x.reverse_bits()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_numbers() {
        let p = PageNo::parse("1A3").unwrap();
        assert_eq!((p.magazine(), p.tens(), p.units()), (1, 10, 3));
        assert_eq!(p.to_string(), "1A3");
        assert_eq!(p.decimal(), None);
        assert_eq!(PageNo::from_decimal(888).unwrap().to_string(), "888");
        assert!(PageNo::parse("900").is_err());
        assert!(PageNo::new(0, 0, 0).is_err());
    }

    #[test]
    fn header_fields_round_trip() {
        let c = Control {
            erase: true,
            subtitle: true,
            serial: true,
            inhibit_display: true,
            ..Control::default()
        };
        let b = build_header(
            PageNo::parse("801").unwrap(),
            0x2A5B & 0x3F7F,
            c,
            0b101,
            b"TELETEXT",
        );
        // Magazine 8 is sent as 0 in the address.
        assert_eq!(b[0], encode_8_4(0));
        match parse(&b).unwrap() {
            Packet::Header(h) => {
                assert_eq!(h.page().unwrap().to_string(), "801");
                assert_eq!(h.subcode, 0x2A5B & 0x3F7F);
                assert_eq!(h.control, c);
                assert_eq!(h.national, 0b101);
                assert_eq!(super::super::hamming::parity(h.text[0]), Some(b'T'));
            }
            p => panic!("{p:?}"),
        }
        let mut bad = b;
        bad[0] ^= 0b11;
        assert_eq!(parse(&bad), Err(Error::Invalid));
    }

    #[test]
    fn rows_triplets_and_links() {
        assert!(matches!(
            parse(&build_row(3, 17, b"x")).unwrap(),
            Packet::Row {
                magazine: 3,
                row: 17,
                ..
            }
        ));
        let t = triplet(41, 0x04, 5);
        match parse(&build_triplets(1, 26, 0, &[t])).unwrap() {
            Packet::Triplets {
                row: 26,
                designation: 0,
                triplets,
                ..
            } => {
                assert_eq!(triplets[0], Some(t));
                assert_eq!(triplets[1], Some(triplet(63, 0x1F, 0)));
            }
            p => panic!("{p:?}"),
        }
        let l = Link {
            page: PageNo::parse("234").unwrap(),
            subcode: 0x3F7F,
        };
        let l2 = Link {
            page: PageNo::parse("899").unwrap(),
            subcode: 1,
        };
        match parse(&build_links(
            2,
            [Some(l), Some(l2), None, None, None, Some(l)],
        ))
        .unwrap()
        {
            Packet::Links { links, .. } => {
                assert_eq!(links[0], Some(l));
                assert_eq!(links[1], Some(l2));
                assert_eq!(links[2].map(|k| k.page.decimal()), Some(None));
            }
            p => panic!("{p:?}"),
        }
    }

    #[test]
    fn pes_round_trip() {
        let pk = [build_row(1, 1, b"A"), build_row(1, 2, b"B")];
        let mut pes = vec![];
        build_pes(&pk, &mut pes);
        let got: Vec<[u8; 42]> = pes_packets(&pes).collect();
        assert_eq!(got, pk);
        assert_eq!(pes_packets(&[0x20, 0x02]).count(), 0);
        // A truncated unit ends the payload.
        assert_eq!(pes_packets(&pes[..50]).count(), 1);
    }
}
