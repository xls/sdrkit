//! Thread on 2.4 GHz (`thread`): the 802.15.4 receiver of `zigbee` with a reader for Thread above the MAC. It shows what a
//! listener sees without the network key: beacons with the network name, devices, 6LoWPAN headers, and MLE and CoAP where they
//! are sent in the clear. A frame that the link layer encrypts is marked. Nothing is decrypted.
//! Standards: IEEE 802.15.4-2020 clauses 7 and 12, RFC 4944, RFC 6282, RFC 7252, the Thread specification (beacon, MLE, ports).

use crate::family::{at_least_value, min_rate_message, set_status, CleanScene, REPORT_S};
use crate::mac154::{self, Addr, Mac};
use crate::typed::{
    ThreadDeviceEvent, ThreadFields, ThreadFrameRow, ThreadNetworkEvent, ThreadNetworkRow,
    PAN_DEVICES, PAN_DEVICE_ROW, PAN_REMOVE, THREAD_DEVICE, THREAD_FRAMES, THREAD_NETWORK,
    THREAD_NETWORKS,
};
use crate::zigbee::{
    channel_hz, channel_of, oqpsk, pan_text, ppdu_chip_pairs, with_fcs, Chips, DecoderCounts,
    Devices, Frame154, FrameDec, Front, CHANNEL_SETTING, DEVICE_CAP, DWELL, FRAME_CAP, GEN_BAD_FCS,
    GEN_INTERFERER_DB, GEN_INTERFERER_HZ, NETWORK_CAP,
};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, ReadFields, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, Event, GenConfig, Generator, HostRequest, LockProfile,
    LockState, Receiver, Record, Results, ResultsDecl, Setting, SettingError, SettingId,
    SettingSet, SignalInfo, StackConfig, StackDescriptor, StateBlob, StateError, TableId, Tuning,
    Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Kept, Kind, Port, RateFrom, StackGraph, Timed,
};
use sdrkit_proto_6lowpan::{self as lowpan, coap, DstAddress, LinkAddress, NextHeader, SrcAddress};
use sdrkit_util_objects::commands;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::Tables as _;
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::sync::Arc;

/// The UDP port of MLE.
pub const MLE_PORT: u16 = 19788;
/// The UDP port of CoAP.
pub const COAP_PORT: u16 = 5683;
/// The UDP port of Thread Management, which is CoAP.
pub const MANAGEMENT_PORT: u16 = 61631;
/// The usual UDP port of a Joiner. The network announces the port that it uses.
pub const JOINER_PORT: u16 = 1000;
/// New networks that each give an event.
pub const NETWORK_EVENTS: u32 = 20;
/// New devices that each give an event.
pub const DEVICE_EVENTS: u32 = 100;

/// The name of MLE command `c`.
pub fn mle_command(c: u8) -> Option<&'static str> {
    Some(match c {
        0 => "Link Request",
        1 => "Link Accept",
        2 => "Link Accept and Request",
        3 => "Link Reject",
        4 => "Advertisement",
        7 => "Data Request",
        8 => "Data Response",
        9 => "Parent Request",
        10 => "Parent Response",
        11 => "Child ID Request",
        12 => "Child ID Response",
        13 => "Child Update Request",
        14 => "Child Update Response",
        15 => "Announce",
        16 => "Discovery Request",
        17 => "Discovery Response",
        _ => return None,
    })
}

/// The service of a UDP port.
pub fn service(port: u16) -> Option<&'static str> {
    Some(match port {
        MLE_PORT => "MLE",
        COAP_PORT => "CoAP",
        MANAGEMENT_PORT => "Thread Management (CoAP)",
        JOINER_PORT => "Joiner",
        _ => return None,
    })
}

/// The payload of a Thread beacon.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadBeacon {
    /// The Thread version: 1 is 1.1, 2 is 1.2 and later.
    pub version: u8,
    /// The network name: characters outside 32 to 126 show as "?".
    pub name: String,
    /// The extended PAN id, in transmit order.
    pub ext_pan: [u8; 8],
}

impl ThreadBeacon {
    /// The Thread beacon at the start of the beacon payload `bp`: the protocol id 3, the version, 16 bytes of name and the
    /// extended PAN id. What follows is not read.
    pub fn of(bp: &[u8]) -> Option<ThreadBeacon> {
        if bp.len() < 26 || bp[0] != 3 {
            return None;
        }
        let raw = &bp[2..18];
        let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
        let name = raw[..end]
            .iter()
            .map(|&c| {
                if (32..127).contains(&c) {
                    c as char
                } else {
                    '?'
                }
            })
            .collect();
        let mut ext_pan = [0u8; 8];
        ext_pan.copy_from_slice(&bp[18..26]);
        Some(ThreadBeacon {
            version: bp[1] >> 4,
            name,
            ext_pan,
        })
    }

    /// The extended PAN id as hex pairs with colons.
    pub fn ext_pan_text(&self) -> String {
        pairs(&self.ext_pan)
    }
}

/// Bytes as upper-case hex pairs joined by colons.
fn pairs(b: &[u8]) -> String {
    let mut s = String::with_capacity(3 * b.len());
    for (i, x) in b.iter().enumerate() {
        if i > 0 {
            s.push(':');
        }
        let _ = write!(s, "{x:02X}");
    }
    s
}

/// What a frame says, for the log and the tables.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Described {
    /// The MAC header.
    pub mac: Mac,
    /// The details text.
    pub text: String,
    /// The link layer encrypts the payload.
    pub encrypted: bool,
    /// The PAN of the frame.
    pub pan: Option<u16>,
    /// A Thread beacon or a 6LoWPAN header was seen.
    pub thread: bool,
    /// Beacon: PAN coordinator, association permitted.
    pub beacon: Option<(bool, bool)>,
    /// The Thread payload of a beacon.
    pub thread_beacon: Option<ThreadBeacon>,
    /// The sender relayed the frame for the originator of a mesh header.
    pub relayed: bool,
    /// An (extended, short) pair learnt from the frame.
    pub bind: Option<(u64, u16)>,
}

fn link(a: Addr) -> Option<LinkAddress> {
    match a {
        Addr::None => None,
        Addr::Short(s) => Some(LinkAddress::Short(s)),
        Addr::Ext(e) => Some(LinkAddress::Extended(e.to_be_bytes())),
    }
}

fn link_text(a: LinkAddress) -> String {
    match a {
        LinkAddress::Short(s) => format!("0x{s:04X}"),
        LinkAddress::Extended(e) => crate::family::hex(&e),
    }
}

/// The text of the 6LoWPAN headers of the MAC payload `pay` of a data frame and of what they carry. Returns whether a 6LoWPAN
/// header was seen and whether a mesh header names an originator other than the sender.
fn lowpan_text(mac: &Mac, pay: &[u8], t: &mut String) -> (bool, bool) {
    let d = lowpan::read(pay);
    if d.next == NextHeader::NotLowpan {
        let _ = write!(t, "Data, {} bytes (not 6LoWPAN)", pay.len());
        return (false, false);
    }
    let mut relayed = false;
    let sep = |t: &mut String| {
        if !t.is_empty() {
            t.push_str(", ");
        }
    };
    if let Some(m) = d.mesh {
        sep(t);
        let _ = write!(
            t,
            "mesh {} > {}, {} hops left",
            link_text(m.originator),
            link_text(m.final_destination),
            m.hops_left
        );
        relayed = link(mac.src).is_some_and(|s| s != m.originator);
    }
    if let Some(f) = d.fragment {
        sep(t);
        if f.first {
            let _ = write!(
                t,
                "first fragment of {} bytes, tag {}",
                f.datagram_size, f.datagram_tag
            );
        } else {
            let _ = write!(
                t,
                "fragment of {} bytes, tag {}, offset {}",
                f.datagram_size, f.datagram_tag, f.offset
            );
        }
    }
    if d.truncated && d.iphc.is_none() {
        sep(t);
        t.push_str(
            if d.mesh.is_none() && pay.first().is_some_and(|h| h & 0xC0 == 0x80) {
                "6LoWPAN mesh header (truncated)"
            } else if pay.iter().any(|h| h & 0xE0 == 0x60) {
                "6LoWPAN IPHC (truncated)"
            } else {
                "6LoWPAN header (truncated)"
            },
        );
        return (true, relayed);
    }
    if let Some(ip) = &d.iphc {
        sep(t);
        t.push_str("6LoWPAN IPHC");
        // The link-layer addresses of the datagram: those of the mesh header when there is one.
        let (ls, ld) = match d.mesh {
            Some(m) => (Some(m.originator), Some(m.final_destination)),
            None => (link(mac.src), link(mac.dst)),
        };
        match ip.source {
            SrcAddress::Full(a) => {
                let _ = write!(t, ", src {}", pairs(&a));
            }
            SrcAddress::Inline { bytes, len, .. } => {
                let n = usize::from(len);
                let _ = write!(t, ", src (inline {n} bytes {})", pairs(&bytes[8 - n..]));
            }
            SrcAddress::Elided { context: false } => {
                if let Some(a) = ip.source_address(ls) {
                    let _ = write!(t, ", src {}", pairs(&a));
                }
            }
            _ => {}
        }
        match ip.destination {
            DstAddress::Full(a) => {
                let _ = write!(t, ", dst {}", pairs(&a));
            }
            DstAddress::Inline { bytes, len, .. } => {
                let n = usize::from(len);
                let _ = write!(t, ", dst (inline {n} bytes {})", pairs(&bytes[8 - n..]));
            }
            DstAddress::Multicast { bytes, len, .. } => {
                let n = usize::from(len);
                let _ = write!(
                    t,
                    ", dst multicast (inline {n} bytes {})",
                    pairs(&bytes[..n])
                );
            }
            DstAddress::Elided { context: false } => {
                if let Some(a) =
                    ip.destination_address(ld.filter(|l| *l != LinkAddress::Short(0xFFFF)))
                {
                    let _ = write!(t, ", dst {}", pairs(&a));
                }
            }
            _ => {}
        }
    }
    match d.next {
        NextHeader::Ipv6 => {
            sep(t);
            t.push_str("6LoWPAN uncompressed IPv6 header");
        }
        NextHeader::Icmpv6 => t.push_str(", ICMPv6"),
        NextHeader::Other(n) => {
            let _ = write!(t, ", next header {n}");
        }
        NextHeader::Compressed(_) => t.push_str(", compressed next header"),
        _ => {}
    }
    if d.truncated {
        t.push_str(" (truncated)");
    }
    let Some(u) = d.udp else {
        return (true, relayed);
    };
    let name = service(u.destination).or(service(u.source));
    let _ = write!(t, ", UDP {} > {}", u.source, u.destination);
    if let Some(n) = name {
        let _ = write!(t, " {n}");
    }
    let body = pay.get(d.payload..).unwrap_or(&[]);
    if u.source == MLE_PORT || u.destination == MLE_PORT {
        // The first byte is the security suite: 255 not secured (the command follows), 0 secured.
        match (body.first(), body.get(1)) {
            (Some(255), Some(&c)) => {
                match mle_command(c) {
                    Some(n) => {
                        let _ = write!(t, ", MLE {n} (not secured)");
                    }
                    None => {
                        let _ = write!(t, ", MLE command {c} (not secured)");
                    }
                };
            }
            (Some(0), _) => t.push_str(", MLE secured with AES-CCM (command encrypted)"),
            _ => t.push_str(", MLE"),
        }
    } else if matches!(name, Some("CoAP" | "Thread Management (CoAP)")) {
        if let Some(m) = coap::read(body) {
            let _ = write!(t, ", CoAP {} {}", m.kind.name(), m.code_text());
            if let Some(n) = m.code_name() {
                let _ = write!(t, " {n}");
            }
            if !m.uri_path.is_empty() {
                let _ = write!(t, " /{}", m.uri_path);
            }
        }
    }
    (true, relayed)
}

/// Describes a PSDU without its FCS.
pub fn describe(b: &[u8]) -> Option<Described> {
    let mac = mac154::parse(b)?;
    let mut d = Described {
        pan: mac154::pan_of(&mac),
        ..Default::default()
    };
    let pay = &b[mac.hdr_len.min(b.len())..];
    if mac.encrypted() {
        let aux = mac.aux.clone().unwrap_or_default();
        let n = pay.len().saturating_sub(aux.mic_len);
        let number = |v: Option<u32>| v.map_or_else(|| "none".to_string(), |v| v.to_string());
        d.text = format!(
            "{}, link-layer AES-CCM encrypted (level {}, key index {}, counter {}), {n} bytes: content not readable without the network key",
            if mac.ftype == 1 { "Thread data" } else { mac.type_name() },
            aux.level,
            number(aux.key_index.map(u32::from)),
            number(aux.counter),
        );
        d.encrypted = true;
        d.mac = mac;
        return Some(d);
    }
    d.text = match mac.ftype {
        2 => format!("Ack #{}", mac.seq.unwrap_or(0)),
        0 => {
            let bcn = mac154::beacon(pay);
            d.beacon = Some((bcn.coordinator, bcn.permit));
            let pan = mac.src_pan.unwrap_or(0);
            match pay.get(bcn.payload..).and_then(ThreadBeacon::of) {
                Some(tb) => {
                    d.thread = true;
                    let text = format!(
                        "Thread beacon, network \"{}\", ext PAN {}, version {}{}",
                        tb.name,
                        tb.ext_pan_text(),
                        tb.version,
                        if bcn.permit { ", joining open" } else { "" }
                    );
                    d.thread_beacon = Some(tb);
                    text
                }
                None => mac154::beacon_text(pan, &bcn),
            }
        }
        3 => {
            let (text, bind) = mac154::command_text(&mac, pay);
            d.bind = bind;
            text
        }
        1 => {
            let mut t = String::new();
            let (thread, relayed) = lowpan_text(&mac, pay, &mut t);
            d.thread = thread;
            d.relayed = relayed;
            t
        }
        _ => format!("{}, {} bytes", mac.type_name(), pay.len()),
    };
    d.mac = mac;
    Some(d)
}

/// A network, keyed by PAN and channel.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Network {
    /// The PAN id.
    pub pan: u16,
    /// The channel where it was heard (0 when not known).
    pub channel: u32,
    /// The network name of a Thread beacon.
    pub name: Option<String>,
    /// The extended PAN id.
    pub ext_pan: Option<String>,
    /// The Thread version of the beacon.
    pub version: Option<u8>,
    /// The last beacon permitted joining.
    pub join_open: bool,
    /// A Thread beacon or a 6LoWPAN header was seen in this PAN.
    pub thread: bool,
    /// The sender of the beacon.
    pub coordinator: Option<String>,
    /// The last level, dBFS.
    pub level_db: f32,
}

impl Shift for Network {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Network {
    type Row = ThreadNetworkRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&ThreadNetworkRow {
            pan: &pan_text(self.pan),
            ch: i64::from(self.channel),
            network_name: self.name.as_deref(),
            ext_pan: self.ext_pan.as_deref(),
            version: self.version.map(i64::from),
            joining_open: self.join_open,
            thread_seen: self.thread,
            coordinator: self.coordinator.as_deref(),
            level_dbfs: f64::from(self.level_db),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// A line of the frame log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRow {
    /// The wall-clock time, when known.
    pub wall: Option<f64>,
    /// The signal time, seconds.
    pub at: f64,
    /// The channel.
    pub channel: u32,
    /// The MAC frame type.
    pub kind: String,
    /// The source.
    pub source: String,
    /// The destination.
    pub destination: String,
    /// The PAN.
    pub pan: Option<u16>,
    /// The PSDU length with the FCS.
    pub len: usize,
    /// The link layer encrypts the payload.
    pub encrypted: bool,
    /// The level, dBFS.
    pub level_db: f32,
    /// The sequence number.
    pub seq: Option<u8>,
    /// The frame requests an acknowledgement.
    pub ack_request: bool,
    /// The description.
    pub details: String,
}

impl Shift for FrameRow {
    fn shift_times(&mut self, by_s: f64) {
        self.at += by_s;
    }
}

impl Shaped for FrameRow {
    type Row = ThreadFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let pan = self.pan.map(pan_text);
        out.put(&ThreadFrameRow {
            utc: self.wall.map(Time),
            time_s: self.at,
            ch: i64::from(self.channel),
            r#type: &self.kind,
            source: &self.source,
            target: &self.destination,
            pan: pan.as_deref(),
            len: self.len as i64,
            encrypted: self.encrypted,
            level_dbfs: f64::from(self.level_db),
            seq: self.seq.map(i64::from),
            ack_request: self.ack_request,
            details: &self.details,
        });
    }
}

/// What `thread.tables` keeps across sessions.
#[derive(Serialize, Deserialize)]
pub struct Store {
    networks: ObjectTable<u64, Network>,
    devices: Devices,
    frames: LogRing<FrameRow>,
    good: u64,
    encrypted: u64,
    per_type: [u64; 4],
    per_channel: [u64; 16],
    new_networks: u32,
    new_devices: u32,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            networks: ObjectTable::new(NETWORK_CAP),
            devices: Devices::new(DEVICE_CAP),
            frames: LogRing::new(FRAME_CAP),
            good: 0,
            encrypted: 0,
            per_type: [0; 4],
            per_channel: [0; 16],
            new_networks: 0,
            new_devices: 0,
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.networks.shift_times(by_s);
        self.devices.table.shift_times(by_s);
        self.frames.shift_times(by_s);
    }
}

impl sdrkit_util_objects::Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("ism.thread", 1);
    fn clear(&mut self) {
        *self = Store::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.networks.adopt(saved.networks);
        self.devices.table.adopt(saved.devices.table);
        self.frames.adopt(saved.frames);
        self.good = saved.good;
        self.encrypted = saved.encrypted;
        self.per_type = saved.per_type;
        self.per_channel = saved.per_channel;
        self.new_networks = saved.new_networks;
        self.new_devices = saved.new_devices;
    }
}

/// The rank of a role: a role only rises.
fn role_rank(r: &str) -> u8 {
    match r {
        "coordinator" => 2,
        "router" => 1,
        _ => 0,
    }
}

impl Store {
    /// Takes a good frame heard at `at` (with the wall-clock time `wall` when it is known). The events of a new network and of
    /// a new device go to `events`.
    fn frame(&mut self, f: &Frame154, at: Stamp, events: &mut Vec<Event>) {
        let n = f.psdu.len();
        let Some(ds) = describe(&f.psdu[..n.saturating_sub(2)]) else {
            return;
        };
        self.good += 1;
        let m = &ds.mac;
        if let Some(c) = self.per_type.get_mut(usize::from(m.ftype)) {
            *c += 1;
        }
        if (11..=26).contains(&f.channel) {
            self.per_channel[(f.channel - 11) as usize] += 1;
        }
        if ds.encrypted {
            self.encrypted += 1;
        }
        self.frames.push(FrameRow {
            wall: at.wall,
            at: f.time,
            channel: f.channel,
            kind: m.type_name().into(),
            source: m.src.text(),
            destination: m.dst.text(),
            pan: ds.pan,
            len: n,
            encrypted: ds.encrypted,
            level_db: f.level_db,
            seq: m.seq,
            ack_request: m.ack_req,
            details: ds.text.clone(),
        });
        if let Some(pan) = ds.pan {
            let key = u64::from(pan) << 8 | u64::from(f.channel);
            if let Ok(up) = self.networks.upsert(&key, at, || Network {
                pan,
                channel: f.channel,
                ..Default::default()
            }) {
                let nw = up.row;
                nw.level_db = f.level_db;
                nw.thread |= ds.thread;
                if let Some((coordinator, permit)) = ds.beacon {
                    nw.join_open = permit;
                    if coordinator || nw.coordinator.is_none() {
                        nw.coordinator = Some(m.src.text());
                    }
                }
                if let Some(tb) = &ds.thread_beacon {
                    let first = nw.name.is_none();
                    nw.name = Some(tb.name.clone());
                    nw.ext_pan = Some(tb.ext_pan_text());
                    nw.version = Some(tb.version);
                    if first && self.new_networks < NETWORK_EVENTS {
                        self.new_networks += 1;
                        let id = pan_text(pan);
                        events.push(THREAD_NETWORK.event(
                            f.time,
                            f.freq_hz,
                            &ThreadNetworkEvent {
                                source: &id,
                                text: &format!(
                                    "Thread: network \"{}\" (PAN {id}) on channel {}",
                                    tb.name, f.channel
                                ),
                                network_name: &tb.name,
                                ext_pan: &tb.ext_pan_text(),
                                ch: i64::from(f.channel),
                                family: Family::Sensor,
                            },
                        ));
                    }
                }
            }
        }
        let (short, ext) = match m.src {
            Addr::Short(s) => (Some(s), None),
            Addr::Ext(e) => (None, Some(e)),
            Addr::None => (None, None),
        };
        if short.is_some() || ext.is_some() {
            let pan = m.src_pan.filter(|&p| p != 0xFFFF).or(ds.pan);
            let new = self.devices.find(short, ext, pan).is_none();
            let id = self.devices.device(short, ext, pan, f.channel, at);
            let mut role = "device";
            if let Some((coordinator, _)) = ds.beacon {
                role = if coordinator || short == Some(0) {
                    "coordinator"
                } else {
                    "router"
                };
            }
            if ds.relayed {
                role = "router";
            }
            if let Some(dv) = self.devices.table.get_mut(&id) {
                dv.level_db = f.level_db;
                dv.last_frame.clone_from(&ds.text);
                if role_rank(role) > role_rank(&dv.role) {
                    dv.role = role.into();
                }
            }
            if new && self.new_devices < DEVICE_EVENTS {
                self.new_devices += 1;
                let pan = pan.map(pan_text);
                events.push(THREAD_DEVICE.event(
                    f.time,
                    f.freq_hz,
                    &ThreadDeviceEvent {
                        source: &m.src.text(),
                        pan: pan.as_deref(),
                        ch: i64::from(f.channel),
                        family: Family::Sensor,
                    },
                ));
            }
        }
        if let Some((e, s)) = ds.bind {
            self.devices.merge(e, s, ds.pan, f.channel, at);
        }
    }
}

/// Step through the channels.
pub const SCAN: Setting<field::Bool> = Setting::new("scan", "Scan channels 11 to 26", "", false)
    .help("Asks a live radio to visit each channel in turn. A network is found on the channel that its commissioner selected.");
/// The network name of the beacon of the test signal.
pub const GEN_NETWORK_NAME: Setting<field::Text> = Setting::new(
    "gen_network_name",
    "Test signal network name",
    "",
    "HomeThread",
)
.section("Test signal")
.advanced();

sdrkit_core::settings! {
    /// The settings of [`Tables`].
    pub enum TablesSetting {
        /// The channel.
        Channel = CHANNEL_SETTING,
        /// Scan the channels.
        Scan = SCAN,
        /// The scan dwell.
        Dwell = DWELL,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Tables`].
    pub enum TablesCommand {
        /// Forgets everything heard.
        Clear = commands::CLEAR,
        /// Forgets one device.
        Remove = PAN_REMOVE.command,
    }
}

/// `thread.tables`: the reader above the MAC, the networks, the devices, the frame log, the counters, the status, and the
/// retune requests of the channel setting and of the scan.
pub struct Tables {
    published: Published,
    devices_table: TableId,
    summary: String,
    store: Store,
    last: Option<(f32, f32, f64)>,
    events: Vec<Event>,
    requests: Vec<HostRequest>,
    now: f64,
    last_good: f64,
    bad: u64,
    second: u32,
    per_second: u32,
    ticks: u32,
    channel: u32,
    want_channel: u32,
    scan: bool,
    dwell_s: f64,
    scan_at: Option<(u32, f64)>,
    configured: bool,
    live: bool,
}

impl Default for Tables {
    fn default() -> Self {
        let decl = Arc::new(
            ResultsDecl::new()
                .with_fields(ThreadFields::SHAPE.fields)
                .with_table(&THREAD_NETWORKS)
                .with_table(&PAN_DEVICES)
                .with_table(&THREAD_FRAMES),
        );
        let devices_table = decl
            .table_id(PAN_DEVICES.key)
            .expect("the tables of thread.tables are declared");
        Tables {
            published: Published::from_decl(decl),
            devices_table,
            summary: String::new(),
            store: Store::default(),
            last: None,
            events: Vec::new(),
            requests: Vec::new(),
            now: 0.0,
            last_good: f64::NEG_INFINITY,
            bad: 0,
            second: 0,
            per_second: 0,
            ticks: 0,
            channel: 0,
            want_channel: 11,
            scan: false,
            dwell_s: 5.0,
            scan_at: None,
            configured: false,
            live: false,
        }
    }
}

impl Tables {
    fn stamp(&self, t: f64) -> Stamp {
        match crate::family::wall(self.live) {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        }
    }

    /// The scan runs: the setting is on and the samples come from a radio.
    fn scanning(&self) -> bool {
        self.scan && self.live
    }

    fn publish(&mut self) {
        let s = &self.store;
        let state = if self.now - self.last_good < 5.0 && s.good > 0 {
            LockState::Locked
        } else {
            LockState::Searching
        };
        self.summary.clear();
        if s.good > 0 {
            let _ = write!(
                self.summary,
                "{} networks, {} devices, {} frames",
                s.networks.len(),
                s.devices.table.len(),
                s.good
            );
        } else if self.scanning() {
            self.summary
                .push_str("scanning the channels 11 to 26 for Thread");
        } else if self.channel > 0 {
            let _ = write!(self.summary, "listening on channel {}", self.channel);
        } else {
            self.summary.push_str("listening for Thread frames");
        }
        let newest = self.last.map(|l| l.0);
        let p = &mut self.published;
        set_status(p, state, &self.summary);
        p.results_mut().update_status(|st| {
            let changed = st.level_dbfs != newest;
            st.level_dbfs = newest;
            changed
        });
        let [beacons, data_frames, acks, commands] = s.per_type.map(|v| v as i64);
        let ch = s.per_channel.map(|v| v as i64);
        p.put(&ThreadFields {
            frames_ok: s.good as i64,
            frames_bad: self.bad as i64,
            encrypted_frames: s.encrypted as i64,
            beacons,
            data_frames,
            acks,
            commands,
            ch11: ch[0],
            ch12: ch[1],
            ch13: ch[2],
            ch14: ch[3],
            ch15: ch[4],
            ch16: ch[5],
            ch17: ch[6],
            ch18: ch[7],
            ch19: ch[8],
            ch20: ch[9],
            ch21: ch[10],
            ch22: ch[11],
            ch23: ch[12],
            ch24: ch[13],
            ch25: ch[14],
            ch26: ch[15],
            frames_per_s: i64::from(self.per_second),
            channel: i64::from(self.channel),
            scanning: self.scan && self.live,
            networks: s.networks.len() as i64,
            devices: s.devices.table.len() as i64,
            level_dbfs: self.last.map(|l| f64::from(l.0)),
            snr_db: self.last.map(|l| f64::from(l.1)),
            offset_hz: self.last.map(|l| Hertz(l.2)),
        });
        shaped::rows(&s.networks, &THREAD_NETWORKS, self.now, p);
        shaped::rows(&s.devices.table, &PAN_DEVICES, self.now, p);
        shaped::log_rows(&s.frames, &THREAD_FRAMES, self.now, p);
    }

    /// The step of the scan at signal time `now`: the first channel at once, then the next one after each dwell.
    fn scan_step(&mut self, now: f64) {
        if !self.scanning() {
            self.scan_at = None;
            return;
        }
        let next = match self.scan_at {
            None => 11,
            Some((k, since)) if now - since >= self.dwell_s => {
                if k >= 26 {
                    11
                } else {
                    k + 1
                }
            }
            Some(_) => return,
        };
        self.scan_at = Some((next, now));
        self.requests
            .push(HostRequest::retune(channel_hz(next), "Thread channel scan"));
    }
}

impl Component for Tables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "thread.tables",
            vec![
                Port::typed::<Frame154>("frames"),
                Port::typed::<DecoderCounts>("counts"),
            ],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(TablesSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.channel = channel_of(inputs[0].tuned_hz);
        self.configured = true;
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn retuned(&mut self, _centre_hz: f64, tuned_hz: f64) {
        self.channel = channel_of(tuned_hz);
    }
    fn attach(&mut self, ctx: &sdrkit_graph::RunContext) {
        self.live = ctx.wall_clock();
    }
    fn process(&mut self, io: &mut Io) {
        let mut new = false;
        for f in io.items::<Frame154>(0) {
            let at = self.stamp(f.value.time);
            self.now = self.now.max(f.value.time);
            self.store.frame(&f.value, at, &mut self.events);
            self.last = Some((f.value.level_db, f.value.snr_db, f.value.offset_hz));
            self.last_good = f.value.time;
            self.second += 1;
            new = true;
        }
        for c in io.items::<DecoderCounts>(1) {
            self.bad = c.value.bad;
            self.now = self.now.max(c.value.time_s);
        }
        let tick = io.tick();
        if tick {
            self.ticks += 1;
            if self.ticks.is_multiple_of(4) {
                self.per_second = self.second;
                self.second = 0;
            }
            self.scan_step(self.now);
        }
        if new || tick {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
        for r in self.requests.drain(..) {
            io.items_out::<HostRequest>(1).push(Timed {
                time: self.now,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TablesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TablesSetting::Channel => {
                self.want_channel = CHANNEL_SETTING.read(id, v)? as u32;
                if self.configured && !self.scanning() {
                    self.requests.push(HostRequest::retune(
                        channel_hz(self.want_channel),
                        "Thread channel",
                    ));
                }
            }
            TablesSetting::Scan => {
                self.scan = SCAN.read(id, v)?;
                self.scan_at = None;
            }
            TablesSetting::Dwell => self.dwell_s = DWELL.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                TablesSetting::Channel,
                Value::I64(i64::from(self.want_channel)),
            ),
            (TablesSetting::Scan, Value::Bool(self.scan)),
            (TablesSetting::Dwell, Value::Duration(self.dwell_s)),
        ])
    }
    fn commands(&self) -> &Commands {
        TablesCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TablesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TablesCommand::Clear => {
                self.store.clear();
                self.per_second = 0;
                self.published.clear_rows(&THREAD_NETWORKS);
                self.published.clear_rows(&PAN_DEVICES);
                self.published.clear_rows(&THREAD_FRAMES);
            }
            TablesCommand::Remove => commands::remove(
                &mut self.store.devices.table,
                self.devices_table,
                &mut self.published,
                args,
                &PAN_DEVICE_ROW,
            )?,
        }
        self.publish();
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        let s = &mut self.store;
        if !kept.table(THREAD_NETWORKS.key) {
            s.networks.clear();
            self.published.clear_rows(&THREAD_NETWORKS);
        }
        if !kept.table(PAN_DEVICES.key) {
            s.devices.table.clear();
            self.published.clear_rows(&PAN_DEVICES);
        }
        if !kept.table(THREAD_FRAMES.key) {
            s.frames.clear();
            self.published.clear_rows(&THREAD_FRAMES);
        }
        if !kept.field("frames_ok") {
            s.good = 0;
        }
        if !kept.field("frames_bad") {
            self.bad = 0;
        }
        if !kept.field("encrypted_frames") {
            s.encrypted = 0;
        }
        for (key, n) in ["beacons", "data_frames", "acks", "commands"]
            .iter()
            .zip(s.per_type.iter_mut())
        {
            if !kept.field(key) {
                *n = 0;
            }
        }
        for (k, n) in s.per_channel.iter_mut().enumerate() {
            if !kept.field(&format!("ch{}", k + 11)) {
                *n = 0;
            }
        }
        self.second = 0;
        self.per_second = 0;
        self.last = None;
        self.last_good = f64::NEG_INFINITY;
        self.publish();
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now)?;
        self.publish();
        Ok(())
    }
}

sdrkit_core::settings! {
    /// The settings of the `thread` stack.
    pub enum ThreadSetting {
        /// The channel.
        Channel = CHANNEL_SETTING,
        /// Scan the channels.
        Scan = SCAN,
        /// The scan dwell.
        Dwell = DWELL,
        /// The network name of the test signal.
        GenNetworkName = GEN_NETWORK_NAME,
        /// Every frame with a bad FCS in the test signal.
        GenBadFcs = GEN_BAD_FCS,
        /// The level of the adjacent transmitter of the test signal.
        GenInterfererDb = GEN_INTERFERER_DB,
        /// The offset of the adjacent transmitter of the test signal.
        GenInterfererHz = GEN_INTERFERER_HZ,
    }
}

/// The least sample rate: two channel widths.
pub const MIN_RATE_HZ: f64 = 4e6;

/// The `thread` receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among
/// them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add("front", Front::default())
        .add("chips", Chips::default())
        .add("frame", FrameDec::default())
        .add("tables", Tables::default())
        .input("front")
        .connect("front", "chips")
        .connect("chips", "frame")
        .connect("frame.frames", "tables.frames")
        .connect("frame.counts", "tables.counts")
        .output("tables.events")
        .output("tables.requests")
        .clock(REPORT_S)
        .status_from("tables")
        .build()?;
    StackGraph::of(crate::typed::shape("thread"), graph)
        .min_rate(MIN_RATE_HZ, &min_rate_message("Thread", MIN_RATE_HZ))
        .route(CHANNEL_SETTING.key(), &["tables.channel"])?
        .route(SCAN.key(), &["tables.scan"])?
        .route(DWELL.key(), &["tables.dwell"])?
        .command("clear", sdrkit_graph::CommandTarget::of("tables", "clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("tables", "remove"),
        )
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The PAN of the test network.
pub const GEN_PAN: u16 = 0x7A31;
/// The extended PAN id of the test network.
pub const GEN_EXT_PAN: [u8; 8] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];

/// A Thread beacon (without its FCS) of the router `source` of PAN `pan`: PAN coordinator, joining permitted when `permit`
/// is set, Thread version 2, the network name (at most 16 bytes) and the extended PAN id.
pub fn beacon_frame(
    seq: u8,
    pan: u16,
    source: u16,
    permit: bool,
    name: &str,
    ext_pan: [u8; 8],
) -> Vec<u8> {
    let mut b = mac154::build_header(
        0,
        false,
        false,
        0,
        seq,
        None,
        Addr::None,
        Some(pan),
        Addr::Short(source),
        false,
    );
    let superframe: u16 = 0x0FFF | 0x4000 | if permit { 0x8000 } else { 0 };
    b.extend(superframe.to_le_bytes());
    // No guaranteed slots, no pending addresses.
    b.extend([0, 0]);
    b.extend([3, 2 << 4]);
    let mut field = [0u8; 16];
    for (d, s) in field.iter_mut().zip(name.bytes()) {
        *d = s;
    }
    b.extend(field);
    b.extend(ext_pan);
    b
}

/// A data frame (without its FCS) that the link layer encrypts: security level 5, key identifier mode 1, the frame counter
/// `counter`, the key index 1, 12 bytes of cipher text and 4 bytes of integrity code.
pub fn encrypted_frame(
    seq: u8,
    pan: u16,
    from: u16,
    to: u16,
    counter: u32,
    rng: &mut Rng,
) -> Vec<u8> {
    let mut b = mac154::build_header(
        1,
        true,
        true,
        1,
        seq,
        Some(pan),
        Addr::Short(to),
        None,
        Addr::Short(from),
        true,
    );
    b.push(0x0D);
    b.extend(counter.to_le_bytes());
    b.push(1);
    b.extend((0..16).map(|_| rng.next_u64() as u8));
    b
}

/// A broadcast data frame (without its FCS) from the extended address `from` with an MLE Discovery Request in the clear:
/// IPHC to the multicast address ff02::2, compressed UDP from and to port 19788.
pub fn discovery_frame(seq: u8, from: u64) -> Vec<u8> {
    let mut b = mac154::build_header(
        1,
        false,
        true,
        0,
        seq,
        Some(0xFFFF),
        Addr::Short(0xFFFF),
        None,
        Addr::Ext(from),
        false,
    );
    b.extend([
        0x7F, 0x3B, 0x02, 0xF0, 0x4D, 0x4C, 0x4D, 0x4C, 0x12, 0x34, 0xFF, 0x10, 0x0B, 0x02, 0x20,
        0x00,
    ]);
    b
}

/// The generator schedule: one Thread beacon, six encrypted data frames between two devices and one MLE Discovery Request
/// in the clear. Each entry is a PSDU with its FCS.
pub fn schedule(name: &str, cycle: u32, rng: &mut Rng) -> Vec<Vec<u8>> {
    let seq = (cycle as u8).wrapping_mul(8);
    let mut v = vec![beacon_frame(seq, GEN_PAN, 0x5000, true, name, GEN_EXT_PAN)];
    for k in 0..6u8 {
        let (from, to) = if k % 2 == 0 {
            (0x5000, 0x5401)
        } else {
            (0x5401, 0x5000)
        };
        v.push(encrypted_frame(
            seq.wrapping_add(1 + k),
            GEN_PAN,
            from,
            to,
            0x1000 + cycle * 8 + u32::from(k),
            rng,
        ));
    }
    v.push(discovery_frame(seq.wrapping_add(7), 0x0123_4567_89AB_CDEF));
    v.into_iter().map(with_fcs).collect()
}

/// The radio is tuned this far above the channel.
pub const TUNE_OFFSET_HZ: f64 = 1e6;
/// The pause between two rounds of the schedule of the test signal, seconds.
pub const CYCLE_PAUSE_S: f64 = 0.03;

/// The `thread` generator: the schedule [`TUNE_OFFSET_HZ`] below the centre of the samples, as a radio tuned with the offset of
/// the stack sees it, the frames 0.8 ms apart, in a loop with a pause of [`CYCLE_PAUSE_S`] after each round, with the options of
/// the `zigbee` generator. The clean signal only: the frames at unit power, or, with an adjacent transmitter, that transmitter
/// at unit power and the frames below it.
///
/// # Errors
///
/// The errors of the scene and of the modulator for a rate that they refuse.
pub fn generator(
    rate: f64,
    seed: u64,
    name: &str,
    o: crate::zigbee::GenOptions,
) -> Result<CleanScene, ConfigError> {
    let frame_db = -o.interferer_db.max(0.0);
    let mut scene = Scene::new(rate)?;
    let mut rng = Rng::stream(seed, 0x54);
    let name = name.to_string();
    let mut t = 0.0005f64;
    let mut cycle = 0u32;
    let mut queue = std::collections::VecDeque::new();
    scene.add_source(Box::new(move || {
        if queue.is_empty() {
            queue.extend(schedule(&name, cycle, &mut rng));
            // The network is quiet between two rounds of the schedule.
            if cycle > 0 {
                t += CYCLE_PAUSE_S;
            }
            cycle = cycle.wrapping_add(1);
        }
        let psdu = queue.pop_front()?;
        let samples = oqpsk(&ppdu_chip_pairs(&psdu, false, o.bad_fcs), rate).ok()?;
        let b = Burst {
            start_s: t,
            offset_hz: -TUNE_OFFSET_HZ,
            level_db: frame_db,
            samples,
        };
        t += b.samples.len() as f64 / rate + 0.0008;
        Some(b)
    }));
    if o.interferer_db > 0.0 {
        let mut rng = Rng::stream(seed, 0x1F);
        let k = std::f32::consts::FRAC_1_SQRT_2;
        let pairs: Vec<Complex32> = (0..50_000)
            .map(|_| {
                let a = if rng.bit() == 1 { k } else { -k };
                let b = if rng.bit() == 1 { k } else { -k };
                Complex32::new(a, b)
            })
            .collect();
        let s = oqpsk(&pairs, rate)?;
        let period = s.len() as f64 / rate;
        scene.add_repeating(0.0, period, o.interferer_hz - TUNE_OFFSET_HZ, 0.0, s)?;
    }
    let info = SignalInfo::new(2e6)
        .with_symbol_rate(2e6)
        .with_bit_rate(250e3)
        .with_power(10f64.powf(frame_db / 10.0));
    Ok(CleanScene::new(scene, rate, info).with_station_at(-TUNE_OFFSET_HZ))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let o = crate::zigbee::gen_options(&cfg.settings);
    let name = cfg.settings.get(&GEN_NETWORK_NAME);
    Ok(Box::new(generator(cfg.rate_hz, cfg.seed, name, o)?))
}

/// Thread.
pub static THREAD: StackDescriptor = StackDescriptor {
    id: "thread",
    name: "Thread",
    group: "IoT, 2.4 GHz",
    blurb: "Thread networks on the 802.15.4 channels 11 to 26: beacons with the network name, devices, 6LoWPAN headers, and MLE and CoAP where they are sent in the clear. Encrypted frames are marked.",
    tuning: Tuning { min_hz: 2405e6, max_hz: 2480e6, tune_offset_hz: TUNE_OFFSET_HZ, ..Tuning::anywhere(2405e6, 4e6, 4e6, 2e6) },
    submodes: &[],
    settings: ThreadSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, one Thread network", o, 0.0);
    c.seconds = o.seconds.max(1.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = ThreadFields::read(r.fields());
        let n = |v: fn(&ThreadFields) -> i64| f.as_ref().map_or(0, v);
        vec![
            at_least_value("frames_ok", n(|f| f.frames_ok), 20),
            at_least_value("networks", n(|f| f.networks), 1),
            at_least_value("encrypted_frames", n(|f| f.encrypted_frames), 6),
        ]
    });
    vec![c]
}

/// The verify case of `thread`.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The benchmark of the Thread receiver: 0.1 s of the test signal at 4 Msps, fed in one piece.
pub fn benchmarks() -> Vec<sdrkit_core::bench::Benchmark> {
    vec![sdrkit_core::bench::Benchmark {
        name: "thread.receiver_4m",
        run: bench_receiver,
    }]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::bench::BenchResult {
    let rate = 4e6;
    let mut x = vec![Complex32::default(); (rate * 0.1) as usize];
    if let Ok(mut g) = generator(rate, 1, "HomeThread", crate::zigbee::GenOptions::default()) {
        g.generate(&mut x);
    }
    let Ok(mut rx) = receiver(&StackConfig::new(
        Values::defaults(&ThreadSetting::schema()),
    )) else {
        return sdrkit_core::bench::BenchResult::default();
    };
    let _ = rx
        .configure(&sdrkit_core::InputInfo::new(rate, 2405e6 + TUNE_OFFSET_HZ, 2405e6).recording());
    let mut out = sdrkit_core::Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), 0.1, || {
        out.clear();
        rx.feed(&x, &mut out);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    /// The text of the MAC payload `pay` of a data frame from an extended address to the broadcast address.
    fn data_text(pay: &str) -> String {
        let mut b = hex("41 C8 21 FF FF FF FF EF CD AB 89 67 45 23 01");
        b.extend(hex(pay));
        describe(&b).unwrap().text
    }

    #[test]
    fn the_payloads_of_data_frames_give_their_texts() {
        let t1 = data_text("7F 3B 02 F0 4D 4C 4D 4C 12 34 FF 10 0B 02 20 00");
        assert!(
            t1.starts_with("6LoWPAN IPHC, src FE:80:00:00:00:00:00:00:03:23:45:67:89:AB:CD:EF"),
            "{t1}"
        );
        assert!(t1.contains("dst multicast (inline 1 bytes 02)"), "{t1}");
        assert!(
            t1.ends_with(", UDP 19788 > 19788 MLE, MLE Discovery Request (not secured)"),
            "{t1}"
        );
        let t2 = data_text("7F 3B 02 F0 4D 4C 4D 4C 12 34 00 15 00 00 00 01 AA BB");
        assert!(
            t2.ends_with(", MLE secured with AES-CCM (command encrypted)"),
            "{t2}"
        );
        assert!(!t2.contains("command 21"));
        let t3 = data_text("B5 44 00 10 00 C1 2C 00 07 7F 3B 02 F7 13 99");
        assert!(
            t3.starts_with(
                "mesh 0x4400 > 0x1000, 5 hops left, first fragment of 300 bytes, tag 7, 6LoWPAN IPHC"
            ),
            "{t3}"
        );
        assert!(t3.ends_with(", UDP 61617 > 61619"), "{t3}");
        assert_eq!(
            data_text("E1 2C 00 07 05 01 02 03"),
            "fragment of 300 bytes, tag 7, offset 40"
        );
        let t5 = data_text("7F 3B 02 F0 16 33 16 33 00 00 40 01 12 34");
        assert!(
            t5.ends_with(", UDP 5683 > 5683 CoAP, CoAP CON 0.01 GET"),
            "{t5}"
        );
        let t6 = data_text(
            "7B 10 3A 01 02 03 04 05 06 07 08 FD 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01",
        );
        assert_eq!(
            t6,
            "6LoWPAN IPHC, src (inline 8 bytes 01:02:03:04:05:06:07:08), \
             dst FD:00:00:00:00:00:00:00:00:00:00:00:00:00:00:01, ICMPv6"
        );
        assert_eq!(data_text("01 02"), "Data, 2 bytes (not 6LoWPAN)");
        assert_eq!(data_text("7F"), "6LoWPAN IPHC (truncated)");
        assert_eq!(data_text("B5 44"), "6LoWPAN mesh header (truncated)");
        assert_eq!(data_text("41 60 00"), "6LoWPAN uncompressed IPv6 header");
    }

    #[test]
    fn the_worked_beacon_gives_the_network() {
        let b1 = hex(
            "00 80 07 31 7A 00 50 FF CF 00 00 03 20 48 6F 6D 65 54 68 72 65 61 64 00 00 00 00 00 00 11 22 33 44 55 66 \
             77 88 4F 0E",
        );
        assert_eq!(
            with_fcs(beacon_frame(
                7,
                0x7A31,
                0x5000,
                true,
                "HomeThread",
                GEN_EXT_PAN
            )),
            b1
        );
        let d = describe(&b1[..b1.len() - 2]).unwrap();
        assert_eq!(d.mac.src, Addr::Short(0x5000));
        assert_eq!(d.pan, Some(0x7A31));
        assert_eq!(d.beacon, Some((true, true)));
        let tb = d.thread_beacon.as_ref().unwrap();
        assert_eq!((tb.name.as_str(), tb.version), ("HomeThread", 2));
        assert_eq!(tb.ext_pan_text(), "11:22:33:44:55:66:77:88");
        assert_eq!(
            d.text,
            "Thread beacon, network \"HomeThread\", ext PAN 11:22:33:44:55:66:77:88, version 2, joining open"
        );
        // More data after the extended PAN id: still a Thread beacon.
        let mut longer = b1[..b1.len() - 2].to_vec();
        longer.extend([0xAA, 0xBB, 0xCC, 0xDD]);
        assert!(describe(&longer).unwrap().thread_beacon.is_some());
        // Another protocol id: a plain 802.15.4 beacon.
        let mut other = b1[..b1.len() - 2].to_vec();
        other[11] = 0;
        let d = describe(&other).unwrap();
        assert!(d.thread_beacon.is_none() && !d.thread);
        assert_eq!(d.text, "Beacon: PAN 0x7A31, PAN coordinator, join allowed");
    }

    #[test]
    fn the_worked_data_frame_and_the_security_header_give_their_fields() {
        let mut d1 = hex("41 C8 21 FF FF FF FF EF CD AB 89 67 45 23 01");
        d1.extend(hex("7F 3B 02 F0 4D 4C 4D 4C 12 34 FF 10 0B 02 20 00"));
        assert_eq!(with_fcs(d1.clone())[31..], [0x5E, 0xA6]);
        assert_eq!(
            with_fcs(discovery_frame(0x21, 0x0123_4567_89AB_CDEF)),
            with_fcs(d1.clone())
        );
        let d = describe(&d1).unwrap();
        assert_eq!(d.mac.type_name(), "Data");
        assert_eq!(d.mac.dst.text(), "broadcast");
        assert_eq!(d.mac.src.text(), "01:23:45:67:89:AB:CD:EF");
        assert!(d.thread && !d.encrypted);
        assert!(d.text.contains("MLE Discovery Request"));
        // Level 5, key mode 1, frame counter 0x12345678, key index 3, then 12 bytes of cipher text and 4 of integrity code.
        let mut e = mac154::build_header(
            1,
            true,
            true,
            1,
            9,
            Some(0x7A31),
            Addr::Short(0x5000),
            None,
            Addr::Short(0x5401),
            true,
        );
        e.extend(hex("0D 78 56 34 12 03"));
        e.extend([0xEE; 16]);
        let d = describe(&e).unwrap();
        assert!(d.encrypted && !d.thread);
        assert_eq!(
            d.text,
            "Thread data, link-layer AES-CCM encrypted (level 5, key index 3, counter 305419896), 12 bytes: content not \
             readable without the network key"
        );
    }
}
