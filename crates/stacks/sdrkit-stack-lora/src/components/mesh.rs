//! The components of the mesh receiver: `mesh.decode` (protocols, keys, tables and results), `mt.client` and `mc.client` (the
//! texting clients), `ham.client` (the amateur client) and `tx.sched` (channel access and the duty cycle).

use super::aprs::AprsSignal;
use super::{count, show_status};
use crate::aprs::station::{message_text_fault, MESSAGE_TEXT_MAX};
use crate::commands::APRS_BEACON;
use crate::commands::{
    AdvertScope, CHANNEL, CHANNEL_ROW, CONTACT_NAME, EMOJI, HOP_LIMIT, INDEX, KEY, MC_KEY, MC_TO,
    MT_TO, PRECISION, REPLY_ID, RESET, SCOPE, WANT_ACK, WHAT,
};
use crate::mesh::decode::{mc_channel, McRx, MeshDecoder, MtRx};
use crate::mesh::ham_client::{
    ax25_ok, HamClient, HamConfig, HamHeard, HamLink, HamOut, OwnStation,
};
use crate::mesh::mc_client::{McClient, McConfig, McOut};
use crate::mesh::mt_client::{self, ClientOut, MtClient, MtConfig, OwnChannel, TextOptions};
use crate::mesh::plan::Plan;
use crate::mesh::sched::{SchedEvent, TxScheduler};
use crate::mesh::store::MeshStore;
use crate::mesh::types::{
    Delivery, Failure, MeshNodeUpdate, MeshPacket, MeshReading, MeshText, Protocol, TxKind,
    TxRequest,
};
use crate::proto::error::KeyError;
use crate::proto::hamlora;
use crate::proto::meshcore::{self as mc, Identity};
use crate::proto::meshtastic as mt;
use crate::settings::{
    HamClientSetting, McClientSetting, MeshDecodeSetting, MtClientSetting, PacketSpeed,
    PositionMode, SchedSetting, APRS_COMMENT, APRS_PATH, APRS_SSID, APRS_SYMBOL, APRS_TOCALL,
    BEACON_INTERVAL, CLOCK0, DUTY_BANDS, DUTY_LIMIT, MC_ADVERT_INTERVAL, MC_NAME,
    MC_SHARE_LOCATION, MT_ACK_BROADCAST, MT_CALLSIGN_PREFIX, MT_CHANNEL_NAME, MT_HOP_LIMIT,
    MT_LICENSED, MT_LONG_NAME, MT_NODEINFO_INTERVAL, MT_NODE_ID, MT_POSITION_INTERVAL,
    MT_POSITION_MODE, MT_ROLE, MT_SHORT_NAME, RAW, TX_BAUD, TX_DELAY, TX_PRE_EMPHASIS,
};
use crate::typed::{
    AprsSendArgs, AprsSendReply, HamFields, IdentityArgs, McChannelAddArgs, McContactAddArgs,
    McSendArgs, MeshChannelRow, MeshContactRow, MeshDecoderRow, MeshFields, MeshMcFields,
    MeshMtFields, MeshNodeEvent, MeshSlotRow, MeshTxFields, MtChannelAddArgs, MtChannelArgs,
    MtRequestArgs, MtSendArgs, MtSendReply, APRS_SEND, CHANNEL_REMOVE, CONTACT_ADD, MC_ADVERT,
    MC_CHANNEL_ADD, MC_IDENTITY, MC_SEND, MESH_CHANNELS, MESH_CONTACTS, MESH_DECODERS, MESH_NODE,
    MESH_NODES, MESH_NODE_ROW, MESH_REMOVE, MESH_SLOTS, MT_CHANNEL, MT_CHANNEL_ADD, MT_IDENTITY,
    MT_REQUEST, MT_SEND,
};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, Time};
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{
    Choice, ConfigError, FieldId, LockState, Record, Results, SettingError, SettingId, SettingSet,
    StateBlob, StateError, TableId, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port, RateFrom,
    RunContext, Timed,
};
use sdrkit_proto_lora_phy::bank::DetectorState;
use sdrkit_proto_lora_phy::components::BankState;
use sdrkit_proto_lora_phy::rx::LoraFrame;
use sdrkit_util_objects::commands::{self as objects, CLEAR};
use sdrkit_util_objects::Tables;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// The most channels with user keys.
pub const CHANNELS_CAP: usize = 64;

/// A note the clients and the scheduler send back to the decoder (`mesh.own`).
#[derive(Clone, Debug, PartialEq)]
pub enum OwnNote {
    /// An own message, new or with a new state.
    Text(MeshText),
    /// An own frame went on the air.
    Sent {
        /// Signal time.
        time: f64,
        /// Its payload.
        payload: Vec<u8>,
    },
    /// The own Meshtastic node number and private key.
    MtIdentity {
        /// The node number.
        node: u32,
        /// The X25519 private key.
        key: [u8; 32],
    },
    /// The seed of the own MeshCore identity.
    McIdentity {
        /// The 32-byte Ed25519 seed.
        seed: [u8; 32],
    },
    /// A MeshCore contact that the user added: its public key.
    McContact([u8; 32]),
    /// The state of the Meshtastic client, for the results.
    MtStatus(MtStatus),
    /// The state of the MeshCore client, for the results.
    McStatus(McStatus),
    /// The state of the amateur client, for the results.
    HamStatus(HamStatus),
    /// An own station after its beacon went on the air.
    HamStation(OwnStation),
    /// The state of the scheduler: frames waiting and frames sent.
    SchedStatus {
        /// Frames waiting.
        pending: usize,
        /// Frames sent.
        sent: u64,
    },
    /// A line for the log.
    Log(String),
}

/// The state of the Meshtastic client that the results show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MtStatus {
    /// The own node id.
    pub node_id: String,
    /// The own public key, base64.
    pub public_key: String,
    /// The long name.
    pub long_name: String,
    /// The short name.
    pub short_name: String,
    /// Own packets waiting for an acknowledgement.
    pub in_flight: usize,
    /// Packets of other nodes sent again.
    pub rebroadcasts: u64,
    /// Warnings about the settings.
    pub warnings: String,
    /// The frequency of the own packets, Hz. `None`: the session listens to no channel of the network.
    pub freq_hz: Option<f64>,
    /// The names of the channels the client sends on, the primary channel first.
    pub channels: Vec<String>,
}

/// The state of the amateur client that the results show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HamStatus {
    /// The source call of own APRS frames; empty without a callsign.
    pub aprs_call: String,
    /// The frequency of own APRS frames, Hz. `None`: the session listens to no channel of the link of the client.
    pub aprs_freq_hz: Option<f64>,
}

/// The signal time after which a client that still has no channel runs the commands that waited for one, s.
const SETTLE_S: f64 = 3.0;

/// One MeshCore contact as the results show it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContactRow {
    /// The hex of its public key.
    pub key: String,
    /// Its name.
    pub name: String,
    /// The wall time it was last heard, seconds since 1970, when the clock is known.
    pub last_heard: Option<f64>,
    /// The SNR of its last packet, dB.
    pub snr_db: f64,
    /// The learned path, or "flood".
    pub path: String,
}

/// The state of the MeshCore client that the results show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct McStatus {
    /// The own public key, hex.
    pub public_key: String,
    /// The own name.
    pub name: String,
    /// The frequency of the own packets, Hz. `None`: the session listens to no channel of the network.
    pub freq_hz: Option<f64>,
    /// The names of the channels the client sends on, the channel Public first.
    pub channels: Vec<String>,
    /// The contacts.
    pub contacts: Vec<ContactRow>,
}

impl Item for OwnNote {
    const KIND: &'static str = "mesh.own";
}

/// What the scheduler reports to the clients (`tx.note`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SchedNote {
    /// On the air.
    Sent {
        /// The own message.
        key: Option<u64>,
        /// When.
        at: f64,
        /// Air time, seconds.
        air_s: f64,
    },
    /// Held by the duty-cycle limit.
    Waiting {
        /// The own message.
        key: Option<u64>,
    },
    /// Cannot be sent.
    Failed {
        /// The own message.
        key: Option<u64>,
        /// Why.
        reason: Failure,
    },
}

impl Item for SchedNote {
    const KIND: &'static str = "tx.note";
}

/// A refused key as the refusal of a command argument: a key that does not parse is malformed, a full table refuses it.
fn bad_key(args: &Values, arg: &str, e: KeyError) -> CommandError {
    let id = args.schema().id(arg).unwrap_or(SettingId(0));
    let error = match e {
        KeyError::Full => SettingError::NotPermitted { id },
        KeyError::NotKeyText | KeyError::TooLong | KeyError::Secret | KeyError::PublicKey => {
            SettingError::Malformed { id, at: None }
        }
    };
    CommandError::BadArgument { arg: id, error }
}

/// An argument that does not parse, with the byte position of the fault when known, as the refusal of a command.
fn malformed(args: &Values, arg: &str, at: Option<usize>) -> CommandError {
    let id = args.schema().id(arg).unwrap_or(SettingId(0));
    CommandError::BadArgument {
        arg: id,
        error: SettingError::Malformed { id, at },
    }
}

/// A channel name that the client does not have, as the refusal of a command.
fn no_channel(args: &Values) -> CommandError {
    CommandError::NoSuchKey {
        arg: args.schema().id(CHANNEL.key()).unwrap_or(SettingId(0)),
    }
}

/// A full channel list, as the refusal of the name of one more channel.
fn full(args: &Values) -> CommandError {
    let id = args
        .schema()
        .id(MtChannelAddArgs::NAME.field.key)
        .unwrap_or(SettingId(0));
    CommandError::BadArgument {
        arg: id,
        error: SettingError::NotPermitted { id },
    }
}

/// A 32-byte key from base64 or hex.
fn key32(text: &str) -> Option<[u8; 32]> {
    let t = text.trim();
    mt::base64_decode(t)
        .filter(|k| k.len() == 32)
        .or_else(|| mt::hex_decode(t))
        .and_then(|k| <[u8; 32]>::try_from(k).ok())
}

sdrkit_core::commands! {
    /// The commands of `mesh.decode`.
    pub enum MeshDecodeCommand {
        /// Clear.
        Clear = CLEAR,
        /// Remove.
        Remove = MESH_REMOVE.command,
        /// Reset.
        Reset = RESET,
        /// Add a Meshtastic channel.
        MtChannelAdd = MT_CHANNEL_ADD.command,
        /// Add a MeshCore channel.
        McChannelAdd = MC_CHANNEL_ADD.command,
        /// Remove a channel.
        ChannelRemove = CHANNEL_REMOVE.command,
    }
}

/// A channel key that the user added, as the saved state keeps it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct UserChannel {
    net: Protocol,
    name: String,
    key: String,
}

/// The saved state of `mesh.decode`.
#[derive(Serialize, Deserialize)]
struct DecodeState {
    tables: StateBlob,
    channels: Vec<UserChannel>,
}

const DECODE_STATE: &str = "lora.mesh_decode";

/// `mesh.decode`.
pub struct MeshDecode {
    /// The decoder and its tables.
    pub dec: MeshDecoder,
    plan: Plan,
    p: Published,
    nodes_table: TableId,
    run_frames: u64,
    run_crc_failed: u64,
    last_good: f64,
    last_signal: f64,
    preambles_seen: u64,
    header_failed: u64,
    last_snr: Option<f64>,
    last_offset: Option<f64>,
    level: Option<f64>,
    now: f64,
    raw: bool,
    det: Vec<DetectorState>,
    /// Per plan entry: the frame count, the errors, the SNR and offset of the last good frame, and the time of the last frame.
    det_frames: Vec<EntryCount>,
    mt: Option<MtStatus>,
    mc: Option<McStatus>,
    ham: Option<HamStatus>,
    sched: (usize, u64),
    channels: Vec<UserChannel>,
    key: String,
    text: String,
    /// The samples arrive in real time, and the host's clock, for the wall time of what is heard while the setting
    /// `clock0` is 0.
    live: bool,
    clock: sdrkit_core::Clock,
    /// The setting `clock0` gives the wall time of signal 0.
    clock_given: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct EntryCount {
    frames: u64,
    errors: u64,
    snr: Option<f64>,
    offset: Option<f64>,
    last: Option<f64>,
}

/// The declaration of every field and table of the mesh receiver.
fn mesh_decl() -> sdrkit_core::ResultsDecl {
    let mut d = sdrkit_core::ResultsDecl::new()
        .with_fields(MeshFields::SHAPE.fields)
        .with_fields(MeshMtFields::SHAPE.fields)
        .with_fields(MeshMcFields::SHAPE.fields)
        .with_fields(HamFields::SHAPE.fields)
        .with_fields(MeshTxFields::SHAPE.fields);
    for t in crate::mesh::store::MeshStore::tables() {
        d = d.with_table(t);
    }
    for t in [
        MESH_DECODERS.def(),
        MESH_CONTACTS.def(),
        MESH_CHANNELS.def(),
        MESH_SLOTS.def(),
    ] {
        d = d.with_table(t);
    }
    d
}

/// The store of every field and table of the mesh receiver.
fn mesh_published() -> Published {
    Published::from_decl(std::sync::Arc::new(mesh_decl()))
}

impl Default for MeshDecode {
    fn default() -> Self {
        let p = mesh_published();
        let nodes_table = p
            .results()
            .decl()
            .table_id(MESH_NODES.key)
            .expect("the node table is declared");
        MeshDecode {
            dec: MeshDecoder::default(),
            plan: Plan::default(),
            p,
            nodes_table,
            run_frames: 0,
            run_crc_failed: 0,
            last_good: -1e9,
            last_signal: -1e9,
            preambles_seen: 0,
            header_failed: 0,
            last_snr: None,
            last_offset: None,
            level: None,
            now: 0.0,
            raw: false,
            det: Vec::new(),
            det_frames: Vec::new(),
            mt: None,
            mc: None,
            ham: None,
            sched: (0, 0),
            channels: Vec::new(),
            key: String::new(),
            text: String::new(),
            live: false,
            clock: sdrkit_core::Clock::fixed(0.0),
            clock_given: false,
        }
    }
}

impl MeshDecode {
    /// Sets the wall time of signal 0 from the host's clock at signal time `t`, once, for live samples while the setting
    /// `clock0` is 0. A recording keeps the times of the signal.
    fn anchor(&mut self, t: f64) {
        if self.live && !self.clock_given && self.dec.wall0 == 0.0 {
            let w0 = self.clock.now() - t;
            if w0 > 0.0 {
                self.dec.wall0 = w0;
                self.dec.store.wall0 = w0;
            }
        }
    }

    /// Sends on the messages of damaged frames whose wait for a good copy is over; every one when `all` is set.
    fn release_held(&mut self, io: &mut Io, all: bool) {
        for m in self.dec.store.release_held(self.now, all) {
            let time = m.time;
            io.items_out::<MeshText>(1).push(Timed { time, value: m });
        }
    }

    fn publish(&mut self) {
        let state: u8 = if self.now - self.last_good < 60.0 {
            2
        } else if self.now - self.last_signal < 10.0 {
            1
        } else {
            0
        };
        let s = &self.dec.store;
        let mt_nodes = s.node_count(Protocol::Meshtastic);
        let mc_nodes = s.node_count(Protocol::MeshCore);
        let t = &mut self.text;
        t.clear();
        if state == 0 && self.run_frames == 0 {
            t.push_str("Mesh (LoRa): searching");
        } else {
            let st = if state == 2 { "decoding" } else { "signal" };
            if mt_nodes + mc_nodes == 0 && !s.devices.is_empty() && s.messages.is_empty() {
                let _ = write!(
                    t,
                    "Mesh (LoRa): {st}, {} frames, {} LoRaWAN devices",
                    self.run_frames,
                    s.devices.len()
                );
            } else {
                let _ = write!(
                    t,
                    "Mesh (LoRa): {st}, {} frames, {mt_nodes} Meshtastic / {mc_nodes} MeshCore nodes, ",
                    self.run_frames
                );
                if !s.stations.is_empty() {
                    let _ = write!(t, "{} APRS stations, ", s.stations.len());
                }
                let _ = write!(t, "{} messages", s.messages.len());
            }
            if let Some(x) = self.last_snr {
                let _ = write!(t, ", SNR {x:.1} dB");
            }
        }
        let lock_state = match state {
            2 => LockState::Locked,
            1 => LockState::Tracking,
            _ => LockState::Searching,
        };
        let p = &mut self.p;
        show_status(
            p,
            lock_state,
            self.last_snr,
            self.last_offset,
            self.level,
            t,
        );
        let protocols = &mut self.key;
        protocols.clear();
        for (i, pr) in Protocol::of_mask(self.plan.mask).enumerate() {
            if i > 0 {
                protocols.push_str(", ");
            }
            protocols.push_str(pr.name());
        }
        p.put(&MeshFields {
            frames_ok: count(self.run_frames),
            frames_bad: count(self.run_crc_failed),
            preambles: count(self.preambles_seen),
            header_failed: count(self.header_failed),
            mt_nodes: count(mt_nodes),
            mc_nodes: count(mc_nodes),
            messages: count(s.messages.len()),
            stations: count(s.stations.len()),
            devices: count(s.devices.len()),
            plan: &self.plan.name,
            searching: &self.plan.line,
            dial: Hertz(self.plan.dial_hz),
            protocols,
            custom: self.plan.custom,
            snr_db: self.last_snr,
            offset_hz: self.last_offset.map(Hertz),
            level_dbfs: self.level,
        });
        let key = t;
        for (i, e) in self.plan.entries.iter().enumerate() {
            let d = self.det.iter().find(|d| d.tag as usize == e.detector);
            let c = self.det_frames.get(i).copied().unwrap_or_default();
            let errors = c.errors + d.map_or(0, |d| d.stats.header_failed + d.stats.sync_failed);
            key.clear();
            let _ = write!(key, "{i:03}");
            MESH_DECODERS.put(
                p,
                key,
                &MeshDecoderRow {
                    net: e.protocol(),
                    preset: &e.name,
                    freq: Hertz(e.freq_hz),
                    offset: Hertz(e.freq_hz - self.plan.dial_hz),
                    sf: i64::from(e.params.sf),
                    bw: Hertz(e.params.bw),
                    cr: i64::from(e.params.cr),
                    sync: i64::from(e.params.sync),
                    preamble: i64::from(e.params.preamble),
                    ldro: e.params.ldro_on(),
                    in_band: e.in_band,
                    frames: count(c.frames),
                    errors: count(errors),
                    snr_db: c.snr,
                    cfo: c.offset.map(Hertz),
                    last_s: c.last,
                },
            );
        }
        self.dec.store.publish(p, self.now, self.raw, key);
        let mt = self.mt.as_ref();
        p.put(&MeshMtFields {
            mt_node_id: mt.map(|m| m.node_id.as_str()),
            mt_public_key: mt.map(|m| m.public_key.as_str()),
            mt_long_name: mt.map(|m| m.long_name.as_str()),
            mt_short_name: mt.map(|m| m.short_name.as_str()),
            mt_in_flight: mt.map(|m| count(m.in_flight)),
            mt_rebroadcasts: mt.map(|m| count(m.rebroadcasts)),
            mt_warnings: mt.map(|m| m.warnings.as_str()),
            mt_freq: mt.and_then(|m| m.freq_hz).map(Hertz),
        });
        let mc = self.mc.as_ref();
        p.put(&MeshMcFields {
            mc_public_key: mc.map(|m| m.public_key.as_str()),
            mc_name: mc.map(|m| m.name.as_str()),
            mc_contacts: mc.map(|m| count(m.contacts.len())),
            mc_freq: mc.and_then(|m| m.freq_hz).map(Hertz),
        });
        if let Some(m) = mc {
            let shown = p
                .results()
                .decl()
                .table_id(MESH_CONTACTS.key)
                .and_then(|id| p.results().table(id))
                .map_or(0, |t| t.len());
            if shown > m.contacts.len() {
                p.clear_rows(&MESH_CONTACTS);
            }
            for c in &m.contacts {
                MESH_CONTACTS.put(
                    p,
                    &c.key,
                    &MeshContactRow {
                        name: &c.name,
                        key: c.key.get(..12).unwrap_or(&c.key),
                        last_heard: c.last_heard.map(Time),
                        snr_db: c.snr_db,
                        path: &c.path,
                    },
                );
            }
        }
        let ham = self.ham.as_ref();
        p.put(&HamFields {
            aprs_call: ham.map(|h| h.aprs_call.as_str()),
            aprs_freq: ham.and_then(|h| h.aprs_freq_hz).map(Hertz),
        });
        p.put(&MeshTxFields {
            tx_pending: count(self.sched.0),
            tx_sent: count(self.sched.1),
        });
    }

    /// Writes the `channels` table, without the keys: the channels the clients send on (of the protocols of the plan),
    /// then the channels that the user added and no client has.
    fn publish_channels(&mut self) {
        let p = &mut self.p;
        p.clear_rows(&MESH_CHANNELS);
        let mt = self.mt.as_ref().filter(|_| self.plan.mask & 1 != 0);
        let mc = self.mc.as_ref().filter(|_| self.plan.mask & 2 != 0);
        let sent = mt
            .iter()
            .flat_map(|m| m.channels.iter().map(|n| (Protocol::Meshtastic, n)))
            .chain(
                mc.iter()
                    .flat_map(|m| m.channels.iter().map(|n| (Protocol::MeshCore, n))),
            );
        let added = self.channels.iter().map(|c| (c.net, &c.name));
        for (net, name) in sent.chain(added) {
            self.key.clear();
            let _ = write!(self.key, "{}/{}", net.kind(), name);
            MESH_CHANNELS.put(
                p,
                &self.key,
                &MeshChannelRow {
                    net,
                    channel: name,
                    key_set: true,
                },
            );
        }
    }

    /// Adds a channel key to the decoder.
    fn add_channel(&mut self, c: &UserChannel) -> Result<(), KeyError> {
        match c.net {
            Protocol::MeshCore => self.dec.add_mc_channel(&c.name, &c.key),
            _ => {
                let key = if c.key.trim().is_empty() {
                    "default"
                } else {
                    &c.key
                };
                self.dec.add_mt_channel(&c.name, key)
            }
        }
    }

    /// Returns the decoder the channel keys of `self.channels` again, after one left.
    fn reload_channels(&mut self) {
        self.dec.clear_user_channels();
        for c in std::mem::take(&mut self.channels) {
            if self.add_channel(&c).is_ok() {
                self.channels.push(c);
            }
        }
    }

    /// The plan in use.
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// The session report.
    pub fn report(&self) -> String {
        self.dec.store.report(&self.plan.line)
    }

    /// The store, for tests and hosts.
    pub fn store(&self) -> &MeshStore {
        &self.dec.store
    }

    fn set_plan(&mut self, p: Plan) {
        if p != self.plan {
            // A plan with no channel in the capture is the state before the stack has tuned the radio: the result
            // `searching` shows its line, the log does not.
            if !p.line.is_empty() && p.entries.iter().any(|e| e.in_band) {
                self.p.results_mut().log(p.line.clone());
            }
            let slots = p.slots != self.plan.slots;
            self.plan = p;
            self.det_frames.clear();
            self.det_frames
                .resize(self.plan.entries.len(), EntryCount::default());
            self.p.clear_rows(&MESH_DECODERS);
            if slots {
                self.publish_slots();
            }
        }
    }

    /// Writes the `slots` table from the slot grid of the plan; empty without one.
    fn publish_slots(&mut self) {
        let p = &mut self.p;
        p.clear_rows(&MESH_SLOTS);
        let Some(g) = self.plan.slots else {
            return;
        };
        let key = &mut self.key;
        for r in g.rows() {
            key.clear();
            let _ = write!(key, "{:04}", r.slot);
            MESH_SLOTS.put(
                p,
                key,
                &MeshSlotRow {
                    slot: i64::from(r.slot),
                    freq_hz: Hertz(r.freq_hz),
                    current: r.current,
                    preset_default: r.preset_default,
                },
            );
        }
    }

    fn node_event(&self, n: &MeshNodeUpdate, t: f64, freq: f64) -> sdrkit_core::Event {
        let rec = self.dec.store.node(n.protocol, &n.id);
        MESH_NODE.event(
            t,
            freq,
            &MeshNodeEvent {
                source: &n.id,
                name: rec.map(|x| x.name()),
                position: rec.and_then(|x| x.position).map(|(la, lo, _)| (la, lo)),
                net: n.protocol,
                family: keys::Family::Message,
            },
        )
    }
}

impl Component for MeshDecode {
    fn info(&self) -> ComponentInfo {
        let item = |p: Port| p.rate_from(RateFrom::Fixed(0.0));
        ComponentInfo::processor(
            "mesh.decode",
            vec![
                Port::typed::<LoraFrame>("frames"),
                Port::typed::<BankState>("state"),
                Port::typed::<Plan>("plan"),
                Port::typed::<OwnNote>("own_mt"),
                Port::typed::<OwnNote>("own_mc"),
                Port::typed::<OwnNote>("own_tx"),
                Port::typed::<OwnNote>("own_ham"),
            ],
            vec![
                item(Port::typed::<MeshPacket>("packets")),
                item(Port::typed::<MeshText>("texts")),
                item(Port::typed::<MeshNodeUpdate>("nodes")),
                item(Port::typed::<MeshReading>("readings")),
                item(Port::typed::<MtRx>("mt")),
                item(Port::typed::<McRx>("mc")),
                item(Port::new("events", Kind::Messages)),
                item(Port::typed::<HamHeard>("ham")),
            ],
        )
        .with_settings(MeshDecodeSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            Format::new(Kind::typed::<MeshPacket>(), 0.0),
            Format::new(Kind::typed::<MeshText>(), 0.0),
            Format::new(Kind::typed::<MeshNodeUpdate>(), 0.0),
            Format::new(Kind::typed::<MeshReading>(), 0.0),
            Format::new(Kind::typed::<MtRx>(), 0.0),
            Format::new(Kind::typed::<McRx>(), 0.0),
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HamHeard>(), 0.0),
        ])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.clock = ctx.clock.clone();
    }

    fn process(&mut self, io: &mut Io) {
        if let Some(t) = io.items::<BankState>(1).last().map(|s| s.value.time) {
            self.anchor(t);
        }
        let mut relist = false;
        if let Some(p) = io.items::<Plan>(2).last() {
            let p = p.value.clone();
            relist = p.mask != self.plan.mask;
            self.set_plan(p);
        }
        for port in 3..7 {
            for n in io.items::<OwnNote>(port) {
                match &n.value {
                    OwnNote::Text(m) => {
                        self.dec.store.upsert_own(m.clone());
                    }
                    OwnNote::Sent { time, payload } => self.dec.sent(*time, payload),
                    OwnNote::MtIdentity { node, key } => self.dec.mt_own = Some((*node, *key)),
                    OwnNote::McIdentity { seed } => {
                        self.dec.mc_own = Some(Identity::from_seed(seed))
                    }
                    OwnNote::McContact(k) => {
                        if !self.dec.mc_contacts.contains(k) {
                            self.dec.mc_contacts.push(*k);
                        }
                    }
                    OwnNote::MtStatus(m) => {
                        relist |= self.mt.as_ref().map(|x| &x.channels) != Some(&m.channels);
                        self.mt = Some(m.clone());
                    }
                    OwnNote::McStatus(m) => {
                        relist |= self.mc.as_ref().map(|x| &x.channels) != Some(&m.channels);
                        self.mc = Some(m.clone());
                    }
                    OwnNote::HamStatus(h) => {
                        self.dec.aprs_own.clone_from(&h.aprs_call);
                        self.ham = Some(h.clone());
                    }
                    OwnNote::HamStation(st) => self.dec.own_station(st),
                    OwnNote::SchedStatus { pending, sent } => self.sched = (*pending, *sent),
                    OwnNote::Log(l) => self.p.results_mut().log(l.clone()),
                }
            }
        }
        if relist {
            self.publish_channels();
        }
        for s in io.items::<BankState>(1) {
            self.now = s.value.time;
            let pre: u64 = s.value.detectors.iter().map(|d| d.stats.preambles).sum();
            if pre > self.preambles_seen || s.value.busy {
                self.last_signal = self.now;
            }
            self.preambles_seen = pre;
            self.header_failed = s
                .value
                .detectors
                .iter()
                .map(|d| d.stats.header_failed)
                .sum();
            self.det.clone_from(&s.value.detectors);
            self.level = Some(s.value.level_db);
        }
        for f in io.items::<LoraFrame>(0) {
            let (fr, t) = (&f.value, f.time);
            self.anchor(t);
            let Some(ei) = self.plan.entry_index(fr.tag, fr.sync) else {
                continue;
            };
            let d = self.dec.frame(fr, &self.plan.entries[ei], t);
            if let Some(e) = self.det_frames.get_mut(ei) {
                e.frames += 1;
                if fr.good() {
                    e.snr = Some(fr.snr_db);
                    e.offset = Some(fr.cfo_hz);
                } else {
                    e.errors += 1;
                }
                e.last = Some(t);
            }
            if fr.good() {
                self.run_frames += 1;
                self.last_good = t;
                self.last_snr = Some(fr.snr_db);
                self.last_offset = Some(fr.cfo_hz);
            } else {
                self.run_crc_failed += 1;
            }
            self.last_signal = self.last_signal.max(t);
            self.now = self.now.max(t);
            for l in d.logs {
                self.p.results_mut().log(l);
            }
            let freq = d.packet.freq_hz;
            for m in d.texts {
                io.items_out::<MeshText>(1)
                    .push(Timed { time: t, value: m });
            }
            for nu in d.nodes {
                let ev = self.node_event(&nu, t, freq);
                io.messages_out(6).push(ev);
                io.items_out::<MeshNodeUpdate>(2)
                    .push(Timed { time: t, value: nu });
            }
            for rd in d.readings {
                io.items_out::<MeshReading>(3)
                    .push(Timed { time: t, value: rd });
            }
            if let Some(x) = d.mt {
                io.items_out::<MtRx>(4).push(Timed { time: t, value: x });
            }
            if let Some(x) = d.mc {
                io.items_out::<McRx>(5).push(Timed { time: t, value: x });
            }
            if let Some(ev) = d.event {
                io.messages_out(6).push(ev);
            }
            if let Some(x) = d.ham {
                io.items_out::<HamHeard>(7)
                    .push(Timed { time: t, value: x });
            }
            io.items_out::<MeshPacket>(0).push(Timed {
                time: t,
                value: d.packet,
            });
        }
        self.release_held(io, false);
        if io.tick() {
            self.publish();
        }
    }

    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        self.release_held(io, true);
        self.publish();
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match MeshDecodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MeshDecodeSetting::Raw => self.raw = RAW.read(id, v)?,
            MeshDecodeSetting::Clock0 => {
                self.dec.wall0 = CLOCK0.read(id, v)?;
                self.dec.store.wall0 = self.dec.wall0;
                self.clock_given = self.dec.wall0 > 0.0;
            }
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (MeshDecodeSetting::Raw, Value::Bool(self.raw)),
            (MeshDecodeSetting::Clock0, Value::F64(self.dec.wall0)),
        ])
    }

    fn commands(&self) -> &Commands {
        MeshDecodeCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let command = MeshDecodeCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        match command {
            MeshDecodeCommand::Clear => {
                self.dec.store.clear();
                MeshStore::clear_rows(&mut self.p);
            }
            MeshDecodeCommand::Remove => {
                objects::remove(
                    &mut self.dec.store.nodes,
                    self.nodes_table,
                    &mut self.p,
                    args,
                    &MESH_NODE_ROW,
                )?;
            }
            MeshDecodeCommand::Reset => {
                self.run_frames = 0;
                self.run_crc_failed = 0;
                self.last_good = -1e9;
                self.last_signal = -1e9;
            }
            MeshDecodeCommand::MtChannelAdd | MeshDecodeCommand::McChannelAdd => {
                let c = if command == MeshDecodeCommand::McChannelAdd {
                    let key = args.get(&MC_KEY).trim();
                    let (name, _) = mc_channel(args.arg(&McChannelAddArgs::NAME)?, key)
                        .map_err(|e| bad_key(args, MC_KEY.key(), e))?;
                    UserChannel {
                        net: Protocol::MeshCore,
                        name,
                        key: key.to_string(),
                    }
                } else {
                    UserChannel {
                        net: Protocol::Meshtastic,
                        name: args.arg(&MtChannelAddArgs::NAME)?.trim().to_string(),
                        key: args.get(&KEY).trim().to_string(),
                    }
                };
                if self.channels.len() >= CHANNELS_CAP {
                    return Err(full(args));
                }
                self.add_channel(&c)
                    .map_err(|e| bad_key(args, KEY.key(), e))?;
                self.channels.retain(|x| x.net != c.net || x.name != c.name);
                self.channels.push(c);
                // A channel of the same name with another key leaves the decoder.
                self.reload_channels();
                self.publish_channels();
            }
            MeshDecodeCommand::ChannelRemove => {
                let row = args.arg(&CHANNEL_ROW)?;
                let at = self
                    .channels
                    .iter()
                    .position(|c| row.split_once('/') == Some((c.net.kind(), c.name.as_str())));
                let Some(at) = at else {
                    let table = self
                        .p
                        .results()
                        .decl()
                        .table_id(MESH_CHANNELS.key)
                        .expect("the channel table is declared");
                    return Err(CommandError::NoSuchRow {
                        table,
                        key: row.to_owned(),
                    });
                };
                let gone = self.channels.remove(at);
                // The client that sent on it drops it with the same command: its next note has the list without it.
                match gone.net {
                    Protocol::MeshCore => self.mc.as_mut().map(|m| &mut m.channels),
                    _ => self.mt.as_mut().map(|m| &mut m.channels),
                }
                .map(|names| {
                    names
                        .iter()
                        .skip(1)
                        .position(|n| *n == gone.name)
                        .map(|i| names.remove(i + 1))
                });
                self.reload_channels();
                self.publish_channels();
            }
        }
        // What is shown changes at once, not at the next report.
        self.publish();
        Ok(None)
    }

    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }

    fn forget(&mut self, kept: &Kept) {
        if !kept.table(MESH_NODES.key) {
            self.dec.store.clear();
            MeshStore::clear_rows(&mut self.p);
        }
        if !kept.field("frames_ok") {
            self.run_frames = 0;
        }
        if !kept.field("frames_bad") {
            self.run_crc_failed = 0;
        }
        self.last_snr = None;
        self.last_offset = None;
        self.level = None;
        self.last_good = -1e9;
        self.last_signal = -1e9;
        self.det.clear();
        self.det_frames.clear();
        self.det_frames
            .resize(self.plan.entries.len(), EntryCount::default());
        self.p.clear_rows(&MESH_DECODERS);
        self.publish();
    }

    fn save_state(&self) -> Option<StateBlob> {
        let s = DecodeState {
            tables: self.dec.store.save_state(self.now).ok()?,
            channels: self.channels.clone(),
        };
        Some(StateBlob::new(
            DECODE_STATE,
            1,
            serde_json::to_vec(&s).ok()?,
        ))
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let s: DecodeState = serde_json::from_slice(state.open(DECODE_STATE, 1)?)
            .map_err(|_| StateError::Damaged)?;
        let now = self.now;
        self.dec.store.restore_state(&s.tables, now)?;
        self.channels = s.channels;
        self.reload_channels();
        self.publish_channels();
        Ok(())
    }
}

/// The time of the newest bank state in a call.
fn state_time(io: &Io, port: usize, now: &mut f64) {
    for s in io.items::<BankState>(port) {
        *now = s.value.time;
    }
}

fn own_notes(io: &mut Io, port: usize, texts: Vec<MeshText>) {
    for m in texts {
        io.items_out::<OwnNote>(port).push(Timed {
            time: m.time,
            value: OwnNote::Text(m),
        });
    }
}

fn cancel_request(id: u64) -> TxRequest {
    TxRequest {
        id,
        payload: Vec::new(),
        lora: None,
        afsk: None,
        freq_hz: 0.0,
        not_before: 0.0,
        cw: 0,
        what: TxKind::Cancel,
        message_key: None,
    }
}

sdrkit_core::commands! {
    /// The commands of `mt.client`.
    pub enum MtClientCommand {
        /// Send a Meshtastic text.
        Send = MT_SEND.command,
        /// Add a Meshtastic channel.
        ChannelAdd = MT_CHANNEL_ADD.command,
        /// Remove a channel.
        ChannelRemove = CHANNEL_REMOVE.command,
        /// Set a Meshtastic channel slot.
        Channel = MT_CHANNEL.command,
        /// Set the Meshtastic key.
        Identity = MT_IDENTITY.command,
        /// Ask a Meshtastic node.
        Request = MT_REQUEST.command,
    }
}

/// An own channel as the saved state keeps it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SavedChannel {
    name: String,
    key: Vec<u8>,
    precision: u32,
}

/// The saved state of `mt.client`: the own key and channels.
#[derive(Serialize, Deserialize)]
struct MtState {
    private_key: [u8; 32],
    channels: Vec<SavedChannel>,
}

const MT_STATE: &str = "lora.mt_client";

/// `mt.client`: the Meshtastic texting client.
pub struct MtClientComp {
    /// The client.
    pub client: MtClient,
    /// The texts given before the session had its channel, with the packet ids of their replies: they are sent when it
    /// has one. A command that follows the start of the session at once is not lost.
    early: Vec<(Values, u32)>,
    planned: bool,
    now: f64,
    pending: Vec<ClientOut>,
    warnings: Vec<String>,
    node_text: String,
    callsign: String,
    position_mode: PositionMode,
    station_position: Option<(f64, f64, i32)>,
    identity_sent: bool,
    primary_name: String,
    /// The state the results show changed since the last status note: (dirty, waiting for an ack, rebroadcasts).
    shown: (bool, usize, u64),
}

impl Default for MtClientComp {
    fn default() -> Self {
        let mut rng = Rng::new(crate::mesh::os_seed());
        let node = mt_client::node_number("", "", &mut rng);
        MtClientComp {
            client: MtClient::new(MtConfig::new(node, mt::new_private_key()), rng.next_u64()),
            now: 0.0,
            pending: Vec::new(),
            warnings: Vec::new(),
            node_text: String::new(),
            callsign: String::new(),
            position_mode: PositionMode::Off,
            station_position: None,
            identity_sent: false,
            early: Vec::new(),
            planned: false,
            primary_name: String::new(),
            shown: (true, 0, 0),
        }
    }
}

impl MtClientComp {
    /// Sends the text of the `mt.send` arguments `args` with the packet id `packet`, or a new one. Returns the packet id.
    fn send(&mut self, args: &Values, packet: Option<u32>) -> Result<u32, CommandError> {
        let now = self.now;
        let to = match args.get(&MT_TO).trim() {
            "" | "^all" => mt::BROADCAST,
            s => mt::parse_node(s).ok_or_else(|| malformed(args, MT_TO.key(), None))?,
        };
        let ch = self
            .slot(args.get(&CHANNEL))
            .ok_or_else(|| no_channel(args))?;
        if to != mt::BROADCAST {
            let o = self.client.need_nodeinfo(now, to);
            self.pending.push(o);
        }
        let packet = packet.unwrap_or_else(|| self.client.new_id());
        let hop = u8::try_from(args.get(&HOP_LIMIT)).unwrap_or(0);
        let opts = TextOptions {
            reply_id: u32::try_from(args.get(&REPLY_ID)).unwrap_or(0),
            emoji: u32::try_from(args.get(&EMOJI)).unwrap_or(0),
            want_ack: args.get(&WANT_ACK).given(),
            hop_limit: (hop > 0).then_some(hop),
            id: Some(packet),
        };
        let o = self
            .client
            .send_text(now, to, ch, args.arg(&MtSendArgs::TEXT)?, opts);
        self.pending.push(o);
        Ok(packet)
    }

    /// The state the results show.
    pub fn status(&self) -> MtStatus {
        let c = &self.client.cfg;
        MtStatus {
            node_id: mt::node_name(c.node),
            public_key: mt::base64_encode(&c.public_key()),
            long_name: c.long_name.clone(),
            short_name: c.short_name.clone(),
            in_flight: self.client.in_flight(),
            rebroadcasts: self.client.rebroadcast_count,
            warnings: self.warnings.join("; "),
            freq_hz: c.tx_channel.as_ref().map(|t| t.freq_hz),
            channels: c.channels.iter().map(|ch| ch.name.clone()).collect(),
        }
    }

    /// The slot of the channel `name` (any case). An empty name is the primary channel.
    fn slot(&self, name: &str) -> Option<usize> {
        let name = name.trim();
        if name.is_empty() {
            return Some(0);
        }
        let chans = &self.client.cfg.channels;
        chans.iter().position(|c| c.name.eq_ignore_ascii_case(name))
    }

    fn check(&mut self) {
        let c = &self.client.cfg;
        self.warnings = mt_client::warnings(
            &c.long_name,
            &c.short_name,
            &self.node_text,
            f64::from(c.hop_limit),
            c.callsign_prefix,
            &self.callsign,
        );
        if c.channels.iter().any(|ch| ch.key.is_empty()) && !c.licensed {
            self.warnings
                .push("no key: the message goes out unencrypted, anyone can read it".into());
        }
        if c.licensed && self.callsign.is_empty() {
            self.warnings
                .push("licensed mode needs the callsign of the station".into());
        }
    }

    fn renumber(&mut self) {
        if mt::parse_node(&self.node_text).is_some() || !self.callsign.is_empty() {
            let mut rng = Rng::new(crate::mesh::os_seed());
            self.client.cfg.node =
                mt_client::node_number(&self.node_text, &self.callsign, &mut rng);
        }
        self.identity_sent = false;
    }

    fn apply_position(&mut self) {
        let c = &mut self.client.cfg;
        c.position_from_host = false;
        c.position = match self.position_mode {
            PositionMode::Station => self.station_position,
            PositionMode::Off => None,
        };
    }

    fn name_primary(&mut self) {
        let name = if self.primary_name.is_empty() {
            "LongFast"
        } else {
            &self.primary_name
        };
        if let Some(c) = self.client.cfg.channels.first_mut() {
            name.clone_into(&mut c.name);
        }
    }

    fn take_plan(&mut self, p: &Plan) {
        let ch = p.tx_channel(1);
        if ch != self.client.cfg.tx_channel {
            self.client.cfg.tx_channel = ch;
            self.shown.0 = true;
        }
    }

    /// The status note, when the state the results show changed.
    fn status_note(&mut self) -> Option<MtStatus> {
        let now = (self.client.in_flight(), self.client.rebroadcast_count);
        if !self.shown.0 && (self.shown.1, self.shown.2) == now {
            return None;
        }
        self.shown = (false, now.0, now.1);
        Some(self.status())
    }
}

impl Component for MtClientComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mt.client",
            vec![
                Port::typed::<MtRx>("mt"),
                Port::typed::<BankState>("state"),
                Port::typed::<SchedNote>("notes"),
                Port::typed::<Plan>("plan"),
            ],
            vec![
                Port::typed::<TxRequest>("tx").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<OwnNote>("own").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(MtClientSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.identity_sent = false;
        Ok(vec![
            Format::new(Kind::typed::<TxRequest>(), 0.0),
            Format::new(Kind::typed::<OwnNote>(), 0.0),
        ])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.client.cfg.tx_enabled = ctx.transmit;
        let call = ctx.station.callsign.trim().to_uppercase();
        if call != self.callsign {
            self.callsign = call;
            self.client.cfg.callsign.clone_from(&self.callsign);
            if mt::parse_node(&self.node_text).is_none() {
                self.renumber();
            }
        }
        self.station_position = ctx.station.position.map(|p| {
            let h = ctx.station.height_m.unwrap_or(p.alt_m);
            (p.lat_deg, p.lon_deg, h.round() as i32)
        });
        self.apply_position();
        self.check();
        self.shown.0 = true;
    }

    fn process(&mut self, io: &mut Io) {
        state_time(io, 1, &mut self.now);
        if let Some(p) = io.items::<Plan>(3).last() {
            let p = p.value.clone();
            self.take_plan(&p);
        }
        // The session settles: the radio moves to the dial and a plan with the channel follows. A
        // command that waited runs then, or after `SETTLE_S` of signal with the reason it cannot send.
        if !self.planned && (self.client.cfg.tx_channel.is_some() || self.now >= SETTLE_S) {
            self.planned = true;
            for (args, packet) in std::mem::take(&mut self.early) {
                // A refusal of a command that waited is in the log of the client, as for a command given now.
                self.shown.0 = true;
                let _ = self.send(&args, Some(packet));
            }
        }
        if !self.identity_sent {
            self.identity_sent = true;
            io.items_out::<OwnNote>(1).push(Timed {
                time: self.now,
                value: OwnNote::MtIdentity {
                    node: self.client.cfg.node,
                    key: self.client.cfg.private_key,
                },
            });
        }
        let mut outs: Vec<ClientOut> = std::mem::take(&mut self.pending);
        for n in io.items::<SchedNote>(2) {
            outs.push(match n.value {
                SchedNote::Sent { key, at, air_s } => self.client.on_sent(at, key, air_s),
                SchedNote::Waiting { key } => {
                    self.client.on_state(key, Delivery::WaitingForDutyCycle)
                }
                SchedNote::Failed { key, reason } => {
                    self.client.on_state(key, Delivery::Failed(reason))
                }
            });
        }
        for p in io.items::<MtRx>(0) {
            self.now = self.now.max(p.time);
            let o = self.client.on_packet(p.time, &p.value);
            outs.push(o);
        }
        if io.tick() {
            outs.push(self.client.tick(self.now));
            if let Some(st) = self.status_note() {
                io.items_out::<OwnNote>(1).push(Timed {
                    time: self.now,
                    value: OwnNote::MtStatus(st),
                });
            }
        }
        for o in outs.drain(..) {
            for l in o.logs {
                io.items_out::<OwnNote>(1).push(Timed {
                    time: self.now,
                    value: OwnNote::Log(l),
                });
            }
            for r in o.tx {
                io.items_out::<TxRequest>(0).push(Timed {
                    time: self.now,
                    value: r,
                });
            }
            for c in o.cancel {
                io.items_out::<TxRequest>(0).push(Timed {
                    time: self.now,
                    value: cancel_request(c),
                });
            }
            own_notes(io, 1, o.texts);
        }
        // Keep the vector's room for the next call.
        self.pending = outs;
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let c = &mut self.client.cfg;
        match MtClientSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MtClientSetting::NodeId => {
                MT_NODE_ID
                    .read(id, v)?
                    .trim()
                    .clone_into(&mut self.node_text);
                if !self.node_text.is_empty() && mt::parse_node(&self.node_text).is_none() {
                    return Err(SettingError::Malformed { id, at: None });
                }
                self.renumber();
            }
            MtClientSetting::LongName => {
                c.long_name = mt::cut_bytes(MT_LONG_NAME.read(id, v)?, 39).to_string();
            }
            MtClientSetting::ShortName => {
                c.short_name = MT_SHORT_NAME.read(id, v)?.chars().take(4).collect();
            }
            MtClientSetting::Role => c.role = MT_ROLE.read(id, v)?,
            MtClientSetting::Licensed => c.licensed = MT_LICENSED.read(id, v)?,
            MtClientSetting::CallsignPrefix => {
                c.callsign_prefix = MT_CALLSIGN_PREFIX.read(id, v)?
            }
            MtClientSetting::ChannelName => {
                MT_CHANNEL_NAME
                    .read(id, v)?
                    .trim()
                    .clone_into(&mut self.primary_name);
                self.name_primary();
            }
            MtClientSetting::HopLimit => {
                c.hop_limit = u8::try_from(MT_HOP_LIMIT.read(id, v)?)
                    .map_err(|_| SettingError::NotPermitted { id })?;
            }
            MtClientSetting::NodeinfoInterval => {
                c.nodeinfo_interval = MT_NODEINFO_INTERVAL.read(id, v)?;
            }
            MtClientSetting::PositionMode => {
                self.position_mode = MT_POSITION_MODE.read(id, v)?;
                self.apply_position();
            }
            MtClientSetting::PositionInterval => {
                c.position_interval = MT_POSITION_INTERVAL.read(id, v)?;
            }
            MtClientSetting::AckBroadcast => c.ack_broadcast = MT_ACK_BROADCAST.read(id, v)?,
            MtClientSetting::Clock0 => c.clock0 = CLOCK0.read(id, v)?,
        }
        self.check();
        self.shown.0 = true;
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        let c = &self.client.cfg;
        values_of([
            (MtClientSetting::NodeId, Value::Text(self.node_text.clone())),
            (MtClientSetting::LongName, Value::Text(c.long_name.clone())),
            (
                MtClientSetting::ShortName,
                Value::Text(c.short_name.clone()),
            ),
            (MtClientSetting::Role, Value::Choice(c.role.index())),
            (MtClientSetting::Licensed, Value::Bool(c.licensed)),
            (
                MtClientSetting::CallsignPrefix,
                Value::Bool(c.callsign_prefix),
            ),
            (
                MtClientSetting::ChannelName,
                Value::Text(self.primary_name.clone()),
            ),
            (
                MtClientSetting::HopLimit,
                Value::I64(i64::from(c.hop_limit)),
            ),
            (
                MtClientSetting::NodeinfoInterval,
                Value::F64(c.nodeinfo_interval),
            ),
            (
                MtClientSetting::PositionMode,
                Value::Choice(self.position_mode.index()),
            ),
            (
                MtClientSetting::PositionInterval,
                Value::F64(c.position_interval),
            ),
            (MtClientSetting::AckBroadcast, Value::Bool(c.ack_broadcast)),
            (MtClientSetting::Clock0, Value::F64(c.clock0)),
        ])
    }

    fn commands(&self) -> &Commands {
        MtClientCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let now = self.now;
        self.shown.0 = true;
        let command = MtClientCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        if !self.planned && command == MtClientCommand::Send {
            // The address is checked now: a refusal is not kept for later. The packet id is given now, for the reply.
            let to = args.get(&MT_TO).trim();
            if !matches!(to, "" | "^all") && mt::parse_node(to).is_none() {
                return Err(malformed(args, MT_TO.key(), None));
            }
            self.slot(args.get(&CHANNEL))
                .ok_or_else(|| no_channel(args))?;
            let packet = self.client.new_id();
            self.early.push((args.clone(), packet));
            return Ok(MT_SEND.reply(
                MtClientCommand::commands(),
                &MtSendReply {
                    packet_id: i64::from(packet),
                },
            ));
        }
        match command {
            MtClientCommand::ChannelAdd => {
                let name = args.arg(&MtChannelAddArgs::NAME)?.trim().to_string();
                let key_text = match args.get(&KEY).trim() {
                    "" => "default",
                    k => k,
                };
                let key = mt::parse_key_text(key_text)
                    .ok_or_else(|| bad_key(args, KEY.key(), KeyError::NotKeyText))?;
                let chans = &mut self.client.cfg.channels;
                match chans.iter().position(|c| c.name == name) {
                    Some(i) => chans[i].key = key,
                    None if chans.len() < 8 => chans.push(OwnChannel {
                        name,
                        key,
                        precision: 13,
                    }),
                    None => return Err(full(args)),
                }
                self.check();
            }
            MtClientCommand::ChannelRemove => {
                // The primary channel stays.
                if let Some(("meshtastic", name)) = args.arg(&CHANNEL_ROW)?.split_once('/') {
                    let chans = &mut self.client.cfg.channels;
                    if let Some(i) = chans.iter().skip(1).position(|c| c.name == name) {
                        chans.remove(i + 1);
                    }
                }
            }
            MtClientCommand::Channel => {
                let index = usize::try_from(args.get(&INDEX)).unwrap_or(0);
                let key_text = match args.get(&KEY).trim() {
                    "" => "default",
                    k => k,
                };
                let key = mt::parse_key_text(key_text)
                    .ok_or_else(|| bad_key(args, KEY.key(), KeyError::NotKeyText))?;
                let chans = &mut self.client.cfg.channels;
                if index > chans.len() {
                    let id = args.schema().id(INDEX.key()).unwrap_or(SettingId(0));
                    return Err(CommandError::BadArgument {
                        arg: id,
                        error: SettingError::NotPermitted { id },
                    });
                }
                let ch = OwnChannel {
                    name: args.arg(&MtChannelArgs::NAME)?.trim().to_string(),
                    key,
                    precision: u32::try_from(args.get(&PRECISION)).unwrap_or(13),
                };
                if index == chans.len() {
                    chans.push(ch);
                } else {
                    chans[index] = ch;
                }
                self.name_primary();
                self.check();
            }
            MtClientCommand::Identity => {
                let key = key32(args.arg(&IdentityArgs::KEY)?).ok_or_else(|| {
                    bad_key(args, IdentityArgs::KEY.field.key, KeyError::NotKeyText)
                })?;
                self.client.cfg.private_key = key;
                self.identity_sent = false;
            }
            MtClientCommand::Send => {
                let packet = self.send(args, None)?;
                return Ok(MT_SEND.reply(
                    MtClientCommand::commands(),
                    &MtSendReply {
                        packet_id: i64::from(packet),
                    },
                ));
            }
            MtClientCommand::Request => {
                let node = mt::parse_node(args.arg(&MtRequestArgs::NODE)?)
                    .ok_or_else(|| malformed(args, MtRequestArgs::NODE.field.key, None))?;
                let o = self.client.request_from(now, node, args.get(&WHAT));
                self.pending.push(o);
            }
        }
        Ok(None)
    }

    fn save_state(&self) -> Option<StateBlob> {
        let c = &self.client.cfg;
        let s = MtState {
            private_key: c.private_key,
            channels: c
                .channels
                .iter()
                .map(|ch| SavedChannel {
                    name: ch.name.clone(),
                    key: ch.key.clone(),
                    precision: ch.precision,
                })
                .collect(),
        };
        Some(StateBlob::new(MT_STATE, 1, serde_json::to_vec(&s).ok()?))
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let s: MtState =
            serde_json::from_slice(state.open(MT_STATE, 1)?).map_err(|_| StateError::Damaged)?;
        let c = &mut self.client.cfg;
        c.private_key = s.private_key;
        if !s.channels.is_empty() {
            c.channels = s
                .channels
                .into_iter()
                .map(|ch| OwnChannel {
                    name: ch.name,
                    key: ch.key,
                    precision: ch.precision,
                })
                .collect();
        }
        self.name_primary();
        self.identity_sent = false;
        self.check();
        self.shown.0 = true;
        Ok(())
    }
}

sdrkit_core::commands! {
    /// The commands of `mc.client`.
    pub enum McClientCommand {
        /// Set the MeshCore identity.
        Identity = MC_IDENTITY.command,
        /// Add a MeshCore channel.
        ChannelAdd = MC_CHANNEL_ADD.command,
        /// Remove a channel.
        ChannelRemove = CHANNEL_REMOVE.command,
        /// Send a MeshCore text.
        Send = MC_SEND.command,
        /// Send an advert.
        Advert = MC_ADVERT.command,
        /// Add a MeshCore contact.
        ContactAdd = CONTACT_ADD.command,
    }
}

/// The saved state of `mc.client`: the identity seed, the channels and the contacts that the user added.
#[derive(Serialize, Deserialize)]
struct McState {
    seed: [u8; 32],
    channels: Vec<(String, Vec<u8>)>,
    contacts: Vec<([u8; 32], String)>,
}

const MC_STATE: &str = "lora.mc_client";

/// `mc.client`: the MeshCore texting client.
pub struct McClientComp {
    /// The client.
    pub client: McClient,
    /// The commands that send, given before the session had its channel: they run when it has one. A command that
    /// follows the start of the session at once is not lost.
    early: Vec<(CommandId, Values)>,
    planned: bool,
    now: f64,
    pending: Vec<McOut>,
    seed: [u8; 32],
    identity_sent: bool,
    share_location: bool,
    station_position: Option<(f64, f64)>,
    /// The contacts that the user added: the key and the name.
    added: Vec<([u8; 32], String)>,
    /// The added contacts the decoder has heard of.
    told: usize,
    /// The state the results show changed since the last status note.
    shown: bool,
}

impl Default for McClientComp {
    fn default() -> Self {
        let mut seed = [0u8; 32];
        if getrandom::getrandom(&mut seed).is_err() {
            seed = sdrkit_algo_crypto::sha256(&crate::mesh::os_seed().to_le_bytes());
        }
        McClientComp {
            client: McClient::new(
                McConfig::new(Identity::from_seed(&seed)),
                Rng::new(crate::mesh::os_seed()).next_u64(),
            ),
            now: 0.0,
            pending: Vec::new(),
            seed,
            identity_sent: false,
            early: Vec::new(),
            planned: false,
            share_location: false,
            station_position: None,
            added: Vec::new(),
            told: 0,
            shown: true,
        }
    }
}

impl McClientComp {
    /// The position of the channel `name` (any case, with or without the `#`). An empty name is the first channel.
    fn slot(&self, name: &str) -> Option<usize> {
        let name = name.trim();
        if name.is_empty() {
            return Some(0);
        }
        let bare = name.trim_start_matches('#');
        self.client.cfg.channels.iter().position(|c| {
            c.0.eq_ignore_ascii_case(name) || c.0.trim_start_matches('#').eq_ignore_ascii_case(bare)
        })
    }

    /// The state the results show.
    pub fn status(&self) -> McStatus {
        let c = &self.client.cfg;
        McStatus {
            public_key: mt::hex(&c.identity.public()),
            name: c.name.clone(),
            freq_hz: c.tx_channel.as_ref().map(|t| t.freq_hz),
            channels: c.channels.iter().map(|ch| ch.0.clone()).collect(),
            contacts: self
                .client
                .contacts
                .iter()
                .map(|row| ContactRow {
                    key: row.key.clone(),
                    name: row.record.name.clone(),
                    last_heard: (c.clock0 > 0.0).then(|| (c.clock0 + row.seen.last_s).floor()),
                    snr_db: row.record.snr,
                    path: row
                        .record
                        .path
                        .as_ref()
                        .map(|p| p.iter().map(|h| mt::hex(h)).collect::<Vec<_>>().join(","))
                        .unwrap_or_else(|| "flood".into()),
                })
                .collect(),
        }
    }

    fn take_plan(&mut self, p: &Plan) {
        let ch = p.tx_channel(2);
        if ch != self.client.cfg.tx_channel {
            self.client.cfg.tx_channel = ch;
            self.shown = true;
        }
    }

    fn apply_location(&mut self) {
        self.client.cfg.location = if self.share_location {
            self.station_position
        } else {
            None
        };
    }
}

impl Component for McClientComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mc.client",
            vec![
                Port::typed::<McRx>("mc"),
                Port::typed::<BankState>("state"),
                Port::typed::<SchedNote>("notes"),
                Port::typed::<Plan>("plan"),
            ],
            vec![
                Port::typed::<TxRequest>("tx").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<OwnNote>("own").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(McClientSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.identity_sent = false;
        Ok(vec![
            Format::new(Kind::typed::<TxRequest>(), 0.0),
            Format::new(Kind::typed::<OwnNote>(), 0.0),
        ])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.client.cfg.tx_enabled = ctx.transmit;
        self.station_position = ctx.station.position.map(|p| (p.lat_deg, p.lon_deg));
        self.apply_location();
        self.shown = true;
    }

    fn process(&mut self, io: &mut Io) {
        state_time(io, 1, &mut self.now);
        if let Some(p) = io.items::<Plan>(3).last() {
            let p = p.value.clone();
            self.take_plan(&p);
        }
        // The session settles: the radio moves to the dial and a plan with the channel follows. A
        // command that waited runs then, or after `SETTLE_S` of signal with the reason it cannot send.
        if !self.planned && (self.client.cfg.tx_channel.is_some() || self.now >= SETTLE_S) {
            self.planned = true;
            for (id, args) in std::mem::take(&mut self.early) {
                // A refusal of a command that waited is in the log of the client, as for a command given now.
                let _ = self.invoke(id, &args);
            }
        }
        if !self.identity_sent {
            self.identity_sent = true;
            io.items_out::<OwnNote>(1).push(Timed {
                time: self.now,
                value: OwnNote::McIdentity { seed: self.seed },
            });
        }
        for (k, _) in &self.added[self.told..] {
            io.items_out::<OwnNote>(1).push(Timed {
                time: self.now,
                value: OwnNote::McContact(*k),
            });
        }
        self.told = self.added.len();
        let mut outs: Vec<McOut> = std::mem::take(&mut self.pending);
        for n in io.items::<SchedNote>(2) {
            if let SchedNote::Sent { key, at, air_s } = n.value {
                outs.push(self.client.on_sent(at, key, air_s));
            }
        }
        for p in io.items::<McRx>(0) {
            self.now = self.now.max(p.time);
            outs.push(self.client.on_packet(p.time, &p.value));
            self.shown = true;
        }
        if io.tick() {
            outs.push(self.client.tick(self.now));
            if std::mem::take(&mut self.shown) {
                let st = self.status();
                io.items_out::<OwnNote>(1).push(Timed {
                    time: self.now,
                    value: OwnNote::McStatus(st),
                });
            }
        }
        for o in outs.drain(..) {
            for l in o.logs {
                io.items_out::<OwnNote>(1).push(Timed {
                    time: self.now,
                    value: OwnNote::Log(l),
                });
            }
            for r in o.tx {
                io.items_out::<TxRequest>(0).push(Timed {
                    time: self.now,
                    value: r,
                });
            }
            own_notes(io, 1, o.texts);
        }
        self.pending = outs;
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let c = &mut self.client.cfg;
        match McClientSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            McClientSetting::Name => c.name = mt::cut_bytes(MC_NAME.read(id, v)?, 31).to_string(),
            McClientSetting::ShareLocation => {
                self.share_location = MC_SHARE_LOCATION.read(id, v)?;
                self.apply_location();
            }
            McClientSetting::AdvertInterval => {
                c.advert_interval = MC_ADVERT_INTERVAL.read(id, v)?
            }
            McClientSetting::Clock0 => c.clock0 = CLOCK0.read(id, v)?,
        }
        self.shown = true;
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        let c = &self.client.cfg;
        values_of([
            (McClientSetting::Name, Value::Text(c.name.clone())),
            (
                McClientSetting::ShareLocation,
                Value::Bool(self.share_location),
            ),
            (
                McClientSetting::AdvertInterval,
                Value::F64(c.advert_interval),
            ),
            (McClientSetting::Clock0, Value::F64(c.clock0)),
        ])
    }

    fn commands(&self) -> &Commands {
        McClientCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let now = self.now;
        self.shown = true;
        let command = McClientCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        if !self.planned && matches!(command, McClientCommand::Send | McClientCommand::Advert) {
            if command == McClientCommand::Send {
                self.slot(args.get(&CHANNEL))
                    .ok_or_else(|| no_channel(args))?;
            }
            self.early.push((id, args.clone()));
            return Ok(None);
        }
        match command {
            McClientCommand::Identity => {
                let seed = key32(args.arg(&IdentityArgs::KEY)?).ok_or_else(|| {
                    bad_key(args, IdentityArgs::KEY.field.key, KeyError::NotKeyText)
                })?;
                self.seed = seed;
                self.client.cfg.identity = Identity::from_seed(&seed);
                self.identity_sent = false;
            }
            McClientCommand::ChannelAdd => {
                let (name, secret) =
                    mc_channel(args.arg(&McChannelAddArgs::NAME)?, args.get(&MC_KEY))
                        .map_err(|e| bad_key(args, MC_KEY.key(), e))?;
                let chans = &mut self.client.cfg.channels;
                match chans.iter_mut().find(|c| c.1 == secret) {
                    Some(c) => c.0 = name,
                    None => chans.push((name, secret)),
                }
            }
            McClientCommand::ChannelRemove => {
                // The channel Public stays.
                if let Some(("meshcore", name)) = args.arg(&CHANNEL_ROW)?.split_once('/') {
                    let chans = &mut self.client.cfg.channels;
                    if let Some(i) = chans.iter().skip(1).position(|c| c.0 == name) {
                        chans.remove(i + 1);
                    }
                }
            }
            McClientCommand::Send => {
                let body = args.arg(&McSendArgs::TEXT)?;
                let o = match args.get(&MC_TO).trim() {
                    "" => {
                        let ch = self
                            .slot(args.get(&CHANNEL))
                            .ok_or_else(|| no_channel(args))?;
                        self.client.send_channel(now, ch, body)
                    }
                    to => self.client.send_dm(now, to, body),
                };
                self.pending.push(o);
            }
            McClientCommand::Advert => {
                let o = self
                    .client
                    .advert(now, args.get(&SCOPE) == AdvertScope::Flood);
                self.pending.push(o);
            }
            McClientCommand::ContactAdd => {
                let text = args.arg(&McContactAddArgs::KEY)?;
                let name = args.get(&CONTACT_NAME);
                self.client
                    .add_contact(now, text, name)
                    .map_err(|e| bad_key(args, McContactAddArgs::KEY.field.key, e))?;
                if let Some(k) = mc::parse_secret(text).and_then(|k| <[u8; 32]>::try_from(k).ok()) {
                    self.added.retain(|(x, _)| *x != k);
                    self.told = self.told.min(self.added.len());
                    self.added.push((k, name.to_string()));
                }
            }
        }
        Ok(None)
    }

    fn save_state(&self) -> Option<StateBlob> {
        let s = McState {
            seed: self.seed,
            channels: self.client.cfg.channels.clone(),
            contacts: self.added.clone(),
        };
        Some(StateBlob::new(MC_STATE, 1, serde_json::to_vec(&s).ok()?))
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let s: McState =
            serde_json::from_slice(state.open(MC_STATE, 1)?).map_err(|_| StateError::Damaged)?;
        self.seed = s.seed;
        self.client.cfg.identity = Identity::from_seed(&s.seed);
        if !s.channels.is_empty() {
            self.client.cfg.channels = s.channels;
        }
        for (k, name) in &s.contacts {
            // The key was checked when it was added. A contact that does not fit the full table is left out.
            let _ = self.client.add_contact(self.now, &mt::hex(k), name);
        }
        self.added = s.contacts;
        self.told = 0;
        self.identity_sent = false;
        self.shown = true;
        Ok(())
    }
}

sdrkit_core::commands! {
    /// The commands of `ham.client`.
    pub enum HamClientCommand {
        /// Send an APRS message.
        AprsSend = APRS_SEND.command,
        /// Send the APRS beacon.
        AprsBeacon = APRS_BEACON,
    }
}

/// `ham.client` and `ham.client.ax25`: the client of the amateur protocols. It sends APRS messages and beacons under the
/// callsign of the station, as LoRa APRS frames in a mesh stack (`ham.client`) or as AX.25 frames on FM in the `aprs`
/// stack (`ham.client.ax25`).
///
/// Inputs: `state` and `plan` (the LoRa bank: the time and the LoRa APRS channel), `notes` (the scheduler), `signal` (the
/// FM front end: the time and the tuned frequency) and `heard` (answers and messages to the station). The LoRa client
/// reads `state` and `plan`, the AX.25 client `signal` and `heard`. Outputs: `tx` (frames for the scheduler) and `own`
/// (own messages, stations, status and log lines).
pub struct HamClientComp {
    /// The client.
    pub client: HamClient,
    /// AX.25 on FM rather than LoRa APRS.
    afsk: bool,
    /// The results field that refuses a command without a valid call.
    call_field: FieldId,
    /// The commands given before the session had its channel, with the message numbers of their replies. They run when
    /// it has one.
    early: Vec<(CommandId, Values, u32)>,
    planned: bool,
    now: f64,
    pending: Vec<HamOut>,
    /// The state the results show changed since the last status note.
    shown: bool,
}

impl Default for HamClientComp {
    fn default() -> Self {
        HamClientComp::new(
            false,
            mesh_decl()
                .field_id(HamFields::APRS_CALL.key)
                .expect("the call field is declared"),
        )
    }
}

impl HamClientComp {
    /// A client for LoRa APRS (`afsk` false) or AX.25 on FM (`afsk` true). `call_field` is the field of the own call in
    /// the results of the stack.
    pub fn new(afsk: bool, call_field: FieldId) -> Self {
        HamClientComp {
            client: HamClient::new(HamConfig::default(), crate::mesh::os_seed()),
            afsk,
            call_field,
            early: Vec::new(),
            planned: false,
            now: 0.0,
            pending: Vec::new(),
            shown: true,
        }
    }

    /// The client of the `aprs` stack: AX.25 frames on FM.
    pub fn afsk() -> Self {
        HamClientComp::new(true, super::aprs::call_field())
    }

    /// Returns true if `call` may be an address of an own frame: a LoRa APRS call, and on AX.25 an AX.25 address.
    fn address_ok(&self, call: &str) -> bool {
        hamlora::call_ok(call) && (!self.afsk || ax25_ok(call))
    }

    /// The refusal of a command while the station has no valid source call.
    fn no_call(&self, call: String) -> CommandError {
        CommandError::NotNow {
            field: self.call_field,
            state: Value::Text(call),
        }
    }

    /// Checks the arguments of `aprs.send`: the addressee is a call of at most nine characters, the text is not empty,
    /// fits a message and holds none of the characters that APRS reserves. Returns the addressee in upper case.
    fn check_send(args: &Values) -> Result<String, CommandError> {
        let a: AprsSendArgs = APRS_SEND.args(args)?;
        let to = a.to.trim().to_uppercase();
        if to.len() > 9 || !hamlora::call_ok(&to) {
            return Err(malformed(args, AprsSendArgs::TO.field.key, None));
        }
        let id = args
            .schema()
            .id(AprsSendArgs::TEXT.field.key)
            .unwrap_or(SettingId(0));
        if let Some(at) = message_text_fault(a.text) {
            return Err(malformed(args, AprsSendArgs::TEXT.field.key, Some(at)));
        }
        if a.text.is_empty() || a.text.chars().count() > MESSAGE_TEXT_MAX {
            return Err(CommandError::BadArgument {
                arg: id,
                error: SettingError::NotPermitted { id },
            });
        }
        Ok(to)
    }

    /// Runs a command whose arguments were checked. `number` is the message number of `aprs.send`.
    fn run(&mut self, command: HamClientCommand, args: &Values, number: u32) {
        let now = self.now;
        let o = match command {
            HamClientCommand::AprsSend => {
                let Ok(to) = Self::check_send(args) else {
                    return;
                };
                let Ok(text) = args.arg(&AprsSendArgs::TEXT).map(str::to_string) else {
                    return;
                };
                self.client.send_message(now, &to, &text, number)
            }
            HamClientCommand::AprsBeacon => self.client.beacon(now),
        };
        self.pending.push(o);
    }

    fn status(&self) -> HamStatus {
        let c = &self.client.cfg;
        HamStatus {
            aprs_call: c.call(),
            aprs_freq_hz: c.link.as_ref().map(HamLink::freq_hz),
        }
    }

    /// Reads the time and the link: from the LoRa bank, or from the FM front end.
    fn read_link(&mut self, io: &Io) {
        if self.afsk {
            for s in io.items::<AprsSignal>(3) {
                self.now = s.value.time;
                let link = s.value.tuned_hz.map(|f| HamLink::Afsk { freq_hz: f });
                if link != self.client.cfg.link {
                    self.client.cfg.link = link;
                    self.shown = true;
                }
            }
            return;
        }
        state_time(io, 0, &mut self.now);
        if let Some(p) = io.items::<Plan>(2).last() {
            let link = p.value.tx_channel(4).map(HamLink::Lora);
            if link != self.client.cfg.link {
                self.client.cfg.link = link;
                self.shown = true;
            }
        }
    }

    /// Writes what the client requests: frames to `tx`, the rest to `own`.
    fn emit(&self, io: &mut Io, o: HamOut) {
        for l in o.logs {
            io.items_out::<OwnNote>(1).push(Timed {
                time: self.now,
                value: OwnNote::Log(l),
            });
        }
        for r in o.tx {
            io.items_out::<TxRequest>(0).push(Timed {
                time: self.now,
                value: r,
            });
        }
        for s in o.stations {
            io.items_out::<OwnNote>(1).push(Timed {
                time: s.time,
                value: OwnNote::HamStation(s),
            });
        }
        let call = self.client.cfg.call();
        let protocol = if self.afsk {
            Protocol::Aprs
        } else {
            Protocol::LoraAprs
        };
        own_notes(
            io,
            1,
            o.messages
                .iter()
                .map(|m| m.mesh_text(&call, protocol))
                .collect(),
        );
    }
}

impl Component for HamClientComp {
    fn info(&self) -> ComponentInfo {
        // The AX.25 client is a separate kind: it checks calls by the AX.25 rule and reads other inputs.
        ComponentInfo::processor(
            if self.afsk { "ham.client.ax25" } else { "ham.client" },
            vec![
                Port::typed::<BankState>("state").optional("the time from the FM front end"),
                Port::typed::<SchedNote>("notes"),
                Port::typed::<Plan>("plan").optional("no LoRa APRS channel"),
                Port::typed::<AprsSignal>("signal").optional("the time from the LoRa bank"),
                Port::typed::<HamHeard>("heard").optional("no answers to own messages"),
            ],
            vec![
                Port::typed::<TxRequest>("tx").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<OwnNote>("own").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(HamClientSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.shown = true;
        Ok(vec![
            Format::new(Kind::typed::<TxRequest>(), 0.0),
            Format::new(Kind::typed::<OwnNote>(), 0.0),
        ])
    }

    fn attach(&mut self, ctx: &RunContext) {
        let c = &mut self.client.cfg;
        c.tx_enabled = ctx.transmit;
        c.callsign = ctx.station.callsign.trim().to_uppercase();
        c.position = ctx.station.position;
        self.shown = true;
    }

    fn process(&mut self, io: &mut Io) {
        self.read_link(io);
        // A command that waited for the channel runs when the session has one, or after `SETTLE_S` of signal with the
        // reason it cannot send.
        if !self.planned && (self.client.cfg.link.is_some() || self.now >= SETTLE_S) {
            self.planned = true;
            for (id, args, number) in std::mem::take(&mut self.early) {
                if let Some(c) = HamClientCommand::from_id(id) {
                    self.run(c, &args, number);
                }
            }
        }
        let mut outs: Vec<HamOut> = std::mem::take(&mut self.pending);
        for n in io.items::<SchedNote>(1) {
            outs.push(match n.value {
                SchedNote::Sent { key, at, .. } => self.client.on_sent(at, key),
                SchedNote::Waiting { key } => {
                    self.client.on_state(key, Delivery::WaitingForDutyCycle)
                }
                SchedNote::Failed { key, reason } => {
                    self.client.on_state(key, Delivery::Failed(reason))
                }
            });
        }
        for h in io.items::<HamHeard>(4) {
            outs.push(self.client.heard(self.now, &h.value));
        }
        if io.tick() {
            let call_ok = self.address_ok(&self.client.cfg.call());
            outs.push(self.client.tick(self.now, call_ok));
            if std::mem::take(&mut self.shown) {
                io.items_out::<OwnNote>(1).push(Timed {
                    time: self.now,
                    value: OwnNote::HamStatus(self.status()),
                });
            }
        }
        for o in outs.drain(..) {
            self.emit(io, o);
        }
        self.pending = outs;
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let afsk = self.afsk;
        let c = &mut self.client.cfg;
        let bad = SettingError::Malformed { id, at: None };
        // A call of a header: a LoRa APRS call of at most nine characters, on AX.25 an AX.25 address.
        let call_ok = |t: &str| t.len() <= 9 && hamlora::call_ok(t) && (!afsk || ax25_ok(t));
        match HamClientSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            HamClientSetting::AprsSsid => {
                c.ssid = u8::try_from(APRS_SSID.read(id, v)?)
                    .map_err(|_| SettingError::NotPermitted { id })?;
            }
            HamClientSetting::AprsTocall => {
                let t = APRS_TOCALL.read(id, v)?.trim().to_uppercase();
                if !call_ok(&t) {
                    return Err(bad);
                }
                c.tocall = t;
            }
            HamClientSetting::AprsPath => {
                let t = APRS_PATH.read(id, v)?.trim().to_uppercase();
                let parts: Vec<&str> = t.split(',').map(str::trim).collect();
                let ok = t.is_empty() || (parts.len() <= 8 && parts.iter().all(|p| call_ok(p)));
                if !ok {
                    return Err(bad);
                }
                c.path = parts.join(",");
            }
            HamClientSetting::AprsSymbol => {
                let t = APRS_SYMBOL.read(id, v)?;
                if t.len() != 2 || !t.bytes().all(|b| (0x21..0x7F).contains(&b)) {
                    return Err(bad);
                }
                c.symbol = t.to_string();
            }
            HamClientSetting::AprsComment => {
                let t = APRS_COMMENT.read(id, v)?;
                if t.chars().any(char::is_control) {
                    return Err(bad);
                }
                c.comment = t.to_string();
            }
            HamClientSetting::TxBaud => c.afsk.baud = TX_BAUD.read(id, v)?.baud(),
            HamClientSetting::TxDelay => c.afsk.txdelay_s = TX_DELAY.read(id, v)?,
            HamClientSetting::TxPreEmphasis => c.afsk.pre_emphasis = TX_PRE_EMPHASIS.read(id, v)?,
            HamClientSetting::BeaconInterval => c.beacon_interval = BEACON_INTERVAL.read(id, v)?,
        }
        self.shown = true;
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        let c = &self.client.cfg;
        let speed = if c.afsk.baud == 9600 {
            PacketSpeed::G3ruh9600
        } else {
            PacketSpeed::Afsk1200
        };
        values_of([
            (HamClientSetting::AprsSsid, Value::I64(i64::from(c.ssid))),
            (HamClientSetting::AprsTocall, Value::Text(c.tocall.clone())),
            (HamClientSetting::AprsPath, Value::Text(c.path.clone())),
            (HamClientSetting::AprsSymbol, Value::Text(c.symbol.clone())),
            (
                HamClientSetting::AprsComment,
                Value::Text(c.comment.clone()),
            ),
            (HamClientSetting::TxBaud, Value::Choice(speed.index())),
            (HamClientSetting::TxDelay, Value::F64(c.afsk.txdelay_s)),
            (
                HamClientSetting::TxPreEmphasis,
                Value::Bool(c.afsk.pre_emphasis),
            ),
            (
                HamClientSetting::BeaconInterval,
                Value::F64(c.beacon_interval),
            ),
        ])
    }

    fn commands(&self) -> &Commands {
        HamClientCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let command = HamClientCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        self.shown = true;
        let call = self.client.cfg.call();
        if !self.address_ok(&call) {
            return Err(self.no_call(call));
        }
        let number = match command {
            HamClientCommand::AprsSend => {
                Self::check_send(args)?;
                self.client.next_number()
            }
            HamClientCommand::AprsBeacon => 0,
        };
        if self.planned {
            self.run(command, args, number);
        } else {
            self.early.push((id, args.clone(), number));
        }
        Ok(match command {
            HamClientCommand::AprsSend => APRS_SEND.reply(
                HamClientCommand::commands(),
                &AprsSendReply {
                    packet_id: i64::from(number),
                },
            ),
            HamClientCommand::AprsBeacon => None,
        })
    }
}

/// Returns the scheduler's notes and the frames it sends at signal time `now`.
fn emit(io: &mut Io, now: f64, out: &mut Vec<TxRequest>, ev: &mut Vec<SchedEvent>) {
    for e in ev.drain(..) {
        let note = match e {
            SchedEvent::Sent {
                message_key,
                at,
                air_s,
                ..
            } => SchedNote::Sent {
                key: message_key,
                at,
                air_s,
            },
            SchedEvent::WaitingForDutyCycle { message_key } => {
                SchedNote::Waiting { key: message_key }
            }
            SchedEvent::Failed {
                message_key,
                reason,
            } => SchedNote::Failed {
                key: message_key,
                reason,
            },
        };
        io.items_out::<SchedNote>(1).push(Timed {
            time: now,
            value: note,
        });
    }
    for r in out.drain(..) {
        io.items_out::<OwnNote>(2).push(Timed {
            time: now,
            value: OwnNote::Sent {
                time: now,
                payload: r.payload.clone(),
            },
        });
        io.items_out::<OwnNote>(2).push(Timed {
            time: now,
            value: OwnNote::Log(format!(
                "TX: {} of {} bytes at {:.6} MHz",
                TxKind::CHOICES[usize::from(r.what.index())].label,
                r.payload.len(),
                r.freq_hz / 1e6
            )),
        });
        io.items_out::<TxRequest>(0).push(Timed {
            time: now,
            value: r,
        });
    }
}

/// `tx.sched`: the transmit scheduler. Frames that are due leave as `tx.request` items, for `lora.modulate` and as
/// `tx.request` events of the graph. It runs at the times of `state` (the LoRa bank: busy while a detector is inside a
/// frame) or of `signal` (the FM front end: busy while a carrier is heard).
pub struct TxSchedComp {
    /// The scheduler.
    pub sched: TxScheduler,
    now: f64,
    out: Vec<TxRequest>,
    ev: Vec<SchedEvent>,
    /// The text of the setting `duty_bands`.
    duty_bands: String,
}

impl Default for TxSchedComp {
    fn default() -> Self {
        let mut sched = TxScheduler::new(Rng::new(crate::mesh::os_seed()).next_u64());
        sched.on = false;
        TxSchedComp {
            sched,
            now: 0.0,
            out: Vec::new(),
            ev: Vec::new(),
            duty_bands: String::new(),
        }
    }
}

impl Component for TxSchedComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "tx.sched",
            vec![
                Port::typed::<TxRequest>("mt").optional("no Meshtastic frames"),
                Port::typed::<TxRequest>("mc").optional("no MeshCore frames"),
                Port::typed::<TxRequest>("ham"),
                Port::typed::<BankState>("state").optional("the times of the FM front end"),
                Port::typed::<Plan>("plan").optional("every frequency is kept"),
                Port::typed::<AprsSignal>("signal").optional("the times of the LoRa bank"),
            ],
            vec![
                Port::typed::<TxRequest>("out").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<SchedNote>("notes").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<OwnNote>("own").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(SchedSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            Format::new(Kind::typed::<TxRequest>(), 0.0),
            Format::new(Kind::typed::<SchedNote>(), 0.0),
            Format::new(Kind::typed::<OwnNote>(), 0.0),
        ])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.sched.on = ctx.transmit;
    }

    fn process(&mut self, io: &mut Io) {
        for i in 0..3 {
            for r in io.items::<TxRequest>(i) {
                if r.value.what == TxKind::Cancel {
                    self.sched.cancel(r.value.id);
                } else {
                    self.sched.push(r.value.clone());
                }
            }
        }
        if let Some(p) = io.items::<Plan>(4).last() {
            self.sched.keep(|hz| p.value.listens_to(hz), &mut self.ev);
        }
        // The bank outputs a state at each clock tick and at each busy change, so the back-off sees every frame. The FM
        // front end outputs its signal at each clock tick.
        let times: Vec<(f64, bool)> = io
            .items::<BankState>(3)
            .iter()
            .map(|s| (s.value.time, s.value.busy))
            .chain(
                io.items::<AprsSignal>(5)
                    .iter()
                    .map(|s| (s.value.time, s.value.busy)),
            )
            .collect();
        if times.is_empty() {
            return;
        }
        for (now, busy) in times {
            self.now = now;
            self.sched.poll(now, busy, &mut self.out, &mut self.ev);
            emit(io, now, &mut self.out, &mut self.ev);
        }
        if io.tick() {
            io.items_out::<OwnNote>(2).push(Timed {
                time: self.now,
                value: OwnNote::SchedStatus {
                    pending: self.sched.pending(),
                    sent: self.sched.sent,
                },
            });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SchedSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SchedSetting::DutyLimit => self.sched.duty.on = DUTY_LIMIT.read(id, v)?,
            SchedSetting::DutyBands => {
                let t = DUTY_BANDS.read(id, v)?;
                self.sched.duty.bands = crate::mesh::duty::parse_bands(t)
                    .ok_or(SettingError::Malformed { id, at: None })?;
                self.duty_bands = t.to_string();
            }
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (SchedSetting::DutyLimit, Value::Bool(self.sched.duty.on)),
            (
                SchedSetting::DutyBands,
                Value::Text(self.duty_bands.clone()),
            ),
        ])
    }
}
