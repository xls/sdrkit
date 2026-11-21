//! Each output runs on its own thread behind a bounded queue. A full queue drops the item and counts it. After a failure
//! the output waits with exponential back-off and tries again.

use std::collections::VecDeque;
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::net::Backoff;
use crate::OutputError;

/// What an output sends, made on the event thread.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// Bytes, with the MQTT topic (empty for other outputs) and the family of the event.
    Packet {
        /// The MQTT topic.
        topic: Box<str>,
        /// The family of the event.
        family: sdrkit_core::publish::keys::Family,
        /// The bytes.
        body: Box<[u8]>,
    },
    /// A spot report.
    Spot(crate::spots::Spot),
}

impl Item {
    /// The bytes of a packet. Empty for a spot.
    pub fn body(&self) -> &[u8] {
        match self {
            Item::Packet { body, .. } => body,
            Item::Spot(_) => &[],
        }
    }
}

/// A destination for items of type `T`.
pub trait Sink<T>: Send {
    /// Sends one item, connecting first when not connected.
    fn deliver(&mut self, item: &T) -> Result<(), OutputError>;
    /// Called when no item came for a while (about a second): keep-alives, reading what the server sent.
    fn idle(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    /// Drops the connection after a failure.
    fn reset(&mut self) {}
    /// Says goodbye (the runner stops).
    fn close(&mut self) {}
    /// Whether the sink holds a live connection, for the status.
    fn connected(&self) -> bool {
        true
    }
}

/// A sink's counters.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SinkStats {
    /// Items delivered.
    pub sent: u64,
    /// Items dropped: queue full, or given up after a failure.
    pub dropped: u64,
    /// Failures.
    pub errors: u64,
    /// Whether the sink is connected.
    pub connected: bool,
    /// The last failure, in words (never holding a password or token).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

enum Msg<T> {
    Item(T),
    Stop,
}

/// A running sink.
pub struct SinkRunner<T: Send + 'static> {
    tx: SyncSender<Msg<T>>,
    stats: Arc<Mutex<SinkStats>>,
    thread: Option<JoinHandle<()>>,
}

/// Retries per item before it is dropped.
const ATTEMPTS: u32 = 3;

impl<T: Send + 'static> SinkRunner<T> {
    /// Starts a sink with a queue of `queue` items (at least 1), and this back-off.
    ///
    /// # Errors
    ///
    /// [`OutputError::Thread`] if the system refuses to start the thread.
    pub fn spawn(
        name: &str,
        mut sink: Box<dyn Sink<T>>,
        queue: usize,
        mut backoff: Backoff,
    ) -> Result<Self, OutputError> {
        let queue = queue.max(1);
        let (tx, rx): (SyncSender<Msg<T>>, Receiver<Msg<T>>) = sync_channel(queue);
        let stats = Arc::new(Mutex::new(SinkStats::default()));
        let st = stats.clone();
        let thread = std::thread::Builder::new()
            .name(format!("sdrkit-out-{name}"))
            .spawn(move || {
                let mut pending: VecDeque<(T, u32)> = VecDeque::new();
                let mut retry_at: Option<Instant> = None;
                let set = |f: &dyn Fn(&mut SinkStats)| {
                    f(&mut st.lock().unwrap_or_else(|p| p.into_inner()))
                };
                loop {
                    let wait = match retry_at {
                        Some(t) => t.saturating_duration_since(Instant::now()),
                        None if pending.is_empty() => Duration::from_secs(1),
                        None => Duration::ZERO,
                    };
                    match rx.recv_timeout(wait) {
                        Ok(Msg::Item(item)) => {
                            pending.push_back((item, 0));
                            while pending.len() > queue {
                                pending.pop_front();
                                set(&|s| s.dropped += 1);
                            }
                        }
                        Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                        Err(RecvTimeoutError::Timeout) => {
                            if pending.is_empty() && retry_at.is_none() {
                                if let Err(e) = sink.idle() {
                                    sink.reset();
                                    set(&|s| {
                                        s.errors += 1;
                                        s.last_error = Some(e.to_string());
                                    });
                                }
                                let c = sink.connected();
                                set(&|s| s.connected = c);
                                continue;
                            }
                        }
                    }
                    if retry_at.is_some_and(|t| Instant::now() < t) {
                        continue;
                    }
                    retry_at = None;
                    while let Some((item, tries)) = pending.pop_front() {
                        match sink.deliver(&item) {
                            Ok(()) => {
                                backoff.reset();
                                set(&|s| {
                                    s.sent += 1;
                                    s.connected = true;
                                });
                            }
                            Err(e) => {
                                sink.reset();
                                let msg = e.to_string();
                                set(&|s| {
                                    s.errors += 1;
                                    s.connected = false;
                                    s.last_error = Some(msg.clone());
                                });
                                if tries + 1 < ATTEMPTS && e.retryable() {
                                    pending.push_front((item, tries + 1));
                                } else {
                                    set(&|s| s.dropped += 1);
                                }
                                retry_at = Some(Instant::now() + backoff.next_wait());
                                break;
                            }
                        }
                    }
                }
                sink.close();
                let left = pending.len() as u64;
                set(&|s| {
                    s.dropped += left;
                    s.connected = false;
                });
            })
            .map_err(OutputError::Thread)?;
        Ok(SinkRunner {
            tx,
            stats,
            thread: Some(thread),
        })
    }

    /// Queues an item; never blocks. Returns false when it was dropped because the queue is full.
    pub fn submit(&self, item: T) -> bool {
        match self.tx.try_send(Msg::Item(item)) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.stats.lock().unwrap_or_else(|p| p.into_inner()).dropped += 1;
                false
            }
        }
    }

    /// The counters.
    pub fn stats(&self) -> SinkStats {
        self.stats.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Stops the sink: what is queued is still offered once (subject to the back-off), then the thread ends.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let _ = self.tx.send(Msg::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl<T: Send + 'static> Drop for SinkRunner<T> {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flaky {
        fail_first: u32,
        got: Arc<Mutex<Vec<u32>>>,
    }

    impl Sink<u32> for Flaky {
        fn deliver(&mut self, item: &u32) -> Result<(), OutputError> {
            if self.fail_first > 0 {
                self.fail_first -= 1;
                return Err(OutputError::Protocol(crate::Protocol::Down));
            }
            self.got.lock().unwrap().push(*item);
            Ok(())
        }
    }

    #[test]
    fn retries_after_back_off_and_keeps_order() {
        let got = Arc::new(Mutex::new(Vec::new()));
        let r = SinkRunner::spawn(
            "t",
            Box::new(Flaky {
                fail_first: 2,
                got: got.clone(),
            }),
            10,
            Backoff::new(Duration::from_millis(5), Duration::from_millis(20)),
        )
        .unwrap();
        for i in 0..5 {
            assert!(r.submit(i));
        }
        let t0 = Instant::now();
        while got.lock().unwrap().len() < 5 && t0.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(*got.lock().unwrap(), [0, 1, 2, 3, 4]);
        let s = r.stats();
        assert_eq!((s.sent, s.errors, s.dropped), (5, 2, 0));
        assert_eq!(s.last_error.as_deref(), Some("protocol: down"));
        r.stop();
    }

    #[test]
    fn a_dead_sink_drops_after_its_attempts() {
        let got = Arc::new(Mutex::new(Vec::new()));
        let r = SinkRunner::spawn(
            "d",
            Box::new(Flaky {
                fail_first: u32::MAX,
                got,
            }),
            2,
            Backoff::new(Duration::from_millis(1), Duration::from_millis(2)),
        )
        .unwrap();
        for i in 0..50 {
            r.submit(i);
        }
        let t0 = Instant::now();
        while r.stats().dropped < 50 && t0.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        let s = r.stats();
        assert_eq!(s.sent, 0);
        assert!(s.dropped >= 48, "{s:?}");
        drop(r);
    }
}
