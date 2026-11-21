//! xRIT header records (CGMS LRIT/HRIT Global Specification 2.6 section 4, with the mission dialects).
//!
//! A file starts with header records. Each record has a type (1 byte) and a length (2 bytes, the record included). The primary
//! header (type 0, 16 bytes) holds the file type, the total header length and the data field length in bits.
//!
//! [`records`] iterates over the records of a file. [`parse`] reads the records this crate supports into [`Headers`].
//! The records that differ between missions are read in a [`Dialect`]:
//!
//! | Type | Record | Dialects |
//! |---|---|---|
//! | 1 | image structure (bits per pixel, columns, lines, compression flag) | all, FY-4 with its own layout |
//! | 2 | image navigation (projection, scaling factors and offsets) | all, FY-4 with floats |
//! | 3 | image data function | all |
//! | 4 | annotation (the file name) | all |
//! | 5 | time stamp (CCSDS day segmented time: days since 1958-01-01, milliseconds of day) | all |
//! | 6 | ancillary text | all |
//! | 7 | key header (encryption key number) | GK-2A, FY-4 |
//! | 128 | segment identification | GOES, GK-2A (different layouts) |
//! | 129 | NOAA product identification (product, sub-product, parameter, compression) | GOES |
//! | 130 | header structure text | GOES |
//! | 131 | Rice compression (flags, pixels per block, lines per packet) | GOES |
//! | 131 | observation time text | GK-2A |
//!
//! ```
//! use sdrkit_proto_ccsds::xrit::headers::{parse, Builder, Dialect};
//! let mut file = Builder::new(0).image(8, 2712, 226, 1).annotation("test.lrit").build(4);
//! file.extend_from_slice(&[1, 2, 3, 4]);
//! let h = parse(&file, Dialect::Goes).unwrap();
//! assert_eq!(h.data_bits, 32);
//! assert_eq!(h.image.as_ref().unwrap().columns, 2712);
//! assert_eq!(h.data(&file), &[1, 2, 3, 4]);
//! ```

use crate::rice;
use crate::{be16, be32, Error, HeaderField, Result};

/// The header dialect of a mission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Dialect {
    /// GOES and Elektro-L (CGMS with the NOAA records).
    #[default]
    Goes,
    /// KMA GK-2A.
    Gk2a,
    /// FY-4.
    Fy4,
}

/// The compression flag of the image structure record (CGMS 4.3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Compression {
    /// No compression (0).
    #[default]
    None,
    /// Lossless compression (1).
    Lossless,
    /// Lossy compression (2).
    Lossy,
    /// Another value.
    Other(u8),
}

impl Compression {
    /// The flag from its byte.
    pub const fn from_byte(b: u8) -> Self {
        match b {
            0 => Compression::None,
            1 => Compression::Lossless,
            2 => Compression::Lossy,
            v => Compression::Other(v),
        }
    }
}

/// The compression named in the NOAA product record (GOES-R HRIT/EMWIN specification, record 129).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NoaaCompression {
    /// No compression (0).
    #[default]
    None,
    /// Rice (1).
    Rice,
    /// JPEG (2).
    Jpeg,
    /// GIF (5).
    Gif,
    /// ZIP (10).
    Zip,
    /// Another value.
    Other(u8),
}

impl NoaaCompression {
    /// The compression from its byte.
    pub const fn from_byte(b: u8) -> Self {
        match b {
            0 => NoaaCompression::None,
            1 => NoaaCompression::Rice,
            2 => NoaaCompression::Jpeg,
            5 => NoaaCompression::Gif,
            10 => NoaaCompression::Zip,
            v => NoaaCompression::Other(v),
        }
    }
}

/// Image structure (record 1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageStructure {
    /// Bits per pixel.
    pub bits: u8,
    /// Columns.
    pub columns: u16,
    /// Lines.
    pub lines: u16,
    /// The compression flag.
    pub compression: Compression,
    /// FY-4 only: the extra fields of its record.
    pub fy4: Option<Fy4Image>,
}

/// The FY-4 fields of record 1.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fy4Image {
    /// Satellite name.
    pub satellite: String,
    /// Instrument.
    pub instrument: String,
    /// Channel.
    pub channel: u8,
    /// Total segments.
    pub total_segments: u16,
    /// Segment number.
    pub segment: u16,
    /// Segment position.
    pub position: u16,
    /// First line of the segment.
    pub first_line: u16,
    /// Algorithm, lossless and level byte.
    pub coding: u8,
}

/// Image navigation (record 2).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Navigation {
    /// The projection name ("GEOS(-75.0)").
    pub projection: String,
    /// Column scaling factor.
    pub cfac: f64,
    /// Line scaling factor.
    pub lfac: f64,
    /// Column offset.
    pub coff: f64,
    /// Line offset.
    pub loff: f64,
}

/// Segment identification (record 128).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Segment {
    /// GOES: image id.
    pub image_id: u16,
    /// Segment sequence number (from 1 on GOES, from 1 on GK-2A).
    pub sequence: u16,
    /// GOES: start column.
    pub start_column: u16,
    /// Start line.
    pub start_line: u16,
    /// Total segments.
    pub total: u16,
    /// GOES: maximum column.
    pub max_column: u16,
    /// GOES: maximum row.
    pub max_row: u16,
}

/// The NOAA product record (129).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoaaProduct {
    /// Product id.
    pub product: u16,
    /// Sub-product id.
    pub subproduct: u16,
    /// Parameter.
    pub parameter: u16,
    /// The compression.
    pub compression: NoaaCompression,
}

/// The Rice compression record (131, GOES). The flags are the SZIP option mask the GOES-R specification uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RiceRecord {
    /// Flags: [`RiceRecord::ALLOW_K13`], [`RiceRecord::MSB`], [`RiceRecord::NN`] and others.
    pub flags: u16,
    /// Pixels per block (`J`).
    pub pixels_per_block: u8,
    /// Scan lines per packet.
    pub lines_per_packet: u8,
}

impl RiceRecord {
    /// SZIP flag: allow k = 13.
    pub const ALLOW_K13: u16 = 1;
    /// SZIP flag: samples are most significant byte first.
    pub const MSB: u16 = 16;
    /// SZIP flag: the nearest-neighbour preprocessor (unit-delay predictor) is on.
    pub const NN: u16 = 32;

    /// The CCSDS 121.0 parameters for lines of `columns` samples of `bits` bits, one sample interval (RSI) per line (as GOES
    /// codes its images). If `pixels_per_block` is 0, the block is 16. If the values are not valid, it returns an error.
    pub fn params(&self, bits: u8, columns: u16) -> Result<rice::Params> {
        let block = if self.pixels_per_block == 0 {
            16
        } else {
            self.pixels_per_block as usize
        };
        let mut p = rice::Params::new(bits as u32, block, 1)?;
        p.rsi = (columns as usize).div_ceil(block).clamp(1, 4096);
        p.preprocess = self.flags & Self::NN != 0;
        Ok(p)
    }
}

/// The parsed headers of a file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Headers {
    /// The dialect they were read in.
    pub dialect: Dialect,
    /// The file type code (0 image, 1 GTS message, 2 alphanumeric text, 3 encryption key message, 130 DCS, 214 EMWIN).
    pub file_type: u8,
    /// Total header length, bytes.
    pub header_len: u32,
    /// Data field length, bits.
    pub data_bits: u64,
    /// Record 1.
    pub image: Option<ImageStructure>,
    /// Record 2.
    pub navigation: Option<Navigation>,
    /// Record 3, the image data function text.
    pub data_function: Option<String>,
    /// Record 4.
    pub annotation: Option<String>,
    /// Record 5: days since 1958-01-01 and milliseconds of the day.
    pub time: Option<(u16, u32)>,
    /// Record 6.
    pub ancillary: Option<String>,
    /// Record 7: the key number (GK-2A: low 16 bits, FY-4: 32 bits). 0 means not encrypted.
    pub key: Option<u32>,
    /// Record 128.
    pub segment: Option<Segment>,
    /// Record 129 (GOES).
    pub noaa: Option<NoaaProduct>,
    /// Record 130 text.
    pub header_structure: Option<String>,
    /// Record 131 (GOES).
    pub rice: Option<RiceRecord>,
    /// Record 131 (GK-2A): observation time text.
    pub observation_time: Option<String>,
}

impl Headers {
    /// The time stamp as Unix seconds. Day 4383 after 1958-01-01 is 1970-01-01.
    pub fn unix_time(&self) -> Option<f64> {
        self.time
            .map(|(d, ms)| (d as f64 - 4383.0) * 86_400.0 + ms as f64 / 1000.0)
    }

    /// The data field (after the headers) of `file`.
    pub fn data<'a>(&self, file: &'a [u8]) -> &'a [u8] {
        let h = (self.header_len as usize).min(file.len());
        &file[h..]
    }

    /// True for a file of images (type 0, or a file with an image structure).
    pub fn is_image(&self) -> bool {
        self.file_type == 0 || self.image.is_some()
    }
}

/// One header record: its type and body (after the 3-byte type and length).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record<'a> {
    /// The record type.
    pub kind: u8,
    /// The record body.
    pub body: &'a [u8],
}

/// The primary header fields: (file type, total header length, data field length in bits).
pub fn primary(file: &[u8]) -> Result<(u8, u32, u64)> {
    if file.len() < 16 {
        return Err(Error::InputTooShort { need: 16 });
    }
    if file[0] != 0 || be16(&file[1..]) != 16 {
        return Err(Error::BadField(HeaderField::XritPrimaryHeader));
    }
    let header_len = be32(&file[4..]);
    if header_len < 16 || header_len as usize > file.len() {
        return Err(Error::BadField(HeaderField::XritTotalLength));
    }
    let mut bits = [0u8; 8];
    bits.copy_from_slice(&file[8..16]);
    Ok((file[3], header_len, u64::from_be_bytes(bits)))
}

/// Walks the records after the primary header. A record that does not fit the header length ends the walk with an error item.
pub fn records(file: &[u8]) -> Result<impl Iterator<Item = Result<Record<'_>>>> {
    let (_, hl, _) = primary(file)?;
    let hl = hl as usize;
    let mut pos = 16;
    let mut failed = false;
    Ok(std::iter::from_fn(move || {
        if failed || pos >= hl {
            return None;
        }
        if pos + 3 > hl {
            failed = true;
            return Some(Err(Error::BadField(HeaderField::XritRecordPast)));
        }
        let kind = file[pos];
        let len = be16(&file[pos + 1..]) as usize;
        if len < 3 || pos + len > hl {
            failed = true;
            return Some(Err(Error::BadField(HeaderField::XritRecordLength)));
        }
        let body = &file[pos + 3..pos + len];
        pos += len;
        Some(Ok(Record { kind, body }))
    }))
}

fn text(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).trim_end().to_string()
}

/// Reads the headers of a file in `dialect`. If the primary header or a record is damaged, it returns an error.
pub fn parse(file: &[u8], dialect: Dialect) -> Result<Headers> {
    let (file_type, header_len, data_bits) = primary(file)?;
    let mut h = Headers {
        dialect,
        file_type,
        header_len,
        data_bits,
        ..Default::default()
    };
    for r in records(file)? {
        let r = r?;
        record(&mut h, r.kind, r.body, dialect);
    }
    Ok(h)
}

fn record(h: &mut Headers, t: u8, b: &[u8], dialect: Dialect) {
    match (t, dialect) {
        (1, Dialect::Fy4) if b.len() >= 32 => {
            h.image = Some(ImageStructure {
                bits: b[16],
                columns: be16(&b[17..]),
                lines: be16(&b[19..]),
                compression: Compression::from_byte(b[21]),
                fy4: Some(Fy4Image {
                    satellite: text(&b[0..9]),
                    instrument: text(&b[9..16]),
                    channel: b[22],
                    total_segments: be16(&b[23..]),
                    segment: be16(&b[25..]),
                    position: be16(&b[27..]),
                    first_line: be16(&b[29..]),
                    coding: b[31],
                }),
            })
        }
        (1, _) if b.len() >= 6 => {
            h.image = Some(ImageStructure {
                bits: b[0],
                columns: be16(&b[1..]),
                lines: be16(&b[3..]),
                compression: Compression::from_byte(b[5]),
                fy4: None,
            })
        }
        (2, _) if b.len() >= 48 => {
            let f = |o: usize| -> f64 {
                if dialect == Dialect::Fy4 {
                    f32::from_bits(be32(&b[o..])) as f64
                } else {
                    be32(&b[o..]) as i32 as f64
                }
            };
            h.navigation = Some(Navigation {
                projection: text(&b[..32]),
                cfac: f(32),
                lfac: f(36),
                coff: f(40),
                loff: f(44),
            })
        }
        (3, _) => h.data_function = Some(text(b)),
        (4, _) => h.annotation = Some(text(b)),
        // CDS time: a P-field byte, 2 bytes of days, 4 bytes of milliseconds.
        (5, _) if b.len() >= 7 => h.time = Some((be16(&b[1..]), be32(&b[3..]))),
        (6, _) => h.ancillary = Some(text(b)),
        (7, Dialect::Gk2a) if b.len() >= 4 => h.key = Some(be32(b) & 0xFFFF),
        (7, Dialect::Gk2a) if b.len() >= 2 => h.key = Some(be16(b) as u32),
        (7, Dialect::Fy4) if b.len() >= 4 => h.key = Some(be32(b)),
        (128, Dialect::Goes) if b.len() >= 14 => {
            h.segment = Some(Segment {
                image_id: be16(b),
                sequence: be16(&b[2..]),
                start_column: be16(&b[4..]),
                start_line: be16(&b[6..]),
                total: be16(&b[8..]),
                max_column: be16(&b[10..]),
                max_row: be16(&b[12..]),
            })
        }
        (128, Dialect::Gk2a) if b.len() >= 4 => {
            h.segment = Some(Segment {
                sequence: b[0] as u16,
                total: b[1] as u16,
                start_line: be16(&b[2..]),
                ..Default::default()
            })
        }
        (129, Dialect::Goes) if b.len() >= 11 && &b[..4] == b"NOAA" => {
            h.noaa = Some(NoaaProduct {
                product: be16(&b[4..]),
                subproduct: be16(&b[6..]),
                parameter: be16(&b[8..]),
                compression: NoaaCompression::from_byte(b[10]),
            })
        }
        (130, _) => h.header_structure = Some(text(b)),
        (131, Dialect::Goes) if b.len() >= 4 => {
            h.rice = Some(RiceRecord {
                flags: be16(b),
                pixels_per_block: b[2],
                lines_per_packet: b[3],
            })
        }
        (131, Dialect::Gk2a) => h.observation_time = Some(text(b)),
        _ => {}
    }
}

/// Writes file headers, for generators and tests.
#[derive(Clone, Debug, Default)]
pub struct Builder {
    file_type: u8,
    records: Vec<u8>,
}

impl Builder {
    /// Headers of a file of `file_type`.
    pub fn new(file_type: u8) -> Self {
        Builder {
            file_type,
            records: Vec::new(),
        }
    }

    /// Adds a record with this body. A body longer than 65,532 bytes is cut.
    pub fn record(mut self, t: u8, body: &[u8]) -> Self {
        let body = &body[..body.len().min(0xFFFF - 3)];
        self.records.push(t);
        self.records
            .extend_from_slice(&((body.len() + 3) as u16).to_be_bytes());
        self.records.extend_from_slice(body);
        self
    }

    /// Record 1 (GOES and GK-2A layout).
    pub fn image(self, bits: u8, columns: u16, lines: u16, compression: u8) -> Self {
        let mut b = vec![bits];
        b.extend_from_slice(&columns.to_be_bytes());
        b.extend_from_slice(&lines.to_be_bytes());
        b.push(compression);
        self.record(1, &b)
    }

    /// Record 2, with 32-bit integer factors (or floats when `floats`, as FY-4 sends them).
    pub fn navigation(self, projection: &str, f: [f64; 4], floats: bool) -> Self {
        let mut b = vec![b' '; 32];
        let n = projection.len().min(32);
        b[..n].copy_from_slice(&projection.as_bytes()[..n]);
        for v in f {
            let w = if floats {
                (v as f32).to_bits()
            } else {
                v as i32 as u32
            };
            b.extend_from_slice(&w.to_be_bytes());
        }
        self.record(2, &b)
    }

    /// Record 4.
    pub fn annotation(self, name: &str) -> Self {
        self.record(4, name.as_bytes())
    }

    /// Record 5 from Unix seconds (a CDS P-field of 0x40, then days since 1958-01-01 and milliseconds).
    pub fn time_unix(self, unix: f64) -> Self {
        let days = (unix / 86_400.0).floor();
        let ms = ((unix - days * 86_400.0) * 1000.0).round() as u32;
        let d = (days as i64 + 4383) as u16;
        let mut b = vec![0x40];
        b.extend_from_slice(&d.to_be_bytes());
        b.extend_from_slice(&ms.to_be_bytes());
        self.record(5, &b)
    }

    /// Record 131 (GOES Rice).
    pub fn rice(self, r: &RiceRecord) -> Self {
        let mut b = r.flags.to_be_bytes().to_vec();
        b.push(r.pixels_per_block);
        b.push(r.lines_per_packet);
        self.record(131, &b)
    }

    /// The primary header and the records, for a data field of `data_len` bytes.
    pub fn build(&self, data_len: usize) -> Vec<u8> {
        let total = 16 + self.records.len();
        let mut out = vec![0u8, 0, 16, self.file_type];
        out.extend_from_slice(&(total as u32).to_be_bytes());
        out.extend_from_slice(&((data_len as u64) * 8).to_be_bytes());
        out.extend_from_slice(&self.records);
        out
    }
}
