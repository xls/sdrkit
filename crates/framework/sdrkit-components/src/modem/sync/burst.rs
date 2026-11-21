//! `modem.burst_detect`: bursts above the noise floor, output as pooled sample buffers.

use super::{is_retune, split_at_tags};
use sdrkit_algo_dsp::floor::{ChunkPercentile, FloorTracker, MinFollower};
use sdrkit_algo_modem::burst::{BurstConfig, BurstDetector};
use sdrkit_core::publish::{field, keys, Field, Published};
use sdrkit_core::EventDef;
use sdrkit_core::Record;
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{Complex32, ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, Timed};
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// How [`BurstDetect`] estimates the noise floor.
    pub enum NoiseFloor {
        /// Minimum follower.
        MinFollower = ("min_follower", "Minimum follower"),
        /// Percentile of windows.
        Percentile = ("percentile", "Percentile of windows"),
    }
}

/// Bursts output since the start.
const BURSTS: Field<field::I64> = Field::new("bursts", "Bursts", "");

/// One burst found by `modem.burst_detect`.
///
/// The samples are shared, counted by `Arc`. The detector fills a buffer again only when no reader holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct IqBurst {
    /// The samples, from `pre` before the window that opened the burst to `post` after the one that closed it.
    pub samples: Arc<Vec<Complex32>>,
    /// The stream index of `samples[0]`.
    pub start: u64,
    /// The sample rate of `samples`, Hz.
    pub rate_hz: f64,
    /// The stream index of the first sample of the window that opened it.
    pub open: u64,
    /// Mean power from the opening on over the floor, less one, dB.
    pub snr_db: f32,
    /// The noise floor at the opening, mean power per sample.
    pub floor: f32,
    /// A frequency guess from the lag-one autocorrelation, Hz from 0 Hz in the stream.
    pub freq_hz: f64,
    /// The burst was cut at `max_len`.
    pub truncated: bool,
}

impl Item for IqBurst {
    const KIND: &'static str = "modem.iq_burst";
    fn event_def() -> Option<&'static EventDef> {
        Some(&IQ_BURST)
    }
    fn write_fields(&self, r: &mut Record) {
        r.set(&LEN, self.samples.len() as i64);
        r.set(&START, self.start as i64);
        r.set(&OPENED, self.open as i64);
        r.set(&keys::SNR_DB, f64::from(self.snr_db));
        r.set(&keys::FREQ_HZ, self.freq_hz);
        r.set(&TRUNCATED, self.truncated);
    }
}

const LEN: Field<field::I64> = Field::new("len", "Samples", "");
const START: Field<field::I64> = Field::new("start", "First sample", "");
const OPENED: Field<field::I64> = Field::new("open", "Opening sample", "");
const TRUNCATED: Field<field::Bool> = Field::new("truncated", "Cut at the length limit", "");

static IQ_BURST: EventDef = EventDef::new(
    "modem.iq_burst",
    "Burst",
    &[
        &LEN,
        &START,
        &OPENED,
        &keys::SNR_DB,
        &keys::FREQ_HZ,
        &TRUNCATED,
    ],
);

const THRESHOLD_DB: Setting<field::F64> =
    Setting::new("threshold_db", "Open level", "dB", 10.0).range(0.0, 100.0, 0.0);
const CLOSE_DB: Setting<field::F64> =
    Setting::new("close_db", "Close level", "dB", 6.0).range(-20.0, 100.0, 0.0);
const FLOOR: Setting<field::Choice<NoiseFloor>> =
    Setting::choice("floor", "Noise floor", NoiseFloor::MinFollower);
const WINDOW: Setting<field::I64> =
    Setting::new("window", "Window", "samples", 64).range(1.0, 65536.0, 1.0);
const PRE: Setting<field::I64> =
    Setting::new("pre", "Before", "samples", 64).range(0.0, 16_777_216.0, 1.0);
const POST: Setting<field::I64> =
    Setting::new("post", "After", "samples", 64).range(0.0, 16_777_216.0, 1.0);
const MAX_LEN: Setting<field::I64> =
    Setting::new("max_len", "Longest burst", "samples", 65536).range(1.0, 67_108_864.0, 1.0);
const POOL: Setting<field::I64> = Setting::new("pool", "Buffers", "", 8).range(1.0, 64.0, 1.0);

sdrkit_core::settings! {
    /// The settings of [`BurstDetect`].
    pub enum BurstDetectSetting {
        /// Open level.
        ThresholdDb = THRESHOLD_DB,
        /// Close level.
        CloseDb = CLOSE_DB,
        /// Noise floor.
        Floor = FLOOR,
        /// Window.
        Window = WINDOW,
        /// Before.
        Pre = PRE,
        /// After.
        Post = POST,
        /// Longest burst.
        MaxLen = MAX_LEN,
        /// Buffers.
        Pool = POOL,
    }
}

/// The burst detector (`modem.burst_detect`): `Iq` in, one typed [`IqBurst`] out per burst, at its end.
///
/// Wraps `sdrkit_algo_modem::burst::BurstDetector`: the power is averaged over windows of `window` samples; a noise floor (`floor`:
/// `min_follower`, or `percentile`, the 20th percentile of 64 windows) follows the windows seen while no burst is open; a burst
/// opens when a window reaches `threshold_db` over the floor and closes below `close_db`, keeping `pre` samples before and `post`
/// after (in samples). A burst longer than `max_len` samples is cut there. The item's time is its first sample's.
///
/// Up to `pool` bursts are held by readers at once; when every buffer is still held, a new burst is dropped and counted
/// (`dropped`). A `Retuned` tag forgets a burst in progress (the floor is kept). Results: `bursts`, `dropped`. Settings rebuild the
/// detector.
pub struct BurstDetect {
    threshold_db: f64,
    close_db: f64,
    floor: NoiseFloor,
    window: usize,
    pre: usize,
    post: usize,
    max_len: usize,
    pool: usize,
    rate: f64,
    det: Option<BurstDetector>,
    buffers: Vec<Arc<Vec<Complex32>>>,
    base: u64,
    bursts: u64,
    dropped: u64,
    results: Published,
}

impl Default for BurstDetect {
    /// Windows of 64 samples, open at 10 dB, close below 6 dB, 64 samples before and after, at most 65536 samples, a pool of 8.
    fn default() -> Self {
        BurstDetect {
            threshold_db: THRESHOLD_DB.default,
            close_db: CLOSE_DB.default,
            floor: NoiseFloor::MinFollower,
            window: WINDOW.default as _,
            pre: PRE.default as _,
            post: POST.default as _,
            max_len: MAX_LEN.default as _,
            pool: POOL.default as _,
            rate: 0.0,
            det: None,
            buffers: Vec::new(),
            base: 0,
            bursts: 0,
            dropped: 0,
            results: Published::new(&[&BURSTS, &keys::DROPPED]),
        }
    }
}

impl BurstDetect {
    /// Bursts output since the start.
    pub fn bursts(&self) -> u64 {
        self.bursts
    }
    /// Bursts dropped because every buffer was still held.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Hands on every finished burst.
    fn drain(&mut self, items: &mut Vec<Timed<IqBurst>>) {
        let Some(det) = &mut self.det else { return };
        let before = self.dropped;
        while let Some(b) = det.take() {
            let Some(slot) = self.buffers.iter_mut().find(|a| Arc::strong_count(a) == 1) else {
                self.dropped += 1;
                continue;
            };
            let Some(v) = Arc::get_mut(slot) else {
                self.dropped += 1;
                continue;
            };
            v.clear();
            v.extend_from_slice(b.samples);
            let start = self.base + b.start;
            items.push(Timed {
                time: start as f64 / self.rate,
                value: IqBurst {
                    samples: Arc::clone(slot),
                    start,
                    rate_hz: self.rate,
                    open: self.base + b.open,
                    snr_db: b.snr_db,
                    floor: b.floor,
                    freq_hz: b.freq * self.rate,
                    truncated: b.truncated,
                },
            });
            self.bursts += 1;
        }
        if !items.is_empty() || self.dropped != before {
            self.results.set(&BURSTS, self.bursts as i64);
            self.results.set(&keys::DROPPED, self.dropped as i64);
        }
    }
}

impl Component for BurstDetect {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.burst_detect",
            vec![Port::iq("in")],
            vec![Port::typed::<IqBurst>("out")],
        )
        .manual_tags()
        .with_settings(BurstDetectSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        super::need_rate(f.rate)?;
        if self.close_db > self.threshold_db {
            return Err(SettingError::Conflict {
                id: BurstDetectSetting::CloseDb.id(),
                with: BurstDetectSetting::ThresholdDb.id(),
            }
            .into());
        }
        self.rate = f.rate;
        let windows_per_s = f.rate / self.window as f64;
        let floor = match self.floor {
            NoiseFloor::MinFollower => {
                FloorTracker::MinFollower(MinFollower::with_times(windows_per_s, 0.01, 3.0)?)
            }
            NoiseFloor::Percentile => FloorTracker::ChunkPercentile(ChunkPercentile::new(
                64,
                0.2,
                self.window.min(u32::MAX as usize) as u32,
                0.3,
            )?),
        };
        let mut cfg = BurstConfig::new(self.window, self.max_len);
        cfg.open_db = self.threshold_db as f32;
        cfg.close_db = self.close_db as f32;
        cfg.pre = self.pre;
        cfg.post = self.post;
        cfg.pool = 4;
        self.det = Some(BurstDetector::new(cfg, floor).map_err(ConfigError::from)?);
        let cap = self.pre + self.window + self.max_len + self.post;
        self.buffers = (0..self.pool)
            .map(|_| Arc::new(Vec::with_capacity(cap)))
            .collect();
        self.base = 0;
        self.bursts = 0;
        self.dropped = 0;
        self.results.set(&BURSTS, 0);
        self.results.set(&keys::DROPPED, 0);
        Ok(vec![Format::new(Kind::typed::<IqBurst>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let input = io.input(0);
        if self.det.is_none() {
            return;
        }
        let first = input.first();
        let items = io.items_out::<IqBurst>(0);
        let window = self.window;
        split_at_tags(
            &mut (&mut *self, items),
            input,
            x.len(),
            |(me, items), a, b| {
                // At most one burst can end per window, and the detector holds four: drained window by window, none is lost.
                for c in x[a..b].chunks(window) {
                    if let Some(d) = &mut me.det {
                        d.process(c);
                    }
                    me.drain(items);
                }
            },
            |(me, _), t| {
                if is_retune(t) {
                    if let Some(d) = &mut me.det {
                        d.reset();
                    }
                    me.base = t.at.max(first);
                }
            },
        );
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BurstDetectSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BurstDetectSetting::ThresholdDb => self.threshold_db = THRESHOLD_DB.read(id, v)?,
            BurstDetectSetting::CloseDb => self.close_db = CLOSE_DB.read(id, v)?,
            BurstDetectSetting::Floor => self.floor = FLOOR.read(id, v)?,
            BurstDetectSetting::Window => self.window = WINDOW.read(id, v)? as usize,
            BurstDetectSetting::Pre => self.pre = PRE.read(id, v)? as usize,
            BurstDetectSetting::Post => self.post = POST.read(id, v)? as usize,
            BurstDetectSetting::MaxLen => self.max_len = MAX_LEN.read(id, v)? as usize,
            BurstDetectSetting::Pool => self.pool = POOL.read(id, v)? as usize,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([
            (
                BurstDetectSetting::ThresholdDb,
                Value::F64(self.threshold_db),
            ),
            (BurstDetectSetting::CloseDb, Value::F64(self.close_db)),
            (BurstDetectSetting::Floor, Value::Choice(self.floor.index())),
            (BurstDetectSetting::Window, Value::I64(self.window as i64)),
            (BurstDetectSetting::Pre, Value::I64(self.pre as i64)),
            (BurstDetectSetting::Post, Value::I64(self.post as i64)),
            (BurstDetectSetting::MaxLen, Value::I64(self.max_len as i64)),
            (BurstDetectSetting::Pool, Value::I64(self.pool as i64)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}
