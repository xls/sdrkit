//! The ARQ link of the `ardop` stack: a state machine that takes the frames heard and the time of the stream, and gives the
//! frames to send, the data received and the changes of state. One data frame type (4FSK.500.100), no change of speed and no
//! choice of bandwidth.

use super::tx::session_id;
use super::{ArdopFrame, RsCodes, TxFrame};
use crate::typed::ArdopLinkState;
use std::collections::VecDeque;

/// The time a frame waits for its answer, seconds, from the time it is handed to the sender.
pub const ANSWER_S: f64 = 9.0;
/// The time between two IDLE frames of a link with nothing to send, seconds.
pub const IDLE_S: f64 = 4.0;
/// The repeats of a frame without an answer after which the link fails.
pub const RETRIES: u32 = 10;
/// The data frame type of the link (the even form; the odd form is one more).
pub const DATA: u8 = 0x4A;
/// The data bytes of one data frame.
pub const DATA_BYTES: usize = 64;
/// The most data bytes that may wait to be sent.
pub const MAX_WAITING: usize = 16_384;
/// The connect request: ConReq500M.
pub const CONREQ: u8 = 0x32;
/// The acknowledgement of a connect request: ConAck500.
pub const CONACK: u8 = 0x3A;
/// DataACK, quality 70.
pub const ACK: u8 = 0xF0;
/// DataNAK, quality 70.
pub const NAK: u8 = 0x10;
/// IDLE.
pub const IDLE: u8 = 0x24;
/// BREAK.
pub const BREAK: u8 = 0x23;
/// DISC.
pub const DISC: u8 = 0x29;
/// END.
pub const END: u8 = 0x2C;

/// A frame heard, as the link reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Heard {
    /// The type byte.
    pub code: u8,
    /// The session id.
    pub session: u8,
    /// The check symbols of the header agree.
    pub header_ok: bool,
    /// The body checked.
    pub body_ok: bool,
    /// The sender, if the frame names one.
    pub from: Option<String>,
    /// The addressee, if the frame names one.
    pub to: Option<String>,
    /// The data of a good data frame.
    pub data: Vec<u8>,
}

impl From<&ArdopFrame> for Heard {
    fn from(f: &ArdopFrame) -> Self {
        Heard {
            code: f.code,
            session: f.session,
            header_ok: f.header_ok,
            body_ok: f.body_ok,
            from: f.from.clone(),
            to: f.to.clone(),
            data: f.bytes.clone(),
        }
    }
}

/// What the link asks of the sender.
#[derive(Clone, Debug, PartialEq)]
pub enum LinkOut {
    /// Send this frame now, in a burst of its own.
    Send(TxFrame),
    /// Data received from the other station, in order and once.
    Deliver(Vec<u8>),
    /// The link state changed, with the reason.
    State(ArdopLinkState, String),
    /// A connect request to the station callsign from this station was not answered.
    Request(String),
}

/// Why the link refuses a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkError {
    /// A link is up or being made.
    Busy,
    /// No link is up.
    NoLink,
    /// More than [`MAX_WAITING`] bytes would wait.
    Long,
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LinkError::Busy => "a link is up",
            LinkError::NoLink => "no link is up",
            LinkError::Long => "more than 16384 bytes would wait",
        })
    }
}

impl std::error::Error for LinkError {}

/// The frame that waits for its answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wait {
    /// Nothing.
    None,
    /// The connect request.
    ConReq,
    /// A data frame of this many bytes.
    Data(usize),
    /// IDLE.
    Idle,
    /// BREAK.
    Break,
    /// DISC.
    Disc,
}

/// The link. Both sides follow one table: the caller (`connect`) and, when answering is on, the called station. "Sending"
/// is the side that holds the turn to send data, "receiving" the other.
pub struct Link {
    own: String,
    peer: String,
    session: u8,
    state: ArdopLinkState,
    reason: String,
    leader_ms: u32,
    answer: bool,
    queue: VecDeque<u8>,
    sent: u64,
    received: u64,
    retries: u32,
    wait: Wait,
    last: Option<TxFrame>,
    deadline: Option<f64>,
    idle_at: Option<f64>,
    heard_at: f64,
    odd: bool,
    rx_last: Option<bool>,
    want_disc: bool,
    /// Until when a repeated DISC of the other station is answered with END again, after the stack answered its DISC.
    end_until: Option<f64>,
    rs: RsCodes,
}

impl Default for Link {
    fn default() -> Self {
        Self::new()
    }
}

impl Link {
    /// No link, answering off, a leader of 240 ms.
    pub fn new() -> Self {
        Link {
            own: String::new(),
            peer: String::new(),
            session: 0xFF,
            state: ArdopLinkState::Idle,
            reason: String::new(),
            leader_ms: 240,
            answer: false,
            queue: VecDeque::new(),
            sent: 0,
            received: 0,
            retries: 0,
            wait: Wait::None,
            last: None,
            deadline: None,
            idle_at: None,
            heard_at: 0.0,
            odd: false,
            rx_last: None,
            want_disc: false,
            end_until: None,
            rs: RsCodes::new(),
        }
    }

    /// Sets the station callsign, the leader of the frames sent and whether a connect request to the station callsign is
    /// answered.
    pub fn set(&mut self, own: &str, leader_ms: u32, answer: bool) {
        if !self.active() {
            own.trim().to_ascii_uppercase().clone_into(&mut self.own);
        }
        self.leader_ms = leader_ms;
        self.answer = answer;
    }

    /// The state.
    pub fn state(&self) -> ArdopLinkState {
        self.state
    }
    /// Why the state is what it is.
    pub fn reason(&self) -> &str {
        &self.reason
    }
    /// Whether a link is up or being made or ended.
    pub fn active(&self) -> bool {
        matches!(
            self.state,
            ArdopLinkState::Connecting
                | ArdopLinkState::Sending
                | ArdopLinkState::Receiving
                | ArdopLinkState::Disconnecting
        )
    }
    /// The other station, once a link was made.
    pub fn peer(&self) -> Option<&str> {
        (!self.peer.is_empty()).then_some(self.peer.as_str())
    }
    /// The session id, once a link was made.
    pub fn session(&self) -> Option<u8> {
        (!self.peer.is_empty()).then_some(self.session)
    }
    /// The data bytes acknowledged.
    pub fn sent(&self) -> u64 {
        self.sent
    }
    /// The data bytes received.
    pub fn received(&self) -> u64 {
        self.received
    }
    /// The data bytes waiting.
    pub fn waiting(&self) -> usize {
        self.queue.len()
    }
    /// The repeats in a row of the frame that waits for its answer.
    pub fn retries(&self) -> u32 {
        self.retries
    }

    fn step(&mut self, s: ArdopLinkState, reason: &str, out: &mut Vec<LinkOut>) {
        self.state = s;
        reason.clone_into(&mut self.reason);
        out.push(LinkOut::State(s, reason.to_owned()));
    }

    /// Sends `f`, which waits for an answer as `wait`.
    fn send(&mut self, f: TxFrame, wait: Wait, now: f64, out: &mut Vec<LinkOut>) {
        let f = f.with_leader(self.leader_ms);
        self.wait = wait;
        self.deadline = (wait != Wait::None).then_some(now + ANSWER_S);
        self.last = Some(f.clone());
        out.push(LinkOut::Send(f));
    }

    /// Sends an answer, which waits for nothing.
    fn reply(&mut self, code: u8, out: &mut Vec<LinkOut>) {
        out.push(LinkOut::Send(
            TxFrame::bare(code, self.session).with_leader(self.leader_ms),
        ));
    }

    fn bare(&self, code: u8) -> TxFrame {
        TxFrame::bare(code, self.session)
    }

    /// A new link with `peer`, from the start.
    fn open(&mut self, peer: &str, caller: &str, called: &str) {
        self.peer = peer.to_owned();
        self.session = session_id(caller, called);
        self.queue.clear();
        self.sent = 0;
        self.received = 0;
        self.retries = 0;
        self.wait = Wait::None;
        self.last = None;
        self.deadline = None;
        self.idle_at = None;
        self.odd = false;
        self.rx_last = None;
        self.want_disc = false;
        self.end_until = None;
    }

    /// Calls `peer` at `now`: the connect request, again every [`ANSWER_S`] until it is answered.
    ///
    /// # Errors
    ///
    /// [`LinkError::Busy`] while a link is up or being made.
    pub fn connect(
        &mut self,
        peer: &str,
        now: f64,
        out: &mut Vec<LinkOut>,
    ) -> Result<(), LinkError> {
        if self.active() {
            return Err(LinkError::Busy);
        }
        let peer = peer.trim().to_ascii_uppercase();
        let own = self.own.clone();
        self.open(&peer, &own, &peer);
        self.heard_at = now;
        self.step(ArdopLinkState::Connecting, "", out);
        let f = TxFrame::two_fields(&mut self.rs, CONREQ, &own, &peer);
        self.send(f, Wait::ConReq, now, out);
        Ok(())
    }

    /// Queues `data` to send on the link.
    ///
    /// # Errors
    ///
    /// [`LinkError::NoLink`] without a link, [`LinkError::Long`] when too much would wait.
    pub fn queue(
        &mut self,
        data: &[u8],
        now: f64,
        out: &mut Vec<LinkOut>,
    ) -> Result<(), LinkError> {
        if !self.active() || self.state == ArdopLinkState::Disconnecting {
            return Err(LinkError::NoLink);
        }
        if self.queue.len() + data.len() > MAX_WAITING {
            return Err(LinkError::Long);
        }
        self.queue.extend(data.iter().copied());
        // A sending side that waits between two IDLE frames sends at once.
        if self.state == ArdopLinkState::Sending && self.wait == Wait::None {
            self.next(now, out);
        }
        Ok(())
    }

    /// Ends the link once the data waiting is sent. Before the answer to the connect request it ends at once.
    pub fn disconnect(&mut self, now: f64, out: &mut Vec<LinkOut>) {
        match self.state {
            ArdopLinkState::Connecting => self.close(
                ArdopLinkState::Closed,
                "disconnected before the answer",
                out,
            ),
            ArdopLinkState::Sending => {
                self.want_disc = true;
                if self.wait == Wait::None {
                    self.next(now, out);
                }
            }
            ArdopLinkState::Receiving => self.want_disc = true,
            _ => {}
        }
    }

    /// Ends the link at once, without DISC.
    pub fn abort(&mut self, reason: &str, out: &mut Vec<LinkOut>) {
        if self.active() {
            self.close(ArdopLinkState::Closed, reason, out);
        }
    }

    fn close(&mut self, s: ArdopLinkState, reason: &str, out: &mut Vec<LinkOut>) {
        self.end_until = None;
        self.wait = Wait::None;
        self.deadline = None;
        self.idle_at = None;
        self.last = None;
        self.queue.clear();
        self.want_disc = false;
        self.step(s, reason, out);
    }

    /// The sending side's next frame: data, DISC when the end is asked for and nothing waits, else IDLE.
    fn next(&mut self, now: f64, out: &mut Vec<LinkOut>) {
        self.idle_at = None;
        if !self.queue.is_empty() {
            let n = self.queue.len().min(DATA_BYTES);
            let bytes: Vec<u8> = self.queue.iter().take(n).copied().collect();
            let code = DATA | u8::from(self.odd);
            let f = TxFrame::data_bytes(&mut self.rs, code, self.session, &bytes);
            self.send(f, Wait::Data(n), now, out);
        } else if self.want_disc {
            self.step(ArdopLinkState::Disconnecting, "", out);
            let f = self.bare(DISC);
            self.send(f, Wait::Disc, now, out);
        } else {
            let f = self.bare(IDLE);
            self.send(f, Wait::Idle, now, out);
        }
    }

    /// The frame that waits for its answer, sent again; the link fails after [`RETRIES`] repeats in a row.
    fn again(&mut self, now: f64, out: &mut Vec<LinkOut>) {
        if self.retries >= RETRIES {
            if self.state == ArdopLinkState::Disconnecting {
                self.close(ArdopLinkState::Closed, "no answer to DISC", out);
            } else {
                self.close(
                    ArdopLinkState::Failed,
                    "no answer from the other station",
                    out,
                );
            }
            return;
        }
        self.retries += 1;
        if let Some(f) = self.last.clone() {
            let wait = self.wait;
            self.send(f, wait, now, out);
        }
    }

    /// A data frame heard by the receiving side: NAK when its body is bad; else ACK, and its data once (a frame whose
    /// even/odd toggle equals the last one taken is a repeat).
    fn take_data(&mut self, h: &Heard, out: &mut Vec<LinkOut>) {
        if !h.body_ok {
            self.reply(NAK, out);
            return;
        }
        let odd = h.code & 1 == 1;
        if self.rx_last != Some(odd) {
            self.rx_last = Some(odd);
            self.received += h.data.len() as u64;
            if !h.data.is_empty() {
                out.push(LinkOut::Deliver(h.data.clone()));
            }
        }
        self.reply(ACK, out);
    }

    /// A frame heard at `now`.
    pub fn heard(&mut self, h: &Heard, now: f64, out: &mut Vec<LinkOut>) {
        if !h.header_ok {
            return;
        }
        let code = h.code;
        if (0x31..=0x38).contains(&code) {
            self.connect_request(h, now, out);
            return;
        }
        if h.session != self.session || self.peer.is_empty() {
            return;
        }
        if !self.active() {
            // The END to the other station's DISC was lost: it sends DISC again while it waits for END.
            if code == DISC && self.end_until.is_some_and(|t| now <= t) {
                self.reply(END, out);
            }
            return;
        }
        self.heard_at = now;
        if code == DISC {
            self.reply(END, out);
            self.close(
                ArdopLinkState::Closed,
                "the other station disconnected",
                out,
            );
            self.end_until = Some(now + f64::from(RETRIES + 1) * ANSWER_S);
            return;
        }
        let data = (code & !1) == DATA;
        let ack = code >= 0xE0;
        let nak = code <= 0x1F;
        match self.state {
            ArdopLinkState::Connecting => {
                if (0x39..=0x3C).contains(&code) {
                    self.retries = 0;
                    self.step(ArdopLinkState::Sending, "connected", out);
                    self.next(now, out);
                }
            }
            ArdopLinkState::Sending => {
                if ack {
                    match self.wait {
                        Wait::Data(n) => {
                            self.queue.drain(..n.min(self.queue.len()));
                            self.sent += n as u64;
                            self.odd = !self.odd;
                            self.retries = 0;
                            self.next(now, out);
                        }
                        Wait::Idle => {
                            self.retries = 0;
                            if self.queue.is_empty() && !self.want_disc {
                                self.wait = Wait::None;
                                self.deadline = None;
                                self.idle_at = Some(now + IDLE_S);
                            } else {
                                self.next(now, out);
                            }
                        }
                        _ => {}
                    }
                } else if nak {
                    if matches!(self.wait, Wait::Data(_) | Wait::Idle) {
                        self.again(now, out);
                    }
                } else if code == BREAK {
                    if let Wait::Data(_) = self.wait {
                        // The data frame is not acknowledged yet: it goes again and the BREAK waits.
                        if let Some(f) = self.last.clone() {
                            let wait = self.wait;
                            self.send(f, wait, now, out);
                        }
                    } else {
                        self.wait = Wait::None;
                        self.deadline = None;
                        self.idle_at = None;
                        self.step(
                            ArdopLinkState::Receiving,
                            "the other station took the turn",
                            out,
                        );
                        self.reply(ACK, out);
                    }
                } else if data {
                    self.wait = Wait::None;
                    self.deadline = None;
                    self.idle_at = None;
                    self.step(
                        ArdopLinkState::Receiving,
                        "the other station took the turn",
                        out,
                    );
                    self.take_data(h, out);
                }
            }
            ArdopLinkState::Receiving => {
                if data {
                    if self.wait == Wait::Break {
                        self.wait = Wait::None;
                        self.deadline = None;
                    }
                    self.take_data(h, out);
                } else if code == IDLE {
                    if !self.queue.is_empty() {
                        let f = self.bare(BREAK);
                        self.send(f, Wait::Break, now, out);
                    } else if self.want_disc {
                        self.step(ArdopLinkState::Disconnecting, "", out);
                        let f = self.bare(DISC);
                        self.send(f, Wait::Disc, now, out);
                    } else {
                        self.reply(ACK, out);
                    }
                } else if code == BREAK {
                    // The earlier ACK was lost.
                    self.reply(ACK, out);
                } else if ack && self.wait == Wait::Break {
                    self.wait = Wait::None;
                    self.deadline = None;
                    self.retries = 0;
                    self.step(ArdopLinkState::Sending, "the stack took the turn", out);
                    self.next(now, out);
                }
            }
            ArdopLinkState::Disconnecting => {
                if code == END {
                    self.close(ArdopLinkState::Closed, "disconnected", out);
                } else if let Some(f) = self.last.clone() {
                    self.send(f, Wait::Disc, now, out);
                }
            }
            _ => {}
        }
    }

    /// A connect request heard: to the station callsign and not in a link, it is answered when answering is on and else
    /// reported; from the station of the link while receiving, the acknowledgement is sent again.
    fn connect_request(&mut self, h: &Heard, now: f64, out: &mut Vec<LinkOut>) {
        let (Some(from), Some(to)) = (&h.from, &h.to) else {
            return;
        };
        if !h.body_ok || self.own.is_empty() || !to.trim().eq_ignore_ascii_case(&self.own) {
            return;
        }
        let from = from.trim().to_ascii_uppercase();
        if self.state == ArdopLinkState::Receiving && from == self.peer {
            self.heard_at = now;
            out.push(LinkOut::Send(
                TxFrame::con_ack(CONACK, self.leader_ms, self.session).with_leader(self.leader_ms),
            ));
            return;
        }
        if self.active() {
            return;
        }
        if !self.answer {
            out.push(LinkOut::Request(from));
            return;
        }
        let own = self.own.clone();
        self.open(&from, &from, &own);
        self.heard_at = now;
        out.push(LinkOut::Send(
            TxFrame::con_ack(CONACK, self.leader_ms, self.session).with_leader(self.leader_ms),
        ));
        self.step(ArdopLinkState::Receiving, "connected", out);
    }

    /// The time is `now`: a frame whose answer is late goes again, IDLE goes after [`IDLE_S`] of nothing to send, and a
    /// receiving side that hears nothing for 11 answer times fails.
    pub fn tick(&mut self, now: f64, out: &mut Vec<LinkOut>) {
        if self.deadline.is_some_and(|d| now >= d) {
            self.deadline = None;
            self.again(now, out);
            return;
        }
        if self.idle_at.is_some_and(|t| now >= t) {
            self.idle_at = None;
            self.next(now, out);
            return;
        }
        let quiet = f64::from(RETRIES + 1) * ANSWER_S;
        if self.state == ArdopLinkState::Receiving
            && self.wait == Wait::None
            && now - self.heard_at > quiet
        {
            self.close(
                ArdopLinkState::Failed,
                "no answer from the other station",
                out,
            );
        }
    }
}
