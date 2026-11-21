//! Multiprotocol encapsulation (MPE): IP datagrams in datagram sections (table_id 3E) on one PID of a transport stream, and
//! the writer of such sections for test signals.
//!
//! A datagram section: table_id, the syntax and private indicators and the section length, MAC address bytes 6 and 5, the
//! scrambling controls, the LLC/SNAP flag and the current/next indicator, the section number and the last section number,
//! MAC address bytes 4 to 1, then the datagram (behind an LLC/SNAP header when the flag is set), then a CRC-32/MPEG-2 (with
//! the syntax indicator set) or a checksum (not checked).
//!
//! Rules of the reader: packets with the transport error indicator, scrambled packets and packets without payload are dropped.
//! A PID whose first payload unit starts with 00 00 01 (a PES) is never read. A PID is taken up when a payload unit start
//! points at a section whose first byte is 3E. Sections shorter than 16 bytes or of another table are ignored; a CRC failure
//! counts as a bad section; payload scrambling or a section number or last section number other than 0 counts as skipped.
//! With LLC/SNAP the EtherType must be 0800 or 86DD. The datagram must be a valid IPv4 or IPv6 packet; its own length cuts it.
//!
//! Standard: ETSI EN 301 192 clause 7; ISO/IEC 13818-1 (sections, pointer field, CRC-32/MPEG-2).

use crate::ip::write_ipv4_header;
use crate::Result;
use sdrkit_proto_mpegts::demux::SectionAssembler;
use sdrkit_proto_mpegts::packet::{parse, TS_SIZE};
use sdrkit_util_bits::checksum::internet_checksum;
use sdrkit_util_bits::crc::catalog::CRC32_MPEG2;
use std::net::Ipv4Addr;

/// The table_id of a datagram section.
pub const DATAGRAM_TABLE: u8 = 0x3E;
/// The shortest datagram section read: 12 header bytes and the 4 CRC bytes.
pub const MIN_SECTION: usize = 16;

/// Counters of an [`MpeReader`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MpeStats {
    /// Transport packets of the PID.
    pub packets: u64,
    /// Packets dropped: transport error, scrambled, or no payload.
    pub dropped: u64,
    /// Datagram sections read.
    pub sections: u64,
    /// Sections whose CRC failed.
    pub bad_sections: u64,
    /// Sections skipped: payload scrambling, or a section number or last section number other than 0.
    pub skipped: u64,
    /// Sections whose content is not a valid IP packet (or whose LLC/SNAP header names another protocol).
    pub not_ip: u64,
    /// IP datagrams given out.
    pub datagrams: u64,
}

/// Why a datagram section gives no datagram.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionFault {
    /// Shorter than [`MIN_SECTION`], or another table, or its length field disagrees with its bytes.
    NotDatagram,
    /// The CRC-32/MPEG-2 failed.
    Crc,
    /// Payload scrambling, or a section number or last section number other than 0.
    Skipped,
    /// No valid IP packet in it.
    NotIp,
}

/// A datagram section taken apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DatagramSection<'a> {
    /// The MAC address, first byte first (MAC_address_1 first).
    pub mac: [u8; 6],
    /// The datagram, cut to its own IP length.
    pub datagram: &'a [u8],
}

/// Takes one datagram section apart. See the [module documentation](self) for the rules.
pub fn parse_section(s: &[u8]) -> std::result::Result<DatagramSection<'_>, SectionFault> {
    if s.len() < MIN_SECTION || s[0] != DATAGRAM_TABLE {
        return Err(SectionFault::NotDatagram);
    }
    let len = 3 + ((usize::from(s[1]) & 0x0F) << 8 | usize::from(s[2]));
    if len < MIN_SECTION || len > s.len() {
        return Err(SectionFault::NotDatagram);
    }
    let s = &s[..len];
    let syntax = s[1] & 0x80 != 0;
    if syntax && CRC32_MPEG2.checksum(s) != 0 {
        return Err(SectionFault::Crc);
    }
    let payload_scrambling = (s[5] >> 4) & 3;
    if payload_scrambling != 0 || s[6] != 0 || s[7] != 0 {
        return Err(SectionFault::Skipped);
    }
    let llc_snap = s[5] & 0x02 != 0;
    let mac = [s[11], s[10], s[9], s[8], s[4], s[3]];
    let mut body = &s[12..len - 4];
    if llc_snap {
        if body.len() < 8 || body[..3] != [0xAA, 0xAA, 0x03] {
            return Err(SectionFault::NotIp);
        }
        let ethertype = u16::from_be_bytes([body[6], body[7]]);
        if ethertype != 0x0800 && ethertype != 0x86DD {
            return Err(SectionFault::NotIp);
        }
        body = &body[8..];
    }
    let n = ip_len(body).ok_or(SectionFault::NotIp)?;
    Ok(DatagramSection {
        mac,
        datagram: &body[..n],
    })
}

/// The length of the valid IP packet at the start of `b`: IPv4 (header length at least 20 bytes, total length within `b`,
/// header checksum good) or IPv6 (payload length within `b`).
pub fn ip_len(b: &[u8]) -> Option<usize> {
    match b.first()? >> 4 {
        4 => {
            let ihl = usize::from(b[0] & 0x0F) * 4;
            if ihl < 20 || b.len() < ihl {
                return None;
            }
            let total = usize::from(u16::from_be_bytes([b[2], b[3]]));
            if total < ihl || total > b.len() || internet_checksum(&[&b[..ihl]]) != 0 {
                return None;
            }
            Some(total)
        }
        6 => {
            if b.len() < 40 {
                return None;
            }
            let total = 40 + usize::from(u16::from_be_bytes([b[4], b[5]]));
            (total <= b.len()).then_some(total)
        }
        _ => None,
    }
}

/// The reader of the datagram sections of one PID. See the [module documentation](self).
#[derive(Clone, Debug, Default)]
pub struct MpeReader {
    pid: u16,
    asm: SectionAssembler,
    seen_start: bool,
    pes: bool,
    taken: bool,
    /// The counters.
    pub stats: MpeStats,
}

impl MpeReader {
    /// A reader of PID `pid`.
    pub fn new(pid: u16) -> Self {
        MpeReader {
            pid,
            ..Default::default()
        }
    }

    /// The PID read.
    pub fn pid(&self) -> u16 {
        self.pid
    }

    /// Drops the section in progress (packets were lost).
    pub fn loss(&mut self) {
        self.asm.drop_partial();
    }

    /// Forgets everything but the PID: the next payload unit start is looked at again.
    pub fn reset(&mut self) {
        *self = MpeReader::new(self.pid);
    }

    /// Takes one transport packet of any PID and calls `out` with each IP datagram that it completes.
    pub fn push(&mut self, p: &[u8], mut out: impl FnMut(&[u8])) {
        let Some(t) = parse(p) else {
            return;
        };
        if t.header.pid != self.pid {
            return;
        }
        self.stats.packets += 1;
        if t.header.tei || t.header.scrambling != 0 || t.payload.is_empty() {
            self.stats.dropped += 1;
            return;
        }
        let pusi = t.header.pusi;
        if pusi && !self.seen_start {
            self.seen_start = true;
            self.pes = t.payload.starts_with(&[0, 0, 1]);
        }
        if self.pes {
            return;
        }
        if !self.taken {
            let ptr = usize::from(t.payload[0]);
            if !(pusi && t.payload.get(1 + ptr) == Some(&DATAGRAM_TABLE)) {
                return;
            }
            self.taken = true;
        }
        let stats = &mut self.stats;
        self.asm.push(t.payload, pusi, &mut |s: &[u8]| {
            if s.first() != Some(&DATAGRAM_TABLE) || s.len() < MIN_SECTION {
                return;
            }
            stats.sections += 1;
            match parse_section(s) {
                Ok(d) => {
                    stats.datagrams += 1;
                    out(d.datagram);
                }
                Err(SectionFault::Crc) => stats.bad_sections += 1,
                Err(SectionFault::Skipped) => stats.skipped += 1,
                Err(SectionFault::NotIp) => stats.not_ip += 1,
                Err(SectionFault::NotDatagram) => {}
            }
        });
    }
}

/// Writes a datagram section that carries `datagram` for MAC address `mac` (first byte first): syntax indicator set, no
/// LLC/SNAP, current, section 0 of 0, with its CRC-32/MPEG-2. It returns an error when the datagram does not fit a section.
pub fn write_section(mac: [u8; 6], datagram: &[u8], out: &mut Vec<u8>) -> Result<()> {
    let len = 9 + datagram.len() + 4;
    if len > 4093 {
        return Err(crate::Error::TooLarge(crate::Limit::Section));
    }
    let at = out.len();
    out.extend_from_slice(&[
        DATAGRAM_TABLE,
        0xB0 | (len >> 8) as u8,
        len as u8,
        mac[5],
        mac[4],
        0xC1,
        0,
        0,
        mac[3],
        mac[2],
        mac[1],
        mac[0],
    ]);
    out.extend_from_slice(datagram);
    let c = CRC32_MPEG2.checksum(&out[at..]);
    out.extend_from_slice(&c.to_be_bytes());
    Ok(())
}

/// Writes an IPv4 packet with one UDP datagram and a UDP checksum of 0 (none), identification 0 and TTL 64.
pub fn write_ipv4_udp_plain(
    out: &mut Vec<u8>,
    src: Ipv4Addr,
    dst: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Result<()> {
    let udp_len = 8 + payload.len();
    if udp_len > 65_515 {
        return Err(crate::Error::TooLarge(crate::Limit::UdpDatagram));
    }
    write_ipv4_header(out, src, dst, 17, 0, false, 0, udp_len)?;
    out.extend_from_slice(&src_port.to_be_bytes());
    out.extend_from_slice(&dst_port.to_be_bytes());
    out.extend_from_slice(&(udp_len as u16).to_be_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(payload);
    Ok(())
}

/// Packs `section` into transport packets of PID `pid`, the first with the payload unit start and a pointer field of 0,
/// the rest of the last packet stuffed with FF. `cc` is the continuity counter, advanced per packet.
pub fn section_packets(section: &[u8], pid: u16, cc: &mut u8, mut out: impl FnMut([u8; TS_SIZE])) {
    let mut pos = 0;
    let mut first = true;
    while pos < section.len() || first {
        let mut p = [0xFFu8; TS_SIZE];
        p[0] = 0x47;
        p[1] = (u8::from(first) << 6) | ((pid >> 8) as u8 & 0x1F);
        p[2] = pid as u8;
        p[3] = 0x10 | (*cc & 0x0F);
        *cc = cc.wrapping_add(1);
        let mut o = 4;
        if first {
            p[4] = 0;
            o = 5;
            first = false;
        }
        let n = (TS_SIZE - o).min(section.len() - pos);
        p[o..o + n].copy_from_slice(&section[pos..pos + n]);
        pos += n;
        out(p);
    }
}
