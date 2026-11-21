//! Journaline news pages: NML objects and their [`Store`].
//!
//! Standard: ETSI TS 102 979.

use crate::Malformed;
use crate::{Error, Result};
use std::collections::BTreeMap;

/// The object type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Kind {
    /// 1: a menu of links.
    #[default]
    Menu,
    /// 2: a plain text message.
    Text,
    /// 3: a title-only message.
    Title,
    /// 4: a list message.
    List,
}

impl Kind {
    const fn code(self) -> u8 {
        match self {
            Kind::Menu => 1,
            Kind::Text => 2,
            Kind::Title => 3,
            Kind::List => 4,
        }
    }
}

/// A decoded NML object.
///
/// ```
/// use sdrkit_proto_bcdata::journaline::{decode, encode, Kind, Object};
///
/// let menu = Object { id: 0, kind: Kind::Menu, title: "News".into(), links: vec![(0x10, "Sport".into())], ..Object::default() };
/// let mut b = Vec::new();
/// encode(&menu, true, &mut b);
/// assert_eq!(decode(&b, 0).unwrap(), menu);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Object {
    /// Object id.
    pub id: u16,
    /// Object type.
    pub kind: Kind,
    /// Static flag.
    pub is_static: bool,
    /// Revision index (0 to 7).
    pub revision: u8,
    /// Title.
    pub title: String,
    /// Menu items: linked object id and text.
    pub links: Vec<(u16, String)>,
    /// Message body.
    pub body: String,
    /// List items. Columns are separated by a tab.
    pub items: Vec<String>,
}

/// Appends the text of one section, with the text controls of [`decode`] applied.
fn text(b: &[u8], out: &mut String) {
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            0x10 => v.push(b'\n'),
            0x11..=0x14 => {}
            0x1A | 0x1B => i += b.get(i + 1).map_or(1, |&l| l as usize + 2),
            0x05 => v.push(b'\t'),
            c if c < 0x20 => {}
            c => v.push(c),
        }
        i += 1;
    }
    let start = out.len();
    sdrkit_util_text::charset_field::decode_into(
        &v,
        sdrkit_util_text::charset_field::Charset::Utf8,
        out,
    );
    let t = out[start..].trim().len();
    let lead = out[start..].len() - out[start..].trim_start().len();
    out.drain(start..start + lead);
    out.truncate(start + t);
}

fn inflate(b: &[u8]) -> Result<Vec<u8>> {
    let zlib =
        b.len() >= 2 && b[0] & 0x0F == 8 && (b[0] as u16 * 256 + b[1] as u16).is_multiple_of(31);
    let r = if zlib {
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(b, 1 << 20)
    } else {
        miniz_oxide::inflate::decompress_to_vec_with_limit(b, 1 << 20)
    };
    r.map_err(|_| Error::Invalid(Malformed::JournalineCompressed))
}

/// Decodes an NML object (the data field of its data group). `ext_header` is the extended header length. In text, 0x10 is a
/// line break, 0x11 to 0x14 are dropped, and a data section (0x1A or 0x1B, a length byte, then length + 1 bytes) is skipped.
/// Compressed content is a zlib stream when it starts with a zlib header, else raw deflate.
pub fn decode(b: &[u8], ext_header: usize) -> Result<Object> {
    if b.len() < 3 + ext_header {
        return Err(Error::Truncated);
    }
    let kind = match b[2] >> 5 {
        1 => Kind::Menu,
        2 => Kind::Text,
        3 => Kind::Title,
        4 => Kind::List,
        _ => return Err(Error::Invalid(Malformed::JournalineObjectType)),
    };
    let mut o = Object {
        id: u16::from_be_bytes([b[0], b[1]]),
        kind,
        is_static: b[2] & 0x10 != 0,
        revision: b[2] & 7,
        ..Object::default()
    };
    let raw = &b[3 + ext_header..];
    let inflated;
    let content: &[u8] = if b[2] & 0x08 != 0 {
        inflated = inflate(raw)?;
        &inflated
    } else {
        raw
    };
    // Cut the content into sections at each code byte (0x00 to 0x0F, but 0x05 is a column separator).
    let mut k = 0;
    while k < content.len() {
        let code = content[k];
        k += 1;
        let link = if code == 0x02 && kind == Kind::Menu {
            let l = content.get(k..k + 2).ok_or(Error::Truncated)?;
            k += 2;
            Some(u16::from_be_bytes([l[0], l[1]]))
        } else {
            None
        };
        let start = k;
        while k < content.len() {
            let c = content[k];
            if c <= 0x0F && c != 0x05 {
                break;
            }
            if c == 0x1A || c == 0x1B {
                k += content.get(k + 1).map_or(1, |&l| l as usize + 2);
            }
            k += 1;
        }
        let section = &content[start..k.min(content.len())];
        match code {
            0x00 => break,
            0x01 => text(section, &mut o.title),
            0x02 => {
                let mut s = String::new();
                text(section, &mut s);
                match link {
                    Some(l) => o.links.push((l, s)),
                    None => o.items.push(s),
                }
            }
            0x03 => text(section, &mut o.body),
            0x04 => {
                let mut s = String::new();
                text(section, &mut s);
                o.items.push(s);
            }
            _ => {}
        }
    }
    Ok(o)
}

/// Appends an NML object with no extended header, compressed (raw deflate) or not.
pub fn encode(o: &Object, compress: bool, out: &mut Vec<u8>) {
    let mut content = vec![0x01];
    let put = |s: &str, c: &mut Vec<u8>| {
        c.extend(s.bytes().map(|b| match b {
            b'\n' => 0x10,
            b'\t' => 0x05,
            b => b,
        }))
    };
    put(&o.title, &mut content);
    for (l, s) in &o.links {
        content.push(0x02);
        content.extend_from_slice(&l.to_be_bytes());
        put(s, &mut content);
    }
    if !o.body.is_empty() {
        content.push(0x03);
        put(&o.body, &mut content);
    }
    for s in &o.items {
        content.push(0x04);
        put(s, &mut content);
    }
    content.push(0x00);
    out.extend_from_slice(&o.id.to_be_bytes());
    out.push(
        (o.kind.code() << 5)
            | ((o.is_static as u8) << 4)
            | ((compress as u8) << 3)
            | (o.revision & 7),
    );
    if compress {
        out.extend(miniz_oxide::deflate::compress_to_vec(&content, 9));
    } else {
        out.extend(content);
    }
}

/// The objects of one Journaline service, by object id. Object 0 is the main menu.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Store {
    /// The objects.
    pub objects: BTreeMap<u16, Object>,
    /// Objects received, repeats included.
    pub received: u64,
}

impl Store {
    /// The most objects kept. When full, a new object pushes out the one with the highest id (menus near the root stay).
    pub const MAX: usize = 512;

    /// Adds an object. It returns false for a repeat of the same revision.
    pub fn add(&mut self, o: Object) -> bool {
        self.received += 1;
        if self.objects.get(&o.id).is_some_and(|x| *x == o) {
            return false;
        }
        if self.objects.len() >= Self::MAX && !self.objects.contains_key(&o.id) {
            match self.objects.keys().next_back() {
                Some(&k) if k > o.id => {
                    self.objects.remove(&k);
                }
                _ => return false,
            }
        }
        self.objects.insert(o.id, o);
        true
    }
}
