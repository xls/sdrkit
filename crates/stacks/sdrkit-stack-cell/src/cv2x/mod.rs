//! C-V2X sidelink (`cv2x`): structure only. LTE sidelink transmissions (the PC5 interface) of a 10 or 20 MHz channel are
//! found by the SC-FDMA cyclic prefix folded over the 0.5 ms slot and by the empty last symbol of each 1 ms subframe. The stack
//! measures the slot timing, the carrier offset and the share of busy subframes. Nothing is decoded.
//!
//! Standard: 3GPP TS 36.211 clauses 5.6 (SC-FDMA baseband) and 9 (sidelink).

pub mod gen;

use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_modem::ofdm::slots::{SlotDetector, SlotReport};
use sdrkit_algo_modem::ofdm::{FrameShape, Numerology};
use crate::typed::{Cv2xDetectionRow, Cv2xFields, CV2X_DETECTIONS};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Complex32, ConfigError, GenConfig, Generator, LockState, Receiver, ResultsDecl,
    Schema, Setting, SettingError, SettingId, SettingSet, StackConfig, Value, Values,
};
use sdrkit_graph::{Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Outbox, Port, StackGraph};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, Seen};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "cv2x";
/// The frequency of the test signal, Hz. The host gives the frequency of a real channel.
pub const TEST_SIGNAL_HZ: f64 = 5915e6;
/// The least input rate of a 10 MHz channel, Hz: the whole 1024-point symbol.
pub const MIN_RATE_HZ: f64 = 15.36e6;
/// What the status line shows below the least rate.
pub const MIN_RATE_MESSAGE: &str = "C-V2X needs a sample rate of at least 15.36 Msps";
/// What the stack reads.
pub const CONTENT_TEXT: &str =
    "structure only: the safety messages (PSCCH / PSSCH) are not read";
/// What no public document read for this stack confirms.
pub const NOTE: &str = "Structure only: nothing is decoded. Not confirmed by a public document read for this stack: 50 resource blocks in a 10 MHz channel (100 in 20 MHz; the test signal setting gen_resource_blocks), the common timing of all transmitters, and the band of the 5.9 GHz ITS channels (the tuning range is a range check only). The thresholds of the slot fold, the busy subframes and the guard symbol are this stack's own.";

sdrkit_core::choice_enum! {
    /// The channel width.
    pub enum ChannelWidth {
        /// 10 MHz: 15.36 Msps, a 1024-point FFT.
        Mhz10 = ("10", "10 MHz"),
        /// 20 MHz: 30.72 Msps, a 2048-point FFT.
        Mhz20 = ("20", "20 MHz"),
    }
}

impl ChannelWidth {
    /// The working rate, Hz.
    pub fn rate_hz(self) -> f64 {
        match self {
            ChannelWidth::Mhz10 => 15.36e6,
            ChannelWidth::Mhz20 => 30.72e6,
        }
    }
    /// The FFT length.
    pub fn fft_len(self) -> usize {
        match self {
            ChannelWidth::Mhz10 => 1024,
            ChannelWidth::Mhz20 => 2048,
        }
    }
    /// The normal prefixes of the seven symbols of a slot (TS 36.211 table 5.6-1 scaled to the working rate).
    pub fn prefixes(self) -> [usize; 7] {
        let (a, b) = match self {
            ChannelWidth::Mhz10 => (80, 72),
            ChannelWidth::Mhz20 => (160, 144),
        };
        [a, b, b, b, b, b, b]
    }
    /// The numerology: a slot of seven SC-FDMA symbols with the half-subcarrier shift, the 1 ms subframe as the frame period.
    pub fn numerology(self) -> Numerology {
        Numerology::new(self.rate_hz(), self.fft_len(), &self.prefixes(), 1e-3)
            .unwrap_or_else(|_| unreachable!("the LTE numerologies are valid"))
            .with_half_shift()
    }
}

/// The channel width.
pub const CHANNEL_MHZ: Setting<field::Choice<ChannelWidth>> =
    Setting::choice("channel_mhz", "Channel width", ChannelWidth::Mhz10)
        .help("10 MHz (15.36 Msps) or 20 MHz (30.72 Msps).");

sdrkit_core::settings! {
    /// The settings of the detector.
    pub enum Cv2xSetting {
        /// The channel width.
        ChannelMhz = CHANNEL_MHZ,
    }
}

/// The settings of the stack: the detector's and the test signal's.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(crate::typed::shape(ID).schema()))
        .clone()
}

/// The most rows of the detection table.
pub const DETECTION_CAP: usize = 100;

/// One detection start.
#[derive(Clone, Debug, PartialEq)]
pub struct DetectionRow {
    /// Signal time, s.
    pub t: f64,
    /// The carrier offset, Hz.
    pub cfo_hz: f64,
    /// The prefix correlation.
    pub correlation: f64,
    /// The slot timing.
    pub slot_timing: usize,
}

impl Shaped for DetectionRow {
    type Row = Cv2xDetectionRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&Cv2xDetectionRow {
            time_s: self.t,
            offset_hz: Hertz(self.cfo_hz),
            correlation: self.correlation,
            slot_timing: self.slot_timing as i64,
        });
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The text of the frame shape over the 1 ms subframe.
pub fn frame_text(shape: FrameShape) -> String {
    match shape {
        FrameShape::Unknown => String::new(),
        FrameShape::Continuous => "continuous over each 1.0 ms frame".into(),
        FrameShape::Bursts { duty } => {
            format!("bursts: on for {:.0} % of each 1.0 ms frame", 100.0 * duty)
        }
    }
}

/// The busy subframes counted over the newest reports.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct BusyCount {
    frames: f64,
    busy: f64,
    last_db_sum: f64,
    last_n: f64,
}

/// The detector component `cell.cv2x_detect`.
pub struct Cv2xDetect {
    width: ChannelWidth,
    /// The input format, kept so a change of width rebuilds the detector.
    format: Option<Format>,
    nco: Option<Nco>,
    resampler: Option<Resampler<Complex32>>,
    mixed: Vec<Complex32>,
    work: Vec<Complex32>,
    det: Option<SlotDetector>,
    last: Option<SlotReport>,
    last_hit: Option<SlotReport>,
    busy: BusyCount,
    was_detected: bool,
    rows: LogRing<DetectionRow>,
    lines: Vec<String>,
    dirty: bool,
    cleared: bool,
    started: bool,
    summary: String,
}

impl Default for Cv2xDetect {
    fn default() -> Self {
        Cv2xDetect {
            width: ChannelWidth::Mhz10,
            format: None,
            nco: None,
            resampler: None,
            mixed: Vec::new(),
            work: Vec::new(),
            det: None,
            last: None,
            last_hit: None,
            busy: BusyCount::default(),
            was_detected: false,
            rows: LogRing::new(DETECTION_CAP),
            lines: Vec::new(),
            dirty: true,
            cleared: false,
            started: false,
            summary: String::new(),
        }
    }
}

impl Cv2xDetect {
    fn build(&mut self, f: &Format) -> Result<(), ConfigError> {
        let work = self.width.rate_hz();
        if f.rate < work * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: work,
                got: f.rate,
            });
        }
        let off = f.signal_offset_hz();
        self.nco = (off != 0.0).then(|| Nco::new(f.rate, -off));
        self.resampler = if (f.rate - work).abs() > 1e-6 {
            Some(Resampler::new(f.rate, work).map_err(|_| ConfigError::RateTooLow {
                need: work,
                got: f.rate,
            })?)
        } else {
            None
        };
        self.det = Some(SlotDetector::new(self.width.numerology()).map_err(|_| {
            ConfigError::RateTooLow {
                need: work,
                got: f.rate,
            }
        })?);
        Ok(())
    }

    /// The symbol and its prefix, and the normal prefix, in microseconds.
    fn durations(&self) -> (f64, f64) {
        let r = self.width.rate_hz();
        let cp = self.width.prefixes()[1] as f64;
        (
            (self.width.fft_len() as f64 + cp) / r * 1e6,
            cp / r * 1e6,
        )
    }

    fn on_report(&mut self, r: &SlotReport) {
        self.last = Some(*r);
        if r.hit {
            self.last_hit = Some(*r);
            // The counts of the newest reports, the older half forgotten after 2 s.
            if self.busy.frames > 2000.0 {
                self.busy.frames *= 0.5;
                self.busy.busy *= 0.5;
                self.busy.last_db_sum *= 0.5;
                self.busy.last_n *= 0.5;
            }
            self.busy.frames += r.frames as f64;
            self.busy.busy += r.busy as f64;
            if let (Some(db), Some(_)) = (r.last_symbol_db, r.frame_timing) {
                self.busy.last_db_sum += db * r.busy as f64;
                self.busy.last_n += r.busy as f64;
            }
        }
    }
}

impl Decoder for Cv2xDetect {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "cell.cv2x_detect"
    }
    fn outputs(&self) -> Vec<Port> {
        Vec::new()
    }
    fn settings(&self) -> Arc<Schema> {
        Cv2xSetting::schema()
    }
    fn values(&self) -> Values {
        sdrkit_graph::values_of([(Cv2xSetting::ChannelMhz, Value::Choice(self.width.index()))])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match Cv2xSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Cv2xSetting::ChannelMhz => self.width = CHANNEL_MHZ.read(id, v)?,
        }
        if let Some(f) = self.format.clone() {
            // A rate below the new width's leaves the detector off until the next configuration.
            if self.build(&f).is_err() {
                self.det = None;
            }
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.format = Some(f.clone());
        self.build(f)?;
        Ok(Vec::new())
    }
    fn retune(&mut self, f: &Format) {
        self.format = Some(f.clone());
        let _ = self.build(f);
    }
    fn process(&mut self, x: &[Complex32], _first: u64, _out: &mut Outbox) {
        self.started = true;
        let input: &[Complex32] = match self.nco.as_mut() {
            Some(nco) => {
                self.mixed.clear();
                self.mixed.extend_from_slice(x);
                nco.mix_in_place(&mut self.mixed);
                &self.mixed
            }
            None => x,
        };
        let samples: &[Complex32] = match self.resampler.as_mut() {
            Some(r) => {
                self.work.clear();
                r.process(input, &mut self.work);
                &self.work
            }
            None => input,
        };
        let Some(mut det) = self.det.take() else {
            return;
        };
        let mut reports = Vec::new();
        det.process(samples, |r| reports.push(*r));
        self.det = Some(det);
        for r in &reports {
            self.on_report(r);
        }
    }
    fn report(&mut self, p: &mut Published) {
        let detected = self.last.is_some_and(|r| r.detected);
        let (symbol_us, prefix_us) = self.durations();
        let frame = self.last.map(|r| frame_text(r.frame)).unwrap_or_default();
        let now = self.last.map_or(0.0, |r| r.time_s);
        if let (true, false, Some(h)) = (detected, self.was_detected, self.last_hit) {
            self.rows.push(DetectionRow {
                t: now,
                cfo_hz: h.cfo_hz,
                correlation: h.correlation,
                slot_timing: h.slot_timing,
            });
            self.dirty = true;
            self.lines.push(format!(
                "C-V2X: SC-FDMA symbol {symbol_us:.1} us, prefix {prefix_us:.1} us, {:.0} samples/s",
                self.width.rate_hz()
            ));
        }
        if !detected && self.was_detected {
            self.lines.push("C-V2X: the sidelink structure is no longer there".into());
        }
        self.was_detected = detected;
        let hit = self.last_hit.filter(|_| detected);
        let b = self.busy;
        let share = (detected && b.frames > 0.0).then(|| 100.0 * b.busy / b.frames);
        if self.cleared {
            self.cleared = false;
            p.results_mut().clear_log();
        }
        for l in self.lines.drain(..) {
            p.results_mut().log(l);
        }
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.rows, &CV2X_DETECTIONS, now, p);
        }
        self.summary.clear();
        self.summary.push_str(&if !self.started {
            "C-V2X: waiting for samples".to_string()
        } else if detected {
            format!("C-V2X: symbol {symbol_us:.1} µs, CP {prefix_us:.1} µs; structure only")
        } else {
            "searching for C-V2X structure".to_string()
        });
        p.put(&Cv2xFields {
            detected,
            symbol_us,
            prefix_us,
            slot_timing: hit.map(|h| h.slot_timing as i64),
            correlation: hit.map(|h| h.correlation),
            offset_hz: hit.map(|h| Hertz(h.cfo_hz)),
            // A subframe is 1 ms: the share of busy subframes times 1000 is the count per second.
            busy_per_s: share.map(|s| s * 10.0),
            busy_share: share,
            guard_seen: (detected && b.last_n > 0.0).then(|| b.last_db_sum / b.last_n <= -6.0),
            frame: Some(frame.as_str()).filter(|f| !f.is_empty()),
            content: CONTENT_TEXT,
            text: &self.summary,
            note: NOTE,
        });
        let energy = self.last.is_some_and(|r| r.energy);
        let lock = match (self.started, detected, energy) {
            (false, ..) => LockState::Off,
            (_, true, _) => LockState::Locked,
            (_, false, true) => LockState::Tracking,
            _ => LockState::Searching,
        };
        let level = self
            .last
            .filter(|r| r.mean_power > 0.0)
            .map(|r| (10.0 * r.mean_power.log10()) as f32);
        let offset = hit.map(|h| h.cfo_hz);
        let summary = &self.summary;
        p.results_mut().update_status(|st| {
            let changed = st.lock_state != lock
                || st.summary != *summary
                || st.level_dbfs != level
                || st.offset_hz != offset;
            if changed {
                st.lock_state = lock;
                st.summary.clone_from(summary);
                st.level_dbfs = level;
                st.offset_hz = offset;
            }
            changed
        });
    }
    fn report_interval_s(&self) -> f64 {
        0.25
    }
    fn clear(&mut self) {
        self.last = None;
        self.last_hit = None;
        self.busy = BusyCount::default();
        self.was_detected = false;
        self.rows.clear();
        self.dirty = true;
        self.cleared = true;
        if let Some(d) = self.det.as_mut() {
            d.reset();
        }
    }
}

/// The receiver: the detector, the channel width routed to it, the `clear` command.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn build(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("detect", DecoderComponent::new(Cv2xDetect::default()))
        .input("detect")
        .status_from("detect")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE_HZ, MIN_RATE_MESSAGE)
        .route(CHANNEL_MHZ.key(), &["detect.channel_mhz"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Makes the receiver.
///
/// # Errors
///
/// As [`build`].
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(build(cfg)?))
}

/// Makes the test signal of the settings of `cfg`.
///
/// # Errors
///
/// [`ConfigError::RateTooLow`] below the working rate of the channel width.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::Cv2xGenerator::from_config(cfg)?))
}
