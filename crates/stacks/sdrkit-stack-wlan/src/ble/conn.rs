//! Connections followed from their CONNECT_IND or AUX_CONNECT_REQ (Bluetooth Core Specification v5.4 Vol 6 Part B clauses
//! 2.3.3.1, 4.5.3, 4.5.8 and 5.1): the connection's access address, its timing and its channel map give the data channel
//! of each connection event, by channel selection algorithm #1 or #2. The capture's receivers open the channel of each event
//! that lies inside the capture. The first packet of an event is the central's and sets the anchor; the packets after it in
//! the event are the peripheral's and the central's answers.
//!
//! The LL control PDUs that change the connection (connection update, channel map, PHY update, terminate) are applied at
//! their instant while the connection is in the clear. Once LL_START_ENC_REQ is heard the payloads are ciphertext: the
//! connection is followed with the parameters it has, and stays followed until it ends or its supervision timeout passes
//! with no packet heard.

use super::Advert;
use crate::typed::{BleAttRow, BleConnectionRow};
use sdrkit_core::interface::Time;
use sdrkit_proto_wlan_phy::ble::csa::{self, ChannelMap, Csa1};
use sdrkit_proto_wlan_phy::ble::ll::{self, Change, DataHeader, Llid};
use sdrkit_proto_wlan_phy::ble::{Addr, AdvPdu, Body, LlData, Phy};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{RowOut, Shaped};
use sdrkit_util_objects::{LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};

/// The most connections kept in the table.
pub const CONN_CAP: usize = 64;
/// The most lines of the ATT log.
pub const ATT_CAP: usize = 400;
/// 1.25 ms, the unit of the connection's timing, s.
pub const UNIT_S: f64 = 1.25e-3;
/// How far after the anchor the first packet of an event may start (window widening for both sleep clocks), s. It is less
/// than the shortest answer of the peripheral after it (an empty PDU and T_IFS: 230 µs).
const ANCHOR_SLACK_S: f64 = 0.1e-3;
/// How long a channel stays open after the start of an event: several packets of an event, s.
const EVENT_OPEN_S: f64 = 6e-3;

/// Which channel selection algorithm a connection uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Csa {
    /// Algorithm #1.
    One,
    /// Algorithm #2.
    Two,
    /// Not known yet: the CONNECT_IND allows #2, the advertiser's support was not heard. Both channels are opened until a
    /// packet tells.
    Either,
}

/// A change waiting for its instant.
#[derive(Clone, Copy, Debug)]
struct Pending {
    instant: u16,
    change: Change,
}

/// A followed connection.
#[derive(Clone, Debug)]
pub struct Conn {
    /// The connection's access address and CRC preset.
    pub aa: u32,
    /// The CRC preset.
    pub crc_init: u32,
    /// The initiator (central) and the advertiser (peripheral).
    pub central: Addr,
    /// The advertiser.
    pub peripheral: Addr,
    /// The connection interval, s.
    pub interval_s: f64,
    /// The peripheral latency, events.
    pub latency: u16,
    /// The supervision timeout, s.
    pub timeout_s: f64,
    /// The channel map.
    pub map: ChannelMap,
    /// The hop increment (algorithm #1).
    pub hop: u8,
    /// The channel selection algorithm.
    pub csa: Csa,
    /// The PHY of the connection.
    pub phy: Phy,
    /// The start of the transmit window of event `counter` while no anchor is known, or the anchor of event `counter`, s.
    pub at_s: f64,
    /// The transmit window, s (0 once an anchor is known).
    pub window_s: f64,
    /// The event counter of `at_s`.
    pub counter: u16,
    /// The time of the last packet heard (or of the CONNECT_IND), s.
    pub last_s: f64,
    /// LL_START_ENC_REQ was heard: the payloads are ciphertext.
    pub encrypted: bool,
    /// LL_TERMINATE_IND was heard.
    pub ended: bool,
    pending: Vec<Pending>,
}

/// What a data packet was, as the follower places it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    /// The connection event counter.
    pub event: u16,
    /// The first packet of its event: sent by the central.
    pub central: bool,
    /// The channel selection algorithm (1 or 2), once known.
    pub csa: Option<u8>,
}

impl Conn {
    /// A connection from the LLData of a CONNECT_IND (`aux` false) or AUX_CONNECT_REQ (`aux` true) whose packet ended at
    /// `end_s`, on `phy`. `ch_sel` is the ChSel bit of a CONNECT_IND.
    pub fn new(central: Addr, peripheral: Addr, ll: &LlData, end_s: f64, phy: Phy, aux: bool, ch_sel: bool) -> Option<Conn> {
        let map = ChannelMap::from_bytes(ll.channel_map)?;
        if ll.interval == 0 {
            return None;
        }
        // transmitWindowDelay: 1.25 ms after a CONNECT_IND, 2.5 ms after an AUX_CONNECT_REQ (3.75 ms on LE Coded).
        let delay = match (aux, phy) {
            (false, _) => 1.25e-3,
            (true, Phy::LeCoded) => 3.75e-3,
            (true, _) => 2.5e-3,
        };
        Some(Conn {
            aa: ll.access_address,
            crc_init: ll.crc_init,
            central,
            peripheral,
            interval_s: f64::from(ll.interval) * UNIT_S,
            latency: ll.latency,
            timeout_s: f64::from(ll.timeout) * 10e-3,
            map,
            hop: ll.hop,
            csa: if aux {
                Csa::Two
            } else if ch_sel {
                Csa::Either
            } else {
                Csa::One
            },
            phy: if aux { phy } else { Phy::Le1M },
            at_s: end_s + delay + f64::from(ll.win_offset) * UNIT_S,
            window_s: f64::from(ll.win_size) * UNIT_S,
            counter: 0,
            last_s: end_s,
            encrypted: false,
            ended: false,
            pending: Vec::new(),
        })
    }

    /// The channels event `counter` may be on with the channel map `map`: one, or two while the algorithm is not known.
    pub fn channels(&self, counter: u16, map: &ChannelMap) -> [Option<u8>; 2] {
        let one = Csa1::channel_of(self.hop, counter, map);
        let two = csa::csa2_channel(counter, csa::channel_identifier(self.aa), map);
        match self.csa {
            Csa::One => [Some(one), None],
            Csa::Two => [Some(two), None],
            Csa::Either => [Some(one), (two != one).then_some(two)],
        }
    }

    /// The map in force at event `counter`: a pending channel map applies from its instant.
    fn map_at(&self, counter: u16) -> ChannelMap {
        let mut m = self.map;
        for p in &self.pending {
            if let Change::ChannelMap { map, instant } = p.change {
                if counter.wrapping_sub(instant) < 0x8000 {
                    if let Some(x) = ChannelMap::from_bytes(map) {
                        m = x;
                    }
                }
            }
        }
        m
    }

    /// The (open from, open to, channel) of each event that may start in `from_s..to_s`.
    pub fn events(&self, from_s: f64, to_s: f64, out: &mut Vec<(f64, f64, u8)>) {
        if self.ended || self.interval_s <= 0.0 {
            return;
        }
        let slack = self.window_s + ANCHOR_SLACK_S;
        let first = ((from_s - EVENT_OPEN_S - slack - self.at_s) / self.interval_s)
            .ceil()
            .max(0.0) as u32;
        for k in first..first + 64 {
            let start = self.at_s + f64::from(k) * self.interval_s;
            if start > to_s + ANCHOR_SLACK_S {
                break;
            }
            let counter = self.counter.wrapping_add(k as u16);
            // A connection update moves the events from its instant on: they are found again from the next packet.
            if self.pending.iter().any(|p| {
                matches!(p.change, Change::Update { .. }) && counter.wrapping_sub(p.instant) < 0x8000
            }) {
                break;
            }
            let map = self.map_at(counter);
            for ch in self.channels(counter, &map).into_iter().flatten() {
                out.push((start - ANCHOR_SLACK_S, start + slack + EVENT_OPEN_S, ch));
            }
        }
    }

    /// Places a packet of the connection that started at `start_s` on `channel`, moves the anchor to it when it is the
    /// first of its event, and applies what its payload changes. `None` when it does not fit an event.
    pub fn heard(&mut self, start_s: f64, channel: u8, pdu: &[u8]) -> Option<Placed> {
        let k = ((start_s - self.at_s - self.window_s / 2.0) / self.interval_s).round();
        if k < 0.0 || k > 65_535.0 {
            return None;
        }
        let from_anchor = start_s - self.at_s - k * self.interval_s;
        let counter = self.counter.wrapping_add(k as u16);
        // A packet before the expected anchor is the central's too: the anchor moves to it.
        let first = from_anchor < self.window_s + ANCHOR_SLACK_S;
        if self.csa == Csa::Either {
            let one = Csa1::channel_of(self.hop, counter, &self.map);
            let two = csa::csa2_channel(counter, csa::channel_identifier(self.aa), &self.map);
            if channel == one && channel != two {
                self.csa = Csa::One;
            } else if channel == two && channel != one {
                self.csa = Csa::Two;
            }
        }
        self.last_s = start_s;
        if first {
            self.apply_pending(counter);
            self.at_s = start_s;
            self.window_s = 0.0;
            self.counter = counter;
        }
        self.read(pdu, first);
        Some(Placed {
            event: counter,
            central: first,
            csa: match self.csa {
                Csa::One => Some(1),
                Csa::Two => Some(2),
                Csa::Either => None,
            },
        })
    }

    /// Applies the changes whose instant is `counter` or before.
    fn apply_pending(&mut self, counter: u16) {
        let mut i = 0;
        while i < self.pending.len() {
            let p = self.pending[i];
            if counter.wrapping_sub(p.instant) < 0x8000 {
                match p.change {
                    Change::ChannelMap { map, .. } => {
                        if let Some(m) = ChannelMap::from_bytes(map) {
                            self.map = m;
                        }
                    }
                    Change::Update {
                        win_size,
                        win_offset,
                        interval,
                        latency,
                        timeout,
                        instant,
                    } => {
                        // The event at the instant starts in a new transmit window after the old anchor of the instant.
                        let old = self.at_s + f64::from(instant.wrapping_sub(self.counter)) * self.interval_s;
                        self.at_s = old + f64::from(win_offset) * UNIT_S;
                        self.window_s = f64::from(win_size) * UNIT_S;
                        self.counter = instant;
                        if interval > 0 {
                            self.interval_s = f64::from(interval) * UNIT_S;
                        }
                        self.latency = latency;
                        self.timeout_s = f64::from(timeout) * 10e-3;
                    }
                    Change::Phy {
                        central_to_peripheral,
                        ..
                    } => {
                        self.phy = match central_to_peripheral {
                            1 => Phy::Le1M,
                            2 => Phy::Le2M,
                            4 => Phy::LeCoded,
                            _ => self.phy,
                        };
                    }
                    Change::Terminate(_) => {}
                }
                self.pending.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Reads an LL control PDU sent in the clear: a change waits for its instant, LL_START_ENC_REQ starts the ciphertext.
    fn read(&mut self, pdu: &[u8], from_central: bool) {
        let Some(h) = DataHeader::parse(pdu) else {
            return;
        };
        if self.encrypted || h.llid != Llid::Control {
            return;
        }
        let p = h.payload(pdu);
        match p.first() {
            Some(&ll::opcode::START_ENC_REQ) => self.encrypted = true,
            Some(_) => match ll::change(p) {
                Some(Change::Terminate(_)) => self.ended = true,
                Some(change @ (Change::Update { instant, .. } | Change::ChannelMap { instant, .. } | Change::Phy { instant, .. }))
                    if from_central && self.pending.len() < 4 =>
                {
                    self.pending.push(Pending { instant, change });
                }
                _ => {}
            },
            None => {}
        }
    }

    /// Whether the connection is over at `now_s`: ended, or silent for its supervision timeout (with a floor of 1 s before
    /// any packet is heard).
    pub fn over(&self, now_s: f64) -> bool {
        self.ended || now_s - self.last_s > self.timeout_s.max(1.0) + self.interval_s
    }
}

/// The one-line summary of the data channel PDU `pdu`: the LL control opcode, the ATT opcode of a clear L2CAP start, or the
/// kind and the length.
pub fn summary(pdu: &[u8]) -> String {
    let Some(h) = DataHeader::parse(pdu) else {
        return "data PDU".into();
    };
    let p = h.payload(pdu);
    match h.llid {
        Llid::Control => p.first().map_or("LL control".into(), |&op| ll::control_name(op).into()),
        Llid::Start => match ll::l2cap(p) {
            Some((_, ll::ATT_CID, info)) => info
                .first()
                .map_or("L2CAP ATT".into(), |&op| ll::att_name(op).into()),
            Some((_, cid, _)) => format!("L2CAP channel {cid:#06x}, {} B", h.length),
            None => format!("L2CAP start, {} B", h.length),
        },
        Llid::Continuation if h.length == 0 => "empty PDU".into(),
        Llid::Continuation => format!("L2CAP continuation, {} B", h.length),
    }
}

/// A connection as the table keeps it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ConnRecord {
    /// The access address as eight hex digits (the row key).
    pub aa: String,
    /// The central's address text.
    pub central: String,
    /// The peripheral's address text.
    pub peripheral: String,
    /// The interval, ms.
    pub interval_ms: f64,
    /// The peripheral latency.
    pub latency: u16,
    /// The supervision timeout, ms.
    pub timeout_ms: f64,
    /// The PHY name.
    pub phy: String,
    /// The channel selection algorithm, once known.
    pub csa: Option<u8>,
    /// The data channels of the channel map.
    pub channels_used: u8,
    /// Packets heard.
    pub packets: u64,
    /// LL_ENC_REQ or LL_START_ENC_REQ was heard.
    pub encrypted: bool,
    /// LL_START_ENC_REQ was heard: the payloads after it are ciphertext.
    pub ciphertext: bool,
    /// The LL control opcode names heard, in the order first heard.
    pub procedures: Vec<String>,
    /// The text of the procedures column.
    pub procedures_text: String,
    /// ATT PDUs read.
    pub att: u64,
    /// LL_TERMINATE_IND was heard.
    pub ended: bool,
    /// The GATT services that a discovery on the connection gave: (first handle, last handle, 16-bit UUID).
    pub services: Vec<(u16, u16, u16)>,
}

impl ConnRecord {
    /// The assigned name of the service that holds `handle`, if a discovery gave it.
    fn service_of(&self, handle: u16) -> Option<&'static str> {
        self.services
            .iter()
            .find(|&&(a, b, _)| (a..=b).contains(&handle))
            .and_then(|&(_, _, u)| sdrkit_util_ids::bluetooth::uuid16(u))
            .map(|u| u.name)
    }
}

impl Shift for ConnRecord {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for ConnRecord {
    type Row = BleConnectionRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&BleConnectionRow {
            access_address: &self.aa,
            central: &self.central,
            peripheral: &self.peripheral,
            interval_ms: self.interval_ms,
            latency: i64::from(self.latency),
            timeout_ms: self.timeout_ms,
            phy: &self.phy,
            csa: self.csa.map(i64::from),
            channels_used: i64::from(self.channels_used),
            packets: self.packets as i64,
            encrypted: self.encrypted,
            procedures: &self.procedures_text,
            att: self.att as i64,
            ended: self.ended,
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// One line of the ATT log.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AttLine {
    /// Signal time, s.
    pub t: f64,
    /// The connection's row key.
    pub conn: String,
    /// Sent by the central.
    pub central: bool,
    /// The opcode name.
    pub opcode: String,
    /// The handle.
    pub handle: Option<u16>,
    /// The service name of the handle.
    pub service: Option<String>,
    /// The value as hex.
    pub value: String,
}

impl Shift for AttLine {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

impl Shaped for AttLine {
    type Row = BleAttRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&BleAttRow {
            signal_s: self.t,
            connection: &self.conn,
            from_central: self.central,
            opcode: &self.opcode,
            handle: self.handle.map(i64::from),
            service: self.service.as_deref(),
            value: &self.value,
        });
    }
}

/// The row key of the connection with access address `aa`.
pub fn conn_key(aa: u32) -> String {
    format!("{aa:08X}")
}

/// The PHY name for the table.
fn phy_name(p: Phy) -> String {
    p.name().to_owned()
}

/// Opens the row of the connection that the CONNECT_IND or AUX_CONNECT_REQ `pdu` (heard on `phy`) starts, stamped `at`.
pub fn connect(rows: &mut ObjectTable<String, ConnRecord>, pdu: &AdvPdu<'_>, phy: Phy, aux: bool, at: Stamp) {
    let Body::Connect { init_a, adv_a, ll } = pdu.body else {
        return;
    };
    let used = ChannelMap::from_bytes(ll.channel_map).map_or(0, |m| m.used().len() as u8);
    let key = conn_key(ll.access_address);
    let rec = ConnRecord {
        aa: key.clone(),
        central: init_a.text(),
        peripheral: adv_a.text(),
        interval_ms: f64::from(ll.interval) * UNIT_S * 1e3,
        latency: ll.latency,
        timeout_ms: f64::from(ll.timeout) * 10.0,
        phy: phy_name(if aux { phy } else { Phy::Le1M }),
        csa: (aux || !pdu.header.ch_sel).then_some(if aux { 2 } else { 1 }),
        channels_used: used,
        ..ConnRecord::default()
    };
    if let Ok(up) = rows.upsert(key.as_str(), at, || rec.clone()) {
        if !up.new {
            *up.row = rec;
        }
    }
}

/// Takes a packet `a` of a followed connection, stamped `at`: the counters of its row, the procedures it shows, and the ATT
/// PDU it carries in the clear (into `log`).
pub fn take(rows: &mut ObjectTable<String, ConnRecord>, log: &mut LogRing<AttLine>, a: &Advert, at: Stamp) {
    let key = conn_key(a.access_address);
    if rows.get_mut(key.as_str()).is_none() {
        return;
    }
    let Ok(up) = rows.upsert(key.as_str(), at, ConnRecord::default) else {
        return;
    };
    let r = up.row;
    r.packets += 1;
    let central = a.conn.is_some_and(|p| p.central);
    let Some(h) = DataHeader::parse(&a.pdu) else {
        return;
    };
    let p = h.payload(&a.pdu);
    if r.csa.is_none() {
        // The follower placed the packet on the channel of one algorithm only once it knew which.
        if let Some(c) = a.conn.and_then(|c| c.csa) {
            r.csa = Some(c);
        }
    }
    match h.llid {
        Llid::Control if !r.ciphertext => {
            if let Some(&op) = p.first() {
                let name = ll::control_name(op);
                if !r.procedures.iter().any(|n| n == name) && r.procedures.len() < 24 {
                    r.procedures.push(name.to_owned());
                    r.procedures_text = r.procedures.join(", ");
                }
                if matches!(op, ll::opcode::ENC_REQ | ll::opcode::START_ENC_REQ) {
                    r.encrypted = true;
                }
                if op == ll::opcode::START_ENC_REQ {
                    r.ciphertext = true;
                }
                if op == ll::opcode::TERMINATE_IND {
                    r.ended = true;
                }
                if let Some(Change::Phy {
                    central_to_peripheral,
                    ..
                }) = ll::change(p)
                {
                    r.phy = match central_to_peripheral {
                        2 => phy_name(Phy::Le2M),
                        4 => phy_name(Phy::LeCoded),
                        1 => phy_name(Phy::Le1M),
                        _ => r.phy.clone(),
                    };
                }
            }
        }
        Llid::Start if !r.ciphertext => {
            let Some((_, ll::ATT_CID, info)) = ll::l2cap(p) else {
                return;
            };
            let Some(att) = ll::att(info) else {
                return;
            };
            if att.opcode == 0x11 {
                for g in ll::groups(att.value) {
                    if !r.services.contains(&g) && r.services.len() < 64 {
                        r.services.push(g);
                    }
                }
            }
            r.att += 1;
            let service = att.handle.and_then(|h| r.service_of(h)).map(str::to_owned);
            let n = att.value.len().min(32);
            log.push(AttLine {
                t: a.time_s,
                conn: key,
                central,
                opcode: ll::att_name(att.opcode).to_owned(),
                handle: att.handle,
                service,
                value: crate::report::hex(&att.value[..n]),
            });
        }
        _ => {}
    }
}
