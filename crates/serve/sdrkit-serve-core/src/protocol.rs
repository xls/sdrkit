//! What a protocol crate gives the core: for each new client a [`Session`] with the bytes it gets first, the outlet of its
//! stream if the stream starts at once, and a [`Parser`] of the bytes it sends. The parser works through a [`Context`]:
//! requests to the owner, replies on the connection, a hang-up, and a transmit ring.

use crate::client::ClientHandle;
use crate::control::{Answer, Control, Request};
use crate::format::supported;
use crate::outlet::{write_patient, Outlet};
use crate::tx::{tx_ring, TxRequest, TxWriter};
use sdrkit_core::net::Conn;
use sdrkit_graph::stream::SampleType;

/// A network protocol that serves raw I/Q.
pub trait Protocol: Send + Sync {
    /// A short name (`rtl_tcp`).
    fn name(&self) -> &'static str;
    /// The session of a new client on `conn`, while the source delivers `source`.
    fn session(&self, conn: &dyn Conn, source: SampleType) -> Session;
}

/// One client's side of a protocol.
pub struct Session {
    /// Sent before anything else.
    pub greeting: Vec<u8>,
    /// The stream from the first chunk on, or `None` until the parser sets one with [`ClientHandle::set_outlet`].
    pub outlet: Option<Box<dyn Outlet>>,
    /// Reads the client's bytes.
    pub parser: Box<dyn Parser>,
}

/// Turns the bytes a client sends into requests and replies. It keeps a partial command between calls.
pub trait Parser: Send {
    /// Takes the next `bytes` from the client, in any split.
    fn feed(&mut self, bytes: &[u8], cx: &mut Context<'_>);
    /// The client is leaving: stop what the session started.
    fn closed(&mut self, cx: &mut Context<'_>) {
        let _ = cx;
    }
}

/// What a parser can do for its client. It runs on the client's reader thread.
pub struct Context<'a> {
    pub(crate) handle: &'a ClientHandle,
    pub(crate) control: &'a dyn Control,
    pub(crate) conn: &'a mut dyn Conn,
    pub(crate) tx_queue_ms: u32,
}

impl Context<'_> {
    /// The client.
    pub fn client(&self) -> &ClientHandle {
        self.handle
    }

    /// Hands `request` to the owner's [`Control`] and returns its answer. Requests and refusals are counted.
    pub fn request(&mut self, request: Request) -> Answer {
        let a = self.control.request(self.handle, &request);
        self.handle.count_request(a == Answer::Refused);
        self.handle.note_answer(&request, &a);
        a
    }

    /// The value in force of the setting that `request` names, as the owner's [`Control::in_force`] says. It is not a
    /// request and is not counted.
    pub fn in_force(&self, request: Request) -> Option<Request> {
        self.control.in_force(self.handle, &request)
    }

    /// Writes `bytes` to the client's connection. A protocol whose stream shares the connection must not reply, because the
    /// writer thread writes the stream there. False if the connection failed or the client is stopping.
    pub fn reply(&mut self, bytes: &[u8]) -> bool {
        let h = self.handle;
        write_patient(self.conn, bytes, &mut || h.stopping()).is_ok()
    }

    /// Counts the connection as a client from now on, for a request that the protocol answers without the owner (a
    /// setting of the transmit side).
    pub fn engage(&mut self) {
        self.handle.engage();
    }

    /// Ends the client after the present call.
    pub fn hang_up(&mut self) {
        self.handle.hang_up();
    }

    /// Asks the owner for a transmit ring of `request`, holding `tx_queue_ms` of samples. The owner's
    /// [`Control::transmit`] decides; on [`Answer::Accepted`] or [`Answer::Adjusted`] the protocol gets the writer.
    ///
    /// # Errors
    ///
    /// The answer if the owner refused, or [`Answer::Refused`] for a format the core does not carry.
    pub fn transmit(&mut self, request: TxRequest) -> Result<TxWriter, Answer> {
        if !supported(request.format) || request.rate_hz.is_nan() || request.rate_hz <= 0.0 {
            self.handle.count_request(true);
            return Err(Answer::Refused);
        }
        let samples = (f64::from(self.tx_queue_ms) / 1000.0 * request.rate_hz).ceil() as usize;
        let (w, r) = tx_ring(request, samples.max(64));
        let a = self.control.transmit(self.handle, &request, r);
        self.handle.count_request(a == Answer::Refused);
        match a {
            Answer::Accepted | Answer::Adjusted(_) => Ok(w),
            Answer::Refused => Err(a),
        }
    }
}
