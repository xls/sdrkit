//! Transmit from a client toward the share: a bounded single-producer, single-consumer byte ring per transmitting client.
//! The protocol writes the client's samples into the [`TxWriter`]; the owner of the share drains the [`TxReader`] that its
//! [`crate::Control::transmit`] accepted. A read that finds fewer samples than it asks for, in a burst that has not ended,
//! is an underrun; the protocol reads the count from the writer and reports it to the client.

use sdrkit_driver_core::sample::size;
use sdrkit_graph::stream::SampleType;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// What a client asks to transmit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TxRequest {
    /// The sample type of its samples.
    pub format: SampleType,
    /// The sample rate, Hz.
    pub rate_hz: f64,
    /// The centre frequency, Hz (0 when the protocol does not say).
    pub centre_hz: f64,
}

#[derive(Debug, Default)]
struct TxShared {
    written: AtomicU64,
    read: AtomicU64,
    refused_writes: AtomicU64,
    underrun_events: AtomicU64,
    underrun_samples: AtomicU64,
    ended: AtomicBool,
}

/// The counters of a transmit ring.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TxCounts {
    /// Samples the client wrote into the ring.
    pub written: u64,
    /// Samples the owner read.
    pub read: u64,
    /// Samples the ring had no room for.
    pub refused_writes: u64,
    /// Underruns: each run of short reads counts once.
    pub underrun_events: u64,
    /// Samples the owner asked for and did not get.
    pub underrun_samples: u64,
}

/// The protocol's end of a transmit ring.
pub struct TxWriter {
    tx: rtrb::Producer<u8>,
    shared: Arc<TxShared>,
    ss: usize,
}

/// The owner's end of a transmit ring.
pub struct TxReader {
    rx: rtrb::Consumer<u8>,
    shared: Arc<TxShared>,
    ss: usize,
    request: TxRequest,
    short: bool,
}

impl std::fmt::Debug for TxWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxWriter")
            .field("room", &self.room())
            .finish()
    }
}

impl std::fmt::Debug for TxReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxReader")
            .field("request", &self.request)
            .finish()
    }
}

/// A ring for `samples` samples of `request.format`.
pub fn tx_ring(request: TxRequest, samples: usize) -> (TxWriter, TxReader) {
    let ss = size(request.format);
    let (tx, rx) = rtrb::RingBuffer::new(samples.max(1) * ss);
    let shared = Arc::new(TxShared::default());
    (
        TxWriter {
            tx,
            shared: shared.clone(),
            ss,
        },
        TxReader {
            rx,
            shared,
            ss,
            request,
            short: false,
        },
    )
}

fn counts(s: &TxShared) -> TxCounts {
    let r = |a: &AtomicU64| a.load(Ordering::Relaxed);
    TxCounts {
        written: r(&s.written),
        read: r(&s.read),
        refused_writes: r(&s.refused_writes),
        underrun_events: r(&s.underrun_events),
        underrun_samples: r(&s.underrun_samples),
    }
}

impl TxWriter {
    /// Writes the whole samples of `bytes` that fit and returns how many bytes it took. It never waits. A write starts a new
    /// burst.
    pub fn write(&mut self, bytes: &[u8]) -> usize {
        let fit = (self.tx.slots() / self.ss).min(bytes.len() / self.ss) * self.ss;
        if let Ok(chunk) = self.tx.write_chunk_uninit(fit) {
            chunk.fill_from_iter(bytes[..fit].iter().copied());
        }
        let n = (fit / self.ss) as u64;
        let refused = (bytes.len() / self.ss) as u64 - n;
        self.shared.written.fetch_add(n, Ordering::Relaxed);
        if refused > 0 {
            self.shared
                .refused_writes
                .fetch_add(refused, Ordering::Relaxed);
        }
        if fit > 0 {
            self.shared.ended.store(false, Ordering::Release);
        }
        fit
    }
    /// Free room, bytes.
    pub fn room(&self) -> usize {
        self.tx.slots() / self.ss * self.ss
    }
    /// Marks the end of the present burst: short reads after it are not underruns.
    pub fn end_burst(&mut self) {
        self.shared.ended.store(true, Ordering::Release);
    }
    /// True if the owner dropped its reader.
    pub fn closed(&self) -> bool {
        self.tx.is_abandoned()
    }
    /// The counters of the ring.
    pub fn counts(&self) -> TxCounts {
        counts(&self.shared)
    }
}

impl TxReader {
    /// What the client asked for.
    pub fn request(&self) -> &TxRequest {
        &self.request
    }
    /// Reads whole samples into `out` and returns the bytes read. A read that gets fewer than `out` holds while the burst
    /// has not ended counts the shortfall as underrun samples, and the first of a run of such reads as an underrun.
    pub fn read(&mut self, out: &mut [u8]) -> usize {
        let want = out.len() / self.ss * self.ss;
        let n = (self.rx.slots() / self.ss * self.ss).min(want);
        if let Ok(chunk) = self.rx.read_chunk(n) {
            let (a, b) = chunk.as_slices();
            out[..a.len()].copy_from_slice(a);
            out[a.len()..a.len() + b.len()].copy_from_slice(b);
            chunk.commit_all();
        }
        self.shared
            .read
            .fetch_add((n / self.ss) as u64, Ordering::Relaxed);
        if n < want && !self.shared.ended.load(Ordering::Acquire) {
            self.shared
                .underrun_samples
                .fetch_add(((want - n) / self.ss) as u64, Ordering::Relaxed);
            if !self.short {
                self.short = true;
                self.shared.underrun_events.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            self.short = false;
        }
        n
    }
    /// Bytes waiting.
    pub fn available(&self) -> usize {
        self.rx.slots() / self.ss * self.ss
    }
    /// True if the burst has ended and every sample of it was read.
    pub fn ended(&self) -> bool {
        self.shared.ended.load(Ordering::Acquire) && self.rx.is_empty()
    }
    /// True if the client's side has gone.
    pub fn closed(&self) -> bool {
        self.rx.is_abandoned()
    }
    /// The counters of the ring.
    pub fn counts(&self) -> TxCounts {
        counts(&self.shared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_pass_in_order_and_short_reads_count_as_underruns() {
        let req = TxRequest {
            format: SampleType::Cs16,
            rate_hz: 1e6,
            centre_hz: 0.0,
        };
        let (mut w, mut r) = tx_ring(req, 4);
        let bytes: Vec<u8> = (0..24).collect();
        assert_eq!(w.write(&bytes[..23]), 16, "whole samples that fit");
        let mut out = [0u8; 12];
        assert_eq!(r.read(&mut out), 12);
        assert_eq!(out[..], bytes[..12]);
        assert_eq!(w.write(&bytes[16..24]), 8);
        let mut out = [0u8; 16];
        assert_eq!(r.read(&mut out), 12);
        assert_eq!(out[..12], [12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]);
        assert_eq!(r.read(&mut out), 0);
        let c = r.counts();
        assert_eq!(
            (
                c.written,
                c.read,
                c.refused_writes,
                c.underrun_events,
                c.underrun_samples
            ),
            (6, 6, 1, 1, 5)
        );
        w.end_burst();
        assert_eq!(r.read(&mut out), 0);
        assert!(r.ended());
        assert_eq!(w.counts().underrun_events, 1);
        drop(r);
        assert!(w.closed());
    }
}
