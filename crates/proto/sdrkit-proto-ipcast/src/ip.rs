//! IPv4, IPv6 and UDP: header parsing with the checksums, IPv4 fragment reassembly, and packet builders for test signals.
//!
//! Standard: RFC 791, RFC 815, RFC 768, RFC 8200, RFC 1071.

use crate::error::{Check, Limit, Malformed};
use crate::error::{Error, Result};
use sdrkit_util_bits::checksum::internet_checksum;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// The IP protocol number of UDP.
pub const UDP: u8 = 17;
/// The IP protocol number of TCP.
pub const TCP: u8 = 6;

/// An IPv4 header whose checksum is correct, and its payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv4<'a> {
    /// The source address.
    pub src: Ipv4Addr,
    /// The destination address.
    pub dst: Ipv4Addr,
    /// The protocol of the payload (17 UDP, 6 TCP ...).
    pub protocol: u8,
    /// The identification of the datagram (for reassembly).
    pub id: u16,
    /// True when the "more fragments" flag is set.
    pub more_fragments: bool,
    /// The fragment offset in bytes.
    pub offset: usize,
    /// The time to live.
    pub ttl: u8,
    /// The header length in bytes (20 to 60).
    pub header_len: usize,
    /// The bytes from the start of the header to Total Length.
    pub packet: &'a [u8],
    /// The payload (after the header, up to Total Length).
    pub payload: &'a [u8],
}

impl<'a> Ipv4<'a> {
    /// Reads an IPv4 packet and makes sure that it is correct: version 4, IHL at least 5, Total Length inside `b`, and the
    /// header checksum.
    pub fn parse(b: &'a [u8]) -> Result<Ipv4<'a>> {
        if b.len() < 20 {
            return Err(Error::Short);
        }
        if b[0] >> 4 != 4 {
            return Err(Error::Invalid(Malformed::IpVersion));
        }
        let ihl = (b[0] & 15) as usize * 4;
        let total = u16::from_be_bytes([b[2], b[3]]) as usize;
        if ihl < 20 || total < ihl {
            return Err(Error::Invalid(Malformed::Ipv4Lengths));
        }
        if total > b.len() {
            return Err(Error::Short);
        }
        if internet_checksum(&[&b[..ihl]]) != 0 {
            return Err(Error::Check(Check::Ipv4HeaderChecksum));
        }
        let fl = u16::from_be_bytes([b[6], b[7]]);
        Ok(Ipv4 {
            src: Ipv4Addr::new(b[12], b[13], b[14], b[15]),
            dst: Ipv4Addr::new(b[16], b[17], b[18], b[19]),
            protocol: b[9],
            id: u16::from_be_bytes([b[4], b[5]]),
            more_fragments: fl & 0x2000 != 0,
            offset: (fl & 0x1FFF) as usize * 8,
            ttl: b[8],
            header_len: ihl,
            packet: &b[..total],
            payload: &b[ihl..total],
        })
    }

    /// True when the packet is a fragment of a larger datagram.
    pub fn is_fragment(&self) -> bool {
        self.more_fragments || self.offset != 0
    }
}

/// An IPv6 header that [`Ipv6::parse`] accepted, and its payload, after the extension headers this reader supports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv6<'a> {
    /// The source address.
    pub src: Ipv6Addr,
    /// The destination address.
    pub dst: Ipv6Addr,
    /// The protocol of the payload after the hop-by-hop, routing and destination option headers.
    pub protocol: u8,
    /// The hop limit.
    pub hop_limit: u8,
    /// True when a fragment header was found. This crate does not reassemble IPv6 fragments.
    pub fragment: bool,
    /// The bytes from the start of the header to the end of the payload.
    pub packet: &'a [u8],
    /// The payload of `protocol`.
    pub payload: &'a [u8],
}

impl<'a> Ipv6<'a> {
    /// Reads an IPv6 packet: version 6, Payload Length inside `b`, and the extension headers 0, 43, 44 and 60 (RFC 8200 clause 4).
    pub fn parse(b: &'a [u8]) -> Result<Ipv6<'a>> {
        if b.len() < 40 {
            return Err(Error::Short);
        }
        if b[0] >> 4 != 6 {
            return Err(Error::Invalid(Malformed::IpVersion));
        }
        let plen = u16::from_be_bytes([b[4], b[5]]) as usize;
        if 40 + plen > b.len() {
            return Err(Error::Short);
        }
        let mut next = b[6];
        let mut at = 40;
        let end = 40 + plen;
        let mut fragment = false;
        loop {
            match next {
                0 | 43 | 60 => {
                    if at + 2 > end {
                        return Err(Error::Short);
                    }
                    let n = (b[at + 1] as usize + 1) * 8;
                    next = b[at];
                    at += n;
                }
                44 => {
                    fragment = true;
                    if at + 8 > end {
                        return Err(Error::Short);
                    }
                    next = b[at];
                    at += 8;
                }
                _ => break,
            }
            if at > end {
                return Err(Error::Short);
            }
        }
        let a = |i: usize| -> Ipv6Addr {
            let mut o = [0u8; 16];
            o.copy_from_slice(&b[i..i + 16]);
            Ipv6Addr::from(o)
        };
        Ok(Ipv6 {
            src: a(8),
            dst: a(24),
            protocol: next,
            hop_limit: b[7],
            fragment,
            packet: &b[..end],
            payload: &b[at..end],
        })
    }
}

/// A UDP datagram over IPv4 or IPv6.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Udp<'a> {
    /// The source address.
    pub src: IpAddr,
    /// The destination address.
    pub dst: IpAddr,
    /// The source port.
    pub src_port: u16,
    /// The destination port.
    pub dst_port: u16,
    /// What the checksum said: `Some(true)` good, `Some(false)` bad, `None` not sent (IPv4 only).
    pub checksum_ok: Option<bool>,
    /// The payload (UDP Length minus 8 bytes).
    pub payload: &'a [u8],
}

impl<'a> Udp<'a> {
    /// Reads the UDP header of `segment` (the IP payload) and calculates its checksum with the pseudo-header of `src` and `dst`.
    pub fn parse(src: IpAddr, dst: IpAddr, segment: &'a [u8]) -> Result<Udp<'a>> {
        if segment.len() < 8 {
            return Err(Error::Short);
        }
        let len = u16::from_be_bytes([segment[4], segment[5]]) as usize;
        if len < 8 || len > segment.len() {
            return Err(Error::Invalid(Malformed::UdpLength));
        }
        let seg = &segment[..len];
        let sent = u16::from_be_bytes([seg[6], seg[7]]);
        let checksum_ok = match (src, dst) {
            (IpAddr::V4(_), IpAddr::V4(_)) if sent == 0 => None,
            (IpAddr::V6(_), IpAddr::V6(_)) if sent == 0 => Some(false),
            _ => Some(udp_checksum(src, dst, seg) == 0),
        };
        Ok(Udp {
            src,
            dst,
            src_port: u16::from_be_bytes([seg[0], seg[1]]),
            dst_port: u16::from_be_bytes([seg[2], seg[3]]),
            checksum_ok,
            payload: &seg[8..],
        })
    }
}

/// The Internet checksum of a UDP segment with its pseudo-header. A segment that holds its correct checksum returns 0. With the
/// checksum field set to 0, it returns the value to put there (send 0xFFFF for a computed 0, RFC 768).
pub fn udp_checksum(src: IpAddr, dst: IpAddr, segment: &[u8]) -> u16 {
    let len = segment.len() as u32;
    match (src, dst) {
        (IpAddr::V4(s), IpAddr::V4(d)) => {
            let ph = [0, UDP, (len >> 8) as u8, len as u8];
            internet_checksum(&[&s.octets(), &d.octets(), &ph, segment])
        }
        (s, d) => {
            let s = match s {
                IpAddr::V6(a) => a,
                IpAddr::V4(a) => a.to_ipv6_mapped(),
            };
            let d = match d {
                IpAddr::V6(a) => a,
                IpAddr::V4(a) => a.to_ipv6_mapped(),
            };
            let ph = [
                (len >> 24) as u8,
                (len >> 16) as u8,
                (len >> 8) as u8,
                len as u8,
                0,
                0,
                0,
                UDP,
            ];
            internet_checksum(&[&s.octets(), &d.octets(), &ph, segment])
        }
    }
}

/// Writes an IPv4 header (no options) with its checksum at the end of `out`: `payload_len` bytes follow it.
#[allow(clippy::too_many_arguments)]
pub fn write_ipv4_header(
    out: &mut Vec<u8>,
    src: Ipv4Addr,
    dst: Ipv4Addr,
    protocol: u8,
    id: u16,
    more_fragments: bool,
    offset: usize,
    payload_len: usize,
) -> Result<()> {
    if !offset.is_multiple_of(8) || offset / 8 > 0x1FFF {
        return Err(Error::Invalid(Malformed::FragmentOffset));
    }
    let total = 20 + payload_len;
    if total > 65535 {
        return Err(Error::TooLarge(Limit::Ipv4Packet));
    }
    let at = out.len();
    let fl = (more_fragments as u16) << 13 | (offset / 8) as u16;
    out.extend_from_slice(&[0x45, 0, (total >> 8) as u8, total as u8]);
    out.extend_from_slice(&id.to_be_bytes());
    out.extend_from_slice(&fl.to_be_bytes());
    out.extend_from_slice(&[64, protocol, 0, 0]);
    out.extend_from_slice(&src.octets());
    out.extend_from_slice(&dst.octets());
    let c = internet_checksum(&[&out[at..at + 20]]);
    out[at + 10..at + 12].copy_from_slice(&c.to_be_bytes());
    Ok(())
}

/// Writes a UDP segment (header, checksum and payload) at the end of `out`.
pub fn write_udp(
    out: &mut Vec<u8>,
    src: IpAddr,
    dst: IpAddr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Result<()> {
    let len = 8 + payload.len();
    if len > 65535 {
        return Err(Error::TooLarge(Limit::UdpDatagram));
    }
    let at = out.len();
    out.extend_from_slice(&src_port.to_be_bytes());
    out.extend_from_slice(&dst_port.to_be_bytes());
    out.extend_from_slice(&(len as u16).to_be_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(payload);
    let mut c = udp_checksum(src, dst, &out[at..]);
    if c == 0 {
        c = 0xFFFF;
    }
    out[at + 6..at + 8].copy_from_slice(&c.to_be_bytes());
    Ok(())
}

/// Writes the IPv4 packets of one UDP datagram at the end of `out`, cut into fragments of at most `mtu` bytes (RFC 791 clause
/// 3.2). It calls `each` with the range of every packet in `out`.
#[allow(clippy::too_many_arguments)]
pub fn write_ipv4_udp(
    out: &mut Vec<u8>,
    src: Ipv4Addr,
    dst: Ipv4Addr,
    id: u16,
    mtu: usize,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
    mut each: impl FnMut(std::ops::Range<usize>),
) -> Result<()> {
    if mtu < 28 {
        return Err(Error::Invalid(Malformed::Mtu));
    }
    let mut seg = Vec::with_capacity(8 + payload.len());
    write_udp(
        &mut seg,
        src.into(),
        dst.into(),
        src_port,
        dst_port,
        payload,
    )?;
    let max = (mtu - 20) / 8 * 8;
    let mut off = 0;
    loop {
        let n = (seg.len() - off).min(max);
        let more = off + n < seg.len();
        let at = out.len();
        write_ipv4_header(out, src, dst, UDP, id, more, off, n)?;
        out.extend_from_slice(&seg[off..off + n]);
        each(at..out.len());
        off += n;
        if !more {
            return Ok(());
        }
    }
}

/// One datagram out of the reassembler: the IPv4 header fields and the whole payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Datagram<'a> {
    /// The source address.
    pub src: Ipv4Addr,
    /// The destination address.
    pub dst: Ipv4Addr,
    /// The protocol.
    pub protocol: u8,
    /// The identification.
    pub id: u16,
    /// True when it came in fragments.
    pub reassembled: bool,
    /// The whole payload.
    pub payload: &'a [u8],
}

impl Datagram<'_> {
    /// The UDP datagram in the payload, when the protocol is UDP.
    pub fn udp(&self) -> Option<Result<Udp<'_>>> {
        (self.protocol == UDP).then(|| Udp::parse(self.src.into(), self.dst.into(), self.payload))
    }
}

/// Counters of the reassembler.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReassemblyStats {
    /// Packets refused (bad header, checksum, lengths).
    pub refused: u64,
    /// Fragments taken.
    pub fragments: u64,
    /// Datagrams made whole from fragments.
    pub reassembled: u64,
    /// Reassemblies dropped: timed out, or pushed out when every buffer was busy.
    pub dropped: u64,
    /// Fragments whose bytes disagree with bytes already received at the same place (kept: the first copy wins).
    pub overlaps: u64,
}

#[derive(Clone, Debug)]
struct Frag {
    active: bool,
    key: (Ipv4Addr, Ipv4Addr, u8, u16),
    started: f64,
    data: Vec<u8>,
    /// One bit per 8-byte block received.
    have: Vec<u64>,
    /// The payload length, once the last fragment is in.
    total: Option<usize>,
}

/// IPv4 reassembly (RFC 791 clause 3.2, RFC 815): fragments are matched by source, destination, protocol and identification,
/// in any order, duplicates and overlaps allowed. Up to `slots` datagrams are in reassembly at a time. A reassembly older than
/// `timeout_s` seconds of signal time is dropped (RFC 1122 clause 3.3.2 recommends 60 to 120 seconds). Each buffer is made the
/// first time it is used. After that it does not allocate.
#[derive(Clone, Debug)]
pub struct Reassembler {
    slots: Vec<Frag>,
    timeout_s: f64,
    /// The counters.
    pub stats: ReassemblyStats,
}

impl Default for Reassembler {
    fn default() -> Self {
        Self::new(16, 60.0)
    }
}

impl Reassembler {
    /// A reassembler with `slots` buffers (at least 1) and a timeout of `timeout_s` seconds.
    pub fn new(slots: usize, timeout_s: f64) -> Reassembler {
        Reassembler {
            slots: (0..slots.max(1))
                .map(|_| Frag {
                    active: false,
                    key: (Ipv4Addr::UNSPECIFIED, Ipv4Addr::UNSPECIFIED, 0, 0),
                    started: 0.0,
                    data: Vec::new(),
                    have: Vec::new(),
                    total: None,
                })
                .collect(),
            timeout_s,
            stats: ReassemblyStats::default(),
        }
    }

    /// Takes one IPv4 packet at signal time `now` (seconds). A whole packet goes to `out` at once. A fragment goes to `out` when
    /// it completes its datagram. A bad packet returns an error (and is counted).
    pub fn push(
        &mut self,
        packet: &[u8],
        now: f64,
        out: &mut impl FnMut(Datagram<'_>),
    ) -> Result<()> {
        let ip = match Ipv4::parse(packet) {
            Ok(ip) => ip,
            Err(e) => {
                self.stats.refused += 1;
                return Err(e);
            }
        };
        for s in &mut self.slots {
            if s.active && now - s.started > self.timeout_s {
                s.active = false;
                self.stats.dropped += 1;
            }
        }
        if !ip.is_fragment() {
            out(Datagram {
                src: ip.src,
                dst: ip.dst,
                protocol: ip.protocol,
                id: ip.id,
                reassembled: false,
                payload: ip.payload,
            });
            return Ok(());
        }
        let end = ip.offset + ip.payload.len();
        if end > 65535 - 20 || (ip.more_fragments && ip.payload.len() % 8 != 0) {
            self.stats.refused += 1;
            return Err(Error::Invalid(Malformed::FragmentLength));
        }
        self.stats.fragments += 1;
        let key = (ip.src, ip.dst, ip.protocol, ip.id);
        let k = self.slot(key, now);
        let s = &mut self.slots[k];
        if s.data.len() < 65536 {
            s.data.resize(65536, 0);
            s.have.resize(65536 / 8 / 64, 0);
        }
        // Copy the blocks not yet received. The first copy of a byte wins. Count the blocks that disagree.
        let mut disagree = false;
        for (j, chunk) in ip.payload.chunks(8).enumerate() {
            let blk = ip.offset / 8 + j;
            let at = blk * 8;
            if s.have[blk / 64] >> (blk % 64) & 1 == 1 {
                disagree |= s.data[at..at + chunk.len()] != *chunk;
            } else {
                s.data[at..at + chunk.len()].copy_from_slice(chunk);
                if chunk.len() == 8 || !ip.more_fragments {
                    s.have[blk / 64] |= 1 << (blk % 64);
                }
            }
        }
        if disagree {
            self.stats.overlaps += 1;
        }
        if !ip.more_fragments {
            s.total = Some(end);
        }
        let Some(total) = s.total else {
            return Ok(());
        };
        let blocks = total.div_ceil(8);
        let complete = (0..blocks).all(|b| s.have[b / 64] >> (b % 64) & 1 == 1);
        if !complete {
            return Ok(());
        }
        s.active = false;
        self.stats.reassembled += 1;
        out(Datagram {
            src: key.0,
            dst: key.1,
            protocol: key.2,
            id: key.3,
            reassembled: true,
            payload: &s.data[..total],
        });
        Ok(())
    }

    fn slot(&mut self, key: (Ipv4Addr, Ipv4Addr, u8, u16), now: f64) -> usize {
        if let Some(k) = self.slots.iter().position(|s| s.active && s.key == key) {
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
        s.key = key;
        s.started = now;
        s.total = None;
        s.have.iter_mut().for_each(|w| *w = 0);
        k
    }

    /// Datagrams in reassembly now.
    pub fn open(&self) -> usize {
        self.slots.iter().filter(|s| s.active).count()
    }
}
