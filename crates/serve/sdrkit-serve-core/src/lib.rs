//! Serving raw I/Q to network clients, apart from any one protocol.
//!
//! A [`Feeder`] takes the blocks of a source in its own format (Cu8, Cs8, Cs16, Cf32) with the centre and rate of the
//! capture, and queues each block once, as a pooled chunk, for every client. Each client has a bounded ring and a writer
//! thread that converts the chunk to the format of the client's [`Outlet`] (or to its own [`Slice`] of the capture) and
//! sends it there with its [`FrameInfo`]. A full ring drops the chunk for that client only. A reader thread per client runs
//! the [`Parser`] of its [`Protocol`] [`Session`], which hands [`Request`]s to the owner's [`Control`], replies, and asks
//! for a transmit ring ([`TxWriter`], [`TxReader`]). A [`Hub`] opens listeners with an allow list and a client limit, and
//! gives the counters as a [`Snapshot`].

mod client;
mod control;
pub mod format;
mod hub;
mod listen;
mod outlet;
mod protocol;
mod slice;
mod stats;
mod tx;

pub use client::ClientHandle;
pub use control::{Answer, Control, RefuseAll, Request};
pub use hub::{Block, Config, Feeder, Gap, Hub};
pub use listen::{allowed, BadCidr, Cidr, Listen, ListenerInfo};
pub use outlet::{time_ns, write_patient, ConnOutlet, FrameInfo, Framer, Outlet, OutletError};
pub use protocol::{Context, Parser, Protocol, Session};
pub use slice::{Slice, SliceError};
pub use stats::{ClientStats, Event, Leave, ListenerStats, Refusal, Snapshot};
pub use tx::{tx_ring, TxCounts, TxReader, TxRequest, TxWriter};

pub use sdrkit_graph::stream::SampleType;

/// Why a hub or a listener could not start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ServeError {
    /// The hub does not carry this sample type.
    #[error("a hub does not carry samples of type {0:?}")]
    Format(SampleType),
    /// The transport could not listen.
    #[error(transparent)]
    Net(#[from] sdrkit_core::net::NetError),
    /// A thread could not start.
    #[error("a thread could not start")]
    Thread,
}
