//! D-PRS: APRS packets in the slow data of D-STAR voice, as `$$CRCxxxx,` followed by a TNC2 packet and a carriage return.
//!
//! Standard: JARL D-STAR specification, D-PRS GPS-A mode

use crate::Part;
use crate::{Error, Result};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;

/// A D-PRS line, split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dprs<'a> {
    /// The TNC2 packet (without the carriage return).
    pub tnc2: &'a str,
    /// The CRC the line carried.
    pub crc: u16,
    /// The CRC is right.
    pub crc_ok: bool,
}

/// Reads a D-PRS line. The CRC is the HDLC FCS (CRC-16/IBM-SDLC) of the text after the comma, carriage return included. An error when it does not start with `$$CRC`, four hex digits and a comma. A wrong CRC is not an error:
/// [`Dprs::crc_ok`] says so.
///
/// ```
/// use sdrkit_proto_link::dprs;
/// let mut line = String::new();
/// dprs::encode("JA1ABC-A>API51,DSTAR*:!3541.50N/13945.00E>test", &mut line);
/// let d = dprs::parse(&line).unwrap();
/// assert!(d.crc_ok);
/// assert_eq!(d.tnc2, "JA1ABC-A>API51,DSTAR*:!3541.50N/13945.00E>test");
/// ```
pub fn parse(line: &str) -> Result<Dprs<'_>> {
    let rest = line
        .strip_prefix("$$CRC")
        .ok_or(Error::Syntax(Part::DprsHeader))?;
    let hex = rest.get(..4).ok_or(Error::Short)?;
    let crc = u16::from_str_radix(hex, 16).map_err(|_| Error::Syntax(Part::DprsCrc))?;
    let body = rest[4..]
        .strip_prefix(',')
        .ok_or(Error::Syntax(Part::DprsHeader))?;
    // The CRC covers the packet and its closing carriage return. For a line received without it, the CRC is computed as if it were there.
    let packet = body.trim_end_matches(['\r', '\n']);
    let c = crc_of(packet);
    Ok(Dprs {
        tnc2: packet,
        crc,
        crc_ok: c == crc,
    })
}

/// The CRC of a packet and its closing carriage return.
fn crc_of(packet: &str) -> u16 {
    let mut data = Vec::with_capacity(packet.len() + 1);
    data.extend_from_slice(packet.as_bytes());
    data.push(b'\r');
    CRC16_IBM_SDLC.checksum(&data)
}

/// Appends the D-PRS line of a TNC2 packet (with its CRC and a closing carriage return) to `out`.
pub fn encode(tnc2: &str, out: &mut String) {
    let crc = crc_of(tnc2);
    out.push_str(&format!("$$CRC{crc:04X},"));
    out.push_str(tnc2);
    out.push('\r');
}
