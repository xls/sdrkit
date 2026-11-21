//! The client of the amateur protocols: APRS messages and beacons under the callsign of the station, as LoRa APRS frames
//! or as AX.25 UI frames on FM. Each transmission is one frame; the transmit scheduler sends it and reports back. On both
//! links a message goes again until its addressee answers, and a numbered message to the station is acknowledged.
//!
//! Standard: APRS 1.0.1 chapter 14 (messages), AX.25 2.2 (UI frames).

use super::types::{AfskTx, Delivery, Failure, MeshText, Protocol, TxChannel, TxKind, TxRequest};
use crate::aprs::station::{beacon_info, message_info};
use crate::proto::hamlora;
use sdrkit_proto_link::ax25::{Address, Frame};
use sdrkit_util_geo::Position;
use std::collections::BTreeMap;

/// The first message key and request id of the client. The other clients use smaller numbers.
const FIRST_KEY: u64 = 1 << 41;

/// The transmissions of a message at most. The wait after transmission `n` (from 1) is 30 * 2^(n-1) s.
pub const MAX_TRANSMISSIONS: u32 = 5;

/// A numbered message heard again within this time is not acknowledged again, s.
const ACK_HOLD_S: f64 = 30.0;

/// The own messages that wait for an answer, at most; also the failed messages that a late answer still delivers.
const OPEN_CAP: usize = 100;

/// Where own frames go: a LoRa APRS channel, or an FM channel for AX.25.
#[derive(Clone, Debug, PartialEq)]
pub enum HamLink {
    /// A LoRa APRS channel.
    Lora(TxChannel),
    /// An FM channel for AX.25 frames, at this carrier, Hz.
    Afsk {
        /// The carrier, Hz.
        freq_hz: f64,
    },
}

impl HamLink {
    /// The carrier, Hz.
    pub fn freq_hz(&self) -> f64 {
        match self {
            HamLink::Lora(c) => c.freq_hz,
            HamLink::Afsk { freq_hz } => *freq_hz,
        }
    }
}

/// The settings of the client.
#[derive(Clone, Debug, PartialEq)]
pub struct HamConfig {
    /// The transmitter is on. Nothing is sent while it is off.
    pub tx_enabled: bool,
    /// Where own frames go. `None`: the session listens to no channel to send on, and nothing is sent.
    pub link: Option<HamLink>,
    /// The callsign of the station, upper case; empty when none.
    pub callsign: String,
    /// The SSID (0 for none).
    pub ssid: u8,
    /// The destination of own frames.
    pub tocall: String,
    /// The digipeater path of own frames, calls separated by commas.
    pub path: String,
    /// The symbol table and code of the beacon.
    pub symbol: String,
    /// The comment of the beacon.
    pub comment: String,
    /// The position of the station.
    pub position: Option<Position>,
    /// How AX.25 frames go on FM.
    pub afsk: AfskTx,
    /// The time between beacons, s; 0 sends a beacon only on request. At least 60 s when not 0.
    pub beacon_interval: f64,
}

impl Default for HamConfig {
    fn default() -> Self {
        HamConfig {
            tx_enabled: false,
            link: None,
            callsign: String::new(),
            ssid: 0,
            tocall: "APRS".into(),
            path: "WIDE1-1".into(),
            symbol: "/-".into(),
            comment: String::new(),
            position: None,
            afsk: AfskTx::default(),
            beacon_interval: 0.0,
        }
    }
}

impl HamConfig {
    /// The source call of own frames: the callsign, with `-SSID` when the SSID is not 0. Empty without a callsign.
    pub fn call(&self) -> String {
        match (self.callsign.is_empty(), self.ssid) {
            (true, _) => String::new(),
            (false, 0) => self.callsign.clone(),
            (false, s) => format!("{}-{s}", self.callsign),
        }
    }
}

/// Returns true if `call` is a valid AX.25 address (a call of 1 to 6 letters and digits, an SSID of 0 to 15).
pub fn ax25_ok(call: &str) -> bool {
    !call.contains('*') && Address::parse(call).is_ok()
}

/// An own message: its row in the messages of the stack.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnMessage {
    /// The key of the message in the client.
    pub key: u64,
    /// Signal time of the command, s.
    pub time: f64,
    /// The addressee.
    pub to: String,
    /// The text.
    pub text: String,
    /// The message number.
    pub number: u32,
    /// The delivery state.
    pub state: Delivery,
    /// Transmissions so far.
    pub sent: u32,
}

impl OwnMessage {
    /// The message as a row of the messages: sent on `protocol` (LoRa APRS or APRS on AX.25) from `call`.
    pub fn mesh_text(&self, call: &str, protocol: Protocol) -> MeshText {
        MeshText {
            time: self.time,
            protocol,
            channel: protocol.name().into(),
            from: call.into(),
            from_name: call.into(),
            to: self.to.clone(),
            text: self.text.clone(),
            hops: -1,
            packet_id: self.number,
            own: true,
            state: Some(self.state),
            key: self.key,
            ..Default::default()
        }
    }
}

/// An own station as the beacon shows it, for the station table.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnStation {
    /// Signal time it went on the air, s.
    pub time: f64,
    /// The protocol.
    pub protocol: Protocol,
    /// The source call.
    pub call: String,
    /// The symbol table and code.
    pub symbol: String,
    /// The position, degrees.
    pub position: Option<(f64, f64)>,
    /// The comment.
    pub comment: String,
    /// The path.
    pub path: String,
    /// The APRS kind of the beacon ("Position" or "Status").
    pub kind: String,
}

/// What a receiver heard that concerns the client: an answer to an own message, or a numbered message to the station.
#[derive(Clone, Debug, PartialEq)]
pub enum HamHeard {
    /// `from` acknowledged (or rejected) own message `number`.
    Answer {
        /// The station that answers.
        from: String,
        /// The message number.
        number: u32,
        /// [`Delivery::Delivered`] or [`Delivery::Rejected`].
        state: Delivery,
    },
    /// `from` sent a message with number `number` to the station: it wants an acknowledgement.
    Message {
        /// Signal time, s.
        time: f64,
        /// The sender.
        from: String,
        /// The message number as sent (1 to 5 characters).
        number: String,
    },
}

/// What the client requests after a call.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HamOut {
    /// Frames to send.
    pub tx: Vec<TxRequest>,
    /// Own messages, new or with a new state.
    pub messages: Vec<OwnMessage>,
    /// Own stations, after their beacon went on the air.
    pub stations: Vec<OwnStation>,
    /// Log lines.
    pub logs: Vec<String>,
}

/// An own message the client still handles: it waits for the scheduler or for an answer.
#[derive(Clone, Debug)]
struct Open {
    msg: OwnMessage,
    info: String,
    /// The time of the next transmission; `None` while a transmission waits for the scheduler.
    next: Option<f64>,
}

/// The client.
#[derive(Clone, Debug)]
pub struct HamClient {
    /// The settings.
    pub cfg: HamConfig,
    next_key: u64,
    number: u32,
    open: BTreeMap<u64, Open>,
    /// Failed messages that went on the air, by key: an answer that comes later still delivers them.
    failed: BTreeMap<u64, OwnMessage>,
    beacons: BTreeMap<u64, OwnStation>,
    acked: BTreeMap<String, f64>,
    last_beacon: Option<f64>,
}

impl HamClient {
    /// A client. `seed` sets the first message number, so that two sessions seldom start with the same number.
    pub fn new(cfg: HamConfig, seed: u64) -> Self {
        HamClient {
            cfg,
            next_key: FIRST_KEY,
            number: (seed % 99_999) as u32,
            open: BTreeMap::new(),
            failed: BTreeMap::new(),
            beacons: BTreeMap::new(),
            acked: BTreeMap::new(),
            last_beacon: None,
        }
    }

    /// The name of the protocol of the link in log lines.
    fn label(&self) -> &'static str {
        match self.cfg.link {
            Some(HamLink::Lora(_)) => "LoRa APRS",
            _ => "APRS",
        }
    }

    /// Why the client sends nothing now, or `None` while it sends.
    fn held(&self) -> Option<Failure> {
        if !self.cfg.tx_enabled {
            Some(Failure::TransmitOff)
        } else if self.cfg.link.is_none() {
            Some(Failure::NoChannel)
        } else {
            None
        }
    }

    /// The frame of information field `info` on the link, as a request with id `id` and message key `key`. `None` (with a
    /// log line in `o`) without a link, or if an address is not an AX.25 address.
    fn request(
        &self,
        info: &str,
        what: TxKind,
        id: u64,
        key: Option<u64>,
        now: f64,
        o: &mut HamOut,
    ) -> Option<TxRequest> {
        let c = &self.cfg;
        let link = c.link.as_ref()?;
        let (payload, lora, afsk) = match link {
            HamLink::Lora(ch) => {
                let tnc2 = hamlora::tnc2(&c.call(), &c.tocall, &c.path, info);
                (
                    hamlora::build_lora_aprs(&tnc2),
                    Some(ch.params.clone()),
                    None,
                )
            }
            HamLink::Afsk { .. } => match ax25_ui(&c.call(), &c.tocall, &c.path, info) {
                Some(b) => (b, None, Some(c.afsk)),
                None => {
                    o.logs.push(
                        "APRS: the call, the destination or the path is not an AX.25 address, nothing is sent"
                            .into(),
                    );
                    return None;
                }
            },
        };
        Some(TxRequest {
            id,
            payload,
            lora,
            afsk,
            freq_hz: link.freq_hz(),
            not_before: now,
            cw: 3,
            what,
            message_key: key,
        })
    }

    fn new_key(&mut self) -> u64 {
        let k = self.next_key;
        self.next_key += 1;
        k
    }

    /// The next message number, 1 to 99999.
    pub fn next_number(&mut self) -> u32 {
        self.number = self.number % 99_999 + 1;
        self.number
    }

    /// Sends message `text` to `to` as message number `number` at signal time `now`. The caller checked the callsign,
    /// the addressee and the text.
    pub fn send_message(&mut self, now: f64, to: &str, text: &str, number: u32) -> HamOut {
        let mut o = HamOut::default();
        let key = self.new_key();
        let held = self.held();
        let msg = OwnMessage {
            key,
            time: now,
            to: to.into(),
            text: text.into(),
            number,
            state: held.map_or(Delivery::Queued, Delivery::Failed),
            sent: 0,
        };
        o.messages.push(msg.clone());
        if let Some(f) = held {
            o.logs.push(format!("{}: {}", self.label(), f.text()));
            return o;
        }
        let info = message_info(to, text, number);
        let Some(r) = self.request(&info, TxKind::Text, key, Some(key), now, &mut o) else {
            return o;
        };
        o.tx.push(r);
        if self.open.len() >= OPEN_CAP {
            self.open.pop_first();
        }
        self.open.insert(
            key,
            Open {
                msg,
                info,
                next: None,
            },
        );
        o
    }

    /// Sends the beacon at signal time `now`: the position of the station, else the comment as a status. The caller
    /// checked the callsign.
    pub fn beacon(&mut self, now: f64) -> HamOut {
        let mut o = HamOut::default();
        self.last_beacon = Some(now);
        if let Some(f) = self.held() {
            o.logs.push(format!("{}: {}", self.label(), f.text()));
            return o;
        }
        let c = &self.cfg;
        let Some(info) = beacon_info(c.position.as_ref(), &c.symbol, &c.comment) else {
            o.logs.push(format!(
                "{}: the position of the station is out of range, nothing is sent",
                self.label()
            ));
            return o;
        };
        let st = OwnStation {
            time: now,
            protocol: Protocol::LoraAprs,
            call: c.call(),
            symbol: c.symbol.clone(),
            position: c.position.as_ref().map(|p| (p.lat_deg, p.lon_deg)),
            comment: c.comment.clone(),
            path: c.path.clone(),
            kind: if c.position.is_some() {
                "Position"
            } else {
                "Status"
            }
            .into(),
        };
        let key = self.new_key();
        if let Some(r) = self.request(&info, TxKind::Beacon, key, Some(key), now, &mut o) {
            o.tx.push(r);
            self.beacons.insert(key, st);
        }
        o
    }

    /// Periodic work at signal time `now`: messages without an answer go again, or fail after the last
    /// transmission, and the beacon of the interval goes. A beacon of the interval is left out while the client cannot
    /// send or `call_ok` refuses the call.
    pub fn tick(&mut self, now: f64, call_ok: bool) -> HamOut {
        let mut o = HamOut::default();
        let due: Vec<u64> = self
            .open
            .iter()
            .filter(|(_, m)| m.next.is_some_and(|t| now >= t))
            .map(|(k, _)| *k)
            .collect();
        for k in due {
            let Some(m) = self.open.get(&k) else {
                continue;
            };
            if m.msg.sent >= MAX_TRANSMISSIONS || self.held().is_some() {
                let reason = self.held().unwrap_or(Failure::NoAck);
                if let Some(mut m) = self.open.remove(&k) {
                    m.msg.state = Delivery::Failed(reason);
                    self.keep_failed(&m.msg);
                    o.messages.push(m.msg);
                }
                continue;
            }
            let info = m.info.clone();
            if let Some(r) = self.request(&info, TxKind::Text, k, Some(k), now, &mut o) {
                o.tx.push(r);
                if let Some(m) = self.open.get_mut(&k) {
                    m.next = None;
                }
            }
        }
        let iv = self.cfg.beacon_interval;
        if iv > 0.0 && self.last_beacon.is_none_or(|l| now - l >= iv.max(60.0)) {
            if call_ok && self.held().is_none() {
                let b = self.beacon(now);
                o.tx.extend(b.tx);
                o.logs.extend(b.logs);
            }
            self.last_beacon = Some(now);
        }
        o
    }

    /// The scheduler sent the frame of message key `key` at `at`: the message turns to sent and its next transmission is
    /// set, the beacon shows as the own station.
    pub fn on_sent(&mut self, at: f64, key: Option<u64>) -> HamOut {
        let mut o = HamOut::default();
        let Some(k) = key else {
            return o;
        };
        if let Some(mut s) = self.beacons.remove(&k) {
            s.time = at;
            o.stations.push(s);
            return o;
        }
        let Some(m) = self.open.get_mut(&k) else {
            return o;
        };
        m.msg.sent += 1;
        if !m.msg.state.done() {
            m.msg.state = Delivery::Sent;
        }
        o.messages.push(m.msg.clone());
        m.next = Some(at + 30.0 * 2f64.powi(m.msg.sent.saturating_sub(1) as i32));
        o
    }

    /// The scheduler holds or failed the frame of message key `key`.
    pub fn on_state(&mut self, key: Option<u64>, state: Delivery) -> HamOut {
        let mut o = HamOut::default();
        let Some(k) = key else {
            return o;
        };
        if self.beacons.contains_key(&k) {
            if let Delivery::Failed(f) = state {
                self.beacons.remove(&k);
                o.logs
                    .push(format!("{}: beacon not sent: {}", self.label(), f.text()));
            }
            return o;
        }
        let Some(m) = self.open.get_mut(&k) else {
            return o;
        };
        m.msg.state = state;
        o.messages.push(m.msg.clone());
        if matches!(state, Delivery::Failed(_)) {
            if let Some(m) = self.open.remove(&k) {
                self.keep_failed(&m.msg);
            }
        }
        o
    }

    /// Keeps the failed message `m` for a late answer if it went on the air.
    fn keep_failed(&mut self, m: &OwnMessage) {
        if m.sent == 0 {
            return;
        }
        if self.failed.len() >= OPEN_CAP {
            self.failed.pop_first();
        }
        self.failed.insert(m.key, m.clone());
    }

    /// What a receiver heard at signal time `now`: an answer ends an own message, also one that failed after it went on
    /// the air (an acknowledgement proves delivery whenever it comes); a numbered message to the station gets an
    /// acknowledgement (once in 30 s for the same sender and number) while the client can send.
    pub fn heard(&mut self, now: f64, h: &HamHeard) -> HamOut {
        let mut o = HamOut::default();
        match h {
            HamHeard::Answer {
                from,
                number,
                state,
            } => {
                let answers =
                    |m: &OwnMessage| m.number == *number && m.to.eq_ignore_ascii_case(from);
                let open = self
                    .open
                    .iter()
                    .find(|(_, m)| answers(&m.msg))
                    .map(|(k, _)| *k);
                let msg = match open.and_then(|k| self.open.remove(&k)) {
                    Some(m) => Some(m.msg),
                    None => self
                        .failed
                        .iter()
                        .find(|(_, m)| answers(m))
                        .map(|(k, _)| *k)
                        .and_then(|k| self.failed.remove(&k)),
                };
                if let Some(mut m) = msg {
                    m.state = *state;
                    o.logs.push(format!(
                        "{}: message {number} to {from} {}",
                        self.label(),
                        state.text()
                    ));
                    o.messages.push(m);
                }
            }
            HamHeard::Message { time, from, number } => {
                let id = format!("{from}/{number}");
                if self.acked.get(&id).is_some_and(|&t| time - t < ACK_HOLD_S) {
                    return o;
                }
                self.acked.insert(id, *time);
                if self.acked.len() > 1000 {
                    self.acked.retain(|_, t| time - *t < ACK_HOLD_S);
                }
                if self.held().is_some() {
                    return o;
                }
                let info = format!(":{from:<9}:ack{number}");
                let id = self.new_key();
                if let Some(r) = self.request(&info, TxKind::Ack, id, None, now, &mut o) {
                    o.tx.push(r);
                }
            }
        }
        o
    }
}

/// The bytes (without the FCS) of an AX.25 UI frame from `source` to `dest` over `path` (calls separated by commas) with
/// information field `info`. `None` if an address is not an AX.25 address or the path has more than eight calls.
pub fn ax25_ui(source: &str, dest: &str, path: &str, info: &str) -> Option<Vec<u8>> {
    let src = Address::parse(source).ok()?;
    let dst = Address::parse(dest).ok()?;
    let reps: Vec<Address> = path
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| Address::parse(s).ok())
        .collect::<Option<_>>()?;
    if reps.len() > 8 {
        return None;
    }
    let mut out = Vec::new();
    Frame::ui(dst, src, &reps, info.as_bytes())
        .and_then(|f| f.encode(&mut out))
        .ok()?;
    Some(out)
}
