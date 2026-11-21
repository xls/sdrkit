//! Iridium (`iridium`): bursts over the whole 1616 to 1626.5 MHz band, demodulated (DE-QPSK, 25 000 symbol/s) and written as RAW
//! text lines.

pub mod gen;
pub mod phy;

use crate::display::{set_series, Clock};
use crate::typed::{
    IridiumFields, IridiumRawEvent, IridiumRawRow, IRIDIUM_RAW, IRIDIUM_RAW_EVENT,
    IRIDIUM_SCATTER_DETECTED, IRIDIUM_SCATTER_UW,
};
use phy::{Burst, BurstDemod, Detector, Found};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Receiver,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kind,
    Outbox, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::shaped::{log_rows, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "iridium";
/// The stack name.
pub const NAME: &str = "Iridium";
/// The lowest sample rate, Hz.
pub const MIN_RATE: f64 = 2e6;
/// Seconds between reports.
pub const REPORT_S: f64 = 0.25;
/// Bursts waiting for their samples at most.
pub const QUEUE: usize = 256;
/// RAW lines kept in the table.
pub const RAW_CAP: usize = 200;
/// Seconds of the burst scatter.
pub const SCATTER_S: f64 = 2.0;

/// What the receiver passes on.
#[derive(Clone, Debug)]
pub enum IrItem {
    /// A demodulated burst.
    Burst {
        burst: Box<Burst>,
        /// Absolute frequency.
        frequency_hz: f64,
        /// Offset from the radio centre.
        offset_hz: f64,
        magnitude_db: f64,
    },
    /// A burst found by the detector.
    Detected { offset_hz: f64 },
    /// The receiver's counts.
    Stats {
        dropped: u64,
        duplicates: u64,
        noise_per_hz: f64,
    },
}
impl Item for IrItem {
    const KIND: &'static str = "satcom.iridium_item";
}

/// The receiver: the burst detector, a queue of bursts waiting for their samples, and the per-burst demodulator.
pub struct IrRx {
    rate: f64,
    centre: f64,
    live: bool,
    det: Option<Detector>,
    demod: Option<BurstDemod>,
    ring: VecDeque<Complex32>,
    ring_first: u64,
    ring_cap: usize,
    found: Vec<Found>,
    queue: VecDeque<Found>,
    burst: Vec<Complex32>,
    dropped: u64,
    seen: u64,
    threshold: f64,
}

impl Default for IrRx {
    fn default() -> Self {
        IrRx {
            rate: 0.0,
            centre: 0.0,
            live: false,
            det: None,
            demod: None,
            ring: VecDeque::new(),
            ring_first: 0,
            ring_cap: 0,
            found: Vec::new(),
            queue: VecDeque::new(),
            burst: Vec::new(),
            dropped: 0,
            seen: 0,
            threshold: THRESHOLD_DB.default,
        }
    }
}

/// The burst level over the noise floor that a burst needs to be demodulated.
pub const THRESHOLD_DB: Setting<field::F64> =
    Setting::new("threshold_db", "Burst threshold", "dB", 13.0)
        .range(8.0, 24.0, 0.5)
        .help("Burst level over the noise floor needed to demodulate a burst.");

sdrkit_core::settings! {
    /// The settings of [`IrRx`].
    pub enum IrRxSetting {
        /// Burst threshold.
        ThresholdDb = THRESHOLD_DB,
    }
}

impl Decoder for IrRx {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.iridium_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<IrItem>("out").rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        IrRxSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match IrRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            // The threshold changes without losing the noise floor or the bursts in progress.
            IrRxSetting::ThresholdDb => {
                self.threshold = THRESHOLD_DB.read(id, v)?;
                if let Some(d) = &mut self.det {
                    d.threshold_db = self.threshold;
                }
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([(IrRxSetting::ThresholdDb, Value::F64(self.threshold))])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate.is_nan() || f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        let mut d = Detector::new(f.rate)?;
        d.threshold_db = self.threshold;
        self.det = Some(d);
        self.demod = Some(BurstDemod::new(f.rate)?);
        self.rate = f.rate;
        self.centre = f.centre_hz;
        self.ring_cap = (0.1 * f.rate) as usize;
        self.ring = VecDeque::with_capacity(self.ring_cap + 1);
        self.ring_first = 0;
        self.queue.clear();
        self.burst = Vec::with_capacity(self.ring_cap);
        self.seen = 0;
        Ok(vec![Format::new(Kind::typed::<IrItem>(), 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        let _ = self.configure(f);
    }
    fn process(&mut self, iq: &[Complex32], first: u64, out: &mut Outbox) {
        let out = out.items::<IrItem>(0);
        if self.ring.is_empty() {
            self.ring_first = first;
        }
        self.seen = first + iq.len() as u64;
        self.ring.extend(iq.iter().copied());
        while self.ring.len() > self.ring_cap {
            self.ring.pop_front();
            self.ring_first += 1;
        }
        self.found.clear();
        let Some(det) = self.det.as_mut() else { return };
        det.process(iq, first, &mut self.found);
        for f in self.found.drain(..) {
            out.push(Timed {
                time: f.start as f64 / self.rate,
                value: IrItem::Detected {
                    offset_hz: f.centre,
                },
            });
            if self.queue.len() >= QUEUE && self.live {
                // A live radio never waits: the burst is dropped and counted.
                self.dropped += 1;
                continue;
            }
            self.queue.push_back(f);
        }
        // Every queued burst whose samples are in the ring.
        let end = first + iq.len() as u64;
        while let Some(f) = self.queue.front() {
            if f.end > end {
                break;
            }
            let Some(f) = self.queue.pop_front() else {
                break;
            };
            if f.start < self.ring_first {
                self.dropped += 1;
                continue;
            }
            let a = (f.start - self.ring_first) as usize;
            let b = ((f.end - self.ring_first) as usize).min(self.ring.len());
            self.burst.clear();
            self.burst.extend(self.ring.range(a..b).copied());
            let Some(demod) = self.demod.as_mut() else {
                break;
            };
            let burst = demod.demodulate(&self.burst, f.centre, f.start);
            let t = burst.uw_at / self.rate;
            out.push(Timed {
                time: t,
                value: IrItem::Burst {
                    frequency_hz: self.centre + burst.centre,
                    offset_hz: burst.centre,
                    magnitude_db: f.peak_db,
                    burst: Box::new(burst),
                },
            });
        }
    }
    fn tick(&mut self, out: &mut Outbox) {
        let Some(d) = self.det.as_ref() else { return };
        out.items::<IrItem>(0).push(Timed {
            time: self.seen as f64 / self.rate.max(1.0),
            value: IrItem::Stats {
                dropped: self.dropped,
                duplicates: d.duplicates,
                noise_per_hz: d.noise_per_hz,
            },
        });
    }
}

/// One RAW text line: the file time, the milliseconds, the frequency, the magnitude and noise, an
/// id, the confidence, the level, the data symbols and the bits.
pub fn raw_line(
    file_unix: u64,
    ms: f64,
    freq: f64,
    mag_db: f64,
    noise_db: f64,
    id: u64,
    b: &Burst,
) -> String {
    let bits: String = b
        .raw_bits
        .iter()
        .map(|&x| if x { '1' } else { '0' })
        .collect();
    format!(
        "RAW: i-{file_unix}-t1 {ms:012.4} {:010} N:{mag_db:05.2}{noise_db:+06.2} I:{id:011} {:3}% {:.5} {:3} {bits}",
        freq.round() as i64,
        b.confidence,
        10f64.powf(b.level_db / 20.0),
        b.data_symbols
    )
}

// The results.

sdrkit_core::choice_enum! {
    /// The direction of a burst, from its unique word.
    pub enum Direction {
        /// From the satellite.
        Downlink = ("downlink", "Downlink"),
        /// To the satellite.
        Uplink = ("uplink", "Uplink"),
    }
}

/// What the display publishes.
fn display_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        Arc::new(
            ResultsDecl::new()
                .with_fields(IridiumFields::SHAPE.fields)
                .with_table(&IRIDIUM_RAW)
                .with_series(&IRIDIUM_SCATTER_DETECTED)
                .with_series(&IRIDIUM_SCATTER_UW),
        )
    })
    .clone()
}

/// A RAW line kept.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RawLine {
    /// Signal time, seconds.
    pub time: f64,
    /// Frequency, Hz.
    pub frequency_hz: f64,
    /// The burst had the downlink unique word.
    pub downlink: bool,
    /// Confidence, %.
    pub confidence: u32,
    /// The line.
    pub line: String,
}

impl Shift for RawLine {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for RawLine {
    type Row = IridiumRawRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&IridiumRawRow {
            at: Seconds(self.time),
            frequency: Hertz(self.frequency_hz),
            direction: if self.downlink {
                Direction::Downlink
            } else {
                Direction::Uplink
            },
            confidence_pct: f64::from(self.confidence),
            line: &self.line,
        });
    }
}

/// The counts and the RAW lines (saved state).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IrTables {
    /// Bursts found.
    pub bursts: u64,
    /// Bursts demodulated.
    pub demodulated: u64,
    /// Bursts with a unique word.
    pub uw_ok: u64,
    /// Downlink bursts.
    pub downlink: u64,
    /// Uplink bursts.
    pub uplink: u64,
    /// The RAW lines.
    pub raw: LogRing<RawLine>,
    /// The next RAW line id.
    pub next_id: u64,
}

impl Default for IrTables {
    fn default() -> Self {
        IrTables {
            bursts: 0,
            demodulated: 0,
            uw_ok: 0,
            downlink: 0,
            uplink: 0,
            raw: LogRing::new(RAW_CAP),
            next_id: 0,
        }
    }
}

impl Shift for IrTables {
    fn shift_times(&mut self, by_s: f64) {
        self.raw.shift_times(by_s);
    }
}

impl Tables for IrTables {
    const FORMAT: sdrkit_util_objects::Format =
        sdrkit_util_objects::Format::new("satcom.iridium", 1);
    fn clear(&mut self) {
        *self = IrTables {
            raw: LogRing::new(self.raw.cap()),
            next_id: self.next_id,
            ..Default::default()
        };
    }
    fn adopt(&mut self, saved: Self) {
        self.bursts = saved.bursts;
        self.demodulated = saved.demodulated;
        self.uw_ok = saved.uw_ok;
        self.downlink = saved.downlink;
        self.uplink = saved.uplink;
        self.raw.adopt(saved.raw);
        self.next_id = saved.next_id;
    }
}

/// Publish each burst as a RAW line.
pub const RAW_OUTPUT: Setting<field::Bool> = Setting::new("raw_output", "RAW output", "", false)
    .help("Publish each burst as a RAW line (events iridium.raw and the table raw). The host writes them where it wants.");

sdrkit_core::settings! {
    /// The settings of [`IrDisplay`].
    pub enum IrDisplaySetting {
        /// RAW output.
        RawOutput = RAW_OUTPUT,
    }
}

/// The display: counters, the burst scatter, the RAW lines.
pub struct IrDisplay {
    clock: Clock,
    live: bool,
    host_clock: sdrkit_core::Clock,
    tables: IrTables,
    dropped: u64,
    duplicates: u64,
    noise_per_hz: f64,
    last_report: (u64, u64),
    burst_rate: f64,
    uw_rate: f64,
    conf_sum: f64,
    esn0_sum: f64,
    n_rep: u64,
    confidence: Option<f64>,
    esn0_db: Option<f64>,
    last_offset_hz: Option<f64>,
    scatter: VecDeque<(f64, f64, bool)>,
    raw_on: bool,
    /// The wall-clock second the RAW lines count from (0 for a recording).
    file_unix: Option<u64>,
    /// The tables changed since the last report.
    dirty: bool,
}

impl Default for IrDisplay {
    fn default() -> Self {
        IrDisplay {
            clock: Clock::default(),
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            tables: IrTables::default(),
            dropped: 0,
            duplicates: 0,
            noise_per_hz: 0.0,
            last_report: (0, 0),
            burst_rate: 0.0,
            uw_rate: 0.0,
            conf_sum: 0.0,
            esn0_sum: 0.0,
            n_rep: 0,
            confidence: None,
            esn0_db: None,
            last_offset_hz: None,
            scatter: VecDeque::with_capacity(2048),
            raw_on: false,
            file_unix: None,
            dirty: true,
        }
    }
}

impl IrDisplay {
    fn burst(
        &mut self,
        t: f64,
        burst: &Burst,
        frequency_hz: f64,
        offset_hz: f64,
        magnitude_db: f64,
        events: &mut Vec<sdrkit_core::Event>,
    ) {
        let d = &mut self.tables;
        d.demodulated += 1;
        self.dirty = true;
        if !burst.uw_ok {
            return;
        }
        d.uw_ok += 1;
        if burst.downlink {
            d.downlink += 1;
        } else {
            d.uplink += 1;
        }
        self.conf_sum += burst.confidence as f64;
        self.esn0_sum += burst.esn0_db;
        self.n_rep += 1;
        self.last_offset_hz = Some(frequency_hz - phy::channel_hz(phy::channel_of(frequency_hz)));
        self.scatter.push_back((t, offset_hz, true));
        if !self.raw_on {
            return;
        }
        // Time stamps: the wall clock of the start (0 for a recording) plus the signal time.
        let wall0 = self.clock.wall(0.0).unwrap_or(0.0);
        let base = *self.file_unix.get_or_insert(wall0.floor() as u64);
        let ms = (wall0 - base as f64 + t) * 1e3;
        let noise_db = 10.0 * self.noise_per_hz.max(1e-30).log10();
        let line = raw_line(
            base,
            ms,
            frequency_hz,
            magnitude_db,
            noise_db.max(-99.0),
            self.tables.next_id,
            burst,
        );
        self.tables.next_id += 1;
        events.push(IRIDIUM_RAW_EVENT.event(
            t,
            frequency_hz,
            &IridiumRawEvent {
                text: &line,
                snr_db: Some(burst.esn0_db).filter(|s| s.is_finite()),
                family: Family::Satellite,
                confidence_pct: f64::from(burst.confidence),
            },
        ));
        self.tables.raw.push(RawLine {
            time: t,
            frequency_hz,
            downlink: burst.downlink,
            confidence: burst.confidence,
            line,
        });
    }

    /// The rates and means of the last clock period.
    fn period(&mut self) {
        let d = &self.tables;
        let (b0, u0) = self.last_report;
        self.burst_rate = (d.bursts.saturating_sub(b0)) as f64 / REPORT_S;
        self.uw_rate = (d.uw_ok.saturating_sub(u0)) as f64 / REPORT_S;
        self.last_report = (d.bursts, d.uw_ok);
        if self.n_rep > 0 {
            self.confidence = Some(self.conf_sum / self.n_rep as f64);
            self.esn0_db = Some(self.esn0_sum / self.n_rep as f64);
        }
        self.conf_sum = 0.0;
        self.esn0_sum = 0.0;
        self.n_rep = 0;
    }

    fn publish(&mut self, p: &mut Published) {
        let now = self.clock.now();
        let d = &self.tables;
        // Detections alone happen in noise: only unique words light the lock state.
        let locked = self.uw_rate > 0.0;
        let lock_state = if locked {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let offset = self.last_offset_hz;
        let snr = if locked {
            self.esn0_db.map(|v| v as f32)
        } else {
            None
        };
        let (burst_rate, uw_rate) = (self.burst_rate, self.uw_rate);
        p.results_mut().update_status(|st| {
            let same = st.lock_state == lock_state && st.offset_hz == offset && st.snr_db == snr;
            if same && !locked && st.summary == "Iridium: searching" {
                return false;
            }
            st.lock_state = lock_state;
            st.offset_hz = offset;
            st.snr_db = snr;
            st.summary.clear();
            if locked {
                use std::fmt::Write;
                let _ = write!(
                    st.summary,
                    "Iridium: {burst_rate:.0} bursts/s, {uw_rate:.0} unique words/s"
                );
            } else {
                st.summary.push_str("Iridium: searching");
            }
            true
        });
        p.put(&IridiumFields {
            bursts: d.bursts as i64,
            demodulated: d.demodulated as i64,
            frames_ok: d.uw_ok as i64,
            frames_bad: (d.demodulated - d.uw_ok) as i64,
            dropped: self.dropped as i64,
            duplicates: self.duplicates as i64,
            downlink: d.downlink as i64,
            uplink: d.uplink as i64,
            burst_rate: Hertz(self.burst_rate),
            uw_rate: Hertz(self.uw_rate),
            confidence_pct: self.confidence,
            esn0_db: self.esn0_db,
            noise_db_hz: (self.noise_per_hz > 0.0).then(|| 10.0 * self.noise_per_hz.log10()),
        });
        if std::mem::take(&mut self.dirty) {
            log_rows(&self.tables.raw, &IRIDIUM_RAW, now, p);
        }
        for (key, uw) in [
            (IRIDIUM_SCATTER_DETECTED.key, false),
            (IRIDIUM_SCATTER_UW.key, true),
        ] {
            let points = || {
                self.scatter
                    .iter()
                    .filter(move |s| s.2 == uw)
                    .map(move |s| [(now - s.0) as f32, s.1 as f32])
            };
            set_series(p.results_mut(), key, false, points);
        }
    }
}

impl Decoder for IrDisplay {
    type Input = Timed<IrItem>;
    fn name(&self) -> &'static str {
        "satcom.iridium_display"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        IrDisplaySetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match IrDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            IrDisplaySetting::RawOutput => self.raw_on = RAW_OUTPUT.read(id, v)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([(IrDisplaySetting::RawOutput, Value::Bool(self.raw_on))])
    }
    fn results(&self) -> Published {
        Published::from_decl(display_decl())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn configure(&mut self, _f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(self.live, self.host_clock.now());
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Timed<IrItem>], _first: u64, out: &mut Outbox) {
        let events = out.messages(0);
        for it in x {
            let t = self.clock.at(it.time);
            match &it.value {
                IrItem::Detected { offset_hz } => {
                    self.tables.bursts += 1;
                    self.scatter.push_back((t, *offset_hz, false));
                }
                IrItem::Burst {
                    burst,
                    frequency_hz,
                    offset_hz,
                    magnitude_db,
                } => self.burst(t, burst, *frequency_hz, *offset_hz, *magnitude_db, events),
                IrItem::Stats {
                    dropped,
                    duplicates,
                    noise_per_hz,
                } => {
                    self.dropped = *dropped;
                    self.duplicates = *duplicates;
                    self.noise_per_hz = *noise_per_hz;
                }
            }
            while self.scatter.front().is_some_and(|s| t - s.0 > SCATTER_S)
                || self.scatter.len() > 2000
            {
                self.scatter.pop_front();
            }
        }
    }
    fn tick(&mut self, _out: &mut Outbox) {
        self.period();
    }
    fn report(&mut self, results: &mut Published) {
        self.publish(results);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.tables.clear();
        self.last_report = (0, 0);
        self.scatter.clear();
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.tables.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.tables.restore_state(state, self.clock.now())?;
        self.last_report = (self.tables.bursts, self.tables.uw_ok);
        self.dirty = true;
        Ok(())
    }
}

sdrkit_core::settings! {
    /// The settings of the Iridium stack. Noise and other impairments of the test signal come from the standard `gen_*`
    /// settings.
    pub enum IridiumSetting {
        /// Burst threshold.
        ThresholdDb = THRESHOLD_DB,
        /// RAW output.
        RawOutput = RAW_OUTPUT,
        /// Doppler of the test signal.
        GenBurstDoppler = gen::GEN_BURST_DOPPLER,
        /// Bursts per frame of the test signal.
        GenBursts = gen::GEN_BURSTS,
        /// Radio centre of the test signal.
        GenCentreHz = gen::GEN_CENTRE_HZ,
    }
}

/// Adds the stack's components to `r`.
pub fn register(r: &mut Registry) {
    r.add("satcom.iridium_receiver", || {
        Box::new(DecoderComponent::new(IrRx::default()))
    });
    r.add("satcom.iridium_display", || {
        Box::new(DecoderComponent::new(IrDisplay::default()))
    });
}

/// The receiver.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = GraphBuilder::new()
        .add("receiver", DecoderComponent::new(IrRx::default()))
        .add("display", DecoderComponent::new(IrDisplay::default()))
        .input("receiver.in")
        .connect("receiver.out", "display.in")
        .output("display.events")
        .status_from("display")
        .clock(REPORT_S)
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(
            MIN_RATE,
            "Iridium needs a sample rate of at least 2000 ksps",
        )
        .default_centre(DESCRIPTOR.tuning.default_hz)
        .route("threshold_db", &["receiver.threshold_db"])?
        .route("raw_output", &["display.raw_output"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::IrGenerator::from_config(cfg)))
}

/// The verification of the stack: the test bursts must give unique words.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, bursts across the band", o, 0.0);
    c.seconds = o.seconds.max(1.0);
    c.expect_audio = Some(false);
    c.check = Some(check_bursts);
    vec![c]
}

fn check_bursts(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let n = crate::field(r, "frames_ok") as i64;
    vec![CheckLine::new(
        "frames_ok",
        n > 10,
        format!("{n} bursts with a unique word (want more than 10)"),
    )]
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "Iridium bursts over the whole band, demodulated and written as RAW text lines. Receive only.",
    tuning: Tuning {
        min_hz: 1610e6,
        max_hz: 1630e6,
        default_hz: 1622e6,
        rate_hz: 10e6,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 10.5e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(9e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: IridiumSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Iridium],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};
