//! Multi-block messages (ARINC 618 clause 2.3.6): a long message is sent as several blocks. Each block but the last ends in ETB.
//!
//! [`Assembler`] joins the blocks of one message on [`sdrkit_util_objects::Joiner`]. Blocks belong together when they have the
//! same registration, direction, label and extra key, and in a downlink the same message sequence number. The caller supplies the
//! extra key (VDL Mode 2 passes the 24-bit source address).
//!
//! - A repeated block (same block sequence letter in a downlink, same block id in an uplink) is dropped.
//! - A gap in the sequence letters marks the message incomplete.
//! - A message whose text grows over [`MAX_JOINED`] characters stops growing and is marked cut and incomplete.
//! - At most [`MAX_OPEN`] messages are open. A new message closes the oldest as incomplete.
//! - A message with no new block for [`TIMEOUT_S`] seconds of signal time is closed as incomplete ([`Assembler::expire`]).

use super::{Block, End};
use sdrkit_util_objects::{JoinRules, Joiner, Numbering, Part};

/// The longest joined text kept, in characters.
pub const MAX_JOINED: usize = 8192;
/// The most messages open at one time.
pub const MAX_OPEN: usize = 64;
/// Seconds of signal time after which an open message leaves incomplete.
pub const TIMEOUT_S: f64 = 45.0;

/// How a block was received.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quality {
    /// Bits the repair changed.
    pub bits_fixed: u32,
    /// The BCS (or the link's own frame check sequence) was good.
    pub bcs_ok: bool,
}

impl Quality {
    /// A block received without errors.
    pub const GOOD: Quality = Quality {
        bits_fixed: 0,
        bcs_ok: true,
    };
}

/// The fields of a message that come from its first block (fixed size, so they are copied without allocation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head {
    /// The mode character.
    pub mode: u8,
    address: [u8; 7],
    /// The acknowledgement character.
    pub tak: u8,
    /// The label (`_d` form).
    pub label: [u8; 2],
    /// The block id of the first block.
    pub block_id: u8,
    msn: [u8; 3],
    /// The message sequence character after the message number (a downlink), a space otherwise.
    pub sequence: u8,
    flight: [u8; 6],
    flight_len: u8,
    /// The caller's extra key.
    pub key_extra: u32,
}

impl Head {
    pub(crate) fn of(b: &Block, key_extra: u32) -> Head {
        let mut address = [b'.'; 7];
        let a = b.address.as_bytes();
        address[7 - a.len().min(7)..].copy_from_slice(&a[a.len().saturating_sub(7)..]);
        let (mut msn, mut flight, mut flight_len) = ([b' '; 3], [b' '; 6], 0);
        let mut sequence = b' ';
        if let Some(p) = b.downlink_prefix() {
            msn.copy_from_slice(p.msn.as_bytes());
            sequence = p.sequence;
            flight[..p.flight.len()].copy_from_slice(p.flight.as_bytes());
            flight_len = p.flight.len() as u8;
        }
        Head {
            mode: b.mode,
            address,
            tak: b.tak,
            label: b.label,
            block_id: b.block_id,
            msn,
            sequence,
            flight,
            flight_len,
            key_extra,
        }
    }

    /// The registration (the padding dots removed).
    pub fn registration(&self) -> &str {
        std::str::from_utf8(&self.address)
            .unwrap_or("")
            .trim_start_matches('.')
            .trim()
    }

    /// The address as sent (7 characters).
    pub fn address(&self) -> &str {
        std::str::from_utf8(&self.address).unwrap_or("")
    }

    /// A digit block id is a downlink.
    pub fn downlink(&self) -> bool {
        self.block_id.is_ascii_digit()
    }

    /// The label as text.
    pub fn label_str(&self) -> &str {
        std::str::from_utf8(&self.label).unwrap_or("??")
    }

    /// The downlink message sequence number (`M01`), empty for an uplink.
    pub fn msn(&self) -> &str {
        std::str::from_utf8(&self.msn).unwrap_or("").trim()
    }

    /// The downlink flight id, empty for an uplink.
    pub fn flight(&self) -> &str {
        std::str::from_utf8(&self.flight[..self.flight_len as usize]).unwrap_or("")
    }
}

/// The key that joins blocks: registration, extra key, direction and label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    address: [u8; 7],
    key_extra: u32,
    downlink: bool,
    label: [u8; 2],
}

/// The body of a message being joined: the first block's fields, the text so far, the quality and the caller's data of the last
/// block.
#[derive(Clone, Debug, PartialEq)]
pub struct Body<X> {
    /// The first block's fields.
    pub head: Head,
    /// The application text of all blocks (each downlink prefix removed).
    pub text: String,
    /// The sum of the repaired bits, and whether every BCS was good.
    pub quality: Quality,
    /// The caller's data of the last block.
    pub extra: X,
}

impl<X> sdrkit_util_objects::Joinable for Body<X> {
    fn joined_len(&self) -> usize {
        self.text.len()
    }

    fn append(&mut self, next: Self) {
        self.text.push_str(&next.text);
        self.quality.bits_fixed += next.quality.bits_fixed;
        self.quality.bcs_ok &= next.quality.bcs_ok;
        self.extra = next.extra;
    }
}

/// A message given out by the assembler.
pub type Joined<X> = sdrkit_util_objects::Joined<Key, Body<X>>;

/// The multi-block assembler. `X` is whatever the caller carries with a block (channel, level ...).
pub struct Assembler<X> {
    joiner: Joiner<Key, Body<X>>,
}

impl<X> Default for Assembler<X> {
    fn default() -> Self {
        Self::new()
    }
}

impl<X> Assembler<X> {
    /// An assembler with the ACARS rules ([`MAX_OPEN`], [`MAX_JOINED`], [`TIMEOUT_S`]).
    pub fn new() -> Self {
        Assembler {
            joiner: Joiner::new(JoinRules {
                timeout_s: TIMEOUT_S,
                max_open: MAX_OPEN,
                max_len: MAX_JOINED,
            }),
        }
    }

    /// Turns joining on or off. If it is off, every block leaves as a message of its own. Turning it off emits the open
    /// messages first.
    pub fn set_enabled(&mut self, on: bool, out: &mut Vec<Joined<X>>) {
        self.joiner.set_enabled(on, out);
    }

    /// The number of open messages.
    pub fn open(&self) -> usize {
        self.joiner.open_count()
    }

    /// Forgets every open message.
    pub fn clear(&mut self) {
        self.joiner.clear();
    }

    /// Emits, incomplete, every open message with no block for [`TIMEOUT_S`] before `now`.
    pub fn expire(&mut self, now: f64, out: &mut Vec<Joined<X>>) {
        self.joiner.expire(now, out);
    }

    /// Emits every open message, incomplete (at the end of the input).
    pub fn flush(&mut self, out: &mut Vec<Joined<X>>) {
        self.joiner.flush(out);
    }

    /// Takes one block received at signal time `time`. Adds to `out` each message this block completes or ends. `key_extra` is
    /// added to the key that joins blocks (0 when the link has none).
    pub fn push(
        &mut self,
        time: f64,
        block: &Block,
        quality: Quality,
        key_extra: u32,
        extra: X,
        out: &mut Vec<Joined<X>>,
    ) {
        let head = Head::of(block, key_extra);
        let key = Key {
            address: head.address,
            key_extra,
            downlink: head.downlink(),
            label: head.label,
        };
        let prefix = block.downlink_prefix();
        let message = prefix.map_or(0, |p| {
            let m = p.msn.as_bytes();
            u32::from_be_bytes([0, m[0], m[1], m[2]])
        });
        let number = match prefix.map(|p| p.sequence) {
            Some(s @ b'A'..=b'Z') => Numbering::Seq(u32::from(s - b'A')),
            _ => Numbering::Tag(u32::from(block.block_id)),
        };
        let part = Part {
            t: time,
            message,
            number,
            last: block.end == End::Etx,
            body: Body {
                head,
                text: block.app_text().to_owned(),
                quality,
                extra,
            },
        };
        self.joiner.push(key, part, out);
    }
}
