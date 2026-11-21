//! The xRIT transport layer: files from space packets. The user data of the packets of one virtual channel and APID, from a
//! first (or unsegmented) packet to a last one, is a transport file. [`FileAssembler`] removes the 10-byte transport header (file
//! counter, 2 bytes; file length in bits, 8 bytes) and emits the file. A file with a missing packet is marked with errors and
//! keeps the place of each packet, so an image decoder can leave a lost line empty.
//!
//! Standard: CGMS LRIT/HRIT Global Specification section 5.

use crate::packet::{Packet, SeqFlags};
use crate::{be16, Error, Result};

/// Bytes of the transport header.
pub const TRANSPORT_HEADER_LEN: usize = 10;

/// A file given out by [`FileAssembler`]. It borrows the assembler's buffers until the callback returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct File<'a> {
    /// The virtual channel.
    pub vcid: u8,
    /// The APID.
    pub apid: u16,
    /// The file counter of the transport header.
    pub counter: u16,
    /// The file length in bits that the transport header provides.
    pub declared_bits: u64,
    /// The file bytes (after the transport header).
    pub bytes: &'a [u8],
    /// Where each packet's data starts in `bytes`.
    pub packet_starts: &'a [u32],
    /// The place of each packet in the file (0 for the first), from the packet sequence counts: a lost packet leaves a step.
    pub packet_index: &'a [u32],
    /// A packet was lost or had a wrong CRC.
    pub errors: bool,
    /// The file length matches the length the transport header declares.
    pub length_ok: bool,
    /// The file was cut short: a new first packet came before the last one.
    pub truncated: bool,
}

/// Counters of [`FileAssembler`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileStats {
    /// Files given out.
    pub files: u64,
    /// Files given out with errors.
    pub with_errors: u64,
    /// Files cut short.
    pub truncated: u64,
    /// Files dropped: too large, or a first packet without a transport header.
    pub dropped: u64,
    /// Packets that belong to no file in progress.
    pub orphans: u64,
}

/// The rules of [`FileAssembler`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssemblerConfig {
    /// Files in progress at the same time (one per virtual channel and APID).
    pub open_files: usize,
    /// The largest file accepted, bytes.
    pub max_bytes: usize,
}

impl Default for AssemblerConfig {
    fn default() -> Self {
        AssemblerConfig {
            open_files: 16,
            max_bytes: 64 << 20,
        }
    }
}

#[derive(Debug, Default)]
struct Slot {
    used: bool,
    vcid: u8,
    apid: u16,
    counter: u16,
    declared_bits: u64,
    bytes: Vec<u8>,
    starts: Vec<u32>,
    index: Vec<u32>,
    errors: bool,
    last_seq: u16,
    age: u64,
}

/// Puts files together from packets. See the [module documentation](self).
///
/// ```
/// use sdrkit_proto_ccsds::packet::{Packet, PrimaryHeader, SeqFlags};
/// use sdrkit_proto_ccsds::xrit::transport::{split_file, AssemblerConfig, FileAssembler};
/// let file: Vec<u8> = (0..3000u32).map(|i| i as u8).collect();
/// let pieces = split_file(&file, 7, 1000, &[]);
/// let mut asm = FileAssembler::new(AssemblerConfig::default());
/// let mut got = Vec::new();
/// for (i, data) in pieces.iter().enumerate() {
///     let flags = match (i, i + 1 == pieces.len()) { (0, _) => SeqFlags::First, (_, true) => SeqFlags::Last, _ => SeqFlags::Continuation };
///     let p = Packet { vcid: 0, header: PrimaryHeader::telemetry(300, flags, i as u16), data, crc_ok: None, gap_before: false };
///     asm.push(&p, |f| { assert!(!f.errors); got = f.bytes.to_vec(); });
/// }
/// assert_eq!(got, file);
/// ```
#[derive(Debug)]
pub struct FileAssembler {
    cfg: AssemblerConfig,
    slots: Vec<Slot>,
    clock: u64,
    stats: FileStats,
}

impl FileAssembler {
    /// An assembler with these rules (at least one file in progress).
    pub fn new(cfg: AssemblerConfig) -> Self {
        let mut slots = Vec::with_capacity(cfg.open_files.max(1));
        slots.resize_with(cfg.open_files.max(1), Slot::default);
        FileAssembler {
            cfg,
            slots,
            clock: 0,
            stats: FileStats::default(),
        }
    }

    /// The counters.
    pub fn stats(&self) -> &FileStats {
        &self.stats
    }

    /// Drops the files in progress (a retune).
    pub fn reset(&mut self) {
        for s in &mut self.slots {
            s.used = false;
        }
    }

    /// Takes a packet (with its CRC already removed). Calls `emit` for the file it completes, and for a file it cuts short.
    pub fn push(&mut self, p: &Packet<'_>, mut emit: impl FnMut(File<'_>)) {
        self.clock += 1;
        let (vcid, apid) = (p.vcid, p.header.apid);
        let found = self
            .slots
            .iter()
            .position(|s| s.used && s.vcid == vcid && s.apid == apid);
        let bad = p.crc_ok == Some(false);
        if p.header.flags.starts() {
            if let Some(i) = found {
                self.stats.truncated += 1;
                self.finish(i, true, &mut emit);
            }
            if p.data.len() < TRANSPORT_HEADER_LEN {
                self.stats.dropped += 1;
                return;
            }
            // A free slot, or the oldest file in progress (cut short).
            let i = match self.slots.iter().position(|s| !s.used) {
                Some(i) => i,
                None => {
                    let i = (0..self.slots.len())
                        .min_by_key(|&i| self.slots[i].age)
                        .unwrap_or(0);
                    self.stats.truncated += 1;
                    self.finish(i, true, &mut emit);
                    i
                }
            };
            let s = &mut self.slots[i];
            s.used = true;
            s.vcid = vcid;
            s.apid = apid;
            s.counter = be16(p.data);
            let mut bits = [0u8; 8];
            bits.copy_from_slice(&p.data[2..10]);
            s.declared_bits = u64::from_be_bytes(bits);
            s.bytes.clear();
            s.starts.clear();
            s.index.clear();
            s.errors = bad;
            s.last_seq = p.header.count;
            s.age = self.clock;
            s.starts.push(0);
            s.index.push(0);
            s.bytes.extend_from_slice(&p.data[TRANSPORT_HEADER_LEN..]);
            if p.header.flags == SeqFlags::Unsegmented {
                self.finish(i, false, &mut emit);
            }
            return;
        }
        let Some(i) = found else {
            self.stats.orphans += 1;
            return;
        };
        let max = self.cfg.max_bytes;
        let s = &mut self.slots[i];
        let step = (p.header.count.wrapping_sub(s.last_seq) & 0x3FFF) as u32;
        s.errors |= bad || p.gap_before || step != 1;
        let prev = s.index.last().copied().unwrap_or(0);
        s.index.push(prev + step.clamp(1, 0x3FFF));
        s.last_seq = p.header.count;
        s.age = self.clock;
        if s.bytes.len() + p.data.len() > max {
            s.used = false;
            self.stats.dropped += 1;
            return;
        }
        s.starts.push(s.bytes.len() as u32);
        s.bytes.extend_from_slice(p.data);
        if p.header.flags.ends() {
            self.finish(i, false, &mut emit);
        }
    }

    fn finish(&mut self, i: usize, truncated: bool, emit: &mut impl FnMut(File<'_>)) {
        let s = &mut self.slots[i];
        s.used = false;
        let errors = s.errors || truncated;
        let length_ok = s.declared_bits == s.bytes.len() as u64 * 8;
        self.stats.files += 1;
        if errors {
            self.stats.with_errors += 1;
        }
        emit(File {
            vcid: s.vcid,
            apid: s.apid,
            counter: s.counter,
            declared_bits: s.declared_bits,
            bytes: &s.bytes,
            packet_starts: &s.starts,
            packet_index: &s.index,
            errors,
            length_ok,
            truncated,
        });
    }
}

/// Splits a file into the user data of its packets, for generators: the first piece carries the transport header with
/// `counter`, then pieces of at most `max_user` bytes. A cut is forced at each offset of `line_starts` (Rice lines start a new
/// packet). `max_user` must be more than 10.
pub fn split_file(
    file: &[u8],
    counter: u16,
    max_user: usize,
    line_starts: &[usize],
) -> Vec<Vec<u8>> {
    let max_user = max_user.max(TRANSPORT_HEADER_LEN + 1);
    let mut cuts: Vec<usize> = line_starts
        .iter()
        .copied()
        .filter(|&c| c > 0 && c < file.len())
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut pieces = Vec::new();
    let mut pos = 0;
    let mut first = true;
    while pos < file.len() || first {
        let room = if first {
            max_user - TRANSPORT_HEADER_LEN
        } else {
            max_user
        };
        let mut end = (pos + room).min(file.len());
        if let Some(&c) = cuts.iter().find(|&&c| c > pos && c < end) {
            end = c;
        }
        let mut user = Vec::with_capacity(end - pos + TRANSPORT_HEADER_LEN);
        if first {
            user.extend_from_slice(&counter.to_be_bytes());
            user.extend_from_slice(&((file.len() as u64) * 8).to_be_bytes());
            first = false;
        }
        user.extend_from_slice(&file[pos..end]);
        pieces.push(user);
        pos = end;
    }
    pieces
}

/// The transport header of the first piece: (file counter, file length in bits).
pub fn transport_header(first: &[u8]) -> Result<(u16, u64)> {
    if first.len() < TRANSPORT_HEADER_LEN {
        return Err(Error::InputTooShort {
            need: TRANSPORT_HEADER_LEN,
        });
    }
    let mut bits = [0u8; 8];
    bits.copy_from_slice(&first[2..10]);
    Ok((be16(first), u64::from_be_bytes(bits)))
}
