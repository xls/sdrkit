//! MSC data groups: the header, the session header and the CRC. DAB carries them in packet mode and X-PAD, DRM in its
//! packet mode.
//!
//! Standard: ETSI EN 300 401 clause 5.3.3, ETSI ES 201 980 clause 6.6.2.

use crate::{Error, Result};
use crate::{Malformed, Parameter};
use sdrkit_util_bits::crc::catalog::CRC16_GENIBUS;
use sdrkit_util_bits::crc::Trailer;

/// Data group type 0: general data.
pub const GENERAL: u8 = 0;
/// Data group type 1: CA messages.
pub const CA_MESSAGES: u8 = 1;
/// Data group type 3: MOT header (EN 301 234 clause 5.1).
pub const MOT_HEADER: u8 = 3;
/// Data group type 4: MOT body, not scrambled.
pub const MOT_BODY: u8 = 4;
/// Data group type 5: MOT body, scrambled.
pub const MOT_BODY_SCRAMBLED: u8 = 5;
/// Data group type 6: MOT directory, not compressed.
pub const MOT_DIRECTORY: u8 = 6;
/// Data group type 7: MOT directory, compressed.
pub const MOT_DIRECTORY_COMPRESSED: u8 = 7;

/// The header and session header of a data group. A field that is absent is `None`. [`Header::write`] sends only the fields that
/// are present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// Data group type (0 to 15).
    pub group_type: u8,
    /// Continuity index (0 to 15).
    pub continuity: u8,
    /// Repetition index (0 to 15).
    pub repetition: u8,
    /// The extension field.
    pub extension: Option<u16>,
    /// The CRC is present. After [`parse`], the CRC is also correct.
    pub crc: bool,
    /// The segment field: last flag and segment number (0 to 32767).
    pub segment: Option<(bool, u16)>,
    /// The transport id of the user access field.
    pub transport_id: Option<u16>,
    /// The user access field is present (with or without a transport id or end user address).
    pub user_access: bool,
}

impl Default for Header {
    /// A header with a CRC and a user access field, everything else zero or absent.
    fn default() -> Self {
        Header {
            group_type: 0,
            continuity: 0,
            repetition: 0,
            extension: None,
            crc: true,
            segment: None,
            transport_id: None,
            user_access: true,
        }
    }
}

/// A data group read from bytes: its header, end user address and data field, borrowed from the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataGroup<'a> {
    /// The header fields.
    pub header: Header,
    /// The end user address (empty when absent).
    pub end_user_address: &'a [u8],
    /// The data group data field.
    pub data: &'a [u8],
}

impl DataGroup<'_> {
    /// The segment number, 0 without a segment field.
    pub fn segment_number(&self) -> u16 {
        self.header.segment.map_or(0, |s| s.1)
    }
    /// The last flag of the segment field, true without one.
    pub fn is_last(&self) -> bool {
        self.header.segment.is_none_or(|s| s.0)
    }
}

/// Reads a data group. If the CRC flag is set, it makes sure that the CRC is correct.
///
/// ```
/// use sdrkit_proto_bcdata::datagroup::{self, Header};
///
/// let mut g = Vec::new();
/// let h = Header { group_type: datagroup::MOT_BODY, segment: Some((true, 0)), transport_id: Some(0x1234), ..Header::default() };
/// h.write(b"abc", &mut g).unwrap();
/// let d = datagroup::parse(&g).unwrap();
/// assert_eq!((d.header.transport_id, d.data), (Some(0x1234), &b"abc"[..]));
/// ```
pub fn parse(b: &[u8]) -> Result<DataGroup<'_>> {
    if b.len() < 2 {
        return Err(Error::Truncated);
    }
    let has_ext = b[0] & 0x80 != 0;
    let crc = b[0] & 0x40 != 0;
    let has_seg = b[0] & 0x20 != 0;
    let user_access = b[0] & 0x10 != 0;
    let end = if crc {
        if b.len() < 4 {
            return Err(Error::Truncated);
        }
        if !CRC16_GENIBUS.verify(b, Trailer::Big) {
            return Err(Error::Crc);
        }
        b.len() - 2
    } else {
        b.len()
    };
    let mut h = Header {
        group_type: b[0] & 15,
        continuity: b[1] >> 4,
        repetition: b[1] & 15,
        extension: None,
        crc,
        segment: None,
        transport_id: None,
        user_access,
    };
    let mut i = 2;
    let need = |i: usize, n: usize| {
        if i + n > end {
            Err(Error::Truncated)
        } else {
            Ok(())
        }
    };
    if has_ext {
        need(i, 2)?;
        h.extension = Some(u16::from_be_bytes([b[i], b[i + 1]]));
        i += 2;
    }
    if has_seg {
        need(i, 2)?;
        let v = u16::from_be_bytes([b[i], b[i + 1]]);
        h.segment = Some((v & 0x8000 != 0, v & 0x7FFF));
        i += 2;
    }
    let mut eua: &[u8] = &[];
    if user_access {
        need(i, 1)?;
        let tid_flag = b[i] & 0x10 != 0;
        let len = (b[i] & 15) as usize;
        i += 1;
        need(i, len)?;
        let mut j = i;
        if tid_flag {
            if len < 2 {
                return Err(Error::Invalid(Malformed::UserAccess));
            }
            h.transport_id = Some(u16::from_be_bytes([b[j], b[j + 1]]));
            j += 2;
        }
        eua = &b[j..i + len];
        i += len;
    }
    Ok(DataGroup {
        header: h,
        end_user_address: eua,
        data: &b[i..end],
    })
}

impl Header {
    /// Appends a data group with this header, no end user address, `data` and (when [`Header::crc`]) the CRC.
    pub fn write(&self, data: &[u8], out: &mut Vec<u8>) -> Result<()> {
        self.write_with_address(&[], data, out)
    }

    /// Appends a data group with an end user address (at most 15 bytes, 13 with a transport id).
    pub fn write_with_address(
        &self,
        end_user_address: &[u8],
        data: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<()> {
        if self.group_type > 15 || self.continuity > 15 || self.repetition > 15 {
            return Err(Error::BadParameter(Parameter::DataGroupHeader));
        }
        let ua_len = end_user_address.len() + if self.transport_id.is_some() { 2 } else { 0 };
        if ua_len > 15 || (!self.user_access && ua_len > 0) {
            return Err(Error::BadParameter(Parameter::UserAccess));
        }
        if self.segment.is_some_and(|s| s.1 > 0x7FFF) {
            return Err(Error::BadParameter(Parameter::SegmentNumber));
        }
        let start = out.len();
        out.push(
            ((self.extension.is_some() as u8) << 7)
                | ((self.crc as u8) << 6)
                | ((self.segment.is_some() as u8) << 5)
                | ((self.user_access as u8) << 4)
                | self.group_type,
        );
        out.push((self.continuity << 4) | self.repetition);
        if let Some(e) = self.extension {
            out.extend_from_slice(&e.to_be_bytes());
        }
        if let Some((last, n)) = self.segment {
            out.extend_from_slice(&(((last as u16) << 15) | n).to_be_bytes());
        }
        if self.user_access {
            out.push(((self.transport_id.is_some() as u8) << 4) | ua_len as u8);
            if let Some(t) = self.transport_id {
                out.extend_from_slice(&t.to_be_bytes());
            }
            out.extend_from_slice(end_user_address);
        }
        out.extend_from_slice(data);
        if self.crc {
            let c = CRC16_GENIBUS.checksum(&out[start..]);
            out.extend_from_slice(&c.to_be_bytes());
        }
        Ok(())
    }
}
