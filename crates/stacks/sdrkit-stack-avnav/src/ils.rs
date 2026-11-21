//! ILS localizer and glide path: the depths of the 90 Hz and 150 Hz tones, their difference (DDM) and sum (SDM), the needle and
//! the command, the flag, the localizer's identification and the paired frequencies. The marker part names the 75 MHz marker
//! beacon that the aircraft passes over ([`crate::marker`]).
//! Standard: ICAO Annex 10 Volume I, 3.1.

use crate::carrier::{AmCarrier, CarrierInfo};
use crate::checked;
use crate::fields::{carrier_fields, put_carrier_status, put_idents};
use crate::ident::IdentEnvelope;
use crate::keyer::ToneKeyer;
use crate::marker::{
    GenMarker, GenMarkerKeying, MarkerGenerator, MarkerKind, MarkerPassage, MarkerPattern,
    MarkerState, MarkerSummary, MarkerTones,
};
use crate::morse::{IdentTracker, MorseDecoder, MorseWord};
use crate::pairing::{glide_path_for, localizer_for, DmeChannel};
use crate::typed::{
    CarrierFields, IdentFields, IlsFields, MarkerFields, MarkerPassageRow, IDENTS, MARKER_PASSAGES,
};
use sdrkit_algo_dsp::goertzel::ToneBank;
use sdrkit_components::dsp::{AudioResampler, BiquadComponent, Channel};
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, GenConfig,
    Generator, LockState, Receiver, Record, Results, Setting, SettingError, SettingId, SettingSet,
    SignalInfo, StackConfig, Value, Values,
};
use sdrkit_graph::components::BiquadShape;
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, DecoderCommand, Format, Graph,
    GraphBuilder, GraphError, Io, Item, Kind, Port, RateFrom, StackGraph, Timed,
};
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "ils";
/// The least radio rate.
pub const MIN_RATE_HZ: f64 = 32e3;
/// Channel width.
pub const CHANNEL_WIDTH_HZ: f64 = 10e3;
/// Channel rate.
pub const CHANNEL_RATE_HZ: f64 = 16e3;
/// Full-scale DDM of the localizer.
pub const LOC_FULL_SCALE_DDM: f64 = 0.155;
/// Full-scale DDM of the glide path.
pub const GP_FULL_SCALE_DDM: f64 = 0.175;

sdrkit_core::choice_enum! {
    /// Which part of the ILS.
    pub enum IlsPart {
        /// Course (left, right).
        Localizer = ("localizer", "Localizer"),
        /// Path (up, down).
        Glideslope = ("glideslope", "Glide path"),
        /// The 75 MHz marker beacons.
        Marker = ("marker", "Marker beacon"),
    }
}

sdrkit_core::choice_enum! {
    /// Which part of the ILS the receiver reads.
    pub enum PartChoice {
        /// By frequency: 74.5 to 75.5 MHz is a marker beacon, above 300 MHz a glide path.
        Auto = ("auto", "By frequency"),
        /// The localizer.
        Localizer = ("localizer", "Localizer"),
        /// The glide path.
        Glideslope = ("glideslope", "Glide path"),
        /// The marker beacons.
        Marker = ("marker", "Marker beacon"),
    }
}

impl PartChoice {
    /// The choice of a submode id (`localizer`, `glideslope`, `marker`), if it names one.
    pub fn of_submode(id: &str) -> Option<PartChoice> {
        match id {
            "localizer" => Some(PartChoice::Localizer),
            "glideslope" => Some(PartChoice::Glideslope),
            "marker" => Some(PartChoice::Marker),
            _ => None,
        }
    }
}

/// The frequencies that the automatic choice reads as a marker beacon, Hz.
pub const MARKER_RANGE_HZ: (f64, f64) = (74.5e6, 75.5e6);

impl IlsPart {
    /// The part that `choice` names, for a station tuned at `tuned_hz`.
    pub fn pick(choice: PartChoice, tuned_hz: f64) -> IlsPart {
        match choice {
            PartChoice::Localizer => IlsPart::Localizer,
            PartChoice::Glideslope => IlsPart::Glideslope,
            PartChoice::Marker => IlsPart::Marker,
            PartChoice::Auto if (MARKER_RANGE_HZ.0..=MARKER_RANGE_HZ.1).contains(&tuned_hz) => {
                IlsPart::Marker
            }
            PartChoice::Auto if tuned_hz > 300e6 => IlsPart::Glideslope,
            PartChoice::Auto => IlsPart::Localizer,
        }
    }
    /// Full-scale DDM. None for the marker part, which has no needle.
    pub fn full_scale(self) -> Option<f64> {
        match self {
            IlsPart::Localizer => Some(LOC_FULL_SCALE_DDM),
            IlsPart::Glideslope => Some(GP_FULL_SCALE_DDM),
            IlsPart::Marker => None,
        }
    }
    /// The nominal SDM. None for the marker part.
    pub fn nominal_sdm(self) -> Option<f64> {
        match self {
            IlsPart::Localizer => Some(0.4),
            IlsPart::Glideslope => Some(0.8),
            IlsPart::Marker => None,
        }
    }
}

/// One measurement of the two tones, averaged.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IlsFix {
    /// Depth of the 90 Hz tone, 0..1.
    pub m90: f32,
    /// Depth of the 150 Hz tone.
    pub m150: f32,
    /// DDM: m90 - m150.
    pub ddm: f32,
    /// SDM: m90 + m150.
    pub sdm: f32,
}

impl Item for IlsFix {
    const KIND: &'static str = "ils.fix";
}

pub const AVERAGE: Setting<field::Duration> =
    Setting::new("average", "Averaging", "s", 0.5).range(0.1, 10.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`IlsDdm`].
    pub enum IlsDdmSetting {
        /// How long the depths are averaged over.
        Average = AVERAGE,
    }
}

/// The tone measurement (`avnav.ils_ddm`, real -> [`IlsFix`]): the depths of the 90 Hz and 150 Hz tones over periods of 0.1 s
/// (a whole number of cycles of both), averaged.
pub struct IlsDdm {
    average: f64,
    rate: f64,
    bank: Option<ToneBank>,
    m90: f64,
    m150: f64,
    started: bool,
}

impl Default for IlsDdm {
    fn default() -> Self {
        IlsDdm {
            average: AVERAGE.default,
            rate: 0.0,
            bank: None,
            m90: 0.0,
            m150: 0.0,
            started: false,
        }
    }
}

impl Component for IlsDdm {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.ils_ddm",
            vec![Port::real("in")],
            vec![Port::typed::<IlsFix>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(IlsDdmSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate < 400.0 {
            return Err(ConfigError::RateTooLow {
                need: 400.0,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        let n = (0.1 * f.rate).round() as usize;
        self.bank = Some(ToneBank::with_rate(f.rate, &[90.0, 150.0], n)?);
        self.started = false;
        Ok(vec![Format {
            rate: 0.0,
            ..f.with_kind(Kind::typed::<IlsFix>())
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let x = io.real(0);
        let out = io.items_out::<IlsFix>(0);
        let Some(mut bank) = self.bank.take() else {
            return;
        };
        bank.process(x, |end, amps| {
            let (a, b) = (amps[0].norm() as f64, amps[1].norm() as f64);
            if self.started {
                let k = (0.1 / self.average.max(0.1)).min(1.0);
                self.m90 += k * (a - self.m90);
                self.m150 += k * (b - self.m150);
            } else {
                (self.m90, self.m150, self.started) = (a, b, true);
            }
            out.push(Timed {
                time: (first + end as u64) as f64 / self.rate,
                value: IlsFix {
                    m90: self.m90 as f32,
                    m150: self.m150 as f32,
                    ddm: (self.m90 - self.m150) as f32,
                    sdm: (self.m90 + self.m150) as f32,
                },
            });
        });
        self.bank = Some(bank);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match IlsDdmSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            IlsDdmSetting::Average => self.average = AVERAGE.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(IlsDdmSetting::Average, Value::Duration(self.average))])
    }
}

sdrkit_core::choice_enum! {
    /// What the ILS needle says.
    pub enum IlsCommand {
        /// On the localizer course.
        OnCourse = ("on_course", "on course"),
        /// On the glide path.
        OnPath = ("on_path", "on path"),
        /// Fly left.
        FlyLeft = ("fly_left", "fly left"),
        /// Fly right.
        FlyRight = ("fly_right", "fly right"),
        /// Fly up.
        FlyUp = ("fly_up", "fly up"),
        /// Fly down.
        FlyDown = ("fly_down", "fly down"),
        /// The flag is out.
        Flag = ("flag", "flag"),
    }
}

fn new_published() -> Published {
    crate::typed::store(
        &[
            IlsFields::SHAPE,
            IdentFields::SHAPE,
            CarrierFields::SHAPE,
            MarkerFields::SHAPE,
        ],
        &[&IDENTS, &MARKER_PASSAGES],
    )
}

pub const PART_SETTING: Setting<field::Choice<PartChoice>> =
    Setting::choice("part", "Part", PartChoice::Auto)
        .help("Localizer, glide path or marker beacon. By frequency, 74.5 to 75.5 MHz is a marker beacon and above 300 MHz a glide path.");
pub const CONFIRM: Setting<field::I64> = Setting::new("confirm", "Hearings to confirm", "", 2)
    .range(1.0, 10.0, 1.0)
    .help("How many times an identification must be heard before it is shown.");

sdrkit_core::settings! {
    /// The settings of [`IlsReport`].
    pub enum IlsReportSetting {
        /// The part read.
        Part = PART_SETTING,
        /// Hearings that confirm an identification.
        Confirm = CONFIRM,
    }
}

/// The ILS report (`avnav.ils_report`). Its command `clear` forgets the station.
pub struct IlsReport {
    part_setting: PartChoice,
    part: IlsPart,
    tuned: f64,
    tracker: IdentTracker,
    wpm: Option<f32>,
    fix: Option<IlsFix>,
    carrier: Option<CarrierInfo>,
    marker: Option<MarkerState>,
    last_passage: Option<MarkerSummary>,
    passages: i64,
    published: Published,
    dme_channel: Option<String>,
    summary: String,
    keying: String,
}

impl Default for IlsReport {
    fn default() -> Self {
        IlsReport {
            part_setting: PART_SETTING.default,
            part: IlsPart::Localizer,
            tuned: 0.0,
            tracker: IdentTracker::new(CONFIRM.default as u32),
            wpm: None,
            fix: None,
            carrier: None,
            marker: None,
            last_passage: None,
            passages: 0,
            published: new_published(),
            dme_channel: None,
            summary: String::with_capacity(128),
            keying: String::with_capacity(96),
        }
    }
}

impl IlsReport {
    /// The part and the paired DME channel for the tuned frequency.
    fn tune(&mut self) {
        self.part = IlsPart::pick(self.part_setting, self.tuned);
        let f = (self.tuned / 1e4).round() / 100.0;
        let loc = match self.part {
            IlsPart::Localizer => Some(f),
            IlsPart::Glideslope => localizer_for(f),
            IlsPart::Marker => None,
        };
        self.dme_channel = loc.and_then(DmeChannel::for_vhf).map(|c| c.name());
    }

    fn publish(&mut self) {
        let part = self.part;
        let c = self.carrier.unwrap_or_default();
        let f = (self.tuned / 1e4).round() / 100.0;
        let paired = match part {
            IlsPart::Localizer => glide_path_for(f),
            IlsPart::Glideslope => localizer_for(f),
            IlsPart::Marker => None,
        };
        put_idents(
            &mut self.published,
            &self.tracker,
            self.tracker.confirmed(),
            self.wpm,
        );
        self.published
            .put(&carrier_fields(self.carrier.as_ref(), self.tuned));
        // The marker part has no needle: the tone measurement is not shown and the flag is out.
        let measured = self.fix.as_ref().filter(|_| part != IlsPart::Marker);
        let fix = measured.copied().unwrap_or_default();
        let sdm_ok = part
            .nominal_sdm()
            .is_some_and(|nominal| (fix.sdm as f64 - nominal).abs() < 0.5 * nominal);
        let flag = !(c.present && measured.is_some() && sdm_ok);
        // The needle, limited to 150 µA either way: positive is fly right on the localizer and fly up on the glide path.
        let needle = part.full_scale().filter(|_| !flag).map(|full| {
            let sign = if part == IlsPart::Localizer {
                1.0
            } else {
                -1.0
            };
            (sign * fix.ddm as f64 / full * 150.0).clamp(-150.0, 150.0)
        });
        let command = match (part, needle) {
            (_, None) | (IlsPart::Marker, _) => IlsCommand::Flag,
            (IlsPart::Localizer, Some(u)) if u.abs() <= 5.0 => IlsCommand::OnCourse,
            (IlsPart::Glideslope, Some(u)) if u.abs() <= 5.0 => IlsCommand::OnPath,
            (IlsPart::Localizer, Some(u)) if u > 0.0 => IlsCommand::FlyRight,
            (IlsPart::Localizer, Some(_)) => IlsCommand::FlyLeft,
            (IlsPart::Glideslope, Some(u)) if u > 0.0 => IlsCommand::FlyUp,
            (IlsPart::Glideslope, Some(_)) => IlsCommand::FlyDown,
        };
        self.published.put(&IlsFields {
            part,
            ddm: needle.map(|_| fix.ddm as f64),
            sdm_pct: measured.map(|f| f.sdm as f64 * 100.0),
            m90_pct: measured.map(|f| f.m90 as f64 * 100.0),
            m150_pct: measured.map(|f| f.m150 as f64 * 100.0),
            needle_ua: needle,
            needle_dots: needle.map(|u| (u / 75.0).clamp(-2.5, 2.5)),
            command,
            flag,
            paired_hz: paired.map(|m| Hertz((m * 1e6).round())),
            tuned_hz: Hertz(self.tuned),
            dme_channel: self.dme_channel.as_deref(),
        });
        let lock_state = if part == IlsPart::Marker {
            self.publish_marker()
        } else {
            self.publish_marker_off();
            let words = &IlsCommand::CHOICES[usize::from(command.index())].label;
            self.summary.clear();
            // Writing into a String cannot fail.
            let _ = match (flag, self.tracker.confirmed()) {
                (true, _) if c.present => write!(self.summary, "Flag"),
                (true, _) => write!(self.summary, "Searching"),
                (false, Some(id)) => write!(self.summary, "{id}, {words}"),
                (false, None) => write!(self.summary, "{words}"),
            };
            if !flag {
                LockState::Locked
            } else if c.present {
                LockState::Tracking
            } else {
                LockState::Searching
            }
        };
        put_carrier_status(
            &mut self.published,
            lock_state,
            self.carrier.as_ref(),
            CHANNEL_WIDTH_HZ,
            &self.summary,
        );
    }

    /// The marker fields outside the marker part: nothing measured.
    fn publish_marker_off(&mut self) {
        self.published.put(&MarkerFields {
            marker: MarkerKind::None,
            marker_tone_hz: None,
            marker_depth_pct: None,
            keying: None,
            keying_rate: None,
            pairs_per_min: None,
            dot_s: None,
            dash_s: None,
            keying_fits: None,
            passage_s: None,
            passages: self.passages,
            keyed_now: false,
            held_depth_pct: None,
            pattern: MarkerPattern::NotYet,
        });
    }

    /// The marker fields and the headline of the marker part: the passage under way, or else the last one. Returns the lock
    /// state: locked while the passage under way fits the standard.
    fn publish_marker(&mut self) -> LockState {
        let current = self.marker.and_then(|m| m.current);
        let shown = current.or(self.last_passage);
        self.keying.clear();
        if let Some(s) = &shown {
            s.write_keying(&mut self.keying);
        }
        let live = self.marker;
        self.published.put(&MarkerFields {
            marker: shown.map_or(MarkerKind::None, |s| s.marker),
            marker_tone_hz: shown.map(|s| Hertz(s.tone_hz.round())),
            marker_depth_pct: shown.map(|s| s.depth * 100.0),
            keying: shown.map(|_| self.keying.as_str()),
            keying_rate: shown.map(|s| s.per_s),
            pairs_per_min: shown.and_then(|s| s.pairs_per_min),
            dot_s: shown.and_then(|s| s.dot_s),
            dash_s: shown.and_then(|s| s.dash_s),
            keying_fits: shown.map(|s| s.fits),
            passage_s: shown.map(|s| s.duration_s),
            passages: self.passages,
            keyed_now: live.is_some_and(|m| m.keyed_now),
            held_depth_pct: live.map(|m| m.held_depth * 100.0),
            pattern: live.map_or(MarkerPattern::NotYet, |m| m.pattern),
        });
        self.summary.clear();
        let _ = match (&current, self.passages) {
            (Some(s), _) if s.fits => write!(self.summary, "{}: {}", s.marker.name(), self.keying),
            (Some(s), _) => write!(
                self.summary,
                "{} tone, the keying does not fit the standard: {}",
                s.marker.name(),
                self.keying
            ),
            (None, 1) => write!(self.summary, "1 marker passage so far"),
            (None, n) if n > 1 => write!(self.summary, "{n} marker passages so far"),
            (None, _) if self.carrier.is_some() || live.is_some() => {
                write!(self.summary, "No marker tone")
            }
            (None, _) => write!(self.summary, "Waiting for data"),
        };
        if current.is_some_and(|s| s.fits) {
            LockState::Locked
        } else if self.carrier.is_some_and(|c| c.present) || live.is_some_and(|m| m.keyed_now) {
            LockState::Tracking
        } else {
            LockState::Searching
        }
    }

    /// A finished passage: a row of the table and a line of the log.
    fn passage(&mut self, s: MarkerSummary) {
        self.passages += 1;
        self.last_passage = Some(s);
        self.keying.clear();
        s.write_row(&mut self.keying);
        let key = format!("{:04}", self.passages);
        MARKER_PASSAGES.put(
            &mut self.published,
            &key,
            &MarkerPassageRow {
                kind: s.kind(),
                marker: s.marker,
                text: &self.keying,
                level_dbfs: self.carrier.map(|c| f64::from(c.level_dbfs)),
            },
        );
        self.published.results_mut().log(format!(
            "{:.1} s: {}, {}",
            s.end_s,
            s.marker.name(),
            self.keying
        ));
    }
}

impl Component for IlsReport {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "avnav.ils_report",
            vec![
                Port::typed::<IlsFix>("fix"),
                Port::typed::<CarrierInfo>("carrier"),
                Port::typed::<MorseWord>("ident"),
                Port::typed::<MarkerState>("marker"),
                Port::typed::<MarkerPassage>("passage"),
            ],
        )
        .with_settings(IlsReportSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.tuned = inputs[0].tuned_hz;
        self.tune();
        self.publish();
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let mut changed = false;
        if let Some(f) = io.items::<IlsFix>(0).last() {
            self.fix = Some(f.value);
            changed = true;
        }
        if let Some(c) = io.items::<CarrierInfo>(1).last() {
            self.carrier = Some(c.value);
            changed = true;
        }
        for w in io.items::<MorseWord>(2) {
            self.tracker.heard(&w.value.text, w.time);
            self.wpm = Some(w.value.wpm);
            self.published
                .results_mut()
                .log(format!("{:.1} s: ident {}", w.time, w.value.text));
            changed = true;
        }
        if let Some(m) = io.items::<MarkerState>(3).last() {
            self.marker = Some(m.value);
            changed = true;
        }
        for p in io.items::<MarkerPassage>(4) {
            self.passage(p.value.0);
            changed = true;
        }
        if changed {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match IlsReportSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            IlsReportSetting::Part => {
                self.part_setting = PART_SETTING.read(id, v)?;
                self.tune();
            }
            IlsReportSetting::Confirm => self.tracker.set_confirm(CONFIRM.read(id, v)? as u32),
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                IlsReportSetting::Part,
                Value::Choice(self.part_setting.index()),
            ),
            (
                IlsReportSetting::Confirm,
                Value::I64(i64::from(self.tracker.confirm())),
            ),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        DecoderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match DecoderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DecoderCommand::Clear => {
                self.tracker.clear();
                self.wpm = None;
                self.fix = None;
                self.carrier = None;
                self.marker = None;
                self.last_passage = None;
                self.passages = 0;
                self.published = new_published();
                self.publish();
            }
        }
        Ok(None)
    }
}

/// The ILS graph: the channel to the carrier stage, then the tone measurement, the marker tones, the identification keyer and the
/// audio filters, and the report.
///
/// # Errors
///
/// A [`GraphError`] if the graph cannot be built.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add(
            "chan",
            checked("chan", Channel::new(CHANNEL_WIDTH_HZ, CHANNEL_RATE_HZ))?,
        )
        .set("chan.wide", Value::Hz(CHANNEL_WIDTH_HZ))
        .add("carrier", checked("carrier", AmCarrier::new(2000.0))?)
        .add("ddm", IlsDdm::default())
        .add("marker", MarkerTones::default())
        .add("keyer", checked("keyer", ToneKeyer::new(1020.0))?)
        .add("morse", MorseDecoder::default())
        .add(
            "lowpass",
            checked(
                "lowpass",
                BiquadComponent::new(BiquadShape::Lowpass, 3000.0, 0.707),
            )?,
        )
        .add(
            "highpass",
            checked(
                "highpass",
                BiquadComponent::new(BiquadShape::Highpass, 300.0, 0.707),
            )?,
        )
        .add(
            "highpass2",
            checked(
                "highpass2",
                BiquadComponent::new(BiquadShape::Highpass, 300.0, 0.707),
            )?,
        )
        .add("audio", checked("audio", AudioResampler::new(48e3))?)
        .add("report", IlsReport::default())
        .input("chan")
        .connect("chan", "carrier")
        .connect("carrier.am", "ddm")
        .connect("carrier.am", "marker")
        .connect("carrier.am", "keyer")
        .connect("keyer", "morse")
        .connect("carrier.am", "lowpass")
        .connect("lowpass", "highpass")
        .connect("highpass", "highpass2")
        .connect("highpass2", "audio")
        .connect("ddm", "report.fix")
        .connect("carrier.info", "report.carrier")
        .connect("morse", "report.ident")
        .connect("marker.state", "report.marker")
        .connect("marker.passage", "report.passage")
        .output("audio")
        .output("morse")
        .output("marker.passage")
        .clock(0.1)
        .status_from("report")
        .reconfigure_on_retune()
        .catalog(catalog)
        .build()
}

pub const STACK_AVERAGE: Setting<field::Duration> =
    Setting::new("average", "Averaging", "s", 0.5).range(0.1, 10.0, 0.0);
pub const CARRIER_THRESHOLD: Setting<field::F64> =
    Setting::new("carrier_threshold", "Carrier threshold", "dB-Hz", 30.0)
        .range(0.0, 90.0, 0.0)
        .section("Advanced");
pub const GEN_DDM: Setting<field::F64> = Setting::new("gen_ddm", "Test signal: DDM", "", 0.05)
    .range(-0.4, 0.4, 0.0)
    .section("Test signal")
    .help("Positive: 90 Hz predominates.")
    .advanced();
pub const GEN_IDENT: Setting<field::Text> =
    Setting::new("gen_ident", "Test signal: identification", "", "ISYD")
        .section("Test signal")
        .advanced();
pub const GEN_WPM: Setting<field::F64> =
    Setting::new("gen_wpm", "Test signal: Morse speed", "wpm", 10.0)
        .range(3.0, 20.0, 0.0)
        .section("Test signal")
        .advanced();
pub const GEN_PAUSE: Setting<field::Duration> =
    Setting::new("gen_pause", "Test signal: pause between idents", "s", 2.0)
        .range(0.5, 30.0, 0.0)
        .section("Test signal")
        .advanced();
pub const GEN_MARKER: Setting<field::Choice<GenMarker>> =
    Setting::choice("gen_marker", "Test signal: marker", GenMarker::Outer)
        .section("Test signal")
        .help("The marker beacon of the part `marker`.")
        .advanced();
pub const GEN_MARKER_KEYING: Setting<field::Choice<GenMarkerKeying>> =
    Setting::choice(
        "gen_marker_keying",
        "Test signal: marker keying",
        GenMarkerKeying::Standard,
    )
    .section("Test signal")
    .advanced();
pub const GEN_MARKER_TONE: Setting<field::Hz> =
    Setting::new("gen_marker_tone", "Test signal: marker tone", "Hz", 0.0)
        .range(0.0, 4000.0, 0.0)
        .section("Test signal")
        .help("0 sends the tone of the marker.")
        .advanced();
pub const GEN_MARKER_DEPTH: Setting<field::F64> =
    Setting::new("gen_marker_depth", "Test signal: marker depth", "%", 95.0)
        .range(0.0, 100.0, 0.0)
        .section("Test signal")
        .advanced();
pub const GEN_MARKER_START: Setting<field::Duration> =
    Setting::new("gen_marker_start", "Test signal: marker keyed from", "s", 0.0)
        .range(0.0, 3600.0, 0.0)
        .section("Test signal")
        .advanced();
pub const GEN_MARKER_STOP: Setting<field::Duration> =
    Setting::new("gen_marker_stop", "Test signal: marker keyed until", "s", 0.0)
        .range(0.0, 3600.0, 0.0)
        .section("Test signal")
        .help("0 keys without end.")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the ILS stack.
    pub enum IlsSetting {
        /// The part read.
        Part = PART_SETTING,
        /// How long the depths are averaged over.
        Average = STACK_AVERAGE,
        /// Hearings that confirm an identification.
        Confirm = CONFIRM,
        /// The least C/N0 of a carrier.
        CarrierThreshold = CARRIER_THRESHOLD,
        /// The DDM of the test signal.
        GenDdm = GEN_DDM,
        /// The identification of the test signal.
        GenIdent = GEN_IDENT,
        /// The Morse speed of the test signal.
        GenWpm = GEN_WPM,
        /// The pause between identifications of the test signal.
        GenPause = GEN_PAUSE,
        /// The marker of the test signal.
        GenMarker = GEN_MARKER,
        /// The keying of the marker test signal.
        GenMarkerKeying = GEN_MARKER_KEYING,
        /// The tone of the marker test signal.
        GenMarkerTone = GEN_MARKER_TONE,
        /// The depth of the marker test signal.
        GenMarkerDepth = GEN_MARKER_DEPTH,
        /// When the marker test signal starts keying.
        GenMarkerStart = GEN_MARKER_START,
        /// When the marker test signal stops keying.
        GenMarkerStop = GEN_MARKER_STOP,
    }
}

fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut values = cfg.settings.clone();
    if let Some(part) = cfg.submode.as_deref().and_then(PartChoice::of_submode) {
        values
            .set(IlsSetting::Part.id(), &Value::Choice(part.index()))
            .map_err(|error| GraphError::Setting {
                key: PART_SETTING.key().into(),
                error,
            })?;
    }
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "ILS needs a sample rate of at least 32 kHz")
        .route("part", &["report.part"])?
        .route("average", &["ddm.average"])?
        .route("confirm", &["report.confirm"])?
        .route("carrier_threshold", &["carrier.threshold"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .on_retune(sdrkit_graph::CommandTarget::of("report", "clear"))?
        .with_settings(&values)
        .build()
}

/// The ILS receiver: a submode (`localizer`, `glideslope`, `marker`) fixes the part.
pub fn receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(stack_graph(cfg)?))
}

/// The ILS test signal: a localizer (with identification) or a glide path, carrier at 0 Hz and unit mean power. The marker part
/// has its own ([`MarkerGenerator`]).
pub struct IlsGenerator {
    rate: f64,
    part: IlsPart,
    ddm: f64,
    key: IdentEnvelope,
    env: Vec<f32>,
    n: u64,
    phase0: f64,
}

impl IlsGenerator {
    /// `part` with `ddm` (positive: 90 Hz predominates) and identification `ident`.
    pub fn new(
        rate: f64,
        part: IlsPart,
        ddm: f64,
        ident: &str,
        wpm: f64,
        pause_s: f64,
        seed: u64,
    ) -> Self {
        let ident = if part == IlsPart::Localizer {
            ident
        } else {
            ""
        };
        IlsGenerator {
            rate,
            part,
            ddm,
            // Edges of 5 ms, so the keying does not click.
            key: IdentEnvelope::new(ident, wpm, rate, 0.3, pause_s, 0.005),
            env: Vec::new(),
            n: 0,
            phase0: sdrkit_algo_sim::rng::Rng::new(seed).uniform() * TAU,
        }
    }
    /// From the stack's settings: the submode (or `part`) picks localizer or glide path.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        IlsGenerator::new(
            cfg.rate_hz,
            gen_part(cfg),
            s.get(&GEN_DDM),
            s.get(&GEN_IDENT),
            s.get(&GEN_WPM),
            s.get(&GEN_PAUSE),
            cfg.seed,
        )
    }
}

impl Generator for IlsGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(CHANNEL_WIDTH_HZ))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let half = self.part.nominal_sdm().unwrap_or(0.0) / 2.0;
        let (a90, a150) = (half + self.ddm / 2.0, half - self.ddm / 2.0);
        // Unit mean power: the carrier and its two tones.
        let a = 1.0 / (1.0 + (a90 * a90 + a150 * a150) / 2.0).sqrt();
        let (s0, c0) = self.phase0.sin_cos();
        self.env.clear();
        self.env.extend(self.key.next(out.len()));
        for (o, &key) in out.iter_mut().zip(&self.env) {
            let t = self.n as f64 / self.rate;
            let e = 1.0
                + a90 * (TAU * ((90.0 * t) % 1.0)).cos()
                + a150 * (TAU * ((150.0 * t) % 1.0)).cos()
                + 0.1 * key as f64 * (TAU * ((1020.0 * t) % 1.0)).cos();
            *o = Complex32::new((a * e * c0) as f32, (a * e * s0) as f32);
            self.n += 1;
        }
        out.len()
    }
}

/// The part of the test signal: the submode, or else the `part` setting; by frequency it is a localizer.
fn gen_part(cfg: &GenConfig) -> IlsPart {
    let choice = cfg
        .submode
        .as_deref()
        .and_then(PartChoice::of_submode)
        .unwrap_or_else(|| cfg.settings.get(&PART_SETTING));
    IlsPart::pick(choice, 0.0)
}

/// Makes the ILS generator: the marker test signal for the marker part.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    if gen_part(cfg) == IlsPart::Marker {
        let s = &cfg.settings;
        return Ok(Box::new(MarkerGenerator::new(
            cfg.rate_hz,
            s.get(&GEN_MARKER),
            s.get(&GEN_MARKER_KEYING),
            s.get(&GEN_MARKER_TONE),
            s.get(&GEN_MARKER_DEPTH) / 100.0,
            s.get(&GEN_MARKER_START),
            s.get(&GEN_MARKER_STOP),
            cfg.seed,
        )));
    }
    Ok(Box::new(IlsGenerator::from_config(cfg)))
}
