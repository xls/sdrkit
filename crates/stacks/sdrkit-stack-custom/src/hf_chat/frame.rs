//! The messages of `hf_chat` and their blocks: the callsign, text and locator codes, the recipient hash, the short and full
//! beacon, the chat, the data frame, the acknowledgement, the side channel of a voice block, the cutting of a message into
//! blocks of `K` bits with a CRC-16 each, and the assembly of the blocks heard. Fields are written most significant bit first.

use super::{Profile, MAX_BLOCKS};
use crate::family::{crc16, pack, push_bits, push_bytes, read_bits};
use sdrkit_util_bits::crc::catalog::CRC24_OPENPGP;

/// The callsign alphabet: index 0 is the space.
const CALL_ALPHABET: &[u8; 40] = b" ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-/.";
/// The text alphabet of 6-bit codes.
const TEXT_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 .,?!/-:@+'\"()%#=;*&$<>[]_|~";
/// The code of '?', sent for a character that is not in the text alphabet.
const UNKNOWN: u8 = 39;

/// The 48-bit value of a callsign of 1 to 9 characters of the callsign alphabet (lower case folded), not all spaces:
/// padded with spaces to 9, base 40, the first character most significant. `None` for another callsign.
pub fn callsign_value(call: &str) -> Option<u64> {
    let c = call.to_ascii_uppercase();
    if c.is_empty() || c.len() > 9 || c.bytes().all(|b| b == b' ') {
        return None;
    }
    let mut v = 0u64;
    for i in 0..9 {
        let b = c.as_bytes().get(i).copied().unwrap_or(b' ');
        v = v * 40 + CALL_ALPHABET.iter().position(|&a| a == b)? as u64;
    }
    Some(v)
}

/// The callsign of a 48-bit value, trailing spaces removed; `None` for a value of 40^9 or more.
pub fn callsign_text(mut v: u64) -> Option<String> {
    if v >= 40u64.pow(9) {
        return None;
    }
    let mut d = [0u8; 9];
    for o in d.iter_mut().rev() {
        *o = CALL_ALPHABET[(v % 40) as usize];
        v /= 40;
    }
    Some(String::from_utf8_lossy(&d).trim_end().to_string())
}

/// The 6-bit code of a character (lower case folded); '?' for one outside the alphabet.
pub fn text_code(c: char) -> u8 {
    let c = c.to_ascii_uppercase();
    u8::try_from(c).ok().and_then(|b| TEXT_ALPHABET.iter().position(|&a| a == b)).map_or(UNKNOWN, |i| i as u8)
}

/// The character of a 6-bit code.
pub fn text_char(code: u8) -> char {
    char::from(TEXT_ALPHABET[usize::from(code & 63)])
}

/// The 25-bit value of a 6-character locator AA00AA (lower case folded); `None` for another text.
pub fn locator_value(loc: &str) -> Option<u32> {
    let b = loc.to_ascii_uppercase().into_bytes();
    if b.len() != 6 {
        return None;
    }
    let letter = |c: u8, top: u8| (b'A'..b'A' + top).contains(&c).then(|| u32::from(c - b'A'));
    let digit = |c: u8| c.is_ascii_digit().then(|| u32::from(c - b'0'));
    let (a, bb, c, d, e, f) = (letter(b[0], 18)?, letter(b[1], 18)?, digit(b[2])?, digit(b[3])?, letter(b[4], 24)?, letter(b[5], 24)?);
    Some(((((a * 18 + bb) * 10 + c) * 10 + d) * 24 + e) * 24 + f)
}

/// The locator of a 25-bit value; `None` for a value of 18662400 or more.
pub fn locator_text(v: u32) -> Option<String> {
    if v >= 18_662_400 {
        return None;
    }
    let (f, v) = (v % 24, v / 24);
    let (e, v) = (v % 24, v / 24);
    let (d, v) = (v % 10, v / 10);
    let (c, v) = (v % 10, v / 10);
    let (b, a) = (v % 18, v / 18);
    let l = |x: u32| char::from(b'A' + x as u8);
    let n = |x: u32| char::from(b'0' + x as u8);
    Some([l(a), l(b), n(c), n(d), l(e), l(f)].iter().collect())
}

/// The 22-bit recipient hash of a callsign: FNV-1a 32 over its 9 bytes (upper case, padded with spaces), then
/// `(h ^ (h >> 22)) & 0x3FFFFF`.
pub fn recipient_hash(call: &str) -> u32 {
    let c = call.to_ascii_uppercase();
    let mut h = 2_166_136_261u32;
    for i in 0..9 {
        h ^= u32::from(c.as_bytes().get(i).copied().unwrap_or(b' '));
        h = h.wrapping_mul(16_777_619);
    }
    (h ^ (h >> 22)) & 0x3F_FFFF
}

/// The CRC-16 (CCITT-FALSE) of a bit string: the bits packed into bytes most significant first, the last byte filled with
/// zero bits.
pub fn crc16_of_bits(bits: &[u8]) -> u16 {
    crc16(&pack(bits))
}

/// The two-bit version of every structured frame.
const VERSION: u64 = 1;

/// A short beacon (frame type 0, 104 bits).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortBeacon {
    /// The callsign.
    pub callsign: String,
    /// The locator, 6 characters.
    pub locator: String,
    /// The power step: EIRP dBm = -40 + 0.5 x step, 0 to 63.
    pub power_step: u8,
    /// The fine latitude inside the locator square, 0 to 63 (no scale defined).
    pub fine_lat: u8,
    /// The fine longitude inside the locator square, 0 to 63 (no scale defined).
    pub fine_lon: u8,
    /// The link quality, 0 to 255 (no meaning defined).
    pub link: u8,
}

/// A full beacon (frame type 2, 147 + 6 n bits).
#[derive(Clone, Debug, PartialEq)]
pub struct FullBeacon {
    /// The callsign.
    pub callsign: String,
    /// The latitude, degrees.
    pub lat: f64,
    /// The longitude, degrees.
    pub lon: f64,
    /// The power step: EIRP dBm = -40 + 0.5 x step.
    pub power_step: u8,
    /// The link quality.
    pub link: u8,
    /// The locator.
    pub locator: String,
    /// Up to 10 characters of text.
    pub text: String,
}

/// The addressee of a chat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recipient {
    /// A full callsign.
    Call(String),
    /// The 22-bit hash of a callsign.
    Hash(u32),
}

impl std::fmt::Display for Recipient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Recipient::Call(c) => f.write_str(c),
            Recipient::Hash(h) => write!(f, "#{h:06X}"),
        }
    }
}

/// A chat (frame type 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chat {
    /// The sender.
    pub from: String,
    /// The addressee.
    pub to: Recipient,
    /// The text goes as UTF-8 bytes, not as 6-bit codes.
    pub raw: bool,
    /// The more-blocks flag (carried, no function).
    pub more: bool,
    /// The sequence number (carried, no function).
    pub seq: u8,
    /// The acknowledgement number (carried, no function).
    pub ack: u8,
    /// The text: at most 100 characters (6-bit codes) or 100 bytes (raw).
    pub text: String,
}

/// One message of `hf_chat`.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    /// A short beacon.
    ShortBeacon(ShortBeacon),
    /// A chat.
    Chat(Chat),
    /// A full beacon.
    FullBeacon(FullBeacon),
    /// Bytes, with a CRC-24.
    Data(Vec<u8>),
}

/// Why a message cannot be sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameError {
    /// A callsign is not 1 to 9 characters of A-Z, 0-9, space, '-', '/' and '.'.
    Callsign,
    /// A locator is not of the form AA00AA.
    Locator,
    /// The data has more than 65535 bytes.
    Length,
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FrameError::Callsign => "a callsign must be 1 to 9 characters of A-Z, 0-9, space, '-', '/' and '.'",
            FrameError::Locator => "the locator must be 6 characters of the form AA00AA",
            FrameError::Length => "the data must be at most 65535 bytes",
        })
    }
}

impl std::error::Error for FrameError {}

fn push_call(out: &mut Vec<u8>, call: &str) -> Result<(), FrameError> {
    push_bits(out, callsign_value(call).ok_or(FrameError::Callsign)?, 48);
    Ok(())
}

fn push_locator(out: &mut Vec<u8>, loc: &str) -> Result<(), FrameError> {
    push_bits(out, u64::from(locator_value(loc).ok_or(FrameError::Locator)?), 25);
    Ok(())
}

/// `v` rounded half away from zero and clamped to the two's complement range of `width` bits, as a field of that width.
fn signed_field(v: f64, width: usize) -> u64 {
    let half = 1i64 << (width - 1);
    let r = (v.abs() + 0.5).floor().copysign(v) as i64;
    (r.clamp(-half, half - 1) as u64) & ((1u64 << width) - 1)
}

fn signed_of(v: u64, width: usize) -> i64 {
    let half = 1i64 << (width - 1);
    let v = v as i64;
    if v >= half { v - 2 * half } else { v }
}

impl Message {
    /// The bit string of the message.
    pub fn bits(&self) -> Result<Vec<u8>, FrameError> {
        let mut b = Vec::new();
        push_bits(&mut b, VERSION, 2);
        match self {
            Message::ShortBeacon(s) => {
                push_bits(&mut b, 0, 3);
                push_call(&mut b, &s.callsign)?;
                push_locator(&mut b, &s.locator)?;
                push_bits(&mut b, u64::from(s.power_step.min(63)), 6);
                push_bits(&mut b, u64::from(s.fine_lat.min(63)), 6);
                push_bits(&mut b, u64::from(s.fine_lon.min(63)), 6);
                push_bits(&mut b, u64::from(s.link), 8);
            }
            Message::Chat(c) => {
                push_bits(&mut b, 1, 3);
                let full = matches!(c.to, Recipient::Call(_));
                push_bits(&mut b, u64::from(c.raw), 1);
                push_bits(&mut b, u64::from(full), 1);
                push_bits(&mut b, u64::from(c.more), 1);
                push_call(&mut b, &c.from)?;
                match &c.to {
                    Recipient::Call(to) => push_call(&mut b, to)?,
                    Recipient::Hash(h) => push_bits(&mut b, u64::from(*h & 0x3F_FFFF), 22),
                }
                push_bits(&mut b, u64::from(c.seq), 8);
                push_bits(&mut b, u64::from(c.ack), 8);
                if c.raw {
                    let bytes: Vec<u8> = c.text.bytes().take(100).collect();
                    push_bits(&mut b, bytes.len() as u64, 7);
                    push_bytes(&mut b, &bytes);
                } else {
                    let codes: Vec<u8> = c.text.chars().take(100).map(text_code).collect();
                    push_bits(&mut b, codes.len() as u64, 7);
                    for code in codes {
                        push_bits(&mut b, u64::from(code), 6);
                    }
                }
            }
            Message::FullBeacon(f) => {
                push_bits(&mut b, 2, 3);
                push_call(&mut b, &f.callsign)?;
                push_bits(&mut b, signed_field(f.lat.clamp(-90.0, 90.0) * f64::from(1u32 << 23) / 90.0, 24), 24);
                push_bits(&mut b, signed_field(f.lon.clamp(-180.0, 180.0) * f64::from(1u32 << 24) / 180.0, 25), 25);
                push_bits(&mut b, u64::from(f.power_step), 8);
                push_bits(&mut b, u64::from(f.link), 8);
                push_locator(&mut b, &f.locator)?;
                let codes: Vec<u8> = f.text.chars().take(10).map(text_code).collect();
                push_bits(&mut b, codes.len() as u64, 4);
                for code in codes {
                    push_bits(&mut b, u64::from(code), 6);
                }
            }
            Message::Data(d) => {
                push_bits(&mut b, 3, 3);
                if d.len() > 65_535 {
                    return Err(FrameError::Length);
                }
                push_bits(&mut b, d.len() as u64, 16);
                push_bytes(&mut b, d);
                push_bits(&mut b, u64::from(CRC24_OPENPGP.checksum(d)), 24);
            }
        }
        Ok(b)
    }

    /// The message of a bit string of exactly its length; `None` for a frame of another version or type, a field out of
    /// range, a length that does not agree, or a data frame whose CRC-24 fails.
    pub fn parse(bits: &[u8]) -> Option<Message> {
        if message_len(bits)? != bits.len() {
            return None;
        }
        let r = |at: usize, w: usize| read_bits(bits, at, w);
        let call = |at: usize| callsign_text(r(at, 48));
        Some(match r(2, 3) {
            0 => Message::ShortBeacon(ShortBeacon {
                callsign: call(5)?,
                locator: locator_text(r(53, 25) as u32)?,
                power_step: r(78, 6) as u8,
                fine_lat: r(84, 6) as u8,
                fine_lon: r(90, 6) as u8,
                link: r(96, 8) as u8,
            }),
            1 => {
                let (raw, full, more) = (r(5, 1) == 1, r(6, 1) == 1, r(7, 1) == 1);
                let from = call(8)?;
                let (to, at) = if full { (Recipient::Call(call(56)?), 104) } else { (Recipient::Hash(r(56, 22) as u32), 78) };
                let n = r(at + 16, 7) as usize;
                let at = at + 23;
                let text = if raw {
                    let bytes: Vec<u8> = (0..n).map(|i| r(at + 8 * i, 8) as u8).collect();
                    String::from_utf8_lossy(&bytes).into_owned()
                } else {
                    (0..n).map(|i| text_char(r(at + 6 * i, 6) as u8)).collect()
                };
                let seq_at = if full { 104 } else { 78 };
                Message::Chat(Chat { from, to, raw, more, seq: r(seq_at, 8) as u8, ack: r(seq_at + 8, 8) as u8, text })
            }
            2 => {
                let n = r(143, 4) as usize;
                Message::FullBeacon(FullBeacon {
                    callsign: call(5)?,
                    lat: signed_of(r(53, 24), 24) as f64 * 90.0 / f64::from(1u32 << 23),
                    lon: signed_of(r(77, 25), 25) as f64 * 180.0 / f64::from(1u32 << 24),
                    power_step: r(102, 8) as u8,
                    link: r(110, 8) as u8,
                    locator: locator_text(r(118, 25) as u32)?,
                    text: (0..n).map(|i| text_char(r(147 + 6 * i, 6) as u8)).collect(),
                })
            }
            3 => {
                let l = r(5, 16) as usize;
                let d: Vec<u8> = (0..l).map(|i| r(21 + 8 * i, 8) as u8).collect();
                if r(21 + 8 * l, 24) as u32 != CRC24_OPENPGP.checksum(&d) {
                    return None;
                }
                Message::Data(d)
            }
            _ => return None,
        })
    }
}

/// The length of the message that begins with `bits`, read from its header; `None` when `bits` is too short to tell, the
/// version is not 1, the type has no length, or a count is out of range (a full beacon text over 10, a chat over 100).
pub fn message_len(bits: &[u8]) -> Option<usize> {
    let r = |at: usize, w: usize| (bits.len() >= at + w).then(|| read_bits(bits, at, w));
    if r(0, 2)? != VERSION {
        return None;
    }
    match r(2, 3)? {
        0 => Some(104),
        1 => {
            let (raw, full) = (r(5, 1)? == 1, r(6, 1)? == 1);
            let h = if full { 127 } else { 101 };
            let n = r(h - 7, 7)? as usize;
            (n <= 100).then_some(h + n * if raw { 8 } else { 6 })
        }
        2 => {
            let n = r(143, 4)? as usize;
            (n <= 10).then_some(147 + 6 * n)
        }
        3 => Some(5 + 16 + 8 * r(5, 16)? as usize + 24),
        _ => None,
    }
}

/// An acknowledgement (110 bits, zero filled to `K`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ack {
    /// The acknowledging station.
    pub callsign: String,
    /// The message id.
    pub id: u8,
    /// Bit `i` set: block `i` arrived with a good CRC.
    pub bitmap: u16,
    /// The SNR of the burst, dB in 2.4 kHz, -35 to +28.
    pub snr_db: i32,
    /// The suggested profile.
    pub profile: u8,
}

/// The CRC-16 of a block or an acknowledgement covers the bits before its last 16 (or before bit 94 of an acknowledgement).
const ACK_BITS: usize = 110;

impl Ack {
    /// The `k` bits of the acknowledgement.
    pub fn bits(&self, k: usize) -> Result<Vec<u8>, FrameError> {
        let mut b = Vec::with_capacity(k);
        push_call(&mut b, &self.callsign)?;
        push_bits(&mut b, u64::from(self.id & 15), 4);
        push_bits(&mut b, u64::from(self.bitmap), 32);
        push_bits(&mut b, (self.snr_db.clamp(-35, 28) + 35) as u64, 6);
        push_bits(&mut b, u64::from(self.profile & 15), 4);
        let crc = crc16_of_bits(&b);
        push_bits(&mut b, u64::from(crc), 16);
        b.resize(k.max(ACK_BITS), 0);
        Ok(b)
    }

    /// The acknowledgement in the `k` bits of a block: the CRC holds, every fill bit is 0, bits 16 to 31 of the bitmap are 0.
    pub fn parse(bits: &[u8]) -> Option<Ack> {
        if bits.len() < ACK_BITS
            || read_bits(bits, 94, 16) as u16 != crc16_of_bits(&bits[..94])
            || bits[ACK_BITS..].iter().any(|&b| b != 0)
            || read_bits(bits, 52, 16) != 0
        {
            return None;
        }
        Some(Ack {
            callsign: callsign_text(read_bits(bits, 0, 48))?,
            id: read_bits(bits, 48, 4) as u8,
            bitmap: read_bits(bits, 68, 16) as u16,
            snr_db: read_bits(bits, 84, 6) as i32 - 35,
            profile: read_bits(bits, 90, 4) as u8,
        })
    }
}

/// What the `K` bits of a decoded block hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    /// A slice of a message.
    Part {
        /// The message id.
        id: u8,
        /// The index of the block in the message.
        index: u8,
        /// The `K - 24` payload bits.
        payload: Vec<u8>,
    },
    /// An acknowledgement.
    Ack(Ack),
}

/// The `K` bits of block `index` of message `id`: the ids, the payload (zero filled to `K - 24`) and the CRC-16.
pub fn block_bits(id: u8, index: u8, payload: &[u8], k: usize) -> Vec<u8> {
    let mut b = Vec::with_capacity(k);
    push_bits(&mut b, u64::from(id & 15), 4);
    push_bits(&mut b, u64::from(index & 15), 4);
    b.extend_from_slice(&payload[..payload.len().min(k - 24)]);
    b.resize(k - 16, 0);
    let crc = crc16_of_bits(&b);
    push_bits(&mut b, u64::from(crc), 16);
    b
}

/// The block in `bits` (`K` bits): a message part when the block CRC holds, else an acknowledgement when its CRC and fill
/// hold, else `None`.
pub fn parse_block(bits: &[u8]) -> Option<Block> {
    let k = bits.len();
    if k < ACK_BITS {
        return None;
    }
    if read_bits(bits, k - 16, 16) as u16 == crc16_of_bits(&bits[..k - 16]) {
        return Some(Block::Part {
            id: read_bits(bits, 0, 4) as u8,
            index: read_bits(bits, 4, 4) as u8,
            payload: bits[8..k - 16].to_vec(),
        });
    }
    Ack::parse(bits).map(Block::Ack)
}

/// The blocks of a message of `bits` in profile `p` as message `id`: `max(1, ceil(len / usable))` blocks of `K` bits.
/// `None` when it needs more than 16.
pub fn blocks(bits: &[u8], id: u8, p: &Profile) -> Option<Vec<Vec<u8>>> {
    let u = p.usable();
    let n = bits.len().div_ceil(u).max(1);
    (n <= MAX_BLOCKS).then(|| (0..n).map(|b| block_bits(id, b as u8, &bits[(b * u).min(bits.len())..((b + 1) * u).min(bits.len())], p.k)).collect())
}

/// The side channel of a voice block (180 bits, 12 with each of the 15 codec frames).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceSide {
    /// The callsign.
    pub callsign: String,
    /// The call number, 0 to 15.
    pub call: u8,
    /// The 108 slow-data bits (no format defined).
    pub slow: Vec<u8>,
}

/// Frames of a voice block.
pub const VOICE_FRAMES: usize = 15;
/// Side-channel bits with each frame.
pub const SIDE_SLICE: usize = 12;

impl VoiceSide {
    /// The 180 bits.
    pub fn bits(&self) -> Result<Vec<u8>, FrameError> {
        let mut b = Vec::with_capacity(180);
        push_call(&mut b, &self.callsign)?;
        push_bits(&mut b, VERSION, 2);
        push_bits(&mut b, 2, 2);
        push_bits(&mut b, u64::from(self.call & 15), 4);
        let mut slow = self.slow.clone();
        slow.resize(108, 0);
        b.extend_from_slice(&slow);
        let crc = crc16_of_bits(&b);
        push_bits(&mut b, u64::from(crc), 16);
        Ok(b)
    }

    /// The side channel in 180 bits, when its CRC holds and its version and type are 1 and 2.
    pub fn parse(bits: &[u8]) -> Option<VoiceSide> {
        if bits.len() != 180 || read_bits(bits, 164, 16) as u16 != crc16_of_bits(&bits[..164]) || read_bits(bits, 48, 4) != 0b0110 {
            return None;
        }
        Some(VoiceSide { callsign: callsign_text(read_bits(bits, 0, 48))?, call: read_bits(bits, 52, 4) as u8, slow: bits[56..164].to_vec() })
    }
}

/// The `K` bits of a voice block of profile `p`: 15 frames, each the codec bits of 40 ms (`codec[i]`, 28 or 48 bits) then
/// slice `i` of the side channel.
pub fn voice_block(p: &Profile, codec: &[Vec<u8>], side: &VoiceSide) -> Result<Vec<u8>, FrameError> {
    let per = p.k / VOICE_FRAMES - SIDE_SLICE;
    let s = side.bits()?;
    let mut b = Vec::with_capacity(p.k);
    for i in 0..VOICE_FRAMES {
        let mut c = codec.get(i).cloned().unwrap_or_default();
        c.resize(per, 0);
        b.extend_from_slice(&c);
        b.extend_from_slice(&s[SIDE_SLICE * i..SIDE_SLICE * (i + 1)]);
    }
    Ok(b)
}

/// The codec bits of each frame and the side channel bits of a voice block of profile `p`.
pub fn split_voice(p: &Profile, bits: &[u8]) -> (Vec<Vec<u8>>, Vec<u8>) {
    let per = p.k / VOICE_FRAMES - SIDE_SLICE;
    let (mut codec, mut side) = (Vec::new(), Vec::new());
    for f in bits.chunks(per + SIDE_SLICE).take(VOICE_FRAMES) {
        codec.push(f[..per].to_vec());
        side.extend_from_slice(&f[per..]);
    }
    (codec, side)
}

/// The FNV-1a 64 of a bit string, one byte a bit: the repeat test of the assembler.
fn fnv64(bits: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bits {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Seconds a delivered message blocks its repeats.
pub const REPEAT_S: f64 = 1200.0;
/// Seconds a partial message is kept.
pub const PARTIAL_S: f64 = 900.0;

/// One message under assembly.
#[derive(Clone, Debug)]
struct Partial {
    k: usize,
    parts: Vec<Option<Vec<u8>>>,
    last: f64,
}

/// What [`Assembler::take`] did with a block.
#[derive(Clone, Debug, PartialEq)]
pub enum Taken {
    /// The block was kept; the message is not complete.
    Kept,
    /// The message is complete: its bits and the message they hold (`None` when they hold no known frame).
    Complete(Vec<u8>, Option<Message>),
    /// The message is complete but was delivered within [`REPEAT_S`].
    Repeat,
}

/// Joins the blocks of each message id (section 11.5): blocks of another `K` replace those held, a message is complete when
/// blocks 0, 1, ... in a row hold the length its header gives, a message delivered in the last 1200 s is a repeat, and a
/// partial message is dropped after 900 s.
#[derive(Clone, Debug, Default)]
pub struct Assembler {
    partial: [Option<Partial>; 16],
    delivered: Vec<(u64, f64)>,
}

impl Assembler {
    /// Takes block `index` of message `id` with its payload of `k - 24` bits at time `now` (s).
    pub fn take(&mut self, id: u8, index: u8, payload: &[u8], k: usize, now: f64) -> Taken {
        for p in self.partial.iter_mut() {
            if p.as_ref().is_some_and(|p| now - p.last > PARTIAL_S) {
                *p = None;
            }
        }
        self.delivered.retain(|d| now - d.1 <= REPEAT_S);
        let slot = &mut self.partial[usize::from(id & 15)];
        if slot.as_ref().is_none_or(|p| p.k != k) {
            *slot = Some(Partial { k, parts: vec![None; MAX_BLOCKS], last: now });
        }
        let Some(p) = slot.as_mut() else {
            return Taken::Kept;
        };
        p.last = now;
        p.parts[usize::from(index & 15)] = Some(payload.to_vec());
        let mut joined = Vec::new();
        for part in &p.parts {
            let Some(b) = part else {
                break;
            };
            joined.extend_from_slice(b);
            if let Some(len) = message_len(&joined) {
                if joined.len() >= len {
                    joined.truncate(len);
                    *slot = None;
                    let h = fnv64(&joined);
                    if self.delivered.iter().any(|d| d.0 == h) {
                        return Taken::Repeat;
                    }
                    self.delivered.push((h, now));
                    let m = Message::parse(&joined);
                    return Taken::Complete(joined, m);
                }
            } else if joined.len() >= 147 {
                // The header is there and gives no length: no message will come of it.
                *slot = None;
                return Taken::Kept;
            }
        }
        Taken::Kept
    }

    /// Forgets every message.
    pub fn clear(&mut self) {
        *self = Assembler::default();
    }
}

/// The messages of the worked examples, for the tests of the stack.
#[cfg(test)]
pub(crate) mod tests_support {
    use super::{Chat, Message, Recipient};

    /// The chat of section 9.10: LY1ABC to CQ, full recipient, sequence 0, acknowledgement 0.
    pub(crate) fn chat() -> Message {
        Message::Chat(Chat {
            from: "LY1ABC".into(),
            to: Recipient::Call("CQ".into()),
            raw: false,
            more: false,
            seq: 0,
            ack: 0,
            text: "CQ DE LY1ABC KO24".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hf_chat::PROFILES;

    fn hex(bits: &[u8]) -> String {
        bits.chunks(4).map(|c| format!("{:x}", c.iter().fold(0u8, |a, &b| (a << 1) | b) << (4 - c.len()))).collect()
    }

    use super::tests_support::chat;

    #[test]
    fn codes_and_worked_examples() {
        assert_eq!(callsign_value("LY1ABC"), Some(82_853_995_712_000));
        assert_eq!(callsign_value("cq"), Some(22_446_080_000_000));
        assert_eq!(callsign_text(82_853_995_712_000).as_deref(), Some("LY1ABC"));
        assert_eq!(callsign_value("   "), None);
        assert_eq!(callsign_value("ABCDEFGHIJ"), None);
        let codes: Vec<u8> = "HELLO 73".chars().map(text_code).collect();
        assert_eq!(codes, [7, 4, 11, 11, 14, 36, 33, 29]);
        assert_eq!(text_code('{'), 39);
        for c in 0..64u8 {
            assert_eq!(text_code(text_char(c)), c);
        }
        assert_eq!(locator_value("KO24RM"), Some(11_188_644));
        assert_eq!(locator_value("AA00AA"), Some(0));
        assert_eq!(locator_value("rr99xx"), Some(18_662_399));
        assert_eq!(locator_text(11_188_644).as_deref(), Some("KO24RM"));
        assert_eq!(recipient_hash("DL2XYZ"), 0x3e_411b);

        let c = chat().bits().unwrap();
        assert_eq!(c.len(), 229);
        assert_eq!(hex(&c), "4a4b5af2fb8e00146a22a000000000221212062485b0d80085214738f0");
        assert_eq!(Message::parse(&c), Some(chat()));
        let sb = Message::ShortBeacon(ShortBeacon {
            callsign: "LY1ABC".into(),
            locator: "KO24RM".into(),
            power_step: 63,
            fine_lat: 17,
            fine_lon: 60,
            link: 0,
        });
        let b = sb.bits().unwrap();
        assert_eq!(hex(&b), "425ad797dc7002aae693f47c00");
        assert_eq!(Message::parse(&b), Some(sb));
        let fb = Message::FullBeacon(FullBeacon {
            callsign: "LY1ABC".into(),
            lat: 54.6872,
            lon: 25.2797,
            power_step: 140,
            link: 7,
            locator: "KO24RM".into(),
            text: "HELLO 73".into(),
        });
        let b = fb.bits().unwrap();
        assert_eq!(b.len(), 195);
        assert_eq!(hex(&b), "525ad797dc70026e38008fd03e301d55734903885967490ba");
        let Some(Message::FullBeacon(back)) = Message::parse(&b) else { panic!("full beacon") };
        assert!((back.lat - 54.6872).abs() < 1e-3 && (back.lon - 25.2797).abs() < 1e-3);
        let ack = Ack { callsign: "DL2XYZ".into(), id: 9, bitmap: 0xA5C3, snr_db: -22, profile: 3 };
        let b = ack.bits(128).unwrap();
        assert_eq!(hex(&b), "19bd85f9f40090000a5c334c40980000");
        assert_eq!(parse_block(&b), Some(Block::Ack(ack)));
        let d = Message::Data(vec![1, 2, 3, 0xFF]);
        let b = d.bits().unwrap();
        assert_eq!(b.len(), 77);
        assert_eq!(hex(&b), "58002008101ff9b0d8a0");
        assert_eq!(Message::parse(&b), Some(d));
        let side = VoiceSide { callsign: "LY1ABC".into(), call: 5, slow: vec![0; 108] };
        let b = side.bits().unwrap();
        assert_eq!(hex(&b), "4b5af2fb8e00650000000000000000000000000005ffc");
        assert_eq!(VoiceSide::parse(&b), Some(side));
    }

    #[test]
    fn blocks_crc_and_assembly() {
        let p3 = &PROFILES[3];
        let c = chat().bits().unwrap();
        let b = blocks(&c, 1, p3).unwrap();
        assert_eq!(b.len(), 1);
        assert_eq!(read_bits(&b[0], 512 - 16, 16), 0x2316);
        // The block CRC catches a flip at 0, 7, K/2 and K - 1.
        for at in [0, 7, 256, 511] {
            let mut x = b[0].clone();
            x[at] ^= 1;
            assert!(!matches!(parse_block(&x), Some(Block::Part { .. })), "{at}");
        }
        // A chat needs two blocks of profile 0.
        let p0 = &PROFILES[0];
        let b0 = blocks(&c, 4, p0).unwrap();
        assert_eq!(b0.len(), 3);
        let mut a = Assembler::default();
        for (i, blk) in b0.iter().enumerate() {
            let Some(Block::Part { id, index, payload }) = parse_block(blk) else { panic!("part") };
            let t = a.take(id, index, &payload, p0.k, 10.0);
            if i + 1 < b0.len() {
                assert_eq!(t, Taken::Kept);
            } else {
                assert_eq!(t, Taken::Complete(c.clone(), Some(chat())));
            }
        }
        // The same message again is a repeat.
        for blk in &b0 {
            let Some(Block::Part { id, index, payload }) = parse_block(blk) else { panic!("part") };
            let t = a.take(id, index, &payload, p0.k, 20.0);
            assert!(matches!(t, Taken::Kept | Taken::Repeat));
        }
        // More than 16 blocks is refused.
        assert!(blocks(&vec![0; 17 * p0.usable()], 0, p0).is_none());
        // The SNR of an acknowledgement clamps.
        let hi = Ack { callsign: "A1".into(), id: 0, bitmap: 1, snr_db: 40, profile: 5 };
        let Some(Block::Ack(h)) = parse_block(&hi.bits(128).unwrap()) else { panic!("ack") };
        assert_eq!(h.snr_db, 28);
        let lo = Ack { snr_db: -50, ..hi };
        let Some(Block::Ack(l)) = parse_block(&lo.bits(128).unwrap()) else { panic!("ack") };
        assert_eq!(l.snr_db, -35);
    }
}
