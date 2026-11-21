//! The baseband header (BBHEADER) of DVB-S2, DVB-S2X and DVB-T2, and baseband descrambling. Some fields have another
//! meaning in each system, so [`BbHeader::rolloff`] and [`BbHeader::gse_sliced`] take the [`System`].
//!
//! Standard: ETSI EN 302 307-1 clauses 5.1.6 and 5.2.2, ETSI EN 302 755 clause 5.1.7.

use crate::error::{Check, Malformed};
use crate::error::{Error, Result};
use sdrkit_util_bits::crc::catalog::CRC8_DVB_S2;
use sdrkit_util_bits::seq::ByteSequence;

/// The length of a BBHEADER in bytes.
pub const BBHEADER_LEN: usize = 10;

/// The system whose baseband header is read. It decides the meaning of the EXT/RO bits, of MATYPE-2 and of TS/GS = 10.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    /// DVB-S2 and DVB-S2X (ETSI EN 302 307-1 and -2).
    DvbS2,
    /// DVB-T2 (ETSI EN 302 755). DVB-C2 uses the same header.
    DvbT2,
}

/// What the data field carries (the TS/GS field of MATYPE-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamFormat {
    /// 00: a generic packetised stream (GFPS) of user packets of UPL bits.
    GenericPacketised,
    /// 01: a generic continuous stream (GCS). In normal mode GSE is also sent with this value (EN 302 755 table 1 note 2).
    GenericContinuous,
    /// 10: GSE. In DVB-S2X this value is GSE-HEM. In DVB-T2 it is a Generic Encapsulated Stream.
    Gse,
    /// 11: an MPEG-2 transport stream.
    Transport,
}

impl StreamFormat {
    fn bits(self) -> u8 {
        match self {
            StreamFormat::GenericPacketised => 0,
            StreamFormat::GenericContinuous => 1,
            StreamFormat::Gse => 2,
            StreamFormat::Transport => 3,
        }
    }
    fn from_bits(b: u8) -> StreamFormat {
        match b & 3 {
            0 => StreamFormat::GenericPacketised,
            1 => StreamFormat::GenericContinuous,
            2 => StreamFormat::Gse,
            _ => StreamFormat::Transport,
        }
    }
}

/// The mode adaptation mode, from the MODE value XORed into the CRC-8 byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// MODE 0: normal mode. User packets carry a CRC-8 and, when used, their own ISSY field.
    Normal,
    /// MODE 1: high efficiency mode. Transport packets carry no sync byte and no CRC-8. One ISSY field is in the header.
    HighEfficiency,
}

/// A parsed BBHEADER.
///
/// ```
/// use sdrkit_proto_ipcast::bbheader::{BbHeader, StreamFormat};
/// // A single transport stream, CCM, roll-off 0.35, UPL 188 x 8 (EN 302 307-1 table 4).
/// let h = BbHeader::transport(38_608, 0);
/// let mut b = [0u8; 10];
/// h.write(&mut b);
/// assert_eq!(&b[..9], &[0xF0, 0x00, 0x05, 0xE0, 0x96, 0xD0, 0x47, 0x00, 0x00]);
/// let back = BbHeader::parse(&b).unwrap();
/// assert_eq!(back.format, StreamFormat::Transport);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BbHeader {
    /// TS/GS: what the data field carries.
    pub format: StreamFormat,
    /// SIS/MIS: true for a single input stream.
    pub single_stream: bool,
    /// CCM/ACM: true for constant coding and modulation.
    pub ccm: bool,
    /// ISSYI: true when the input stream synchronizer (ISSY) is active.
    pub issyi: bool,
    /// NPD: true when null packet deletion is active.
    pub npd: bool,
    /// The last 2 bits of MATYPE-1: RO in DVB-S2, EXT in DVB-T2.
    pub ext: u8,
    /// MATYPE-2: the ISI (DVB-S2) or PLP_ID (DVB-T2). It has a meaning only for multiple input streams.
    pub stream_id: u8,
    /// UPL: the length of a transmitted user packet in bits. In HEM this field is not sent and is 0 here.
    pub upl: u16,
    /// DFL: the length of the data field in bits.
    pub dfl: u16,
    /// SYNC: the sync byte of the user packets. In HEM this field is not sent and is 0 here.
    pub sync: u8,
    /// SYNCD: the distance in bits from the start of the data field to the first user packet that starts in it. 65535: no
    /// user packet starts in this data field.
    pub syncd: u16,
    /// The mode adaptation mode.
    pub mode: Mode,
    /// In HEM with ISSYI set: the ISSY field from the UPL bytes (2 most significant) and the SYNC byte (least significant).
    pub issy: Option<[u8; 3]>,
}

impl BbHeader {
    /// The header of a single transport stream in normal mode, CCM, roll-off 0.35: the header of EN 302 307-1 table 4.
    pub fn transport(dfl: u16, syncd: u16) -> BbHeader {
        BbHeader {
            format: StreamFormat::Transport,
            single_stream: true,
            ccm: true,
            issyi: false,
            npd: false,
            ext: 0,
            stream_id: 0,
            upl: 188 * 8,
            dfl,
            sync: 0x47,
            syncd,
            mode: Mode::Normal,
            issy: None,
        }
    }

    /// Reads the first 10 bytes of `b`. If the input is short, it returns [`Error::Short`]. If the CRC-8 matches neither MODE 0
    /// nor MODE 1, it returns [`Error::Check`].
    pub fn parse(b: &[u8]) -> Result<BbHeader> {
        let b: &[u8; BBHEADER_LEN] = b
            .get(..BBHEADER_LEN)
            .and_then(|s| s.try_into().ok())
            .ok_or(Error::Short)?;
        let crc = CRC8_DVB_S2.checksum(&b[..9]);
        let mode = match crc ^ b[9] {
            0 => Mode::Normal,
            1 => Mode::HighEfficiency,
            _ => return Err(Error::Check(Check::BbHeaderCrc8)),
        };
        let format = StreamFormat::from_bits(b[0] >> 6);
        let issyi = b[0] & 0x08 != 0;
        let (upl, sync, issy) = match mode {
            Mode::Normal => (u16::from_be_bytes([b[2], b[3]]), b[6], None),
            Mode::HighEfficiency => (0, 0, issyi.then_some([b[2], b[3], b[6]])),
        };
        Ok(BbHeader {
            format,
            single_stream: b[0] & 0x20 != 0,
            ccm: b[0] & 0x10 != 0,
            issyi,
            npd: b[0] & 0x04 != 0,
            ext: b[0] & 3,
            stream_id: b[1],
            upl,
            dfl: u16::from_be_bytes([b[4], b[5]]),
            sync,
            syncd: u16::from_be_bytes([b[7], b[8]]),
            mode,
            issy,
        })
    }

    /// Writes the 10 header bytes, with the CRC-8 XOR the MODE last.
    pub fn write(&self, out: &mut [u8; BBHEADER_LEN]) {
        out[0] = (self.format.bits() << 6)
            | (self.single_stream as u8) << 5
            | (self.ccm as u8) << 4
            | (self.issyi as u8) << 3
            | (self.npd as u8) << 2
            | (self.ext & 3);
        out[1] = self.stream_id;
        let (upl, sync) = match (self.mode, self.issy) {
            (Mode::HighEfficiency, Some(i)) => ([i[0], i[1]], i[2]),
            (Mode::HighEfficiency, None) => ([0, 0], 0),
            (Mode::Normal, _) => (self.upl.to_be_bytes(), self.sync),
        };
        out[2..4].copy_from_slice(&upl);
        out[4..6].copy_from_slice(&self.dfl.to_be_bytes());
        out[6] = sync;
        out[7..9].copy_from_slice(&self.syncd.to_be_bytes());
        let mode = match self.mode {
            Mode::Normal => 0,
            Mode::HighEfficiency => 1,
        };
        out[9] = CRC8_DVB_S2.checksum(&out[..9]) ^ mode;
    }

    /// The roll-off factor that the RO bits give in DVB-S2 (EN 302 307-1 table 3): 0.35, 0.25 or 0.20. RO = 11 is reserved in
    /// DVB-S2 and has other meanings in DVB-S2X, so it returns `None`. In DVB-T2 the bits are EXT and this returns `None`.
    pub fn rolloff(&self, system: System) -> Option<f32> {
        match (system, self.ext) {
            (System::DvbS2, 0) => Some(0.35),
            (System::DvbS2, 1) => Some(0.25),
            (System::DvbS2, 2) => Some(0.20),
            _ => None,
        }
    }

    /// The data field length in whole bytes. If DFL is not a multiple of 8, it returns [`Error::Invalid`].
    pub fn data_field_len(&self) -> Result<usize> {
        if !self.dfl.is_multiple_of(8) {
            return Err(Error::Invalid(Malformed::Dfl));
        }
        Ok(self.dfl as usize / 8)
    }

    /// Whether the data field carries GSE packets: TS/GS = 10, or TS/GS = 01 with UPL = 0 (GSE sent as a continuous stream).
    pub fn carries_gse(&self) -> bool {
        match self.format {
            StreamFormat::Gse => true,
            StreamFormat::GenericContinuous => self.upl == 0,
            _ => false,
        }
    }

    /// Whether GSE packets can be cut across data fields, so that SYNCD points to the first packet start: GSE in HEM, or the
    /// DVB-S2X GSE-HEM format (TS/GS = 10 in DVB-S2).
    pub fn gse_sliced(&self, system: System) -> bool {
        self.format == StreamFormat::Gse
            && (self.mode == Mode::HighEfficiency || system == System::DvbS2)
    }
}

/// Descrambles (or scrambles) a whole BBFRAME in place: header, data field and padding. The sequence is 1 + x^14 + x^15 loaded
/// with 100101010000000 at the start of each BBFRAME (EN 302 307-1 clause 5.2.2, EN 302 755 clause 5.2.2). It does not allocate.
pub fn bb_descramble(frame: &mut [u8]) {
    let mut seq = ByteSequence::register(
        sdrkit_util_bits::seq::DVB_DISPERSAL,
        sdrkit_util_bits::BitOrder::Msb,
    );
    seq.apply(frame);
}
