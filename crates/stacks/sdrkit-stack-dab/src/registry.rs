//! The registered names DAB signals by code: text in a label character set, languages, programme types, announcement types and
//! user applications. The tables live in the shared crates. This module reads them with DAB's codes.
//!
//! Standard: TS 101 756.

use sdrkit_util_ids::pty::PtyTable;
use sdrkit_util_text::charset_field::{display_into, encode_into, Charset};

/// Label or text bytes in character set `charset` (TS 101 756 table 1: 0 EBU Latin, 4 ISO 8859-1, 6 UCS-2, 15 UTF-8) as text
/// for display: control characters dropped, trailing white space removed. A reserved set is read as EBU Latin.
pub fn label_text(bytes: &[u8], charset: u8) -> String {
    let mut s = String::with_capacity(bytes.len());
    display_into(bytes, Charset::from_code(charset), &mut s);
    s
}

/// Text in EBU Latin (characters it cannot hold become `?`), for the test signal.
pub fn to_ebu_latin(s: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(s.len());
    encode_into(s, Charset::EbuLatin, &mut v);
    v
}

/// The language name for a code (TS 101 756 tables 9 and 10), empty for a reserved code.
pub fn language(code: u8) -> &'static str {
    sdrkit_util_ids::language::broadcast(code).unwrap_or("")
}

/// The programme type name for code `pty` in international table `table` (1: TS 101 756 table 12, 2: table 13, North
/// America), empty for a code without a name.
pub fn programme_type(pty: u8, table: u8) -> &'static str {
    let t = if table == 2 {
        PtyTable::DabNorthAmerica
    } else {
        PtyTable::Dab
    };
    sdrkit_util_ids::pty::name(pty, t).unwrap_or("")
}

/// The announcement type name for bit `n` of the ASu and ASw flags (TS 101 756 table 14).
pub fn announcement_type(n: u8) -> &'static str {
    sdrkit_util_ids::announcement::name(n).unwrap_or("Reserved")
}

/// The user application name (TS 101 756 table 16).
pub fn user_application(t: u16) -> &'static str {
    match t {
        0x002 => "SlideShow",
        0x004 => "TPEG",
        0x007 => "SPI",
        0x009 => "DMB",
        0x00D => "Filecasting",
        0x00E => "FIS",
        0x44A => "Journaline",
        _ => "Reserved",
    }
}

/// The name of a character set code (TS 101 756 table 1).
pub fn charset_name(c: u8) -> &'static str {
    match c {
        0 => "EBU Latin (0)",
        4 => "ISO 8859-1 (4)",
        6 => "UCS-2 (6)",
        15 => "UTF-8 (15)",
        _ => "Reserved",
    }
}
