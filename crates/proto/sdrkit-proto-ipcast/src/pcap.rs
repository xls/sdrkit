//! PCAP file bytes: a 24-byte file header, then a 16-byte record header and the packet bytes for each packet. The writer
//! appends to a buffer that the caller owns, and the host writes the file.
//!
//! Standard: draft-ietf-opsawg-pcap.

use crate::error::Malformed;
use crate::error::{Error, Result};

/// The link type of the packets (the LINKTYPE registry of tcpdump.org).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkType {
    /// 101, LINKTYPE_RAW: IPv4 or IPv6 packets, told apart by the version nibble.
    Raw,
    /// 228, LINKTYPE_IPV4: IPv4 packets only.
    Ipv4,
    /// 229, LINKTYPE_IPV6: IPv6 packets only.
    Ipv6,
    /// 1, LINKTYPE_ETHERNET: Ethernet frames (for example GSE bridged frames).
    Ethernet,
}

impl LinkType {
    /// The number of the link type.
    pub fn code(self) -> u32 {
        match self {
            LinkType::Raw => 101,
            LinkType::Ipv4 => 228,
            LinkType::Ipv6 => 229,
            LinkType::Ethernet => 1,
        }
    }
}

/// Writes PCAP file bytes. It holds no buffer.
///
/// ```
/// use sdrkit_proto_ipcast::pcap::{LinkType, PcapWriter};
/// let w = PcapWriter::new(LinkType::Raw);
/// let mut file = Vec::new();
/// w.file_header(&mut file);
/// w.record(1.5, &[0x45, 0, 0, 20], &mut file).unwrap();
/// assert_eq!(&file[..4], &[0xD4, 0xC3, 0xB2, 0xA1]);
/// assert_eq!(file.len(), 24 + 16 + 4);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct PcapWriter {
    link: LinkType,
    nanoseconds: bool,
    snaplen: u32,
}

impl PcapWriter {
    /// A writer of `link` packets with microsecond times and a snap length of 262144 bytes.
    pub fn new(link: LinkType) -> PcapWriter {
        PcapWriter {
            link,
            nanoseconds: false,
            snaplen: 262_144,
        }
    }

    /// The same writer with nanosecond times (magic 0xA1B23C4D).
    pub fn with_nanoseconds(mut self) -> PcapWriter {
        self.nanoseconds = true;
        self
    }

    /// Appends the 24-byte file header: magic, version 2.4, zero time zone and accuracy, snap length, link type.
    pub fn file_header(&self, out: &mut Vec<u8>) {
        let magic: u32 = if self.nanoseconds {
            0xA1B2_3C4D
        } else {
            0xA1B2_C3D4
        };
        out.extend_from_slice(&magic.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&self.snaplen.to_le_bytes());
        out.extend_from_slice(&self.link.code().to_le_bytes());
    }

    /// Appends one record: the time `time_s` (seconds since 1970, or since the start of the signal), the captured length and the
    /// length on the air (orig_len), and the packet (cut to the snap length). If the time is negative or not finite, it returns an error.
    pub fn record(&self, time_s: f64, packet: &[u8], out: &mut Vec<u8>) -> Result<()> {
        if !time_s.is_finite() || !(0.0..4.294_967_296e9).contains(&time_s) {
            return Err(Error::Invalid(Malformed::PcapTime));
        }
        let secs = time_s.floor();
        let scale = if self.nanoseconds { 1e9 } else { 1e6 };
        let frac = (((time_s - secs) * scale).round() as u32).min(scale as u32 - 1);
        let cap = packet.len().min(self.snaplen as usize);
        out.extend_from_slice(&(secs as u32).to_le_bytes());
        out.extend_from_slice(&frac.to_le_bytes());
        out.extend_from_slice(&(cap as u32).to_le_bytes());
        out.extend_from_slice(&(packet.len() as u32).to_le_bytes());
        out.extend_from_slice(&packet[..cap]);
        Ok(())
    }
}
