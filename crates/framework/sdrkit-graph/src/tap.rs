//! Taps: copies of any stream for another thread (a scope, an audio monitor, a spectrum of any point in the chain).
//!
//! A tap holds at most `chunks` buffers in two bounded single-producer, single-consumer rings. When no buffer is free (the reader
//! is behind), the copy is dropped and counted. Neither side waits or locks.

use crate::stream::{Buffer, Format};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug)]
struct Shared {
    dropped_items: AtomicU64,
    dropped_chunks: AtomicU64,
    rate_bits: AtomicU64,
    centre_bits: AtomicU64,
}

/// The reading end of a tap. Send it to the thread that reads; dropping it removes the tap.
#[derive(Debug)]
pub struct TapReader {
    full: Consumer<Buffer>,
    empty: Producer<Buffer>,
    shared: Arc<Shared>,
    format: Format,
}

pub(crate) struct TapWriter {
    full: Producer<Buffer>,
    empty: Consumer<Buffer>,
    /// Buffers not made yet (each is made on first need, up to the cap).
    unmade: usize,
    largest: usize,
    shared: Arc<Shared>,
}

pub(crate) fn tap(format: Format, chunks: usize) -> (TapWriter, TapReader) {
    let cap = chunks.max(1);
    let (fp, fc) = RingBuffer::new(cap);
    let (ep, ec) = RingBuffer::new(cap);
    let shared = Arc::new(Shared {
        dropped_items: AtomicU64::new(0),
        dropped_chunks: AtomicU64::new(0),
        rate_bits: AtomicU64::new(format.rate.to_bits()),
        centre_bits: AtomicU64::new(format.centre_hz.to_bits()),
    });
    (
        TapWriter {
            full: fp,
            empty: ec,
            unmade: cap,
            largest: 0,
            shared: shared.clone(),
        },
        TapReader {
            full: fc,
            empty: ep,
            shared,
            format,
        },
    )
}

impl TapWriter {
    /// Copies `src` into a free buffer for the reader; drops it (and counts it) when none is free. Never waits.
    pub(crate) fn push(&mut self, src: &Buffer) {
        let mut b = match self.empty.pop() {
            Ok(b) => b,
            Err(_) if self.unmade > 0 => {
                self.unmade -= 1;
                Buffer::default()
            }
            Err(_) => return self.dropped(src),
        };
        b.copy_from(src);
        self.largest = self.largest.max(b.bytes());
        if self.full.push(b).is_err() {
            // Only when the reader has put buffers of its own into circulation: this one is freed.
            self.dropped(src);
        }
    }
    /// As [`push`](Self::push), for the graph's input: its samples come from the caller's slice.
    pub(crate) fn push_input(&mut self, src: &Buffer, iq: &[sdrkit_core::Complex32]) {
        let mut b = match self.empty.pop() {
            Ok(b) => b,
            Err(_) if self.unmade > 0 => {
                self.unmade -= 1;
                Buffer::default()
            }
            Err(_) => return self.dropped_n(iq.len()),
        };
        b.copy_input(src, iq);
        self.largest = self.largest.max(b.bytes());
        if self.full.push(b).is_err() {
            self.dropped_n(iq.len());
        }
    }
    fn dropped_n(&self, n: usize) {
        self.shared
            .dropped_items
            .fetch_add(n as u64, Ordering::Relaxed);
        self.shared.dropped_chunks.fetch_add(1, Ordering::Relaxed);
    }
    fn dropped(&self, src: &Buffer) {
        self.shared
            .dropped_items
            .fetch_add(src.len() as u64, Ordering::Relaxed);
        self.shared.dropped_chunks.fetch_add(1, Ordering::Relaxed);
    }
    /// The reader was dropped.
    pub(crate) fn closed(&self) -> bool {
        self.full.is_abandoned()
    }
    pub(crate) fn reformat(&self, f: &Format) {
        self.shared
            .rate_bits
            .store(f.rate.to_bits(), Ordering::Relaxed);
        self.shared
            .centre_bits
            .store(f.centre_hz.to_bits(), Ordering::Relaxed);
    }
    /// An upper estimate of the bytes the tap's buffers hold: buffers made times the largest copy.
    pub(crate) fn bytes(&self) -> usize {
        (self.full.buffer().capacity() - self.unmade)
            * (self.largest + std::mem::size_of::<Buffer>())
    }
}

impl TapReader {
    /// Takes the oldest waiting chunk: swaps it into `into`, whose old allocation goes back to the graph for reuse. Returns false when
    /// nothing is waiting. Never waits.
    pub fn pop(&mut self, into: &mut Buffer) -> bool {
        match self.full.pop() {
            Ok(mut b) => {
                std::mem::swap(into, &mut b);
                // The ring back has room for every buffer in circulation; if not, the buffer is simply freed here.
                let _ = self.empty.push(b);
                true
            }
            Err(_) => false,
        }
    }
    /// Chunks waiting to be read.
    pub fn waiting(&self) -> usize {
        self.full.slots()
    }
    /// Items dropped because the reader was behind.
    pub fn dropped_items(&self) -> u64 {
        self.shared.dropped_items.load(Ordering::Relaxed)
    }
    /// Chunks dropped because the reader was behind.
    pub fn dropped_chunks(&self) -> u64 {
        self.shared.dropped_chunks.load(Ordering::Relaxed)
    }
    /// The stream's format when the tap was made.
    pub fn format(&self) -> &Format {
        &self.format
    }
    /// The stream's rate now (it follows a reconfigure of the graph).
    pub fn rate(&self) -> f64 {
        f64::from_bits(self.shared.rate_bits.load(Ordering::Relaxed))
    }
    /// The stream's centre frequency now (as of the last configure).
    pub fn centre_hz(&self) -> f64 {
        f64::from_bits(self.shared.centre_bits.load(Ordering::Relaxed))
    }
}
