//! e-Czas, the Polish time service on the 225 kHz long-wave carrier (`eczas`). The graph is a phase demodulator with a frequency
//! loop, a slicer that finds the sync word and cuts the 96-bit frames, and the keeper that decodes them, keeps the time and
//! compares the computer clock.
//! Standard: GUM e-CzasPL frame description v1.0

use crate::calendar;
use crate::clock::{ClockMode, LiveClock, CLOCK_MODE, CLOCK_ORIGIN};
use crate::family::{lock_state_of, status, wall, write_series, REPORT_S};
use sdrkit_algo_dsp::fir::{design_by_taps, Fir};
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::publish::keys::Family;
use crate::typed::{EczasFields, EczasFrameRow, EczasTimeEvent, ECZAS_FRAMES, ECZAS_SERIES, ECZAS_TIME};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CaseSetting, CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    Choice, Clock, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, Event,
    GenConfig, Generator, Hz, Receiver, Record, Results, ResultsDecl, SeriesId, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StateBlob, StateError, Status,
    Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Kind, Port, RateFrom, RunContext, StackGraph, Timed,
};
use sdrkit_util_bits::crc::catalog::CRC8_SMBUS;
use sdrkit_util_geo::Position;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::shaped::{log_rows, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Tables};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::sync::Arc;

/// The scrambling bytes of the data ("\nGUM+").
pub const SCRAMBLE: [u8; 5] = [0x0A, 0x47, 0x55, 0x4D, 0x2B];
/// The sync word.
pub const SYNC: u16 = 0x5555;
/// Unix time of the count origin (2000-01-01 00:00:00 UTC).
pub const EPOCH: i64 = 946_684_800;
/// The transmitter site (Solec Kujawski).
pub const SITE: (f64, f64) = (53.017, 18.267);
/// Bits per second.
pub const BAUD: f64 = 50.0;
/// The working rate.
pub const WORK: f64 = 8000.0;
/// The symbol rate of the slicer input.
pub const SYM_RATE: f64 = 500.0;

/// The fields of a time packet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimePacket {
    /// 3 s periods since 2000-01-01.
    pub count: u32,
    /// Hours added to UTC for local time.
    pub tz_hours: u8,
    /// Leap second announced.
    pub leap_announce: bool,
    /// The leap second is subtracted.
    pub leap_subtract: bool,
    /// Local-time change soon.
    pub dst_change_soon: bool,
    /// Transmitter state 0–3.
    pub tx_state: u8,
}

impl TimePacket {
    /// UTC, Unix seconds.
    pub fn utc(&self) -> i64 {
        EPOCH + 3 * self.count as i64
    }
    /// The packet for a UTC time (whole multiple of 3 s after the epoch) and a local offset.
    pub fn at(utc: i64, tz_hours: u8) -> TimePacket {
        TimePacket {
            count: ((utc - EPOCH) / 3) as u32 & 0x3FFF_FFFF,
            tz_hours,
            ..Default::default()
        }
    }
}

sdrkit_core::choice_enum! {
    /// The transmitter state that a time packet announces.
    pub enum TxState {
        /// Normal.
        Normal = ("normal", "Normal"),
        /// Planned off for a day.
        OffDay = ("off_day", "Planned off for a day"),
        /// Planned off for a week.
        OffWeek = ("off_week", "Planned off for a week"),
        /// Planned off for longer than a week.
        OffLonger = ("off_longer", "Planned off for longer than a week"),
    }
}

impl TxState {
    /// The state of the two state bits of a time packet.
    pub fn of(bits: u8) -> TxState {
        match bits {
            0 => TxState::Normal,
            1 => TxState::OffDay,
            2 => TxState::OffWeek,
            _ => TxState::OffLonger,
        }
    }
}

/// Poland's local offset at a UTC instant: +2 h in EU summer time, else +1 h.
pub fn poland_offset(utc: i64) -> u8 {
    if calendar::eu_summer(utc) {
        2
    } else {
        1
    }
}

/// Builds the 12 bytes of a time packet.
pub fn encode_time(p: &TimePacket) -> [u8; 12] {
    let mut bits: u64 = 0; // bits 24..63 of the frame, MSB first in a 40-bit field
    let put = |bits: &mut u64, pos: u32, len: u32, v: u64| {
        // pos counted from frame bit 0; field occupies frame bits pos..pos+len.
        let shift = 63 - (pos + len - 1);
        *bits |= (v & ((1u64 << len) - 1)) << shift;
    };
    put(&mut bits, 24, 3, 0b101);
    put(&mut bits, 27, 30, p.count as u64);
    let (tz0, tz1) = match p.tz_hours {
        1 => (1, 0),
        2 => (0, 1),
        3 => (1, 1),
        _ => (0, 0),
    };
    put(&mut bits, 57, 1, tz0);
    put(&mut bits, 58, 1, tz1);
    put(&mut bits, 59, 1, p.leap_announce as u64);
    put(&mut bits, 60, 1, p.leap_subtract as u64);
    put(&mut bits, 61, 1, p.dst_change_soon as u64);
    put(&mut bits, 62, 1, (p.tx_state & 1) as u64);
    put(&mut bits, 63, 1, ((p.tx_state >> 1) & 1) as u64);
    let mut data = [0u8; 5];
    for (i, d) in data.iter_mut().enumerate() {
        *d = (bits >> (32 - 8 * i)) as u8;
    }
    frame(0x60, data)
}

/// A frame with a header byte and five plain data bytes (scrambled here, CRC over the scrambled bytes).
pub fn frame(header: u8, data: [u8; 5]) -> [u8; 12] {
    let mut f = [0u8; 12];
    f[0] = 0x55;
    f[1] = 0x55;
    f[2] = header;
    for i in 0..5 {
        f[3 + i] = data[i] ^ SCRAMBLE[i];
    }
    f[11] = CRC8_SMBUS.checksum(&f[3..8]);
    f
}

/// Why a frame is not a good time packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decoded {
    /// A good time packet.
    Time(TimePacket),
    /// Another packet of the minute (header).
    Other(u8),
    /// The CRC failed.
    BadCrc,
    /// The time marker is wrong.
    WrongType,
}

/// Decodes 12 frame bytes.
pub fn decode(f: &[u8; 12]) -> Decoded {
    if CRC8_SMBUS.checksum(&f[3..8]) != f[11] {
        return Decoded::BadCrc;
    }
    if f[2] != 0x60 {
        return Decoded::Other(f[2]);
    }
    let mut bits = 0u64;
    for i in 0..5 {
        bits |= ((f[3 + i] ^ SCRAMBLE[i]) as u64) << (32 - 8 * i);
    }
    let get = |pos: u32, len: u32| (bits >> (63 - (pos + len - 1))) & ((1u64 << len) - 1);
    if get(24, 3) != 0b101 {
        return Decoded::WrongType;
    }
    let tz = match (get(57, 1), get(58, 1)) {
        (1, 0) => 1,
        (0, 1) => 2,
        (1, 1) => 3,
        _ => 0,
    };
    Decoded::Time(TimePacket {
        count: get(27, 30) as u32,
        tz_hours: tz,
        leap_announce: get(59, 1) == 1,
        leap_subtract: get(60, 1) == 1,
        dst_change_soon: get(61, 1) == 1,
        tx_state: (get(62, 1) | get(63, 1) << 1) as u8,
    })
}

/// One received 12-byte frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Signal time of the start of the first bit, s.
    pub time: f64,
    /// The bytes.
    pub bytes: [u8; 12],
    /// Sync quality (1 = just found).
    pub quality: f64,
}

impl Item for Frame {
    const KIND: &'static str = "eczas.frame";
}

/// The symbol stream: one value per 2 ms (the phase change in units of 16384 per 36°) with the measurements of the demodulator.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Symbols {
    /// Signal time of the first value's window start.
    pub t0: f64,
    /// The values.
    pub v: Vec<f32>,
    /// Carrier offset applied by the frequency loop, Hz.
    pub offset_hz: f64,
    /// Carrier-to-noise in 100 Hz, dB.
    pub snr_db: f64,
    /// Carrier level, dBFS.
    pub level_dbfs: f64,
    /// Phase-plot points (carrier rotated to the real axis, unit mean radius).
    pub phase: Vec<(f32, f32)>,
}

impl Item for Symbols {
    const KIND: &'static str = "eczas.symbols";
}

/// The demodulator (`eczas.demod`: iq to `eczas.symbols` and audio).
pub struct Demod {
    rate: f64,
    audio_input: bool,
    tone_hz: f64,
    osc: Nco,
    base: f64,
    resampler: Option<Resampler<Complex32>>,
    delay_s: f64,
    start: Option<u64>,
    mixed: Vec<Complex32>,
    y8: Vec<Complex32>,
    lp: Fir<Complex32>,
    last: Complex32,
    // edge detector
    dly: VecDeque<f32>,
    mean_sum: f64,
    mean_win: VecDeque<f32>,
    // 500 Hz
    acc: f32,
    acc_n: u32,
    n8: u64,
    // frequency loop
    fl_sum: f64,
    fl_n: u32,
    loop_hz: f64,
    // measurements
    pw: f64,
    pn: f64,
    pw_acc: (f64, u32),
    pn_acc: (f64, u32),
    snr: f64,
    level: f64,
    phase_pts: VecDeque<(f32, f32)>,
    mean_amp: f64,
    listen: bool,
    tone_ph: f64,
    lp_delay: f64,
    out_t0: Option<f64>,
}

impl Default for Demod {
    fn default() -> Self {
        Demod {
            rate: 0.0,
            audio_input: false,
            tone_hz: 1000.0,
            osc: Nco::new(1.0, 0.0),
            base: 0.0,
            resampler: None,
            delay_s: 0.0,
            start: None,
            mixed: Vec::new(),
            y8: Vec::new(),
            lp: Fir::new(&[1.0]),
            last: Complex32::new(1.0, 0.0),
            dly: VecDeque::new(),
            mean_sum: 0.0,
            mean_win: VecDeque::new(),
            acc: 0.0,
            acc_n: 0,
            n8: 0,
            fl_sum: 0.0,
            fl_n: 0,
            loop_hz: 0.0,
            pw: 0.0,
            pn: 0.0,
            pw_acc: (0.0, 0),
            pn_acc: (0.0, 0),
            snr: 0.0,
            level: -150.0,
            phase_pts: VecDeque::new(),
            mean_amp: 0.0,
            listen: false,
            tone_ph: 0.0,
            lp_delay: 0.0,
            out_t0: None,
        }
    }
}

const EDGE_DELAY: usize = 400; // 50 ms at 8 kHz
const EDGE_MEAN: usize = 800; // 100 ms

impl Demod {
    fn setup(&mut self, f: &Format) {
        self.rate = f.rate;
        self.base = if self.audio_input {
            self.tone_hz
        } else {
            f.signal_offset_hz()
        };
        self.osc = Nco::new(f.rate, -self.base);
        self.resampler = Resampler::new(f.rate, WORK).ok();
        self.delay_s = self.resampler.as_ref().map_or(0.0, |r| r.delay() / f.rate);
        self.start = None;
        let taps =
            design_by_taps(81, 150.0 / WORK, Window::kaiser(6.0)).unwrap_or_else(|_| vec![1.0]);
        self.lp = Fir::new(&taps);
        self.lp_delay = self.lp.group_delay();
        self.dly = VecDeque::from(vec![0.0; EDGE_DELAY]);
        self.mean_win = VecDeque::from(vec![0.0; EDGE_MEAN]);
        self.mean_sum = 0.0;
        self.acc = 0.0;
        self.acc_n = 0;
        self.n8 = 0;
        self.loop_hz = 0.0;
        self.fl_sum = 0.0;
        self.fl_n = 0;
        self.out_t0 = None;
    }
}

impl Component for Demod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "eczas.demod",
            vec![Port::iq("in")],
            vec![
                Port::typed::<Symbols>("symbols").rate_from(RateFrom::Fixed(0.0)),
                Port::audio("audio").rate_from(RateFrom::Fixed(WORK)),
            ],
        )
        .with_settings(DemodSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.setup(&inputs[0]);
        Ok(vec![
            Format::new(Kind::typed::<Symbols>(), 0.0),
            Format::new(Kind::Audio, WORK),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let start = *self.start.get_or_insert(first);
        let x = io.iq(0);
        self.mixed.clear();
        for &s in x {
            // An audio input is real: the carrier is a tone (its image falls outside the channel after mixing).
            let s = if self.audio_input {
                Complex32::new(s.re, 0.0)
            } else {
                s
            };
            self.mixed.push(s * self.osc.next_sample());
        }
        self.y8.clear();
        if let Some(r) = self.resampler.as_mut() {
            r.process(&self.mixed, &mut self.y8);
        }
        let base = start as f64 / self.rate.max(1.0) - self.delay_s;
        let mut sym = Symbols::default();
        let audio = &mut io.audio_out(1).samples;
        for k8 in 0..self.y8.len() {
            let y = self.y8[k8];
            self.n8 += 1;
            // Wide power (the 8 kHz band).
            self.pw_acc.0 += y.norm_sqr() as f64;
            self.pw_acc.1 += 1;
            // The low-pass of 150 Hz either side.
            let z = self.lp.process_sample(y);
            self.pn_acc.0 += z.norm_sqr() as f64;
            self.pn_acc.1 += 1;
            // Phase differentiator.
            let d = z * self.last.conj();
            self.last = z;
            let dphi = if d.norm_sqr() > 0.0 {
                d.im.atan2(d.re)
            } else {
                0.0
            };
            // Frequency loop: the mean phase step is the frequency error.
            self.fl_sum += dphi as f64;
            self.fl_n += 1;
            if self.fl_n == 800 {
                let err_hz = self.fl_sum / self.fl_n as f64 * WORK / TAU;
                if self.pw > 0.0 && self.snr > 6.0 {
                    let step = (0.5 * err_hz).clamp(-20.0, 20.0);
                    self.loop_hz += step;
                    self.osc.set_freq(-(self.base + self.loop_hz));
                }
                self.fl_sum = 0.0;
                self.fl_n = 0;
            }
            // Measurements every 0.25 s: C/N in 100 Hz from the wide and the narrow power.
            if self.pw_acc.1 == 2000 {
                let pw = self.pw_acc.0 / 2000.0;
                let pn = self.pn_acc.0 / self.pn_acc.1.max(1) as f64;
                self.pw = pw;
                self.pn = pn;
                let n0 = ((pw - pn) / (WORK - 300.0)).max(1e-30);
                let c = (pn - n0 * 300.0).max(1e-30);
                let gain = if self.audio_input { 4.0 } else { 1.0 };
                self.snr = power_to_db(c / (n0 * 100.0));
                self.level = power_to_db(c * gain);
                self.mean_amp = (c * gain).sqrt();
                self.pw_acc = (0.0, 0);
                self.pn_acc = (0.0, 0);
            }
            if self.n8.is_multiple_of(33) {
                let r = z.norm().max(1e-12);
                let a = self.mean_amp.max(1e-12) as f32;
                let _ = r;
                self.phase_pts.push_back((z.re / a, z.im / a));
                if self.phase_pts.len() > 240 {
                    self.phase_pts.pop_front();
                }
            }
            // Edge detector: delayed 50 ms minus the 100 ms local mean.
            let v = dphi;
            self.dly.push_back(v);
            let dv = self.dly.pop_front().unwrap_or(0.0);
            self.mean_win.push_back(v);
            self.mean_sum += v as f64;
            self.mean_sum -= self.mean_win.pop_front().unwrap_or(0.0) as f64;
            let e = dv - (self.mean_sum / EDGE_MEAN as f64) as f32;
            // To 500 Hz: the sum of 16 phase steps (the phase change over 2 ms).
            if self.acc_n == 0 && self.out_t0.is_none() {
                // Time of this 500 Hz sample's window start, corrected for the filter, the derivative and the edge delay.
                let t8 = base + (self.n8 - 1) as f64 / WORK;
                let delay = (self.lp_delay + 0.5 + EDGE_DELAY as f64) / WORK;
                self.out_t0 = Some(t8 - delay);
            }
            self.acc += e;
            self.acc_n += 1;
            if self.acc_n == 16 {
                if sym.v.is_empty() {
                    sym.t0 = self.out_t0.unwrap_or(0.0);
                }
                sym.v.push(self.acc * 16384.0 / (36f32.to_radians()));
                self.acc = 0.0;
                self.acc_n = 0;
                self.out_t0 = None;
            }
            if self.listen {
                self.tone_ph += 800.0 / WORK;
                let key = (e.abs() * 50.0).min(1.0);
                let a = (0.3 * key) as f64 * (TAU * self.tone_ph).sin();
                audio.push(a as f32);
            }
        }
        sym.offset_hz = self.loop_hz;
        sym.snr_db = self.snr;
        sym.level_dbfs = self.level;
        if io.tick() {
            sym.phase = self.phase_pts.iter().copied().collect();
        }
        let time = sym.t0;
        io.items_out::<Symbols>(0).push(Timed { time, value: sym });
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DemodSetting::AudioInput => {
                self.audio_input = AUDIO_INPUT.read(id, v)?;
                Ok(Applied::Reformat)
            }
            DemodSetting::AudioToneHz => {
                self.tone_hz = AUDIO_TONE_HZ.read(id, v)?;
                Ok(Applied::Reformat)
            }
            DemodSetting::Listen => {
                self.listen = LISTEN.read(id, v)?;
                Ok(Applied::Live)
            }
        }
    }
    fn values(&self) -> Values {
        values_of([
            (DemodSetting::AudioInput, Value::Bool(self.audio_input)),
            (DemodSetting::AudioToneHz, Value::Hz(self.tone_hz)),
            (DemodSetting::Listen, Value::Bool(self.listen)),
        ])
    }
}

/// The samples are a receiver's audio, with the carrier as a tone.
pub const AUDIO_INPUT: Setting<field::Bool> =
    Setting::new("audio_input", "Audio recording", "", false);
/// The frequency of the carrier tone in the audio.
pub const AUDIO_TONE_HZ: Setting<field::Hz> =
    Setting::new("audio_tone_hz", "Carrier tone in the audio", "Hz", 1000.0)
        .range(100.0, 20000.0, 1.0);
/// Play the keying as a tone.
pub const LISTEN: Setting<field::Bool> = Setting::new("listen", "Listen", "", false);
/// The UTC of the test signal's first sample, 0 for the host clock.
pub const GEN_START_UTC: Setting<field::F64> = Setting::new(
    "gen_start_utc",
    "Test signal: start (UTC, 0 = host clock)",
    "s",
    0.0,
)
.range(0.0, 1e11, 1.0)
.section("Test signal")
.advanced();
/// The test signal is the carrier only.
pub const GEN_CARRIER_ONLY: Setting<field::Bool> =
    Setting::new("gen_carrier_only", "Test signal: carrier only", "", false)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of [`Demod`].
    pub enum DemodSetting {
        /// The samples are a receiver's audio.
        AudioInput = AUDIO_INPUT,
        /// The frequency of the carrier tone in the audio.
        AudioToneHz = AUDIO_TONE_HZ,
        /// Play the keying as a tone.
        Listen = LISTEN,
    }
}

sdrkit_core::settings! {
    /// The settings of the e-Czas stack.
    pub enum EczasSetting {
        /// The samples are a receiver's audio.
        AudioInput = AUDIO_INPUT,
        /// The frequency of the carrier tone in the audio.
        AudioToneHz = AUDIO_TONE_HZ,
        /// Play the keying as a tone.
        Listen = LISTEN,
        /// What the received time is compared with.
        ClockMode = CLOCK_MODE,
        /// The UTC of sample 0 for the fixed clock.
        ClockOrigin = CLOCK_ORIGIN,
        /// The UTC of the test signal's first sample.
        GenStartUtc = GEN_START_UTC,
        /// The test signal is the carrier only.
        GenCarrierOnly = GEN_CARRIER_ONLY,
    }
}

/// The frame slicer (`eczas.slicer`: `eczas.symbols` to `eczas.frame`): finds the 16-bit sync by correlation, cuts 96 bits.
pub struct Slicer {
    buf: VecDeque<f32>,
    t_first: f64,
    pattern: [f32; 16],
    pending: Option<(usize, f64)>,
    skip: usize,
    /// Syncs found.
    pub syncs: u64,
    /// Syncs not followed by the sync word.
    pub false_syncs: u64,
    quality: f64,
    trace: VecDeque<f32>,
    offset: f64,
    snr: f64,
    level: f64,
    phase: Vec<(f32, f32)>,
}

impl Default for Slicer {
    fn default() -> Self {
        // Expected phase change per bit for the sync word 0x5555 sent as phase states (bit 1 = +18°, bit 0 = −18°): every bit
        // changes the state, alternately up and down.
        let mut pattern = [0f32; 16];
        let mut prev = 0i32; // the bit before the frame is unknown: weight 0 for the first
        for (k, p) in pattern.iter_mut().enumerate() {
            let b = ((SYNC >> (15 - k)) & 1) as i32;
            *p = if k == 0 { 0.0 } else { (b - prev) as f32 };
            prev = b;
        }
        Slicer {
            buf: VecDeque::new(),
            t_first: 0.0,
            pattern,
            pending: None,
            skip: 0,
            syncs: 0,
            false_syncs: 0,
            quality: 0.0,
            trace: VecDeque::new(),
            offset: 0.0,
            snr: 0.0,
            level: -150.0,
            phase: Vec::new(),
        }
    }
}

const SPB: usize = 10; // symbols per bit at 500 Hz

/// The weights of the `SPB` phase steps of a bit: a least-squares line through the `SPB + 1` phase values of the bit has the
/// phase change as `sum w_i d_i` with `w_i` proportional to `(i + 1)(SPB - i)` and `sum w_i = SPB`.
const BIT_WEIGHTS: [f32; SPB] = bit_weights();

const fn bit_weights() -> [f32; SPB] {
    let mut w = [0f32; SPB];
    let mut total = 0usize;
    let mut i = 0;
    while i < SPB {
        total += (i + 1) * (SPB - i);
        i += 1;
    }
    let mut i = 0;
    while i < SPB {
        w[i] = ((i + 1) * (SPB - i) * SPB) as f32 / total as f32;
        i += 1;
    }
    w
}

impl Slicer {
    /// The phase change over the bit that starts at symbol `at`, from the slope of a least-squares line through its phase.
    fn bit_sum(&self, at: usize) -> f32 {
        BIT_WEIGHTS
            .iter()
            .enumerate()
            .map(|(k, w)| w * self.buf[at + k])
            .sum()
    }
    fn corr(&self, at: usize) -> f32 {
        let mut c = 0.0;
        for k in 1..16 {
            c += self.pattern[k] * self.bit_sum(at + k * SPB);
        }
        c
    }
}

/// What the slicer measured, for the keeper.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlicerStatus {
    /// Syncs.
    pub syncs: u64,
    /// False syncs.
    pub false_syncs: u64,
    /// Sync quality.
    pub quality: f64,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// C/N in 100 Hz.
    pub snr_db: f64,
    /// Level dBFS.
    pub level_dbfs: f64,
    /// Phase plot.
    pub phase: Vec<(f32, f32)>,
    /// Slicer trace.
    pub trace: Vec<f32>,
}

impl Item for SlicerStatus {
    const KIND: &'static str = "eczas.status";
}

impl Component for Slicer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "eczas.slicer",
            vec![Port::typed::<Symbols>("symbols")],
            vec![
                Port::typed::<Frame>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<SlicerStatus>("status").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.buf.clear();
        self.pending = None;
        self.skip = 0;
        Ok(vec![
            Format::new(Kind::typed::<Frame>(), 0.0),
            Format::new(Kind::typed::<SlicerStatus>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let symbols = io.items::<Symbols>(0);
        let frames = io.items_out::<Frame>(0);
        for s in symbols {
            let s = &s.value;
            self.offset = s.offset_hz;
            self.snr = s.snr_db;
            self.level = s.level_dbfs;
            if !s.phase.is_empty() {
                self.phase.clone_from(&s.phase);
            }
            if self.buf.is_empty() {
                self.t_first = s.t0;
            }
            for &v in &s.v {
                self.buf.push_back(v);
                self.trace.push_back(v);
                if self.trace.len() > 1000 {
                    self.trace.pop_front();
                }
            }
            // A frame is 96 bits = 960 symbols; keep the search running over the buffer.
            let need = 96 * SPB + 2 * SPB;
            let thr = 15.0 * 16384.0 * 0.45;
            while self.buf.len() >= need {
                if self.skip > 0 {
                    let k = self.skip.min(self.buf.len() - need + 1).max(1);
                    for _ in 0..k {
                        self.buf.pop_front();
                    }
                    self.t_first += k as f64 / SYM_RATE;
                    self.skip -= k.min(self.skip);
                    continue;
                }
                let c = self.corr(0);
                if c > thr {
                    // The best alignment within the next bit.
                    let mut best = (c, 0usize);
                    for d in 1..SPB {
                        let cd = self.corr(d);
                        if cd > best.0 {
                            best = (cd, d);
                        }
                    }
                    let at = best.1;
                    self.quality = (best.0 / thr) as f64;
                    self.syncs += 1;
                    // Cut 96 bits: bit = the phase state (an up step → 1, down → 0, no step → as before).
                    let mut bits = [false; 96];
                    let mut prev = false;
                    let step = 16384.0 * 0.5;
                    for (k, b) in bits.iter_mut().enumerate() {
                        let v = self.bit_sum(at + k * SPB);
                        *b = if v > step {
                            true
                        } else if v < -step || k == 0 {
                            false
                        } else {
                            prev
                        };
                        prev = *b;
                    }
                    let mut bytes = [0u8; 12];
                    for (i, byte) in bytes.iter_mut().enumerate() {
                        for j in 0..8 {
                            *byte = (*byte << 1) | bits[8 * i + j] as u8;
                        }
                    }
                    // The first bit's state follows the bit before the frame: take the sync word as known.
                    if bytes[0] & 0x7F == 0x55 & 0x7F && bytes[1] == 0x55 {
                        bytes[0] = 0x55;
                        let t = self.t_first + at as f64 / SYM_RATE;
                        frames.push(Timed {
                            time: t,
                            value: Frame {
                                time: t,
                                bytes,
                                quality: self.quality,
                            },
                        });
                        self.skip = at + 96 * SPB - SPB;
                    } else {
                        self.false_syncs += 1;
                        self.skip = at + SPB;
                    }
                } else {
                    self.buf.pop_front();
                    self.t_first += 1.0 / SYM_RATE;
                }
            }
        }
        if io.tick() {
            let st = SlicerStatus {
                syncs: self.syncs,
                false_syncs: self.false_syncs,
                quality: self.quality,
                offset_hz: self.offset,
                snr_db: self.snr,
                level_dbfs: self.level,
                phase: self.phase.clone(),
                trace: self.trace.iter().copied().collect(),
            };
            io.items_out::<SlicerStatus>(1).push(Timed {
                time: self.t_first,
                value: st,
            });
        }
    }
}

/// A row of the frame log.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogRow {
    /// Signal time of the first bit, s.
    pub signal_time: f64,
    /// The host time, when the samples are live.
    pub host_time: Option<f64>,
    /// What the frame is.
    pub kind: FrameKind,
    /// Accepted.
    pub ok: bool,
    /// The text.
    pub text: String,
    /// The 12 bytes of the frame.
    pub bytes: [u8; 12],
}

sdrkit_core::choice_enum! {
    /// What a frame of the log is.
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum FrameKind {
        /// A time packet.
        Time = ("time", "Time packet"),
        /// A frame that failed its CRC or has the wrong type.
        Bad = ("bad", "Bad frame"),
        /// A packet of another slot.
        Other = ("other", "Other slot"),
        /// A time packet that disagrees with the time received.
        Disagrees = ("disagrees", "Disagrees"),
    }
}

impl Shift for LogRow {
    fn shift_times(&mut self, by_s: f64) {
        self.signal_time += by_s;
    }
}

/// Frames kept.
pub const MAX_FRAMES: usize = 100;

/// What `eczas` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("eczas").results_decl()
}

impl Shaped for LogRow {
    type Row = EczasFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&EczasFrameRow {
            signal_time_s: self.signal_time,
            host_time: self.host_time.map(Time),
            kind: self.kind,
            ok: self.ok,
            text: &self.text,
            bytes: Arc::from(&self.bytes[..]),
        });
    }
}

/// What the keeper keeps: the frame log.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The frames, oldest first.
    pub frames: LogRing<LogRow>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            frames: LogRing::new(MAX_FRAMES),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.frames.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("science.eczas_frames", 2);
    fn clear(&mut self) {
        self.frames.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.frames.adopt(saved.frames);
    }
}

/// The keeper (`eczas.keeper`): decodes frames, keeps the time, compares clocks, publishes.
pub struct Keeper {
    rate: f64,
    t: f64,
    clock: ClockMode,
    clock_choice: crate::clock::ClockChoice,
    clock_origin: f64,
    live: LiveClock,
    live_input: bool,
    host_clock: Clock,
    position: Option<Position>,
    last: Option<(f64, TimePacket)>,
    prev_ok: Option<(f64, i64)>,
    confirmed: bool,
    store: Store,
    good: u64,
    bad: u64,
    other: u64,
    frames: u64,
    status: SlicerStatus,
    offsets: VecDeque<(f64, f64)>,
    level_hist: VecDeque<(f32, f32)>,
    snr_hist: VecDeque<(f32, f32)>,
    sec_acc: f64,
    published: Published,
    series: [SeriesId; 5],
    shown: Status,
    events: Vec<Event>,
}

impl Default for Keeper {
    fn default() -> Self {
        let decl = Arc::new(declaration());
        let series = ECZAS_SERIES.map(|d| decl.series_id(d.key).expect("the declaration holds the series"));
        Keeper {
            rate: 0.0,
            t: 0.0,
            clock: ClockMode::of(CLOCK_MODE.default, CLOCK_ORIGIN.default),
            clock_choice: CLOCK_MODE.default,
            clock_origin: CLOCK_ORIGIN.default,
            live: LiveClock::default(),
            live_input: false,
            host_clock: Clock::fixed(0.0),
            position: None,
            last: None,
            prev_ok: None,
            confirmed: false,
            store: Store::default(),
            good: 0,
            bad: 0,
            other: 0,
            frames: 0,
            status: SlicerStatus::default(),
            offsets: VecDeque::new(),
            level_hist: VecDeque::new(),
            snr_hist: VecDeque::new(),
            sec_acc: 0.0,
            published: Published::from_decl(decl),
            series,
            shown: Status::default(),
            events: Vec::new(),
        }
    }
}

impl Keeper {
    /// What the component keeps.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The distance (m) and the path delay (s) from the transmitter, when the station position is known.
    fn delay(&self) -> Option<(f64, f64)> {
        let d = Position::new(SITE.0, SITE.1).distance_m(&self.position?);
        Some((d, d / sdrkit_util_geo::SPEED_OF_LIGHT_M_S))
    }

    fn wall_of(&self, signal_t: f64) -> Option<f64> {
        match self.clock {
            ClockMode::None => None,
            ClockMode::Fixed(o) => Some(o + signal_t),
            ClockMode::Live => self.live.origin().map(|o| o + signal_t),
        }
    }

    fn frame(&mut self, f: &Frame) {
        self.frames += 1;
        let host = wall(self.live_input, &self.host_clock);
        let (kind, ok, text) = match decode(&f.bytes) {
            Decoded::Time(p) => {
                let utc = p.utc();
                // Disagreement with the time already received (advanced by the elapsed signal time).
                let disagree = self.last.as_ref().is_some_and(|(t0, q)| {
                    let expect = q.utc() as f64 + (f.time - t0);
                    (utc as f64 - expect).abs() > 1.5
                });
                if disagree && self.prev_ok.is_some() {
                    self.bad += 1;
                    (
                        FrameKind::Disagrees,
                        false,
                        format!("{} disagrees with the time received", calendar::iso(utc)),
                    )
                } else {
                    self.good += 1;
                    self.confirmed = self.last.as_ref().is_some_and(|(t0, q)| {
                        (q.utc() as f64 + (f.time - t0) - utc as f64).abs() < 0.5
                    });
                    self.last = Some((f.time, p));
                    self.prev_ok = Some((f.time, utc));
                    if let Some(w) = self.wall_of(f.time) {
                        let d = self.delay().map_or(0.0, |x| x.1);
                        let off = w - (utc as f64 + d);
                        self.offsets.push_back((f.time, off));
                        if self.offsets.len() > 90 {
                            self.offsets.pop_front();
                        }
                    }
                    let text = format!("{}, +{} h", calendar::iso(utc), p.tz_hours);
                    self.events.push(ECZAS_TIME.event(
                        f.time,
                        0.0,
                        &EczasTimeEvent {
                            text: &text,
                            family: Family::Other,
                            utc: Time(utc as f64),
                            tz_hours: i64::from(p.tz_hours),
                            tx_state: TxState::of(p.tx_state),
                        },
                    ));
                    (FrameKind::Time, true, text)
                }
            }
            Decoded::Other(h) => {
                self.other += 1;
                (
                    FrameKind::Other,
                    true,
                    format!("slot packet 0x{h:02X} (not the time packet)"),
                )
            }
            Decoded::BadCrc => {
                self.bad += 1;
                (FrameKind::Bad, false, "bad CRC".to_string())
            }
            Decoded::WrongType => {
                self.bad += 1;
                (FrameKind::Bad, false, "wrong type".to_string())
            }
        };
        self.store.frames.push(LogRow {
            signal_time: f.time,
            host_time: host,
            kind,
            ok,
            text,
            bytes: f.bytes,
        });
    }

    fn state(&self) -> u8 {
        if self
            .last
            .as_ref()
            .is_some_and(|(t0, _)| self.t - t0 < 180.0)
        {
            2
        } else if self.status.snr_db > 15.0 {
            1
        } else {
            0
        }
    }

    fn publish(&mut self) {
        let state = self.state();
        let path = self.delay();
        let utc_now = self.last.map(|(t0, p)| p.utc() as f64 + (self.t - t0));
        let st = &self.status;
        status(
            &mut self.shown,
            lock_state_of(state),
            Some(st.snr_db as f32),
            Some(st.level_dbfs as f32),
            Some(st.offset_hz),
            |s| {
                let _ = match (state, self.last, utc_now) {
                    (2, Some((_, q)), Some(u)) => write!(
                        s,
                        "e-Czas {} (+{} h)",
                        calendar::iso(u.floor() as i64),
                        q.tz_hours
                    ),
                    (1, _, _) => write!(
                        s,
                        "e-Czas: carrier found, {:.0} dB, waiting for a time packet",
                        st.snr_db
                    ),
                    _ => write!(s, "e-Czas: no carrier"),
                };
                if path.is_none() && state == 2 {
                    s.push_str(" (no station position: no path delay)");
                }
            },
        );
        let p = &mut self.published;
        p.results_mut().set_status(&self.shown);
        let packet = utc_now.zip(self.last.map(|(_, q)| q));
        let (clock_offset_s, clock_offset_avg_s) = match self.offsets.back() {
            Some(&(_, o)) => {
                let n = self.offsets.len().min(10);
                let avg = self.offsets.iter().rev().take(n).map(|x| x.1).sum::<f64>() / n as f64;
                (Some(o), Some(avg))
            }
            None => (None, None),
        };
        p.put(&EczasFields {
            utc: packet.map(|(u, _)| Time(u)),
            tz_hours: packet.map(|(_, q)| i64::from(q.tz_hours)),
            leap_announce: packet.map(|(_, q)| q.leap_announce),
            leap_subtract: packet.map(|(_, q)| q.leap_subtract),
            dst_change_soon: packet.map(|(_, q)| q.dst_change_soon),
            tx_state: packet.map(|(_, q)| TxState::of(q.tx_state)),
            last_packet: packet.map(|(_, q)| Time(q.utc() as f64)),
            confirmed: self.confirmed,
            offset_hz: Hertz(self.status.offset_hz),
            snr_db: self.status.snr_db,
            level_dbfs: self.status.level_dbfs,
            sync_quality: self.status.quality,
            syncs: self.status.syncs as i64,
            frames: self.frames as i64,
            frames_ok: self.good as i64,
            frames_bad: self.bad as i64,
            other_slots: self.other as i64,
            false_syncs: self.status.false_syncs as i64,
            distance_m: path.map(|x| x.0),
            delay_ms: path.map(|x| x.1 * 1000.0),
            clock_offset_s,
            clock_offset_avg_s,
        });
        log_rows(&self.store.frames, &ECZAS_FRAMES, self.t, p);
        let r = p.results_mut();
        let [level, snr, offsets, phase, trace] = self.series;
        write_series(r, level, true, self.level_hist.iter().map(|&(a, b)| [a, b]));
        write_series(r, snr, true, self.snr_hist.iter().map(|&(a, b)| [a, b]));
        write_series(
            r,
            offsets,
            false,
            self.offsets.iter().map(|&(a, b)| [a as f32, b as f32]),
        );
        write_series(
            r,
            phase,
            false,
            self.status.phase.iter().map(|&(a, b)| [a, b]),
        );
        write_series(
            r,
            trace,
            true,
            self.status
                .trace
                .iter()
                .enumerate()
                .map(|(i, &v)| [i as f32 / SYM_RATE as f32, v]),
        );
    }
}

sdrkit_core::settings! {
    /// The settings of [`Keeper`].
    pub enum KeeperSetting {
        /// What the received time is compared with.
        ClockMode = CLOCK_MODE,
        /// The UTC of sample 0 for the fixed clock.
        ClockOrigin = CLOCK_ORIGIN,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Keeper`].
    pub enum KeeperCommand {
        /// Forgets the frame log and the clock offsets.
        Clear = objcmd::CLEAR,
    }
}

impl Component for Keeper {
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "eczas.keeper",
            vec![
                Port::iq("in"),
                Port::typed::<Frame>("frames"),
                Port::typed::<SlicerStatus>("status"),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(KeeperSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.position = ctx.station.position;
        self.live_input = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        if !self.live_input && self.clock == ClockMode::Live {
            self.clock = ClockMode::None;
        }
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let n = io.input_len(0);
        self.t = (first + n as u64) as f64 / self.rate.max(1.0);
        if let (ClockMode::Live, Some(w)) = (self.clock, wall(self.live_input, &self.host_clock)) {
            self.live.arrive(w, self.t);
        }
        for st in io.items::<SlicerStatus>(2) {
            self.status = st.value.clone();
        }
        for f in io.items::<Frame>(1) {
            self.frame(&f.value);
        }
        // Histories, one point a second.
        self.sec_acc += n as f64 / self.rate.max(1.0);
        if self.sec_acc >= 1.0 {
            self.sec_acc -= 1.0;
            self.level_hist
                .push_back((self.t as f32, self.status.level_dbfs as f32));
            self.snr_hist
                .push_back((self.t as f32, self.status.snr_db as f32));
            while self.level_hist.len() > 120 {
                self.level_hist.pop_front();
            }
            while self.snr_hist.len() > 120 {
                self.snr_hist.pop_front();
            }
        }
        if io.tick() {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match KeeperSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            KeeperSetting::ClockMode => self.clock_choice = CLOCK_MODE.read(id, v)?,
            KeeperSetting::ClockOrigin => self.clock_origin = CLOCK_ORIGIN.read(id, v)?,
        }
        self.clock = ClockMode::of(self.clock_choice, self.clock_origin);
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                KeeperSetting::ClockMode,
                Value::Choice(self.clock_choice.index()),
            ),
            (KeeperSetting::ClockOrigin, Value::F64(self.clock_origin)),
        ])
    }
    fn commands(&self) -> &Commands {
        KeeperCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match KeeperCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            KeeperCommand::Clear => {
                self.store.clear();
                self.offsets.clear();
            }
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.t).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.t)?;
        self.publish();
        Ok(())
    }
}

/// The lowest input rate.
pub const MIN_RATE: f64 = 8e3;

/// The receiver: the demodulator, the slicer and the keeper.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build).
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("demod", Demod::default())
        .add("slicer", Slicer::default())
        .add("keeper", Keeper::default())
        .input("demod")
        .input("keeper.in")
        .connect("demod.symbols", "slicer")
        .connect("slicer.frames", "keeper.frames")
        .connect("slicer.status", "keeper.status")
        .output("demod.audio")
        .output("keeper.events")
        .clock(REPORT_S)
        .status_from("keeper")
        .declare(&declaration())
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape("eczas"), graph)
        .min_rate(MIN_RATE, "e-Czas needs a sample rate of at least 8 ksps")
        .route(AUDIO_INPUT.key(), &["demod.audio_input"])?
        .route(AUDIO_TONE_HZ.key(), &["demod.audio_tone_hz"])?
        .route(LISTEN.key(), &["demod.listen"])?
        .route(CLOCK_MODE.key(), &["keeper.clock_mode"])?
        .route(CLOCK_ORIGIN.key(), &["keeper.clock_origin"])?
        .command(
            objcmd::CLEAR.key,
            sdrkit_graph::CommandTarget::of("keeper", "clear"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// The receiver as the engine takes it.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

/// Generator options.
#[derive(Clone, Debug)]
pub struct EczasGenOptions {
    /// UTC of sample 0.
    pub start_utc: f64,
    /// The local offset (None: Poland's).
    pub tz: Option<u8>,
    /// Time packets every `stride` slots.
    pub stride: usize,
    /// Carrier only.
    pub carrier_only: bool,
    /// A leap second is announced.
    pub leap_announce: bool,
    /// The leap second is taken away.
    pub leap_subtract: bool,
    /// A change of summer time is near.
    pub dst_change_soon: bool,
    /// The transmitter state.
    pub tx_state: u8,
    /// Real audio with the carrier as a tone at this frequency, as a recording of a receiver's audio.
    pub audio_tone: Option<f64>,
}

impl Default for EczasGenOptions {
    fn default() -> Self {
        EczasGenOptions {
            start_utc: crate::family::TEST_START_UTC,
            tz: None,
            stride: 4,
            carrier_only: false,
            leap_announce: false,
            leap_subtract: false,
            dst_change_soon: false,
            tx_state: 0,
            audio_tone: None,
        }
    }
}

/// The e-Czas generator: the carrier at 0 Hz (or a real tone) with amplitude 1, phase-keyed by the frames.
pub struct EczasGenerator {
    rate: f64,
    o: EczasGenOptions,
    n: u64,
    phase: f64,
    cache: Option<(i64, [u8; 12])>,
}

impl EczasGenerator {
    /// At `rate`.
    pub fn new(rate: f64, o: EczasGenOptions) -> EczasGenerator {
        EczasGenerator {
            rate,
            o,
            n: 0,
            phase: 0.0,
            cache: None,
        }
    }

    /// From the stack's settings.
    pub fn from_config(cfg: &GenConfig) -> EczasGenerator {
        let s = &cfg.settings;
        let start = s.get(&GEN_START_UTC);
        let o = EczasGenOptions {
            start_utc: if start > 0.0 {
                start
            } else {
                crate::family::TEST_START_UTC
            },
            carrier_only: s.get(&GEN_CARRIER_ONLY),
            ..Default::default()
        };
        EczasGenerator::new(cfg.rate_hz, o)
    }

    fn frame_bytes(&mut self, slot_start: i64) -> [u8; 12] {
        if let Some((t, b)) = self.cache {
            if t == slot_start {
                return b;
            }
        }
        let slot = (slot_start.rem_euclid(60) / 3) as usize;
        let b = if slot.is_multiple_of(self.o.stride.max(1)) {
            let tz = self.o.tz.unwrap_or_else(|| poland_offset(slot_start));
            encode_time(&TimePacket {
                leap_announce: self.o.leap_announce,
                leap_subtract: self.o.leap_subtract,
                dst_change_soon: self.o.dst_change_soon,
                tx_state: self.o.tx_state,
                ..TimePacket::at(slot_start, tz)
            })
        } else {
            frame(0x40 + slot as u8, [slot as u8, 0x12, 0x34, 0x56, 0x78])
        };
        self.cache = Some((slot_start, b));
        b
    }

    /// The keying phase (radians) at a UTC instant.
    fn keying(&mut self, utc: f64) -> f64 {
        if self.o.carrier_only {
            return 0.0;
        }
        let slot_start = (utc / 3.0).floor() as i64 * 3;
        let into = utc - slot_start as f64;
        let state = |bytes: &[u8; 12], k: isize| -> f64 {
            if !(0..96).contains(&k) {
                return -1.0;
            }
            let b = (bytes[k as usize / 8] >> (7 - k as usize % 8)) & 1;
            if b == 1 {
                1.0
            } else {
                -1.0
            }
        };
        let bytes = self.frame_bytes(slot_start);
        let k = (into * BAUD).floor() as isize;
        let frac = into * BAUD - k as f64;
        let cur = state(&bytes, k);
        let prev = state(&bytes, k - 1);
        let ramp = 0.019 * BAUD; // 19 ms of the 20 ms bit
        let s = if frac < ramp {
            prev + (cur - prev) * frac / ramp
        } else {
            cur
        };
        s * 18f64.to_radians()
    }
}

impl Generator for EczasGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            let utc = self.o.start_utc + self.n as f64 / self.rate;
            let k = self.keying(utc);
            let f = self.o.audio_tone.unwrap_or(0.0);
            self.phase = (self.phase + f / self.rate).rem_euclid(1.0);
            let p = TAU * self.phase + k;
            *o = if self.o.audio_tone.is_some() {
                Complex32::new(p.cos() as f32, 0.0)
            } else {
                Complex32::new(p.cos() as f32, p.sin() as f32)
            };
            self.n += 1;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of e-Czas is its carrier to noise in 100 Hz.
        Some(SignalInfo {
            symbol_rate_hz: Some(BAUD),
            bit_rate_hz: Some(BAUD),
            ..SignalInfo::new(100.0)
        })
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

/// The generator.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(EczasGenerator::from_config(cfg)))
}

/// The verify case: 40 s of e-Czas from a fixed start.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = o.seconds.max(40.0);
    c.expect_audio = Some(false);
    // The SNR of e-Czas is stated in 100 Hz; 30 dB is a strong long-wave carrier.
    if !o.clean {
        c.snr_db = Some(30.0);
    }
    // A second before a slot, so the first frame is whole.
    c.settings
        .push(CaseSetting::new(&GEN_START_UTC, 1_791_813_599.0));
    c.check = Some(|r| {
        let n = crate::family::field(r, EczasFields::FRAMES_OK.key);
        vec![CheckLine::new(
            EczasFields::FRAMES_OK.key,
            n >= 2.0,
            format!("{n}, at least 2 expected"),
        )]
    });
    vec![c]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets() {
        let p = TimePacket::at(1_780_272_012, 2);
        let b = encode_time(&p);
        assert_eq!(
            b,
            [0x55, 0x55, 0x60, 0xA2, 0x00, 0xB9, 0x4F, 0x0B, 0x00, 0x00, 0x00, 0x4E]
        );
        assert_eq!(decode(&b), Decoded::Time(p));
        let mut bad = b;
        bad[11] ^= 1;
        assert_eq!(decode(&bad), Decoded::BadCrc);
        // Every single-bit error of the data and CRC bits is caught.
        for bit in 0..48 {
            let mut e = b;
            let byte = 3 + bit / 8;
            let byte = if byte >= 8 { 11 } else { byte };
            e[byte] ^= 1 << (bit % 8);
            assert_ne!(decode(&e), Decoded::Time(p), "{bit}");
        }
        let m = TimePacket {
            count: 0x3FFF_FFFF,
            tz_hours: 1,
            leap_announce: true,
            ..Default::default()
        };
        assert_eq!(decode(&encode_time(&m)), Decoded::Time(m));
        let leap = TimePacket::at(calendar::unix(2028, 2, 29, 23, 59, 57).unwrap(), 1);
        assert_eq!(calendar::iso(leap.utc()), "2028-02-29T23:59:57Z");
        assert_eq!(
            poland_offset(calendar::unix(2026, 3, 29, 0, 59, 59).unwrap()),
            1
        );
        assert_eq!(
            poland_offset(calendar::unix(2026, 3, 29, 1, 0, 0).unwrap()),
            2
        );
        let mut wrong = encode_time(&p);
        wrong[3] ^= 0x80; // the time marker
        wrong[11] = CRC8_SMBUS.checksum(&wrong[3..8]);
        assert_eq!(decode(&wrong), Decoded::WrongType);
    }
}
