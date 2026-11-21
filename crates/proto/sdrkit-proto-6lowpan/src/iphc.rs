//! The compressed IPv6 header (LOWPAN_IPHC) of RFC 6282 section 3.1.

use crate::{Cursor, LinkAddress};

/// The source address of a compressed IPv6 header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SrcAddress {
    /// The whole address is in the header.
    Full([u8; 16]),
    /// The unspecified address `::`.
    Unspecified,
    /// The last 8 or 2 bytes are in the header (`len` of them, at the end of `bytes`). `context` says that the prefix comes
    /// from a context, else it is the link-local prefix.
    Inline {
        /// The bytes of the header, right-aligned.
        bytes: [u8; 8],
        /// 8 or 2.
        len: u8,
        /// The prefix comes from a context.
        context: bool,
    },
    /// Nothing is in the header: the address comes from the link-layer source address, with the link-local prefix or the
    /// prefix of a context.
    Elided {
        /// The prefix comes from a context.
        context: bool,
    },
}

/// The destination address of a compressed IPv6 header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DstAddress {
    /// The whole address is in the header.
    Full([u8; 16]),
    /// The last 8 or 2 bytes are in the header.
    Inline {
        /// The bytes of the header, right-aligned.
        bytes: [u8; 8],
        /// 8 or 2.
        len: u8,
        /// The prefix comes from a context.
        context: bool,
    },
    /// Nothing is in the header: the address comes from the link-layer destination address.
    Elided {
        /// The prefix comes from a context.
        context: bool,
    },
    /// A multicast address whose 6, 4 or 1 bytes are in the header (`len` of them, at the start of `bytes`). `address` is the
    /// whole address when the header alone gives it.
    Multicast {
        /// The bytes of the header.
        bytes: [u8; 6],
        /// 6, 4 or 1.
        len: u8,
        /// The whole address, without a context.
        address: Option<[u8; 16]>,
    },
    /// A combination that RFC 6282 reserves.
    Reserved,
}

/// A compressed IPv6 header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Iphc {
    /// The two bytes of the base header, the first in the high byte.
    pub base: u16,
    /// The context identifier byte: the source context in the high half.
    pub context: Option<u8>,
    /// The traffic class and flow label bytes in the header: 4, 3, 1 or 0.
    pub traffic_len: u8,
    /// The next header number, when it is in the header. `None`: the next header is compressed.
    pub next_header: Option<u8>,
    /// The hop limit.
    pub hop_limit: u8,
    /// The source address.
    pub source: SrcAddress,
    /// The destination address.
    pub destination: DstAddress,
}

/// The link-local address `fe80::` with the interface identifier `iid`.
fn link_local(iid: [u8; 8]) -> [u8; 16] {
    let mut a = [0u8; 16];
    a[0] = 0xFE;
    a[1] = 0x80;
    a[8..].copy_from_slice(&iid);
    a
}

/// The interface identifier of `len` right-aligned bytes: 8 bytes as they are, 2 bytes as `0000:00ff:fe00:XXXX`.
fn iid_of(bytes: [u8; 8], len: u8) -> [u8; 8] {
    if len == 2 {
        [0, 0, 0, 0xFF, 0xFE, 0, bytes[6], bytes[7]]
    } else {
        bytes
    }
}

impl Iphc {
    /// The source address, when the header and the link-layer source address `link` give it without a context.
    pub fn source_address(&self, link: Option<LinkAddress>) -> Option<[u8; 16]> {
        match self.source {
            SrcAddress::Full(a) => Some(a),
            SrcAddress::Unspecified => Some([0; 16]),
            SrcAddress::Inline {
                bytes,
                len,
                context: false,
            } => Some(link_local(iid_of(bytes, len))),
            SrcAddress::Elided { context: false } => link.map(|l| link_local(l.interface_id())),
            _ => None,
        }
    }

    /// The destination address, when the header and the link-layer destination address `link` give it without a context.
    pub fn destination_address(&self, link: Option<LinkAddress>) -> Option<[u8; 16]> {
        match self.destination {
            DstAddress::Full(a) => Some(a),
            DstAddress::Inline {
                bytes,
                len,
                context: false,
            } => Some(link_local(iid_of(bytes, len))),
            DstAddress::Elided { context: false } => link.map(|l| link_local(l.interface_id())),
            DstAddress::Multicast { address, .. } => address,
            _ => None,
        }
    }
}

fn right_aligned(s: &[u8]) -> [u8; 8] {
    let mut b = [0u8; 8];
    b[8 - s.len()..].copy_from_slice(s);
    b
}

fn full(c: &mut Cursor<'_>) -> Option<[u8; 16]> {
    let mut a = [0u8; 16];
    a.copy_from_slice(c.take(16)?);
    Some(a)
}

/// Reads the header at the cursor. `None` when it runs past the end of the payload.
pub(crate) fn read(c: &mut Cursor<'_>) -> Option<Iphc> {
    let base = c.u16()?;
    let tf = (base >> 11) & 3;
    let nh = base & 0x0400 != 0;
    let hlim = (base >> 8) & 3;
    let cid = base & 0x80 != 0;
    let sac = base & 0x40 != 0;
    let sam = (base >> 4) & 3;
    let m = base & 0x08 != 0;
    let dac = base & 0x04 != 0;
    let dam = base & 3;
    let context = if cid { Some(c.u8()?) } else { None };
    let traffic_len = [4u8, 3, 1, 0][usize::from(tf)];
    c.take(usize::from(traffic_len))?;
    let next_header = if nh { None } else { Some(c.u8()?) };
    let hop_limit = match hlim {
        0 => c.u8()?,
        1 => 1,
        2 => 64,
        _ => 255,
    };
    let source = match (sac, sam) {
        (false, 0) => SrcAddress::Full(full(c)?),
        (true, 0) => SrcAddress::Unspecified,
        (context, 1) => SrcAddress::Inline {
            bytes: right_aligned(c.take(8)?),
            len: 8,
            context,
        },
        (context, 2) => SrcAddress::Inline {
            bytes: right_aligned(c.take(2)?),
            len: 2,
            context,
        },
        (context, _) => SrcAddress::Elided { context },
    };
    let destination = if m {
        let len = match (dac, dam) {
            (false, 0) => 16,
            (false, 1) => 6,
            (false, 2) => 4,
            (false, _) => 1,
            (true, 0) => 6,
            (true, _) => 0,
        };
        match len {
            16 => DstAddress::Full(full(c)?),
            0 => DstAddress::Reserved,
            n => {
                let s = c.take(n)?;
                let mut bytes = [0u8; 6];
                bytes[..n].copy_from_slice(s);
                let mut a = [0u8; 16];
                a[0] = 0xFF;
                // RFC 6282 section 3.2.4: ffXX::00XX:XXXX:XXXX, ffXX::00XX:XXXX and ff02::00XX.
                let address = match (dac, n) {
                    (false, 6) => {
                        a[1] = s[0];
                        a[11..].copy_from_slice(&s[1..]);
                        Some(a)
                    }
                    (false, 4) => {
                        a[1] = s[0];
                        a[13..].copy_from_slice(&s[1..]);
                        Some(a)
                    }
                    (false, 1) => {
                        a[1] = 0x02;
                        a[15] = s[0];
                        Some(a)
                    }
                    _ => None,
                };
                DstAddress::Multicast {
                    bytes,
                    len: n as u8,
                    address,
                }
            }
        }
    } else {
        match (dac, dam) {
            (false, 0) => DstAddress::Full(full(c)?),
            (true, 0) => DstAddress::Reserved,
            (context, 1) => DstAddress::Inline {
                bytes: right_aligned(c.take(8)?),
                len: 8,
                context,
            },
            (context, 2) => DstAddress::Inline {
                bytes: right_aligned(c.take(2)?),
                len: 2,
                context,
            },
            (context, _) => DstAddress::Elided { context },
        }
    };
    Some(Iphc {
        base,
        context,
        traffic_len,
        next_header,
        hop_limit,
        source,
        destination,
    })
}
