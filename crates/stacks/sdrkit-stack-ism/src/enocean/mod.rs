//! EnOcean (`enocean`): the telegrams of EnOcean devices (energy-harvesting switches, contacts and sensors). Each burst is
//! read twice: as ERP1 (868.3 MHz, amplitude keying, 125 kbit/s, 8/12 coding) from its envelope, and as ERP2 (frequency keying,
//! 125 kbit/s, ±62.5 kHz, at 902.875 or 928.35 MHz) from its discriminator. A telegram with a good check gives its sender, its
//! type and, for the common profiles, a reading. The subtelegrams of one telegram (up to three, inside the 100 ms RX maturity
//! time) count as one telegram. Secure telegrams are shown as encrypted. Receive only.
//!
//! Standards: ISO/IEC 14543-3-10 (ERP1) and 14543-3-11 (ERP2) as the public EnOcean documents describe them; EnOcean Equipment
//! Profiles (module [`frame`]).

pub mod frame;

use crate::family::{hex, min_rate_message, set_status, wall, CleanScene, REPORT_S};
use crate::typed::{
    EnoceanDeviceRow, EnoceanFields, EnoceanTelegramEvent, EnoceanTelegramRow, ENOCEAN_DEVICES,
    ENOCEAN_TELEGRAM, ENOCEAN_TELEGRAMS,
};
use frame::{Eep, Hash, Radio, Telegram};
use sdrkit_algo_dsp::fir::{design_by_taps, Fir};
use sdrkit_algo_dsp::floor::{FloorTracker, MinFollower};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_dsp::stats::centred_mean;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::burst::{BurstConfig, BurstDetector};
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::interface::{Fields, ReadFields, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, EventDef, GenConfig, Generator, LockProfile, LockState,
    Receiver, Record, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kept,
    Kind, Outbox, Port, RunContext, StackGraph, Timed,
};
use sdrkit_proto_ookdev::{level_train, Coding, Slicer, Timing};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "enocean";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 500e3;
/// The rate the stack asks for, Hz.
pub const RATE_HZ: f64 = 1e6;
/// The ERP1 frequency, Hz: the default frequency. Other frequencies come from the session (the plan `enocean`).
pub const ERP1_HZ: f64 = 868.3e6;
/// The bit rate of ERP1 and ERP2, bit/s.
pub const BIT_RATE: f64 = 125e3;
/// The ERP2 deviation, Hz.
pub const ERP2_DEVIATION_HZ: f64 = 62.5e3;
/// The half width of the channel filter, Hz: the ERP1 carrier tolerance (±86 kHz) and the signal.
pub const CHANNEL_HZ: f64 = 220e3;
/// Subtelegrams of one telegram end within this time of the first, s (the RX maturity time of ERP1 and ERP2).
pub const MATURITY_S: f64 = 0.1;
/// Telegrams kept in the log.
pub const TELEGRAMS_CAP: usize = 500;
/// Devices kept.
pub const DEVICES_CAP: usize = 500;
/// A telegram within this many seconds of signal shows the lock.
pub const RECENT_S: f64 = 60.0;
/// What no real signal has confirmed and what is not read.
pub const NOT_CONFIRMED: &str = "Decoded from the public EnOcean documents and checked only on the stack's own test signal, not on a real device. Not read: the 4-bit check of the 6-byte ERP1 switch form (its layout is also unconfirmed), start of frame variant 1 when its 1.5-bit runs read short, ERP2 telegrams of 6 bytes or fewer (no check), secure telegram contents (encrypted). 4BS data is decoded only after a teach-in of the sender with its profile.";

/// The detection threshold.
pub const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold", "Detection threshold", "dB", 10.0)
        .range(6.0, 25.0, 1.0)
        .help("How far above the noise floor a burst must rise.");

sdrkit_core::settings! {
    /// The settings of `ism.enocean`.
    pub enum EnoceanSetting {
        /// The detection threshold.
        Threshold = THRESHOLD,
    }
}

sdrkit_core::choice_enum! {
    /// The radio protocol of the test signal.
    pub enum GenRadio {
        /// ERP1, amplitude keying.
        Erp1 = ("erp1", "ERP1 (amplitude keying)"),
        /// ERP2, frequency keying.
        Erp2 = ("erp2", "ERP2 (frequency keying)"),
    }
}

/// The radio protocol of the test signal.
pub const GEN_RADIO: Setting<field::Choice<GenRadio>> =
    Setting::choice("gen_radio", "Test signal protocol", GenRadio::Erp1)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `enocean` stack: the detection threshold and the test signal.
    pub enum EnoceanStackSetting {
        /// The detection threshold.
        Threshold = THRESHOLD,
        /// The radio protocol of the test signal.
        GenRadio = GEN_RADIO,
    }
}

/// One telegram of the log: its first subtelegram and the count of its copies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    /// Signal time of the first subtelegram, s.
    pub t: f64,
    /// Wall time, s since 1970, when the samples are live.
    pub wall: Option<f64>,
    /// The radio protocol.
    pub radio: Radio,
    /// The telegram type.
    pub rorg: u8,
    /// The sender id, hex.
    pub sender: String,
    /// The destination id of an addressed telegram, hex.
    pub dest: Option<String>,
    /// The reading.
    pub reading: String,
    /// The payload, hex; for a secure telegram empty.
    pub data: String,
    /// The repeater level text of the first subtelegram.
    pub hops: String,
    /// Subtelegrams heard.
    pub subtelegrams: u32,
    /// The check.
    pub hash: Hash,
    /// Level, dBFS.
    pub level_db: f32,
}

impl Shift for LogEntry {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

impl Shaped for LogEntry {
    type Row = EnoceanTelegramRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&EnoceanTelegramRow {
            utc: self.wall.map(Time),
            time_s: self.t,
            sender: &self.sender,
            radio: self.radio.text(),
            telegram_type: frame::rorg_name(self.rorg),
            reading: &self.reading,
            data: &self.data,
            destination: self.dest.as_deref(),
            repeater: &self.hops,
            subtelegrams: i64::from(self.subtelegrams),
            check: self.hash.text(),
            level_dbfs: f64::from(self.level_db),
        });
    }
}

impl Item for LogEntry {
    const KIND: &'static str = "enocean.telegram";
    fn event_def() -> Option<&'static EventDef> {
        Some(ENOCEAN_TELEGRAM.def())
    }
    fn write_fields(&self, f: &mut Record) {
        EnoceanTelegramEvent {
            sender: &self.sender,
            telegram_type: frame::rorg_name(self.rorg),
            reading: &self.reading,
            family: Family::Other,
        }
        .write(f);
    }
}

/// One sender.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// The sender id, hex.
    pub id: String,
    /// The radio protocol last heard.
    pub radio: Radio,
    /// The last telegram type.
    pub rorg: u8,
    /// The profile from a teach-in.
    pub eep: Option<Eep>,
    /// The profile of the last reading, when certain.
    pub profile: Option<String>,
    /// The last reading.
    pub reading: String,
    /// Telegrams.
    pub telegrams: u64,
    /// Secure telegrams.
    pub secure: u64,
    /// The last level, dBFS.
    pub level_db: f32,
}

impl Shift for Device {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Device {
    type Row = EnoceanDeviceRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let profile = self.eep.map(Eep::text).or_else(|| self.profile.clone());
        let name = match &profile {
            Some(p) => format!("EnOcean {p} {}", self.id),
            None => format!("EnOcean {}", self.id),
        };
        out.put(&EnoceanDeviceRow {
            name: &name,
            radio: self.radio.text(),
            telegram_type: frame::rorg_name(self.rorg),
            profile: profile.as_deref(),
            reading: &self.reading,
            telegrams: i64::try_from(self.telegrams).unwrap_or(i64::MAX),
            secure: i64::try_from(self.secure).unwrap_or(i64::MAX),
            level_dbfs: f64::from(self.level_db),
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// The counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Counters {
    /// Telegrams (subtelegrams of one telegram count once).
    pub telegrams: u64,
    /// Subtelegrams with a good check.
    pub subtelegrams: u64,
    /// Frames that the coding accepted and the check refused.
    pub bad: u64,
    /// Bursts seen.
    pub bursts: u64,
}

/// What the stack keeps and saves.
#[derive(Serialize, Deserialize)]
pub struct Held {
    /// The telegram log.
    pub telegrams: LogRing<LogEntry>,
    /// The senders, by id.
    pub devices: ObjectTable<String, Device>,
    /// The counters.
    pub counters: Counters,
}

impl Default for Held {
    fn default() -> Self {
        Held {
            telegrams: LogRing::new(TELEGRAMS_CAP),
            devices: ObjectTable::new(DEVICES_CAP),
            counters: Counters::default(),
        }
    }
}

impl Shift for Held {
    fn shift_times(&mut self, by_s: f64) {
        self.telegrams.shift_times(by_s);
        self.devices.shift_times(by_s);
    }
}

impl Tables for Held {
    const FORMAT: StateFormat = StateFormat::new("ism.enocean", 1);
    fn clear(&mut self) {
        self.telegrams.clear();
        self.devices.clear();
        self.counters = Counters::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.telegrams.adopt(saved.telegrams);
        self.devices.adopt(saved.devices);
        self.counters = saved.counters;
    }
}

/// The NRZ timing of both protocols: 8 µs bits, a train ends after 200 µs without a mark.
const TIMING: Timing = Timing::new(Coding::Nrz, 8.0, 8.0, 200.0);
/// The discriminator output is held to ±this many deviations, then moved up by it, so the two levels of the frequency keying
/// are two positive levels for the two-level slicer.
const DISC_CLAMP: f32 = 2.5;
/// The longest burst, s: three ERP2 subtelegrams of 255 bytes and their gaps would be longer, but each subtelegram is a burst.
const MAX_BURST_S: f64 = 0.03;

/// The subtelegrams of one burst `x` at `rate`, read as ERP1 and as ERP2.
pub struct BurstReader {
    rate: f64,
    slicer: Slicer,
    disc: Discriminator,
    env: Vec<f32>,
    smooth: Vec<f32>,
    bits: Vec<u8>,
    /// Frames that the coding accepted and the check refused, since made.
    pub bad: u64,
}

impl BurstReader {
    /// A reader at `rate`. Errors for a rate that is not positive.
    pub fn new(rate: f64) -> Result<BurstReader, ConfigError> {
        let disc = Discriminator::new(rate, ERP2_DEVIATION_HZ).map_err(|_| ConfigError::RateTooLow {
            need: MIN_RATE_HZ,
            got: rate,
        })?;
        Ok(BurstReader {
            rate,
            slicer: Slicer::new(),
            disc,
            env: Vec::new(),
            smooth: Vec::new(),
            bits: Vec::new(),
            bad: 0,
        })
    }

    /// Reads `x` and appends each subtelegram with a good check to `out`.
    pub fn read(&mut self, x: &[Complex32], out: &mut Vec<Telegram>) {
        if x.len() < 32 {
            return;
        }
        // ERP1: the envelope, a 1 is low power.
        self.env.clear();
        self.env.extend(x.iter().map(|z| z.norm()));
        centred_mean(&self.env, 1, &mut self.smooth);
        let n0 = out.len();
        if let Some(train) = level_train(&self.smooth, self.rate) {
            let (bits, bad) = (&mut self.bits, &mut self.bad);
            let _ = self.slicer.slice(&TIMING, &train, |rows| {
                for row in 0..rows.num_rows() {
                    bits.clear();
                    bits.extend((0..rows.len(row)).map(|i| rows.bit(row, i) ^ 1));
                    let Some(start) = frame::erp1_data_start(bits) else {
                        continue;
                    };
                    let Some(bytes) = frame::erp1_bytes(bits, start) else {
                        continue;
                    };
                    match frame::erp1_telegram(&bytes) {
                        Some(t) => out.push(t),
                        None => *bad += 1,
                    }
                }
            });
        }
        if out.len() > n0 {
            return;
        }
        // ERP2: the discriminator over the carrier, a 1 is the upper tone.
        let peak = x.iter().map(|z| z.norm_sqr()).fold(0f32, f32::max);
        let on = |z: &Complex32| z.norm_sqr() > 0.25 * peak;
        let (Some(a), Some(e)) = (x.iter().position(on), x.iter().rposition(on)) else {
            return;
        };
        let x = &x[a..=e];
        if x.len() < 32 {
            return;
        }
        self.disc.reset();
        self.env.clear();
        self.env.resize(x.len(), 0.0);
        self.disc.process_into(x, &mut self.env);
        for v in &mut self.env {
            *v = v.clamp(-DISC_CLAMP, DISC_CLAMP) + DISC_CLAMP;
        }
        centred_mean(&self.env, 1, &mut self.smooth);
        if let Some(train) = level_train(&self.smooth, self.rate) {
            let (bits, bad) = (&mut self.bits, &mut self.bad);
            let _ = self.slicer.slice(&TIMING, &train, |rows| {
                for row in 0..rows.num_rows() {
                    bits.clear();
                    bits.extend((0..rows.len(row)).map(|i| rows.bit(row, i)));
                    // The slicer ends a train at its last mark: lower-tone bits at the end of the frame are not in the row.
                    // Up to 8 of them are put back as zeros; the CRC decides.
                    bits.extend([0; 8]);
                    let Some(d) = frame::erp2_data(bits) else {
                        continue;
                    };
                    match frame::erp2_telegram(&d) {
                        Some(t) => out.push(t),
                        None if d.len() > frame::ERP2_SHORT_MAX => *bad += 1,
                        None => {}
                    }
                }
            });
        }
    }
}

/// The `ism.enocean` decoder: a channel filter, bursts over the noise floor, the [`BurstReader`] on each, and the tables.
pub struct EnoceanDecoder {
    threshold_db: f64,
    rate: f64,
    filter: Option<Fir<Complex32>>,
    det: Option<BurstDetector>,
    reader: Option<BurstReader>,
    window: usize,
    /// Filter outputs still to drop before the detector: the filter's history fills first.
    settle: usize,
    filtered: Vec<Complex32>,
    burst: Vec<Complex32>,
    found: Vec<Telegram>,
    /// Switch-form subtelegrams (no tested check) waiting for a second copy, with their times.
    unchecked: Vec<(f64, Telegram)>,
    base: u64,
    held: Held,
    out: Vec<Timed<LogEntry>>,
    level_acc: f64,
    level_n: u64,
    level_dbfs: Option<f64>,
    n: u64,
    now: f64,
    live: bool,
    dirty: bool,
    cleared: bool,
    started: bool,
}

impl Default for EnoceanDecoder {
    fn default() -> Self {
        EnoceanDecoder {
            threshold_db: THRESHOLD.default,
            rate: 0.0,
            filter: None,
            det: None,
            reader: None,
            window: 1,
            settle: 0,
            filtered: Vec::new(),
            burst: Vec::new(),
            found: Vec::new(),
            unchecked: Vec::new(),
            base: 0,
            held: Held::default(),
            out: Vec::new(),
            level_acc: 0.0,
            level_n: 0,
            level_dbfs: None,
            n: 0,
            now: 0.0,
            live: false,
            dirty: true,
            cleared: false,
            started: false,
        }
    }
}

impl EnoceanDecoder {
    /// What the stack keeps.
    pub fn held(&self) -> &Held {
        &self.held
    }

    /// Builds the filter, the detector and the reader for `rate`.
    fn build(&mut self, rate: f64) -> Result<(), ConfigError> {
        let low = || ConfigError::RateTooLow {
            need: MIN_RATE_HZ,
            got: rate,
        };
        let cutoff = (CHANNEL_HZ / rate).min(0.45);
        let taps = ((4.0 * rate / 100e3).ceil() as usize) | 1;
        let h = design_by_taps(taps, cutoff, Window::Hamming).map_err(|_| low())?;
        self.filter = Some(Fir::new(&h));
        self.settle = taps;
        // Windows of 16 µs (two bits): open `threshold_db` over the least window power, close 4 dB lower.
        self.window = ((16e-6 * rate).round() as usize).max(1);
        let windows_per_s = rate / self.window as f64;
        let floor = MinFollower::with_times(windows_per_s, 0.01, 3.0).map_err(|_| low())?;
        let mut cfg = BurstConfig::new(self.window, (MAX_BURST_S * rate) as usize);
        cfg.open_db = self.threshold_db as f32;
        cfg.close_db = (self.threshold_db - 4.0) as f32;
        cfg.pre = 4 * self.window;
        cfg.post = 4 * self.window;
        cfg.pool = 16;
        self.det = Some(BurstDetector::new(cfg, FloorTracker::MinFollower(floor)).map_err(|_| low())?);
        self.reader = Some(BurstReader::new(rate)?);
        self.base = self.n + taps as u64;
        Ok(())
    }

    /// Reads every finished burst.
    fn drain(&mut self) {
        loop {
            let Some(det) = self.det.as_mut() else { return };
            let Some(b) = det.take() else { return };
            let t = (self.base + b.start) as f64 / self.rate;
            self.burst.clear();
            self.burst.extend_from_slice(b.samples);
            let p = self.burst.iter().map(|z| z.norm_sqr()).fold(0f32, f32::max);
            let level = 10.0 * p.max(1e-20).log10();
            self.held.counters.bursts += 1;
            self.found.clear();
            if let Some(r) = self.reader.as_mut() {
                let bad = r.bad;
                r.read(&self.burst, &mut self.found);
                self.held.counters.bad += r.bad - bad;
            }
            for tg in std::mem::take(&mut self.found) {
                self.accept(tg, t, level);
            }
        }
    }

    /// Adds a subtelegram: a copy of a telegram within the maturity time adds to its count, another is a new telegram.
    fn accept(&mut self, tg: Telegram, t: f64, level: f32) {
        // The switch form has no tested check: it counts only when a second copy arrives within the maturity time.
        if tg.hash == Hash::NotTested {
            self.unchecked.retain(|(at, _)| t - *at <= MATURITY_S);
            let Some(i) = self.unchecked.iter().position(|(_, u)| *u == tg) else {
                self.unchecked.push((t, tg));
                return;
            };
            let (first, copy) = self.unchecked.swap_remove(i);
            self.add(copy.clone(), first, level);
            self.add(copy, t, level);
            return;
        }
        self.add(tg, t, level);
    }

    /// Adds a checked subtelegram (see [`EnoceanDecoder::accept`]).
    fn add(&mut self, tg: Telegram, t: f64, level: f32) {
        self.held.counters.subtelegrams += 1;
        self.dirty = true;
        let sender = tg.sender_text();
        let secure = frame::is_secure(tg.rorg);
        let data = if secure { String::new() } else { hex(&tg.data) };
        if let Some(e) = self.held.telegrams.find_recent_mut(16, |e| {
            t - e.t <= MATURITY_S
                && e.sender == sender
                && e.rorg == tg.rorg
                && e.radio == tg.radio
                && e.data == data
        }) {
            e.subtelegrams += 1;
            return;
        }
        let at = Stamp {
            t,
            wall: wall(self.live),
        };
        let eep = self.held.devices.get(&sender).and_then(|d| d.eep);
        let r = frame::reading(&tg, eep);
        let Ok(up) = self.held.devices.upsert(&sender, at, || Device {
            id: sender.clone(),
            radio: tg.radio,
            rorg: tg.rorg,
            eep: None,
            profile: None,
            reading: String::new(),
            telegrams: 0,
            secure: 0,
            level_db: level,
        }) else {
            return;
        };
        let d = up.row;
        d.radio = tg.radio;
        d.rorg = tg.rorg;
        if r.learned.is_some() {
            d.eep = r.learned;
        }
        if r.profile.is_some() {
            d.profile.clone_from(&r.profile);
        }
        d.reading.clone_from(&r.text);
        d.telegrams += 1;
        d.secure += u64::from(secure);
        d.level_db = level;
        let entry = LogEntry {
            t,
            wall: at.wall,
            radio: tg.radio,
            rorg: tg.rorg,
            sender,
            dest: tg.dest.map(|v| format!("{v:08X}")),
            reading: r.text,
            data,
            hops: tg.hops_text().to_string(),
            subtelegrams: 1,
            hash: tg.hash,
            level_db: level,
        };
        self.held.counters.telegrams += 1;
        self.held.telegrams.push(entry.clone());
        self.out.push(Timed {
            time: t,
            value: entry,
        });
    }
}

impl Decoder for EnoceanDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "ism.enocean"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<LogEntry>("telegrams")]
    }
    fn settings(&self) -> Arc<sdrkit_core::Schema> {
        EnoceanSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(EnoceanSetting::Threshold, Value::F64(self.threshold_db))])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match EnoceanSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            EnoceanSetting::Threshold => self.threshold_db = THRESHOLD.read(id, v)?,
        }
        if self.rate > 0.0 {
            let _ = self.build(self.rate);
        }
        Ok(())
    }
    fn results(&self) -> Published {
        crate::typed::results(ID)
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE_HZ {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.build(input.rate)?;
        Ok(vec![Format {
            rate: 0.0,
            frame_len: 0,
            media: None,
            ..input.with_kind(Kind::typed::<LogEntry>())
        }])
    }
    fn retune(&mut self, _input: &Format) {
        if self.rate > 0.0 {
            let _ = self.build(self.rate);
        }
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.filtered.clear();
        self.filtered.resize(x.len(), Complex32::default());
        match self.filter.as_mut() {
            Some(f) => f.process(x, &mut self.filtered),
            None => self.filtered.copy_from_slice(x),
        }
        for z in x {
            self.level_acc += f64::from(z.norm_sqr());
        }
        self.level_n += x.len() as u64;
        self.n += x.len() as u64;
        let w = self.window.max(1);
        let mut at = self.settle.min(self.filtered.len());
        self.settle -= at;
        while at < self.filtered.len() {
            let end = (at + w).min(self.filtered.len());
            if let Some(d) = self.det.as_mut() {
                d.process(&self.filtered[at..end]);
            }
            self.drain();
            at = end;
        }
        self.now = (first + x.len() as u64) as f64 / self.rate.max(1.0);
        out.items::<LogEntry>(0).append(&mut self.out);
    }
    fn report(&mut self, p: &mut Published) {
        if self.level_n > 0 {
            self.level_dbfs =
                Some(10.0 * (self.level_acc / self.level_n as f64).max(1e-20).log10());
            self.level_acc = 0.0;
            self.level_n = 0;
        }
        let c = self.held.counters;
        p.put(&EnoceanFields {
            telegrams: c.telegrams as i64,
            subtelegrams: c.subtelegrams as i64,
            frames_bad: c.bad as i64,
            devices: self.held.devices.len() as i64,
            bursts: c.bursts as i64,
            level_dbfs: self.level_dbfs,
            note: NOT_CONFIRMED,
        });
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&ENOCEAN_TELEGRAMS);
            p.clear_rows(&ENOCEAN_DEVICES);
        }
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.held.telegrams, &ENOCEAN_TELEGRAMS, self.now, p);
            shaped::rows(&self.held.devices, &ENOCEAN_DEVICES, self.now, p);
        }
        let newest = self.held.telegrams.newest_first().next();
        let recent = newest.is_some_and(|f| self.now - f.t <= RECENT_S);
        let lock = if !self.started {
            LockState::Off
        } else if recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let summary = if c.telegrams > 0 {
            format!(
                "EnOcean: {} telegrams from {} devices, {} failed their check (decoded from the public documents; not confirmed on a real device)",
                c.telegrams,
                self.held.devices.len(),
                c.bad
            )
        } else if self.started {
            "EnOcean: listening for telegrams (ERP1 and ERP2)".to_string()
        } else {
            "EnOcean: waiting for samples".to_string()
        };
        set_status(p, lock, &summary);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.held.clear();
        self.cleared = true;
        self.dirty = true;
    }
    fn forget(&mut self, kept: &Kept) {
        self.level_dbfs = None;
        self.level_acc = 0.0;
        self.level_n = 0;
        if !kept.table(ENOCEAN_TELEGRAMS.key) {
            self.held.telegrams.clear();
            self.cleared = true;
        }
        if !kept.table(ENOCEAN_DEVICES.key) {
            self.held.devices.clear();
            self.cleared = true;
        }
        let c = &mut self.held.counters;
        if !kept.field("telegrams") {
            c.telegrams = 0;
        }
        if !kept.field("subtelegrams") {
            c.subtelegrams = 0;
        }
        if !kept.field("frames_bad") {
            c.bad = 0;
        }
        if !kept.field("bursts") {
            c.bursts = 0;
        }
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

/// The `enocean` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a value of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("rx", DecoderComponent::new(EnoceanDecoder::default()))
        .input("rx")
        .output("rx")
        .clock(REPORT_S)
        .status_from("rx")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE_HZ, &min_rate_message("EnOcean", MIN_RATE_HZ))
        .route(THRESHOLD.key(), &["rx.threshold"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// One telegram of the test signal: RORG, data, sender, ERP1 status byte.
pub type TestTelegram = (u8, &'static [u8], u32, u8);

/// The telegrams of the test signal, in order, each written from the profile sheets: a rocker (F6-02-01, N-message, button
/// A0 pressed), a contact (D5-00-01, closed), the teach-ins and data of A5-02-05 (21.0 °C), A5-04-01 (50 %, 20.0 °C), A5-06-01
/// (range ILL1) and A5-07-01 (motion), a secure telegram, and a rocker release with a CRC status.
pub const TEST_TELEGRAMS: [TestTelegram; 11] = [
    (0xF6, &[0x30], 0x0029_1A01, 0x30),
    (0xD5, &[0x09], 0x0180_A1B2, 0x00),
    (0xA5, &[0x08, 0x28, 0x0B, 0x80], 0x0181_0001, 0x00),
    (0xA5, &[0x00, 0x00, 121, 0x08], 0x0181_0001, 0x00),
    (0xA5, &[0x10, 0x08, 0x0B, 0x80], 0x0181_0002, 0x00),
    (0xA5, &[0x00, 125, 125, 0x0A], 0x0181_0002, 0x00),
    (0xA5, &[0x1C, 0x08, 0x0B, 0x80], 0x0181_0003, 0x00),
    (0xA5, &[165, 0x00, 0x80, 0x09], 0x0181_0003, 0x00),
    (0xA5, &[0x18, 0x08, 0x0B, 0x80], 0x0181_0004, 0x00),
    (0x30, &[0x4D, 0x1A, 0x77, 0x0C, 0xE2, 0x95], 0x0182_0001, 0x00),
    (0xF6, &[0x00], 0x0029_1A01, 0xA0),
];

/// The A5-06-01 data telegram that follows its teach-in (DB1 = 128, about 30 000 lx; supply 3.0 V).
pub const TEST_LIGHT: TestTelegram = (0xA5, &[150, 0x00, 128, 0x08], 0x0181_0004, 0x00);

/// The ERP1 envelope of `bytes` at `rate`: a lead-in at the low level, then each logical bit as one level for 8 µs (a 1 at
/// `low`, a 0 at full amplitude), then the low level for one more bit.
pub fn erp1_samples(bytes: &[u8], rate: f64, low: f32) -> Vec<Complex32> {
    let mut bits = vec![1u8; 3];
    frame::erp1_bits(bytes, &mut bits);
    bits.push(1);
    let per_bit = rate / BIT_RATE;
    let n = (bits.len() as f64 * per_bit).ceil() as usize;
    (0..n)
        .map(|i| {
            let b = bits[((i as f64 / per_bit) as usize).min(bits.len() - 1)];
            Complex32::new(if b == 1 { low } else { 1.0 }, 0.0)
        })
        .collect()
}

/// The ERP2 samples of `data_pl` at `rate`: continuous-phase frequency keying, ±62.5 kHz, a 1 the upper tone.
pub fn erp2_samples(data_pl: &[u8], rate: f64) -> Vec<Complex32> {
    let mut bits = Vec::new();
    frame::erp2_bits(data_pl, &mut bits);
    let Ok(mut m) = Cpm::with_deviation(Shape::Rect, 1.0, ERP2_DEVIATION_HZ, BIT_RATE, rate) else {
        return Vec::new();
    };
    let levels: Vec<f32> = bits.iter().map(|&b| if b == 1 { 1.0 } else { -1.0 }).collect();
    let mut out = Vec::new();
    m.push(&levels, &mut out);
    m.flush(&mut out);
    out
}

/// The test telegrams in order: the list, with the light data after its teach-in.
pub fn test_telegrams() -> Vec<TestTelegram> {
    let mut v = TEST_TELEGRAMS.to_vec();
    v.insert(9, TEST_LIGHT);
    v
}

/// The test signal at `rate`: one telegram every 80 ms, each as three subtelegrams 3 ms and 25 ms after the first (inside the
/// slots of an original sender), ERP1 at -24 dB low level or ERP2.
///
/// # Errors
///
/// [`ConfigError`] if the scene refuses the rate.
pub fn scene(rate: f64, radio: GenRadio) -> Result<Scene, ConfigError> {
    let mut scene = Scene::new(rate)?;
    let list = test_telegrams();
    let bursts: Vec<Vec<Complex32>> = list
        .iter()
        .map(|&(rorg, data, sender, status)| match radio {
            GenRadio::Erp1 => erp1_samples(
                &frame::erp1_encode(rorg, data, None, sender, status),
                rate,
                0.063,
            ),
            GenRadio::Erp2 => erp2_samples(&frame::erp2_encode(rorg, data, None, sender), rate),
        })
        .collect();
    let (mut k, mut copy) = (0usize, 0usize);
    scene.add_source(Box::new(move || {
        let start = 0.01 + 0.08 * k as f64 + [0.0, 0.003, 0.025][copy];
        let x = Burst {
            start_s: start,
            offset_hz: 0.0,
            level_db: 0.0,
            samples: bursts[k % bursts.len()].clone(),
        };
        copy += 1;
        if copy == 3 {
            copy = 0;
            k += 1;
        }
        Some(x)
    }));
    Ok(scene)
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let radio = cfg.settings.get(&GEN_RADIO);
    Ok(Box::new(CleanScene::new(
        scene(cfg.rate_hz, radio)?,
        cfg.rate_hz,
        SignalInfo::new(300e3).with_bit_rate(BIT_RATE).with_power(0.5),
    )))
}

/// EnOcean. The default frequency is that of ERP1; the plan `enocean` gives the ERP2 frequencies.
pub static ENOCEAN: StackDescriptor = StackDescriptor {
    id: ID,
    name: "EnOcean",
    group: "Home automation",
    blurb: "EnOcean switches, contacts and sensors (ERP1 at 868.3 MHz, ERP2 at 902.875 and 928.35 MHz): sender, telegram type and the readings of the common profiles; secure telegrams shown as encrypted.",
    tuning: Tuning::anywhere(ERP1_HZ, RATE_HZ, MIN_RATE_HZ, 300e3),
    submodes: &[],
    settings: EnoceanStackSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

fn verify_cases(o: &sdrkit_core::verify::VerifyOptions) -> Vec<sdrkit_core::verify::VerifyCase> {
    let mut c = sdrkit_core::verify::VerifyCase::new("test signal, ERP1 telegrams", o, 0.0);
    c.seconds = o.seconds.max(2.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = EnoceanFields::read(r.fields());
        vec![
            crate::family::at_least_value("telegrams", f.as_ref().map_or(0, |f| f.telegrams), 20),
            crate::family::at_least_value("devices", f.as_ref().map_or(0, |f| f.devices), 7),
        ]
    });
    vec![c]
}

/// The verify case of `enocean`.
pub static VERIFY: sdrkit_core::verify::VerifyHooks = sdrkit_core::verify::VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The benchmark of the receiver: 1 s of the ERP1 test signal at 1 Msps, fed in one piece.
pub fn benchmarks() -> Vec<sdrkit_core::bench::Benchmark> {
    vec![sdrkit_core::bench::Benchmark {
        name: "enocean.receiver_1m",
        run: bench_receiver,
    }]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::bench::BenchResult {
    let rate = RATE_HZ;
    let mut x = vec![Complex32::default(); rate as usize];
    if let Ok(mut s) = scene(rate, GenRadio::Erp1) {
        s.generate(&mut x);
    }
    let Ok(mut rx) = receiver(&StackConfig::new(Values::defaults(
        &EnoceanStackSetting::schema(),
    ))) else {
        return sdrkit_core::bench::BenchResult::default();
    };
    let _ = rx.configure(&sdrkit_core::InputInfo::new(rate, ERP1_HZ, ERP1_HZ).recording());
    let mut out = sdrkit_core::Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), 1.0, || {
        out.clear();
        rx.feed(&x, &mut out);
    })
}
