//! TV (auto): listens to the channel, works out which TV standard it carries and its parameters, and requests the host to
//! switch the receiver of that standard on (a `HostRequest::Follow` to its stack, on the channel's centre).
//!
//! The detector is a [`Decoder`] on a capture: on a helper thread when the samples are live, in line for a recording.

pub mod detect;

use crate::typed::{AutoFields, AutoFoundEvent, AUTO_FOUND};
use detect::{detect, Detection, Standard, Verdict};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, HostRequest, LockState, Receiver, ResultsDecl, Schema,
    Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, Tuning, Value,
    Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Helper, Kind,
    Outbox, Port, RateFrom, StackGraph, Timed,
};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "tv-auto";

/// What TV (auto) publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// MPEG-2 video decoded to frames of raw pixels by the stack that TV (auto) switches on.
pub const DECODE_VIDEO: Setting<field::Bool> = sdrkit_proto_mpegts::layer::DECODE_VIDEO.help(
    "The stack that TV (auto) switches on sends MPEG-2 video as frames of raw pixels (I420) in display order. \
     H.264 and HEVC always leave as coded pictures.",
);

/// The input comes from a dish (an LNB).
pub const SATELLITE: Setting<field::Bool> = Setting::new("satellite", "Satellite input", "", false)
    .help("The input comes from a dish (an LNB).");

sdrkit_core::settings! {
    /// The settings of TV (auto).
    pub enum AutoSetting {
        /// Satellite input.
        Satellite = SATELLITE,
        /// MPEG-2 video decoded to frames of raw pixels, in the stack that it switches on.
        DecodeVideo = DECODE_VIDEO,
    }
}

/// The detector of TV (auto) as a decoder: it waits 1 s, captures 0.35 s, detects, and requests the receiver of the standard
/// found. After three captures with no signal, or three unsure ones, it stops until it is cleared.
pub struct AutoDecoder {
    rate: f64,
    rf: f64,
    satellite: bool,
    decode_video: bool,
    skip: usize,
    cap: Vec<Complex32>,
    want: usize,
    done: bool,
    try_again: bool,
    tries: u32,
    no_signal: u32,
    unsure: u32,
    line: String,
    switch_to: Option<&'static str>,
    last: Option<Detection>,
}

impl Default for AutoDecoder {
    fn default() -> Self {
        AutoDecoder {
            rate: 8e6,
            rf: 0.0,
            satellite: false,
            decode_video: false,
            skip: 0,
            cap: Vec::new(),
            want: 0,
            done: false,
            try_again: false,
            tries: 0,
            no_signal: 0,
            unsure: 0,
            line: String::new(),
            switch_to: None,
            last: None,
        }
    }
}

impl AutoDecoder {
    fn start(&mut self, settle_s: f64) {
        self.skip = (settle_s * self.rate) as usize;
        self.want = ((0.35 * self.rate) as usize).max(65_536);
        self.cap.clear();
        self.done = false;
        self.try_again = false;
        self.line = "Listening to the channel...".into();
    }

    fn apply(&mut self, det: Detection, time: f64, out: &mut Outbox) {
        self.tries += 1;
        self.switch_to = None;
        match det.verdict {
            Verdict::NoSignal => {
                self.no_signal += 1;
                if self.no_signal >= 3 {
                    self.line =
                        "No TV signal found here. Tune to a TV channel (or pick the standard by hand in the stack list).".into();
                    self.try_again = true;
                } else {
                    self.line = "No TV signal on this frequency: still listening...".into();
                }
            }
            _ if det.verdict < Verdict::Likely || det.standard.stack_id().is_none() => {
                self.unsure += 1;
                if self.unsure >= 3 {
                    self.line = format!(
                        "Unsure: {}. Pick the standard by hand in the stack list.",
                        det.headline
                    );
                    self.try_again = true;
                } else {
                    self.line = format!("Unsure what this is ({}): looking again...", det.headline);
                }
            }
            _ => {
                let id = det.standard.stack_id().unwrap_or("");
                self.switch_to = det.standard.stack_id();
                // A channel more than 0.4 MHz off the centre is tuned onto, rounded to 1 kHz (not for analogue TV).
                let freq = if det.standard != Standard::Analog && det.centre_offset_hz.abs() > 0.4e6
                {
                    ((self.rf + det.centre_offset_hz) / 1e3).round() * 1e3
                } else {
                    self.rf
                };
                let name = match det.standard {
                    Standard::DvbT | Standard::DvbT2 => "DVB-T",
                    s => s.name(),
                };
                self.line = format!("Found {}. The {name} stack is locking...", det.headline);
                let width = det.width_mhz * 1e6;
                if let Some(signal) = det
                    .standard
                    .signal_type()
                    .filter(|_| freq > 0.0 && width > 0.0)
                {
                    out.items::<HostRequest>(0).push(Timed {
                        time,
                        value: HostRequest::Follow {
                            freq_hz: freq,
                            signal: Some(signal),
                            talkgroup: None,
                            bandwidth_hz: width,
                            label: det.headline.clone(),
                            // The receiver found stays on until the user picks another mode.
                            max_s: 86_400.0,
                            settings: vec![(
                                DECODE_VIDEO.key().to_owned(),
                                Value::Bool(self.decode_video),
                            )],
                        },
                    });
                }
                let found = AutoFoundEvent {
                    stack: id,
                    standard: det.standard,
                    text: &det.headline,
                    width: Hertz(width),
                    sure: det.verdict == Verdict::Sure,
                    family: Family::Receiver,
                };
                out.messages(1).push(AUTO_FOUND.event(time, freq, &found));
                self.done = true;
            }
        }
        self.last = Some(det);
        if !self.done && !self.try_again {
            self.start(1.0);
        }
    }
}

impl Decoder for AutoDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "tv.auto"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
        ]
    }
    fn settings(&self) -> Arc<Schema> {
        AutoSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (AutoSetting::Satellite, Value::Bool(self.satellite)),
            (AutoSetting::DecodeVideo, Value::Bool(self.decode_video)),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match AutoSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AutoSetting::Satellite => self.satellite = SATELLITE.read(id, v)?,
            AutoSetting::DecodeVideo => self.decode_video = DECODE_VIDEO.read(id, v)?,
        }
        Ok(())
    }
    fn results(&self) -> Published {
        crate::typed::results(ID)
    }
    fn clear(&mut self) {
        self.no_signal = 0;
        self.unsure = 0;
        self.start(0.0);
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.rate = input.rate;
        self.rf = input.centre_hz;
        self.no_signal = 0;
        self.unsure = 0;
        self.tries = 0;
        self.last = None;
        self.start(1.0);
        Ok(vec![
            Format::new(Kind::typed::<HostRequest>(), 0.0),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        if self.done || self.try_again {
            return;
        }
        let mut x = x;
        if self.skip > 0 {
            let k = self.skip.min(x.len());
            self.skip -= k;
            x = &x[k..];
        }
        let need = self.want - self.cap.len();
        self.cap.extend_from_slice(&x[..need.min(x.len())]);
        if self.cap.len() >= self.want {
            self.line = "working out the standard...".into();
            let det = detect(&self.cap, self.rate, self.rf, self.satellite);
            self.apply(det, first as f64 / self.rate.max(1.0), out);
        }
    }
    fn helper(&self) -> Helper {
        Helper::WhenLive {
            batch: 0,
            backlog: 1 << 22,
        }
    }
    fn report(&mut self, r: &mut Published) {
        crate::status::write_status(
            r.results_mut(),
            &crate::status::StatusLine {
                lock_state: if self.switch_to.is_some() {
                    LockState::Locked
                } else {
                    LockState::Searching
                },
                snr_db: None,
                offset_hz: None,
                level_dbfs: None,
                summary: &self.line,
            },
        );
        let progress = if self.want > 0 {
            100.0 * self.cap.len() as f64 / self.want as f64
        } else {
            0.0
        };
        let last = self.last.as_ref();
        r.put(&AutoFields {
            line: &self.line,
            progress_pct: progress.min(100.0),
            tries: self.tries as i64,
            switch_to: self.switch_to,
            standard: last.map(|d| d.standard),
            headline: last.map(|d| d.headline.as_str()),
            verdict: last.map(|d| d.verdict),
            confidence_pct: last.map(|d| 100.0 * d.confidence),
            width: last.map(|d| Hertz(d.width_mhz * 1e6)),
            centre_offset: last.map(|d| Hertz(d.centre_offset_hz)),
            try_again_offered: self.try_again,
        });
    }
    fn report_interval_s(&self) -> f64 {
        0.1
    }
}

/// The TV (auto) receiver graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<sdrkit_graph::Graph, GraphError> {
    GraphBuilder::new()
        .add("auto", DecoderComponent::new(AutoDecoder::default()))
        .input("auto.in")
        .output("auto.requests")
        .output("auto.events")
        .status_from("auto")
        .catalog(catalog)
        .build()
}

/// The TV (auto) receiver. Command `try_again` starts the detection again after it stopped.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that the stack refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(2e6, "TV (auto) needs 2 Msps or more")
        .route("satellite", &["auto.satellite"])?
        .route("decode_video", &["auto.decode_video"])?
        .command(
            "try_again",
            sdrkit_graph::CommandTarget::of("auto", "clear"),
        )
        .default_centre(DESCRIPTOR.tuning.default_hz)
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The TV (auto) stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "TV (auto)",
    group: "TV",
    blurb: "Finds out which TV standard the channel carries (DVB-T, DVB-T2, DVB-C, DVB-S/S2, ISDB-T, ATSC 1.0 / 3.0, DTMB, analogue) and its parameters, then switches the right stack on",
    tuning: Tuning { kind: Some(sdrkit_core::TuningKind::Channels), min_hz: 40e6, max_hz: 2.2e9, ..Tuning::anywhere(474e6, 10e6, 2e6, 8e6) },
    submodes: &[],
    settings: AutoSetting::schema,
    make_receiver,
    make_generator: None,
    lock_profile: crate::TV_LOCK_PROFILE,
    signal_types: &[SignalType::DvbT, SignalType::DvbT2, SignalType::DvbC, SignalType::DvbS, SignalType::DvbS2, SignalType::IsdbT, SignalType::Atsc1, SignalType::Atsc3, SignalType::Dtmb, SignalType::AnalogTv],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::AUTO),
    benchmarks: None,
};
