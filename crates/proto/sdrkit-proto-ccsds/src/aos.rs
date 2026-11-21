//! AOS transfer frames (CCSDS 732.0-B-4).
//!
//! An AOS transfer frame (also called a VCDU, a virtual channel data unit) has a 6-byte primary header, an optional 2-byte frame
//! header error control field, an optional insert zone of fixed length, the data field, an optional OCF (operational control
//! field, 4 bytes) and an optional FECF (frame error control field, a CRC-16). The data field of a packet channel is an M_PDU
//! (multiplexing protocol data unit): a 2-byte header with the first header pointer, then the packet zone. A bitstream channel
//! uses a B_PDU instead. [`Layout`] holds the parts a link uses.
//!
//! The frame header error control field is a Reed-Solomon (10,6) code over GF(16). This crate keeps its place but does not
//! decode it.
//!
//! ```
//! use sdrkit_proto_ccsds::aos::{Layout, PrimaryHeader};
//! use sdrkit_proto_ccsds::tm::FirstHeader;
//! let layout = Layout::METEOR;
//! assert_eq!(layout.data_zone_len(), 882);
//! let h = PrimaryHeader { spacecraft: 0x2A, vcid: 5, counter: 0x12_3456, ..PrimaryHeader::default() };
//! let mut frame = vec![0u8; layout.frame_len];
//! layout.build(&h, &[0xAB, 0xCD], FirstHeader::At(10), &[7; 882], &mut frame).unwrap();
//! let f = layout.parse(&frame).unwrap();
//! assert_eq!(f.header, h);
//! assert_eq!(f.first_header, FirstHeader::At(10));
//! assert_eq!(f.insert_zone, &[0xAB, 0xCD]);
//! ```

use crate::tm::FirstHeader;
use crate::{be16, be32, Error, HeaderField, Result};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;

/// The AOS virtual channel of fill frames (all ones, clause 4.1.2.3).
pub const IDLE_VCID: u8 = 63;

/// The AOS primary header (CCSDS 732.0-B-4 clause 4.1.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrimaryHeader {
    /// Transfer frame version number (1 for AOS).
    pub version: u8,
    /// Spacecraft identifier, 8 bits.
    pub spacecraft: u8,
    /// Virtual channel identifier, 6 bits (63 is fill).
    pub vcid: u8,
    /// Virtual channel frame count, 24 bits.
    pub counter: u32,
    /// Replay flag: the frame is played back, not real time.
    pub replay: bool,
    /// The frame count cycle field is in use.
    pub cycle_in_use: bool,
    /// Virtual channel frame count cycle, 4 bits: it counts the wraps of `counter` when `cycle_in_use`.
    pub cycle: u8,
}

impl Default for PrimaryHeader {
    fn default() -> Self {
        PrimaryHeader {
            version: 1,
            spacecraft: 0,
            vcid: 0,
            counter: 0,
            replay: false,
            cycle_in_use: false,
            cycle: 0,
        }
    }
}

impl PrimaryHeader {
    /// Reads the 6 header bytes.
    pub fn parse(b: &[u8]) -> Result<Self> {
        if b.len() < 6 {
            return Err(Error::InputTooShort { need: 6 });
        }
        let id = be16(b);
        Ok(PrimaryHeader {
            version: (id >> 14) as u8,
            spacecraft: (id >> 6) as u8,
            vcid: (id & 0x3F) as u8,
            counter: (b[2] as u32) << 16 | (b[3] as u32) << 8 | b[4] as u32,
            replay: b[5] & 0x80 != 0,
            cycle_in_use: b[5] & 0x40 != 0,
            cycle: b[5] & 0x0F,
        })
    }

    /// Writes the 6 header bytes. If a field does not fit its width, it returns an error.
    pub fn write(&self, out: &mut [u8]) -> Result<()> {
        if out.len() < 6 {
            return Err(Error::OutputTooShort { need: 6 });
        }
        if self.version > 3 || self.vcid > 63 || self.counter > 0xFF_FFFF || self.cycle > 15 {
            return Err(Error::BadField(HeaderField::AosHeader));
        }
        let id = (self.version as u16) << 14 | (self.spacecraft as u16) << 6 | self.vcid as u16;
        out[..2].copy_from_slice(&id.to_be_bytes());
        out[2] = (self.counter >> 16) as u8;
        out[3] = (self.counter >> 8) as u8;
        out[4] = self.counter as u8;
        out[5] = (self.replay as u8) << 7 | (self.cycle_in_use as u8) << 6 | self.cycle;
        Ok(())
    }

    /// The frame count with the cycle extension when it is in use (28 bits), else the 24-bit count.
    pub fn extended_counter(&self) -> u32 {
        if self.cycle_in_use {
            (self.cycle as u32) << 24 | self.counter
        } else {
            self.counter
        }
    }

    /// True for a fill frame (virtual channel 63).
    pub fn is_idle(&self) -> bool {
        self.vcid == IDLE_VCID
    }
}

/// The parts a link puts in its AOS frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Bytes of a frame (892 behind RS(255,223) with depth 4).
    pub frame_len: usize,
    /// The 2-byte frame header error control field is present.
    pub header_error_control: bool,
    /// Bytes of the insert zone.
    pub insert_zone: usize,
    /// Frames carry an OCF.
    pub ocf: bool,
    /// Frames end with a FECF.
    pub fecf: bool,
}

/// An AOS frame with an M_PDU, taken apart by [`Layout::parse`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AosFrame<'a> {
    /// The primary header.
    pub header: PrimaryHeader,
    /// The insert zone.
    pub insert_zone: &'a [u8],
    /// The data field (M_PDU or B_PDU header included).
    pub data_field: &'a [u8],
    /// The first header pointer of the M_PDU.
    pub first_header: FirstHeader,
    /// The M_PDU packet zone (after its header).
    pub zone: &'a [u8],
    /// The OCF, when the layout has one.
    pub ocf: Option<u32>,
    /// The FECF result, when the layout has one.
    pub fecf_ok: Option<bool>,
}

impl Layout {
    /// 892-byte frames, no insert zone, no OCF and no FECF: RS(255,223) depth 4 links (GOES, GK-2A, Elektro-L).
    pub const RS_I4: Layout = Layout {
        frame_len: 892,
        header_error_control: false,
        insert_zone: 0,
        ocf: false,
        fecf: false,
    };

    /// As [`Layout::RS_I4`] with a 2-byte insert zone (Meteor-M LRPT, MetOp AHRPT, FY-3).
    pub const METEOR: Layout = Layout {
        insert_zone: 2,
        ..Layout::RS_I4
    };

    /// 1020-byte frames without Reed-Solomon (FY-4, behind an LDPC code).
    pub const FY4: Layout = Layout {
        frame_len: 1020,
        ..Layout::RS_I4
    };

    /// 2044-byte frames behind the marker of 2048-byte CADUs, ending in a 2-byte FECF, no insert zone and no OCF: the frames
    /// carried in DVB-S2 data fields. The zone is 2034 bytes.
    pub const CADU_2048: Layout = Layout {
        frame_len: 2044,
        fecf: true,
        ..Layout::RS_I4
    };

    fn trailer(&self) -> usize {
        4 * self.ocf as usize + 2 * self.fecf as usize
    }

    fn data_start(&self) -> usize {
        6 + 2 * self.header_error_control as usize + self.insert_zone
    }

    /// Bytes of the data field.
    pub fn data_field_len(&self) -> usize {
        self.frame_len
            .saturating_sub(self.data_start() + self.trailer())
    }

    /// Bytes of the M_PDU packet zone (the data field less its 2-byte header).
    pub fn data_zone_len(&self) -> usize {
        self.data_field_len().saturating_sub(2)
    }

    /// Takes a frame apart, reading the data field as an M_PDU. If the frame is shorter than the layout, it returns an error.
    pub fn parse<'a>(&self, frame: &'a [u8]) -> Result<AosFrame<'a>> {
        if frame.len() < self.frame_len || self.data_zone_len() == 0 {
            return Err(Error::InputTooShort {
                need: self.frame_len,
            });
        }
        let frame = &frame[..self.frame_len];
        let header = PrimaryHeader::parse(frame)?;
        let ds = self.data_start();
        let insert_zone = &frame[ds - self.insert_zone..ds];
        let data_field = &frame[ds..ds + self.data_field_len()];
        let mut pos = ds + self.data_field_len();
        let ocf = if self.ocf {
            pos += 4;
            Some(be32(&frame[pos - 4..]))
        } else {
            None
        };
        let fecf_ok = self
            .fecf
            .then(|| CRC16_IBM_3740.checksum(&frame[..pos]) == be16(&frame[pos..]));
        Ok(AosFrame {
            header,
            insert_zone,
            data_field,
            first_header: FirstHeader::from_field(be16(data_field)),
            zone: &data_field[2..],
            ocf,
            fecf_ok,
        })
    }

    /// Writes a frame with an M_PDU into `out` (`frame_len` bytes): header, insert zone (zero padded), M_PDU header with
    /// `first_header`, `zone` (zero padded to the zone length), a zero OCF and the FECF.
    pub fn build(
        &self,
        header: &PrimaryHeader,
        insert: &[u8],
        first_header: FirstHeader,
        zone: &[u8],
        out: &mut [u8],
    ) -> Result<()> {
        if out.len() < self.frame_len {
            return Err(Error::OutputTooShort {
                need: self.frame_len,
            });
        }
        if self.data_zone_len() == 0
            || zone.len() > self.data_zone_len()
            || insert.len() > self.insert_zone
        {
            return Err(Error::BadLength);
        }
        let out = &mut out[..self.frame_len];
        out.fill(0);
        header.write(out)?;
        let ds = self.data_start();
        out[ds - self.insert_zone..ds - self.insert_zone + insert.len()].copy_from_slice(insert);
        out[ds..ds + 2].copy_from_slice(&first_header.field().to_be_bytes());
        out[ds + 2..ds + 2 + zone.len()].copy_from_slice(zone);
        let pos = ds + self.data_field_len() + 4 * self.ocf as usize;
        if self.fecf {
            let c = CRC16_IBM_3740.checksum(&out[..pos]);
            out[pos..pos + 2].copy_from_slice(&c.to_be_bytes());
        }
        Ok(())
    }
}

/// The B_PDU header of a bitstream channel (CCSDS 732.0-B-4 clause 4.1.5): the bitstream data pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitstreamPointer {
    /// The last valid user data bit is at this bit of the bitstream zone. The rest is idle.
    LastValid(u16),
    /// Every bit of the zone is user data (all ones, 0x3FFF).
    AllValid,
    /// The zone holds only idle data (all ones minus one, 0x3FFE).
    IdleOnly,
}

impl BitstreamPointer {
    /// Reads the 2-byte B_PDU header.
    pub fn parse(b: &[u8]) -> Result<Self> {
        if b.len() < 2 {
            return Err(Error::InputTooShort { need: 2 });
        }
        Ok(match be16(b) & 0x3FFF {
            0x3FFF => BitstreamPointer::AllValid,
            0x3FFE => BitstreamPointer::IdleOnly,
            p => BitstreamPointer::LastValid(p),
        })
    }

    /// The 2-byte B_PDU header (spare bits 0).
    pub fn bytes(self) -> [u8; 2] {
        let v = match self {
            BitstreamPointer::AllValid => 0x3FFF,
            BitstreamPointer::IdleOnly => 0x3FFE,
            BitstreamPointer::LastValid(p) => p & 0x3FFF,
        };
        v.to_be_bytes()
    }
}
