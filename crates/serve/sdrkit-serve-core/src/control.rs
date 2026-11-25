//! Requests that a client makes of the radio, and the [`Control`] through which a protocol hands them to the owner of the
//! share. The owner decides the policy (follow the host, let one client steer, or give each client its own slice); the core
//! only carries the request and counts the answer.

use crate::client::ClientHandle;
use crate::tx::{TxReader, TxRequest};

/// A request of a client, in the units of the radio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Request {
    /// Centre frequency, Hz.
    Frequency {
        /// The frequency.
        hz: f64,
    },
    /// Sample rate, samples per second.
    Rate {
        /// The rate.
        hz: f64,
    },
    /// Bandwidth of the radio's filter, Hz.
    Bandwidth {
        /// The bandwidth.
        hz: f64,
    },
    /// Manual gain, dB.
    Gain {
        /// The gain.
        db: f64,
    },
    /// Automatic gain of the radio on or off.
    Agc {
        /// True for automatic gain.
        on: bool,
    },
    /// The bias tee (power on the antenna input) on or off.
    BiasTee {
        /// True to switch it on.
        on: bool,
    },
    /// Frequency correction, parts per million.
    Correction {
        /// The correction.
        ppm: f64,
    },
    /// A request that only the protocol names: its name and up to two arguments.
    Other {
        /// The protocol's name of the request (`direct_sampling`).
        name: &'static str,
        /// The arguments, unused ones 0.
        args: [i64; 2],
    },
}

/// The answer to a [`Request`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Answer {
    /// Done as asked.
    Accepted,
    /// Not done; nothing changed.
    Refused,
    /// Done with other values: the request as granted.
    Adjusted(Request),
}

/// Takes the requests of the clients of a share. It is called on the reader thread of each client, so several calls can run
/// at once. It must not wait long: the client's commands queue behind it.
pub trait Control: Send + Sync {
    /// Answers `request` of `client`.
    fn request(&self, client: &ClientHandle, request: &Request) -> Answer;

    /// Answers a request of `client` to transmit. To accept, the owner keeps `reader` and drains it into its transmit side,
    /// where its own transmit switch applies; to refuse, it drops `reader`. The default refuses.
    fn transmit(&self, client: &ClientHandle, request: &TxRequest, reader: TxReader) -> Answer {
        let _ = (client, request, reader);
        Answer::Refused
    }
}

/// A [`Control`] that refuses every request: the clients only listen.
#[derive(Clone, Copy, Debug, Default)]
pub struct RefuseAll;

impl Control for RefuseAll {
    fn request(&self, _client: &ClientHandle, _request: &Request) -> Answer {
        Answer::Refused
    }
}
