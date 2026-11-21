//! What an APRS frame does: the frame log, the station table (objects and items under their names, third-party packets through
//! their gateway), digipeated copies, the messages (received and own, in the shared message log), and the answers and
//! messages to the station that the amateur client acts on. The formats of own messages and beacons are here; the client
//! sends them.
//!
//! Standard: APRS 1.0.1 chapter 14 (messages), AX.25 2.2 (frames).

use super::info::{fmt_lat, fmt_lon, parse_info, AprsInfo};
use super::rx::RawFrame;
use crate::mesh::ham_client::{HamHeard, OwnStation};
use crate::mesh::store::MessageLog;
use crate::mesh::types::{message_text, Delivery, MeshText, Protocol};
use crate::typed::{AprsFrameRow, AprsPacketEvent, AprsStationRow, APRS_PACKET};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys;
use sdrkit_core::Event;
use sdrkit_proto_link::aprs::{encode_uncompressed, Symbol};
use sdrkit_proto_link::ax25::Frame;
use sdrkit_util_geo::Position;
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{RowOut, Shaped};
use sdrkit_util_objects::{
    fingerprint, Dedup, Format, LogRing, ObjectTable, Seen, Shift, Stamp, Tables,
};
use serde::{Deserialize, Serialize};

/// Frames kept.
pub const FRAMES_CAP: usize = 200;
/// Stations kept.
pub const STATIONS_CAP: usize = 300;
/// A digipeated copy is the same frame within this time, seconds.
pub const COPY_WINDOW_S: f64 = 30.0;

/// One AX.25 frame with its APRS content (`aprs.frame`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ax25Frame {
    /// Signal time.
    pub time: f64,
    /// From.
    pub from: String,
    /// To.
    pub to: String,
    /// Path.
    pub path: String,
    /// The APRS kind, or the AX.25 frame type.
    pub ptype: String,
    /// Summary.
    pub summary: String,
    /// Information as received.
    pub info: String,
    /// 1200 or 9600.
    pub speed: u32,
    /// Position.
    pub position: Option<(f64, f64)>,
    /// TNC2 text.
    pub tnc2: String,
    /// The decoded APRS content.
    pub aprs: Option<AprsInfo>,
    /// A digipeated copy.
    pub copy: bool,
    /// The frame bytes without the FCS (for a KISS server).
    pub bytes: Vec<u8>,
}

impl sdrkit_graph::Item for Ax25Frame {
    const KIND: &'static str = "aprs.frame";
}

impl Shift for Ax25Frame {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for Ax25Frame {
    type Row = AprsFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AprsFrameRow {
            time_s: self.time,
            source: &self.from,
            target: &self.to,
            path: &self.path,
            r#type: &self.ptype,
            text: &self.summary,
            bit_rate: Hertz(f64::from(self.speed)),
        });
    }
}

/// A station or object record (`aprs.station`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AprsStation {
    /// Call with SSID, or the object's name.
    pub call: String,
    /// Symbol table and code.
    pub symbol: String,
    /// Position (latitude, longitude).
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
    /// The station that sent it (objects, items and third-party packets).
    pub via: String,
}

impl sdrkit_graph::Item for AprsStation {
    const KIND: &'static str = "aprs.station";
}

impl Shift for AprsStation {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for AprsStation {
    type Row = AprsStationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AprsStationRow {
            call: &self.call,
            symbol: &self.symbol,
            position: self.position,
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
            course_deg: self.course.map(f64::from),
            speed_kt: self.speed_kn,
            alt_m: self.alt_m,
            comment: &self.comment,
            via: &self.via,
        });
    }
    fn point(&self) -> Option<Point<'_>> {
        let (lat, lon) = self.position?;
        let p = Point::new(
            Position::new(lat, lon),
            format!("{} {}", self.call, self.symbol),
        );
        Some(match self.course {
            Some(c) => p.heading(c as f32),
            None => p,
        })
    }
}

/// The row of the APRS message `i` from `from`, heard at signal time `time` on `protocol` (LoRa APRS or APRS on AX.25),
/// and its key in a [`MessageLog`]: the same sender, number, addressee and text are one message. `None` for an
/// acknowledgement, a rejection, and any frame that is not a message with a text.
pub fn message_row(
    protocol: Protocol,
    from: &str,
    i: &AprsInfo,
    time: f64,
) -> Option<(MeshText, String)> {
    if i.kind != "Message" || i.is_ack || i.is_rej {
        return None;
    }
    let text = i.text.as_deref()?;
    let to = i.addressee.clone().unwrap_or_default();
    let no = i.msg_no.as_deref().unwrap_or_default();
    let key = format!("aprs/{from}/{no}/{to}/{text}");
    let m = MeshText {
        time,
        protocol,
        channel: protocol.name().into(),
        from: from.into(),
        from_name: from.into(),
        to,
        text: message_text(text),
        hops: -1,
        packet_id: no.parse().unwrap_or(0),
        ..Default::default()
    };
    Some((m, key))
}

/// The position report `!DDMM.hhN<table>DDDMM.hhE<code><comment>`. `None` if the position is out of range.
pub fn position_text(p: &Position, symbol: &str, comment: &str) -> Option<String> {
    let mut sym = symbol.chars();
    let symbol = Symbol {
        table: sym.next().unwrap_or('/'),
        code: sym.next().unwrap_or('-'),
    };
    let mut s = String::from("!");
    encode_uncompressed(p, symbol, 0, &mut s).ok()?;
    s.push_str(comment);
    Some(s)
}

/// The information field of a beacon: the position report when `p` is given, else the status `>comment`. `None` if the
/// position is out of range.
pub fn beacon_info(p: Option<&Position>, symbol: &str, comment: &str) -> Option<String> {
    match p {
        Some(p) => position_text(p, symbol, comment),
        None => Some(format!(">{comment}")),
    }
}

/// The information field of message number `number` to `to`: `:ADDRESSEE:text{number`, the addressee padded to nine
/// characters.
pub fn message_info(to: &str, text: &str, number: u32) -> String {
    format!(":{to:<9}:{text}{{{number}")
}

/// The longest text of an APRS message, characters.
pub const MESSAGE_TEXT_MAX: usize = 67;

/// The first character that an APRS message text must not hold ("|", "~" or "{"), as its byte position.
pub fn message_text_fault(text: &str) -> Option<usize> {
    text.find(['|', '~', '{'])
}

/// Everything heard: the frames, the stations and the memory of digipeated copies.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heard {
    /// Frames, oldest first.
    pub frames: LogRing<Ax25Frame>,
    /// Stations, objects and items by call or name.
    pub stations: ObjectTable<String, AprsStation>,
    /// APRS frames.
    pub aprs_frames: u64,
    /// Digipeated copies.
    pub copies_count: u64,
    /// Frames that are not AX.25.
    pub bad_parse: u64,
    /// Messages, received and own.
    pub messages: MessageLog,
    #[serde(skip, default = "copies")]
    copies: Dedup<u64>,
}

fn copies() -> Dedup<u64> {
    Dedup::new(COPY_WINDOW_S, 1000)
}

impl Default for Heard {
    fn default() -> Self {
        Heard {
            frames: LogRing::new(FRAMES_CAP),
            stations: ObjectTable::new(STATIONS_CAP),
            aprs_frames: 0,
            copies_count: 0,
            bad_parse: 0,
            messages: MessageLog::default(),
            copies: copies(),
        }
    }
}

impl Shift for Heard {
    fn shift_times(&mut self, by_s: f64) {
        self.frames.shift_times(by_s);
        self.stations.shift_times(by_s);
        self.messages.shift_times(by_s);
    }
}

impl Tables for Heard {
    const FORMAT: Format = Format::new("lora.aprs_heard", 2);
    fn clear(&mut self) {
        self.frames.clear();
        self.stations.clear();
        self.messages.clear();
        self.copies.clear();
        self.aprs_frames = 0;
        self.copies_count = 0;
        self.bad_parse = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.frames.adopt(saved.frames);
        self.stations.adopt(saved.stations);
        self.messages.adopt(saved.messages);
        self.aprs_frames = saved.aprs_frames;
        self.copies_count = saved.copies_count;
        self.bad_parse = saved.bad_parse;
    }
}

/// The tables of the APRS stack.
#[derive(Debug)]
pub struct AprsTables {
    /// What was heard.
    pub heard: Heard,
    /// The source call of own frames, from the client; empty without a callsign.
    pub own_call: String,
    /// Messages to the station must match the SSID of its call.
    pub match_ssid: bool,
    /// The frequency the stack listens to, Hz.
    pub freq_hz: f64,
    /// The SNR (dB) and the level (dBFS) of the latest signal report, for the rows of received messages.
    pub signal: Option<(f64, f64)>,
    /// Log lines waiting for the component.
    pub logs: Vec<String>,
}

impl Default for AprsTables {
    fn default() -> Self {
        AprsTables {
            heard: Heard::default(),
            own_call: String::new(),
            match_ssid: false,
            freq_hz: 144.8e6,
            signal: None,
            logs: Vec::new(),
        }
    }
}

fn apply(s: &mut AprsStation, i: &AprsInfo) {
    if let (Some(la), Some(lo)) = (i.lat, i.lon) {
        s.position = Some((la, lo));
        s.alt_m = i.alt_m.or(s.alt_m);
        s.course = i.course.or(s.course);
        s.speed_kn = i.speed_kn.or(s.speed_kn);
        if let (Some(t), Some(c)) = (i.symbol_table, i.symbol_code) {
            s.symbol = format!("{t}{c}");
        }
        if !i.comment.is_empty() {
            s.comment = i.comment.clone();
        }
    }
    if i.kind == "Status" {
        s.comment = i.comment.clone();
    }
}

fn new_station(call: &str) -> impl FnOnce() -> AprsStation + '_ {
    move || AprsStation {
        call: call.into(),
        ..Default::default()
    }
}

/// The call without its SSID.
fn base_call(c: &str) -> &str {
    c.split('-').next().unwrap_or(c)
}

/// Returns true if `addressee` is the station `own` (its source call, empty for none): the whole call with
/// `match_ssid`, else the call without the SSID.
fn for_me(own: &str, match_ssid: bool, addressee: &str) -> bool {
    let me = own.trim();
    if me.is_empty() {
        return false;
    }
    if match_ssid {
        addressee.trim().eq_ignore_ascii_case(me)
    } else {
        base_call(addressee.trim()).eq_ignore_ascii_case(base_call(me))
    }
}

/// What the message information `i` from `from`, heard at `time`, asks of the station `own`: an answer to an own message,
/// or a numbered message that wants an acknowledgement. `None` for any other frame, for one to another station and for the
/// station's own frames heard back. The caller passes only frames with a good checksum.
pub fn to_station(
    own: &str,
    match_ssid: bool,
    from: &str,
    i: &AprsInfo,
    time: f64,
) -> Option<HamHeard> {
    if i.kind != "Message" || from.eq_ignore_ascii_case(own.trim()) {
        return None;
    }
    if !for_me(own, match_ssid, i.addressee.as_deref()?) {
        return None;
    }
    if i.is_ack || i.is_rej {
        let number = i.msg_no.as_deref()?.trim().parse().ok()?;
        let state = if i.is_ack {
            Delivery::Delivered
        } else {
            Delivery::Rejected
        };
        return Some(HamHeard::Answer {
            from: from.into(),
            number,
            state,
        });
    }
    Some(HamHeard::Message {
        time,
        from: from.into(),
        number: i.msg_no.as_deref()?.trim().into(),
    })
}

impl AprsTables {
    /// A received frame. Returns its row, adds what the client acts on (answers to own messages, messages to the station)
    /// to `to_client` and its event to `events`.
    pub fn frame(
        &mut self,
        raw: &RawFrame,
        to_client: &mut Vec<HamHeard>,
        events: &mut Vec<Event>,
    ) -> Option<Ax25Frame> {
        let Ok(f) = Frame::parse(&raw.bytes) else {
            self.heard.bad_parse += 1;
            return None;
        };
        let time = raw.time;
        let from = f.source().to_string();
        let to = f.destination().to_string();
        let path = f
            .repeaters()
            .iter()
            .map(|r| format!("{r}{}", if r.flag { "*" } else { "" }))
            .collect::<Vec<_>>()
            .join(",");
        let info_text = String::from_utf8_lossy(f.info).into_owned();
        let kind = f.control.name();
        let mut row = Ax25Frame {
            time,
            from: from.clone(),
            to: to.clone(),
            path: path.clone(),
            ptype: kind.into(),
            info: info_text.clone(),
            speed: raw.speed,
            tnc2: f.to_string(),
            bytes: raw.bytes.clone(),
            ..Default::default()
        };
        // Digipeated copies: the same source, destination and information within the copy window.
        let mut key = Vec::with_capacity(from.len() + to.len() + f.info.len() + 2);
        key.extend_from_slice(from.as_bytes());
        key.push(b'>');
        key.extend_from_slice(to.as_bytes());
        key.push(b':');
        key.extend_from_slice(f.info);
        let copy = self.heard.copies.seen(fingerprint(&key), time);
        row.copy = copy;
        let is_aprs = kind == "UI" && f.pid == Some(0xF0);
        if is_aprs {
            match parse_info(f.info, f.destination().call.as_str()) {
                Ok(i) => {
                    row.ptype = i.kind.clone();
                    row.summary = i.summary.clone();
                    if let (Some(la), Some(lo)) = (i.lat, i.lon) {
                        row.position = Some((la, lo));
                    }
                    // A received message has its row; an own message heard back keeps its own row only.
                    let own = from.eq_ignore_ascii_case(self.own_call.trim());
                    if let Some((mut m, key)) =
                        message_row(Protocol::Aprs, &from, &i, time).filter(|_| !own)
                    {
                        m.snr_db = self.signal.map(|s| s.0);
                        m.level_dbfs = self.signal.map(|s| s.1);
                        self.heard.messages.add(m, &key, 0.0);
                    }
                    if !copy {
                        self.heard.aprs_frames += 1;
                        self.station(&from, &i, time, "");
                        if let (Some(h), Some(inner)) = (&i.third_party, &i.inner) {
                            self.third_party(h, inner, &from, time, 0);
                        }
                        to_client.extend(to_station(
                            &self.own_call,
                            self.match_ssid,
                            &from,
                            &i,
                            time,
                        ));
                    } else {
                        self.heard.copies_count += 1;
                        let _ =
                            self.heard
                                .stations
                                .upsert(&from, Stamp::at(time), new_station(&from));
                    }
                    row.aprs = Some(i);
                }
                Err(e) => {
                    row.summary = format!("AX.25 {kind}: {info_text} ({e})");
                }
            }
        } else {
            row.summary = format!("AX.25 {kind}: {info_text}");
        }
        if copy {
            row.summary = format!(
                "(again, via {}) {}",
                if path.is_empty() { "direct" } else { &path },
                row.summary
            );
        }
        events.push(APRS_PACKET.event(
            time,
            self.freq_hz,
            &AprsPacketEvent {
                source: &from,
                target: &to,
                path: &path,
                info: &info_text,
                r#type: &row.ptype,
                text: &row.summary,
                position: row.position,
                tnc2: &row.tnc2,
                family: keys::Family::Aprs,
            },
        ));
        self.heard.frames.push(row.clone());
        Some(row)
    }

    fn station(&mut self, from: &str, i: &AprsInfo, time: f64, via: &str) {
        let at = Stamp::at(time);
        let new = !self.heard.stations.contains(from);
        if let Ok(up) = self
            .heard
            .stations
            .upsert(&from.to_string(), at, new_station(from))
        {
            if !via.is_empty() {
                up.row.via = via.into();
            }
            if !matches!(i.kind.as_str(), "Object" | "Item") || i.name.is_none() {
                apply(up.row, i);
            }
        }
        if new {
            self.logs.push(format!("APRS: new station {from}"));
        }
        if let (Some(name), true) = (&i.name, matches!(i.kind.as_str(), "Object" | "Item")) {
            if let Ok(up) = self.heard.stations.upsert(name, at, new_station(name)) {
                up.row.object = true;
                up.row.via = from.into();
                apply(up.row, i);
            }
        }
    }

    fn third_party(
        &mut self,
        header: &str,
        inner: &AprsInfo,
        gateway: &str,
        time: f64,
        depth: u32,
    ) {
        let Some((src, _)) = header.split_once('>') else {
            return;
        };
        self.station(src, inner, time, gateway);
        if let (Some(h), Some(next)) = (&inner.third_party, &inner.inner) {
            if depth < 3 {
                self.third_party(h, next, gateway, time, depth + 1);
            }
        }
    }

    /// An own message, new or with a new state ([`MessageLog::upsert_own`]).
    pub fn own_message(&mut self, m: MeshText) {
        self.heard.messages.upsert_own(m, 0.0);
    }

    /// The own station after its beacon went on the air.
    pub fn own_station(&mut self, s: &OwnStation) {
        if let Ok(up) = self
            .heard
            .stations
            .upsert(&s.call, Stamp::at(s.time), new_station(&s.call))
        {
            up.row.symbol.clone_from(&s.symbol);
            if s.position.is_some() {
                up.row.position = s.position;
            }
            up.row.comment.clone_from(&s.comment);
        }
    }

    /// The position text of a station row.
    pub fn position_line(s: &AprsStation) -> String {
        s.position
            .map(|(a, b)| format!("{} {}", fmt_lat(a), fmt_lon(b)))
            .unwrap_or_default()
    }
}
