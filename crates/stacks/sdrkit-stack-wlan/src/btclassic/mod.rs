//! Bluetooth Classic discovery (`btclassic`): BR/EDR packets on every 1 MHz channel inside a wide capture, found by the access
//! code of each packet. The LAP in the sync word gives the devices and piconets; inquiry responses (FHS packets) give the full
//! address, the class of device and the clock, and the extended inquiry response 1.25 ms after an FHS gives the name and
//! the services. In a piconet whose LAP an FHS named, the UAP of that FHS checks the packet headers: two packets fix the
//! master clock's whitening seed, and the type of each packet is counted. The headers of other piconets cannot be checked
//! and their payloads are not decoded; connections are not followed. A capture narrower than the 79 channels sees about
//! (covered channels / 79) of the packets.
//! Standard: Bluetooth Core Specification v5.4, Vol 2 Part A and Part B; Bluetooth Assigned Numbers (Class of Device).

pub mod gen;

use crate::report::wall;
use crate::ble::adv::{kind_of_class, DeviceKind as ClassKind};
use crate::typed::{
    BtclassicDeviceEvent, BtclassicDeviceRow, BtclassicFields, BtclassicInquiryEvent,
    BtclassicPacketRow, BtclassicPacketTypeRow, BTCLASSIC_DEVICES, BTCLASSIC_DEVICE_NEW, BTCLASSIC_INQUIRY,
    BTCLASSIC_PACKETS, BTCLASSIC_PACKET_TYPES,
};
use sdrkit_algo_dsp::fir::design_by_taps;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::resample_span;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::chanburst::{ChannelGrid, GridBurst, GridConfig};
use sdrkit_core::interface::{Fields as _, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Clock, Complex32, ConfigError, EventDef, GenConfig, Generator, LockProfile,
    LockState, Receiver, Record, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kept,
    Kind, Outbox, Port, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_proto_bt::baseband::{find_access_codes, AccessHit, CHANNELS, GIAC, LIAC};
use sdrkit_proto_bt::cod;
use sdrkit_proto_bt::fhs::{read_packet, Fhs, PACKET_AIR_BITS};
use sdrkit_proto_bt::packet::{header_readings, read_eir, type_name, HeaderReading};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "btclassic";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 4e6;
/// The rate the bits are read at, Hz (4 samples per bit).
pub const WORK_RATE_HZ: f64 = 4e6;
/// The most devices kept.
pub const DEVICE_CAP: usize = 400;
/// The most packets kept in the log.
pub const PACKET_CAP: usize = 300;
/// An access code within this many seconds of signal shows the lock.
pub const RECENT_S: f64 = 5.0;
/// The levels kept for each device.
pub const LEVEL_HISTORY: usize = 24;
/// The report period, s.
pub const REPORT_S: f64 = 0.25;
/// What no public document confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: the modulation index range 0.28 to 0.35 (the test signal uses 0.32); the burst thresholds (10 dB, 3 % leakage, wideband from 5 channels) are not tuned on a real 2.4 GHz band. The CRC preset for a UAP other than 0 is not known, so only inquiry responses are checked.";

/// Sync-word bits that may differ.
pub const SYNC_ERRORS: Setting<field::I64> = Setting::new(
    "sync_errors",
    "Sync word errors allowed",
    "bits",
    2,
)
.range(0.0, 4.0, 1.0)
.help("How many of the 64 sync-word bits may differ from the sync word of the LAP they carry.");
/// The burst threshold.
pub const THRESHOLD_DB: Setting<field::F64> =
    Setting::new("threshold_db", "Burst threshold", "dB", 10.0)
        .range(6.0, 20.0, 1.0)
        .help("How far above the noise floor of a 1 MHz channel a burst must rise.");
/// The width from which adjacent active channels are one wideband signal.
pub const WIDE_CHANNELS: Setting<field::I64> =
    Setting::new("wide_channels", "Wideband from", "channels", 5)
        .range(3.0, 10.0, 1.0)
        .help("Adjacent active channels of similar level, this many or more, are a wideband signal (Wi-Fi) and are skipped.");

sdrkit_core::settings! {
    /// The settings of `wlan.btclassic`.
    pub enum RxSetting {
        /// Sync-word errors.
        SyncErrors = SYNC_ERRORS,
        /// The burst threshold.
        ThresholdDb = THRESHOLD_DB,
        /// The wideband width.
        WideChannels = WIDE_CHANNELS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `btclassic` stack.
    pub enum BtSetting {
        /// Sync-word errors.
        SyncErrors = SYNC_ERRORS,
        /// The burst threshold.
        ThresholdDb = THRESHOLD_DB,
        /// The wideband width.
        WideChannels = WIDE_CHANNELS,
        /// The wideband bursts of the test signal.
        GenWideband = gen::GEN_WIDEBAND,
    }
}

/// The kind of a device row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceKind {
    /// A scanner sending GIAC ID packets.
    Inquiry,
    /// A scanner sending LIAC ID packets.
    LimitedInquiry,
    /// The LAP of a paged device or of a piconet master.
    Piconet,
    /// A device that answered an inquiry.
    Discoverable,
}

impl DeviceKind {
    /// The text shown.
    pub fn text(self) -> &'static str {
        match self {
            DeviceKind::Inquiry => "inquiry (GIAC)",
            DeviceKind::LimitedInquiry => "limited inquiry (LIAC)",
            DeviceKind::Piconet => "device or piconet master",
            DeviceKind::Discoverable => "discoverable device",
        }
    }
}

/// A device or piconet, keyed by LAP.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// The LAP.
    pub lap: u32,
    /// What it is.
    pub kind: DeviceKind,
    /// The FHS of its last inquiry response.
    pub fhs: Option<FhsFields>,
    /// The name its extended inquiry response gave ("" when none).
    pub name: String,
    /// The services its extended inquiry response listed, joined with ", ".
    pub services: String,
    /// The last levels, dBFS, oldest first.
    pub levels: Vec<f32>,
    /// Packets heard.
    pub packets: u64,
    /// The channel of the last packet.
    pub last_channel: u32,
    /// Channel k heard: bit k.
    pub channels: u128,
}

/// The fields of an FHS packet that a device row shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FhsFields {
    /// The upper address part.
    pub uap: u8,
    /// The non-significant address part.
    pub nap: u16,
    /// The class of device.
    pub class: u32,
    /// CLK27..CLK2.
    pub clock: u32,
}

impl FhsFields {
    fn of(f: &Fhs) -> FhsFields {
        FhsFields {
            uap: f.uap,
            nap: f.nap,
            class: f.class,
            clock: f.clock,
        }
    }

    fn address(&self, lap: u32) -> String {
        Fhs {
            lap,
            uap: self.uap,
            nap: self.nap,
            ..Fhs::default()
        }
        .address_text()
    }
}

impl Shift for Device {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// One line of the packet log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PacketRow {
    /// Signal time, s.
    pub t: f64,
    /// Wall time, when live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall: Option<f64>,
    /// The channel centre, MHz.
    pub channel_mhz: f64,
    /// The LAP of the access code.
    pub lap: u32,
    /// The level, dBFS.
    pub level_dbfs: f32,
    /// Sync-word bits that differed.
    pub errors: u32,
    /// What the packet is.
    pub text: String,
}

impl Shift for PacketRow {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// The counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Counters {
    /// Good access codes.
    pub access_codes: u64,
    /// Bursts analysed without an access code.
    pub without_code: u64,
    /// GIAC and LIAC ID packets.
    pub inquiry_ids: u64,
    /// FHS packets read.
    pub inquiry_responses: u64,
    /// EIR packets read.
    pub eir: u64,
    /// Headers read with a known UAP.
    pub headers: u64,
    /// Packets of piconets whose UAP is not known.
    pub without_uap: u64,
    /// Bursts analysed.
    pub analysed: u64,
}

/// What the stack keeps and saves.
#[derive(Serialize, Deserialize)]
pub struct BtHeld {
    /// The devices.
    pub devices: ObjectTable<u32, Device>,
    /// The packet log.
    pub packets: LogRing<PacketRow>,
    /// The counters.
    pub counters: Counters,
    /// The packets of each (LAP, TYPE) whose header was read.
    pub types: Vec<(u32, u8, u64)>,
}

impl Default for BtHeld {
    fn default() -> Self {
        BtHeld {
            devices: ObjectTable::new(DEVICE_CAP),
            packets: LogRing::new(PACKET_CAP),
            counters: Counters::default(),
            types: Vec::new(),
        }
    }
}

impl Shift for BtHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.devices.shift_times(by_s);
        self.packets.shift_times(by_s);
    }
}

impl Tables for BtHeld {
    const FORMAT: StateFormat = StateFormat::new("wlan.btclassic", 2);
    fn clear(&mut self) {
        self.devices.clear();
        self.packets.clear();
        self.counters = Counters::default();
        self.types.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.devices.adopt(saved.devices);
        self.packets.adopt(saved.packets);
        self.counters = saved.counters;
        self.types = saved.types;
    }
}

fn lap_text(lap: u32) -> String {
    format!("{lap:06X}")
}

impl Shaped for Device {
    type Row = BtclassicDeviceRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let f = self.fhs.as_ref();
        let h: Vec<String> = self.levels.iter().map(|l| format!("{l:.1}")).collect();
        out.put(&BtclassicDeviceRow {
            lap: &lap_text(self.lap),
            kind: self.kind.text(),
            address: f.map(|f| f.address(self.lap)).as_deref(),
            class: f.map(|f| cod::text(f.class)).as_deref(),
            major_class: f.map(|f| cod::major_name(cod::major(f.class))),
            class_raw: f.map(|f| i64::from(f.class)),
            clock: f.map(|f| i64::from(f.clock)),
            level_dbfs: self.levels.last().map(|&l| f64::from(l)),
            level_history: &h.join(" "),
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
            packets: self.packets as i64,
            last_channel_mhz: 2402.0 + f64::from(self.last_channel),
            channels_heard: &format!("{:020X}", self.channels),
            device_kind: f.map_or(ClassKind::Unknown, |f| kind_of_class(f.class)),
            name: (!self.name.is_empty()).then_some(self.name.as_str()),
            services: (!self.services.is_empty()).then_some(self.services.as_str()),
        });
    }
}

impl Shaped for PacketRow {
    type Row = BtclassicPacketRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&BtclassicPacketRow {
            utc: self.wall.map(Time),
            time_s: self.t,
            channel_mhz: self.channel_mhz,
            lap: &lap_text(self.lap),
            level_dbfs: f64::from(self.level_dbfs),
            sync_errors: i64::from(self.errors),
            text: &self.text,
        });
    }
}

/// A new device, as an event item.
#[derive(Clone, Debug, PartialEq)]
pub struct NewDevice {
    /// The LAP.
    pub lap: u32,
    /// What it is.
    pub kind: DeviceKind,
}

impl Item for NewDevice {
    const KIND: &'static str = "btclassic.device_new";
    fn event_def() -> Option<&'static EventDef> {
        Some(BTCLASSIC_DEVICE_NEW.def())
    }
    fn write_fields(&self, f: &mut Record) {
        BtclassicDeviceEvent {
            lap: &lap_text(self.lap),
            kind: self.kind.text(),
            family: Family::Other,
        }
        .write(f);
    }
}

/// An inquiry response, as an event item.
#[derive(Clone, Debug, PartialEq)]
pub struct InquiryResponse {
    /// The FHS fields.
    pub fhs: Fhs,
}

impl Item for InquiryResponse {
    const KIND: &'static str = "btclassic.inquiry_response";
    fn event_def() -> Option<&'static EventDef> {
        Some(BTCLASSIC_INQUIRY.def())
    }
    fn write_fields(&self, f: &mut Record) {
        BtclassicInquiryEvent {
            address: &self.fhs.address_text(),
            class: &cod::text(self.fhs.class),
            family: Family::Other,
        }
        .write(f);
    }
}

/// The number of taps of the channel filter at `rate`: odd, at least max(15, 3.3 × rate / 0.9 MHz), at most 257.
fn filter_taps(rate: f64) -> usize {
    let n = (3.3 * rate / 0.9e6).ceil().max(15.0) as usize;
    (n | 1).min(257)
}

/// The FFT frame of the burst detector: the largest power of two from 32 to 256 that spans at most 8 µs.
fn grid_frame(rate: f64) -> usize {
    let mut n = 32;
    while n < 256 && (2 * n) as f64 <= rate * 8e-6 {
        n *= 2;
    }
    n
}

/// One packet read from a burst.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    /// The access code.
    pub hit: AccessHit,
    /// The FHS packet, for a GIAC access code with a trailer that reads as one.
    pub fhs: Option<Fhs>,
    /// The bits after the access code of a code with a trailer (at most [`AFTER_BITS`]).
    pub after: Vec<u8>,
}

/// The bits after an access code that a packet may hold: the header and a DH5 payload.
pub const AFTER_BITS: usize = 54 + 2744 + 16;
/// The time from the start of an FHS to the start of its extended inquiry response, s (Vol 2 Part B clause 8.4.3).
const EIR_AFTER_FHS_S: f64 = 1.25e-3;
/// One slot, s: CLK1 advances once a slot.
const SLOT_S: f64 = 625e-6;

/// The master clock of a piconet as its headers show it.
#[derive(Clone, Debug)]
struct PiconetClock {
    lap: u32,
    /// A packet's start and its whitening seed (CLK1..CLK6), once known.
    known: Option<(f64, u8)>,
    /// The last packet's start and its readings, while the clock is not known.
    pending: Option<(f64, Vec<HeaderReading>)>,
}

/// The bits of a burst and what they hold.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BurstRead {
    /// The packets found, in order.
    pub heard: Vec<Heard>,
    /// The carrier offset, Hz.
    pub cfo_hz: f64,
    /// The mean power of the burst, dBFS.
    pub level_dbfs: f32,
}

/// Reads the access codes and FHS packets of a burst that was mixed to 0 Hz, filtered and brought to 4 Msps, from its
/// discriminator output `d` (radians per sample): for each of the 4 sample phases the bits are the means over 4 samples sliced
/// at their mean (without 4 bits at each end); the phase whose best access code has the fewest sync errors wins.
pub fn read_bits(d: &[f32], max_errors: u32, bits: &mut Vec<u8>) -> (Vec<Heard>, f64) {
    let mut best: Option<(u32, usize, f32)> = None;
    let mut values = Vec::with_capacity(d.len() / 4 + 1);
    for phase in 0..4 {
        slice(d, phase, &mut values, bits);
        if values.len() < 76 {
            continue;
        }
        let thr = threshold(&values);
        let mut fewest = None;
        find_access_codes(bits, max_errors, |h| {
            fewest = Some(fewest.map_or(h.errors, |e: u32| e.min(h.errors)));
        });
        if let Some(e) = fewest {
            if best.is_none_or(|(b, _, _)| e < b) {
                best = Some((e, phase, thr));
            }
        }
    }
    let Some((_, phase, thr)) = best else {
        return (Vec::new(), 0.0);
    };
    slice(d, phase, &mut values, bits);
    let mut heard = Vec::new();
    find_access_codes(bits, max_errors, |hit| {
        let fhs = (hit.lap == GIAC && hit.trailer)
            .then(|| bits.get(hit.at + 68..hit.at + 68 + PACKET_AIR_BITS))
            .flatten()
            .and_then(|b| read_packet(b).ok())
            .map(|p| p.fhs);
        let after = if hit.trailer {
            let from = (hit.at + 68).min(bits.len());
            bits[from..(from + AFTER_BITS).min(bits.len())].to_vec()
        } else {
            Vec::new()
        };
        heard.push(Heard { hit, fhs, after });
    });
    (heard, f64::from(thr) * WORK_RATE_HZ / std::f64::consts::TAU)
}

/// The bit values of phase `phase` (means of 4 discriminator samples) into `values`, sliced at their threshold into `bits`.
fn slice(d: &[f32], phase: usize, values: &mut Vec<f32>, bits: &mut Vec<u8>) {
    values.clear();
    bits.clear();
    let mut i = phase;
    while i + 4 <= d.len() {
        values.push(d[i..i + 4].iter().sum::<f32>() / 4.0);
        i += 4;
    }
    let thr = threshold(values);
    bits.extend(values.iter().map(|&v| u8::from(v > thr)));
}

/// The mean of the bit values without 4 at each end.
fn threshold(v: &[f32]) -> f32 {
    let inner = if v.len() > 8 { &v[4..v.len() - 4] } else { v };
    inner.iter().sum::<f32>() / inner.len().max(1) as f32
}

/// The `wlan.btclassic` decoder: the burst detector over the channels inside the capture, the demodulator of each burst, the
/// access-code and FHS reader, and the tables.
pub struct BtDecoder {
    sync_errors: u32,
    threshold_db: f64,
    wide_channels: usize,
    rate: f64,
    centre_hz: f64,
    relative: bool,
    /// Signal time of the first sample after the detector was last built, s: its sample index counts from there.
    origin_s: f64,
    fresh: bool,
    grid: Option<ChannelGrid>,
    /// The channel number of each channel of the grid.
    channels: Vec<u32>,
    taps: Vec<f32>,
    bursts: Vec<GridBurst>,
    mixed: Vec<Complex32>,
    filtered: Vec<Complex32>,
    work: Vec<Complex32>,
    disc: Vec<f32>,
    bits: Vec<u8>,
    held: BtHeld,
    cfo_hz: Option<f64>,
    snr_db: Option<f64>,
    level_acc: f64,
    level_n: u64,
    input_dbfs: Option<f64>,
    now: f64,
    last_code: f64,
    live: bool,
    clock: Option<Clock>,
    new_devices: Vec<Timed<NewDevice>>,
    responses: Vec<Timed<InquiryResponse>>,
    /// The last FHS: its start, its channel and the LAP of its sender.
    last_fhs: Option<(f64, u32, u32)>,
    /// The clock of each piconet whose headers are read.
    clocks: Vec<PiconetClock>,
    summary: String,
    dirty: bool,
    cleared: bool,
    started: bool,
}

impl Default for BtDecoder {
    fn default() -> Self {
        BtDecoder {
            sync_errors: 2,
            threshold_db: 10.0,
            wide_channels: 5,
            rate: 0.0,
            centre_hz: 0.0,
            relative: false,
            origin_s: 0.0,
            fresh: true,
            grid: None,
            channels: Vec::new(),
            taps: Vec::new(),
            bursts: Vec::new(),
            mixed: Vec::new(),
            filtered: Vec::new(),
            work: Vec::new(),
            disc: Vec::new(),
            bits: Vec::new(),
            held: BtHeld::default(),
            cfo_hz: None,
            snr_db: None,
            level_acc: 0.0,
            level_n: 0,
            input_dbfs: None,
            now: 0.0,
            last_code: f64::NEG_INFINITY,
            live: false,
            clock: None,
            new_devices: Vec::new(),
            responses: Vec::new(),
            last_fhs: None,
            clocks: Vec::new(),
            summary: String::with_capacity(128),
            dirty: true,
            cleared: false,
            started: false,
        }
    }
}

impl BtDecoder {
    /// The devices, the log and the counters.
    pub fn held(&self) -> &BtHeld {
        &self.held
    }

    /// Builds the burst detector for the rate and the radio centre.
    fn build(&mut self) -> Result<(), ConfigError> {
        self.fresh = true;
        let fs = self.rate;
        let c = if self.relative {
            gen::TEST_CENTRE_HZ
        } else {
            self.centre_hz
        };
        self.channels = (0..CHANNELS).filter(|&k| gen::covered(k, fs, c)).collect();
        if self.channels.is_empty() {
            // The capture holds no whole channel: watch the one nearest the centre.
            let k = ((c - 2402e6) / 1e6).round().clamp(0.0, 78.0) as u32;
            self.channels.push(k);
        }
        let centres = self
            .channels
            .iter()
            .map(|&k| sdrkit_proto_bt::baseband::channel_hz(k) - c)
            .collect();
        let mut cfg = GridConfig::new(fs, centres, grid_frame(fs));
        cfg.half_width_hz = 450e3;
        cfg.threshold_db = self.threshold_db;
        cfg.wide_channels = self.wide_channels;
        self.grid = Some(ChannelGrid::new(cfg).map_err(ConfigError::from)?);
        self.taps = design_by_taps(filter_taps(fs), 600e3 / fs, Window::Hamming)
            .map_err(ConfigError::from)?;
        Ok(())
    }

    fn stamp(&self, t: f64) -> Stamp {
        match self.clock.as_ref().and_then(|c| wall(self.live, c)) {
            Some(w) => Stamp::at(t).with_wall(w),
            None => Stamp::at(t),
        }
    }

    /// Brings a burst to 0 Hz at 4 Msps and leaves the discriminator output in `disc`. Returns the mean power, dBFS.
    fn demodulate(&mut self, b: &GridBurst, offset_hz: f64) -> f32 {
        let fs = self.rate;
        self.mixed.clear();
        self.mixed.resize(b.samples.len(), Complex32::default());
        let mut nco = Nco::new(fs, -offset_hz);
        nco.mix(&b.samples, &mut self.mixed);
        let ratio = fs / WORK_RATE_HZ;
        let step = ratio.round() as usize;
        let n = self.taps.len();
        self.work.clear();
        if (ratio - step as f64).abs() < 1e-9 && step >= 1 {
            let mut i = 0;
            while i + n <= self.mixed.len() {
                let acc = self.mixed[i..i + n]
                    .iter()
                    .zip(&self.taps)
                    .fold(Complex32::default(), |a, (x, &t)| a + x * t);
                self.work.push(acc);
                i += step;
            }
        } else {
            self.filtered.clear();
            for w in self.mixed.windows(n) {
                self.filtered.push(
                    w.iter()
                        .zip(&self.taps)
                        .fold(Complex32::default(), |a, (x, &t)| a + x * t),
                );
            }
            resample_span(&self.filtered, fs, WORK_RATE_HZ, &mut self.work);
        }
        self.disc.clear();
        for w in self.work.windows(2) {
            let z = w[1] * w[0].conj();
            self.disc.push(z.im.atan2(z.re));
        }
        let inner = if self.work.len() > 24 {
            &self.work[12..self.work.len() - 12]
        } else {
            &self.work[..]
        };
        let p = inner.iter().map(|z| z.norm_sqr()).sum::<f32>() / inner.len().max(1) as f32;
        10.0 * p.max(1e-20).log10()
    }

    fn burst(&mut self, b: GridBurst) {
        let Some(grid) = self.grid.as_ref() else {
            return;
        };
        let offset = grid.config().centres_hz[b.channel];
        let k = self.channels[b.channel];
        let t = self.origin_s + b.start as f64 / self.rate;
        self.held.counters.analysed += 1;
        let level = self.demodulate(&b, offset);
        let mut bits = std::mem::take(&mut self.bits);
        let (heard, cfo) = read_bits(&self.disc, self.sync_errors, &mut bits);
        self.bits = bits;
        if heard.is_empty() {
            self.held.counters.without_code += 1;
            return;
        }
        self.cfo_hz = Some(cfo);
        self.snr_db = Some(f64::from(b.snr_db));
        self.last_code = t;
        let at = self.stamp(t);
        for h in heard {
            // The bits run at 1 Mbit/s from the start of the burst.
            let tp = t + h.hit.at as f64 * 1e-6;
            self.packet(&h, k, level, tp, at);
        }
        self.dirty = true;
    }

    /// Reads what follows the access code of `h`: an extended inquiry response after an FHS, or the header of a packet of a
    /// piconet whose UAP is known. Returns the text for the packet log, when there is one.
    fn after_code(&mut self, h: &Heard, k: u32, t: f64, at: Stamp) -> Option<String> {
        if h.after.is_empty() || h.fhs.is_some() {
            return None;
        }
        if h.hit.lap == GIAC {
            let e = read_eir(&h.after)?;
            self.held.counters.eir += 1;
            let info = crate::ble::adv::parse_ad(&e.data);
            let text = format!(
                "extended inquiry response ({}){}",
                type_name(e.kind),
                info.name.as_deref().map(|n| format!(": {n}")).unwrap_or_default()
            );
            // The response belongs to the FHS sent 1.25 ms before it on the same channel.
            if let Some((tf, kf, lap)) = self.last_fhs {
                if kf == k && (t - tf - EIR_AFTER_FHS_S).abs() < 0.1e-3 {
                    if let Ok(up) = self.held.devices.upsert(&lap, at, || Device {
                        lap,
                        kind: DeviceKind::Discoverable,
                        fhs: None,
                        name: String::new(),
                        services: String::new(),
                        levels: Vec::new(),
                        packets: 0,
                        last_channel: k,
                        channels: 0,
                    }) {
                        if let Some(n) = info.name {
                            up.row.name = n;
                        }
                        if !info.services.is_empty() {
                            up.row.services = info.services.join(", ");
                        }
                    }
                }
            }
            return Some(text);
        }
        if h.hit.lap == LIAC {
            return None;
        }
        let lap = h.hit.lap;
        let uap = self
            .held
            .devices
            .get_mut(&lap)
            .and_then(|d| d.fhs.as_ref().map(|f| f.uap));
        let Some(uap) = uap else {
            self.held.counters.without_uap += 1;
            return None;
        };
        let readings = header_readings(&h.after, uap);
        // A seed and its complement whiten a header into two that both pass the HEC: one packet cannot tell the clock. Two
        // packets can: only the true seed advances by the slots between them. Once known, the clock picks the reading.
        let slots = |t0: f64| ((t - t0) / SLOT_S).round() as i64;
        let advance = |c: u8, n: i64| (i64::from(c) + n).rem_euclid(64) as u8;
        let entry = self.clocks.iter().position(|c| c.lap == lap);
        let r = match entry.map(|i| self.clocks[i].clone()) {
            Some(PiconetClock {
                known: Some((t0, c0)),
                ..
            }) => readings.iter().find(|r| r.clk == advance(c0, slots(t0))).copied(),
            Some(PiconetClock {
                pending: Some((t0, ref before)),
                ..
            }) => {
                let n = slots(t0);
                let pairs: Vec<(HeaderReading, HeaderReading)> = readings
                    .iter()
                    .flat_map(|r| {
                        before
                            .iter()
                            .filter(move |b| advance(b.clk, n) == r.clk)
                            .map(move |b| (*b, *r))
                    })
                    .collect();
                // Two fits (the packets an even half-cycle apart) tell nothing yet.
                let pair = (pairs.len() == 1).then(|| pairs[0]);
                if let Some((b, _)) = pair {
                    // The packet before is now read too.
                    self.count_type(lap, b.kind);
                }
                pair.map(|(_, r)| r)
            }
            _ => None,
        };
        let state = PiconetClock {
            lap,
            known: r.map(|r| (t, r.clk)),
            pending: r.is_none().then(|| (t, readings.clone())),
        };
        match entry {
            Some(i) => {
                // A clock once known stays known when a packet fits no reading.
                if r.is_some() || self.clocks[i].known.is_none() {
                    self.clocks[i] = state;
                }
            }
            None if self.clocks.len() < 64 => self.clocks.push(state),
            None => {}
        }
        let r = r?;
        self.count_type(lap, r.kind);
        Some(format!(
            "{} of LAP {}, LT_ADDR {}",
            type_name(r.kind),
            lap_text(lap),
            r.lt_addr
        ))
    }

    /// Counts a header of TYPE `kind` read in the piconet of `lap`.
    fn count_type(&mut self, lap: u32, kind: u8) {
        self.held.counters.headers += 1;
        let types = &mut self.held.types;
        match types.iter().position(|e| e.0 == lap && e.1 == kind) {
            Some(i) => types[i].2 += 1,
            None if types.len() < 256 => types.push((lap, kind, 1)),
            None => {}
        }
    }

    fn packet(&mut self, h: &Heard, k: u32, level: f32, t: f64, at: Stamp) {
        if let Some(f) = &h.fhs {
            self.last_fhs = Some((t, k, f.lap));
        }
        let read = self.after_code(h, k, t, at);
        let c = &mut self.held.counters;
        c.access_codes += 1;
        let (lap, kind, text) = match (&h.fhs, h.hit.lap) {
            (Some(f), _) => {
                c.inquiry_responses += 1;
                self.responses.push(Timed {
                    time: t,
                    value: InquiryResponse { fhs: *f },
                });
                (
                    f.lap,
                    DeviceKind::Discoverable,
                    format!(
                        "inquiry response: {}, {}",
                        f.address_text(),
                        cod::text(f.class)
                    ),
                )
            }
            (None, GIAC) if !h.hit.trailer => {
                c.inquiry_ids += 1;
                (
                    GIAC,
                    DeviceKind::Inquiry,
                    "ID packet, GIAC (inquiry)".to_string(),
                )
            }
            (None, LIAC) if !h.hit.trailer => {
                c.inquiry_ids += 1;
                (
                    LIAC,
                    DeviceKind::LimitedInquiry,
                    "ID packet, LIAC (limited inquiry)".to_string(),
                )
            }
            (None, GIAC) => (
                GIAC,
                DeviceKind::Inquiry,
                "access code of LAP 9E8B33 (GIAC)".to_string(),
            ),
            (None, LIAC) => (
                LIAC,
                DeviceKind::LimitedInquiry,
                "access code of LAP 9E8B00 (LIAC)".to_string(),
            ),
            (None, lap) => (
                lap,
                DeviceKind::Piconet,
                format!("access code of LAP {}", lap_text(lap)),
            ),
        };
        self.held.packets.push(PacketRow {
            t,
            wall: at.wall,
            channel_mhz: 2402.0 + f64::from(k),
            lap: h.hit.lap,
            level_dbfs: level,
            errors: h.hit.errors,
            text: read.unwrap_or(text),
        });
        if let Ok(up) = self.held.devices.upsert(&lap, at, || Device {
            lap,
            kind,
            fhs: None,
            name: String::new(),
            services: String::new(),
            levels: Vec::new(),
            packets: 0,
            last_channel: k,
            channels: 0,
        }) {
            if up.new {
                self.new_devices.push(Timed {
                    time: t,
                    value: NewDevice { lap, kind },
                });
            }
            let d = up.row;
            d.kind = kind;
            if let Some(f) = &h.fhs {
                d.fhs = Some(FhsFields::of(f));
            }
            crate::report::push_history(&mut d.levels, level, LEVEL_HISTORY);
            d.packets += 1;
            d.last_channel = k;
            d.channels |= 1 << k.min(127);
        }
    }

    /// Writes the status line into `out` (cleared first); no allocation once `out` is long enough.
    fn summary(&self, out: &mut String) {
        use std::fmt::Write as _;
        out.clear();
        let n = self.channels.len();
        let c = &self.held.counters;
        // Writing into a `String` cannot fail.
        let _ = if !self.started {
            write!(out, "Bluetooth Classic: waiting for samples")
        } else if c.access_codes == 0 {
            write!(
                out,
                "Bluetooth Classic: watching {n} of 79 channels, no access code yet"
            )
        } else {
            write!(
                out,
                "Bluetooth Classic: {} devices, {} access codes, {} inquiry responses; watching {n} of 79 channels",
                self.held.devices.len(),
                c.access_codes,
                c.inquiry_responses
            )
        };
    }
}

impl Decoder for BtDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "wlan.btclassic"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<NewDevice>("devices"),
            Port::typed::<InquiryResponse>("responses"),
        ]
    }
    fn settings(&self) -> Arc<sdrkit_core::Schema> {
        RxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (
                RxSetting::SyncErrors,
                Value::I64(i64::from(self.sync_errors)),
            ),
            (RxSetting::ThresholdDb, Value::F64(self.threshold_db)),
            (
                RxSetting::WideChannels,
                Value::I64(self.wide_channels as i64),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match RxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RxSetting::SyncErrors => self.sync_errors = SYNC_ERRORS.read(id, v)?.clamp(0, 4) as u32,
            RxSetting::ThresholdDb => self.threshold_db = THRESHOLD_DB.read(id, v)?,
            RxSetting::WideChannels => {
                self.wide_channels = WIDE_CHANNELS.read(id, v)?.clamp(3, 10) as usize;
            }
        }
        if self.rate > 0.0 {
            // A bad rate was refused at configure, so the detector builds again.
            let _ = self.build();
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::new(BtclassicFields::SHAPE.fields)
            .with_tables(&[
                BTCLASSIC_DEVICES.def(),
                BTCLASSIC_PACKETS.def(),
                BTCLASSIC_PACKET_TYPES.def(),
            ])
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE_HZ {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.centre_hz = input.centre_hz;
        self.relative = input.centre_hz <= 0.0;
        self.build()?;
        let f = |k: Kind| Format {
            rate: 0.0,
            frame_len: 0,
            media: None,
            ..input.with_kind(k)
        };
        Ok(vec![
            f(Kind::typed::<NewDevice>()),
            f(Kind::typed::<InquiryResponse>()),
        ])
    }
    fn retune(&mut self, input: &Format) {
        self.centre_hz = input.centre_hz;
        self.relative = input.centre_hz <= 0.0;
        let _ = self.build();
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = Some(ctx.clock.clone());
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.now = (first + x.len() as u64) as f64 / self.rate.max(1.0);
        if std::mem::take(&mut self.fresh) {
            self.origin_s = first as f64 / self.rate.max(1.0);
        }
        for z in x {
            self.level_acc += f64::from(z.norm_sqr());
        }
        self.level_n += x.len() as u64;
        let mut bursts = std::mem::take(&mut self.bursts);
        if let Some(g) = self.grid.as_mut() {
            g.process(x, |b| bursts.push(b));
        }
        for b in bursts.drain(..) {
            self.burst(b);
        }
        self.bursts = bursts;
        out.items::<NewDevice>(0).append(&mut self.new_devices);
        out.items::<InquiryResponse>(1).append(&mut self.responses);
    }
    fn report(&mut self, p: &mut Published) {
        if self.level_n > 0 {
            self.input_dbfs =
                Some(10.0 * (self.level_acc / self.level_n as f64).max(1e-20).log10());
            self.level_acc = 0.0;
            self.level_n = 0;
        }
        let c = self.held.counters;
        let recent = self.now - self.last_code <= RECENT_S;
        p.put(&BtclassicFields {
            access_codes: c.access_codes as i64,
            bursts_without_code: c.without_code as i64,
            inquiry_ids: c.inquiry_ids as i64,
            inquiry_responses: c.inquiry_responses as i64,
            bursts_analysed: c.analysed as i64,
            bursts_wideband: self.grid.as_ref().map_or(0, |g| g.wideband() as i64),
            channels_covered: self.channels.len() as i64,
            channels_relative: self.relative,
            cfo_hz: self.cfo_hz,
            snr_db: self.snr_db,
            input_level_dbfs: self.input_dbfs,
            state: i64::from(recent),
            devices: self.held.devices.len() as i64,
            note: NOT_CONFIRMED,
            eir_responses: c.eir as i64,
            headers_read: c.headers as i64,
            packets_without_uap: c.without_uap as i64,
        });
        if std::mem::take(&mut self.cleared) {
            p.results_mut().clear_log();
            p.clear_rows(BTCLASSIC_DEVICES.def());
            p.clear_rows(BTCLASSIC_PACKETS.def());
            p.clear_rows(BTCLASSIC_PACKET_TYPES.def());
        }
        if std::mem::take(&mut self.dirty) {
            shaped::rows(&self.held.devices, &BTCLASSIC_DEVICES, self.now, p);
            shaped::log_rows(&self.held.packets, &BTCLASSIC_PACKETS, self.now, p);
            for &(lap, kind, n) in &self.held.types {
                let l = lap_text(lap);
                BTCLASSIC_PACKET_TYPES.put(
                    p,
                    &format!("{l}/{}", type_name(kind)),
                    &BtclassicPacketTypeRow {
                        lap: &l,
                        packet_type: type_name(kind),
                        packets: n as i64,
                    },
                );
            }
        }
        let lock = if !self.started {
            LockState::Off
        } else if recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let mut summary = std::mem::take(&mut self.summary);
        self.summary(&mut summary);
        let level = self.input_dbfs.map(|l| l as f32);
        p.results_mut().update_status(|s| {
            let changed = s.lock_state != lock || s.summary != summary || s.level_dbfs != level;
            if changed {
                s.lock_state = lock;
                s.summary.clone_from(&summary);
                s.level_dbfs = level;
            }
            changed
        });
        self.summary = summary;
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.held.clear();
        self.cleared = true;
        self.dirty = true;
    }
    /// The counters and the measurements start again; the devices and the packet log stay if `kept` holds them.
    fn forget(&mut self, kept: &Kept) {
        self.held.counters = Counters::default();
        self.cfo_hz = None;
        self.snr_db = None;
        self.input_dbfs = None;
        self.level_acc = 0.0;
        self.level_n = 0;
        self.last_code = f64::NEG_INFINITY;
        if !kept.table(BTCLASSIC_DEVICES.key) {
            self.held.devices.clear();
        }
        if !kept.table(BTCLASSIC_PACKETS.key) {
            self.held.packets.clear();
        }
        self.held.types.clear();
        self.clocks.clear();
        self.last_fhs = None;
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.now)?;
        self.cleared = true;
        self.dirty = true;
        Ok(())
    }
}

/// The `btclassic` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a value of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("rx", DecoderComponent::new(BtDecoder::default()))
        .input("rx")
        .output("rx.devices")
        .output("rx.responses")
        .clock(REPORT_S)
        .status_from("rx")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE_HZ,
            "Bluetooth Classic needs a sample rate of at least 4 Msps",
        )
        .route(SYNC_ERRORS.key(), &["rx.sync_errors"])?
        .route(THRESHOLD_DB.key(), &["rx.threshold_db"])?
        .route(WIDE_CHANNELS.key(), &["rx.wide_channels"])?
        .command("clear_devices", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the component of this stack to a registry.
pub fn register(r: &mut Registry) {
    r.add("wlan.btclassic", || {
        Box::new(DecoderComponent::new(BtDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::BtGenerator::from_config(cfg)?))
}

/// The `btclassic` stack. Its default frequency is the centre of the test signal (2441 MHz); the session gives the real one.
pub static BTCLASSIC: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Bluetooth Classic",
    group: "Data and IoT",
    blurb: "Bluetooth BR/EDR discovery over a wide capture: devices and piconets by the LAP of every access code; inquiry responses give the address, class of device and clock.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        no_offset_tuning: true,
        ..Tuning::anywhere(gen::TEST_CENTRE_HZ, 20e6, MIN_RATE_HZ, 20e6)
    },
    submodes: &[],
    settings: BtSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::BTCLASSIC),
    benchmarks: Some(benchmarks),
};

/// The benchmark of the receiver: 0.5 s of the test signal at 20 Msps, fed in one piece.
pub fn benchmarks() -> Vec<sdrkit_core::bench::Benchmark> {
    vec![sdrkit_core::bench::Benchmark {
        name: "btclassic.receiver_20m",
        run: bench_receiver,
    }]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::bench::BenchResult {
    let rate = 20e6;
    let mut x = vec![Complex32::default(); (rate * 0.5) as usize];
    if let Ok(mut s) = gen::scene(rate, 1, false, gen::TEST_CENTRE_HZ) {
        s.generate(&mut x);
    }
    let Ok(mut rx) = receiver(&StackConfig::new(Values::defaults(&BtSetting::schema()))) else {
        return sdrkit_core::bench::BenchResult::default();
    };
    let c = gen::TEST_CENTRE_HZ;
    let _ = rx.configure(&sdrkit_core::InputInfo::new(rate, c, c).recording());
    let mut out = sdrkit_core::Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), 0.5, || {
        out.clear();
        rx.feed(&x, &mut out);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_filter_and_frame_sizes() {
        assert_eq!(filter_taps(4e6), 15);
        assert_eq!(filter_taps(20e6), 75);
        assert_eq!(filter_taps(100e6), 257);
        assert_eq!(grid_frame(20e6), 128);
        assert_eq!(grid_frame(4e6), 32);
        assert_eq!(grid_frame(8e6), 64);
    }

    #[test]
    fn the_bits_of_a_clean_packet_are_read_at_every_phase() {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(4);
        let bits = gen::packet(&gen::Sends::Fhs(gen::phone(), 0x15), 0.1, &mut rng);
        let mut x = vec![Complex32::new(1.0, 0.0); 40];
        x.extend(gen::gfsk(&bits, WORK_RATE_HZ).unwrap());
        x.extend(vec![*x.last().unwrap(); 40]);
        for skip in 0..4 {
            let d: Vec<f32> = x[skip..]
                .windows(2)
                .map(|w| {
                    let z = w[1] * w[0].conj();
                    z.im.atan2(z.re)
                })
                .collect();
            let mut b = Vec::new();
            let (heard, cfo) = read_bits(&d, 2, &mut b);
            assert_eq!(heard.len(), 1, "skip {skip}");
            assert_eq!(heard[0].hit.lap, GIAC);
            assert_eq!(heard[0].fhs.map(|f| f.lap), Some(0xA1B2C3));
            assert!(cfo.abs() < 20e3, "{cfo}");
        }
    }
}
