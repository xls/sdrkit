//! 6LoWPAN headers as IEEE 802.15.4 frames carry them, and the first headers above them.
//!
//! [`read`] walks the chain of headers at the start of a MAC payload: the mesh header and the fragment headers of RFC 4944
//! section 5, the compressed IPv6 header (IPHC) of RFC 6282 section 3, the compressed extension headers of section 4.2 and
//! the compressed UDP header of section 4.3. It returns typed headers and never reads past the end of the payload: a header
//! that is cut off is reported as cut off. [`coap`] reads the fixed header and the Uri-Path options of a CoAP message
//! (RFC 7252 section 3).
//!
//! The reader rebuilds an address only where the header alone and the link-layer addresses give it (the link-local prefix
//! `fe80::/64`, RFC 6282 section 3.2.2). An address that needs a context is reported as it was sent.
//!
//! ```
//! use sdrkit_proto_6lowpan::{read, NextHeader};
//!
//! // IPHC, a multicast destination ff02::2, compressed UDP with both ports and the checksum in line.
//! let d = read(&[0x7F, 0x3B, 0x02, 0xF0, 0x4D, 0x4C, 0x4D, 0x4C, 0x12, 0x34, 0xFF, 0x10]);
//! let udp = d.udp.unwrap();
//! assert_eq!((udp.source, udp.destination), (19788, 19788));
//! assert_eq!(d.next, NextHeader::Udp);
//! assert_eq!(d.payload, 10);
//! ```

#![warn(missing_docs)]

pub mod coap;
mod iphc;

pub use iphc::{DstAddress, Iphc, SrcAddress};

/// A link-layer address of a mesh header, or of the frame that carries the datagram.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LinkAddress {
    /// A 16-bit short address.
    Short(u16),
    /// A 64-bit extended address, in transmit order (the first byte is the most significant byte of the EUI-64).
    Extended([u8; 8]),
}

impl LinkAddress {
    /// The interface identifier that stateless address compression derives from the address (RFC 6282 section 3.2.2): the
    /// EUI-64 with its universal/local bit inverted, or `0000:00ff:fe00:XXXX` for a short address.
    pub fn interface_id(&self) -> [u8; 8] {
        match *self {
            LinkAddress::Short(a) => {
                let b = a.to_be_bytes();
                [0, 0, 0, 0xFF, 0xFE, 0, b[0], b[1]]
            }
            LinkAddress::Extended(mut e) => {
                e[0] ^= 0x02;
                e
            }
        }
    }
}

/// The mesh addressing header (RFC 4944 section 5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mesh {
    /// The hops left.
    pub hops_left: u8,
    /// The originator.
    pub originator: LinkAddress,
    /// The final destination.
    pub final_destination: LinkAddress,
}

/// A fragment header (RFC 4944 section 5.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fragment {
    /// The size of the whole datagram before compression, bytes.
    pub datagram_size: u16,
    /// The tag that the fragments of one datagram share.
    pub datagram_tag: u16,
    /// The offset of this fragment in the datagram, bytes (the header carries it in units of 8 bytes). 0 for the first
    /// fragment.
    pub offset: u16,
    /// This is the first fragment: the headers of the datagram follow it.
    pub first: bool,
}

/// A UDP header, plain or compressed (RFC 6282 section 4.3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Udp {
    /// The source port.
    pub source: u16,
    /// The destination port.
    pub destination: u16,
    /// The checksum. `None` when the compressed header leaves it out.
    pub checksum: Option<u16>,
    /// The header was compressed.
    pub compressed: bool,
}

/// What follows the headers that were read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextHeader {
    /// The payload of a UDP datagram.
    Udp,
    /// An ICMPv6 message.
    Icmpv6,
    /// Another protocol, by its IPv6 next-header number.
    Other(u8),
    /// A compressed header that this reader does not know.
    Compressed(u8),
    /// An uncompressed IPv6 header (dispatch 0x41).
    Ipv6,
    /// The rest of a later fragment: the headers were in the first fragment.
    FragmentData,
    /// The payload does not start with a 6LoWPAN dispatch that this reader knows.
    NotLowpan,
    /// Nothing: the payload ends after the headers, or inside them.
    None,
}

/// The headers at the start of a MAC payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Datagram {
    /// The mesh header.
    pub mesh: Option<Mesh>,
    /// The fragment header.
    pub fragment: Option<Fragment>,
    /// The compressed IPv6 header.
    pub iphc: Option<Iphc>,
    /// The UDP header.
    pub udp: Option<Udp>,
    /// What follows the headers.
    pub next: NextHeader,
    /// The offset of what follows the headers in the payload.
    pub payload: usize,
    /// A header runs past the end of the payload. The headers before it are given.
    pub truncated: bool,
}

/// A reader over a payload that never reads past its end.
pub(crate) struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(b: &'a [u8], at: usize) -> Self {
        Cursor { b, at }
    }
    pub(crate) fn at(&self) -> usize {
        self.at
    }
    pub(crate) fn peek(&self) -> Option<u8> {
        self.b.get(self.at).copied()
    }
    pub(crate) fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    pub(crate) fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }
    pub(crate) fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|s| u16::from_be_bytes([s[0], s[1]]))
    }
}

fn link_address(c: &mut Cursor<'_>, short: bool) -> Option<LinkAddress> {
    if short {
        c.u16().map(LinkAddress::Short)
    } else {
        let s = c.take(8)?;
        let mut e = [0u8; 8];
        e.copy_from_slice(s);
        Some(LinkAddress::Extended(e))
    }
}

fn mesh(c: &mut Cursor<'_>) -> Option<Mesh> {
    let d = c.u8()?;
    let hops_left = match d & 0x0F {
        // The value 15 says that the hops are in the next byte.
        0x0F => c.u8()?,
        h => h,
    };
    let originator = link_address(c, d & 0x20 != 0)?;
    let final_destination = link_address(c, d & 0x10 != 0)?;
    Some(Mesh {
        hops_left,
        originator,
        final_destination,
    })
}

fn fragment(c: &mut Cursor<'_>) -> Option<Fragment> {
    let first = c.peek()? & 0xF8 == 0xC0;
    let w = c.u16()?;
    let datagram_tag = c.u16()?;
    let offset = if first { 0 } else { u16::from(c.u8()?) * 8 };
    Some(Fragment {
        datagram_size: w & 0x07FF,
        datagram_tag,
        offset,
        first,
    })
}

/// A compressed UDP header whose first byte is `11110 C PP`.
fn udp_compressed(c: &mut Cursor<'_>) -> Option<Udp> {
    let h = c.u8()?;
    let (source, destination) = match h & 3 {
        0 => (c.u16()?, c.u16()?),
        1 => (c.u16()?, 0xF000 + u16::from(c.u8()?)),
        2 => (0xF000 + u16::from(c.u8()?), c.u16()?),
        _ => {
            let b = c.u8()?;
            (0xF0B0 + u16::from(b >> 4), 0xF0B0 + u16::from(b & 0x0F))
        }
    };
    let checksum = if h & 0x04 == 0 { Some(c.u16()?) } else { None };
    Some(Udp {
        source,
        destination,
        checksum,
        compressed: true,
    })
}

fn udp_plain(c: &mut Cursor<'_>) -> Option<Udp> {
    let (source, destination) = (c.u16()?, c.u16()?);
    let _length = c.u16()?;
    Some(Udp {
        source,
        destination,
        checksum: Some(c.u16()?),
        compressed: false,
    })
}

/// Reads the next header after the IPv6 header, given its number.
fn by_number(d: &mut Datagram, c: &mut Cursor<'_>, number: u8) {
    d.next = match number {
        17 => match udp_plain(c) {
            Some(u) => {
                d.udp = Some(u);
                NextHeader::Udp
            }
            None => {
                d.truncated = true;
                NextHeader::None
            }
        },
        58 => NextHeader::Icmpv6,
        n => NextHeader::Other(n),
    };
}

/// Reads the compressed next headers (RFC 6282 section 4): extension headers are passed over until UDP or a header that is not
/// compressed.
fn compressed_next(d: &mut Datagram, c: &mut Cursor<'_>) {
    loop {
        let Some(h) = c.peek() else {
            d.next = NextHeader::None;
            return;
        };
        if h & 0xF8 == 0xF0 {
            match udp_compressed(c) {
                Some(u) => {
                    d.udp = Some(u);
                    d.next = NextHeader::Udp;
                }
                None => {
                    d.truncated = true;
                    d.next = NextHeader::None;
                }
            }
            return;
        }
        // An extension header 1110 EEE N. EID 7 is an IPv6 header in IPHC form, which this reader does not open.
        if h & 0xF0 != 0xE0 || h & 0x0E == 0x0E {
            d.next = NextHeader::Compressed(h);
            return;
        }
        c.u8();
        let number = if h & 1 == 0 { c.u8() } else { None };
        let body = c.u8().and_then(|len| c.take(usize::from(len)));
        if body.is_none() || (h & 1 == 0 && number.is_none()) {
            d.truncated = true;
            d.next = NextHeader::None;
            return;
        }
        if let Some(n) = number {
            by_number(d, c, n);
            return;
        }
    }
}

/// Reads the 6LoWPAN headers at the start of the MAC payload `b`: a mesh header, then a fragment header, then IPHC with its
/// next headers.
pub fn read(b: &[u8]) -> Datagram {
    let mut d = Datagram {
        mesh: None,
        fragment: None,
        iphc: None,
        udp: None,
        next: NextHeader::None,
        payload: 0,
        truncated: false,
    };
    let mut c = Cursor::new(b, 0);
    let cut = |d: &mut Datagram, c: &Cursor<'_>| {
        d.truncated = true;
        d.next = NextHeader::None;
        d.payload = c.at();
    };
    if c.peek().is_some_and(|h| h & 0xC0 == 0x80) {
        match mesh(&mut c) {
            Some(m) => d.mesh = Some(m),
            None => {
                cut(&mut d, &c);
                d.payload = b.len();
                return d;
            }
        }
    }
    if c.peek()
        .is_some_and(|h| h & 0xF8 == 0xC0 || h & 0xF8 == 0xE0)
    {
        match fragment(&mut c) {
            Some(f) => d.fragment = Some(f),
            None => {
                cut(&mut d, &c);
                d.payload = b.len();
                return d;
            }
        }
        if d.fragment.is_some_and(|f| !f.first) {
            d.next = NextHeader::FragmentData;
            d.payload = c.at();
            return d;
        }
    }
    match c.peek() {
        Some(h) if h & 0xE0 == 0x60 => match iphc::read(&mut c) {
            Some(ip) => {
                let nh = ip.next_header;
                d.iphc = Some(ip);
                match nh {
                    Some(n) => by_number(&mut d, &mut c, n),
                    None => compressed_next(&mut d, &mut c),
                }
            }
            None => {
                d.truncated = true;
                d.payload = b.len();
                return d;
            }
        },
        Some(0x41) => {
            c.u8();
            d.next = NextHeader::Ipv6;
        }
        Some(_) if d.mesh.is_none() && d.fragment.is_none() => d.next = NextHeader::NotLowpan,
        Some(h) => d.next = NextHeader::Compressed(h),
        None => d.next = NextHeader::None,
    }
    d.payload = if d.truncated { b.len() } else { c.at() };
    d
}
