//! Advertising data: the AD structures of AdvData and ScanRspData (Bluetooth Core Specification v5.4, Vol 3 Part C clause 11, and
//! the Core Specification Supplement v11, Part A).
//!
//! An AD structure is a length byte, an AD type byte and `length - 1` data bytes. A length of 0 ends the significant part
//! (the rest is padding). [`Structures`] walks them without copying. [`Ad::value`] reads the common types. Company and UUID names
//! come from `sdrkit_util_ids::bluetooth`. Mode-specific readings of manufacturer or service data (iBeacon, Eddystone, Remote ID ...)
//! stay in the stacks. They take the [`AdValue::Manufacturer`] and [`AdValue::ServiceData16`] slices from here.
//!
//! ```
//! use sdrkit_proto_wlan_phy::ble::ad::{self, AdValue};
//! let mut data = Vec::new();
//! ad::push(&mut data, ad::FLAGS, &[0x06]).unwrap();
//! ad::push_manufacturer(&mut data, 0x004C, &[0x02, 0x15]).unwrap();
//! let v: Vec<AdValue> = ad::structures(&data).map(|s| s.value()).collect();
//! assert_eq!(v[0], AdValue::Flags(0x06));
//! assert_eq!(v[1], AdValue::Manufacturer { company: 0x004C, data: &[0x02, 0x15] });
//! assert_eq!(ad::company_name(0x004C), Some("Apple, Inc."));
//! ```

use crate::{Error, Result};

/// AD type: Flags (Supplement Part A clause 1.3).
pub const FLAGS: u8 = 0x01;
/// AD type: incomplete list of 16-bit service UUIDs (clause 1.1).
pub const UUID16_INCOMPLETE: u8 = 0x02;
/// AD type: complete list of 16-bit service UUIDs.
pub const UUID16_COMPLETE: u8 = 0x03;
/// AD type: incomplete list of 32-bit service UUIDs.
pub const UUID32_INCOMPLETE: u8 = 0x04;
/// AD type: complete list of 32-bit service UUIDs.
pub const UUID32_COMPLETE: u8 = 0x05;
/// AD type: incomplete list of 128-bit service UUIDs.
pub const UUID128_INCOMPLETE: u8 = 0x06;
/// AD type: complete list of 128-bit service UUIDs.
pub const UUID128_COMPLETE: u8 = 0x07;
/// AD type: shortened local name (clause 1.2).
pub const SHORT_NAME: u8 = 0x08;
/// AD type: complete local name.
pub const COMPLETE_NAME: u8 = 0x09;
/// AD type: TX power level, dBm (clause 1.5).
pub const TX_POWER: u8 = 0x0A;
/// AD type: service data with a 16-bit UUID (clause 1.11).
pub const SERVICE_DATA16: u8 = 0x16;
/// AD type: appearance (clause 1.12).
pub const APPEARANCE: u8 = 0x19;
/// AD type: service data with a 32-bit UUID.
pub const SERVICE_DATA32: u8 = 0x20;
/// AD type: service data with a 128-bit UUID.
pub const SERVICE_DATA128: u8 = 0x21;
/// AD type: manufacturer specific data (clause 1.4): a company identifier, then the company's bytes.
pub const MANUFACTURER: u8 = 0xFF;

/// One AD structure: its type and data (the bytes after the type byte).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ad<'a> {
    /// The AD type.
    pub kind: u8,
    /// The data.
    pub data: &'a [u8],
}

/// A list of UUIDs of one width, read on demand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Uuids<'a> {
    /// The width of each UUID, bytes (2, 4 or 16).
    pub width: usize,
    /// The list as sent (each UUID least significant byte first).
    pub bytes: &'a [u8],
}

impl<'a> Uuids<'a> {
    /// The 16- and 32-bit UUIDs as numbers (128-bit lists give none: use [`Uuids::bytes`]).
    pub fn values(&self) -> impl Iterator<Item = u32> + 'a {
        let w = if self.width <= 4 {
            self.width
        } else {
            usize::MAX
        };
        self.bytes
            .chunks_exact(w.min(self.bytes.len().max(1)))
            .filter(move |c| c.len() == w)
            .map(|c| c.iter().rev().fold(0u32, |a, &b| (a << 8) | b as u32))
    }
}

/// What a common AD structure says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdValue<'a> {
    /// Flags (bit 0 LE limited discoverable, 1 LE general discoverable, 2 BR/EDR not supported, ...).
    Flags(u8),
    /// A list of service UUIDs. `complete` says whether the list is complete.
    Uuids {
        /// Complete or incomplete list.
        complete: bool,
        /// The UUIDs.
        list: Uuids<'a>,
    },
    /// The local name, UTF-8 as sent ([`AdValue::name_text`] returns it as text). `complete` is false for a shortened name.
    Name {
        /// Complete or shortened.
        complete: bool,
        /// The name bytes.
        bytes: &'a [u8],
    },
    /// TX power level, dBm.
    TxPower(i8),
    /// Appearance (GAP appearance value).
    Appearance(u16),
    /// Service data for a 16-bit UUID.
    ServiceData16 {
        /// The UUID.
        uuid: u16,
        /// The service's bytes.
        data: &'a [u8],
    },
    /// Service data for a 32-bit UUID.
    ServiceData32 {
        /// The UUID.
        uuid: u32,
        /// The service's bytes.
        data: &'a [u8],
    },
    /// Service data for a 128-bit UUID (least significant byte first, as sent).
    ServiceData128 {
        /// The UUID bytes as sent.
        uuid: &'a [u8],
        /// The service's bytes.
        data: &'a [u8],
    },
    /// Manufacturer specific data.
    Manufacturer {
        /// The company identifier (`sdrkit_util_ids::bluetooth::company`).
        company: u16,
        /// The company's bytes after the identifier.
        data: &'a [u8],
    },
    /// Any other type, or a known type whose length is wrong.
    Other(Ad<'a>),
}

impl<'a> AdValue<'a> {
    /// The name as text (invalid UTF-8 replaced), for [`AdValue::Name`].
    pub fn name_text(&self) -> Option<std::borrow::Cow<'a, str>> {
        match *self {
            AdValue::Name { bytes, .. } => Some(String::from_utf8_lossy(bytes)),
            _ => None,
        }
    }
}

impl<'a> Ad<'a> {
    /// Reads the structure as one of the common types.
    pub fn value(&self) -> AdValue<'a> {
        let d = self.data;
        let u16le = |at: usize| u16::from_le_bytes([d[at], d[at + 1]]);
        match self.kind {
            FLAGS if d.len() == 1 => AdValue::Flags(d[0]),
            UUID16_INCOMPLETE | UUID16_COMPLETE if d.len().is_multiple_of(2) => AdValue::Uuids {
                complete: self.kind == UUID16_COMPLETE,
                list: Uuids { width: 2, bytes: d },
            },
            UUID32_INCOMPLETE | UUID32_COMPLETE if d.len().is_multiple_of(4) => AdValue::Uuids {
                complete: self.kind == UUID32_COMPLETE,
                list: Uuids { width: 4, bytes: d },
            },
            UUID128_INCOMPLETE | UUID128_COMPLETE if d.len().is_multiple_of(16) => AdValue::Uuids {
                complete: self.kind == UUID128_COMPLETE,
                list: Uuids {
                    width: 16,
                    bytes: d,
                },
            },
            SHORT_NAME | COMPLETE_NAME => AdValue::Name {
                complete: self.kind == COMPLETE_NAME,
                bytes: d,
            },
            TX_POWER if d.len() == 1 => AdValue::TxPower(d[0] as i8),
            APPEARANCE if d.len() == 2 => AdValue::Appearance(u16le(0)),
            SERVICE_DATA16 if d.len() >= 2 => AdValue::ServiceData16 {
                uuid: u16le(0),
                data: &d[2..],
            },
            SERVICE_DATA32 if d.len() >= 4 => AdValue::ServiceData32 {
                uuid: u32::from_le_bytes([d[0], d[1], d[2], d[3]]),
                data: &d[4..],
            },
            SERVICE_DATA128 if d.len() >= 16 => AdValue::ServiceData128 {
                uuid: &d[..16],
                data: &d[16..],
            },
            MANUFACTURER if d.len() >= 2 => AdValue::Manufacturer {
                company: u16le(0),
                data: &d[2..],
            },
            _ => AdValue::Other(*self),
        }
    }
}

/// The AD structures of advertising data, in order (see [`structures`]).
#[derive(Clone, Debug)]
pub struct Structures<'a> {
    rest: &'a [u8],
    truncated: bool,
}

impl<'a> Structures<'a> {
    /// Whether the walk stopped at a structure running past the end of the data (seen once the iterator has ended).
    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

impl<'a> Iterator for Structures<'a> {
    type Item = Ad<'a>;

    fn next(&mut self) -> Option<Ad<'a>> {
        let (&len, tail) = self.rest.split_first()?;
        let len = len as usize;
        if len == 0 {
            // The significant part ends. The rest is padding.
            self.rest = &[];
            return None;
        }
        if len > tail.len() {
            self.truncated = true;
            self.rest = &[];
            return None;
        }
        let (s, rest) = tail.split_at(len);
        self.rest = rest;
        Some(Ad {
            kind: s[0],
            data: &s[1..],
        })
    }
}

/// Walks the AD structures of `data`. A zero length ends the walk (padding follows). A structure running past the end stops it
/// (and sets [`Structures::truncated`]).
pub fn structures(data: &[u8]) -> Structures<'_> {
    Structures {
        rest: data,
        truncated: false,
    }
}

/// The first structure of type `kind`.
pub fn find(data: &[u8], kind: u8) -> Option<Ad<'_>> {
    structures(data).find(|a| a.kind == kind)
}

/// Every service-data structure for the 16-bit `uuid`: their data after the UUID.
pub fn service_data16(data: &[u8], uuid: u16) -> impl Iterator<Item = &[u8]> {
    structures(data).filter_map(move |a| match a.value() {
        AdValue::ServiceData16 { uuid: u, data } if u == uuid => Some(data),
        _ => None,
    })
}

/// The company name of a company identifier (Bluetooth Assigned Numbers, through `sdrkit_util_ids::bluetooth`).
pub fn company_name(id: u16) -> Option<&'static str> {
    sdrkit_util_ids::bluetooth::company(id)
}

/// Appends one AD structure. Errors when the data is over 254 bytes.
pub fn push(out: &mut Vec<u8>, kind: u8, data: &[u8]) -> Result<()> {
    if data.len() > 254 {
        return Err(Error::TooLong);
    }
    out.push(data.len() as u8 + 1);
    out.push(kind);
    out.extend_from_slice(data);
    Ok(())
}

/// Appends manufacturer specific data for `company`.
pub fn push_manufacturer(out: &mut Vec<u8>, company: u16, data: &[u8]) -> Result<()> {
    if data.len() > 252 {
        return Err(Error::TooLong);
    }
    out.push(data.len() as u8 + 3);
    out.push(MANUFACTURER);
    out.extend_from_slice(&company.to_le_bytes());
    out.extend_from_slice(data);
    Ok(())
}

/// Appends service data for the 16-bit `uuid`.
pub fn push_service_data16(out: &mut Vec<u8>, uuid: u16, data: &[u8]) -> Result<()> {
    if data.len() > 252 {
        return Err(Error::TooLong);
    }
    out.push(data.len() as u8 + 3);
    out.push(SERVICE_DATA16);
    out.extend_from_slice(&uuid.to_le_bytes());
    out.extend_from_slice(data);
    Ok(())
}

/// Appends a complete list of 16-bit service UUIDs.
pub fn push_uuids16(out: &mut Vec<u8>, uuids: &[u16]) -> Result<()> {
    if uuids.len() > 127 {
        return Err(Error::TooLong);
    }
    out.push(2 * uuids.len() as u8 + 1);
    out.push(UUID16_COMPLETE);
    for u in uuids {
        out.extend_from_slice(&u.to_le_bytes());
    }
    Ok(())
}
