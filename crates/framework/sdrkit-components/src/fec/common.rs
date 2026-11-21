//! Pieces shared by the `fec.*` components: the per-frame status item, the erasure-list
//! item, a pool of packet buffers (so emitting byte packets allocates nothing in steady state), and the frame gatherer that cuts
//! a soft-bit stream into code words at fixed lengths and at `FrameStart` tags.

use sdrkit_core::publish::field;
use sdrkit_core::{EventDef, Field, Record};
use sdrkit_graph::{Item, Tag, TagKind};

/// Turns a plain name into a choice key: lower case, every run of other characters one `_` (`"dvb-s2 normal 1/2"` becomes
/// `"dvb_s2_normal_1_2"`).
pub(crate) fn snake(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c.to_ascii_lowercase());
        } else if !s.ends_with('_') {
            s.push('_');
        }
    }
    while s.ends_with('_') {
        s.pop();
    }
    s
}

/// What a frame decoder reports for each frame (or packet) it decodes, on its typed `status` output (kind `"fec.status"`), in
/// the order of its data output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStatus {
    /// The frame decoded (every code word of it corrected, and its check passed where the decoder has one).
    pub ok: bool,
    /// Symbols (Reed-Solomon) or bits (BCH, LDPC, turbo) the decoder changed.
    pub corrected: u32,
    /// Iterations used by an iterative decoder; 1 for the others.
    pub iterations: u32,
    /// Code words of the frame that could not be decoded (an interleaved Reed-Solomon frame has several; 0 or 1 otherwise).
    pub failed_words: u32,
}

impl Item for FrameStatus {
    const KIND: &'static str = "fec.status";
    fn event_def() -> Option<&'static EventDef> {
        Some(&FRAME_STATUS)
    }
    fn write_fields(&self, r: &mut Record) {
        r.set(&OK, self.ok);
        r.set(&CORRECTED, i64::from(self.corrected));
        r.set(&ITERATIONS, i64::from(self.iterations));
        r.set(&FAILED_WORDS, i64::from(self.failed_words));
    }
}

const OK: Field<field::Bool> = Field::new("ok", "Decoded", "");
const CORRECTED: Field<field::I64> = Field::new("corrected", "Corrected", "");
const ITERATIONS: Field<field::I64> = Field::new("iterations", "Iterations", "");
const FAILED_WORDS: Field<field::I64> = Field::new("failed_words", "Failed code words", "");

static FRAME_STATUS: EventDef = EventDef::new(
    "fec.status",
    "Frame status",
    &[&OK, &CORRECTED, &ITERATIONS, &FAILED_WORDS],
);

/// The symbols of one packet known to be unreliable, as indices into the packet (kind `"fec.erasures"`): the input of
/// `fec.rs_decode` on its optional `erasures` input, one list per packet, in packet order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Erasures(pub Vec<u16>);

impl Item for Erasures {
    const KIND: &'static str = "fec.erasures";
}

pub(crate) use sdrkit_graph::PacketPool;

/// Cuts a stream into frames of a fixed length. A `FrameStart` tag starts a new frame at its item: a partial frame before it is
/// dropped (and counted). Any cut of the stream produces the same frames.
#[derive(Debug)]
pub(crate) struct Framer<T> {
    buf: Vec<T>,
    len: usize,
    /// Absolute index of the first item in `buf`.
    start: u64,
    /// Items dropped in partial frames since the last reset.
    pub dropped: u64,
}

impl<T> Default for Framer<T> {
    fn default() -> Self {
        Framer {
            buf: Vec::new(),
            len: 1,
            start: 0,
            dropped: 0,
        }
    }
}

impl<T: Copy> Framer<T> {
    /// Frames of `len` items (allocates the buffer).
    pub fn reset(&mut self, len: usize) {
        self.buf.clear();
        self.buf.reserve(len);
        self.len = len.max(1);
        self.dropped = 0;
    }

    /// Adds `items` (the first at absolute index `first`; `tags` those of the same buffer), calling `frame(items, index)` for each
    /// complete frame with the absolute index of its first item.
    pub fn feed(
        &mut self,
        items: &[T],
        first: u64,
        tags: &[Tag],
        mut frame: impl FnMut(&[T], u64),
    ) {
        let mut pos = 0usize;
        let mut tags = tags
            .iter()
            .filter(|t| matches!(t.kind, TagKind::FrameStart { .. }))
            .map(|t| t.at.saturating_sub(first) as usize)
            .filter(|&r| r < items.len())
            .peekable();
        while pos < items.len() {
            if tags.peek() == Some(&pos) {
                tags.next();
                self.dropped += self.buf.len() as u64;
                self.buf.clear();
            }
            let stop = tags.peek().copied().unwrap_or(items.len()).max(pos);
            if self.buf.is_empty() {
                self.start = first + pos as u64;
            }
            let take = (self.len - self.buf.len()).min(stop - pos);
            if take == 0 {
                continue;
            }
            self.buf.extend_from_slice(&items[pos..pos + take]);
            pos += take;
            if self.buf.len() == self.len {
                frame(&self.buf, self.start);
                self.buf.clear();
            }
        }
    }
}
