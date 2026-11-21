//! Packet mode: DAB packets with the packet-mode FEC, and DRM packets. Both carry MSC data groups. The assemblers keep one
//! buffer for each address and give each complete data group to a callback.
//!
//! Standard: ETSI EN 300 401 clauses 5.3.2 and 5.3.5, ETSI ES 201 980 clause 6.6.1.

use crate::{Error, Result};
use crate::{Malformed, Parameter};
use sdrkit_algo_fec::rs::{codes, Rs};
use sdrkit_util_bits::crc::catalog::CRC16_GENIBUS;
use sdrkit_util_bits::crc::Trailer;

/// The DAB packet lengths in bytes, by the 2-bit packet length code.
pub const DAB_LENGTHS: [usize; 4] = [24, 48, 72, 96];
/// The DAB address of padding packets.
pub const PADDING_ADDRESS: u16 = 0;
/// The DAB address of FEC packets (EN 300 401 clause 5.3.5).
pub const FEC_ADDRESS: u16 = 1022;
/// The largest data group the assemblers keep, in bytes. A data group data field is at most 8191 bytes (13-bit segment
/// size), plus at most 25 bytes of headers and CRC.
pub const MAX_GROUP: usize = 8191 + 25;

/// The header of a DAB packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DabHeader {
    /// Packet length in bytes (24, 48, 72 or 96).
    pub len: usize,
    /// Continuity index (0 to 3).
    pub continuity: u8,
    /// First packet of a data group.
    pub first: bool,
    /// Last packet of a data group.
    pub last: bool,
    /// Address (0 to 1023).
    pub address: u16,
    /// Command flag.
    pub command: bool,
    /// Useful data length (0 to 91).
    pub useful: usize,
}

impl DabHeader {
    /// Reads the three header bytes of a DAB packet.
    pub fn parse(b: &[u8]) -> Result<DabHeader> {
        if b.len() < 3 {
            return Err(Error::Truncated);
        }
        let len = DAB_LENGTHS[(b[0] >> 6) as usize];
        let useful = (b[2] & 0x7F) as usize;
        if useful > len - 5 {
            return Err(Error::Invalid(Malformed::UsefulDataLength));
        }
        Ok(DabHeader {
            len,
            continuity: (b[0] >> 4) & 3,
            first: b[0] & 0x08 != 0,
            last: b[0] & 0x04 != 0,
            address: (((b[0] & 3) as u16) << 8) | b[1] as u16,
            command: b[2] & 0x80 != 0,
            useful,
        })
    }
}

/// Writes DAB packets for one address.
#[derive(Clone, Debug)]
pub struct DabPacketWriter {
    address: u16,
    continuity: u8,
}

impl DabPacketWriter {
    /// A writer for `address` (1 to 1021: 0 is padding and 1022 is FEC).
    pub fn new(address: u16) -> Result<Self> {
        if address == 0 || address >= FEC_ADDRESS {
            return Err(Error::BadParameter(Parameter::PacketAddress));
        }
        Ok(DabPacketWriter {
            address,
            continuity: 0,
        })
    }

    /// Appends the packets of one data group. Each packet is the shortest length that holds its part (at most 91 bytes).
    pub fn write_group(&mut self, group: &[u8], out: &mut Vec<u8>) -> Result<()> {
        if group.is_empty() || group.len() > MAX_GROUP {
            return Err(Error::BadParameter(Parameter::DataGroupLength));
        }
        let n = group.len().div_ceil(91);
        for (i, part) in group.chunks(91).enumerate() {
            let code = DAB_LENGTHS
                .iter()
                .position(|&l| l - 5 >= part.len())
                .unwrap_or(3);
            let len = DAB_LENGTHS[code];
            let start = out.len();
            out.push(
                ((code as u8) << 6)
                    | (self.continuity << 4)
                    | (((i == 0) as u8) << 3)
                    | (((i + 1 == n) as u8) << 2)
                    | (self.address >> 8) as u8,
            );
            out.push(self.address as u8);
            out.push(part.len() as u8);
            out.extend_from_slice(part);
            out.resize(start + len - 2, 0);
            let c = CRC16_GENIBUS.checksum(&out[start..]);
            out.extend_from_slice(&c.to_be_bytes());
            self.continuity = (self.continuity + 1) & 3;
        }
        Ok(())
    }
}

/// Appends a 24-byte padding packet (address 0, no useful data).
pub fn dab_padding(out: &mut Vec<u8>) {
    let start = out.len();
    out.resize(start + 22, 0);
    let c = CRC16_GENIBUS.checksum(&out[start..]);
    out.extend_from_slice(&c.to_be_bytes());
}

/// The counters of an assembler.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Packets with a correct CRC.
    pub packets_ok: u64,
    /// Packets with a bad CRC or header.
    pub packets_bad: u64,
    /// Padding packets.
    pub padding: u64,
    /// Data groups given to the callback.
    pub groups: u64,
    /// Data groups dropped (a missing packet, a new first packet before the last one, too large).
    pub groups_dropped: u64,
    /// Continuation packets without a data group in progress (after a drop, or when reception starts).
    pub orphans: u64,
    /// Bytes corrected by the packet-mode FEC.
    pub fec_corrected: u64,
    /// FEC frames decoded.
    pub fec_frames: u64,
}

/// One data group being collected.
#[derive(Clone, Debug, Default)]
struct Slot {
    address: u16,
    continuity: u8,
    active: bool,
    used: u64,
    buf: Vec<u8>,
}

/// The slots of an assembler: a few addresses, the least recently used one is reused.
#[derive(Clone, Debug)]
struct Slots {
    slots: Vec<Slot>,
    clock: u64,
}

impl Slots {
    fn new(n: usize) -> Self {
        Slots {
            slots: vec![Slot::default(); n.max(1)],
            clock: 0,
        }
    }

    fn find(&mut self, address: u16) -> Option<&mut Slot> {
        self.slots
            .iter_mut()
            .find(|s| s.active && s.address == address)
    }

    /// The slot for a new data group at `address`.
    fn start(&mut self, address: u16, continuity: u8, stats: &mut Stats) -> &mut Slot {
        self.clock += 1;
        let i = match self
            .slots
            .iter()
            .position(|s| s.active && s.address == address)
        {
            Some(i) => {
                stats.groups_dropped += 1;
                i
            }
            None => match self.slots.iter().position(|s| !s.active) {
                Some(i) => i,
                None => {
                    stats.groups_dropped += 1;
                    let mut best = 0;
                    for (i, s) in self.slots.iter().enumerate() {
                        if s.used < self.slots[best].used {
                            best = i;
                        }
                    }
                    best
                }
            },
        };
        let s = &mut self.slots[i];
        s.address = address;
        s.continuity = continuity;
        s.active = true;
        s.used = self.clock;
        s.buf.clear();
        s
    }

    fn clear(&mut self) {
        for s in &mut self.slots {
            s.active = false;
        }
    }
}

/// Collects one part of a data group into its slot. If the group is complete, it goes to `out`.
#[allow(clippy::too_many_arguments)]
fn collect(
    slots: &mut Slots,
    stats: &mut Stats,
    address: u16,
    continuity: u8,
    modulo: u8,
    first: bool,
    last: bool,
    data: &[u8],
    out: &mut dyn FnMut(u16, &[u8]),
) {
    if first && last {
        stats.groups += 1;
        out(address, data);
        if let Some(s) = slots.find(address) {
            s.active = false;
            stats.groups_dropped += 1;
        }
        return;
    }
    let s = if first {
        slots.start(address, continuity, stats)
    } else {
        match slots.find(address) {
            Some(s) if (s.continuity + 1) % modulo == continuity => s,
            Some(s) => {
                s.active = false;
                stats.groups_dropped += 1;
                return;
            }
            None => {
                stats.orphans += 1;
                return;
            }
        }
    };
    if s.buf.len() + data.len() > MAX_GROUP {
        s.active = false;
        stats.groups_dropped += 1;
        return;
    }
    s.continuity = continuity;
    s.buf.extend_from_slice(data);
    if last {
        s.active = false;
        stats.groups += 1;
        out(address, &s.buf);
    }
}

/// Reassembles data groups from DAB packets without FEC (EN 300 401 clause 5.3.2).
///
/// ```
/// use sdrkit_proto_bcdata::packet::{DabAssembler, DabPacketWriter};
///
/// let mut w = DabPacketWriter::new(17).unwrap();
/// let mut stream = Vec::new();
/// w.write_group(&[7u8; 100], &mut stream).unwrap();
/// let mut a = DabAssembler::new();
/// let mut got = Vec::new();
/// a.push(&stream, &mut |addr, g| got.push((addr, g.to_vec())));
/// assert_eq!(got, vec![(17, vec![7u8; 100])]);
/// ```
#[derive(Clone, Debug)]
pub struct DabAssembler {
    slots: Slots,
    rest: Vec<u8>,
    /// The counters.
    pub stats: Stats,
}

impl Default for DabAssembler {
    fn default() -> Self {
        Self::new()
    }
}

impl DabAssembler {
    /// An assembler that collects up to 8 addresses at the same time.
    pub fn new() -> Self {
        Self::with_addresses(8)
    }

    /// An assembler that collects up to `n` addresses at the same time.
    pub fn with_addresses(n: usize) -> Self {
        DabAssembler {
            slots: Slots::new(n),
            rest: Vec::with_capacity(96),
            stats: Stats::default(),
        }
    }

    /// Forgets the data groups in progress (for example after a retune). The counters stay.
    pub fn reset(&mut self) {
        self.slots.clear();
        self.rest.clear();
    }

    /// Takes the bytes of the sub-channel (any cut). It passes each complete data group, with its address, to `out`.
    pub fn push(&mut self, bytes: &[u8], out: &mut dyn FnMut(u16, &[u8])) {
        let mut b = bytes;
        if !self.rest.is_empty() {
            let len = DAB_LENGTHS[(self.rest[0] >> 6) as usize];
            let take = (len - self.rest.len()).min(b.len());
            self.rest.extend_from_slice(&b[..take]);
            b = &b[take..];
            if self.rest.len() < len {
                return;
            }
            let p = std::mem::take(&mut self.rest);
            self.packet(&p, out);
            self.rest = p;
            self.rest.clear();
        }
        while !b.is_empty() {
            let len = DAB_LENGTHS[(b[0] >> 6) as usize];
            if b.len() < len {
                self.rest.extend_from_slice(b);
                return;
            }
            self.packet(&b[..len], out);
            b = &b[len..];
        }
    }

    /// Takes one whole packet.
    pub fn packet(&mut self, p: &[u8], out: &mut dyn FnMut(u16, &[u8])) {
        let Ok(h) = DabHeader::parse(p) else {
            self.stats.packets_bad += 1;
            return;
        };
        if p.len() != h.len || !CRC16_GENIBUS.verify(p, Trailer::Big) {
            self.stats.packets_bad += 1;
            return;
        }
        self.stats.packets_ok += 1;
        if h.address == PADDING_ADDRESS {
            self.stats.padding += 1;
            return;
        }
        if h.address == FEC_ADDRESS || h.command {
            return;
        }
        let data = &p[3..3 + h.useful];
        collect(
            &mut self.slots,
            &mut self.stats,
            h.address,
            h.continuity,
            4,
            h.first,
            h.last,
            data,
            out,
        );
    }
}

/// Bytes in the application data table of one FEC frame: 12 rows of 188 bytes (EN 300 401 clause 5.3.5).
pub const FEC_ADT: usize = 2256;
/// Bytes of the nine FEC packets of one FEC frame.
pub const FEC_PACKETS: usize = 9 * 24;
/// Bytes of one FEC frame.
pub const FEC_FRAME: usize = FEC_ADT + FEC_PACKETS;

/// The packet-mode FEC of DAB (EN 300 401 clause 5.3.5). The 2256 bytes of packets before nine FEC packets are the application
/// data table of 12 rows; each row is the data of one RS(204,188) code word ([`sdrkit_algo_fec::rs::codes::DVB_204_188`]). The
/// nine FEC packets (address 1022, 2-byte header, no CRC) carry the 192 parity bytes and 6 zero bytes.
///
/// The encoder numbers the FEC packets 0 to 8 in the continuity index and the first and last flags. The decoder finds the FEC
/// packets by their length and address only.
pub struct DabFec {
    rs: Rs,
    word: [u8; 204],
    synced: bool,
    buf: Vec<u8>,
    assembler: DabAssembler,
}

impl Default for DabFec {
    fn default() -> Self {
        Self::new()
    }
}

impl DabFec {
    /// A decoder for a sub-channel with FEC scheme 1 (FIG 0/14).
    pub fn new() -> Self {
        DabFec {
            // DVB_204_188 is a valid code, so `new` cannot fail.
            rs: Rs::new(codes::DVB_204_188).unwrap_or_else(|_| unreachable!("valid RS code")),
            word: [0; 204],
            synced: false,
            buf: Vec::with_capacity(3 * FEC_FRAME),
            assembler: DabAssembler::new(),
        }
    }

    /// The counters (FEC and packets).
    pub fn stats(&self) -> Stats {
        self.assembler.stats
    }

    /// Forgets everything in progress.
    pub fn reset(&mut self) {
        self.synced = false;
        self.buf.clear();
        self.assembler.reset();
    }

    /// Appends the nine FEC packets for an application data table of exactly 2256 bytes.
    pub fn encode(&self, adt: &[u8], out: &mut Vec<u8>) -> Result<()> {
        if adt.len() != FEC_ADT {
            return Err(Error::BadParameter(Parameter::ApplicationDataTable));
        }
        let mut table = [0u8; 198];
        let mut word = [0u8; 204];
        for m in 0..12 {
            for c in 0..188 {
                word[c] = adt[c * 12 + m];
            }
            self.rs.encode_word(&mut word)?;
            for c in 0..16 {
                table[c * 12 + m] = word[188 + c];
            }
        }
        for k in 0..9u8 {
            out.push((k << 2) | 0x03);
            out.push(0xFE);
            out.extend_from_slice(&table[k as usize * 22..k as usize * 22 + 22]);
        }
        Ok(())
    }

    fn is_fec_packet(p: &[u8]) -> bool {
        p[0] >> 6 == 0 && p[0] & 3 == 3 && p[1] == 0xFE
    }

    /// Takes the bytes of the sub-channel (any cut). It passes each complete data group to `out`.
    pub fn push(&mut self, bytes: &[u8], out: &mut dyn FnMut(u16, &[u8])) {
        self.buf.extend_from_slice(bytes);
        loop {
            if self.buf.len() < FEC_FRAME {
                return;
            }
            let at = if self.synced
                && (0..9).all(|k| Self::is_fec_packet(&self.buf[FEC_ADT + 24 * k..]))
            {
                Some(FEC_ADT)
            } else {
                self.synced = false;
                (FEC_ADT..=self.buf.len() - FEC_PACKETS)
                    .step_by(24)
                    .find(|&i| (0..9).all(|k| Self::is_fec_packet(&self.buf[i + 24 * k..])))
            };
            let Some(i) = at else {
                // No FEC frame: pass the oldest bytes on without correction.
                if self.buf.len() >= 2 * FEC_FRAME {
                    let n = self.buf.len() - FEC_FRAME;
                    let n = n - n % 24;
                    let mut b = std::mem::take(&mut self.buf);
                    self.assembler.push(&b[..n], out);
                    b.drain(..n);
                    self.buf = b;
                }
                return;
            };
            let start = i - FEC_ADT;
            let mut b = std::mem::take(&mut self.buf);
            if start > 0 {
                self.assembler.push(&b[..start], out);
            }
            self.correct(&mut b[start..start + FEC_FRAME]);
            self.assembler.push(&b[start..start + FEC_ADT], out);
            b.drain(..start + FEC_FRAME);
            self.buf = b;
            self.synced = true;
        }
    }

    /// Corrects one FEC frame in place.
    fn correct(&mut self, frame: &mut [u8]) {
        let (adt, fec) = frame.split_at_mut(FEC_ADT);
        let rsd = |j: usize| fec[(j / 22) * 24 + 2 + j % 22];
        for m in 0..12 {
            for c in 0..188 {
                self.word[c] = adt[c * 12 + m];
            }
            for c in 0..16 {
                self.word[188 + c] = rsd(c * 12 + m);
            }
            if let Ok(d) = self.rs.decode(&mut self.word, &[]) {
                if d.corrected > 0 {
                    self.assembler.stats.fec_corrected += d.corrected as u64;
                    for c in 0..188 {
                        adt[c * 12 + m] = self.word[c];
                    }
                }
            }
        }
        self.assembler.stats.fec_frames += 1;
    }
}

/// The header byte of a DRM packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrmHeader {
    /// First packet of a data unit.
    pub first: bool,
    /// Last packet of a data unit.
    pub last: bool,
    /// Packet id (0 to 3).
    pub id: u8,
    /// Padded packet: the first data byte is the number of useful bytes.
    pub padded: bool,
    /// Continuity index (0 to 7).
    pub continuity: u8,
}

impl DrmHeader {
    /// Reads the header byte.
    pub const fn parse(b: u8) -> DrmHeader {
        DrmHeader {
            first: b & 0x80 != 0,
            last: b & 0x40 != 0,
            id: (b >> 4) & 3,
            padded: b & 0x08 != 0,
            continuity: b & 7,
        }
    }

    /// The header byte.
    pub const fn byte(&self) -> u8 {
        ((self.first as u8) << 7)
            | ((self.last as u8) << 6)
            | ((self.id & 3) << 4)
            | ((self.padded as u8) << 3)
            | (self.continuity & 7)
    }
}

/// Writes DRM packets of one packet id and length (ES 201 980 clause 6.6.1).
#[derive(Clone, Debug)]
pub struct DrmPacketWriter {
    len: usize,
    id: u8,
    continuity: u8,
}

impl DrmPacketWriter {
    /// A writer of packets of `len` bytes (4 to 258) with packet id `id` (0 to 3).
    pub fn new(id: u8, len: usize) -> Result<Self> {
        if id > 3 || !(4..=258).contains(&len) {
            return Err(Error::BadParameter(Parameter::DrmPacket));
        }
        Ok(DrmPacketWriter {
            len,
            id,
            continuity: 0,
        })
    }

    /// The packet length.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Never true: a packet has at least 4 bytes.
    pub fn is_empty(&self) -> bool {
        false
    }

    fn put(&mut self, first: bool, last: bool, part: &[u8], out: &mut Vec<u8>) {
        let dl = self.len - 3;
        let padded = part.len() < dl;
        let start = out.len();
        out.push(
            DrmHeader {
                first,
                last,
                id: self.id,
                padded,
                continuity: self.continuity,
            }
            .byte(),
        );
        if padded {
            out.push(part.len() as u8);
        }
        out.extend_from_slice(part);
        out.resize(start + 1 + dl, 0);
        let c = CRC16_GENIBUS.checksum(&out[start..]);
        out.extend_from_slice(&c.to_be_bytes());
        self.continuity = (self.continuity + 1) & 7;
    }

    /// Appends the packets that carry one data unit (a data group). A padded packet holds one byte less.
    pub fn write_unit(&mut self, unit: &[u8], out: &mut Vec<u8>) -> Result<()> {
        if unit.is_empty() {
            return Err(Error::BadParameter(Parameter::EmptyDataUnit));
        }
        let dl = self.len - 3;
        let mut rest = unit;
        let mut first = true;
        while !rest.is_empty() {
            // A part that does not fill the data field must leave room for the length byte.
            let take = if rest.len() >= dl {
                dl
            } else {
                rest.len().min(dl - 1)
            };
            let (part, r) = rest.split_at(take);
            rest = r;
            self.put(first, rest.is_empty(), part, out);
            first = false;
        }
        Ok(())
    }

    /// Appends one padding packet (padded, no useful bytes).
    pub fn write_padding(&mut self, out: &mut Vec<u8>) {
        self.put(false, false, &[], out);
    }
}

/// Reassembles data units from DRM packets of one length (ES 201 980 clause 6.6.1). It collects every packet id, or one only.
#[derive(Clone, Debug)]
pub struct DrmAssembler {
    len: usize,
    only: Option<u8>,
    slots: Slots,
    last_cont: [Option<u8>; 4],
    rest: Vec<u8>,
    /// The counters.
    pub stats: Stats,
}

impl DrmAssembler {
    /// An assembler of packets of `len` bytes (4 to 258). With `only`, it keeps one packet id and ignores the others.
    pub fn new(len: usize, only: Option<u8>) -> Result<Self> {
        if !(4..=258).contains(&len) || only.is_some_and(|i| i > 3) {
            return Err(Error::BadParameter(Parameter::DrmPacket));
        }
        Ok(DrmAssembler {
            len,
            only,
            slots: Slots::new(4),
            last_cont: [None; 4],
            rest: Vec::with_capacity(len),
            stats: Stats::default(),
        })
    }

    /// Forgets the data units in progress. The counters stay.
    pub fn reset(&mut self) {
        self.slots.clear();
        self.last_cont = [None; 4];
        self.rest.clear();
    }

    /// Takes the packet bytes of the data stream (any cut). It passes each complete data unit, with its packet id, to `out`.
    pub fn push(&mut self, bytes: &[u8], out: &mut dyn FnMut(u16, &[u8])) {
        let mut b = bytes;
        if !self.rest.is_empty() {
            let take = (self.len - self.rest.len()).min(b.len());
            self.rest.extend_from_slice(&b[..take]);
            b = &b[take..];
            if self.rest.len() < self.len {
                return;
            }
            let p = std::mem::take(&mut self.rest);
            self.packet(&p, out);
            self.rest = p;
            self.rest.clear();
        }
        let mut chunks = b.chunks_exact(self.len);
        for p in &mut chunks {
            self.packet(p, out);
        }
        self.rest.extend_from_slice(chunks.remainder());
    }

    /// Takes one whole packet.
    pub fn packet(&mut self, p: &[u8], out: &mut dyn FnMut(u16, &[u8])) {
        if p.len() != self.len || !CRC16_GENIBUS.verify(p, Trailer::Big) {
            self.stats.packets_bad += 1;
            return;
        }
        let h = DrmHeader::parse(p[0]);
        if self.only.is_some_and(|i| i != h.id) {
            return;
        }
        self.stats.packets_ok += 1;
        // A gap in the continuity of this id loses the unit in progress.
        if let Some(c) = self.last_cont[h.id as usize] {
            if (c + 1) & 7 != h.continuity {
                if let Some(s) = self.slots.find(h.id as u16) {
                    s.active = false;
                    self.stats.groups_dropped += 1;
                }
            }
        }
        self.last_cont[h.id as usize] = Some(h.continuity);
        let field = &p[1..p.len() - 2];
        let data = if h.padded {
            let n = field[0] as usize;
            if n == 0 {
                self.stats.padding += 1;
                return;
            }
            if n > field.len() - 1 {
                self.stats.packets_bad += 1;
                return;
            }
            &field[1..1 + n]
        } else {
            field
        };
        collect(
            &mut self.slots,
            &mut self.stats,
            h.id as u16,
            h.continuity,
            8,
            h.first,
            h.last,
            data,
            out,
        );
    }
}
