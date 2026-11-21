//! The Meshtastic texting client: the own node, channel and direct texts (channel keys or PKI), acknowledgements
//! with retries and delivery states, NodeInfo and position exchange, requests on other nodes, and the flooding rules of role CLIENT.

use super::decode::MtRx;
use super::types::{
    message_text, Delivery, Failure, MeshText, Protocol, TxChannel, TxKind, TxRequest,
};
use crate::proto::meshtastic::{self as mt, Data, Header, Position, Route, Routing, User};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_util_objects::{Dedup, ObjectTable, Stamp};
use std::collections::{BTreeMap, VecDeque};

/// The largest Data payload of a text.
pub const MAX_TEXT: usize = 233;
/// The dedupe window, s.
pub const DEDUP_S: f64 = 600.0;
/// The largest contention window of a node that repeats a packet, as a power of two of back-off slots. A node that
/// heard a packet well waits `2 * REPEAT_CW_MAX` slots and then up to `2^REPEAT_CW_MAX` more before it repeats it.
pub const REPEAT_CW_MAX: u32 = 8;
/// The messages that failed and are kept, so that a repeat or an acknowledgement that comes late still counts.
const LATE_CAP: usize = 16;

/// One own channel (5.12 `mt_channels`).
#[derive(Clone, Debug, PartialEq)]
pub struct OwnChannel {
    /// Name ("" = the preset's).
    pub name: String,
    /// Expanded key (empty: no encryption).
    pub key: Vec<u8>,
    /// Position precision bits (0..32).
    pub precision: u32,
}

sdrkit_core::choice_enum! {
    /// The role of the own node. The keys are the names of `Config.DeviceConfig.Role` in the Meshtastic protobufs.
    pub enum Role {
        /// Rebroadcasts.
        Client = ("CLIENT", "Client"),
        /// Never rebroadcasts.
        ClientMute = ("CLIENT_MUTE", "Client (mute)"),
        /// No NodeInfo or position unless requested.
        ClientHidden = ("CLIENT_HIDDEN", "Client (hidden)"),
    }
}

impl Role {
    /// The role number of `Config.DeviceConfig.Role`.
    pub fn number(self) -> u32 {
        match self {
            Role::Client => 0,
            Role::ClientMute => 1,
            Role::ClientHidden => 8,
        }
    }
}

sdrkit_core::choice_enum! {
    /// What the client can ask another node for.
    pub enum MtRequest {
        /// Its node information.
        NodeInfo = ("nodeinfo", "Node information"),
        /// Its position.
        Position = ("position", "Position"),
        /// The route to it.
        Traceroute = ("traceroute", "Route trace"),
        /// Its telemetry.
        Telemetry = ("telemetry", "Telemetry"),
    }
}

impl MtRequest {
    /// The kind of the frame that makes the request.
    pub fn kind(self) -> TxKind {
        match self {
            MtRequest::NodeInfo => TxKind::NodeInfo,
            MtRequest::Position => TxKind::Position,
            MtRequest::Traceroute => TxKind::Traceroute,
            MtRequest::Telemetry => TxKind::Telemetry,
        }
    }
}

/// The client's settings.
#[derive(Clone, Debug)]
pub struct MtConfig {
    /// Node number.
    pub node: u32,
    /// Long name (at most 39 bytes).
    pub long_name: String,
    /// Short name (at most 4 characters).
    pub short_name: String,
    /// Role.
    pub role: Role,
    /// X25519 private key.
    pub private_key: [u8; 32],
    /// Licensed mode.
    pub licensed: bool,
    /// Callsign.
    pub callsign: String,
    /// Put `<callsign>: ` in front of own texts.
    pub callsign_prefix: bool,
    /// Own channels. Index 0 is the primary.
    pub channels: Vec<OwnChannel>,
    /// Hop limit of own packets.
    pub hop_limit: u8,
    /// The transmitter is on. Nothing is sent while it is off.
    pub tx_enabled: bool,
    /// NodeInfo interval, s (0: only on request).
    pub nodeinfo_interval: f64,
    /// Fixed position (lat, lon, alt) when position sending is on.
    pub position: Option<(f64, f64, i32)>,
    /// Position from the host (not fixed).
    pub position_from_host: bool,
    /// Position interval, s.
    pub position_interval: f64,
    /// Want-ack on own channel texts.
    pub ack_broadcast: bool,
    /// The channel the client sends on: the primary channel while the session listens to it. `None`: nothing is sent.
    pub tx_channel: Option<TxChannel>,
    /// The host's clock at signal time 0 (Unix s).
    pub clock0: f64,
}

impl MtConfig {
    /// The defaults with a node number and a private key.
    pub fn new(node: u32, private_key: [u8; 32]) -> Self {
        MtConfig {
            node,
            long_name: "sdrkit".into(),
            short_name: "SDRK".into(),
            role: Role::ClientMute,
            private_key,
            licensed: false,
            callsign: String::new(),
            callsign_prefix: false,
            channels: vec![OwnChannel {
                name: "LongFast".into(),
                key: mt::DEFAULT_KEY.to_vec(),
                precision: 13,
            }],
            hop_limit: 3,
            tx_enabled: false,
            nodeinfo_interval: 10800.0,
            position: None,
            position_from_host: false,
            position_interval: 900.0,
            ack_broadcast: true,
            tx_channel: None,
            clock0: 0.0,
        }
    }
    /// The own public key.
    pub fn public_key(&self) -> [u8; 32] {
        mt::public_key(&self.private_key)
    }
}

/// A node number from the setting text, or made: a random one, or FNV-1a of the lower-case callsign.
pub fn node_number(text: &str, callsign: &str, rng: &mut Rng) -> u32 {
    if let Some(n) = mt::parse_node(text).filter(|&n| !reserved(n)) {
        return n;
    }
    if !callsign.is_empty() {
        let mut h: u32 = 0x811C_9DC5;
        for b in callsign.to_lowercase().bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(0x0100_0193);
        }
        if h < 0x100 {
            h += 0x1000;
        }
        if !reserved(h) {
            return h;
        }
    }
    loop {
        let n = rng.next_u32();
        if !reserved(n) {
            return n;
        }
    }
}

fn reserved(n: u32) -> bool {
    n <= 3 || n == mt::BROADCAST
}

/// The warnings of a configuration: shown, never a refusal.
pub fn warnings(
    long: &str,
    short: &str,
    node_text: &str,
    hop: f64,
    prefix: bool,
    callsign: &str,
) -> Vec<String> {
    let mut w = Vec::new();
    if short.chars().count() > 4 {
        w.push("the short name is longer than 4 characters (cut)".into());
    }
    if short.is_empty() {
        w.push("empty short name: other nodes will show the node ID".into());
    }
    if long.len() > 39 {
        w.push("the long name is longer than 39 bytes (cut)".into());
    }
    if long.is_empty() {
        w.push("the long name is empty".into());
    }
    if !node_text.is_empty()
        && mt::parse_node(node_text)
            .filter(|&n| !reserved(n))
            .is_none()
    {
        w.push("the node ID is not 8 hex digits or is reserved: another one is made".into());
    }
    if !(0.0..=7.0).contains(&hop) {
        w.push("the hop limit is outside 0..7 (limited)".into());
    } else if hop > 3.0 {
        w.push("a hop limit above 3 loads the whole mesh".into());
    }
    if prefix && callsign.is_empty() {
        w.push("the callsign prefix is ticked with no callsign".into());
    }
    w
}

#[derive(Clone, Debug)]
struct Outgoing {
    id: u32,
    bytes: Vec<u8>,
    want_ack: bool,
    dest: u32,
    tries: u32,
    deadline: Option<f64>,
    state: Delivery,
    relays: Vec<u8>,
    is_text: bool,
    channel: String,
    text: String,
    time: f64,
}

#[derive(Clone, Debug)]
struct Rebroadcast {
    sender: u32,
    id: u32,
    bytes: Vec<u8>,
    at: f64,
    req: u64,
}

/// What the client requests after a call.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientOut {
    /// Frames to send.
    pub tx: Vec<TxRequest>,
    /// Requests to drop from the scheduler (a cancelled rebroadcast).
    pub cancel: Vec<u64>,
    /// Own messages, new or with a new state.
    pub texts: Vec<MeshText>,
    /// Log lines.
    pub logs: Vec<String>,
}

/// The client's record of another node: its key and name (from NodeInfo), the learned next hop, and when NodeInfo was last
/// requested or answered.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Peer {
    /// The public key.
    pub key: Option<[u8; 32]>,
    /// The long name.
    pub name: Option<String>,
    /// The learned next hop: the relay byte of the copy with the fewest hops, and those hops.
    pub next_hop: Option<(u8, u32)>,
    /// When the own NodeInfo was last sent to it as an answer, signal time.
    pub replied_nodeinfo: Option<f64>,
    /// When its NodeInfo was last requested, signal time.
    pub asked_nodeinfo: Option<f64>,
}

/// Nodes the client keeps.
pub const PEERS_CAP: usize = 500;

/// The client.
#[derive(Clone, Debug)]
pub struct MtClient {
    /// The settings.
    pub cfg: MtConfig,
    rng: Rng,
    recent_ids: VecDeque<u32>,
    seen: Dedup<u64>,
    out: BTreeMap<u32, Outgoing>,
    /// The messages of `out` that failed for want of an answer, oldest first. At most `LATE_CAP`.
    late: VecDeque<u32>,
    rebroadcasts: Vec<Rebroadcast>,
    /// The nodes known to the client, by node number.
    pub peers: ObjectTable<u32, Peer>,
    last_nodeinfo: Option<f64>,
    last_position: Option<f64>,
    host_position: Option<(f64, f64, i32)>,
    next_req: u64,
    /// Packets rebroadcast.
    pub rebroadcast_count: u64,
    /// Acknowledgements sent.
    pub acks_sent: u64,
}

/// What an own text asks for beyond its text and its addressee ([`MtClient::send_text`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextOptions {
    /// The packet id that the text replies to, 0 for none.
    pub reply_id: u32,
    /// A reaction, as a Unicode code point, 0 for none.
    pub emoji: u32,
    /// Ask for an acknowledgement. `None`: always for a direct text, and for a channel text as
    /// [`MtConfig::ack_broadcast`] says.
    pub want_ack: Option<bool>,
    /// The hop limit, at most 7. `None` takes [`MtConfig::hop_limit`].
    pub hop_limit: Option<u8>,
    /// The packet id, from [`MtClient::new_id`]. `None` takes a new one.
    pub id: Option<u32>,
}

impl MtClient {
    /// A client. `seed` makes packet ids and waits repeatable.
    pub fn new(cfg: MtConfig, seed: u64) -> Self {
        MtClient {
            cfg,
            rng: Rng::new(seed),
            recent_ids: VecDeque::new(),
            seen: Dedup::new(DEDUP_S, 4000),
            out: BTreeMap::new(),
            late: VecDeque::new(),
            rebroadcasts: Vec::new(),
            peers: ObjectTable::new(PEERS_CAP),
            last_nodeinfo: None,
            last_position: None,
            host_position: None,
            next_req: 1,
            rebroadcast_count: 0,
            acks_sent: 0,
        }
    }
    /// The record of node `node`, made when new (heard at `now`).
    fn peer(&mut self, node: u32, now: f64) -> Option<&mut Peer> {
        if self.peers.contains(&node) {
            return self.peers.get_mut(&node);
        }
        self.peers
            .upsert(&node, Stamp::at(now), Peer::default)
            .ok()
            .map(|u| u.row)
    }
    /// A public key learned elsewhere (the node table).
    pub fn learn_key(&mut self, node: u32, key: [u8; 32]) {
        if let Some(p) = self.peer(node, 0.0) {
            p.key = Some(key);
        }
    }
    /// The host's location (position mode "from the host's location").
    pub fn set_host_position(&mut self, lat: f64, lon: f64, alt: i32) {
        self.host_position = Some((lat, lon, alt));
    }
    fn own_byte(&self) -> u8 {
        (self.cfg.node & 0xFF) as u8
    }
    /// A packet id that the client has not used lately, for a packet that it sends.
    pub fn new_id(&mut self) -> u32 {
        loop {
            let id = self.rng.next_u32();
            if id != 0 && !self.recent_ids.contains(&id) {
                self.recent_ids.push_back(id);
                if self.recent_ids.len() > 1000 {
                    self.recent_ids.pop_front();
                }
                return id;
            }
        }
    }
    fn key_of(&self, ch: usize) -> Vec<u8> {
        if self.cfg.licensed {
            return Vec::new();
        }
        self.cfg
            .channels
            .get(ch)
            .map(|c| c.key.clone())
            .unwrap_or_default()
    }
    fn channel_name(&self, ch: usize) -> String {
        self.cfg
            .channels
            .get(ch)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "LongFast".into())
    }
    fn channel_hash(&self, ch: usize) -> u8 {
        mt::channel_hash(&self.channel_name(ch), &self.key_of(ch))
    }
    fn request(
        &mut self,
        payload: Vec<u8>,
        what: TxKind,
        cw: u8,
        message_key: Option<u64>,
        not_before: f64,
    ) -> Option<TxRequest> {
        let ch = self.cfg.tx_channel.as_ref()?;
        let id = self.next_req;
        self.next_req += 1;
        Some(TxRequest {
            id,
            payload,
            lora: Some(ch.params.clone()),
            afsk: None,
            freq_hz: ch.freq_hz,
            not_before,
            cw,
            what,
            message_key,
        })
    }
    /// Builds a packet: a channel packet with the channel's key, or a PKI direct message.
    #[allow(clippy::too_many_arguments)]
    fn packet(
        &mut self,
        to: u32,
        ch: usize,
        data: &Data,
        want_ack: bool,
        hop_limit: u8,
        pki: bool,
        id: Option<u32>,
    ) -> (u32, Vec<u8>) {
        let id = id.unwrap_or_else(|| self.new_id());
        let next_hop = if to != mt::BROADCAST {
            self.peers
                .get(&to)
                .and_then(|p| p.next_hop)
                .map_or(0, |n| n.0)
        } else {
            0
        };
        let mut h = Header {
            to,
            from: self.cfg.node,
            id,
            hop_limit: hop_limit.min(7),
            want_ack,
            via_mqtt: false,
            hop_start: hop_limit.min(7),
            channel: self.channel_hash(ch),
            next_hop,
            relay: self.own_byte(),
        };
        let plain = data.encode();
        if pki {
            if let Some(peer) = self.key_of_node(to).copied() {
                h.channel = 0;
                let extra = self.rng.next_u32();
                let body = mt::pki_shared_key(&self.cfg.private_key, &peer)
                    .and_then(|shared| mt::pki_encrypt(&shared, id, h.from, extra, &plain));
                if let Some(body) = body {
                    let mut out = h.bytes().to_vec();
                    out.extend_from_slice(&body);
                    return (id, out);
                }
            }
        }
        (id, mt::build_channel_packet(&h, &self.key_of(ch), &plain))
    }
    fn own_user(&self) -> Vec<u8> {
        let long = if self.cfg.licensed && !self.cfg.callsign.is_empty() {
            self.cfg.callsign.clone()
        } else {
            mt::cut_bytes(&self.cfg.long_name, 39).to_string()
        };
        User {
            id: mt::node_name(self.cfg.node),
            long_name: long,
            short_name: self.cfg.short_name.chars().take(4).collect(),
            hw_model: 255,
            is_licensed: self.cfg.licensed,
            role: self.cfg.role.number(),
            public_key: if self.cfg.licensed {
                Vec::new()
            } else {
                self.cfg.public_key().to_vec()
            },
            is_unmessagable: false,
        }
        .encode()
    }
    fn nodeinfo(&mut self, now: f64, to: u32, want_response: bool) -> Option<TxRequest> {
        let data = Data {
            port: 4,
            payload: self.own_user(),
            want_response,
            bitfield: Some(0),
            ..Default::default()
        };
        let hop = self.cfg.hop_limit;
        let (_, b) = self.packet(to, 0, &data, false, hop, false, None);
        self.last_nodeinfo = Some(now);
        self.request(b, TxKind::NodeInfo, 3, None, now)
    }
    fn position_data(&self, now: f64) -> Option<Data> {
        let (lat, lon, alt) = if self.cfg.position_from_host {
            self.host_position?
        } else {
            self.cfg.position?
        };
        let prec = self.cfg.channels.first().map_or(32, |c| c.precision);
        let time = (self.cfg.clock0 + now) as u32;
        Some(Data {
            port: 3,
            payload: Position::encode(lat, lon, alt, time, prec, !self.cfg.position_from_host),
            bitfield: Some(0),
            ..Default::default()
        })
    }

    /// Sends a text: to `to` (broadcast for a channel text) on own channel `ch`, with `opts`. Returns the message (state
    /// "queued"); its packet id is `opts.id`, or a new one.
    pub fn send_text(
        &mut self,
        now: f64,
        to: u32,
        ch: usize,
        text: &str,
        opts: TextOptions,
    ) -> ClientOut {
        let mut o = ClientOut::default();
        let mut t = text.to_string();
        if self.cfg.callsign_prefix && !self.cfg.callsign.is_empty() {
            t = format!("{}: {t}", self.cfg.callsign);
        }
        let t = mt::cut_bytes(&t, MAX_TEXT).to_string();
        if let Some(f) = self.held() {
            o.logs.push(format!("Meshtastic: {}", f.text()));
        }
        // The own NodeInfo goes out first in a session (and after its interval).
        let due = match self.last_nodeinfo {
            None => true,
            Some(l) => self.cfg.nodeinfo_interval > 0.0 && now - l >= self.cfg.nodeinfo_interval,
        };
        if due && self.held().is_none() && self.cfg.role != Role::ClientHidden {
            let r = self.nodeinfo(now, mt::BROADCAST, false);
            o.tx.extend(r);
        }
        let direct = to != mt::BROADCAST;
        let pki = direct && !self.cfg.licensed && self.key_of_node(to).is_some();
        let want_ack = opts.want_ack.unwrap_or(direct || self.cfg.ack_broadcast);
        let data = Data {
            port: 1,
            payload: t.as_bytes().to_vec(),
            reply_id: opts.reply_id,
            emoji: opts.emoji,
            bitfield: Some(0),
            ..Default::default()
        };
        let hop = opts.hop_limit.unwrap_or(self.cfg.hop_limit);
        let (id, bytes) = self.packet(to, ch, &data, want_ack, hop, pki, opts.id);
        let channel = if direct {
            "DM".to_string()
        } else {
            self.channel_name(ch)
        };
        let og = Outgoing {
            id,
            bytes: bytes.clone(),
            want_ack,
            dest: to,
            tries: 0,
            deadline: None,
            state: Delivery::Queued,
            relays: Vec::new(),
            is_text: true,
            channel,
            text: t,
            time: now,
        };
        o.texts.push(self.text_of(&og));
        match self.held() {
            None => {
                let r = self.request(bytes, TxKind::Text, 3, Some(id as u64), now);
                o.tx.extend(r);
                self.out.insert(id, og);
            }
            Some(f) => {
                let mut og = og;
                og.state = Delivery::Failed(f);
                o.texts.push(self.text_of(&og));
            }
        }
        o
    }
    fn text_of(&self, og: &Outgoing) -> MeshText {
        MeshText {
            time: og.time,
            protocol: Protocol::Meshtastic,
            channel: og.channel.clone(),
            from: mt::node_name(self.cfg.node),
            from_name: self.cfg.long_name.clone(),
            to: mt::node_name(og.dest),
            text: message_text(&og.text),
            hops: 0,
            packet_id: og.id,
            own: true,
            state: Some(og.state),
            relays: og.relays.len() as u32,
            key: og.id as u64,
            ..Default::default()
        }
    }

    /// Sends a request for `what` to node `node`.
    pub fn request_from(&mut self, now: f64, node: u32, what: MtRequest) -> ClientOut {
        let mut o = ClientOut::default();
        if let Some(f) = self.held() {
            o.logs.push(format!("Meshtastic: {}", f.text()));
            return o;
        }
        let data = match what {
            MtRequest::NodeInfo => Data {
                port: 4,
                payload: self.own_user(),
                want_response: true,
                ..Default::default()
            },
            MtRequest::Position => Data {
                port: 3,
                payload: self
                    .position_data(now)
                    .map(|d| d.payload)
                    .unwrap_or_default(),
                want_response: true,
                ..Default::default()
            },
            MtRequest::Traceroute => Data {
                port: 70,
                payload: Route::default().encode(),
                want_response: true,
                ..Default::default()
            },
            MtRequest::Telemetry => Data {
                port: 67,
                want_response: true,
                ..Default::default()
            },
        };
        let hop = self.cfg.hop_limit;
        let pki =
            !self.cfg.licensed && self.key_of_node(node).is_some() && what != MtRequest::NodeInfo;
        let (_, b) = self.packet(node, 0, &data, false, hop, pki, None);
        let r = self.request(b, what.kind(), 3, None, now);
        o.tx.extend(r);
        o
    }

    /// Why the client sends nothing now, or `None` while it sends.
    fn held(&self) -> Option<Failure> {
        if !self.cfg.tx_enabled {
            Some(Failure::TransmitOff)
        } else if self.cfg.tx_channel.is_none() {
            Some(Failure::NoChannel)
        } else {
            None
        }
    }
    /// The back-off slot of the channel, s: 2.5 symbols and 0.2 ms. 0 without a channel.
    fn slot_s(&self) -> f64 {
        self.cfg
            .tx_channel
            .as_ref()
            .map_or(0.0, |c| 2.5 * c.params.symbol_s() + 0.2e-3)
    }

    /// The scheduler put a request on the air: own messages go to "sent" and their timeout starts.
    pub fn on_sent(&mut self, now: f64, message_key: Option<u64>, air_s: f64) -> ClientOut {
        let mut o = ClientOut::default();
        let Some(k) = message_key else {
            return o;
        };
        // Without a channel the timeout ends after the air time, and `tick` fails the message.
        let ack_air = self
            .cfg
            .tx_channel
            .as_ref()
            .map_or(0.0, |c| c.params.airtime(16 + 32));
        let slot = self.slot_s();
        let jitter = self.rng.uniform();
        if let Some(og) = self.out.get_mut(&(k as u32)) {
            og.tries += 1;
            if og.state == Delivery::Queued || og.state == Delivery::WaitingForDutyCycle {
                og.state = Delivery::Sent;
                let m = og.clone();
                o.texts.push(self.text_of(&m));
            }
            let og = self
                .out
                .get_mut(&(k as u32))
                .expect("the message is still in the map");
            // The answer is the repeat of a node or its acknowledgement. Both come after the longest back-off of a
            // node. A packet sent again before that makes the node drop the repeat it holds, and no answer comes.
            let backoff = f64::from(2 * REPEAT_CW_MAX + (1 << REPEAT_CW_MAX)) * slot;
            og.deadline = og
                .want_ack
                .then_some(now + air_s + backoff + air_s.max(ack_air) + jitter);
        }
        o
    }
    /// The scheduler holds a message for the duty cycle, or failed it.
    pub fn on_state(&mut self, message_key: Option<u64>, state: Delivery) -> ClientOut {
        let mut o = ClientOut::default();
        if let Some(og) = message_key.and_then(|k| self.out.get_mut(&(k as u32))) {
            og.state = state;
            let m = og.clone();
            if m.state.done() {
                self.out.remove(&m.id);
            }
            o.texts.push(self.text_of(&m));
        }
        o
    }

    /// Remembers packet `id` of `from`. Returns true the first time within the de-duplication window.
    fn mark_seen(&mut self, now: f64, from: u32, id: u32) -> bool {
        !self.seen.seen(((from as u64) << 32) | id as u64, now)
    }

    /// A packet was heard (from the decoder).
    pub fn on_packet(&mut self, now: f64, rx: &MtRx) -> ClientOut {
        let mut o = ClientOut::default();
        let h = rx.header;
        let me = self.cfg.node;
        if !rx.crc_ok {
            return o;
        }
        // Own packets heard back: implicit acknowledgements.
        if h.from == me {
            if h.relay != self.own_byte() {
                if let Some(og) = self.out.get_mut(&h.id) {
                    if !og.relays.contains(&h.relay) {
                        og.relays.push(h.relay);
                    }
                    if og.dest == mt::BROADCAST {
                        if matches!(
                            og.state,
                            Delivery::Sent
                                | Delivery::Queued
                                | Delivery::Failed(Failure::Routing(mt::ROUTING_MAX_RETRANSMIT))
                        ) {
                            og.state = Delivery::AckedByMesh;
                        }
                        og.deadline = None;
                    } else if og.state == Delivery::Sent {
                        og.state = Delivery::RelayedNotConfirmed;
                    }
                    let m = og.clone();
                    o.texts.push(self.text_of(&m));
                }
            }
            return o;
        }
        // Another node's rebroadcast of a packet the own node waits to rebroadcast cancels it.
        if let Some(i) = self
            .rebroadcasts
            .iter()
            .position(|r| r.sender == h.from && r.id == h.id)
        {
            let r = self.rebroadcasts.remove(i);
            o.cancel.push(r.req);
            o.logs.push(format!(
                "Meshtastic: rebroadcast of {} {:08x} cancelled, heard from another node",
                mt::node_name(h.from),
                h.id
            ));
        }
        let first = self.mark_seen(now, h.from, h.id);
        if let Some(d) = &rx.data {
            if d.port == 4 {
                if let (Some(u), Some(p)) = (User::parse(&d.payload), self.peer(h.from, now)) {
                    if let Ok(k) = <[u8; 32]>::try_from(u.public_key.as_slice()) {
                        p.key = Some(k);
                    }
                    if !u.long_name.is_empty() {
                        p.name = Some(u.long_name);
                    }
                }
            }
            if h.hops() >= 0 {
                if let Some(p) = self.peer(h.from, now) {
                    let e = p.next_hop.get_or_insert((h.relay, u32::MAX));
                    if (h.hops() as u32) < e.1 {
                        *e = (h.relay, h.hops() as u32);
                    }
                }
            }
        }
        let for_me = h.to == me;
        if for_me && self.held().is_none() {
            // Acknowledge, also for duplicates.
            if h.want_ack && rx.data.is_some() {
                let data = Data {
                    port: 5,
                    payload: Routing::encode_error(0),
                    request_id: h.id,
                    bitfield: Some(0),
                    ..Default::default()
                };
                let hop = if h.hop_start > 0 {
                    h.hop_start
                } else {
                    self.cfg.hop_limit
                };
                let ch = self.own_channel_index(rx.channel.as_deref());
                let (_, b) = self.packet(h.from, ch, &data, false, hop, rx.pki, None);
                let r = self.request(b, TxKind::Ack, 3, None, now);
                o.tx.extend(r);
                self.acks_sent += 1;
            }
            if rx.unknown_key && first {
                let data = Data {
                    port: 5,
                    payload: Routing::encode_error(35),
                    request_id: h.id,
                    ..Default::default()
                };
                let hop = self.cfg.hop_limit;
                let (_, b) = self.packet(h.from, 0, &data, false, hop, false, None);
                let r = self.request(b, TxKind::Ack, 3, None, now);
                o.tx.extend(r);
            }
        }
        if let (true, Some(d)) = (for_me || h.to == mt::BROADCAST, &rx.data) {
            match d.port {
                5 => self.on_routing(now, h.from, d, &mut o),
                4 if d.want_response && self.held().is_none() => {
                    let last = self.peers.get(&h.from).and_then(|p| p.replied_nodeinfo);
                    if last.is_none_or(|t| now - t >= 600.0) {
                        if let Some(p) = self.peer(h.from, now) {
                            p.replied_nodeinfo = Some(now);
                        }
                        let r = self.nodeinfo(now, h.from, false);
                        o.tx.extend(r);
                    }
                }
                3 if d.want_response && for_me && self.held().is_none() => {
                    if let Some(mut pd) = self.position_data(now) {
                        pd.request_id = h.id;
                        let hop = self.cfg.hop_limit;
                        let (_, b) = self.packet(h.from, 0, &pd, false, hop, false, None);
                        let r = self.request(b, TxKind::Position, 3, None, now);
                        o.tx.extend(r);
                    }
                }
                70 if d.want_response && for_me && self.held().is_none() => {
                    let mut route = Route::parse(&d.payload).unwrap_or_default();
                    route.snr_towards.push((rx.snr * 4.0).round() as i32);
                    let reply = Data {
                        port: 70,
                        payload: route.encode(),
                        request_id: h.id,
                        ..Default::default()
                    };
                    let hop = self.cfg.hop_limit;
                    let (_, b) = self.packet(h.from, 0, &reply, false, hop, false, None);
                    let r = self.request(b, TxKind::Traceroute, 3, None, now);
                    o.tx.extend(r);
                }
                _ => {}
            }
        }
        // Flooding, role CLIENT only.
        if first
            && self.cfg.role == Role::Client
            && self.held().is_none()
            && h.hop_limit > 0
            && !for_me
            && (h.next_hop == 0 || h.next_hop == self.own_byte())
        {
            let mut b = rx.raw.clone();
            if b.len() >= 16 {
                b[12] = (b[12] & !7) | (h.hop_limit - 1);
                b[15] = self.own_byte();
                let frac = ((rx.snr + 20.0) / 30.0).clamp(0.0, 1.0);
                let cw = (3.0 + 4.0 * frac).round() as u8;
                let slot = self.slot_s();
                let wait = self.rng.below(1 << cw) as f64 * slot;
                let Some(r) = self.request(b.clone(), TxKind::Rebroadcast, cw, None, now + wait)
                else {
                    return o;
                };
                self.rebroadcasts.push(Rebroadcast {
                    sender: h.from,
                    id: h.id,
                    bytes: b,
                    at: now + wait,
                    req: r.id,
                });
                self.rebroadcast_count += 1;
                o.tx.push(r);
            }
        }
        self.rebroadcasts.retain(|r| r.at > now - 5.0);
        let _ = self
            .rebroadcasts
            .iter()
            .map(|r| r.bytes.len())
            .sum::<usize>();
        o
    }
    fn own_channel_index(&self, name: Option<&str>) -> usize {
        name.and_then(|n| self.cfg.channels.iter().position(|c| c.name == n))
            .unwrap_or(0)
    }
    fn on_routing(&mut self, _now: f64, from: u32, d: &Data, o: &mut ClientOut) {
        let Some(r) = Routing::parse(&d.payload) else {
            return;
        };
        let Some(og) = self.out.get_mut(&d.request_id) else {
            return;
        };
        if r.error == 0 {
            if og.dest == from || og.dest == mt::BROADCAST {
                og.state = if og.dest == mt::BROADCAST {
                    Delivery::AckedByMesh
                } else {
                    Delivery::Delivered
                };
                og.deadline = None;
            }
        } else {
            og.state = Delivery::Failed(Failure::Routing(r.error));
            og.deadline = None;
        }
        let m = og.clone();
        if m.state.done() {
            self.out.remove(&m.id);
        }
        if m.is_text {
            o.texts.push(self.text_of(&m));
        }
    }

    /// Periodic work at `now`: retransmissions, failures, NodeInfo and position broadcasts.
    pub fn tick(&mut self, now: f64) -> ClientOut {
        let mut o = ClientOut::default();
        let mut resend = Vec::new();
        let mut failed = Vec::new();
        let no_channel = self.cfg.tx_channel.is_none();
        for og in self.out.values_mut() {
            if let Some(dl) = og.deadline {
                if now >= dl {
                    og.deadline = None;
                    if no_channel {
                        og.state = Delivery::Failed(Failure::NoChannel);
                        failed.push(og.id);
                    } else if og.tries >= 4 {
                        og.state = if og.dest == mt::BROADCAST && !og.relays.is_empty() {
                            Delivery::AckedByMesh
                        } else if og.state == Delivery::RelayedNotConfirmed {
                            Delivery::RelayedNotConfirmed
                        } else {
                            Delivery::Failed(Failure::Routing(mt::ROUTING_MAX_RETRANSMIT))
                        };
                        failed.push(og.id);
                    } else {
                        resend.push((og.id, og.bytes.clone()));
                    }
                }
            }
        }
        for (id, b) in resend {
            let r = self.request(b, TxKind::Retransmission, 3, Some(id as u64), now);
            o.tx.extend(r);
        }
        for id in failed {
            // A message without an answer stays for an answer that comes late. Every other one is done.
            let unanswered = self.out.get(&id).is_some_and(|m| {
                m.state == Delivery::Failed(Failure::Routing(mt::ROUTING_MAX_RETRANSMIT))
            });
            let m = if unanswered {
                self.late.push_back(id);
                if self.late.len() > LATE_CAP {
                    if let Some(old) = self.late.pop_front() {
                        self.out.remove(&old);
                    }
                }
                self.out.get(&id).cloned()
            } else {
                self.out.remove(&id)
            };
            if let Some(m) = m {
                if let Delivery::Failed(Failure::Routing(_)) = m.state {
                    if m.dest != mt::BROADCAST {
                        // Two failed direct deliveries forget the learned next hop.
                        if let Some(p) = self.peers.get_mut(&m.dest) {
                            p.next_hop = None;
                        }
                    }
                }
                o.texts.push(self.text_of(&m));
            }
        }
        if self.held().is_none() && self.cfg.role != Role::ClientHidden {
            if let Some(l) = self.last_nodeinfo {
                if self.cfg.nodeinfo_interval > 0.0 && now - l >= self.cfg.nodeinfo_interval {
                    let r = self.nodeinfo(now, mt::BROADCAST, false);
                    o.tx.extend(r);
                }
            }
            let pos_due = self
                .last_position
                .is_none_or(|l| now - l >= self.cfg.position_interval.max(30.0));
            if pos_due {
                if let Some(d) = self.position_data(now) {
                    self.last_position = Some(now);
                    let hop = self.cfg.hop_limit;
                    let (_, b) = self.packet(mt::BROADCAST, 0, &d, false, hop, false, None);
                    let r = self.request(b, TxKind::Position, 3, None, now);
                    o.tx.extend(r);
                }
            }
        }
        o
    }
    /// Requests the NodeInfo of a node before a direct message when its key or name is unknown (at most once per 10 minutes).
    pub fn need_nodeinfo(&mut self, now: f64, node: u32) -> ClientOut {
        let mut o = ClientOut::default();
        let known = self
            .peers
            .get(&node)
            .is_some_and(|p| p.key.is_some() && p.name.is_some());
        if known || self.held().is_some() {
            return o;
        }
        let asked = self.peers.get(&node).and_then(|p| p.asked_nodeinfo);
        if asked.is_some_and(|t| now - t < 600.0) {
            return o;
        }
        if let Some(p) = self.peer(node, now) {
            p.asked_nodeinfo = Some(now);
        }
        let r = self.nodeinfo(now, node, true);
        o.tx.extend(r);
        o
    }
    /// Own messages still in flight.
    pub fn in_flight(&self) -> usize {
        self.out
            .values()
            .filter(|m| {
                matches!(
                    m.state,
                    Delivery::Queued | Delivery::WaitingForDutyCycle | Delivery::Sent
                )
            })
            .count()
    }
    /// A node's public key, when known.
    pub fn key_of_node(&self, node: u32) -> Option<&[u8; 32]> {
        self.peers.get(&node).and_then(|p| p.key.as_ref())
    }
}
