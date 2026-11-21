//! Generic Stream Encapsulation: the receiver ([`GseReader`]) with reassembly, label re-use and extension headers, and the
//! encapsulator ([`GseWriter`]).
//!
//! Standard: ETSI TS 102 606-1, RFC 4326, RFC 5163.

use crate::error::{Error, Result};
use crate::error::{Feature, Limit, Malformed};
use sdrkit_util_bits::crc::catalog::CRC32_MPEG2;

/// The largest GSE_Length (12 bits).
pub const MAX_GSE_LENGTH: usize = 4095;
/// The largest reassembled PDU: Total_Length is 16 bits.
pub const MAX_TOTAL: usize = 65535;

/// The label of a GSE PDU.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Label {
    /// LT = 10: broadcast, no label.
    #[default]
    Broadcast,
    /// LT = 01: a 3-byte label.
    Three([u8; 3]),
    /// LT = 00: a 6-byte label (a MAC address).
    Six([u8; 6]),
}

impl Label {
    /// The label bytes (empty for broadcast).
    pub fn bytes(&self) -> &[u8] {
        match self {
            Label::Broadcast => &[],
            Label::Three(b) => b,
            Label::Six(b) => b,
        }
    }
    fn lt(&self) -> u16 {
        match self {
            Label::Six(_) => 0,
            Label::Three(_) => 1,
            Label::Broadcast => 2,
        }
    }
}

/// What the extension headers of a PDU said.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Extensions {
    /// Optional extension headers skipped.
    pub optional: u8,
    /// The 32-bit timestamp of a Timestamp extension header (type 0x0301), when present.
    pub timestamp: Option<u32>,
    /// True when the PDU was a Bridged Frame (type 0x0001): `pdu` is then an Ethernet frame and `protocol` its EtherType.
    pub bridged: bool,
}

/// One whole PDU out of the reader.
#[derive(Clone, Copy, Debug)]
pub struct GsePdu<'a> {
    /// The protocol of the PDU after the extension headers (0x0800 IPv4, 0x86DD IPv6 ...).
    pub protocol: u16,
    /// The label.
    pub label: Label,
    /// What the extension headers said.
    pub ext: Extensions,
    /// The PDU.
    pub pdu: &'a [u8],
    /// True when the PDU came in fragments (its CRC-32 was right).
    pub reassembled: bool,
}

/// Counters of the reader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GseStats {
    /// GSE packets read (padding not included).
    pub packets: u64,
    /// PDUs delivered.
    pub pdus: u64,
    /// Reassembled PDUs whose CRC-32 was wrong.
    pub crc_errors: u64,
    /// Reassembled PDUs whose length was not Total_Length.
    pub length_errors: u64,
    /// Fragments with no start fragment (lost or evicted).
    pub orphans: u64,
    /// Reassemblies that a new start with the same Frag_ID, the age limit or a lack of buffers ended.
    pub dropped: u64,
    /// PDUs discarded for a mandatory extension header this reader does not know, or a Test PDU.
    pub ext_errors: u64,
    /// Packets discarded for label re-use with no label before them in the frame.
    pub label_errors: u64,
    /// Packets that did not fit the data field or had a bad header.
    pub bad: u64,
    /// Times the reader of cut packets (GSE-HEM) lost the packet chain and started again at SYNCD.
    pub resyncs: u64,
}

#[derive(Clone, Debug)]
struct Slot {
    active: bool,
    fid: u8,
    started: u64,
    /// Total_Length bytes, then Protocol_Type, label, extension headers and PDU as they arrive.
    data: Vec<u8>,
    /// The label of the start packet.
    label: Label,
    /// The label bytes in `data` (0 for label re-use).
    label_bytes: usize,
}

/// Reads GSE packets out of data fields and reassembles fragmented PDUs, with up to `slots` PDUs in reassembly at a time
/// (annex A.2 lets a receiver use fewer than 256). A reassembly older than `max_age` data fields is dropped. After the first
/// data fields it does not allocate.
#[derive(Clone, Debug)]
pub struct GseReader {
    slots: Vec<Slot>,
    max_age: u64,
    frame: u64,
    last_label: Option<Label>,
    carry: Vec<u8>,
    sliced_synced: bool,
    /// The counters.
    pub stats: GseStats,
}

impl Default for GseReader {
    fn default() -> Self {
        Self::new(16, 256)
    }
}

impl GseReader {
    /// A reader with `slots` reassembly buffers (1 to 256) that drops a reassembly after `max_age` data fields.
    pub fn new(slots: usize, max_age: u64) -> GseReader {
        let n = slots.clamp(1, 256);
        GseReader {
            slots: (0..n)
                .map(|_| Slot {
                    active: false,
                    fid: 0,
                    started: 0,
                    data: Vec::new(),
                    label: Label::Broadcast,
                    label_bytes: 0,
                })
                .collect(),
            max_age: max_age.max(1),
            frame: 0,
            last_label: None,
            carry: Vec::with_capacity(MAX_GSE_LENGTH + 2),
            sliced_synced: false,
            stats: GseStats::default(),
        }
    }

    /// Drops every reassembly and any packet cut across data fields (after a lost frame).
    pub fn reset(&mut self) {
        for s in &mut self.slots {
            s.active = false;
        }
        self.carry.clear();
        self.sliced_synced = false;
    }

    /// Reads one data field whose GSE packets do not cross its end (normal mode). PDUs go to `out`.
    pub fn push(&mut self, df: &[u8], out: &mut impl FnMut(GsePdu<'_>)) {
        self.start_frame();
        let mut i = 0;
        while df.len() - i >= 2 {
            match packet_len(&df[i..]) {
                None => break, // padding
                Some(n) if i + n > df.len() => {
                    self.stats.bad += 1;
                    break;
                }
                Some(n) => {
                    self.packet(&df[i..i + n], out);
                    i += n;
                }
            }
        }
    }

    /// Reads one data field whose GSE packets can continue in the next one (GSE-HEM of DVB-S2X, GSE in DVB-T2 HEM). `syncd` is
    /// the SYNCD of the header in bits: the first packet that starts in this field. The reader joins the parts of a packet cut
    /// across fields. After [`GseReader::reset`] it starts at SYNCD.
    pub fn push_sliced(&mut self, df: &[u8], syncd: u16, out: &mut impl FnMut(GsePdu<'_>)) {
        self.start_frame();
        let mut i = 0;
        if self.sliced_synced && !self.agrees(df, syncd) {
            // Complete GSE packets have no CRC, so a lost field would join the kept head to an unrelated tail. SYNCD points to
            // the start of the next packet. If that is not where the kept packet ends, start again at SYNCD.
            self.stats.resyncs += 1;
            self.sliced_synced = false;
        }
        if !self.sliced_synced {
            self.carry.clear();
            if syncd == 0xFFFF || !syncd.is_multiple_of(8) || syncd as usize / 8 > df.len() {
                return;
            }
            i = syncd as usize / 8;
            self.sliced_synced = true;
        }
        if !self.carry.is_empty() {
            // Complete the packet kept from the last field: first its 2-byte header, then its body.
            while self.carry.len() < 2 && i < df.len() {
                self.carry.push(df[i]);
                i += 1;
            }
            let Some(n) = packet_len(&self.carry) else {
                self.carry.clear();
                return;
            };
            let need = n - self.carry.len();
            let take = need.min(df.len() - i);
            self.carry.extend_from_slice(&df[i..i + take]);
            i += take;
            if self.carry.len() < n {
                return;
            }
            let pkt = std::mem::take(&mut self.carry);
            self.packet(&pkt, out);
            self.carry = pkt;
            self.carry.clear();
        }
        while i < df.len() {
            if df.len() - i < 2 {
                self.carry.extend_from_slice(&df[i..]);
                return;
            }
            match packet_len(&df[i..]) {
                None => return,
                Some(n) if i + n > df.len() => {
                    self.carry.extend_from_slice(&df[i..]);
                    return;
                }
                Some(n) => {
                    self.packet(&df[i..i + n], out);
                    i += n;
                }
            }
        }
    }

    /// Whether SYNCD agrees with the packet kept from the last field: the first packet start in `df` must be where SYNCD says.
    fn agrees(&self, df: &[u8], syncd: u16) -> bool {
        let first_start = if self.carry.is_empty() {
            Some(0)
        } else {
            // The header of the kept packet, completed from this field when it was cut inside its 2 bytes.
            let mut h = [0u8; 2];
            for (k, b) in h.iter_mut().enumerate() {
                *b = match self.carry.get(k) {
                    Some(&x) => x,
                    None => match df.get(k - self.carry.len()) {
                        Some(&x) => x,
                        None => return syncd == 0xFFFF,
                    },
                };
            }
            match packet_len(&h) {
                Some(n) => Some(n - self.carry.len()).filter(|&e| e < df.len()),
                None => return false,
            }
        };
        match first_start {
            None => syncd == 0xFFFF,
            Some(e) if syncd == 0xFFFF => packet_len(&df[e..]).is_none(),
            Some(e) => syncd as usize == e * 8,
        }
    }

    fn start_frame(&mut self) {
        self.frame += 1;
        self.last_label = None;
        let (frame, max_age) = (self.frame, self.max_age);
        for s in &mut self.slots {
            if s.active && frame - s.started > max_age {
                s.active = false;
                self.stats.dropped += 1;
            }
        }
    }

    /// One whole GSE packet, header included.
    fn packet(&mut self, p: &[u8], out: &mut impl FnMut(GsePdu<'_>)) {
        self.stats.packets += 1;
        let h = u16::from_be_bytes([p[0], p[1]]);
        let (s, e, lt) = (h & 0x8000 != 0, h & 0x4000 != 0, (h >> 12) & 3);
        let b = &p[2..];
        match (s, e) {
            (true, true) => {
                let Some((label, used)) = self.read_label(lt, &b[2.min(b.len())..]) else {
                    return;
                };
                if b.len() < 2 + used {
                    self.stats.bad += 1;
                    return;
                }
                let proto = u16::from_be_bytes([b[0], b[1]]);
                self.deliver(proto, label, &b[2 + used..], false, out);
            }
            (true, false) => {
                if b.len() < 5 {
                    self.stats.bad += 1;
                    return;
                }
                let fid = b[0];
                let total = u16::from_be_bytes([b[1], b[2]]) as usize;
                let Some((label, used)) = self.read_label(lt, &b[5..]) else {
                    return;
                };
                if b.len() < 5 + used || total < 2 + used {
                    self.stats.bad += 1;
                    return;
                }
                let slot = self.slot_for_start(fid);
                let frame = self.frame;
                let s = &mut self.slots[slot];
                s.active = true;
                s.fid = fid;
                s.started = frame;
                s.label = label;
                s.label_bytes = used;
                s.data.clear();
                s.data.reserve(MAX_TOTAL + 2);
                // Total_Length, Protocol_Type, then the label bytes as sent, then the first part of the PDU.
                s.data.extend_from_slice(&b[1..5]);
                s.data.extend_from_slice(&b[5..]);
            }
            (false, end) => {
                if b.is_empty() || (end && b.len() < 5) {
                    self.stats.bad += 1;
                    return;
                }
                let fid = b[0];
                let Some(k) = self.slots.iter().position(|s| s.active && s.fid == fid) else {
                    self.stats.orphans += 1;
                    return;
                };
                let body = if end { &b[1..b.len() - 4] } else { &b[1..] };
                let s = &mut self.slots[k];
                let total = u16::from_be_bytes([s.data[0], s.data[1]]) as usize;
                if s.data.len() - 2 + body.len() > total {
                    // Annex A.2: more bytes than Total_Length ends the reassembly.
                    s.active = false;
                    self.stats.length_errors += 1;
                    return;
                }
                s.data.extend_from_slice(body);
                if !end {
                    return;
                }
                s.active = false;
                if s.data.len() - 2 != total {
                    self.stats.length_errors += 1;
                    return;
                }
                let crc = u32::from_be_bytes([
                    b[b.len() - 4],
                    b[b.len() - 3],
                    b[b.len() - 2],
                    b[b.len() - 1],
                ]);
                if CRC32_MPEG2.checksum(&s.data) != crc {
                    self.stats.crc_errors += 1;
                    return;
                }
                let label = s.label;
                let proto = u16::from_be_bytes([s.data[2], s.data[3]]);
                let skip = 4 + s.label_bytes;
                let data = std::mem::take(&mut s.data);
                self.deliver(proto, label, &data[skip..], true, out);
                self.slots[k].data = data;
            }
        }
    }

    /// The label of a start or complete packet and the bytes it uses. `None` (counted) for label re-use with no label before.
    fn read_label(&mut self, lt: u16, b: &[u8]) -> Option<(Label, usize)> {
        let r = match lt {
            0 => b
                .get(..6)
                .map(|l| (Label::Six(l.try_into().unwrap_or_default()), 6)),
            1 => b
                .get(..3)
                .map(|l| (Label::Three(l.try_into().unwrap_or_default()), 3)),
            2 => Some((Label::Broadcast, 0)),
            _ => match self.last_label {
                Some(l) => Some((l, 0)),
                None => {
                    self.stats.label_errors += 1;
                    return None;
                }
            },
        };
        match r {
            Some((l, n)) => {
                self.last_label = Some(l);
                Some((l, n))
            }
            None => {
                self.stats.bad += 1;
                None
            }
        }
    }

    fn slot_for_start(&mut self, fid: u8) -> usize {
        if let Some(k) = self.slots.iter().position(|s| s.active && s.fid == fid) {
            // Annex A.2: a new start with a Frag_ID in use discards the old fragments.
            self.stats.dropped += 1;
            return k;
        }
        if let Some(k) = self.slots.iter().position(|s| !s.active) {
            return k;
        }
        // Every buffer is busy: drop the oldest reassembly.
        self.stats.dropped += 1;
        let mut k = 0;
        for (i, s) in self.slots.iter().enumerate() {
            if s.started < self.slots[k].started {
                k = i;
            }
        }
        k
    }

    fn deliver(
        &mut self,
        proto: u16,
        label: Label,
        data: &[u8],
        reassembled: bool,
        out: &mut impl FnMut(GsePdu<'_>),
    ) {
        match parse_extensions(proto, data) {
            Ok((protocol, ext, pdu)) => {
                self.stats.pdus += 1;
                out(GsePdu {
                    protocol,
                    label,
                    ext,
                    pdu,
                    reassembled,
                });
            }
            Err(_) => self.stats.ext_errors += 1,
        }
    }
}

/// The length of the GSE packet at the start of `b` (header included), or `None` for padding.
fn packet_len(b: &[u8]) -> Option<usize> {
    let h = u16::from_be_bytes([b[0], *b.get(1)?]);
    if h & 0xF000 == 0 {
        return None;
    }
    Some(2 + (h & 0x0FFF) as usize)
}

/// Follows the extension headers from `proto` (RFC 4326 clause 5). It returns the final protocol, what the headers said and the
/// PDU after them.
pub fn parse_extensions(mut proto: u16, mut data: &[u8]) -> Result<(u16, Extensions, &[u8])> {
    let mut ext = Extensions::default();
    for _ in 0..16 {
        if proto >= 1536 {
            return Ok((proto, ext, data));
        }
        let h_len = ((proto >> 8) & 7) as usize;
        let h_type = proto & 0xFF;
        if proto < 256 {
            return match proto {
                0x0001 => {
                    // Bridged Frame: an Ethernet header (destination, source, EtherType) and its payload.
                    if data.len() < 14 {
                        return Err(Error::Short);
                    }
                    ext.bridged = true;
                    Ok((u16::from_be_bytes([data[12], data[13]]), ext, data))
                }
                0x0000 => Err(Error::Unsupported(Feature::TestSndu)),
                _ => Err(Error::Unsupported(Feature::MandatoryExtensionHeader)),
            };
        }
        if !(1..=5).contains(&h_len) {
            return Err(Error::Invalid(Malformed::HLen));
        }
        let n = 2 * h_len;
        if data.len() < n {
            return Err(Error::Short);
        }
        if h_type == 1 && h_len == 3 {
            ext.timestamp = Some(u32::from_be_bytes([data[0], data[1], data[2], data[3]]));
        }
        ext.optional += 1;
        proto = u16::from_be_bytes([data[n - 2], data[n - 1]]);
        data = &data[n..];
    }
    Err(Error::TooLarge(Limit::ExtensionHeaderChain))
}

/// Encapsulates PDUs into GSE packets that fill data fields (for test signals and transmitters). A PDU goes as one complete
/// packet when it fits the field and GSE_Length. Else it goes as start, intermediate and end fragments with the CRC-32. Packets
/// do not cross the end of a data field (normal mode). After the first PDUs it does not allocate.
#[derive(Clone, Debug, Default)]
pub struct GseWriter {
    /// Queued PDUs: protocol, label, start and end in `store`.
    queue: std::collections::VecDeque<(u16, Label, usize, usize)>,
    store: Vec<u8>,
    /// The PDU in fragments: Frag_ID, bytes of its CRC-covered data sent, its CRC-32.
    frag: Option<(u8, usize, u32)>,
    next_fid: u8,
    scratch: Vec<u8>,
}

impl GseWriter {
    /// Queues one PDU of `protocol` with `label`. If the PDU and its header are longer than Total_Length allows, it returns
    /// [`Error::TooLarge`].
    pub fn push_pdu(&mut self, protocol: u16, label: Label, pdu: &[u8]) -> Result<()> {
        if 2 + label.bytes().len() + pdu.len() > MAX_TOTAL {
            return Err(Error::TooLarge(Limit::Pdu));
        }
        if self.queue.is_empty() && self.frag.is_none() {
            self.store.clear();
        }
        let a = self.store.len();
        self.store.extend_from_slice(pdu);
        self.queue.push_back((protocol, label, a, self.store.len()));
        Ok(())
    }

    /// Bytes of PDUs not yet sent.
    pub fn backlog(&self) -> usize {
        self.queue.iter().map(|q| q.3 - q.2).sum()
    }

    /// Fills one data field with GSE packets and zero padding. It returns the bytes used by packets.
    pub fn fill(&mut self, df: &mut [u8]) -> usize {
        df.fill(0);
        let mut i = 0;
        while let Some(&(proto, label, a, b)) = self.queue.front() {
            let room = df.len() - i;
            let pdu_len = b - a;
            let lb = label.bytes().len();
            if let Some((fid, sent, crc)) = self.frag {
                // A continuation: header, Frag_ID, data, and the CRC-32 in the end packet.
                if room < 4 {
                    break;
                }
                // `sent` counts Total_Length, Protocol_Type, label and PDU bytes already in packets.
                self.covered(proto, label, a, b);
                let left = self.scratch.len() - sent;
                let space = (room - 3).min(MAX_GSE_LENGTH - 1);
                if left + 4 <= space {
                    let glen = 1 + left + 4;
                    write_header(&mut df[i..], false, true, 3, glen);
                    df[i + 2] = fid;
                    df[i + 3..i + 3 + left].copy_from_slice(&self.scratch[sent..]);
                    df[i + 3 + left..i + 7 + left].copy_from_slice(&crc.to_be_bytes());
                    i += 2 + glen;
                    self.frag = None;
                    self.queue.pop_front();
                    continue;
                }
                let n = left.min(space);
                if n == 0 {
                    break;
                }
                write_header(&mut df[i..], false, false, 3, 1 + n);
                df[i + 2] = fid;
                df[i + 3..i + 3 + n].copy_from_slice(&self.scratch[sent..sent + n]);
                self.frag = Some((fid, sent + n, crc));
                i += 3 + n;
                break;
            }
            let whole = 2 + lb + pdu_len;
            if 2 + whole <= room && whole <= MAX_GSE_LENGTH {
                write_header(&mut df[i..], true, true, label.lt(), whole);
                df[i + 2..i + 4].copy_from_slice(&proto.to_be_bytes());
                df[i + 4..i + 4 + lb].copy_from_slice(label.bytes());
                df[i + 4 + lb..i + 2 + whole].copy_from_slice(&self.store[a..b]);
                i += 2 + whole;
                self.queue.pop_front();
                continue;
            }
            // A start fragment: header, Frag_ID, Total_Length, Protocol_Type, label, then part of the PDU.
            let fixed = 2 + 1 + 2 + 2 + lb;
            if room < fixed + 1 {
                break;
            }
            self.covered(proto, label, a, b);
            let crc = CRC32_MPEG2.checksum(&self.scratch);
            let n = (room - fixed)
                .min(MAX_GSE_LENGTH - (fixed - 2))
                .min(pdu_len);
            let glen = fixed - 2 + n;
            let fid = self.next_fid;
            self.next_fid = self.next_fid.wrapping_add(1);
            write_header(&mut df[i..], true, false, label.lt(), glen);
            df[i + 2] = fid;
            // scratch = Total_Length, Protocol_Type, label, PDU: send its first 4 + lb + n bytes.
            df[i + 3..i + 2 + glen].copy_from_slice(&self.scratch[..4 + lb + n]);
            self.frag = Some((fid, 4 + lb + n, crc));
            i += 2 + glen;
            break;
        }
        i
    }

    /// Puts the CRC-covered bytes of the queued PDU in `scratch`: Total_Length, Protocol_Type, label, PDU.
    fn covered(&mut self, proto: u16, label: Label, a: usize, b: usize) {
        let total = (2 + label.bytes().len() + (b - a)) as u16;
        self.scratch.clear();
        self.scratch.extend_from_slice(&total.to_be_bytes());
        self.scratch.extend_from_slice(&proto.to_be_bytes());
        self.scratch.extend_from_slice(label.bytes());
        self.scratch.extend_from_slice(&self.store[a..b]);
    }
}

fn write_header(b: &mut [u8], s: bool, e: bool, lt: u16, glen: usize) {
    let h = (s as u16) << 15 | (e as u16) << 14 | (lt & 3) << 12 | (glen as u16 & 0x0FFF);
    b[..2].copy_from_slice(&h.to_be_bytes());
}
