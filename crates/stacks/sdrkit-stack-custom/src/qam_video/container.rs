//! The container layer of `qam_video`: typed packets with a CRC-32, the fragments of a codec unit, and the FEC groups that
//! pack packets into cells protected by the Cauchy erasure code.

use crate::family::crc32;
use sdrkit_algo_fec::cauchy::CauchyCode;

/// Packet types.
pub const VIDEO: u8 = 0;
/// Audio.
pub const AUDIO: u8 = 1;
/// Text.
pub const TEXT: u8 = 2;
/// Picture.
pub const PICTURE: u8 = 3;
/// Data.
pub const DATA: u8 = 4;
/// The fill byte.
pub const FILL: u8 = 0xFF;

/// A packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    /// The type, 0 to 4.
    pub kind: u8,
    /// The stream, 0 to 15.
    pub stream: u8,
    /// The sequence number of the stream.
    pub seq: u16,
    /// The payload.
    pub payload: Vec<u8>,
}

impl Packet {
    /// The bytes of the packet: header, payload and CRC-32.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(9 + self.payload.len());
        v.push((self.kind << 4) | (self.stream & 15));
        v.extend_from_slice(&self.seq.to_be_bytes());
        v.extend_from_slice(&(self.payload.len() as u16).to_be_bytes());
        v.extend_from_slice(&self.payload);
        let c = crc32(&v);
        v.extend_from_slice(&c.to_be_bytes());
        v
    }
}

/// What parsing a packet gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Parsed {
    /// A packet and the bytes it took.
    Packet(Packet, usize),
    /// The bytes end inside the packet.
    NeedMore,
    /// Not a packet: a type above 4 or a CRC-32 that fails.
    Invalid,
}

/// Parses the packet at the start of `b`.
pub fn parse(b: &[u8]) -> Parsed {
    if b.len() < 5 {
        return Parsed::NeedMore;
    }
    let kind = b[0] >> 4;
    if kind > DATA {
        return Parsed::Invalid;
    }
    let len = usize::from(u16::from_be_bytes([b[3], b[4]]));
    if b.len() < 9 + len {
        return Parsed::NeedMore;
    }
    let c = u32::from_be_bytes([b[5 + len], b[6 + len], b[7 + len], b[8 + len]]);
    if crc32(&b[..5 + len]) != c {
        return Parsed::Invalid;
    }
    Parsed::Packet(
        Packet {
            kind,
            stream: b[0] & 15,
            seq: u16::from_be_bytes([b[1], b[2]]),
            payload: b[5..5 + len].to_vec(),
        },
        9 + len,
    )
}

/// The fragment payloads of a codec unit of time `ms` with at most `max` payload bytes each.
pub fn fragments(unit: &[u8], ms: u32, key: bool, max: usize) -> Vec<Vec<u8>> {
    let room = if max > 6 { max - 1 } else { 1 };
    let first_room = if room > 4 { room - 4 } else { 1 };
    let mut out = Vec::new();
    let mut at = 0;
    loop {
        let first = out.is_empty();
        let n = (if first { first_room } else { room }).min(unit.len() - at);
        let last = at + n >= unit.len();
        let mut p = vec![u8::from(first) | (u8::from(last) << 1) | (u8::from(key) << 2)];
        if first {
            p.extend_from_slice(&ms.to_be_bytes());
        }
        p.extend_from_slice(&unit[at..at + n]);
        at += n;
        out.push(p);
        if last {
            return out;
        }
    }
}

/// A codec unit put together again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    /// The presentation time, ms.
    pub ms: u32,
    /// A key unit.
    pub key: bool,
    /// The bytes.
    pub bytes: Vec<u8>,
}

/// Joins the fragments of one stream.
#[derive(Clone, Debug, Default)]
pub struct Reassembler {
    open: Option<(Unit, u16)>,
}

impl Reassembler {
    /// Takes the next packet of the stream; returns a unit when it completes one.
    pub fn push(&mut self, p: &Packet) -> Option<Unit> {
        let f = *p.payload.first()?;
        if f & 1 == 1 {
            if p.payload.len() < 5 {
                self.open = None;
                return None;
            }
            let ms = u32::from_be_bytes([p.payload[1], p.payload[2], p.payload[3], p.payload[4]]);
            self.open = Some((
                Unit {
                    ms,
                    key: f & 4 != 0,
                    bytes: p.payload[5..].to_vec(),
                },
                p.seq,
            ));
        } else {
            match self.open.as_mut() {
                Some((u, last)) if p.seq == last.wrapping_add(1) => {
                    u.bytes.extend_from_slice(&p.payload[1..]);
                    *last = p.seq;
                }
                _ => {
                    self.open = None;
                    return None;
                }
            }
        }
        if f & 2 != 0 {
            return self.open.take().map(|o| o.0);
        }
        None
    }
}

/// The FEC profiles: (data cells, parity cells).
pub const FEC: [(usize, usize); 8] = [(4, 0), (14, 2), (12, 4), (28, 4), (24, 8), (6, 2), (56, 8), (48, 16)];

/// The largest packet payload of a FEC profile with cells of `cell` bytes.
pub fn largest_payload(fec: usize, cell: usize) -> usize {
    FEC[fec].0 * (cell - 1) - 9
}

/// Packs packets into the cells of FEC groups (transmit).
pub struct Packer {
    g: usize,
    p: usize,
    cell: usize,
    buf: Vec<u8>,
    offsets: Vec<u8>,
    fill: usize,
    code: CauchyCode,
    /// Cells ready, in order; frames take four at a time.
    pub cells: Vec<Vec<u8>>,
}

impl Packer {
    /// A packer for FEC profile `fec` and cells of `cell` bytes.
    pub fn new(fec: usize, cell: usize) -> Packer {
        let (g, p) = FEC[fec];
        Packer {
            g,
            p,
            cell,
            buf: vec![FILL; g * (cell - 1)],
            offsets: vec![FILL; g],
            fill: 0,
            code: CauchyCode::new(g, p).expect("a FEC profile"),
            cells: Vec::new(),
        }
    }

    /// Adds a packet; false when it is larger than the largest payload.
    pub fn add(&mut self, packet: &Packet) -> bool {
        let b = packet.to_bytes();
        if b.len() > self.buf.len() {
            return false;
        }
        if b.len() > self.buf.len() - self.fill {
            self.close();
        }
        let c = self.fill / (self.cell - 1);
        if self.offsets[c] == FILL {
            self.offsets[c] = (self.fill % (self.cell - 1)) as u8;
        }
        self.buf[self.fill..self.fill + b.len()].copy_from_slice(&b);
        self.fill += b.len();
        if self.fill == self.buf.len() {
            self.close();
        }
        true
    }

    /// Closes the open group, when it holds a byte.
    pub fn close(&mut self) {
        if self.fill == 0 {
            return;
        }
        let c = self.cell;
        let mut data = Vec::with_capacity(self.g * c);
        for i in 0..self.g {
            data.push(self.offsets[i]);
            data.extend_from_slice(&self.buf[i * (c - 1)..(i + 1) * (c - 1)]);
        }
        let mut par = vec![0u8; self.p * c];
        let _ = self.code.encode(&data, c, &mut par);
        self.cells.extend(data.chunks(c).map(<[u8]>::to_vec));
        self.cells.extend(par.chunks(c).map(<[u8]>::to_vec));
        self.buf.fill(FILL);
        self.offsets.fill(FILL);
        self.fill = 0;
    }

    /// The payloads of the whole frames ready (four cells each).
    pub fn frames(&mut self) -> Vec<Vec<u8>> {
        let n = self.cells.len() / 4 * 4;
        self.cells.drain(..n).collect::<Vec<_>>().chunks(4).map(|f| f.concat()).collect()
    }
}

/// A group being received.
struct Open {
    number: u32,
    fec: usize,
    cells: Vec<u8>,
    present: Vec<bool>,
}

/// What the unpacker counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroupCounts {
    /// Groups closed.
    pub groups: u64,
    /// Groups restored by the erasure code.
    pub restored: u64,
    /// Groups with data cells missing that could not be restored.
    pub lost: u64,
}

/// Puts received cells into groups and reads the packets out (receive).
pub struct Unpacker {
    cell: usize,
    open: Vec<Open>,
    closed: Vec<u32>,
    /// The counters.
    pub counts: GroupCounts,
}

impl Unpacker {
    /// An unpacker for cells of `cell` bytes.
    pub fn new(cell: usize) -> Unpacker {
        Unpacker {
            cell,
            open: Vec::new(),
            closed: Vec::new(),
            counts: GroupCounts::default(),
        }
    }

    /// Takes the cells of a frame (`None` for a block that failed) and appends the packets of groups that close.
    pub fn frame(&mut self, seq: u16, fec: usize, cells: &[Option<Vec<u8>>], out: &mut Vec<Packet>) {
        let (g, p) = FEC[fec.min(7)];
        let per = (g + p) / 4;
        let number = u32::from(seq) / per as u32;
        if self.closed.contains(&number) {
            return;
        }
        // A frame of another group closes the others.
        let others: Vec<u32> = self.open.iter().map(|o| o.number).filter(|&n| n != number).collect();
        for n in others {
            self.close(n, out);
        }
        if !self.open.iter().any(|o| o.number == number) {
            let (g2, p2) = FEC[fec.min(7)];
            self.open.push(Open {
                number,
                fec: fec.min(7),
                cells: vec![0; (g2 + p2) * self.cell],
                present: vec![false; g2 + p2],
            });
        }
        let c = self.cell;
        let o = self.open.iter_mut().find(|o| o.number == number).expect("open group");
        let (gg, pp) = FEC[o.fec];
        let pos = usize::from(seq) % ((gg + pp) / 4);
        for (b, cell) in cells.iter().enumerate() {
            let i = 4 * pos + b;
            if let (Some(cell), true) = (cell, i < gg + pp) {
                o.cells[i * c..(i + 1) * c].copy_from_slice(&cell[..c]);
                o.present[i] = true;
            }
        }
        if o.present[..gg].iter().all(|&v| v) {
            self.close(number, out);
        }
    }

    /// Closes every open group (the end of the stream).
    pub fn finish(&mut self, out: &mut Vec<Packet>) {
        let all: Vec<u32> = self.open.iter().map(|o| o.number).collect();
        for n in all {
            self.close(n, out);
        }
    }

    fn close(&mut self, number: u32, out: &mut Vec<Packet>) {
        let Some(i) = self.open.iter().position(|o| o.number == number) else {
            return;
        };
        let mut o = self.open.remove(i);
        self.closed.push(number);
        if self.closed.len() > 16 {
            self.closed.remove(0);
        }
        self.counts.groups += 1;
        let (g, p) = FEC[o.fec];
        let c = self.cell;
        if o.present[..g].iter().any(|&v| !v) {
            let code = CauchyCode::new(g, p).expect("a FEC profile");
            if o.present.iter().filter(|&&v| v).count() >= g && code.recover(&mut o.cells, c, &mut o.present).is_ok() {
                self.counts.restored += 1;
            } else {
                self.counts.lost += 1;
            }
        }
        // Runs of present data cells.
        let mut k = 0;
        while k < g {
            if !o.present[k] {
                k += 1;
                continue;
            }
            let start = k;
            while k < g && o.present[k] {
                k += 1;
            }
            parse_run(&o.cells, c, start, k, out);
        }
    }
}

/// Reads the packets of the present data cells `start .. end`.
fn parse_run(cells: &[u8], c: usize, start: usize, end: usize, out: &mut Vec<Packet>) {
    let mut stream = Vec::with_capacity((end - start) * (c - 1));
    let mut offsets = Vec::with_capacity(end - start);
    for i in start..end {
        offsets.push(cells[i * c]);
        stream.extend_from_slice(&cells[i * c + 1..(i + 1) * c]);
    }
    // The start of each cell that has a packet start, in stream bytes.
    let starts: Vec<usize> = offsets
        .iter()
        .enumerate()
        .filter(|(_, &o)| o != FILL && usize::from(o) < c - 1)
        .map(|(i, &o)| i * (c - 1) + usize::from(o))
        .collect();
    let mut at = if start == 0 { 0 } else { match starts.first() {
        Some(&s) => s,
        None => return,
    } };
    while at < stream.len() {
        if stream[at] == FILL {
            break;
        }
        match parse(&stream[at..]) {
            Parsed::Packet(p, n) => {
                out.push(p);
                at += n;
            }
            Parsed::NeedMore => break,
            Parsed::Invalid => match starts.iter().find(|&&s| s / (c - 1) > at / (c - 1)) {
                Some(&s) => at = s,
                None => break,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_vector_and_damage() {
        let p = Packet {
            kind: AUDIO,
            stream: 3,
            seq: 0xBEEF,
            payload: vec![1, 2, 3, 4, 5],
        };
        let b = p.to_bytes();
        assert_eq!(b, vec![0x13, 0xBE, 0xEF, 0x00, 0x05, 1, 2, 3, 4, 5, 0x24, 0xA1, 0x81, 0x05]);
        assert_eq!(parse(&b), Parsed::Packet(p.clone(), 14));
        assert_eq!(parse(&b[..13]), Parsed::NeedMore);
        for i in 0..b.len() {
            let mut d = b.clone();
            d[i] ^= 0x40;
            assert!(matches!(parse(&d), Parsed::Invalid | Parsed::NeedMore), "{i}");
        }
        let e = Packet { payload: Vec::new(), ..p };
        assert_eq!(parse(&e.to_bytes()), Parsed::Packet(e.clone(), 9));
    }

    #[test]
    fn fragments_and_reassembly() {
        let unit: Vec<u8> = (0xA0..=0xA9).collect();
        let f = fragments(&unit, 123_456, true, 8);
        assert_eq!(f, vec![vec![0x05, 0x00, 0x01, 0xE2, 0x40, 0xA0, 0xA1, 0xA2], vec![0x06, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9]]);
        let big: Vec<u8> = (0..34).collect();
        let f = fragments(&big, 123_456, true, 8);
        assert_eq!(f.len(), 6);
        let mut r = Reassembler::default();
        let mut got = None;
        for (i, p) in f.iter().enumerate() {
            got = r.push(&Packet { kind: VIDEO, stream: 0, seq: i as u16, payload: p.clone() }).or(got);
        }
        assert_eq!(got, Some(Unit { ms: 123_456, key: true, bytes: big.clone() }));
        let mut r = Reassembler::default();
        let mut got = None;
        for (i, p) in f.iter().enumerate().filter(|(i, _)| *i != 3) {
            got = r.push(&Packet { kind: VIDEO, stream: 0, seq: i as u16, payload: p.clone() }).or(got);
        }
        assert_eq!(got, None);
        assert_eq!(fragments(&[], 5, false, 100).len(), 1);
    }

    #[test]
    fn groups_round_trip_with_losses() {
        for fec in 0..8 {
            let (g, p) = FEC[fec];
            assert!((g + p) % 4 == 0 && ((g + p) / 4).is_power_of_two());
            let mut pk = Packer::new(fec, 178);
            let mut sent = Vec::new();
            for i in 0..60u16 {
                let pl: Vec<u8> = (0..(37 * i as usize) % 900 + 1).map(|j| (j + i as usize) as u8).collect();
                let pl = pl[..pl.len().min(largest_payload(fec, 178))].to_vec();
                let packet = Packet { kind: DATA, stream: 1, seq: i, payload: pl };
                assert!(pk.add(&packet));
                sent.push(packet);
            }
            pk.close();
            let frames = pk.frames();
            assert!(pk.cells.is_empty());
            // Lose p cells of each group (none when p = 0).
            let mut un = Unpacker::new(178);
            let mut got = Vec::new();
            let per = (g + p) / 4;
            for (f, fr) in frames.iter().enumerate() {
                let cells: Vec<Option<Vec<u8>>> = fr
                    .chunks(178)
                    .enumerate()
                    .map(|(b, c)| {
                        let i = 4 * (f % per) + b;
                        (i >= p || p == 0).then(|| c.to_vec()).filter(|_| p == 0 || i >= p)
                    })
                    .collect();
                un.frame(f as u16, fec, &cells, &mut got);
            }
            un.finish(&mut got);
            assert_eq!(got, sent, "fec {fec}");
            if p > 0 {
                assert!(un.counts.restored > 0);
            }
            assert_eq!(un.counts.lost, 0);
        }
    }
}
