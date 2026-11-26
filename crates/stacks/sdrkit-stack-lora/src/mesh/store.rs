//! The tables of the mesh receiver: packets, nodes, messages, readings, type counts, APRS stations and LoRaWAN devices, with
//! their caps, the saved state, the session report and the map points.

use super::types::{Delivery, MeshPacket, MeshReading, MeshText, Protocol};
use crate::typed::{
    MeshDeviceRow, MeshMessageRow, MeshNodeRow, MeshPacketRow, MeshReadingRow, MeshStationRow,
    MeshTypeRow, MESH_DEVICES, MESH_MESSAGES, MESH_NODES, MESH_PACKETS, MESH_READINGS,
    MESH_STATIONS, MESH_TYPES,
};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::{Published, TableDef};
use sdrkit_core::{Choice, Results};
use sdrkit_util_geo::Position;
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    fingerprint, Dedup, Format, LogRing, ObjectTable, Seen, Shift, Stamp, Stats, Tables, Tally,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Packets kept.
pub const PACKETS_CAP: usize = 100;
/// Nodes kept.
pub const NODES_CAP: usize = 150;
/// Messages kept.
pub const MESSAGES_CAP: usize = 100;
/// Readings kept.
pub const READINGS_CAP: usize = 2000;
/// Readings shown (the newest).
pub const READINGS_SHOWN: usize = 200;
/// APRS stations kept.
pub const STATIONS_CAP: usize = 150;
/// LoRaWAN devices kept.
pub const DEVICES_CAP: usize = 200;
/// Packet types counted one by one.
pub const TYPES_CAP: usize = 200;
/// Message keys remembered for de-duplication.
pub const DEDUP_CAP: usize = 4000;
/// A message heard again within this time is the same message, seconds.
pub const DEDUP_S: f64 = 600.0;
/// How long the message of a frame with a failed CRC waits for a copy with a good CRC, seconds of signal time.
pub const HOLD_S: f64 = 3.0;
/// The most messages that wait, and the most shown messages of damaged frames that a good copy can still replace.
const DAMAGED_CAP: usize = 64;

fn text(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

fn hops(h: i32) -> Option<i64> {
    (h >= 0).then_some(i64::from(h))
}

fn opt_f32(v: Option<f32>) -> Option<f64> {
    v.map(f64::from)
}

/// A node record (Meshtastic and MeshCore).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeRec {
    /// Protocol.
    pub protocol: Protocol,
    /// "!xxxxxxxx", or the 12 hex digits of a MeshCore key.
    pub id: String,
    /// Long name.
    pub long_name: String,
    /// Short name.
    pub short_name: String,
    /// Hardware.
    pub hw: String,
    /// Role.
    pub role: String,
    /// Public key, hex.
    pub public_key: String,
    /// The SNR of its packets, dB.
    pub snr: Stats,
    /// The level of its last packet, dBFS.
    pub last_level: f64,
    /// Hops away (-1 unknown).
    pub hops: i32,
    /// Its packets that the decoder did not decrypt.
    pub encrypted: u32,
    /// Position: latitude, longitude, altitude (metres, when sent).
    pub position: Option<(f64, f64, Option<f64>)>,
    /// Position time (Unix seconds, as sent).
    pub position_time: u32,
    /// Battery percent (above 100: powered).
    pub battery: Option<u32>,
    /// Volts.
    pub voltage: Option<f32>,
    /// Channel use, percent.
    pub ch_util: Option<f32>,
    /// Air time, percent.
    pub air_tx: Option<f32>,
    /// Uptime, s.
    pub uptime: Option<u32>,
    /// Temperature, °C.
    pub temp: Option<f32>,
    /// Humidity, percent.
    pub humidity: Option<f32>,
    /// Pressure, hPa.
    pub pressure: Option<f32>,
    /// The last relay byte seen on its packets.
    pub last_relay: u8,
    /// Learned next hop (the relay byte of the packet with the fewest hops).
    pub next_hop: u8,
    /// The fewest hops seen.
    pub best_hops: i32,
    /// MeshCore: the advert signature checks out.
    pub signature_ok: Option<bool>,
    /// Last device report, signal time.
    pub device_time: f64,
    /// Last environment report, signal time.
    pub env_time: f64,
}

impl NodeRec {
    /// The name shown: the long name, or the id.
    pub fn name(&self) -> &str {
        if self.long_name.is_empty() {
            &self.id
        } else {
            &self.long_name
        }
    }
    /// The battery as shown.
    pub fn battery_text(&self) -> String {
        match (self.battery, self.voltage) {
            (Some(b), _) if b > 100 => "powered".into(),
            (Some(b), _) => format!("{b} %"),
            (None, Some(v)) => format!("{v:.2} V"),
            _ => "-".into(),
        }
    }
    /// The SNR of the last packet, dB (0 before any).
    pub fn last_snr(&self) -> f64 {
        self.snr.last.unwrap_or(0.0)
    }
}

impl Shift for NodeRec {
    fn shift_times(&mut self, by_s: f64) {
        self.device_time += by_s;
        self.env_time += by_s;
    }
}

impl Shaped for NodeRec {
    type Row = MeshNodeRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&MeshNodeRow {
            net: self.protocol,
            name: self.name(),
            short: text(&self.short_name),
            id: &self.id,
            hw: text(&self.hw),
            role: text(&self.role),
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
            snr_db: self.snr.last,
            hops: hops(self.hops),
            battery_pct: self.battery.map(f64::from),
            voltage_v: opt_f32(self.voltage),
            position: self.position.map(|(a, b, _)| (a, b)),
        });
    }
    fn point(&self) -> Option<Point<'_>> {
        let (lat, lon, _) = self.position?;
        Some(Point::new(Position::new(lat, lon), self.name()))
    }
}

/// An APRS station or object (LoRa APRS and MeshCom).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StationRec {
    /// Protocol.
    pub protocol: Protocol,
    /// Call (or object name).
    pub call: String,
    /// Symbol table and code.
    pub symbol: String,
    /// Position.
    pub position: Option<(f64, f64)>,
    /// Altitude, m.
    pub alt_m: Option<f64>,
    /// Course, degrees.
    pub course: Option<u16>,
    /// Speed, knots.
    pub speed_kn: Option<f64>,
    /// Comment.
    pub comment: String,
    /// An object or item.
    pub object: bool,
    /// Who sent the object.
    pub via: String,
    /// Last SNR, dB.
    pub snr: f64,
    /// Path of the last packet.
    pub path: String,
    /// APRS kind of the last packet.
    pub last_type: String,
    /// Battery, percent (MeshCom).
    pub battery: Option<u8>,
}

impl Shift for StationRec {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for StationRec {
    type Row = MeshStationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&MeshStationRow {
            net: self.protocol,
            call: &self.call,
            symbol: text(&self.symbol),
            position: self.position,
            r#type: text(&self.last_type),
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
            snr_db: self.snr,
            path: text(&self.path),
            comment: text(&self.comment),
            battery_pct: self.battery.map(f64::from),
        });
    }
    fn point(&self) -> Option<Point<'_>> {
        let (lat, lon) = self.position?;
        let p = Point::new(Position::new(lat, lon), self.call.as_str());
        Some(match self.course {
            Some(c) => p.heading(f32::from(c)),
            None => p,
        })
    }
}

/// A LoRaWAN device.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DeviceRec {
    /// Key: the DevAddr, or `DevEUI <eui>`.
    pub id: String,
    /// DevAddr.
    pub devaddr: String,
    /// DevEUI.
    pub dev_eui: String,
    /// JoinEUI.
    pub join_eui: String,
    /// Maker.
    pub vendor: String,
    /// Network operator.
    pub network: String,
    /// Frequency, Hz.
    pub freq_hz: f64,
    /// Last SNR, dB.
    pub snr: f64,
    /// Last level, dBFS.
    pub level: f64,
    /// SF (0 for FSK).
    pub sf: u8,
    /// Last FCnt.
    pub last_fcnt: Option<u16>,
    /// Uplinks.
    pub uplinks: u32,
    /// Downlinks.
    pub downlinks: u32,
    /// Join requests.
    pub joins: u32,
    /// Confirmed frames.
    pub confirmed: u32,
    /// Frames missed by the counter.
    pub missed: u32,
    /// Last FPort.
    pub fport: Option<u8>,
    /// ADR.
    pub adr: bool,
    /// Last type.
    pub last_type: String,
    /// Last MAC commands.
    pub mac: String,
    /// Battery byte (-1 never sent).
    pub battery: i32,
    /// Margin.
    pub margin: Option<i8>,
    /// Last heard on FSK.
    pub fsk: bool,
}

impl Shift for DeviceRec {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for DeviceRec {
    type Row = MeshDeviceRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&MeshDeviceRow {
            id: &self.id,
            network: text(&self.network),
            vendor: text(if self.vendor.is_empty() {
                &self.join_eui
            } else {
                &self.vendor
            }),
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
            level_dbfs: self.level,
            snr_db: self.snr,
            fcnt: self.last_fcnt.map(i64::from),
            missed: i64::from(self.missed),
            sf: (self.sf > 0).then_some(i64::from(self.sf)),
            freq: Hertz(self.freq_hz),
            uplinks: i64::from(self.uplinks),
            downlinks: i64::from(self.downlinks),
            joins: i64::from(self.joins),
            r#type: text(&self.last_type),
            fport: self.fport.map(i64::from),
            mac: text(&self.mac),
        });
    }
}

impl Shift for MeshPacket {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

/// The row of packet `p` in the packet list, with the raw payload in the content column when `raw` is set.
pub fn packet_row(p: &MeshPacket, raw: bool) -> MeshPacketRow<'_> {
    let content = if raw || p.raw.is_empty() && !p.decrypted && p.summary.is_empty() {
        &p.raw
    } else {
        &p.summary
    };
    let summary = if p.decrypted || p.note.is_empty() {
        &p.summary
    } else {
        &p.note
    };
    MeshPacketRow {
        time_s: p.time,
        net: p.protocol,
        r#type: text(&p.ptype),
        source: text(&p.from),
        target: text(&p.to),
        bytes: i64::try_from(p.size).unwrap_or(i64::MAX),
        content: text(content),
        hops: hops(p.hops),
        snr_db: p.snr_db,
        sf: i64::from(p.sf),
        bw: Hertz(p.bw),
        summary: text(summary),
    }
}

impl Shift for MeshText {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for MeshText {
    type Row = MeshMessageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let failure = self.state.and_then(|s| s.failure()).map(|f| f.reason());
        out.put(&MeshMessageRow {
            time_s: self.time,
            net: self.protocol,
            channel: text(&self.channel),
            source: text(&self.from),
            sender: text(&self.from_name),
            target: text(&self.to),
            text: &self.text,
            hops: hops(self.hops),
            state: self.state.map(|s| s.state()),
            failure,
            relays: self.own.then_some(i64::from(self.relays)),
            packet_id: (self.packet_id != 0).then_some(i64::from(self.packet_id)),
            utc: self.wall.map(|w| Time(w.floor())),
            snr_db: self.snr_db,
            level_dbfs: self.level_dbfs,
            crc_ok: (!self.own).then_some(!self.damaged),
            copies: (!self.own && self.copies > 0).then_some(i64::from(self.copies)),
            failure_text: failure.map(sdrkit_core::Choice::label),
        });
    }
}

impl Shift for MeshReading {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

/// The row of reading `x` in the readings.
pub fn reading_row(x: &MeshReading) -> MeshReadingRow<'_> {
    MeshReadingRow {
        time_s: x.time,
        node: &x.node,
        battery_pct: x.battery.map(f64::from),
        voltage_v: opt_f32(x.voltage),
        ch_util_pct: opt_f32(x.ch_util),
        air_tx_pct: opt_f32(x.air_tx),
        temp_c: opt_f32(x.temp),
        humidity_pct: opt_f32(x.humidity),
        pressure_hpa: opt_f32(x.pressure),
        position: x.position.map(|(a, b, _)| (a, b)),
        alt_m: x.position.and_then(|p| p.2),
    }
}

/// The messages of a stack: received texts with their copies and the wait of damaged frames, and own messages with
/// their delivery state. The mesh receiver and the `aprs` stack each keep one and show it as the table `messages`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessageLog {
    /// The rows, oldest first.
    rows: LogRing<MeshText>,
    #[serde(skip, default = "dedup")]
    keys: Dedup<u64>,
    /// Messages of frames with a failed CRC that wait for a good copy, oldest first, each with the fingerprint of its key.
    #[serde(skip)]
    held: Vec<(u64, MeshText)>,
    /// The shown messages of frames with a failed CRC: the fingerprint of the key and the signal time.
    #[serde(skip)]
    damaged: Vec<(u64, f64)>,
}

fn dedup() -> Dedup<u64> {
    Dedup::new(DEDUP_S, DEDUP_CAP)
}

impl Default for MessageLog {
    fn default() -> Self {
        MessageLog {
            rows: LogRing::new(MESSAGES_CAP),
            keys: dedup(),
            held: Vec::new(),
            damaged: Vec::new(),
        }
    }
}

impl Shift for MessageLog {
    fn shift_times(&mut self, by_s: f64) {
        self.rows.shift_times(by_s);
    }
}

/// The wall time of signal time `t` for the computer clock `wall0` at signal time 0 (0 when not known).
fn wall_of(wall0: f64, t: f64) -> Option<f64> {
    (wall0 > 0.0).then_some(wall0 + t)
}

impl MessageLog {
    /// The number of rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Returns true if the log has no row.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The rows, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &MeshText> {
        self.rows.iter()
    }

    /// Removes every row and what waits.
    pub fn clear(&mut self) {
        self.rows.clear();
        self.keys.clear();
        self.held.clear();
        self.damaged.clear();
    }

    /// Takes the rows of a saved log.
    pub fn adopt(&mut self, saved: Self) {
        self.rows.adopt(saved.rows);
        self.keys.clear();
        self.held.clear();
        self.damaged.clear();
    }

    /// Adds the message of a frame unless the same key was seen in the last 10 minutes; a repeat adds one to the copies
    /// of the shown row, and a good repeat corrects a row shown from a damaged frame. A message of a damaged frame with
    /// this key that waits is dropped and its copies are counted, and one that is shown gets the content of `m` in its
    /// row. `wall0` is the computer clock at signal time 0 (0 when not known). Returns whether the message was added or
    /// its row was corrected.
    pub fn add(&mut self, mut m: MeshText, key: &str, wall0: f64) -> bool {
        let fp = fingerprint(key.as_bytes());
        m.copy_key = fp;
        m.copies = m.copies.max(1);
        if let Some(i) = self.held.iter().position(|h| h.0 == fp) {
            let (_, h) = self.held.remove(i);
            m.copies = m.copies.saturating_add(h.copies);
        }
        m.wall = wall_of(wall0, m.time);
        if let Some(i) = self.damaged.iter().position(|d| d.0 == fp) {
            self.damaged.remove(i);
            self.keys.seen(fp, m.time);
            match self.shown_mut(fp) {
                Some(x) => {
                    m.copies = m.copies.saturating_add(x.copies);
                    *x = m;
                }
                None => {
                    self.rows.push(m);
                }
            }
            return true;
        }
        if self.keys.seen(fp, m.time) {
            // A good repeat of a text shown from a damaged frame (protocols without the wait) corrects the row.
            let Some(x) = self.shown_mut(fp) else {
                return false;
            };
            let copies = x.copies.saturating_add(1);
            let corrected = x.damaged && !m.damaged;
            if corrected {
                *x = m;
            }
            x.copies = copies;
            return corrected;
        }
        self.rows.push(m);
        true
    }

    /// Takes the message of a frame with a failed CRC. It waits [`HOLD_S`] for a copy with a good CRC and is then shown by
    /// [`MessageLog::release_held`]. A message whose key was seen, or waits already, is dropped and counted as a copy.
    pub fn hold(&mut self, mut m: MeshText, key: &str) {
        let fp = fingerprint(key.as_bytes());
        if self.keys.contains(&fp, m.time) || self.damaged.iter().any(|d| d.0 == fp) {
            self.count_copy(fp);
            return;
        }
        if let Some(h) = self.held.iter_mut().find(|h| h.0 == fp) {
            h.1.copies = h.1.copies.saturating_add(1);
            return;
        }
        m.copy_key = fp;
        m.copies = m.copies.max(1);
        m.damaged = true;
        if self.held.len() >= DAMAGED_CAP {
            self.held.remove(0);
        }
        self.held.push((fp, m));
    }

    /// The newest shown received message with the key fingerprint `fp`.
    fn shown_mut(&mut self, fp: u64) -> Option<&mut MeshText> {
        self.rows
            .find_recent_mut(MESSAGES_CAP, |x| !x.own && x.copy_key == fp)
    }

    /// Adds one to the copies of the shown message with the key fingerprint `fp`, if it is still in the log.
    fn count_copy(&mut self, fp: u64) {
        if let Some(x) = self.shown_mut(fp) {
            x.copies = x.copies.saturating_add(1);
        }
    }

    /// Shows the messages of damaged frames that waited [`HOLD_S`] before signal time `now` without a good copy; every one
    /// when `all` is set, at the end of the signal. Returns them. A good copy that comes within 10 minutes corrects the row.
    pub fn release_held(&mut self, now: f64, all: bool, wall0: f64) -> Vec<MeshText> {
        let mut out = Vec::new();
        self.damaged.retain(|d| now - d.1 <= DEDUP_S);
        while self
            .held
            .first()
            .is_some_and(|h| all || now - h.1.time >= HOLD_S)
        {
            let (fp, mut m) = self.held.remove(0);
            m.wall = wall_of(wall0, m.time);
            if self.damaged.len() >= DAMAGED_CAP {
                self.damaged.remove(0);
            }
            self.damaged.push((fp, m.time));
            self.rows.push(m.clone());
            out.push(m);
        }
        out
    }

    /// Updates an own message (by its key and protocol), or adds it while it is new: queued, or failed at once. A later
    /// state of a message whose row has left the log adds no row.
    pub fn upsert_own(&mut self, mut m: MeshText, wall0: f64) {
        let (key, proto) = (m.key, m.protocol);
        m.wall = wall_of(wall0, m.time);
        match self.rows.find_recent_mut(MESSAGES_CAP, |x| {
            x.own && x.key == key && x.protocol == proto
        }) {
            Some(x) => *x = m,
            None if matches!(m.state, None | Some(Delivery::Queued | Delivery::Failed(_))) => {
                self.rows.push(m);
            }
            None => {}
        }
    }

    /// Writes the rows to the table `messages`.
    pub fn publish(&self, p: &mut Published, now: f64) {
        shaped::log_rows(&self.rows, &MESH_MESSAGES, now, p);
    }
}

/// The tables.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshStore {
    /// Packets, oldest first.
    pub packets: LogRing<MeshPacket>,
    /// Nodes by "protocol/id".
    pub nodes: ObjectTable<String, NodeRec>,
    /// Messages.
    pub messages: MessageLog,
    /// Readings, oldest first.
    pub readings: LogRing<MeshReading>,
    /// Packets by protocol and type: `<protocol key>/<type>`.
    pub types: Tally<String>,
    /// APRS stations by "protocol/call".
    pub stations: ObjectTable<String, StationRec>,
    /// LoRaWAN devices by id.
    pub devices: ObjectTable<String, DeviceRec>,
    /// Packets added since the store was made: the row key of the newest packet is one less.
    #[serde(default)]
    pub packets_added: u64,
    /// Readings added since the store was made.
    #[serde(default)]
    pub readings_added: u64,
    /// The computer clock at signal time 0 (seconds since 1970), 0 when not known.
    #[serde(skip)]
    pub wall0: f64,
}

impl Default for MeshStore {
    fn default() -> Self {
        MeshStore {
            packets: LogRing::new(PACKETS_CAP),
            nodes: ObjectTable::new(NODES_CAP),
            messages: MessageLog::default(),
            readings: LogRing::new(READINGS_CAP),
            types: Tally::new(TYPES_CAP),
            stations: ObjectTable::new(STATIONS_CAP),
            devices: ObjectTable::new(DEVICES_CAP),
            packets_added: 0,
            readings_added: 0,
            wall0: 0.0,
        }
    }
}

impl Shift for MeshStore {
    fn shift_times(&mut self, by_s: f64) {
        self.packets.shift_times(by_s);
        self.nodes.shift_times(by_s);
        self.messages.shift_times(by_s);
        self.readings.shift_times(by_s);
        self.stations.shift_times(by_s);
        self.devices.shift_times(by_s);
    }
}

impl Tables for MeshStore {
    const FORMAT: Format = Format::new("lora.mesh_tables", 2);
    fn clear(&mut self) {
        self.packets.clear();
        self.nodes.clear();
        self.messages.clear();
        self.readings.clear();
        self.types.clear();
        self.stations.clear();
        self.devices.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.packets.adopt(saved.packets);
        self.nodes.adopt(saved.nodes);
        self.messages.adopt(saved.messages);
        self.readings.adopt(saved.readings);
        self.types = saved.types;
        self.stations.adopt(saved.stations);
        self.devices.adopt(saved.devices);
        self.packets_added = saved.packets_added.max(self.packets.len() as u64);
        self.readings_added = saved.readings_added.max(self.readings.len() as u64);
    }
}

fn type_key(p: Protocol, t: &str) -> String {
    format!("{}/{t}", p.kind())
}

fn node_key(p: Protocol, id: &str) -> String {
    format!("{}/{id}", p.kind())
}

fn new_node(p: Protocol, id: &str) -> impl FnOnce() -> NodeRec + '_ {
    move || NodeRec {
        protocol: p,
        id: id.into(),
        hops: -1,
        best_hops: i32::MAX,
        ..Default::default()
    }
}

impl MeshStore {
    /// The stamp of signal time `now`, with the wall time when the clock is known.
    fn stamp(&self, now: f64) -> Stamp {
        if self.wall0 > 0.0 {
            Stamp::at(now).with_wall(self.wall0 + now)
        } else {
            Stamp::at(now)
        }
    }

    /// Adds a packet row.
    pub fn add_packet(&mut self, p: MeshPacket) {
        self.packets.push(p);
        self.packets_added += 1;
    }

    /// Counts a packet type.
    pub fn count(&mut self, p: Protocol, t: &str) {
        self.types.add(type_key(p, t));
    }

    /// The count of a packet type.
    pub fn type_count(&self, p: Protocol, t: &str) -> u64 {
        self.types.get(&type_key(p, t))
    }

    /// A node.
    pub fn node(&self, p: Protocol, id: &str) -> Option<&NodeRec> {
        self.nodes.get(&node_key(p, id))
    }

    /// When a node was first and last heard, and how often.
    pub fn node_seen(&self, p: Protocol, id: &str) -> Option<&Seen> {
        self.nodes.seen(&node_key(p, id))
    }

    /// A node to update, made (and counted as heard at `now`) when it is new. The second value is true for a new node. `None`
    /// only for a time that is not finite.
    pub fn node_mut(&mut self, p: Protocol, id: &str, now: f64) -> Option<(&mut NodeRec, bool)> {
        let k = node_key(p, id);
        if self.nodes.contains(&k) {
            return self.nodes.get_mut(&k).map(|n| (n, false));
        }
        let up = self
            .nodes
            .upsert(&k, self.stamp(now), new_node(p, id))
            .ok()?;
        Some((up.row, true))
    }

    /// A node is heard: the count, the SNR statistics and the hops. Returns true for a new node.
    pub fn heard(
        &mut self,
        p: Protocol,
        id: &str,
        now: f64,
        snr: f64,
        level: f64,
        hops: i32,
    ) -> bool {
        let k = node_key(p, id);
        let new = !self.nodes.contains(&k);
        if let Ok(up) = self.nodes.upsert(&k, self.stamp(now), new_node(p, id)) {
            let n = up.row;
            n.snr.add(snr);
            n.last_level = level;
            if hops >= 0 {
                n.hops = hops;
            }
        }
        new
    }

    /// Adds the message of a frame ([`MessageLog::add`]). Returns whether the message was added or its row was corrected.
    pub fn add_message(&mut self, m: MeshText, key: String) -> bool {
        self.messages.add(m, &key, self.wall0)
    }

    /// Takes the message of a frame with a failed CRC ([`MessageLog::hold`]).
    pub fn hold_message(&mut self, m: MeshText, key: String) {
        self.messages.hold(m, &key);
    }

    /// Shows the messages of damaged frames that waited long enough ([`MessageLog::release_held`]), and returns them.
    pub fn release_held(&mut self, now: f64, all: bool) -> Vec<MeshText> {
        self.messages.release_held(now, all, self.wall0)
    }

    /// Adds an own message or updates it ([`MessageLog::upsert_own`]).
    pub fn upsert_own(&mut self, m: MeshText) {
        self.messages.upsert_own(m, self.wall0);
    }

    /// Adds a reading.
    pub fn add_reading(&mut self, r: MeshReading) {
        self.readings.push(r);
        self.readings_added += 1;
    }

    /// A station is heard at `now`, made when new (true in the second value). `None` only for a time that is not finite.
    pub fn station_mut(
        &mut self,
        p: Protocol,
        call: &str,
        now: f64,
    ) -> Option<(&mut StationRec, bool)> {
        let k = format!("{}/{call}", p.kind());
        let new = !self.stations.contains(&k);
        let up = self
            .stations
            .upsert(&k, self.stamp(now), || StationRec {
                protocol: p,
                call: call.into(),
                ..Default::default()
            })
            .ok()?;
        Some((up.row, new))
    }

    /// A device is heard at `now`, made when new (true in the second value). `None` only for a time that is not finite.
    pub fn device_mut(&mut self, id: &str, now: f64) -> Option<(&mut DeviceRec, bool)> {
        let new = !self.devices.contains(id);
        let up = self
            .devices
            .upsert(&id.to_string(), self.stamp(now), || DeviceRec {
                id: id.into(),
                battery: -1,
                ..Default::default()
            })
            .ok()?;
        Some((up.row, new))
    }

    /// Nodes of one protocol.
    pub fn node_count(&self, p: Protocol) -> usize {
        self.nodes.iter().filter(|n| n.record.protocol == p).count()
    }

    /// Writes the tables (the packets, the nodes, the messages, the readings, the stations, the devices and the type counts) and
    /// the map points. `key` holds the row keys while they are written.
    pub fn publish(&self, p: &mut Published, now: f64, raw: bool, key: &mut String) {
        let first = self.packets_added - self.packets.len() as u64;
        for (i, pk) in self.packets.iter().enumerate() {
            key.clear();
            let _ = write!(key, "{:06}", first + i as u64);
            MESH_PACKETS.put(p, key, &packet_row(pk, raw));
        }
        shaped::rows(&self.nodes, &MESH_NODES, now, p);
        self.messages.publish(p, now);
        let n = self.readings.len();
        let first = self.readings_added - n as u64;
        for (i, x) in self
            .readings
            .iter()
            .enumerate()
            .skip(n.saturating_sub(READINGS_SHOWN))
        {
            key.clear();
            let _ = write!(key, "{:06}", first + i as u64);
            MESH_READINGS.put(p, key, &reading_row(x));
        }
        shaped::rows(&self.stations, &MESH_STATIONS, now, p);
        self.publish_devices(p, now);
        let res: &mut Results = p.results_mut();
        shaped::row_points(&self.nodes, &MESH_NODES, "nodes/", res);
        shaped::row_points(&self.stations, &MESH_STATIONS, "stations/", res);
    }

    /// Writes the device table and the type counts only (the tables of `lorawanwide`).
    pub fn publish_devices(&self, p: &mut Published, now: f64) {
        shaped::rows(&self.devices, &MESH_DEVICES, now, p);
        for (k, n) in self.types.iter() {
            let Some((net, ty)) = k.split_once('/') else {
                continue;
            };
            let net = Protocol::CHOICES
                .iter()
                .position(|c| c.key == net)
                .and_then(|i| Protocol::from_index(u16::try_from(i).ok()?))
                .unwrap_or(Protocol::Lora);
            MESH_TYPES.put(
                p,
                k,
                &MeshTypeRow {
                    net,
                    r#type: ty,
                    count: i64::try_from(n).unwrap_or(i64::MAX),
                },
            );
        }
    }

    /// The tables that [`MeshStore::publish`] writes.
    pub fn tables() -> [&'static TableDef; 7] {
        [
            MESH_PACKETS.def(),
            MESH_NODES.def(),
            MESH_MESSAGES.def(),
            MESH_READINGS.def(),
            MESH_STATIONS.def(),
            MESH_DEVICES.def(),
            MESH_TYPES.def(),
        ]
    }

    /// Removes every row of the tables that [`MeshStore::publish`] writes.
    pub fn clear_rows(p: &mut Published) {
        for t in Self::tables() {
            p.clear_rows(t);
        }
        p.results_mut().clear_points();
    }

    /// The session report.
    pub fn report(&self, plan_line: &str) -> String {
        let mut s = String::new();
        s += &format!("Listening to: {plan_line}\n");
        let t0 = self.packets.iter().next().map(|p| p.time).unwrap_or(0.0);
        let t1 = self
            .packets
            .newest_first()
            .next()
            .map(|p| p.time)
            .unwrap_or(0.0);
        s += &format!("Time span: {t0:.1} s to {t1:.1} s of signal\n");
        let mut by: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
        for p in self.packets.iter() {
            let e = by.entry(p.protocol.name()).or_default();
            if p.crc_ok {
                e.0 += 1
            } else {
                e.1 += 1
            }
        }
        for (k, (g, b)) in by {
            s += &format!("{k}: {g} good, {b} bad frames\n");
        }
        for (k, n) in self.types.iter() {
            s += &format!("  {k}: {n}\n");
        }
        for row in self.nodes.iter() {
            let n = row.record;
            let seen = row.seen;
            s += &format!(
                "{} {} {}: {} packets, heard {:.1} to {:.1} s, SNR {:.1} / {:.1} / {:.1} dB, hops {}, battery {}{}\n",
                n.name(),
                n.id,
                n.hw,
                seen.count,
                seen.first_s,
                seen.last_s,
                n.snr.min.unwrap_or(0.0),
                n.snr.mean().unwrap_or(0.0),
                n.snr.max.unwrap_or(0.0),
                n.hops,
                n.battery_text(),
                n.position
                    .map(|(a, b, _)| format!(", at {a:.5}, {b:.5}"))
                    .unwrap_or_default()
            );
        }
        s
    }
}
