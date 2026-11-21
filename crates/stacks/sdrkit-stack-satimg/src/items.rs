//! The stream items of the stacks: CADUs, AOS frames, space packets, xRIT files, products, texts and GGAK frames. Products,
//! texts and GGAK frames leave the graph as the events of [`crate::typed`]. Pictures leave as the strip and picture items of
//! `sdrkit-codec-image`.

use sdrkit_algo_fec::rs::Basis;
use crate::typed::{GgakFrameEvent, ProductEvent, TextEvent, GGAK_EVENT, PRODUCT_EVENT, TEXT_EVENT};
use sdrkit_core::interface::Fields as _;
use sdrkit_core::publish::keys::Family;
use sdrkit_core::{Choice, EventDef, Record};
use sdrkit_graph::Item;
use sdrkit_proto_ccsds::xrit::headers::Headers;
use std::sync::Arc;

/// Bytes in a CADU (sync word included).
pub const CADU_LEN: usize = 1024;

/// One channel access data unit: the sync word and the 1020 bytes after it, derandomised and (where the link has it) Reed-Solomon
/// corrected.
#[derive(Clone, Debug, PartialEq)]
pub struct Cadu {
    /// The sync word then the frame bytes.
    pub bytes: [u8; CADU_LEN],
    /// Reed-Solomon symbols corrected, `None` when uncorrectable (or no Reed-Solomon on this link: see `checked`).
    pub corrected: Option<u16>,
    /// Reed-Solomon was applied.
    pub checked: bool,
    /// The basis used.
    pub basis: Basis,
    /// Signal time of its first bit, seconds.
    pub time: f64,
}

impl Cadu {
    /// Whether it goes on to the upper layers (it passed Reed-Solomon, or the link has none).
    pub fn good(&self) -> bool {
        !self.checked || self.corrected.is_some()
    }
    /// The transfer frame (VCDU) bytes: after the sync word, without the Reed-Solomon check symbols when the link has them.
    pub fn vcdu(&self) -> &[u8] {
        if self.checked {
            &self.bytes[4..4 + 892]
        } else {
            &self.bytes[4..]
        }
    }
}

impl Item for Cadu {
    const KIND: &'static str = "satimg.cadu";
}

/// Bytes of the largest VCDU data zone (FY-4: 1012).
pub const MAX_ZONE: usize = 1012;

/// One corrected transfer frame (CCSDS 732.0 AOS VCDU).
#[derive(Clone, Debug, PartialEq)]
pub struct Vcdu {
    /// The version (1 for AOS).
    pub version: u8,
    /// Spacecraft id.
    pub spacecraft: u8,
    /// Virtual channel id.
    pub vcid: u8,
    /// The 24-bit counter.
    pub counter: u32,
    /// The M_PDU first header pointer (2047: no packet starts in this zone).
    pub first_header: u16,
    /// The data zone (M_PDU packet zone), `zone_len` bytes used.
    pub zone: [u8; MAX_ZONE],
    /// Bytes in `zone`.
    pub zone_len: u16,
    /// Signal time.
    pub time: f64,
}

impl Vcdu {
    /// The data zone.
    pub fn data(&self) -> &[u8] {
        &self.zone[..self.zone_len as usize]
    }
}

impl Item for Vcdu {
    const KIND: &'static str = "satimg.vcdu";
}

/// One space packet (CCSDS 133.0).
#[derive(Clone, Debug, PartialEq)]
pub struct SpacePacket {
    /// The virtual channel it came on.
    pub vcid: u8,
    /// The spacecraft of the VCDU.
    pub spacecraft: u8,
    /// APID (11 bits).
    pub apid: u16,
    /// Sequence flags: 1 first, 0 continuation, 2 last, 3 unsegmented.
    pub flags: u8,
    /// 14-bit sequence count.
    pub seq: u16,
    /// The user data (after the 6-byte primary header).
    pub data: Arc<[u8]>,
    /// CRC result where the mission has one.
    pub crc_ok: Option<bool>,
    /// A gap preceded it (a lost VCDU or a sequence jump).
    pub gap_before: bool,
    /// Signal time.
    pub time: f64,
}

impl Item for SpacePacket {
    const KIND: &'static str = "ccsds.packet";
}

/// One whole space packet, primary header included, as a frame layer gives it out.
#[derive(Clone, Debug, PartialEq)]
pub struct WholePacket {
    /// The virtual channel it came on.
    pub vcid: u8,
    /// The spacecraft of the frame.
    pub spacecraft: u8,
    /// The packet: the 6-byte primary header, then the rest.
    pub bytes: Arc<[u8]>,
    /// Frames were lost on this channel before it.
    pub gap_before: bool,
    /// Signal time.
    pub time: f64,
}

impl WholePacket {
    /// The APID.
    pub fn apid(&self) -> u16 {
        u16::from_be_bytes([self.bytes[0], self.bytes[1]]) & 0x7FF
    }
    /// The sequence flags: 1 first, 0 continuation, 2 last, 3 unsegmented.
    pub fn flags(&self) -> u8 {
        self.bytes[2] >> 6
    }
    /// The 14-bit sequence count.
    pub fn seq(&self) -> u16 {
        u16::from_be_bytes([self.bytes[2], self.bytes[3]]) & 0x3FFF
    }
}

impl Item for WholePacket {
    const KIND: &'static str = "ccsds.whole_packet";
}

sdrkit_core::choice_enum! {
    /// What a file of a broadcast is, for the host that stores it.
    pub enum FileKind {
        /// Complete, not otherwise placed.
        Complete = ("complete", "Complete"),
        /// Parts are missing.
        Incomplete = ("incomplete", "Incomplete"),
        /// An HRIT segment of a picture.
        HritSegment = ("hrit_segment", "HRIT segment"),
        /// Other data of the broadcast.
        Additional = ("additional", "Additional data"),
        /// Its bzip2 data is damaged; the bytes are as they came.
        BadBzip2 = ("bad_bzip2", "Bad bzip2"),
        /// A generic payload of GOES-R Rebroadcast.
        GrbGeneric = ("grb_generic", "GRB generic payload"),
    }
}

/// A file that a stack gives to the host to store: the `satimg.file` item. The stack never writes files itself.
#[derive(Clone, Debug, PartialEq)]
pub struct SatFile {
    /// The name, cleaned for storing.
    pub name: String,
    /// What it is.
    pub kind: FileKind,
    /// The bytes.
    pub bytes: Arc<[u8]>,
    /// Parts it was sent in (0 when not sent in parts).
    pub parts: u32,
    /// Parts received.
    pub received: u32,
    /// All of it came.
    pub complete: bool,
    /// Signal time it was finished.
    pub time: f64,
}

impl Item for SatFile {
    const KIND: &'static str = "satimg.file";
}

/// One LRIT / HRIT file.
#[derive(Clone, Debug, PartialEq)]
pub struct XritFile {
    /// Virtual channel.
    pub vcid: u8,
    /// APID.
    pub apid: u16,
    /// The file (headers and data field), as it was sent.
    pub bytes: Arc<[u8]>,
    /// Where each packet's user data starts in `bytes` (Rice images are one compressed line per packet).
    pub packet_starts: Vec<u32>,
    /// Each packet's place in the file counting lost ones (0 for the first packet): a jump means packets were lost there.
    pub packet_index: Vec<u32>,
    /// The parsed headers.
    pub headers: Headers,
    /// A packet was lost, failed its CRC, or the file was cut short.
    pub errors: bool,
    /// A new first packet ended it before its last packet.
    pub truncated: bool,
    /// Signal time it was completed.
    pub time: f64,
}

impl Item for XritFile {
    const KIND: &'static str = "xrit.file";
}

sdrkit_core::choice_enum! {
    /// What kind of product.
    pub enum ProductKind {
        /// An administrative message.
        AdminMessage = ("admin_message", "Admin message"),
        /// Data collection system.
        Dcs = ("dcs", "DCS"),
        /// EMWIN text.
        Emwin = ("emwin", "EMWIN"),
        /// A mesoscale or local area image.
        Mesoscale = ("mesoscale", "Mesoscale"),
        /// A full-disk image.
        FullDisk = ("full_disk", "Full disk"),
        /// A CONUS image.
        Conus = ("conus", "CONUS"),
        /// Any other image.
        Image = ("image", "Image"),
        /// Text.
        Text = ("text", "Text"),
        /// Anything else.
        Other = ("other", "Other"),
    }
}

impl ProductKind {
    /// The name the screens show.
    pub fn label(self) -> &'static str {
        match Self::CHOICES[usize::from(self.index())].label {
            std::borrow::Cow::Borrowed(l) => l,
            std::borrow::Cow::Owned(_) => "",
        }
    }
    /// An image kind.
    pub fn is_image(self) -> bool {
        matches!(
            self,
            ProductKind::Mesoscale
                | ProductKind::FullDisk
                | ProductKind::Conus
                | ProductKind::Image
        )
    }
}

/// One product as the screens list it, published each time it changes.
#[derive(Clone, Debug, PartialEq)]
pub struct ProductItem {
    /// Running id.
    pub id: u32,
    /// Kind.
    pub kind: ProductKind,
    /// Name.
    pub name: String,
    /// Virtual channel and APID.
    pub vcid: u8,
    /// APID.
    pub apid: u16,
    /// Rows received and total (images).
    pub rows: u32,
    /// Picture height.
    pub height: u32,
    /// Picture width.
    pub width: u32,
    /// Segments received and total.
    pub segments: (u32, u32),
    /// Every segment arrived.
    pub complete: bool,
    /// A file had errors.
    pub errors: bool,
    /// Grows with every change.
    pub version: u32,
    /// A note ("picture not decoded: ...").
    pub note: String,
}

impl Item for ProductItem {
    const KIND: &'static str = "satimg.product";
    fn event_def() -> Option<&'static EventDef> {
        Some(PRODUCT_EVENT.def())
    }
    fn write_fields(&self, r: &mut Record) {
        ProductEvent {
            source: &self.name,
            text: &self.name,
            family: Family::Satellite,
            id: i64::from(self.id),
            kind: self.kind,
            name: &self.name,
            vcid: i64::from(self.vcid),
            apid: i64::from(self.apid),
            rows: i64::from(self.rows),
            width: i64::from(self.width),
            height: i64::from(self.height),
            segments: i64::from(self.segments.0),
            segments_total: i64::from(self.segments.1),
            complete: self.complete,
            errors: self.errors,
            version: i64::from(self.version),
            note: (!self.note.is_empty()).then_some(self.note.as_str()),
        }
        .write(r);
    }
}

/// A text product.
#[derive(Clone, Debug, PartialEq)]
pub struct SatText {
    /// The product's name.
    pub name: String,
    /// The text (UTF-8; ZIP unpacked).
    pub text: String,
    /// The file name inside a ZIP.
    pub inner_name: Option<String>,
    /// The product it belongs to.
    pub product: u32,
}

impl Item for SatText {
    const KIND: &'static str = "satimg.text";
    fn event_def() -> Option<&'static EventDef> {
        Some(TEXT_EVENT.def())
    }
    fn write_fields(&self, r: &mut Record) {
        TextEvent {
            source: &self.name,
            text: &self.text,
            family: Family::Satellite,
            name: &self.name,
            inner_name: self.inner_name.as_deref(),
            id: i64::from(self.product),
        }
        .write(r);
    }
}

/// One Elektro-L GGAK telemetry frame.
#[derive(Clone, Debug, PartialEq)]
pub struct GgakFrame {
    /// The 224 bytes.
    pub bytes: Vec<u8>,
    /// Byte 4 high nibble.
    pub marker: u8,
    /// Byte 4 low nibble.
    pub subframe: u8,
    /// Bytes 5 to 12, big-endian.
    pub timestamp: u64,
    /// The thirteen (Bx, By, Bz) triples.
    pub field: Vec<[i16; 3]>,
    /// The CRC is right.
    pub crc_ok: bool,
    /// The polarity was inverted.
    pub inverted: bool,
    /// Signal time.
    pub time: f64,
}

impl Item for GgakFrame {
    const KIND: &'static str = "satimg.ggak_frame";
    fn event_def() -> Option<&'static EventDef> {
        Some(GGAK_EVENT.def())
    }
    fn write_fields(&self, r: &mut Record) {
        let t = self.field.first().copied().unwrap_or([0; 3]);
        GgakFrameEvent {
            family: Family::Satellite,
            marker: i64::from(self.marker),
            subframe: i64::from(self.subframe),
            timestamp: self.timestamp as i64,
            crc_ok: self.crc_ok,
            inverted: self.inverted,
            bx: i64::from(t[0]),
            by: i64::from(t[1]),
            bz: i64::from(t[2]),
        }
        .write(r);
    }
}
