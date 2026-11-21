//! The MOT slideshow: the slide information of a MOT header and the categories of a categorised slideshow.
//!
//! Standard: ETSI TS 101 499.

use crate::mot::{param, Header, MotTime};
use sdrkit_core::media::Codec;

/// The picture format of a slide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PictureFormat {
    /// JFIF (ContentSubType 1).
    Jpeg,
    /// PNG (ContentSubType 3).
    Png,
}

impl PictureFormat {
    /// The MIME type.
    pub const fn mime(self) -> &'static str {
        match self {
            PictureFormat::Jpeg => "image/jpeg",
            PictureFormat::Png => "image/png",
        }
    }
    /// The codec of its pictures.
    pub const fn codec(self) -> Codec {
        match self {
            PictureFormat::Jpeg => Codec::Jpeg,
            PictureFormat::Png => Codec::Png,
        }
    }
}

/// The slideshow information of a MOT header.
///
/// ```
/// use sdrkit_proto_bcdata::mot::{self, Header, HeaderWriter, MotTime};
/// use sdrkit_proto_bcdata::slideshow::{PictureFormat, Slide};
///
/// let mut b = Vec::new();
/// HeaderWriter::new(10, 2, 1).content_name("a.jpg").param(mot::param::CATEGORY_SLIDE, &[3, 7]).write(&mut b).unwrap();
/// let s = Slide::from_header(&Header::parse(&b).unwrap()).unwrap();
/// assert_eq!((s.format, s.category, s.due(0)), (PictureFormat::Jpeg, Some((3, 7)), true));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slide<'a> {
    /// The picture format.
    pub format: PictureFormat,
    /// CategoryID and SlideID.
    pub category: Option<(u8, u8)>,
    /// CategoryTitle (UTF-8 bytes, as sent).
    pub category_title: Option<&'a str>,
    /// ClickThroughURL.
    pub click_through: Option<&'a str>,
    /// AlternativeLocationURL.
    pub alternative_location: Option<&'a str>,
    /// Alert: 1 is an emergency alert.
    pub alert: Option<u8>,
    /// TriggerTime.
    pub trigger: Option<MotTime>,
}

fn utf8(d: Option<&[u8]>) -> Option<&str> {
    d.and_then(|d| std::str::from_utf8(d).ok())
}

impl<'a> Slide<'a> {
    /// The slide information of a header. `None` if the object is not a JFIF or PNG image.
    pub fn from_header(h: &Header<'a>) -> Option<Slide<'a>> {
        let format = match (h.content_type, h.content_subtype) {
            (2, 1) => PictureFormat::Jpeg,
            (2, 3) => PictureFormat::Png,
            _ => return None,
        };
        Some(Slide {
            format,
            category: h
                .param(param::CATEGORY_SLIDE)
                .filter(|d| d.len() >= 2)
                .map(|d| (d[0], d[1])),
            category_title: utf8(h.param(param::CATEGORY_TITLE)),
            click_through: utf8(h.param(param::CLICK_THROUGH_URL)),
            alternative_location: utf8(h.param(param::ALTERNATIVE_LOCATION_URL)),
            alert: h.param(param::ALERT).and_then(|d| d.first().copied()),
            trigger: h.trigger_time(),
        })
    }

    /// Whether the slide shows at `now_unix_s`: at once for "now" or no TriggerTime, else from the TriggerTime on.
    pub fn due(&self, now_unix_s: i64) -> bool {
        match self.trigger {
            None | Some(MotTime::Now) => true,
            Some(MotTime::At { unix_s, .. }) => now_unix_s >= unix_s,
        }
    }
}

/// One category of a categorised slideshow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    /// CategoryID.
    pub id: u8,
    /// CategoryTitle (empty until one is received).
    pub title: String,
    /// The slides: SlideID and ContentName, in SlideID order.
    pub slides: Vec<(u8, String)>,
}

/// The categories of a categorised slideshow, built from the slides received.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Categories {
    /// The categories, in CategoryID order.
    pub list: Vec<Category>,
}

impl Categories {
    /// The most categories kept.
    pub const MAX: usize = 64;
    /// The most slides kept in one category.
    pub const MAX_SLIDES: usize = 64;

    /// Adds a slide with its ContentName. A slide with the same CategoryID and SlideID replaces the one before.
    pub fn add(&mut self, slide: &Slide<'_>, content_name: &str) {
        let Some((cat, sid)) = slide.category else {
            return;
        };
        let i = match self.list.binary_search_by_key(&cat, |c| c.id) {
            Ok(i) => i,
            Err(i) => {
                if self.list.len() >= Self::MAX {
                    return;
                }
                self.list.insert(
                    i,
                    Category {
                        id: cat,
                        title: String::new(),
                        slides: Vec::new(),
                    },
                );
                i
            }
        };
        let c = &mut self.list[i];
        if let Some(t) = slide.category_title {
            if c.title != t {
                c.title.clear();
                c.title.push_str(t);
            }
        }
        match c.slides.binary_search_by_key(&sid, |s| s.0) {
            Ok(j) => {
                if c.slides[j].1 != content_name {
                    c.slides[j].1.clear();
                    c.slides[j].1.push_str(content_name);
                }
            }
            Err(j) => {
                if c.slides.len() < Self::MAX_SLIDES {
                    c.slides.insert(j, (sid, content_name.to_string()));
                }
            }
        }
    }

    /// Forgets every category.
    pub fn clear(&mut self) {
        self.list.clear();
    }
}
