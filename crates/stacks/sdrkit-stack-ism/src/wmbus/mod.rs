//! Wireless M-Bus meters (`wmbus`): every channel of the plan in the capture is demodulated at once, frames are checked block by
//! block, and unencrypted application data is decoded. A payload under a security mode is listed as encrypted and never decoded.
//!
//! Standard: EN 13757-4 (radio link), EN 13757-3 (application layer)

pub mod link;
pub mod rx;
pub mod send;

use crate::family::{at_least_value, hex, min_rate_message, wall, CleanScene, REPORT_S};
use crate::typed::{
    WmbusChannelRow, WmbusFields, WmbusFrameEvent, WmbusFrameRow, WmbusMeterArgs, WmbusMeterRow,
    WmbusRecordRow, WMBUS_CHANNELS, WMBUS_FRAME, WMBUS_FRAMES, WMBUS_LEVEL_HISTORY, WMBUS_METERS,
    WMBUS_METER_ROW, WMBUS_RECORDS, WMBUS_REMOVE, WMBUS_TRACE,
};
use link::{add_crcs, manufacturer_code, Format as FrameFormat, Parsed, Record as DataRecord};
use rx::{Chain, Phy, WmbusFrame};
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, ReadFields, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::AutoGain;
use sdrkit_core::SignalType;
use sdrkit_core::{
    Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, LockState, Receiver, Record,
    Results, ResultsDecl, SeriesId, Setting, SettingSet, SignalInfo, StackConfig, StackDescriptor,
    StateBlob, StateError, Status, TableId, Tuning, Values,
};
use sdrkit_graph::Catalog;
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError, Io, Kept, Kind,
    Port, RateFrom, RunContext, StackGraph, Timed,
};
use sdrkit_util_bits::line::{three_of_six, Manchester, ManchesterConv};
use sdrkit_util_objects::commands;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::Tables as _;
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

/// The least sample rate of the stack.
pub const MIN_RATE_HZ: f64 = 1e6;
/// Meters kept.
pub const METERS_CAP: usize = 300;
/// Frames kept in the log.
pub const FRAMES_CAP: usize = 200;
/// Seconds of level history.
pub const HISTORY_S: usize = 120;
/// Data records of meters kept.
pub const RECORDS_CAP: usize = 3000;

/// `wmbus.channels`: the channel plan and one chain for each channel.
#[derive(Default)]
pub struct Channels {
    chains: Vec<Chain>,
    rate: f64,
    out: Vec<WmbusFrame>,
}

/// One channel as the tables show it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelInfo {
    /// The channel frequency.
    pub hz: f64,
    /// The physical modes on it ("S1", "T1 C1", "R2", "N1").
    pub phy: String,
    /// The slow power, dBFS.
    pub level_dbfs: f32,
    /// The noise floor, dBFS.
    pub floor_dbfs: f32,
    /// Good frames.
    pub good: u64,
    /// Frames with a failed CRC.
    pub bad: u64,
}

/// The state of the channels, sent at every report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelState {
    /// The channels.
    pub channels: Vec<ChannelInfo>,
    /// Syncs found.
    pub syncs: u64,
    /// Syncs that did not give a frame.
    pub false_syncs: u64,
}

impl sdrkit_graph::Item for ChannelState {
    const KIND: &'static str = "wmbus.channels";
}

/// Whether a frame on a narrow channel belongs to a stronger neighbour within 15 kHz (spec 5.3: the strongest of overlapping
/// 12.5 kHz channels owns the signal).
fn owned_by_neighbour(chains: &[Chain], f: &WmbusFrame) -> bool {
    let me = chains
        .iter()
        .find(|c| c.ch.hz == f.channel_hz)
        .map_or(0.0, |c| c.fast);
    chains.iter().any(|c| {
        !c.ch.phy.wide()
            && c.ch.hz != f.channel_hz
            && (c.ch.hz - f.channel_hz).abs() <= 15e3
            && c.fast > 1.5 * me
    })
}

impl Component for Channels {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wmbus.channels",
            vec![Port::iq("in")],
            vec![
                Port::typed::<WmbusFrame>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<ChannelState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.chains = rx::plan(f.centre_hz, f.rate)
            .into_iter()
            .map(|c| Chain::new(c, f.rate, f.centre_hz))
            .collect();
        let mut o = f.with_kind(Kind::typed::<WmbusFrame>());
        o.rate = 0.0;
        let mut s = f.with_kind(Kind::typed::<ChannelState>());
        s.rate = 0.0;
        Ok(vec![o, s])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let first = io.input(0).first();
        for c in self.chains.iter_mut() {
            c.feed(x, first, &mut self.out);
        }
        let chains = &self.chains;
        self.out.retain(|f| !owned_by_neighbour(chains, f));
        if !self.out.is_empty() {
            let items = io.items_out::<WmbusFrame>(0);
            for f in self.out.drain(..) {
                items.push(Timed {
                    time: f.time,
                    value: f,
                });
            }
        }
        if io.tick() {
            let mut st = ChannelState::default();
            for c in &self.chains {
                let (s, f) = c.syncs();
                st.syncs += s;
                st.false_syncs += f;
                st.channels.push(ChannelInfo {
                    hz: c.ch.hz,
                    phy: c.ch.phy.label().into(),
                    level_dbfs: power_to_db((c.slow as f64).max(1e-20)) as f32,
                    floor_dbfs: power_to_db((c.floor as f64).max(1e-20)) as f32,
                    good: c.good,
                    bad: c.bad,
                });
            }
            let t = io.input(0).end() as f64 / self.rate.max(1.0);
            io.items_out::<ChannelState>(1)
                .push(Timed { time: t, value: st });
        }
    }
}

/// One meter of the table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Meter {
    /// The manufacturer letters.
    pub manufacturer: String,
    /// The id (8 digits).
    pub id: String,
    /// The device type name.
    pub type_name: String,
    /// The version.
    pub version: u8,
    /// The physical mode of the last frame.
    pub phy: String,
    /// The channel of the last frame.
    pub channel_hz: f64,
    /// The level of the last frame, dBFS.
    pub level_dbfs: f32,
    /// The payload is encrypted.
    pub encrypted: bool,
    /// The CI field of the last frame.
    pub ci: u8,
    /// The summary of the last frame.
    pub summary: String,
    /// Every record of the last frame.
    pub records: Vec<DataRecord>,
    /// The application status.
    pub status: String,
}

impl Shift for Meter {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Meter {
    type Row = WmbusMeterRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&WmbusMeterRow {
            maker: &self.manufacturer,
            meter_id: &self.id,
            device_type: &self.type_name,
            phy: &self.phy,
            frequency: Hertz(self.channel_hz),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
            level_dbfs: f64::from(self.level_dbfs),
            encrypted: self.encrypted,
            last_reading: &self.summary,
            status: &self.status,
        });
    }
}

/// A data record of the last frame of a meter.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MeterRecord {
    /// The row key of the meter.
    pub meter: String,
    /// The place of the record in its frame.
    pub index: usize,
    /// The record.
    pub record: DataRecord,
}

impl Shift for MeterRecord {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for MeterRecord {
    type Row = WmbusRecordRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let notes = (!self.record.notes.is_empty()).then(|| self.record.notes.join(", "));
        out.put(&WmbusRecordRow {
            meter: &self.meter,
            index: self.index as i64,
            quantity: &self.record.quantity,
            value: &self.record.value,
            notes: notes.as_deref(),
        });
    }
}

/// One frame of the log.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FrameLog {
    /// Signal time, seconds.
    pub at: f64,
    /// Wall-clock time, when the samples are live.
    pub wall: Option<f64>,
    /// The physical mode.
    pub phy: String,
    /// The channel.
    pub channel_hz: f64,
    /// The level, dBFS.
    pub level_dbfs: f32,
    /// Every CRC passed.
    pub crc_ok: bool,
    /// The payload is encrypted.
    pub encrypted: bool,
    /// The manufacturer letters.
    pub manufacturer: String,
    /// The meter id.
    pub id: String,
    /// The summary.
    pub summary: String,
    /// The bytes in hex (without CRCs; the bytes on air when a CRC failed).
    pub hex: String,
}

impl Shift for FrameLog {
    fn shift_times(&mut self, by_s: f64) {
        self.at += by_s;
    }
}

impl Shaped for FrameLog {
    type Row = WmbusFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let meter = format!("{} {}", self.manufacturer, self.id);
        out.put(&WmbusFrameRow {
            time_s: self.at,
            time: self.wall.map(Time),
            phy: &self.phy,
            frequency: Hertz(self.channel_hz),
            meter: Some(meter.trim()).filter(|m| !m.is_empty()),
            level_dbfs: f64::from(self.level_dbfs),
            crc_ok: self.crc_ok,
            encrypted: self.encrypted,
            result: &self.summary,
            frame: &self.hex,
        });
    }
}

/// What the stack keeps through a restart and in the saved state: the meters and their records, the frame log and the frame
/// counts.
#[derive(Serialize, Deserialize)]
pub struct Store {
    meters: ObjectTable<String, Meter>,
    records: ObjectTable<String, MeterRecord>,
    frames: LogRing<FrameLog>,
    good: u64,
    bad: u64,
    encrypted: u64,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            meters: ObjectTable::new(METERS_CAP),
            records: ObjectTable::new(RECORDS_CAP),
            frames: LogRing::new(FRAMES_CAP),
            good: 0,
            bad: 0,
            encrypted: 0,
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.meters.shift_times(by_s);
        self.records.shift_times(by_s);
        self.frames.shift_times(by_s);
    }
}

impl sdrkit_util_objects::Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("ism.wmbus", 2);
    fn clear(&mut self) {
        self.meters.clear();
        self.records.clear();
        self.frames.clear();
        self.good = 0;
        self.bad = 0;
        self.encrypted = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.meters.adopt(saved.meters);
        self.records.adopt(saved.records);
        self.frames.adopt(saved.frames);
        self.good = saved.good;
        self.bad = saved.bad;
        self.encrypted = saved.encrypted;
    }
}

/// `wmbus.meters`: the link layer, the application data and the tables.
pub struct Meters {
    published: Published,
    meters_table: TableId,
    level_series: Option<SeriesId>,
    trace_series: Option<SeriesId>,
    channel_keys: Vec<String>,
    channel_hz: Vec<f64>,
    store: Store,
    channels: ChannelState,
    history: VecDeque<f32>,
    trace: Vec<f32>,
    ticks: u64,
    now: f64,
    last_offset_hz: Option<f64>,
    live: bool,
    events: Vec<Event>,
}

impl Default for Meters {
    fn default() -> Self {
        let published = Published::from_decl(Arc::new(
            ResultsDecl::new()
                .with_fields(WmbusFields::SHAPE.fields)
                .with_table(&WMBUS_METERS)
                .with_table(&WMBUS_RECORDS)
                .with_table(&WMBUS_FRAMES)
                .with_table(&WMBUS_CHANNELS)
                .with_series(&WMBUS_LEVEL_HISTORY)
                .with_series(&WMBUS_TRACE),
        ));
        let decl = published.results().decl();
        let meters_table = decl
            .table_id(WMBUS_METERS.key)
            .expect("the tables of wmbus.meters are declared");
        let level_series = decl.series_id(WMBUS_LEVEL_HISTORY.key);
        let trace_series = decl.series_id(WMBUS_TRACE.key);
        Meters {
            published,
            meters_table,
            level_series,
            trace_series,
            channel_keys: Vec::new(),
            channel_hz: Vec::new(),
            store: Store::default(),
            channels: ChannelState::default(),
            history: VecDeque::with_capacity(HISTORY_S + 1),
            trace: Vec::new(),
            ticks: 0,
            now: 0.0,
            last_offset_hz: None,
            live: false,
            events: Vec::new(),
        }
    }
}

impl Meters {
    fn add(&mut self, f: &WmbusFrame) {
        self.now = self.now.max(f.time);
        let wall = wall(self.live);
        let mut log = FrameLog {
            at: f.time,
            wall,
            phy: f.phy.clone(),
            channel_hz: f.channel_hz,
            level_dbfs: f.level_db,
            crc_ok: f.data.is_some(),
            encrypted: false,
            manufacturer: String::new(),
            id: String::new(),
            summary: "CRC failed".into(),
            hex: hex(&f.air),
        };
        self.last_offset_hz = Some(f.offset_hz);
        self.trace.clone_from(&f.trace);
        match f.data.as_ref().and_then(|d| link::parse(d).map(|p| (d, p))) {
            Some((data, p)) => {
                self.store.good += 1;
                if p.encrypted {
                    self.store.encrypted += 1;
                }
                log.encrypted = p.encrypted;
                log.manufacturer.clone_from(&p.manufacturer);
                log.id.clone_from(&p.id);
                log.summary = p.summary();
                log.hex = hex(data);
                self.meter(f, &p, wall);
                self.published.results_mut().log(format!(
                    "wM-Bus {} {} {}: {}",
                    f.phy, p.manufacturer, p.id, log.summary
                ));
                self.events.push(WMBUS_FRAME.event(
                    f.time,
                    f.channel_hz,
                    &WmbusFrameEvent {
                        source: &format!("{} {}", p.manufacturer, p.id),
                        text: &format!(
                            "wM-Bus {} {} {}: {}",
                            f.phy, p.manufacturer, p.id, log.summary
                        ),
                        family: Family::Sensor,
                        maker: &p.manufacturer,
                        meter_id: &p.id,
                        device_type: &p.type_name(),
                        phy: &f.phy,
                        encrypted: p.encrypted,
                        level_dbfs: f64::from(f.level_db),
                    },
                ));
            }
            None => {
                if f.data.is_some() {
                    log.summary = "frame too short".into();
                    self.store.good += 1;
                } else {
                    self.store.bad += 1;
                }
            }
        }
        self.store.frames.push(log);
    }

    fn meter(&mut self, f: &WmbusFrame, p: &Parsed, wall: Option<f64>) {
        let key = format!("{} {} {}", p.manufacturer, p.id, p.type_name());
        let at = match wall {
            Some(w) => Stamp::at(f.time).with_wall(w),
            None => Stamp::at(f.time),
        };
        for (index, record) in p.records.iter().enumerate() {
            let rk = format!("{key}/{index}");
            if let Ok(up) = self.store.records.upsert(&rk, at, MeterRecord::default) {
                *up.row = MeterRecord {
                    meter: key.clone(),
                    index,
                    record: record.clone(),
                };
            }
        }
        for index in p.records.len().. {
            if self
                .store
                .records
                .remove(&format!("{key}/{index}"))
                .is_none()
            {
                break;
            }
        }
        if let Ok(up) = self.store.meters.upsert(&key, at, Meter::default) {
            *up.row = Meter {
                manufacturer: p.manufacturer.clone(),
                id: p.id.clone(),
                type_name: p.type_name(),
                version: p.version,
                phy: f.phy.clone(),
                channel_hz: f.channel_hz,
                level_dbfs: f.level_db,
                encrypted: p.encrypted,
                ci: p.ci,
                summary: p.summary(),
                records: p.records.clone(),
                status: p.status.clone(),
            };
        }
    }

    fn state(&self) -> u8 {
        let strongest = self.strongest();
        if self.store.good + self.store.bad > 0 {
            2
        } else if strongest.is_some_and(|c| c.level_dbfs > c.floor_dbfs + 10.0) {
            1
        } else {
            0
        }
    }

    fn strongest(&self) -> Option<&ChannelInfo> {
        self.channels
            .channels
            .iter()
            .max_by(|a, b| a.level_dbfs.total_cmp(&b.level_dbfs))
    }

    fn publish(&mut self) {
        let state = self.state();
        let strongest = self.strongest().cloned();
        let mut summary = String::new();
        match state {
            2 => {
                let _ = write!(
                    summary,
                    "Wireless M-Bus: {} meters, {} frames",
                    self.store.meters.len(),
                    self.store.good
                );
            }
            1 => summary.push_str("Wireless M-Bus: signal on a channel, no frame yet"),
            _ => summary.push_str("Wireless M-Bus: no signal"),
        }
        let p = &mut self.published;
        {
            let r = p.results_mut();
            r.set_status(&Status {
                lock_state: LockState::from_level(state),
                level_dbfs: strongest.as_ref().map(|c| c.level_dbfs),
                snr_db: strongest.as_ref().map(|c| c.level_dbfs - c.floor_dbfs),
                offset_hz: self.last_offset_hz,
                summary,
            });
            let n = self.history.len() as f32;
            let history = || {
                self.history
                    .iter()
                    .enumerate()
                    .map(move |(i, v)| [i as f32 - n, *v])
            };
            let trace = || self.trace.iter().enumerate().map(|(i, v)| [i as f32, *v]);
            if let Some(id) = self.level_series {
                r.update_series(id, |series| copy_points(series, history()));
            }
            if let Some(id) = self.trace_series {
                r.update_series(id, |series| copy_points(series, trace()));
            }
        }
        p.put(&WmbusFields {
            frames_ok: self.store.good as i64,
            frames_bad: self.store.bad as i64,
            meters: self.store.meters.len() as i64,
            syncs: self.channels.syncs as i64,
            false_syncs: self.channels.false_syncs as i64,
            encrypted_frames: self.store.encrypted as i64,
        });
        shaped::rows(&self.store.meters, &WMBUS_METERS, self.now, p);
        shaped::rows(&self.store.records, &WMBUS_RECORDS, self.now, p);
        shaped::log_rows(&self.store.frames, &WMBUS_FRAMES, self.now, p);
        let same = self.channel_keys.len() == self.channels.channels.len()
            && self
                .channels
                .channels
                .iter()
                .zip(&self.channel_hz)
                .all(|(c, hz)| c.hz == *hz);
        if !same {
            p.clear_rows(&WMBUS_CHANNELS);
            self.channel_hz = self.channels.channels.iter().map(|c| c.hz).collect();
            self.channel_keys = self
                .channels
                .channels
                .iter()
                .map(|c| format!("{:.4} {}", c.hz / 1e6, c.phy))
                .collect();
        }
        for (c, id) in self.channels.channels.iter().zip(&self.channel_keys) {
            WMBUS_CHANNELS.put(
                p,
                id,
                &WmbusChannelRow {
                    frequency: Hertz(c.hz),
                    phy: &c.phy,
                    level_dbfs: f64::from(c.level_dbfs),
                    noise_dbfs: f64::from(c.floor_dbfs),
                    frames: i64::try_from(c.good).unwrap_or(i64::MAX),
                    crc_failures: i64::try_from(c.bad).unwrap_or(i64::MAX),
                },
            );
        }
    }
}

/// Makes `series` a line of `points`. Returns whether it changed.
fn copy_points(
    series: &mut sdrkit_core::Series,
    points: impl Iterator<Item = [f32; 2]> + Clone,
) -> bool {
    if series.line && series.points.iter().copied().eq(points.clone()) {
        return false;
    }
    series.line = true;
    series.points.clear();
    series.points.extend(points);
    true
}

impl Component for Meters {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wmbus.meters",
            vec![
                Port::typed::<WmbusFrame>("frames"),
                Port::typed::<ChannelState>("state"),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
    }
    fn process(&mut self, io: &mut Io) {
        let mut new = false;
        for f in io.items::<WmbusFrame>(0) {
            self.add(&f.value);
            new = true;
        }
        for s in io.items::<ChannelState>(1) {
            self.now = self.now.max(s.time);
            self.channels.clone_from(&s.value);
            self.ticks += 1;
            // One level a second (four reports a second).
            if self.ticks.is_multiple_of(4) {
                if let Some(l) = self.strongest().map(|c| c.level_dbfs) {
                    if self.history.len() == HISTORY_S {
                        self.history.pop_front();
                    }
                    self.history.push_back(l);
                }
            }
            new = true;
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
        if new {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        MetersCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match MetersCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            MetersCommand::Clear => {
                self.store.clear();
                self.published.clear_rows(&WMBUS_METERS);
                self.published.clear_rows(&WMBUS_RECORDS);
                self.published.clear_rows(&WMBUS_FRAMES);
            }
            MetersCommand::Remove => {
                let row = WMBUS_REMOVE.args::<WmbusMeterArgs>(args)?.row.to_owned();
                commands::remove(
                    &mut self.store.meters,
                    self.meters_table,
                    &mut self.published,
                    args,
                    &WMBUS_METER_ROW,
                )?;
                let prefix = format!("{row}/");
                let gone: Vec<String> = self
                    .store
                    .records
                    .iter()
                    .filter(|r| r.key.starts_with(&prefix))
                    .map(|r| r.key.clone())
                    .collect();
                for k in &gone {
                    self.store.records.remove(k);
                    self.published.remove_row(&WMBUS_RECORDS, k);
                }
            }
        }
        self.publish();
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        let s = &mut self.store;
        if !kept.table(WMBUS_METERS.key) {
            s.meters.clear();
            self.published.clear_rows(&WMBUS_METERS);
        }
        if !kept.table(WMBUS_RECORDS.key) {
            s.records.clear();
            self.published.clear_rows(&WMBUS_RECORDS);
        }
        if !kept.table(WMBUS_FRAMES.key) {
            s.frames.clear();
            self.published.clear_rows(&WMBUS_FRAMES);
        }
        for (key, n) in [
            ("frames_ok", &mut s.good),
            ("frames_bad", &mut s.bad),
            ("encrypted_frames", &mut s.encrypted),
        ] {
            if !kept.field(key) {
                *n = 0;
            }
        }
        if !kept.field("syncs") {
            self.channels.syncs = 0;
        }
        if !kept.field("false_syncs") {
            self.channels.false_syncs = 0;
        }
        self.channels.channels.clear();
        self.channel_keys.clear();
        self.channel_hz.clear();
        self.published.clear_rows(&WMBUS_CHANNELS);
        self.history.clear();
        self.trace.clear();
        self.last_offset_hz = None;
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

sdrkit_core::commands! {
    /// The commands of [`Meters`].
    pub enum MetersCommand {
        /// Forgets everything heard.
        Clear = commands::CLEAR,
        /// Forgets one meter and its records.
        Remove = WMBUS_REMOVE.command,
    }
}

/// Builds the `wmbus` graph.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`].
pub fn build_graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .catalog(catalog)
        .add("channels", Channels::default())
        .add("meters", Meters::default())
        .add("sender", crate::tx::FrameSender::<send::WmbusFrames>::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("channels")
        .input("sender.in")
        .connect("channels.frames", "meters.frames")
        .connect("channels.state", "meters.state")
        .connect("sender.out", "tx.in")
        .output("meters.events")
        .output("sender.events")
        .clock(REPORT_S)
        .status_from("meters")
        .build()
}

sdrkit_core::choice_enum! {
    /// The channel plan of the test signal.
    pub enum GenPlan {
        /// 868 MHz.
        Mhz868 = ("868", "868 MHz"),
        /// 169 MHz.
        Mhz169 = ("169", "169 MHz"),
    }
}

sdrkit_core::choice_enum! {
    /// The physical modes of the test signal.
    pub enum GenPhy {
        /// All modes.
        All = ("all", "All"),
        /// S1.
        S1 = ("S1", "S1"),
        /// T1.
        T1 = ("T1", "T1"),
        /// C1 with frame format A.
        C1a = ("C1A", "C1 format A"),
        /// C1 with frame format B.
        C1b = ("C1B", "C1 format B"),
        /// R2.
        R2 = ("R2", "R2"),
        /// N1.
        N1 = ("N1", "N1"),
    }
}

/// The channel plan of the test signal.
pub const GEN_PLAN: Setting<field::Choice<GenPlan>> =
    Setting::choice("gen_plan", "Test signal plan", GenPlan::Mhz868)
        .section("Test signal")
        .advanced();
/// The physical modes of the test signal.
pub const GEN_PHY: Setting<field::Choice<GenPhy>> =
    Setting::choice("gen_phy", "Test signal mode", GenPhy::All)
        .section("Test signal")
        .advanced();
/// An encrypted meter in the test signal.
pub const GEN_ENCRYPTED: Setting<field::Bool> =
    Setting::new("gen_encrypted", "Encrypted meter", "", true)
        .section("Test signal")
        .advanced();
/// No meters in the test signal.
pub const GEN_SILENCE: Setting<field::Bool> =
    Setting::new("gen_silence", "No meters (noise only)", "", false)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `wmbus` stack.
    pub enum WmbusSetting {
        /// The channel plan of the test signal.
        GenPlan = GEN_PLAN,
        /// The physical modes of the test signal.
        GenPhy = GEN_PHY,
        /// An encrypted meter in the test signal.
        GenEncrypted = GEN_ENCRYPTED,
        /// No meters in the test signal.
        GenSilence = GEN_SILENCE,
        /// The level sent.
        TxLevel = crate::tx::TX_LEVEL,
    }
}

/// The `wmbus` receiver.
///
/// # Errors
///
/// The errors of [`build_graph`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among them a
/// refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(
        crate::typed::shape("wmbus"),
        build_graph(&Catalog::of(cfg)?)?,
    )
    .min_rate(
        MIN_RATE_HZ,
        &min_rate_message("Wireless M-Bus meters", MIN_RATE_HZ),
    )
    .route(crate::tx::TX_LEVEL.key(), &["sender.tx_level"])?
    .command("clear", sdrkit_graph::CommandTarget::of("meters", "clear"))
    .command(
        "remove",
        sdrkit_graph::CommandTarget::of("meters", "remove"),
    )
    .command("send", sdrkit_graph::CommandTarget::of("sender", "send"))
    .command("halt", sdrkit_graph::CommandTarget::of("sender", "halt"))
    .with_settings(&cfg.settings)
    .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// A simulated meter.
#[derive(Clone, Debug)]
pub struct SimMeter {
    /// The manufacturer letters.
    pub mfr: &'static str,
    /// The id (8 digits).
    pub id: u32,
    /// The version.
    pub version: u8,
    /// The device type.
    pub dev_type: u8,
    /// The physical mode ("S1", "T1", "C1 (A)", "C1 (B)", "R2", "N1").
    pub phy: &'static str,
    /// The channel.
    pub hz: f64,
}

/// The simulated meters (spec 5.7).
pub const SIM_METERS: [SimMeter; 7] = [
    SimMeter {
        mfr: "KAM",
        id: 0x12345678,
        version: 0x1B,
        dev_type: 0x07,
        phy: "S1",
        hz: 868.3e6,
    },
    SimMeter {
        mfr: "ELV",
        id: 0x40112233,
        version: 0x2A,
        dev_type: 0x04,
        phy: "T1",
        hz: 868.95e6,
    },
    SimMeter {
        mfr: "EMH",
        id: 0x50667788,
        version: 0x01,
        dev_type: 0x02,
        phy: "T1",
        hz: 868.95e6,
    },
    SimMeter {
        mfr: "GWF",
        id: 0x61000042,
        version: 0x03,
        dev_type: 0x03,
        phy: "C1 (A)",
        hz: 868.95e6,
    },
    SimMeter {
        mfr: "SEN",
        id: 0x72004711,
        version: 0x0C,
        dev_type: 0x16,
        phy: "C1 (B)",
        hz: 868.95e6,
    },
    SimMeter {
        mfr: "QDS",
        id: 0x83001234,
        version: 0x05,
        dev_type: 0x08,
        phy: "R2",
        hz: 868.09e6,
    },
    SimMeter {
        mfr: "ITW",
        id: 0x94005566,
        version: 0x02,
        dev_type: 0x07,
        phy: "N1",
        hz: 169.43125e6,
    },
];

/// Appends `v` as `n` bytes of little-endian BCD (EN 13757-3 type A).
fn bcd_le(v: u64, n: usize, d: &mut Vec<u8>) {
    let mut b = [0u8; 8];
    // The value always fits: the generator's numbers have at most 2n digits.
    let _ = sdrkit_util_bits::bcd::encode(v % 10u64.pow(2 * n as u32), 2 * n, &mut b[..n]);
    d.extend(b[..n].iter().rev());
}

/// The link-layer header of a frame: the length octet (0 until [`set_length`]), the C field 0x44 (SND-NR), the manufacturer
/// code, the id, the version and the device type.
pub fn link_header(mfr_code: u16, id: u32, version: u8, dev_type: u8) -> Vec<u8> {
    let mut d = vec![0u8, 0x44, mfr_code as u8, (mfr_code >> 8) as u8];
    // The id is shown as its hex digits, so its BCD bytes are the bytes of the number.
    d.extend(id.to_le_bytes());
    d.extend([version, dev_type]);
    d
}

/// The physical mode and the frame format of a mode name ("S1", "T1", "C1 (A)", "C1 (B)", "R2", "N1").
pub fn phy_of(name: &str) -> (Phy, FrameFormat) {
    let phy = match name {
        "S1" => Phy::S1,
        "R2" => Phy::R2,
        "N1" => Phy::N1,
        _ => Phy::T1C1,
    };
    let fmt = if name == "C1 (B)" {
        FrameFormat::B
    } else {
        FrameFormat::A
    };
    (phy, fmt)
}

/// The length octet of frame bytes `d` (no CRCs) in format `fmt`: the frame without itself; format B counts the CRCs.
pub fn set_length(d: &mut [u8], fmt: FrameFormat) {
    let n = d.len();
    d[0] = match fmt {
        FrameFormat::B => (n - 1 + if n <= 126 { 2 } else { 4 }) as u8,
        FrameFormat::A => (n - 1) as u8,
    };
}

/// The clean samples at 0 Hz of frame bytes `data` (no CRCs) in the mode `name`: the CRCs of its format, the chips of
/// [`sim_chips`] and the modulation of [`chips_to_samples`].
pub fn frame_samples(name: &str, data: &[u8], rate: f64) -> Vec<Complex32> {
    let (phy, fmt) = phy_of(name);
    let air = add_crcs(data, fmt);
    chips_to_samples(&sim_chips(name, &air), phy.chip_rate(), phy.dev_hz(), rate)
}

/// The frame bytes (no CRCs) of meter `i` for transmission `n`. `rng` supplies the bytes of the encrypted payload.
pub fn sim_frame(i: usize, n: u64, rng: &mut Rng) -> Vec<u8> {
    let m = &SIM_METERS[i];
    let mut d = link_header(manufacturer_code(m.mfr), m.id, m.version, m.dev_type);
    d.extend([0x7A, n as u8, 0x00]);
    let water = |d: &mut Vec<u8>| {
        d.extend([0x0C, 0x13]);
        bcd_le(123456 + 3 * n, 4, d);
        d.extend([0x4C, 0x13]);
        bcd_le(120000, 4, d);
        d.extend([0x42, 0x6C, 0x21, 0x3A, 0x02, 0x3B]);
        d.extend(((40 + n % 17) as u16).to_le_bytes());
        d.extend([0x01, 0xFD, 0x17, 0x00]);
    };
    if i == 2 {
        d.extend([0x00, 0x05]);
        let mut payload = [0u8; 32];
        rng.fill_bytes(&mut payload);
        d.extend(payload);
    } else {
        d.extend([0x00, 0x00]);
        match i {
            0 | 6 => water(&mut d),
            1 => {
                d.extend([0x0C, 0x06]);
                bcd_le(8123456 + n, 4, &mut d);
                d.extend([0x0C, 0x13]);
                bcd_le(450000 + 7 * n, 4, &mut d);
                d.extend([0x0B, 0x2D]);
                bcd_le(342 + n % 9, 3, &mut d);
                d.extend([0x0A, 0x5A]);
                bcd_le(652 + n % 5, 2, &mut d);
                d.extend([0x0A, 0x5E]);
                bcd_le(412, 2, &mut d);
            }
            3 => {
                d.extend([0x0C, 0x15]);
                bcd_le(2345678 + n, 4, &mut d);
                d.extend([0x02, 0x6C, 0x21, 0x3A]);
            }
            4 => {
                water(&mut d);
                for k in 1..=24u64 {
                    d.extend([0xCC, (k & 0xF) as u8, 0x13]);
                    bcd_le(100000 + 100 * k, 4, &mut d);
                }
            }
            _ => {
                d.extend([0x0C, 0x6E]);
                bcd_le(1723 + n, 4, &mut d);
                d.extend([0x42, 0x6C, 0x21, 0x3A]);
            }
        }
    }
    set_length(&mut d, phy_of(m.phy).1);
    d
}

/// The chips (0 or 1) of a frame on air for `phy`: 20 preamble pairs, the sync word, for C the second sync, the coded bytes and
/// a tail of 4 pairs.
pub fn sim_chips(phy: &str, air: &[u8]) -> Vec<u8> {
    let mut c: Vec<u8> = Vec::new();
    for _ in 0..20 {
        c.extend([0, 1]);
    }
    let push16 = |c: &mut Vec<u8>, v: u16| (0..16).rev().for_each(|i| c.push(((v >> i) & 1) as u8));
    let sync = match phy {
        "S1" | "R2" => 0x7696,
        "N1" => 0xF68D,
        _ => 0x543D,
    };
    push16(&mut c, sync);
    match phy {
        "C1 (A)" => push16(&mut c, 0x54CD),
        "C1 (B)" => push16(&mut c, 0x543D),
        _ => {}
    }
    let mut bits = Vec::with_capacity(8);
    for &b in air {
        bits.clear();
        bits.extend((0..8).rev().map(|i| (b >> i) & 1));
        match phy {
            "S1" | "R2" => Manchester::encode(&bits, ManchesterConv::Thomas, &mut c),
            "T1" => {
                for nib in [b >> 4, b & 0xF] {
                    let w = three_of_six::encode(nib);
                    c.extend((0..6).rev().map(|i| (w >> i) & 1));
                }
            }
            _ => c.extend_from_slice(&bits),
        }
    }
    for _ in 0..4 {
        c.extend([0, 1]);
    }
    c
}

/// The clean samples of `chips` at 0 Hz: 2-FSK with a Gaussian frequency pulse (BT 0.5), `dev` Hz for a chip, unit amplitude,
/// with 4 chips of carrier after the frame.
pub fn chips_to_samples(chips: &[u8], chip_rate: f64, dev: f64, rate: f64) -> Vec<Complex32> {
    let Ok(mut m) = Cpm::with_deviation(Shape::GaussianFreq { bt: 0.5 }, 4.0, dev, chip_rate, rate)
    else {
        return Vec::new();
    };
    let mut levels: Vec<f32> = chips
        .iter()
        .map(|&c| if c != 0 { 1.0 } else { -1.0 })
        .collect();
    let last = *levels.last().unwrap_or(&-1.0);
    levels.extend([last; 4]);
    let mut out = Vec::new();
    m.push(&levels, &mut out);
    m.flush(&mut out);
    out
}

/// The test-signal options of the stack.
#[derive(Clone, Debug)]
pub struct GenParams {
    /// The 169 MHz plan (else 868 MHz).
    pub plan169: bool,
    /// Only this physical mode ("S1", "T1", "C1 (A)", "C1 (B)", "R2", "N1").
    pub only_phy: Option<&'static str>,
    /// The encrypted meter is on.
    pub encrypted: bool,
    /// No meters.
    pub silence: bool,
    /// The seed of the encrypted payloads.
    pub seed: u64,
}

/// The capture centre of the test signal.
pub fn gen_centre(plan169: bool) -> f64 {
    if plan169 {
        169.4375e6
    } else {
        868.625e6
    }
}

/// The `wmbus` generator: the meters in turn, 0.5 s apart from 0.1 s, at their channels around the capture centre
/// ([`gen_centre`]). `None` when the rate cannot hold the scene.
pub fn generator(rate: f64, p: GenParams) -> Option<CleanScene> {
    let centre = gen_centre(p.plan169);
    let meters: Vec<usize> = SIM_METERS
        .iter()
        .enumerate()
        .filter(|(i, m)| {
            (m.hz < 300e6) == p.plan169
                && (p.encrypted || *i != 2)
                && p.only_phy.is_none_or(|ph| ph == m.phy)
                && !p.silence
        })
        .map(|(i, _)| i)
        .collect();
    let mut rng = Rng::new(p.seed ^ 0x3B);
    let mut t = 0.1f64;
    let mut k = 0usize;
    let mut scene = Scene::new(rate).ok()?;
    scene.add_source(Box::new(move || {
        if meters.is_empty() {
            return None;
        }
        let i = meters[k % meters.len()];
        let n = (k / meters.len()) as u64;
        k += 1;
        let m = &SIM_METERS[i];
        let samples = frame_samples(m.phy, &sim_frame(i, n, &mut rng), rate);
        let start_s = t;
        t += samples.len() as f64 / rate + 0.5;
        Some(Burst {
            start_s,
            offset_hz: m.hz - centre,
            level_db: 0.0,
            samples,
        })
    }));
    Some(CleanScene::new(scene, rate, SignalInfo::new(250e3)))
}

/// The test-signal options from the stack's settings.
pub fn gen_params(cfg: &GenConfig) -> GenParams {
    let s = &cfg.settings;
    GenParams {
        plan169: s.get(&GEN_PLAN) == GenPlan::Mhz169,
        only_phy: match s.get(&GEN_PHY) {
            GenPhy::S1 => Some("S1"),
            GenPhy::T1 => Some("T1"),
            GenPhy::C1a => Some("C1 (A)"),
            GenPhy::C1b => Some("C1 (B)"),
            GenPhy::R2 => Some("R2"),
            GenPhy::N1 => Some("N1"),
            GenPhy::All => None,
        },
        encrypted: s.get(&GEN_ENCRYPTED),
        silence: s.get(&GEN_SILENCE),
        seed: cfg.seed,
    }
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let g = generator(cfg.rate_hz, gen_params(cfg)).ok_or(ConfigError::NoRate)?;
    Ok(Box::new(g))
}

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, 868 MHz meters", o, 0.0);
    c.freq_hz = Some(gen_centre(false));
    c.seconds = o.seconds.max(6.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = WmbusFields::read(r.fields());
        let n = |v: fn(&WmbusFields) -> i64| f.as_ref().map_or(0, v);
        vec![
            at_least_value("frames_ok", n(|f| f.frames_ok), 6),
            at_least_value("meters", n(|f| f.meters), 5),
            at_least_value("encrypted_frames", n(|f| f.encrypted_frames), 1),
        ]
    });
    vec![c]
}

/// The test-signal case of `wmbus`.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

/// Wireless M-Bus meters.
pub static WMBUS: StackDescriptor = StackDescriptor {
    id: "wmbus",
    name: "Wireless M-Bus meters",
    group: "Meters, 868 / 169 MHz",
    blurb: "Water, heat, gas and electricity meters (EN 13757-4 S, T, C, R2 and N): ids and unencrypted readings; encrypted payloads shown as such. R2 and N1 have no real-signal confirmation yet.",
    tuning: Tuning { min_hz: 150e6, max_hz: 880e6, no_offset_tuning: true, ..Tuning::anywhere(868.625e6, 2e6, MIN_RATE_HZ, 1.6e6) },
    submodes: &[],
    settings: WmbusSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::WirelessMbus],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};
