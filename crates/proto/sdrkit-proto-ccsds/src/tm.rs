//! TM transfer frames (CCSDS 132.0-B-3).
//!
//! A TM transfer frame has a 6-byte primary header, an optional secondary header, the data field, an optional operational control
//! field (OCF, 4 bytes) and an optional frame error control field (FECF, a CRC-16 over the rest of the frame). [`Layout`] holds
//! the parts a link uses. [`Layout::parse`] reads a frame and [`Layout::build`] writes one.
//!
//! ```
//! use sdrkit_proto_ccsds::tm::{FirstHeader, Layout, PrimaryHeader};
//! let layout = Layout { frame_len: 64, secondary_header_len: 0, ocf: true, fecf: true };
//! let h = PrimaryHeader { spacecraft: 0x2AB, vcid: 5, ocf: true, mc_count: 9, vc_count: 200, first_header: FirstHeader::At(0), ..PrimaryHeader::default() };
//! let mut frame = vec![0u8; 64];
//! layout.build(&h, &[], &[1, 2, 3], Some(0xDEAD_BEEF), &mut frame).unwrap();
//! let f = layout.parse(&frame).unwrap();
//! assert_eq!(f.header, h);
//! assert_eq!(f.ocf, Some(0xDEAD_BEEF));
//! assert_eq!(f.fecf_ok, Some(true));
//! assert_eq!(&f.data[..3], &[1, 2, 3]);
//! ```

use crate::{be16, be32, Error, HeaderField, Param, Result};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;

/// The first header pointer of a TM data field or an AOS M_PDU (CCSDS 132.0-B-3 clause 4.1.2.7.6, 732.0-B-4 clause 4.1.4.2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FirstHeader {
    /// The first packet header starts at this byte of the packet zone.
    At(u16),
    /// No packet starts in this frame (all ones, 0x7FF). The zone continues a packet.
    #[default]
    None,
    /// The zone holds only idle data (all ones minus one, 0x7FE).
    IdleOnly,
}

impl FirstHeader {
    /// The pointer from its 11-bit field.
    pub const fn from_field(v: u16) -> Self {
        match v & 0x7FF {
            0x7FF => FirstHeader::None,
            0x7FE => FirstHeader::IdleOnly,
            p => FirstHeader::At(p),
        }
    }

    /// The 11-bit field.
    pub const fn field(self) -> u16 {
        match self {
            FirstHeader::At(p) => p & 0x7FF,
            FirstHeader::None => 0x7FF,
            FirstHeader::IdleOnly => 0x7FE,
        }
    }
}

/// The TM primary header (CCSDS 132.0-B-3 clause 4.1.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PrimaryHeader {
    /// Transfer frame version number (0 for TM).
    pub version: u8,
    /// Spacecraft identifier, 10 bits.
    pub spacecraft: u16,
    /// Virtual channel identifier, 3 bits.
    pub vcid: u8,
    /// An operational control field is present.
    pub ocf: bool,
    /// Master channel frame count, 8 bits.
    pub mc_count: u8,
    /// Virtual channel frame count, 8 bits.
    pub vc_count: u8,
    /// A secondary header is present.
    pub secondary_header: bool,
    /// Synchronisation flag: the data field is not packet-synchronous (VCA_SDU or idle data).
    pub sync: bool,
    /// Packet order flag (0 when `sync` is 0).
    pub packet_order: bool,
    /// Segment length identifier, 2 bits (3 when `sync` is 0).
    pub segment_length_id: u8,
    /// The first header pointer.
    pub first_header: FirstHeader,
}

impl PrimaryHeader {
    /// Reads the 6 header bytes.
    pub fn parse(b: &[u8]) -> Result<Self> {
        if b.len() < 6 {
            return Err(Error::InputTooShort { need: 6 });
        }
        let id = be16(b);
        let status = be16(&b[4..]);
        Ok(PrimaryHeader {
            version: (id >> 14) as u8,
            spacecraft: (id >> 4) & 0x3FF,
            vcid: ((id >> 1) & 7) as u8,
            ocf: id & 1 != 0,
            mc_count: b[2],
            vc_count: b[3],
            secondary_header: status >> 15 != 0,
            sync: (status >> 14) & 1 != 0,
            packet_order: (status >> 13) & 1 != 0,
            segment_length_id: ((status >> 11) & 3) as u8,
            first_header: FirstHeader::from_field(status),
        })
    }

    /// Writes the 6 header bytes. If a field does not fit its width, it returns an error.
    pub fn write(&self, out: &mut [u8]) -> Result<()> {
        if out.len() < 6 {
            return Err(Error::OutputTooShort { need: 6 });
        }
        if self.version > 3
            || self.spacecraft > 0x3FF
            || self.vcid > 7
            || self.segment_length_id > 3
        {
            return Err(Error::BadField(HeaderField::TmHeader));
        }
        let id = (self.version as u16) << 14
            | self.spacecraft << 4
            | (self.vcid as u16) << 1
            | self.ocf as u16;
        let status = (self.secondary_header as u16) << 15
            | (self.sync as u16) << 14
            | (self.packet_order as u16) << 13
            | (self.segment_length_id as u16) << 11
            | self.first_header.field();
        out[..2].copy_from_slice(&id.to_be_bytes());
        out[2] = self.mc_count;
        out[3] = self.vc_count;
        out[4..6].copy_from_slice(&status.to_be_bytes());
        Ok(())
    }

    /// The master channel identifier: version and spacecraft (clause 4.1.2.1).
    pub fn mcid(&self) -> u16 {
        (self.version as u16) << 10 | self.spacecraft
    }
}

/// The parts a link puts in its TM frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Bytes of a frame (fixed for a physical channel).
    pub frame_len: usize,
    /// Bytes of the secondary header including its 1-byte identification field, or 0 when there is none.
    pub secondary_header_len: usize,
    /// Frames carry an OCF. The header's flag marks it in each frame; this field reserves its space in the layout.
    pub ocf: bool,
    /// Frames end with a FECF.
    pub fecf: bool,
}

/// A TM frame taken apart by [`Layout::parse`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TmFrame<'a> {
    /// The primary header.
    pub header: PrimaryHeader,
    /// The secondary header data (after its identification byte), empty when there is none.
    pub secondary: &'a [u8],
    /// The data field.
    pub data: &'a [u8],
    /// The OCF, when the header says one is present.
    pub ocf: Option<u32>,
    /// The FECF result, when the layout has one.
    pub fecf_ok: Option<bool>,
}

impl Layout {
    /// Bytes of the data field.
    pub fn data_len(&self) -> usize {
        self.frame_len.saturating_sub(
            6 + self.secondary_header_len + 4 * self.ocf as usize + 2 * self.fecf as usize,
        )
    }

    /// Takes a frame apart. If the frame is shorter than the layout, it returns an error.
    pub fn parse<'a>(&self, frame: &'a [u8]) -> Result<TmFrame<'a>> {
        if frame.len() < self.frame_len || self.data_len() == 0 {
            return Err(Error::InputTooShort {
                need: self.frame_len,
            });
        }
        let frame = &frame[..self.frame_len];
        let header = PrimaryHeader::parse(frame)?;
        let mut pos = 6;
        let mut secondary: &[u8] = &[];
        if self.secondary_header_len > 0 {
            secondary = &frame[pos + 1..pos + self.secondary_header_len];
            pos += self.secondary_header_len;
        }
        let data = &frame[pos..pos + self.data_len()];
        pos += self.data_len();
        let ocf = if self.ocf {
            let v = be32(&frame[pos..]);
            pos += 4;
            header.ocf.then_some(v)
        } else {
            None
        };
        let fecf_ok = self
            .fecf
            .then(|| CRC16_IBM_3740.checksum(&frame[..pos]) == be16(&frame[pos..]));
        Ok(TmFrame {
            header,
            secondary,
            data,
            ocf,
            fecf_ok,
        })
    }

    /// Writes a frame into `out` (`frame_len` bytes): header, secondary header (its identification byte made from the layout),
    /// `data` (zero padded), OCF and FECF.
    pub fn build(
        &self,
        header: &PrimaryHeader,
        secondary: &[u8],
        data: &[u8],
        ocf: Option<u32>,
        out: &mut [u8],
    ) -> Result<()> {
        if out.len() < self.frame_len {
            return Err(Error::OutputTooShort {
                need: self.frame_len,
            });
        }
        if self.data_len() == 0 || data.len() > self.data_len() {
            return Err(Error::BadLength);
        }
        if self.secondary_header_len > 0 && secondary.len() > self.secondary_header_len - 1 {
            return Err(Error::BadLength);
        }
        if self.secondary_header_len > 64 {
            return Err(Error::BadParameter(Param::TmSecondaryHeader));
        }
        let out = &mut out[..self.frame_len];
        out.fill(0);
        header.write(out)?;
        let mut pos = 6;
        if self.secondary_header_len > 0 {
            // Version 00 and the length of the secondary header minus one (clause 4.1.3.2).
            out[pos] = (self.secondary_header_len - 1) as u8 & 0x3F;
            out[pos + 1..pos + 1 + secondary.len()].copy_from_slice(secondary);
            pos += self.secondary_header_len;
        }
        out[pos..pos + data.len()].copy_from_slice(data);
        pos += self.data_len();
        if self.ocf {
            out[pos..pos + 4].copy_from_slice(&ocf.unwrap_or(0).to_be_bytes());
            pos += 4;
        }
        if self.fecf {
            let c = CRC16_IBM_3740.checksum(&out[..pos]);
            out[pos..pos + 2].copy_from_slice(&c.to_be_bytes());
        }
        Ok(())
    }
}
