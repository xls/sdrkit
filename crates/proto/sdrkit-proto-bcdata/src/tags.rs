//! DL Plus and RadioText Plus (RT+) tags: one content-type model for both, the parsers of the DL Plus command and of the
//! RT+ group, and [`TagSet`], the latest text of each content type.
//!
//! Standard: ETSI TS 102 980 clause 7.2 and Annex A, RDS Forum R06/040_1 (RT+).

use crate::{Error, Result};
use crate::{Malformed, Parameter};
use sdrkit_util_ids::content_type::{self, ContentType};

/// One tag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tag {
    /// The content type (0 to 127). Type 0 is a dummy.
    pub content_type: u8,
    /// The start marker: the index of the first character.
    pub start: u8,
    /// The length marker: the number of characters less 1.
    pub length_marker: u8,
}

impl Tag {
    /// The tagged part of `text`, counted in characters and trimmed. `None` for a dummy tag, a tag past the end of the text, or
    /// a part that is only white space.
    pub fn text<'a>(&self, text: &'a str) -> Option<&'a str> {
        if self.content_type == 0 {
            return None;
        }
        let start = self.start as usize;
        let end = start + self.length_marker as usize + 1;
        let mut idx = text
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(text.len()));
        let a = idx.nth(start)?;
        let b = if end == start {
            a
        } else {
            idx.nth(end - start - 1)?
        };
        let s = text[a..b].trim();
        (!s.is_empty()).then_some(s)
    }

    /// The content type's class and label, `None` for a reserved type.
    pub fn info(&self) -> Option<ContentType> {
        content_type::get(self.content_type)
    }
}

/// The item toggle and item running bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Item {
    /// Item toggle.
    pub toggle: bool,
    /// Item running.
    pub running: bool,
}

/// The tags of one command or group (at most 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tags {
    list: [Tag; 4],
    n: usize,
}

impl Tags {
    /// The tags.
    pub fn as_slice(&self) -> &[Tag] {
        &self.list[..self.n]
    }
    fn push(&mut self, t: Tag) {
        if self.n < 4 {
            self.list[self.n] = t;
            self.n += 1;
        }
    }
}

/// Reads the body of a DL Plus command (the bytes after the 2-byte prefix, without the CRC). Only the tags command (CId 0) is
/// read. Other commands give [`Error::Invalid`].
pub fn parse_dl_plus(body: &[u8]) -> Result<(Item, Tags)> {
    let b0 = *body.first().ok_or(Error::Truncated)?;
    if b0 >> 4 != 0 {
        return Err(Error::Invalid(Malformed::DlPlusCommand));
    }
    let item = Item {
        toggle: b0 & 0x08 != 0,
        running: b0 & 0x04 != 0,
    };
    let n = (b0 & 3) as usize + 1;
    let mut tags = Tags::default();
    for k in 0..n {
        let t = body.get(1 + 3 * k..4 + 3 * k).ok_or(Error::Truncated)?;
        tags.push(Tag {
            content_type: t[0] & 0x7F,
            start: t[1] & 0x7F,
            length_marker: t[2] & 0x7F,
        });
    }
    Ok((item, tags))
}

/// Appends the body of a DL Plus tags command (1 to 4 tags).
pub fn write_dl_plus(item: Item, tags: &[Tag], out: &mut Vec<u8>) -> Result<()> {
    if tags.is_empty() || tags.len() > 4 {
        return Err(Error::BadParameter(Parameter::DlPlusTagCount));
    }
    out.push(((item.toggle as u8) << 3) | ((item.running as u8) << 2) | (tags.len() as u8 - 1));
    for t in tags {
        out.extend_from_slice(&[
            t.content_type & 0x7F,
            t.start & 0x7F,
            t.length_marker & 0x7F,
        ]);
    }
    Ok(())
}

/// Reads the two tags of an RT+ group from blocks B, C and D.
pub fn parse_rtplus(b: u16, c: u16, d: u16) -> (Item, Tags) {
    let item = Item {
        toggle: b & 0x10 != 0,
        running: b & 0x08 != 0,
    };
    let mut tags = Tags::default();
    tags.push(Tag {
        content_type: (((b & 7) << 3) | (c >> 13)) as u8,
        start: ((c >> 7) & 0x3F) as u8,
        length_marker: ((c >> 1) & 0x3F) as u8,
    });
    tags.push(Tag {
        content_type: (((c & 1) << 5) | (d >> 11)) as u8,
        start: ((d >> 5) & 0x3F) as u8,
        length_marker: (d & 0x1F) as u8,
    });
    (item, tags)
}

/// The latest text of each content type, for one service.
///
/// For RT+ use [`TagSet::merge_received`] and [`TagSet::refresh`]. They keep the newest tag of each content type and take its
/// text only when every RadioText character it covers has arrived. For DL Plus use [`TagSet::replace`].
///
/// ```
/// use sdrkit_proto_bcdata::tags::{parse_rtplus, TagSet};
///
/// // Artist "ABBA" at 0, title "SOS" at 7, in "ABBA - SOS".
/// let (item, t) = parse_rtplus(0x0008 | (4 >> 3), ((4 & 7) << 13) | (0 << 7) | (3 << 1), (1 << 11) | (7 << 5) | 2);
/// let mut set = TagSet::default();
/// let decode = |b: &[u8], s: &mut String| s.extend(b.iter().map(|&c| c as char));
/// set.merge_received(item, &t, b"ABBA - SOS", |_| true, decode);
/// assert_eq!(set.text(4), Some("ABBA"));
/// assert_eq!(set.text(1), Some("SOS"));
/// ```
#[derive(Clone, Debug, Default)]
pub struct TagSet {
    /// The item bits of the last command or group.
    pub item: Item,
    entries: Vec<(u8, String)>,
    /// The newest RT+ tag of each content type, in content type order.
    specs: Vec<Tag>,
    scratch: String,
}

impl PartialEq for TagSet {
    fn eq(&self, other: &Self) -> bool {
        self.item == other.item && self.entries == other.entries && self.specs == other.specs
    }
}

impl Eq for TagSet {}

impl TagSet {
    /// The content types and texts, in content type order.
    pub fn entries(&self) -> impl Iterator<Item = (u8, &str)> {
        self.entries.iter().map(|(t, s)| (*t, s.as_str()))
    }

    /// The text of a content type.
    pub fn text(&self, content_type: u8) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.0 == content_type)
            .map(|e| e.1.as_str())
    }

    /// Forgets every text and every kept RT+ tag. Call it when the RadioText A/B flag changes.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.specs.clear();
    }

    /// The RT+ tags kept by [`TagSet::merge_received`], one per content type, in content type order.
    pub fn tags(&self) -> &[Tag] {
        &self.specs
    }

    /// Sets the text of a type. It returns true if this changed something.
    fn set(&mut self, t: u8, s: &str) -> bool {
        match self.entries.binary_search_by_key(&t, |e| e.0) {
            Ok(i) => {
                let e = &mut self.entries[i].1;
                if e == s {
                    return false;
                }
                e.clear();
                e.push_str(s);
            }
            Err(i) => self.entries.insert(i, (t, s.to_string())),
        }
        true
    }

    /// RT+: adds the tags of one group and reads the text of every kept tag that the received RadioText covers.
    ///
    /// `rt` is the RadioText buffer as received so far (64 bytes for 2A, 32 for 2B), with any byte where a segment is missing.
    /// `received(i)` says whether byte `i` has arrived. `decode` appends the text of some RadioText bytes (the RDS character
    /// table). A tag of content type 0 is a dummy and is not kept. A new tag of a content type replaces the kept one. A changed
    /// item toggle starts a new item, which clears the texts and the kept tags. Returns true if something changed.
    ///
    /// A tag has a text only when every byte it covers has arrived, the bytes hold no carriage return (0x0D), and the text is not
    /// empty after trimming. A tag past the end of `rt` has none. Until then the type keeps its last text. This is the rule of the
    /// RT+ specification (RDS Forum R06/040_1) that a receiver shows tagged content only from complete RadioText.
    ///
    /// ```
    /// use sdrkit_proto_bcdata::tags::{parse_rtplus, TagSet};
    ///
    /// // 2A RadioText "Hello world\r": segments of 4 characters. Artist at 0 length 5, title at 6 length 5.
    /// let rt = b"Hello world\r";
    /// let (item, tags) = parse_rtplus(0x0008, (4 << 13) | (4 << 1), (1 << 11) | (6 << 5) | 4);
    /// let decode = |b: &[u8], s: &mut String| s.extend(b.iter().map(|&c| c as char));
    /// let mut set = TagSet::default();
    /// set.merge_received(item, &tags, rt, |i| i < 8, decode); // segments 0 and 1 only
    /// assert_eq!((set.text(4), set.text(1)), (Some("Hello"), None));
    /// set.refresh(rt, |_| true, decode); // segment 2 arrives
    /// assert_eq!(set.text(1), Some("world"));
    /// ```
    pub fn merge_received(
        &mut self,
        item: Item,
        tags: &Tags,
        rt: &[u8],
        received: impl Fn(usize) -> bool,
        decode: impl FnMut(&[u8], &mut String),
    ) -> bool {
        let mut changed = item != self.item;
        if item.toggle != self.item.toggle {
            changed |= !self.entries.is_empty();
            self.entries.clear();
            self.specs.clear();
        }
        self.item = item;
        for t in tags.as_slice().iter().filter(|t| t.content_type != 0) {
            match self
                .specs
                .binary_search_by_key(&t.content_type, |x| x.content_type)
            {
                Ok(i) => self.specs[i] = *t,
                Err(i) => self.specs.insert(i, *t),
            }
        }
        changed | self.refresh(rt, received, decode)
    }

    /// RT+: takes again the text of every kept tag, after new RadioText segments arrived. The arguments and the rule are those of
    /// [`TagSet::merge_received`]. It returns true if a text changed.
    pub fn refresh(
        &mut self,
        rt: &[u8],
        received: impl Fn(usize) -> bool,
        mut decode: impl FnMut(&[u8], &mut String),
    ) -> bool {
        let mut changed = false;
        let mut scratch = std::mem::take(&mut self.scratch);
        for k in 0..self.specs.len() {
            let t = self.specs[k];
            let start = t.start as usize;
            let end = start + t.length_marker as usize + 1;
            let Some(bytes) = rt.get(start..end) else {
                continue;
            };
            if !(start..end).all(&received) || bytes.contains(&0x0D) {
                continue;
            }
            scratch.clear();
            decode(bytes, &mut scratch);
            let text = scratch.trim();
            if !text.is_empty() {
                changed |= self.set(t.content_type, text);
            }
        }
        self.scratch = scratch;
        changed
    }

    /// DL Plus: the tags of one command describe the whole label, so they replace the texts. It returns true if something
    /// changed.
    pub fn replace(&mut self, item: Item, tags: &Tags, text: &str) -> bool {
        let mut changed = item != self.item;
        self.item = item;
        let before = self.entries.len();
        self.entries.retain(|e| {
            tags.as_slice()
                .iter()
                .any(|t| t.content_type == e.0 && t.text(text).is_some())
        });
        changed |= self.entries.len() != before;
        for t in tags.as_slice() {
            if let Some(s) = t.text(text) {
                changed |= self.set(t.content_type, s);
            }
        }
        changed
    }
}
