//! VOR and Doppler VOR: the radial from the phase difference of the two 30 Hz signals, the course deviation for a selected course,
//! the modulation checks and the Morse identification.
//! Standard: ICAO Annex 10 Volume I, 3.3.

use crate::carrier::{AmCarrier, CarrierInfo};
use crate::fields::{carrier_fields, put_carrier_status, put_idents};
use crate::ident::IdentEnvelope;
use crate::keyer::ToneKeyer;
use crate::morse::{IdentTracker, MorseDecoder, MorseWord};
use crate::pairing::DmeChannel;
use crate::typed::{CarrierFields, IdentFields, VorFields, IDENTS};
use num_complex::Complex64;
use sdrkit_algo_dsp::decim::Decimator;
use sdrkit_algo_dsp::goertzel::Goertzel;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_components::dsp::{AudioResampler, BiquadComponent, Channel};
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, GenConfig, Generator,
    LockState, Receiver, Record, Results, Setting, SettingError, SettingId, SettingSet, SignalInfo,
    StackConfig, Value, Values,
};
use sdrkit_graph::components::BiquadShape;
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, DecoderCommand, Format, Graph,
    GraphBuilder, GraphError, Io, Item, Kind, Port, RateFrom, StackGraph, Timed,
};
use sdrkit_util_geo::position::{wrap180, wrap360};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "vor";
/// The 30 Hz navigation signals.
pub const NAV_HZ: f64 = 30.0;
/// The subcarrier.
pub const SUBCARRIER_HZ: f64 = 9960.0;
/// The subcarrier's deviation.
pub const DEVIATION_HZ: f64 = 480.0;
/// The least radio rate.
pub const MIN_RATE_HZ: f64 = 64e3;
/// The channel's rate.
pub const CHANNEL_RATE_HZ: f64 = 32e3;
/// The channel's width.
pub const CHANNEL_WIDTH_HZ: f64 = 25e3;

/// One bearing measurement, averaged.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VorFix {
    /// The radial: bearing from the station, degrees magnetic as the station is aligned, 0 to 360.
    pub radial_deg: f64,
    /// The radial of this period alone.
    pub raw_deg: f64,
    /// How steady the bearing is: the length of the averaged unit vector, 0 (random) to 1 (steady).
    pub steadiness: f32,
    /// Depth of the 30 Hz AM, 0..1.
    pub am30_depth: f32,
    /// Depth of the 9960 Hz subcarrier, 0..1.
    pub subcarrier_depth: f32,
    /// Peak deviation of the subcarrier's 30 Hz FM, Hz.
    pub deviation_hz: f32,
    /// The signal looks like a VOR: every depth and the deviation plausible, and the bearing steady.
    pub valid: bool,
}

impl Item for VorFix {
    const KIND: &'static str = "vor.fix";
}

pub const AVERAGE: Setting<field::Duration> = Setting::new("average", "Averaging", "s", 1.0)
    .range(0.1, 30.0, 0.0)
    .help("How long the bearing is averaged over.");
pub const PERIOD: Setting<field::Duration> =
    Setting::new("period", "Measurement", "s", 0.1).range(0.05, 1.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`VorBearing`].
    pub enum VorBearingSetting {
        /// How long the bearing is averaged over.
        Average = AVERAGE,
        /// The time of one measurement.
        Period = PERIOD,
    }
}

/// The bearing measurement (`avnav.vor_bearing`, real -> [`VorFix`]). The 30 Hz AM is measured on the input. The subcarrier is
/// mixed to 0 Hz, decimated and put through a discriminator, and its 30 Hz FM is measured there. Both measurements cover the
/// same span of the signal, so their phase difference is the radial.
pub struct VorBearing {
    average: f64,
    period_s: f64,
    rate: f64,
    var: Option<Goertzel>,
    nco: Nco,
    mixed: Vec<Complex32>,
    dec: Option<Decimator<Complex32>>,
    decd: Vec<Complex32>,
    dec_rate: f64,
    last: Complex32,
    refc: Option<Goertzel>,
    mag: f64,
    nmag: usize,
    correction: Complex64,
    vq: VecDeque<(f64, Complex64)>,
    rq: VecDeque<(Complex64, f64)>,
    avg: Complex64,
    am30: f64,
    sc: f64,
    dev: f64,
    started: bool,
}

impl Default for VorBearing {
    fn default() -> Self {
        VorBearing {
            average: AVERAGE.default,
            period_s: PERIOD.default,
            rate: 0.0,
            var: None,
            nco: Nco::new(1.0, 0.0),
            mixed: Vec::new(),
            dec: None,
            decd: Vec::new(),
            dec_rate: 0.0,
            last: Complex32::default(),
            refc: None,
            mag: 0.0,
            nmag: 0,
            correction: Complex64::new(1.0, 0.0),
            vq: VecDeque::with_capacity(8),
            rq: VecDeque::with_capacity(8),
            avg: Complex64::new(0.0, 0.0),
            am30: 0.0,
            sc: 0.0,
            dev: 0.0,
            started: false,
        }
    }
}

impl VorBearing {
    /// One measurement: `v` and `r` are the amplitude phasors of the 30 Hz AM and of the 30 Hz FM (Hz), `sc` the subcarrier depth.
    fn emit(
        &mut self,
        time: f64,
        v: Complex64,
        r: Complex64,
        sc: f64,
        out: &mut Vec<Timed<VorFix>>,
    ) {
        let am30 = v.norm();
        let dev = r.norm();
        let r = r * self.correction;
        // Datum minus variable: the variable 30 Hz lags the datum by the radial.
        let raw = wrap360((r.arg() - v.arg()).to_degrees());
        let unit = Complex64::from_polar(1.0, raw.to_radians());
        let a = (self.period_s / self.average.max(self.period_s)).min(1.0);
        if !self.started {
            self.avg = unit;
            self.am30 = am30;
            self.sc = sc;
            self.dev = dev;
            self.started = true;
        } else {
            self.avg += a * (unit - self.avg);
            self.am30 += a * (am30 - self.am30);
            self.sc += a * (sc - self.sc);
            self.dev += a * (dev - self.dev);
        }
        let steadiness = self.avg.norm().min(1.0);
        let valid = (0.1..0.6).contains(&self.am30)
            && (0.1..0.6).contains(&self.sc)
            && (300.0..700.0).contains(&self.dev)
            && steadiness > 0.8;
        out.push(Timed {
            time,
            value: VorFix {
                radial_deg: wrap360(self.avg.arg().to_degrees()),
                raw_deg: raw,
                steadiness: steadiness as f32,
                am30_depth: self.am30 as f32,
                subcarrier_depth: self.sc as f32,
                deviation_hz: self.dev as f32,
                valid,
            },
        });
    }
}

impl Component for VorBearing {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.vor_bearing",
            vec![Port::real("in")],
            vec![Port::typed::<VorFix>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(VorBearingSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate < 2.5 * (SUBCARRIER_HZ + DEVIATION_HZ + 1000.0) {
            return Err(ConfigError::RateTooLow {
                need: 2.5 * (SUBCARRIER_HZ + DEVIATION_HZ + 1000.0),
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.nco = Nco::new(f.rate, -SUBCARRIER_HZ);
        let factor = ((f.rate / 4000.0).floor() as usize).max(1);
        let dec = Decimator::new(f.rate, factor, 800.0, 60.0);
        self.dec_rate = dec.output_rate();
        // The datum path is late by the decimator's output delay and half a discriminator step.
        let late = dec.output_delay() / f.rate + 0.5 / self.dec_rate;
        self.correction = Complex64::from_polar(1.0, TAU * NAV_HZ * late);
        self.dec = Some(dec);
        // Both measurements span the same samples: a whole number of decimated samples, and `factor` input samples for each.
        let nr = ((self.period_s * self.dec_rate).round() as usize).max(1);
        self.var = Some(Goertzel::with_rate(f.rate, NAV_HZ, nr * factor)?);
        self.refc = Some(Goertzel::with_rate(self.dec_rate, NAV_HZ, nr)?);
        self.mag = 0.0;
        self.nmag = 0;
        self.last = Complex32::default();
        self.vq.clear();
        self.rq.clear();
        self.started = false;
        Ok(vec![Format {
            rate: 0.0,
            ..f.with_kind(Kind::typed::<VorFix>())
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let x = io.real(0);
        // The variable (AM) path.
        if let Some(g) = &mut self.var {
            for (i, &s) in x.iter().enumerate() {
                if let Some(v) = g.push(s) {
                    let t = (first + i as u64 + 1) as f64 / self.rate;
                    if self.vq.len() < 64 {
                        self.vq
                            .push_back((t, Complex64::new(v.re as f64, v.im as f64)));
                    }
                }
            }
        }
        // The datum (subcarrier FM) path.
        self.mixed.resize(x.len(), Complex32::default());
        self.nco.mix_real(x, &mut self.mixed);
        self.decd.clear();
        if let Some(d) = &mut self.dec {
            d.process(&self.mixed, &mut self.decd);
        }
        let k = self.dec_rate / TAU;
        if let Some(g) = &mut self.refc {
            for &z in &self.decd {
                let d = z * self.last.conj();
                self.last = z;
                let f = if d.norm_sqr() > 0.0 {
                    (d.im as f64).atan2(d.re as f64) * k
                } else {
                    0.0
                };
                self.mag += z.norm() as f64;
                self.nmag += 1;
                if let Some(r) = g.push(f as f32) {
                    let sc = 2.0 * self.mag / self.nmag as f64;
                    self.mag = 0.0;
                    self.nmag = 0;
                    if self.rq.len() < 64 {
                        self.rq
                            .push_back((Complex64::new(r.re as f64, r.im as f64), sc));
                    }
                }
            }
        }
        let out = io.items_out::<VorFix>(0);
        while let (Some(&(t, v)), Some(&(r, sc))) = (self.vq.front(), self.rq.front()) {
            self.vq.pop_front();
            self.rq.pop_front();
            self.emit(t, v, r, sc, out);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match VorBearingSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            VorBearingSetting::Average => {
                self.average = AVERAGE.read(id, v)?;
                Ok(Applied::Live)
            }
            VorBearingSetting::Period => {
                self.period_s = PERIOD.read(id, v)?;
                Ok(Applied::Reformat)
            }
        }
    }
    fn values(&self) -> Values {
        values_of([
            (VorBearingSetting::Average, Value::Duration(self.average)),
            (VorBearingSetting::Period, Value::Duration(self.period_s)),
        ])
    }
}

sdrkit_core::choice_enum! {
    /// The TO/FROM flag of a course deviation indicator.
    pub enum ToFrom {
        /// The selected course leads to the station.
        To = ("to", "TO"),
        /// The selected course leads away from the station.
        From = ("from", "FROM"),
        /// No valid signal: the flag is out.
        Off = ("off", "OFF"),
    }
}

/// The course deviation indicator's reading for a radial and a selected course.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cdi {
    /// [`ToFrom::To`] or [`ToFrom::From`].
    pub to_from: ToFrom,
    /// Degrees off the selected course. Positive: the course is to the right (fly right).
    pub deviation_deg: f64,
    /// The same in dots, 2° a dot, limited to ±5 (full scale ±10°).
    pub dots: f64,
}

/// The CDI for `radial` (from the station) and the selected course `obs`.
pub fn cdi(radial: f64, obs: f64) -> Cdi {
    let d = wrap180(obs - radial);
    let (to_from, dev) = if d.abs() <= 90.0 {
        (ToFrom::From, d)
    } else {
        (ToFrom::To, -wrap180(d - 180.0))
    };
    Cdi {
        to_from,
        deviation_deg: dev,
        dots: (dev / 2.0).clamp(-5.0, 5.0),
    }
}

fn new_published() -> Published {
    crate::typed::store(
        &[VorFields::SHAPE, IdentFields::SHAPE, CarrierFields::SHAPE],
        &[&IDENTS],
    )
}

pub const OBS: Setting<field::F64> = Setting::new("obs", "Course (OBS)", "°", 0.0)
    .range(0.0, 360.0, 1.0)
    .help("The selected course: the deviation and the TO/FROM flag are shown for it.");
pub const CONFIRM: Setting<field::I64> = Setting::new("confirm", "Hearings to confirm", "", 2)
    .range(1.0, 10.0, 1.0)
    .help("How many times an identification must be heard before it is shown.");

sdrkit_core::settings! {
    /// The settings of [`VorReport`].
    pub enum VorReportSetting {
        /// The selected course.
        Obs = OBS,
        /// Hearings that confirm an identification.
        Confirm = CONFIRM,
    }
}

/// The VOR report (`avnav.vor_report`): fixes, the carrier and identification words in, results out. Its command `clear`
/// forgets the station.
pub struct VorReport {
    obs: f64,
    tracker: IdentTracker,
    fix: Option<VorFix>,
    fixes: u64,
    wpm: Option<f32>,
    carrier: Option<CarrierInfo>,
    published: Published,
    tuned: f64,
    dme_channel: Option<String>,
    summary: String,
}

impl Default for VorReport {
    fn default() -> Self {
        VorReport {
            obs: OBS.default,
            tracker: IdentTracker::new(CONFIRM.default as u32),
            fix: None,
            fixes: 0,
            wpm: None,
            carrier: None,
            published: new_published(),
            tuned: 0.0,
            dme_channel: None,
            summary: String::with_capacity(64),
        }
    }
}

impl VorReport {
    fn clear(&mut self) {
        self.tracker.clear();
        self.fix = None;
        self.fixes = 0;
        self.wpm = None;
        self.carrier = None;
        self.published = new_published();
        self.publish();
    }
    fn publish(&mut self) {
        let p = &mut self.published;
        let c = self.carrier.unwrap_or_default();
        put_idents(p, &self.tracker, self.tracker.confirmed(), self.wpm);
        p.put(&carrier_fields(self.carrier.as_ref(), self.tuned));
        let fix = self.fix.as_ref();
        let valid = c.present && fix.is_some_and(|f| f.valid);
        let radial = fix.filter(|_| valid).map(|f| f.radial_deg);
        let d = radial.map(|r| cdi(r, self.obs));
        let d = d.as_ref();
        p.put(&VorFields {
            radial_deg: radial,
            bearing_to_deg: radial.map(|r| wrap360(r + 180.0)),
            obs_deg: self.obs,
            to_from: d.map_or(ToFrom::Off, |d| d.to_from),
            cdi_deg: d.map(|d| d.deviation_deg),
            cdi_dots: d.map(|d| d.dots),
            am30_pct: fix.map(|f| f.am30_depth as f64 * 100.0),
            subcarrier_pct: fix.map(|f| f.subcarrier_depth as f64 * 100.0),
            deviation_hz: fix.map(|f| Hertz(f.deviation_hz as f64)),
            steadiness: fix.map(|f| f.steadiness as f64),
            fixes: self.fixes as i64,
            tuned_hz: Hertz(self.tuned),
            dme_channel: self.dme_channel.as_deref(),
        });
        self.summary.clear();
        // Writing into a String cannot fail.
        let _ = match (radial, self.tracker.confirmed()) {
            (Some(r), Some(id)) => write!(self.summary, "{id}, radial {r:.0}"),
            (Some(r), None) => write!(self.summary, "Radial {r:.0}"),
            (None, _) if c.present => write!(self.summary, "Carrier, no bearing"),
            _ => write!(self.summary, "Searching"),
        };
        let lock_state = if valid {
            LockState::Locked
        } else if c.present {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        put_carrier_status(
            p,
            lock_state,
            self.carrier.as_ref(),
            CHANNEL_WIDTH_HZ,
            &self.summary,
        );
    }
}

impl Component for VorReport {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "avnav.vor_report",
            vec![
                Port::typed::<VorFix>("fix"),
                Port::typed::<CarrierInfo>("carrier"),
                Port::typed::<MorseWord>("ident"),
            ],
        )
        .with_settings(VorReportSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.tuned = inputs[0].tuned_hz;
        self.dme_channel = DmeChannel::for_vhf(self.tuned / 1e6).map(|c| c.name());
        self.publish();
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let mut changed = false;
        if let Some(f) = io.items::<VorFix>(0).last() {
            self.fix = Some(f.value);
            self.fixes += io.items::<VorFix>(0).len() as u64;
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
        if changed {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match VorReportSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            VorReportSetting::Obs => {
                self.obs = wrap360(OBS.read(id, v)?);
                self.publish();
            }
            VorReportSetting::Confirm => self.tracker.set_confirm(CONFIRM.read(id, v)? as u32),
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (VorReportSetting::Obs, Value::F64(self.obs)),
            (
                VorReportSetting::Confirm,
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
            DecoderCommand::Clear => self.clear(),
        }
        Ok(None)
    }
}

/// The VOR graph: the channel to the carrier stage, then the bearing, the identification keyer and the audio filters, and the
/// report.
///
/// # Errors
///
/// A [`GraphError`] if the graph cannot be built.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add(
            "chan",
            GraphError::checked("chan", Channel::new(CHANNEL_WIDTH_HZ, CHANNEL_RATE_HZ))?,
        )
        .set("chan.wide", Value::Hz(CHANNEL_WIDTH_HZ))
        .add("carrier", GraphError::checked("carrier", AmCarrier::new(2000.0))?)
        .add("bearing", VorBearing::default())
        .add("keyer", GraphError::checked("keyer", ToneKeyer::new(1020.0))?)
        .add("morse", MorseDecoder::default())
        .add(
            "lowpass",
            GraphError::checked(
                "lowpass",
                BiquadComponent::new(BiquadShape::Lowpass, 3000.0, 0.54),
            )?,
        )
        .add(
            "lowpass2",
            GraphError::checked(
                "lowpass2",
                BiquadComponent::new(BiquadShape::Lowpass, 3000.0, 1.31),
            )?,
        )
        .add(
            "highpass",
            GraphError::checked(
                "highpass",
                BiquadComponent::new(BiquadShape::Highpass, 250.0, 0.707),
            )?,
        )
        .add("audio", GraphError::checked("audio", AudioResampler::new(48e3))?)
        .add("report", VorReport::default())
        .input("chan")
        .connect("chan", "carrier")
        .connect("carrier.am", "bearing")
        .connect("carrier.am", "keyer")
        .connect("keyer", "morse")
        .connect("carrier.am", "lowpass")
        .connect("lowpass", "lowpass2")
        .connect("lowpass2", "highpass")
        .connect("highpass", "audio")
        .connect("bearing", "report.fix")
        .connect("carrier.info", "report.carrier")
        .connect("morse", "report.ident")
        .output("audio")
        .output("morse")
        .clock(0.1)
        .status_from("report")
        .reconfigure_on_retune()
        .catalog(catalog)
        .build()
}

pub const STACK_AVERAGE: Setting<field::Duration> = Setting::new("average", "Averaging", "s", 1.0)
    .range(0.1, 30.0, 0.0)
    .help("How long the bearing is averaged over.");
pub const CARRIER_THRESHOLD: Setting<field::F64> =
    Setting::new("carrier_threshold", "Carrier threshold", "dB-Hz", 30.0)
        .range(0.0, 90.0, 0.0)
        .section("Advanced");

sdrkit_core::choice_enum! {
    /// The station of the test signal.
    pub enum VorKind {
        /// A conventional VOR.
        Conventional = ("conventional", "Conventional VOR"),
        /// A Doppler VOR.
        Doppler = ("doppler", "Doppler VOR"),
    }
}

pub const GEN_KIND: Setting<field::Choice<VorKind>> =
    Setting::choice("gen_kind", "Test signal: station", VorKind::Conventional)
        .section("Test signal")
        .advanced();
pub const GEN_RADIAL: Setting<field::F64> =
    Setting::new("gen_radial", "Test signal: radial", "°", 45.0)
        .range(0.0, 360.0, 0.0)
        .section("Test signal")
        .advanced();
pub const GEN_IDENT: Setting<field::Text> =
    Setting::new("gen_ident", "Test signal: identification", "", "SYD")
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
pub const GEN_ROTATE: Setting<field::F64> =
    Setting::new("gen_rotate", "Test signal: radial change", "°/s", 0.0)
        .range(-30.0, 30.0, 0.0)
        .section("Test signal")
        .help("The radial moves this fast (an aircraft flying round the station).")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the VOR stack.
    pub enum VorSetting {
        /// The selected course.
        Obs = OBS,
        /// How long the bearing is averaged over.
        Average = STACK_AVERAGE,
        /// Hearings that confirm an identification.
        Confirm = CONFIRM,
        /// The least C/N0 of a carrier.
        CarrierThreshold = CARRIER_THRESHOLD,
        /// The station of the test signal.
        GenKind = GEN_KIND,
        /// The radial of the test signal.
        GenRadial = GEN_RADIAL,
        /// The identification of the test signal.
        GenIdent = GEN_IDENT,
        /// The Morse speed of the test signal.
        GenWpm = GEN_WPM,
        /// The pause between identifications of the test signal.
        GenPause = GEN_PAUSE,
        /// How fast the radial of the test signal moves.
        GenRotate = GEN_ROTATE,
    }
}

fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "VOR needs a sample rate of at least 64 kHz")
        .route("obs", &["report.obs"])?
        .route("average", &["bearing.average"])?
        .route("confirm", &["report.confirm"])?
        .route("carrier_threshold", &["carrier.threshold"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .on_retune(sdrkit_graph::CommandTarget::of("report", "clear"))?
        .with_settings(&cfg.settings)
        .build()
}

/// The VOR receiver.
pub fn receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(stack_graph(cfg)?))
}

/// What a VOR test signal sends.
#[derive(Clone, Debug)]
pub struct VorSignal {
    /// Doppler VOR (datum AM, variable FM from moving sideband antennas) instead of conventional.
    pub doppler: bool,
    /// The radial at the start, degrees.
    pub radial_deg: f64,
    /// Degrees a second the radial moves.
    pub rotate_deg_s: f64,
    /// Identification.
    pub ident: String,
    /// Morse speed.
    pub wpm: f64,
    /// Pause between identifications, seconds.
    pub pause_s: f64,
}

impl Default for VorSignal {
    fn default() -> Self {
        VorSignal {
            doppler: false,
            radial_deg: 45.0,
            rotate_deg_s: 0.0,
            ident: "SYD".into(),
            wpm: 10.0,
            pause_s: 2.0,
        }
    }
}

/// The VOR test signal, at baseband with the carrier at 0 Hz and unit mean power.
pub struct VorGenerator {
    rate: f64,
    sig: VorSignal,
    n: u64,
    key: IdentEnvelope,
    env: Vec<f32>,
    phase0: f64,
}

impl VorGenerator {
    /// At `rate_hz`.
    pub fn new(rate_hz: f64, sig: VorSignal, seed: u64) -> Self {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
        VorGenerator {
            rate: rate_hz,
            // Edges of 5 ms, so the keying does not click.
            key: IdentEnvelope::new(&sig.ident, sig.wpm, rate_hz, 0.3, sig.pause_s, 0.005),
            sig,
            n: 0,
            env: Vec::new(),
            phase0: rng.uniform() * TAU,
        }
    }
    /// From the stack's settings.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let sig = VorSignal {
            doppler: s.get(&GEN_KIND) == VorKind::Doppler,
            radial_deg: s.get(&GEN_RADIAL),
            rotate_deg_s: s.get(&GEN_ROTATE),
            ident: s.get(&GEN_IDENT).to_owned(),
            wpm: s.get(&GEN_WPM),
            pause_s: s.get(&GEN_PAUSE),
        };
        VorGenerator::new(cfg.rate_hz, sig, cfg.seed)
    }
}

impl Generator for VorGenerator {
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
        let rate = self.rate;
        let beta = DEVIATION_HZ / NAV_HZ;
        let (c0s, c0c) = self.phase0.sin_cos();
        self.env.clear();
        self.env.extend(self.key.next(out.len()));
        // The mean power of the carrier with its 30 Hz AM and the subcarrier (each 30 % depth): 1 + 0.3^2 / 2 + 0.3^2 / 2.
        let a = 1.0 / (1.0f64 + 0.09).sqrt();
        for (o, &key) in out.iter_mut().zip(&self.env) {
            let t = self.n as f64 / rate;
            let theta = (self.sig.radial_deg + self.sig.rotate_deg_s * t).to_radians();
            let w30 = TAU * ((NAV_HZ * t) % 1.0);
            let wsc = TAU * ((SUBCARRIER_HZ * t) % 1.0);
            let id = 0.1 * key as f64 * (TAU * ((1020.0 * t) % 1.0)).cos();
            let (i, q) = if self.sig.doppler {
                // Carrier with the 30 Hz datum as AM. The sidebands come from antennas on opposite sides of a circle turning
                // 30 times a second, so each is Doppler shifted by ±480 Hz with the phase of the bearing.
                let env = 1.0 + 0.3 * w30.cos() + id;
                let pm = beta * (w30 + theta).sin();
                let usb = Complex64::from_polar(0.16, wsc + pm);
                let lsb = Complex64::from_polar(0.14, -(wsc + pm));
                (env + usb.re + lsb.re, usb.im + lsb.im)
            } else {
                let env =
                    1.0 + 0.3 * (w30 - theta).cos() + 0.3 * (wsc + beta * w30.sin()).cos() + id;
                (env, 0.0)
            };
            *o = Complex32::new(
                (a * (i * c0c - q * c0s)) as f32,
                (a * (i * c0s + q * c0c)) as f32,
            );
            self.n += 1;
        }
        out.len()
    }
}

/// Makes the VOR generator.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(VorGenerator::from_config(cfg)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cdi_reads_to_and_from() {
        let c = cdi(90.0, 90.0);
        assert_eq!((c.to_from, c.deviation_deg), (ToFrom::From, 0.0));
        let c = cdi(80.0, 90.0);
        assert_eq!(c.to_from, ToFrom::From);
        assert!((c.deviation_deg - 10.0).abs() < 1e-9 && c.dots == 5.0);
        let c = cdi(270.0, 90.0);
        assert_eq!((c.to_from, c.deviation_deg), (ToFrom::To, 0.0));
        // Inbound on course 090 (station east), aircraft north of the track: radial 280, the station is to the right.
        let c = cdi(280.0, 90.0);
        assert_eq!(c.to_from, ToFrom::To);
        assert!(c.deviation_deg > 0.0, "{c:?}");
    }
}
