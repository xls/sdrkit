//! What happens to a frame: it is offered to the protocols of its setting, the first that accepts it owns it, and the packet row,
//! the tables, the messages, the readings and the event follow. Meshtastic, MeshCore, LoRa APRS, MeshCom and LoRaWAN.

use super::ham_client::HamHeard;
use super::plan::{protocols_of_sync, PlanEntry};
use super::store::MeshStore;
use super::types::{
    message_text, printable_raw, LorawanCols, MeshNodeUpdate, MeshPacket, MeshReading, MeshText,
    Protocol,
};
use crate::aprs::station::{message_row, to_station};
use crate::proto::error::{KeyError, ParseError, Part};
use crate::proto::hamlora;
use crate::proto::lorawan::{self, LorawanFrame};
use crate::proto::meshcore::{self as mc, Identity, McPacket};
use crate::proto::meshtastic::{self as mt, Channel, Data, Header};
use crate::typed::{LoraPacketEvent, LORA_PACKET};
use sdrkit_core::Event;
use sdrkit_proto_lora_phy::rx::LoraFrame;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// A Meshtastic packet after decryption (`mt.packet`): what the client needs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MtRx {
    /// The header.
    pub header: Header,
    /// The Data message when it was readable.
    pub data: Option<Data>,
    /// The channel it decrypted with (`None`: PKI or not read).
    pub channel: Option<String>,
    /// It came by PKI.
    pub pki: bool,
    /// The sender's public key was not known (a PKI packet to the own node).
    pub unknown_key: bool,
    /// Signal time.
    pub time: f64,
    /// SNR.
    pub snr: f64,
    /// The frame's CRC was right.
    pub crc_ok: bool,
    /// Sent by the own node.
    pub own: bool,
    /// The raw frame.
    pub raw: Vec<u8>,
}

impl sdrkit_graph::Item for MtRx {
    const KIND: &'static str = "mt.packet";
}

/// A MeshCore packet (`mc.packet`), with its plaintext when it was for the own identity.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct McRx {
    /// The packet.
    pub packet: McPacket,
    /// The plaintext of an end-to-end packet for the own identity.
    pub plain: Option<Vec<u8>>,
    /// The peer's public key (when decrypted).
    pub peer: Option<Vec<u8>>,
    /// A good advert's key, name and timestamp.
    pub advert: Option<(Vec<u8>, String, u32)>,
    /// Signal time.
    pub time: f64,
    /// SNR.
    pub snr: f64,
    /// Sent by the own identity.
    pub own: bool,
    /// The frame passed its checksum.
    pub crc_ok: bool,
}

impl sdrkit_graph::Item for McRx {
    const KIND: &'static str = "mc.packet";
}

/// The result of one frame.
#[derive(Clone, Debug, Default)]
pub struct Decoded {
    /// The packet row.
    pub packet: MeshPacket,
    /// Meshtastic.
    pub mt: Option<MtRx>,
    /// MeshCore.
    pub mc: Option<McRx>,
    /// LoRaWAN.
    pub lorawan: Option<LorawanFrame>,
    /// What a good LoRa APRS frame asks of the own station: an answer to an own message, or a message to acknowledge.
    pub ham: Option<HamHeard>,
    /// New messages.
    pub texts: Vec<MeshText>,
    /// Node updates.
    pub nodes: Vec<MeshNodeUpdate>,
    /// Readings.
    pub readings: Vec<MeshReading>,
    /// The `lora.packet` event.
    pub event: Option<Event>,
    /// Log lines.
    pub logs: Vec<String>,
}

/// The decoder of the mesh receiver: keys, identities and the tables.
#[derive(Debug)]
pub struct MeshDecoder {
    /// The tables.
    pub store: MeshStore,
    /// Meshtastic channels (defaults first).
    pub mt_channels: Vec<Channel>,
    /// MeshCore channels (Public first): name and secret.
    pub mc_channels: Vec<(String, Vec<u8>)>,
    /// The own Meshtastic node number and private key.
    pub mt_own: Option<(u32, [u8; 32])>,
    /// The own MeshCore identity.
    pub mc_own: Option<Identity>,
    /// MeshCore contacts added by the user (public keys).
    pub mc_contacts: Vec<[u8; 32]>,
    /// The source call of own LoRa APRS frames (empty: none). Answers and numbered messages to it go to the client.
    pub aprs_own: String,
    /// Payloads sent in the last 10 s.
    pub own_payloads: VecDeque<(f64, Vec<u8>)>,
    /// Computer clock at signal time 0 (Unix s), 0 when times follow the signal.
    pub wall0: f64,
}

impl Default for MeshDecoder {
    fn default() -> Self {
        MeshDecoder {
            store: MeshStore::default(),
            mt_channels: mt::default_channels(),
            mc_channels: vec![("Public".into(), mc::PUBLIC_SECRET.to_vec())],
            mt_own: None,
            mc_own: None,
            mc_contacts: Vec::new(),
            aprs_own: String::new(),
            own_payloads: VecDeque::new(),
            wall0: 0.0,
        }
    }
}

/// Seconds of signal as "time" and the computer clock.
fn wall(w0: f64, t: f64) -> f64 {
    if w0 > 0.0 {
        w0 + t
    } else {
        0.0
    }
}

/// The name and the secret of a MeshCore channel from what a person enters. With an empty secret, the name Public is the
/// public channel and any other name is a hashtag channel: `xyz` and `#xyz` are both the channel `#xyz`.
///
/// # Errors
///
/// [`KeyError::Secret`] if the secret is not 16 or 32 bytes in hex or base64.
pub fn mc_channel(name: &str, secret: &str) -> Result<(String, Vec<u8>), KeyError> {
    let name = name.trim();
    if !secret.trim().is_empty() {
        let s = mc::parse_secret(secret).ok_or(KeyError::Secret)?;
        return Ok((name.to_owned(), s));
    }
    if name.eq_ignore_ascii_case("public") {
        return Ok(("Public".to_owned(), mc::PUBLIC_SECRET.to_vec()));
    }
    let name = if name.starts_with('#') {
        name.to_owned()
    } else {
        format!("#{name}")
    };
    let s = mc::hashtag_secret(&name).to_vec();
    Ok((name, s))
}

impl MeshDecoder {
    /// Adds a Meshtastic channel from a name and a key text: base64, hex or a key word.
    ///
    /// # Errors
    ///
    /// [`KeyError::NotKeyText`] or [`KeyError::TooLong`] if the key is refused.
    pub fn add_mt_channel(&mut self, name: &str, key: &str) -> Result<(), KeyError> {
        let k = mt::parse_key_text(key).ok_or(KeyError::NotKeyText)?;
        let k = mt::expand_key(&k).ok_or(KeyError::TooLong)?;
        if self
            .mt_channels
            .iter()
            .any(|c| c.name == name && c.key == k)
        {
            return Ok(());
        }
        self.mt_channels.push(Channel {
            name: name.into(),
            key: k,
            user: true,
        });
        Ok(())
    }
    /// Adds a MeshCore channel from a name and a secret text, as [`mc_channel`] reads them.
    ///
    /// # Errors
    ///
    /// [`KeyError::Secret`] if the secret is not 16 or 32 bytes.
    pub fn add_mc_channel(&mut self, name: &str, secret: &str) -> Result<(), KeyError> {
        let (name, s) = mc_channel(name, secret)?;
        if let Some(c) = self.mc_channels.iter_mut().find(|c| c.1 == s) {
            c.0 = name;
            return Ok(());
        }
        self.mc_channels.push((name, s));
        Ok(())
    }
    /// Drops the user channels (the defaults and Public stay).
    pub fn clear_user_channels(&mut self) {
        self.mt_channels.retain(|c| !c.user);
        self.mc_channels.truncate(1);
    }
    /// The channel names (defaults first).
    pub fn channel_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.mt_channels.iter().map(|c| c.name.clone()).collect();
        v.extend(self.mc_channels.iter().map(|c| c.0.clone()));
        v
    }
    /// Remembers an own transmission (its payload marks the frame "own" when heard back).
    pub fn sent(&mut self, time: f64, payload: &[u8]) {
        self.own_payloads.push_back((time, payload.to_vec()));
        while self.own_payloads.len() > 64 {
            self.own_payloads.pop_front();
        }
    }

    /// Decodes one frame of a plan entry, heard at signal time `time` (s).
    pub fn frame(&mut self, f: &LoraFrame, entry: &PlanEntry, time: f64) -> Decoded {
        let mut d = Decoded::default();
        let freq = f.channel_hz.max(entry.freq_hz) + f.cfo_hz;
        while self.own_payloads.front().is_some_and(|p| p.0 < time - 10.0) {
            self.own_payloads.pop_front();
        }
        let own_payload = self.own_payloads.iter().any(|p| p.1 == f.payload);
        let mut p = MeshPacket {
            time,
            wall: wall(self.wall0, time),
            protocol: entry.protocol(),
            preset: entry.name.clone(),
            freq_hz: freq,
            offset_hz: f.cfo_hz,
            sf: f.sf,
            cr: f.cr,
            bw: f.bw,
            level_db: f.level_db,
            snr_db: f.snr_db,
            size: f.payload.len(),
            crc_ok: f.good(),
            hops: -1,
            own: own_payload,
            ..Default::default()
        };
        let mut mask = entry.mask & protocols_of_sync(f.sync);
        if mask == 0 {
            mask = entry.mask;
        }
        let mut first_note = String::new();
        let mut accepted = None;
        for proto in Protocol::of_mask(mask) {
            let r = match proto {
                Protocol::LoraAprs => self.lora_aprs(f, &mut p, &mut d),
                Protocol::MeshCom => self.meshcom(f, &mut p, &mut d),
                Protocol::Lorawan => self.lorawan(f, &mut p, &mut d),
                Protocol::Meshtastic => self.meshtastic(f, &mut p, &mut d),
                Protocol::MeshCore => self.meshcore(f, &mut p, &mut d),
                Protocol::Lora | Protocol::Aprs => Err(ParseError::NotThis),
            };
            match r {
                Ok(()) => {
                    accepted = Some(proto);
                    break;
                }
                Err(n) => {
                    if first_note.is_empty() {
                        first_note = match n {
                            ParseError::NotThis => format!("not a {} packet", proto.name()),
                            n => n.to_string(),
                        };
                    }
                }
            }
        }
        let crc_bad = !f.good();
        match accepted {
            Some(proto) => {
                p.protocol = proto;
                p.parsed = true;
                if crc_bad && !p.note.starts_with("CRC error") {
                    p.note = format!("CRC error; {}", p.note);
                }
                if matches!(proto, Protocol::LoraAprs | Protocol::MeshCom) {
                    p.raw = printable_raw(&f.payload);
                } else if proto == Protocol::Lorawan {
                    p.raw = lorawan::hex_pairs(&f.payload);
                }
            }
            None => {
                p.protocol = Protocol::Lora;
                p.ptype = "unknown".into();
                p.note = if crc_bad {
                    format!("CRC error; {first_note}")
                } else {
                    first_note
                };
                p.raw = printable_raw(&f.payload);
            }
        }
        let tcount = p.ptype.clone();
        self.store.count(p.protocol, &tcount);
        if crc_bad {
            self.store.count(p.protocol, "CRC error");
        }
        let named = p.parsed && !p.from.is_empty();
        d.event = Some(LORA_PACKET.event(
            time,
            freq,
            &LoraPacketEvent {
                net: p.protocol,
                modulation: crate::results::Modulation::Lora,
                snr_db: f.snr_db,
                sf: Some(i64::from(f.sf)),
                payload: f.payload.as_slice().into(),
                crc_ok: f.good(),
                source: named.then_some(p.from.as_str()),
                target: named.then_some(p.to.as_str()),
                devaddr: None,
                iq_inverted: None,
                crc: None,
                bw: None,
                cr: None,
                implicit: None,
                sync: None,
                level_dbfs: None,
                family: crate::results::family(p.protocol),
            },
        ));
        self.store.add_packet(p.clone());
        d.packet = p;
        d
    }

    // ---- Meshtastic ----

    fn meshtastic(
        &mut self,
        f: &LoraFrame,
        p: &mut MeshPacket,
        d: &mut Decoded,
    ) -> Result<(), ParseError> {
        let b = &f.payload;
        let Some(h) = Header::parse(b) else {
            return Err(ParseError::Short(Part::MeshtasticHeader));
        };
        p.from = mt::node_name(h.from);
        p.to = mt::node_name(h.to);
        p.packet_id = format!("0x{:08x}", h.id);
        p.hop_limit = Some(h.hop_limit);
        p.hop_start = Some(h.hop_start);
        p.via_mqtt = h.via_mqtt;
        p.next_hop = Some(h.next_hop);
        p.relay = Some(h.relay);
        p.hops = h.hops();
        p.channel = format!("0x{:02x}", h.channel);
        let own_node = self.mt_own.map(|o| o.0);
        let own = p.own || own_node == Some(h.from);
        p.own = own;
        let time = p.time;
        let mut rx = MtRx {
            header: h,
            time,
            snr: f.snr_db,
            crc_ok: f.good(),
            own,
            raw: b.clone(),
            ..Default::default()
        };
        let sender = p.from.clone();
        // A frame with a failed CRC changes no node: its sender can be wrong too.
        let good = f.good();
        if !own && good {
            let new = self.store.heard(
                Protocol::Meshtastic,
                &sender,
                time,
                f.snr_db,
                f.level_db,
                h.hops(),
            );
            if let Some((n, _)) = self.store.node_mut(Protocol::Meshtastic, &sender, time) {
                n.last_relay = h.relay;
                if h.hops() >= 0 && h.hops() < n.best_hops {
                    n.best_hops = h.hops();
                    n.next_hop = h.relay;
                }
            }
            if new {
                d.logs.push(format!("Meshtastic: new node {sender}"));
            }
            d.nodes.push(MeshNodeUpdate {
                protocol: Protocol::Meshtastic,
                id: sender.clone(),
            });
        }
        if b.len() == 16 {
            p.ptype = "EMPTY".into();
            p.note = "no payload".into();
            d.mt = Some(rx);
            return Ok(());
        }
        let body = &b[16..];
        // Channels whose hash matches first, then every other key.
        let mut order: Vec<usize> = (0..self.mt_channels.len())
            .filter(|&i| self.mt_channels[i].hash() == h.channel)
            .collect();
        let hash_known = !order.is_empty();
        order.extend(
            (0..self.mt_channels.len()).filter(|&i| self.mt_channels[i].hash() != h.channel),
        );
        let mut found: Option<(Data, Option<usize>)> = None;
        let mut tmp = Vec::with_capacity(body.len());
        let mut tried_keys: Vec<&[u8]> = Vec::new();
        for &i in &order {
            let c = &self.mt_channels[i];
            if c.key.is_empty() && h.channel != c.hash() {
                continue;
            }
            if tried_keys.contains(&c.key.as_slice()) && c.hash() != h.channel {
                continue;
            }
            tried_keys.push(&c.key);
            tmp.clear();
            tmp.extend_from_slice(body);
            mt::aes_ctr(&c.key, h.id, h.from, &mut tmp);
            if let Some(data) = Data::parse_strict(&tmp) {
                found = Some((data, Some(i)));
                break;
            }
        }
        // A public-key direct message to the own node.
        let direct_pki = h.channel == 0 && h.to != mt::BROADCAST;
        if found.is_none() && direct_pki {
            if let Some((num, key)) = self.mt_own {
                if h.to == num {
                    let pk = self
                        .store
                        .node(Protocol::Meshtastic, &sender)
                        .and_then(|n| mt::hex_decode(&n.public_key))
                        .filter(|k| k.len() == 32);
                    match pk {
                        Some(pk) => {
                            let peer: [u8; 32] = pk.try_into().unwrap_or([0; 32]);
                            match mt::pki_shared_key(&key, &peer)
                                .and_then(|shared| mt::pki_decrypt(&shared, h.id, h.from, body))
                                .and_then(|x| Data::parse_strict(&x))
                            {
                                Some(data) => found = Some((data, None)),
                                None => {
                                    p.ptype = "ENCRYPTED".into();
                                    p.note = "PKI decryption failed".into();
                                }
                            }
                        }
                        None => {
                            rx.unknown_key = true;
                            p.ptype = "ENCRYPTED".into();
                            p.note = "encrypted (direct): sender's public key not known yet".into();
                        }
                    }
                }
            }
        }
        let Some((data, ch)) = found else {
            if p.ptype.is_empty() {
                p.ptype = "ENCRYPTED".into();
                p.note = if direct_pki {
                    "encrypted (direct)".into()
                } else if hash_known {
                    "wrong key or damaged payload".into()
                } else {
                    format!("unknown channel 0x{:02x}", h.channel)
                };
            }
            if !own && good {
                if let Some((n, _)) = self.store.node_mut(Protocol::Meshtastic, &sender, time) {
                    n.encrypted += 1;
                }
            }
            d.mt = Some(rx);
            return Ok(());
        };
        p.decrypted = true;
        match ch {
            Some(i) => {
                let c = &self.mt_channels[i];
                p.channel = c.name.clone();
                if c.hash() != h.channel {
                    p.note = format!(
                        "decrypted with the key of channel {} (header hash differs)",
                        c.name
                    );
                }
                rx.channel = Some(c.name.clone());
            }
            None => {
                p.channel = "DM".into();
                rx.pki = true;
            }
        }
        p.ptype = mt::port_name(data.port);
        let info = mt::decode_port(&data);
        p.summary = info.summary.clone();
        if !own {
            if good {
                self.mt_updates(&h, &data, &info, p, d);
            }
            self.mt_text(&h, &data, &info, p, good, d);
        }
        rx.data = Some(data);
        d.mt = Some(rx);
        Ok(())
    }

    fn mt_updates(
        &mut self,
        h: &Header,
        data: &Data,
        info: &mt::PortInfo,
        p: &MeshPacket,
        d: &mut Decoded,
    ) {
        let id = mt::node_name(h.from);
        let time = p.time;
        let w = p.wall;
        if let (Some(u), Some((n, _))) = (
            &info.user,
            self.store.node_mut(Protocol::Meshtastic, &id, time),
        ) {
            let changed = !u.long_name.is_empty() && u.long_name != n.long_name;
            if !u.long_name.is_empty() {
                n.long_name = u.long_name.clone();
            }
            if !u.short_name.is_empty() {
                n.short_name = u.short_name.clone();
            }
            n.hw = mt::hw_name(u.hw_model);
            n.role = mt::role_name(u.role);
            if u.public_key.len() == 32 {
                n.public_key = mt::hex(&u.public_key);
            }
            if changed {
                d.logs
                    .push(format!("Meshtastic: node {id} \"{}\"", u.long_name));
            }
        }
        if let Some(pos) = &info.position {
            if pos.has_fix() && data.port == 3 {
                let (lat, lon) = (pos.lat.unwrap_or(0.0), pos.lon.unwrap_or(0.0));
                let alt = pos.alt.map(|a| a as f64);
                if let Some((n, _)) = self.store.node_mut(Protocol::Meshtastic, &id, time) {
                    n.position = Some((lat, lon, alt));
                    n.position_time = pos.time;
                }
                let r = MeshReading {
                    time,
                    wall: w,
                    node: id.clone(),
                    position: Some((lat, lon, alt)),
                    ..Default::default()
                };
                self.store.add_reading(r.clone());
                d.readings.push(r);
            }
        }
        if let (Some(t), Some((n, _))) = (
            &info.telemetry,
            self.store.node_mut(Protocol::Meshtastic, &id, time),
        ) {
            if t.device {
                n.battery = t.battery.or(n.battery);
                n.voltage = t.voltage.or(n.voltage);
                n.ch_util = t.ch_util.or(n.ch_util);
                n.air_tx = t.air_tx.or(n.air_tx);
                n.uptime = t.uptime.or(n.uptime);
                n.device_time = time;
            }
            if t.environment {
                n.temp = t.temperature.or(n.temp);
                n.humidity = t.humidity.or(n.humidity);
                n.pressure = t.pressure.or(n.pressure);
                n.env_time = time;
            }
            let r = MeshReading {
                time,
                wall: w,
                node: id.clone(),
                battery: t.battery,
                voltage: t.voltage,
                ch_util: t.ch_util,
                air_tx: t.air_tx,
                temp: t.temperature,
                humidity: t.humidity,
                pressure: t.pressure,
                position: None,
            };
            self.store.add_reading(r.clone());
            d.readings.push(r);
        }
    }

    /// The text of a packet becomes a message. From a frame with a good CRC it is shown at once. From a damaged frame it
    /// waits in the store for a good copy of the same sender and packet id.
    fn mt_text(
        &mut self,
        h: &Header,
        data: &Data,
        info: &mt::PortInfo,
        p: &MeshPacket,
        good: bool,
        d: &mut Decoded,
    ) {
        let id = mt::node_name(h.from);
        let time = p.time;
        if let Some(text) = &info.text {
            let from_name = self
                .store
                .node(Protocol::Meshtastic, &id)
                .map(|n| n.long_name.clone())
                .unwrap_or_default();
            let channel = if h.to != mt::BROADCAST && p.channel != "DM" {
                "DM".to_string()
            } else {
                p.channel.clone()
            };
            let m = MeshText {
                time,
                protocol: Protocol::Meshtastic,
                channel,
                from: id.clone(),
                from_name,
                to: mt::node_name(h.to),
                text: message_text(text),
                hops: h.hops(),
                packet_id: h.id,
                reply_id: data.reply_id,
                emoji: data.emoji,
                snr_db: Some(p.snr_db),
                level_dbfs: Some(p.level_db),
                damaged: !p.crc_ok,
                ..Default::default()
            };
            let key = format!(
                "mt/{id}/{}/0/{}",
                h.id,
                if h.id == 0 { text.as_str() } else { "" }
            );
            if !good {
                self.store.hold_message(m, key);
            } else if self.store.add_message(m.clone(), key) {
                d.texts.push(m);
            }
        }
    }

    // ---- MeshCore ----

    fn meshcore(
        &mut self,
        f: &LoraFrame,
        p: &mut MeshPacket,
        d: &mut Decoded,
    ) -> Result<(), ParseError> {
        let pk = McPacket::parse(&f.payload)?;
        let time = p.time;
        p.ptype = mc::type_name(pk.ptype);
        p.path = pk.path_text();
        p.hops = pk.path.len() as i32;
        let hop = pk.hop_text();
        let pl = &pk.payload;
        let mut rx = McRx {
            packet: pk.clone(),
            time,
            snr: f.snr_db,
            own: p.own,
            crc_ok: f.good(),
            ..Default::default()
        };
        match pk.ptype {
            4 => match mc::parse_advert(pl) {
                None => {
                    p.note = "incomplete advert".into();
                }
                Some(a) => {
                    let id = mt::hex(&a.public_key[..6]);
                    let own = self
                        .mc_own
                        .as_ref()
                        .is_some_and(|o| o.public() == a.public_key);
                    p.own |= own;
                    p.from = id.clone();
                    p.decrypted = true;
                    let pos = a
                        .position
                        .map(|(la, lo)| format!(", {la:.5}, {lo:.5}"))
                        .unwrap_or_default();
                    p.summary = format!(
                        "{}, {}{pos}, {}{hop}",
                        if a.name.is_empty() {
                            "(no name)"
                        } else {
                            &a.name
                        },
                        mc::role_name(a.role),
                        if a.signature_ok {
                            "signature ok"
                        } else {
                            "signature NOT valid"
                        }
                    );
                    if a.incomplete {
                        p.note = "incomplete advert data".into();
                    } else if !a.signature_ok {
                        p.note = "signature not valid".into();
                    }
                    // A frame with a failed CRC changes no node and no contact.
                    if !p.own && f.good() {
                        let new = self.store.heard(
                            Protocol::MeshCore,
                            &id,
                            time,
                            f.snr_db,
                            f.level_db,
                            pk.path.len() as i32,
                        );
                        if let Some((n, _)) = self.store.node_mut(Protocol::MeshCore, &id, time) {
                            n.public_key = mt::hex(&a.public_key);
                            n.role = mc::role_name(a.role).into();
                            if !a.name.is_empty() {
                                n.long_name = a.name.clone();
                            }
                            n.signature_ok = Some(a.signature_ok);
                            if let Some((la, lo)) = a.position {
                                n.position = Some((la, lo, None));
                                n.position_time = a.timestamp;
                            }
                        }
                        if new {
                            d.logs
                                .push(format!("MeshCore: new node {id} \"{}\"", a.name));
                        }
                        d.nodes.push(MeshNodeUpdate {
                            protocol: Protocol::MeshCore,
                            id: id.clone(),
                        });
                        if let Some((la, lo)) = a.position {
                            let r = MeshReading {
                                time,
                                wall: p.wall,
                                node: id.clone(),
                                position: Some((la, lo, None)),
                                ..Default::default()
                            };
                            self.store.add_reading(r.clone());
                            d.readings.push(r);
                        }
                    }
                    if a.signature_ok && f.good() {
                        rx.advert = Some((a.public_key.to_vec(), a.name.clone(), a.timestamp));
                    }
                }
            },
            5 | 6 => {
                if pl.len() < 4 {
                    p.note = "incomplete group packet".into();
                } else {
                    self.mc_group(pl, pk.ptype, &pk, p, d)?;
                }
            }
            0 | 1 | 2 | 8 => {
                if pl.len() < 4 {
                    p.note = "incomplete direct packet".into();
                } else {
                    let (dst, src) = (pl[0], pl[1]);
                    p.from = format!("{src:02x}");
                    p.to = format!("{dst:02x}");
                    let mut done = false;
                    if let Some(own) = &self.mc_own {
                        if dst == own.hash() {
                            for key in self.mc_keys_with_hash(src) {
                                let Some(secret) = own.shared_secret(&key) else {
                                    continue;
                                };
                                if let Some(plain) =
                                    mc::mac_then_decrypt(&secret, &pl[2..4], &pl[4..])
                                {
                                    p.decrypted = true;
                                    rx.plain = Some(plain.clone());
                                    rx.peer = Some(key.to_vec());
                                    let peer_id = mt::hex(&key[..6]);
                                    p.from = peer_id.clone();
                                    if pk.ptype == 2 && plain.len() >= 5 {
                                        let ts = u32::from_le_bytes(
                                            plain[0..4]
                                                .try_into()
                                                .expect("the range has the array's length"),
                                        );
                                        let end = plain[5..]
                                            .iter()
                                            .position(|&c| c == 0)
                                            .unwrap_or(plain.len() - 5);
                                        let text =
                                            mc::clean(&String::from_utf8_lossy(&plain[5..5 + end]));
                                        p.summary = format!("{}{hop}", mt::cut_text(&text, 80));
                                        let name = self
                                            .store
                                            .node(Protocol::MeshCore, &peer_id)
                                            .map(|n| n.long_name.clone())
                                            .unwrap_or_default();
                                        let m = MeshText {
                                            time,
                                            protocol: Protocol::MeshCore,
                                            channel: "DM".into(),
                                            from: peer_id.clone(),
                                            from_name: name,
                                            to: mt::hex(&own.public()[..6]),
                                            text: message_text(&text),
                                            hops: pk.path.len() as i32,
                                            sender_time: ts,
                                            snr_db: Some(p.snr_db),
                                            level_dbfs: Some(p.level_db),
                                            damaged: !p.crc_ok,
                                            ..Default::default()
                                        };
                                        let k = format!("mc/{peer_id}/0/{ts}/{text}");
                                        if self.store.add_message(m.clone(), k) {
                                            d.texts.push(m);
                                        }
                                    } else {
                                        p.summary = format!("{} bytes{hop}", plain.len());
                                    }
                                    done = true;
                                    break;
                                }
                            }
                        }
                    }
                    if !done && self.mc_own.is_none() {
                        // Without the own identity, try the end-to-end types as group text.
                        let save = (p.decrypted, p.summary.clone());
                        if self.mc_group(pl, 5, &pk, p, d).is_ok() && p.decrypted {
                            done = true;
                        } else {
                            (p.decrypted, p.summary) = save;
                            p.note.clear();
                            p.channel.clear();
                        }
                    }
                    if !done {
                        p.note = "encrypted (direct)".into();
                        p.summary = format!("to {dst:02x} from {src:02x}{hop}");
                    }
                }
            }
            7 => {
                if pl.len() >= 35 {
                    p.to = format!("{:02x}", pl[0]);
                    p.from = mt::hex(&pl[1..7]);
                }
                p.note = "encrypted (direct)".into();
                p.summary = format!("to {} from {}{hop}", p.to, p.from);
            }
            3 => {
                if pl.len() >= 4 {
                    let code = u32::from_le_bytes(
                        pl[0..4]
                            .try_into()
                            .expect("the range has the array's length"),
                    );
                    p.summary = format!("ack {code:08x}{hop}");
                    p.decrypted = true;
                }
            }
            9 => {
                if pl.len() >= 9 {
                    let tag = u32::from_le_bytes(
                        pl[0..4]
                            .try_into()
                            .expect("the range has the array's length"),
                    );
                    let size = 1usize << (pl[8] & 3);
                    let n = (pl.len() - 9) / size;
                    let snrs: Vec<String> = pk
                        .path
                        .iter()
                        .flat_map(|h| h.iter().copied())
                        .map(|b| format!("{:.2}", b as i8 as f64 / 4.0))
                        .collect();
                    p.summary = format!("trace {tag}, {n} nodes, SNR {} dB", snrs.join(" "));
                    p.path.clear();
                    p.decrypted = true;
                }
            }
            11 => {
                p.summary = format!("control, first byte 0x{:02x}", pl[0]);
            }
            _ => {
                p.summary = format!("{} bytes", pl.len());
            }
        }
        d.mc = Some(rx);
        Ok(())
    }

    fn mc_keys_with_hash(&self, h: u8) -> Vec<[u8; 32]> {
        let mut v: Vec<[u8; 32]> = self
            .store
            .nodes
            .iter()
            .map(|r| r.record)
            .filter(|n| n.protocol == Protocol::MeshCore && n.signature_ok == Some(true))
            .filter_map(|n| mt::hex_decode(&n.public_key))
            .filter_map(|k| <[u8; 32]>::try_from(k).ok())
            .filter(|k| k[0] == h)
            .collect();
        for k in &self.mc_contacts {
            if k[0] == h && !v.contains(k) {
                v.push(*k);
            }
        }
        v
    }

    fn mc_group(
        &mut self,
        pl: &[u8],
        ptype: u8,
        pk: &McPacket,
        p: &mut MeshPacket,
        d: &mut Decoded,
    ) -> Result<(), ParseError> {
        let hop = pk.hop_text();
        let hash = pl[0];
        p.channel = format!("0x{hash:02x}");
        let mut any = false;
        let mut plain = None;
        for (name, secret) in &self.mc_channels {
            if mc::channel_hash(secret) != hash {
                continue;
            }
            any = true;
            if let Some(x) = mc::mac_then_decrypt(secret, &pl[1..3], &pl[3..]) {
                plain = Some((name.clone(), x));
                break;
            }
        }
        let Some((name, x)) = plain else {
            p.note = if any {
                "MAC mismatch (wrong key or damaged)".into()
            } else {
                format!("unknown channel 0x{hash:02x}")
            };
            return Ok(());
        };
        p.decrypted = true;
        p.channel = name.clone();
        p.note.clear();
        if ptype == 5 {
            match mc::parse_group_text(&x) {
                Ok(g) => {
                    p.from = g.sender.clone();
                    p.summary = format!(
                        "{}{}{hop}",
                        if g.sender.is_empty() {
                            String::new()
                        } else {
                            format!("{}: ", g.sender)
                        },
                        mt::cut_text(&g.text, 80)
                    );
                    let m = MeshText {
                        time: p.time,
                        protocol: Protocol::MeshCore,
                        channel: name.clone(),
                        from: g.sender.clone(),
                        from_name: g.sender.clone(),
                        to: "^all".into(),
                        text: message_text(&g.text),
                        hops: pk.path.len() as i32,
                        sender_time: g.timestamp,
                        snr_db: Some(p.snr_db),
                        level_dbfs: Some(p.level_db),
                        damaged: !p.crc_ok,
                        ..Default::default()
                    };
                    let k = format!("mc/{}/0/{}/{}", g.sender, g.timestamp, g.text);
                    if !p.own && self.store.add_message(m.clone(), k) {
                        d.texts.push(m);
                    }
                }
                Err(e) => p.summary = e.to_string(),
            }
        } else if x.len() < 3 {
            p.summary = "short group data".into();
        } else {
            let t = u16::from_le_bytes([x[0], x[1]]);
            p.summary = format!("data type 0x{t:04x}, {} bytes{hop}", x[2]);
        }
        Ok(())
    }

    // ---- LoRa APRS and MeshCom ----

    fn lora_aprs(
        &mut self,
        f: &LoraFrame,
        p: &mut MeshPacket,
        d: &mut Decoded,
    ) -> Result<(), ParseError> {
        let a = hamlora::parse_lora_aprs(&f.payload)?;
        p.ptype = a.info.kind.clone();
        p.from = a.source.clone();
        p.to = a.info.addressee.clone().unwrap_or_else(|| a.dest.clone());
        p.path = a.path.clone();
        p.summary = a.info.summary.clone();
        p.decrypted = true;
        // A frame with a failed CRC changes no station.
        if !p.own && f.good() {
            self.aprs_station(
                Protocol::LoraAprs,
                &a.source,
                &a.path,
                &a.info,
                f,
                p,
                None,
                d,
            );
        }
        if !p.own && f.good() {
            d.ham = to_station(&self.aprs_own, true, &a.source, &a.info, p.time);
        }
        if p.own {
            return Ok(());
        }
        if let Some((mut m, key)) = message_row(Protocol::LoraAprs, &a.source, &a.info, p.time) {
            m.snr_db = Some(p.snr_db);
            m.level_dbfs = Some(p.level_db);
            m.damaged = !p.crc_ok;
            if self.store.add_message(m.clone(), key) {
                d.texts.push(m);
            }
        }
        Ok(())
    }

    /// Shows an own station after its beacon went on the air.
    pub fn own_station(&mut self, s: &super::ham_client::OwnStation) {
        let Some((r, _)) = self.store.station_mut(s.protocol, &s.call, s.time) else {
            return;
        };
        r.symbol.clone_from(&s.symbol);
        if s.position.is_some() {
            r.position = s.position;
        }
        r.comment.clone_from(&s.comment);
        r.path.clone_from(&s.path);
        r.last_type.clone_from(&s.kind);
    }

    fn meshcom(
        &mut self,
        f: &LoraFrame,
        p: &mut MeshPacket,
        d: &mut Decoded,
    ) -> Result<(), ParseError> {
        let m = hamlora::parse_meshcom(&f.payload)?;
        p.ptype = m.kind.clone();
        p.from = m.source.clone();
        p.to = m.dest.clone();
        p.path = m.relays.clone();
        p.packet_id = format!("{:08X}", m.msg_id);
        p.hop_limit = Some(m.max_hop);
        p.decrypted = true;
        match m.kind.as_str() {
            "ACK" => {
                p.summary = format!("ack of {:08X}", m.acked.unwrap_or(0));
            }
            "TEXT" | "HEY" => {
                let t: String = m.text.chars().take(60).collect();
                p.summary = format!("{t} ({})", m.trailer_summary());
            }
            _ => {
                p.summary = m
                    .info
                    .as_ref()
                    .map(|i| i.summary.clone())
                    .unwrap_or_default();
                if let Some(b) = m.battery {
                    p.summary = format!("{}, battery {b} %", p.summary);
                }
            }
        }
        // A frame with a failed CRC changes no station.
        if m.kind != "ACK" && !p.own && f.good() {
            let info = m.info.clone().unwrap_or_default();
            self.aprs_station(
                Protocol::MeshCom,
                &m.source,
                &m.relays,
                &info,
                f,
                p,
                m.battery,
                d,
            );
            if let Some((s, _)) = self
                .store
                .stations
                .get_mut(&format!("meshcom/{}", m.source))
                .map(|s| (s, ()))
            {
                s.last_type = m.kind.clone();
            }
        }
        if m.kind == "TEXT" && !p.own {
            let text = m.text.clone();
            let msg = MeshText {
                time: p.time,
                protocol: Protocol::MeshCom,
                channel: m.channel(),
                from: m.source.clone(),
                from_name: m.source.clone(),
                to: m.dest.clone(),
                text: message_text(&text),
                hops: -1,
                packet_id: m.msg_id,
                snr_db: Some(p.snr_db),
                level_dbfs: Some(p.level_db),
                damaged: !p.crc_ok,
                ..Default::default()
            };
            if self.store.add_message(
                msg.clone(),
                format!("meshcom/{}/{}/{}/{text}", m.source, m.msg_id, m.dest),
            ) {
                d.texts.push(msg);
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn aprs_station(
        &mut self,
        proto: Protocol,
        source: &str,
        path: &str,
        info: &crate::aprs::info::AprsInfo,
        f: &LoraFrame,
        p: &MeshPacket,
        battery: Option<u8>,
        d: &mut Decoded,
    ) {
        let time = p.time;
        let mut new = false;
        if let Some((s, n)) = self.store.station_mut(proto, source, time) {
            new = n;
            s.snr = f.snr_db;
            s.path = path.into();
            s.last_type = info.kind.clone();
            if battery.is_some() {
                s.battery = battery;
            }
        }
        if new {
            d.logs
                .push(format!("{}: new station {source}", proto.name()));
        }
        let target = match &info.name {
            Some(name) if matches!(info.kind.as_str(), "Object" | "Item") => {
                if let Some((o, _)) = self.store.station_mut(proto, name, time) {
                    o.object = true;
                    o.via = source.into();
                    o.last_type = info.kind.clone();
                }
                name.clone()
            }
            _ => source.to_string(),
        };
        let key = format!("{}/{target}", proto.kind());
        let Some(s) = self.store.stations.get_mut(&key) else {
            return;
        };
        if let (Some(la), Some(lo)) = (info.lat, info.lon) {
            s.position = Some((la, lo));
            s.alt_m = info.alt_m.or(s.alt_m);
            s.course = info.course;
            s.speed_kn = info.speed_kn;
            if let (Some(t), Some(c)) = (info.symbol_table, info.symbol_code) {
                s.symbol = format!("{t}{c}");
            }
            if !info.comment.is_empty() {
                s.comment = info.comment.clone();
            }
        } else if info.kind == "Status" {
            s.comment = info.comment.clone();
        }
    }

    // ---- LoRaWAN ----

    fn lorawan(
        &mut self,
        f: &LoraFrame,
        p: &mut MeshPacket,
        d: &mut Decoded,
    ) -> Result<(), ParseError> {
        let lw = lorawan::parse(&f.payload)?;
        p.ptype = lorawan::mtype_name(lw.mtype).into();
        fill_lorawan_row(p, &lw);
        p.decrypted = false;
        if !f.good() {
            p.note = "CRC error".into();
            d.lorawan = Some(lw);
            return Ok(());
        }
        let (id, new) = update_device(
            &mut self.store,
            &lw,
            p.time,
            p.freq_hz,
            f.snr_db,
            f.level_db,
            f.sf,
            false,
        );
        if new {
            if let Some(dv) = self.store.devices.get(&id) {
                d.logs.push(format!(
                    "LoRaWAN: new device {id} ({}) ({})",
                    dv.network, dv.vendor
                ));
            }
        }
        d.lorawan = Some(lw);
        Ok(())
    }
}

/// Fills the LoRaWAN columns of a packet row.
pub fn fill_lorawan_row(p: &mut MeshPacket, lw: &LorawanFrame) {
    if let Some(a) = lw.devaddr {
        p.from = format!("{a:08X}");
    } else if let Some(e) = lw.dev_eui {
        p.from = lorawan::eui_text(e);
    }
    if let Some(j) = lw.join_eui {
        p.to = lorawan::eui_text(j);
    }
    if let Some(c) = lw.fcnt {
        p.packet_id = c.to_string();
    }
    p.summary = lw.summary.clone();
    p.note = lw.note.clone();
    p.lorawan = Some(LorawanCols {
        network: lw.network.clone(),
        mtype: lorawan::mtype_name(lw.mtype).into(),
        fport: lw.fport,
        frm_len: lw.frm_len,
        flags: lw.flags.clone(),
        mac: lw.mac_names.clone(),
        mic: lw.mic.map(|m| format!("{m:08X}")).unwrap_or_default(),
    });
}

/// Updates the device table from a frame with a good CRC: returns the key and whether it is new.
#[allow(clippy::too_many_arguments)]
pub fn update_device(
    store: &mut MeshStore,
    lw: &LorawanFrame,
    time: f64,
    freq: f64,
    snr: f64,
    level: f64,
    sf: u8,
    fsk: bool,
) -> (String, bool) {
    let id = match (lw.devaddr, lw.dev_eui) {
        (Some(a), _) => format!("{a:08X}"),
        (None, Some(e)) => format!("DevEUI {}", lorawan::eui_text(e)),
        _ => return (String::new(), false),
    };
    let Some((dv, new)) = store.device_mut(&id, time) else {
        return (id, false);
    };
    dv.freq_hz = freq;
    dv.snr = snr;
    dv.level = level;
    dv.sf = sf;
    dv.fsk = fsk;
    dv.last_type = lorawan::mtype_name(lw.mtype).into();
    if let Some(a) = lw.devaddr {
        dv.devaddr = format!("{a:08X}");
        dv.network = lw.network.clone();
    }
    if let Some(e) = lw.dev_eui {
        dv.dev_eui = lorawan::eui_text(e);
        dv.vendor = lw.vendor.clone();
        dv.network = "join".into();
    }
    if let Some(j) = lw.join_eui {
        dv.join_eui = lorawan::eui_text(j);
    }
    if matches!(lw.mtype, 0 | 6) {
        dv.joins += 1;
    } else if lw.uplink {
        dv.uplinks += 1;
        if let Some(c) = lw.fcnt {
            if let Some(last) = dv.last_fcnt {
                let ahead = c.wrapping_sub(last);
                if (2..1000).contains(&ahead) {
                    dv.missed += (ahead - 1) as u32;
                }
            }
            dv.last_fcnt = Some(c);
        }
    } else if lw.mtype != 1 {
        dv.downlinks += 1;
    }
    if lw.confirmed {
        dv.confirmed += 1;
    }
    if lw.fport.is_some() {
        dv.fport = lw.fport;
    }
    dv.adr = lw.adr;
    if !lw.mac_commands.is_empty() {
        dv.mac = lw.mac_commands.join(", ");
    }
    if let Some(b) = lw.battery {
        dv.battery = b as i32;
    }
    if lw.margin.is_some() {
        dv.margin = lw.margin;
    }
    (id, new)
}
