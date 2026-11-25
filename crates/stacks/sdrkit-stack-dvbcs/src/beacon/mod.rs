//! The TV satellite beacon decoder (`satbeacon`): the carrier measurements and the PCM telemetry frames of the narrow beacon
//! carrier of a TV satellite.
//!
//! Standard: IRIG 106 chapter 4 (PCM frame sync patterns).

pub mod dsp;
pub mod frames;

use crate::typed::{
    SatbeaconCarrierEvent, SatbeaconDecodedRow, SatbeaconFields, SatbeaconFrameEvent,
    SatbeaconFrameRow, SatbeaconLineRow, SATBEACON_CARRIER, SATBEACON_DECODED, SATBEACON_FRAME,
    SATBEACON_FRAMES, SATBEACON_LINES,
};
use sdrkit_proto_dvbs2::receive::write_status;

use dsp::{biphase, chip_rate, find_lines, subcarrier, CarrierPll, Line, SubDemod};
use frames::{analyse, BeaconFrame, Learner};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::command::Command;
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::record::Record;
use sdrkit_core::results::{LockState, SeriesId};
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, EventDef, GenConfig,
    Generator, LockProfile, Receiver, Setting, SettingSet, SignalInfo, StackConfig, Values,
};
use sdrkit_graph::{
    no_values, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io, Kept, Kind,
    Port, RateFrom, StackGraph, Timed,
};
use sdrkit_proto_dvbs2::transmit::TEST_SIGNAL;
use std::f64::consts::PI;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "satbeacon";

/// The lowest input rate.
pub const MIN_RATE: f64 = 250e3;

/// Input samples per graph pass.
pub const PASS: usize = 4096;

/// The period of the results, seconds of signal.
pub const REPORT_S: f64 = 0.25;

/// The spacing of the pilot carrier of the test signal above the main carrier, Hz.
pub const PILOT_SPACING_HZ: f64 = 545_642.0;

/// Bits between the telemetry and the frame stages.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BeaconBits {
    /// Bits (0 or 1).
    pub bits: Vec<u8>,
}

impl sdrkit_graph::Item for BeaconBits {
    const KIND: &'static str = "beacon.bits";
}

/// The bit rate of the test signal.
pub const GEN_BITRATE: Setting<field::F64> =
    Setting::new("gen_bitrate", "Bit rate", "bit/s", 4800.0)
        .range(300.0, 9600.0, 100.0)
        .section(TEST_SIGNAL)
        .advanced();
/// The subcarrier of the test signal.
pub const GEN_SUBCARRIER_KHZ: Setting<field::F64> =
    Setting::new("gen_subcarrier_khz", "Subcarrier", "kHz", 48.0)
        .range(5.0, 100.0, 0.5)
        .section(TEST_SIGNAL)
        .advanced();
/// NRZ instead of bi-phase in the test signal.
pub const GEN_NRZ: Setting<field::Bool> =
    Setting::new("gen_nrz", "NRZ instead of bi-phase", "", false)
        .section(TEST_SIGNAL)
        .advanced();
/// The test signal is the carrier only.
pub const GEN_CARRIER_ONLY: Setting<field::Bool> =
    Setting::new("gen_carrier_only", "Carrier only", "", false)
        .section(TEST_SIGNAL)
        .advanced();
/// A pilot carrier in the test signal.
pub const GEN_PILOT: Setting<field::Bool> = Setting::new("gen_pilot", "Pilot carrier", "", true)
    .section(TEST_SIGNAL)
    .help("A second carrier 545.642 kHz above the beacon, 8 dB below it.")
    .advanced();

sdrkit_core::settings! {
    /// The settings of the beacon decoder: the options of its test signal. The carrier offset, its drift and the noise are
    /// the generator settings that the framework adds.
    pub enum BeaconSetting {
        /// The bit rate of the test signal.
        GenBitrate = GEN_BITRATE,
        /// The subcarrier of the test signal.
        GenSubcarrierKhz = GEN_SUBCARRIER_KHZ,
        /// NRZ in the test signal.
        GenNrz = GEN_NRZ,
        /// The carrier only.
        GenCarrierOnly = GEN_CARRIER_ONLY,
        /// A pilot carrier.
        GenPilot = GEN_PILOT,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The carrier.

/// The state the carrier stage sends to the frame stage on each tick of the graph clock.
#[derive(Clone, Debug, Default)]
pub struct CarrierState {
    /// Lines of the last second, strongest first.
    pub lines: Vec<Line>,
    /// The main carrier's offset, Hz (the PLL's frequency).
    pub carrier_hz: Option<f64>,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// C/N0, dB-Hz.
    pub cn0: f64,
    /// Noise density, dBFS/Hz.
    pub noise_density: f64,
    /// Drift, Hz per minute.
    pub drift: f64,
    /// Carrier locked.
    pub carrier_locked: bool,
    /// PM rms, degrees.
    pub pm_rms_deg: f64,
    /// Work rate.
    pub work_rate: f64,
    /// The spacing of a second carrier above the main one, Hz.
    pub pilot_hz: Option<f64>,
    /// RF spectrum picture.
    pub rf: Vec<f32>,
    /// Signal time, s.
    pub time: f64,
    /// The tuned centre, Hz (for the RF frequency).
    pub centre_hz: f64,
    /// Log lines since the last state.
    pub log: Vec<String>,
}

impl sdrkit_graph::Item for CarrierState {
    const KIND: &'static str = "beacon.carrier_state";
}

/// `beacon.carrier`: the line finder, the mixer and decimator to the work rate and the carrier PLL. Out: the phase modulation and
/// the state.
#[derive(Default)]
pub struct BeaconCarrier {
    fs: f64,
    buf: Vec<Complex32>,
    n: usize,
    pll: Option<CarrierPll>,
    track: Vec<(f64, f64)>,
    t: f64,
    st: CarrierState,
}

impl Component for BeaconCarrier {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "beacon.carrier",
            vec![Port::iq("in")],
            vec![
                Port::real("pm"),
                Port::typed::<CarrierState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = BeaconCarrier::default();
        self.fs = inputs[0].rate;
        self.n = 1usize << ((self.fs.log2().floor() as u32).min(18));
        self.buf = Vec::with_capacity(self.n);
        self.st.centre_hz = inputs[0].centre_hz;
        let dec = ((self.fs / 200e3).floor() as usize).max(1);
        Ok(vec![
            Format::new(Kind::Real, self.fs / dec as f64),
            Format::new(Kind::typed::<CarrierState>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.t += x.len() as f64 / self.fs;
        let st = &mut self.st;
        let mut rest = x;
        while !rest.is_empty() {
            let take = (self.n - self.buf.len()).min(rest.len());
            self.buf.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.buf.len() == self.n {
                let (lines, nd, pic) = find_lines(&self.buf, self.fs, 6);
                self.buf.clear();
                st.rf = pic;
                st.noise_density = nd;
                if let Some(main) = lines.first().copied().filter(|l| l.over_noise_db > 20.0) {
                    if self.pll.is_none() {
                        self.pll = Some(CarrierPll::new(self.fs, main.offset_hz, 15.0));
                        st.log.push(format!(
                            "carrier {:+.1} Hz, {:.1} dB-Hz",
                            main.offset_hz,
                            main.level_dbfs - nd
                        ));
                    }
                    st.level_dbfs = main.level_dbfs;
                    st.cn0 = main.level_dbfs - nd;
                    st.pilot_hz = lines
                        .iter()
                        .skip(1)
                        .find(|l| (l.offset_hz - main.offset_hz).abs() > 120e3)
                        .map(|l| l.offset_hz - main.offset_hz);
                } else if self.pll.is_none() {
                    st.carrier_hz = None;
                }
                st.lines = lines;
            }
        }
        st.time = self.t;
        if let Some(pll) = self.pll.as_mut() {
            let out = io.real_out(0);
            pll.process(x, out);
            let f = pll.freq_hz();
            if (self.track.last().map_or(-1.0, |t| t.0) + 1.0) <= self.t {
                self.track.push((self.t, f));
                while self.track.first().is_some_and(|t| t.0 < self.t - 300.0) {
                    self.track.remove(0);
                }
            }
            st.carrier_hz = Some(f);
            st.carrier_locked = pll.lock > 0.3;
            st.pm_rms_deg = pll.pm_rms.sqrt().to_degrees();
            st.work_rate = pll.work_rate();
            if self.track.len() >= 3 {
                let n = self.track.len() as f64;
                let (sum_t, sum_f) = self
                    .track
                    .iter()
                    .fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
                let (mt, mf) = (sum_t / n, sum_f / n);
                let (num, den) = self.track.iter().fold((0.0, 0.0), |a, p| {
                    (a.0 + (p.0 - mt) * (p.1 - mf), a.1 + (p.0 - mt).powi(2))
                });
                st.drift = num / den.max(1e-9) * 60.0;
            }
        }
        if io.tick() {
            let v = CarrierState {
                log: std::mem::take(&mut st.log),
                ..st.clone()
            };
            io.items_out::<CarrierState>(1).push(Timed {
                time: self.t,
                value: v,
            });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The telemetry.

/// The state the telemetry stage sends to the frame stage on each tick of the graph clock.
#[derive(Clone, Debug, Default)]
pub struct TelemetryState {
    /// PM spectrum picture.
    pub pm: Vec<f32>,
    /// Subcarrier: centre, tones, spacing, over noise.
    pub subcarrier: Option<(f64, u32, f64, f64)>,
    /// Bit rate.
    pub bit_rate: f64,
    /// "NRZ" or "bi-phase".
    pub coding: Option<Coding>,
    /// Clock locked.
    pub clock_locked: bool,
}

impl sdrkit_graph::Item for TelemetryState {
    const KIND: &'static str = "beacon.telemetry_state";
}

/// `beacon.telemetry`: the subcarrier, the Costas loop, the chip clock and the bits.
#[derive(Default)]
pub struct BeaconTelemetry {
    rate: f64,
    spec: Vec<f64>,
    spec_n: usize,
    seg: Vec<f32>,
    fft: Option<Fft>,
    win: Vec<Complex32>,
    demod: Option<SubDemod>,
    biphase: Option<(bool, usize)>,
    st: TelemetryState,
}

impl Component for BeaconTelemetry {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "beacon.telemetry",
            vec![Port::real("pm")],
            vec![
                Port::typed::<BeaconBits>("bits").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<TelemetryState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = BeaconTelemetry::default();
        self.rate = inputs[0].rate;
        self.spec = vec![0.0; 4096];
        self.fft = Some(Fft::forward(8192));
        Ok(vec![
            Format::new(Kind::typed::<BeaconBits>(), 0.0),
            Format::new(Kind::typed::<TelemetryState>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let pm = io.real(0);
        self.bits(io, pm);
        if io.tick() {
            io.items_out::<TelemetryState>(1).push(Timed {
                time: 0.0,
                value: self.st.clone(),
            });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

impl BeaconTelemetry {
    fn bits(&mut self, io: &mut Io, pm: &[f32]) {
        if pm.is_empty() {
            return;
        }
        // The PM spectrum (8192 points, 0 to rate / 2), averaged.
        self.seg.extend_from_slice(pm);
        while self.seg.len() >= 8192 {
            self.win.clear();
            self.win
                .extend(self.seg[..8192].iter().enumerate().map(|(i, &v)| {
                    Complex32::new(
                        v * (0.5 - 0.5 * (2.0 * PI * i as f64 / 8192.0).cos()) as f32,
                        0.0,
                    )
                }));
            self.seg.drain(..8192);
            if let Some(f) = self.fft.as_mut() {
                f.process(&mut self.win);
            }
            for (s, v) in self.spec.iter_mut().zip(&self.win[..4096]) {
                *s += v.norm_sqr() as f64;
            }
            self.spec_n += 1;
        }
        if self.demod.is_none() && self.spec_n >= 16 {
            let s: Vec<f64> = self.spec.iter().map(|v| v / self.spec_n as f64).collect();
            let m = s.iter().cloned().fold(1e-30, f64::max);
            self.st.pm = s
                .chunks(16)
                .map(|c| {
                    sdrkit_algo_dsp::meter::power_to_db(c.iter().cloned().fold(0.0, f64::max) / m)
                        as f32
                })
                .collect();
            match subcarrier(&s, self.rate) {
                Some(sc) if sc.3 > 10.0 => {
                    self.st.subcarrier = Some(sc);
                    let bw = if sc.1 == 2 { sc.2 } else { 8000.0 };
                    self.demod = Some(SubDemod::new(self.rate, sc.0, bw));
                }
                _ => {
                    self.st.subcarrier = None;
                    self.spec.iter_mut().for_each(|v| *v = 0.0);
                    self.spec_n = 0;
                }
            }
        }
        let Some(d) = self.demod.as_mut() else {
            return;
        };
        d.process(pm);
        if d.chip_rate <= 0.0 && d.base.len() >= (self.rate * 0.5) as usize {
            if let Some(r) = chip_rate(&d.base, self.rate) {
                d.set_chip_rate(r);
            }
        }
        // Chips to bits.
        if self.biphase.is_none() && d.chips.len() >= 400 {
            let (bp, skip) = biphase(&d.chips[d.chips.len() - 400..]);
            self.biphase = Some((bp, skip));
            if bp {
                d.chips.drain(..skip);
            }
        }
        let Some((bp, _)) = self.biphase else {
            return;
        };
        let mut bits = Vec::new();
        if bp {
            let n = d.chips.len() / 2;
            for i in 0..n {
                bits.push(d.chips[2 * i]);
            }
            d.chips.drain(..2 * n);
        } else {
            bits.append(&mut d.chips);
        }
        self.st.coding = Some(if bp { Coding::Biphase } else { Coding::Nrz });
        self.st.bit_rate = d.clock_rate() / if bp { 2.0 } else { 1.0 };
        self.st.clock_locked = d.clock_lock > 0.3;
        if let Some(sc) = self.st.subcarrier.as_mut() {
            sc.0 = d.subcarrier_hz();
        }
        if !bits.is_empty() {
            io.items_out::<BeaconBits>(0).push(Timed {
                time: 0.0,
                value: BeaconBits { bits },
            });
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The frames and the results.

sdrkit_core::choice_enum! {
    /// What the beacon decoder does now.
    pub enum BeaconState {
        /// No carrier.
        NoCarrier = ("no_carrier", "No carrier"),
        /// The carrier loop is locked.
        CarrierLocked = ("carrier_locked", "Carrier locked"),
        /// The bit clock is locked.
        ClockLocked = ("clock_locked", "Bit clock locked"),
        /// Frames are found.
        FramesFound = ("frames_found", "Frames found"),
    }
}

sdrkit_core::choice_enum! {
    /// The line code of the telemetry.
    pub enum Coding {
        /// NRZ.
        Nrz = ("nrz", "NRZ"),
        /// Bi-phase.
        Biphase = ("biphase", "Bi-phase"),
    }
}

impl sdrkit_graph::Item for BeaconFrame {
    const KIND: &'static str = "beacon.frame";
    fn event_def() -> Option<&'static EventDef> {
        Some(SATBEACON_FRAME.def())
    }
    fn write_fields(&self, r: &mut Record) {
        SatbeaconFrameEvent {
            frame: self.id as i64,
            counter: self.counter.map(i64::from),
            errors: i64::from(self.errors),
            good: self.good,
            bytes: Arc::from(self.bytes.as_slice()),
        }
        .write(r);
    }
}

/// The carrier measurements of one second of signal.
#[derive(Clone, Copy, Debug, Default)]
pub struct CarrierReading {
    /// Signal time, s.
    pub time: f64,
    /// The carrier's offset, Hz.
    pub offset_hz: Option<f64>,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// C/N0, dB-Hz.
    pub cn0: f64,
    /// Drift, Hz per minute.
    pub drift: f64,
    /// Bit rate, bit/s.
    pub bit_rate: f64,
    /// Frame length, bits.
    pub frame_bits: usize,
}

impl sdrkit_graph::Item for CarrierReading {
    const KIND: &'static str = "beacon.carrier";
    fn event_def() -> Option<&'static EventDef> {
        Some(SATBEACON_CARRIER.def())
    }
    fn write_fields(&self, r: &mut Record) {
        SatbeaconCarrierEvent {
            signal_time: Seconds(self.time),
            offset_hz: self.offset_hz.map(Hertz),
            level_dbfs: self.level_dbfs,
            cn0_dbhz: self.cn0,
            drift_hz_min: self.drift,
            bit_rate_bps: (self.bit_rate > 0.0).then_some(self.bit_rate),
            frame_bits: (self.frame_bits > 0).then_some(self.frame_bits as i64),
        }
        .write(r);
    }
}

const RF_SPECTRUM: SeriesId = SeriesId(0);
const PM_SPECTRUM: SeriesId = SeriesId(1);
const COLUMN_STABILITY: SeriesId = SeriesId(2);

/// Forgets the frames learned.
pub const CLEAR: Command = Command::new("clear", "Clear").help("Forgets the frames learned.");

sdrkit_core::commands! {
    /// The commands of [`BeaconFrames`].
    pub enum BeaconCommand {
        /// Forgets the frames learned.
        Clear = CLEAR,
    }
}

/// `beacon.frames`: the frame learner (sync word, frame length, template, counter) and the results. Output `frames` carries
/// each frame found, `readings` the carrier measurements once a second. Command `clear` forgets the frames learned.
pub struct BeaconFrames {
    learner: Learner,
    published: Published,
    carrier: CarrierState,
    telemetry: TelemetryState,
    first_frames: Option<f64>,
    template: Arc<[u8]>,
    fixed_mask: Arc<[u8]>,
    last_line: f64,
    key: String,
    summary: String,
    new: Vec<BeaconFrame>,
}

impl Default for BeaconFrames {
    fn default() -> Self {
        BeaconFrames {
            learner: Learner::default(),
            published: Published::from_decl(Arc::new(crate::typed::shape(ID).results_decl())),
            carrier: CarrierState::default(),
            telemetry: TelemetryState::default(),
            first_frames: None,
            template: Arc::from(&[][..]),
            fixed_mask: Arc::from(&[][..]),
            last_line: f64::NEG_INFINITY,
            key: String::new(),
            summary: String::new(),
            new: Vec::new(),
        }
    }
}

impl BeaconFrames {
    fn publish(&mut self, readings: &mut Vec<Timed<CarrierReading>>) {
        let (c, t, l) = (&self.carrier, &self.telemetry, &self.learner);
        let rf = c.carrier_hz.map(|f| f + c.centre_hz);
        let state = if l.good > 0 {
            BeaconState::FramesFound
        } else if t.bit_rate > 0.0 && t.subcarrier.is_some() {
            BeaconState::ClockLocked
        } else if c.carrier_hz.is_some() {
            BeaconState::CarrierLocked
        } else {
            BeaconState::NoCarrier
        };
        if state == BeaconState::FramesFound && self.first_frames.is_none() {
            self.first_frames = Some(c.time);
        }
        if *self.template != *l.template {
            self.template = Arc::from(l.template.as_slice());
        }
        if *self.fixed_mask != *l.fixed {
            self.fixed_mask = Arc::from(l.fixed.as_slice());
        }
        let p = &mut self.published;
        let subcarrier = t.subcarrier;
        p.put(&SatbeaconFields {
            state,
            signal_time: Seconds(c.time),
            work_rate: Hertz(c.work_rate),
            offset_hz: c.carrier_hz.map(Hertz),
            rf: rf.map(Hertz),
            level_dbfs: c.level_dbfs,
            cn0_dbhz: c.cn0,
            drift_hz_min: c.drift,
            noise_density_dbfs: c.noise_density,
            carrier_locked: c.carrier_locked,
            pm_rms_deg: c.pm_rms_deg,
            subcarrier: subcarrier.map(|s| Hertz(s.0)),
            tones: subcarrier.map(|s| s.1 as i64),
            tone_spacing: subcarrier.map(|s| Hertz(s.2)),
            subcarrier_snr_db: subcarrier.map(|s| s.3),
            bit_rate_bps: (t.bit_rate > 0.0).then_some(t.bit_rate),
            coding: t.coding,
            clock_locked: t.clock_locked,
            frame_bits: (l.frame_len > 0).then_some(l.frame_len as i64),
            frame_agreement: l.agreement,
            fixed_bits: l.fixed.iter().map(|m| m.count_ones() as i64).sum(),
            sync_word: l.sync,
            counter_byte: l.counter.map(|c| c.0 as i64),
            counter_modulo: l.counter.map(|c| c.1 as i64),
            bits: l.total_bits as i64,
            frames_ok: l.good as i64,
            frames_bad: l.bad as i64,
            template: Arc::clone(&self.template),
            fixed_mask: Arc::clone(&self.fixed_mask),
            pilot_spacing: c.pilot_hz.map(Hertz),
            lines: c.lines.len() as i64,
            first_frames: self.first_frames.map(Seconds),
        });
        let key = &mut self.key;
        for (i, li) in c.lines.iter().enumerate() {
            key.clear();
            let _ = write!(key, "{i}");
            SATBEACON_LINES.put(
                p,
                key,
                &SatbeaconLineRow {
                    offset: Hertz(li.offset_hz),
                    level_dbfs: li.level_dbfs,
                    over_noise_db: li.over_noise_db,
                },
            );
        }
        for i in c.lines.len()..SATBEACON_LINES.cap {
            key.clear();
            let _ = write!(key, "{i}");
            p.remove_row(SATBEACON_LINES.def(), key);
        }
        for f in l.frames.iter().rev() {
            key.clear();
            let _ = write!(key, "{}", f.id);
            SATBEACON_FRAMES.put(
                p,
                key,
                &SatbeaconFrameRow {
                    frame: f.id as i64,
                    counter: f.counter.map(i64::from),
                    errors: i64::from(f.errors),
                    good: f.good,
                    bytes: Arc::from(f.bytes.as_slice()),
                    time_s: f.time_s,
                },
            );
        }
        let frame_s = if t.bit_rate > 0.0 {
            l.frame_len as f64 / t.bit_rate
        } else {
            1.0
        };
        for v in analyse(l, frame_s, rf) {
            key.clear();
            let _ = write!(key, "{}{}", v.name, v.bytes);
            SATBEACON_DECODED.put(
                p,
                key,
                &SatbeaconDecodedRow {
                    name: &v.name,
                    kind: v.kind,
                    value: &v.value,
                    bytes: &v.bytes,
                    confidence: v.confidence,
                    reason: &v.reason,
                },
            );
        }
        // The carrier measurements once a second of signal.
        if c.time >= self.last_line + 1.0 {
            self.last_line = c.time;
            readings.push(Timed {
                time: c.time,
                value: CarrierReading {
                    time: c.time,
                    offset_hz: c.carrier_hz,
                    level_dbfs: c.level_dbfs,
                    cn0: c.cn0,
                    drift: c.drift,
                    bit_rate: t.bit_rate,
                    frame_bits: l.frame_len,
                },
            });
        }
        let lock_state = match state {
            BeaconState::FramesFound => LockState::Locked,
            BeaconState::CarrierLocked | BeaconState::ClockLocked => LockState::Tracking,
            BeaconState::NoCarrier => LockState::Searching,
        };
        let sm = &mut self.summary;
        sm.clear();
        let _ = match (state, c.carrier_hz) {
            (BeaconState::NoCarrier, _) | (_, None) => {
                write!(sm, "TV sat beacon decoder: no carrier in the capture")
            }
            (BeaconState::CarrierLocked, Some(f)) => write!(
                sm,
                "TV sat beacon decoder: carrier {f:+.1} Hz, {:.1} dB-Hz",
                c.cn0
            ),
            _ => write!(
                sm,
                "TV sat beacon decoder: carrier {:.1} dB-Hz, {:.0} bit/s on {:.1} kHz, {} frames",
                c.cn0,
                t.bit_rate,
                t.subcarrier.map_or(0.0, |s| s.0) / 1e3,
                l.good
            ),
        };
        let r = p.results_mut();
        write_status(
            r,
            lock_state,
            Some(c.cn0 as f32),
            Some(c.level_dbfs as f32),
            c.carrier_hz,
            &self.summary,
        );
        r.update_series(RF_SPECTRUM, |x| {
            x.fill(true, c.rf.iter().enumerate().map(|(i, &v)| [i as f32, v]))
        });
        r.update_series(PM_SPECTRUM, |x| {
            x.fill(true, t.pm.iter().enumerate().map(|(i, &v)| [i as f32, v]))
        });
        r.update_series(COLUMN_STABILITY, |x| {
            x.fill(
                true,
                l.stability.iter().enumerate().map(|(i, &v)| [i as f32, v]),
            )
        });
    }
}

impl Component for BeaconFrames {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "beacon.frames",
            vec![
                Port::typed::<BeaconBits>("bits"),
                Port::typed::<CarrierState>("carrier"),
                Port::typed::<TelemetryState>("telemetry"),
            ],
            vec![
                Port::typed::<BeaconFrame>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<CarrierReading>("readings").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.learner = Learner::default();
        self.first_frames = None;
        Ok(vec![
            Format::new(Kind::typed::<BeaconFrame>(), 0.0),
            Format::new(Kind::typed::<CarrierReading>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        self.new.clear();
        let t = self.carrier.time;
        for b in io.items::<BeaconBits>(0) {
            self.learner.push(&b.value.bits, t, &mut self.new);
        }
        io.items_out::<BeaconFrame>(0)
            .extend(self.new.drain(..).map(|f| Timed { time: t, value: f }));
        let mut fresh = false;
        for c in io.items::<CarrierState>(1) {
            for l in &c.value.log {
                self.published.results_mut().log(l.clone());
            }
            self.carrier.clone_from(&c.value);
            fresh = true;
        }
        if let Some(t) = io.items::<TelemetryState>(2).last() {
            self.telemetry.clone_from(&t.value);
            fresh = true;
        }
        if fresh {
            self.publish(io.items_out::<CarrierReading>(1));
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&sdrkit_core::Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        BeaconCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match BeaconCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            BeaconCommand::Clear => self.clear_frames(),
        }
        Ok(None)
    }
    fn forget(&mut self, _: &Kept) {
        self.clear_frames();
        self.carrier = CarrierState::default();
        self.telemetry = TelemetryState::default();
        self.last_line = f64::NEG_INFINITY;
        self.published.clear_rows(SATBEACON_LINES.def());
    }
}

impl BeaconFrames {
    /// Forgets the frames learned, their rows and the decoded view.
    fn clear_frames(&mut self) {
        self.learner = Learner::default();
        self.first_frames = None;
        self.published.clear_rows(SATBEACON_FRAMES.def());
        self.published.clear_rows(SATBEACON_DECODED.def());
    }
}

/// The graph of the beacon decoder.
pub fn builder() -> GraphBuilder {
    GraphBuilder::new()
        .add("carrier", BeaconCarrier::default())
        .add("telemetry", BeaconTelemetry::default())
        .add("frames", BeaconFrames::default())
        .input("carrier.in")
        .connect("carrier.pm", "telemetry.pm")
        .connect("telemetry.bits", "frames.bits")
        .connect("carrier.state", "frames.carrier")
        .connect("telemetry.state", "frames.telemetry")
        .output("frames.frames")
        .output("frames.readings")
        .clock(REPORT_S)
        .reconfigure_on_retune()
        .status_from("frames")
}

/// The beacon decoder with the settings of `cfg`: the graph in fixed passes of [`PASS`] samples.
///
/// # Errors
///
/// [`GraphError::Setting`] if `cfg` holds a value that a component refuses, or another [`GraphError`] if the graph does not
/// build.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = builder().catalog(&Catalog::of(cfg)?).build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(
            MIN_RATE,
            "too low for the beacon decoder (250 ksps or more)",
        )
        .fixed_passes(PASS)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The generator.

/// The beacon generator's parameters.
#[derive(Clone, Debug)]
pub struct BeaconGenParams {
    /// Sample rate.
    pub fs: f64,
    /// A pilot carrier 545.642 kHz above, 8 dB below the beacon.
    pub pilot: bool,
    /// Subcarrier, Hz.
    pub subcarrier: f64,
    /// Bit rate.
    pub bitrate: f64,
    /// Data clock error of the test signal, ppm.
    pub clock_ppm: f64,
    /// NRZ instead of bi-phase.
    pub nrz: bool,
    /// Peak PM index, rad.
    pub index: f64,
    /// Carrier only.
    pub carrier_only: bool,
    /// Seed of the changing words.
    pub seed: u64,
}

impl BeaconGenParams {
    /// The default test signal at `fs`.
    pub fn new(fs: f64) -> Self {
        BeaconGenParams {
            fs,
            pilot: true,
            subcarrier: 48_000.0,
            bitrate: 4800.0,
            clock_ppm: 7.0,
            nrz: false,
            index: 1.2,
            carrier_only: false,
            seed: 1,
        }
    }
}

/// A test frame (256 bytes): FAF320, FE, the counter, fixed words, commutated, idle fill, seconds, a copy of the
/// counter, changing words and the checksum. The layout is made up for the tests.
pub fn test_frame(n: u64, seconds: u64, rng: &mut Rng) -> Vec<u8> {
    let mut f = vec![0xFBu8; 256];
    f[..3].copy_from_slice(&[0xFA, 0xF3, 0x20]);
    f[3] = 0xFE;
    f[4] = (n % 32) as u8;
    f[5..10].copy_from_slice(&[0x00, 0x00, 0x1E, 0xA3, 0x2B]);
    f[10] = [0x80, 0x08, 0x66][(n % 3) as usize];
    f[15] = (seconds % 256) as u8;
    f[122] = (n % 32) as u8;
    for k in [20usize, 21, 40, 60, 80, 100, 140, 160, 180, 200, 220, 240] {
        f[k] = rng.next_u32() as u8;
    }
    f[30] = 0x60;
    f[31] = 0x00;
    let s = f[..255].iter().fold(0u8, |a, &b| a.wrapping_add(b));
    f[255] = s;
    f
}

/// The beacon generator: the beacon carrier at 0 Hz, phase modulated by a subcarrier with the telemetry bits, and a pilot carrier
/// above it, at unit mean power. The framework moves it to `gen_offset_hz` and adds drift and noise.
pub struct BeaconGenerator {
    p: BeaconGenParams,
    t: u64,
    pilot: sdrkit_algo_dsp::nco::Nco,
    sc_ph: f64,
    data_ph: f64,
    bits: Vec<u8>,
    bit_i: usize,
    frame_n: u64,
    rng: Rng,
    amplitude: f32,
}

impl BeaconGenerator {
    /// A generator with these parameters.
    pub fn new(p: BeaconGenParams) -> Self {
        let pilot_power = if p.pilot { 10f64.powf(-0.8) } else { 0.0 };
        BeaconGenerator {
            rng: Rng::new(p.seed),
            pilot: sdrkit_algo_dsp::nco::Nco::new(p.fs, PILOT_SPACING_HZ),
            amplitude: (1.0 / (1.0 + pilot_power)).sqrt() as f32,
            p,
            t: 0,
            sc_ph: 0.0,
            data_ph: 0.0,
            bits: Vec::new(),
            bit_i: 0,
            frame_n: 0,
        }
    }
    /// The generator of a generator configuration. The schema of its values must be that of [`BeaconSetting`].
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let mut p = BeaconGenParams::new(cfg.rate_hz);
        p.bitrate = s.get(&GEN_BITRATE);
        p.subcarrier = s.get(&GEN_SUBCARRIER_KHZ) * 1e3;
        p.nrz = s.get(&GEN_NRZ);
        p.carrier_only = s.get(&GEN_CARRIER_ONLY);
        p.pilot = s.get(&GEN_PILOT);
        p.seed = cfg.seed;
        Self::new(p)
    }
    fn next_bit(&mut self) -> u8 {
        if self.bit_i >= self.bits.len() {
            let secs = (self.t as f64 / self.p.fs) as u64;
            let f = test_frame(self.frame_n, secs, &mut self.rng);
            self.frame_n += 1;
            self.bits = f
                .iter()
                .flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1))
                .collect();
            self.bit_i = 0;
        }
        let b = self.bits[self.bit_i];
        self.bit_i += 1;
        b
    }
}

impl Generator for BeaconGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.fs
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let fs = self.p.fs;
        let chip_rate =
            self.p.bitrate * (1.0 + self.p.clock_ppm * 1e-6) * if self.p.nrz { 1.0 } else { 2.0 };
        let mut cur_bit = if self.bits.is_empty() {
            self.next_bit()
        } else {
            self.bits[self.bit_i.saturating_sub(1)]
        };
        let a = self.amplitude;
        let r = a * 10f32.powf(-8.0 / 20.0);
        for o in out.iter_mut() {
            let mut phase = 0.0;
            if !self.p.carrier_only {
                let chip_idx = self.data_ph.floor() as u64;
                let d = if self.p.nrz || chip_idx.is_multiple_of(2) {
                    cur_bit
                } else {
                    1 - cur_bit
                };
                let s = if d == 1 { 1.0 } else { -1.0 };
                phase = self.p.index * s * (2.0 * PI * self.sc_ph).sin();
                self.sc_ph = (self.sc_ph + self.p.subcarrier / fs).fract();
                let before = self.data_ph;
                self.data_ph += chip_rate / fs;
                let per_bit = if self.p.nrz { 1.0 } else { 2.0 };
                if (self.data_ph / per_bit).floor() > (before / per_bit).floor() {
                    cur_bit = self.next_bit();
                }
            }
            let (sn, cs) = phase.sin_cos();
            let mut v = Complex32::new(a * cs as f32, a * sn as f32);
            let pilot = self.pilot.next_sample();
            if self.p.pilot {
                v += pilot * r;
            }
            *o = v;
            self.t += 1;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo {
            bandwidth_hz: 2.0 * (self.p.subcarrier + self.p.bitrate),
            symbol_rate_hz: Some(self.p.bitrate),
            bit_rate_hz: Some(self.p.bitrate),
            power: 1.0,
        })
    }
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, sdrkit_core::ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(
    cfg: &GenConfig,
) -> Result<Box<dyn sdrkit_core::Generator>, sdrkit_core::ConfigError> {
    Ok(Box::new(BeaconGenerator::from_config(cfg)))
}

/// The TV satellite beacon decoder stack.
pub static SATBEACON: sdrkit_core::StackDescriptor = sdrkit_core::StackDescriptor {
    id: ID,
    name: "TV sat beacon decoder",
    group: "TV",
    blurb: "The beacon carrier of a TV satellite through an LNB: carrier measurements and PCM telemetry frames.",
    tuning: sdrkit_core::Tuning {
        min_hz: 1000e6,
        max_hz: 20_000e6,
        no_offset_tuning: true,
        ..sdrkit_core::Tuning::anywhere(11_705.25e6, 2e6, MIN_RATE, 1.75e6)
    },
    submodes: &[],
    settings: BeaconSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: sdrkit_core::AutoGain::Radio,
    verify: Some(&crate::verify::BEACON_HOOKS),
    benchmarks: None,
};
