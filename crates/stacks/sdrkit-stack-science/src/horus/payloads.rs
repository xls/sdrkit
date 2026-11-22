//! The payload table of `horus` (`horus.payloads`): the packets of every channel with duplicates left out, the payloads by
//! callsign with their tracks, highest altitude and climb rate, the custom fields of each last v2 packet, the packet log,
//! the events, the map points and the status line. What it holds stays over a change of frequency, and it is the saved
//! state of the stack.

use super::bank::{BankReport, Heard};
use super::demod::Decoded;
use super::packet::{
    custom_fields, format_of, parse_formats, parse_sentence, Binary, CustomFormat, IdList,
};
use super::{Kind, State, CUSTOM_FORMATS, PAYLOAD_IDS};
use crate::family::{lock_state_of, status, wall, TEST_START_UTC};
use crate::typed::{
    HorusCustomFieldRow, HorusLogFields, HorusPacketEvent, HorusPacketRow, HorusPayloadRow,
    HORUS_CUSTOM_FIELDS, HORUS_PACKET, HORUS_PACKETS, HORUS_PAYLOADS,
};
use sdrkit_core::interface::{Fields, Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Clock, CommandError, CommandId, CommandSet, Commands, ConfigError, Event, Record, Results,
    ResultsDecl, SettingError, SettingId, SettingSet, StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind as PortKind, Port, RateFrom,
    RunContext,
};
use sdrkit_util_objects::commands as objcmd;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

/// Payloads kept; the least recently heard goes first.
pub const MAX_PAYLOADS: usize = 64;
/// Packets of the log that the table shows.
pub const MAX_LOG: usize = 200;
/// Packets kept for the saved state and for reading them again with new tables.
pub const MAX_STORED: usize = 10_000;
/// Rows of the custom field table.
pub const MAX_CUSTOM_ROWS: usize = 512;
/// Track points of a payload.
pub const MAX_TRACK: usize = 3000;
/// The same packet within this time is a duplicate, s.
const DUPLICATE_S: f64 = 30.0;
/// A payload heard within this time is active, s.
const ACTIVE_S: f64 = 120.0;
/// The climb rate is taken over a point at least this old, s ...
const CLIMB_MIN_S: f64 = 30.0;
/// ... and at most this old, s.
const CLIMB_MAX_S: f64 = 180.0;
/// The owner and the version of the saved state.
const STATE_OWNER: &str = "science.horus";
const STATE_VERSION: u32 = 1;

/// A packet read with the present tables.
#[derive(Clone, Debug, PartialEq)]
pub struct Packet {
    /// The kind.
    pub kind: Kind,
    /// The callsign, or "ID n".
    pub callsign: String,
    /// The payload ID of a binary packet.
    pub payload_id: Option<u16>,
    /// The sequence number.
    pub seq: u32,
    /// The time of day: hours, minutes, seconds.
    pub time: (u8, u8, u8),
    /// The position, degrees; `None` without a fix.
    pub position: Option<(f64, f64)>,
    /// The altitude, m.
    pub alt_m: f64,
    /// The speed, km/h.
    pub speed_kmh: Option<f64>,
    /// The satellites.
    pub sats: Option<i64>,
    /// The temperature, degrees C.
    pub temp_c: Option<f64>,
    /// The battery, V.
    pub battery_v: Option<f64>,
    /// The custom fields (v2) or the fields after the altitude (RTTY): name and value.
    pub fields: Vec<(String, String)>,
    /// The packet as a sentence.
    pub sentence: String,
    /// The bytes of a binary packet.
    pub bytes: Option<Vec<u8>>,
}

/// Reads `decoded` with the ID list and the custom formats. `None` for a sentence that does not read.
pub fn read(decoded: &Decoded, ids: &IdList, formats: &[CustomFormat]) -> Option<Packet> {
    match decoded {
        Decoded::Binary { bytes, .. } => {
            let b = Binary::parse(bytes)?;
            let callsign = ids.callsign(b.id);
            let fields = b
                .custom
                .as_ref()
                .map(|c| custom_fields(format_of(formats, &callsign, Some(b.id)), c))
                .unwrap_or_default();
            let sentence = b.sentence(&callsign, &fields);
            Some(Packet {
                kind: if b.version == 1 { Kind::V1 } else { Kind::V2 },
                callsign,
                payload_id: Some(b.id),
                seq: u32::from(b.seq),
                time: b.time,
                position: b.has_fix().then_some((b.lat, b.lon)),
                alt_m: f64::from(b.alt_m),
                speed_kmh: Some(f64::from(b.speed_kmh)),
                sats: Some(i64::from(b.sats)),
                temp_c: Some(f64::from(b.temp_c)),
                battery_v: Some(b.battery_v()),
                fields,
                sentence,
                bytes: Some(bytes.clone()),
            })
        }
        Decoded::Rtty { sentence, .. } => {
            let first = parse_sentence(sentence, &[])?;
            let names = format_of(formats, &first.callsign, None)
                .map(|f| f.sentence.as_slice())
                .unwrap_or(&[]);
            let s = parse_sentence(sentence, names)?;
            let fix = !(s.lat == 0.0 && s.lon == 0.0) && s.lat.is_finite() && s.lon.is_finite();
            Some(Packet {
                kind: Kind::Rtty,
                callsign: s.callsign,
                payload_id: None,
                seq: s.seq,
                time: s.time,
                position: fix.then_some((s.lat, s.lon)),
                alt_m: s.alt_m,
                speed_kmh: None,
                sats: None,
                temp_c: None,
                battery_v: None,
                fields: s.fields,
                sentence: s.text,
                bytes: None,
            })
        }
    }
}

/// The time of the packet `time` on the receive date of `received` (UTC, Unix seconds): a time more than 12 h ahead of the
/// receive time is of the day before, more than 12 h behind of the day after.
pub fn track_time(time: (u8, u8, u8), received: f64) -> f64 {
    let day = (received / 86_400.0).floor() * 86_400.0;
    let t = day + f64::from(time.0) * 3600.0 + f64::from(time.1) * 60.0 + f64::from(time.2);
    if t - received > 43_200.0 {
        t - 86_400.0
    } else if received - t > 43_200.0 {
        t + 86_400.0
    } else {
        t
    }
}

/// How a packet was received.
#[derive(Clone, Debug, PartialEq)]
struct Reception {
    received: f64,
    channel: usize,
    offset_hz: f64,
    freq_hz: Option<f64>,
    snr_db: f64,
    spacing_hz: f64,
    baud: f64,
    fixed: Option<usize>,
    bits: Option<u8>,
}

/// One payload.
#[derive(Clone, Debug)]
struct Payload {
    last: Packet,
    rx: Reception,
    packets: u64,
    first_heard: f64,
    /// Track points: time, latitude, longitude, altitude.
    track: VecDeque<(f64, f64, f64, f64)>,
    max_alt: f64,
    climb: Option<f64>,
}

impl Payload {
    /// Takes the packet `p`: its track point, the highest altitude and the climb rate.
    fn take(&mut self, p: Packet, rx: Reception) {
        if let Some((lat, lon)) = p.position {
            let t = track_time(p.time, rx.received);
            if self.track.back().is_none_or(|b| t > b.0) {
                if self.track.len() == MAX_TRACK {
                    self.track.pop_front();
                }
                self.track.push_back((t, lat, lon, p.alt_m));
                self.climb = self
                    .track
                    .iter()
                    .find(|q| t - q.0 <= CLIMB_MAX_S && t - q.0 >= CLIMB_MIN_S)
                    .map(|q| (p.alt_m - q.3) / (t - q.0));
            }
        }
        self.max_alt = self.max_alt.max(p.alt_m);
        self.packets += 1;
        self.last = p;
        self.rx = rx;
    }
}

/// One entry of the log, as it is saved.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    rx: Reception,
    decoded: Decoded,
    callsign: String,
    kind: Kind,
    sentence: String,
    number: u64,
}

sdrkit_core::settings! {
    /// The settings of [`PayloadTable`].
    pub enum TableSetting {
        /// The payload ID list.
        PayloadIds = PAYLOAD_IDS,
        /// The custom field formats.
        CustomFormats = CUSTOM_FORMATS,
    }
}

sdrkit_core::commands! {
    /// The commands of [`PayloadTable`].
    pub enum TableCommand {
        /// Forgets every payload and the log.
        Clear = objcmd::CLEAR,
    }
}

/// What the payload table publishes.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(HorusLogFields::SHAPE.fields)
        .with_table(HORUS_PAYLOADS.def())
        .with_table(HORUS_CUSTOM_FIELDS.def())
        .with_table(HORUS_PACKETS.def())
        .points("Horus balloons")
}

/// The payload table component.
pub struct PayloadTable {
    rate: f64,
    t: f64,
    ids_text: String,
    ids: IdList,
    formats_text: String,
    formats: Vec<CustomFormat>,
    payloads: Vec<Payload>,
    log: VecDeque<Entry>,
    numbered: u64,
    report: BankReport,
    events: Vec<Event>,
    published: Published,
    shown: Status,
    live: bool,
    clock: Clock,
    key: String,
    text: String,
}

impl Default for PayloadTable {
    fn default() -> Self {
        let formats_text = CUSTOM_FORMATS.default.to_string();
        PayloadTable {
            rate: 0.0,
            t: 0.0,
            ids_text: PAYLOAD_IDS.default.to_string(),
            ids: IdList::parse(PAYLOAD_IDS.default),
            formats: parse_formats(&formats_text).unwrap_or_default(),
            formats_text,
            payloads: Vec::new(),
            log: VecDeque::new(),
            numbered: 0,
            report: BankReport::default(),
            events: Vec::new(),
            published: Published::from_decl(Arc::new(declaration())),
            shown: Status::default(),
            live: false,
            clock: Clock::fixed(0.0),
            key: String::new(),
            text: String::new(),
        }
    }
}

impl PayloadTable {
    /// The receive time of the signal time now: the wall clock of live samples, else the time from the start of the test
    /// signal.
    fn now(&self) -> f64 {
        wall(self.live, &self.clock).unwrap_or(TEST_START_UTC + self.t)
    }

    /// Takes a packet received at `rx`. Returns the packet, or `None` for a duplicate or a packet that does not read.
    fn add(&mut self, decoded: Decoded, rx: Reception) -> Option<Packet> {
        let p = read(&decoded, &self.ids, &self.formats)?;
        let dup = self
            .log
            .iter()
            .rev()
            .take_while(|e| rx.received - e.rx.received <= DUPLICATE_S)
            .any(|e| e.callsign == p.callsign && e.kind == p.kind && read_seq(e) == Some(p.seq));
        if dup {
            return None;
        }
        self.numbered += 1;
        if self.log.len() == MAX_STORED {
            self.log.pop_front();
        }
        self.log.push_back(Entry {
            rx: rx.clone(),
            decoded,
            callsign: p.callsign.clone(),
            kind: p.kind,
            sentence: p.sentence.clone(),
            number: self.numbered,
        });
        match self
            .payloads
            .iter()
            .position(|q| q.last.callsign == p.callsign)
        {
            Some(i) => {
                let mut q = self.payloads.remove(i);
                q.take(p.clone(), rx);
                self.payloads.insert(0, q);
            }
            None => {
                if self.payloads.len() == MAX_PAYLOADS {
                    self.payloads.pop();
                }
                let mut q = Payload {
                    last: p.clone(),
                    first_heard: rx.received,
                    rx: rx.clone(),
                    packets: 0,
                    track: VecDeque::new(),
                    max_alt: p.alt_m,
                    climb: None,
                };
                q.take(p.clone(), rx);
                self.payloads.insert(0, q);
            }
        }
        Some(p)
    }

    /// Reads every packet of the log again with the present tables.
    fn reread(&mut self) {
        let log: Vec<Entry> = std::mem::take(&mut self.log).into();
        self.payloads.clear();
        self.numbered = 0;
        for e in log {
            let _ = self.add(e.decoded, e.rx);
        }
        self.published.results_mut().clear_points();
    }

    fn publish(&mut self) {
        let now = self.now();
        let active = self
            .payloads
            .iter()
            .filter(|q| now - q.rx.received <= ACTIVE_S)
            .count();
        let p = &mut self.published;
        p.put(&HorusLogFields {
            packets: self.numbered as i64,
            payloads_active: active as i64,
            unreadable_ids: self.ids.unreadable as i64,
        });
        p.clear_rows(HORUS_PAYLOADS.def());
        p.clear_rows(HORUS_CUSTOM_FIELDS.def());
        for q in self.payloads.iter().rev() {
            let l = &q.last;
            HORUS_PAYLOADS.put(
                p,
                &l.callsign,
                &HorusPayloadRow {
                    payload: &l.callsign,
                    r#type: l.kind,
                    payload_id: l.payload_id.map(i64::from),
                    seq: i64::from(l.seq),
                    time: Time(track_time(l.time, q.rx.received)),
                    position: l.position,
                    height_m: l.alt_m,
                    max_alt_m: q.max_alt,
                    climb_m_s: q.climb,
                    speed_kmh: l.speed_kmh,
                    sats: l.sats,
                    temp_c: l.temp_c,
                    battery_v: l.battery_v,
                    snr_db: q.rx.snr_db,
                    packets: q.packets as i64,
                    offset: Hertz(q.rx.offset_hz),
                    frequency: q.rx.freq_hz.map(Hertz),
                    baud: q.rx.baud,
                    spacing: Hertz(q.rx.spacing_hz),
                    corrections: q.rx.fixed.map(|f| f as i64),
                    rtty_bits: q.rx.bits.map(i64::from),
                    track_points: q.track.len() as i64,
                    sentence: &l.sentence,
                    first_heard: Some(Time(q.first_heard.floor())),
                    last_heard: Some(Time(q.rx.received.floor())),
                },
            );
            for (i, (name, value)) in l.fields.iter().enumerate() {
                self.key.clear();
                let _ = write!(self.key, "{}/{}", l.callsign, i + 1);
                HORUS_CUSTOM_FIELDS.put(
                    p,
                    &self.key,
                    &HorusCustomFieldRow {
                        payload: &l.callsign,
                        name,
                        value,
                    },
                );
            }
        }
        p.clear_rows(HORUS_PACKETS.def());
        // The last row written is shown first: write the newest packet last.
        let skip = self.log.len().saturating_sub(MAX_LOG);
        for e in self.log.iter().skip(skip) {
            self.key.clear();
            let _ = write!(self.key, "{:06}", e.number);
            HORUS_PACKETS.put(
                p,
                &self.key,
                &HorusPacketRow {
                    received: Time(e.rx.received),
                    payload: &e.callsign,
                    r#type: e.kind,
                    snr_db: e.rx.snr_db,
                    offset: Hertz(e.rx.offset_hz),
                    channel: e.rx.channel as i64,
                    sentence: &e.sentence,
                },
            );
        }
        let r = p.results_mut();
        r.clear_points();
        let table = r.decl().table_id(HORUS_PAYLOADS.def().key);
        for q in &self.payloads {
            if let Some((lat, lon)) = q.last.position {
                self.text.clear();
                let _ = write!(self.text, "{} at {:.0} m", q.last.callsign, q.last.alt_m);
                r.update_point(&q.last.callsign, lat, lon, &self.text, None);
                r.link_point(
                    &q.last.callsign,
                    table.map(|t| (t, q.last.callsign.as_str())),
                );
            }
        }
        let state = self.report.state;
        let newest = self.payloads.first().map(|q| &q.last);
        status(
            &mut self.shown,
            lock_state_of(state as u8),
            self.report.snr_db.map(|s| s as f32),
            None,
            self.report.offset_hz,
            |line| {
                let _ = match (state, newest) {
                    (State::Decoding, Some(l)) => summary(line, l),
                    (State::Waiting, _) => write!(line, "Horus: waiting for tones"),
                    _ => write!(line, "Horus: listening"),
                };
            },
        );
        p.results_mut().set_status(&self.shown);
    }

    fn reception(&self, h: &Heard, received: f64) -> Reception {
        let (baud, fixed, bits) = match &h.decoded {
            Decoded::Binary { fixed, .. } => (super::demod::BAUD, Some(*fixed), None),
            Decoded::Rtty { baud, bits, .. } => (*baud, None, Some(*bits)),
        };
        Reception {
            received,
            channel: h.channel,
            offset_hz: h.offset_hz,
            freq_hz: h.freq_hz,
            snr_db: h.snr_db,
            spacing_hz: h.spacing_hz,
            baud,
            fixed,
            bits,
        }
    }

    /// The saved state: one line a packet of the log, oldest first.
    fn state_text(&self) -> String {
        let mut s = String::new();
        for e in &self.log {
            let r = &e.rx;
            let data = match &e.decoded {
                Decoded::Binary { bytes, fixed } => {
                    let mut h = String::new();
                    for b in bytes {
                        let _ = write!(h, "{b:02X}");
                    }
                    format!("binary\t{fixed}\t{h}")
                }
                Decoded::Rtty {
                    sentence,
                    baud,
                    bits,
                } => format!("rtty\t{baud}/{bits}\t{sentence}"),
            };
            let _ = writeln!(
                s,
                "{}\t{}\t{}\t{}\t{}\t{}\t{data}",
                r.received,
                r.channel,
                r.offset_hz,
                r.freq_hz.map_or(String::new(), |f| f.to_string()),
                r.snr_db,
                r.spacing_hz
            );
        }
        s
    }
}

/// The sequence number of the packet of `e`.
fn read_seq(e: &Entry) -> Option<u32> {
    match &e.decoded {
        Decoded::Binary { bytes, .. } => Binary::parse(bytes).map(|b| u32::from(b.seq)),
        Decoded::Rtty { sentence, .. } => parse_sentence(sentence, &[]).map(|s| s.seq),
    }
}

/// "Horus <callsign> #<seq> <lat> <lon> <alt> m".
fn summary(line: &mut String, l: &Packet) -> std::fmt::Result {
    let (lat, lon) = l.position.unwrap_or((0.0, 0.0));
    write!(
        line,
        "Horus {} #{} {lat:.5} {lon:.5} {:.0} m",
        l.callsign, l.seq, l.alt_m
    )
}

/// One line of the saved state.
fn read_line(line: &str) -> Option<(Decoded, Reception)> {
    let f: Vec<&str> = line.splitn(9, '\t').collect();
    if f.len() != 9 {
        return None;
    }
    let num = |s: &str| s.parse::<f64>().ok().filter(|v| v.is_finite());
    let (received, channel, offset_hz) = (num(f[0])?, f[1].parse().ok()?, num(f[2])?);
    let freq_hz = if f[3].is_empty() {
        None
    } else {
        Some(num(f[3])?)
    };
    let (snr_db, spacing_hz) = (num(f[4])?, num(f[5])?);
    let (decoded, baud, fixed, bits) = match f[6] {
        "binary" => {
            let fixed: usize = f[7].parse().ok()?;
            let h = f[8];
            if !h.len().is_multiple_of(2) {
                return None;
            }
            let bytes: Vec<u8> = (0..h.len() / 2)
                .map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).ok())
                .collect::<Option<_>>()?;
            Binary::parse(&bytes)?;
            (
                Decoded::Binary { bytes, fixed },
                super::demod::BAUD,
                Some(fixed),
                None,
            )
        }
        "rtty" => {
            let (baud, bits) = f[7].split_once('/')?;
            let (baud, bits) = (num(baud)?, bits.parse().ok()?);
            parse_sentence(f[8], &[])?;
            (
                Decoded::Rtty {
                    sentence: f[8].to_string(),
                    baud,
                    bits,
                },
                baud,
                None,
                Some(bits),
            )
        }
        _ => return None,
    };
    Some((
        decoded,
        Reception {
            received,
            channel,
            offset_hz,
            freq_hz,
            snr_db,
            spacing_hz,
            baud,
            fixed,
            bits,
        },
    ))
}

impl Component for PayloadTable {
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "horus.payloads",
            vec![
                Port::iq("in"),
                Port::typed::<Heard>("packets"),
                Port::typed::<BankReport>("report"),
            ],
            vec![Port::new("events", PortKind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(TableSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.clock = ctx.clock.clone();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.report = BankReport::default();
        self.publish();
        Ok(vec![Format::new(PortKind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let n = io.input_len(0);
        self.t = (first + n as u64) as f64 / self.rate.max(1.0);
        for h in io.items::<Heard>(1) {
            let rx = self.reception(&h.value, self.now());
            let Some(p) = self.add(h.value.decoded.clone(), rx) else {
                continue;
            };
            self.text.clear();
            let _ = summary(&mut self.text, &p);
            let climb = self.payloads.first().and_then(|q| q.climb);
            let ev = HORUS_PACKET.event(
                h.time,
                h.value.freq_hz.unwrap_or(0.0),
                &HorusPacketEvent {
                    source: &p.callsign,
                    text: &self.text,
                    family: Family::Sonde,
                    position: p.position,
                    height_m: Some(p.alt_m),
                    r#type: p.kind,
                    seq: i64::from(p.seq),
                    climb_m_s: climb,
                    speed_kmh: p.speed_kmh,
                    sats: p.sats,
                    temp_c: p.temp_c,
                    battery_v: p.battery_v,
                    snr_db: h.value.snr_db,
                    offset: Hertz(h.value.offset_hz),
                    channel: h.value.channel as i64,
                    sentence: &p.sentence,
                },
            );
            self.events.push(ev);
        }
        for r in io.items::<BankReport>(2) {
            self.events.extend(r.value.events.iter().cloned());
            self.report = BankReport {
                events: Vec::new(),
                ..r.value.clone()
            };
        }
        if io.tick() {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TableSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TableSetting::PayloadIds => {
                let text = PAYLOAD_IDS.read(id, v)?;
                self.ids = IdList::parse(text);
                self.ids_text = text.to_string();
            }
            TableSetting::CustomFormats => {
                let text = CUSTOM_FORMATS.read(id, v)?;
                self.formats =
                    parse_formats(text).map_err(|_| SettingError::Malformed { id, at: None })?;
                self.formats_text = text.to_string();
            }
        }
        self.reread();
        self.publish();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (TableSetting::PayloadIds, Value::Text(self.ids_text.clone())),
            (
                TableSetting::CustomFormats,
                Value::Text(self.formats_text.clone()),
            ),
        ])
    }
    fn commands(&self) -> &Commands {
        TableCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match TableCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TableCommand::Clear => {
                self.payloads.clear();
                self.log.clear();
                self.numbered = 0;
                self.published.results_mut().clear_points();
            }
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        Some(StateBlob::new(
            STATE_OWNER,
            STATE_VERSION,
            self.state_text().into_bytes(),
        ))
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let bytes = state.open(STATE_OWNER, STATE_VERSION)?;
        let text = std::str::from_utf8(bytes).map_err(|_| StateError::Damaged)?;
        let lines: Vec<(Decoded, Reception)> = text
            .lines()
            .filter(|l| !l.is_empty())
            .map(read_line)
            .collect::<Option<_>>()
            .ok_or(StateError::Damaged)?;
        self.payloads.clear();
        self.log.clear();
        self.numbered = 0;
        for (d, rx) in lines {
            let _ = self.add(d, rx);
        }
        self.published.results_mut().clear_points();
        self.publish();
        Ok(())
    }
}
