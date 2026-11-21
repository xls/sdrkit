//! What a host shows of broadcast data: the result sets and tables of radio text with its tags, the programme guide, the
//! slideshow and the logos, and [`Publisher`], which writes them from the decoded data. A stack lists them from
//! [`interface`] in its description and calls the [`Publisher`] when its decoders give new data.
//!
//! Each table has a row shape and each group of fields a shape ([`LabelFields`], [`SlideFields`]). The publisher writes
//! whole rows and sets through them. A stack with an interface description names the same shapes and tables
//! ([`LABEL_TAG_TABLE`] and the others) in its description.

use crate::slideshow::{Categories, PictureFormat, Slide};
use crate::spi::Guide;
use crate::tags::TagSet;
use sdrkit_core::interface::{Family, Fields, Seconds, StackShape, Table, Time};
use sdrkit_core::media::{Codec, Picture};
use sdrkit_core::publish::{field, keys, Field, Published, TableDef};
use sdrkit_core::record::ContentType;
use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};

/// The item running bit of the last tags: the tagged item (a song) is playing.
pub const ITEM_RUNNING: Field<field::Bool> = Field::new("item_running", "Item running", "");
/// The name of the slide shown (its MOT ContentName).
pub const SLIDE_NAME: Field<field::Text> = Field::new("slide_name", "Slide", "");
/// The category title of the slide shown.
pub const SLIDE_CATEGORY: Field<field::Text> = Field::new("slide_category", "Slide category", "");
/// The click-through URL of the slide shown.
pub const SLIDE_LINK: Field<field::Text> = Field::new("slide_link", "Slide link", "");
/// The slide shown is an emergency alert (TS 101 499 Alert parameter 1).
pub const SLIDE_ALERT: Field<field::Bool> = Field::new("slide_alert", "Emergency slide", "");

/// Column: a content type code (0 to 127, TS 102 980 Annex A).
pub const CONTENT_CODE: Field<field::I64> = Field::new("code", "Code", "");
/// Column: what people call a content type ("Title").
pub const CONTENT_LABEL: Field<field::Text> = Field::new("type", "Type", "");
/// Column: a name.
pub const NAME: Field<field::Text> = Field::new("name", "Name", "");
/// Column: a short name.
pub const SHORT_NAME: Field<field::Text> = Field::new("short_name", "Short name", "");
/// Column: a description.
pub const DESCRIPTION: Field<field::Text> = Field::new("description", "Description", "");
/// Column: the service identifier of the first bearer.
pub const SID: Field<field::I64> = Field::new("sid", "SId", "");
/// Column: the ensemble identifier of the first bearer.
pub const EID: Field<field::I64> = Field::new("eid", "EId", "");
/// Column: the genre links (TS 102 818 genre href), separated by spaces.
pub const GENRES: Field<field::Text> = Field::new("genres", "Genres", "");
/// Column: the URL of the first logo.
pub const LOGO_URL: Field<field::Text> = Field::new("logo_url", "Logo", "");
/// Column: the programme id (CRID).
pub const CRID: Field<field::Text> = Field::new("crid", "Programme id", "");
/// Column: the start time.
pub const START: Field<field::Time> = Field::new("start", "Start", "s");
/// Column: the duration.
pub const DURATION: Field<field::Duration> = Field::new("duration", "Duration", "s");
/// Column: a slideshow CategoryID.
pub const CATEGORY: Field<field::I64> = Field::new("category", "Category", "");
/// Column: a slideshow CategoryTitle.
pub const CATEGORY_TITLE: Field<field::Text> = Field::new("title", "Title", "");
/// Column: a slideshow SlideID.
pub const SLIDE_ID: Field<field::I64> = Field::new("slide", "Slide", "");
/// Column: a PNG picture.
pub const PNG: Field<field::Bytes> =
    Field::new("png", "PNG picture", "").content(ContentType::Media(Codec::Png));
/// Column: a JPEG picture.
pub const JPEG: Field<field::Bytes> =
    Field::new("jpeg", "JPEG picture", "").content(ContentType::Media(Codec::Jpeg));

/// The radio text and whether its item is running.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LabelFields<'a> {
    /// The radio text.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// The tagged item (a song) is playing.
    #[field(shared = ITEM_RUNNING)]
    pub item_running: Option<bool>,
}

/// The slide shown.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SlideFields<'a> {
    /// The name of the slide (its MOT ContentName).
    #[field(shared = SLIDE_NAME)]
    pub slide_name: Option<&'a str>,
    /// The category title of the slide.
    #[field(shared = SLIDE_CATEGORY)]
    pub slide_category: Option<&'a str>,
    /// The click-through URL of the slide.
    #[field(shared = SLIDE_LINK)]
    pub slide_link: Option<&'a str>,
    /// The slide is an emergency alert.
    #[field(shared = SLIDE_ALERT)]
    pub slide_alert: bool,
}

/// A tag of the radio text. The row key is the class name of the content type (`ITEM.TITLE`), or its code for a
/// reserved type.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LabelTagRow<'a> {
    /// The content type code (0 to 127).
    #[field(shared = CONTENT_CODE)]
    pub code: i64,
    /// What people call the content type ("Title").
    #[field(shared = CONTENT_LABEL)]
    pub r#type: Option<&'a str>,
    /// The tagged text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A service of the service information. The row key is the SId in hexadecimal, or the place of a service without a
/// bearer.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GuideServiceRow<'a> {
    /// The name.
    #[field(shared = NAME)]
    pub name: Option<&'a str>,
    /// The short name.
    #[field(shared = SHORT_NAME)]
    pub short_name: Option<&'a str>,
    /// The description.
    #[field(shared = DESCRIPTION)]
    pub description: Option<&'a str>,
    /// The service identifier of the first bearer.
    #[field(shared = SID)]
    pub sid: Option<i64>,
    /// The ensemble identifier of the first bearer.
    #[field(shared = EID)]
    pub eid: Option<i64>,
    /// The genre links, separated by spaces.
    #[field(shared = GENRES)]
    pub genres: Option<&'a str>,
    /// The URL of the first logo.
    #[field(shared = LOGO_URL)]
    pub logo_url: Option<&'a str>,
}

/// A programme of the programme information. The row key is `<crid>@<start>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GuideProgrammeRow<'a> {
    /// The programme id (CRID).
    #[field(shared = CRID)]
    pub crid: &'a str,
    /// The name.
    #[field(shared = NAME)]
    pub name: Option<&'a str>,
    /// The start time.
    #[field(shared = START)]
    pub start: Time,
    /// The duration.
    #[field(shared = DURATION)]
    pub duration: Seconds,
    /// The description.
    #[field(shared = DESCRIPTION)]
    pub description: Option<&'a str>,
    /// The genre links, separated by spaces.
    #[field(shared = GENRES)]
    pub genres: Option<&'a str>,
}

/// A programme group. The row key is its name.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GuideGroupRow<'a> {
    /// The name.
    #[field(shared = NAME)]
    pub name: &'a str,
}

/// A slide of a categorised slideshow. The row key is `<CategoryID>/<SlideID>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SlideCategoryRow<'a> {
    /// The CategoryID.
    #[field(shared = CATEGORY)]
    pub category: i64,
    /// The CategoryTitle.
    #[field(shared = CATEGORY_TITLE)]
    pub title: Option<&'a str>,
    /// The SlideID.
    #[field(shared = SLIDE_ID)]
    pub slide: i64,
    /// The name of the slide.
    #[field(shared = NAME)]
    pub name: &'a str,
}

/// A logo. The row key is its MOT ContentName. A row holds the picture in the member of its format.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LogoRow {
    /// A PNG picture.
    #[field(shared = PNG)]
    pub png: Option<Arc<[u8]>>,
    /// A JPEG picture.
    #[field(shared = JPEG)]
    pub jpeg: Option<Arc<[u8]>>,
}

/// The tags of the radio text.
pub static LABEL_TAG_TABLE: Table<LabelTagRow<'static>> =
    Table::new("label_tags", "Text tags", 128);
/// The services of the service information.
pub static GUIDE_SERVICE_TABLE: Table<GuideServiceRow<'static>> =
    Table::new("guide_services", "Guide services", Guide::MAX_SERVICES);
/// The programmes of the programme information.
pub static GUIDE_PROGRAMME_TABLE: Table<GuideProgrammeRow<'static>> =
    Table::new("guide_programmes", "Programmes", Guide::MAX_PROGRAMMES);
/// The programme groups.
pub static GUIDE_GROUP_TABLE: Table<GuideGroupRow<'static>> =
    Table::new("guide_groups", "Programme groups", Guide::MAX_GROUPS);
/// The slides of a categorised slideshow.
pub static SLIDE_CATEGORY_TABLE: Table<SlideCategoryRow<'static>> = Table::new(
    "slide_categories",
    "Slide categories",
    Categories::MAX * Categories::MAX_SLIDES,
);
/// The logos.
pub static LOGO_TABLE: Table<LogoRow> = Table::new("logos", "Logos", 256);

/// The interface of the result sets and tables of this crate, which stacks of several families show: the radio text,
/// the slide shown, the text tags, the guide, the slide categories, the logos and the counts of the playout component
/// ([`crate::component::PlayoutFields`]). A family whose stacks list one of them names this interface with
/// `Family::uses`.
pub fn interface() -> &'static Family {
    static SHARED: OnceLock<Family> = OnceLock::new();
    SHARED.get_or_init(|| {
        Family::shared("bcdata", 1, 0).stack(
            StackShape::new("bcdata")
                .fields::<LabelFields>()
                .fields::<SlideFields>()
                .table(&LABEL_TAG_TABLE)
                .table(&GUIDE_SERVICE_TABLE)
                .table(&GUIDE_PROGRAMME_TABLE)
                .table(&GUIDE_GROUP_TABLE)
                .table(&SLIDE_CATEGORY_TABLE)
                .table(&LOGO_TABLE)
                .component("playout", |c| c.fields::<crate::component::PlayoutFields>()),
        )
    })
}

/// The declaration of [`LABEL_TAG_TABLE`], for a stack that lists its tables by declaration.
pub const LABEL_TAGS: TableDef =
    Table::<LabelTagRow<'static>>::def_of("label_tags", "Text tags", 128);
/// Writes decoded broadcast data into a [`Published`] that declares the result sets and tables of [`interface`]. A cell is written only when
/// its value changed, so the change counters move only for real changes. Its scratch space is kept, so a label update
/// allocates nothing once warm.
#[derive(Debug, Default)]
pub struct Publisher {
    key: String,
    gone: Vec<String>,
    guide_objects: Option<u64>,
}

impl Publisher {
    /// A publisher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes the radio text `text` (a dynamic label or a RadioText) and its tags: the `text` field, `item_running` and the
    /// `label_tags` table. A row of a content type that `tags` does not hold is removed.
    pub fn label(&mut self, p: &mut Published, text: &str, tags: &TagSet) {
        p.put(&LabelFields {
            text: Some(text),
            item_running: Some(tags.item.running),
        });
        for (code, t) in tags.entries() {
            self.key.clear();
            tag_key(code, &mut self.key);
            let row = LabelTagRow {
                code: i64::from(code),
                r#type: sdrkit_util_ids::content_type::get(code).map(|c| c.label),
                text: t,
            };
            LABEL_TAG_TABLE.put(p, &self.key, &row);
        }
        self.remove_where(p, &LABEL_TAG_TABLE, |_, r| {
            r.get(&CONTENT_CODE)
                .is_none_or(|c| tags.text(c as u8).is_none())
        });
    }

    /// Writes the guide `g`: the `guide_services`, `guide_programmes` and `guide_groups` tables. Nothing is written when `g`
    /// has no new object since the last call. A row that `g` does not hold is removed.
    pub fn guide(&mut self, p: &mut Published, g: &Guide) {
        if self.guide_objects == Some(g.objects) {
            return;
        }
        self.guide_objects = Some(g.objects);
        let services = &g.services;
        for (i, s) in services.iter().enumerate() {
            self.key.clear();
            service_key(s, i, &mut self.key);
            let bearer = s.bearers.first();
            let genres = (!s.genres.is_empty()).then(|| s.genres.join(" "));
            let row = GuideServiceRow {
                name: best_name(&s.names),
                short_name: non_empty(&s.names[0]),
                description: non_empty(&s.description),
                sid: bearer.map(|b| i64::from(b.1)),
                eid: bearer.map(|b| i64::from(b.0)),
                genres: genres.as_deref(),
                logo_url: s.logos.first().map(String::as_str),
            };
            GUIDE_SERVICE_TABLE.put(p, &self.key, &row);
        }
        let mut key = String::new();
        self.remove_where(p, &GUIDE_SERVICE_TABLE, |k, _| {
            !services.iter().enumerate().any(|(i, s)| {
                key.clear();
                service_key(s, i, &mut key);
                key == k
            })
        });
        for pr in &g.programmes {
            self.key.clear();
            push_fmt(&mut self.key, format_args!("{}@{}", pr.id, pr.start));
            let genres = (!pr.genres.is_empty()).then(|| pr.genres.join(" "));
            let row = GuideProgrammeRow {
                crid: &pr.id,
                name: best_name(&pr.names),
                start: Time(pr.start as f64),
                duration: Seconds(f64::from(pr.duration_s)),
                description: non_empty(&pr.description),
                genres: genres.as_deref(),
            };
            GUIDE_PROGRAMME_TABLE.put(p, &self.key, &row);
        }
        self.remove_where(p, &GUIDE_PROGRAMME_TABLE, |_, r| {
            let (Some(id), Some(start)) = (r.get(&CRID), r.get(&START)) else {
                return true;
            };
            !g.programmes
                .iter()
                .any(|x| x.id == id && x.start as f64 == start)
        });
        for name in &g.groups {
            GUIDE_GROUP_TABLE.put(p, name, &GuideGroupRow { name });
        }
        self.remove_where(p, &GUIDE_GROUP_TABLE, |k, _| {
            !g.groups.iter().any(|n| n == k)
        });
    }

    /// Writes the slide shown: the fields `slide_name`, `slide_category`, `slide_link` and `slide_alert`. It returns the
    /// picture for the picture stream. The picture shares `data`, the MOT body.
    pub fn slide(
        &mut self,
        p: &mut Published,
        slide: &Slide<'_>,
        name: &str,
        data: Arc<[u8]>,
    ) -> Picture {
        p.put(&SlideFields {
            slide_name: non_empty(name),
            slide_category: slide.category_title,
            slide_link: slide.click_through,
            slide_alert: slide.alert == Some(1),
        });
        Picture {
            codec: slide.format.codec(),
            layout: None,
            name: non_empty(name).map(str::to_string),
            data,
        }
    }

    /// Writes the categories of a categorised slideshow: the `slide_categories` table. A row that `c` does not hold is removed.
    pub fn categories(&mut self, p: &mut Published, c: &Categories) {
        for cat in &c.list {
            for (sid, name) in &cat.slides {
                self.key.clear();
                push_fmt(&mut self.key, format_args!("{}/{}", cat.id, sid));
                let row = SlideCategoryRow {
                    category: i64::from(cat.id),
                    title: non_empty(&cat.title),
                    slide: i64::from(*sid),
                    name,
                };
                SLIDE_CATEGORY_TABLE.put(p, &self.key, &row);
            }
        }
        self.remove_where(p, &SLIDE_CATEGORY_TABLE, |_, r| {
            let (Some(cid), Some(sid)) = (r.get(&CATEGORY), r.get(&SLIDE_ID)) else {
                return true;
            };
            !c.list
                .iter()
                .any(|x| i64::from(x.id) == cid && x.slides.iter().any(|s| i64::from(s.0) == sid))
        });
    }

    /// Writes a logo of `format` named `name` (its MOT ContentName, which a service's `logo_url` names) into the `logos`
    /// table. The cell shares `data`.
    pub fn logo(&mut self, p: &mut Published, name: &str, format: PictureFormat, data: Arc<[u8]>) {
        let row = match format {
            PictureFormat::Png => LogoRow {
                png: Some(data),
                jpeg: None,
            },
            PictureFormat::Jpeg => LogoRow {
                png: None,
                jpeg: Some(data),
            },
        };
        LOGO_TABLE.put(p, name, &row);
    }

    /// Clears what [`Publisher::label`] wrote: the display was cleared.
    pub fn clear_label(&mut self, p: &mut Published) {
        p.put(&LabelFields {
            text: None,
            item_running: None,
        });
        p.clear_rows(&LABEL_TAG_TABLE);
    }

    /// Removes the rows of `t` for which `gone(key, row)` is true.
    fn remove_where(
        &mut self,
        p: &mut Published,
        t: &TableDef,
        mut gone: impl FnMut(&str, &sdrkit_core::Record) -> bool,
    ) {
        let r = p.results_mut();
        let Some(id) = r.decl().table_id(t.key) else {
            return;
        };
        let Some(table) = r.table(id) else {
            return;
        };
        let mut n = 0;
        for (k, row) in table.rows_newest_first() {
            if gone(k, row) {
                if let Some(slot) = self.gone.get_mut(n) {
                    slot.clear();
                    slot.push_str(k);
                } else {
                    self.gone.push(k.to_string());
                }
                n += 1;
            }
        }
        if let Some(mut table) = r.table_mut(id) {
            for k in &self.gone[..n] {
                table.remove(k);
            }
        }
    }
}

/// The row key of a content type: its class name, or its code for a reserved type.
fn tag_key(code: u8, out: &mut String) {
    match sdrkit_util_ids::content_type::get(code) {
        Some(c) => out.push_str(c.class),
        None => push_fmt(out, format_args!("{code}")),
    }
}

/// The row key of a service: its SId in hexadecimal, or its place.
fn service_key(s: &crate::spi::Service, i: usize, out: &mut String) {
    match s.bearers.first() {
        Some(b) => push_fmt(out, format_args!("{:X}", b.1)),
        None => push_fmt(out, format_args!("#{i}")),
    }
}

/// Appends `args` to `out`.
fn push_fmt(out: &mut String, args: std::fmt::Arguments<'_>) {
    // Writing to a `String` cannot fail.
    let _ = out.write_fmt(args);
}

/// The longest name that is not empty (long, then medium, then short).
fn best_name(names: &[String; 3]) -> Option<&str> {
    names
        .iter()
        .rev()
        .map(String::as_str)
        .find(|s| !s.is_empty())
}

fn non_empty(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}
