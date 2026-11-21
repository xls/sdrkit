//! ACARS blocks (ARINC 618, "Air/Ground Character-Oriented Protocol Specification").
//!
//! A block as sent on VHF (and inside the VDL Mode 2, HFDL and satellite links that carry ACARS):
//!
//! ```text
//! pre-key  '+' '*'  SYN SYN  SOH  mode  address(7)  TAK  label(2)  block-id  [STX text]  ETX|ETB  BCS(2)  DEL
//! ```
//!
//! Every character from the mode to the ETX or ETB is 7-bit ASCII with **odd parity in bit 7**. Thus the end characters go as
//! 0x83 (ETX) and 0x97 (ETB) ([`ETX`], [`ETB`]). The block check sequence (BCS) is CRC-16/KERMIT
//! ([`sdrkit_util_bits::crc::catalog::CRC16_KERMIT`]) over those bytes as sent, low byte first. The text holds at most 220 characters
//! ([`MAX_TEXT`]). A downlink (aircraft to ground) has a digit block id and starts its text with a 4-character message sequence
//! number and a 6-character flight id. An uplink has a letter block id.
//!
//! In this module a **body** is the bytes from the mode to the ETX or ETB inclusive. The work goes in three steps, none of which
//! allocates:
//!
//! 1. [`split_frame`] finds the body and the BCS in a received frame.
//! 2. [`check_and_repair`] tests the BCS and repairs up to three bit errors with the parity bits.
//! 3. [`strip_parity`] removes the parity bits and [`Block::parse`] returns a borrowed view of the fields.
//!
//! [`assemble::Assembler`] joins the blocks of a multi-block message and [`Message`] is the owned, fully read message.
//!
//! ```
//! use sdrkit_proto_aero::acars::{self, Block, BlockSpec, End, Check};
//! let spec = BlockSpec::new('2', "N123AB", b'2', End::Etx)
//!     .label("QA")
//!     .text("M01AAB0123KJFK1030");
//! let mut frame = Vec::new();
//! spec.frame(0, false, &mut frame).unwrap();
//! let f = acars::split_frame(&frame).unwrap();
//! let mut body = f.body.to_vec();
//! assert_eq!(acars::check_and_repair(&mut body, f.bcs), Check::Good { bits_fixed: 0 });
//! acars::strip_parity(&mut body);
//! let b = Block::parse(&body).unwrap();
//! assert_eq!(b.registration(), "N123AB");
//! assert_eq!(b.downlink_prefix().unwrap().flight, "AB0123");
//! ```

pub mod assemble;
mod message;
mod repair;

pub use message::{read_text, reading_of, Message, Reading, Tak};
pub use repair::{check_and_repair, syndrome, Check, MAX_BODY};

use crate::error::{Error, FramingByte, Item, Result};
use sdrkit_util_bits::checksum::{odd_parity7, with_odd_parity7};
use sdrkit_util_bits::crc::catalog::CRC16_KERMIT;

/// Start of heading (ARINC 618 clause 2.3.1). It has an odd number of one bits, so it is the same with parity: 0x01.
pub const SOH: u8 = 0x01;
/// Start of text (the same with parity: 0x02).
pub const STX: u8 = 0x02;
/// End of text as sent, with its odd parity bit: 0x83. It ends the last (or only) block of a message. The 7-bit character is
/// [`ETX_CHAR`]. Compare received bytes that still have their parity bits with this constant.
pub const ETX: u8 = 0x83;
/// End of block as sent, with its odd parity bit: 0x97. More blocks follow. The 7-bit character is [`ETB_CHAR`].
pub const ETB: u8 = 0x97;
/// The 7-bit ETX character (0x03), for bytes whose parity bits are cleared.
pub const ETX_CHAR: u8 = 0x03;
/// The 7-bit ETB character (0x17), for bytes whose parity bits are cleared.
pub const ETB_CHAR: u8 = 0x17;
/// Synchronous idle, the character sync sent twice before SOH (the same with parity: 0x16).
pub const SYN: u8 = 0x16;
/// Delete: the block suffix after the BCS, and the second label character of the general response label `_` DEL (the same
/// with parity: 0x7F).
pub const DEL: u8 = 0x7F;
/// Negative acknowledgement: the technical acknowledgement character when nothing is acknowledged (the same with parity: 0x15).
pub const NAK: u8 = 0x15;
/// Acknowledgement, the 7-bit character. It has an even number of one bits, so it is sent as 0x86.
pub const ACK: u8 = 0x06;
/// The pre-key byte (all ones) sent before the bit sync.
pub const PREKEY: u8 = 0xFF;
/// The bit sync characters `+` `*` as sent with parity: 0xAB 0x2A.
pub const BIT_SYNC_SENT: [u8; 2] = [0xAB, 0x2A];
/// The most text characters one block holds (ARINC 618 clause 2.3.6).
pub const MAX_TEXT: usize = 220;

/// `c` (7 bits) with its odd parity bit in bit 7, as every ACARS character is sent.
#[inline]
pub const fn with_parity(c: u8) -> u8 {
    with_odd_parity7(c)
}

/// Whether `b` has odd parity over its 8 bits.
#[inline]
pub const fn parity_ok(b: u8) -> bool {
    odd_parity7(b).is_some()
}

/// Clears bit 7 of every byte in place and returns how many bytes had even (wrong) parity.
pub fn strip_parity(body: &mut [u8]) -> usize {
    let mut bad = 0;
    for b in body.iter_mut() {
        bad += usize::from(!parity_ok(*b));
        *b &= 0x7F;
    }
    bad
}

/// The block check sequence of a body as sent: CRC-16/KERMIT, low byte first.
pub fn bcs(body: &[u8]) -> [u8; 2] {
    CRC16_KERMIT.checksum(body).to_le_bytes()
}

/// How a block ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum End {
    /// ETX: the last (or only) block of a message.
    Etx,
    /// ETB: more blocks follow.
    Etb,
}

/// The parts of a received frame (see [`split_frame`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame<'a> {
    /// The body as sent (mode .. ETX or ETB, parity bits on).
    pub body: &'a [u8],
    /// The two BCS bytes, low first.
    pub bcs: [u8; 2],
    /// Bytes of the input used, including the suffix DEL when present.
    pub len: usize,
}

/// Finds the body and the BCS of the frame at the start of `bytes`. The frame can start with pre-key (0xFF), bit sync (`+*`)
/// and SYN bytes. Then come SOH, the body up to the first ETX or ETB (parity ignored), the two BCS bytes and an optional DEL.
pub fn split_frame(bytes: &[u8]) -> Result<Frame<'_>> {
    let mut p = 0;
    while p < bytes.len() && matches!(bytes[p], PREKEY | SYN | 0xAB | 0x2A) {
        p += 1;
    }
    match bytes.get(p) {
        None => return Err(Error::Short),
        Some(&b) if b & 0x7F == SOH => p += 1,
        Some(_) => return Err(Error::Framing(FramingByte::Soh)),
    }
    let start = p;
    // Mode, address, TAK, label and block id come before anything that can end the block.
    let first_end = start + 12;
    if bytes.len() <= first_end {
        return Err(Error::Short);
    }
    let end = bytes[first_end..]
        .iter()
        .take(MAX_BODY - 12)
        .position(|&b| matches!(b & 0x7F, ETX_CHAR | ETB_CHAR))
        .map(|i| first_end + i)
        .ok_or(if bytes.len() - first_end > MAX_BODY - 12 {
            Error::TooLong(MAX_BODY)
        } else {
            Error::Short
        })?;
    let bcs = match bytes.get(end + 1..end + 3) {
        Some(c) => [c[0], c[1]],
        None => return Err(Error::Short),
    };
    let mut len = end + 3;
    if bytes.get(len).map(|b| b & 0x7F) == Some(DEL) {
        len += 1;
    }
    Ok(Frame {
        body: &bytes[start..=end],
        bcs,
        len,
    })
}

/// A block's fields, borrowed from a body whose parity bits are cleared ([`strip_parity`]). All text views are 7-bit ASCII.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block<'a> {
    /// The mode character (`2` for category B, `A` .. for category A ...).
    pub mode: u8,
    /// The address as sent: the registration with leading dots to 7 characters.
    pub address: &'a str,
    /// The technical acknowledgement character (NAK, or the block id acknowledged).
    pub tak: u8,
    /// The label, with a DEL second character given as `d` (so the general response reads `_d`).
    pub label: [u8; 2],
    /// The block id: a digit for a downlink, a letter for an uplink, 0 or space when none.
    pub block_id: u8,
    /// The text between STX and the end character, when the block has one.
    pub text: Option<&'a str>,
    /// How the block ends.
    pub end: End,
}

/// The fields at the start of a downlink text (ARINC 618 clause 2.3.6.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DownlinkPrefix<'a> {
    /// The message sequence number: three characters (`M01`).
    pub msn: &'a str,
    /// The block sequence character after it (`A` for the first block, `B` ...).
    pub sequence: u8,
    /// The flight id, trailing spaces removed.
    pub flight: &'a str,
    /// The text after the prefix.
    pub rest: &'a str,
}

/// The sublabel and message function identifier at the start of an H1 text (ARINC 620 clause 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sublabel<'a> {
    /// Two characters.
    pub sublabel: &'a str,
    /// The message function identifier, two characters, when present.
    pub mfi: Option<&'a str>,
}

fn ascii(b: &[u8]) -> Result<&str> {
    match b.iter().find(|&&c| c >= 0x80) {
        Some(&c) => Err(Error::BadChar(c)),
        // 7-bit bytes are always valid UTF-8.
        None => std::str::from_utf8(b).map_err(|_| Error::BadChar(0x80)),
    }
}

impl<'a> Block<'a> {
    /// Reads the fields of `body` (mode .. ETX or ETB, parity bits cleared). It returns an error in three cases: the body is
    /// shorter than a block without text, a byte still has bit 7 set, or the body does not end in ETX or ETB.
    pub fn parse(body: &'a [u8]) -> Result<Block<'a>> {
        if body.len() < 13 {
            return Err(Error::Short);
        }
        if body.len() > MAX_BODY {
            return Err(Error::TooLong(MAX_BODY));
        }
        let end = match body[body.len() - 1] {
            ETX_CHAR => End::Etx,
            ETB_CHAR => End::Etb,
            _ => return Err(Error::Framing(FramingByte::EtxOrEtb)),
        };
        let text = match body.len() {
            13 => None,
            _ if body[12] == STX => Some(ascii(&body[13..body.len() - 1])?),
            _ => return Err(Error::Framing(FramingByte::Stx)),
        };
        let l2 = if body[10] == DEL { b'd' } else { body[10] };
        Ok(Block {
            mode: body[0],
            address: ascii(&body[1..8])?,
            tak: body[8],
            label: [body[9], l2],
            block_id: body[11],
            text,
            end,
        })
    }

    /// The registration: the address without its leading dots and spaces.
    pub fn registration(&self) -> &'a str {
        self.address.trim_start_matches('.').trim()
    }

    /// The label as text (`H1`, `_d`).
    pub fn label_str(&self) -> &str {
        std::str::from_utf8(&self.label).unwrap_or("??")
    }

    /// A digit block id is a downlink (aircraft to ground).
    pub fn downlink(&self) -> bool {
        self.block_id.is_ascii_digit()
    }

    /// The downlink prefix, when this is a downlink with at least 10 text characters.
    pub fn downlink_prefix(&self) -> Option<DownlinkPrefix<'a>> {
        let t = self.text?;
        if !self.downlink() || t.len() < 10 {
            return None;
        }
        Some(DownlinkPrefix {
            msn: &t[..3],
            sequence: t.as_bytes()[3],
            flight: t[4..10].trim_end(),
            rest: &t[10..],
        })
    }

    /// The application text: the text after the downlink prefix (a downlink) or the whole text (an uplink). Empty when none.
    pub fn app_text(&self) -> &'a str {
        match self.downlink_prefix() {
            Some(p) => p.rest,
            None if self.downlink() => "",
            None => self.text.unwrap_or(""),
        }
    }

    /// The H1 sublabel and MFI: `#M1B/PS ...` in a downlink, `- #MD/AT ...` in an uplink.
    pub fn sublabel(&self) -> Option<Sublabel<'a>> {
        if &self.label != b"H1" {
            return None;
        }
        sublabel(self.app_text(), self.downlink())
    }
}

/// The H1 sublabel and MFI at the start of an application text.
pub fn sublabel(t: &str, downlink: bool) -> Option<Sublabel<'_>> {
    let b = t.as_bytes();
    let (sub, rest) = if downlink && b.len() >= 4 && b[0] == b'#' && b[3] == b'B' {
        (&t[1..3], &t[4..])
    } else if !downlink && b.len() >= 5 && t.starts_with("- #") {
        (&t[3..5], &t[5..])
    } else {
        return None;
    };
    let r = rest.as_bytes();
    let mfi = (r.len() >= 4 && r[0] == b'/' && r[3] == b' ').then(|| &rest[1..3]);
    Some(Sublabel { sublabel: sub, mfi })
}

/// What a block is made of, for [`BlockSpec::body`] and [`BlockSpec::frame`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockSpec<'a> {
    /// The mode character.
    pub mode: char,
    /// The registration (padded with leading dots to 7 characters when sent).
    pub address: &'a str,
    /// The technical acknowledgement character (NAK when nothing is acknowledged).
    pub tak: u8,
    /// The label: two characters, `_d` for `_` DEL.
    pub label: &'a str,
    /// The block id.
    pub block_id: u8,
    /// The text after STX, when there is one (a downlink includes its prefix). 7-bit ASCII, at most [`MAX_TEXT`] characters.
    pub text: Option<&'a str>,
    /// How the block ends.
    pub end: End,
}

impl<'a> BlockSpec<'a> {
    /// A block with NAK as the acknowledgement, label `_d` and no text.
    pub fn new(mode: char, address: &'a str, block_id: u8, end: End) -> Self {
        BlockSpec {
            mode,
            address,
            tak: NAK,
            label: "_d",
            block_id,
            text: None,
            end,
        }
    }

    /// The same with this label.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    /// The same with this text.
    pub fn text(mut self, text: &'a str) -> Self {
        self.text = Some(text);
        self
    }

    /// The same with this acknowledgement character.
    pub fn tak(mut self, tak: u8) -> Self {
        self.tak = tak;
        self
    }

    /// Appends the body as sent (parity on) to `out` and returns its BCS (low byte first). These are errors, and nothing is appended:
    /// a character that is not 7-bit printable where one is needed, an address over 7 characters, a label that is not two
    /// characters, and text over [`MAX_TEXT`].
    pub fn body(&self, out: &mut Vec<u8>) -> Result<[u8; 2]> {
        let printable = |c: u8| (0x20..0x7F).contains(&c);
        let a = self.address.as_bytes();
        if a.len() > 7 {
            return Err(Error::TooLong(7));
        }
        if let Some(&c) = a.iter().find(|&&c| !printable(c)) {
            return Err(Error::BadChar(c));
        }
        let l = self.label.as_bytes();
        if l.len() != 2 {
            return Err(Error::BadField(Item::Label));
        }
        let mode = u8::try_from(self.mode as u32)
            .ok()
            .filter(|&c| printable(c))
            .ok_or(Error::BadField(Item::Mode))?;
        let text = self.text.map(str::as_bytes);
        if let Some(t) = text {
            if t.len() > MAX_TEXT {
                return Err(Error::TooLong(MAX_TEXT));
            }
            // Control characters such as CR and LF are allowed in text. ETX, ETB, STX, SOH and bytes over 0x7E are not allowed.
            if let Some(&c) = t
                .iter()
                .find(|&&c| c > 0x7E || matches!(c, SOH | STX | ETX_CHAR | ETB_CHAR))
            {
                return Err(Error::BadChar(c));
            }
        }
        if self.tak > 0x7F || self.block_id > 0x7F {
            return Err(Error::BadField(Item::TakOrBlockId));
        }
        let start = out.len();
        out.push(with_parity(mode));
        out.extend(std::iter::repeat_n(with_parity(b'.'), 7 - a.len()));
        out.extend(a.iter().map(|&c| with_parity(c)));
        out.push(with_parity(self.tak));
        out.push(with_parity(l[0]));
        out.push(with_parity(if self.label == "_d" { DEL } else { l[1] }));
        out.push(with_parity(self.block_id));
        if let Some(t) = text {
            out.push(with_parity(STX));
            out.extend(t.iter().map(|&c| with_parity(c)));
        }
        out.push(match self.end {
            End::Etx => ETX,
            End::Etb => ETB,
        });
        Ok(bcs(&out[start..]))
    }

    /// Appends the whole frame to `out`. The frame is `prekey` bytes of 0xFF, the bit and character sync (`+*` SYN SYN) when
    /// `sync`, then SOH, the body, the BCS and DEL.
    pub fn frame(&self, prekey: usize, sync: bool, out: &mut Vec<u8>) -> Result<()> {
        let start = out.len();
        out.extend(std::iter::repeat_n(PREKEY, prekey));
        if sync {
            out.extend_from_slice(&BIT_SYNC_SENT);
            out.extend_from_slice(&[SYN, SYN]);
        }
        out.push(SOH);
        match self.body(out) {
            Ok(c) => {
                out.extend_from_slice(&c);
                out.push(DEL);
                Ok(())
            }
            Err(e) => {
                out.truncate(start);
                Err(e)
            }
        }
    }
}

/// The next block id of a multi-block message: digits 0 to 9 (downlink) and letters A to Z (uplink) wrap around.
pub fn next_block_id(id: u8) -> u8 {
    match id {
        b'9' => b'0',
        b'Z' => b'A',
        b'0'..=b'8' | b'A'..=b'Y' => id + 1,
        _ => id,
    }
}

/// Appends the printable reading of 7-bit text to `out`. CR LF, a lone CR and a lone LF each give one line break. Other
/// control characters and DEL become `.`. At most `max` characters are written. Returns whether the text was cut.
pub fn printable_into(text: &[u8], max: usize, out: &mut String) -> bool {
    let mut i = 0;
    let mut n = 0;
    while i < text.len() {
        if n == max {
            return true;
        }
        let c = text[i] & 0x7F;
        match c {
            b'\r' => {
                out.push('\n');
                if text.get(i + 1).map(|x| x & 0x7F) == Some(b'\n') {
                    i += 1;
                }
            }
            b'\n' => out.push('\n'),
            0..=0x1F | 0x7F => out.push('.'),
            _ => out.push(c as char),
        }
        n += 1;
        i += 1;
    }
    false
}
