//! The work counters of a component. The graph times every call of a component and counts its items. A component can
//! also time its inner steps as named sections:
//!
//! ```ignore
//! let t = io.section("viterbi");   // or `self.meter.section("viterbi")` with a handle from `io.meter()`
//! decode(..);
//! drop(t);                         // the time until here counts for the section
//! ```
//!
//! A section costs two clock reads, and nothing while the metrics of the graph are off.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The counters of one section.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SectionCount {
    pub name: &'static str,
    pub ns: u64,
    pub worst_ns: u64,
    pub calls: u64,
}

/// The counters of one component. One thread writes them, any thread reads them.
#[derive(Debug, Default)]
pub(crate) struct Counters {
    pub ns: AtomicU64,
    pub worst_ns: AtomicU64,
    pub calls: AtomicU64,
    pub items_in: AtomicU64,
    pub items_out: AtomicU64,
    pub sections: Mutex<Vec<SectionCount>>,
}

impl Counters {
    /// Counts one call of `ns` nanoseconds with its items.
    pub(crate) fn call(&self, ns: u64, items_in: u64, items_out: u64) {
        add(&self.ns, ns);
        add(&self.calls, 1);
        add(&self.items_in, items_in);
        add(&self.items_out, items_out);
        if ns > self.worst_ns.load(Relaxed) {
            self.worst_ns.store(ns, Relaxed);
        }
    }

    fn section(&self, name: &'static str, ns: u64) {
        let mut s = self.sections.lock().unwrap_or_else(|p| p.into_inner());
        let k = match s.iter().position(|c| c.name == name) {
            Some(k) => k,
            None => {
                s.push(SectionCount {
                    name,
                    ..SectionCount::default()
                });
                s.len() - 1
            }
        };
        let c = &mut s[k];
        c.ns += ns;
        c.calls += 1;
        c.worst_ns = c.worst_ns.max(ns);
    }

    #[cfg(test)]
    pub(crate) fn sections(&self) -> Vec<SectionCount> {
        self.sections
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

/// One pass in this many has its waits measured.
pub(crate) const SAMPLE_EVERY: u64 = 16;

/// The nanoseconds from a fixed instant of the process to `t`.
pub(crate) fn ns_of(t: Instant) -> u64 {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    t.saturating_duration_since(*EPOCH.get_or_init(Instant::now))
        .as_nanos() as u64
}

/// One thread writes: a load and a store are enough, and cheaper than an atomic add.
fn add(a: &AtomicU64, n: u64) {
    a.store(a.load(Relaxed) + n, Relaxed);
}

/// The counters of one stream: what its writer wrote, and when the writer ended a sampled pass. One thread writes.
#[derive(Debug, Default)]
pub(crate) struct StreamCount {
    pub items: AtomicU64,
    pub bytes: AtomicU64,
    /// The bytes after the last call, their sum over the calls, the calls, and the most.
    pub held: AtomicU64,
    pub held_sum: AtomicU64,
    pub held_n: AtomicU64,
    pub held_max: AtomicU64,
    /// The times the writer found the stream at its bound of items.
    pub full: AtomicU64,
    /// The end of the writer's call in four recent sampled passes: the pass and the time.
    end_seq: [AtomicU64; 4],
    end_ns: [AtomicU64; 4],
}

impl StreamCount {
    /// Counts one call of the writer that left `items` of `bytes` in the stream.
    pub(crate) fn wrote(&self, items: u64, bytes: u64) {
        add(&self.items, items);
        add(&self.bytes, bytes);
        self.held.store(bytes, Relaxed);
        add(&self.held_sum, bytes);
        add(&self.held_n, 1);
        if bytes > self.held_max.load(Relaxed) {
            self.held_max.store(bytes, Relaxed);
        }
    }

    /// Notes that the writer ended the sampled pass `seq` at `ns`.
    pub(crate) fn stamp(&self, seq: u64, ns: u64) {
        let k = (seq / SAMPLE_EVERY % 4) as usize;
        self.end_ns[k].store(ns, Relaxed);
        self.end_seq[k].store(seq + 1, Relaxed);
    }

    /// When the writer ended the sampled pass `seq`, if it is one of the four that the stream keeps.
    pub(crate) fn end_of(&self, seq: u64) -> Option<u64> {
        let k = (seq / SAMPLE_EVERY % 4) as usize;
        let ns = self.end_ns[k].load(Relaxed);
        (self.end_seq[k].load(Relaxed) == seq + 1).then_some(ns)
    }
}

/// The counters of one input port of a component. One thread writes.
#[derive(Debug, Default)]
pub(crate) struct InputCount {
    pub wait_sum: AtomicU64,
    pub wait_n: AtomicU64,
    pub wait_worst: AtomicU64,
    /// The calls that found the input empty.
    pub empty: AtomicU64,
}

impl InputCount {
    pub(crate) fn empty(&self) {
        add(&self.empty, 1);
    }

    pub(crate) fn waited(&self, ns: u64) {
        add(&self.wait_sum, ns);
        add(&self.wait_n, 1);
        if ns > self.wait_worst.load(Relaxed) {
            self.wait_worst.store(ns, Relaxed);
        }
    }
}

/// The load of one graph, as its counters stood when the graph was dropped while [`record`] ran.
#[derive(Clone, Debug, Default)]
pub struct GraphLoad {
    /// The work of each component and section, in run order.
    pub components: Vec<sdrkit_core::ComponentLoad>,
    /// The load of each stage, in run order.
    pub stages: Vec<sdrkit_core::StageLoad>,
    /// The stages as the stack declared them, with their recorded cost.
    pub decls: Vec<sdrkit_core::StageDecl>,
    /// The traffic of each connection.
    pub connections: Vec<sdrkit_core::ConnectionLoad>,
}

thread_local! {
    static RECORDING: std::cell::RefCell<Option<Vec<GraphLoad>>> = const { std::cell::RefCell::new(None) };
}

/// Runs `f` and returns its result with the load of each graph that was dropped on this thread while it ran. A
/// benchmark builds and drops its own graph, so the host reads its load this way.
pub fn record<R>(f: impl FnOnce() -> R) -> (R, Vec<GraphLoad>) {
    let before = RECORDING.with(|r| r.borrow_mut().replace(Vec::new()));
    let out = f();
    let loads = RECORDING.with(|r| std::mem::replace(&mut *r.borrow_mut(), before));
    (out, loads.unwrap_or_default())
}

/// True while [`record`] runs on this thread.
pub(crate) fn recording() -> bool {
    RECORDING.with(|r| r.borrow().is_some())
}

/// Keeps the load of a graph for [`record`].
pub(crate) fn note(load: GraphLoad) {
    RECORDING.with(|r| {
        if let Some(v) = r.borrow_mut().as_mut() {
            v.push(load);
        }
    });
}

/// The counters of a component, for code inside the component that times its sections. [`Io::meter`](crate::Io::meter)
/// returns it. A handle of a graph whose metrics are off counts nothing.
#[derive(Clone, Debug, Default)]
pub struct Meter(pub(crate) Option<Arc<Counters>>);

impl Meter {
    /// Starts the timer of the section `name`. The time until the timer is dropped counts for the section.
    pub fn section(&self, name: &'static str) -> SectionTimer {
        SectionTimer {
            at: self.0.as_ref().map(|c| (Arc::clone(c), Instant::now())),
            name,
        }
    }
}

/// A running timer of one section ([`Meter::section`]).
#[derive(Debug)]
pub struct SectionTimer {
    at: Option<(Arc<Counters>, Instant)>,
    name: &'static str,
}

impl Drop for SectionTimer {
    fn drop(&mut self) {
        if let Some((c, t0)) = self.at.take() {
            c.section(self.name, t0.elapsed().as_nanos() as u64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stream_keeps_the_end_of_four_sampled_passes() {
        let c = StreamCount::default();
        c.stamp(0, 10);
        c.stamp(SAMPLE_EVERY, 20);
        assert_eq!(c.end_of(0), Some(10));
        assert_eq!(c.end_of(SAMPLE_EVERY), Some(20));
        c.stamp(4 * SAMPLE_EVERY, 30);
        assert_eq!(c.end_of(0), None);
        assert_eq!(c.end_of(4 * SAMPLE_EVERY), Some(30));
        c.wrote(3, 24);
        c.wrote(1, 8);
        assert_eq!(c.items.load(Relaxed), 4);
        assert_eq!((c.held.load(Relaxed), c.held_max.load(Relaxed)), (8, 24));
    }

    #[test]
    fn a_section_counts_its_time_and_an_empty_handle_counts_nothing() {
        let c = Arc::new(Counters::default());
        let m = Meter(Some(Arc::clone(&c)));
        drop(m.section("a"));
        drop(m.section("b"));
        drop(m.section("a"));
        let s = c.sections();
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].name, s[0].calls), ("a", 2));
        assert_eq!((s[1].name, s[1].calls), ("b", 1));
        drop(Meter::default().section("a"));
    }
}
