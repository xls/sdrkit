//! MOT, the Multimedia Object Transfer protocol: object headers and their parameters, segments, the directory, and the
//! [`Assembler`] that returns each complete object once, in header mode and in directory mode. DAB and DRM use it for the
//! slideshow, the broadcast website and SPI.
//!
//! Standard: ETSI EN 301 234 clauses 5 to 7.

use crate::datagroup::{self, DataGroup};
use crate::{Error, Result};
use crate::{Malformed, Parameter};
use sdrkit_util_text::charset_field::{self as text, Charset};

/// Parameter ids of the header extension (EN 301 234 clause 6.2 and TS 101 499 clause 6).
pub mod param {
    /// ExpireTime (EN 301 234).
    pub const EXPIRE_TIME: u8 = 0x04;
    /// TriggerTime (EN 301 234).
    pub const TRIGGER_TIME: u8 = 0x05;
    /// ContentName: character set byte, then the name (EN 301 234).
    pub const CONTENT_NAME: u8 = 0x0C;
    /// MimeType (EN 301 234).
    pub const MIME_TYPE: u8 = 0x10;
    /// CompressionType (EN 301 234).
    pub const COMPRESSION_TYPE: u8 = 0x11;
    /// CategoryID/SlideID (TS 101 499).
    pub const CATEGORY_SLIDE: u8 = 0x25;
    /// CategoryTitle (TS 101 499).
    pub const CATEGORY_TITLE: u8 = 0x26;
    /// ClickThroughURL (TS 101 499).
    pub const CLICK_THROUGH_URL: u8 = 0x27;
    /// AlternativeLocationURL (TS 101 499).
    pub const ALTERNATIVE_LOCATION_URL: u8 = 0x28;
    /// Alert (TS 101 499).
    pub const ALERT: u8 = 0x29;
}

/// A MOT time (EN 301 234 clause 6.2.4.1): validity flag (1), MJD (17), Rfu (2), UTC flag (1), hours (5), minutes (6), and
/// with the UTC flag seconds (6) and milliseconds (10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotTime {
    /// The validity flag is 0: "now".
    Now,
    /// A UTC time.
    At {
        /// Seconds since 1970-01-01 00:00 UTC.
        unix_s: i64,
        /// Milliseconds (0 in the short form).
        ms: u16,
    },
}

impl MotTime {
    /// Reads a MOT time of 4 or 6 bytes.
    pub fn parse(b: &[u8]) -> Result<MotTime> {
        if b.len() < 4 {
            return Err(Error::Truncated);
        }
        let v = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        if v >> 31 == 0 {
            return Ok(MotTime::Now);
        }
        let mjd = ((v >> 14) & 0x1_FFFF) as i64;
        let long = v & 0x800 != 0;
        let h = ((v >> 6) & 31) as i64;
        let m = (v & 63) as i64;
        if h > 23 || m > 59 {
            return Err(Error::Invalid(Malformed::MotTime));
        }
        let (s, ms) = if long {
            if b.len() < 6 {
                return Err(Error::Truncated);
            }
            let w = u16::from_be_bytes([b[4], b[5]]);
            ((w >> 10) as i64, w & 0x3FF)
        } else {
            (0, 0)
        };
        Ok(MotTime::At {
            unix_s: (mjd - 40_587) * 86_400 + h * 3600 + m * 60 + s,
            ms,
        })
    }

    /// Appends the time: 4 bytes for "now" and for a whole minute, else the 6-byte long form.
    pub fn write(&self, out: &mut Vec<u8>) {
        match *self {
            MotTime::Now => out.extend_from_slice(&[0; 4]),
            MotTime::At { unix_s, ms } => {
                let mjd = (unix_s.div_euclid(86_400) + 40_587) as u32;
                let tod = unix_s.rem_euclid(86_400) as u32;
                let long = !tod.is_multiple_of(60) || ms != 0;
                let v = (1 << 31)
                    | ((mjd & 0x1_FFFF) << 14)
                    | ((long as u32) << 11)
                    | ((tod / 3600) << 6)
                    | ((tod / 60) % 60);
                out.extend_from_slice(&v.to_be_bytes());
                if long {
                    out.extend_from_slice(
                        &((((tod % 60) as u16) << 10) | (ms & 0x3FF)).to_be_bytes(),
                    );
                }
            }
        }
    }
}

/// One header extension parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Param<'a> {
    /// ParamId (0 to 63).
    pub id: u8,
    /// Its data field.
    pub data: &'a [u8],
}

/// Reads one parameter at the start of `b`. It returns the parameter and its length in bytes.
fn read_param(b: &[u8]) -> Result<(Param<'_>, usize)> {
    let first = *b.first().ok_or(Error::Truncated)?;
    let id = first & 63;
    let (len, at) = match first >> 6 {
        0 => (0, 1),
        1 => (1, 1),
        2 => (4, 1),
        _ => {
            let l0 = *b.get(1).ok_or(Error::Truncated)?;
            if l0 & 0x80 != 0 {
                let l1 = *b.get(2).ok_or(Error::Truncated)?;
                ((((l0 & 0x7F) as usize) << 8) | l1 as usize, 3)
            } else {
                (l0 as usize, 2)
            }
        }
    };
    let data = b.get(at..at + len).ok_or(Error::Truncated)?;
    Ok((Param { id, data }, at + len))
}

/// The parameters of a header extension, in order.
#[derive(Clone, Debug)]
pub struct Params<'a> {
    rest: &'a [u8],
}

impl<'a> Iterator for Params<'a> {
    type Item = Param<'a>;
    fn next(&mut self) -> Option<Param<'a>> {
        let (p, n) = read_param(self.rest).ok()?;
        self.rest = &self.rest[n..];
        Some(p)
    }
}

/// A MOT header (core and extension), borrowed from its bytes. [`Header::parse`] reads every parameter once, so the accessors
/// cannot fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header<'a> {
    /// BodySize in bytes.
    pub body_size: u32,
    /// HeaderSize in bytes (core and extension).
    pub header_size: u16,
    /// ContentType (6 bits).
    pub content_type: u8,
    /// ContentSubType (9 bits).
    pub content_subtype: u16,
    /// The header bytes (core and extension).
    pub bytes: &'a [u8],
}

impl<'a> Header<'a> {
    /// Reads a header from the start of `b`. `b` must hold at least HeaderSize bytes. The extension must be whole parameters.
    pub fn parse(b: &'a [u8]) -> Result<Header<'a>> {
        if b.len() < 7 {
            return Err(Error::Truncated);
        }
        let core = u64::from_be_bytes([0, b[0], b[1], b[2], b[3], b[4], b[5], b[6]]);
        let header_size = ((core >> 15) & 0x1FFF) as u16;
        if header_size < 7 {
            return Err(Error::Invalid(Malformed::HeaderSize));
        }
        let bytes = b.get(..header_size as usize).ok_or(Error::Truncated)?;
        let mut rest = &bytes[7..];
        while !rest.is_empty() {
            let (_, n) = read_param(rest)?;
            rest = &rest[n..];
        }
        Ok(Header {
            body_size: (core >> 28) as u32,
            header_size,
            content_type: ((core >> 9) & 63) as u8,
            content_subtype: (core & 0x1FF) as u16,
            bytes,
        })
    }

    /// Every parameter of the extension.
    pub fn params(&self) -> Params<'a> {
        Params {
            rest: &self.bytes[7..],
        }
    }

    /// The data of the first parameter with `id`.
    pub fn param(&self, id: u8) -> Option<&'a [u8]> {
        self.params().find(|p| p.id == id).map(|p| p.data)
    }

    /// The ContentName: its character set and its bytes.
    pub fn content_name(&self) -> Option<(Charset, &'a [u8])> {
        let d = self.param(param::CONTENT_NAME)?;
        let (&cs, name) = d.split_first()?;
        Some((Charset::from_high_nibble(cs), name))
    }

    /// The ContentName as text (empty without one). This allocates; use [`Header::content_name`] in a hot path.
    pub fn content_name_text(&self) -> String {
        let mut s = String::new();
        if let Some((cs, b)) = self.content_name() {
            text::display_into(b, cs, &mut s);
        }
        s
    }

    /// The TriggerTime.
    pub fn trigger_time(&self) -> Option<MotTime> {
        self.param(param::TRIGGER_TIME)
            .and_then(|d| MotTime::parse(d).ok())
    }

    /// The ExpireTime.
    pub fn expire_time(&self) -> Option<MotTime> {
        self.param(param::EXPIRE_TIME)
            .and_then(|d| MotTime::parse(d).ok())
    }

    /// The MimeType text.
    pub fn mime_type(&self) -> Option<&'a str> {
        self.param(param::MIME_TYPE)
            .and_then(|d| std::str::from_utf8(d).ok())
    }

    /// The CompressionType (1 is gzip).
    pub fn compression_type(&self) -> Option<u8> {
        self.param(param::COMPRESSION_TYPE)
            .and_then(|d| d.first().copied())
    }

    /// The content type as a MIME type, from MimeType if present, else from ContentType and ContentSubType (TS 101 756
    /// Table 17, text and image types only).
    pub fn mime(&self) -> Option<&'a str> {
        if let Some(m) = self.mime_type() {
            return Some(m);
        }
        content_mime(self.content_type, self.content_subtype)
    }
}

/// The MIME type of a ContentType and ContentSubType (TS 101 756 Table 17): text 0 to 2 and image 0 to 3.
pub const fn content_mime(content_type: u8, subtype: u16) -> Option<&'static str> {
    Some(match (content_type, subtype) {
        (1, 0) => "text/plain",
        (1, 1) => "text/plain;charset=iso-8859-1",
        (1, 2) => "text/html",
        (2, 0) => "image/gif",
        (2, 1) => "image/jpeg",
        (2, 2) => "image/bmp",
        (2, 3) => "image/png",
        _ => return None,
    })
}

/// Appends one parameter with the shortest PLI for its length.
pub fn write_param(id: u8, data: &[u8], out: &mut Vec<u8>) -> Result<()> {
    if id > 63 || data.len() > 0x7FFF {
        return Err(Error::BadParameter(Parameter::MotParameter));
    }
    match data.len() {
        0 => out.push(id),
        1 => out.push(0x40 | id),
        4 => out.push(0x80 | id),
        n if n < 128 => out.extend_from_slice(&[0xC0 | id, n as u8]),
        n => out.extend_from_slice(&[0xC0 | id, 0x80 | (n >> 8) as u8, n as u8]),
    }
    out.extend_from_slice(data);
    Ok(())
}

/// Builds a MOT header. The parameters are written in the order they are added.
#[derive(Clone, Debug)]
pub struct HeaderWriter {
    body_size: u32,
    content_type: u8,
    subtype: u16,
    params: Vec<u8>,
    error: Option<Error>,
}

impl HeaderWriter {
    /// A header for a body of `body_size` bytes and the given content type and subtype.
    pub fn new(body_size: u32, content_type: u8, subtype: u16) -> Self {
        HeaderWriter {
            body_size,
            content_type,
            subtype,
            params: Vec::new(),
            error: None,
        }
    }

    /// Adds a parameter.
    pub fn param(mut self, id: u8, data: &[u8]) -> Self {
        if let Err(e) = write_param(id, data, &mut self.params) {
            self.error.get_or_insert(e);
        }
        self
    }

    /// Adds ContentName in UTF-8.
    pub fn content_name(self, name: &str) -> Self {
        let mut d = Vec::with_capacity(name.len() + 1);
        d.push(Charset::Utf8.code() << 4);
        d.extend_from_slice(name.as_bytes());
        self.param(param::CONTENT_NAME, &d)
    }

    /// Adds TriggerTime.
    pub fn trigger_time(self, t: MotTime) -> Self {
        let mut d = Vec::with_capacity(6);
        t.write(&mut d);
        self.param(param::TRIGGER_TIME, &d)
    }

    /// Appends the header (core and extension).
    pub fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        if let Some(e) = self.error {
            return Err(e);
        }
        let size = 7 + self.params.len();
        if size > 0x1FFF
            || self.body_size > 0x0FFF_FFFF
            || self.content_type > 63
            || self.subtype > 0x1FF
        {
            return Err(Error::BadParameter(Parameter::HeaderCoreField));
        }
        let core = ((self.body_size as u64) << 28)
            | ((size as u64) << 15)
            | ((self.content_type as u64) << 9)
            | self.subtype as u64;
        out.extend_from_slice(&core.to_be_bytes()[1..]);
        out.extend_from_slice(&self.params);
        Ok(())
    }
}

/// Appends one segment: the segmentation header (repetition count and size) and the data (at most 8191 bytes).
pub fn write_segment(repetition: u8, data: &[u8], out: &mut Vec<u8>) -> Result<()> {
    if data.len() > 0x1FFF || repetition > 7 {
        return Err(Error::BadParameter(Parameter::SegmentHeader));
    }
    out.extend_from_slice(&(((repetition as u16) << 13) | data.len() as u16).to_be_bytes());
    out.extend_from_slice(data);
    Ok(())
}

/// Reads one segment: the repetition count and the segment data.
pub fn parse_segment(b: &[u8]) -> Result<(u8, &[u8])> {
    if b.len() < 2 {
        return Err(Error::Truncated);
    }
    let v = u16::from_be_bytes([b[0], b[1]]);
    let data = b
        .get(2..2 + (v & 0x1FFF) as usize)
        .ok_or(Error::Truncated)?;
    Ok(((v >> 13) as u8, data))
}

/// A MOT directory (uncompressed), borrowed from its bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Directory<'a> {
    /// NumberOfObjects.
    pub objects: u16,
    /// DataCarouselPeriod in tenths of a second (0: not given).
    pub carousel_period: u32,
    /// SegmentSize.
    pub segment_size: u16,
    /// The DirectoryExtension (directory parameters).
    pub extension: &'a [u8],
    entries: &'a [u8],
}

impl<'a> Directory<'a> {
    /// Reads a directory and every entry header once.
    pub fn parse(b: &'a [u8]) -> Result<Directory<'a>> {
        if b.len() < 13 {
            return Err(Error::Truncated);
        }
        if b[0] & 0x80 != 0 {
            return Err(Error::Invalid(Malformed::CompressedDirectory));
        }
        let size = (u32::from_be_bytes([b[0], b[1], b[2], b[3]]) & 0x3FFF_FFFF) as usize;
        let b = b.get(..size).ok_or(Error::Truncated)?;
        if size < 13 {
            return Err(Error::Invalid(Malformed::DirectorySize));
        }
        let ext_len = u16::from_be_bytes([b[11], b[12]]) as usize;
        let extension = b.get(13..13 + ext_len).ok_or(Error::Truncated)?;
        let d = Directory {
            objects: u16::from_be_bytes([b[4], b[5]]),
            carousel_period: u32::from_be_bytes([0, b[6], b[7], b[8]]),
            segment_size: u16::from_be_bytes([b[9], b[10]]) & 0x1FFF,
            extension,
            entries: &b[13 + ext_len..],
        };
        let mut n = 0;
        let mut rest = d.entries;
        while !rest.is_empty() {
            let h = Header::parse(rest.get(2..).ok_or(Error::Truncated)?)?;
            rest = &rest[2 + h.header_size as usize..];
            n += 1;
        }
        if n != d.objects {
            return Err(Error::Invalid(Malformed::NumberOfObjects));
        }
        Ok(d)
    }

    /// The entries: transport id and header.
    pub fn entries(&self) -> impl Iterator<Item = (u16, Header<'a>)> + 'a {
        let mut rest = self.entries;
        std::iter::from_fn(move || {
            let tid = u16::from_be_bytes([*rest.first()?, *rest.get(1)?]);
            let h = Header::parse(&rest[2..]).ok()?;
            rest = &rest[2 + h.header_size as usize..];
            Some((tid, h))
        })
    }

    /// The header of one transport id.
    pub fn header(&self, transport_id: u16) -> Option<Header<'a>> {
        self.entries().find(|e| e.0 == transport_id).map(|e| e.1)
    }
}

/// Appends a directory of `(transport id, header bytes)` entries, with no directory extension.
pub fn write_directory(
    entries: &[(u16, &[u8])],
    carousel_period: u32,
    segment_size: u16,
    out: &mut Vec<u8>,
) -> Result<()> {
    let size = 13 + entries.iter().map(|e| 2 + e.1.len()).sum::<usize>();
    if size > 0x3FFF_FFFF
        || entries.len() > 0xFFFF
        || carousel_period > 0xFF_FFFF
        || segment_size > 0x1FFF
    {
        return Err(Error::BadParameter(Parameter::DirectoryField));
    }
    out.extend_from_slice(&(size as u32).to_be_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_be_bytes());
    out.extend_from_slice(&carousel_period.to_be_bytes()[1..]);
    out.extend_from_slice(&segment_size.to_be_bytes());
    out.extend_from_slice(&[0, 0]);
    for (tid, h) in entries {
        out.extend_from_slice(&tid.to_be_bytes());
        out.extend_from_slice(h);
    }
    Ok(())
}

/// A complete object, given to the callback of [`Assembler::push`].
#[derive(Clone, Copy, Debug)]
pub struct Object<'a> {
    /// The transport id.
    pub transport_id: u16,
    /// The header (from the object's own header in header mode, from the directory in directory mode).
    pub header: Header<'a>,
    /// The body.
    pub body: &'a [u8],
}

/// The size limits of an [`Assembler`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Objects collected at the same time.
    pub objects: usize,
    /// The largest body in bytes.
    pub body: usize,
    /// The largest directory in bytes.
    pub directory: usize,
}

impl Default for Limits {
    /// 16 objects, 1 MiB bodies, a 4 MiB directory.
    fn default() -> Self {
        Limits {
            objects: 16,
            body: 1 << 20,
            directory: 4 << 20,
        }
    }
}

/// The counters of an [`Assembler`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Objects given to the callback.
    pub objects: u64,
    /// Complete objects not given again because nothing changed.
    pub repeats: u64,
    /// Scrambled bodies (data group type 5) ignored.
    pub scrambled: u64,
    /// Compressed directories (data group type 7) ignored.
    pub compressed_directories: u64,
    /// Objects dropped: too large, a size that differs from BodySize, a bad header, or pushed out by newer objects.
    pub dropped: u64,
    /// Data groups without a transport id or with a bad segment.
    pub bad_groups: u64,
}

/// The segments of one part (header, body or directory), in one buffer.
#[derive(Clone, Debug, Default)]
struct Parts {
    data: Vec<u8>,
    index: Vec<(u16, u32, u16)>,
    last: Option<u16>,
}

/// What [`Parts::insert`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Insert {
    New,
    Same,
    Different,
}

impl Parts {
    fn clear(&mut self) {
        self.data.clear();
        self.index.clear();
        self.last = None;
    }

    fn insert(&mut self, seg: u16, last: bool, d: &[u8]) -> Insert {
        if let Some(&(_, off, len)) = self.index.iter().find(|e| e.0 == seg) {
            let old = &self.data[off as usize..off as usize + len as usize];
            return if old == d {
                Insert::Same
            } else {
                Insert::Different
            };
        }
        self.index
            .push((seg, self.data.len() as u32, d.len() as u16));
        self.data.extend_from_slice(d);
        if last {
            self.last = Some(seg);
        }
        Insert::New
    }

    fn complete(&self) -> bool {
        self.last.is_some_and(|l| {
            self.index.len() == l as usize + 1 && self.index.iter().all(|e| e.0 <= l)
        })
    }

    /// Joins the segments in order into `out` (cleared first).
    fn join(&mut self, out: &mut Vec<u8>) {
        self.index.sort_unstable_by_key(|e| e.0);
        out.clear();
        for &(_, off, len) in &self.index {
            out.extend_from_slice(&self.data[off as usize..off as usize + len as usize]);
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Slot {
    tid: u16,
    used: u64,
    active: bool,
    header: Parts,
    body: Parts,
    /// The fingerprint of what was last given, 0 when nothing was.
    given: u64,
}

/// FNV-1a 64 over the header and body: enough to tell a changed object from a repeat.
fn fingerprint(header: &[u8], body: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in header.iter().chain(body) {
        h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01B3);
    }
    h | 1
}

/// Collects MOT objects from data groups, in header mode and directory mode (EN 301 234 clauses 5 and 7).
///
/// ```
/// use sdrkit_proto_bcdata::datagroup::{self, Header as Dg};
/// use sdrkit_proto_bcdata::mot::{self, Assembler, HeaderWriter};
///
/// let body = b"\x89PNG...";
/// let mut h = Vec::new();
/// HeaderWriter::new(body.len() as u32, 2, 3).content_name("slide.png").write(&mut h).unwrap();
/// let mut groups = Vec::new();
/// for (t, part) in [(datagroup::MOT_HEADER, &h[..]), (datagroup::MOT_BODY, &body[..])] {
///     let mut seg = Vec::new();
///     mot::write_segment(0, part, &mut seg).unwrap();
///     let mut g = Vec::new();
///     Dg { group_type: t, segment: Some((true, 0)), transport_id: Some(7), ..Dg::default() }.write(&seg, &mut g).unwrap();
///     groups.push(g);
/// }
/// let mut a = Assembler::new();
/// let mut names = Vec::new();
/// for g in &groups {
///     a.push(&datagroup::parse(g).unwrap(), &mut |o| names.push(o.header.content_name_text()));
/// }
/// assert_eq!(names, vec!["slide.png".to_string()]);
/// ```
#[derive(Clone, Debug)]
pub struct Assembler {
    limits: Limits,
    slots: Vec<Slot>,
    clock: u64,
    dir: Parts,
    dir_tid: Option<u16>,
    directory: Vec<u8>,
    header_buf: Vec<u8>,
    body_buf: Vec<u8>,
    /// The counters.
    pub stats: Stats,
}

impl Default for Assembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Assembler {
    /// An assembler with the default [`Limits`].
    pub fn new() -> Self {
        Self::with_limits(Limits::default())
    }

    /// An assembler with the given limits.
    pub fn with_limits(limits: Limits) -> Self {
        Assembler {
            limits,
            slots: vec![Slot::default(); limits.objects.max(1)],
            clock: 0,
            dir: Parts::default(),
            dir_tid: None,
            directory: Vec::new(),
            header_buf: Vec::new(),
            body_buf: Vec::new(),
            stats: Stats::default(),
        }
    }

    /// Forgets every object and the directory. The counters stay.
    pub fn reset(&mut self) {
        for s in &mut self.slots {
            *s = Slot {
                header: std::mem::take(&mut s.header),
                body: std::mem::take(&mut s.body),
                ..Slot::default()
            };
            s.header.clear();
            s.body.clear();
        }
        self.dir.clear();
        self.dir_tid = None;
        self.directory.clear();
    }

    /// The current directory (directory mode), if one is complete.
    pub fn directory(&self) -> Option<Directory<'_>> {
        Directory::parse(&self.directory).ok()
    }

    fn slot(&mut self, tid: u16) -> usize {
        self.clock += 1;
        if let Some(i) = self.slots.iter().position(|s| s.active && s.tid == tid) {
            self.slots[i].used = self.clock;
            return i;
        }
        let i = match self.slots.iter().position(|s| !s.active) {
            Some(i) => i,
            None => {
                // Push out the least recently used object.
                let i = (0..self.slots.len())
                    .min_by_key(|&i| self.slots[i].used)
                    .unwrap_or(0);
                if self.slots[i].given == 0 {
                    self.stats.dropped += 1;
                }
                i
            }
        };
        let s = &mut self.slots[i];
        s.tid = tid;
        s.used = self.clock;
        s.active = true;
        s.given = 0;
        s.header.clear();
        s.body.clear();
        i
    }

    /// Takes one data group. Each object that completes (or completes again with a change) goes to `out`.
    pub fn push(&mut self, g: &DataGroup<'_>, out: &mut dyn FnMut(Object<'_>)) {
        let t = g.header.group_type;
        if !matches!(
            t,
            datagroup::MOT_HEADER
                | datagroup::MOT_BODY
                | datagroup::MOT_BODY_SCRAMBLED
                | datagroup::MOT_DIRECTORY
                | datagroup::MOT_DIRECTORY_COMPRESSED
        ) {
            return;
        }
        let (Some(tid), Ok((_, seg))) = (g.header.transport_id, parse_segment(g.data)) else {
            self.stats.bad_groups += 1;
            return;
        };
        let n = g.segment_number();
        match t {
            datagroup::MOT_BODY_SCRAMBLED => self.stats.scrambled += 1,
            datagroup::MOT_DIRECTORY_COMPRESSED => self.stats.compressed_directories += 1,
            datagroup::MOT_DIRECTORY => {
                if self.dir_tid != Some(tid) {
                    self.dir.clear();
                    self.dir_tid = Some(tid);
                }
                if self.dir.insert(n, g.is_last(), seg) == Insert::Different {
                    self.dir.clear();
                    self.dir.insert(n, g.is_last(), seg);
                }
                if self.dir.data.len() > self.limits.directory {
                    self.dir.clear();
                    self.stats.dropped += 1;
                    return;
                }
                if self.dir.complete() {
                    let mut d = std::mem::take(&mut self.directory);
                    self.dir.join(&mut d);
                    self.dir.clear();
                    self.directory = d;
                    if Directory::parse(&self.directory).is_err() {
                        self.directory.clear();
                        self.stats.dropped += 1;
                        return;
                    }
                    // Bodies that waited for their header can complete now.
                    for i in 0..self.slots.len() {
                        if self.slots[i].active {
                            self.try_complete(i, out);
                        }
                    }
                }
            }
            _ => {
                let i = self.slot(tid);
                let s = &mut self.slots[i];
                let part = if t == datagroup::MOT_HEADER {
                    &mut s.header
                } else {
                    &mut s.body
                };
                match part.insert(n, g.is_last(), seg) {
                    Insert::Same => {
                        if s.given != 0 && part.complete() {
                            self.stats.repeats += 1;
                        }
                        return;
                    }
                    Insert::Different => {
                        // A new version of the object: start again from this segment.
                        part.clear();
                        part.insert(n, g.is_last(), seg);
                        if t == datagroup::MOT_HEADER {
                            s.body.clear();
                        }
                    }
                    Insert::New => {}
                }
                if s.body.data.len() > self.limits.body {
                    s.active = false;
                    self.stats.dropped += 1;
                    return;
                }
                self.try_complete(i, out);
            }
        }
    }

    fn try_complete(&mut self, i: usize, out: &mut dyn FnMut(Object<'_>)) {
        let s = &mut self.slots[i];
        if !s.body.complete() {
            return;
        }
        let tid = s.tid;
        let mut hb = std::mem::take(&mut self.header_buf);
        if s.header.complete() {
            s.header.join(&mut hb);
        } else if let Some(h) = Directory::parse(&self.directory)
            .ok()
            .and_then(|d| d.header(tid))
        {
            hb.clear();
            hb.extend_from_slice(h.bytes);
        } else {
            self.header_buf = hb;
            return;
        }
        let mut bb = std::mem::take(&mut self.body_buf);
        s.body.join(&mut bb);
        match Header::parse(&hb) {
            Ok(h) if h.body_size as usize == bb.len() => {
                let f = fingerprint(&hb, &bb);
                if s.given == f {
                    self.stats.repeats += 1;
                } else {
                    s.given = f;
                    self.stats.objects += 1;
                    out(Object {
                        transport_id: tid,
                        header: h,
                        body: &bb,
                    });
                }
                // Keep the header (to tell a new version), drop the body segments: a repeat collects them again.
                s.body.clear();
            }
            _ => {
                s.active = false;
                self.stats.dropped += 1;
            }
        }
        self.header_buf = hb;
        self.body_buf = bb;
    }
}
