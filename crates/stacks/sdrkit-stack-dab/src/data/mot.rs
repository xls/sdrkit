//! MOT objects as DAB items: an owned copy of each object that the shared assembler completes, with the slideshow fields read
//! once, so the object can leave its component.
//!
//! Standard: EN 301 234, TS 101 499.

use sdrkit_core::media::Codec;
use sdrkit_proto_bcdata::mot::{param, Header, MotTime, Object};
use sdrkit_util_text::charset_field::{display_into, Charset};
use std::sync::Arc;

/// The header of an MOT object, read once from the shared view.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MotHeader {
    /// BodySize.
    pub body_size: u32,
    /// HeaderSize.
    pub header_size: u16,
    /// ContentType.
    pub content_type: u8,
    /// ContentSubType.
    pub content_subtype: u16,
    /// ContentName with its character set byte first, as sent.
    pub content_name_raw: Vec<u8>,
    /// ContentName as text.
    pub content_name: String,
    /// CategoryID and SlideID.
    pub category_slide: Option<(u8, u8)>,
    /// CategoryTitle.
    pub category_title: String,
    /// ClickThroughURL.
    pub click_through: String,
    /// AlternativeLocationURL.
    pub alternative_url: String,
    /// TriggerTime: seconds since 1970 and milliseconds, `None` for "now" or no TriggerTime.
    pub trigger: Option<(i64, u16)>,
    /// ExpireTime, the same way.
    pub expire: Option<(i64, u16)>,
    /// CompressionType (when compressed).
    pub compression: Option<u8>,
}

fn time(t: Option<MotTime>) -> Option<(i64, u16)> {
    match t? {
        MotTime::Now => None,
        MotTime::At { unix_s, ms } => Some((unix_s, ms)),
    }
}

fn text(h: &Header<'_>, id: u8) -> String {
    let mut s = String::new();
    if let Some(d) = h.param(id) {
        display_into(d, Charset::Utf8, &mut s);
    }
    s
}

impl MotHeader {
    /// The fields of a shared header view.
    pub fn of(h: &Header<'_>) -> Self {
        MotHeader {
            body_size: h.body_size,
            header_size: h.header_size,
            content_type: h.content_type,
            content_subtype: h.content_subtype,
            content_name_raw: h.param(param::CONTENT_NAME).unwrap_or_default().to_vec(),
            content_name: h.content_name_text(),
            category_slide: h
                .param(param::CATEGORY_SLIDE)
                .filter(|d| d.len() >= 2)
                .map(|d| (d[0], d[1])),
            category_title: text(h, param::CATEGORY_TITLE),
            click_through: text(h, param::CLICK_THROUGH_URL),
            alternative_url: text(h, param::ALTERNATIVE_LOCATION_URL),
            trigger: time(h.trigger_time()),
            expire: time(h.expire_time()),
            compression: h.compression_type(),
        }
    }
}

/// A completed MOT object, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct MotObject {
    /// Transport id.
    pub transport_id: u16,
    /// Header.
    pub header: MotHeader,
    /// The header bytes as sent (core and extension), for a [`Header`] view.
    pub header_bytes: Arc<[u8]>,
    /// Body.
    pub body: Arc<[u8]>,
}

impl MotObject {
    /// An owned copy of a shared object.
    pub fn of(o: &Object<'_>) -> Self {
        MotObject {
            transport_id: o.transport_id,
            header: MotHeader::of(&o.header),
            header_bytes: o.header.bytes.into(),
            body: o.body.into(),
        }
    }
}

/// The codec of a slideshow picture by MOT content subtype (content type 2), `None` for other objects and for BMP, which has
/// no codec of its own.
///
/// Standard: TS 101 499 clause 6.2.
pub fn picture_codec(h: &MotHeader) -> Option<Codec> {
    if h.content_type != 2 {
        return None;
    }
    match h.content_subtype {
        0 => Some(Codec::Gif),
        1 => Some(Codec::Jpeg),
        3 => Some(Codec::Png),
        _ => None,
    }
}
