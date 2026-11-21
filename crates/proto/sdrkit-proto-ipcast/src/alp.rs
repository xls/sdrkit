//! ATSC 3.0 baseband packet headers and ALP, the link layer protocol: single, segmented and concatenated packets,
//! signalling and TS packets, read by [`AlpReader`] and written by the builders.
//!
//! Standard: ATSC A/322 clause 5.2.2, ATSC A/330 clause 5.

use crate::adapt::NULL_PACKET;
use crate::error::{Error, Result};
use crate::error::{Limit, Malformed};

/// The pointer value for "no ALP packet starts in this baseband packet".
pub const NO_POINTER: u16 = 8191;

/// ALP packet types (A/330 table 5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketType {
    /// 000: IPv4.
    Ipv4,
    /// 010: compressed IP (ROHC-U).
    CompressedIp,
    /// 100: link layer signalling.
    Signalling,
    /// 110: packet type extension.
    Extension,
    /// 111: MPEG-2 transport stream.
    Mpeg2Ts,
    /// 001, 011, 101: reserved.
    Reserved(u8),
}

impl PacketType {
    /// The type of a 3-bit code.
    pub fn from_code(c: u8) -> PacketType {
        match c & 7 {
            0 => PacketType::Ipv4,
            2 => PacketType::CompressedIp,
            4 => PacketType::Signalling,
            6 => PacketType::Extension,
            7 => PacketType::Mpeg2Ts,
            c => PacketType::Reserved(c),
        }
    }
    /// The 3-bit code.
    pub fn code(self) -> u8 {
        match self {
            PacketType::Ipv4 => 0,
            PacketType::CompressedIp => 2,
            PacketType::Signalling => 4,
            PacketType::Extension => 6,
            PacketType::Mpeg2Ts => 7,
            PacketType::Reserved(c) => c & 7,
        }
    }
}

/// A parsed baseband packet header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BbpHeader {
    /// The offset of the first ALP packet start in the payload, or [`NO_POINTER`].
    pub pointer: u16,
    /// OFI, the optional field indicator (0 when the base field is 1 byte).
    pub ofi: u8,
    /// The counter extension, when sent.
    pub counter: Option<u16>,
    /// The header length in bytes: base field, optional field and extension field.
    pub len: usize,
}

impl BbpHeader {
    /// Reads a baseband packet header.
    pub fn parse(b: &[u8]) -> Result<BbpHeader> {
        let b0 = *b.first().ok_or(Error::Short)?;
        if b0 & 0x80 == 0 {
            return Ok(BbpHeader {
                pointer: (b0 & 0x7F) as u16,
                len: 1,
                ..BbpHeader::default()
            });
        }
        let b1 = *b.get(1).ok_or(Error::Short)?;
        let mut h = BbpHeader {
            pointer: (b0 & 0x7F) as u16 | ((b1 >> 2) as u16) << 7,
            ofi: b1 & 3,
            len: 2,
            counter: None,
        };
        let byte = |i: usize| b.get(i).copied().ok_or(Error::Short);
        match h.ofi {
            0 => {}
            1 | 2 => {
                let o = byte(2)?;
                let (ty, n, opt) = if h.ofi == 1 {
                    (o >> 5, (o & 0x1F) as usize, 1)
                } else {
                    (o >> 5, (o & 0x1F) as usize | (byte(3)? as usize) << 5, 2)
                };
                let ext = b.get(2 + opt..2 + opt + n).ok_or(Error::Short)?;
                if ty == 0 {
                    h.counter = counter(ext)?;
                }
                h.len = 2 + opt + n;
            }
            _ => {
                let o = byte(2)?;
                let num = (o >> 5) as usize;
                let total = (o & 0x1F) as usize | (byte(3)? as usize) << 5;
                let ext = b.get(4..4 + total).ok_or(Error::Short)?;
                if 2 * num > ext.len() {
                    return Err(Error::Invalid(Malformed::BbpExtensionHeaders));
                }
                let mut at = 2 * num;
                for k in 0..num {
                    let ty = ext[2 * k] >> 5;
                    let n = (ext[2 * k] & 0x1F) as usize | (ext[2 * k + 1] as usize) << 5;
                    let payload = ext
                        .get(at..at + n)
                        .ok_or(Error::Invalid(Malformed::BbpExtensionLength))?;
                    if ty == 0 {
                        h.counter = counter(payload)?;
                    }
                    at += n;
                }
                h.len = 4 + total;
            }
        }
        Ok(h)
    }
}

fn counter(ext: &[u8]) -> Result<Option<u16>> {
    match ext.len() {
        0 => Ok(None),
        1 => Ok(Some(ext[0] as u16)),
        2 => Ok(Some(u16::from_be_bytes([ext[0], ext[1]]))),
        _ => Err(Error::Invalid(Malformed::CounterLength)),
    }
}

/// What a written baseband packet header carries after the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BbpExtension {
    /// OFI 00 (or a 1-byte base field when the pointer is below 128).
    None,
    /// OFI 01 with a counter of `bytes` (1 or 2) bytes.
    Counter {
        /// The counter value.
        value: u16,
        /// The counter length.
        bytes: u8,
    },
    /// Padding of `n` bytes after the base field: OFI 01 with EXT_TYPE 111 for up to 32 bytes, else OFI 10.
    Padding(usize),
}

/// Writes a baseband packet header at the end of `out`.
pub fn write_bbp_header(pointer: u16, ext: BbpExtension, out: &mut Vec<u8>) -> Result<()> {
    if pointer > NO_POINTER {
        return Err(Error::Invalid(Malformed::Pointer));
    }
    if pointer < 128 && ext == BbpExtension::None {
        out.push(pointer as u8);
        return Ok(());
    }
    let base = |ofi: u8, out: &mut Vec<u8>| {
        out.push(0x80 | (pointer & 0x7F) as u8);
        out.push(((pointer >> 7) as u8) << 2 | ofi);
    };
    match ext {
        BbpExtension::None => base(0, out),
        BbpExtension::Counter { value, bytes } => {
            if !(1..=2).contains(&bytes) || bytes == 1 && value > 255 {
                return Err(Error::Invalid(Malformed::Counter));
            }
            base(1, out);
            out.push(bytes);
            if bytes == 2 {
                out.extend_from_slice(&value.to_be_bytes());
            } else {
                out.push(value as u8);
            }
        }
        BbpExtension::Padding(n) => {
            if n == 0 {
                base(0, out);
            } else if n <= 32 {
                // The optional field is the first padding byte, then n - 1 zero bytes.
                base(1, out);
                out.push(0xE0 | (n as u8 - 1));
                out.resize(out.len() + n - 1, 0);
            } else {
                let e = n - 2;
                if e > 0x1FFF {
                    return Err(Error::TooLarge(Limit::Padding));
                }
                base(2, out);
                out.push(0xE0 | (e & 0x1F) as u8);
                out.push((e >> 5) as u8);
                out.resize(out.len() + e, 0);
            }
        }
    }
    Ok(())
}

/// The signalling information header of a signalling packet (A/330 table 5.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignallingHeader {
    /// signaling_type: 1 is the link mapping table, 2 the ROHC-U description table.
    pub kind: u8,
    /// signaling_type_extension.
    pub extension: u16,
    /// signaling_version.
    pub version: u8,
    /// signaling_format: 0 binary, 1 XML, 2 JSON.
    pub format: u8,
    /// signaling_encoding: 0 none, 1 DEFLATE.
    pub encoding: u8,
}

/// One ALP packet out of the reader (not TS: TS packets come one by one as [`AlpEvent::Ts`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlpPacket<'a> {
    /// The packet type.
    pub kind: PacketType,
    /// The sub-stream identifier, when SIF is set.
    pub sid: Option<u8>,
    /// The header extension type and bytes, when HEF is set.
    pub header_extension: Option<(u8, &'a [u8])>,
    /// The signalling information header of a signalling packet.
    pub signalling: Option<SignallingHeader>,
    /// The extended_type of a type extension packet.
    pub extended_type: Option<u16>,
    /// The payload. For a segmented packet, the payload made whole.
    pub payload: &'a [u8],
}

/// What the ALP reader provides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlpEvent<'a> {
    /// A packet.
    Packet(AlpPacket<'a>),
    /// One 188-byte transport packet, its sync byte and (with HDM) its header put back. `null` is true for a deleted null packet
    /// put back.
    Ts {
        /// The packet.
        packet: &'a [u8; 188],
        /// True for a null packet put back.
        null: bool,
    },
}

/// Counters of the reader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AlpStats {
    /// Packets read.
    pub packets: u64,
    /// Packets dropped as malformed, and segments out of order.
    pub dropped: u64,
    /// Baseband packets with a bad header.
    pub bad_headers: u64,
    /// Bytes dropped while looking for the first packet start after a gap.
    pub skipped: u64,
    /// Packets of type 2 (compressed IP, ROHC-U), which this reader does not decode.
    pub compressed_ip: u64,
}

/// The largest ALP payload this reader holds in one buffer.
pub const MAX_PAYLOAD: usize = 65535;

/// Reads ALP packets from baseband packets (or from the joined payloads), carrying a packet cut across baseband packets over to
/// the next. It does not allocate after [`AlpReader::new`].
#[derive(Clone, Debug)]
pub struct AlpReader {
    buf: Vec<u8>,
    seg: Vec<u8>,
    seg_next: Option<u8>,
    seg_kind: u8,
    seg_sid: Option<u8>,
    synced: bool,
    ts: [u8; 188],
    /// The counters.
    pub stats: AlpStats,
}

impl Default for AlpReader {
    fn default() -> Self {
        Self::new()
    }
}

impl AlpReader {
    /// A reader with its buffers made.
    pub fn new() -> AlpReader {
        AlpReader {
            buf: Vec::with_capacity(2 * (MAX_PAYLOAD + 300)),
            seg: Vec::with_capacity(MAX_PAYLOAD + 8),
            seg_next: None,
            seg_kind: 0,
            seg_sid: None,
            synced: false,
            ts: [0; 188],
            stats: AlpStats::default(),
        }
    }

    /// Starts again after a gap (a lost baseband packet): bytes up to the next pointer are dropped.
    pub fn gap(&mut self) {
        self.buf.clear();
        self.seg.clear();
        self.seg_next = None;
        self.synced = false;
    }

    /// Takes one baseband packet (descrambled, header included).
    pub fn push_bbp(&mut self, bbp: &[u8], out: &mut impl FnMut(AlpEvent<'_>)) {
        let h = match BbpHeader::parse(bbp) {
            Ok(h) => h,
            Err(_) => {
                self.stats.bad_headers += 1;
                self.gap();
                return;
            }
        };
        let payload = &bbp[h.len.min(bbp.len())..];
        if !self.synced {
            if h.pointer == NO_POINTER {
                self.stats.skipped += payload.len() as u64;
                return;
            }
            let p = h.pointer as usize;
            if p > payload.len() {
                self.stats.bad_headers += 1;
                return;
            }
            self.stats.skipped += p as u64;
            self.synced = true;
            self.push_bytes(&payload[p..], out);
        } else {
            self.push_bytes(payload, out);
        }
    }

    /// Takes the next bytes of the ALP stream.
    pub fn push_bytes(&mut self, bytes: &[u8], out: &mut impl FnMut(AlpEvent<'_>)) {
        let mut rest = bytes;
        while !rest.is_empty() {
            let room = self.buf.capacity() - self.buf.len();
            if room == 0 {
                // A packet longer than any ALP packet: the stream is lost.
                self.stats.dropped += 1;
                self.gap();
                return;
            }
            let n = room.min(rest.len());
            self.buf.extend_from_slice(&rest[..n]);
            rest = &rest[n..];
            let used = self.drain(out);
            self.buf.drain(..used);
        }
    }

    /// Reads whole packets from the buffer. It returns the bytes used.
    fn drain(&mut self, out: &mut impl FnMut(AlpEvent<'_>)) -> usize {
        let mut at = 0;
        loop {
            let b = &self.buf[at..];
            match packet_len(b) {
                Ok(None) => return at,
                Ok(Some(n)) => {
                    self.packet(at, at + n, out);
                    at += n;
                }
                Err(_) => {
                    // A header that cannot be right: the stream is lost until the next pointer.
                    self.stats.dropped += 1;
                    self.synced = false;
                    self.seg.clear();
                    self.seg_next = None;
                    return self.buf.len();
                }
            }
        }
    }

    /// One whole ALP packet at `buf[start..end]`.
    fn packet(&mut self, start: usize, end: usize, out: &mut impl FnMut(AlpEvent<'_>)) {
        self.stats.packets += 1;
        let buf = std::mem::take(&mut self.buf);
        let r = self.packet_inner(&buf[start..end], out);
        self.buf = buf;
        if r.is_err() {
            self.stats.dropped += 1;
        }
    }

    fn packet_inner(&mut self, p: &[u8], out: &mut impl FnMut(AlpEvent<'_>)) -> Result<()> {
        let kind = PacketType::from_code(p[0] >> 5);
        if kind == PacketType::Mpeg2Ts {
            return self.ts_packets(p, out);
        }
        if kind == PacketType::CompressedIp {
            self.stats.compressed_ip += 1;
        }
        let h = Header::parse(p)?;
        let mut at = h.len;
        let sig = |kind: PacketType,
                   b: &[u8],
                   at: &mut usize|
         -> Result<(Option<SignallingHeader>, Option<u16>)> {
            match kind {
                PacketType::Signalling => {
                    let s = b.get(*at..*at + 5).ok_or(Error::Short)?;
                    *at += 5;
                    Ok((
                        Some(SignallingHeader {
                            kind: s[0],
                            extension: u16::from_be_bytes([s[1], s[2]]),
                            version: s[3],
                            format: s[4] >> 6,
                            encoding: (s[4] >> 4) & 3,
                        }),
                        None,
                    ))
                }
                PacketType::Extension => {
                    let s = b.get(*at..*at + 2).ok_or(Error::Short)?;
                    *at += 2;
                    Ok((None, Some(u16::from_be_bytes([s[0], s[1]]))))
                }
                _ => Ok((None, None)),
            }
        };
        match h.form {
            Form::Single => {
                let (signalling, extended_type) = sig(kind, p, &mut at)?;
                let payload = p.get(at..at + h.payload_len).ok_or(Error::Short)?;
                out(AlpEvent::Packet(AlpPacket {
                    kind,
                    sid: h.sid,
                    header_extension: h.ext.map(|(t, a, b)| (t, &p[a..b])),
                    signalling,
                    extended_type,
                    payload,
                }));
            }
            Form::Segment { seq, last } => {
                let part = p.get(at..at + h.payload_len).ok_or(Error::Short)?;
                if seq == 0 {
                    self.seg.clear();
                    self.seg_kind = kind.code();
                    self.seg_sid = h.sid;
                    self.seg_next = Some(1);
                } else if self.seg_next != Some(seq) || self.seg_kind != kind.code() {
                    self.seg.clear();
                    self.seg_next = None;
                    return Err(Error::Invalid(Malformed::SegmentSequence));
                } else {
                    self.seg_next = Some(seq + 1);
                }
                if self.seg.len() + part.len() > self.seg.capacity() {
                    self.seg_next = None;
                    return Err(Error::TooLarge(Limit::SegmentedPacket));
                }
                self.seg.extend_from_slice(part);
                if last {
                    self.seg_next = None;
                    let whole = std::mem::take(&mut self.seg);
                    let mut a = 0;
                    let r = sig(kind, &whole, &mut a).map(|(signalling, extended_type)| {
                        out(AlpEvent::Packet(AlpPacket {
                            kind,
                            sid: self.seg_sid,
                            header_extension: None,
                            signalling,
                            extended_type,
                            payload: &whole[a..],
                        }))
                    });
                    self.seg = whole;
                    self.seg.clear();
                    r?;
                }
            }
            Form::Concat { lens, count } => {
                let (signalling, extended_type) = sig(kind, p, &mut at)?;
                let mut sum = 0;
                for &l in &lens[..count - 1] {
                    sum += l as usize;
                }
                if sum >= h.payload_len {
                    return Err(Error::Invalid(Malformed::ComponentLengths));
                }
                let mut o = at;
                for (k, &given) in lens.iter().enumerate().take(count) {
                    let l = if k + 1 < count {
                        given as usize
                    } else {
                        h.payload_len - sum
                    };
                    let payload = p.get(o..o + l).ok_or(Error::Short)?;
                    out(AlpEvent::Packet(AlpPacket {
                        kind,
                        sid: h.sid,
                        header_extension: None,
                        signalling,
                        extended_type,
                        payload,
                    }));
                    o += l;
                }
            }
        }
        Ok(())
    }

    fn ts_packets(&mut self, p: &[u8], out: &mut impl FnMut(AlpEvent<'_>)) -> Result<()> {
        let (n, ahf) = ts_count(p[0]);
        let (hdm, dnp) = if ahf {
            let a = *p.get(1).ok_or(Error::Short)?;
            let hdm = a & 0x80 != 0;
            let d = (a & 0x7F) as usize;
            (hdm, if d == 0 && !hdm { 128 } else { d })
        } else {
            (false, 0)
        };
        for _ in 0..dnp {
            out(AlpEvent::Ts {
                packet: &NULL_PACKET,
                null: true,
            });
        }
        let mut at = 1 + ahf as usize;
        for k in 0..n {
            self.ts[0] = 0x47;
            if k == 0 || !hdm {
                let body = p.get(at..at + 187).ok_or(Error::Short)?;
                self.ts[1..].copy_from_slice(body);
                at += 187;
            } else {
                let body = p.get(at..at + 184).ok_or(Error::Short)?;
                // The header of the first packet with the continuity counter counted up.
                self.ts[3] = (self.ts[3] & 0xF0) | ((self.ts[3] + 1) & 0x0F);
                self.ts[4..].copy_from_slice(body);
                at += 184;
            }
            out(AlpEvent::Ts {
                packet: &self.ts,
                null: false,
            });
        }
        Ok(())
    }
}

/// NUMTS (0 means 16) and AHF of a TS packet header byte.
fn ts_count(b0: u8) -> (usize, bool) {
    let numts = ((b0 >> 1) & 0x0F) as usize;
    (if numts == 0 { 16 } else { numts }, b0 & 1 != 0)
}

#[derive(Clone, Copy, Debug)]
enum Form {
    Single,
    Segment { seq: u8, last: bool },
    Concat { lens: [u16; 8], count: usize },
}

/// A parsed ALP header (not TS).
#[derive(Clone, Copy, Debug)]
struct Header {
    form: Form,
    payload_len: usize,
    sid: Option<u8>,
    /// Header extension type and its byte range in the packet.
    ext: Option<(u8, usize, usize)>,
    /// Bytes of base, additional and optional headers.
    len: usize,
}

impl Header {
    fn parse(b: &[u8]) -> Result<Header> {
        if b.len() < 2 {
            return Err(Error::Short);
        }
        let pc = b[0] & 0x10 != 0;
        let flag = b[0] & 0x08 != 0;
        let len11 = ((b[0] & 7) as usize) << 8 | b[1] as usize;
        let byte = |i: usize| b.get(i).copied().ok_or(Error::Short);
        let mut at = 2;
        let (form, payload_len, sif, hef) = match (pc, flag) {
            (false, false) => (Form::Single, len11, false, false),
            (false, true) => {
                let a = byte(2)?;
                at = 3;
                (
                    Form::Single,
                    len11 | ((a >> 3) as usize) << 11,
                    a & 2 != 0,
                    a & 1 != 0,
                )
            }
            (true, false) => {
                let a = byte(2)?;
                at = 3;
                (
                    Form::Segment {
                        seq: a >> 3,
                        last: a & 4 != 0,
                    },
                    len11,
                    a & 2 != 0,
                    a & 1 != 0,
                )
            }
            (true, true) => {
                let a = byte(2)?;
                let len = len11 | ((a >> 4) as usize) << 11;
                let count = ((a >> 1) & 7) as usize + 2;
                let sif = a & 1 != 0;
                let nlen = count - 1;
                let bits = 12 * nlen + if nlen % 2 == 1 { 4 } else { 0 };
                let lb = b.get(3..3 + bits / 8).ok_or(Error::Short)?;
                let mut lens = [0u16; 8];
                for (k, l) in lens.iter_mut().take(nlen).enumerate() {
                    let bit = 12 * k;
                    let v = (lb[bit / 8] as u16) << 8 | *lb.get(bit / 8 + 1).unwrap_or(&0) as u16;
                    *l = if bit % 8 == 0 { v >> 4 } else { v & 0x0FFF };
                }
                at = 3 + bits / 8;
                (Form::Concat { lens, count }, len, sif, false)
            }
        };
        if payload_len == 0 {
            return Err(Error::Invalid(Malformed::AlpLength));
        }
        let sid = if sif {
            at += 1;
            Some(byte(at - 1)?)
        } else {
            None
        };
        let ext = if hef {
            let t = byte(at)?;
            let n = byte(at + 1)? as usize + 1;
            let r = (t, at + 2, at + 2 + n);
            at += 2 + n;
            Some(r)
        } else {
            None
        };
        Ok(Header {
            form,
            payload_len,
            sid,
            ext,
            len: at,
        })
    }
}

/// The length of the ALP packet at the start of `b`: `Ok(None)` when more bytes are needed.
fn packet_len(b: &[u8]) -> Result<Option<usize>> {
    if b.len() < 2 {
        return Ok(None);
    }
    if PacketType::from_code(b[0] >> 5) == PacketType::Mpeg2Ts {
        let (n, ahf) = ts_count(b[0]);
        let hdm = ahf && b[1] & 0x80 != 0;
        let body = if hdm { 187 + (n - 1) * 184 } else { n * 187 };
        let need = 1 + ahf as usize + body;
        return Ok((b.len() >= need).then_some(need));
    }
    let h = match Header::parse(b) {
        Ok(h) => h,
        Err(Error::Short) => return Ok(None),
        Err(e) => return Err(e),
    };
    let extra = match PacketType::from_code(b[0] >> 5) {
        PacketType::Signalling if matches!(h.form, Form::Single | Form::Concat { .. }) => 5,
        PacketType::Extension if matches!(h.form, Form::Single | Form::Concat { .. }) => 2,
        _ => 0,
    };
    let need = h.len + extra + h.payload_len;
    Ok((b.len() >= need).then_some(need))
}

/// Appends a single ALP packet (no segmentation) of `kind` with `payload` to `out`. A payload of 2048 bytes or more uses
/// HM = 1 and length_MSB. `sid` adds a sub-stream identifier. Signalling and extension packets take their extra header as the
/// first bytes of `payload` (5 and 2 bytes).
pub fn write_single(
    kind: PacketType,
    sid: Option<u8>,
    payload: &[u8],
    out: &mut Vec<u8>,
) -> Result<()> {
    if kind == PacketType::Mpeg2Ts {
        return Err(Error::Invalid(Malformed::PacketType));
    }
    let extra = match kind {
        PacketType::Signalling => 5,
        PacketType::Extension => 2,
        _ => 0,
    };
    if payload.len() < extra + 1 {
        return Err(Error::Short);
    }
    let len = payload.len() - extra;
    if len > MAX_PAYLOAD {
        return Err(Error::TooLarge(Limit::AlpPayload));
    }
    let hm = len >= 2048 || sid.is_some();
    out.push(kind.code() << 5 | (hm as u8) << 3 | ((len >> 8) & 7) as u8);
    out.push(len as u8);
    if hm {
        out.push(((len >> 11) as u8) << 3 | 0x04 | (sid.is_some() as u8) << 1);
    }
    if let Some(s) = sid {
        out.push(s);
    }
    out.extend_from_slice(payload);
    Ok(())
}

/// Appends an ALP packet of 1 to 16 transport packets to `out`, after `deleted_nulls` (0 to 128) deleted null packets. With
/// `header_deletion`, the packets after the first are sent without their header. The caller makes sure that their headers are
/// the first header with the continuity counter counted up (A/330 clause 5.4.4).
pub fn write_ts(
    packets: &[[u8; 188]],
    deleted_nulls: usize,
    header_deletion: bool,
    out: &mut Vec<u8>,
) -> Result<()> {
    let n = packets.len();
    if !(1..=16).contains(&n) {
        return Err(Error::Invalid(Malformed::Numts));
    }
    if deleted_nulls > 128 || header_deletion && deleted_nulls > 127 {
        return Err(Error::Invalid(Malformed::Dnp));
    }
    let ahf = deleted_nulls > 0 || header_deletion;
    out.push(7 << 5 | ((n & 0x0F) as u8) << 1 | ahf as u8);
    if ahf {
        out.push((header_deletion as u8) << 7 | (deleted_nulls & 0x7F) as u8);
    }
    for (k, p) in packets.iter().enumerate() {
        if header_deletion && k > 0 {
            out.extend_from_slice(&p[4..]);
        } else {
            out.extend_from_slice(&p[1..]);
        }
    }
    Ok(())
}
