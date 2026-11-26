//! The counters of a hub as a snapshot, and the events of its listeners and clients. Counters are atomics that the feeder and
//! the writers update; a snapshot copies them and never locks anything the feeder uses.

use crate::client::ClientShared;
use crate::control::{Answer, Request};
use crate::hub::{lock, Inner};
use crate::slice::Slice;
use sdrkit_graph::stream::SampleType;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

/// Why a client left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leave {
    /// The client hung up or the connection failed.
    Closed,
    /// The client's loss ran on for `drop_after_s`.
    TooSlow,
    /// The owner kicked it.
    Kicked,
    /// The hub shut down.
    Shutdown,
}

/// Why a listener refused a connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The address lies outside the allow list.
    NotAllowed,
    /// `max_clients` clients are connected.
    Full,
}

/// Something that happened to a listener or a client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// A listener admitted a client.
    ClientJoined {
        /// The client.
        client: u64,
        /// The listener.
        listener: u64,
        /// The client's address.
        peer: Option<SocketAddr>,
    },
    /// A client left.
    ClientLeft {
        /// The client.
        client: u64,
        /// The listener that took it.
        listener: u64,
        /// The client's address.
        peer: Option<SocketAddr>,
        /// Why.
        reason: Leave,
    },
    /// A listener refused a connection.
    Refused {
        /// The listener.
        listener: u64,
        /// The address that connected.
        peer: Option<SocketAddr>,
        /// Why.
        reason: Refusal,
    },
}

/// The counters of one client.
#[derive(Clone, Debug, PartialEq)]
pub struct ClientStats {
    /// The client's id.
    pub id: u64,
    /// The listener that took it.
    pub listener: u64,
    /// Its address.
    pub peer: Option<SocketAddr>,
    /// The protocol's name.
    pub protocol: &'static str,
    /// The sample type of its stream, `None` while it has no stream.
    pub wire: Option<SampleType>,
    /// When it connected.
    pub since: SystemTime,
    /// Bytes written to it, the greeting not counted.
    pub bytes: u64,
    /// Samples written to it.
    pub samples: u64,
    /// Samples of the source dropped because its ring was full.
    pub dropped_samples: u64,
    /// Chunks dropped because its ring was full.
    pub dropped_chunks: u64,
    /// Loss runs: each run of drops counts once.
    pub drop_events: u64,
    /// Breaks in its stream: its own drops, losses of the source and retunes.
    pub gaps: u64,
    /// Chunks waiting in its ring.
    pub queue_fill: usize,
    /// The capacity of its ring, chunks.
    pub queue_capacity: usize,
    /// Requests it made.
    pub requests: u64,
    /// Requests that were refused.
    pub refused_requests: u64,
    /// Its slice, if it has one. A client with only a rate of its own has a slice at the capture centre.
    pub slice: Option<Slice>,
    /// True once it asked for a setting or a stream. A connection that only reads the device information is not
    /// engaged: it is not a client of the share.
    pub engaged: bool,
    /// The last request of each kind that was not carried out as asked, with its answer.
    pub not_done: Vec<(Request, Answer)>,
    /// What carries its stream: "tcp", "udp", or empty without a stream.
    pub path: &'static str,
    /// The rate that it asked for last.
    pub asked_rate_hz: Option<f64>,
    /// True once its writer has stopped.
    pub closed: bool,
}

/// The counters of one listener.
#[derive(Clone, Debug, PartialEq)]
pub struct ListenerStats {
    /// The listener's id.
    pub id: u64,
    /// The address it listens on.
    pub addr: SocketAddr,
    /// The protocol's name.
    pub protocol: &'static str,
    /// When it started.
    pub since: SystemTime,
    /// Its clients now.
    pub clients: usize,
    /// Connections admitted.
    pub accepted: u64,
    /// Connections refused by the allow list.
    pub refused_allow: u64,
    /// Connections refused because `max_clients` clients were connected.
    pub refused_full: u64,
    /// Bytes written to its clients, past and present.
    pub bytes: u64,
}

/// The counters of a hub at one moment.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// The sample type of the source, as of the last block.
    pub format: SampleType,
    /// The centre and rate of the capture, as of the last block.
    pub capture: Slice,
    /// Blocks pushed.
    pub blocks: u64,
    /// Samples pushed.
    pub samples: u64,
    /// Chunks that found no free chunk in the pool, and blocks of a format the hub does not carry.
    pub pool_misses: u64,
    /// The listeners.
    pub listeners: Vec<ListenerStats>,
    /// The clients that have not been removed yet.
    pub clients: Vec<ClientStats>,
}

/// The counters of client `s` now.
fn client_stats(s: &ClientShared, inner: &Inner) -> ClientStats {
    let r = |a: &std::sync::atomic::AtomicU64| a.load(Ordering::Relaxed);
    ClientStats {
        id: s.id,
        listener: s.listener,
        peer: s.peer,
        protocol: s.protocol,
        wire: s.wire(),
        since: s.since,
        bytes: r(&s.bytes),
        samples: r(&s.samples),
        dropped_samples: r(&s.dropped_samples),
        dropped_chunks: r(&s.dropped_chunks),
        drop_events: r(&s.drop_events),
        gaps: r(&s.gaps),
        queue_fill: s.fill.load(Ordering::Relaxed),
        queue_capacity: s.capacity,
        requests: r(&s.requests),
        refused_requests: r(&s.refused),
        slice: lock(&s.slice).map(|mut sl| {
            if s.follow_centre.load(Ordering::Acquire) {
                sl.centre_hz = inner.capture().1.centre_hz;
            }
            sl
        }),
        engaged: s.engaged.load(Ordering::Acquire),
        not_done: lock(&s.not_done).clone(),
        path: *lock(&s.path),
        asked_rate_hz: Some(f64::from_bits(s.asked_rate.load(Ordering::Relaxed)))
            .filter(|r| *r > 0.0),
        closed: s.closed.load(Ordering::Acquire),
    }
}

/// The counters of the hub now. Clients that left a moment ago with requests not carried out come too, closed.
pub(crate) fn snapshot(inner: &Inner) -> Snapshot {
    let r = |a: &std::sync::atomic::AtomicU64| a.load(Ordering::Relaxed);
    let mut clients: Vec<ClientStats> = lock(&inner.clients)
        .iter()
        .map(|c| client_stats(&c.shared, inner))
        .collect();
    let listeners = lock(&inner.listeners)
        .iter()
        .map(|l| {
            let s = &l.shared;
            let mine = clients.iter().filter(|c| c.listener == s.id);
            ListenerStats {
                id: s.id,
                addr: s.addr,
                protocol: s.protocol,
                since: s.since,
                clients: mine.clone().filter(|c| !c.closed && c.engaged).count(),
                accepted: r(&s.accepted),
                refused_allow: r(&s.refused_allow),
                refused_full: r(&s.refused_full),
                bytes: r(&s.closed_bytes) + mine.map(|c| c.bytes).sum::<u64>(),
            }
        })
        .collect();
    // After the listeners, whose byte counts hold those of the clients that left.
    clients.extend(
        lock(&inner.departed)
            .iter()
            .map(|(s, _)| client_stats(s, inner)),
    );
    let (format, capture) = inner.capture();
    Snapshot {
        format,
        capture,
        blocks: r(&inner.blocks),
        samples: r(&inner.samples),
        pool_misses: r(&inner.pool_misses),
        listeners,
        clients,
    }
}
