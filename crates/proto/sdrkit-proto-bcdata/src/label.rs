//! The dynamic label: segments, the clear and DL Plus commands, and [`Decoder`], which puts the label of one service
//! together and keeps its DL Plus tags.
//!
//! Standard: ETSI EN 300 401 clause 7.4.5.2, ETSI TS 102 980.

use crate::tags::{self, Item, Tag, TagSet, Tags};
use crate::Malformed;
use crate::Parameter;
use crate::{Error, Result};
use sdrkit_util_bits::crc::catalog::CRC16_GENIBUS;
use sdrkit_util_bits::crc::Trailer;
use sdrkit_util_text::charset_field::{self as text, Charset};

/// Command 0b0001: clear the display.
pub const COMMAND_CLEAR: u8 = 1;
/// Command 0b0010: DL Plus.
pub const COMMAND_DL_PLUS: u8 = 2;

/// The length in bytes of the segment that starts `b` (prefix, data and CRC), from its prefix. `None` if `b` has fewer than 2
/// bytes or the segment is a command this crate does not know.
pub fn segment_len(b: &[u8]) -> Option<usize> {
    let (b0, b1) = (*b.first()?, *b.get(1)?);
    if b0 & 0x10 == 0 {
        return Some(2 + (b0 & 15) as usize + 1 + 2);
    }
    match b0 & 15 {
        COMMAND_CLEAR => Some(4),
        COMMAND_DL_PLUS => Some(2 + (b1 & 15) as usize + 1 + 2),
        _ => None,
    }
}

/// What a segment changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Update {
    /// Nothing visible: a part of a label, a repeat, or a DL Plus command waiting for its label.
    Nothing,
    /// A new or changed label is complete.
    Label,
    /// The display was cleared.
    Cleared,
    /// The DL Plus tags changed.
    Tags,
}

#[derive(Clone, Copy, Debug, Default)]
struct Seg {
    present: bool,
    last: bool,
    len: u8,
    bytes: [u8; 16],
}

/// Puts the dynamic label of one service together.
///
/// ```
/// use sdrkit_proto_bcdata::label::{self, Decoder, Update};
/// use sdrkit_util_text::charset_field::Charset;
///
/// let mut segs = Vec::new();
/// label::write_label("Beyoncé - Halo", Charset::Utf8, false, &mut segs).unwrap();
/// let mut d = Decoder::new();
/// let mut at = 0;
/// let mut last = Update::Nothing;
/// while at < segs.len() {
///     let n = label::segment_len(&segs[at..]).unwrap();
///     last = d.push(&segs[at..at + n]).unwrap();
///     at += n;
/// }
/// assert_eq!((last, d.label()), (Update::Label, "Beyoncé - Halo"));
/// ```
#[derive(Clone, Debug)]
pub struct Decoder {
    segs: [Seg; 8],
    seg_toggle: Option<bool>,
    charset: Charset,
    label: String,
    toggle: bool,
    has_label: bool,
    pending: Option<(bool, [u8; 16], usize)>,
    tags: TagSet,
    count: u64,
    scratch: Vec<u8>,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    /// A decoder with no label.
    pub fn new() -> Self {
        Decoder {
            segs: [Seg::default(); 8],
            seg_toggle: None,
            charset: Charset::EbuLatin,
            label: String::with_capacity(512),
            toggle: false,
            has_label: false,
            pending: None,
            tags: TagSet::default(),
            count: 0,
            scratch: Vec::with_capacity(128),
        }
    }

    /// Forgets the label and the tags (a new service). The count stays.
    pub fn reset(&mut self) {
        let count = self.count;
        let mut label = std::mem::take(&mut self.label);
        label.clear();
        *self = Decoder {
            label,
            count,
            ..Decoder::new()
        };
    }

    /// The label text for display (empty when there is none).
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The toggle bit of the label.
    pub fn toggle(&self) -> bool {
        self.toggle
    }

    /// The character set of the label.
    pub fn charset(&self) -> Charset {
        self.charset
    }

    /// The DL Plus tags of the label.
    pub fn tags(&self) -> &TagSet {
        &self.tags
    }

    /// Labels completed so far.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Takes one whole segment (prefix, data and CRC).
    pub fn push(&mut self, seg: &[u8]) -> Result<Update> {
        let n = segment_len(seg).ok_or(Error::Invalid(Malformed::LabelCommand))?;
        if seg.len() != n {
            return Err(Error::Truncated);
        }
        if !CRC16_GENIBUS.verify(seg, Trailer::Big) {
            return Err(Error::Crc);
        }
        let (b0, b1) = (seg[0], seg[1]);
        let toggle = b0 & 0x80 != 0;
        let data = &seg[2..n - 2];
        if b0 & 0x10 != 0 {
            return Ok(match b0 & 15 {
                COMMAND_CLEAR => {
                    self.label.clear();
                    self.has_label = false;
                    self.segs = [Seg::default(); 8];
                    self.seg_toggle = None;
                    self.tags.clear();
                    Update::Cleared
                }
                _ => {
                    let mut body = [0u8; 16];
                    body[..data.len()].copy_from_slice(data);
                    self.dl_plus(b1 & 0x80 != 0, body, data.len())
                }
            });
        }
        let first = b0 & 0x40 != 0;
        let number = if first { 0 } else { ((b1 >> 4) & 7) as usize };
        let mut s = Seg {
            present: true,
            last: b0 & 0x20 != 0,
            len: data.len() as u8,
            bytes: [0; 16],
        };
        s.bytes[..data.len()].copy_from_slice(data);
        let changed_first = first
            && self.segs[0].present
            && self.segs[0].bytes[..self.segs[0].len as usize] != s.bytes[..s.len as usize];
        if self.seg_toggle != Some(toggle) || changed_first {
            self.segs = [Seg::default(); 8];
        }
        self.seg_toggle = Some(toggle);
        if first {
            self.charset = Charset::from_code(b1 >> 4);
        }
        self.segs[number] = s;
        // Complete when segments 0 to the last are present and none beyond.
        let Some(last) = self.segs.iter().position(|x| x.present && x.last) else {
            return Ok(Update::Nothing);
        };
        if self.segs[..=last].iter().any(|x| !x.present)
            || self.segs[last + 1..].iter().any(|x| x.present)
        {
            return Ok(Update::Nothing);
        }
        self.scratch.clear();
        for s in &self.segs[..=last] {
            self.scratch.extend_from_slice(&s.bytes[..s.len as usize]);
        }
        let same = self.has_label && toggle == self.toggle && {
            let old_len = self.label.len();
            text::display_into(&self.scratch, self.charset, &mut self.label);
            let same = self.label[..old_len] == self.label[old_len..];
            self.label.truncate(old_len);
            same
        };
        if same {
            return Ok(Update::Nothing);
        }
        self.label.clear();
        text::display_into(&self.scratch, self.charset, &mut self.label);
        self.toggle = toggle;
        self.has_label = true;
        self.tags.clear();
        self.count += 1;
        if let Some((link, body, len)) = self.pending.take() {
            self.dl_plus(link, body, len);
        }
        Ok(Update::Label)
    }

    fn dl_plus(&mut self, link: bool, body: [u8; 16], len: usize) -> Update {
        let Ok((item, tags)) = tags::parse_dl_plus(&body[..len]) else {
            return Update::Nothing;
        };
        if !self.has_label || link != self.toggle {
            self.pending = Some((link, body, len));
            return Update::Nothing;
        }
        if self.tags.replace(item, &tags, &self.label) {
            Update::Tags
        } else {
            Update::Nothing
        }
    }
}

fn finish(start: usize, out: &mut Vec<u8>) {
    let c = CRC16_GENIBUS.checksum(&out[start..]);
    out.extend_from_slice(&c.to_be_bytes());
}

/// Appends the segments of a label (at most 8 segments of 16 bytes, cut at character boundaries).
pub fn write_label(label: &str, charset: Charset, toggle: bool, out: &mut Vec<u8>) -> Result<()> {
    let mut bytes = Vec::with_capacity(label.len());
    text::encode_into(label, charset, &mut bytes);
    // Cut points that do not split a character.
    let mut cuts = vec![0usize];
    let unit = |i: usize| match charset {
        Charset::Ucs2 => i.is_multiple_of(2),
        Charset::Utf8 => bytes.get(i).is_none_or(|&b| b & 0xC0 != 0x80),
        _ => true,
    };
    while *cuts.last().unwrap_or(&0) < bytes.len() {
        let from = *cuts.last().unwrap_or(&0);
        let mut to = (from + 16).min(bytes.len());
        while to > from + 1 && !unit(to) {
            to -= 1;
        }
        cuts.push(to);
    }
    let n = cuts.len() - 1;
    if n == 0 || n > 8 {
        return Err(Error::BadParameter(Parameter::LabelLength));
    }
    for k in 0..n {
        let part = &bytes[cuts[k]..cuts[k + 1]];
        let start = out.len();
        out.push(
            ((toggle as u8) << 7)
                | (((k == 0) as u8) << 6)
                | (((k + 1 == n) as u8) << 5)
                | (part.len() as u8 - 1),
        );
        out.push(if k == 0 {
            charset.code() << 4
        } else {
            (k as u8) << 4
        });
        out.extend_from_slice(part);
        finish(start, out);
    }
    Ok(())
}

/// Appends a "clear display" command.
pub fn write_clear(toggle: bool, out: &mut Vec<u8>) {
    let start = out.len();
    out.extend_from_slice(&[((toggle as u8) << 7) | 0x70 | COMMAND_CLEAR, 0]);
    finish(start, out);
}

/// Appends a DL Plus tags command for the label with toggle `link`.
pub fn write_dl_plus(
    toggle: bool,
    link: bool,
    item: Item,
    tags: &[Tag],
    out: &mut Vec<u8>,
) -> Result<()> {
    let mut body = Vec::with_capacity(13);
    tags::write_dl_plus(item, tags, &mut body)?;
    let start = out.len();
    out.extend_from_slice(&[
        ((toggle as u8) << 7) | 0x70 | COMMAND_DL_PLUS,
        ((link as u8) << 7) | (body.len() as u8 - 1),
    ]);
    out.extend_from_slice(&body);
    finish(start, out);
    Ok(())
}

/// The tags of a DL Plus command, to read one without a [`Decoder`].
pub fn parse_dl_plus_command(seg: &[u8]) -> Result<(bool, Item, Tags)> {
    let n = segment_len(seg).ok_or(Error::Invalid(Malformed::LabelCommand))?;
    if seg.len() != n || seg[0] & 0x1F != 0x10 | COMMAND_DL_PLUS {
        return Err(Error::Invalid(Malformed::NotDlPlus));
    }
    if !CRC16_GENIBUS.verify(seg, Trailer::Big) {
        return Err(Error::Crc);
    }
    let (item, t) = tags::parse_dl_plus(&seg[2..n - 2])?;
    Ok((seg[1] & 0x80 != 0, item, t))
}
