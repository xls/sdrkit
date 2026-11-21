//! GBAS VHF data broadcast (`gbas_vdb`): a detector. It finds the bursts of a ground station on a 25 kHz channel by their
//! power, counts the slots of the 500 ms frame that they occupy and looks for the line of the 10 500 symbol/s rate in their
//! power. The message content is not decoded.
//! Standard: ICAO Annex 10 Volume I, Appendix B 3.6; US patent 11 988 752 B2 (burst timing and fields).

pub mod gen;

use crate::typed::{GbasVdbBurstRow, GbasVdbFields, GBAS_VDB_BURSTS};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::floor::{ChunkPercentile, FloorTracker};
use sdrkit_algo_modem::burst::{BurstConfig, BurstDetector};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Receiver,
    SettingSet, StackConfig, StackDescriptor, StateBlob, StateError, Tuning, Values,
};
use sdrkit_graph::{
    no_values, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Kept, Outbox,
    StackGraph,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The stack id.
pub const ID: &str = "gbas_vdb";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 62.5e3;
/// The working rate, Hz.
pub const WORK_RATE: f64 = 100e3;
/// The width kept flat: 13 kHz each side.
pub const PASS_HZ: f64 = 26e3;
/// The frequency of the test signal, Hz. The session gives the frequency of a real channel.
pub const TEST_SIGNAL_HZ: f64 = 113.0e6;
/// The symbol rate, Hz.
pub const SYMBOL_RATE: f64 = 10_500.0;
/// The frequency the symbol-rate line is compared with, Hz.
pub const COMPARE_HZ: f64 = 7_300.0;
/// The frame, s.
pub const FRAME_S: f64 = 0.5;
/// A slot, s.
pub const SLOT_S: f64 = 0.0625;
/// Slots of a frame.
pub const SLOTS: usize = 8;
/// The power window, samples (1 ms).
pub const WINDOW: usize = 100;
/// Samples under the open level that close a burst (2 ms).
pub const CLOSE_AFTER: usize = 200;
/// The least noise floor, mean power per sample (-120 dBFS): on input of exact zeros no window opens a burst.
pub const FLOOR_MIN: f32 = 1e-12;
/// The shortest and longest bursts that count, s.
pub const BURST_S: (f64, f64) = (0.005, 0.080);
/// The bursts kept for the slot count, s.
pub const SLOT_SPAN_S: f64 = 2.0;
/// A row of the burst log for every this many bursts.
pub const LOG_EVERY: u64 = 16;
/// The most rows of the burst log.
pub const LOG_CAP: usize = 200;
/// What no public document confirms.
pub const NOTE: &str = "Not confirmed by a public document: the slots are counted from the first burst kept, not from the UTC \
     frame, so they are slots busy and not slot letters. The burst content is not decoded.";

/// A row of the burst log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogRow {
    /// Signal time of the burst, s.
    pub t: f64,
    /// Its length, ms.
    pub length_ms: f64,
    /// Slots busy then.
    pub slots_busy: u8,
    /// Its SNR, dB.
    pub snr_db: f64,
}

impl Shift for LogRow {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// What the stack keeps and saves.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Store {
    /// The burst log.
    pub log: LogRing<LogRow>,
    /// Bursts counted.
    pub bursts: u64,
    /// The symbol-rate line was seen.
    pub line_seen: bool,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            log: LogRing::new(LOG_CAP),
            bursts: 0,
            line_seen: false,
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.log.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("avdata.gbas_vdb", 1);
    fn clear(&mut self) {
        self.log.clear();
        self.bursts = 0;
        self.line_seen = false;
    }
    fn adopt(&mut self, saved: Self) {
        self.log.adopt(saved.log);
        self.bursts = saved.bursts;
        self.line_seen = saved.line_seen;
    }
}

impl Shaped for LogRow {
    type Row = GbasVdbBurstRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&GbasVdbBurstRow {
            time_s: self.t,
            length_ms: self.length_ms,
            slots_busy: i64::from(self.slots_busy),
            snr_db: self.snr_db,
        });
    }
}

/// The slots of a frame that the bursts at times `t` (s, oldest first) occupy, counted from the first burst: each burst in the
/// slot nearest to its offset from the first.
pub fn slots_busy(t: impl Iterator<Item = f64> + Clone) -> u8 {
    let Some(first) = t.clone().next() else {
        return 0;
    };
    let mut busy = [false; SLOTS];
    for v in t {
        let s = (((v - first).rem_euclid(FRAME_S) / SLOT_S).round() as usize) % SLOTS;
        busy[s] = true;
    }
    busy.iter().filter(|b| **b).count() as u8
}

/// Whether the power of `x` (at `rate`) has the line of the symbol rate: its component at 10 500 Hz more than 3 times that
/// at 7 300 Hz.
pub fn symbol_line(x: &[Complex32], rate: f64) -> bool {
    let mut a = Complex32::default();
    let mut b = Complex32::default();
    let (wa, wb) = (TAU * SYMBOL_RATE / rate, TAU * COMPARE_HZ / rate);
    for (i, v) in x.iter().enumerate() {
        let p = v.norm_sqr();
        let n = i as f64;
        a += Complex32::from_polar(p, -(wa * n).rem_euclid(TAU) as f32);
        b += Complex32::from_polar(p, -(wb * n).rem_euclid(TAU) as f32);
    }
    a.norm() > 3.0 * b.norm()
}

/// The `avdata.gbas_vdb` decoder.
pub struct GbasDecoder {
    ch: Option<Channelizer>,
    det: Option<BurstDetector>,
    buf: Vec<Complex32>,
    rate: f64,
    input: u64,
    t0: f64,
    recent: VecDeque<f64>,
    last_len_ms: Option<f64>,
    last_snr_db: Option<f64>,
    store: Store,
    started: bool,
    dirty: bool,
    cleared: bool,
    summary: String,
}

impl Default for GbasDecoder {
    fn default() -> Self {
        GbasDecoder {
            ch: None,
            det: None,
            buf: Vec::with_capacity(1 << 15),
            rate: 0.0,
            input: 0,
            t0: 0.0,
            recent: VecDeque::with_capacity(64),
            last_len_ms: None,
            last_snr_db: None,
            store: Store::default(),
            started: false,
            dirty: true,
            cleared: false,
            summary: String::new(),
        }
    }
}

impl GbasDecoder {
    fn now(&self) -> f64 {
        self.input as f64 / self.rate.max(1.0)
    }

    fn build(&mut self, f: &Format) -> Result<(), ConfigError> {
        let mut cfg = ChannelConfig::new(f.rate, f.signal_offset_hz(), PASS_HZ, WORK_RATE);
        cfg.exact_rate = true;
        self.ch = Some(Channelizer::new(cfg).map_err(|_| ConfigError::RateTooLow {
            need: MIN_RATE_HZ,
            got: f.rate,
        })?);
        let mut bc = BurstConfig::new(WINDOW, (BURST_S.1 * WORK_RATE) as usize + WINDOW);
        bc.open_db = 6.02;
        bc.close_db = 6.02;
        bc.pre = 0;
        bc.post = CLOSE_AFTER;
        bc.pool = 16;
        bc.floor_min = FLOOR_MIN;
        // The tenth percentile of 50 windows (50 ms), smoothed: with every slot busy the gaps between the bursts still hold it,
        // and the windows of an open burst are not seen.
        let floor = ChunkPercentile::new(50, 0.1, WINDOW as u32, 0.3).map_err(|_| {
            ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: f.rate,
            }
        })?;
        self.det = Some(
            BurstDetector::new(bc, FloorTracker::ChunkPercentile(floor)).map_err(|_| {
                ConfigError::RateTooLow {
                    need: MIN_RATE_HZ,
                    got: f.rate,
                }
            })?,
        );
        self.t0 = self.now();
        Ok(())
    }

    fn take_bursts(&mut self) {
        let Some(det) = self.det.as_mut() else {
            return;
        };
        while let Some(b) = det.take() {
            let lead = (b.open - b.start) as usize;
            let len = b.samples.len().saturating_sub(lead + CLOSE_AFTER);
            let len_s = len as f64 / WORK_RATE;
            if !(BURST_S.0..=BURST_S.1).contains(&len_s) || b.truncated {
                continue;
            }
            let t = self.t0 + b.open as f64 / WORK_RATE;
            if !self.store.line_seen && symbol_line(&b.samples[lead..lead + len], WORK_RATE) {
                self.store.line_seen = true;
            }
            while self.recent.front().is_some_and(|&r| t - r > SLOT_SPAN_S) {
                self.recent.pop_front();
            }
            self.recent.push_back(t);
            self.store.bursts += 1;
            self.last_len_ms = Some(len_s * 1e3);
            self.last_snr_db = Some(f64::from(b.snr_db));
            if self.store.bursts % LOG_EVERY == 1 {
                let busy = slots_busy(self.recent.iter().copied());
                self.store.log.push(LogRow {
                    t,
                    length_ms: len_s * 1e3,
                    slots_busy: busy,
                    snr_db: f64::from(b.snr_db),
                });
            }
            self.dirty = true;
        }
    }
}

impl Decoder for GbasDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "avdata.gbas_vdb"
    }
    fn outputs(&self) -> Vec<sdrkit_graph::Port> {
        Vec::new()
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Published {
        crate::typed::results(ID)
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.input = 0;
        self.build(f)?;
        Ok(Vec::new())
    }
    fn retune(&mut self, f: &Format) {
        self.recent.clear();
        let _ = self.build(f);
    }
    fn process(&mut self, x: &[Complex32], first: u64, _out: &mut Outbox) {
        self.started = true;
        self.input = first + x.len() as u64;
        if self.ch.is_none() {
            return;
        }
        // Pieces short enough that the pool of finished bursts never fills.
        for piece in x.chunks(16_384) {
            self.buf.clear();
            if let Some(ch) = self.ch.as_mut() {
                ch.process(piece, &mut self.buf);
            }
            if let Some(det) = self.det.as_mut() {
                det.process(&self.buf);
            }
            self.take_bursts();
        }
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.now();
        while self.recent.front().is_some_and(|&r| now - r > SLOT_SPAN_S) {
            self.recent.pop_front();
        }
        let busy = slots_busy(self.recent.iter().copied());
        let s = &self.store;
        if self.cleared {
            self.cleared = false;
            p.results_mut().clear_log();
        }
        self.summary.clear();
        if busy > 0 {
            self.summary.push_str(&format!("{busy} of 8 slots busy"));
            if s.line_seen {
                self.summary.push_str(", symbol rate 10 500 /s seen");
            }
            self.summary.push_str("; content not decoded");
        } else if self.started {
            self.summary.push_str("GBAS VDB detector: no bursts");
        } else {
            self.summary
                .push_str("GBAS VDB detector: waiting for samples");
        }
        p.put(&GbasVdbFields {
            slots_busy: i64::from(busy),
            symbol_rate_seen: s.line_seen,
            bursts: i64::try_from(s.bursts).unwrap_or(i64::MAX),
            length_ms: self.last_len_ms,
            snr_db: self.last_snr_db,
            text: &self.summary,
            content: "content not decoded",
            note: NOTE,
        });
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.store.log, &GBAS_VDB_BURSTS, now, p);
        }
        let lock = match (self.started, busy > 0, self.store.line_seen) {
            (false, ..) => LockState::Off,
            (_, true, true) => LockState::Locked,
            (_, true, false) => LockState::Tracking,
            _ => LockState::Searching,
        };
        let summary = &self.summary;
        p.results_mut().update_status(|st| {
            let changed = st.lock_state != lock || st.summary != *summary;
            if changed {
                st.lock_state = lock;
                st.summary.clone_from(summary);
            }
            changed
        });
    }
    fn report_interval_s(&self) -> f64 {
        0.25
    }
    fn clear(&mut self) {
        self.store.clear();
        self.recent.clear();
        self.last_len_ms = None;
        self.last_snr_db = None;
        self.cleared = true;
        self.dirty = true;
    }
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(GBAS_VDB_BURSTS.key) {
            self.store.log.clear();
            self.dirty = true;
        }
        if !kept.field("bursts") {
            self.store.bursts = 0;
        }
        self.store.line_seen = false;
        self.recent.clear();
        self.last_len_ms = None;
        self.last_snr_db = None;
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

/// The receiver.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("gbas", DecoderComponent::new(GbasDecoder::default()))
        .input("gbas")
        .status_from("gbas")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE_HZ,
            "The GBAS VDB detector needs a sample rate of at least 62.5 kHz",
        )
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("avdata.gbas_vdb", || {
        Box::new(DecoderComponent::new(GbasDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::GbasGenerator::from_config(cfg)?))
}

fn verify_cases(o: &sdrkit_core::verify::VerifyOptions) -> Vec<sdrkit_core::verify::VerifyCase> {
    let mut c = sdrkit_core::verify::VerifyCase::new("test signal", o, 500.0);
    c.seconds = o.seconds.max(3.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| vec![crate::verify::at_least(r, "slots_busy", 3.0, "slots busy")]);
    vec![c]
}

/// The engine's checks of the test signal.
pub static HOOKS: sdrkit_core::verify::VerifyHooks = sdrkit_core::verify::VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The GBAS VDB detector.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "GBAS VDB",
    group: "Aviation",
    blurb: "A detector of the GBAS VHF data broadcast: bursts, the slots busy of the 500 ms frame and the 10 500 symbol/s \
            line. The message content is not decoded.",
    tuning: Tuning::anywhere(TEST_SIGNAL_HZ, 250e3, MIN_RATE_HZ, 25e3),
    submodes: &[],
    settings: gen::GbasSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};
