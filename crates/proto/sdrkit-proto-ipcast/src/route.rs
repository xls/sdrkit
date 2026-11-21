//! ROUTE delivery objects made whole from source packets ([`RouteAssembler`]), and the MIME multipart package that carries
//! the service layer signalling ([`mime_parts`], [`mime_package`]).
//!
//! Standard: RFC 9223 clauses 9.2 and 9.3, ATSC A/331 annex A, RFC 2046 clause 5.1, RFC 2387.

use crate::error::{Error, Result};
use crate::error::{Limit, Malformed};
use crate::lct::Lct;
use std::net::{IpAddr, Ipv4Addr};

/// The destination address and UDP port of a ROUTE session's datagrams. A TSI names a session only together with them.
pub type Destination = (IpAddr, u16);

/// The destination of no session.
const NO_DESTINATION: Destination = (IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);

/// One whole object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteObject<'a> {
    /// The destination of its session's datagrams.
    pub dst: Destination,
    /// The TSI of its session.
    pub tsi: u64,
    /// The TOI.
    pub toi: u64,
    /// The codepoint of its packets.
    pub codepoint: u8,
    /// The object.
    pub data: &'a [u8],
}

/// Counters of the assembler.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RouteStats {
    /// Source packets taken.
    pub packets: u64,
    /// Repair packets (not used: this receiver does no FEC).
    pub repair: u64,
    /// Objects made whole.
    pub objects: u64,
    /// Objects dropped: too large, timed out, or pushed out when every buffer was busy.
    pub dropped: u64,
    /// Packets refused (bad header, data past the object length).
    pub refused: u64,
}

/// The most byte ranges an object in assembly keeps apart. More gaps than this drop the object.
const RANGES: usize = 64;

#[derive(Clone, Debug)]
struct Slot {
    active: bool,
    dst: Destination,
    tsi: u64,
    toi: u64,
    codepoint: u8,
    started: f64,
    data: Vec<u8>,
    /// Received byte ranges, sorted and merged.
    ranges: [(u64, u64); RANGES],
    nranges: usize,
    len: Option<u64>,
}

/// Makes ROUTE objects whole, with up to `slots` objects in assembly at a time. An object longer than `max_object` bytes or
/// older than `timeout_s` seconds of signal time is dropped. Buffers grow to the largest object seen, then are reused.
#[derive(Clone, Debug)]
pub struct RouteAssembler {
    slots: Vec<Slot>,
    max_object: u64,
    timeout_s: f64,
    /// The counters.
    pub stats: RouteStats,
}

impl Default for RouteAssembler {
    fn default() -> Self {
        Self::new(8, 16 << 20, 30.0)
    }
}

impl RouteAssembler {
    /// An assembler with `slots` buffers (at least 1), objects up to `max_object` bytes and a timeout of `timeout_s` seconds.
    pub fn new(slots: usize, max_object: u64, timeout_s: f64) -> RouteAssembler {
        RouteAssembler {
            slots: (0..slots.max(1))
                .map(|_| Slot {
                    active: false,
                    dst: NO_DESTINATION,
                    tsi: 0,
                    toi: 0,
                    codepoint: 0,
                    started: 0.0,
                    data: Vec::new(),
                    ranges: [(0, 0); RANGES],
                    nranges: 0,
                    len: None,
                })
                .collect(),
            max_object,
            timeout_s,
            stats: RouteStats::default(),
        }
    }

    /// Takes one ROUTE packet (a UDP payload sent to `dst`) at signal time `now`. Objects are kept apart by destination, TSI
    /// and TOI. A whole object goes to `out`. A bad packet returns an error.
    pub fn push(
        &mut self,
        p: &[u8],
        dst: Destination,
        now: f64,
        out: &mut impl FnMut(RouteObject<'_>),
    ) -> Result<()> {
        let h = match Lct::parse(p) {
            Ok(h) => h,
            Err(e) => {
                self.stats.refused += 1;
                return Err(e);
            }
        };
        if !h.source {
            self.stats.repair += 1;
            return Ok(());
        }
        let Some(so) = p.get(h.len..h.len + 4) else {
            self.stats.refused += 1;
            return Err(Error::Short);
        };
        let offset = u32::from_be_bytes([so[0], so[1], so[2], so[3]]) as u64;
        let data = &p[h.len + 4..];
        let end = offset + data.len() as u64;
        self.stats.packets += 1;
        for s in &mut self.slots {
            if s.active && now - s.started > self.timeout_s {
                s.active = false;
                self.stats.dropped += 1;
            }
        }
        let k = self.slot(dst, h.tsi, h.toi, h.codepoint, now);
        let s = &mut self.slots[k];
        if let Some(t) = h.transfer_len {
            s.len = Some(t);
        } else if h.close_object && s.len.is_none() {
            s.len = Some(end);
        }
        let limit = s.len.unwrap_or(self.max_object).min(self.max_object);
        if end > limit || s.len.is_some_and(|l| l > self.max_object) {
            s.active = false;
            self.stats.dropped += 1;
            return Err(Error::TooLarge(Limit::RouteObject));
        }
        if (s.data.len() as u64) < end {
            s.data.resize(end as usize, 0);
        }
        s.data[offset as usize..end as usize].copy_from_slice(data);
        if !add_range(&mut s.ranges, &mut s.nranges, offset, end) {
            s.active = false;
            self.stats.dropped += 1;
            return Ok(());
        }
        let Some(len) = s.len else {
            return Ok(());
        };
        if !(s.nranges == 1 && s.ranges[0] == (0, len)) && len != 0 {
            return Ok(());
        }
        s.active = false;
        self.stats.objects += 1;
        out(RouteObject {
            dst: s.dst,
            tsi: s.tsi,
            toi: s.toi,
            codepoint: s.codepoint,
            data: &s.data[..len as usize],
        });
        Ok(())
    }

    fn slot(&mut self, dst: Destination, tsi: u64, toi: u64, codepoint: u8, now: f64) -> usize {
        if let Some(k) = self
            .slots
            .iter()
            .position(|s| s.active && s.dst == dst && s.tsi == tsi && s.toi == toi)
        {
            return k;
        }
        let k = match self.slots.iter().position(|s| !s.active) {
            Some(k) => k,
            None => {
                self.stats.dropped += 1;
                let mut k = 0;
                for (i, s) in self.slots.iter().enumerate() {
                    if s.started < self.slots[k].started {
                        k = i;
                    }
                }
                k
            }
        };
        let s = &mut self.slots[k];
        s.active = true;
        s.dst = dst;
        s.tsi = tsi;
        s.toi = toi;
        s.codepoint = codepoint;
        s.started = now;
        s.nranges = 0;
        s.len = None;
        s.data.clear();
        k
    }

    /// Objects in assembly now.
    pub fn open(&self) -> usize {
        self.slots.iter().filter(|s| s.active).count()
    }
}

/// Adds [a, b) to the sorted, merged ranges. Returns false when there is no room for another gap.
fn add_range(r: &mut [(u64, u64); RANGES], n: &mut usize, a: u64, b: u64) -> bool {
    if a == b {
        return true;
    }
    // Merge every range that touches [a, b) into it.
    let (mut lo, mut hi) = (a, b);
    let mut placed = false;
    let mut tmp = [(0u64, 0u64); RANGES + 1];
    let mut t = 0;
    for &(x, y) in r.iter().take(*n) {
        if y < lo {
            tmp[t] = (x, y);
            t += 1;
        } else if x > hi {
            if !placed {
                tmp[t] = (lo, hi);
                t += 1;
                placed = true;
            }
            tmp[t] = (x, y);
            t += 1;
        } else {
            lo = lo.min(x);
            hi = hi.max(y);
        }
    }
    if !placed {
        tmp[t] = (lo, hi);
        t += 1;
    }
    if t > RANGES {
        return false;
    }
    r[..t].copy_from_slice(&tmp[..t]);
    *n = t;
    true
}

/// One part of a MIME multipart package.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MimePart<'a> {
    /// The Content-Type header value (empty when absent).
    pub content_type: &'a str,
    /// The Content-Location header value (empty when absent).
    pub content_location: &'a str,
    /// The Content-Encoding header value (empty when absent).
    pub content_encoding: &'a str,
    /// The body.
    pub body: &'a [u8],
}

/// The boundary parameter of a `multipart/...` Content-Type value (1 to 70 characters, quotes taken off).
pub fn mime_boundary(content_type: &str) -> Option<&str> {
    content_type
        .split(';')
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            k.trim().eq_ignore_ascii_case("boundary").then_some(v)
        })
        .next()
        .map(|b| b.trim().trim_matches('"'))
        .filter(|b| !b.is_empty() && b.len() <= 70)
}

/// The boundary of a package that starts with its delimiter line: the rest of the first line after "--", trimmed.
fn first_line_boundary(object: &[u8]) -> Option<&str> {
    let line = object.strip_prefix(b"--")?;
    let end = line.iter().position(|&c| c == b'\n').unwrap_or(line.len());
    std::str::from_utf8(&line[..end])
        .ok()
        .map(str::trim)
        .filter(|b| !b.is_empty() && b.len() <= 70)
}

/// Calls `each` for every part of a multipart MIME object and returns their number (RFC 2046 clause 5.1.1). The object is
/// either a header block, a blank line, then parts between "--boundary" lines, with the boundary from its Content-Type header;
/// or a package without the header block, which starts with its delimiter line: the boundary is then the rest of that first
/// line. A part that is itself a package is read with [`mime_package`]. It does not allocate.
pub fn mime_parts<'a>(object: &'a [u8], each: impl FnMut(MimePart<'a>)) -> Result<usize> {
    let object = object.trim_ascii_start();
    if object.starts_with(b"--") {
        let boundary =
            first_line_boundary(object).ok_or(Error::Invalid(Malformed::MimeBoundary))?;
        return Ok(parts(object, boundary, each));
    }
    let (headers, body) = split_headers(object).ok_or(Error::Invalid(Malformed::MimeHeaders))?;
    let ct = header(headers, "content-type").ok_or(Error::Invalid(Malformed::MimeContentType))?;
    mime_package(ct, body, each)
}

/// Calls `each` for every part of the package `body` whose Content-Type value is `content_type` (a part of an outer package
/// that is itself `multipart/...`), and returns their number. Without a boundary in `content_type` the body must start with
/// its delimiter line. It does not allocate.
pub fn mime_package<'a>(
    content_type: &str,
    body: &'a [u8],
    each: impl FnMut(MimePart<'a>),
) -> Result<usize> {
    let boundary = mime_boundary(content_type)
        .or_else(|| first_line_boundary(body.trim_ascii_start()))
        .ok_or(Error::Invalid(Malformed::MimeBoundary))?;
    // The boundary may live in `content_type` only as long as this call: it is compared, not kept.
    Ok(parts(body, boundary, each))
}

/// The parts of `body` between the delimiter lines of `boundary`, up to the closing delimiter or the end.
fn parts<'a>(body: &'a [u8], boundary: &str, mut each: impl FnMut(MimePart<'a>)) -> usize {
    let mut count = 0;
    // Skip the preamble to the first delimiter.
    let Some(i) = find_delim(body, boundary) else {
        return 0;
    };
    let mut rest = &body[i..];
    loop {
        // `rest` starts at "--boundary".
        let after = &rest[2 + boundary.len()..];
        if after.starts_with(b"--") {
            return count;
        }
        // The rest of the delimiter line (padding) belongs to the delimiter.
        let Some(nl) = after.iter().position(|&c| c == b'\n') else {
            return count;
        };
        let after = &after[nl + 1..];
        let Some(j) = find_delim(after, boundary) else {
            return count;
        };
        let mut part = &after[..j];
        // The CRLF before the delimiter belongs to the delimiter.
        part = part
            .strip_suffix(b"\r\n")
            .or_else(|| part.strip_suffix(b"\n"))
            .unwrap_or(part);
        // A part without headers starts with the blank line.
        let (ph, pb) = if let Some(b) = part
            .strip_prefix(b"\r\n")
            .or_else(|| part.strip_prefix(b"\n"))
        {
            (&[][..], b)
        } else {
            split_headers(part).unwrap_or((&[], part))
        };
        each(MimePart {
            content_type: header(ph, "content-type").unwrap_or(""),
            content_location: header(ph, "content-location").unwrap_or(""),
            content_encoding: header(ph, "content-encoding").unwrap_or(""),
            body: pb,
        });
        count += 1;
        rest = &after[j..];
    }
}

/// The index of the next "--boundary" at the start of a line.
fn find_delim(b: &[u8], boundary: &str) -> Option<usize> {
    let n = boundary.len() + 2;
    (0..b.len().checked_sub(n - 1)?).find(|&i| {
        (i == 0 || b[i - 1] == b'\n')
            && b[i] == b'-'
            && b[i + 1] == b'-'
            && &b[i + 2..i + n] == boundary.as_bytes()
    })
}

/// Splits headers from the body at the first blank line.
fn split_headers(b: &[u8]) -> Option<(&[u8], &[u8])> {
    for i in 0..b.len() {
        if b[i..].starts_with(b"\r\n\r\n") {
            return Some((&b[..i], &b[i + 4..]));
        }
        if b[i..].starts_with(b"\n\n") {
            return Some((&b[..i], &b[i + 2..]));
        }
    }
    None
}

/// The value of header `name` (no case), trimmed.
fn header<'a>(headers: &'a [u8], name: &str) -> Option<&'a str> {
    let text = std::str::from_utf8(headers).ok()?;
    text.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}
