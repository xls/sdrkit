//! Meteosat HRDCP (`hrdcp`): high-rate data collection platforms. Each sends one message in its slot: a 2 s carrier, a BPSK
//! preamble and marker, then offset QPSK at 1200 bit/s with the K = 7 code, the randomiser, interleaved Reed-Solomon and a
//! CRC-32. Every carrier in the capture is followed. Channel numbers come from a plan file that the host names.
//!
//! Standard: EUMETSAT TD 16, Meteosat Data Collection and Distribution Service, sections 3.2 to 3.4 and 5.3, tables 4 and 11.

pub mod gen;
pub mod message;
pub mod rx;

use crate::display::{set_series, set_status, Clock};
use crate::modes::goes::body::{ascii_fields, format_of, DataFormat};
use crate::modes::goes::rx::Scanner;
use crate::typed::{
    HrdcpCarrierRow, HrdcpFields, HrdcpPlatformRow, HrdcpReportEvent, HrdcpReportRow,
    HRDCP_CARRIERS, HRDCP_PLATFORMS, HRDCP_REPORT, HRDCP_REPORTS, HRDCP_SPECTRUM,
};
use rx::{Follower, Outcome, Phase};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Receiver,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, SignalType, StackConfig,
    StackDescriptor, StateBlob, StateError, Storage, StorePath, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kind,
    Outbox, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "hrdcp";
/// The stack name.
pub const NAME: &str = "Meteosat HRDCP";
/// The least input rate.
pub const MIN_RATE: f64 = 48e3;
/// Seconds between two reports.
pub const REPORT_S: f64 = 0.25;
/// The most carriers followed at once.
pub const MAX_FOLLOWERS: usize = 6;
/// Input kept to replay into a new follower, s.
pub const REPLAY_S: f64 = 0.6;
/// The most messages the report log keeps.
pub const REPORT_CAP: usize = 300;
/// The most platforms the platform table keeps.
pub const PLATFORM_CAP: usize = 300;
/// Points of the band line.
pub const LINE_POINTS: usize = 256;
/// The dial of the test signal. It is not a channel of a regional plan: the host gives the frequency.
pub const TEST_SIGNAL_HZ: f64 = 402.3e6;
/// What the stack says about its sources.
pub const NOTE_TEXT: &str = "experimental: not confirmed on a real signal. The message format and the coding follow \
     EUMETSAT TD 16 and are not yet checked against it; the loop gains of the demodulator, the marker threshold and the \
     follower timing are not confirmed by a public document.";

/// The carrier line over the local noise floor that a carrier needs.
pub const THRESHOLD_DB: Setting<field::F64> =
    Setting::new("threshold_db", "Carrier threshold", "dB", 12.0)
        .range(6.0, 25.0, 0.5)
        .help("A carrier line must stand this far over the local noise floor.");
/// The channel plan: a plan file in the host's storage.
pub const CHANNEL_PLAN: Setting<field::Text> =
    Setting::new("channel_plan", "Channel plan", "", "").help(
        "The path of a plan file in the host's storage whose numbered bands give the channel numbers (docs/schema/plans.json). Empty for none.",
    );

sdrkit_core::settings! {
    /// The settings of the stack.
    pub enum HrdcpSetting {
        /// Carrier threshold.
        ThresholdDb = THRESHOLD_DB,
        /// Channel plan.
        ChannelPlan = CHANNEL_PLAN,
        /// Scene of the test signal.
        GenScene = gen::GEN_SCENE,
    }
}

sdrkit_core::settings! {
    /// The settings of [`HrdcpRx`].
    pub enum HrdcpRxSetting {
        /// Carrier threshold.
        ThresholdDb = THRESHOLD_DB,
    }
}

sdrkit_core::settings! {
    /// The settings of [`HrdcpDisplay`].
    pub enum HrdcpDisplaySetting {
        /// Channel plan.
        ChannelPlan = CHANNEL_PLAN,
    }
}

/// A carrier followed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CarrierRow {
    /// Hz from the input's 0 Hz.
    pub offset_hz: f64,
    /// The line over the floor when found, dB.
    pub snr_db: f64,
    /// 0 looking for the marker, 2 decoding.
    pub state: u8,
    /// Bytes decoded so far.
    pub bytes: usize,
}

/// A message received.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Received {
    /// The decoded message.
    pub msg: message::Decoded,
    /// The carrier, Hz from the input's 0 Hz.
    pub offset_hz: f64,
    /// The carrier line over the floor, dB.
    pub snr_db: f64,
}

/// What the receiver passes on.
#[derive(Clone, Debug)]
pub enum HrdcpItem {
    /// A message whose header decoded.
    Message(Box<Received>),
    /// A message whose header did not decode.
    Bad,
    /// At each tick: the carriers followed and the band line.
    Status {
        /// The carriers followed.
        carriers: Vec<CarrierRow>,
        /// The spectrum in dB over the floor across the input.
        line: Vec<f32>,
        /// The width the line spans, Hz.
        span_hz: f64,
    },
}

impl Item for HrdcpItem {
    const KIND: &'static str = "satcom.hrdcp_item";
}

/// The receiver: the scanner and a follower per carrier.
pub struct HrdcpRx {
    rate: f64,
    thr_db: f64,
    scanner: Option<Scanner>,
    followers: Vec<Follower>,
    ring: VecDeque<Complex32>,
    ring_cap: usize,
    next_eval: u64,
    next_index: u64,
    /// Lines to ignore, with the signal time until which they are ignored.
    ignored: Vec<(f64, f64)>,
    line: Vec<f32>,
    pending: Vec<Timed<HrdcpItem>>,
}

impl Default for HrdcpRx {
    fn default() -> Self {
        HrdcpRx {
            rate: 0.0,
            thr_db: THRESHOLD_DB.default,
            scanner: None,
            followers: Vec::new(),
            ring: VecDeque::new(),
            ring_cap: 0,
            next_eval: 0,
            next_index: 0,
            ignored: Vec::new(),
            line: Vec::new(),
            pending: Vec::new(),
        }
    }
}

impl HrdcpRx {
    fn eval(&mut self, now: f64) {
        let Some(sc) = self.scanner.as_mut() else {
            return;
        };
        let (lines, spec) = sc.evaluate(self.thr_db);
        let n = spec.len();
        self.line.clear();
        self.line.extend(
            (0..LINE_POINTS)
                .map(|c| spec[(c * n / LINE_POINTS + n / (2 * LINE_POINTS)).min(n - 1)]),
        );
        self.ignored.retain(|i| i.1 > now);
        // The strongest lines first. When every follower is busy, a line 6 dB stronger than the weakest follower still
        // looking for its marker takes that follower's place (a clean signal shows weak spurious lines beside its carriers).
        let mut lines = lines;
        lines.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (off, snr) in lines {
            if self.ignored.iter().any(|i| (i.0 - off).abs() < 300.0)
                || self
                    .followers
                    .iter()
                    .any(|f| (f.offset - off).abs() < 1200.0)
            {
                continue;
            }
            if self.followers.len() >= MAX_FOLLOWERS {
                let weakest = self
                    .followers
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.phase == Phase::Marker)
                    .min_by(|a, b| a.1.snr_db.total_cmp(&b.1.snr_db))
                    .map(|(i, f)| (i, f.snr_db));
                match weakest {
                    Some((i, w)) if snr > w + 6.0 => {
                        self.followers.swap_remove(i);
                    }
                    _ => continue,
                }
            }
            let start = now - self.ring.len() as f64 / self.rate;
            let Ok(mut f) = Follower::new(self.rate, off, start, snr) else {
                continue;
            };
            // Replay what came before the line was confirmed.
            let (a, b) = self.ring.as_slices();
            f.process(a);
            f.process(b);
            self.followers.push(f);
        }
        self.collect(now);
    }

    /// Hands on the outcomes of the followers that ended and frees them.
    fn collect(&mut self, now: f64) {
        let mut keep = Vec::with_capacity(self.followers.len());
        for mut f in self.followers.drain(..) {
            match f.outcome.take() {
                None => keep.push(f),
                Some(Outcome::Message(m)) => self.pending.push(Timed {
                    time: now,
                    value: HrdcpItem::Message(Box::new(Received {
                        msg: *m,
                        offset_hz: f.offset,
                        snr_db: f.snr_db,
                    })),
                }),
                Some(o) => {
                    // A failed follower ignores its frequency for 1.5 s, so the rest of its carrier opens no new one.
                    self.ignored.push((f.offset, now + 1.5));
                    if o == Outcome::BadHeader {
                        self.pending.push(Timed {
                            time: now,
                            value: HrdcpItem::Bad,
                        });
                    }
                }
            }
        }
        self.followers = keep;
    }
}

impl Decoder for HrdcpRx {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.hrdcp_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<HrdcpItem>("out").rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        HrdcpRxSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match HrdcpRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            HrdcpRxSetting::ThresholdDb => self.thr_db = THRESHOLD_DB.read(id, v)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([(HrdcpRxSetting::ThresholdDb, Value::F64(self.thr_db))])
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate.is_nan() || f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.scanner = Some(Scanner::new(f.rate));
        self.followers.clear();
        self.ring.clear();
        self.ring_cap = (REPLAY_S * f.rate) as usize;
        self.ring.reserve(self.ring_cap + 1);
        self.next_eval = (0.25 * f.rate) as u64;
        self.next_index = 0;
        let mut o = Format::new(Kind::typed::<HrdcpItem>(), 0.0);
        o.centre_hz = f.centre_hz;
        o.tuned_hz = f.tuned_hz;
        Ok(vec![o])
    }
    fn retune(&mut self, _f: &Format) {
        // The carriers belong to the old centre: start the search again.
        self.scanner = Some(Scanner::new(self.rate));
        self.followers.clear();
        self.ring.clear();
        self.ignored.clear();
    }
    fn process(&mut self, iq: &[Complex32], first: u64, out: &mut Outbox) {
        self.next_index = first + iq.len() as u64;
        let step = ((0.25 * self.rate) as u64).max(1);
        let mut done = 0usize;
        while done < iq.len() {
            let at = first + done as u64;
            let n = (self.next_eval.saturating_sub(at) as usize).clamp(1, iq.len() - done);
            let piece = &iq[done..done + n];
            if let Some(sc) = self.scanner.as_mut() {
                sc.push(piece, at);
            }
            for f in self.followers.iter_mut() {
                f.process(piece);
            }
            self.ring.extend(piece.iter().copied());
            while self.ring.len() > self.ring_cap {
                self.ring.pop_front();
            }
            done += n;
            if first + done as u64 >= self.next_eval {
                self.next_eval += step;
                self.eval((first + done as u64) as f64 / self.rate);
            }
        }
        out.items::<HrdcpItem>(0).append(&mut self.pending);
    }
    fn tick(&mut self, out: &mut Outbox) {
        let carriers = self
            .followers
            .iter()
            .map(|f| CarrierRow {
                offset_hz: f.offset,
                snr_db: f.snr_db,
                state: if f.phase == Phase::Data { 2 } else { 0 },
                bytes: f.bytes_so_far(),
            })
            .collect();
        out.items::<HrdcpItem>(0).push(Timed {
            time: self.next_index as f64 / self.rate.max(1.0),
            value: HrdcpItem::Status {
                carriers,
                line: self.line.clone(),
                span_hz: self.rate,
            },
        });
    }
}

// ---------------------------------------------------------------- the channel plan

/// The numbered channels of a plan: (first, last, centre of the first, step) for each numbered band.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelPlan {
    /// The name of the plan.
    pub name: String,
    /// The numbered bands, in file order.
    pub ranges: Vec<(u32, u32, f64, f64)>,
}

impl ChannelPlan {
    /// The plan of a plan file's text: the numbered bands of its first group.
    ///
    /// # Errors
    ///
    /// The plan registry's error when the text is not a plan file or holds no group.
    pub fn parse(text: &str) -> Result<ChannelPlan, sdrkit_data_plans::PlanError> {
        let mut plans = sdrkit_data_plans::Plans::new();
        let ids = sdrkit_data_plans::api::import_as(
            &mut plans,
            text,
            sdrkit_data_plans::api::PlanFormat::Auto,
            "",
            "",
        )?;
        let g = ids
            .first()
            .and_then(|id| plans.group(id))
            .ok_or_else(|| sdrkit_data_plans::PlanError::Invalid(vec!["no plan".into()]))?;
        Ok(ChannelPlan {
            name: g.group.name.clone(),
            ranges: g
                .group
                .bands
                .iter()
                .filter_map(|b| b.numbering.as_ref())
                .map(|n| (n.first, n.last, n.first_centre_hz + n.offset_hz, n.step_hz))
                .collect(),
        })
    }

    /// The channel of a frequency: the nearest channel of the first numbered band that holds it within half a step.
    pub fn channel_of(&self, hz: f64) -> Option<u32> {
        self.ranges.iter().find_map(|&(first, last, f0, step)| {
            let k = ((hz - f0) / step).round();
            let n = first as f64 + k;
            (k >= 0.0 && n <= last as f64).then_some(n as u32)
        })
    }
}

// ---------------------------------------------------------------- the display

fn display_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        Arc::new(
            ResultsDecl::new()
                .with_fields(HrdcpFields::SHAPE.fields)
                .with_table(&HRDCP_REPORTS)
                .with_table(&HRDCP_PLATFORMS)
                .with_table(&HRDCP_CARRIERS)
                .series(
                    HRDCP_SPECTRUM.key,
                    HRDCP_SPECTRUM.label,
                    HRDCP_SPECTRUM.x_unit,
                    HRDCP_SPECTRUM.y_unit,
                ),
        )
    })
    .clone()
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// One message of the log.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReportRow {
    /// When it was received.
    pub at: Stamp,
    /// The address, 8 hex digits.
    pub address: String,
    /// Address bits corrected.
    pub corrected: usize,
    /// The sequence counter.
    pub sequence: u16,
    /// An alert message.
    pub alert: bool,
    /// The health bits.
    pub health: u16,
    /// The compression field.
    pub compression: u8,
    /// The CRC passed.
    pub crc_ok: bool,
    /// Coded frames.
    pub frames: usize,
    /// Frames whose words did not decode.
    pub frames_bad: usize,
    /// Reed-Solomon bytes corrected.
    pub rs_corrected: u32,
    /// Platform data bytes.
    pub bytes: usize,
    /// The data are ASCII.
    pub ascii: bool,
    /// The carrier frequency, Hz.
    pub frequency_hz: f64,
    /// Its channel, when a plan gives one.
    pub channel: Option<u32>,
    /// The carrier line over the floor, dB.
    pub snr_db: f64,
    /// The data as text, or the first 512 bytes in hex.
    pub text: String,
}

impl Shift for ReportRow {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

impl ReportRow {
    fn type_text(&self) -> &'static str {
        if self.alert {
            "alert"
        } else {
            "self-timed"
        }
    }
    fn health_text(&self) -> String {
        format!("{:010b}", self.health & 0x3FF)
    }
    fn compression_text(&self) -> &'static str {
        match self.compression {
            0 => "none",
            1 => "gzip",
            _ => "other",
        }
    }
    fn format_text(&self) -> &'static str {
        if self.ascii {
            "ASCII"
        } else {
            "binary"
        }
    }
}

impl Shaped for ReportRow {
    type Row = HrdcpReportRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&HrdcpReportRow {
            at: Seconds(self.at.t),
            heard_at: self.at.wall.map(Time),
            address: &self.address,
            bits_corrected: self.corrected as i64,
            sequence: i64::from(self.sequence),
            r#type: self.type_text(),
            health: self.health_text().as_str(),
            compression: self.compression_text(),
            crc_ok: self.crc_ok,
            frames: self.frames as i64,
            frames_bad: self.frames_bad as i64,
            rs_corrected: i64::from(self.rs_corrected),
            bytes: self.bytes as i64,
            format: self.format_text(),
            bit_rate_hz: Hertz(1200.0),
            manchester: false,
            freq_hz: Hertz(self.frequency_hz),
            channel: self.channel.map(i64::from),
            snr_db: self.snr_db,
            text: &self.text,
        });
    }
}

/// One platform: its last message.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlatformRow {
    /// Its last message.
    pub last: ReportRow,
}

impl Shift for PlatformRow {
    fn shift_times(&mut self, by_s: f64) {
        self.last.shift_times(by_s);
    }
}

impl Shaped for PlatformRow {
    type Row = HrdcpPlatformRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let r = &self.last;
        let end = r
            .text
            .char_indices()
            .nth(64)
            .map_or(r.text.len(), |(i, _)| i);
        out.put(&HrdcpPlatformRow {
            address: &r.address,
            channel: r.channel.map(i64::from),
            snr_db: r.snr_db,
            format: r.format_text(),
            text: &r.text[..end],
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
        });
    }
}

/// What the display keeps across sessions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heard {
    /// The good messages, newest last.
    pub reports: LogRing<ReportRow>,
    /// The platforms, by address.
    pub platforms: ObjectTable<String, PlatformRow>,
    /// Good messages.
    pub good: u64,
    /// Bad messages.
    pub bad: u64,
}

impl Default for Heard {
    fn default() -> Self {
        Heard {
            reports: LogRing::new(REPORT_CAP),
            platforms: ObjectTable::new(PLATFORM_CAP),
            good: 0,
            bad: 0,
        }
    }
}

impl Shift for Heard {
    fn shift_times(&mut self, by_s: f64) {
        self.reports.shift_times(by_s);
        self.platforms.shift_times(by_s);
    }
}

impl Tables for Heard {
    const FORMAT: StateFormat = StateFormat::new("satcom.hrdcp", 1);
    fn clear(&mut self) {
        self.reports.clear();
        self.platforms.clear();
        self.good = 0;
        self.bad = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.reports.adopt(saved.reports);
        self.platforms.adopt(saved.platforms);
        self.good = saved.good;
        self.bad = saved.bad;
    }
}

/// The display: the message log, the platforms, the carriers, the band line, the channel plan and the events.
pub struct HrdcpDisplay {
    clock: Clock,
    heard: Heard,
    centre_hz: f64,
    carriers: Vec<CarrierRow>,
    line: Vec<f32>,
    rate: f64,
    live: bool,
    host_clock: sdrkit_core::Clock,
    storage: Option<Arc<dyn Storage>>,
    plan_path: String,
    plan: Option<ChannelPlan>,
    plan_fault: Option<String>,
    log: Vec<String>,
    dirty: bool,
    summary: String,
}

impl Default for HrdcpDisplay {
    fn default() -> Self {
        HrdcpDisplay {
            clock: Clock::default(),
            heard: Heard::default(),
            centre_hz: 0.0,
            carriers: Vec::new(),
            line: Vec::new(),
            rate: 0.0,
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            storage: None,
            plan_path: String::new(),
            plan: None,
            plan_fault: None,
            log: Vec::new(),
            dirty: true,
            summary: String::new(),
        }
    }
}

impl HrdcpDisplay {
    /// Reads the plan file of the setting from the host's storage, when both are there.
    fn load_plan(&mut self, id: Option<SettingId>) -> Result<(), SettingError> {
        self.plan = None;
        self.plan_fault = None;
        if self.plan_path.is_empty() {
            return Ok(());
        }
        let malformed = |id: Option<SettingId>| SettingError::Malformed {
            id: id.unwrap_or(SettingId(0)),
            at: None,
        };
        let Ok(path) = StorePath::new(&self.plan_path) else {
            return Err(malformed(id));
        };
        let Some(storage) = self.storage.as_ref() else {
            self.plan_fault = Some("the host's storage is not there yet".into());
            return Ok(());
        };
        match storage.read(&path) {
            Ok(bytes) => match ChannelPlan::parse(&String::from_utf8_lossy(&bytes)) {
                Ok(p) => self.plan = Some(p),
                Err(_) if id.is_some() => return Err(malformed(id)),
                Err(e) => self.plan_fault = Some(e.to_string()),
            },
            Err(e) => self.plan_fault = Some(e.to_string()),
        }
        Ok(())
    }

    fn channel_of(&self, hz: f64) -> Option<u32> {
        self.plan.as_ref().and_then(|p| p.channel_of(hz))
    }

    fn message(&mut self, r: &Received, t: f64, out: &mut Outbox) {
        let m = &r.msg;
        let h = &m.header;
        let freq = self.centre_hz + r.offset_hz;
        let address = format!("{:08X}", h.address);
        let complete = m.data.len() == h.length;
        if !(m.crc_ok && complete) {
            self.heard.bad += 1;
            self.log.push(format!(
                "HRDCP {address} seq {}: {} bytes, {} frames, {} bad, CRC {}",
                h.sequence,
                h.length,
                m.frames,
                m.frames_bad,
                if complete { "BAD" } else { "missing" }
            ));
            return;
        }
        let ascii = m
            .data
            .iter()
            .all(|&c| (0x20..0x7F).contains(&c) || c == b'\r' || c == b'\n' || c == b'\t');
        let text = if ascii {
            let t = String::from_utf8_lossy(&m.data).into_owned();
            // The text and pseudo-binary readers of GOES DCS.
            match format_of(&m.data) {
                DataFormat::Ascii => ascii_fields(&t).join(" "),
                _ => t,
            }
        } else {
            m.data.iter().take(512).fold(String::new(), |mut s, b| {
                let _ = write!(s, "{b:02X}");
                s
            })
        };
        let row = ReportRow {
            at: self.clock.stamp(t),
            address: address.clone(),
            corrected: h.corrected,
            sequence: h.sequence,
            alert: h.alert,
            health: h.health,
            compression: h.compression,
            crc_ok: true,
            frames: m.frames,
            frames_bad: m.frames_bad,
            rs_corrected: m.rs_corrected,
            bytes: h.length,
            ascii,
            frequency_hz: freq,
            channel: self.channel_of(freq),
            snr_db: r.snr_db,
            text,
        };
        self.heard.good += 1;
        self.dirty = true;
        out.messages(0).push(HRDCP_REPORT.event(
            t,
            freq,
            &HrdcpReportEvent {
                source: &row.address,
                text: &row.text,
                snr_db: Some(row.snr_db).filter(|s| s.is_finite()),
                family: Family::Sensor,
                address: &row.address,
                channel: row.channel.map(i64::from),
                sequence: i64::from(row.sequence),
                r#type: row.type_text(),
                health: row.health_text().as_str(),
                compression: row.compression_text(),
                bytes: row.bytes as i64,
                rs_corrected: i64::from(row.rs_corrected),
                crc_ok: true,
            },
        ));
        let _ = self
            .heard
            .platforms
            .upsert(&row.address, row.at, PlatformRow::default);
        if let Some(p) = self.heard.platforms.get_mut(&row.address) {
            p.last = row.clone();
        }
        self.heard.reports.push(row);
    }
}

impl Decoder for HrdcpDisplay {
    type Input = Timed<HrdcpItem>;
    fn name(&self) -> &'static str {
        "satcom.hrdcp_display"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        HrdcpDisplaySetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(
            HrdcpDisplaySetting::ChannelPlan,
            Value::Text(self.plan_path.as_str().into()),
        )])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match HrdcpDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            HrdcpDisplaySetting::ChannelPlan => {
                let path = CHANNEL_PLAN.read(id, v)?.trim().to_string();
                if !path.is_empty() && StorePath::new(&path).is_err() {
                    return Err(SettingError::Malformed { id, at: None });
                }
                let old = std::mem::replace(&mut self.plan_path, path);
                if let Err(e) = self.load_plan(Some(id)) {
                    self.plan_path = old;
                    let _ = self.load_plan(None);
                    return Err(e);
                }
                self.dirty = true;
            }
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::from_decl(display_decl())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(self.live, self.host_clock.now());
        self.centre_hz = f.centre_hz;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        self.centre_hz = f.centre_hz;
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.host_clock = ctx.clock.clone();
        self.storage = Some(Arc::clone(&ctx.storage));
        if self.plan.is_none() && !self.plan_path.is_empty() {
            let _ = self.load_plan(None);
        }
    }
    fn process(&mut self, x: &[Timed<HrdcpItem>], _first: u64, out: &mut Outbox) {
        for it in x {
            let t = self.clock.at(it.time);
            match &it.value {
                HrdcpItem::Bad => self.heard.bad += 1,
                HrdcpItem::Message(r) => self.message(r, t, out),
                HrdcpItem::Status {
                    carriers,
                    line,
                    span_hz,
                } => {
                    self.carriers.clone_from(carriers);
                    self.line.clone_from(line);
                    self.rate = *span_hz;
                }
            }
        }
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.clock.now();
        let h = &self.heard;
        let plan = match (&self.plan, &self.plan_fault) {
            (Some(pl), _) => pl.name.clone(),
            (None, Some(f)) => format!("not loaded: {f}"),
            (None, None) => String::new(),
        };
        p.put(&HrdcpFields {
            frames_ok: h.good as i64,
            frames_bad: h.bad as i64,
            carriers: self.carriers.len() as i64,
            plan: plan.as_str(),
            note: NOTE_TEXT,
        });
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&h.reports, &HRDCP_REPORTS, now, p);
            shaped::rows(&h.platforms, &HRDCP_PLATFORMS, now, p);
        }
        let centre = self.centre_hz;
        let rows: Vec<(CarrierRow, Option<u32>)> = self
            .carriers
            .iter()
            .map(|c| (c.clone(), self.channel_of(centre + c.offset_hz)))
            .collect();
        shaped::indexed_rows(p, &HRDCP_CARRIERS, &rows, |(c, ch), out| {
            out.put(&HrdcpCarrierRow {
                    offset_hz: Hertz(c.offset_hz),
                    channel: ch.map(i64::from),
                    snr_db: c.snr_db,
                    state: i64::from(c.state),
                    bit_rate_hz: (c.state == 2).then_some(Hertz(1200.0)),
                    bytes: c.bytes as i64,
                },
            );
        });
        let r = p.results_mut();
        for l in self.log.drain(..) {
            r.log(l);
        }
        if !self.line.is_empty() {
            let n = self.line.len() as f64;
            let rate = self.rate;
            let points = || {
                self.line
                    .iter()
                    .enumerate()
                    .map(move |(k, &v)| [((k as f64 + 0.5) / n * rate - rate / 2.0) as f32, v])
            };
            set_series(r, HRDCP_SPECTRUM.key, true, points);
        }
        let h = &self.heard;
        let state = if h.good > 0 {
            2
        } else if !self.carriers.is_empty() {
            1
        } else {
            0
        };
        let (reports, platforms) = (h.reports.len(), h.platforms.len());
        let summary = &mut self.summary;
        summary.clear();
        let _ = if reports == 0 {
            write!(summary, "HRDCP: no platform heard")
        } else {
            write!(
                summary,
                "HRDCP: {reports} reports from {platforms} platforms"
            )
        };
        let offset = self.carriers.first().map(|c| c.offset_hz);
        let snr = h.reports.newest_first().next().map(|r| r.snr_db as f32);
        set_status(r, LockState::from_level(state), offset, snr, None, summary);
    }
    fn clear(&mut self) {
        self.heard.clear();
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.heard.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.heard.restore_state(state, self.clock.now())?;
        self.dirty = true;
        Ok(())
    }
}

/// Adds the components of this stack to `r`.
pub fn register(r: &mut Registry) {
    r.add("satcom.hrdcp_receiver", || {
        Box::new(DecoderComponent::new(HrdcpRx::default()))
    });
    r.add("satcom.hrdcp_display", || {
        Box::new(DecoderComponent::new(HrdcpDisplay::default()))
    });
}

/// The receiver.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = GraphBuilder::new()
        .add("rx", DecoderComponent::new(HrdcpRx::default()))
        .add("display", DecoderComponent::new(HrdcpDisplay::default()))
        .input("rx")
        .connect("rx.out", "display")
        .output("display.events")
        .status_from("display")
        .clock(REPORT_S)
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(
            MIN_RATE,
            "Meteosat HRDCP needs a sample rate of at least 48 ksps",
        )
        .default_centre(DESCRIPTOR.tuning.default_hz)
        .route("threshold_db", &["rx.threshold_db"])?
        .route("channel_plan", &["display.channel_plan"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::HrdcpGenerator::from_config(cfg)))
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "Meteosat high-rate data collection platforms (1200 bit/s offset QPSK, coded): address, sequence, health, data and the platform table, for every carrier in the capture. Experimental: the demodulator's loop values are not confirmed by a public document. Receive only.",
    tuning: Tuning {
        min_hz: 401.0e6,
        max_hz: 403.0e6,
        default_hz: TEST_SIGNAL_HZ,
        rate_hz: 1e6,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 0.25e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: HrdcpSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Dcp],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};

/// The verification of the stack: platform A's message.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, two platforms", o, 0.0);
    c.seconds = c.seconds.max(9.0);
    c.expect_audio = Some(false);
    c.check = Some(check_reports);
    vec![c]
}

/// At least one message received whole.
fn check_reports(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let ok = crate::field(r, HrdcpFields::FRAMES_OK.key) as i64;
    vec![CheckLine::new(
        "reports",
        ok >= 1,
        format!("{ok} messages received"),
    )]
}

/// The benchmark of the receiver: the two platforms of the test signal at 1 Msps.
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "hrdcp.receiver_1m",
        run: |s| crate::bench_receiver(s, &DESCRIPTOR, None, 8.0),
    }]
}
