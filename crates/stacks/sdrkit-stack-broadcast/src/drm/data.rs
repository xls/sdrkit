//! What DRM adds to the shared data layers: the counters of a packet-mode service, and a MOT object as the receiver keeps it.
//!
//! Standard: ETSI ES 201 980 V4.3.1 clause 6.6 (packet mode); the MOT, slideshow and Journaline layers are in `sdrkit-proto-bcdata`.

use sdrkit_proto_bcdata::mot::{self, MotTime};
use sdrkit_proto_bcdata::packet::{DrmAssembler, DrmHeader};
use sdrkit_proto_bcdata::slideshow::Slide;
use sdrkit_util_bits::crc::catalog::CRC16_GENIBUS;
use sdrkit_util_bits::crc::Trailer;
use std::sync::Arc;

/// The counters of a packet-mode service.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PacketStats {
    /// Packets of the service's packet id with a good CRC.
    pub packets: u64,
    /// Packets with a bad CRC.
    pub bad_crc: u64,
    /// Packets lost: jumps of the continuity index.
    pub lost: u64,
    /// Continuation packets without their first packet.
    pub orphans: u64,
    /// Padding packets.
    pub empty: u64,
    /// Packets of another packet id.
    pub other_id: u64,
    /// Data units complete.
    pub units_good: u64,
    /// Data units dropped.
    pub units_dropped: u64,
    /// Bytes of the complete data units.
    pub unit_bytes: u64,
}

/// The packet layer of one packet-mode service: the shared assembler of one packet id, and the counters the service shows.
#[derive(Clone, Debug)]
pub struct PacketService {
    id: u8,
    len: usize,
    asm: DrmAssembler,
    next_continuity: Option<u8>,
    /// The counters.
    pub stats: PacketStats,
}

impl PacketService {
    /// The service of packet id `id` with packets of `len` bytes (header, data and CRC). `None` for a length or id the standard
    /// does not allow.
    pub fn new(id: u8, len: usize) -> Option<PacketService> {
        Some(PacketService {
            id,
            len,
            asm: DrmAssembler::new(len, Some(id)).ok()?,
            next_continuity: None,
            stats: PacketStats::default(),
        })
    }

    /// Takes the bytes of one logical frame of the stream; each complete data unit goes to `out`.
    pub fn push(&mut self, bytes: &[u8], out: &mut dyn FnMut(&[u8])) {
        for p in bytes.chunks_exact(self.len) {
            if !CRC16_GENIBUS.verify(p, Trailer::Big) {
                self.stats.bad_crc += 1;
                continue;
            }
            let h = DrmHeader::parse(p[0]);
            if h.id != self.id {
                self.stats.other_id += 1;
                continue;
            }
            if let Some(c) = self.next_continuity {
                if c != h.continuity {
                    self.stats.lost += ((h.continuity + 8 - c) & 7) as u64;
                }
            }
            self.next_continuity = Some((h.continuity + 1) & 7);
            self.stats.packets += 1;
            let before = self.asm.stats;
            let stats = &mut self.stats;
            self.asm.packet(p, &mut |_, unit| {
                stats.units_good += 1;
                stats.unit_bytes += unit.len() as u64;
                out(unit);
            });
            let after = self.asm.stats;
            self.stats.orphans += after.orphans - before.orphans;
            self.stats.empty += after.padding - before.padding;
            self.stats.units_dropped += after.groups_dropped - before.groups_dropped;
        }
    }
}

/// A complete MOT object as the receiver keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct MotObject {
    /// The transport id.
    pub transport_id: u16,
    /// ContentName, or empty.
    pub name: String,
    /// ContentType.
    pub content_type: u8,
    /// ContentSubType.
    pub content_subtype: u16,
    /// The slideshow parameters, for a slide.
    pub slide: Option<SlideInfo>,
    /// The body.
    pub body: Arc<[u8]>,
}

/// The slideshow parameters of a slide (ETSI TS 101 499).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlideInfo {
    /// CategoryID and SlideID.
    pub category: Option<(u8, u8)>,
    /// CategoryTitle.
    pub category_title: String,
    /// ClickThroughURL.
    pub click_url: String,
    /// AlternativeLocationURL.
    pub alt_url: String,
    /// Alert (1 is an emergency alert).
    pub alert: Option<u8>,
    /// TriggerTime as text: "now", "YYYY-MM-DD HH:MM", or empty when not sent.
    pub trigger: String,
}

impl MotObject {
    /// The owned object of a complete object of the shared MOT assembler.
    pub fn from_object(o: &mot::Object<'_>) -> MotObject {
        let h = &o.header;
        let slide = Slide::from_header(h).map(|s| SlideInfo {
            category: s.category,
            category_title: s.category_title.unwrap_or("").into(),
            click_url: s.click_through.unwrap_or("").into(),
            alt_url: s.alternative_location.unwrap_or("").into(),
            alert: s.alert,
            trigger: s.trigger.map(time_text).unwrap_or_default(),
        });
        MotObject {
            transport_id: o.transport_id,
            name: h.content_name_text(),
            content_type: h.content_type,
            content_subtype: h.content_subtype,
            slide,
            body: Arc::from(o.body),
        }
    }
    /// The content type as text ("image/PNG").
    pub fn content_type_text(&self) -> String {
        content_type_text(self.content_type, self.content_subtype)
    }
}

/// The text of a ContentType and ContentSubType ("image/JPEG"), from the MIME types of TS 101 756 table 17.
pub fn content_type_text(t: u8, s: u16) -> String {
    match (t, s) {
        (2, 1) => "image/JPEG".into(),
        (2, 3) => "image/PNG".into(),
        (2, 0) => "image/GIF".into(),
        (2, 2) => "image/BMP".into(),
        (1, 2) => "text/HTML".into(),
        (1, 0) | (1, 1) => "text/plain".into(),
        _ => mot::content_mime(t, s).map_or_else(|| format!("type {t}/{s}"), String::from),
    }
}

/// A MOT time as text: "now", or "YYYY-MM-DD HH:MM" in UTC.
pub fn time_text(t: MotTime) -> String {
    match t {
        MotTime::Now => "now".into(),
        MotTime::At { unix_s, .. } => {
            let mjd = unix_s.div_euclid(86_400) + 40_587;
            let tod = unix_s.rem_euclid(86_400);
            super::fac::date_text(mjd as u32, (tod / 3600) as u8, ((tod / 60) % 60) as u8)
                .unwrap_or_default()
        }
    }
}
