//! The service layer signalling (SLS) of a ROUTE service: the documents of one delivery object on TSI 0, in every form the
//! object can have ([`read_sls`]).
//!
//! Standard: ATSC A/331 clause 7.1.6.1 and annex A.3.3.4, A.3.3.5; RFC 2046 clause 5.1; RFC 1952.

use crate::gzip;
use crate::route::{mime_package, mime_parts, MimePart};

/// Packages inside packages are read to this depth.
pub const SLS_MAX_DEPTH: usize = 4;
/// The largest object or part taken after gunzip.
pub const SLS_MAX_BYTES: usize = 4 << 20;

/// What a signalling document is, from its XML root element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlsKind {
    /// `UserServiceDescription` or `BundleDescription`: the user service bundle description.
    Usbd,
    /// `S-TSID`: the service-based transport session instance description.
    Stsid,
    /// `MPD`: the media presentation description.
    Mpd,
    /// Another root element.
    Other,
}

impl SlsKind {
    /// The kind of the root element `name` (without a namespace prefix).
    pub fn of(name: &str) -> SlsKind {
        match name {
            "UserServiceDescription" | "BundleDescription" => SlsKind::Usbd,
            "S-TSID" => SlsKind::Stsid,
            "MPD" => SlsKind::Mpd,
            _ => SlsKind::Other,
        }
    }
}

/// One signalling document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlsDocument<'a> {
    /// What it is.
    pub kind: SlsKind,
    /// The Content-Type of its part (empty for a bare document or when absent).
    pub content_type: &'a str,
    /// The Content-Location of its part (empty for a bare document or when absent).
    pub content_location: &'a str,
    /// The XML.
    pub xml: &'a [u8],
}

/// What [`read_sls`] found in one object.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlsRead {
    /// Documents read (XML with a root element).
    pub documents: usize,
    /// The object is, or holds, a `multipart/signed` package. The signature is not verified.
    pub signed: bool,
    /// Gzip members that did not inflate (a broken member, a wrong CRC, or more than [`SLS_MAX_BYTES`]): not read.
    pub broken_gzip: usize,
    /// Parts not read: no XML root element, a package without a boundary, or a package deeper than [`SLS_MAX_DEPTH`].
    pub refused: usize,
}

/// The name of the root element of an XML document, without its namespace prefix: the first element after a byte order mark,
/// white space, the XML declaration, processing instructions, comments and a document type. `None` when there is none.
pub fn xml_root(xml: &[u8]) -> Option<&str> {
    let mut b = xml.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(xml);
    loop {
        b = b.trim_ascii_start();
        let skip_to = |b: &[u8], end: &[u8]| -> Option<usize> {
            b.windows(end.len())
                .position(|w| w == end)
                .map(|i| i + end.len())
        };
        if b.starts_with(b"<?") {
            b = &b[skip_to(b, b"?>")?..];
        } else if b.starts_with(b"<!--") {
            b = &b[skip_to(b, b"-->")?..];
        } else if b.starts_with(b"<!") {
            b = &b[skip_to(b, b">")?..];
        } else {
            break;
        }
    }
    let b = b.strip_prefix(b"<")?;
    let end = b
        .iter()
        .position(|c| c.is_ascii_whitespace() || *c == b'>' || *c == b'/')?;
    let name = std::str::from_utf8(&b[..end]).ok()?;
    let name = name.rsplit(':').next()?;
    let first = name.chars().next()?;
    (first.is_alphabetic() || first == '_').then_some(name)
}

fn is_gzip(b: &[u8]) -> bool {
    b.len() >= 18 && b[0] == 0x1F && b[1] == 0x8B
}

fn is_multipart(content_type: &str) -> bool {
    content_type
        .trim_start()
        .get(..10)
        .is_some_and(|t| t.eq_ignore_ascii_case("multipart/"))
}

/// The type names a signature part of a signed package.
fn is_signature(content_type: &str) -> bool {
    let t = content_type.to_ascii_lowercase();
    t.contains("pkcs7") || t.contains("signature")
}

struct Reader<'e> {
    read: SlsRead,
    each: &'e mut dyn FnMut(SlsDocument<'_>),
}

impl Reader<'_> {
    /// Gunzips `b` when it is a gzip member (or `encoded` says so). `Err` when it does not inflate.
    fn inflate(&mut self, b: &[u8], encoded: bool) -> Result<Option<Vec<u8>>, ()> {
        if !(encoded || is_gzip(b)) {
            return Ok(None);
        }
        match gzip::decompress(b, SLS_MAX_BYTES) {
            Ok(v) => Ok(Some(v)),
            Err(_) => {
                self.read.broken_gzip += 1;
                Err(())
            }
        }
    }

    /// One document.
    fn document(&mut self, part: &MimePart<'_>) {
        match xml_root(part.body) {
            Some(root) => {
                self.read.documents += 1;
                (self.each)(SlsDocument {
                    kind: SlsKind::of(root),
                    content_type: part.content_type,
                    content_location: part.content_location,
                    xml: part.body,
                });
            }
            None => self.read.refused += 1,
        }
    }

    /// A part of a package (or the object itself, with empty headers) at `depth` packages down: gunzipped when compressed, then a
    /// package again or a document.
    fn part(&mut self, part: MimePart<'_>, depth: usize) {
        let gz = part.content_encoding.trim().eq_ignore_ascii_case("gzip");
        let Ok(owned) = self.inflate(part.body, gz) else {
            return;
        };
        let body = owned.as_deref().unwrap_or(part.body);
        let part = MimePart { body, ..part };
        let start = body.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(body);
        let start = start.trim_ascii_start();
        let multipart = is_multipart(part.content_type);
        if multipart || start.starts_with(b"-") {
            self.package(part.content_type, body, depth + 1);
        } else if depth == 0 && !start.starts_with(b"<") {
            // The object itself with a header block in front (form 1 or 4).
            self.package("", body, 1);
        } else {
            self.document(&part);
        }
    }

    /// A package at `depth`: `content_type` is its Content-Type value when it is a part of an outer package, empty when `body`
    /// is a whole object (with its header block, or starting with its delimiter line).
    fn package(&mut self, content_type: &str, body: &[u8], depth: usize) {
        if depth > SLS_MAX_DEPTH {
            self.read.refused += 1;
            return;
        }
        let mut signed = content_type
            .to_ascii_lowercase()
            .contains("multipart/signed");
        if content_type.is_empty() {
            // The type of a whole object is in its header block, when it has one.
            let head = body.trim_ascii_start();
            if !head.starts_with(b"--") {
                let end = head
                    .windows(2)
                    .position(|w| w == b"\n\n" || w == b"\n\r")
                    .unwrap_or(head.len());
                signed = head[..end]
                    .to_ascii_lowercase()
                    .windows(16)
                    .any(|w| w == b"multipart/signed");
            }
        }
        self.read.signed |= signed;
        let mut inner = |p: MimePart<'_>| {
            if signed && is_signature(p.content_type) {
                return;
            }
            self.part(p, depth);
        };
        let r = if content_type.is_empty() {
            mime_parts(body, &mut inner)
        } else {
            mime_package(content_type, body, &mut inner)
        };
        if r.is_err() {
            self.read.refused += 1;
        }
    }
}

/// Reads the signalling documents of one SLS delivery object and calls `each` for every one. The object is any of:
///
/// 1. a `multipart/related` package: a header block with its Content-Type and boundary, a blank line, the parts;
/// 2. the same without the header block: it starts with its delimiter line, whose rest is the boundary;
/// 3. either one gzip-compressed (it starts with the gzip magic and has 18 bytes or more): gunzipped, then read. A member
///    that does not inflate is counted and not read;
/// 4. a `multipart/signed` package whose first part is the `multipart/related` package and whose signature part (a type with
///    "pkcs7" or "signature") is skipped. The signature is not verified;
/// 5. a bare XML document.
///
/// A part with `Content-Encoding: gzip`, or whose body starts with the gzip magic, is gunzipped; a part whose type is
/// `multipart/...`, or whose body then starts with "-", is read as a package again, to [`SLS_MAX_DEPTH`] packages deep.
pub fn read_sls(object: &[u8], each: &mut dyn FnMut(SlsDocument<'_>)) -> SlsRead {
    let mut r = Reader {
        read: SlsRead::default(),
        each,
    };
    if object.trim_ascii_start().is_empty() {
        return r.read;
    }
    r.part(
        MimePart {
            content_type: "",
            content_location: "",
            content_encoding: "",
            body: object,
        },
        0,
    );
    r.read
}
