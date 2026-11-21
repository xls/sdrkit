//! The selective-repeat layer of `arq_packet`: the block format (byte offset, last flag, length, data), the sender that keeps
//! the byte ranges not yet acknowledged and has one burst in flight, the assembler of a receiver, the profile stepper that
//! follows the acknowledgements, and the one-way listener that keeps one assembler per station and lists a message once.

use super::{Profile, FLAG_ACK_REQUEST, FLAG_FINAL, FLAG_RETRANSMIT, MAX_BLOCKS, PROFILES};
use sdrkit_util_objects::{fingerprint, Dedup};
use std::collections::HashMap;

/// The largest message, bytes (offsets have 23 bits).
pub const MAX_MESSAGE: usize = 1 << 23;

/// One block of a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// The byte position of `data` in the message.
    pub offset: usize,
    /// Whether the block holds the last byte of the message.
    pub last: bool,
    /// The message bytes, at most 255.
    pub data: Vec<u8>,
}

impl Block {
    /// The `payload` bytes of the block: offset and last flag in 3 bytes, the length, the data, zeros.
    pub fn encode(&self, payload: usize) -> Vec<u8> {
        let o = self.offset as u32;
        let mut b = vec![((o >> 16) & 0x7F) as u8 | if self.last { 0x80 } else { 0 }, (o >> 8) as u8, o as u8, self.data.len() as u8];
        b.extend_from_slice(&self.data);
        b.resize(payload, 0);
        b
    }

    /// Reads a block of `bytes`; `None` when its length is above `bytes.len() - 4`.
    pub fn decode(bytes: &[u8]) -> Option<Block> {
        if bytes.len() < 4 {
            return None;
        }
        let l = usize::from(bytes[3]);
        if l > bytes.len() - 4 {
            return None;
        }
        Some(Block {
            offset: (usize::from(bytes[0] & 0x7F) << 16) | (usize::from(bytes[1]) << 8) | usize::from(bytes[2]),
            last: bytes[0] & 0x80 != 0,
            data: bytes[4..4 + l].to_vec(),
        })
    }

    /// The end of the block in the message.
    pub fn end(&self) -> usize {
        self.offset + self.data.len()
    }
}

/// The blocks of at most `c` bytes that cover `message[from..to]`; the block that ends the message has the last flag. An
/// empty message is one empty last block.
pub fn cut(message: &[u8], from: usize, to: usize, c: usize) -> Vec<Block> {
    if message.is_empty() {
        return vec![Block { offset: 0, last: true, data: Vec::new() }];
    }
    let mut out = Vec::new();
    let mut at = from;
    while at < to {
        let e = (at + c.max(1)).min(to);
        out.push(Block { offset: at, last: e == message.len(), data: message[at..e].to_vec() });
        at = e;
    }
    out
}

/// A burst that a sender wants to transmit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutBurst {
    /// The sequence number.
    pub seq: u8,
    /// The header flags.
    pub flags: u8,
    /// The profile id.
    pub profile: usize,
    /// The blocks, in burst order.
    pub blocks: Vec<Block>,
}

impl OutBurst {
    /// The payloads of the blocks at the burst's profile.
    pub fn payloads(&self) -> Vec<Vec<u8>> {
        let p = PROFILES[self.profile.min(8)].payload();
        self.blocks.iter().map(|b| b.encode(p)).collect()
    }
}

/// Removes `[a, b)` from a sorted list of disjoint ranges.
fn subtract(ranges: &mut Vec<(usize, usize)>, a: usize, b: usize) {
    let mut out = Vec::with_capacity(ranges.len() + 1);
    for &(s, e) in ranges.iter() {
        if e <= a || s >= b {
            out.push((s, e));
            continue;
        }
        if s < a {
            out.push((s, a));
        }
        if e > b {
            out.push((b, e));
        }
    }
    *ranges = out;
}

/// Adds `[a, b)` to a sorted list of disjoint ranges, merging neighbours.
fn add(ranges: &mut Vec<(usize, usize)>, a: usize, b: usize) {
    if a >= b {
        return;
    }
    ranges.push((a, b));
    ranges.sort_unstable();
    let mut out: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
    for &(s, e) in ranges.iter() {
        match out.last_mut() {
            Some(l) if s <= l.1 => l.1 = l.1.max(e),
            _ => out.push((s, e)),
        }
    }
    *ranges = out;
}

/// The two-way sender of one message.
#[derive(Clone, Debug)]
pub struct Sender {
    message: Vec<u8>,
    unacked: Vec<(usize, usize)>,
    in_flight: Option<(u8, Vec<(usize, usize)>)>,
    seq: u8,
    sent_end: usize,
    /// Payload bytes sent so far.
    pub payload_bytes: usize,
    /// Bursts sent so far.
    pub bursts: usize,
}

impl Sender {
    /// A sender of `message` (1 byte to 8 MiB; longer is cut, and an empty message is done at once); the first burst has
    /// sequence number `last_seq + 1`.
    pub fn new(message: &[u8], last_seq: u8) -> Sender {
        let m = message[..message.len().min(MAX_MESSAGE)].to_vec();
        let unacked = if m.is_empty() { Vec::new() } else { vec![(0, m.len())] };
        Sender { message: m, unacked, in_flight: None, seq: last_seq % 64, sent_end: 0, payload_bytes: 0, bursts: 0 }
    }

    /// Whether every byte was acknowledged.
    pub fn done(&self) -> bool {
        self.unacked.is_empty()
    }

    /// The sequence number of the burst in flight.
    pub fn in_flight(&self) -> Option<u8> {
        self.in_flight.as_ref().map(|f| f.0)
    }

    /// The message.
    pub fn message(&self) -> &[u8] {
        &self.message
    }

    /// The next burst at `profile` with at most `max_blocks` blocks (1 to 1023): the lowest bytes not yet acknowledged first.
    /// A burst still in flight counts as lost. `None` when the message is done.
    pub fn next_burst(&mut self, profile: usize, max_blocks: usize) -> Option<OutBurst> {
        if self.done() {
            return None;
        }
        self.in_flight = None;
        let p: Profile = PROFILES[profile.min(8)];
        let m = max_blocks.clamp(1, MAX_BLOCKS);
        let mut blocks = Vec::new();
        for &(s, e) in &self.unacked {
            for b in cut(&self.message, s, e, p.data()) {
                if blocks.len() == m {
                    break;
                }
                blocks.push(b);
            }
            if blocks.len() == m {
                break;
            }
        }
        let mut flags = FLAG_ACK_REQUEST;
        for b in &blocks {
            if b.offset < self.sent_end {
                flags |= FLAG_RETRANSMIT;
            }
            self.sent_end = self.sent_end.max(b.end());
            if b.last {
                flags |= FLAG_FINAL;
            }
        }
        self.seq = (self.seq + 1) % 64;
        let ranges = blocks.iter().map(|b| (b.offset, b.end())).collect();
        self.in_flight = Some((self.seq, ranges));
        self.payload_bytes += blocks.len() * p.payload();
        self.bursts += 1;
        Some(OutBurst { seq: self.seq, flags, profile, blocks })
    }

    /// Takes an acknowledgement of burst `seq`: every block whose bitmap entry is set leaves the unacknowledged set. Returns
    /// false (and changes nothing) when no burst is in flight or `seq` is not the one in flight.
    pub fn on_ack(&mut self, seq: u8, bitmap: &[bool]) -> bool {
        match &self.in_flight {
            Some((s, _)) if *s == seq => {}
            _ => return false,
        }
        let (_, ranges) = self.in_flight.take().unwrap_or_default();
        for (i, &(a, b)) in ranges.iter().enumerate().take(512) {
            if bitmap.get(i).copied().unwrap_or(false) {
                subtract(&mut self.unacked, a, b);
            }
        }
        true
    }

    /// No acknowledgement came: the burst in flight is lost and its bytes stay unacknowledged.
    pub fn on_timeout(&mut self) {
        self.in_flight = None;
    }
}

/// The assembler of one message at a receiver.
#[derive(Clone, Debug, Default)]
pub struct Assembler {
    buf: Vec<u8>,
    held: Vec<(usize, usize)>,
    total: Option<usize>,
}

impl Assembler {
    /// Places a good block. Returns false for a block outside the largest message.
    pub fn accept(&mut self, b: &Block) -> bool {
        if b.end() > MAX_MESSAGE {
            return false;
        }
        if self.buf.len() < b.end() {
            self.buf.resize(b.end(), 0);
        }
        self.buf[b.offset..b.end()].copy_from_slice(&b.data);
        add(&mut self.held, b.offset, b.end());
        if b.last {
            self.total = Some(b.end());
        }
        true
    }

    /// The whole message once its length is known and every byte of it is held.
    pub fn complete(&self) -> Option<&[u8]> {
        let t = self.total?;
        if t == 0 {
            return Some(&[]);
        }
        (self.held.first() == Some(&(0, t)) || self.held.iter().any(|&(s, e)| s == 0 && e >= t)).then(|| &self.buf[..t])
    }

    /// Takes the blocks of one data burst (the payload bytes of each block, `None` when its CRC failed) and returns the bitmap
    /// of an acknowledgement: set for each block that passed and whose header is valid.
    pub fn take_burst(&mut self, blocks: &[Option<Vec<u8>>]) -> Vec<bool> {
        blocks
            .iter()
            .map(|b| b.as_deref().and_then(Block::decode).is_some_and(|b| self.accept(&b)))
            .collect()
    }
}

/// The profile stepper of a sender.
#[derive(Clone, Debug)]
pub struct Stepper {
    /// The current profile id.
    pub profile: usize,
    hold: u32,
    fail_streak: u32,
    up: u32,
}

impl Stepper {
    /// A stepper that starts at `profile`.
    pub fn new(profile: usize) -> Stepper {
        Stepper { profile: profile.min(8), hold: 0, fail_streak: 0, up: 0 }
    }

    fn down(&mut self, n: usize) {
        self.profile = self.profile.saturating_sub(n);
        self.hold = 4;
        self.up = 0;
    }

    /// Takes an acknowledgement that reports `esn0_db` and `failed` of `blocks` blocks.
    pub fn on_ack(&mut self, esn0_db: f64, failed: usize, blocks: usize) {
        let held = self.hold > 0;
        if held {
            self.hold -= 1;
        }
        let f = failed as f64 / blocks.max(1) as f64;
        if f > 0.20 {
            self.fail_streak += 1;
            if self.fail_streak >= 2 {
                self.fail_streak = 0;
                self.down(2);
            } else {
                self.down(1);
            }
            return;
        }
        self.fail_streak = 0;
        let margin = esn0_db - PROFILES[self.profile].required_db;
        if margin < 1.0 && self.profile > 0 {
            self.down(1);
            return;
        }
        let can_up = self.profile < 8 && margin >= 3.0 && PROFILES[self.profile + 1].required_db + 1.5 <= esn0_db;
        if can_up && !held {
            self.up += 1;
            if self.up >= 2 {
                self.profile += 1;
                self.up = 0;
            }
        } else {
            self.up = 0;
        }
    }

    /// No acknowledgement came.
    pub fn on_timeout(&mut self) {
        self.fail_streak = 0;
        self.down(2);
    }
}

/// A message that a listener completed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delivered {
    /// The sending station.
    pub callsign: String,
    /// The message.
    pub bytes: Vec<u8>,
    /// Whether the same message from the same station was delivered in the last 120 s.
    pub repeat: bool,
}

/// The one-way listener: one assembler per station, dropped after 30 s without a burst from it; a message delivered again
/// within 120 s is a repeat.
#[derive(Clone, Debug)]
pub struct Listener {
    open: HashMap<String, (Assembler, f64)>,
    seen: Dedup<u64>,
}

impl Default for Listener {
    fn default() -> Self {
        Listener { open: HashMap::new(), seen: Dedup::new(120.0, 4096) }
    }
}

impl Listener {
    /// Takes the blocks of a data burst from `call` at signal time `now` (seconds) and returns the acknowledgement bitmap and the
    /// message it completed, if any.
    pub fn take(&mut self, call: &str, blocks: &[Option<Vec<u8>>], now: f64) -> (Vec<bool>, Option<Delivered>) {
        self.open.retain(|_, (_, t)| now - *t <= 30.0);
        let (asm, t) = self.open.entry(call.to_string()).or_default();
        *t = now;
        let bitmap = asm.take_burst(blocks);
        let Some(m) = asm.complete().map(<[u8]>::to_vec) else {
            return (bitmap, None);
        };
        self.open.remove(call);
        let mut key = call.as_bytes().to_vec();
        key.push(0);
        key.extend_from_slice(&m);
        let repeat = self.seen.seen(fingerprint(&key), now);
        (bitmap, Some(Delivered { callsign: call.to_string(), bytes: m, repeat }))
    }

    /// Forgets every station.
    pub fn clear(&mut self) {
        self.open.clear();
        self.seen = Dedup::new(120.0, 4096);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_vector() {
        let b = Block { offset: 0x012345, last: true, data: b"ABC".to_vec() };
        let e = b.encode(70);
        assert_eq!(&e[..8], &[0x81, 0x23, 0x45, 0x03, 0x41, 0x42, 0x43, 0x00]);
        assert!(e[8..].iter().all(|&v| v == 0) && e.len() == 70);
        assert_eq!(Block::decode(&e), Some(b));
        let mut bad = e.clone();
        bad[3] = 67;
        assert_eq!(Block::decode(&bad), None);
        let c: Vec<usize> = PROFILES.iter().map(Profile::data).collect();
        assert_eq!(c, [66, 66, 138, 210, 138, 210, 186, 210, 234]);
    }

    #[test]
    fn ranges() {
        let mut r = vec![(0, 100)];
        subtract(&mut r, 10, 20);
        subtract(&mut r, 90, 120);
        assert_eq!(r, [(0, 10), (20, 90)]);
        add(&mut r, 10, 20);
        assert_eq!(r, [(0, 90)]);
    }
}
