//! Space packets and their transport in M_PDU packet zones. [`Assembler`] takes the zones of each virtual channel in order and
//! emits whole packets; after a lost frame it drops the packet in progress and resumes at the next first header pointer. If
//! [`AssemblerConfig::crc`] is on, it checks and removes the CRC-16 that some missions append to the data field (the CP_PDU
//! check of the CGMS xRIT transport).
//!
//! Standard: CCSDS 133.0-B-2; CCSDS 732.0-B-4 clause 4.1.4; CCSDS 132.0-B-3 clause 4.1.2.7.

use crate::demux::{Continuity, VcCounters};
use crate::tm::FirstHeader;
use crate::{be16, Error, HeaderField, Param, Result};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;

/// The APID of idle packets (all ones, CCSDS 133.0-B-2 clause 4.1.3.3.4.4).
pub const IDLE_APID: u16 = 0x7FF;

/// The largest space packet: 6 header bytes and 65,536 data bytes.
pub const MAX_PACKET: usize = 6 + 65_536;

/// The sequence flags (CCSDS 133.0-B-2 clause 4.1.3.4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SeqFlags {
    /// A middle segment of user data (00).
    Continuation,
    /// The first segment (01).
    First,
    /// The last segment (10).
    Last,
    /// Unsegmented user data (11).
    #[default]
    Unsegmented,
}

impl SeqFlags {
    /// The flags from their 2-bit field.
    pub const fn from_bits(v: u8) -> Self {
        match v & 3 {
            0 => SeqFlags::Continuation,
            1 => SeqFlags::First,
            2 => SeqFlags::Last,
            _ => SeqFlags::Unsegmented,
        }
    }

    /// The 2-bit field.
    pub const fn bits(self) -> u8 {
        match self {
            SeqFlags::Continuation => 0,
            SeqFlags::First => 1,
            SeqFlags::Last => 2,
            SeqFlags::Unsegmented => 3,
        }
    }

    /// True for the first segment or unsegmented data.
    pub const fn starts(self) -> bool {
        matches!(self, SeqFlags::First | SeqFlags::Unsegmented)
    }

    /// True for the last segment or unsegmented data.
    pub const fn ends(self) -> bool {
        matches!(self, SeqFlags::Last | SeqFlags::Unsegmented)
    }
}

/// The space packet primary header (CCSDS 133.0-B-2 clause 4.1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PrimaryHeader {
    /// Packet version number (0).
    pub version: u8,
    /// Packet type: true for telecommand, false for telemetry.
    pub telecommand: bool,
    /// A secondary header is present.
    pub secondary_header: bool,
    /// Application process identifier, 11 bits.
    pub apid: u16,
    /// Sequence flags.
    pub flags: SeqFlags,
    /// Packet sequence count, 14 bits.
    pub count: u16,
    /// Bytes of the data field (1 to 65,536). The header field holds this minus one.
    pub data_len: u32,
}

impl PrimaryHeader {
    /// A telemetry header without a secondary header. [`build`] sets the data length.
    pub const fn telemetry(apid: u16, flags: SeqFlags, count: u16) -> Self {
        PrimaryHeader {
            version: 0,
            telecommand: false,
            secondary_header: false,
            apid,
            flags,
            count,
            data_len: 0,
        }
    }

    /// Reads the 6 header bytes.
    pub fn parse(b: &[u8]) -> Result<Self> {
        if b.len() < 6 {
            return Err(Error::InputTooShort { need: 6 });
        }
        let id = be16(b);
        let seq = be16(&b[2..]);
        Ok(PrimaryHeader {
            version: (id >> 13) as u8,
            telecommand: (id >> 12) & 1 != 0,
            secondary_header: (id >> 11) & 1 != 0,
            apid: id & 0x7FF,
            flags: SeqFlags::from_bits((seq >> 14) as u8),
            count: seq & 0x3FFF,
            data_len: be16(&b[4..]) as u32 + 1,
        })
    }

    /// The 6 header bytes. If a field does not fit its width, it returns an error.
    pub fn bytes(&self) -> Result<[u8; 6]> {
        if self.version > 7
            || self.apid > 0x7FF
            || self.count > 0x3FFF
            || !(1..=65_536).contains(&self.data_len)
        {
            return Err(Error::BadField(HeaderField::SpacePacketHeader));
        }
        let id = (self.version as u16) << 13
            | (self.telecommand as u16) << 12
            | (self.secondary_header as u16) << 11
            | self.apid;
        let seq = (self.flags.bits() as u16) << 14 | self.count;
        let mut b = [0u8; 6];
        b[..2].copy_from_slice(&id.to_be_bytes());
        b[2..4].copy_from_slice(&seq.to_be_bytes());
        b[4..].copy_from_slice(&((self.data_len - 1) as u16).to_be_bytes());
        Ok(b)
    }

    /// Bytes of the whole packet.
    pub const fn packet_len(&self) -> usize {
        6 + self.data_len as usize
    }

    /// True for an idle packet.
    pub const fn is_idle(&self) -> bool {
        self.apid == IDLE_APID
    }
}

/// Appends a packet with header `h` (its data length set from `data`) and data field `data` to `out`.
pub fn build(h: &PrimaryHeader, data: &[u8], out: &mut Vec<u8>) -> Result<()> {
    let h = PrimaryHeader {
        data_len: data.len() as u32,
        ..*h
    };
    out.extend_from_slice(&h.bytes()?);
    out.extend_from_slice(data);
    Ok(())
}

/// Appends a packet whose data field is `data` followed by its CRC-16 (CRC-16/IBM-3740 of `data`, the CGMS CP_PDU check).
pub fn build_with_crc(h: &PrimaryHeader, data: &[u8], out: &mut Vec<u8>) -> Result<()> {
    let h = PrimaryHeader {
        data_len: data.len() as u32 + 2,
        ..*h
    };
    out.extend_from_slice(&h.bytes()?);
    out.extend_from_slice(data);
    out.extend_from_slice(&CRC16_IBM_3740.checksum(data).to_be_bytes());
    Ok(())
}

/// Appends an idle packet of exactly `len` bytes (at least 7) to `out`: APID 0x7FF, unsegmented, data all zeros.
pub fn idle(len: usize, out: &mut Vec<u8>) -> Result<()> {
    if !(7..=MAX_PACKET).contains(&len) {
        return Err(Error::BadParameter(Param::IdlePacketLength));
    }
    let start = out.len();
    let h = PrimaryHeader {
        data_len: (len - 6) as u32,
        ..PrimaryHeader::telemetry(IDLE_APID, SeqFlags::Unsegmented, 0)
    };
    out.extend_from_slice(&h.bytes()?);
    out.resize(start + len, 0);
    Ok(())
}

/// Cuts a stream of whole packets into packet zones of `zone_len` bytes, for generators: returns each zone with its first header
/// pointer. The last zone is filled with an idle packet, or with zeros when fewer than 7 bytes are left.
pub fn pack_zones(packets: &[&[u8]], zone_len: usize) -> Result<Vec<(FirstHeader, Vec<u8>)>> {
    if zone_len == 0 || zone_len > 0x7FE {
        return Err(Error::BadParameter(Param::ZoneLength));
    }
    let mut stream = Vec::new();
    let mut starts = Vec::with_capacity(packets.len());
    for p in packets {
        starts.push(stream.len());
        stream.extend_from_slice(p);
    }
    let mut out = Vec::new();
    let mut pos = 0;
    let mut next_start = 0;
    while pos < stream.len() {
        let end = pos + zone_len;
        while next_start < starts.len() && starts[next_start] < pos {
            next_start += 1;
        }
        let fh = match starts.get(next_start) {
            Some(&s) if s < end => FirstHeader::At((s - pos) as u16),
            _ => FirstHeader::None,
        };
        let n = (stream.len() - pos).min(zone_len);
        let mut zone = Vec::with_capacity(zone_len);
        zone.extend_from_slice(&stream[pos..pos + n]);
        let room = zone_len - n;
        if room >= 7 {
            idle(room, &mut zone)?;
        } else {
            zone.resize(zone_len, 0);
        }
        out.push((fh, zone));
        pos = end;
    }
    Ok(out)
}

/// Puts one whole packet at the start of a packet zone and fills the rest with an idle packet (or zeros when fewer than 7
/// bytes are left). Returns the first header pointer (0). If the packet does not fit, it returns an error.
pub fn fill_zone(packet: &[u8], zone: &mut [u8]) -> Result<FirstHeader> {
    if packet.len() > zone.len() {
        return Err(Error::BadLength);
    }
    zone[..packet.len()].copy_from_slice(packet);
    let room = zone.len() - packet.len();
    zone[packet.len()..].fill(0);
    if room >= 7 {
        let h = PrimaryHeader {
            data_len: (room - 6) as u32,
            ..PrimaryHeader::telemetry(IDLE_APID, SeqFlags::Unsegmented, 0)
        };
        zone[packet.len()..packet.len() + 6].copy_from_slice(&h.bytes()?);
    }
    Ok(FirstHeader::At(0))
}

/// The rules of an [`Assembler`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssemblerConfig {
    /// The largest packet accepted (bytes, header included). A longer one counts as a damaged header.
    pub max_packet: usize,
    /// Each packet's data field ends with a CRC-16/IBM-3740 of the rest (the xRIT CP_PDU check). The assembler makes sure that it is correct and removes it.
    pub crc: bool,
    /// Bits of the frame count given to [`Assembler::push_mpdu`] (24 for AOS, 8 for TM).
    pub count_bits: u32,
    /// Give out idle packets too.
    pub keep_idle: bool,
    /// Bytes of a secondary header that the length field does not count: a packet whose secondary header flag is set is this
    /// much longer than its length field says. 0 follows CCSDS 133.0.
    pub uncounted_secondary: u16,
}

impl Default for AssemblerConfig {
    fn default() -> Self {
        AssemblerConfig {
            max_packet: MAX_PACKET,
            crc: false,
            count_bits: 24,
            keep_idle: false,
            uncounted_secondary: 0,
        }
    }
}

/// A packet given out by [`Assembler`]. It borrows the assembler's buffer until the callback returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet<'a> {
    /// The virtual channel it came on.
    pub vcid: u8,
    /// The primary header.
    pub header: PrimaryHeader,
    /// The data field (without the CRC when [`AssemblerConfig::crc`] is on).
    pub data: &'a [u8],
    /// The CRC result when [`AssemblerConfig::crc`] is on.
    pub crc_ok: Option<bool>,
    /// Frames were lost on this channel since the last packet given out.
    pub gap_before: bool,
}

/// Counters of [`Assembler`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AssemblerStats {
    /// Zones taken.
    pub zones: u64,
    /// Packets given out (idle packets not counted).
    pub packets: u64,
    /// Idle packets seen.
    pub idle: u64,
    /// Packets lost: cut by a frame gap, or with a damaged header.
    pub lost: u64,
    /// Packets with a wrong CRC (given out, with `crc_ok` false).
    pub crc_errors: u64,
    /// Zones refused: a first header pointer beyond the zone.
    pub bad_pointers: u64,
    /// Frame count gaps.
    pub gaps: u64,
}

#[derive(Debug, Default)]
struct VcBuf {
    buf: Vec<u8>,
    active: bool,
    gap: bool,
}

/// Reassembles space packets from the M_PDU zones of each virtual channel. See the [module documentation](self).
///
/// ```
/// use sdrkit_proto_ccsds::packet::{build, pack_zones, Assembler, AssemblerConfig, PrimaryHeader, SeqFlags};
/// let mut a = Vec::new();
/// build(&PrimaryHeader::telemetry(5, SeqFlags::Unsegmented, 1), &[9u8; 300], &mut a).unwrap();
/// let zones = pack_zones(&[&a], 100).unwrap();
/// let mut asm = Assembler::new(AssemblerConfig::default());
/// let mut got = Vec::new();
/// for (i, (fh, z)) in zones.iter().enumerate() {
///     asm.push_mpdu(1, Some(i as u32), *fh, z, |p| got.push(p.data.len()));
/// }
/// assert_eq!(got, [300]);
/// ```
#[derive(Debug)]
pub struct Assembler {
    cfg: AssemblerConfig,
    counters: VcCounters,
    vcs: Vec<VcBuf>,
    stats: AssemblerStats,
}

impl Assembler {
    /// An assembler with these rules. If `count_bits` is not 1 to 32, it uses 24.
    pub fn new(cfg: AssemblerConfig) -> Self {
        let counters = VcCounters::new(cfg.count_bits)
            .unwrap_or_else(|_| VcCounters::new(24).expect("24 bits"));
        let mut vcs = Vec::with_capacity(64);
        vcs.resize_with(64, VcBuf::default);
        Assembler {
            cfg,
            counters,
            vcs,
            stats: AssemblerStats::default(),
        }
    }

    /// The counters.
    pub fn stats(&self) -> &AssemblerStats {
        &self.stats
    }

    /// The frame count continuity of each virtual channel.
    pub fn counters(&self) -> &VcCounters {
        &self.counters
    }

    /// Drops the packets in progress and the last counts (a retune).
    pub fn reset(&mut self) {
        for v in &mut self.vcs {
            v.buf.clear();
            v.active = false;
            v.gap = false;
        }
        self.counters.restart();
    }

    /// Takes the packet zone of one frame of virtual channel `vcid` (0 to 63) with its first header pointer. `count` is the
    /// frame's virtual channel frame count, or `None` when the link has no reliable count. Calls `emit` for each packet the
    /// zone completes.
    pub fn push_mpdu(
        &mut self,
        vcid: u8,
        count: Option<u32>,
        first_header: FirstHeader,
        zone: &[u8],
        mut emit: impl FnMut(Packet<'_>),
    ) {
        let vcid = vcid.min(63);
        self.stats.zones += 1;
        let gap = match count.map(|c| self.counters.push(vcid, c)) {
            Some(Continuity::Gap { .. }) => {
                self.stats.gaps += 1;
                true
            }
            _ => false,
        };
        let cfg = self.cfg;
        let st = &mut self.vcs[vcid as usize];
        if st.buf.capacity() == 0 {
            st.buf.reserve_exact(cfg.max_packet + 2048);
        }
        if gap {
            if st.active && !st.buf.is_empty() {
                self.stats.lost += 1;
            }
            st.buf.clear();
            st.active = false;
            st.gap = true;
        }
        match first_header {
            FirstHeader::IdleOnly => {}
            FirstHeader::None => {
                if st.active {
                    st.buf.extend_from_slice(zone);
                    extract(st, vcid, &cfg, &mut self.stats, false, &mut emit);
                }
            }
            FirstHeader::At(p) => {
                let p = p as usize;
                if p >= zone.len() {
                    self.stats.bad_pointers += 1;
                    if st.active && !st.buf.is_empty() {
                        self.stats.lost += 1;
                    }
                    st.buf.clear();
                    st.active = false;
                    return;
                }
                if st.active {
                    st.buf.extend_from_slice(&zone[..p]);
                    extract(st, vcid, &cfg, &mut self.stats, true, &mut emit);
                }
                st.buf.clear();
                st.buf.extend_from_slice(&zone[p..]);
                st.active = true;
                extract(st, vcid, &cfg, &mut self.stats, false, &mut emit);
            }
        }
    }
}

fn extract(
    st: &mut VcBuf,
    vcid: u8,
    cfg: &AssemblerConfig,
    stats: &mut AssemblerStats,
    closing: bool,
    emit: &mut impl FnMut(Packet<'_>),
) {
    let mut pos = 0;
    while st.buf.len() - pos >= 6 {
        let Ok(h) = PrimaryHeader::parse(&st.buf[pos..]) else {
            break;
        };
        let mut len = h.packet_len();
        if h.secondary_header && !h.is_idle() {
            len += usize::from(cfg.uncounted_secondary);
        }
        if h.version != 0 || len > cfg.max_packet || (cfg.crc && !h.is_idle() && h.data_len < 2) {
            // A damaged header: drop what is buffered until the next pointer.
            stats.lost += 1;
            st.buf.clear();
            st.active = false;
            return;
        }
        if st.buf.len() - pos < len {
            break;
        }
        if h.is_idle() {
            stats.idle += 1;
        }
        if !h.is_idle() || cfg.keep_idle {
            let mut data = &st.buf[pos + 6..pos + len];
            let mut crc_ok = None;
            if cfg.crc && !h.is_idle() {
                let n = data.len() - 2;
                let ok = CRC16_IBM_3740.checksum(&data[..n]) == be16(&data[n..]);
                if !ok {
                    stats.crc_errors += 1;
                }
                crc_ok = Some(ok);
                data = &data[..n];
            }
            if !h.is_idle() {
                stats.packets += 1;
            }
            emit(Packet {
                vcid,
                header: h,
                data,
                crc_ok,
                gap_before: std::mem::take(&mut st.gap),
            });
        }
        pos += len;
    }
    let rest = st.buf.len() - pos;
    st.buf.copy_within(pos.., 0);
    st.buf.truncate(rest);
    if closing && !st.buf.is_empty() {
        // The pointer says a new packet starts here: what is left was a damaged packet.
        stats.lost += 1;
        st.buf.clear();
    }
}
