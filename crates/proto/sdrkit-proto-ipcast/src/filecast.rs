//! Files of a carousel file broadcast, each cut into numbered parts of a fixed size and sent as UDP datagrams: the
//! reassembly of the files, the cleaning of their names, and the writers of the datagrams for test signals.
//!
//! Every datagram is one packet. Common header: byte 1 the type (03 head, 01 part, FF tail), bytes 2-3 the packet length
//! (LE16, not beyond the bytes received), bytes 4-7 the file id (LE32). A head announces a file: the number of parts N at
//! bytes 72-73 (LE16), the file name from byte 84 to a zero byte (longer than 4 bytes), the file size (LE32) 56 bytes after
//! that zero byte. A part carries its number k (LE16 at bytes 8-9) and its data from byte 16, at most P bytes (P is the part
//! size), placed at k x P of the file. A tail ends the file of its id. Other types are counted and ignored. No public
//! document describes this format: the layout and P are not confirmed by a public document.
//!
//! Reassembly: a head opens a file unless that id finished within the repeat window or is open already (a repeat). Memory
//! of open files is held to a limit, the file heard least recently being finished as incomplete to make room. A file
//! finishes when all its parts are in (complete), at its tail, or after a silence (incomplete). A finished file is cut to its
//! size; parts that never came are zeros.

use std::collections::BTreeMap;

/// Packet type of a head.
pub const HEAD: u8 = 0x03;
/// Packet type of a part.
pub const PART: u8 = 0x01;
/// Packet type of a tail.
pub const TAIL: u8 = 0xFF;
/// Where a head's file name starts.
pub const NAME_AT: usize = 84;
/// Bytes from the zero byte after the name to the file size.
pub const SIZE_AFTER_NAME: usize = 56;
/// Where a part's data starts.
pub const PART_DATA_AT: usize = 16;
/// The longest name kept after cleaning, bytes.
pub const MAX_NAME: usize = 200;

/// The limits and timers of a [`FileAssembler`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilecastConfig {
    /// The part size P, bytes.
    pub part_bytes: usize,
    /// Seconds of signal without a packet of a file after which it finishes as incomplete.
    pub max_silence_s: f64,
    /// Seconds after a file finished during which its head and parts are repeats.
    pub repeat_s: f64,
    /// The most bytes (N x P each) of all open files.
    pub max_open_bytes: usize,
    /// The most bytes (N x P) of one file.
    pub max_file_bytes: usize,
}

impl Default for FilecastConfig {
    /// P = 1408, 120 s of silence, 900 s of repeats, 768 MiB open, 256 MiB a file.
    fn default() -> Self {
        FilecastConfig {
            part_bytes: 1408,
            max_silence_s: 120.0,
            repeat_s: 900.0,
            max_open_bytes: 768 << 20,
            max_file_bytes: 256 << 20,
        }
    }
}

/// Counters of a [`FileAssembler`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FilecastStats {
    /// Packets taken.
    pub packets: u64,
    /// Heads.
    pub heads: u64,
    /// Parts.
    pub parts: u64,
    /// Tails.
    pub tails: u64,
    /// Packets refused: too short, a length beyond the bytes, a bad head, a part number out of range.
    pub bad: u64,
    /// Parts of a file that is not open.
    pub orphan: u64,
    /// Heads and parts of a file that is open already or finished within the repeat window.
    pub repeats: u64,
    /// Files finished early to make room.
    pub evicted: u64,
    /// Packets of other types.
    pub other: u64,
}

/// A finished file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinishedFile {
    /// The file id.
    pub id: u32,
    /// The name as the head gives it (not cleaned; see [`clean_name`]).
    pub name: String,
    /// The bytes, cut to the file size.
    pub data: Vec<u8>,
    /// Parts N.
    pub parts: u32,
    /// Parts received.
    pub received: u32,
    /// All parts came.
    pub complete: bool,
}

#[derive(Clone, Debug)]
struct OpenFile {
    name: String,
    size: usize,
    parts: u32,
    have: Vec<bool>,
    received: u32,
    data: Vec<u8>,
    heard: f64,
    reserved: usize,
}

/// The reassembly of the files. See the [module documentation](self).
#[derive(Clone, Debug, Default)]
pub struct FileAssembler {
    cfg: FilecastConfig,
    open: BTreeMap<u32, OpenFile>,
    finished: BTreeMap<u32, f64>,
    open_bytes: usize,
    since_expiry: u32,
    /// The counters.
    pub stats: FilecastStats,
}

fn le16(b: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes([b[at], b[at + 1]]))
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

impl FileAssembler {
    /// An assembler with `cfg`.
    pub fn new(cfg: FilecastConfig) -> Self {
        FileAssembler {
            cfg,
            ..Default::default()
        }
    }

    /// The limits and timers.
    pub fn config(&self) -> &FilecastConfig {
        &self.cfg
    }

    /// Files open now.
    pub fn open_files(&self) -> usize {
        self.open.len()
    }

    /// Forgets the open files, the finished ids and the counters.
    pub fn clear(&mut self) {
        *self = FileAssembler::new(self.cfg);
    }

    /// Takes one packet (one UDP payload) at signal time `now` and calls `out` with each file it finishes.
    pub fn push(&mut self, pkt: &[u8], now: f64, out: &mut impl FnMut(FinishedFile)) {
        self.stats.packets += 1;
        self.since_expiry += 1;
        if self.since_expiry >= 1024 {
            self.expire(now, out);
        }
        if pkt.len() < 8 {
            self.stats.bad += 1;
            return;
        }
        let len = le16(pkt, 2);
        if len > pkt.len() || len < 8 {
            self.stats.bad += 1;
            return;
        }
        let pkt = &pkt[..len];
        let id = le32(pkt, 4);
        match pkt[1] {
            HEAD => self.head(id, pkt, now, out),
            PART => self.part(id, pkt, now, out),
            TAIL => {
                self.stats.tails += 1;
                if let Some(f) = self.open.remove(&id) {
                    self.finish(id, f, now, out);
                }
            }
            _ => self.stats.other += 1,
        }
    }

    fn recently_finished(&self, id: u32, now: f64) -> bool {
        self.finished
            .get(&id)
            .is_some_and(|&t| now - t <= self.cfg.repeat_s)
    }

    fn head(&mut self, id: u32, pkt: &[u8], now: f64, out: &mut impl FnMut(FinishedFile)) {
        self.stats.heads += 1;
        if pkt.len() < NAME_AT + 1 {
            self.stats.bad += 1;
            return;
        }
        let Some(l) = pkt[NAME_AT..].iter().position(|&b| b == 0) else {
            self.stats.bad += 1;
            return;
        };
        if l <= 4 || pkt.len() < NAME_AT + l + SIZE_AFTER_NAME + 4 {
            self.stats.bad += 1;
            return;
        }
        let parts = le16(pkt, 72);
        let size = le32(pkt, NAME_AT + l + SIZE_AFTER_NAME) as usize;
        let total = parts * self.cfg.part_bytes;
        if parts == 0 || size == 0 || total < size || total > self.cfg.max_file_bytes {
            self.stats.bad += 1;
            return;
        }
        if let Some(f) = self.open.get_mut(&id) {
            f.heard = now;
            self.stats.repeats += 1;
            return;
        }
        if self.recently_finished(id, now) {
            self.stats.repeats += 1;
            return;
        }
        while self.open_bytes + total > self.cfg.max_open_bytes {
            let Some((&old, _)) = self
                .open
                .iter()
                .min_by(|a, b| a.1.heard.total_cmp(&b.1.heard))
            else {
                break;
            };
            if let Some(f) = self.open.remove(&old) {
                self.stats.evicted += 1;
                self.finish(old, f, now, out);
            }
        }
        if self.open_bytes + total > self.cfg.max_open_bytes {
            self.stats.bad += 1;
            return;
        }
        let name = String::from_utf8_lossy(&pkt[NAME_AT..NAME_AT + l]).into_owned();
        self.open_bytes += total;
        self.open.insert(
            id,
            OpenFile {
                name,
                size,
                parts: parts as u32,
                have: vec![false; parts],
                received: 0,
                data: Vec::new(),
                heard: now,
                reserved: total,
            },
        );
    }

    fn part(&mut self, id: u32, pkt: &[u8], now: f64, out: &mut impl FnMut(FinishedFile)) {
        self.stats.parts += 1;
        if pkt.len() < PART_DATA_AT {
            self.stats.bad += 1;
            return;
        }
        let k = le16(pkt, 8);
        let p = self.cfg.part_bytes;
        let Some(f) = self.open.get_mut(&id) else {
            if self.recently_finished(id, now) {
                self.stats.repeats += 1;
            } else {
                self.stats.orphan += 1;
            }
            return;
        };
        if k >= f.have.len() {
            self.stats.bad += 1;
            return;
        }
        f.heard = now;
        if f.have[k] {
            self.stats.repeats += 1;
            return;
        }
        let data = &pkt[PART_DATA_AT..pkt.len().min(PART_DATA_AT + p)];
        let at = k * p;
        if f.data.len() < at + data.len() {
            f.data.resize(at + data.len(), 0);
        }
        f.data[at..at + data.len()].copy_from_slice(data);
        f.have[k] = true;
        f.received += 1;
        if f.received == f.parts {
            if let Some(f) = self.open.remove(&id) {
                self.finish(id, f, now, out);
            }
        }
    }

    fn finish(&mut self, id: u32, mut f: OpenFile, now: f64, out: &mut impl FnMut(FinishedFile)) {
        self.open_bytes -= f.reserved;
        self.finished.insert(id, now);
        f.data.resize(f.size, 0);
        out(FinishedFile {
            id,
            name: f.name,
            data: f.data,
            parts: f.parts,
            received: f.received,
            complete: f.received == f.parts,
        });
    }

    /// Finishes, as incomplete, the files silent for longer than the configured time at signal time `now`, and forgets
    /// finished ids older than the repeat window.
    pub fn expire(&mut self, now: f64, out: &mut impl FnMut(FinishedFile)) {
        self.since_expiry = 0;
        let silent: Vec<u32> = self
            .open
            .iter()
            .filter(|(_, f)| now - f.heard > self.cfg.max_silence_s)
            .map(|(&id, _)| id)
            .collect();
        for id in silent {
            if let Some(f) = self.open.remove(&id) {
                self.finish(id, f, now, out);
            }
        }
        let window = self.cfg.repeat_s;
        self.finished.retain(|_, t| now - *t <= window);
    }

    /// Finishes every open file (the end of the input).
    pub fn flush(&mut self, now: f64, out: &mut impl FnMut(FinishedFile)) {
        let ids: Vec<u32> = self.open.keys().copied().collect();
        for id in ids {
            if let Some(f) = self.open.remove(&id) {
                self.finish(id, f, now, out);
            }
        }
    }
}

/// A file name made safe to store: control characters (below 20 hex and 7F) and / \ : * ? " < > | become "_"; leading and
/// trailing dots and spaces go; a name longer than [`MAX_NAME`] bytes keeps its last bytes. The result can be empty.
pub fn clean_name(name: &str) -> String {
    let mapped: String = name
        .chars()
        .map(|c| {
            if (c as u32) < 0x20 || c as u32 == 0x7F || "/\\:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let t = mapped.trim_matches(|c| c == '.' || c == ' ');
    if t.len() <= MAX_NAME {
        return t.to_owned();
    }
    let mut start = t.len() - MAX_NAME;
    while !t.is_char_boundary(start) {
        start += 1;
    }
    t[start..].to_owned()
}

/// A head packet of `len` bytes (at least 85 + name length + 60) for file `id` named `name` of `size` bytes in `parts` parts.
pub fn head_packet(id: u32, name: &str, size: u32, parts: u16, len: usize) -> Vec<u8> {
    let need = NAME_AT + name.len() + 1 + SIZE_AFTER_NAME + 3;
    let len = len.max(need).min(0xFFFF);
    let mut p = vec![0u8; len];
    p[1] = HEAD;
    p[2..4].copy_from_slice(&(len as u16).to_le_bytes());
    p[4..8].copy_from_slice(&id.to_le_bytes());
    p[72..74].copy_from_slice(&parts.to_le_bytes());
    p[NAME_AT..NAME_AT + name.len()].copy_from_slice(name.as_bytes());
    let at = NAME_AT + name.len() + SIZE_AFTER_NAME;
    p[at..at + 4].copy_from_slice(&size.to_le_bytes());
    p
}

/// A part packet: file `id`, part `k`, `data`.
pub fn part_packet(id: u32, k: u16, data: &[u8]) -> Vec<u8> {
    let len = PART_DATA_AT + data.len();
    let mut p = vec![0u8; PART_DATA_AT];
    p[1] = PART;
    p[2..4].copy_from_slice(&(len.min(0xFFFF) as u16).to_le_bytes());
    p[4..8].copy_from_slice(&id.to_le_bytes());
    p[8..10].copy_from_slice(&k.to_le_bytes());
    p.extend_from_slice(data);
    p
}

/// A tail packet of file `id` (16 bytes).
pub fn tail_packet(id: u32) -> Vec<u8> {
    let mut p = vec![0u8; 16];
    p[1] = TAIL;
    p[2..4].copy_from_slice(&16u16.to_le_bytes());
    p[4..8].copy_from_slice(&id.to_le_bytes());
    p
}
