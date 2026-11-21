//! The MeshCore texting client: the own identity, contacts from signed adverts, direct texts with acknowledgement
//! codes and retries, learned paths, channel texts and own adverts. The client never repeats other nodes' packets.

use super::decode::McRx;
use super::types::{
    message_text, Delivery, Failure, MeshText, Protocol, TxChannel, TxKind, TxRequest,
};
use crate::proto::error::KeyError;
use crate::proto::meshcore::{self as mc, Identity, McPacket};
use crate::proto::meshtastic::hex;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_util_objects::{ObjectTable, Stamp};
use std::collections::BTreeMap;

/// The longest direct text, bytes.
pub const MAX_DM: usize = 160;
/// Contacts kept. Contacts the user added never leave.
pub const CONTACTS_CAP: usize = 200;

/// A contact.
#[derive(Clone, Debug, PartialEq)]
pub struct Contact {
    /// Public key.
    pub key: [u8; 32],
    /// Name.
    pub name: String,
    /// Advert timestamp.
    pub advert_time: u32,
    /// SNR.
    pub snr: f64,
    /// The learned path (repeater hashes), when any.
    pub path: Option<Vec<Vec<u8>>>,
}

/// The client's settings.
#[derive(Clone, Debug)]
pub struct McConfig {
    /// The identity.
    pub identity: Identity,
    /// Advert name (at most 31 bytes).
    pub name: String,
    /// Advert location.
    pub location: Option<(f64, f64)>,
    /// Flood advert interval, s (0: only when the user sends one).
    pub advert_interval: f64,
    /// The transmitter is on. Nothing is sent while it is off.
    pub tx_enabled: bool,
    /// Channels: name and secret.
    pub channels: Vec<(String, Vec<u8>)>,
    /// The channel the client sends on. `None`: the session listens to no MeshCore channel, and nothing is sent.
    pub tx_channel: Option<TxChannel>,
    /// The host's clock at signal time 0 (Unix s).
    pub clock0: f64,
}

impl McConfig {
    /// The defaults with an identity.
    pub fn new(identity: Identity) -> Self {
        McConfig {
            identity,
            name: "sdrkit".into(),
            location: None,
            advert_interval: 0.0,
            tx_enabled: false,
            channels: vec![("Public".into(), mc::PUBLIC_SECRET.to_vec())],
            tx_channel: None,
            clock0: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
struct Outgoing {
    key: u64,
    peer: [u8; 32],
    text: String,
    attempt: u8,
    expected: u32,
    deadline: Option<f64>,
    state: Delivery,
    time: f64,
    direct: bool,
}

/// What the client requests after a call.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct McOut {
    /// Frames to send.
    pub tx: Vec<TxRequest>,
    /// Own messages, new or with a new state.
    pub texts: Vec<MeshText>,
    /// Log lines.
    pub logs: Vec<String>,
}

/// The client.
#[derive(Clone, Debug)]
pub struct McClient {
    /// The settings.
    pub cfg: McConfig,
    /// Contacts by the hex of their key.
    pub contacts: ObjectTable<String, Contact>,
    out: BTreeMap<u64, Outgoing>,
    rng: Rng,
    next_key: u64,
    next_req: u64,
    last_advert: Option<f64>,
    /// Acknowledgements sent.
    pub acks_sent: u64,
}

impl McClient {
    /// A client. `seed` makes it repeatable.
    pub fn new(cfg: McConfig, seed: u64) -> Self {
        McClient {
            cfg,
            contacts: ObjectTable::new(CONTACTS_CAP),
            out: BTreeMap::new(),
            rng: Rng::new(seed),
            next_key: 1 << 40,
            next_req: 1 << 40,
            last_advert: None,
            acks_sent: 0,
        }
    }
    fn clock(&self, now: f64) -> u32 {
        (self.cfg.clock0 + now) as u32
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
    /// A request on the channel of the client, or `None` without a channel.
    fn request(
        &mut self,
        payload: Vec<u8>,
        what: TxKind,
        key: Option<u64>,
        now: f64,
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
            not_before: now,
            cw: 3,
            what,
            message_key: key,
        })
    }
    /// Adds a contact from a public key in hex or base64, at signal time `now`. A contact the user adds never leaves the table.
    ///
    /// # Errors
    ///
    /// [`KeyError::PublicKey`] if the key is not 32 bytes, [`KeyError::Full`] if the table has no room.
    pub fn add_contact(&mut self, now: f64, key_text: &str, name: &str) -> Result<(), KeyError> {
        let key: [u8; 32] = mc::parse_secret(key_text)
            .and_then(|k| k.try_into().ok())
            .ok_or(KeyError::PublicKey)?;
        let id = hex(&key);
        let up = self
            .contacts
            .upsert(&id, Stamp::at(now), || Contact {
                key,
                name: name.into(),
                advert_time: 0,
                snr: 0.0,
                path: None,
            })
            .map_err(|_| KeyError::Full)?;
        if !name.is_empty() {
            up.row.name = name.into();
        }
        self.contacts.set_favourite(&id, true);
        Ok(())
    }
    fn own_advert(&mut self, now: f64, flood: bool) -> Option<TxRequest> {
        let a = mc::build_advert(
            &self.cfg.identity,
            self.clock(now),
            1,
            self.cfg.location,
            mt_cut(&self.cfg.name, 31),
        );
        let p = McPacket {
            route: if flood { 1 } else { 2 },
            ptype: 4,
            version: 0,
            hash_size: 1,
            path: Vec::new(),
            payload: a,
        };
        self.last_advert = Some(now);
        self.request(
            p.encode(),
            if flood {
                TxKind::Advert
            } else {
                TxKind::LocalAdvert
            },
            None,
            now,
        )
    }
    /// Sends the own advert: to the mesh (flood) or local (zero hop).
    pub fn advert(&mut self, now: f64, flood: bool) -> McOut {
        let mut o = McOut::default();
        match self.held() {
            None => o.tx.extend(self.own_advert(now, flood)),
            Some(f) => o.logs.push(format!("MeshCore: {}", f.text())),
        }
        o
    }
    /// Sends a channel text on own channel `ch`.
    pub fn send_channel(&mut self, now: f64, ch: usize, text: &str) -> McOut {
        let mut o = McOut::default();
        let Some((name, secret)) = self.cfg.channels.get(ch).cloned() else {
            o.logs.push(format!("MeshCore: no channel {ch}"));
            return o;
        };
        let Some(b) = mc::build_group_text(
            &secret,
            self.clock(now),
            mt_cut(&self.cfg.name, 31),
            text,
            Vec::new(),
        ) else {
            o.logs
                .push(format!("MeshCore: the secret of channel {ch} is too short"));
            return o;
        };
        let key = self.next_key;
        self.next_key += 1;
        let m = MeshText {
            time: now,
            protocol: Protocol::MeshCore,
            channel: name,
            from: hex(&self.cfg.identity.public()[..6]),
            from_name: self.cfg.name.clone(),
            to: "^all".into(),
            text: message_text(text),
            own: true,
            state: Some(match self.held() {
                None => Delivery::Queued,
                Some(f) => Delivery::Failed(f),
            }),
            key,
            ..Default::default()
        };
        o.texts.push(m);
        if self.held().is_none() {
            let r = self.request(b, TxKind::Text, Some(key), now);
            o.tx.extend(r);
            self.out.insert(
                key,
                Outgoing {
                    key,
                    peer: [0; 32],
                    text: text.into(),
                    attempt: 0,
                    expected: 0,
                    deadline: None,
                    state: Delivery::Queued,
                    time: now,
                    direct: false,
                },
            );
        }
        o
    }
    fn dm_packet(&mut self, now: f64, og: &mut Outgoing) -> Option<Vec<u8>> {
        let secret = self.cfg.identity.shared_secret(&og.peer)?;
        let plain = mc::text_plain(self.clock(now), og.attempt, mt_cut(&og.text, MAX_DM));
        og.expected = mc::ack_code(&plain, &self.cfg.identity.public());
        let mut payload = vec![og.peer[0], self.cfg.identity.hash()];
        payload.extend_from_slice(&mc::encrypt_then_mac(&secret, &plain)?);
        let path = if og.attempt < 2 {
            self.contacts
                .get(&hex(&og.peer))
                .and_then(|c| c.path.clone())
        } else {
            // The last attempt goes by flood with the path forgotten.
            if let Some(c) = self.contacts.get_mut(&hex(&og.peer)) {
                c.path = None;
            }
            None
        };
        og.direct = path.is_some();
        Some(
            McPacket {
                route: if path.is_some() { 2 } else { 1 },
                ptype: 2,
                version: 0,
                hash_size: 1,
                path: path.unwrap_or_default(),
                payload,
            }
            .encode(),
        )
    }
    /// Sends a direct text to a contact (by the hex of its key, or its name).
    pub fn send_dm(&mut self, now: f64, to: &str, text: &str) -> McOut {
        let mut o = McOut::default();
        let Some(c) = self
            .contacts
            .iter()
            .map(|r| r.record)
            .find(|c| hex(&c.key).starts_with(&to.to_lowercase()) || c.name == to)
            .cloned()
        else {
            o.logs.push(format!("MeshCore: no contact {to}"));
            return o;
        };
        let key = self.next_key;
        self.next_key += 1;
        let mut og = Outgoing {
            key,
            peer: c.key,
            text: text.into(),
            attempt: 0,
            expected: 0,
            deadline: None,
            state: Delivery::Queued,
            time: now,
            direct: false,
        };
        if let Some(f) = self.held() {
            og.state = Delivery::Failed(f);
            o.texts.push(self.text_of(&og));
            return o;
        }
        let Some(b) = self.dm_packet(now, &mut og) else {
            o.logs
                .push("MeshCore: the contact's key is not a valid point".into());
            return o;
        };
        o.texts.push(self.text_of(&og));
        let r = self.request(b, TxKind::Text, Some(key), now);
        o.tx.extend(r);
        self.out.insert(key, og);
        o
    }
    fn text_of(&self, og: &Outgoing) -> MeshText {
        let to = if og.peer == [0; 32] {
            "^all".to_string()
        } else {
            hex(&og.peer[..6])
        };
        MeshText {
            time: og.time,
            protocol: Protocol::MeshCore,
            channel: if og.peer == [0; 32] {
                "Public".into()
            } else {
                "DM".into()
            },
            from: hex(&self.cfg.identity.public()[..6]),
            from_name: self.cfg.name.clone(),
            to,
            text: message_text(&og.text),
            own: true,
            state: Some(og.state),
            key: og.key,
            ..Default::default()
        }
    }
    /// The scheduler put a request on the air.
    pub fn on_sent(&mut self, now: f64, key: Option<u64>, air_s: f64) -> McOut {
        let mut o = McOut::default();
        let Some(og) = key.and_then(|k| self.out.get_mut(&k)) else {
            return o;
        };
        if og.peer == [0; 32] {
            // Channel texts have no acknowledgement.
            og.state = Delivery::Sent;
            let m = og.clone();
            self.out.remove(&m.key);
            o.texts.push(self.text_of(&m));
            return o;
        }
        let hops = self
            .contacts
            .get(&hex(&og.peer))
            .and_then(|c| c.path.as_ref())
            .map_or(0, |p| p.len());
        og.deadline = Some(
            now + if og.direct {
                (hops as f64 + 1.0) * 2.0 * air_s + 2.0
            } else {
                12.0
            },
        );
        if og.state == Delivery::Queued {
            og.state = Delivery::Sent;
            let m = og.clone();
            o.texts.push(self.text_of(&m));
        }
        o
    }
    /// A packet was heard (from the decoder).
    pub fn on_packet(&mut self, now: f64, rx: &McRx) -> McOut {
        let mut o = McOut::default();
        if rx.own || !rx.crc_ok {
            return o;
        }
        let pk = &rx.packet;
        if let Some((key, name, ts)) = &rx.advert {
            if let Ok(k) = <[u8; 32]>::try_from(key.as_slice()) {
                if k != self.cfg.identity.public() {
                    if let Ok(up) = self.contacts.upsert(&hex(&k), Stamp::at(now), || Contact {
                        key: k,
                        name: name.clone(),
                        advert_time: *ts,
                        snr: rx.snr,
                        path: None,
                    }) {
                        up.row.name = name.clone();
                        up.row.advert_time = *ts;
                        up.row.snr = rx.snr;
                    }
                }
            }
        }
        match pk.ptype {
            3 if pk.payload.len() >= 4 => {
                let code = u32::from_le_bytes(
                    pk.payload[0..4]
                        .try_into()
                        .expect("the range has the array's length"),
                );
                self.acked(code, &mut o);
            }
            2 | 8 => {
                let (Some(plain), Some(peer)) = (&rx.plain, &rx.peer) else {
                    return o;
                };
                let Ok(peer) = <[u8; 32]>::try_from(peer.as_slice()) else {
                    return o;
                };
                if pk.ptype == 2 {
                    self.answer_dm(now, pk, plain, &peer, &mut o);
                } else {
                    self.on_path(plain, &peer, &mut o);
                }
            }
            _ => {}
        }
        o
    }
    fn acked(&mut self, code: u32, o: &mut McOut) {
        if let Some(k) = self
            .out
            .values()
            .find(|g| g.expected == code && g.peer != [0; 32])
            .map(|g| g.key)
        {
            let mut g = self
                .out
                .remove(&k)
                .expect("the key was just read from the map");
            g.state = Delivery::Delivered;
            o.texts.push(self.text_of(&g));
        }
    }
    fn answer_dm(&mut self, now: f64, pk: &McPacket, plain: &[u8], peer: &[u8; 32], o: &mut McOut) {
        if self.held().is_some() {
            return;
        }
        let end = plain[5..]
            .iter()
            .position(|&c| c == 0)
            .map_or(plain.len(), |e| e + 5);
        let code = mc::ack_code(&plain[..end], peer);
        let flood = pk.route == 0 || pk.route == 1;
        let back: Vec<Vec<u8>> = pk.path.iter().rev().cloned().collect();
        let b = if flood {
            // A PATH packet returns the path to the sender and carries the acknowledgement.
            let Some(secret) = self.cfg.identity.shared_secret(peer) else {
                return;
            };
            let mut p = vec![(back.len() as u8 & 0x3F) | ((pk.hash_size.max(1) - 1) << 6)];
            for h in &back {
                p.extend_from_slice(h);
            }
            p.push(3);
            p.extend_from_slice(&code.to_le_bytes());
            let mut payload = vec![peer[0], self.cfg.identity.hash()];
            let Some(ct) = mc::encrypt_then_mac(&secret, &p) else {
                return;
            };
            payload.extend_from_slice(&ct);
            McPacket {
                route: 1,
                ptype: 8,
                version: 0,
                hash_size: 1,
                path: Vec::new(),
                payload,
            }
            .encode()
        } else {
            McPacket {
                route: 2,
                ptype: 3,
                version: 0,
                hash_size: pk.hash_size.max(1),
                path: back,
                payload: code.to_le_bytes().to_vec(),
            }
            .encode()
        };
        self.acks_sent += 1;
        let r = self.request(b, TxKind::Ack, None, now);
        o.tx.extend(r);
    }
    fn on_path(&mut self, plain: &[u8], peer: &[u8; 32], o: &mut McOut) {
        if plain.is_empty() {
            return;
        }
        let count = (plain[0] & 0x3F) as usize;
        let size = ((plain[0] >> 6) + 1) as usize;
        if plain.len() < 1 + count * size {
            return;
        }
        let path: Vec<Vec<u8>> = (0..count)
            .map(|i| plain[1 + i * size..1 + (i + 1) * size].to_vec())
            .collect();
        if let Some(c) = self.contacts.get_mut(&hex(peer)) {
            c.path = Some(path);
            o.logs.push(format!("MeshCore: path to {} learned", c.name));
        }
        let at = 1 + count * size;
        if plain.len() >= at + 5 && plain[at] == 3 {
            let code = u32::from_le_bytes(
                plain[at + 1..at + 5]
                    .try_into()
                    .expect("the range has the array's length"),
            );
            self.acked(code, o);
        }
    }
    /// Periodic work: retries and failures, the advert interval.
    pub fn tick(&mut self, now: f64) -> McOut {
        let mut o = McOut::default();
        let due: Vec<u64> = self
            .out
            .values()
            .filter(|g| g.deadline.is_some_and(|d| now >= d))
            .map(|g| g.key)
            .collect();
        for k in due {
            let mut g = self
                .out
                .remove(&k)
                .expect("the key was just read from the map");
            g.deadline = None;
            if g.attempt >= 2 {
                g.state = Delivery::Failed(Failure::NoAck);
                o.texts.push(self.text_of(&g));
                continue;
            }
            if self.cfg.tx_channel.is_none() {
                g.state = Delivery::Failed(Failure::NoChannel);
                o.texts.push(self.text_of(&g));
                continue;
            }
            g.attempt += 1;
            if let Some(b) = self.dm_packet(now, &mut g) {
                let r = self.request(b, TxKind::Retransmission, Some(k), now);
                o.tx.extend(r);
            }
            self.out.insert(k, g);
        }
        if self.held().is_none() && self.cfg.advert_interval > 0.0 {
            let due = self
                .last_advert
                .is_none_or(|l| now - l >= self.cfg.advert_interval);
            if due {
                o.tx.extend(self.own_advert(now, true));
            }
        }
        let _ = self.rng.next_u32();
        o
    }
    /// Messages in flight.
    pub fn in_flight(&self) -> usize {
        self.out.len()
    }
    /// The attempt number of an own message in flight.
    pub fn attempt_of(&self, key: u64) -> Option<u8> {
        self.out.get(&key).map(|g| g.attempt)
    }
}

fn mt_cut(s: &str, n: usize) -> &str {
    crate::proto::meshtastic::cut_bytes(s, n)
}
