//! 802.11p V2X (`v2x`): 10 MHz OFDM frames of road vehicles and roadside units. The channel is resampled to 10 Msps and fed to
//! the 20 MHz receiver of `sdrkit-proto-wlan-phy` as if it were 20 Msps: every time doubles and the carrier spacing halves to
//! 156.25 kHz, which is clause 17 at half clock. Data frames are read for SAE J2735 basic safety messages and ETSI cooperative
//! awareness messages; their positions go on the map. Only the temporary id that a message carries is shown.
//! Standard: IEEE Std 802.11-2020 clause 17 (10 MHz channel spacing); SAE J2735; ETSI EN 302 637-2; IEEE 1609.3.

pub mod gen;
pub mod its;

use its::{scan_mpdu, Found};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_core::command::{CommandError, CommandId, CommandSet};
use crate::typed::{
    V2xFields, V2xFrameRow, V2xVehicleEvent, V2xVehicleRow, V2X_FRAMES, V2X_REMOVE, V2X_VEHICLE,
    V2X_VEHICLES, VEHICLE_ROW,
};
use sdrkit_core::interface::{Fields as _, Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, Complex32, ConfigError, EventDef, GenConfig, Generator, LockState, Receiver, Record,
    ResultsDecl, StackConfig, StateBlob, StateError, Values,
};
use sdrkit_graph::{
    no_values, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kind,
    Outbox, Port, RateFrom, StackGraph, Timed,
};
use sdrkit_proto_wlan_phy::ofdm::OfdmReceiver;
use sdrkit_util_geo::{Position, KNOT_M_S};
use sdrkit_util_objects::commands::{self, Tables};
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "v2x";
/// The rate of the 10 MHz channel, Hz.
pub const WORK_RATE: f64 = 10e6;
/// The rate that the 20 MHz receiver is built for: the channel is fed to it as if it had this rate.
pub const PHY_RATE: f64 = 20e6;
/// The least input rate, Hz.
pub const MIN_RATE: f64 = 10e6;
/// The width kept flat around the channel: the 52 carriers span 8.125 MHz.
pub const PASS_HZ: f64 = 8e6;
/// The most vehicles kept.
pub const VEHICLE_CAP: usize = 256;
/// The most frames in the log.
pub const FRAME_CAP: usize = 300;
/// A vehicle that sends nothing for this long leaves the table, s.
pub const EXPIRY_S: f64 = 300.0;
/// A safety message within this time keeps the stack locked, s.
pub const LOCK_HOLD_S: f64 = 10.0;
/// What no public document confirms.
pub const NOTE: &str = "Not confirmed by a public document: the BSM speed scale of 0.02 m/s and 8191 as unavailable; the \
     3-bit transmission state without an extension bit; the split of the first two octets of the message frame into an \
     extension bit and a message id (the two octets 00 14 are matched); the CAM layout up to the position; the WSMP and \
     GeoNetworking headers are not parsed, so a message is found by its first octets and a chance match is possible.";

sdrkit_core::choice_enum! {
    /// What a row of the frame log is.
    #[derive(Serialize, Deserialize)]
    pub enum FrameKind {
        /// A J2735 basic safety message.
        Bsm = ("bsm", "BSM"),
        /// An ETSI cooperative awareness message.
        Cam = ("cam", "CAM"),
        /// An ETSI decentralised environmental notification, counted.
        Denm = ("denm", "DENM"),
        /// A WSMP frame with no BSM.
        Wsmp = ("wsmp", "WSMP"),
        /// A GeoNetworking frame with no CAM.
        GeoNet = ("geonetworking", "GeoNetworking"),
    }
}

/// A row of the frame log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRow {
    /// Signal time, s.
    pub t: f64,
    /// What the frame held.
    pub kind: FrameKind,
    /// The temporary id of the message, empty for none.
    pub id: String,
    /// What it said.
    pub text: String,
    /// The level, dBFS.
    pub level_dbfs: f64,
}

impl Shift for FrameRow {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// A vehicle or roadside unit, keyed by the kind of message and its temporary id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vehicle {
    /// BSM or CAM.
    pub kind: FrameKind,
    /// The temporary id: 8 hex digits of a BSM, the decimal stationID of a CAM.
    pub id: String,
    /// The last position.
    pub position: Option<Position>,
    /// The last speed, m/s.
    pub speed_m_s: Option<f64>,
    /// The last heading, degrees.
    pub heading_deg: Option<f64>,
    /// The last text.
    pub text: String,
    /// Messages received.
    pub frames: u64,
    /// The level of the last message, dBFS.
    pub level_dbfs: f64,
    /// The event of the new vehicle was sent.
    #[serde(skip)]
    announced: bool,
}

impl Shift for Vehicle {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// What the stack keeps and saves.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Store {
    /// The vehicles, keyed by kind and id.
    pub vehicles: ObjectTable<String, Vehicle>,
    /// The frame log.
    pub frames: LogRing<FrameRow>,
    /// Safety messages (BSM and CAM).
    pub safety: u64,
    /// DENM frames.
    pub denm: u64,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            vehicles: ObjectTable::new(VEHICLE_CAP),
            frames: LogRing::new(FRAME_CAP),
            safety: 0,
            denm: 0,
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.vehicles.shift_times(by_s);
        self.frames.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("road.v2x", 1);
    fn clear(&mut self) {
        self.vehicles.clear();
        self.frames.clear();
        self.safety = 0;
        self.denm = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.vehicles.adopt(saved.vehicles);
        self.frames.adopt(saved.frames);
        self.safety = saved.safety;
        self.denm = saved.denm;
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

impl Shaped for Vehicle {
    type Row = V2xVehicleRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&V2xVehicleRow {
            kind: self.kind,
            source: &self.id,
            position: self.position.map(|p| (p.lat_deg, p.lon_deg)),
            speed_kt: self.speed_m_s.map(|v| v / KNOT_M_S),
            course_deg: self.heading_deg,
            text: &self.text,
            frames: i64::try_from(self.frames).unwrap_or(i64::MAX),
            level_dbfs: self.level_dbfs,
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
    fn point(&self) -> Option<Point<'_>> {
        let p = Point::new(self.position?, format!("{} {}", self.kind.label(), self.id));
        Some(match self.heading_deg {
            Some(h) => p.heading(h as f32),
            None => p,
        })
    }
}

impl Shaped for FrameRow {
    type Row = V2xFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&V2xFrameRow {
            time_s: self.t,
            kind: self.kind,
            source: &self.id,
            text: &self.text,
            level_dbfs: self.level_dbfs,
        });
    }
}

/// A new vehicle as an event item.
#[derive(Clone, Debug, PartialEq)]
pub struct VehicleItem {
    /// The vehicle.
    pub vehicle: Vehicle,
}

impl Item for VehicleItem {
    const KIND: &'static str = "v2x.vehicle";
    fn event_def() -> Option<&'static EventDef> {
        Some(V2X_VEHICLE.def())
    }
    fn write_fields(&self, f: &mut Record) {
        let v = &self.vehicle;
        V2xVehicleEvent {
            kind: v.kind,
            source: &v.id,
            position: v.position.map(|p| (p.lat_deg, p.lon_deg)),
            text: &v.text,
            family: Family::Message,
        }
        .write(f);
    }
}

/// One message as the decoder found it in a frame.
struct Heard {
    t: f64,
    found: Found,
    level_dbfs: f64,
    snr_db: f64,
    offset_hz: f64,
}

/// The `road.v2x` decoder: channel, OFDM receiver, frame reader and tables.
pub struct V2xDecoder {
    ch: Option<Channelizer>,
    buf: Vec<Complex32>,
    rx: OfdmReceiver,
    /// Signal time of the receiver's first sample, s.
    t0: f64,
    /// Input samples so far.
    input: u64,
    rate: f64,
    heard: Vec<Heard>,
    store: Store,
    out: Vec<Timed<VehicleItem>>,
    last_safety_t: Option<f64>,
    last: Option<(f64, f64, f64)>,
    stats_base: (u64, u64, u64),
    started: bool,
    dirty: bool,
    cleared: bool,
    summary: String,
}

impl Default for V2xDecoder {
    fn default() -> Self {
        V2xDecoder {
            ch: None,
            buf: Vec::with_capacity(1 << 16),
            rx: OfdmReceiver::new(),
            t0: 0.0,
            input: 0,
            rate: 0.0,
            heard: Vec::new(),
            store: Store::default(),
            out: Vec::new(),
            last_safety_t: None,
            last: None,
            stats_base: (0, 0, 0),
            started: false,
            dirty: true,
            cleared: false,
            summary: String::new(),
        }
    }
}

impl V2xDecoder {
    fn now(&self) -> f64 {
        self.input as f64 / self.rate.max(1.0)
    }

    fn build(&mut self, f: &Format) -> Result<(), ConfigError> {
        let mut cfg = ChannelConfig::new(f.rate, f.signal_offset_hz(), PASS_HZ, WORK_RATE);
        cfg.exact_rate = true;
        self.ch = Some(Channelizer::new(cfg).map_err(|_| ConfigError::RateTooLow {
            need: MIN_RATE,
            got: f.rate,
        })?);
        let s = self.rx.stats();
        self.stats_base = (
            self.stats_base.0 + s.frames,
            self.stats_base.1 + s.bad_fcs,
            self.stats_base.2 + s.preambles,
        );
        self.rx = OfdmReceiver::new();
        self.t0 = self.now();
        Ok(())
    }

    fn take(&mut self, h: Heard) {
        self.last = Some((h.level_dbfs, h.snr_db, h.offset_hz));
        self.dirty = true;
        let (kind, id, text, position, speed, heading) = match &h.found {
            Found::Bsm(b) => (
                FrameKind::Bsm,
                format!("{:08X}", b.id),
                b.text(),
                b.position,
                b.speed_m_s,
                b.heading_deg,
            ),
            Found::Cam(c) => (
                FrameKind::Cam,
                c.station_id.to_string(),
                c.text(),
                c.position,
                None,
                None,
            ),
            Found::Denm => {
                self.store.denm += 1;
                (
                    FrameKind::Denm,
                    String::new(),
                    "hazard warning (counted, not decoded)".to_string(),
                    None,
                    None,
                    None,
                )
            }
            Found::Wsmp(n) => (
                FrameKind::Wsmp,
                String::new(),
                format!("WSMP, {n} bytes, no BSM found"),
                None,
                None,
                None,
            ),
            Found::GeoNet(n) => (
                FrameKind::GeoNet,
                String::new(),
                format!("GeoNetworking, {n} bytes, no CAM found"),
                None,
                None,
                None,
            ),
        };
        self.store.frames.push(FrameRow {
            t: h.t,
            kind,
            id: id.clone(),
            text: text.clone(),
            level_dbfs: h.level_dbfs,
        });
        if !matches!(kind, FrameKind::Bsm | FrameKind::Cam) {
            return;
        }
        self.store.safety += 1;
        self.last_safety_t = Some(h.t);
        let key = format!("{}/{id}", kind.key());
        let Ok(up) = self
            .store
            .vehicles
            .upsert(&key, Stamp::at(h.t), || Vehicle {
                kind,
                id: id.clone(),
                position: None,
                speed_m_s: None,
                heading_deg: None,
                text: String::new(),
                frames: 0,
                level_dbfs: 0.0,
                announced: false,
            })
        else {
            return;
        };
        let v = up.row;
        v.frames += 1;
        v.text = text;
        v.level_dbfs = h.level_dbfs;
        v.speed_m_s = speed;
        v.heading_deg = heading;
        if let Some((lat, lon)) = position {
            v.position = Some(Position::new(lat, lon));
        }
        if !v.announced && v.position.is_some() {
            v.announced = true;
            self.out.push(Timed {
                time: h.t,
                value: VehicleItem { vehicle: v.clone() },
            });
        }
    }
}

impl Decoder for V2xDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "road.v2x"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<VehicleItem>("vehicles").rate_from(RateFrom::Fixed(0.0))]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate < MIN_RATE * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.input = 0;
        self.build(f)?;
        Ok(vec![Format::new(Kind::typed::<VehicleItem>(), 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        let _ = self.build(f);
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.input = first + x.len() as u64;
        let Some(ch) = self.ch.as_mut() else {
            return;
        };
        self.buf.clear();
        ch.process(x, &mut self.buf);
        let t0 = self.t0;
        let heard = &mut self.heard;
        self.rx.process(&self.buf, |f| {
            let Some(found) = scan_mpdu(f.mpdu) else {
                return;
            };
            heard.push(Heard {
                t: t0 + f.sample as f64 / WORK_RATE,
                found,
                level_dbfs: f.level_dbfs,
                snr_db: f.snr_db,
                // The receiver measures as if at 20 Msps: the offset at 10 Msps is half of it.
                offset_hz: f.offset_hz * WORK_RATE / PHY_RATE,
            });
        });
        let mut heard = std::mem::take(&mut self.heard);
        for h in heard.drain(..) {
            self.take(h);
        }
        self.heard = heard;
        out.items::<VehicleItem>(0).append(&mut self.out);
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.now();
        self.store.vehicles.expire(now, EXPIRY_S);
        let s = self.rx.stats();
        let as_count = |v: u64| i64::try_from(v).unwrap_or(i64::MAX);
        let good = self.stats_base.0 + s.frames;
        if self.cleared {
            self.cleared = false;
            p.results_mut().clear_log();
        }
        let n = self.store.safety;
        let lock = if !self.started {
            LockState::Off
        } else if self.last_safety_t.is_some_and(|t| now - t <= LOCK_HOLD_S) {
            LockState::Locked
        } else if good > 0 {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        self.summary.clear();
        self.summary.push_str(&match lock {
            LockState::Off => "V2X: waiting for samples".to_string(),
            LockState::Locked => format!("{n} safety messages"),
            LockState::Tracking => format!("{n} safety messages; 802.11p frames, none lately"),
            LockState::Searching => "V2X: listening for 802.11p frames".to_string(),
        });
        p.put(&V2xFields {
            frames_ok: as_count(good),
            frames_bad: as_count(self.stats_base.1 + s.bad_fcs),
            safety_messages: as_count(self.store.safety),
            denm: as_count(self.store.denm),
            vehicles: self.store.vehicles.len() as i64,
            preambles: as_count(self.stats_base.2 + s.preambles),
            level_dbfs: self.last.map(|l| l.0),
            snr_db: self.last.map(|l| l.1),
            offset_hz: self.last.map(|l| Hertz(l.2)),
            text: self.summary.as_str(),
            note: NOTE,
        });
        if std::mem::take(&mut self.dirty) {
            shaped::rows(&self.store.vehicles, &V2X_VEHICLES, now, p);
            shaped::log_rows(&self.store.frames, &V2X_FRAMES, now, p);
            shaped::points(&self.store.vehicles, "vehicle/", p.results_mut());
        }
        let level = self.last.map(|l| l.0 as f32);
        let summary = self.summary.clone();
        p.results_mut().update_status(|st| {
            let changed = st.lock_state != lock || st.level_dbfs != level || st.summary != summary;
            if changed {
                st.lock_state = lock;
                st.level_dbfs = level;
                st.summary.clone_from(&summary);
            }
            changed
        });
    }
    fn report_interval_s(&self) -> f64 {
        0.25
    }
    fn commands(&self) -> &'static [&'static sdrkit_core::command::Command] {
        V2xCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match V2xCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            V2xCommand::Remove => {
                let mut p = self.results();
                let table = p
                    .results()
                    .decl()
                    .table_id(V2X_VEHICLES.key)
                    .ok_or(CommandError::NoSuchId { id })?;
                commands::remove(&mut self.store.vehicles, table, &mut p, args, &VEHICLE_ROW)?;
                self.dirty = true;
            }
        }
        Ok(None)
    }
    fn clear(&mut self) {
        self.store.clear();
        self.last_safety_t = None;
        self.last = None;
        self.cleared = true;
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now())?;
        self.dirty = true;
        Ok(())
    }
}


sdrkit_core::commands! {
    /// The commands of the decoder beside `clear`.
    pub enum V2xCommand {
        /// Forgets one vehicle.
        Remove = V2X_REMOVE.command,
    }
}

/// The receiver: the decoder with the `clear` and `remove` commands.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("v2x", DecoderComponent::new(V2xDecoder::default()))
        .input("v2x")
        .output("v2x")
        .status_from("v2x")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE,
            "802.11p V2X needs a sample rate of at least 10 MHz",
        )
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command("remove", sdrkit_graph::CommandTarget::of("v2x", "remove"))
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

/// Makes the test signal.
///
/// # Errors
///
/// [`ConfigError::RateTooLow`] below 10 Msps.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::V2xGenerator::from_config(cfg)?))
}
