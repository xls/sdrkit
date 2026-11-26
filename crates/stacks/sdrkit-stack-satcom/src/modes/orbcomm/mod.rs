//! Orbcomm subscriber downlinks (`orbcomm`): up to two 137 MHz downlinks at 4800 bit/s SDPSK, with packets, satellite positions,
//! orbital elements, channel lists and messages.

pub mod demod;
pub mod gen;
pub mod packet;

use crate::display::{set_series, Clock};
use crate::typed::{
    OrbcommChannelArgs, OrbcommDecoderRow, OrbcommFields, OrbcommMessageEvent, OrbcommPacketRow,
    OrbcommPacketTypeRow, OrbcommSatelliteChannelRow, OrbcommSatelliteRow, ORBCOMM_CONSTELLATION,
    ORBCOMM_DECODERS, ORBCOMM_MESSAGE, ORBCOMM_PACKETS, ORBCOMM_PACKET_TYPES, ORBCOMM_PRESET_APPLY,
    ORBCOMM_REMOVE, ORBCOMM_SATELLITES, ORBCOMM_SATELLITE_CHANNELS, ORBCOMM_SATELLITE_ROW,
};
use demod::Decoder as Downlink;
use packet::{raster, Elements, Packet};
use sdrkit_algo_dsp::floor::BinMedian;
use sdrkit_algo_dsp::meter::power_to_db as db;
use sdrkit_algo_dsp::spectrum::{Scaling, SpectrumAnalyzer, SpectrumConfig};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::command::{CommandError, CommandId, CommandSet};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Choice, Command, Complex32, ConfigError, GenConfig, Generator, HostRequest,
    LockProfile, LockState, Receiver, Record, ResultsDecl, Schema, Setting, SettingError,
    SettingId, SettingSet, StackConfig, StackDescriptor, StateBlob, StateError, Tuning, Value,
    Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kind,
    Outbox, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_util_geo::Position;
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    JoinRules, Joined, Joiner, LogRing, Numbering, ObjectTable, Part, Seen, Shift, Spacing, Trail,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "orbcomm";
/// The stack name.
pub const NAME: &str = "Orbcomm";
pub const MIN_RATE: f64 = 100e3;
pub const REPORT_S: f64 = 0.25;
pub const LOG_CAP: usize = 500;
pub const SATELLITES_CAP: usize = 64;
/// The positions kept for each satellite's track.
pub const TRACK_CAP: usize = 120;
/// The default dial (the first downlink channel of the presets).
pub const DEFAULT_DIAL_HZ: f64 = 137.25e6;

sdrkit_core::choice_enum! {
    /// The state of a downlink decoder.
    pub enum DecoderState {
        /// Free.
        Free = ("free", "Free"),
        /// Acquiring a carrier.
        Acquiring = ("acquiring", "Acquiring"),
        /// Tracking the carrier.
        Tracking = ("tracking", "Tracking"),
        /// Frames in lock.
        FrameLock = ("frame_lock", "Frame lock"),
    }
}

impl DecoderState {
    /// The state of a decoder's state number (0 to 3).
    pub fn of(state: u8) -> DecoderState {
        DecoderState::from_index(u16::from(state.min(3))).unwrap_or(DecoderState::Free)
    }
}

/// One decoder's state at a tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecoderStatus {
    pub index: usize,
    pub active: bool,
    /// 0 free, 1 acquiring, 2 tracking, 3 frame lock.
    pub state: u8,
    /// The carrier, Hz from the input's 0 Hz.
    pub offset_hz: f64,
    /// The carrier's absolute frequency.
    pub frequency_hz: f64,
    pub snr_db: f64,
    pub level_db: f64,
    pub satellite: Option<u8>,
    pub clock_ppm: f64,
    pub frames: u64,
    pub good: u64,
    pub bad: u64,
    pub sync_losses: u64,
    pub constellation: Vec<[f32; 2]>,
}

/// A packet received, with its check.
#[derive(Clone, Debug)]
pub struct Received {
    pub decoder: usize,
    pub bytes: [u8; packet::PACKET],
    pub ok: bool,
    pub satellite: Option<u8>,
    /// The carrier's absolute frequency.
    pub carrier_hz: f64,
}

/// What the receiver passes on.
#[derive(Clone, Debug)]
pub enum OrbItem {
    /// A packet with its check.
    Raw(Received),
    /// The decoders at a clock tick.
    Status(Vec<DecoderStatus>),
}
impl Item for OrbItem {
    const KIND: &'static str = "satcom.orbcomm_item";
}

/// The receiver: carrier detection and up to two downlink decoders.
pub struct OrbRx {
    rate: f64,
    centre: f64,
    decoders_wanted: usize,
    decoders: Vec<Option<(Downlink, f64, u64)>>,
    blacklist: Vec<(f64, f64)>,
    spectrum: Option<SpectrumAnalyzer>,
    floor: Option<BinMedian>,
    acc: Vec<f32>,
    acc_frames: usize,
    p: Vec<f32>,
    sm: Vec<f32>,
    next_eval: u64,
    /// Input samples seen since the start.
    seen: u64,
    raw: Vec<demod::RawPacket>,
    /// Carriers found and not decoded: (offset Hz, Hz above the floor in dB).
    pub candidates: Vec<(f64, f64)>,
}

impl Default for OrbRx {
    fn default() -> Self {
        OrbRx {
            rate: 0.0,
            centre: 0.0,
            decoders_wanted: 2,
            decoders: Vec::new(),
            blacklist: Vec::new(),
            spectrum: None,
            floor: None,
            acc: Vec::new(),
            acc_frames: 0,
            p: Vec::new(),
            sm: Vec::new(),
            next_eval: 0,
            seen: 0,
            raw: Vec::new(),
            candidates: Vec::new(),
        }
    }
}

/// Spectra a second for the carrier detection.
const DETECT_FRAMES_PER_S: f64 = 16.0;

impl OrbRx {
    /// Detection: the spectrum smoothed over 5 kHz, carriers 4 dB over the floor, away from the band edges and the DC spike, at
    /// the centroid of their half-power region.
    fn detect(&mut self, now: f64) {
        let n = self.acc.len();
        if n == 0 {
            return;
        }
        let bin = self.rate / n as f64;
        let k = 1.0 / self.acc_frames.max(1) as f32;
        for (p, a) in self.p.iter_mut().zip(&self.acc) {
            *p = a * k;
        }
        let floor = self.floor.as_mut().expect("configured");
        // The radio's DC spike is filled with the noise.
        let dc = n / 2;
        let w = (1500.0 / bin).ceil() as usize;
        let fill = floor.estimate(self.p.iter().copied()).unwrap_or(0.0);
        for v in self
            .p
            .iter_mut()
            .take((dc + w).min(n))
            .skip(dc.saturating_sub(w))
        {
            *v = fill;
        }
        let kk = ((5e3 / bin) as usize).max(1) | 1;
        self.sm.iter_mut().for_each(|v| *v = 0.0);
        let mut run = 0.0f64;
        for i in 0..n {
            run += self.p[i] as f64;
            if i >= kk {
                run -= self.p[i - kk] as f64;
            }
            if i + 1 >= kk {
                self.sm[i + 1 - kk / 2 - 1] = (run / kk as f64) as f32;
            }
        }
        let sm = &self.sm;
        let fl = floor.estimate(sm.iter().copied()).unwrap_or(0.0).max(1e-30);
        let edge = 16e3;
        let f = |i: usize| (i as f64 - (n / 2) as f64) * bin;
        let mut cands: Vec<(f64, f64)> = Vec::new();
        let mut i = 1;
        while i + 1 < n {
            if sm[i] > fl * 2.5
                && sm[i] >= sm[i - 1]
                && sm[i] >= sm[i + 1]
                && f(i).abs() < self.rate / 2.0 - edge
            {
                let half = sm[i] * 0.5;
                let mut a = i;
                while a > 0 && sm[a - 1] > half {
                    a -= 1;
                }
                let mut b = i;
                while b + 1 < n && sm[b + 1] > half {
                    b += 1;
                }
                let (mut num, mut den) = (0.0, 0.0);
                for j in a..=b {
                    let e = (sm[j] - fl).max(0.0) as f64;
                    num += e * f(j);
                    den += e;
                }
                cands.push((num / den.max(1e-30), db(sm[i] as f64 / fl as f64)));
                i = b + 1;
            } else {
                i += 1;
            }
        }
        cands.sort_by(|a, b| b.1.total_cmp(&a.1));
        self.blacklist.retain(|b| b.1 > now);
        let mut taken: Vec<f64> = self.decoders.iter().flatten().map(|d| d.0.centre).collect();
        let mut free: Vec<(f64, f64)> = Vec::new();
        for c in cands {
            if taken
                .iter()
                .chain(free.iter().map(|x| &x.0))
                .any(|t| (t - c.0).abs() < 9e3)
            {
                continue;
            }
            if self.blacklist.iter().any(|b| (b.0 - c.0).abs() < 3e3) {
                continue;
            }
            free.push(c);
        }
        self.candidates.clone_from(&free);
        for slot in 0..self.decoders_wanted.min(2) {
            if self.decoders[slot].is_none() && !free.is_empty() {
                let (fc, _) = free.remove(0);
                // The SDPSK spectrum's centroid lies a quarter of the rate above the carrier.
                let c = fc - demod::BAUD / 4.0;
                if let Ok(mut d) = Downlink::new(self.rate, c) {
                    d.t0 = now;
                    self.decoders[slot] = Some((d, now, 0));
                    taken.push(c);
                }
            }
        }
    }

    /// Decoders that hear no frames are let go, and their carrier is passed over for 30 s.
    fn drop_silent(&mut self, now: f64) {
        for d in self.decoders.iter_mut() {
            let drop = match d {
                Some((dec, start, last_frames)) => {
                    let fresh = dec.frames > *last_frames;
                    *last_frames = dec.frames;
                    if fresh {
                        *start = now;
                    }
                    now - *start > if dec.frames > 0 { 8.0 } else { 6.0 }
                }
                None => false,
            };
            if drop {
                if let Some((dec, _, _)) = d.take() {
                    self.blacklist
                        .push((dec.centre + demod::BAUD / 4.0, now + 30.0));
                }
            }
        }
    }
}

sdrkit_core::choice_enum! {
    /// How many downlinks are decoded at once.
    pub enum DecoderCount {
        /// One.
        One = ("1", "1"),
        /// Two.
        Two = ("2", "2"),
    }
}

/// How many downlinks are decoded at once.
pub const DECODERS: Setting<field::Choice<DecoderCount>> =
    Setting::choice("decoders", "Decoders", DecoderCount::Two).help("Downlinks decoded at once.");

sdrkit_core::settings! {
    /// The settings of [`OrbRx`].
    pub enum OrbRxSetting {
        /// Decoders.
        Decoders = DECODERS,
    }
}

impl Decoder for OrbRx {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.orbcomm_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<OrbItem>("out").rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        OrbRxSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match OrbRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            OrbRxSetting::Decoders => {
                self.decoders_wanted = match DECODERS.read(id, v)? {
                    DecoderCount::One => 1,
                    DecoderCount::Two => 2,
                };
                if self.decoders_wanted < 2 && self.decoders.len() == 2 {
                    self.decoders[1] = None;
                }
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        let n = if self.decoders_wanted < 2 {
            DecoderCount::One
        } else {
            DecoderCount::Two
        };
        values_of([(OrbRxSetting::Decoders, Value::Choice(n.index()))])
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate.is_nan() || f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        let n = ((f.rate / 40.0) as usize)
            .next_power_of_two()
            .clamp(1024, 65_536);
        let mut cfg = SpectrumConfig::new(n, f.rate);
        cfg.window = Window::Hann;
        cfg.scaling = Scaling::Power;
        cfg.frames_per_second = DETECT_FRAMES_PER_S;
        let spectrum = SpectrumAnalyzer::new(cfg)?;
        let mut floor = BinMedian::new(DETECT_FRAMES_PER_S as u32)?;
        floor.reserve(n);
        self.rate = f.rate;
        self.centre = f.centre_hz;
        self.spectrum = Some(spectrum);
        self.floor = Some(floor);
        self.acc = vec![0.0; n];
        self.p = vec![0.0; n];
        self.sm = vec![0.0; n];
        self.acc_frames = 0;
        self.decoders = (0..2).map(|_| None).collect();
        self.blacklist.clear();
        self.next_eval = f.rate as u64;
        self.seen = 0;
        Ok(vec![Format::new(Kind::typed::<OrbItem>(), 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        let _ = self.configure(f);
    }
    fn process(&mut self, iq: &[Complex32], first: u64, out: &mut Outbox) {
        let Some(hop) = self
            .spectrum
            .as_ref()
            .map(|s| s.hop().floor().max(1.0) as usize)
        else {
            return;
        };
        self.seen = first + iq.len() as u64;
        let out = out.items::<OrbItem>(0);
        // An evaluation once a second of input, at an absolute sample, so any cut produces the same result.
        let mut done = 0usize;
        while done < iq.len() {
            let at = first + done as u64;
            let until = (self.next_eval.saturating_sub(at) as usize)
                .min(iq.len() - done)
                .max(1);
            let piece = &iq[done..done + until];
            let s = self.spectrum.as_mut().expect("configured");
            for part in piece.chunks(hop) {
                // A part of at most one hop completes at most one frame.
                if s.push(part) > 0 {
                    for (a, p) in self.acc.iter_mut().zip(s.power()) {
                        *a += *p;
                    }
                    self.acc_frames += 1;
                }
            }
            for (slot, d) in self.decoders.iter_mut().enumerate() {
                if let Some((dec, _, _)) = d {
                    self.raw.clear();
                    dec.process(piece, &mut self.raw);
                    let carrier = self.centre + dec.carrier();
                    for r in self.raw.drain(..) {
                        out.push(Timed {
                            time: dec.t0 + r.time,
                            value: OrbItem::Raw(Received {
                                decoder: slot,
                                bytes: r.bytes,
                                ok: r.ok,
                                satellite: r.satellite,
                                carrier_hz: carrier,
                            }),
                        });
                    }
                }
            }
            done += until;
            if first + done as u64 >= self.next_eval {
                self.next_eval += self.rate as u64;
                let now = (first + done as u64) as f64 / self.rate;
                self.drop_silent(now);
                self.detect(now);
                self.acc.iter_mut().for_each(|v| *v = 0.0);
                self.acc_frames = 0;
            }
        }
    }
    fn tick(&mut self, out: &mut Outbox) {
        let st: Vec<DecoderStatus> = self
            .decoders
            .iter_mut()
            .enumerate()
            .map(|(i, d)| match d {
                Some((dec, _, _)) => DecoderStatus {
                    index: i,
                    active: true,
                    state: dec.state,
                    offset_hz: dec.carrier(),
                    frequency_hz: self.centre + dec.carrier(),
                    snr_db: dec.snr_db,
                    level_db: dec.level_db,
                    satellite: dec.satellite,
                    clock_ppm: dec.clock_ppm(),
                    frames: dec.frames,
                    good: dec.good,
                    bad: dec.bad,
                    sync_losses: dec.sync_losses,
                    constellation: std::mem::take(&mut dec.constellation),
                },
                None => DecoderStatus {
                    index: i,
                    ..Default::default()
                },
            })
            .collect();
        let time = self.seen as f64 / self.rate.max(1.0);
        out.items::<OrbItem>(0).push(Timed {
            time,
            value: OrbItem::Status(st),
        });
    }
}

// The results.

sdrkit_core::choice_enum! {
    /// The type of a packet, or a message joined from its parts.
    #[derive(Serialize, Deserialize)]
    pub enum PacketType {
        /// Synchronisation.
        Sync = ("sync", "Sync"),
        /// A part of a message.
        Message = ("message", "Message"),
        /// Uplink channels.
        Uplink = ("uplink", "Uplink"),
        /// Downlink channels.
        Downlink = ("downlink", "Downlink"),
        /// Network control.
        Network = ("network", "Network"),
        /// Fill.
        Fill = ("fill", "Fill"),
        /// Ephemeris.
        Ephemeris = ("ephemeris", "Ephemeris"),
        /// Orbital elements.
        Elements = ("elements", "Elements"),
        /// Another type.
        Other = ("other", "Other"),
        /// A message joined from its parts.
        Joined = ("message_joined", "Message joined"),
    }
}

impl PacketType {
    /// The type of a packet.
    pub fn of(p: &Packet) -> PacketType {
        match p {
            Packet::Sync { .. } => PacketType::Sync,
            Packet::Message { .. } => PacketType::Message,
            Packet::Uplink { .. } => PacketType::Uplink,
            Packet::Downlink { .. } => PacketType::Downlink,
            Packet::Network { .. } => PacketType::Network,
            Packet::Fill => PacketType::Fill,
            Packet::Ephemeris(_) => PacketType::Ephemeris,
            Packet::Elements(_) => PacketType::Elements,
            Packet::Other { .. } => PacketType::Other,
        }
    }
}

/// What the display publishes.
fn display_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        let c = ORBCOMM_CONSTELLATION;
        Arc::new(
            ResultsDecl::new()
                .with_fields(OrbcommFields::SHAPE.fields)
                .with_table(&ORBCOMM_SATELLITES)
                .with_table(&ORBCOMM_SATELLITE_CHANNELS)
                .with_table(&ORBCOMM_DECODERS)
                .with_table(&ORBCOMM_PACKETS)
                .with_table(&ORBCOMM_PACKET_TYPES)
                .series(c.key, c.label, c.x_unit, c.y_unit)
                .points("Satellites"),
        )
    })
    .clone()
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// A satellite row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SatRow {
    pub id: u8,
    pub frequency_hz: Option<f64>,
    pub doppler_hz: Option<f64>,
    pub snr_db: Option<f64>,
    pub frames: u64,
    pub bad: u64,
    /// The last position from the ephemeris (on WGS-84).
    pub position: Option<Position>,
    /// The satellite's clock (seconds in the hour) of that position.
    pub position_time: Option<u32>,
    pub track: Trail,
    pub elements: Option<Elements>,
    pub downlinks: Vec<f64>,
    pub uplinks: Vec<f64>,
}

impl SatRow {
    fn new(id: u8) -> Self {
        SatRow {
            id,
            frequency_hz: None,
            doppler_hz: None,
            snr_db: None,
            frames: 0,
            bad: 0,
            position: None,
            position_time: None,
            track: Trail::new(TRACK_CAP, Spacing::seconds(0.0)),
            elements: None,
            downlinks: Vec::new(),
            uplinks: Vec::new(),
        }
    }
}

impl Shift for SatRow {
    fn shift_times(&mut self, by_s: f64) {
        self.track.shift_times(by_s);
    }
}

impl Shaped for SatRow {
    type Row = OrbcommSatelliteRow;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&OrbcommSatelliteRow {
            satellite: i64::from(self.id),
            frequency: self.frequency_hz.map(Hertz),
            doppler: self.doppler_hz.map(Hertz),
            snr_db: self.snr_db,
            frames: self.frames as i64,
            heard: columns::heard(seen),
            bad: self.bad as i64,
            position: self.position.map(|p| (p.lat_deg, p.lon_deg)),
            height_m: self.position.map(|p| p.alt_m),
            inclination_deg: self.elements.as_ref().map(|e| e.inclination),
            period: self.elements.as_ref().map(|e| Seconds(e.period_min * 60.0)),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
    fn point(&self) -> Option<Point<'_>> {
        self.position
            .map(|p| Point::new(p, format!("Orbcomm {}", self.id)))
    }
}

/// A packet log line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogLine {
    /// Signal time.
    pub time: f64,
    /// Wall-clock time, when known.
    pub wall: Option<f64>,
    /// The satellite, when known.
    pub satellite: Option<u8>,
    /// The decoder.
    pub decoder: usize,
    /// The type of the packet.
    pub kind: PacketType,
    /// The bytes of the packet (none for a joined message).
    pub bytes: Arc<[u8]>,
    /// The text.
    pub text: String,
}

impl Shift for LogLine {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for LogLine {
    type Row = OrbcommPacketRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&OrbcommPacketRow {
            at: Seconds(self.time),
            wall: self.wall.map(Time),
            satellite: self.satellite.map(i64::from),
            decoder: self.decoder as i64,
            r#type: self.kind,
            bytes: (!self.bytes.is_empty()).then(|| self.bytes.clone()),
            text: &self.text,
        });
    }
}

/// What the display keeps (saved state).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrbTables {
    pub satellites: ObjectTable<u8, SatRow>,
    pub log: LogRing<LogLine>,
    /// Packets by type, in the order of [`PacketType`].
    pub counts: [u64; 9],
    pub joined: u64,
    pub good: u64,
    pub bad: u64,
}

impl Default for OrbTables {
    fn default() -> Self {
        OrbTables {
            satellites: ObjectTable::new(SATELLITES_CAP),
            log: LogRing::new(LOG_CAP),
            counts: [0; 9],
            joined: 0,
            good: 0,
            bad: 0,
        }
    }
}

impl Shift for OrbTables {
    fn shift_times(&mut self, by_s: f64) {
        self.satellites.shift_times(by_s);
        self.log.shift_times(by_s);
    }
}

impl Tables for OrbTables {
    const FORMAT: sdrkit_util_objects::Format =
        sdrkit_util_objects::Format::new("satcom.orbcomm", 1);
    fn clear(&mut self) {
        self.satellites.clear();
        self.log.clear();
        self.counts = [0; 9];
        self.joined = 0;
        self.good = 0;
        self.bad = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.satellites.adopt(saved.satellites);
        self.log.adopt(saved.log);
        self.counts = saved.counts;
        self.joined = saved.joined;
        self.good = saved.good;
        self.bad = saved.bad;
    }
}

sdrkit_core::choice_enum! {
    /// The channel presets: the dial the host is requested to tune to.
    pub enum Channel {
        /// 137.2500 MHz.
        C1372500 = ("137.2500", "137.2500 MHz"),
        /// 137.2875 MHz.
        C1372875 = ("137.2875", "137.2875 MHz"),
        /// 137.3125 MHz.
        C1373125 = ("137.3125", "137.3125 MHz"),
        /// 137.4625 MHz.
        C1374625 = ("137.4625", "137.4625 MHz"),
        /// 137.6625 MHz.
        C1376625 = ("137.6625", "137.6625 MHz"),
        /// 137.7375 MHz.
        C1377375 = ("137.7375", "137.7375 MHz"),
        /// 137.5 MHz, the band.
        Band = ("137.5", "137.5 MHz (band)"),
    }
}

/// The dial of a channel preset, Hz.
pub fn channel_hz(c: Channel) -> f64 {
    match c {
        Channel::C1372500 => 137.2500e6,
        Channel::C1372875 => 137.2875e6,
        Channel::C1373125 => 137.3125e6,
        Channel::C1374625 => 137.4625e6,
        Channel::C1376625 => 137.6625e6,
        Channel::C1377375 => 137.7375e6,
        Channel::Band => 137.5e6,
    }
}

/// The argument of `preset.apply`: the channel preset.
pub const CHANNEL: Setting<field::Choice<Channel>> =
    Setting::choice("channel", "Channel", Channel::C1372500);

sdrkit_core::commands! {
    /// The commands of [`OrbDisplay`].
    pub enum OrbDisplayCommand {
        /// Tunes to a channel preset.
        PresetApply = ORBCOMM_PRESET_APPLY.command,
        /// Forgets one satellite.
        Remove = ORBCOMM_REMOVE.command,
    }
}
/// The map follows the selected satellite.
pub const FOLLOW: Setting<field::Bool> = Setting::new("follow", "Follow satellite", "", false)
    .help("The map follows the selected satellite.");

sdrkit_core::settings! {
    /// The settings of [`OrbDisplay`].
    pub enum OrbDisplaySetting {
        /// Follow satellite.
        Follow = FOLLOW,
    }
}

/// The display: satellites, decoders, the packet log, message joining, events and host requests.
pub struct OrbDisplay {
    clock: Clock,
    live: bool,
    host_clock: sdrkit_core::Clock,
    tables: OrbTables,
    joiner: Joiner<u8, String>,
    joined: Vec<Joined<u8, String>>,
    decoders: Vec<DecoderStatus>,
    follow: bool,
    retune: Option<Channel>,
    /// The tables changed since the last report.
    dirty: bool,
}

impl Default for OrbDisplay {
    fn default() -> Self {
        OrbDisplay {
            clock: Clock::default(),
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            tables: OrbTables::default(),
            joiner: Joiner::new(JoinRules {
                timeout_s: 120.0,
                max_open: 32,
                max_len: 1024,
            }),
            joined: Vec::new(),
            decoders: Vec::new(),
            follow: false,
            retune: None,
            dirty: true,
        }
    }
}

impl OrbDisplay {
    fn log(
        &mut self,
        t: f64,
        sat: Option<u8>,
        decoder: usize,
        kind: PacketType,
        bytes: &[u8],
        text: String,
    ) {
        self.tables.log.push(LogLine {
            time: t,
            wall: self.clock.wall(t),
            satellite: sat,
            decoder,
            kind,
            bytes: Arc::from(bytes),
            text,
        });
    }

    fn packet(&mut self, t: f64, r: &Received, events: &mut Vec<sdrkit_core::Event>) {
        let Received {
            decoder,
            ref bytes,
            ok,
            satellite,
            carrier_hz,
        } = *r;
        self.dirty = true;
        if !ok {
            self.tables.bad += 1;
            if let Some(s) = satellite.and_then(|s| self.tables.satellites.get_mut(&s)) {
                s.bad += 1;
            }
            return;
        }
        self.tables.good += 1;
        let pk = packet::parse(bytes);
        let ty = PacketType::of(&pk);
        if let Some(c) = self.tables.counts.get_mut(usize::from(ty.index())) {
            *c += 1;
        }
        let abs = carrier_hz;
        let sat_id = match pk {
            Packet::Sync { satellite, .. } => Some(satellite),
            _ => satellite,
        };
        if let Some(id) = sat_id {
            let stamp = self.clock.stamp(t);
            if let Ok(u) = self
                .tables
                .satellites
                .upsert(&id, stamp, || SatRow::new(id))
            {
                let s = u.row;
                s.frequency_hz = Some(abs);
                s.doppler_hz = Some(abs - raster(abs));
                match &pk {
                    Packet::Sync { .. } => s.frames += 1,
                    Packet::Ephemeris(e) => {
                        if let Some(p) = e.position() {
                            s.position = Some(p);
                            s.position_time = Some(e.seconds);
                            let _ = s.track.push(t, p, ());
                        }
                    }
                    Packet::Elements(e) => s.elements = Some(e.clone()),
                    Packet::Downlink { channels } => s.downlinks.clone_from(channels),
                    Packet::Uplink { channels } => s.uplinks.clone_from(channels),
                    _ => {}
                }
            }
        }
        match &pk {
            Packet::Sync { .. } | Packet::Fill => {}
            Packet::Message {
                id,
                part,
                last,
                text,
            } => {
                self.log(
                    t,
                    sat_id,
                    decoder,
                    PacketType::Message,
                    bytes,
                    pk.describe(),
                );
                let key = sat_id.unwrap_or(0);
                self.joiner.push(
                    key,
                    Part {
                        t,
                        message: *id as u32,
                        number: Numbering::Seq(*part as u32),
                        last: *last,
                        body: text.clone(),
                    },
                    &mut self.joined,
                );
                for j in std::mem::take(&mut self.joined) {
                    self.message(j, *id, decoder, abs, events);
                }
            }
            _ => self.log(t, sat_id, decoder, ty, bytes, pk.describe()),
        }
    }

    /// A message joined from its parts: logged and sent as an event.
    fn message(
        &mut self,
        j: Joined<u8, String>,
        id: u8,
        decoder: usize,
        freq: f64,
        events: &mut Vec<sdrkit_core::Event>,
    ) {
        let whole = j.body.trim_end().to_string();
        self.tables.joined += 1;
        let t = j.last_t;
        self.log(
            t,
            Some(j.key),
            decoder,
            PacketType::Joined,
            &[],
            whole.clone(),
        );
        let mut source = String::new();
        let _ = write!(source, "satellite {}", j.key);
        events.push(ORBCOMM_MESSAGE.event(
            t,
            freq,
            &OrbcommMessageEvent {
                source: &source,
                text: &whole,
                family: Family::Satellite,
                message_id: i64::from(id),
                complete: j.complete,
            },
        ));
    }

    fn publish(&mut self, p: &mut Published) {
        let now = self.clock.now();
        let any_lock = self.decoders.iter().any(|d| d.state == 3);
        let any = self.decoders.iter().any(|d| d.active);
        let first = self.decoders.iter().find(|d| d.active);
        let mut summary = String::new();
        let _ = write!(
            summary,
            "Orbcomm packets ok {} bad {}  satellites {}",
            self.tables.good,
            self.tables.bad,
            self.tables.satellites.len()
        );
        for d in self.decoders.iter().filter(|d| d.active) {
            let f = d.frequency_hz;
            let _ = write!(summary, "  [{}] {:.4} MHz", d.index, f / 1e6);
            if let Some(s) = d.satellite {
                let _ = write!(summary, " sat {s}");
            }
            let _ = write!(
                summary,
                " dop {:+.0} Hz {:.1} dB{}",
                f - raster(f),
                d.snr_db,
                if d.state == 3 { " lock" } else { "" }
            );
        }
        let lock_state = if any_lock {
            LockState::Locked
        } else if any {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        let offset = first.map(|d| {
            let f = d.frequency_hz;
            f - raster(f)
        });
        let snr = first.map(|d| d.snr_db as f32);
        p.results_mut().update_status(|st| {
            let same = st.lock_state == lock_state
                && st.offset_hz == offset
                && st.snr_db == snr
                && st.summary == summary;
            if !same {
                st.lock_state = lock_state;
                st.offset_hz = offset;
                st.snr_db = snr;
                st.summary = summary;
            }
            !same
        });
        p.put(&OrbcommFields {
            frames_ok: self.tables.good as i64,
            frames_bad: self.tables.bad as i64,
            messages_joined: self.tables.joined as i64,
            satellites: self.tables.satellites.len() as i64,
        });
        let mut key = String::new();
        if std::mem::take(&mut self.dirty) {
            shaped::rows(&self.tables.satellites, &ORBCOMM_SATELLITES, now, p);
            shaped::row_points(
                &self.tables.satellites,
                &ORBCOMM_SATELLITES,
                "orbcomm/",
                p.results_mut(),
            );
            shaped::log_rows(&self.tables.log, &ORBCOMM_PACKETS, now, p);
            p.clear_rows(&ORBCOMM_SATELLITE_CHANNELS);
            for r in self.tables.satellites.iter() {
                let s = r.record;
                for (up, list) in [(false, &s.downlinks), (true, &s.uplinks)] {
                    for (i, f) in list.iter().enumerate() {
                        key.clear();
                        let _ = write!(key, "{}/{}{i}", s.id, if up { 'u' } else { 'd' });
                        ORBCOMM_SATELLITE_CHANNELS.put(
                            p,
                            &key,
                            &OrbcommSatelliteChannelRow {
                                satellite: i64::from(s.id),
                                uplink: up,
                                frequency: Hertz(*f),
                            },
                        );
                    }
                }
            }
            for (i, n) in self.tables.counts.iter().enumerate() {
                let Some(ty) = PacketType::from_index(i as u16) else {
                    continue;
                };
                ORBCOMM_PACKET_TYPES.put(
                    p,
                    &PacketType::CHOICES[i].key,
                    &OrbcommPacketTypeRow {
                        r#type: ty,
                        count: *n as i64,
                    },
                );
            }
        }
        for d in &self.decoders {
            key.clear();
            let _ = write!(key, "{}", d.index);
            let f = d.frequency_hz;
            let on = d.active;
            ORBCOMM_DECODERS.put(
                p,
                &key,
                &OrbcommDecoderRow {
                    decoder: d.index as i64,
                    state: if on {
                        DecoderState::of(d.state)
                    } else {
                        DecoderState::Free
                    },
                    frequency: on.then_some(Hertz(f)),
                    offset: on.then_some(Hertz(d.offset_hz)),
                    doppler: on.then(|| Hertz(f - raster(f))),
                    snr_db: on.then_some(d.snr_db),
                    level_db: on.then_some(d.level_db),
                    satellite: d.satellite.filter(|_| on).map(i64::from),
                    clock_ppm: on.then_some(d.clock_ppm),
                    frames: on.then_some(d.frames as i64),
                    frames_ok: on.then_some(d.good as i64),
                    frames_bad: on.then_some(d.bad as i64),
                    sync_losses: on.then_some(d.sync_losses as i64),
                },
            );
        }
        if let Some(d) = first {
            if !d.constellation.is_empty() {
                set_series(p.results_mut(), ORBCOMM_CONSTELLATION.key, false, || {
                    d.constellation.iter().copied()
                });
            }
        }
    }
}

impl Decoder for OrbDisplay {
    type Input = Timed<OrbItem>;
    fn name(&self) -> &'static str {
        "satcom.orbcomm_display"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
        ]
    }
    fn settings(&self) -> Arc<Schema> {
        OrbDisplaySetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match OrbDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            OrbDisplaySetting::Follow => self.follow = FOLLOW.read(id, v)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([(OrbDisplaySetting::Follow, Value::Bool(self.follow))])
    }
    fn commands(&self) -> &'static [&'static Command] {
        OrbDisplayCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match OrbDisplayCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            OrbDisplayCommand::PresetApply => {
                let OrbcommChannelArgs { channel } = ORBCOMM_PRESET_APPLY.args(args)?;
                self.retune = channel;
            }
            OrbDisplayCommand::Remove => {
                let p = self.results();
                crate::display::remove(
                    &mut self.tables.satellites,
                    &ORBCOMM_SATELLITES,
                    &p,
                    args,
                    &ORBCOMM_SATELLITE_ROW,
                )?;
                self.dirty = true;
            }
        }
        Ok(None)
    }
    fn results(&self) -> Published {
        Published::from_decl(display_decl())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.host_clock = ctx.clock.clone();
    }
    fn configure(&mut self, _f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(self.live, self.host_clock.now());
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn retune(&mut self, _f: &Format) {
        self.decoders.clear();
    }
    fn process(&mut self, x: &[Timed<OrbItem>], _first: u64, out: &mut Outbox) {
        let mut events = std::mem::take(out.messages(0));
        for it in x {
            let t = self.clock.at(it.time);
            match &it.value {
                OrbItem::Raw(r) => self.packet(t, r, &mut events),
                OrbItem::Status(s) => {
                    for d in s.iter().filter(|d| d.active) {
                        if let Some(row) = d
                            .satellite
                            .and_then(|id| self.tables.satellites.get_mut(&id))
                        {
                            row.snr_db = Some(d.snr_db);
                        }
                    }
                    self.decoders.clone_from(s);
                }
            }
        }
        *out.messages(0) = events;
        if let Some(c) = self.retune.take() {
            out.items::<HostRequest>(1).push(Timed {
                time: self.clock.now(),
                value: HostRequest::Retune {
                    tuned_hz: channel_hz(c),
                    reason: format!(
                        "Orbcomm channel {}",
                        Channel::CHOICES[c.index() as usize].key
                    ),
                },
            });
        }
    }
    fn report(&mut self, results: &mut Published) {
        self.publish(results);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.tables.clear();
        self.joiner.clear();
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.tables.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.tables.restore_state(state, self.clock.now())?;
        self.dirty = true;
        Ok(())
    }
}

sdrkit_core::settings! {
    /// The settings of the Orbcomm stack. Noise and other impairments of the test signal come from the standard `gen_*`
    /// settings.
    pub enum OrbcommSetting {
        /// Decoders.
        Decoders = DECODERS,
        /// Follow satellite.
        Follow = FOLLOW,
        /// Downlinks of the test signal.
        GenDownlinks = gen::GEN_DOWNLINKS,
        /// Doppler of the test signal.
        GenSatelliteDopplerHz = gen::GEN_SATELLITE_DOPPLER_HZ,
        /// Doppler ramp of the test signal.
        GenRampHz = gen::GEN_RAMP_HZ,
        /// Ramp leg of the test signal.
        GenRampS = gen::GEN_RAMP_S,
        /// Broken checks of the test signal.
        GenBroken = gen::GEN_BROKEN,
        /// Radio centre of the test signal.
        GenCentreHz = gen::GEN_CENTRE_HZ,
    }
}

/// Adds the stack's components to `r`.
pub fn register(r: &mut Registry) {
    r.add("satcom.orbcomm_receiver", || {
        Box::new(DecoderComponent::new(OrbRx::default()))
    });
    r.add("satcom.orbcomm_display", || {
        Box::new(DecoderComponent::new(OrbDisplay::default()))
    });
}

/// The receiver.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = GraphBuilder::new()
        .add("receiver", DecoderComponent::new(OrbRx::default()))
        .add("display", DecoderComponent::new(OrbDisplay::default()))
        .input("receiver.in")
        .connect("receiver.out", "display.in")
        .output("display.events")
        .output("display.requests")
        .status_from("display")
        .clock(REPORT_S)
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "Orbcomm needs a sample rate of at least 100 ksps")
        .default_centre(DEFAULT_DIAL_HZ + DESCRIPTOR.tuning.tune_offset_hz)
        .route("decoders", &["receiver.decoders"])?
        .route("follow", &["display.follow"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command(
            ORBCOMM_PRESET_APPLY.command.key,
            sdrkit_graph::CommandTarget::of("display", ORBCOMM_PRESET_APPLY.command.key),
        )
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("display", "remove"),
        )
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::OrbGenerator::from_config(cfg)))
}

/// The verification of the stack: a downlink at Es/N0 20 dB must give good packets.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, one downlink", o, 0.0);
    c.freq_hz = Some(DEFAULT_DIAL_HZ + DESCRIPTOR.tuning.tune_offset_hz);
    c.seconds = o.seconds.max(8.0);
    c.expect_audio = Some(false);
    c.check = Some(check_packets);
    vec![c]
}

fn check_packets(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let n = crate::field(r, "frames_ok") as i64;
    vec![CheckLine::new(
        "frames_ok",
        n > 50,
        format!("{n} packets with a good check (want more than 50)"),
    )]
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "Orbcomm 137 MHz downlinks: packets, satellite positions, orbital elements, channel lists and messages. Receive only.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 136e6,
        max_hz: 139e6,
        default_hz: DEFAULT_DIAL_HZ,
        rate_hz: 1e6,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 0.6e6,
        tune_offset_hz: 25e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: OrbcommSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Orbcomm],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};
