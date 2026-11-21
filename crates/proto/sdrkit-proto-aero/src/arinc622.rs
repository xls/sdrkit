//! ARINC 622 envelopes: binary ATS applications (ADS-C, FANS-1/A CPDLC) carried as hex text inside ACARS messages.
//!
//! The text form is
//!
//! ```text
//! GROUND.IMI ADDRESS HEX CRC
//! /PIKCPYA.ADS.HB-JHM 07040B000C000D010E011000 440D
//! ```
//!
//! (spaces added here only). GROUND is the ground address of 7 (or 4) letters and digits. IMI (imbedded message identifier) is
//! three characters that name the application (`ADS`, `DIS`, `AT1` ...). ADDRESS is the aircraft registration padded with
//! leading dots to 7 characters, so a 6-character registration becomes `.HB-JHM`. HEX is the binary message. CRC is the CRC-16 of
//! ARINC 622 as four hex digits, high byte first.
//!
//! The CRC is CRC-16/GENIBUS ([`CRC16_GENIBUS`]: x^16 + x^12 + x^5 + 1, register all ones, result inverted). It covers the IMI,
//! the 7 address characters and the binary message. Some senders leave out the padding dots of the address. [`find`] then uses
//! the block's registration to find where the hex starts, and computes the CRC over the padded form.
//!
//! ```
//! use sdrkit_proto_aero::arinc622::{find, Imi};
//! let e = find("/PIKCPYA.ADS.HB-JHM07040B000C000D010E011000440D", "").unwrap();
//! assert_eq!((e.ground, e.imi, e.address), ("PIKCPYA", Imi::Ads, ".HB-JHM"));
//! assert!(e.crc_ok);
//! let mut buf = [0u8; 64];
//! assert_eq!(e.binary(&mut buf).unwrap(), &[7, 4, 0x0B, 0, 0x0C, 0, 0x0D, 1, 0x0E, 1, 0x10, 0]);
//! ```

use crate::error::{Error, Item, Result};
use sdrkit_util_bits::crc::catalog::CRC16_GENIBUS;
use std::fmt::Write as _;

/// The imbedded message identifier: the application an envelope carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Imi {
    /// `ADS`: an ADS-C message (ARINC 745).
    Ads,
    /// `DIS`: an ADS-C disconnect.
    Dis,
    /// `AT1`: a FANS-1/A CPDLC message.
    At1,
    /// `CR1`: a FANS-1/A CPDLC connect request.
    Cr1,
    /// `CC1`: a FANS-1/A CPDLC connect confirm.
    Cc1,
    /// `DR1`: a FANS-1/A CPDLC disconnect request.
    Dr1,
}

impl Imi {
    /// Every identifier this crate reads.
    pub const ALL: [Imi; 6] = [Imi::Ads, Imi::Dis, Imi::At1, Imi::Cr1, Imi::Cc1, Imi::Dr1];

    /// The three characters.
    pub fn as_str(self) -> &'static str {
        match self {
            Imi::Ads => "ADS",
            Imi::Dis => "DIS",
            Imi::At1 => "AT1",
            Imi::Cr1 => "CR1",
            Imi::Cc1 => "CC1",
            Imi::Dr1 => "DR1",
        }
    }

    /// The identifier of three characters.
    pub fn parse(s: &str) -> Option<Imi> {
        Imi::ALL.into_iter().find(|i| i.as_str() == s)
    }

    /// True for the CPDLC identifiers.
    pub fn is_cpdlc(self) -> bool {
        matches!(self, Imi::At1 | Imi::Cr1 | Imi::Cc1 | Imi::Dr1)
    }
}

/// An envelope found in a text. The fields borrow from the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Envelope<'a> {
    /// The ground address.
    pub ground: &'a str,
    /// The application.
    pub imi: Imi,
    /// The aircraft address as sent (7 characters with the padding dots, or fewer when the sender left them out).
    pub address: &'a str,
    /// The hex digits of the binary message and its CRC.
    pub hex: &'a str,
    /// The CRC is good.
    pub crc_ok: bool,
}

fn addr_char(c: u8) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit()
}

fn nibble(c: u8) -> Result<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        _ => Err(Error::BadHex),
    }
}

/// Decodes hex text into `out` and returns the bytes written. An error for an odd length, a character that is not a hex digit,
/// or an `out` that is too short.
pub fn unhex<'o>(hex: &str, out: &'o mut [u8]) -> Result<&'o [u8]> {
    let h = hex.as_bytes();
    if !h.len().is_multiple_of(2) {
        return Err(Error::BadHex);
    }
    let n = h.len() / 2;
    let cap = out.len();
    let dst = out.get_mut(..n).ok_or(Error::TooLong(cap * 2))?;
    for (d, p) in dst.iter_mut().zip(h.as_chunks::<2>().0) {
        *d = nibble(p[0])? << 4 | nibble(p[1])?;
    }
    Ok(dst)
}

/// The ARINC 622 CRC of an envelope: over the IMI, the address padded with leading dots to 7 characters, and `data`.
pub fn crc(imi: Imi, address: &str, data: &[u8]) -> u16 {
    let mut d = CRC16_GENIBUS.digest();
    d.update(imi.as_str().as_bytes());
    for _ in address.len()..7 {
        d.update(b".");
    }
    d.update(address.as_bytes());
    d.update(data);
    d.finalize()
}

impl<'a> Envelope<'a> {
    /// Decodes the binary message (the CRC removed) into `out`.
    pub fn binary<'o>(&self, out: &'o mut [u8]) -> Result<&'o [u8]> {
        let b = unhex(self.hex, out)?;
        let n = b.len().checked_sub(2).ok_or(Error::Short)?;
        Ok(&b[..n])
    }
}

fn hex_crc_ok(imi: Imi, address: &str, hex: &str) -> bool {
    let h = hex.as_bytes();
    if h.len() < 4 {
        return false;
    }
    let byte = |p: &[u8]| Ok::<u8, Error>(nibble(p[0])? << 4 | nibble(p[1])?);
    let mut d = CRC16_GENIBUS.digest();
    d.update(imi.as_str().as_bytes());
    for _ in address.len()..7 {
        d.update(b".");
    }
    d.update(address.as_bytes());
    let (data, tail) = h.split_at(h.len() - 4);
    for p in data.as_chunks::<2>().0 {
        match byte(p) {
            Ok(b) => d.update(&[b]),
            Err(_) => return false,
        }
    }
    match (byte(&tail[..2]), byte(&tail[2..])) {
        (Ok(hi), Ok(lo)) => d.finalize() == u16::from_be_bytes([hi, lo]),
        _ => false,
    }
}

/// Finds the first envelope of a known [`Imi`] in `text`. `registration` (the block's, it can be empty) helps where the sender left
/// out the padding dots of the address. `None` when there is none.
pub fn find<'a>(text: &'a str, registration: &str) -> Option<Envelope<'a>> {
    find_where(text, registration, false)
}

/// Finds the first CPDLC envelope (`AT1`, `CR1`, `CC1`, `DR1`) in `text`, as [`find`] does, with the ground address rule of
/// FANS-1/A: the 7 characters before the IMI when they are letters or digits and the character before them is not, else the 4
/// before it under the same test.
pub fn find_cpdlc<'a>(text: &'a str, registration: &str) -> Option<Envelope<'a>> {
    find_where(text, registration, true)
}

fn find_where<'a>(text: &'a str, registration: &str, cpdlc: bool) -> Option<Envelope<'a>> {
    let b = text.as_bytes();
    let reg = registration.trim_start_matches('.');
    let mut i = 0;
    while i + 4 <= b.len() {
        let at = i;
        i += 1;
        if b[at] != b'.' {
            continue;
        }
        let Some(imi) = text
            .get(at + 1..at + 4)
            .and_then(Imi::parse)
            .filter(|i| !cpdlc || i.is_cpdlc())
        else {
            continue;
        };
        // The ground address: 7 address characters, or 4 at the start or after '/' or a space (CPDLC: after any character
        // that is not an address character).
        let ground_start = [7usize, 4].into_iter().find_map(|n| {
            let s = at.checked_sub(n)?;
            let lead = s == 0
                || if cpdlc {
                    !addr_char(b[s - 1])
                } else {
                    n == 7 || matches!(b[s - 1], b'/' | b' ')
                };
            (b[s..at].iter().all(|&c| addr_char(c)) && lead).then_some(s)
        });
        let Some(gs) = ground_start else { continue };
        let a0 = at + 4;
        let after = &text[a0..];
        let undotted = after.trim_start_matches('.');
        let dots = after.len() - undotted.len();
        let alen = if !reg.is_empty() && reg.len() < 7 && undotted.starts_with(reg) {
            dots + reg.len()
        } else {
            7
        };
        let Some(address) = text.get(a0..a0 + alen) else {
            continue;
        };
        let rest = &text[a0 + alen..];
        let n = rest.bytes().take_while(u8::is_ascii_hexdigit).count() & !1;
        let hex = &rest[..n];
        let crc_ok = hex_crc_ok(imi, address.trim_start_matches('.'), hex);
        return Some(Envelope {
            ground: &text[gs..at],
            imi,
            address,
            hex,
            crc_ok,
        });
    }
    None
}

/// Appends an envelope to `out`: `GROUND.IMI`, the address padded with leading dots to 7 characters, the data and the CRC in
/// upper-case hex. An error when the ground address is not 4 or 7 letters or digits, or the address is longer than 7
/// characters.
pub fn build(ground: &str, imi: Imi, address: &str, data: &[u8], out: &mut String) -> Result<()> {
    if !matches!(ground.len(), 4 | 7) || !ground.bytes().all(addr_char) {
        return Err(Error::BadField(Item::GroundAddress));
    }
    let address = address.trim_start_matches('.');
    if address.len() > 7 || !address.is_ascii() {
        return Err(Error::BadField(Item::AircraftAddress));
    }
    out.reserve(12 + 2 * data.len() + 4);
    out.push_str(ground);
    out.push('.');
    out.push_str(imi.as_str());
    for _ in address.len()..7 {
        out.push('.');
    }
    out.push_str(address);
    for b in data {
        let _ = write!(out, "{b:02X}");
    }
    let _ = write!(out, "{:04X}", crc(imi, address, data));
    Ok(())
}
