//! SCADA radio telemetry (`scada`): Modbus RTU, Modbus ASCII and DNP3 over narrowband FSK radio links (Bell 202 and V.23 audio
//! FSK, direct 2-FSK and 4-FSK), in async characters or HDLC. The stack finds the modem by itself, reads the frames, pairs polls
//! with replies and lists the devices with their registers and points. Proprietary radio framings are not decoded: their
//! bursts are kept as raw bits. DNP3 Secure Authentication is shown raw. Receive only: the stack has no path to send or
//! repeat a frame.
//!
//! Standards: Modbus Application Protocol V1.1b3, Modbus over Serial Line V1.02, IEEE Std 1815, ISO/IEC 13239. The `note`
//! field names what no public document confirms.

pub mod dnp3;
pub mod frame;
pub mod gen;
pub mod modbus;
pub mod modem;

use crate::typed::{
    ScadaCsvReply, ScadaDeviceEvent, ScadaDeviceRow, ScadaFields, ScadaMessageEvent,
    ScadaMessageRow, SCADA_DEVICE, SCADA_DEVICES, SCADA_EXPORT_CSV, SCADA_MESSAGE, SCADA_MESSAGES,
};
use frame::Protocol;
use modbus::{Asked, Pdu};
use modem::{RxFrame, RxUnknown, ScadaHeard, ScadaModem, ScadaStatus};
use sdrkit_core::interface::{Fields, Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::record::RecordDecl;
use sdrkit_core::{
    Clock, Command, CommandError, CommandId, ConfigError, EventDef, GenConfig, Generator,
    LockState, Receiver, Record, ResultsDecl, Schema, StackConfig, StateBlob, StateError, Values,
};
use sdrkit_graph::{
    no_values, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Item, Outbox, Port, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "scada";
/// The least input rate, Hz.
pub const MIN_RATE: f64 = 96e3;
/// The most messages kept.
pub const MESSAGE_CAP: usize = 1000;
/// The most devices kept.
pub const DEVICE_CAP: usize = 256;
/// The most points kept for each device.
pub const POINT_CAP: usize = 64;
/// A message within this many seconds of signal shows the lock.
pub const RECENT_S: f64 = 10.0;
/// A Modbus request and a DNP3 link request wait this long for their reply, s.
pub const MODBUS_WAIT_S: f64 = 5.0;
/// A DNP3 application request waits this long for its response, s.
pub const DNP3_WAIT_S: f64 = 10.0;
/// The header of the CSV export.
pub const CSV_HEADER: &str =
    "id,time_s,wall_clock_utc,link,protocol,from,to,function,summary,crc,direction,pair_id,response_ms,encrypted,raw_hex";
/// What no public document available here confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: the carriage of async bytes on direct 2-FSK and 4-FSK with an idle mark, the 4-FSK dibit order (+3 01, +1 00, -1 10, -3 11), the Modbus function length table, the exception codes, the DNP3 qualifier, string and file objects, and the Bell 202 and V.23 tones. Proprietary radio framings are not decoded: their bursts are shown as not recognised, with the raw bits. DNP3 Secure Authentication is shown raw. Receive only.";

/// The direction of a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// A poll or command.
    Request,
    /// A reply, a confirm.
    Reply,
    /// An unsolicited response.
    Unsolicited,
}

impl Direction {
    fn label(self) -> &'static str {
        match self {
            Direction::Request => "request",
            Direction::Reply => "reply",
            Direction::Unsolicited => "unsolicited",
        }
    }
}

/// One message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScadaMessage {
    /// The message number.
    pub id: u64,
    /// Signal time, s.
    pub t: f64,
    /// Wall-clock time, s since 1970 (UTC), when known.
    pub wall: Option<f64>,
    /// The link.
    pub link: String,
    /// The protocol: "Modbus RTU", "Modbus ASCII", "DNP3", "unknown".
    pub protocol: String,
    /// The sender.
    pub from: String,
    /// The receiver.
    pub to: String,
    /// The function.
    pub function: String,
    /// The summary.
    pub summary: String,
    /// The check: good, failed, none.
    pub crc: Option<bool>,
    /// The direction.
    pub direction: Option<Direction>,
    /// The paired message.
    pub pair: Option<u64>,
    /// The response time, ms.
    pub response_ms: Option<f64>,
    /// DNP3 Secure Authentication.
    pub encrypted: bool,
    /// The bytes or bits as hex.
    pub raw_hex: String,
    /// -20 log10 of the eye error, dB.
    pub quality_db: f64,
}

impl Shift for ScadaMessage {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// The role of a device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// A Modbus slave.
    Slave,
    /// A DNP3 master.
    Master,
    /// A DNP3 outstation.
    Outstation,
}

impl Role {
    fn label(self) -> &'static str {
        match self {
            Role::Slave => "slave",
            Role::Master => "master",
            Role::Outstation => "outstation",
        }
    }
}

/// One device.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// "Modbus" or "DNP3".
    pub protocol: String,
    /// The address.
    pub address: u16,
    /// The role.
    pub role: Role,
    /// First heard, signal time.
    pub first_s: f64,
    /// Last heard, signal time.
    pub last_s: f64,
    /// First and last heard, wall clock.
    pub first_wall: Option<f64>,
    /// Last heard, wall clock.
    pub last_wall: Option<f64>,
    /// The last poll, signal time.
    pub last_poll_s: Option<f64>,
    /// Polls sent or received.
    pub polls: u64,
    /// Replies sent or received.
    pub replies: u64,
    /// Polls with no reply.
    pub unanswered: u64,
    /// Exception replies.
    pub exceptions: u64,
    /// Unsolicited responses.
    pub unsolicited: u64,
    /// Frames whose check failed.
    pub crc_failures: u64,
    /// The sum and count of the response times, ms.
    pub response_sum_ms: f64,
    /// The count of the response times.
    pub responses: u64,
    /// The last internal indications.
    pub last_iin: Option<String>,
    /// The peers (addresses).
    pub peers: Vec<u16>,
    /// The registers or points seen: name and value.
    pub points: Vec<(String, String)>,
}

impl Device {
    fn key(&self) -> String {
        format!("{}/{}", self.protocol.to_ascii_lowercase(), self.address)
    }

    fn point(&mut self, name: String, value: String) {
        if let Some(p) = self.points.iter_mut().find(|p| p.0 == name) {
            p.1 = value;
        } else if self.points.len() < POINT_CAP {
            self.points.push((name, value));
        }
    }

    fn peer(&mut self, a: u16) {
        if !self.peers.contains(&a) && self.peers.len() < 16 {
            self.peers.push(a);
        }
    }
}

/// The counters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScadaCounters {
    /// Modbus messages.
    pub modbus: u64,
    /// DNP3 messages.
    pub dnp3: u64,
    /// Messages of no known protocol.
    pub unknown: u64,
    /// Checks that failed.
    pub crc_failures: u64,
    /// Secure Authentication messages.
    pub encrypted: u64,
    /// Bursts not recognised.
    pub not_recognised: u64,
}

/// What the stack keeps and saves.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScadaHeld {
    /// The messages, newest last.
    pub messages: LogRing<ScadaMessage>,
    /// The devices.
    pub devices: Vec<Device>,
    /// The counters.
    pub counters: ScadaCounters,
    /// The next message number.
    pub next_id: u64,
    /// The link of the last good frame.
    pub link: Option<String>,
}

impl Default for ScadaHeld {
    fn default() -> Self {
        ScadaHeld {
            messages: LogRing::new(MESSAGE_CAP),
            devices: Vec::new(),
            counters: ScadaCounters::default(),
            next_id: 1,
            link: None,
        }
    }
}

impl Shift for ScadaHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.messages.shift_times(by_s);
        for d in &mut self.devices {
            d.first_s += by_s;
            d.last_s += by_s;
            if let Some(t) = d.last_poll_s.as_mut() {
                *t += by_s;
            }
        }
    }
}

impl Tables for ScadaHeld {
    const FORMAT: StateFormat = StateFormat::new("trunk.scada", 1);
    fn clear(&mut self) {
        *self = ScadaHeld::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.messages.adopt(saved.messages);
        self.devices = saved.devices;
        self.counters = saved.counters;
        self.next_id = saved.next_id;
        self.link = saved.link;
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

impl Shaped for ScadaMessage {
    type Row = ScadaMessageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&ScadaMessageRow {
            id: self.id as i64,
            time_s: self.t,
            wall_clock_utc: self.wall.map(Time),
            link: &self.link,
            protocol: &self.protocol,
            from: &self.from,
            to: &self.to,
            function: &self.function,
            summary: &self.summary,
            crc: crc_text(self.crc),
            direction: self.direction.map(Direction::label),
            pair_id: self.pair.map(|p| p as i64),
            response_ms: self.response_ms,
            encrypted: self.encrypted,
            raw_hex: &self.raw_hex,
            quality_db: self.quality_db,
        });
    }
}

fn crc_text(c: Option<bool>) -> &'static str {
    match c {
        Some(true) => "good",
        Some(false) => "failed",
        None => "",
    }
}

/// A message as an event item.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageItem(pub ScadaMessage);

impl Item for MessageItem {
    const KIND: &'static str = "scada.message";
    fn event_def() -> Option<&'static EventDef> {
        Some(SCADA_MESSAGE.def())
    }
    fn write_fields(&self, f: &mut Record) {
        let m = &self.0;
        ScadaMessageEvent {
            source: &m.from,
            target: &m.to,
            protocol: &m.protocol,
            function: &m.function,
            text: &m.summary,
            crc: crc_text(m.crc),
            family: Family::Sensor,
        }
        .write(f);
    }
}

/// A new device as an event item.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceItem {
    /// "Modbus" or "DNP3".
    pub protocol: String,
    /// The address.
    pub address: u16,
    /// The role.
    pub role: Role,
}

impl Item for DeviceItem {
    const KIND: &'static str = "scada.device";
    fn event_def() -> Option<&'static EventDef> {
        Some(SCADA_DEVICE.def())
    }
    fn write_fields(&self, f: &mut Record) {
        ScadaDeviceEvent {
            protocol: &self.protocol,
            address: i64::from(self.address),
            role: self.role.label(),
            family: Family::Sensor,
        }
        .write(f);
    }
}

/// A Modbus request that waits for its reply.
#[derive(Clone, Copy, Debug)]
struct ModbusWait {
    slave: u8,
    function: u8,
    id: u64,
    t: f64,
    asked: Asked,
}

/// A DNP3 request that waits: the requester, the responder, the sequence or link function, the message.
#[derive(Clone, Copy, Debug)]
struct Dnp3Wait {
    from: u16,
    to: u16,
    seq: u8,
    id: u64,
    t: f64,
}

/// The `scada.messages` decoder: meaning, pairing, devices, the tables, the events and the export.
pub struct ScadaMessages {
    held: ScadaHeld,
    now: f64,
    started: bool,
    last_message: Option<f64>,
    status: ScadaStatus,
    offset_hz: Option<f32>,
    deviation_hz: Option<f32>,
    clock: Clock,
    wall_origin: Option<f64>,
    modbus_wait: Vec<ModbusWait>,
    app_wait: Vec<Dnp3Wait>,
    confirm_wait: Vec<Dnp3Wait>,
    link_wait: Vec<Dnp3Wait>,
    segments: dnp3::Segments,
    messages_out: Vec<Timed<MessageItem>>,
    devices_out: Vec<Timed<DeviceItem>>,
    pending_log: Vec<String>,
    cleared: bool,
    dirty: bool,
    summary: String,
}

impl Default for ScadaMessages {
    fn default() -> Self {
        ScadaMessages {
            held: ScadaHeld::default(),
            now: 0.0,
            started: false,
            last_message: None,
            status: ScadaStatus::default(),
            offset_hz: None,
            deviation_hz: None,
            clock: Clock::fixed(0.0),
            wall_origin: None,
            modbus_wait: Vec::new(),
            app_wait: Vec::new(),
            confirm_wait: Vec::new(),
            link_wait: Vec::new(),
            segments: dnp3::Segments::default(),
            messages_out: Vec::new(),
            devices_out: Vec::new(),
            pending_log: Vec::new(),
            cleared: false,
            dirty: false,
            summary: String::new(),
        }
    }
}

static COMMANDS: [&Command; 1] = [&SCADA_EXPORT_CSV.command];

/// A CSV cell: quoted when it holds a comma, a quote or a line break (RFC 4180).
fn csv_cell(s: &str, out: &mut String) {
    if s.contains([',', '"', '\n', '\r']) {
        out.push('"');
        out.push_str(&s.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(s);
    }
}

impl ScadaMessages {
    /// The messages, devices and counters.
    pub fn held(&self) -> &ScadaHeld {
        &self.held
    }

    /// The messages as CSV text: the header, then one row a message.
    pub fn csv(&self) -> String {
        let mut s = String::from(CSV_HEADER);
        s.push('\n');
        for m in self.held.messages.iter() {
            let wall = m
                .wall
                .and_then(|w| sdrkit_util_geo::time::iso8601(w, 3).ok())
                .map(|t| t.as_str().to_string())
                .unwrap_or_default();
            let cells = [
                m.id.to_string(),
                format!("{:.3}", m.t),
                wall,
                m.link.clone(),
                m.protocol.clone(),
                m.from.clone(),
                m.to.clone(),
                m.function.clone(),
                m.summary.clone(),
                crc_text(m.crc).to_string(),
                m.direction.map_or("", Direction::label).to_string(),
                m.pair.map_or(String::new(), |p| p.to_string()),
                m.response_ms.map_or(String::new(), |r| format!("{r:.0}")),
                m.encrypted.to_string(),
                m.raw_hex.clone(),
            ];
            for (k, c) in cells.iter().enumerate() {
                if k > 0 {
                    s.push(',');
                }
                csv_cell(c, &mut s);
            }
            s.push('\n');
        }
        s
    }

    fn wall(&mut self, t: f64) -> Option<f64> {
        if self.wall_origin.is_none() {
            let now = self.clock.now();
            if now > 0.0 {
                self.wall_origin = Some(now - t);
            }
        }
        self.wall_origin.map(|o| o + t)
    }

    /// The device of a protocol and address, made when new.
    fn device(&mut self, t: f64, protocol: &str, address: u16, role: Role) -> &mut Device {
        let wall = self.wall(t);
        let at = self
            .held
            .devices
            .iter()
            .position(|d| d.protocol == protocol && d.address == address);
        let i = match at {
            Some(i) => i,
            None => {
                if self.held.devices.len() >= DEVICE_CAP {
                    // The device heard longest ago leaves.
                    if let Some(old) = self
                        .held
                        .devices
                        .iter()
                        .enumerate()
                        .min_by(|a, b| a.1.last_s.total_cmp(&b.1.last_s))
                        .map(|(i, _)| i)
                    {
                        self.held.devices.remove(old);
                    }
                }
                self.held.devices.push(Device {
                    protocol: protocol.to_string(),
                    address,
                    role,
                    first_s: t,
                    last_s: t,
                    first_wall: wall,
                    last_wall: wall,
                    last_poll_s: None,
                    polls: 0,
                    replies: 0,
                    unanswered: 0,
                    exceptions: 0,
                    unsolicited: 0,
                    crc_failures: 0,
                    response_sum_ms: 0.0,
                    responses: 0,
                    last_iin: None,
                    peers: Vec::new(),
                    points: Vec::new(),
                });
                self.devices_out.push(Timed {
                    time: t,
                    value: DeviceItem {
                        protocol: protocol.to_string(),
                        address,
                        role,
                    },
                });
                self.pending_log.push(format!(
                    "scada: new device {protocol} {} {address}",
                    role.label()
                ));
                self.held.devices.len() - 1
            }
        };
        let d = &mut self.held.devices[i];
        d.last_s = d.last_s.max(t);
        d.last_wall = wall.or(d.last_wall);
        d
    }

    fn push_message(&mut self, m: ScadaMessage) {
        self.last_message = Some(m.t);
        self.pending_log.push(format!(
            "scada: {} {} -> {}: {} {}",
            m.protocol, m.from, m.to, m.function, m.summary
        ));
        self.messages_out.push(Timed {
            time: m.t,
            value: MessageItem(m.clone()),
        });
        self.held.messages.push(m);
        self.dirty = true;
    }

    fn new_message(&mut self, t: f64, f: &RxFrame) -> ScadaMessage {
        let id = self.held.next_id;
        self.held.next_id += 1;
        ScadaMessage {
            id,
            t,
            wall: self.wall(t),
            link: f.link.short.clone(),
            protocol: f.protocol.map_or("unknown", Protocol::label).to_string(),
            from: String::new(),
            to: String::new(),
            function: String::new(),
            summary: String::new(),
            crc: Some(f.crc_ok),
            direction: None,
            pair: None,
            response_ms: None,
            encrypted: false,
            raw_hex: modbus::hex(&f.bytes),
            quality_db: f64::from(f.quality_db),
        }
    }

    /// Sets the pair and response time of a reply and of the request it answers.
    fn pair(&mut self, request: u64, request_t: f64, reply: &mut ScadaMessage) -> f64 {
        let ms = (reply.t - request_t) * 1000.0;
        reply.pair = Some(request);
        reply.response_ms = Some(ms);
        if let Some(r) = self
            .held
            .messages
            .iter_mut()
            .rev()
            .find(|m| m.id == request)
        {
            r.pair = Some(reply.id);
        }
        ms
    }

    /// Counts the requests that waited too long as unanswered.
    fn expire(&mut self, now: f64) {
        let mut gone: Vec<(&'static str, u16)> = Vec::new();
        self.modbus_wait.retain(|w| {
            let keep = now - w.t <= MODBUS_WAIT_S;
            if !keep {
                gone.push(("Modbus", u16::from(w.slave)));
            }
            keep
        });
        self.app_wait.retain(|w| {
            let keep = now - w.t <= DNP3_WAIT_S;
            if !keep {
                gone.push(("DNP3", w.to));
            }
            keep
        });
        self.confirm_wait.retain(|w| now - w.t <= DNP3_WAIT_S);
        self.link_wait.retain(|w| now - w.t <= MODBUS_WAIT_S);
        for (p, a) in gone {
            if let Some(d) = self
                .held
                .devices
                .iter_mut()
                .find(|d| d.protocol == p && d.address == a)
            {
                d.unanswered += 1;
                self.dirty = true;
            }
        }
    }

    fn frame(&mut self, t: f64, f: &RxFrame) {
        self.expire(t);
        if f.crc_ok {
            self.held.link = Some(f.link.long.clone());
            self.offset_hz = Some(f.link.offset_hz);
            self.deviation_hz = Some(f.link.deviation_hz);
        } else {
            self.held.counters.crc_failures += 1;
        }
        match f.protocol {
            Some(Protocol::ModbusRtu) => self.modbus(t, f, 2),
            Some(Protocol::ModbusAscii) => self.modbus(t, f, 1),
            Some(Protocol::Dnp3) => self.dnp3(t, f),
            None => {
                self.held.counters.unknown += 1;
                let mut m = self.new_message(t, f);
                m.function = "HDLC frame".into();
                m.summary = f.note.clone().unwrap_or_default();
                self.push_message(m);
            }
        }
    }

    fn modbus(&mut self, t: f64, f: &RxFrame, check_len: usize) {
        self.held.counters.modbus += 1;
        let mut m = self.new_message(t, f);
        let Some(p) = Pdu::of(&f.bytes, check_len) else {
            return;
        };
        let slave = p.address;
        let slave_text = if slave == 0 {
            "all slaves".to_string()
        } else {
            format!("slave {slave}")
        };
        m.function = if frame::KNOWN_FUNCTIONS.contains(&p.base()) {
            modbus::function_name(p.function).to_string()
        } else {
            format!("function {}", p.base())
        };
        if !f.crc_ok {
            m.from = "unknown".into();
            m.to = slave_text;
            m.summary = f.note.clone().unwrap_or_else(|| "CRC failed".into());
            if slave != 0 {
                self.device(t, "Modbus", u16::from(slave), Role::Slave)
                    .crc_failures += 1;
            }
            self.push_message(m);
            return;
        }
        let waiting = self
            .modbus_wait
            .iter()
            .position(|w| w.slave == slave && w.function == p.base());
        let reply = p.is_exception() || waiting.is_some() || !p.looks_like_request();
        if !frame::KNOWN_FUNCTIONS.contains(&p.base()) {
            m.summary = f.note.clone().unwrap_or_default();
        }
        if reply {
            m.direction = Some(Direction::Reply);
            m.from = slave_text;
            m.to = "master".into();
            let w = waiting.map(|i| self.modbus_wait.remove(i));
            if m.summary.is_empty() {
                m.summary = modbus::reply_summary(&p, w.as_ref().map(|w| &w.asked));
            }
            let ms = w.map(|w| self.pair(w.id, w.t, &mut m));
            if slave != 0 {
                let values = modbus::reply_values(&p, w.as_ref().map(|w| &w.asked));
                let d = self.device(t, "Modbus", u16::from(slave), Role::Slave);
                d.replies += 1;
                if p.is_exception() {
                    d.exceptions += 1;
                }
                if let Some(ms) = ms {
                    d.response_sum_ms += ms;
                    d.responses += 1;
                }
                for (num, v) in values {
                    if let Some(num) = num {
                        d.point(format!("{num:05}"), v.to_string());
                    }
                }
            }
        } else {
            m.direction = Some(Direction::Request);
            m.from = "master".into();
            m.to = slave_text;
            if m.summary.is_empty() {
                m.summary = modbus::request_summary(&p);
            }
            if slave != 0 {
                if let Some(i) = self.modbus_wait.iter().position(|w| w.slave == slave) {
                    self.modbus_wait.remove(i);
                    self.device(t, "Modbus", u16::from(slave), Role::Slave)
                        .unanswered += 1;
                }
                self.modbus_wait.push(ModbusWait {
                    slave,
                    function: p.base(),
                    id: m.id,
                    t,
                    asked: modbus::asked(&p),
                });
                let written: Vec<(u32, u16)> = match p.function {
                    6 => vec![(
                        modbus::number(6, u32::from(modbus::asked(&p).start)),
                        u16::from_be_bytes([p.data[2], p.data[3]]),
                    )],
                    16 => p
                        .data
                        .get(5..)
                        .unwrap_or(&[])
                        .chunks(2)
                        .filter(|c| c.len() == 2)
                        .enumerate()
                        .map(|(k, c)| {
                            (
                                modbus::number(16, u32::from(modbus::asked(&p).start) + k as u32),
                                u16::from_be_bytes([c[0], c[1]]),
                            )
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                let d = self.device(t, "Modbus", u16::from(slave), Role::Slave);
                d.polls += 1;
                d.last_poll_s = Some(t);
                for (num, v) in written {
                    d.point(format!("{num:05}"), v.to_string());
                }
            }
        }
        self.push_message(m);
    }

    fn dnp3(&mut self, t: f64, f: &RxFrame) {
        self.held.counters.dnp3 += 1;
        let mut m = self.new_message(t, f);
        let Some(link) = dnp3::Link::of(&f.bytes) else {
            return;
        };
        let (src_role, dst_role) = if link.dir {
            (Role::Master, Role::Outstation)
        } else {
            (Role::Outstation, Role::Master)
        };
        let name = |r: Role, a: u16| {
            if dnp3::is_broadcast(a) {
                "broadcast".to_string()
            } else {
                format!("{} {a}", r.label())
            }
        };
        m.from = name(src_role, link.src);
        m.to = name(dst_role, link.dest);
        {
            let d = self.device(t, "DNP3", link.src, src_role);
            d.peer(link.dest);
            if !f.crc_ok {
                d.crc_failures += 1;
            }
        }
        if !dnp3::is_broadcast(link.dest) {
            self.device(t, "DNP3", link.dest, dst_role).peer(link.src);
        }
        m.function = link.function_name().to_string();
        if !f.crc_ok {
            m.summary = f.note.clone().unwrap_or_else(|| "CRC failed".into());
            self.push_message(m);
            return;
        }
        // Link pairing: requests 0, 2, 3, 9 with their ack or link status.
        if link.prm && matches!(link.function, 0 | 2 | 3 | 9) {
            self.link_wait.push(Dnp3Wait {
                from: link.src,
                to: link.dest,
                seq: link.function,
                id: m.id,
                t,
            });
            if !link.has_user_data() {
                m.direction = Some(Direction::Request);
            }
        } else if !link.prm {
            m.direction = Some(Direction::Reply);
            if let Some(i) = self
                .link_wait
                .iter()
                .position(|w| w.from == link.dest && w.to == link.src)
            {
                let w = self.link_wait.remove(i);
                self.pair(w.id, w.t, &mut m);
            }
        }
        if !link.has_user_data() {
            m.summary = link.flags();
            self.push_message(m);
            return;
        }
        let (user, _) = frame::dnp3_user_data(&f.bytes);
        let Some((&tb, data)) = user.split_first() else {
            self.push_message(m);
            return;
        };
        let tr = dnp3::Transport::of(tb);
        let fragment = match self.segments.push(link.src, link.dest, tr, data) {
            Ok(Some(frag)) => frag,
            Ok(None) => {
                m.summary = format!("transport segment {}; the fragment continues", tr.seq);
                self.push_message(m);
                return;
            }
            Err(note) => {
                m.summary = note;
                self.push_message(m);
                return;
            }
        };
        let Some(app) = dnp3::read_app(&fragment) else {
            m.summary = "application header cut short".into();
            self.push_message(m);
            return;
        };
        let h = app.header;
        m.function = dnp3::app_function_name(h.function).to_string();
        m.summary = dnp3::summary(&app);
        if app.encrypted {
            m.encrypted = true;
            self.held.counters.encrypted += 1;
        }
        let (master, outstation) = if link.dir {
            (link.src, link.dest)
        } else {
            (link.dest, link.src)
        };
        match h.function {
            129 | 130 => {
                m.direction = Some(if h.function == 130 {
                    Direction::Unsolicited
                } else {
                    Direction::Reply
                });
                let w = if h.function == 129 {
                    self.app_wait
                        .iter()
                        .position(|w| w.from == master && w.to == outstation && w.seq == h.seq)
                        .map(|i| self.app_wait.remove(i))
                } else {
                    None
                };
                let ms = w.map(|w| self.pair(w.id, w.t, &mut m));
                if h.con {
                    self.confirm_wait.push(Dnp3Wait {
                        from: outstation,
                        to: master,
                        seq: h.seq,
                        id: m.id,
                        t,
                    });
                }
                let d = self.device(t, "DNP3", outstation, Role::Outstation);
                if h.function == 130 {
                    d.unsolicited += 1;
                } else {
                    d.replies += 1;
                }
                if let Some(ms) = ms {
                    d.response_sum_ms += ms;
                    d.responses += 1;
                }
                if let Some((a, b)) = h.iin {
                    d.last_iin = Some(dnp3::iin_text(a, b));
                }
                for p in &app.points {
                    d.point(format!("{} {}", p.kind, p.index), p.value.clone());
                }
            }
            0 => {
                m.direction = Some(Direction::Reply);
                if let Some(i) = self
                    .confirm_wait
                    .iter()
                    .position(|w| w.from == outstation && w.to == master && w.seq == h.seq)
                {
                    let w = self.confirm_wait.remove(i);
                    self.pair(w.id, w.t, &mut m);
                }
            }
            _ => {
                m.direction = Some(Direction::Request);
                if let Some(i) = self
                    .app_wait
                    .iter()
                    .position(|w| w.from == master && w.to == outstation && w.seq == h.seq)
                {
                    self.app_wait.remove(i);
                    self.device(t, "DNP3", outstation, Role::Outstation)
                        .unanswered += 1;
                }
                self.app_wait.push(Dnp3Wait {
                    from: master,
                    to: outstation,
                    seq: h.seq,
                    id: m.id,
                    t,
                });
                for a in [master, outstation] {
                    let role = if a == master {
                        Role::Master
                    } else {
                        Role::Outstation
                    };
                    let d = self.device(t, "DNP3", a, role);
                    d.polls += 1;
                    d.last_poll_s = Some(t);
                }
                if h.function == 2 || (3..=6).contains(&h.function) {
                    let d = self.device(t, "DNP3", outstation, Role::Outstation);
                    for p in &app.points {
                        d.point(format!("{} {}", p.kind, p.index), p.value.clone());
                    }
                }
            }
        }
        self.push_message(m);
    }

    fn unknown(&mut self, t: f64, u: &RxUnknown) {
        self.held.counters.not_recognised += 1;
        let id = self.held.next_id;
        self.held.next_id += 1;
        let m = ScadaMessage {
            id,
            t,
            wall: self.wall(t),
            link: u.link.short.clone(),
            protocol: "unknown".into(),
            from: String::new(),
            to: String::new(),
            function: "not recognised".into(),
            summary: format!(
                "{} bits, higher frequency 1, packed most significant first",
                u.nbits
            ),
            crc: None,
            direction: None,
            pair: None,
            response_ms: None,
            encrypted: false,
            raw_hex: modbus::hex(&u.bits),
            quality_db: f64::from(u.quality_db),
        };
        self.push_message(m);
    }
}

impl Shaped for Device {
    type Row = ScadaDeviceRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let mut peers = String::new();
        for (k, p) in self.peers.iter().enumerate() {
            if k > 0 {
                peers.push_str(", ");
            }
            let _ = write!(peers, "{p}");
        }
        let mut points = String::new();
        for (k, (n, v)) in self.points.iter().enumerate() {
            if k > 0 {
                points.push_str(", ");
            }
            let _ = write!(points, "{n} = {v}");
        }
        out.put(&ScadaDeviceRow {
            protocol: &self.protocol,
            address: i64::from(self.address),
            role: self.role.label(),
            first_heard: self.first_wall.map(Time),
            last_heard: self.last_wall.map(Time),
            first_s: self.first_s,
            last_s: self.last_s,
            last_poll_s: self.last_poll_s,
            polls: self.polls as i64,
            replies: self.replies as i64,
            unanswered: self.unanswered as i64,
            exceptions: self.exceptions as i64,
            unsolicited: self.unsolicited as i64,
            crc_failures: self.crc_failures as i64,
            mean_response_ms: (self.responses > 0)
                .then(|| self.response_sum_ms / self.responses as f64),
            last_iin: self.last_iin.as_deref(),
            peers: &peers,
            points: &points,
        });
    }
}

impl Decoder for ScadaMessages {
    type Input = Timed<ScadaHeard>;
    fn name(&self) -> &'static str {
        "scada.messages"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<MessageItem>("messages"),
            Port::typed::<DeviceItem>("devices"),
        ]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &'static [&'static Command] {
        &COMMANDS
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match id.index() {
            0 => {
                let mut r = Record::new(Arc::new(RecordDecl::of(SCADA_EXPORT_CSV.command.reply)));
                ScadaCsvReply { csv: &self.csv() }.write(&mut r);
                Ok(Some(r))
            }
            _ => Err(CommandError::NoSuchId { id }),
        }
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = ctx.clock.clone();
    }
    fn results(&self) -> Published {
        Published::new(<ScadaFields<'static> as Fields>::SHAPE.fields)
            .with_tables(&[&SCADA_MESSAGES, &SCADA_DEVICES])
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            crate::typed_format::<MessageItem>(input),
            crate::typed_format::<DeviceItem>(input),
        ])
    }
    fn process(&mut self, x: &[Timed<ScadaHeard>], _first: u64, out: &mut Outbox) {
        for it in x {
            self.started = true;
            self.now = self.now.max(it.time);
            match &it.value {
                ScadaHeard::Status(s) => self.status = *s,
                ScadaHeard::Frame(f) => self.frame(it.time, f),
                ScadaHeard::Unknown(u) => self.unknown(it.time, u),
            }
        }
        out.items::<MessageItem>(0).append(&mut self.messages_out);
        out.items::<DeviceItem>(1).append(&mut self.devices_out);
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.now;
        let recent = self.last_message.is_some_and(|t| now - t <= RECENT_S);
        let c = &self.held.counters;
        p.put(&ScadaFields {
            locked: self.status.locked,
            link: self.held.link.as_deref(),
            messages: self.held.messages.len() as i64,
            modbus: c.modbus as i64,
            dnp3: c.dnp3 as i64,
            unknown: c.unknown as i64,
            crc_failures: c.crc_failures as i64,
            encrypted: c.encrypted as i64,
            not_recognised: c.not_recognised as i64,
            devices: self.held.devices.len() as i64,
            offset_hz: self.offset_hz.map(|v| Hertz(f64::from(v))),
            deviation_hz: self.deviation_hz.map(|v| Hertz(f64::from(v))),
            level_dbfs: self.started.then_some(f64::from(self.status.level_dbfs)),
            note: NOT_CONFIRMED,
        });
        if self.cleared {
            self.cleared = false;
            p.results_mut().clear_log();
        }
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.held.messages, &SCADA_MESSAGES, now, p);
            shaped::rows_of(
                self.held.devices.iter(),
                |d, w| w.write_str(&d.key()),
                &SCADA_DEVICES,
                now,
                p,
            );
            for line in self.pending_log.drain(..) {
                p.results_mut().log(line);
            }
        }
        let lock_state = if !self.started {
            LockState::Off
        } else if self.status.locked || recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let level = self.started.then_some(self.status.level_dbfs);
        let n = self.held.messages.len();
        let r = p.results_mut();
        if !self.started {
            crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("SCADA: waiting for samples"),
            );
        } else if n > 0 {
            crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("SCADA: {n} messages, {} devices", self.held.devices.len()),
            );
        } else {
            crate::set_status(
                r,
                lock_state,
                level,
                &mut self.summary,
                format_args!("SCADA: listening"),
            );
        }
    }
    fn clear(&mut self) {
        self.held.clear();
        self.last_message = None;
        self.modbus_wait.clear();
        self.app_wait.clear();
        self.confirm_wait.clear();
        self.link_wait.clear();
        self.segments.clear();
        self.pending_log.clear();
        self.cleared = true;
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.now)?;
        self.pending_log.clear();
        self.cleared = true;
        self.dirty = true;
        Ok(())
    }
}

/// The settings of the `scada` stack: the test signal settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(Schema::of(gen::SETTINGS)))
        .clone()
}

/// The receiver: the modem and the message decoder, with the `clear` and `export_csv` commands.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("modem", DecoderComponent::new(ScadaModem::default()))
        .add("messages", DecoderComponent::new(ScadaMessages::default()))
        .input("modem")
        .connect("modem", "messages")
        .output("messages.messages")
        .output("messages.devices")
        .status_from("messages")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE, "SCADA needs a sample rate of at least 96 ksps")
        .command("clear", CommandTarget::every("clear"))
        .command("export_csv", CommandTarget::of("messages", "export_csv"))
        .with_settings(&cfg.settings)
        .build()
}

/// Makes the receiver.
///
/// # Errors
///
/// As [`receiver`].
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// Makes the looping test signal, the channel where the receiver looks.
///
/// # Errors
///
/// [`ConfigError`] if the rate cannot hold the signal.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::ScadaGenerator::from_config(
        cfg,
        crate::SCADA.tuning.tune_offset_hz,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame::{ascii_frame, dnp3_frame, rtu_frame};
    use modem::LinkText;

    fn link() -> LinkText {
        LinkText {
            short: "Bell 202 AFSK 1200 Bd, 8E1".into(),
            long: "Bell 202 AFSK 1200 Bd, 8E1, tones 1200 / 2200 Hz".into(),
            offset_hz: 0.0,
            deviation_hz: 3000.0,
        }
    }

    fn rtu(m: &mut ScadaMessages, t: f64, bytes: &[u8]) {
        m.frame(
            t,
            &RxFrame {
                link: link(),
                protocol: Some(Protocol::ModbusRtu),
                bytes: rtu_frame(bytes),
                crc_ok: true,
                note: None,
                quality_db: 20.0,
            },
        );
    }

    fn dnp(m: &mut ScadaMessages, t: f64, frame: Vec<u8>) {
        let (_, bad) = frame::dnp3_user_data(&frame);
        m.frame(
            t,
            &RxFrame {
                link: link(),
                protocol: Some(Protocol::Dnp3),
                bytes: frame,
                crc_ok: bad.is_none(),
                note: bad.map(|n| format!("CRC failed in data block {n}")),
                quality_db: 20.0,
            },
        );
    }

    fn last(m: &ScadaMessages) -> &ScadaMessage {
        m.held.messages.iter().last().unwrap()
    }

    fn device<'a>(m: &'a ScadaMessages, p: &str, a: u16) -> &'a Device {
        m.held
            .devices
            .iter()
            .find(|d| d.protocol == p && d.address == a)
            .unwrap()
    }

    #[test]
    fn modbus_reads_writes_exceptions_and_unanswered_polls() {
        let mut m = ScadaMessages::default();
        rtu(&mut m, 1.0, &[0x11, 3, 0, 0x6B, 0, 3]);
        rtu(&mut m, 1.1, &[0x11, 3, 6, 2, 0x2B, 0, 0, 0, 0x64]);
        let r = last(&m);
        assert_eq!((r.from.as_str(), r.to.as_str()), ("slave 17", "master"));
        assert_eq!(r.pair, Some(1));
        assert!((r.response_ms.unwrap() - 100.0).abs() < 1e-6);
        assert_eq!(m.held.messages.iter().next().unwrap().pair, Some(2));
        let d = device(&m, "Modbus", 17);
        assert_eq!((d.polls, d.replies), (1, 1));
        assert_eq!(d.points[0], ("40108".to_string(), "555".to_string()));
        // An exception pairs with its request.
        rtu(&mut m, 2.0, &[1, 3, 0, 0, 0, 10]);
        rtu(&mut m, 2.05, &[1, 0x83, 2]);
        assert_eq!(last(&m).summary, "exception 2: illegal data address");
        assert_eq!(last(&m).pair, Some(3));
        assert_eq!(device(&m, "Modbus", 1).exceptions, 1);
        // A write of one register and its echo.
        rtu(&mut m, 3.0, &[0x11, 6, 0, 1, 0, 3]);
        rtu(&mut m, 3.02, &[0x11, 6, 0, 1, 0, 3]);
        assert_eq!(last(&m).direction, Some(Direction::Reply));
        assert_eq!(last(&m).summary, "echo: register 40002 = 3");
        // Diagnostics: the echo of sub-function 0.
        rtu(&mut m, 4.0, &[0x11, 8, 0, 0, 0xA5, 0x37]);
        rtu(&mut m, 4.01, &[0x11, 8, 0, 0, 0xA5, 0x37]);
        assert_eq!(last(&m).summary, "echo: sub-function 0, data A5 37");
        // An 8-byte coil reply (3 bytes of coils) pairs as a reply.
        rtu(&mut m, 5.0, &[0x11, 1, 0, 0x13, 0, 0x13]);
        rtu(&mut m, 5.02, &[0x11, 1, 3, 0xCD, 0x6B, 0x05]);
        assert_eq!(last(&m).direction, Some(Direction::Reply));
        assert!(last(&m)
            .summary
            .starts_with("3 bytes: 00020 = 1, 00021 = 0"));
        // Two polls with no reply: the first counts as unanswered at once, the second after 5 s.
        rtu(&mut m, 6.0, &[0x22, 3, 0, 0, 0, 1]);
        rtu(&mut m, 6.5, &[0x22, 4, 0, 0, 0, 1]);
        assert_eq!(device(&m, "Modbus", 0x22).unanswered, 1);
        rtu(&mut m, 12.0, &[0x11, 3, 0, 0x6B, 0, 1]);
        assert_eq!(device(&m, "Modbus", 0x22).unanswered, 2);
        // A write of several registers.
        rtu(&mut m, 13.0, &[0x11, 16, 0, 1, 0, 2, 4, 0, 10, 1, 2]);
        assert_eq!(
            last(&m).summary,
            "registers from 40002, quantity 2: 10, 258"
        );
        // Modbus ASCII: the same meaning, the LRC as its check.
        let a = ascii_frame(&[0x11, 3, 0, 0x6B, 0, 3]);
        let mut found = Vec::new();
        frame::find_ascii(&a, &mut found);
        m.frame(
            14.0,
            &RxFrame {
                link: link(),
                protocol: Some(Protocol::ModbusAscii),
                bytes: found[0].bytes.clone(),
                crc_ok: true,
                note: None,
                quality_db: 20.0,
            },
        );
        assert_eq!(last(&m).protocol, "Modbus ASCII");
        assert_eq!(last(&m).summary, "start 107 (register 40108), quantity 3");
    }

    #[test]
    fn dnp3_link_application_points_and_pairing() {
        let mut m = ScadaMessages::default();
        // Request link status and its link status.
        dnp(&mut m, 1.0, dnp3_frame(0xC9, 1, 3, &[]));
        dnp(&mut m, 1.05, dnp3_frame(0x0B, 3, 1, &[]));
        let r = last(&m);
        assert_eq!(r.function, "link status");
        assert_eq!(
            (r.from.as_str(), r.to.as_str()),
            ("outstation 1", "master 3")
        );
        assert_eq!(r.pair, Some(1));
        // The class polls of worked frame 6, then a response with indications and points.
        dnp(
            &mut m,
            2.0,
            dnp3_frame(
                0xC4,
                1,
                3,
                &[
                    0xC0, 0xC1, 1, 0x3C, 2, 6, 0x3C, 3, 6, 0x3C, 4, 6, 0x3C, 1, 6,
                ],
            ),
        );
        assert_eq!(
            last(&m).summary,
            "g60v2 all, g60v3 all, g60v4 all, g60v1 all: class 1, 2, 3 poll and integrity poll"
        );
        let mut resp = vec![0xC0, 0xE1, 129, 0x82, 0x00];
        resp.extend_from_slice(&[30, 1, 0x00, 0, 0, 0x01, 0xE8, 0x03, 0, 0]);
        dnp(&mut m, 2.2, dnp3_frame(0x44, 3, 1, &resp));
        let r = last(&m);
        assert_eq!(r.function, "response");
        assert_eq!(r.pair, Some(3));
        assert!(
            r.summary.contains("IIN class 1 events, device restart"),
            "{}",
            r.summary
        );
        assert!(r.summary.contains("AI 0 = 1000"), "{}", r.summary);
        let d = device(&m, "DNP3", 1);
        assert_eq!(d.role, Role::Outstation);
        assert_eq!(
            d.last_iin.as_deref(),
            Some("class 1 events, device restart")
        );
        assert_eq!(d.points[0], ("AI 0".to_string(), "1000".to_string()));
        // The confirm that the response asked for (CON) pairs with it.
        dnp(&mut m, 2.3, dnp3_frame(0xC4, 1, 3, &[0xC0, 0xC1, 0]));
        assert_eq!(last(&m).function, "confirm");
        assert_eq!(last(&m).pair, Some(4));
        // An unsolicited response with CON and its confirm.
        dnp(
            &mut m,
            3.0,
            dnp3_frame(0x44, 3, 1, &[0xC0, 0xF2, 130, 0, 0]),
        );
        assert_eq!(last(&m).direction, Some(Direction::Unsolicited));
        dnp(&mut m, 3.1, dnp3_frame(0xC4, 1, 3, &[0xC0, 0xC2, 0]));
        assert_eq!(last(&m).pair, Some(6));
        assert_eq!(device(&m, "DNP3", 1).unsolicited, 1);
        // A time sync: a write of g50v1.
        let ms: u64 = 1_794_134_527_250;
        let mut w = vec![0xC0, 0xC3, 2, 50, 1, 0x07, 1];
        w.extend_from_slice(&ms.to_le_bytes()[..6]);
        dnp(&mut m, 4.0, dnp3_frame(0xC4, 1, 3, &w));
        assert_eq!(
            last(&m).summary,
            "g50v1 count 1; time sync 2026-11-08T10:42:07.250Z"
        );
        // A response in two segments.
        let mut long = vec![0xC4, 129, 0, 0, 30, 2, 0x00, 0, 9];
        for k in 0..10u8 {
            long.extend_from_slice(&[0x01, k, 0]);
        }
        dnp(
            &mut m,
            5.0,
            dnp3_frame(0x44, 3, 1, &[&[0x40u8][..], &long[..20]].concat()),
        );
        assert!(last(&m).summary.contains("the fragment continues"));
        dnp(
            &mut m,
            5.1,
            dnp3_frame(0x44, 3, 1, &[&[0x81u8][..], &long[20..]].concat()),
        );
        assert!(
            last(&m).summary.contains("AI 9 = 9"),
            "{}",
            last(&m).summary
        );
        // A lost first segment.
        dnp(
            &mut m,
            6.0,
            dnp3_frame(0x44, 3, 1, &[0x85, 0xC0, 129, 0, 0]),
        );
        assert_eq!(
            last(&m).summary,
            "segment 5 came without the first segment; fragment dropped"
        );
        // Secure Authentication is shown raw.
        dnp(
            &mut m,
            7.0,
            dnp3_frame(0xC4, 1, 3, &[0xC0, 0xC4, 32, 120, 1, 0x5B, 1, 0]),
        );
        assert!(last(&m).encrypted);
        assert_eq!(m.held.counters.encrypted, 1);
        // A damaged data block.
        let mut f = dnp3_frame(0xC4, 1, 3, &[0xC0, 0xC5, 1, 0x3C, 1, 6]);
        f[12] ^= 0x10;
        dnp(&mut m, 8.0, f);
        assert_eq!(last(&m).crc, Some(false));
        assert_eq!(last(&m).summary, "CRC failed in data block 1");
        assert_eq!(device(&m, "DNP3", 3).crc_failures, 1);
    }

    #[test]
    fn the_csv_export() {
        let mut m = ScadaMessages::default();
        rtu(&mut m, 1.0, &[0x11, 3, 0, 0x6B, 0, 3]);
        rtu(&mut m, 1.1, &[0x11, 3, 6, 2, 0x2B, 0, 0, 0, 0x64]);
        let csv = m.csv();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], CSV_HEADER);
        assert_eq!(
            lines[1],
            "1,1.000,,\"Bell 202 AFSK 1200 Bd, 8E1\",Modbus RTU,master,slave 17,read holding registers,\"start 107 (register 40108), quantity 3\",good,request,2,,false,11 03 00 6B 00 03 76 87"
        );
        assert!(lines[2].starts_with("2,1.100,,"));
        assert!(lines[2].contains(",reply,1,100,"));
    }
}
