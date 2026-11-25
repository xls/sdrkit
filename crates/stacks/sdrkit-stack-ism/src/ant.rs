//! ANT+ sensors (`ant`): 1 Mbit/s GFSK frames on one channel, the ANT+ network marker and the frames of other ANT networks, the
//! public device profiles, and a table of sensors with the history of their main value. `send` sends one ANT+ frame.
//!
//! Standard: ANT+ device profiles (heart rate, bike speed and cadence, bike power, stride, environment, common pages).

pub mod send;

use crate::family::{checked, hex, min_rate_message, CleanScene, REPORT_S};
use crate::typed::{
    AntDemodFields, AntFields, AntFrameRow, AntSensorEvent, AntSensorRow, ANT_FRAMES, ANT_HISTORY,
    ANT_REMOVE, ANT_SENSOR, ANT_SENSORS, ANT_SENSOR_ROW,
};
use sdrkit_algo_dsp::floor::MinFollower;
use sdrkit_algo_dsp::fm::LagDiscriminator;
use sdrkit_algo_dsp::stats::WindowSum;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::bench::{measure, BenchResult, Benchmark};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, ReadFields, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::AutoGain;
use sdrkit_core::SignalType;
use sdrkit_core::{
    Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, LockState, Receiver, Record,
    Results, ResultsDecl, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, StateBlob, StateError, Status, TableId, Tuning, Value, Values,
};
use sdrkit_graph::Catalog;
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Decoder, DecoderComponent, Format, GraphBuilder,
    GraphError, Io, Item, Kept, Kind, Outbox, Port, RateFrom, StackGraph, Timed,
};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;
use sdrkit_util_bits::pack::unpack;
use sdrkit_util_bits::BitOrder;
use sdrkit_util_objects::commands;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp, Tables,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

/// The ANT+ network marker.
pub const ANT_PLUS: u16 = 0xA6C5;
/// The bit rate, bit/s.
pub const BIT_RATE: f64 = 1e6;
/// The rate the demodulator runs at: 4 samples a bit.
pub const DEMOD_RATE: f64 = 4e6;
/// The width of the channel filter, Hz (650 kHz on each side).
pub const CHANNEL_WIDTH: f64 = 1.3e6;
/// The sensors kept.
pub const SENSOR_CAP: usize = 100;
/// The frames kept in the log.
pub const LOG_CAP: usize = 400;
/// The values kept in the history of a sensor (about 10 minutes at 4 Hz).
pub const HISTORY_CAP: usize = 2400;

/// The 17 bytes of an ANT frame from the marker to the CRC.
pub const FRAME_LEN: usize = 17;

/// One ANT frame with a good CRC (`ant.frame`).
#[derive(Clone, Debug, PartialEq)]
pub struct AntFrame {
    /// The 17 bytes (marker to CRC).
    pub bytes: [u8; FRAME_LEN],
    /// The level over preamble and frame, dBFS.
    pub level_dbfs: f32,
    /// The SNR, dB.
    pub snr_db: f32,
    /// The decision level: the carrier offset from the channel, Hz.
    pub offset_hz: f32,
    /// The deviation, Hz.
    pub dev_hz: f32,
    /// The carrier frequency: the channel plus the offset, Hz.
    pub freq_hz: f64,
}

impl Item for AntFrame {
    const KIND: &'static str = "ant.frame";
}

impl AntFrame {
    /// The network marker.
    pub fn marker(&self) -> u16 {
        u16::from_be_bytes([self.bytes[0], self.bytes[1]])
    }
    /// The device number.
    pub fn device(&self) -> u16 {
        u16::from_le_bytes([self.bytes[2], self.bytes[3]])
    }
    /// The device type (the low 7 bits).
    pub fn device_type(&self) -> u8 {
        self.bytes[4] & 0x7F
    }
    /// The pairing bit.
    pub fn pairing(&self) -> bool {
        self.bytes[4] & 0x80 != 0
    }
    /// The transmission type.
    pub fn tx_type(&self) -> u8 {
        self.bytes[5]
    }
    /// The 8 payload bytes.
    pub fn payload(&self) -> &[u8] {
        &self.bytes[7..15]
    }
}

/// Whether `b` holds a 17-byte frame with a good CRC: CRC-16/IBM-3740 over bytes 0 to 14, sent high byte first.
pub fn crc_ok(b: &[u8]) -> bool {
    b.len() >= FRAME_LEN && CRC16_IBM_3740.checksum(&b[..15]) == u16::from_be_bytes([b[15], b[16]])
}

/// Builds a 17-byte frame: marker, device number, device type, transmission type, length 0x0A, the payload and the CRC.
pub fn build(marker: u16, device: u16, dtype: u8, tx: u8, payload: &[u8; 8]) -> [u8; FRAME_LEN] {
    let mut b = [0u8; FRAME_LEN];
    b[..2].copy_from_slice(&marker.to_be_bytes());
    b[2..4].copy_from_slice(&device.to_le_bytes());
    b[4] = dtype;
    b[5] = tx;
    b[6] = 0x0A;
    b[7..15].copy_from_slice(payload);
    let c = CRC16_IBM_3740.checksum(&b[..15]);
    b[15..].copy_from_slice(&c.to_be_bytes());
    b
}

/// The size of the history rings of the demodulator, samples (a power of two).
const RING: usize = 8192;
/// The 24 bits of the end of the preamble and the ANT+ marker.
const SYNC_PLUS: u32 = 0xAAA6C5;
/// The rise of the power floor per sample, dB: a factor of 1 + 2e-7.
const FLOOR_RISE_DB: f32 = 8.685_889e-7;
/// The least power floor: -80 dBFS, below the noise of any radio. Without it a signal with no noise at all triggers on the
/// ringing of the channel filter before each burst.
const MIN_FLOOR: f32 = 1e-8;
/// The share of a drop the power floor follows at each sample. Half a drop a sample follows the minimum within a few samples, and
/// it never rounds the floor to 0 (a full step `f + (x - f)` in `f32` can).
const FLOOR_ATTACK: f32 = 0.5;

/// The demodulator (`ant.demod`): the phase step over one bit, the sync search, the eye and the frame.
pub struct Demod {
    rate: f64,
    channel_hz: f64,
    /// The phase step over one bit (4 samples), Hz.
    disc: Option<LagDiscriminator>,
    sm: Vec<f32>,
    pw: Vec<f32>,
    pos: u64,
    pow96: WindowSum<f32>,
    floor: MinFollower,
    floor_now: f32,
    last_cand: u64,
    busy_until: u64,
    pending: VecDeque<(u64, bool)>,
    other_networks: bool,
    bad: u64,
    syncs: u64,
    pat: [f32; 24],
}

impl Default for Demod {
    fn default() -> Self {
        let mut pat = [0f32; 24];
        for (k, v) in pat.iter_mut().enumerate() {
            *v = if (SYNC_PLUS >> (23 - k)) & 1 != 0 {
                1.0
            } else {
                -1.0
            };
        }
        Demod {
            rate: DEMOD_RATE,
            channel_hz: 0.0,
            disc: None,
            sm: vec![0.0; RING],
            pw: vec![0.0; RING],
            pos: 0,
            pow96: WindowSum::new(96).expect("a window of 96"),
            floor: MinFollower::new(FLOOR_ATTACK, FLOOR_RISE_DB).expect("a valid follower"),
            floor_now: f32::MAX,
            last_cand: 0,
            busy_until: 0,
            pending: VecDeque::with_capacity(256),
            other_networks: true,
            bad: 0,
            syncs: 0,
            pat,
        }
    }
}

impl Demod {
    fn sm_at(&self, t: i64) -> f32 {
        self.sm[(t as usize) & (RING - 1)]
    }

    /// The frame whose sync search fired at sample `n`: the widest eye within 3 samples, then the 136 bits at three bit
    /// alignments, kept when the CRC passes.
    fn frame(&mut self, n: u64, plus: bool, out: &mut Outbox) {
        let mut best: Option<(i64, f32, f32, f32)> = None;
        let mut bits = [0f32; 24];
        for sh in -3i64..=3 {
            let e = n as i64 + sh;
            if e < 200 {
                continue;
            }
            for (k, b) in bits.iter_mut().enumerate() {
                *b = self.sm_at(e - 4 * (23 - k as i64));
            }
            let m = bits.iter().sum::<f32>() / 24.0;
            let (mut a, mut na, mut b, mut nb) = (0f32, 0, 0f32, 0);
            for k in 0..24 {
                let one = if plus { self.pat[k] > 0.0 } else { bits[k] > m };
                if one {
                    a += bits[k];
                    na += 1;
                } else {
                    b += bits[k];
                    nb += 1;
                }
            }
            if na == 0 || nb == 0 {
                continue;
            }
            let (one, zero) = (a / na as f32, b / nb as f32);
            let eye = one - zero;
            if best.is_none_or(|x| eye > x.1) {
                best = Some((sh, eye, (one + zero) / 2.0, eye / 2.0));
            }
        }
        let Some((sh, eye, mid, dev)) = best else {
            return;
        };
        if eye < 100e3 {
            return;
        }
        let e = n as i64 + sh;
        let start = e - 4 * (23 - 8);
        let mut chosen = None;
        for off in [0i64, 1, -1] {
            let mut b = [0u8; FRAME_LEN];
            for j in 0..136i64 {
                if self.sm_at(start + 4 * (j + off)) > mid {
                    b[(j / 8) as usize] |= 0x80 >> (j % 8);
                }
            }
            if crc_ok(&b) && b[6] == 0x0A {
                chosen = Some((b, off));
                break;
            }
        }
        let Some((b, off)) = chosen else {
            if plus {
                self.bad += 1;
            }
            return;
        };
        let marker = u16::from_be_bytes([b[0], b[1]]);
        if (plus && marker != ANT_PLUS)
            || (!plus && marker == ANT_PLUS)
            || (marker != ANT_PLUS && !self.other_networks)
        {
            return;
        }
        let mut var = 0f64;
        for j in 0..136i64 {
            let v = self.sm_at(start + 4 * (j + off));
            let ideal = if v > mid { mid + dev } else { mid - dev };
            var += ((v - ideal) as f64).powi(2);
        }
        let var = var / 136.0;
        let snr = 10.0 * ((dev as f64).powi(2) / var.max(1e-6)).log10();
        let p0 = (start - 32).max(0);
        let p1 = start + 4 * 136;
        let lvl = (p0..p1)
            .map(|t| self.pw[(t as usize) & (RING - 1)] as f64)
            .sum::<f64>()
            / (p1 - p0) as f64;
        self.busy_until = p1 as u64;
        let time = p0 as f64 / self.rate;
        out.items::<AntFrame>(0).push(Timed {
            time,
            value: AntFrame {
                bytes: b,
                level_dbfs: (10.0 * lvl.max(1e-20).log10()) as f32,
                snr_db: snr as f32,
                offset_hz: mid,
                dev_hz: dev,
                freq_hz: self.channel_hz + mid as f64,
            },
        });
    }

    /// The sync search at the newest sample: the ANT+ pattern, else (with other networks on) an alternating preamble.
    fn search(&mut self) -> Option<bool> {
        let n = self.pos as i64 - 1;
        let mut v = [0f32; 24];
        for (k, x) in v.iter_mut().enumerate() {
            *x = self.sm_at(n - 4 * (23 - k as i64));
        }
        let m = v.iter().sum::<f32>() / 24.0;
        let (mut sabs, mut corr) = (0f32, 0f32);
        for k in 0..24 {
            let d = v[k] - m;
            sabs += d.abs();
            corr += d * self.pat[k];
        }
        if sabs > 24.0 * 20e3 && corr > 0.72 * sabs {
            return Some(true);
        }
        if !self.other_networks {
            return None;
        }
        let (mut a, mut s9) = (0f32, 0f32);
        for (k, x) in v.iter().take(9).enumerate() {
            let d = x - m;
            a += if k % 2 == 0 { d } else { -d };
            s9 += d.abs();
        }
        if s9 > 9.0 * 40e3 && a.abs() > 0.8 * s9 && corr <= 0.3 * sabs {
            let alternations = (0..16).filter(|&k| (v[k] > m) != (v[k + 1] > m)).count();
            if alternations < 15 {
                return Some(false);
            }
        }
        None
    }
}

impl Decoder for Demod {
    type Input = Complex32;

    fn name(&self) -> &'static str {
        "ant.demod"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<AntFrame>("frames").rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<sdrkit_core::Schema> {
        DemodSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DemodSetting::OtherNetworks => self.other_networks = OTHER_NETWORKS.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([(
            DemodSetting::OtherNetworks,
            Value::Bool(self.other_networks),
        )])
    }
    fn results(&self) -> Published {
        Published::new(AntDemodFields::SHAPE.fields)
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        let other = self.other_networks;
        let (bad, syncs) = (self.bad, self.syncs);
        *self = Demod {
            other_networks: other,
            bad,
            syncs,
            ..Demod::default()
        };
        self.rate = input.rate;
        self.channel_hz = input.tuned_hz;
        self.disc = Some(LagDiscriminator::new(self.rate, 1.0, 4)?);
        let mut o = input.with_kind(Kind::typed::<AntFrame>());
        o.rate = 0.0;
        Ok(vec![o])
    }
    fn retune(&mut self, input: &Format) {
        self.channel_hz = input.tuned_hz;
        self.pending.clear();
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        let _ = first;
        for &s in x {
            let i = (self.pos as usize) & (RING - 1);
            let p = s.norm_sqr();
            self.pw[i] = p;
            let pow96 = self.pow96.push(p);
            // The frequency over one bit: the phase advance across 4 samples.
            self.sm[i] = self.disc.as_mut().map_or(0.0, |d| d.process_sample(s));
            let pavg = (pow96 / 96.0) as f32;
            self.pos += 1;
            if self.pos > 96 {
                self.floor_now = self
                    .floor
                    .push(pavg.max(MIN_FLOOR))
                    .unwrap_or(self.floor_now);
            }
            if self.pos > 200
                && pavg > 6.0 * self.floor_now
                && self.pos > self.busy_until
                && self.pos >= self.last_cand + 4
            {
                if let Some(plus) = self.search() {
                    self.syncs += 1;
                    self.last_cand = self.pos;
                    self.pending.push_back((self.pos - 1, plus));
                }
            }
            while let Some(&(t, plus)) = self.pending.front() {
                if self.pos < t + 4 * 140 + 8 {
                    break;
                }
                self.pending.pop_front();
                if t + 1 > self.busy_until {
                    self.frame(t, plus, out);
                }
            }
        }
    }
    fn report(&mut self, r: &mut Published) {
        r.put(&AntDemodFields {
            frames_bad: self.bad as i64,
            syncs: self.syncs as i64,
        });
    }
    fn clear(&mut self) {
        self.bad = 0;
        self.syncs = 0;
    }
    fn forget(&mut self, kept: &Kept) {
        self.pending.clear();
        if !kept.field("frames_bad") {
            self.bad = 0;
        }
        if !kept.field("syncs") {
            self.syncs = 0;
        }
    }
}

/// Also decode frames of networks other than ANT+.
pub(crate) const OTHER_NETWORKS: Setting<field::Bool> =
    Setting::new("other_networks", "Other ANT networks", "", true)
        .help("Also decodes frames of networks other than ANT+.");

const WHEEL_MM: Setting<field::F64> =
    Setting::new("wheel_mm", "Wheel circumference", "mm", 2096.0).range(1000.0, 3000.0, 1.0);

sdrkit_core::settings! {
    /// The settings of [`Demod`].
    pub enum DemodSetting {
        /// Other ANT networks.
        OtherNetworks = OTHER_NETWORKS,
    }
}

sdrkit_core::settings! {
    /// The settings of [`Sensors`].
    pub enum SensorsSetting {
        /// Wheel circumference.
        WheelMm = WHEEL_MM,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Sensors`].
    pub enum SensorsCommand {
        /// Forgets everything heard.
        Clear = commands::CLEAR,
        /// Forgets one sensor.
        Remove = ANT_REMOVE.command,
    }
}

/// The name of a device type.
pub fn type_name(t: u8) -> &'static str {
    match t {
        120 => "Heart rate monitor",
        121 => "Bike speed and cadence",
        122 => "Bike cadence",
        123 => "Bike speed",
        124 => "Footpod (stride)",
        11 => "Bike power meter",
        17 => "Fitness equipment",
        15 => "Multi-sport speed and distance",
        119 => "Weight scale",
        16 => "Remote control",
        25 => "Environment sensor",
        20 => "Light electric vehicle",
        18 => "Blood pressure monitor",
        19 => "Geocache",
        31 => "Muscle oxygen monitor",
        40 => "Bike radar",
        35 => "Bike lights",
        34 => "Shifting",
        _ => "ANT device",
    }
}

/// The FIT name of an ANT+ manufacturer id (`sdrkit_util_ids::ant`), or "manufacturer n" for an id not in the list.
pub fn manufacturer(id: u16) -> String {
    sdrkit_util_ids::ant::manufacturer(id)
        .map(String::from)
        .unwrap_or_else(|| format!("manufacturer {id}"))
}

/// One sensor, keyed by device number, type and transmission type.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sensor {
    /// The device number.
    pub device: u16,
    /// The device type.
    pub dtype: u8,
    /// The transmission type.
    pub tx_type: u8,
    /// The network marker.
    pub marker: u16,
    /// The pairing bit was seen.
    pub pairing: bool,
    /// The manufacturer.
    pub manufacturer: Option<String>,
    /// The serial number.
    pub serial: Option<u32>,
    /// The model number.
    pub model: Option<u16>,
    /// The hardware revision.
    pub hw: Option<u8>,
    /// The software revision.
    pub sw: Option<u8>,
    /// The battery charge, %.
    pub battery_pct: Option<u8>,
    /// The battery status.
    pub battery_status: Option<String>,
    /// The battery voltage, V.
    pub battery_v: Option<f32>,
    /// The operating time, s.
    pub operating_s: Option<u32>,
    /// The newest heart rate, bpm.
    pub heart_rate: Option<u8>,
    /// The beat count.
    pub beat_count: Option<u8>,
    /// The R-R interval, ms.
    pub rr_ms: Option<f32>,
    /// The power, W.
    pub power_w: Option<u16>,
    /// The cadence, rpm.
    pub cadence: Option<f32>,
    /// The speed, km/h.
    pub speed_kmh: Option<f32>,
    /// The distance, m.
    pub distance_m: Option<f32>,
    /// The temperature, °C.
    pub temperature_c: Option<f32>,
    /// The text of the last page.
    pub last_page: String,
    /// The pages seen, one bit for each page number 0 to 127.
    pub pages: u128,
    /// The level of the last frame, dBFS.
    pub level_dbfs: f32,
    /// The carrier frequency of the last frame, Hz.
    pub freq_hz: f64,
    /// The signal time of the last frame, s.
    pub last_s: f64,
    /// The measured message period, s.
    pub period_s: Option<f64>,
    /// The main value of the profile over time: (signal time s, value).
    pub history: VecDeque<[f32; 2]>,
    /// The decision level, smoothed 0.7 old and 0.3 new, Hz.
    pub level_hz: f32,
    #[serde(skip)]
    cad: Option<(u16, u16)>,
    #[serde(skip)]
    spd: Option<(u16, u16)>,
}

impl Shift for Sensor {
    fn shift_times(&mut self, by_s: f64) {
        self.last_s += by_s;
        for h in &mut self.history {
            h[0] += by_s as f32;
        }
    }
}

fn battery_status(s: u8) -> &'static str {
    match s {
        1 => "new",
        2 => "good",
        3 => "ok",
        4 => "low",
        5 => "critical",
        _ => "unknown",
    }
}

fn push_hist(s: &mut Sensor, t: f64, v: f32) {
    if s.history.len() >= HISTORY_CAP {
        s.history.pop_front();
    }
    s.history.push_back([t as f32, v]);
}

/// Decodes payload `p` (8 bytes) of sensor `s` heard at signal time `t`: updates the sensor and returns the page text. The
/// wheel circumference `wheel_m` is used to compute the speed of a wheel sensor.
pub fn decode_page(s: &mut Sensor, p: &[u8], t: f64, wheel_m: f64) -> String {
    if p.len() < 8 {
        return String::new();
    }
    let page = p[0] & 0x7F;
    let le = |i: usize| u16::from_le_bytes([p[i], p[i + 1]]);
    let common = |s: &mut Sensor| -> Option<String> {
        match page {
            80 => {
                s.hw = Some(p[3]);
                s.manufacturer = Some(manufacturer(le(4)));
                s.model = Some(le(6));
                Some(format!(
                    "Manufacturer: {}, model {}, hw {}",
                    manufacturer(le(4)),
                    le(6),
                    p[3]
                ))
            }
            81 => {
                s.sw = Some(p[3]);
                let ser = u32::from_le_bytes([p[4], p[5], p[6], p[7]]);
                s.serial = Some(ser);
                Some(format!("Product: sw {}, serial {ser}", p[3]))
            }
            82 => {
                if p[2] != 0xFF {
                    s.battery_pct = Some(p[2]);
                }
                let v = (p[7] & 0xF) as f32 + p[6] as f32 / 256.0;
                s.battery_v = (p[7] & 0xF != 0xF).then_some(v);
                let st = battery_status((p[7] >> 4) & 7);
                s.battery_status = Some(st.into());
                Some(format!("Battery: {v:.2} V, {st}"))
            }
            _ => None,
        }
    };
    let other = |p: &[u8]| format!("page {page}: {}", hex(&p[1..8]));
    match s.dtype {
        120 => {
            let bt = le(4);
            let hr = p[7];
            s.beat_count = Some(p[6]);
            let mut extra = String::new();
            match page {
                1 => {
                    let secs = (p[1] as u32 | (p[2] as u32) << 8 | (p[3] as u32) << 16) * 2;
                    s.operating_s = Some(secs);
                    extra = format!(", operating time {secs} s");
                }
                2 => {
                    s.manufacturer = Some(manufacturer(p[1] as u16));
                    s.serial = Some(le(2) as u32);
                    extra = format!(
                        ", manufacturer {}, serial {}",
                        manufacturer(p[1] as u16),
                        le(2)
                    );
                }
                3 => {
                    s.hw = Some(p[1]);
                    s.sw = Some(p[2]);
                    s.model = Some(p[3] as u16);
                }
                4 => {
                    let rr = bt.wrapping_sub(le(2));
                    let ms = rr as f32 * 1000.0 / 1024.0;
                    s.rr_ms = Some(ms);
                    extra = format!(", R-R {ms:.0} ms");
                }
                7 => {
                    s.battery_pct = Some(p[1]).filter(|&x| x != 0xFF);
                    let v = (p[3] & 0xF) as f32 + p[2] as f32 / 256.0;
                    s.battery_v = (p[3] & 0xF != 0xF).then_some(v);
                    s.battery_status = Some(battery_status((p[3] >> 4) & 7).into());
                }
                _ => {}
            }
            if hr == 0 {
                return "no heart rate (sensor not on the skin?)".into();
            }
            s.heart_rate = Some(hr);
            push_hist(s, t, hr as f32);
            format!(
                "{hr} bpm, beat {} at {:.3} s{extra}",
                p[6],
                bt as f64 / 1024.0
            )
        }
        121 => {
            let (ct, cr, st, sr) = (le(0), le(2), le(4), le(6));
            if let Some((pt, pr)) = s.cad {
                let dr = cr.wrapping_sub(pr);
                let dt = ct.wrapping_sub(pt);
                if dr > 0 && dr < 50 && dt > 0 {
                    s.cadence = Some(dr as f32 * 60.0 * 1024.0 / dt as f32);
                }
            }
            if let Some((pt, pr)) = s.spd {
                let dr = sr.wrapping_sub(pr);
                let dt = st.wrapping_sub(pt);
                if dr > 0 && dr < 200 && dt > 0 {
                    let v = (dr as f64 * wheel_m * 1024.0 / dt as f64 * 3.6) as f32;
                    s.speed_kmh = Some(v);
                    push_hist(s, t, v);
                }
            }
            s.cad = Some((ct, cr));
            s.spd = Some((st, sr));
            format!(
                "cadence {:.0} rpm, speed {:.1} km/h",
                s.cadence.unwrap_or(0.0),
                s.speed_kmh.unwrap_or(0.0)
            )
        }
        122 | 123 => {
            if let Some(c) = common(s) {
                return c;
            }
            let (et, r) = (le(4), le(6));
            let prev = if s.dtype == 122 { s.cad } else { s.spd };
            if let Some((pt, pr)) = prev {
                let dr = r.wrapping_sub(pr);
                let dt = et.wrapping_sub(pt);
                if dt > 0 && dr > 0 {
                    if s.dtype == 122 && dr < 50 {
                        let c = dr as f32 * 60.0 * 1024.0 / dt as f32;
                        s.cadence = Some(c);
                        push_hist(s, t, c);
                    } else if s.dtype == 123 && dr < 200 {
                        let v = (dr as f64 * wheel_m * 1024.0 / dt as f64 * 3.6) as f32;
                        s.speed_kmh = Some(v);
                        push_hist(s, t, v);
                    }
                }
            }
            if s.dtype == 122 {
                s.cad = Some((et, r));
            } else {
                s.spd = Some((et, r));
            }
            format!("page {page}")
        }
        11 => match page {
            0x10 => {
                let pw = le(6);
                s.power_w = Some(pw);
                if p[3] != 0xFF {
                    s.cadence = Some(p[3] as f32);
                }
                push_hist(s, t, pw as f32);
                let bal = if p[2] == 0xFF {
                    String::new()
                } else {
                    format!(
                        ", {} {}%",
                        if p[2] & 0x80 != 0 { "right" } else { "pedal" },
                        p[2] & 0x7F
                    )
                };
                format!("Power {pw} W, cadence {} rpm{bal}", p[3])
            }
            0x11 => "Wheel torque".into(),
            0x12 => "Crank torque".into(),
            0x01 => format!("Calibration {}", p[1]),
            0x02 => "Parameters".into(),
            _ => common(s).unwrap_or_else(|| other(p)),
        },
        124 => match page {
            1 | 2 => {
                let speed = (p[4] & 0xF) as f32 + p[5] as f32 / 256.0;
                s.speed_kmh = Some(speed * 3.6);
                push_hist(s, t, speed * 3.6);
                if page == 1 {
                    let d = p[3] as f32 + (p[4] >> 4) as f32 / 16.0;
                    s.distance_m = Some(d);
                    format!("distance {d:.1} m, speed {speed:.2} m/s, strides {}", p[6])
                } else {
                    s.cadence = Some(p[3] as f32);
                    format!("cadence {} strides/min, speed {speed:.2} m/s", p[3])
                }
            }
            _ => common(s).unwrap_or_else(|| other(p)),
        },
        25 => match page {
            1 => {
                let temp = i16::from_le_bytes([p[6], p[7]]) as f32 / 100.0;
                s.temperature_c = Some(temp);
                push_hist(s, t, temp);
                let lo = ((((p[4] & 0xF) as i16) << 8 | p[3] as i16) << 4 >> 4) as f32 / 10.0;
                let hi = ((((p[5] as i16) << 4) | (p[4] >> 4) as i16) << 4 >> 4) as f32 / 10.0;
                format!("temperature {temp:.2} C, 24 h low {lo:.1}, high {hi:.1}")
            }
            _ => common(s).unwrap_or_else(|| other(p)),
        },
        _ => common(s).unwrap_or_else(|| other(p)),
    }
}

/// The value column of a sensor: its main measurement, in words.
fn main_value(s: &Sensor) -> String {
    if let Some(h) = s.heart_rate {
        format!("{h} bpm")
    } else if let Some(p) = s.power_w {
        format!("{p} W")
    } else if let Some(v) = s.speed_kmh {
        format!("{v:.1} km/h")
    } else if let Some(c) = s.cadence {
        format!("{c:.0} rpm")
    } else if let Some(t) = s.temperature_c {
        format!("{t:.1} °C")
    } else {
        s.last_page.clone()
    }
}

impl Shaped for Sensor {
    type Row = AntSensorRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let name = format!("{} {}", type_name(self.dtype), self.device);
        out.put(&AntSensorRow {
            name: &name,
            device: i64::from(self.device),
            r#type: type_name(self.dtype),
            marker: &format!("{:04X}", self.marker),
            manufacturer: self.manufacturer.as_deref(),
            serial: self.serial.map(i64::from),
            value: &main_value(self),
            battery: self.battery_status.as_deref(),
            period: self.period_s.map(Seconds),
            level_dbfs: f64::from(self.level_dbfs),
            frequency: Hertz(self.freq_hz),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// A line of the frame log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameLine {
    /// The signal time, s.
    pub time_s: f64,
    /// The device number.
    pub device: u16,
    /// The device type.
    pub dtype: u8,
    /// The transmission type.
    pub tx_type: u8,
    /// The network marker.
    pub marker: u16,
    /// The level, dBFS.
    pub level_dbfs: f32,
    /// The payload.
    pub payload: [u8; 8],
    /// "Page n: text".
    pub text: String,
}

impl Shift for FrameLine {
    fn shift_times(&mut self, by_s: f64) {
        self.time_s += by_s;
    }
}

impl Shaped for FrameLine {
    type Row = AntFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AntFrameRow {
            time_s: self.time_s,
            device: i64::from(self.device),
            r#type: type_name(self.dtype),
            tx_type: i64::from(self.tx_type),
            marker: &format!("{:04X}", self.marker),
            level_dbfs: f64::from(self.level_dbfs),
            payload: &hex(&self.payload),
            text: &self.text,
        });
    }
}

/// What `ant` keeps: the sensors and the frame log.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The sensors, keyed by device number, type and transmission type.
    pub sensors: ObjectTable<u32, Sensor>,
    /// The frame log.
    pub log: LogRing<FrameLine>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            sensors: ObjectTable::new(SENSOR_CAP),
            log: LogRing::new(LOG_CAP),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.sensors.shift_times(by_s);
        self.log.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("ism.ant", 1);
    fn clear(&mut self) {
        self.sensors.clear();
        self.log.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.sensors.adopt(saved.sensors);
        self.log.adopt(saved.log);
    }
}

/// The key of a sensor: device number, type and transmission type.
pub fn sensor_key(device: u16, dtype: u8, tx_type: u8) -> u32 {
    (device as u32) << 16 | (dtype as u32) << 8 | tx_type as u32
}

/// The sensor table (`ant.sensors`): profiles, sensors, the frame log, the status and the events.
pub struct Sensors {
    store: Store,
    published: Published,
    sensors_table: TableId,
    history_id: Option<sdrkit_core::SeriesId>,
    wheel_m: f64,
    channel_hz: f64,
    snr: f32,
    dev: Option<f32>,
    good: u64,
    now: f64,
    last_frame: f64,
    newest: Option<u32>,
    events: Vec<Event>,
    new_count: usize,
}

impl Default for Sensors {
    fn default() -> Self {
        let published = Published::from_decl(Arc::new(
            ResultsDecl::new()
                .with_fields(AntFields::SHAPE.fields)
                .with_table(&ANT_SENSORS)
                .with_table(&ANT_FRAMES)
                .with_series(&ANT_HISTORY),
        ));
        let decl = published.results().decl();
        let sensors_table = decl
            .table_id(ANT_SENSORS.key)
            .expect("the tables of ant.sensors are declared");
        let history_id = decl.series_id(ANT_HISTORY.key);
        Sensors {
            store: Store::default(),
            published,
            sensors_table,
            history_id,
            wheel_m: 2.096,
            channel_hz: 0.0,
            snr: 0.0,
            dev: None,
            good: 0,
            now: 0.0,
            last_frame: f64::NEG_INFINITY,
            newest: None,
            events: Vec::new(),
            new_count: 0,
        }
    }
}

impl Sensors {
    /// The sensors held.
    pub fn store(&self) -> &Store {
        &self.store
    }

    fn frame(&mut self, t: f64, f: &AntFrame) {
        self.good += 1;
        self.now = self.now.max(t);
        self.last_frame = t;
        self.snr = 0.8 * self.snr + 0.2 * f.snr_db;
        self.dev = Some(f.dev_hz);
        let key = sensor_key(f.device(), f.device_type(), f.tx_type());
        let Ok(up) = self
            .store
            .sensors
            .upsert(&key, Stamp::at(t), Sensor::default)
        else {
            return;
        };
        let s = up.row;
        if up.new {
            s.device = f.device();
            s.dtype = f.device_type();
            s.tx_type = f.tx_type();
            s.marker = f.marker();
            if self.new_count < 20 {
                self.new_count += 1;
                let other = if f.marker() != ANT_PLUS {
                    " (another ANT network)"
                } else {
                    ""
                };
                let text = format!(
                    "ANT+: new sensor {}, {}{other}",
                    s.device,
                    type_name(s.dtype)
                );
                self.events.push(ANT_SENSOR.event(
                    t,
                    f.freq_hz,
                    &AntSensorEvent {
                        source: &s.device.to_string(),
                        text: &text,
                        family: Family::Sensor,
                        device: i64::from(s.device),
                        r#type: type_name(s.dtype),
                        marker: &format!("{:04X}", s.marker),
                    },
                ));
            }
        } else if t > s.last_s {
            let mut dt = t - s.last_s;
            // A missed frame results in a multiple of the period.
            if let Some(p) = s.period_s {
                let k = (dt / p).round();
                if k >= 2.0 && (dt / k - p).abs() < 0.1 * p {
                    dt /= k;
                }
            }
            if (0.05..=5.0).contains(&dt) {
                s.period_s = Some(s.period_s.map_or(dt, |p| 0.9 * p + 0.1 * dt));
            }
        }
        s.level_hz = if up.new {
            f.offset_hz
        } else {
            0.7 * s.level_hz + 0.3 * f.offset_hz
        };
        s.last_s = t;
        s.level_dbfs = f.level_dbfs;
        s.freq_hz = f.freq_hz;
        s.pairing |= f.pairing();
        let p = f.payload();
        s.pages |= 1u128 << (p[0] & 0x7F);
        let text = decode_page(s, p, t, self.wheel_m);
        s.last_page.clone_from(&text);
        let mut payload = [0u8; 8];
        payload.copy_from_slice(p);
        self.store.log.push(FrameLine {
            time_s: t,
            device: f.device(),
            dtype: f.device_type(),
            tx_type: f.tx_type(),
            marker: f.marker(),
            level_dbfs: f.level_dbfs,
            payload,
            text: format!("Page {}: {text}", p[0] & 0x7F),
        });
        self.newest = Some(key);
    }

    fn publish(&mut self) {
        let locked = self.now - self.last_frame < 5.0;
        let mut levels: Vec<f32> = self
            .store
            .sensors
            .iter()
            .map(|r| r.record.level_hz)
            .collect();
        levels.sort_by(f32::total_cmp);
        let offset = levels.get(levels.len() / 2).copied();
        let strap = self
            .store
            .sensors
            .iter_newest_first()
            .find(|r| r.record.heart_rate.is_some());
        let n = self.store.sensors.len();
        let summary = match strap {
            Some(h) => format!(
                "{} bpm (sensor {}), {n} sensors, {} frames",
                h.record.heart_rate.unwrap_or(0),
                h.record.device,
                self.good
            ),
            None if self.good == 0 => format!(
                "listening for ANT+ sensors on {:.0} MHz",
                self.channel_hz / 1e6
            ),
            None => format!("{n} sensors, {} frames", self.good),
        };
        let hr = strap.and_then(|h| h.record.heart_rate);
        let history = self
            .newest
            .and_then(|k| self.store.sensors.get(&k))
            .map(|s| &s.history);
        let p = &mut self.published;
        {
            let r = p.results_mut();
            r.set_status(&Status {
                lock_state: LockState::from_level(if locked { 2 } else { 0 }),
                snr_db: self.dev.map(|_| self.snr),
                level_dbfs: None,
                offset_hz: offset.map(f64::from),
                summary,
            });
            if let (Some(id), Some(h)) = (self.history_id, history) {
                r.update_series(id, |s| {
                    let same = s.line && s.points.iter().eq(h.iter());
                    if !same {
                        s.line = true;
                        s.points.clear();
                        s.points.extend(h.iter().copied());
                    }
                    !same
                });
            }
        }
        p.put(&AntFields {
            frames_ok: self.good as i64,
            sensors: n as i64,
            heart_rate_bpm: hr.map(f64::from),
            deviation: self.dev.map(|d| Hertz(f64::from(d))),
            channel: Hertz(self.channel_hz),
        });
        shaped::rows(&self.store.sensors, &ANT_SENSORS, self.now, p);
        shaped::log_rows(&self.store.log, &ANT_FRAMES, self.now, p);
    }
}

impl Component for Sensors {
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ant.sensors",
            vec![Port::typed::<AntFrame>("frames")],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(SensorsSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.channel_hz = inputs.first().map_or(0.0, |f| f.tuned_hz);
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut new = false;
        for f in io.items::<AntFrame>(0) {
            self.frame(f.time, &f.value);
            new = true;
        }
        let tick = io.tick();
        if tick {
            self.now += REPORT_S;
        }
        if new || tick {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SensorsSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SensorsSetting::WheelMm => self.wheel_m = WHEEL_MM.read(id, v)? / 1000.0,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(SensorsSetting::WheelMm, Value::F64(self.wheel_m * 1000.0))])
    }
    fn commands(&self) -> &Commands {
        SensorsCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match SensorsCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SensorsCommand::Clear => {
                self.store.clear();
                self.newest = None;
                self.published.clear_rows(&ANT_SENSORS);
                self.published.clear_rows(&ANT_FRAMES);
            }
            SensorsCommand::Remove => {
                commands::remove(
                    &mut self.store.sensors,
                    self.sensors_table,
                    &mut self.published,
                    args,
                    &ANT_SENSOR_ROW,
                )?;
                if self
                    .newest
                    .is_some_and(|k| self.store.sensors.get(&k).is_none())
                {
                    self.newest = None;
                }
            }
        }
        self.publish();
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(ANT_SENSORS.key) {
            self.store.sensors.clear();
            self.newest = None;
            self.published.clear_rows(&ANT_SENSORS);
        }
        if !kept.table(ANT_FRAMES.key) {
            self.store.log.clear();
            self.published.clear_rows(&ANT_FRAMES);
        }
        if !kept.field("frames_ok") {
            self.good = 0;
        }
        self.dev = None;
        self.snr = 0.0;
        self.last_frame = f64::NEG_INFINITY;
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

/// The `ant` receiver: the channel to 4 Msps, the demodulator and the sensor table.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among
/// them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add(
            "front",
            checked(
                "front",
                sdrkit_components::dsp::Channel::new(CHANNEL_WIDTH, DEMOD_RATE),
            )?,
        )
        .set("front.exact", Value::Bool(true))
        .add("demod", DecoderComponent::new(Demod::default()))
        .add("sensors", Sensors::default())
        .add("sender", crate::tx::FrameSender::<send::AntFrames>::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("front")
        .input("sender.in")
        .connect("front", "demod")
        .connect("demod.frames", "sensors.frames")
        .connect("sender.out", "tx.in")
        .output("sensors.events")
        .output("sender.events")
        .clock(REPORT_S)
        .status_from("sensors")
        .build()?;
    StackGraph::of(crate::typed::shape("ant"), graph)
        .min_rate(DEMOD_RATE, &min_rate_message("ANT+ sensors", DEMOD_RATE))
        .route(OTHER_NETWORKS.key(), &["demod.other_networks"])?
        .route(WHEEL_MM.key(), &["sensors.wheel_mm"])?
        .route(crate::tx::TX_LEVEL.key(), &["sender.tx_level"])?
        .command("clear", sdrkit_graph::CommandTarget::of("sensors", "clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("sensors", "remove"),
        )
        .command("send", sdrkit_graph::CommandTarget::of("sender", "send"))
        .command("halt", sdrkit_graph::CommandTarget::of("sender", "halt"))
        .with_settings(&cfg.settings)
        .build()
}

sdrkit_core::choice_enum! {
    /// The sensors of the test signal.
    pub enum GenSensors {
        /// All three.
        All = ("all", "All three"),
        /// The heart rate strap alone.
        Strap = ("strap", "Heart rate strap alone"),
        /// No sensor.
        None = ("none", "No sensor"),
    }
}

/// The sensors of the test signal.
pub const GEN_SENSORS: Setting<field::Choice<GenSensors>> =
    Setting::choice("gen_sensors", "Test signal sensors", GenSensors::All)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `ant` stack.
    pub enum AntSetting {
        /// Other ANT networks.
        OtherNetworks = OTHER_NETWORKS,
        /// Wheel circumference.
        WheelMm = WHEEL_MM,
        /// The sensors of the test signal.
        GenSensors = GEN_SENSORS,
        /// The level sent.
        TxLevel = crate::tx::TX_LEVEL,
    }
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// A test sensor of the generator.
#[derive(Clone, Debug)]
pub struct TestSensor {
    /// The device number.
    pub device: u16,
    /// The device type.
    pub dtype: u8,
    /// The transmission type.
    pub tx: u8,
    /// The message period, s.
    pub period: f64,
    /// The carrier offset from the channel, Hz.
    pub offset: f64,
    /// The level, dB.
    pub level_db: f64,
    /// Every n-th frame has a bad CRC (0: none).
    pub bad_every: u64,
    /// The network marker.
    pub marker: u16,
}

/// The test sensors: "all", "strap" or "none".
pub fn test_sensors(which: GenSensors) -> Vec<TestSensor> {
    let hr = TestSensor {
        device: 0x6941,
        dtype: 120,
        tx: 1,
        period: 8070.0 / 32768.0,
        offset: 0.0,
        level_db: 0.0,
        bad_every: 0,
        marker: ANT_PLUS,
    };
    match which {
        GenSensors::Strap => vec![hr],
        GenSensors::None => vec![],
        GenSensors::All => vec![
            hr,
            TestSensor {
                device: 12345,
                dtype: 11,
                tx: 5,
                period: 8182.0 / 32768.0,
                offset: 12e3,
                level_db: -3.0,
                bad_every: 0,
                marker: ANT_PLUS,
            },
            TestSensor {
                device: 777,
                dtype: 121,
                tx: 1,
                period: 8086.0 / 32768.0,
                offset: -20e3,
                level_db: -6.0,
                bad_every: 0,
                marker: ANT_PLUS,
            },
        ],
    }
}

/// The heart rate the strap sends in message `m`.
pub fn hr_of(m: u64) -> f64 {
    79.5 + 19.5 * (0.02 * m as f64).sin()
}

/// The payload of message `m` of a test sensor; `state` carries the counters between messages.
pub fn payload(ts: &TestSensor, m: u64, state: &mut [f64; 4]) -> [u8; 8] {
    let mut p = [0xFFu8; 8];
    match ts.dtype {
        120 => {
            // state: [beat time in 1/1024 s, previous beat time, beat count, beat phase]
            let hr = hr_of(m);
            state[3] += ts.period * hr / 60.0;
            while state[3] >= 1.0 {
                state[3] -= 1.0;
                state[1] = state[0];
                state[0] += 60.0 / hr * 1024.0;
                state[2] += 1.0;
            }
            let bt = (state[0] as u64 & 0xFFFF) as u16;
            let toggle = ((m / 4) % 2) as u8;
            let pos = m % 65;
            let page = if (61..=64).contains(&pos) {
                [1, 2, 3, 7][(pos - 61) as usize]
            } else if m > 10 && m.is_multiple_of(3) {
                4
            } else {
                0
            };
            p[0] = page | (toggle << 7);
            match page {
                1 => {
                    let t = ((m as f64 * ts.period) / 2.0) as u32;
                    p[1..4].copy_from_slice(&t.to_le_bytes()[..3]);
                }
                2 => {
                    p[1] = 1;
                    p[2..4].copy_from_slice(&12345u16.to_le_bytes());
                }
                3 => {
                    p[1] = 2;
                    p[2] = 11;
                    p[3] = 7;
                }
                4 => {
                    let prev = (state[1] as u64 & 0xFFFF) as u16;
                    p[2..4].copy_from_slice(&prev.to_le_bytes());
                }
                7 => {
                    p[1] = 85;
                    p[2] = 128;
                    p[3] = 0x20 | 2;
                }
                _ => {}
            }
            p[4..6].copy_from_slice(&bt.to_le_bytes());
            p[6] = state[2] as u8;
            p[7] = hr.round() as u8;
        }
        11 => {
            let pos = m % 121;
            if pos == 118 {
                p = [80, 0xFF, 0xFF, 1, 32, 0, 11, 0];
            } else if pos == 119 {
                p = [81, 0xFF, 0xFF, 3, 0xD2, 0x04, 0, 0];
            } else if pos == 120 {
                p = [82, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 64, 0x30 | 3];
            } else {
                state[0] += 180.0;
                let acc = (state[0] as u64 & 0xFFFF) as u16;
                p = [0x10, m as u8, 0x80 | 52, 85, 0, 0, 0, 0];
                p[4..6].copy_from_slice(&acc.to_le_bytes());
                p[6..8].copy_from_slice(&180u16.to_le_bytes());
            }
        }
        _ => {
            // Speed and cadence: 25 km/h with a 2096 mm wheel, 85 rpm.
            let t = m as f64 * ts.period;
            let crank = t * 85.0 / 60.0;
            let wheel = t * (25.0 / 3.6) / 2.096;
            let ct = ((crank.floor() * 60.0 / 85.0) * 1024.0) as u64 as u16;
            let st = ((wheel.floor() * 2.096 / (25.0 / 3.6)) * 1024.0) as u64 as u16;
            p[0..2].copy_from_slice(&ct.to_le_bytes());
            p[2..4].copy_from_slice(&(crank.floor() as u64 as u16).to_le_bytes());
            p[4..6].copy_from_slice(&st.to_le_bytes());
            p[6..8].copy_from_slice(&(wheel.floor() as u64 as u16).to_le_bytes());
        }
    }
    p
}

/// The levels of a frame for the modulator: 4 bits of plain carrier, the preamble (0xAA or 0x55 into the marker), the 17 bytes
/// and 4 bits of plain carrier.
pub fn frame_levels(bytes: &[u8], out: &mut Vec<f32>) {
    let pre = if bytes[0] & 0x80 != 0 { 0xAA } else { 0x55 };
    let mut bits = Vec::with_capacity(8 * (bytes.len() + 1));
    unpack(&[pre], BitOrder::Msb, &mut bits);
    unpack(bytes, BitOrder::Msb, &mut bits);
    out.extend([0.0; 4]);
    out.extend(bits.iter().map(|&b| if b != 0 { 1.0 } else { -1.0 }));
    out.extend([0.0; 4]);
}

/// The GFSK modulator of ANT frames at `rate` samples a second: BT 0.5, ±160 kHz at 1 Mbit/s.
///
/// # Errors
///
/// The error of the modulator for a rate it refuses.
pub fn modulator(rate: f64) -> Result<Cpm, sdrkit_algo_sim::SimError> {
    Cpm::gfsk(0.5, 2.0 * 160e3 / BIT_RATE, rate / BIT_RATE)
}

/// Appends to `out` the clean samples at 0 Hz of the frame `bytes` ([`frame_levels`]) from `cpm`; `levels` is a buffer.
pub fn modulate(cpm: &mut Cpm, bytes: &[u8], levels: &mut Vec<f32>, out: &mut Vec<Complex32>) {
    levels.clear();
    frame_levels(bytes, levels);
    cpm.push(levels, out);
    cpm.flush(out);
}

/// The settings of the generator.
#[derive(Clone, Debug)]
pub struct GenParams {
    /// The sensors.
    pub sensors: Vec<TestSensor>,
    /// The seed of the content.
    pub seed: u64,
}

/// The `ant` generator at `rate` samples a second: every sensor sends its frames at its period, each a GFSK burst (BT 0.5,
/// ±160 kHz) at its offset and level. The signal is clean: noise and the receiver's errors come from the `gen_*` settings.
pub fn generator(rate: f64, p: GenParams) -> CleanScene {
    let rate = if rate > 0.0 && rate.is_finite() {
        rate
    } else {
        DEMOD_RATE
    };
    let info = SignalInfo::new(BIT_RATE)
        .with_symbol_rate(BIT_RATE)
        .with_bit_rate(BIT_RATE);
    let mut scene = CleanScene::new(Scene::new(rate).expect("a positive rate"), rate, info);
    for (i, ts) in p.sensors.into_iter().enumerate() {
        let Ok(mut cpm) = modulator(rate) else {
            continue;
        };
        let mut t = 0.01 + 0.037 * i as f64;
        let mut m = 0u64;
        let mut state = [0f64; 4];
        let mut levels = Vec::new();
        let source = move || {
            let pl = payload(&ts, m, &mut state);
            let mut b = build(ts.marker, ts.device, ts.dtype, ts.tx, &pl);
            if ts.bad_every > 0 && m % ts.bad_every == ts.bad_every - 1 {
                b[16] ^= 0x01;
            }
            let mut samples = Vec::new();
            modulate(&mut cpm, &b, &mut levels, &mut samples);
            let burst = Burst {
                start_s: t,
                offset_hz: ts.offset,
                level_db: ts.level_db,
                samples,
            };
            t += ts.period;
            m += 1;
            Some(burst)
        };
        scene.scene_mut().add_source(Box::new(source));
    }
    scene
}

/// The settings of the generator from a configuration.
pub fn gen_params(cfg: &GenConfig) -> GenParams {
    GenParams {
        sensors: test_sensors(cfg.settings.get(&GEN_SENSORS)),
        seed: cfg.seed,
    }
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(generator(cfg.rate_hz, gen_params(cfg))))
}

/// The time the demodulator takes for 1 s of a three-sensor signal at 4 Msps (`ism.ant_demod`).
pub fn benchmark() -> Benchmark {
    Benchmark {
        name: "ism.ant_demod",
        run: |seconds| -> BenchResult {
            let mut g = generator(
                DEMOD_RATE,
                GenParams {
                    sensors: test_sensors(GenSensors::All),
                    seed: 1,
                },
            );
            let mut x = vec![Complex32::default(); DEMOD_RATE as usize];
            g.generate(&mut x);
            let mut d = Demod::default();
            let f = Format::iq(DEMOD_RATE, 0.0, 0.0);
            let _ = d.configure(&f);
            let mut out = Outbox::new(&[Kind::typed::<AntFrame>()]);
            let mut first = 0u64;
            measure(seconds, x.len(), 1.0, || {
                d.process(&x, first, &mut out);
                first += x.len() as u64;
                out.items::<AntFrame>(0).clear();
            })
        },
    }
}

/// The verify case: the three test sensors.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, three sensors", o, 0.0);
    c.seconds = o.seconds.max(10.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = AntFields::read(r.fields());
        let frames = f.as_ref().map_or(0, |f| f.frames_ok);
        let sensors = f.as_ref().map_or(0, |f| f.sensors);
        vec![
            CheckLine::new(
                "frames_ok",
                frames >= 20,
                format!("{frames}, at least 20 expected"),
            ),
            CheckLine::new("sensors", sensors >= 3, format!("{sensors}, 3 expected")),
        ]
    });
    vec![c]
}

/// ANT+ sensors.
pub static ANT: StackDescriptor = StackDescriptor {
    id: "ant",
    name: "ANT+ sensors",
    group: "IoT, 2.4 GHz",
    blurb: "ANT+ heart rate straps, bike speed, cadence and power sensors, footpods and environment sensors on 2457 MHz; sends one ANT+ frame.",
    tuning: Tuning { min_hz: 2400e6, max_hz: 2524e6, tune_offset_hz: 1e6, ..Tuning::anywhere(2457e6, 4e6, 4e6, 2e6) },
    submodes: &[],
    settings: AntSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Ant],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};
