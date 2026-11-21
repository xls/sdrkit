//! Service and programme information in its binary form: the element tree ([`decode`]) and the [`Guide`] of services and
//! programmes. DAB and DRM carry it as MOT objects.
//!
//! Standard: ETSI TS 102 371, ETSI TS 102 818.

use crate::Malformed;
use crate::{Error, Result};

/// One element: its tag, attributes, character data and children.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Element {
    /// The element tag.
    pub tag: u8,
    /// Attributes: tag and data.
    pub attrs: Vec<(u8, Vec<u8>)>,
    /// Character data, with tokens put back.
    pub text: String,
    /// Child elements.
    pub children: Vec<Element>,
}

impl Element {
    /// The first child with `tag`.
    pub fn child(&self, tag: u8) -> Option<&Element> {
        self.children.iter().find(|c| c.tag == tag)
    }
    /// The children with `tag`.
    pub fn children_with(&self, tag: u8) -> impl Iterator<Item = &Element> {
        self.children.iter().filter(move |c| c.tag == tag)
    }
    /// The data of attribute `tag`.
    pub fn attr(&self, tag: u8) -> Option<&[u8]> {
        self.attrs
            .iter()
            .find(|a| a.0 == tag)
            .map(|a| a.1.as_slice())
    }
}

/// The most nested elements [`decode`] reads.
pub const MAX_DEPTH: usize = 32;

fn length(b: &[u8], i: &mut usize) -> Result<usize> {
    let l = *b.get(*i).ok_or(Error::Truncated)? as usize;
    *i += 1;
    let n = match l {
        0xFE => 2,
        0xFF => 3,
        _ => return Ok(l),
    };
    let d = b.get(*i..*i + n).ok_or(Error::Truncated)?;
    *i += n;
    Ok(d.iter().fold(0, |a, &x| (a << 8) | x as usize))
}

type Tokens = [Vec<u8>; 20];

fn expand(s: &[u8], tokens: &Tokens, out: &mut String) {
    let mut v = Vec::with_capacity(s.len());
    for &c in s {
        match tokens.get(c as usize) {
            Some(t) if !t.is_empty() => v.extend_from_slice(t),
            _ => v.push(c),
        }
    }
    sdrkit_util_text::charset_field::display_into(
        &v,
        sdrkit_util_text::charset_field::Charset::Utf8,
        out,
    );
}

fn element(b: &[u8], tokens: &mut Tokens, depth: usize) -> Result<Element> {
    if depth > MAX_DEPTH {
        return Err(Error::Invalid(Malformed::SpiNesting));
    }
    let mut e = Element {
        tag: *b.first().ok_or(Error::Truncated)?,
        ..Element::default()
    };
    let mut i = 1;
    let len = length(b, &mut i)?;
    let body = b.get(i..i + len).ok_or(Error::Truncated)?;
    let mut j = 0;
    while j < body.len() {
        let tag = body[j];
        let mut k = j + 1;
        let l = length(body, &mut k)?;
        let d = body.get(k..k + l).ok_or(Error::Truncated)?;
        match tag {
            0x01 => expand(d, tokens, &mut e.text),
            0x04 if depth == 0 => {
                let mut t = 0;
                while t + 2 <= d.len() {
                    let (id, tl) = (d[t] as usize, d[t + 1] as usize);
                    let s = d.get(t + 2..t + 2 + tl).ok_or(Error::Truncated)?;
                    if (1..20).contains(&id) {
                        tokens[id] = s.to_vec();
                    }
                    t += 2 + tl;
                }
            }
            // The default content id (0x05) and default language (0x06) of the top-level element are kept as attributes.
            0x05 | 0x06 if depth == 0 => e.attrs.push((tag, d.to_vec())),
            0x80..=0xFF => e.attrs.push((tag, d.to_vec())),
            _ => e
                .children
                .push(element(&body[j..k + l], tokens, depth + 1)?),
        }
        j = k + l;
    }
    Ok(e)
}

/// Decodes a binary SPI object into its element tree.
pub fn decode(b: &[u8]) -> Result<Element> {
    let mut tokens: Tokens = Default::default();
    element(b, &mut tokens, 0)
}

/// A time point (TS 102 371 clause 5.4.5): Rfu (1), MJD (17), Rfu (1), LTO flag (1), UTC flag (1), hours (5), minutes (6),
/// with the UTC flag seconds (6) and milliseconds (10), with the LTO flag a local time offset (8). It returns UTC seconds
/// since 1970.
pub fn timepoint(b: &[u8]) -> Result<i64> {
    if b.len() < 4 {
        return Err(Error::Truncated);
    }
    let v = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    let mjd = ((v >> 14) & 0x1_FFFF) as i64;
    let long = v & 0x800 != 0;
    let h = ((v >> 6) & 31) as i64;
    let m = (v & 63) as i64;
    let s = if long {
        let w = b.get(4..6).ok_or(Error::Truncated)?;
        (w[0] >> 2) as i64
    } else {
        0
    };
    Ok((mjd - 40_587) * 86_400 + h * 3600 + m * 60 + s)
}

/// A service of the service information.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Service {
    /// Short, medium and long names.
    pub names: [String; 3],
    /// Short or long description.
    pub description: String,
    /// DAB bearers: EId, SId and SCIdS.
    pub bearers: Vec<(u16, u32, u8)>,
    /// Logo URLs.
    pub logos: Vec<String>,
    /// Genre links (TS 102 818 genre href), such as "3.6.1".
    pub genres: Vec<String>,
}

/// A programme of the programme information.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Programme {
    /// The programme id (CRID).
    pub id: String,
    /// Short, medium and long names.
    pub names: [String; 3],
    /// Start, UTC seconds since 1970.
    pub start: i64,
    /// Duration in seconds.
    pub duration_s: u32,
    /// Description.
    pub description: String,
    /// Genre links.
    pub genres: Vec<String>,
}

/// The guide built from decoded SPI objects.
///
/// ```
/// use sdrkit_proto_bcdata::spi::{decode, write, Guide};
///
/// let svc = write::element(0x28, &[], &[write::text(0x12, "Test Melody")]);
/// let si = write::element(0x03, &[], &[svc]);
/// let mut g = Guide::default();
/// g.add(&decode(&si).unwrap());
/// assert_eq!(g.services[0].names[2], "Test Melody");
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Guide {
    /// Services, from the last service information object.
    pub services: Vec<Service>,
    /// Programmes in start order.
    pub programmes: Vec<Programme>,
    /// Programme group names.
    pub groups: Vec<String>,
    /// Objects added.
    pub objects: u64,
}

fn names(e: &Element) -> [String; 3] {
    [0x10, 0x11, 0x12].map(|t| e.child(t).map_or(String::new(), |c| c.text.clone()))
}

fn description(e: &Element) -> String {
    e.children_with(0x13)
        .find_map(|m| {
            m.child(0x1A)
                .or_else(|| m.child(0x1B))
                .map(|d| d.text.clone())
        })
        .unwrap_or_default()
}

fn genres(e: &Element) -> Vec<String> {
    e.children_with(0x14)
        .filter_map(|g| g.attr(0x80))
        .filter(|h| !h.is_empty())
        .map(|h| {
            let mut s = (h[0] & 15).to_string();
            for b in &h[1..] {
                s.push('.');
                s.push_str(&b.to_string());
            }
            s
        })
        .collect()
}

impl Guide {
    /// The most programmes kept: the earliest go first.
    pub const MAX_PROGRAMMES: usize = 500;
    /// The most services kept: the first of an object stay.
    pub const MAX_SERVICES: usize = 256;
    /// The most programme groups kept: the oldest go first.
    pub const MAX_GROUPS: usize = 256;

    /// Adds what a decoded object holds: service information (root 0x03) or programme information (root 0x02).
    pub fn add(&mut self, root: &Element) {
        self.objects += 1;
        match root.tag {
            0x03 => {
                let mut list = Vec::new();
                let svcs = root
                    .children_with(0x28)
                    .chain(root.children_with(0x26).flat_map(|e| e.children_with(0x28)));
                for s in svcs.take(Self::MAX_SERVICES) {
                    let mut out = Service {
                        names: names(s),
                        description: description(s),
                        genres: genres(s),
                        ..Service::default()
                    };
                    for b in s.children.iter().filter(|c| c.tag == 0x29 || c.tag == 0x2D) {
                        if let Some(id) = b.attr(0x80).filter(|id| id.len() >= 6) {
                            let long_sid = id[0] & 0x10 != 0 && id.len() >= 8;
                            let eid = u16::from_be_bytes([id[2], id[3]]);
                            let sid = if long_sid {
                                u32::from_be_bytes([id[4], id[5], id[6], id[7]])
                            } else {
                                u16::from_be_bytes([id[4], id[5]]) as u32
                            };
                            out.bearers.push((eid, sid, id[0] & 15));
                        }
                    }
                    for m in s.children_with(0x13) {
                        for mm in m.children_with(0x2B) {
                            if let Some(u) = mm.attr(0x82) {
                                out.logos.push(String::from_utf8_lossy(u).into_owned());
                            }
                        }
                    }
                    list.push(out);
                }
                self.services = list;
            }
            0x02 => {
                for p in root.children_with(0x21).flat_map(|s| s.children_with(0x1C)) {
                    let id = p
                        .attr(0x80)
                        .map_or(String::new(), |b| String::from_utf8_lossy(b).into_owned());
                    let (mut start, mut duration_s) = (0, 0);
                    if let Some(t) = p.child(0x19).and_then(|l| l.child(0x2C)) {
                        start = t.attr(0x80).and_then(|d| timepoint(d).ok()).unwrap_or(0);
                        duration_s = t
                            .attr(0x81)
                            .map_or(0, |d| d.iter().take(4).fold(0, |a, &x| (a << 8) | x as u32));
                    }
                    let prog = Programme {
                        id,
                        names: names(p),
                        start,
                        duration_s,
                        description: description(p),
                        genres: genres(p),
                    };
                    self.programmes
                        .retain(|x| !(x.id == prog.id && x.start == prog.start));
                    self.programmes.push(prog);
                }
                self.programmes.sort_by_key(|p| p.start);
                if self.programmes.len() > Self::MAX_PROGRAMMES {
                    let n = self.programmes.len() - Self::MAX_PROGRAMMES;
                    self.programmes.drain(..n);
                }
                for pg in root.children_with(0x20).flat_map(|g| g.children_with(0x23)) {
                    let n = names(pg);
                    let s = n
                        .into_iter()
                        .rev()
                        .find(|s| !s.is_empty())
                        .unwrap_or_default();
                    if !s.is_empty() && !self.groups.contains(&s) {
                        if self.groups.len() == Self::MAX_GROUPS {
                            self.groups.remove(0);
                        }
                        self.groups.push(s);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Writes binary SPI (for generators and tests).
pub mod write {
    /// An element with attributes and already written children.
    pub fn element(tag: u8, attrs: &[(u8, Vec<u8>)], children: &[Vec<u8>]) -> Vec<u8> {
        let mut body = Vec::new();
        for (t, v) in attrs {
            body.push(*t);
            push_len(&mut body, v.len());
            body.extend_from_slice(v);
        }
        for c in children {
            body.extend_from_slice(c);
        }
        let mut v = vec![tag];
        push_len(&mut v, body.len());
        v.extend(body);
        v
    }

    /// Appends a length field.
    pub fn push_len(v: &mut Vec<u8>, n: usize) {
        if n < 0xFE {
            v.push(n as u8);
        } else if n < 0x1_0000 {
            v.push(0xFE);
            v.extend_from_slice(&(n as u16).to_be_bytes());
        } else {
            v.push(0xFF);
            v.extend_from_slice(&(n as u32).to_be_bytes()[1..]);
        }
    }

    /// An element with character data only (shortName 0x10, mediumName 0x11, longName 0x12 ...).
    pub fn text(tag: u8, s: &str) -> Vec<u8> {
        let mut cdata = vec![0x01];
        push_len(&mut cdata, s.len());
        cdata.extend_from_slice(s.as_bytes());
        element(tag, &[], &[cdata])
    }

    /// A time point in the long UTC form.
    pub fn timepoint(unix_s: i64) -> Vec<u8> {
        let mjd = (unix_s.div_euclid(86_400) + 40_587) as u32;
        let tod = unix_s.rem_euclid(86_400) as u32;
        let v = ((mjd & 0x1_FFFF) << 14) | (1 << 11) | ((tod / 3600) << 6) | ((tod / 60) % 60);
        let mut b = v.to_be_bytes().to_vec();
        b.extend_from_slice(&(((tod % 60) as u16) << 10).to_be_bytes());
        b
    }

    /// A DAB bearer id with a 16-bit SId.
    pub fn bearer(ecc: u8, eid: u16, sid: u16, scids: u8) -> Vec<u8> {
        let mut v = vec![0x40 | (scids & 15), ecc];
        v.extend_from_slice(&eid.to_be_bytes());
        v.extend_from_slice(&sid.to_be_bytes());
        v
    }
}
