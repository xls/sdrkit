//! The session layer of `file_transfer`: the messages (little-endian fields), byte-range sets, the data block, the control
//! frames that carry a message, the two-way sender, the receiver rules with resume and the whole-file check, the profile
//! stepper, the grouping of data frames into bursts, the one-way carousel and the rules for a file name from the air.

use super::{max_blocks, FrameHeader, FrameKind, TxFrame, Width, CONTROL_PROFILE, MAX_CONTROL_BLOCKS, MAX_FRAMES, PROFILES};
use sdrkit_algo_crypto::blake2s::blake2s_256;
use sdrkit_algo_crypto::Sha256;
use std::collections::VecDeque;

/// The most ranges a message carries.
pub const MAX_RANGES: usize = 100;
/// The longest message a control frame carries, bytes.
pub const MAX_MESSAGE: usize = MAX_CONTROL_BLOCKS * 236 - 2;
/// The default largest payload a receiver takes, bytes.
pub const DEFAULT_LIMIT: u64 = 1 << 31;
/// Retries in a row after which a two-way transfer fails.
pub const MAX_RETRIES: u32 = 6;
/// The hash code of BLAKE2s-256.
pub const HASH_BLAKE2S: u8 = 0;
/// The hash code of SHA-256.
pub const HASH_SHA256: u8 = 1;

/// The whole-file hash of `code` (0 BLAKE2s-256, 1 SHA-256) over `parts` in order; `None` for another code.
pub fn hash_parts<'a>(code: u8, parts: impl IntoIterator<Item = &'a [u8]>) -> Option<[u8; 32]> {
    match code {
        HASH_BLAKE2S => {
            let mut h = sdrkit_algo_crypto::blake2s::Blake2s::new_256();
            for p in parts {
                h.update(p);
            }
            let mut out = [0u8; 32];
            out.copy_from_slice(&h.finalize());
            Some(out)
        }
        HASH_SHA256 => {
            let mut h = Sha256::new();
            for p in parts {
                h.update(p);
            }
            Some(h.finalize_reset())
        }
        _ => None,
    }
}

/// The whole-file hash of `code` over `data`.
pub fn hash(code: u8, data: &[u8]) -> Option<[u8; 32]> {
    hash_parts(code, [data])
}

/// A set of byte ranges `[start, end)`, sorted, without overlaps or touching neighbours.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ranges(Vec<(u64, u64)>);

impl Ranges {
    /// The set `[0, n)` (empty when `n` is 0).
    pub fn upto(n: u64) -> Ranges {
        let mut r = Ranges::default();
        r.add(0, n);
        r
    }

    /// The ranges in order.
    pub fn list(&self) -> &[(u64, u64)] {
        &self.0
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Bytes in the set.
    pub fn bytes(&self) -> u64 {
        self.0.iter().map(|r| r.1 - r.0).sum()
    }

    /// Adds `[a, e)`.
    pub fn add(&mut self, a: u64, e: u64) {
        if a >= e {
            return;
        }
        let (mut a, mut e) = (a, e);
        let mut out = Vec::with_capacity(self.0.len() + 1);
        let mut placed = false;
        for &(s, t) in &self.0 {
            if t < a {
                out.push((s, t));
            } else if s > e {
                if !placed {
                    out.push((a, e));
                    placed = true;
                }
                out.push((s, t));
            } else {
                a = a.min(s);
                e = e.max(t);
            }
        }
        if !placed {
            out.push((a, e));
        }
        self.0 = out;
    }

    /// Removes `[a, e)`.
    pub fn remove(&mut self, a: u64, e: u64) {
        if a >= e {
            return;
        }
        let mut out = Vec::with_capacity(self.0.len() + 1);
        for &(s, t) in &self.0 {
            if t <= a || s >= e {
                out.push((s, t));
                continue;
            }
            if s < a {
                out.push((s, a));
            }
            if t > e {
                out.push((e, t));
            }
        }
        self.0 = out;
    }

    /// Whether `[a, e)` is wholly in the set.
    pub fn covers(&self, a: u64, e: u64) -> bool {
        a >= e || self.0.iter().any(|&(s, t)| s <= a && e <= t)
    }

    /// The first range.
    pub fn first(&self) -> Option<(u64, u64)> {
        self.0.first().copied()
    }

    /// At most [`MAX_RANGES`] ranges for a message: all when there are few enough, else the largest, sorted by start.
    pub fn for_message(&self) -> Vec<(u32, u32)> {
        let mut r: Vec<(u64, u64)> = self.0.clone();
        if r.len() > MAX_RANGES {
            r.sort_by(|x, y| (y.1 - y.0).cmp(&(x.1 - x.0)).then(x.0.cmp(&y.0)));
            r.truncate(MAX_RANGES);
            r.sort();
        }
        r.iter().map(|&(a, e)| (a.min(u64::from(u32::MAX)) as u32, e.min(u64::from(u32::MAX)) as u32)).collect()
    }
}

/// A session message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// The announcement of a file (type 1).
    Open(Open),
    /// The receiver takes the file (type 2).
    Accept {
        /// Session id.
        session: u16,
        /// The receiving station.
        receiver: String,
        /// The measured Es/N0, tenths of a dB.
        esn0_tenths: i16,
        /// The chosen profile.
        profile: u8,
        /// The byte ranges the receiver already has.
        ranges: Vec<(u32, u32)>,
    },
    /// The data follows (type 3).
    Start {
        /// Session id.
        session: u16,
        /// The profile of the data.
        profile: u8,
    },
    /// All data was sent (type 4).
    Finish {
        /// Session id.
        session: u16,
        /// Payload bytes.
        size: u64,
        /// The hash code.
        hash_code: u8,
        /// The hash of the payload.
        hash: [u8; 32],
        /// The hash of the uncompressed file.
        uncompressed_hash: [u8; 32],
    },
    /// The result of the check (type 5).
    Verified {
        /// Session id.
        session: u16,
        /// The file is good.
        good: bool,
    },
    /// The check failed (type 6).
    Reject {
        /// Session id.
        session: u16,
        /// 0 blocks are missing, 1 hash mismatch, 2 decompression failed.
        reason: u8,
        /// For reason 0, the ranges the receiver has.
        ranges: Vec<(u32, u32)>,
    },
    /// The transfer ends (type 7).
    Abort {
        /// Session id.
        session: u16,
        /// 1 busy, 2 compression not supported, 3 file too big.
        reason: u8,
    },
    /// The sender asks for the acknowledgement of a burst again (type 8).
    Poll {
        /// Session id.
        session: u16,
        /// The burst number.
        burst: u16,
    },
    /// The acknowledgement of a burst (type 9).
    Ack(Ack),
}

/// The fields of OPEN.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Open {
    /// Session id.
    pub session: u16,
    /// The sending station.
    pub sender: String,
    /// The receiving station, "CQ" for anyone.
    pub receiver: String,
    /// The width code.
    pub width: u8,
    /// The file name.
    pub name: String,
    /// Payload bytes.
    pub size: u64,
    /// Bytes of the uncompressed file.
    pub uncompressed: u64,
    /// The hash code.
    pub hash_code: u8,
    /// The hash of the payload.
    pub hash: [u8; 32],
    /// The compression id (0 none).
    pub compression: u8,
    /// The starting profile.
    pub profile: u8,
    /// The resume offset hint (0).
    pub resume_hint: u32,
    /// File bytes a block at the starting profile.
    pub data_per_block: u16,
}

impl Open {
    /// The transfer key: BLAKE2s-256 of the name as str(96), the two sizes, the hash code, the hash and the compression id.
    pub fn key(&self) -> [u8; 32] {
        let mut b = Vec::new();
        put_str(&mut b, &self.name, 96);
        b.extend_from_slice(&self.size.to_le_bytes());
        b.extend_from_slice(&self.uncompressed.to_le_bytes());
        b.push(self.hash_code);
        b.extend_from_slice(&self.hash);
        b.push(self.compression);
        blake2s_256(&b)
    }
}

/// The fields of ACK.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ack {
    /// Session id.
    pub session: u16,
    /// The burst number.
    pub burst: u16,
    /// Frames of the burst as far as the receiver knows (at most 16).
    pub frames: u8,
    /// The measured Es/N0, tenths of a dB.
    pub esn0_tenths: i16,
    /// Block loss, percent.
    pub loss: u8,
    /// The profile the receiver asks for.
    pub profile: u8,
    /// For each frame index, `None` when its header was not decoded, else the CRC result of each block.
    pub blocks: Vec<Option<Vec<bool>>>,
}

fn put_str(b: &mut Vec<u8>, s: &str, max: usize) {
    let mut n = s.len().min(max);
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    b.push(n as u8);
    b.extend_from_slice(&s.as_bytes()[..n]);
}

fn put_ranges(b: &mut Vec<u8>, r: &[(u32, u32)]) {
    let r = &r[..r.len().min(MAX_RANGES)];
    b.extend_from_slice(&(r.len() as u16).to_le_bytes());
    for &(s, e) in r {
        b.extend_from_slice(&s.to_le_bytes());
        b.extend_from_slice(&e.to_le_bytes());
    }
}

/// A reader of message fields; every read fails past the end.
struct Rd<'a>(&'a [u8]);

impl Rd<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        if self.0.len() < n {
            return None;
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Some(a)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn str(&mut self) -> Option<String> {
        let n = usize::from(self.u8()?);
        Some(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    fn hash(&mut self) -> Option<[u8; 32]> {
        self.take(32)?.try_into().ok()
    }
    fn ranges(&mut self) -> Option<Vec<(u32, u32)>> {
        let c = usize::from(self.u16()?);
        if c > MAX_RANGES {
            return None;
        }
        (0..c).map(|_| Some((self.u32()?, self.u32()?))).collect()
    }
}

impl Message {
    /// The session id.
    pub fn session(&self) -> u16 {
        match self {
            Message::Open(o) => o.session,
            Message::Ack(a) => a.session,
            Message::Accept { session, .. }
            | Message::Start { session, .. }
            | Message::Finish { session, .. }
            | Message::Verified { session, .. }
            | Message::Reject { session, .. }
            | Message::Abort { session, .. }
            | Message::Poll { session, .. } => *session,
        }
    }

    /// The bytes of the message.
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::new();
        match self {
            Message::Open(o) => {
                b.push(1);
                b.extend_from_slice(&o.session.to_le_bytes());
                put_str(&mut b, &o.sender, 12);
                put_str(&mut b, &o.receiver, 12);
                b.push(o.width);
                put_str(&mut b, &o.name, 96);
                b.extend_from_slice(&o.size.to_le_bytes());
                b.extend_from_slice(&o.uncompressed.to_le_bytes());
                b.push(o.hash_code);
                b.extend_from_slice(&o.hash);
                b.push(o.compression);
                b.push(o.profile);
                b.extend_from_slice(&o.resume_hint.to_le_bytes());
                b.extend_from_slice(&o.data_per_block.to_le_bytes());
            }
            Message::Accept { session, receiver, esn0_tenths, profile, ranges } => {
                b.push(2);
                b.extend_from_slice(&session.to_le_bytes());
                put_str(&mut b, receiver, 12);
                b.extend_from_slice(&esn0_tenths.to_le_bytes());
                b.push(*profile);
                put_ranges(&mut b, ranges);
            }
            Message::Start { session, profile } => {
                b.push(3);
                b.extend_from_slice(&session.to_le_bytes());
                b.push(*profile);
            }
            Message::Finish { session, size, hash_code, hash, uncompressed_hash } => {
                b.push(4);
                b.extend_from_slice(&session.to_le_bytes());
                b.extend_from_slice(&size.to_le_bytes());
                b.push(*hash_code);
                b.extend_from_slice(hash);
                b.extend_from_slice(uncompressed_hash);
            }
            Message::Verified { session, good } => {
                b.push(5);
                b.extend_from_slice(&session.to_le_bytes());
                b.push(u8::from(*good));
            }
            Message::Reject { session, reason, ranges } => {
                b.push(6);
                b.extend_from_slice(&session.to_le_bytes());
                b.push(*reason);
                put_ranges(&mut b, ranges);
            }
            Message::Abort { session, reason } => {
                b.push(7);
                b.extend_from_slice(&session.to_le_bytes());
                b.push(*reason);
            }
            Message::Poll { session, burst } => {
                b.push(8);
                b.extend_from_slice(&session.to_le_bytes());
                b.extend_from_slice(&burst.to_le_bytes());
            }
            Message::Ack(a) => {
                b.push(9);
                b.extend_from_slice(&a.session.to_le_bytes());
                b.extend_from_slice(&a.burst.to_le_bytes());
                b.push(a.frames);
                b.extend_from_slice(&a.esn0_tenths.to_le_bytes());
                b.push(a.loss);
                b.push(a.profile);
                let mut seen = 0u16;
                for (i, f) in a.blocks.iter().enumerate().take(MAX_FRAMES) {
                    if f.is_some() {
                        seen |= 1 << i;
                    }
                }
                b.extend_from_slice(&seen.to_le_bytes());
                for f in a.blocks.iter().take(MAX_FRAMES).flatten() {
                    b.push(f.len() as u8);
                    let mut bytes = vec![0u8; f.len().div_ceil(8)];
                    for (k, &ok) in f.iter().enumerate() {
                        if ok {
                            bytes[k / 8] |= 1 << (k % 8);
                        }
                    }
                    b.extend_from_slice(&bytes);
                }
            }
        }
        b
    }

    /// Reads a message; `None` for an unknown type or fields that do not fit. Extra bytes are ignored.
    pub fn decode(bytes: &[u8]) -> Option<Message> {
        let mut r = Rd(bytes);
        let t = r.u8()?;
        let session = r.u16()?;
        Some(match t {
            1 => Message::Open(Open {
                session,
                sender: r.str()?,
                receiver: r.str()?,
                width: r.u8()?,
                name: r.str()?,
                size: r.u64()?,
                uncompressed: r.u64()?,
                hash_code: r.u8()?,
                hash: r.hash()?,
                compression: r.u8()?,
                profile: r.u8()?,
                resume_hint: r.u32()?,
                data_per_block: r.u16()?,
            }),
            2 => Message::Accept {
                session,
                receiver: r.str()?,
                esn0_tenths: r.u16()? as i16,
                profile: r.u8()?,
                ranges: r.ranges()?,
            },
            3 => Message::Start { session, profile: r.u8()? },
            4 => Message::Finish {
                session,
                size: r.u64()?,
                hash_code: r.u8()?,
                hash: r.hash()?,
                uncompressed_hash: r.hash()?,
            },
            5 => Message::Verified { session, good: r.u8()? == 1 },
            6 => Message::Reject { session, reason: r.u8()?, ranges: r.ranges()? },
            7 => Message::Abort { session, reason: r.u8()? },
            8 => Message::Poll { session, burst: r.u16()? },
            9 => {
                let burst = r.u16()?;
                let frames = r.u8()?;
                let esn0_tenths = r.u16()? as i16;
                let loss = r.u8()?;
                let profile = r.u8()?;
                let seen = r.u16()?;
                let mut blocks = Vec::new();
                for i in 0..MAX_FRAMES.min(usize::from(frames).max(16 - seen.leading_zeros() as usize)) {
                    if seen & (1 << i) == 0 {
                        blocks.push(None);
                        continue;
                    }
                    let nb = usize::from(r.u8()?);
                    let bits = r.take(nb.div_ceil(8))?;
                    blocks.push(Some((0..nb).map(|k| bits[k / 8] & (1 << (k % 8)) != 0).collect()));
                }
                Message::Ack(Ack { session, burst, frames, esn0_tenths, loss, profile, blocks })
            }
            _ => return None,
        })
    }

    /// The frame kind that carries the message: acknowledgements in acknowledgement frames, the others in control frames.
    pub fn frame_kind(&self) -> FrameKind {
        match self {
            Message::Ack(_) => FrameKind::Ack,
            _ => FrameKind::Control,
        }
    }
}

/// The block contents of a message: its length (u16 little-endian) and its bytes, cut into chunks of the payload of profile 1;
/// `None` when it needs more than 5 blocks.
pub fn message_chunks(m: &Message) -> Option<Vec<Vec<u8>>> {
    let body = m.encode();
    if body.len() > MAX_MESSAGE {
        return None;
    }
    let mut b = (body.len() as u16).to_le_bytes().to_vec();
    b.extend_from_slice(&body);
    let p = PROFILES[CONTROL_PROFILE].payload();
    Some(b.chunks(p).map(<[u8]>::to_vec).collect())
}

/// The message of the blocks of a control or acknowledgement frame; `None` when a block failed or the length does not fit.
pub fn join_message(blocks: &[Option<Vec<u8>>]) -> Option<Message> {
    let mut b = Vec::new();
    for x in blocks {
        b.extend_from_slice(x.as_ref()?);
    }
    if b.len() < 2 {
        return None;
    }
    let len = usize::from(u16::from_le_bytes([b[0], b[1]]));
    if len > b.len() - 2 {
        return None;
    }
    Message::decode(&b[2..2 + len])
}

/// The frame that carries `m` from `call`, with frame number `frame`.
pub fn control_frame(m: &Message, width: Width, call: &str, frame: u16) -> Option<TxFrame> {
    let blocks = message_chunks(m)?;
    Some(TxFrame {
        header: FrameHeader {
            width,
            profile: CONTROL_PROFILE as u8,
            kind: m.frame_kind(),
            session: m.session(),
            frame,
            blocks: blocks.len() as u8,
            index: 0,
            last: true,
            callsign: call.to_string(),
        },
        blocks,
    })
}

/// The content of a data block: the byte offset, then the file bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataBlock {
    /// The byte offset of `data` in the payload.
    pub offset: u32,
    /// The bytes.
    pub data: Vec<u8>,
}

impl DataBlock {
    /// The block content: offset u32, length u16 (both little-endian), the data.
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(6 + self.data.len());
        b.extend_from_slice(&self.offset.to_le_bytes());
        b.extend_from_slice(&(self.data.len() as u16).to_le_bytes());
        b.extend_from_slice(&self.data);
        b
    }

    /// Reads the content of a block of `payload` bytes; `None` when the length is above `payload - 6`.
    pub fn decode(payload: &[u8]) -> Option<DataBlock> {
        if payload.len() < 6 {
            return None;
        }
        let offset = u32::from_le_bytes(payload[..4].try_into().ok()?);
        let l = usize::from(u16::from_le_bytes([payload[4], payload[5]]));
        if l > payload.len() - 6 {
            return None;
        }
        Some(DataBlock { offset, data: payload[6..6 + l].to_vec() })
    }

    /// The end of the block in the payload.
    pub fn end(&self) -> u64 {
        u64::from(self.offset) + self.data.len() as u64
    }
}

/// A burst to send: its number and profile, and the blocks of each frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Burst {
    /// The burst number, 0 to 4095.
    pub number: u16,
    /// The profile id.
    pub profile: u8,
    /// The data blocks of each frame.
    pub frames: Vec<Vec<DataBlock>>,
}

impl Burst {
    /// The frames of the burst from `call` in session `session` and `width`.
    pub fn tx_frames(&self, session: u16, width: Width, call: &str) -> Vec<TxFrame> {
        let n = self.frames.len();
        self.frames
            .iter()
            .enumerate()
            .map(|(i, f)| TxFrame {
                header: FrameHeader {
                    width,
                    profile: self.profile,
                    kind: FrameKind::Data,
                    session,
                    frame: (self.number << 4) | i as u16,
                    blocks: f.len() as u8,
                    index: i as u8,
                    last: i + 1 == n,
                    callsign: call.to_string(),
                },
                blocks: f.iter().map(DataBlock::encode).collect(),
            })
            .collect()
    }

    /// File bytes in the burst.
    pub fn data_bytes(&self) -> usize {
        self.frames.iter().flatten().map(|b| b.data.len()).sum()
    }
}

/// Fills a burst from the "to do" set: up to 16 frames of the most blocks of `profile` in `width`, `D` bytes a block from
/// the first range each time; the ranges are taken out of `todo`. `None` when `todo` is empty.
pub fn fill_burst(todo: &mut Ranges, payload: &[u8], number: u16, profile: usize, width: Width) -> Option<Burst> {
    let p = PROFILES[profile.min(9)];
    let per = max_blocks(&p, width);
    let d = p.data() as u64;
    let mut frames = Vec::new();
    while frames.len() < MAX_FRAMES && !todo.is_empty() {
        let mut f = Vec::new();
        while f.len() < per {
            let Some((a, e)) = todo.first() else { break };
            let l = d.min(e - a);
            f.push(DataBlock { offset: a as u32, data: payload[a as usize..(a + l) as usize].to_vec() });
            todo.remove(a, a + l);
        }
        frames.push(f);
    }
    (!frames.is_empty()).then_some(Burst { number, profile: profile as u8, frames })
}

/// The profile stepper of a receiver (the profile it asks the sender for).
#[derive(Clone, Debug)]
pub struct Stepper {
    /// The current profile.
    pub profile: usize,
    history: VecDeque<f64>,
    hold: u32,
    good: u32,
    /// The lowest profile.
    pub min: usize,
    /// The highest profile.
    pub max: usize,
}

impl Stepper {
    /// A stepper at `profile` over profiles 0 to 9.
    pub fn new(profile: usize) -> Stepper {
        Stepper { profile: profile.min(9), history: VecDeque::new(), hold: 0, good: 0, min: 0, max: 9 }
    }

    /// The step after a burst with Es/N0 `esn0` dB and block loss `loss` (0 to 1); returns the new profile.
    pub fn after_burst(&mut self, esn0: f64, loss: f64) -> usize {
        self.history.push_back(esn0);
        while self.history.len() > 4 {
            self.history.pop_front();
        }
        let avg = self.history.iter().sum::<f64>() / self.history.len() as f64;
        self.hold = self.hold.saturating_sub(1);
        let c = self.profile;
        if avg < PROFILES[c].required_db + 0.5 || loss > 0.2 {
            let down = if loss > 0.5 { 2 } else { 1 };
            self.profile = c.saturating_sub(down).max(self.min);
            self.good = 0;
            if self.profile < c {
                self.hold = 10;
            }
            return self.profile;
        }
        if c < self.max && avg >= PROFILES[c + 1].required_db + 2.0 && loss < 0.02 {
            self.good += 1;
        } else {
            self.good = 0;
        }
        if self.good >= 3 && self.hold == 0 {
            self.good = 0;
            self.profile = c + 1;
        }
        self.profile
    }
}

/// The starting profile for a measured Es/N0: the highest profile 1 to 9 with `Req + 2 dB <= esn0`, else 0; `fallback` when
/// the Es/N0 is unknown.
pub fn start_profile(esn0: Option<f64>, fallback: u8) -> u8 {
    let Some(e) = esn0 else {
        return fallback;
    };
    (1..=9).rev().find(|&p| PROFILES[p].required_db + 2.0 <= e).unwrap_or(0) as u8
}

/// Es/N0 in tenths of a dB for a message: clipped to -30 to +60 dB; 0 when unknown.
pub fn tenths(esn0: Option<f64>) -> i16 {
    esn0.map_or(0, |e| (e.clamp(-30.0, 60.0) * 10.0).round() as i16)
}

/// What a two-way sender puts on the air next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SenderOut {
    /// A message in a control frame.
    Control(Message),
    /// A data burst.
    Burst(Burst),
}

/// Where a two-way sender is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SenderState {
    /// OPEN sent; waiting for ACCEPT.
    Opening,
    /// A burst is out; waiting for its ACK.
    Sending,
    /// FINISH sent; waiting for VERIFIED.
    Finishing,
    /// The receiver confirmed the file.
    Done,
    /// The transfer failed, with the reason.
    Failed(String),
}

/// The settings of a two-way sender.
#[derive(Clone, Debug)]
pub struct SenderConfig {
    /// Session id.
    pub session: u16,
    /// The sending station.
    pub callsign: String,
    /// The receiving station ("CQ" for anyone).
    pub recipient: String,
    /// The width.
    pub width: Width,
    /// The file name (without path).
    pub name: String,
    /// The starting profile.
    pub profile: usize,
    /// Keep the profile whatever the receiver asks.
    pub fixed_profile: bool,
    /// The lowest and highest profile the sender takes.
    pub profiles: (usize, usize),
}

/// The sender of a two-way session.
#[derive(Clone, Debug)]
pub struct Sender {
    cfg: SenderConfig,
    payload: Vec<u8>,
    hash: [u8; 32],
    todo: Ranges,
    acked: Ranges,
    profile: usize,
    burst: u16,
    in_flight: Option<Burst>,
    retries: u32,
    state: SenderState,
    /// File bytes put in data blocks so far.
    pub data_sent: u64,
}

impl Sender {
    /// A sender of `payload` (the file, raw).
    pub fn new(cfg: SenderConfig, payload: Vec<u8>) -> Sender {
        let hash = hash(HASH_BLAKE2S, &payload).expect("a known hash code");
        let profile = cfg.profile.clamp(cfg.profiles.0, cfg.profiles.1);
        Sender {
            todo: Ranges::upto(payload.len() as u64),
            acked: Ranges::default(),
            payload,
            hash,
            profile,
            burst: 0,
            in_flight: None,
            retries: 0,
            state: SenderState::Opening,
            data_sent: 0,
            cfg,
        }
    }

    /// Where the sender is.
    pub fn state(&self) -> &SenderState {
        &self.state
    }

    /// The current profile.
    pub fn profile(&self) -> usize {
        self.profile
    }

    /// The settings.
    pub fn config(&self) -> &SenderConfig {
        &self.cfg
    }

    /// Bytes the receiver acknowledged.
    pub fn acked_bytes(&self) -> u64 {
        self.acked.bytes()
    }

    /// The payload size.
    pub fn size(&self) -> u64 {
        self.payload.len() as u64
    }

    /// The OPEN of the transfer.
    pub fn open(&self) -> Message {
        Message::Open(Open {
            session: self.cfg.session,
            sender: self.cfg.callsign.clone(),
            receiver: self.cfg.recipient.clone(),
            width: self.cfg.width.code(),
            name: self.cfg.name.clone(),
            size: self.payload.len() as u64,
            uncompressed: self.payload.len() as u64,
            hash_code: HASH_BLAKE2S,
            hash: self.hash,
            compression: 0,
            profile: self.profile as u8,
            resume_hint: 0,
            data_per_block: PROFILES[self.profile].data() as u16,
        })
    }

    fn finish(&self) -> Message {
        Message::Finish {
            session: self.cfg.session,
            size: self.payload.len() as u64,
            hash_code: HASH_BLAKE2S,
            hash: self.hash,
            uncompressed_hash: self.hash,
        }
    }

    fn take_profile(&mut self, p: u8) {
        if !self.cfg.fixed_profile {
            self.profile = usize::from(p).clamp(self.cfg.profiles.0, self.cfg.profiles.1.min(9));
        }
    }

    /// The next burst, or FINISH when nothing is left to send.
    fn next(&mut self) -> SenderOut {
        match fill_burst(&mut self.todo, &self.payload, self.burst, self.profile, self.cfg.width) {
            Some(b) => {
                self.data_sent += b.data_bytes() as u64;
                self.in_flight = Some(b.clone());
                self.state = SenderState::Sending;
                SenderOut::Burst(b)
            }
            None => {
                self.in_flight = None;
                self.state = SenderState::Finishing;
                SenderOut::Control(self.finish())
            }
        }
    }

    /// The first transmission: OPEN.
    pub fn start(&mut self) -> SenderOut {
        self.state = SenderState::Opening;
        SenderOut::Control(self.open())
    }

    /// Takes a reply; returns what goes on the air in answer (in order).
    pub fn on_message(&mut self, m: &Message) -> Vec<SenderOut> {
        if m.session() != self.cfg.session || matches!(self.state, SenderState::Done | SenderState::Failed(_)) {
            return Vec::new();
        }
        let size = self.payload.len() as u64;
        match (m, &self.state) {
            (Message::Abort { reason, .. }, _) => {
                self.state = SenderState::Failed(format!("the receiver ended the transfer (reason {reason})"));
                Vec::new()
            }
            (Message::Accept { profile, ranges, .. }, SenderState::Opening) => {
                for &(a, e) in ranges {
                    let (a, e) = (u64::from(a).min(size), u64::from(e).min(size));
                    self.todo.remove(a, e);
                    self.acked.add(a, e);
                }
                self.take_profile(*profile);
                self.retries = 0;
                let start = SenderOut::Control(Message::Start { session: self.cfg.session, profile: self.profile as u8 });
                vec![start, self.next()]
            }
            (Message::Reject { .. }, SenderState::Opening) => {
                self.state = SenderState::Failed("the receiver refused the file".into());
                Vec::new()
            }
            (Message::Ack(a), SenderState::Sending) if self.in_flight.as_ref().is_some_and(|b| b.number == a.burst) => {
                let b = self.in_flight.take().expect("a burst in flight");
                for (i, f) in b.frames.iter().enumerate() {
                    let got = a.blocks.get(i).and_then(|v| v.as_ref());
                    for (j, blk) in f.iter().enumerate() {
                        let ok = got.is_some_and(|g| g.get(j).copied().unwrap_or(false));
                        let (s, e) = (u64::from(blk.offset), blk.end());
                        if ok {
                            self.acked.add(s, e);
                        } else if !self.acked.covers(s, e) {
                            self.todo.add(s, e);
                        }
                    }
                }
                self.take_profile(a.profile);
                self.retries = 0;
                self.burst = (self.burst + 1) % 4096;
                vec![self.next()]
            }
            (Message::Verified { good, .. }, SenderState::Finishing) => {
                self.state = if *good {
                    SenderState::Done
                } else {
                    SenderState::Failed("the receiver found the file bad".into())
                };
                Vec::new()
            }
            (Message::Reject { reason, ranges, .. }, SenderState::Finishing) => {
                self.todo = Ranges::upto(size);
                self.acked = Ranges::default();
                if *reason == 0 {
                    for &(a, e) in ranges {
                        let (a, e) = (u64::from(a).min(size), u64::from(e).min(size));
                        self.todo.remove(a, e);
                        self.acked.add(a, e);
                    }
                }
                self.retries = 0;
                vec![self.next()]
            }
            _ => Vec::new(),
        }
    }

    /// No reply came in time: OPEN, POLL or FINISH again; nothing when the transfer fails after 6 retries in a row.
    pub fn on_timeout(&mut self) -> Option<SenderOut> {
        let m = match &self.state {
            SenderState::Opening => self.open(),
            SenderState::Sending => Message::Poll {
                session: self.cfg.session,
                burst: self.in_flight.as_ref().map_or(self.burst, |b| b.number),
            },
            SenderState::Finishing => self.finish(),
            _ => return None,
        };
        self.retries += 1;
        if self.retries > MAX_RETRIES {
            self.state = SenderState::Failed(format!("no reply after {MAX_RETRIES} retries"));
            return None;
        }
        Some(SenderOut::Control(m))
    }
}

/// One data frame as the receiver decoded it.
#[derive(Clone, Debug, PartialEq)]
pub struct RxData {
    /// The frame index in the burst.
    pub index: u8,
    /// The last frame of the burst.
    pub last: bool,
    /// The profile id.
    pub profile: u8,
    /// The payload of each block; `None` when its CRC failed.
    pub blocks: Vec<Option<Vec<u8>>>,
    /// The Es/N0 of the frame, dB.
    pub esn0_db: f64,
}

/// A burst as the receiver collected it.
#[derive(Clone, Debug, PartialEq)]
pub struct RxBurst {
    /// Session id.
    pub session: u16,
    /// The burst number.
    pub number: u16,
    /// The frames by index.
    pub frames: Vec<RxData>,
}

/// Groups data frames into bursts by session id and burst number. A burst is complete when its last frame arrives, when a
/// frame of another burst (or a repeated index) arrives, on [`Collector::flush`], or 1 s after its last frame.
#[derive(Clone, Debug, Default)]
pub struct Collector {
    open: Option<(RxBurst, f64)>,
}

impl Collector {
    /// Takes a data frame of session `session` and frame number `frame` at time `now`; appends the bursts it completes.
    pub fn take(&mut self, session: u16, frame: u16, f: RxData, now: f64, out: &mut Vec<RxBurst>) {
        let number = frame >> 4;
        if let Some((b, _)) = &self.open {
            if b.session != session || b.number != number || b.frames.iter().any(|g| g.index == f.index) {
                self.flush(out);
            }
        }
        let last = f.last;
        let b = self.open.get_or_insert_with(|| (RxBurst { session, number, frames: Vec::new() }, now));
        b.0.frames.push(f);
        b.1 = now;
        if last {
            self.flush(out);
        }
    }

    /// Completes a burst that saw no frame for 1 s.
    pub fn tick(&mut self, now: f64, out: &mut Vec<RxBurst>) {
        if self.open.as_ref().is_some_and(|(_, t)| now - t > 1.0) {
            self.flush(out);
        }
    }

    /// Completes the open burst.
    pub fn flush(&mut self, out: &mut Vec<RxBurst>) {
        if let Some((b, _)) = self.open.take() {
            out.push(b);
        }
    }
}

/// What the receiver side of a session reports.
#[derive(Clone, Debug, PartialEq)]
pub enum RxEvent {
    /// A transfer was announced: a new one, or one held before (`held` bytes already there).
    Opened {
        /// Session id.
        session: u16,
        /// The sending station.
        sender: String,
        /// The name as announced.
        name: String,
        /// Payload bytes.
        size: u64,
        /// Bytes already held from an earlier session.
        held: u64,
    },
    /// The file passed every check.
    Delivered {
        /// Session id.
        session: u16,
        /// The sending station.
        sender: String,
        /// The name as announced.
        name: String,
        /// The file.
        bytes: Vec<u8>,
        /// Its hash.
        hash: [u8; 32],
    },
    /// The whole-file check failed: the received bytes were dropped.
    HashMismatch {
        /// Session id.
        session: u16,
    },
    /// FINISH came while blocks are missing.
    Missing {
        /// Session id.
        session: u16,
        /// Bytes missing.
        bytes: u64,
    },
    /// An OPEN was answered with ABORT.
    Refused {
        /// Session id.
        session: u16,
        /// The ABORT reason.
        reason: u8,
    },
}

/// Bytes of a page of a received payload.
const PAGE: u64 = 1 << 16;

/// One transfer at the receiver: the announcement, the bytes received in pages and the ranges they cover.
#[derive(Clone, Debug)]
struct Transfer {
    key: [u8; 32],
    open: Open,
    pages: Vec<Option<Box<[u8]>>>,
    have: Ranges,
    complete: bool,
    stepper: Stepper,
    last_ack: Option<Message>,
}

impl Transfer {
    fn new(open: &Open) -> Transfer {
        Transfer {
            key: open.key(),
            open: open.clone(),
            pages: vec![None; open.size.div_ceil(PAGE) as usize],
            have: Ranges::default(),
            complete: false,
            stepper: Stepper::new(usize::from(open.profile).min(9)),
            last_ack: None,
        }
    }

    fn write(&mut self, offset: u64, data: &[u8]) {
        let mut at = offset;
        let mut d = data;
        while !d.is_empty() {
            let p = (at / PAGE) as usize;
            let o = (at % PAGE) as usize;
            let n = d.len().min(PAGE as usize - o);
            let page = self.pages[p].get_or_insert_with(|| vec![0u8; PAGE as usize].into_boxed_slice());
            page[o..o + n].copy_from_slice(&d[..n]);
            at += n as u64;
            d = &d[n..];
        }
        self.have.add(offset, offset + data.len() as u64);
    }

    fn parts(&self) -> impl Iterator<Item = &[u8]> {
        let size = self.open.size;
        self.pages.iter().enumerate().map(move |(i, p)| {
            let n = (size - i as u64 * PAGE).min(PAGE) as usize;
            p.as_deref().map_or(&ZEROS[..n], |b| &b[..n])
        })
    }

    fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.open.size as usize);
        for p in self.parts() {
            out.extend_from_slice(p);
        }
        out
    }

    fn drop_bytes(&mut self) {
        self.pages.iter_mut().for_each(|p| *p = None);
        self.have = Ranges::default();
    }
}

static ZEROS: [u8; PAGE as usize] = [0; PAGE as usize];

/// The receiver side of the session protocol: it answers OPEN, bursts, POLL and FINISH, resumes a held transfer, and
/// delivers a file only when every byte is there and the hash checks.
#[derive(Clone, Debug)]
pub struct SessionReceiver {
    /// The own callsign; empty takes every OPEN.
    pub callsign: String,
    /// The largest payload taken, bytes.
    pub limit: u64,
    active: Option<Transfer>,
    held: Vec<Transfer>,
}

impl Default for SessionReceiver {
    fn default() -> Self {
        SessionReceiver { callsign: String::new(), limit: DEFAULT_LIMIT, active: None, held: Vec::new() }
    }
}

/// Transfers kept for a resume.
const HELD: usize = 4;

impl SessionReceiver {
    /// A receiver that takes payloads up to `limit` bytes.
    pub fn with_limit(limit: u64) -> SessionReceiver {
        SessionReceiver { limit, ..SessionReceiver::default() }
    }

    /// The active transfer: session id, payload size, bytes held, and whether it is complete.
    pub fn active(&self) -> Option<(u16, u64, u64, bool)> {
        self.active.as_ref().map(|t| (t.open.session, t.open.size, t.have.bytes(), t.complete))
    }

    /// The name, sender and hash of the active transfer.
    pub fn active_info(&self) -> Option<(&str, &str, [u8; 32])> {
        self.active.as_ref().map(|t| (t.open.name.as_str(), t.open.sender.as_str(), t.open.hash))
    }

    /// Forgets every transfer.
    pub fn clear(&mut self) {
        self.active = None;
        self.held.clear();
    }

    fn park(&mut self) {
        if let Some(t) = self.active.take() {
            if !t.complete {
                self.held.retain(|h| h.key != t.key);
                self.held.push(t);
                if self.held.len() > HELD {
                    self.held.remove(0);
                }
            }
        }
    }

    fn accept(t: &Transfer, call: &str, esn0: Option<f64>) -> Message {
        Message::Accept {
            session: t.open.session,
            receiver: call.to_string(),
            esn0_tenths: tenths(esn0),
            profile: start_profile(esn0, t.open.profile),
            ranges: t.have.for_message(),
        }
    }

    /// Takes a message heard with Es/N0 `esn0`; appends the replies and what happened.
    pub fn on_message(&mut self, m: &Message, esn0: Option<f64>, replies: &mut Vec<Message>, ev: &mut Vec<RxEvent>) {
        let call = self.callsign.clone();
        match m {
            Message::Open(o) => {
                if !call.is_empty() && !o.receiver.eq_ignore_ascii_case("CQ") && !o.receiver.eq_ignore_ascii_case(&call) {
                    return;
                }
                if o.size > self.limit {
                    replies.push(Message::Abort { session: o.session, reason: 3 });
                    ev.push(RxEvent::Refused { session: o.session, reason: 3 });
                    return;
                }
                if o.compression != 0 {
                    replies.push(Message::Abort { session: o.session, reason: 2 });
                    ev.push(RxEvent::Refused { session: o.session, reason: 2 });
                    return;
                }
                let key = o.key();
                if let Some(t) = self.active.as_mut().filter(|t| t.key == key) {
                    // The same file: a repeat, a resend of OPEN, or a new session of a transfer in progress.
                    let new_session = t.open.session != o.session;
                    if new_session && !t.complete {
                        t.open.session = o.session;
                        t.open.sender.clone_from(&o.sender);
                        t.last_ack = None;
                        ev.push(RxEvent::Opened {
                            session: o.session,
                            sender: o.sender.clone(),
                            name: o.name.clone(),
                            size: o.size,
                            held: t.have.bytes(),
                        });
                    }
                    let mut a = Self::accept(t, &call, esn0);
                    if let Message::Accept { session, .. } = &mut a {
                        *session = o.session;
                    }
                    replies.push(a);
                    return;
                }
                self.park();
                let t = match self.held.iter().position(|h| h.key == key) {
                    Some(i) => {
                        let mut t = self.held.remove(i);
                        t.open = o.clone();
                        t.last_ack = None;
                        t.stepper = Stepper::new(usize::from(o.profile).min(9));
                        t
                    }
                    None => Transfer::new(o),
                };
                ev.push(RxEvent::Opened {
                    session: o.session,
                    sender: o.sender.clone(),
                    name: o.name.clone(),
                    size: o.size,
                    held: t.have.bytes(),
                });
                replies.push(Self::accept(&t, &call, esn0));
                self.active = Some(t);
            }
            Message::Poll { session, burst } => {
                if let Some(t) = self.active.as_ref().filter(|t| t.open.session == *session) {
                    if let Some(Message::Ack(a)) = &t.last_ack {
                        if a.burst == *burst {
                            replies.push(Message::Ack(a.clone()));
                        }
                    }
                }
            }
            Message::Abort { session, .. } => {
                if self.active.as_ref().is_some_and(|t| t.open.session == *session) {
                    self.park();
                }
            }
            Message::Finish { session, size, hash_code, hash, .. } => {
                let Some(t) = self.active.as_mut().filter(|t| t.open.session == *session) else {
                    return;
                };
                if t.complete {
                    replies.push(Message::Verified { session: *session, good: true });
                    return;
                }
                if *size != t.open.size || !t.have.covers(0, t.open.size) {
                    let missing = t.open.size - t.have.bytes();
                    replies.push(Message::Reject { session: *session, reason: 0, ranges: t.have.for_message() });
                    ev.push(RxEvent::Missing { session: *session, bytes: missing });
                    return;
                }
                let got = hash_parts(*hash_code, t.parts());
                let good = got == Some(*hash) && (*hash_code != t.open.hash_code || got == Some(t.open.hash));
                if !good {
                    t.drop_bytes();
                    replies.push(Message::Reject { session: *session, reason: 1, ranges: Vec::new() });
                    ev.push(RxEvent::HashMismatch { session: *session });
                    return;
                }
                t.complete = true;
                let bytes = t.bytes();
                t.drop_bytes();
                t.have = Ranges::upto(t.open.size);
                let key = t.key;
                self.held.retain(|h| h.key != key);
                replies.push(Message::Verified { session: *session, good: true });
                ev.push(RxEvent::Delivered {
                    session: *session,
                    sender: t.open.sender.clone(),
                    name: t.open.name.clone(),
                    bytes,
                    hash: *hash,
                });
            }
            // START marks the transfer as started; the replies of the other side go to a sender.
            _ => {}
        }
    }

    /// Takes a data burst; appends the ACK.
    pub fn on_burst(&mut self, b: &RxBurst, replies: &mut Vec<Message>) {
        let Some(t) = self.active.as_mut().filter(|t| t.open.session == b.session) else {
            return;
        };
        let size = t.open.size;
        let mut blocks: Vec<Option<Vec<bool>>> = vec![None; MAX_FRAMES];
        let mut frames = 0usize;
        let (mut seen, mut nseen, mut failed) = (0usize, 0usize, 0usize);
        let mut es = 0.0;
        let mut profile = t.stepper.profile;
        for f in &b.frames {
            let i = usize::from(f.index).min(MAX_FRAMES - 1);
            frames = frames.max(i + 1);
            nseen += 1;
            seen += f.blocks.len();
            es += f.esn0_db;
            profile = usize::from(f.profile).min(9);
            let mut ok = Vec::with_capacity(f.blocks.len());
            for blk in &f.blocks {
                let good = blk.as_ref().and_then(|p| DataBlock::decode(p));
                match good {
                    Some(d) => {
                        if d.end() <= size && !t.complete {
                            t.write(u64::from(d.offset), &d.data);
                        }
                        ok.push(true);
                    }
                    None => {
                        failed += 1;
                        ok.push(false);
                    }
                }
            }
            blocks[i] = Some(ok);
        }
        blocks.truncate(frames);
        let lost = frames - nseen;
        let a = if nseen > 0 { (2 * seen + nseen) / (2 * nseen) } else { 0 };
        let loss = if nseen == 0 {
            1.0
        } else {
            let den = seen + lost * a;
            if den == 0 {
                0.0
            } else {
                (failed + lost * a) as f64 / den as f64
            }
        };
        let esn0 = if nseen > 0 { es / nseen as f64 } else { 0.0 };
        t.stepper.profile = profile;
        let req = t.stepper.after_burst(esn0, loss);
        let ack = Message::Ack(Ack {
            session: b.session,
            burst: b.number,
            frames: frames as u8,
            esn0_tenths: tenths(Some(esn0)),
            loss: (loss * 100.0).round() as u8,
            profile: req as u8,
            blocks,
        });
        t.last_ack = Some(ack.clone());
        replies.push(ack);
    }
}

/// One step of the one-way carousel.
#[derive(Clone, Debug, PartialEq)]
pub enum CarouselItem {
    /// A message in a control frame.
    Control(Message),
    /// A data burst.
    Burst(Burst),
    /// Silence, seconds.
    Silence(f64),
}

/// The default transmitter: OPEN, START, the data bursts in file order with OPEN and START again before burst 8, 16, 24 ...
/// of a pass, FINISH and 0.5 s of silence, for each pass. The session id stays; the burst number counts on over passes.
#[derive(Clone, Debug)]
pub struct Carousel {
    open: Message,
    payload: Vec<u8>,
    hash: [u8; 32],
    profile: usize,
    width: Width,
    passes: usize,
    pass: usize,
    in_pass: bool,
    bursts: usize,
    reopened: bool,
    todo: Ranges,
    queue: VecDeque<CarouselItem>,
    /// The next burst number.
    pub burst: u16,
}

impl Carousel {
    /// A carousel of `payload` named `name` from `call` in `session`, at `profile` and `width`, for `passes` passes, with
    /// burst numbers from `burst`.
    pub fn new(
        payload: Vec<u8>,
        name: &str,
        call: &str,
        session: u16,
        profile: usize,
        width: Width,
        passes: usize,
        burst: u16,
    ) -> Carousel {
        let hash = hash(HASH_BLAKE2S, &payload).expect("a known hash code");
        let profile = profile.min(9);
        let open = Message::Open(Open {
            session,
            sender: call.to_string(),
            receiver: "CQ".into(),
            width: width.code(),
            name: name.to_string(),
            size: payload.len() as u64,
            uncompressed: payload.len() as u64,
            hash_code: HASH_BLAKE2S,
            hash,
            compression: 0,
            profile: profile as u8,
            resume_hint: 0,
            data_per_block: PROFILES[profile].data() as u16,
        });
        Carousel {
            open,
            payload,
            hash,
            profile,
            width,
            passes: passes.max(1),
            pass: 0,
            in_pass: false,
            bursts: 0,
            reopened: false,
            todo: Ranges::default(),
            queue: VecDeque::new(),
            burst,
        }
    }

    /// The width of the carousel.
    pub fn width(&self) -> Width {
        self.width
    }

    /// The session id.
    pub fn session(&self) -> u16 {
        self.open.session()
    }

    fn opening(&mut self) -> Option<CarouselItem> {
        let s = self.open.session();
        self.queue.push_back(CarouselItem::Control(Message::Start { session: s, profile: self.profile as u8 }));
        Some(CarouselItem::Control(self.open.clone()))
    }

    /// The next item; `None` after the last pass.
    pub fn next_item(&mut self) -> Option<CarouselItem> {
        if let Some(i) = self.queue.pop_front() {
            return Some(i);
        }
        if self.pass >= self.passes {
            return None;
        }
        if !self.in_pass {
            self.in_pass = true;
            self.todo = Ranges::upto(self.payload.len() as u64);
            self.bursts = 0;
            self.reopened = false;
            return self.opening();
        }
        if !self.todo.is_empty() {
            if self.bursts > 0 && self.bursts.is_multiple_of(8) && !self.reopened {
                self.reopened = true;
                return self.opening();
            }
            self.reopened = false;
            let b = fill_burst(&mut self.todo, &self.payload, self.burst, self.profile, self.width)?;
            self.burst = (self.burst + 1) % 4096;
            self.bursts += 1;
            return Some(CarouselItem::Burst(b));
        }
        self.in_pass = false;
        self.pass += 1;
        self.queue.push_back(CarouselItem::Silence(0.5));
        Some(CarouselItem::Control(Message::Finish {
            session: self.open.session(),
            size: self.payload.len() as u64,
            hash_code: HASH_BLAKE2S,
            hash: self.hash,
            uncompressed_hash: self.hash,
        }))
    }
}

/// The name under which a received file is saved, from the name on the air: the text after the last "/" or "\", every
/// character other than an ASCII letter, digit, ".", "_", "-", "(", ")", "+" or space replaced by "_", leading and trailing
/// dots and spaces removed, at most 100 characters (an extension of up to 12 characters kept), "received.bin" when empty,
/// and "_" before a reserved device name.
pub fn safe_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let s: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || ".-_()+ ".contains(c) { c } else { '_' })
        .collect();
    let mut s = s.trim_matches(|c| c == '.' || c == ' ').to_string();
    if s.len() > 100 {
        s = match s.rfind('.') {
            Some(i) if s.len() - i - 1 <= 12 && i > 0 => {
                let ext = s[i..].to_string();
                format!("{}{}", &s[..100 - ext.len()], ext)
            }
            _ => s[..100].to_string(),
        };
    }
    if s.is_empty() {
        s = "received.bin".into();
    }
    let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    const RESERVED: [&str; 11] = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "LPT1", "LPT2", "LPT3"];
    if RESERVED.contains(&stem.as_str()) {
        s.insert(0, '_');
    }
    s
}

/// `name`, or `name` with " (n)" before its extension for the first `n` from 1 that `exists` says is free.
pub fn unique_name(name: &str, exists: impl Fn(&str) -> bool) -> String {
    if !exists(name) {
        return name.to_string();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (1..)
        .map(|n| format!("{stem} ({n}){ext}"))
        .find(|c| !exists(c))
        .expect("a free name")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|v| format!("{v:02x}")).collect()
    }

    fn hello() -> [u8; 32] {
        hash(HASH_BLAKE2S, b"hello").unwrap()
    }

    #[test]
    fn hash_vectors() {
        assert_eq!(hex(&hello()), "19213bacc58dee6dbde3ceb9a47cbb330b3d86f8cca8997eb00be456f140ca25");
        assert_eq!(hex(&hash(HASH_BLAKE2S, b"abc").unwrap()), "508c5e8c327c14e2e1a72ba34eeb452f37458b209ed63a294d999b4c86675982");
        assert_eq!(hex(&hash(HASH_BLAKE2S, b"").unwrap()), "69217a3079908094e11121d042354a7c1f55b6482ca1a51e1b250dfd1ed0eef9");
        assert_eq!(hex(&hash(HASH_SHA256, b"abc").unwrap()), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(hash(2, b"abc"), None);
        assert_eq!(crate::family::crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn message_vectors() {
        let open = Message::Open(Open {
            session: 0x1234,
            sender: "LY1ABC".into(),
            receiver: "CQ".into(),
            width: 2,
            name: "a.txt".into(),
            size: 5,
            uncompressed: 5,
            hash_code: 0,
            hash: hello(),
            compression: 0,
            profile: 2,
            resume_hint: 0,
            data_per_block: 470,
        });
        let cases: Vec<(Message, String)> = vec![
            (open, "013412064c59314142430243510205612e747874050000000000000005000000000000000019213bacc58dee6dbde3ceb9a47cbb330b3d86f8cca8997eb00be456f140ca25000200000000d601".into()),
            (
                Message::Accept { session: 0x1234, receiver: "DL2XYZ".into(), esn0_tenths: 231, profile: 4, ranges: vec![(0, 1000), (2000, 2500)] },
                "02341206444c3258595ae70004020000000000e8030000d0070000c4090000".into(),
            ),
            (Message::Start { session: 0x1234, profile: 2 }, "03341202".into()),
            (
                Message::Finish { session: 0x1234, size: 5, hash_code: 0, hash: hello(), uncompressed_hash: hello() },
                format!("043412050000000000000000{}{}", hex(&hello()), hex(&hello())),
            ),
            (Message::Verified { session: 0x1234, good: true }, "05341201".into()),
            (Message::Reject { session: 0x1234, reason: 0, ranges: vec![(0, 10)] }, "063412000100000000000a000000".into()),
            (Message::Abort { session: 0x1234, reason: 3 }, "07341203".into()),
            (Message::Poll { session: 0x1234, burst: 7 }, "0834120700".into()),
            (
                Message::Ack(Ack {
                    session: 0x1234,
                    burst: 3,
                    frames: 3,
                    esn0_tenths: -57,
                    loss: 12,
                    profile: 3,
                    blocks: vec![Some((0..9).map(|k| k != 2).collect()), None, Some(vec![true])],
                }),
                "093412030003c7ff0c03050009fb010101".into(),
            ),
        ];
        for (m, want) in cases {
            let b = m.encode();
            assert_eq!(hex(&b), want, "{m:?}");
            assert_eq!(Message::decode(&b), Some(m.clone()));
            let mut longer = b.clone();
            longer.extend_from_slice(&[1, 2, 3]);
            assert_eq!(Message::decode(&longer), Some(m.clone()), "extra bytes are ignored");
            assert_eq!(Message::decode(&b[..b.len() - 1]), None, "a cut message is invalid");
        }
        assert_eq!(Message::Finish { session: 1, size: 5, hash_code: 0, hash: hello(), uncompressed_hash: hello() }.encode().len(), 76);
        // More than 100 ranges is refused.
        let mut b = Message::Reject { session: 1, reason: 0, ranges: vec![] }.encode();
        b[4..6].copy_from_slice(&101u16.to_le_bytes());
        b.extend(std::iter::repeat_n(0, 808));
        assert_eq!(Message::decode(&b), None);
    }

    #[test]
    fn control_frame_example_and_largest_ack() {
        let m = Message::Start { session: 0x1234, profile: 2 };
        assert_eq!(m.encode(), vec![0x03, 0x34, 0x12, 0x02]);
        let f = control_frame(&m, Width::W200K, "LY1ABC", 0).unwrap();
        assert_eq!(f.header.blocks, 1);
        assert_eq!(f.blocks[0], vec![0x04, 0x00, 0x03, 0x34, 0x12, 0x02]);
        assert_eq!(join_message(&[Some(f.blocks[0].clone())]), Some(m));
        let big = Message::Ack(Ack {
            session: 1,
            burst: 2,
            frames: 16,
            esn0_tenths: 0,
            loss: 0,
            profile: 9,
            blocks: vec![Some(vec![true; 39]); 16],
        });
        let chunks = message_chunks(&big).expect("the largest ACK fits");
        assert!(chunks.len() <= 5);
        let back: Vec<Option<Vec<u8>>> = chunks.into_iter().map(Some).collect();
        assert_eq!(join_message(&back), Some(big));
        let mut bad = back.clone();
        bad[0] = None;
        assert_eq!(join_message(&bad), None);
    }

    #[test]
    fn data_block_round_trip() {
        let b = DataBlock { offset: 123_456, data: vec![9; 200] };
        let mut payload = b.encode();
        payload.resize(PROFILES[0].payload(), 0);
        assert_eq!(DataBlock::decode(&payload), Some(b));
        payload[4] = 231;
        assert_eq!(DataBlock::decode(&payload), None);
    }

    #[test]
    fn ranges_and_segmentation() {
        let mut r = Ranges::upto(1000);
        r.remove(100, 200);
        r.remove(500, 600);
        assert_eq!(r.list(), &[(0, 100), (200, 500), (600, 1000)]);
        r.add(100, 200);
        assert_eq!(r.list(), &[(0, 500), (600, 1000)]);
        assert!(r.covers(10, 500) && !r.covers(400, 700));
        let mut many = Ranges::default();
        for i in 0..150u64 {
            many.add(i * 100, i * 100 + 1 + i % 7);
        }
        let m = many.for_message();
        assert_eq!(m.len(), 100);
        assert!(m.windows(2).all(|w| w[0].0 < w[1].0));
        let payload: Vec<u8> = (0..2000u32).map(|i| i as u8).collect();
        let mut todo = Ranges::upto(2000);
        let b = fill_burst(&mut todo, &payload, 5, 0, Width::W200K).unwrap();
        // 200 kHz P0: one block a frame of 230 bytes.
        assert_eq!(b.frames.len(), 9);
        assert!(b.frames.iter().all(|f| f.len() == 1));
        assert_eq!(b.frames[8][0].data.len(), 2000 - 8 * 230);
        assert!(todo.is_empty());
        let fr = b.tx_frames(7, Width::W200K, "LY1ABC");
        assert_eq!(fr[3].header.frame, (5 << 4) | 3);
        assert!(fr[8].header.last && !fr[7].header.last);
    }

    #[test]
    fn carousel_order_over_two_passes() {
        // 200 kHz P0: one block of 230 bytes a frame, 16 frames a burst; 40 000 bytes are 11 bursts a pass.
        let payload = vec![7u8; 40_000];
        let mut c = Carousel::new(payload, "a.bin", "LY1ABC", 9, 0, Width::W200K, 2, 4090);
        let mut seq = Vec::new();
        while let Some(i) = c.next_item() {
            seq.push(match i {
                CarouselItem::Control(Message::Open(_)) => "O".to_string(),
                CarouselItem::Control(Message::Start { .. }) => "S".to_string(),
                CarouselItem::Control(Message::Finish { .. }) => "F".to_string(),
                CarouselItem::Burst(b) => format!("{}", b.number),
                CarouselItem::Silence(_) => "-".to_string(),
                CarouselItem::Control(_) => "?".to_string(),
            });
        }
        let pass1 = "O S 4090 4091 4092 4093 4094 4095 0 1 O S 2 3 4 F -";
        let pass2 = "O S 5 6 7 8 9 10 11 12 O S 13 14 15 F -";
        assert_eq!(seq.join(" "), format!("{pass1} {pass2}"));
    }

    #[test]
    fn stepper_rules() {
        let mut s = Stepper::new(4);
        assert_eq!(s.after_burst(40.0, 0.0), 4);
        assert_eq!(s.after_burst(40.0, 0.0), 4);
        assert_eq!(s.after_burst(40.0, 0.0), 5);
        assert_eq!(s.after_burst(40.0, 0.3), 4);
        for _ in 0..3 {
            assert_eq!(s.after_burst(40.0, 0.0), 4, "the hold keeps the profile");
        }
        assert_eq!(s.after_burst(40.0, 0.6), 2);
        let mut s = Stepper::new(6);
        assert_eq!(s.after_burst(20.0, 0.0), 5);
        assert_eq!(start_profile(Some(40.0), 2), 9);
        assert_eq!(start_profile(Some(12.0), 2), 4);
        assert_eq!(start_profile(Some(-10.0), 2), 0);
        assert_eq!(start_profile(None, 2), 2);
    }

    #[test]
    fn names_from_the_air() {
        assert_eq!(safe_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_name("C:\\dir\\file name.txt"), "file name.txt");
        assert_eq!(safe_name("a*b?c.txt"), "a_b_c.txt");
        assert_eq!(safe_name(" ..hidden. "), "hidden");
        assert_eq!(safe_name("..."), "received.bin");
        assert_eq!(safe_name("con.txt"), "_con.txt");
        assert_eq!(safe_name("COM3"), "_COM3");
        assert_eq!(safe_name("photo é.jpg"), "photo _.jpg");
        let long = format!("{}.extension12", "x".repeat(200));
        let s = safe_name(&long);
        assert_eq!(s.len(), 100);
        assert!(s.ends_with(".extension12"));
        assert_eq!(safe_name(&"y".repeat(150)).len(), 100);
        let taken = ["a.txt", "a (1).txt"];
        assert_eq!(unique_name("a.txt", |n| taken.contains(&n)), "a (2).txt");
        assert_eq!(unique_name("b", |n| n == "b"), "b (1)");
        assert_eq!(unique_name("c.txt", |_| false), "c.txt");
    }
}
