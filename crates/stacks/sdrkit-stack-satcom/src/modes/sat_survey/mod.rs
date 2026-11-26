//! Satellite carrier survey (`sat_survey`): carriers, channels or frame structures of satellite signals that have no public
//! specification, measured and never decoded. Twelve submodes share four methods: carriers followed through the pass
//! (named by their Doppler curve when the host gives element sets and the station position), channel energy on a fixed
//! grid, the frame rate of a framed signal from the gaps between frames, and a sequence repeated every period. A submode is a
//! method with its signal parameters; frequencies and channel names are data that the host gives as settings.
//!
//! Documents: ETSI TS 101 376-5-5 clause 4.2 (the GMR-1 carrier spacing of 31.25 kHz); Komodromos and Humphreys, OneWeb
//! parameter estimation (a synchronisation sequence repeated every millisecond); Spacetrack Report 3 and Vallado et al.
//! (2006) for SGP4.

pub mod carrier;
pub mod channel;
pub mod framegap;
pub mod gen;
pub mod names;
pub mod repeat;

use crate::typed::{
    SatSurveyCarrierEvent, SatSurveyCarrierRow, SatSurveyChannelRow, SatSurveyFields,
    SAT_SURVEY_CARRIER, SAT_SURVEY_CARRIERS, SAT_SURVEY_CHANNELS,
};
use carrier::CarrierSurvey;
use channel::ChannelSurvey;
use framegap::{FrameGap, FrameLine};
use names::ChannelTable;
use repeat::RepeatSeq;
use sdrkit_algo_dsp::linetrack::LineTrack;
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::{field, Published, SeriesDef};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Benchmark, Clock, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile,
    LockState, Receiver, ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet,
    StackConfig, StackDescriptor, Status, SubMode, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kind, Outbox, Port, RateFrom, Registry, RunContext, StackGraph,
};
use sdrkit_util_geo::orbit::dopplermatch::{match_track, read_catalogue, MatchParams};
use sdrkit_util_geo::orbit::Satellite;
use sdrkit_util_geo::{Observer, Position};
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "sat_survey";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 250e3;
/// The rate of the default submode, Hz.
pub const RATE_HZ: f64 = 1e6;
/// Results are published this often, s of signal.
pub const REPORT_S: f64 = 0.25;
/// An active carrier line is shown from this length, s.
pub const SHOWN_MIN_S: f64 = 2.5;
/// The drift of a line is fitted over this many points.
pub const DRIFT_POINTS: usize = 16;
/// A finished line of this length or more is matched against the element sets, s.
pub const MATCH_MIN_S: f64 = 30.0;
/// The Doppler match compares at most this many points of a track.
pub const MATCH_POINTS: usize = 96;
/// A track whose frequency noise is this large (rms, Hz) or larger is not named.
pub const MATCH_NOISE_HZ: f64 = 60.0;

/// The rules of the Doppler match for a track of `points` points: at most [`MATCH_POINTS`] compared, and the rms limit set
/// three standard deviations of the rms estimate under [`MATCH_NOISE_HZ`], so a track with that much noise is refused (the
/// rms of `n` points has a standard deviation of `1 / sqrt(2 (n - 1))` of itself).
pub fn match_params(points: usize) -> MatchParams {
    let n = points.clamp(2, MATCH_POINTS) as f64;
    MatchParams {
        max_points: MATCH_POINTS,
        believable_rms_hz: MATCH_NOISE_HZ * (1.0 - 3.0 / (2.0 * (n - 1.0)).sqrt()).max(0.25),
        ..MatchParams::default()
    }
}
/// The most carrier lines shown.
pub const CARRIER_ROWS: usize = 64;
/// The most channels shown.
pub const CHANNEL_ROWS: usize = 128;
/// The frame rate searched by `starlink`, Hz. Not confirmed: no document that states it could be read.
pub const STARLINK_FRAME_HZ: f64 = 750.0;
/// The repetition rate of the `oneweb` sequence, Hz (Komodromos and Humphreys).
pub const ONEWEB_REPEAT_HZ: f64 = 1000.0;
/// The GMR-1 carrier spacing, Hz (ETSI TS 101 376-5-5 clause 4.2).
pub const GMR1_GRID_HZ: f64 = 31_250.0;

/// What no public document confirms.
pub const NOTE_TEXT: &str = "Not confirmed by a public document: every detection rule of the survey. Carrier: transforms \
with bins under the submode's bin, frames of 0.25 s, a background of up to 64 medians, peaks over max(threshold, the level \
noise reaches once in 100 frames + 1.6 dB), tracks gated by max(3 bins, 0.25 × (0.032 Hz/s per MHz + 20 bins/s) × gap + 2 \
bins), ended after 5 s unseen, kept from 5 s and 8 points, shown from 2.5 s; the Doppler match of lines of 30 s or more with \
96 points, -2° elevation, offsets to 25 kHz, refused from 60 Hz of noise (rms limit 3 deviations under), confident over 3 ×, ambiguous under 1.5 × the best. \
Channel: decisions every 20 ms, the floor at the 40th percentile, 2 hits to turn active. Frame gap: the envelope at about \
200 ksps, segments of 65536 values, 8 harmonics over clock errors of ±60 ppm, detected 3 dB under the threshold after 4 \
segments. Repeated sequence: 64 bins, folds of 400 periods, detected 3 dB over the threshold. The per-submode thresholds and \
bins. The Starlink frame rate of 750 Hz and the Starlink test signal (32 subcarriers, 1056-sample symbols at 240 Msps, 302 \
symbols a frame): no document that states them could be read. The channel tables are from public satellite lists and press \
reports.";

/// The method of a submode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// Narrow carriers followed through the pass.
    Carrier = 0,
    /// Channel energy on a grid.
    Channel = 1,
    /// The frame rate from the gaps between frames.
    FrameGap = 2,
    /// A sequence repeated every period.
    Repeat = 3,
}

impl Method {
    /// The key shown in the results.
    pub fn key(self) -> &'static str {
        match self {
            Method::Carrier => "carrier",
            Method::Channel => "channel",
            Method::FrameGap => "frame gap",
            Method::Repeat => "repeated sequence",
        }
    }
}

/// The signal parameters of a submode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// The submode id.
    pub id: &'static str,
    /// The method.
    pub method: Method,
    /// The rate the submode asks for, Hz.
    pub asked_rate_hz: f64,
    /// The bin the transform aims at, Hz (carrier and channel methods).
    pub bin_hz: f64,
    /// The threshold when the setting is 0, dB.
    pub threshold_db: f64,
    /// The frame or repetition rate, Hz (frame methods).
    pub frame_hz: f64,
    /// The channel grid, Hz (channel method).
    pub grid_hz: f64,
    /// Finished carrier lines are matched against the element sets.
    pub doppler_match: bool,
}

const fn carrier(id: &'static str, rate: f64, bin: f64, thr: f64, dm: bool) -> Params {
    Params {
        id,
        method: Method::Carrier,
        asked_rate_hz: rate,
        bin_hz: bin,
        threshold_db: thr,
        frame_hz: 0.0,
        grid_hz: 0.0,
        doppler_match: dm,
    }
}

const fn channel(id: &'static str, rate: f64, bin: f64, grid: f64) -> Params {
    Params {
        id,
        method: Method::Channel,
        asked_rate_hz: rate,
        bin_hz: bin,
        threshold_db: 8.0,
        frame_hz: 0.0,
        grid_hz: grid,
        doppler_match: false,
    }
}

const fn framed(id: &'static str, method: Method, frame: f64) -> Params {
    Params {
        id,
        method,
        asked_rate_hz: 20e6,
        bin_hz: 0.0,
        threshold_db: 8.0,
        frame_hz: frame,
        grid_hz: 0.0,
        doppler_match: false,
    }
}

/// The submode the stack starts in.
pub const DEFAULT_SUBMODE: &str = "doppler_id";

/// The parameters of every submode, in the order of [`SUBMODES`].
pub static PARAMS: [Params; 12] = [
    carrier("zombie", 1e6, 8.0, 12.0, true),
    carrier("doppler_id", 1e6, 8.0, 12.0, true),
    channel("uhf_satcom", 10e6, 1000.0, 5000.0),
    channel("gmr1", 10e6, 2000.0, GMR1_GRID_HZ),
    framed("starlink", Method::FrameGap, STARLINK_FRAME_HZ),
    framed("oneweb", Method::Repeat, ONEWEB_REPEAT_HZ),
    carrier("tdrs", 2e6, 30.0, 10.0, false),
    carrier("tiangong", 1e6, 10.0, 10.0, true),
    carrier("tsikada", 1e6, 8.0, 10.0, true),
    channel("gonets", 4e6, 500.0, 25_000.0),
    channel("tiantong", 10e6, 2000.0, 25_000.0),
    carrier("iss", 1e6, 10.0, 10.0, true),
];

/// The submodes; each default dial is the usual dial of `data/science/science_dials.csv`.
pub static SUBMODES: [SubMode; 12] = [
    SubMode {
        id: "zombie",
        name: "Satellites still transmitting",
        default_hz: 145.970e6,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "doppler_id",
        name: "Carriers named by Doppler",
        default_hz: 437.5e6,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "uhf_satcom",
        name: "UHF military satellite channels",
        default_hz: 255e6,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "gmr1",
        name: "GMR-1 channels",
        default_hz: 1535e6,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "starlink",
        name: "Starlink frames",
        default_hz: 11_325e6,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "oneweb",
        name: "OneWeb sequence",
        default_hz: 11_075e6,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "tdrs",
        name: "TDRS forward link",
        default_hz: 2106.4e6,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "tiangong",
        name: "Tiangong carriers",
        default_hz: 2224.075e6,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "tsikada",
        name: "Tsikada navigation carriers",
        default_hz: 150e6,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "gonets",
        name: "Gonets messaging bursts",
        default_hz: 388.5e6,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "tiantong",
        name: "Tiantong S-band channels",
        default_hz: 2185e6,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "iss",
        name: "Space station downlinks",
        default_hz: 145.8e6,
        ..SubMode::DEFAULT
    },
];

/// The parameters of submode `id`; the default submode for `None` or an id the stack does not have.
pub fn params_of(id: Option<&str>) -> &'static Params {
    let id = id.unwrap_or(DEFAULT_SUBMODE);
    PARAMS
        .iter()
        .find(|p| p.id == id)
        .or_else(|| PARAMS.iter().find(|p| p.id == DEFAULT_SUBMODE))
        .unwrap_or(&PARAMS[0])
}

/// The default dial of submode `id`, Hz.
fn default_dial(id: &str) -> f64 {
    SUBMODES
        .iter()
        .find(|s| s.id == id)
        .map_or(0.0, |s| s.default_hz)
}

// ---------------------------------------------------------------------------------------------------------------------------
// Settings.

/// The threshold.
pub const THRESHOLD: Setting<field::F64> = Setting::new("threshold_db", "Threshold", "dB", 0.0)
    .range(0.0, 30.0, 0.0)
    .zero_means("the submode's own")
    .help("The detection threshold: 3 to 30 dB, 0 for the submode's own.");
/// The channel table.
pub const CHANNELS: Setting<field::Text> = Setting::new("channels", "Channel table", "", "").help(
    "Channels as text, one a line (or separated by ;): frequency_hz,name[,norad]. Lines of the data file \
     (submode,frequency_hz,name,norad) are taken for this submode only. A measured frequency takes the nearest name within \
     max(3.5 kHz, 30 ppm).",
);
/// The element sets.
pub const SATELLITES: Setting<field::Text> =
    Setting::new("satellites", "Satellite elements", "", "").help(
        "Two-line element sets (each with or without a name line, checksums checked). With the station position they name \
     the finished carrier lines of the submodes with a Doppler match.",
    );
/// The UTC of sample 0.
pub const START_UTC: Setting<field::F64> = Setting::new(
    "start_utc",
    "UTC of sample 0 (files; 0 = host clock)",
    "s",
    0.0,
)
.range(0.0, 1e11, 1.0);

sdrkit_core::settings! {
    /// The settings of [`SurveyDecoder`].
    pub enum DecoderSetting {
        /// The threshold.
        Threshold = THRESHOLD,
        /// The channel table.
        Channels = CHANNELS,
        /// The element sets.
        Satellites = SATELLITES,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
    }
}

sdrkit_core::settings! {
    /// The settings of the `sat_survey` stack.
    pub enum SurveySetting {
        /// The threshold.
        Threshold = THRESHOLD,
        /// The channel table.
        Channels = CHANNELS,
        /// The element sets.
        Satellites = SATELLITES,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// The test signal is on.
        GenSignal = gen::GEN_SIGNAL,
        /// The S/N of the test signal.
        GenLevel = gen::GEN_LEVEL_DB,
        /// The carrier offset of the test signal.
        GenCarrier = gen::GEN_CARRIER_HZ,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Results.

/// The last carrier frame in dB over the background, 256 values.
pub const SPECTRUM: SeriesDef = SeriesDef {
    key: "spectrum",
    label: "Spectrum over the background",
    x_unit: "Hz",
    y_unit: "dB",
};

/// What the decoder publishes.
fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(SatSurveyFields::SHAPE.fields)
        .with_table(&SAT_SURVEY_CARRIERS)
        .with_table(&SAT_SURVEY_CHANNELS)
        .with_series(&SPECTRUM)
        .event(&SAT_SURVEY_CARRIER)
}

// ---------------------------------------------------------------------------------------------------------------------------
// The decoder.

/// One element set of the catalogue.
struct Sat {
    sat: Satellite,
    name: String,
    norad: u32,
}

/// A carrier line as shown.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    /// Its number.
    pub id: u64,
    /// Offset from the centre, Hz (last point).
    pub offset_hz: f64,
    /// Frequency, MHz.
    pub mhz: f64,
    /// Level over the background, dB (last point).
    pub snr_db: f64,
    /// Length, s.
    pub duration_s: f64,
    /// Drift over the last points, Hz/s.
    pub drift_hz_s: Option<f64>,
    /// Active.
    pub active: bool,
    /// Channel name, or empty.
    pub channel: String,
    /// Satellite name, or empty.
    pub satellite: String,
    /// Catalogue number of the satellite.
    pub norad: Option<u32>,
    /// rms of the match, Hz.
    pub rms_hz: Option<f64>,
    /// Another satellite fits about as well.
    pub ambiguous: bool,
}

impl Line {
    /// The row of the line.
    fn row(&self) -> SatSurveyCarrierRow<'_> {
        SatSurveyCarrierRow {
            line: i64::try_from(self.id).unwrap_or(i64::MAX),
            offset_hz: Hertz(self.offset_hz),
            mhz: self.mhz,
            snr_db: self.snr_db,
            duration_s: Seconds(self.duration_s),
            drift_hz_s: self.drift_hz_s,
            active: self.active,
            channel: &self.channel,
            satellite: &self.satellite,
            norad: self.norad.map(i64::from),
            rms_hz: self.rms_hz,
            ambiguous: self.ambiguous,
        }
    }
}

/// The method running at the input rate.
enum Engine {
    Carrier(CarrierSurvey),
    Channel(ChannelSurvey),
    FrameGap(FrameGap),
    Repeat(Box<RepeatSeq>),
}

/// The UTC of sample 0: the setting, else the host clock now.
fn start_utc_of(setting: f64, clock: &Clock) -> f64 {
    if setting > 0.0 {
        setting
    } else {
        clock.now().floor()
    }
}

/// The survey decoder of one submode.
pub struct SurveyDecoder {
    p: &'static Params,
    threshold_set: f64,
    channels_text: String,
    table: ChannelTable,
    satellites_text: String,
    catalogue: Vec<Sat>,
    start_setting: f64,
    start_utc: f64,
    clock: Clock,
    observer: Option<Observer>,
    rate: f64,
    centre: f64,
    engine: Option<Engine>,
    buf: Vec<Complex32>,
    finished: VecDeque<Line>,
    events: Vec<Event>,
    started: bool,
    status: Status,
}

impl SurveyDecoder {
    /// The decoder of submode `id` (the default for `None` or an unknown id).
    pub fn new(id: Option<&str>) -> Self {
        SurveyDecoder {
            p: params_of(id),
            threshold_set: THRESHOLD.default,
            channels_text: String::new(),
            table: ChannelTable::default(),
            satellites_text: String::new(),
            catalogue: Vec::new(),
            start_setting: START_UTC.default,
            start_utc: 0.0,
            clock: Clock::fixed(0.0),
            observer: None,
            rate: 0.0,
            centre: 0.0,
            engine: None,
            buf: Vec::new(),
            finished: VecDeque::new(),
            events: Vec::new(),
            started: false,
            status: Status::default(),
        }
    }

    /// The threshold in force, dB.
    fn threshold(&self) -> f64 {
        if self.threshold_set > 0.0 {
            self.threshold_set
        } else {
            self.p.threshold_db
        }
    }

    fn make_engine(&self) -> Engine {
        let (p, thr) = (self.p, self.threshold());
        match p.method {
            Method::Carrier => {
                Engine::Carrier(CarrierSurvey::new(self.rate, p.bin_hz, thr, self.centre))
            }
            Method::Channel => Engine::Channel(ChannelSurvey::new(
                self.rate,
                self.centre,
                p.bin_hz,
                p.grid_hz,
                thr,
            )),
            Method::FrameGap => Engine::FrameGap(FrameGap::new(self.rate, p.frame_hz, thr)),
            Method::Repeat => Engine::Repeat(Box::new(RepeatSeq::new(self.rate, p.frame_hz, thr))),
        }
    }

    /// A line of `t`, matched against the catalogue when `matched` and the rules allow.
    fn line_of(&self, t: &LineTrack, active: bool, matched: bool) -> Line {
        let last = t.last();
        let mut l = Line {
            id: t.id(),
            offset_hz: last.hz,
            mhz: (self.centre + last.hz) / 1e6,
            snr_db: last.level_db,
            duration_s: t.duration_s(),
            drift_hz_s: t.drift_hz_s(DRIFT_POINTS),
            active,
            channel: self
                .table
                .name_of(self.centre + last.hz)
                .map(|c| c.name.clone())
                .unwrap_or_default(),
            ..Line::default()
        };
        if matched && self.p.doppler_match && t.duration_s() >= MATCH_MIN_S {
            if let Some(obs) = &self.observer {
                let pts: Vec<(f64, f64)> = t
                    .points()
                    .iter()
                    .map(|q| (self.start_utc + q.t, q.hz))
                    .collect();
                let sats: Vec<Satellite> = self.catalogue.iter().map(|s| s.sat.clone()).collect();
                let m = match_track(
                    &pts,
                    self.centre + t.first().hz,
                    obs,
                    &sats,
                    &match_params(pts.len()),
                );
                if let Some(c) = m.named() {
                    let s = &self.catalogue[c.index];
                    l.satellite = s.name.clone();
                    l.norad = Some(s.norad);
                    l.rms_hz = Some(c.rms_hz);
                    l.ambiguous = m.ambiguous();
                }
            }
        }
        l
    }

    /// Takes the finished lines of the carrier method: rows and events.
    fn take_finished(&mut self, out: &mut Outbox) {
        let done = match self.engine.as_mut() {
            Some(Engine::Carrier(c)) => c.take_finished(),
            _ => return,
        };
        for t in done {
            let l = self.line_of(&t, false, true);
            let text = if l.satellite.is_empty() {
                format!("Carrier at {:.6} MHz, {:.0} s", l.mhz, l.duration_s)
            } else {
                format!(
                    "Carrier at {:.6} MHz, {:.0} s: {}",
                    l.mhz, l.duration_s, l.satellite
                )
            };
            let ev = SAT_SURVEY_CARRIER.event(
                t.last().t,
                self.centre + l.offset_hz,
                &SatSurveyCarrierEvent {
                    text: text.as_str(),
                    line: i64::try_from(l.id).unwrap_or(i64::MAX),
                    offset_hz: Hertz(l.offset_hz),
                    mhz: l.mhz,
                    snr_db: l.snr_db,
                    duration_s: Seconds(l.duration_s),
                    drift_hz_s: l.drift_hz_s,
                    channel: l.channel.as_str(),
                    satellite: l.satellite.as_str(),
                    norad: l.norad.map(i64::from),
                    rms_hz: l.rms_hz,
                    ambiguous: l.ambiguous,
                },
            );
            self.events.push(ev);
            self.finished.push_back(l);
            while self.finished.len() > CARRIER_ROWS {
                self.finished.pop_front();
            }
        }
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }

    /// The carrier lines shown now: the finished ones, then the active ones of [`SHOWN_MIN_S`] or more; at most
    /// [`CARRIER_ROWS`], the newest kept.
    pub fn lines(&self) -> Vec<Line> {
        let mut v: Vec<Line> = self.finished.iter().cloned().collect();
        if let Some(Engine::Carrier(c)) = &self.engine {
            v.extend(
                c.tracker()
                    .active()
                    .iter()
                    .filter(|t| t.duration_s() >= SHOWN_MIN_S)
                    .map(|t| self.line_of(t, true, false)),
            );
        }
        let skip = v.len().saturating_sub(CARRIER_ROWS);
        v.drain(..skip);
        v
    }

    fn publish_carriers(&self, p: &mut Published) -> usize {
        p.clear_rows(&SAT_SURVEY_CARRIERS);
        let lines = self.lines();
        for l in &lines {
            SAT_SURVEY_CARRIERS.put(p, &l.id.to_string(), &l.row());
        }
        if let Some(Engine::Carrier(c)) = &self.engine {
            if let Some(id) = p.results().decl().series_id(SPECTRUM.key) {
                let s = c.spectrum();
                let step = c.bin_hz() * (c.fft_len() / s.len()) as f64;
                let x0 = -(c.fft_len() as f64 / 2.0) * c.bin_hz() + step / 2.0;
                p.results_mut().update_series(id, |ser| {
                    ser.points.clear();
                    ser.points.extend(
                        s.iter()
                            .enumerate()
                            .map(|(i, &v)| [(x0 + i as f64 * step) as f32, v]),
                    );
                    true
                });
            }
        }
        lines.len()
    }

    fn publish_channels(&self, p: &mut Published, c: &ChannelSurvey) -> (usize, usize) {
        p.clear_rows(&SAT_SURVEY_CHANNELS);
        let mut seen: Vec<&channel::ChannelState> =
            c.channels().iter().filter(|g| g.ever).collect();
        let active = seen.iter().filter(|g| g.active).count();
        let count = seen.len();
        seen.sort_by(|a, b| b.active.cmp(&a.active).then(b.snr_db.total_cmp(&a.snr_db)));
        seen.truncate(CHANNEL_ROWS);
        // The best row is written last: it is the newest.
        for g in seen.iter().rev() {
            let f = self.centre + g.offset_hz;
            let name = self
                .table
                .name_of(f)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            SAT_SURVEY_CHANNELS.put(
                p,
                &format!("{}", g.grid_hz.round() as i64),
                &SatSurveyChannelRow {
                    offset_hz: Hertz(g.offset_hz),
                    frequency_hz: Hertz(f),
                    width_hz: g.width_hz,
                    snr_db: g.snr_db,
                    duty: g.duty,
                    bursts: i64::try_from(g.bursts).unwrap_or(i64::MAX),
                    last_seen_s: g.last_seen_s,
                    active: g.active,
                    name: &name,
                },
            );
        }
        (active, count)
    }
}

impl Decoder for SurveyDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.sat_survey"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::Threshold => {
                let v = THRESHOLD.read(id, value)?;
                if v > 0.0 && v < 3.0 {
                    return Err(SettingError::OutOfRange {
                        id,
                        min: 3.0,
                        max: 30.0,
                        got: v,
                    });
                }
                self.threshold_set = v;
                let thr = self.threshold();
                match self.engine.as_mut() {
                    Some(Engine::Carrier(c)) => c.set_threshold(thr),
                    Some(Engine::Channel(c)) => c.set_threshold(thr),
                    Some(Engine::FrameGap(c)) => c.set_threshold(thr),
                    Some(Engine::Repeat(c)) => c.set_threshold(thr),
                    None => {}
                }
            }
            DecoderSetting::Channels => {
                let text = CHANNELS.read(id, value)?;
                self.table = ChannelTable::parse(text, self.p.id)
                    .map_err(|_| SettingError::Malformed { id, at: None })?;
                self.channels_text = text.to_string();
            }
            DecoderSetting::Satellites => {
                let text = SATELLITES.read(id, value)?;
                let (sets, _bad) = read_catalogue(text);
                self.catalogue = sets
                    .iter()
                    .filter_map(|t| {
                        Satellite::from_tle(t).ok().map(|sat| Sat {
                            sat,
                            name: t.name.as_str().to_string(),
                            norad: t.catalog_number,
                        })
                    })
                    .collect();
                self.satellites_text = text.to_string();
            }
            DecoderSetting::StartUtc => {
                self.start_setting = START_UTC.read(id, value)?;
                self.start_utc = start_utc_of(self.start_setting, &self.clock);
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::Threshold, Value::F64(self.threshold_set)),
            (
                DecoderSetting::Channels,
                Value::Text(self.channels_text.clone()),
            ),
            (
                DecoderSetting::Satellites,
                Value::Text(self.satellites_text.clone()),
            ),
            (DecoderSetting::StartUtc, Value::F64(self.start_setting)),
        ])
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = ctx.clock.clone();
        self.start_utc = start_utc_of(self.start_setting, &self.clock);
        self.observer = ctx.station.position.map(|p| {
            let alt = ctx.station.height_m.unwrap_or(p.alt_m);
            Observer::new(Position { alt_m: alt, ..p })
        });
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if !input.rate.is_finite() || input.rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.centre = input.centre_hz;
        self.start_utc = start_utc_of(self.start_setting, &self.clock);
        self.engine = Some(self.make_engine());
        self.finished.clear();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.started = true;
        self.buf.clear();
        self.buf.extend(x.iter().map(|z| {
            if z.re.is_finite() && z.im.is_finite() {
                *z
            } else {
                Complex32::default()
            }
        }));
        match self.engine.as_mut() {
            Some(Engine::Carrier(c)) => c.process(&self.buf),
            Some(Engine::Channel(c)) => c.process(&self.buf),
            Some(Engine::FrameGap(c)) => c.process(&self.buf),
            Some(Engine::Repeat(c)) => c.process(&self.buf),
            None => {}
        }
        self.take_finished(out);
    }
    fn report(&mut self, p: &mut Published) {
        let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
        let mut threshold = self.threshold();
        let (mut bin, mut lines, mut channels) = (None, None, None);
        let mut found = false;
        let mut summary = String::new();
        let mut frame: Option<FrameLine> = None;
        match &self.engine {
            Some(Engine::Carrier(c)) => {
                bin = Some(c.bin_hz());
                threshold = c.peak_threshold_db();
                let n = self.publish_carriers(p);
                lines = Some(count(n));
                found = n > 0;
                summary = format!("Survey ({}): {n} carrier lines", self.p.id);
            }
            Some(Engine::Channel(c)) => {
                bin = Some(c.bin_hz());
                threshold = c.on_level_db();
                let (active, seen) = self.publish_channels(p, c);
                channels = Some((count(active), count(seen)));
                found = seen > 0;
                summary = format!(
                    "Survey ({}): {active} channels active, {seen} seen",
                    self.p.id
                );
            }
            Some(Engine::FrameGap(c)) => frame = Some(c.line()),
            Some(Engine::Repeat(c)) => frame = Some(c.line()),
            None => {}
        }
        if let Some(f) = &frame {
            found = f.detected;
            summary = if f.searches == 0 {
                format!("Survey ({}): measuring", self.p.id)
            } else {
                format!(
                    "Survey ({}): {} at {:.3} Hz, {:.1} dB",
                    self.p.id,
                    if f.detected {
                        "detected"
                    } else {
                        "not detected"
                    },
                    f.measured_hz,
                    f.level_db
                )
            };
        }
        let st = &mut self.status;
        st.lock_state = match (self.started, found) {
            (false, _) => LockState::Off,
            (true, false) => LockState::Searching,
            (true, true) => LockState::Locked,
        };
        st.summary.clear();
        if self.started {
            st.summary.push_str(&summary);
        } else {
            st.summary.push_str("Survey: waiting for samples");
        }
        let measured = frame.as_ref().filter(|f| f.searches > 0);
        p.put(&SatSurveyFields {
            text: st.summary.as_str(),
            method: self.p.method.key(),
            threshold_in_force_db: threshold,
            bin_hz: bin,
            catalogue: count(self.catalogue.len()),
            lines,
            channels_active: channels.map(|c| c.0),
            channels_seen: channels.map(|c| c.1),
            frame_rate_hz: frame.as_ref().map(|f| f.searched_hz),
            measured_rate_hz: measured.map(|f| f.measured_hz),
            frame_level_db: measured.map(|f| f.level_db),
            detected: frame.as_ref().map(|f| f.detected),
            note: NOTE_TEXT,
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.finished.clear();
        self.events.clear();
        match self.engine.as_mut() {
            Some(Engine::Carrier(c)) => c.clear(),
            Some(Engine::Channel(c)) => c.clear(),
            Some(Engine::FrameGap(c)) => c.clear(),
            Some(Engine::Repeat(c)) => c.clear(),
            None => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The stack.

/// The receiver of the submode of `cfg`: one decoder on the input.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let p = params_of(cfg.submode.as_deref());
    let graph = GraphBuilder::new()
        .add("dec", DecoderComponent::new(SurveyDecoder::new(Some(p.id))))
        .input("dec")
        .output("dec.events")
        .status_from("dec")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE_HZ,
            "The satellite carrier survey needs a sample rate of at least 250 ksps",
        )
        .default_centre(default_dial(p.id))
        .route(THRESHOLD.key(), &["dec.threshold_db"])?
        .route(CHANNELS.key(), &["dec.channels"])?
        .route(SATELLITES.key(), &["dec.satellites"])?
        .route(START_UTC.key(), &["dec.start_utc"])?
        .command("clear", CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's component to a registry.
pub fn register(r: &mut Registry) {
    r.add("satcom.sat_survey", || {
        Box::new(DecoderComponent::new(SurveyDecoder::new(None)))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::SatSurveyGenerator::from_config(cfg)?))
}

fn check_lines(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let n = crate::field(r, SatSurveyFields::LINES.key);
    vec![CheckLine::new(
        SatSurveyFields::LINES.key,
        n >= 1.0,
        format!("{n}, at least 1 expected"),
    )]
}

fn check_channels(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let n = crate::field(r, SatSurveyFields::CHANNELS_SEEN.key);
    vec![CheckLine::new(
        SatSurveyFields::CHANNELS_SEEN.key,
        (3.0..=11.0).contains(&n),
        format!("{n}, 3 to 11 expected"),
    )]
}

fn check_detected(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let d = crate::field(r, SatSurveyFields::DETECTED.key);
    let level = crate::field(r, SatSurveyFields::FRAME_LEVEL_DB.key);
    vec![CheckLine::new(
        SatSurveyFields::DETECTED.key,
        d >= 1.0,
        format!("level {level:.1} dB"),
    )]
}

/// The cases at the stack's rate (1 Msps). The frame-gap method is left out: it needs its asked rate of 20 Msps, and at
/// 1 Msps its level stays near 5.5 dB, at the edge of detection; the crate tests run it at 20 Msps.
fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let case = |name: &str,
                submode: &str,
                seconds: f64,
                check: fn(&sdrkit_core::Results) -> Vec<CheckLine>| {
        let mut c = VerifyCase::new(name, o, 0.0);
        c.submode = Some(submode.into());
        c.seconds = o.seconds.max(seconds);
        c.snr_db = None;
        c.expect_audio = Some(false);
        c.check = Some(check);
        c
    };
    vec![
        case(
            "test signal, carrier method (tsikada)",
            "tsikada",
            8.0,
            check_lines,
        ),
        case(
            "test signal, channel method (uhf_satcom)",
            "uhf_satcom",
            8.0,
            check_channels,
        ),
        case(
            "test signal, repeated sequence (oneweb)",
            "oneweb",
            2.0,
            check_detected,
        ),
    ]
}

/// The engine's checks of the test signal.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// Times the receiver of `submode` at `rate` on `signal_s` seconds of its own test signal.
fn bench_at(seconds: f64, submode: &str, rate: f64, signal_s: f64) -> sdrkit_core::BenchResult {
    use sdrkit_core::{BenchResult, InputInfo, Output};
    let settings = Values::defaults(&(DESCRIPTOR.settings)());
    let p = params_of(Some(submode));
    let mut g = gen::SatSurveyGenerator::new(rate, p, gen::GenParams::of(p.method), 1);
    let mut x = vec![Complex32::default(); (rate * signal_s) as usize];
    g.generate(&mut x);
    let cfg = StackConfig::new(settings).with_submode(submode);
    let Ok(mut rx) = (DESCRIPTOR.make_receiver)(&cfg) else {
        return BenchResult::default();
    };
    let dial = default_dial(submode);
    if rx
        .configure(&InputInfo::new(rate, dial, dial).recording())
        .is_err()
    {
        return BenchResult::default();
    }
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), signal_s, || {
        for c in x.chunks(65536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// The stack's benchmarks: each method at its asked rate.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "sat_survey.carrier_1m",
            run: |s| bench_at(s, "tsikada", 1e6, 2.0),
        },
        Benchmark {
            name: "sat_survey.channel_10m",
            run: |s| bench_at(s, "uhf_satcom", 10e6, 0.4),
        },
        Benchmark {
            name: "sat_survey.frame_gap_20m",
            run: |s| bench_at(s, "starlink", 20e6, 0.2),
        },
        Benchmark {
            name: "sat_survey.repeat_20m",
            run: |s| bench_at(s, "oneweb", 20e6, 0.2),
        },
    ]
}

/// The satellite carrier survey.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Satellite carrier survey",
    group: "Satellite",
    blurb: "Carriers, channels and frame structures of satellite signals without a public specification: measured, never \
            decoded; carriers named by their Doppler curve when element sets and the station position are given. Receive only.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 30e6,
        max_hz: 12.7e9,
        default_hz: 437.5e6,
        rate_hz: RATE_HZ,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 0.9 * RATE_HZ,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &SUBMODES,
    settings: SurveySetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};
